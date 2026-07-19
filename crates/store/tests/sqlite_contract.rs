use rusqlite::Connection;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, NotificationChannel,
    QuotaAccountId, ReminderLeadTime, ReminderProfile, ReminderProfileParts,
    ReminderProfileRevision, UsageProviderId,
};
use tokenmaster_store::{EXPECTED_SQLITE_VERSION, JournalMode, ProbeStore, UsageStore};

fn seed_current_benefit_archive(path: &std::path::Path) {
    let now = 1_721_234_567_890;
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
        .expect("create current archive")
        .apply_benefit_observation(&observation)
        .expect("seed benefit");
}

fn reminder_profile() -> ReminderProfile {
    ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(2).expect("revision"),
        lead_times: vec![ReminderLeadTime::new(21_600).expect("lead")],
        channels: vec![NotificationChannel::InApp],
    })
    .expect("profile")
}

#[test]
fn bundled_sqlite_matches_reviewed_version() {
    let store = ProbeStore::in_memory().expect("store");
    assert_eq!(
        store.sqlite_version().expect("version"),
        EXPECTED_SQLITE_VERSION
    );
    assert_eq!(EXPECTED_SQLITE_VERSION, "3.53.2");
}

#[test]
fn keyset_pages_are_newest_first_and_capped() {
    let mut store = ProbeStore::in_memory().expect("store");
    store.seed_sessions(1_001).expect("seed");

    let first = store.page_before(None, usize::MAX).expect("page");
    assert_eq!(first.len(), 256);
    assert_eq!(first[0].id, 1_001);

    let second = store
        .page_before(first.last().map(|row| row.id), 20)
        .expect("page");
    assert_eq!(second.len(), 20);
    assert!(second[0].id < first[255].id);
}

#[test]
fn file_store_reopens_without_recreating_data() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("probe.sqlite3");
    {
        let mut store = ProbeStore::open(&path).expect("first open");
        store.seed_sessions(3).expect("seed");
    }

    let reopened = ProbeStore::open(&path).expect("second open");
    assert_eq!(reopened.session_count().expect("count"), 3);
}

#[test]
fn open_current_missing_archive_never_creates_a_database_or_sidecars() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("current.sqlite3");

    UsageStore::open_current(&path).expect_err("missing archive");

    assert!(!path.exists());
    assert!(!path.with_extension("sqlite3-wal").exists());
    assert!(!path.with_extension("sqlite3-shm").exists());
}

#[test]
fn open_current_user_version_mismatch_never_migrates_or_mutates_the_archive() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("legacy.sqlite3");
    drop(UsageStore::open(&path).expect("create current archive"));
    let connection = Connection::open(&path).expect("legacy fixture");
    connection
        .pragma_update(None, "user_version", 12_i64)
        .expect("legacy version");
    drop(connection);
    let before = std::fs::read(&path).expect("legacy bytes");

    UsageStore::open_current(&path).expect_err("legacy archive");

    assert_eq!(std::fs::read(&path).expect("legacy bytes"), before);
    let version = Connection::open(&path)
        .expect("legacy archive")
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .expect("legacy version");
    assert_eq!(version, 12);
}

#[test]
fn open_current_validates_runtime_policy_and_retains_the_connection_for_profile_writes() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("current.sqlite3");
    seed_current_benefit_archive(&path);

    let mut store = UsageStore::open_current(&path).expect("current archive");
    let policy = store.runtime_policy().expect("runtime policy");
    assert_eq!(policy.journal_mode(), JournalMode::Wal);
    assert_eq!(policy.busy_timeout_ms(), 250);
    store
        .set_benefit_reminder_global_profile(&reminder_profile())
        .expect("global profile transaction");
    drop(store);

    let revision = Connection::open(&path)
        .expect("current archive")
        .query_row(
            "SELECT revision FROM benefit_reminder_profile
             WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("global profile revision");
    assert_eq!(revision, 2);
}

#[test]
fn seed_count_is_fail_closed_above_one_million() {
    let mut store = ProbeStore::in_memory().expect("store");
    assert!(store.seed_sessions(1_000_001).is_err());
    assert_eq!(store.session_count().expect("count"), 0);
}

#[test]
#[ignore = "M0 scale gate; run explicitly"]
fn one_million_rows_remain_page_bounded() {
    let mut store = ProbeStore::in_memory().expect("store");
    store.seed_sessions(1_000_000).expect("seed");
    assert_eq!(store.session_count().expect("count"), 1_000_000);
    assert_eq!(store.page_before(None, 10_000).expect("page").len(), 256);
}
