use tempfile::TempDir;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, NotificationChannel,
    QuotaAccountId, ReminderLeadTime, ReminderProfile, ReminderProfileParts,
    ReminderProfileRevision, UsageProviderId,
};
use tokenmaster_store::{MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE, UsageStore};

const NOW_MS: i64 = 1_800_000_000_000;

fn scope(account: &str) -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new(account).expect("account"),
        None,
    )
}

fn lot(id: u8, expiry_at_ms: i64) -> BenefitLotObservation {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([id; 32]),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 1,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(NOW_MS - 1_000),
        expiry: BenefitExpiry::exact_utc(expiry_at_ms).expect("expiry"),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot")
}

fn observation(
    account: &str,
    id: u8,
    observed_at_ms: i64,
    lots: Vec<BenefitLotObservation>,
) -> BenefitInventoryObservation {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope(account),
        observation_id: BenefitObservationId::from_bytes([id; 32]),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots,
    })
    .expect("observation")
}

fn recommended(revision: u64) -> ReminderProfile {
    ReminderProfile::recommended(ReminderProfileRevision::new(revision).expect("revision"))
        .expect("profile")
}

#[test]
fn overdue_thresholds_collapse_to_one_durable_in_app_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-reminder.sqlite3");
    let expiry_at_ms = NOW_MS + 30 * 60 * 1_000;
    {
        let mut store = UsageStore::open(&path).expect("store");
        store
            .apply_benefit_observation(&observation(
                "acct_private",
                1,
                NOW_MS - 1,
                vec![lot(1, expiry_at_ms)],
            ))
            .expect("observation");

        let processed = store
            .process_due_benefit_reminders(
                NOW_MS,
                NotificationChannel::InApp,
                MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
            )
            .expect("process due reminders");
        assert_eq!(processed.examined_count(), 5);
        assert_eq!(processed.expired_count(), 0);
        assert_eq!(processed.delivery_count(), 1);
        assert_eq!(processed.pending_due_count(), 0);
        assert_eq!(processed.retained_delivery_count(), 1);
        assert_eq!(processed.nearest_due_at_ms(), None);
        let delivery = &processed.deliveries()[0];
        assert_eq!(delivery.kind(), BenefitKind::BankedRateLimitReset);
        assert_eq!(delivery.quantity(), 1);
        assert_eq!(delivery.label_key(), "benefit.codex.banked_reset");
        assert_eq!(
            delivery.lead_time(),
            ReminderLeadTime::new(60 * 60).expect("one hour")
        );
        assert_eq!(delivery.channel(), NotificationChannel::InApp);
        assert_eq!(delivery.expiry_at_ms(), expiry_at_ms);
        assert_eq!(delivery.delivered_at_ms(), NOW_MS);
    }

    let mut reopened = UsageStore::open(&path).expect("reopen");
    let duplicate = reopened
        .process_due_benefit_reminders(
            NOW_MS + 1,
            NotificationChannel::InApp,
            MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
        )
        .expect("restart pass");
    assert_eq!(duplicate.examined_count(), 0);
    assert_eq!(duplicate.delivery_count(), 0);

    reopened
        .set_benefit_reminder_override(&scope("acct_private"), Some(&recommended(2)))
        .expect("rebuild profile");
    let rebuilt = reopened
        .process_due_benefit_reminders(
            NOW_MS + 2,
            NotificationChannel::InApp,
            MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
        )
        .expect("rebuilt queue");
    assert_eq!(rebuilt.examined_count(), 0);
    assert_eq!(rebuilt.delivery_count(), 0);
    assert_eq!(rebuilt.pending_due_count(), 0);
}

#[test]
fn collapsed_receipt_preserves_future_more_urgent_threshold() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-reminder-future.sqlite3");
    let expiry_at_ms = NOW_MS + 5 * 60 * 60 * 1_000;
    let mut store = UsageStore::open(&path).expect("store");
    store
        .apply_benefit_observation(&observation(
            "acct_private",
            1,
            NOW_MS - 1,
            vec![lot(1, expiry_at_ms)],
        ))
        .expect("observation");

    let first = store
        .process_due_benefit_reminders(
            NOW_MS,
            NotificationChannel::InApp,
            MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
        )
        .expect("first due pass");
    assert_eq!(first.examined_count(), 4);
    assert_eq!(first.delivery_count(), 1);
    assert_eq!(
        first.deliveries()[0].lead_time(),
        ReminderLeadTime::new(6 * 60 * 60).expect("six hours")
    );
    assert_eq!(
        first.nearest_due_at_ms(),
        Some(expiry_at_ms - 60 * 60 * 1_000)
    );
    assert_eq!(first.pending_due_count(), 1);

    store
        .set_benefit_reminder_override(&scope("acct_private"), Some(&recommended(2)))
        .expect("profile rebuild");
    let second = store
        .process_due_benefit_reminders(
            expiry_at_ms - 30 * 60 * 1_000,
            NotificationChannel::InApp,
            MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
        )
        .expect("urgent due pass");
    assert_eq!(second.examined_count(), 1);
    assert_eq!(second.delivery_count(), 1);
    assert_eq!(
        second.deliveries()[0].lead_time(),
        ReminderLeadTime::new(60 * 60).expect("one hour")
    );
    assert_eq!(second.pending_due_count(), 0);
    assert_eq!(second.retained_delivery_count(), 2);
}

#[test]
fn expired_rows_are_drained_without_publication_and_limits_fail_closed() {
    let mut store = UsageStore::in_memory().expect("store");
    let expiry_at_ms = NOW_MS + 1_000;
    store
        .apply_benefit_observation(&observation(
            "acct_private",
            1,
            NOW_MS,
            vec![lot(1, expiry_at_ms)],
        ))
        .expect("observation");

    let expired = store
        .process_due_benefit_reminders(
            expiry_at_ms + 1,
            NotificationChannel::InApp,
            MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
        )
        .expect("expired pass");
    assert_eq!(expired.examined_count(), 5);
    assert_eq!(expired.expired_count(), 5);
    assert_eq!(expired.delivery_count(), 0);
    assert_eq!(expired.pending_due_count(), 0);
    assert_eq!(expired.retained_delivery_count(), 0);

    assert!(
        store
            .process_due_benefit_reminders(NOW_MS, NotificationChannel::InApp, 0)
            .is_err()
    );
    assert!(
        store
            .process_due_benefit_reminders(
                NOW_MS,
                NotificationChannel::InApp,
                MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE + 1,
            )
            .is_err()
    );
    assert!(
        store
            .process_due_benefit_reminders(
                0,
                NotificationChannel::InApp,
                MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
            )
            .is_err()
    );
}

#[test]
fn custom_profile_can_leave_reminders_disabled_after_receipt() {
    let mut store = UsageStore::in_memory().expect("store");
    store
        .apply_benefit_observation(&observation(
            "acct_private",
            1,
            NOW_MS - 1,
            vec![lot(1, NOW_MS + 30 * 60 * 1_000)],
        ))
        .expect("observation");
    store
        .process_due_benefit_reminders(
            NOW_MS,
            NotificationChannel::InApp,
            MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
        )
        .expect("delivery");

    let disabled = ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(2).expect("revision"),
        lead_times: Vec::new(),
        channels: vec![NotificationChannel::InApp],
    })
    .expect("disabled profile");
    let applied = store
        .set_benefit_reminder_override(&scope("acct_private"), Some(&disabled))
        .expect("disable reminders");
    assert_eq!(applied.pending_due_count(), 0);
}
