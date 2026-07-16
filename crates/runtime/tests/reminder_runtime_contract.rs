use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    UsageProviderId,
};
use tokenmaster_engine::{RefreshOutcome, WriterLease};
use tokenmaster_platform::PowerLifecycleEvent;
use tokenmaster_runtime::{
    BenefitReminderFailure, BenefitReminderRetryMode, BenefitReminderRuntime,
    BenefitReminderRuntimeConfig, BenefitReminderRuntimePhase, RuntimeErrorCode,
    RuntimeWriterLease,
};
use tokenmaster_store::UsageStore;

fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_millis(),
    )
    .expect("wall clock")
}

fn scope() -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new("acct_private").expect("account"),
        None,
    )
}

fn seed(path: &std::path::Path, observed_at_ms: i64, expiry_at_ms: i64) {
    let lot = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([7; 32]),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 1,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(observed_at_ms - 1_000),
        expiry: BenefitExpiry::exact_utc(expiry_at_ms).expect("expiry"),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot");
    let observation = BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope(),
        observation_id: BenefitObservationId::from_bytes([1; 32]),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots: vec![lot],
    })
    .expect("observation");
    UsageStore::open(path)
        .expect("store")
        .apply_benefit_observation(&observation)
        .expect("seed benefit");
}

fn wait_completion(runtime: &BenefitReminderRuntime) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if runtime.try_completion().expect("completion").is_some() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "reminder completion timed out"
        );
        std::thread::yield_now();
    }
}

#[test]
fn startup_replays_unacknowledged_event_after_restart_and_ack_stops_replay() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("reminder.sqlite3");
    let observed_at_ms = now_ms();
    seed(&path, observed_at_ms, observed_at_ms + 30 * 60 * 1_000);

    let mut runtime = BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(path.clone()).expect("config"),
    )
    .expect("runtime");
    wait_completion(&runtime);
    let snapshot = runtime.snapshot().expect("snapshot");
    assert_eq!(snapshot.phase(), BenefitReminderRuntimePhase::Running);
    assert_eq!(
        snapshot.refresh().outcome(),
        Some(RefreshOutcome::Completed)
    );
    assert_eq!(snapshot.refresh().examined_count(), 5);
    assert_eq!(snapshot.refresh().delivery_count(), 1);
    assert_eq!(snapshot.refresh().pending_due_count(), 0);
    assert_eq!(snapshot.refresh().nearest_due_at_ms(), None);
    let batch = runtime
        .take_notifications()
        .expect("take notifications")
        .expect("notification batch");
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].kind(), BenefitKind::BankedRateLimitReset);
    assert_eq!(batch[0].label_key(), "benefit.codex.banked_reset");
    assert!(runtime.take_notifications().expect("empty batch").is_none());
    assert_eq!(
        runtime.shutdown().expect("shutdown"),
        BenefitReminderRuntimePhase::Stopped
    );

    let mut restarted = BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(path).expect("restart config"),
    )
    .expect("restart");
    wait_completion(&restarted);
    assert_eq!(
        restarted
            .snapshot()
            .expect("restart snapshot")
            .refresh()
            .delivery_count(),
        1
    );
    let replayed = restarted
        .take_notifications()
        .expect("restart notifications")
        .expect("replayed notification");
    assert_eq!(replayed.len(), 1);
    assert!(
        restarted
            .release_notifications()
            .expect("release notification")
    );
    assert_eq!(
        restarted
            .take_notifications()
            .expect("retake notification")
            .expect("released notification")
            .len(),
        1
    );
    assert!(restarted.acknowledge_notifications().expect("acknowledge"));
    restarted.shutdown().expect("restart shutdown");

    let mut acknowledged = BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(directory.path().join("reminder.sqlite3"))
            .expect("acknowledged config"),
    )
    .expect("acknowledged restart");
    wait_completion(&acknowledged);
    assert_eq!(
        acknowledged
            .snapshot()
            .expect("acknowledged snapshot")
            .refresh()
            .delivery_count(),
        0
    );
    assert!(
        acknowledged
            .take_notifications()
            .expect("acknowledged notifications")
            .is_none()
    );
    acknowledged.shutdown().expect("acknowledged shutdown");
}

#[test]
fn acknowledgement_contention_keeps_the_leased_batch_retryable() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("reminder-ack-busy.sqlite3");
    let observed_at_ms = now_ms();
    seed(&path, observed_at_ms, observed_at_ms + 30 * 60 * 1_000);
    let mut runtime = BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(path.clone()).expect("config"),
    )
    .expect("runtime");
    wait_completion(&runtime);
    assert_eq!(
        runtime
            .take_notifications()
            .expect("take notification")
            .expect("notification")
            .len(),
        1
    );

    let mut competing = RuntimeWriterLease::new(&path).expect("competing lease");
    let guard = competing.try_acquire().expect("hold competing lease");
    let error = runtime
        .acknowledge_notifications()
        .expect_err("contended acknowledgement");
    assert_eq!(error.code(), RuntimeErrorCode::Busy);
    drop(guard);
    assert!(
        runtime
            .acknowledge_notifications()
            .expect("retry acknowledgement")
    );
    wait_completion(&runtime);
    runtime.shutdown().expect("shutdown");
}

#[test]
fn hints_coalesce_and_resume_forces_one_recovery_pass() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("reminder-hints.sqlite3");
    let observed_at_ms = now_ms();
    seed(&path, observed_at_ms, observed_at_ms + 5 * 60 * 60 * 1_000);
    let mut runtime =
        BenefitReminderRuntime::start(BenefitReminderRuntimeConfig::new(path).expect("config"))
            .expect("runtime");
    wait_completion(&runtime);
    let first = runtime
        .take_notifications()
        .expect("notifications")
        .expect("first notification");
    assert_eq!(first.len(), 1);
    assert!(runtime.acknowledge_notifications().expect("acknowledge"));
    for _ in 0..10_000 {
        runtime.notify_inventory_changed().expect("inventory hint");
        runtime.notify_profile_changed().expect("profile hint");
        runtime.notify_clock_changed().expect("clock hint");
    }
    let pending = runtime.snapshot().expect("coalesced snapshot");
    assert!(pending.schedule().reconciliation_pending());
    assert!(pending.worker().pending_count() <= 1);

    assert_eq!(
        runtime
            .apply_power_event(PowerLifecycleEvent::Suspend)
            .expect("suspend"),
        BenefitReminderRuntimePhase::Paused
    );
    assert_eq!(
        runtime
            .apply_power_event(PowerLifecycleEvent::Resume)
            .expect("resume"),
        BenefitReminderRuntimePhase::Running
    );
    wait_completion(&runtime);
    assert_eq!(
        runtime.shutdown().expect("shutdown"),
        BenefitReminderRuntimePhase::Stopped
    );
}

#[test]
fn writer_contention_opens_no_sqlite_and_uses_accelerated_retry() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("contended.sqlite3");
    let mut competing = RuntimeWriterLease::new(&path).expect("competing lease");
    let guard = competing.try_acquire().expect("hold competing lease");
    let mut runtime = BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(path.clone()).expect("config"),
    )
    .expect("runtime");
    wait_completion(&runtime);
    let snapshot = runtime.snapshot().expect("snapshot");
    assert_eq!(
        snapshot.refresh().failure(),
        Some(BenefitReminderFailure::Busy)
    );
    assert_eq!(
        snapshot.refresh().retry_mode(),
        BenefitReminderRetryMode::Accelerated
    );
    assert!(!path.exists(), "contended pass must not open SQLite");
    drop(guard);
    runtime.reconcile_now().expect("retry hint");
    wait_completion(&runtime);
    assert_eq!(
        runtime.snapshot().expect("recovered").refresh().outcome(),
        Some(RefreshOutcome::Completed)
    );
    assert!(path.exists());
    runtime.shutdown().expect("shutdown");
}

#[test]
fn reminder_store_fault_leaves_live_usage_runtime_unchanged() {
    let source_root = TempDir::new().expect("usage source root");
    let archive_root = TempDir::new().expect("archive root");
    let configured = [ConfiguredCodexRoot::new(source_root.path(), None, true)];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("usage discovery request");
    let usage_path = archive_root.path().join("usage.sqlite3");
    let mut usage =
        tokenmaster_runtime::LiveRuntime::start(&usage_path, request).expect("usage runtime");
    let usage_deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if usage.try_completion().expect("usage completion").is_some() {
            break;
        }
        assert!(
            std::time::Instant::now() < usage_deadline,
            "usage completion timed out"
        );
        std::thread::yield_now();
    }
    let usage_before = usage.snapshot().expect("usage before").engine();

    let invalid_path = archive_root.path().join("invalid-reminder.sqlite3");
    std::fs::write(&invalid_path, b"not a sqlite archive").expect("invalid archive");
    let mut reminder = BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(invalid_path).expect("reminder config"),
    )
    .expect("reminder runtime");
    wait_completion(&reminder);
    assert_eq!(
        reminder
            .snapshot()
            .expect("reminder fault")
            .refresh()
            .failure(),
        Some(BenefitReminderFailure::StoreUnavailable)
    );
    assert_eq!(
        usage.snapshot().expect("usage after").engine(),
        usage_before
    );
    reminder.shutdown().expect("reminder shutdown");
    usage.shutdown().expect("usage shutdown");
}
