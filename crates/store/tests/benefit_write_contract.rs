use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, NotificationChannel,
    QuotaAccountId, ReminderLeadTime, ReminderProfile, ReminderProfileParts,
    ReminderProfileRevision, UsageProviderId,
};
use tokenmaster_store::{BenefitApplyStatus, UsageStore};

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

fn scope() -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new("acct_private").expect("account"),
        None,
    )
}

fn lot(id: u8, state: BenefitState, expiry: BenefitExpiry) -> BenefitLotObservation {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([id; 32]),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 1,
        state,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(OBSERVED_AT_MS - 1_000),
        expiry,
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot")
}

fn observation(
    id: u8,
    observed_at_ms: i64,
    lots: Vec<BenefitLotObservation>,
) -> BenefitInventoryObservation {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope(),
        observation_id: BenefitObservationId::from_bytes([id; 32]),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots,
    })
    .expect("observation")
}

fn counts(connection: &Connection) -> (i64, i64, i64, i64, i64) {
    connection
        .query_row(
            "SELECT state.revision,
                    (SELECT count(*) FROM benefit_lot_current),
                    (SELECT count(*) FROM benefit_lot_revision),
                    (SELECT count(*) FROM benefit_change),
                    (SELECT count(*) FROM benefit_reminder_due)
             FROM benefit_state AS state WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("benefit counts")
}

#[test]
fn changed_duplicate_freshness_missing_and_restart_are_transactional() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-write.sqlite3");
    let first_lot = lot(
        1,
        BenefitState::Available,
        BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10 * 24 * 60 * 60 * 1_000).expect("expiry"),
    );
    let aggregate = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([2; 32]),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 2,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: None,
        expiry: BenefitExpiry::unknown(),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::Medium,
        detail_kind: BenefitDetailKind::ProviderAggregate,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("aggregate");
    let first = observation(1, OBSERVED_AT_MS, vec![first_lot.clone(), aggregate]);

    {
        let mut store = UsageStore::open(&path).expect("store");
        let applied = store
            .apply_benefit_observation(&first)
            .expect("initial benefit observation");
        assert_eq!(applied.status(), BenefitApplyStatus::Changed);
        assert_eq!(applied.benefit_revision().get(), 1);
        assert_eq!(applied.change_count(), 2);
        assert_eq!(applied.pending_due_count(), 5);

        let duplicate = store
            .apply_benefit_observation(&first)
            .expect("duplicate observation");
        assert_eq!(duplicate.status(), BenefitApplyStatus::Duplicate);
        assert_eq!(duplicate.benefit_revision().get(), 1);
        assert_eq!(duplicate.change_count(), 0);

        let refreshed = observation(
            2,
            OBSERVED_AT_MS + 1,
            vec![first_lot.clone(), first.lots()[1].clone()],
        );
        let freshness = store
            .apply_benefit_observation(&refreshed)
            .expect("freshness observation");
        assert_eq!(freshness.status(), BenefitApplyStatus::FreshnessOnly);
        assert_eq!(freshness.benefit_revision().get(), 2);
        assert_eq!(freshness.change_count(), 0);
        assert_eq!(freshness.pending_due_count(), 5);

        let missing = observation(3, OBSERVED_AT_MS + 2, Vec::new());
        let ambiguous = store
            .apply_benefit_observation(&missing)
            .expect("missing observation");
        assert_eq!(ambiguous.status(), BenefitApplyStatus::Changed);
        assert_eq!(ambiguous.change_count(), 2);
        assert_eq!(ambiguous.pending_due_count(), 0);
    }

    let connection = Connection::open(&path).expect("inspect store");
    assert_eq!(counts(&connection), (3, 2, 4, 4, 0));
    drop(connection);

    let returned = observation(4, OBSERVED_AT_MS + 3, vec![first_lot]);
    let mut reopened = UsageStore::open(&path).expect("reopen");
    let reappeared = reopened
        .apply_benefit_observation(&returned)
        .expect("reappeared observation");
    assert_eq!(reappeared.status(), BenefitApplyStatus::Changed);
    assert_eq!(reappeared.change_count(), 1);
    assert_eq!(reappeared.pending_due_count(), 5);
}

#[test]
fn scope_override_replaces_defaults_and_removal_restores_inheritance() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-profile.sqlite3");
    let mut store = UsageStore::open(&path).expect("store");
    let input = observation(
        1,
        OBSERVED_AT_MS,
        vec![lot(
            1,
            BenefitState::Available,
            BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10 * 60 * 60 * 1_000).expect("expiry"),
        )],
    );
    store
        .apply_benefit_observation(&input)
        .expect("initial observation");

    let custom = ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(2).expect("revision"),
        lead_times: vec![
            ReminderLeadTime::new(6 * 60 * 60).expect("six hours"),
            ReminderLeadTime::new(3 * 60 * 60).expect("three hours"),
        ],
        channels: vec![NotificationChannel::InApp],
    })
    .expect("custom profile");
    let applied = store
        .set_benefit_reminder_override(&scope(), Some(&custom))
        .expect("apply override");
    assert_eq!(applied.pending_due_count(), 2);
    drop(store);

    let connection = Connection::open(&path).expect("inspect override");
    let thresholds = connection
        .prepare(
            "SELECT threshold_seconds FROM benefit_reminder_due
             ORDER BY threshold_seconds DESC",
        )
        .expect("due thresholds")
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("due rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect due thresholds");
    assert_eq!(thresholds, vec![21_600, 10_800]);
    drop(connection);

    let mut reopened = UsageStore::open(&path).expect("reopen");
    let removed = reopened
        .set_benefit_reminder_override(&scope(), None)
        .expect("remove override");
    assert_eq!(removed.pending_due_count(), 5);
}

#[test]
fn terminal_retirement_and_later_reappearance_keep_monotonic_lot_revision() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("benefit-terminal-reappearance.sqlite3");
    let terminal = lot(
        9,
        BenefitState::Activated,
        BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10_000).expect("expiry"),
    );

    {
        let mut store = UsageStore::open(&path).expect("store");
        store
            .apply_benefit_observation(&observation(1, OBSERVED_AT_MS, vec![terminal.clone()]))
            .expect("terminal lot");
        let retired = store
            .apply_benefit_observation(&observation(2, OBSERVED_AT_MS + 1, Vec::new()))
            .expect("terminal retirement");
        assert_eq!(retired.status(), BenefitApplyStatus::Changed);
        assert_eq!(retired.change_count(), 1);
    }

    let available = BenefitLotObservation::new(BenefitLotObservationParts {
        state: BenefitState::Available,
        ..terminal.into_parts()
    })
    .expect("available again");
    let mut reopened = UsageStore::open(&path).expect("reopen");
    let returned = reopened
        .apply_benefit_observation(&observation(3, OBSERVED_AT_MS + 2, vec![available]))
        .expect("terminal lot reappears");
    assert_eq!(returned.status(), BenefitApplyStatus::Changed);
    assert_eq!(returned.change_count(), 1);
    drop(reopened);

    let connection = Connection::open(&path).expect("inspect revisions");
    let revisions = connection
        .prepare(
            "SELECT lot_revision FROM benefit_lot_revision
             WHERE lot_id = ?1 ORDER BY lot_revision",
        )
        .expect("prepare revisions")
        .query_map([[9_u8; 32].as_slice()], |row| row.get::<_, i64>(0))
        .expect("revision rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect revisions");
    assert_eq!(revisions, vec![1, 3]);
    let current_revision: i64 = connection
        .query_row(
            "SELECT lot_revision FROM benefit_lot_current WHERE lot_id = ?1",
            [[9_u8; 32].as_slice()],
            |row| row.get(0),
        )
        .expect("current revision");
    assert_eq!(current_revision, 3);
}
