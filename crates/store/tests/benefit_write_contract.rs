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
use tokenmaster_store::{BenefitApplyStatus, StoreErrorCode, UsageStore};

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

fn scope() -> BenefitScope {
    scope_with_account("acct_private")
}

fn scope_with_account(account_id: &str) -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new(account_id).expect("account"),
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
    observation_for(scope(), id, observed_at_ms, lots)
}

fn observation_for(
    scope: BenefitScope,
    id: u8,
    observed_at_ms: i64,
    lots: Vec<BenefitLotObservation>,
) -> BenefitInventoryObservation {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id: BenefitObservationId::from_bytes([id; 32]),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots,
    })
    .expect("observation")
}

fn profile(revision: u64, lead_times: &[u32], enabled: bool) -> ReminderProfile {
    ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(revision).expect("revision"),
        lead_times: lead_times
            .iter()
            .copied()
            .map(|seconds| ReminderLeadTime::new(seconds).expect("lead time"))
            .collect(),
        channels: enabled
            .then_some(NotificationChannel::InApp)
            .into_iter()
            .collect(),
    })
    .expect("profile")
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

fn global_profile_state(connection: &Connection) -> (i64, i64, i64, i64) {
    connection
        .query_row(
            "SELECT
               (SELECT revision FROM benefit_reminder_profile
                 WHERE profile_kind = 'global' AND length(profile_scope_id) = 0),
               (SELECT count(*) FROM benefit_reminder_threshold
                 WHERE profile_kind = 'global' AND length(profile_scope_id) = 0),
               (SELECT count(*) FROM benefit_reminder_due),
               (SELECT count(*) FROM benefit_reminder_delivery)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("global profile state")
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
fn global_profile_rebuilds_inherited_scopes_and_preserves_overrides_and_deliveries() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-global-profile.sqlite3");
    let inherited_scope = scope_with_account("acct_inherited");
    let override_scope = scope_with_account("acct_override");
    let expiry = BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10 * 60 * 60 * 1_000).expect("expiry");
    let mut store = UsageStore::open(&path).expect("store");
    store
        .apply_benefit_observation(&observation_for(
            inherited_scope.clone(),
            1,
            OBSERVED_AT_MS,
            vec![lot(1, BenefitState::Available, expiry.clone())],
        ))
        .expect("inherited observation");
    store
        .apply_benefit_observation(&observation_for(
            override_scope.clone(),
            2,
            OBSERVED_AT_MS + 1,
            vec![lot(2, BenefitState::Available, expiry)],
        ))
        .expect("override observation");
    let delivered = store
        .process_due_in_app_benefit_reminders(OBSERVED_AT_MS, 1)
        .expect("delivery receipt");
    assert_eq!(delivered.delivery_count(), 1);
    store
        .set_benefit_reminder_override(&override_scope, Some(&profile(1, &[21_600, 10_800], true)))
        .expect("scope override");
    drop(store);

    let mut store = UsageStore::open(&path).expect("reopen");
    let applied = store
        .set_benefit_reminder_global_profile(&profile(2, &[21_600, 10_800], true))
        .expect("global profile");
    assert_eq!(applied.pending_due_count(), 4);
    drop(store);

    let connection = Connection::open(&path).expect("inspect global profile");
    let rows = connection
        .prepare(
            "SELECT scope_id, threshold_seconds, profile_revision
             FROM benefit_reminder_due
             ORDER BY scope_id, threshold_seconds DESC",
        )
        .expect("due rows")
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .expect("map due rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect due rows");
    assert_eq!(rows.len(), 4);
    assert!(
        rows.iter()
            .all(|(_, threshold, _)| [21_600, 10_800].contains(threshold))
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM benefit_reminder_delivery",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("delivery count"),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|(_, _, revision)| *revision == 2)
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|(_, _, revision)| *revision == 1)
            .count(),
        2
    );
}

#[test]
fn global_profile_admission_noop_and_disabled_empty_profile_are_atomic() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("benefit-global-profile-admission.sqlite3");
    let mut store = UsageStore::open(&path).expect("store");
    store
        .apply_benefit_observation(&observation(
            1,
            OBSERVED_AT_MS,
            vec![lot(
                1,
                BenefitState::Available,
                BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10 * 60 * 60 * 1_000).expect("expiry"),
            )],
        ))
        .expect("initial observation");
    let defaults = profile(1, &[604_800, 86_400, 43_200, 21_600, 3_600], true);
    let no_op = store
        .set_benefit_reminder_global_profile(&defaults)
        .expect("identical global profile");
    assert_eq!(no_op.benefit_revision().get(), 1);
    assert_eq!(no_op.pending_due_count(), 5);

    store
        .set_benefit_reminder_global_profile(&profile(2, &[21_600], true))
        .expect("newer global profile");
    let before_rejections = global_profile_state(&Connection::open(&path).expect("inspect state"));
    let stale = store
        .set_benefit_reminder_global_profile(&defaults)
        .expect_err("stale profile");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
    let equivocation = store
        .set_benefit_reminder_global_profile(&profile(2, &[10_800], true))
        .expect_err("same revision different profile");
    assert_eq!(equivocation.code(), StoreErrorCode::InvalidValue);
    assert_eq!(
        global_profile_state(&Connection::open(&path).expect("inspect state")),
        before_rejections
    );
    let disabled = store
        .set_benefit_reminder_global_profile(&profile(3, &[], false))
        .expect("disabled empty profile");
    assert_eq!(disabled.pending_due_count(), 0);
    drop(store);

    let connection = Connection::open(&path).expect("inspect disabled profile");
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM benefit_reminder_threshold WHERE profile_kind = 'global'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("global threshold count"),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM benefit_reminder_due", [], |row| row
                .get::<_, i64>(
                0
            ))
            .expect("due count"),
        0
    );
}

#[test]
fn global_profile_rejects_scope_and_total_lot_lookahead_before_mutation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("benefit-global-profile-capacity.sqlite3");
    let mut store = UsageStore::open(&path).expect("store");
    for index in 0..33_u8 {
        store
            .apply_benefit_observation(&observation_for(
                scope_with_account(&format!("acct_scope_{index}")),
                index.saturating_add(1),
                OBSERVED_AT_MS + i64::from(index),
                Vec::new(),
            ))
            .expect("empty scope observation");
    }
    let before_scope_rejection =
        global_profile_state(&Connection::open(&path).expect("inspect state"));
    let scope_error = store
        .set_benefit_reminder_global_profile(&profile(2, &[21_600], true))
        .expect_err("scope lookahead capacity");
    assert_eq!(scope_error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        global_profile_state(&Connection::open(&path).expect("inspect state")),
        before_scope_rejection
    );
    drop(store);

    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-global-profile-lots.sqlite3");
    let mut store = UsageStore::open(&path).expect("store");
    let expiry =
        BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10 * 24 * 60 * 60 * 1_000).expect("expiry");
    let lots: Vec<BenefitLotObservation> = (0..64_u8)
        .map(|id| lot(id, BenefitState::Available, expiry.clone()))
        .collect();
    for index in 0..4_u8 {
        store
            .apply_benefit_observation(&observation_for(
                scope_with_account(&format!("acct_many_{index}")),
                index.saturating_add(1),
                OBSERVED_AT_MS + i64::from(index),
                lots.clone(),
            ))
            .expect("64 lots");
    }
    store
        .apply_benefit_observation(&observation_for(
            scope_with_account("acct_one_more"),
            5,
            OBSERVED_AT_MS + 5,
            vec![lot(9, BenefitState::Available, expiry)],
        ))
        .expect("one more lot");
    let before_lot_rejection =
        global_profile_state(&Connection::open(&path).expect("inspect state"));
    let lot_error = store
        .set_benefit_reminder_global_profile(&profile(2, &[21_600], true))
        .expect_err("lot capacity");
    assert_eq!(lot_error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        global_profile_state(&Connection::open(&path).expect("inspect state")),
        before_lot_rejection
    );
}

#[test]
fn global_profile_ignores_overridden_scope_and_lot_capacity() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("benefit-global-profile-overrides.sqlite3");
    let inherited_scope = scope_with_account("acct_inherited");
    let expiry =
        BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10 * 24 * 60 * 60 * 1_000).expect("expiry");
    let lots: Vec<BenefitLotObservation> = (0..64_u8)
        .map(|id| lot(id, BenefitState::Available, expiry.clone()))
        .collect();
    let mut store = UsageStore::open(&path).expect("store");
    store
        .apply_benefit_observation(&observation_for(
            inherited_scope,
            1,
            OBSERVED_AT_MS,
            vec![lot(127, BenefitState::Available, expiry)],
        ))
        .expect("inherited observation");
    for index in 0..33_u8 {
        let scope = scope_with_account(&format!("acct_override_{index}"));
        let current_lots = if index < 5 { lots.clone() } else { Vec::new() };
        store
            .apply_benefit_observation(&observation_for(
                scope.clone(),
                index.saturating_add(2),
                OBSERVED_AT_MS + i64::from(index) + 1,
                current_lots,
            ))
            .expect("override observation");
        store
            .set_benefit_reminder_override(&scope, Some(&profile(1, &[21_600], true)))
            .expect("scope override");
    }
    let delivery_at_ms = OBSERVED_AT_MS + 10 * 24 * 60 * 60 * 1_000 - 21_600 * 1_000 + 1;
    let mut found_override_delivery = false;
    for offset in 0..400_i64 {
        let deliveries = store
            .process_due_in_app_benefit_reminders(delivery_at_ms + offset, 1)
            .expect("delivery");
        store
            .acknowledge_benefit_reminders(deliveries.deliveries(), delivery_at_ms + offset + 1)
            .expect("acknowledgement");
        let connection = Connection::open(&path).expect("inspect delivery");
        let override_deliveries: i64 = connection
            .query_row(
                "SELECT count(*) FROM benefit_reminder_delivery AS delivery
                 WHERE EXISTS(SELECT 1 FROM benefit_reminder_profile AS profile
                              WHERE profile.profile_kind = 'scope'
                                AND profile.profile_scope_id = delivery.scope_id)",
                [],
                |row| row.get(0),
            )
            .expect("override delivery count");
        if override_deliveries > 0 {
            found_override_delivery = true;
            break;
        }
    }
    assert!(found_override_delivery, "override delivery was not reached");
    drop(store);

    let connection = Connection::open(&path).expect("inspect before");
    let before = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM benefit_reminder_profile WHERE profile_kind = 'scope'),
               (SELECT count(*) FROM benefit_reminder_due AS due
                  WHERE EXISTS(SELECT 1 FROM benefit_reminder_profile AS profile
                               WHERE profile.profile_kind = 'scope'
                                 AND profile.profile_scope_id = due.scope_id)),
               (SELECT count(*) FROM benefit_reminder_delivery AS delivery
                  WHERE EXISTS(SELECT 1 FROM benefit_reminder_profile AS profile
                               WHERE profile.profile_kind = 'scope'
                                 AND profile.profile_scope_id = delivery.scope_id)),
               (SELECT count(*) FROM benefit_reminder_ack AS acknowledgement
                  JOIN benefit_reminder_delivery AS delivery
                    ON delivery.delivery_id = acknowledgement.delivery_id
                  WHERE EXISTS(SELECT 1 FROM benefit_reminder_profile AS profile
                               WHERE profile.profile_kind = 'scope'
                                 AND profile.profile_scope_id = delivery.scope_id))",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .expect("override state");
    assert!(before.2 > 0, "override delivery preserved");
    assert!(before.3 > 0, "override acknowledgement preserved");
    drop(connection);

    let mut store = UsageStore::open(&path).expect("reopen");
    let applied = store
        .set_benefit_reminder_global_profile(&profile(2, &[21_600], true))
        .expect("global profile despite overridden capacity");
    assert!(applied.pending_due_count() > 0);
    drop(store);

    let connection = Connection::open(&path).expect("inspect after");
    let after = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM benefit_reminder_profile WHERE profile_kind = 'scope'),
               (SELECT count(*) FROM benefit_reminder_due AS due
                  WHERE EXISTS(SELECT 1 FROM benefit_reminder_profile AS profile
                               WHERE profile.profile_kind = 'scope'
                                 AND profile.profile_scope_id = due.scope_id)),
               (SELECT count(*) FROM benefit_reminder_delivery AS delivery
                  WHERE EXISTS(SELECT 1 FROM benefit_reminder_profile AS profile
                               WHERE profile.profile_kind = 'scope'
                                 AND profile.profile_scope_id = delivery.scope_id)),
               (SELECT count(*) FROM benefit_reminder_ack AS acknowledgement
                  JOIN benefit_reminder_delivery AS delivery
                    ON delivery.delivery_id = acknowledgement.delivery_id
                  WHERE EXISTS(SELECT 1 FROM benefit_reminder_profile AS profile
                               WHERE profile.profile_kind = 'scope'
                                 AND profile.profile_scope_id = delivery.scope_id))",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .expect("override state");
    assert_eq!(after, before);
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
