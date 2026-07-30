use tokenmaster_benefits::{
    BenefitCurrentLot, BenefitRevision, collapse_due_reminders, schedule_reminders,
};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitError, BenefitEvidenceSource, BenefitExpiry,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitScope, BenefitState, BenefitTarget, NotificationChannel, QuotaAccountId,
    ReminderLeadTime, ReminderProfile, ReminderProfileParts, ReminderProfileRevision,
    UsageProviderId,
};

const NOW_MS: i64 = 1_800_000_000_000;

fn scope() -> Result<BenefitScope, BenefitError> {
    Ok(BenefitScope::new(
        UsageProviderId::new("codex").map_err(|_| BenefitError::InvalidId {
            field: "provider_id",
        })?,
        QuotaAccountId::new("acct_private").map_err(|_| BenefitError::InvalidId {
            field: "account_id",
        })?,
        None,
    ))
}

fn current(
    id: u8,
    state: BenefitState,
    expiry: BenefitExpiry,
) -> Result<BenefitCurrentLot, Box<dyn std::error::Error>> {
    Ok(BenefitCurrentLot::new(
        BenefitLotObservation::new(BenefitLotObservationParts {
            lot_id: BenefitLotId::from_bytes([id; 32]),
            kind: BenefitKind::BankedRateLimitReset,
            quantity: 1,
            state,
            target: BenefitTarget::Provider,
            granted_at_ms: None,
            expiry,
            source: BenefitEvidenceSource::ProviderOfficial,
            confidence: BenefitConfidence::High,
            detail_kind: BenefitDetailKind::ProviderDetail,
            label_key: BenefitLabelKey::new("benefit.codex.banked_reset")?,
        })?,
        BenefitRevision::new(1)?,
    )?)
}

fn profile(revision: u64, seconds: &[u32]) -> Result<ReminderProfile, Box<dyn std::error::Error>> {
    Ok(ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(revision)?,
        lead_times: seconds
            .iter()
            .copied()
            .map(ReminderLeadTime::new)
            .collect::<Result<Vec<_>, _>>()?,
        channels: vec![NotificationChannel::InApp],
    })?)
}

#[test]
fn schedule_is_bounded_deterministic_and_skips_unschedulable_lots()
-> Result<(), Box<dyn std::error::Error>> {
    let lots = vec![
        current(
            1,
            BenefitState::Available,
            BenefitExpiry::exact_utc(NOW_MS + 10 * 60 * 60 * 1_000)?,
        )?,
        current(2, BenefitState::Available, BenefitExpiry::unknown())?,
        current(
            3,
            BenefitState::Activated,
            BenefitExpiry::exact_utc(NOW_MS + 10 * 60 * 60 * 1_000)?,
        )?,
    ];
    let scheduled =
        schedule_reminders(&scope()?, &lots, &profile(1, &[6 * 60 * 60, 3 * 60 * 60])?)?;

    assert_eq!(scheduled.len(), 2);
    assert_eq!(scheduled[0].lead_time().seconds(), 6 * 60 * 60);
    assert_eq!(scheduled[1].lead_time().seconds(), 3 * 60 * 60);
    assert!(scheduled[0].due_at_ms() < scheduled[1].due_at_ms());
    assert_eq!(
        format!("{:?}", scheduled[0].delivery_id()),
        "ReminderDeliveryId([redacted])"
    );
    Ok(())
}

#[test]
fn overdue_thresholds_collapse_to_one_most_urgent_notice_per_lot()
-> Result<(), Box<dyn std::error::Error>> {
    let lots = vec![
        current(
            1,
            BenefitState::Available,
            BenefitExpiry::exact_utc(NOW_MS + 30 * 60 * 1_000)?,
        )?,
        current(
            2,
            BenefitState::Available,
            BenefitExpiry::exact_utc(NOW_MS + 90 * 60 * 1_000)?,
        )?,
    ];
    let scheduled = schedule_reminders(
        &scope()?,
        &lots,
        &profile(1, &[7 * 24 * 60 * 60, 24 * 60 * 60, 60 * 60])?,
    )?;
    let due = collapse_due_reminders(&scheduled, NOW_MS, 256)?;

    assert_eq!(due.len(), 2);
    let first = due
        .iter()
        .find(|entry| entry.lot_id() == BenefitLotId::from_bytes([1; 32]))
        .ok_or("missing first due lot")?;
    assert_eq!(first.lead_time().seconds(), 60 * 60);
    let second = due
        .iter()
        .find(|entry| entry.lot_id() == BenefitLotId::from_bytes([2; 32]))
        .ok_or("missing second due lot")?;
    assert_eq!(second.lead_time().seconds(), 24 * 60 * 60);
    Ok(())
}

#[test]
fn expired_rows_and_delivery_page_overflow_fail_or_drop_safely()
-> Result<(), Box<dyn std::error::Error>> {
    let lot = current(
        1,
        BenefitState::Available,
        BenefitExpiry::exact_utc(NOW_MS - 1)?,
    )?;
    let scheduled = schedule_reminders(&scope()?, &[lot], &profile(1, &[60])?)?;
    assert!(collapse_due_reminders(&scheduled, NOW_MS, 256)?.is_empty());
    assert!(collapse_due_reminders(&scheduled, NOW_MS, 0).is_err());
    Ok(())
}
