use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopInAppNotification, DesktopInAppNotificationBatch, DesktopNotificationKind,
    DesktopNotificationPresentationReceipt,
};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    UsageProviderId,
};
use tokenmaster_runtime::{
    BenefitReminderRuntime, BenefitReminderRuntimeConfig, BenefitReminderRuntimePhase,
};
use tokenmaster_store::UsageStore;

use crate::notification::{
    NotificationPresenter, PresentationFailure, ReminderPresentationCoordinator,
    ReminderPresentationPort, RuntimeReminderPresentationPort,
};

fn one_batch() -> DesktopInAppNotificationBatch {
    DesktopInAppNotificationBatch::new(vec![
        DesktopInAppNotification::new(
            DesktopNotificationKind::BankedRateLimitReset,
            2,
            "benefit.banked_reset",
            3_600,
            1_000,
            3_601_000,
            1_001,
        )
        .expect("valid notification"),
    ])
    .expect("valid batch")
}

struct FakePortState {
    ready: Option<DesktopInAppNotificationBatch>,
    leased: Option<DesktopInAppNotificationBatch>,
    acknowledgements: VecDeque<Result<bool, PresentationFailure>>,
    releases: VecDeque<Result<bool, PresentationFailure>>,
    take_count: usize,
    acknowledge_count: usize,
    release_count: usize,
}

struct FakePort {
    state: Mutex<FakePortState>,
}

struct BlockingReleasePort {
    inner: FakePort,
    release_started: AtomicUsize,
    release_allowed: AtomicUsize,
}

struct PanickingAcknowledgePort {
    inner: FakePort,
    acknowledge_started: AtomicUsize,
}

impl PanickingAcknowledgePort {
    fn new() -> Self {
        Self {
            inner: FakePort::with_one_batch(),
            acknowledge_started: AtomicUsize::new(0),
        }
    }
}

impl ReminderPresentationPort for PanickingAcknowledgePort {
    fn take(&self) -> Result<Option<DesktopInAppNotificationBatch>, PresentationFailure> {
        self.inner.take()
    }

    fn acknowledge(&self) -> Result<bool, PresentationFailure> {
        self.acknowledge_started.store(1, Ordering::Release);
        panic!("synthetic receipt worker panic");
    }

    fn release(&self) -> Result<bool, PresentationFailure> {
        self.inner.release()
    }
}

impl BlockingReleasePort {
    fn new() -> Self {
        Self {
            inner: FakePort::with_one_batch(),
            release_started: AtomicUsize::new(0),
            release_allowed: AtomicUsize::new(0),
        }
    }
}

impl ReminderPresentationPort for BlockingReleasePort {
    fn take(&self) -> Result<Option<DesktopInAppNotificationBatch>, PresentationFailure> {
        self.inner.take()
    }

    fn acknowledge(&self) -> Result<bool, PresentationFailure> {
        self.inner.acknowledge()
    }

    fn release(&self) -> Result<bool, PresentationFailure> {
        self.release_started.store(1, Ordering::Release);
        while self.release_allowed.load(Ordering::Acquire) == 0 {
            std::thread::yield_now();
        }
        self.inner.release()
    }
}

impl FakePort {
    fn with_one_batch() -> Self {
        Self {
            state: Mutex::new(FakePortState {
                ready: Some(one_batch()),
                leased: None,
                acknowledgements: VecDeque::from([Ok(true)]),
                releases: VecDeque::new(),
                take_count: 0,
                acknowledge_count: 0,
                release_count: 0,
            }),
        }
    }

    fn with_acknowledgements(
        acknowledgements: impl IntoIterator<Item = Result<bool, PresentationFailure>>,
    ) -> Self {
        let port = Self::with_one_batch();
        port.state.lock().expect("fake port state").acknowledgements =
            acknowledgements.into_iter().collect();
        port
    }

    fn with_releases(
        releases: impl IntoIterator<Item = Result<bool, PresentationFailure>>,
    ) -> Self {
        let port = Self::with_one_batch();
        port.state.lock().expect("fake port state").releases = releases.into_iter().collect();
        port
    }

    fn counts(&self) -> (usize, usize, usize) {
        let state = self.state.lock().expect("fake port state");
        (
            state.take_count,
            state.acknowledge_count,
            state.release_count,
        )
    }
}

impl ReminderPresentationPort for FakePort {
    fn take(&self) -> Result<Option<DesktopInAppNotificationBatch>, PresentationFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PresentationFailure::Internal)?;
        state.take_count += 1;
        if state.leased.is_some() {
            return Ok(None);
        }
        let batch = state.ready.take();
        state.leased.clone_from(&batch);
        Ok(batch)
    }

    fn acknowledge(&self) -> Result<bool, PresentationFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PresentationFailure::Internal)?;
        state.acknowledge_count += 1;
        let result = state.acknowledgements.pop_front().unwrap_or(Ok(true));
        if result == Ok(true) {
            state.leased = None;
        }
        result
    }

    fn release(&self) -> Result<bool, PresentationFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PresentationFailure::Internal)?;
        state.release_count += 1;
        let result = state.releases.pop_front().unwrap_or(Ok(true));
        if result == Ok(true) {
            state.ready = state.leased.take();
        }
        result
    }
}

#[derive(Default)]
struct FakePresenter {
    receipt: Mutex<Option<Arc<dyn DesktopNotificationPresentationReceipt>>>,
    present_count: AtomicUsize,
    reject: bool,
}

impl FakePresenter {
    fn rejecting() -> Self {
        Self {
            reject: true,
            ..Self::default()
        }
    }

    fn receipt(&self) -> Arc<dyn DesktopNotificationPresentationReceipt> {
        self.receipt
            .lock()
            .expect("fake presenter receipt")
            .as_ref()
            .expect("scheduled receipt")
            .clone()
    }

    fn complete_presented(&self) {
        self.receipt().presented();
    }

    fn complete_failed(&self) {
        self.receipt().failed();
    }
}

impl NotificationPresenter for FakePresenter {
    fn present(
        &self,
        _batch: DesktopInAppNotificationBatch,
        receipt: Arc<dyn DesktopNotificationPresentationReceipt>,
    ) -> Result<(), PresentationFailure> {
        self.present_count.fetch_add(1, Ordering::AcqRel);
        if self.reject {
            return Err(PresentationFailure::Closed);
        }
        *self
            .receipt
            .lock()
            .map_err(|_| PresentationFailure::Internal)? = Some(receipt);
        Ok(())
    }
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !predicate() {
        assert!(Instant::now() < deadline, "condition timed out");
        std::thread::yield_now();
    }
}

fn assert_stays(duration: Duration, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        assert!(
            predicate(),
            "condition changed before the stability deadline"
        );
        std::thread::yield_now();
    }
}

fn seed_real_reminder(path: &std::path::Path) {
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_millis(),
    )
    .expect("wall clock");
    let scope = BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new("private-account").expect("account"),
        None,
    );
    let lot = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([7; 32]),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 2,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(now - 1_000),
        expiry: BenefitExpiry::exact_utc(now + 30 * 60 * 1_000).expect("expiry"),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot");
    let observation = BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id: BenefitObservationId::from_bytes([1; 32]),
        observed_at_ms: now,
        fresh_until_ms: now + 1_000,
        stale_after_ms: now + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots: vec![lot],
    })
    .expect("observation");
    UsageStore::open(path)
        .expect("store")
        .apply_benefit_observation(&observation)
        .expect("seed benefit");
}

#[test]
fn real_reminder_port_maps_and_acknowledges_only_the_leased_batch() {
    let temporary = TempDir::new().expect("temporary directory");
    let path = temporary.path().join("notification.sqlite3");
    seed_real_reminder(&path);
    let runtime = Arc::new(Mutex::new(
        BenefitReminderRuntime::start(
            BenefitReminderRuntimeConfig::new(path).expect("runtime config"),
        )
        .expect("reminder runtime"),
    ));
    wait_until(|| {
        runtime
            .lock()
            .expect("runtime")
            .try_completion()
            .expect("completion")
            .is_some()
    });
    let port = RuntimeReminderPresentationPort::new(Arc::clone(&runtime));
    let batch = port.take().expect("take").expect("one mapped notification");
    assert_eq!(batch.len(), 1);
    assert_eq!(
        batch.rows()[0].kind(),
        DesktopNotificationKind::BankedRateLimitReset
    );
    assert_eq!(batch.rows()[0].quantity(), 2);
    assert_eq!(batch.rows()[0].label_key(), "benefit.codex.banked_reset");
    assert!(port.take().expect("leased duplicate").is_none());
    assert!(port.acknowledge().expect("acknowledge"));
    assert!(port.take().expect("acknowledged duplicate").is_none());
    drop(port);
    assert_eq!(
        runtime
            .lock()
            .expect("runtime")
            .shutdown()
            .expect("shutdown"),
        BenefitReminderRuntimePhase::Stopped
    );
}

#[test]
fn real_reminder_port_can_release_a_lease_after_outer_mutex_poison() {
    let temporary = TempDir::new().expect("temporary directory");
    let path = temporary.path().join("notification-poison.sqlite3");
    seed_real_reminder(&path);
    let runtime = Arc::new(Mutex::new(
        BenefitReminderRuntime::start(
            BenefitReminderRuntimeConfig::new(path).expect("runtime config"),
        )
        .expect("reminder runtime"),
    ));
    wait_until(|| {
        runtime
            .lock()
            .expect("runtime")
            .try_completion()
            .expect("completion")
            .is_some()
    });
    let port = RuntimeReminderPresentationPort::new(Arc::clone(&runtime));
    assert!(port.take().expect("take").is_some());

    let poisoned_runtime = Arc::clone(&runtime);
    assert!(
        std::thread::spawn(move || {
            let _guard = poisoned_runtime.lock().expect("runtime lock");
            panic!("synthetic outer runtime mutex poison");
        })
        .join()
        .is_err()
    );

    assert!(port.release().expect("poison recovery release"));
    drop(port);
    assert_eq!(
        runtime
            .lock()
            .unwrap_err()
            .into_inner()
            .shutdown()
            .expect("shutdown"),
        BenefitReminderRuntimePhase::Stopped
    );
}

#[test]
fn visible_receipt_precedes_acknowledgement() {
    let port = Arc::new(FakePort::with_one_batch());
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("coordinator");

    assert!(coordinator.pump().expect("lease and schedule"));
    assert_eq!(port.counts(), (1, 0, 0));
    presenter.complete_presented();
    wait_until(|| port.counts().1 == 1);
    assert_eq!(port.counts(), (1, 1, 0));
    coordinator.shutdown().expect("joined shutdown");
}

#[test]
fn scheduling_and_callback_failures_release_the_lease() {
    let rejected_port = Arc::new(FakePort::with_one_batch());
    let mut rejected = ReminderPresentationCoordinator::start_for_test(
        rejected_port.clone(),
        Arc::new(FakePresenter::rejecting()),
        Duration::from_millis(5),
    )
    .expect("rejected coordinator");
    assert_eq!(
        rejected.pump().expect_err("schedule must fail"),
        PresentationFailure::Closed
    );
    wait_until(|| rejected_port.counts().2 == 1);
    rejected.shutdown().expect("rejected shutdown");

    let callback_port = Arc::new(FakePort::with_one_batch());
    let callback_presenter = Arc::new(FakePresenter::default());
    let mut callback = ReminderPresentationCoordinator::start_for_test(
        callback_port.clone(),
        callback_presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("callback coordinator");
    assert!(callback.pump().expect("scheduled"));
    callback_presenter.complete_failed();
    wait_until(|| callback_port.counts().2 == 1);
    assert_eq!(callback_port.counts(), (1, 0, 1));
    callback.shutdown().expect("callback shutdown");
}

#[test]
fn released_presentation_failure_retries_without_an_unrelated_completion() {
    let port = Arc::new(FakePort::with_one_batch());
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("coordinator");

    assert!(coordinator.pump().expect("initial presentation"));
    presenter.complete_failed();
    wait_until(|| presenter.present_count.load(Ordering::Acquire) == 2);
    assert_eq!(port.counts(), (2, 0, 1));

    presenter.complete_presented();
    wait_until(|| port.counts().1 == 1);
    coordinator.shutdown().expect("shutdown");
}

#[test]
fn externally_represented_retry_wakes_the_receipt_worker_immediately() {
    let port = Arc::new(FakePort::with_one_batch());
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_secs(5),
    )
    .expect("coordinator");

    assert!(coordinator.pump().expect("initial presentation"));
    presenter.complete_failed();
    wait_until(|| port.counts().2 == 1);
    wait_until(|| coordinator.pump().expect("external presentation retry"));
    presenter.complete_presented();
    wait_until(|| port.counts().1 == 1);

    coordinator.shutdown().expect("shutdown");
}

#[test]
fn retryable_acknowledgement_retries_and_terminal_failure_releases() {
    let retry_port = Arc::new(FakePort::with_acknowledgements([
        Err(PresentationFailure::Busy),
        Err(PresentationFailure::StoreUnavailable),
        Ok(true),
    ]));
    let retry_presenter = Arc::new(FakePresenter::default());
    let mut retry = ReminderPresentationCoordinator::start_for_test(
        retry_port.clone(),
        retry_presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("retry coordinator");
    assert!(retry.pump().expect("scheduled"));
    retry_presenter.complete_presented();
    wait_until(|| retry_port.counts().1 == 3);
    assert_eq!(retry_port.counts(), (1, 3, 0));
    retry.shutdown().expect("retry shutdown");

    let terminal_port = Arc::new(FakePort::with_acknowledgements([Err(
        PresentationFailure::Internal,
    )]));
    let terminal_presenter = Arc::new(FakePresenter::default());
    let mut terminal = ReminderPresentationCoordinator::start_for_test(
        terminal_port.clone(),
        terminal_presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("terminal coordinator");
    assert!(terminal.pump().expect("scheduled"));
    terminal_presenter.complete_presented();
    wait_until(|| terminal_port.counts().2 == 1);
    assert_eq!(terminal_port.counts(), (1, 1, 1));
    assert_stays(Duration::from_millis(25), || {
        terminal_presenter.present_count.load(Ordering::Acquire) == 1
    });
    terminal.shutdown().expect("terminal shutdown");
}

#[test]
fn receipt_is_one_shot_and_pump_is_capacity_one() {
    let port = Arc::new(FakePort::with_one_batch());
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("coordinator");
    assert!(coordinator.pump().expect("first pump"));
    for _ in 0..10_000 {
        assert!(!coordinator.pump().expect("coalesced pump"));
    }
    assert_eq!(port.counts(), (1, 0, 0));
    assert_eq!(presenter.present_count.load(Ordering::Acquire), 1);

    let receipt = presenter.receipt();
    receipt.presented();
    receipt.failed();
    wait_until(|| port.counts().1 == 1);
    assert_eq!(port.counts(), (1, 1, 0));
    coordinator.shutdown().expect("joined shutdown");
}

#[test]
fn shutdown_releases_scheduled_and_retrying_leases() {
    let scheduled_port = Arc::new(FakePort::with_one_batch());
    let scheduled_presenter = Arc::new(FakePresenter::default());
    let mut scheduled = ReminderPresentationCoordinator::start_for_test(
        scheduled_port.clone(),
        scheduled_presenter,
        Duration::from_secs(1),
    )
    .expect("scheduled coordinator");
    assert!(scheduled.pump().expect("scheduled"));
    scheduled.shutdown().expect("scheduled shutdown");
    assert_eq!(scheduled_port.counts(), (1, 0, 1));

    let retry_port = Arc::new(FakePort::with_acknowledgements([Err(
        PresentationFailure::Busy,
    )]));
    let retry_presenter = Arc::new(FakePresenter::default());
    let mut retry = ReminderPresentationCoordinator::start_for_test(
        retry_port.clone(),
        retry_presenter.clone(),
        Duration::from_secs(1),
    )
    .expect("retry coordinator");
    assert!(retry.pump().expect("scheduled"));
    retry_presenter.complete_presented();
    wait_until(|| retry_port.counts().1 == 1);
    retry.shutdown().expect("retry shutdown");
    assert_eq!(retry_port.counts(), (1, 1, 1));
}

#[test]
fn release_keeps_local_backpressure_until_the_runtime_lease_is_ready() {
    let port = Arc::new(BlockingReleasePort::new());
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("coordinator");
    assert!(coordinator.pump().expect("scheduled"));
    presenter.complete_failed();
    wait_until(|| port.release_started.load(Ordering::Acquire) == 1);

    let pump_during_release = coordinator.pump().expect("coalesced during release");
    let takes_during_release = port.inner.counts().0;
    port.release_allowed.store(1, Ordering::Release);
    wait_until(|| port.inner.counts().2 == 1);

    assert!(!pump_during_release);
    assert_eq!(takes_during_release, 1);
    coordinator.shutdown().expect("shutdown");
}

#[test]
fn failed_release_keeps_local_backpressure_until_shutdown_recovers_the_lease() {
    let port = Arc::new(FakePort::with_releases([
        Err(PresentationFailure::Internal),
        Ok(true),
    ]));
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("coordinator");

    assert!(coordinator.pump().expect("scheduled"));
    presenter.complete_failed();
    wait_until(|| port.counts().2 == 1);
    assert!(!coordinator.pump().expect("backpressure remains"));
    assert_eq!(port.counts(), (1, 0, 1));

    coordinator.shutdown().expect("shutdown release retry");
    assert_eq!(port.counts(), (1, 0, 2));
}

#[test]
fn false_release_keeps_local_backpressure_until_shutdown_recovers_the_lease() {
    let port = Arc::new(FakePort::with_releases([Ok(false), Ok(true)]));
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("coordinator");

    assert!(coordinator.pump().expect("scheduled"));
    presenter.complete_failed();
    wait_until(|| port.counts().2 == 1);
    assert!(!coordinator.pump().expect("backpressure remains"));
    assert_eq!(port.counts(), (1, 0, 1));

    coordinator.shutdown().expect("shutdown release retry");
    assert_eq!(port.counts(), (1, 0, 2));
}

#[test]
fn worker_panic_reports_failure_and_still_releases_the_lease() {
    let port = Arc::new(PanickingAcknowledgePort::new());
    let presenter = Arc::new(FakePresenter::default());
    let mut coordinator = ReminderPresentationCoordinator::start_for_test(
        port.clone(),
        presenter.clone(),
        Duration::from_millis(5),
    )
    .expect("coordinator");
    assert!(coordinator.pump().expect("scheduled"));
    presenter.complete_presented();
    wait_until(|| port.acknowledge_started.load(Ordering::Acquire) == 1);

    assert_eq!(
        coordinator
            .shutdown()
            .expect_err("worker panic must surface"),
        PresentationFailure::Internal
    );
    assert_eq!(port.inner.counts(), (1, 0, 1));
}
