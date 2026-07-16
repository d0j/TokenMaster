use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitError, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLocalDate, BenefitLocalDateTime, BenefitLocalTime,
    BenefitLotId, BenefitLotObservation, BenefitLotObservationParts, BenefitObservationId,
    BenefitScope, BenefitState, BenefitTarget, BenefitTimeZoneId, MAX_BENEFIT_LOTS_PER_OBSERVATION,
    MAX_REMINDER_THRESHOLDS, NotificationChannel, QuotaAccountId, QuotaWindowId,
    RECOMMENDED_REMINDER_LEAD_SECONDS, ReminderLeadTime, ReminderProfile, ReminderProfileParts,
    ReminderProfileRevision, UsageProviderId,
};

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

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

fn lot_id(value: u8) -> BenefitLotId {
    BenefitLotId::from_bytes([value; 32])
}

fn observation_id(value: u8) -> BenefitObservationId {
    BenefitObservationId::from_bytes([value; 32])
}

fn lot(id: u8, expiry: BenefitExpiry) -> Result<BenefitLotObservation, BenefitError> {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: lot_id(id),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 1,
        state: BenefitState::Available,
        target: BenefitTarget::QuotaWindow(
            QuotaWindowId::new("codex.primary")
                .map_err(|_| BenefitError::InvalidId { field: "window_id" })?,
        ),
        granted_at_ms: Some(OBSERVED_AT_MS - 86_400_000),
        expiry,
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset")?,
    })
}

fn inventory(
    lots: Vec<BenefitLotObservation>,
) -> Result<BenefitInventoryObservation, BenefitError> {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope()?,
        observation_id: observation_id(9),
        observed_at_ms: OBSERVED_AT_MS,
        fresh_until_ms: OBSERVED_AT_MS + 1_200_000,
        stale_after_ms: OBSERVED_AT_MS + 7_200_000,
        completeness: BenefitInventoryCompleteness::CompleteQuantityPartialDetails,
        lots,
    })
}

#[test]
fn opaque_ids_are_fixed_and_debug_redacted() {
    let lot = lot_id(0xAB);
    let observation = observation_id(0xCD);

    assert_eq!(lot.as_bytes(), &[0xAB; 32]);
    assert_eq!(observation.as_bytes(), &[0xCD; 32]);
    assert_eq!(format!("{lot:?}"), "BenefitLotId([redacted])");
    assert_eq!(
        format!("{observation:?}"),
        "BenefitObservationId([redacted])"
    );
    assert!(!format!("{lot:?}").contains("171"));
}

#[test]
fn expiry_precision_is_typed_and_conservative_only_when_resolvable() -> Result<(), BenefitError> {
    let exact = BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10_000)?;
    assert_eq!(exact.conservative_utc_ms(), Some(OBSERVED_AT_MS + 10_000));

    let bounded = BenefitExpiry::bounded_utc(OBSERVED_AT_MS + 20_000, OBSERVED_AT_MS + 30_000)?;
    assert_eq!(bounded.conservative_utc_ms(), Some(OBSERVED_AT_MS + 20_000));
    assert_eq!(
        BenefitExpiry::bounded_utc(10, 9),
        Err(BenefitError::InvalidExpiry)
    );

    let date = BenefitLocalDate::new(2028, 2, 29)?;
    let time = BenefitLocalTime::new(23, 59, 58, 999)?;
    let zone = BenefitTimeZoneId::new("Asia/Jerusalem")?;
    let local = BenefitExpiry::provider_local(BenefitLocalDateTime::new(date, time), zone.clone());
    assert_eq!(local.conservative_utc_ms(), None);
    let provider_date = BenefitExpiry::provider_date(date, Some(zone));
    assert_eq!(provider_date.conservative_utc_ms(), None);
    assert_eq!(
        BenefitLocalDate::new(2027, 2, 29),
        Err(BenefitError::InvalidLocalDate)
    );
    assert_eq!(
        BenefitTimeZoneId::new("../private"),
        Err(BenefitError::InvalidTimeZoneId)
    );
    Ok(())
}

#[test]
fn lot_and_inventory_bounds_reject_false_or_ambiguous_facts() -> Result<(), BenefitError> {
    let expiry = BenefitExpiry::exact_utc(OBSERVED_AT_MS + 86_400_000)?;
    let valid = lot(1, expiry.clone())?;
    assert_eq!(valid.quantity(), 1);
    assert_eq!(valid.expiry(), &expiry);
    assert_eq!(valid.label_key(), "benefit.codex.banked_reset");

    let zero_quantity = BenefitLotObservation::new(BenefitLotObservationParts {
        quantity: 0,
        ..valid.clone().into_parts()
    });
    assert_eq!(zero_quantity, Err(BenefitError::InvalidQuantity));

    let duplicate = inventory(vec![valid.clone(), valid.clone()]);
    assert_eq!(duplicate, Err(BenefitError::DuplicateLotId));

    let too_many = (0..=MAX_BENEFIT_LOTS_PER_OBSERVATION)
        .map(|index| lot(index as u8, BenefitExpiry::unknown()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        inventory(too_many),
        Err(BenefitError::CapacityExceeded {
            limit: MAX_BENEFIT_LOTS_PER_OBSERVATION,
        })
    );

    let invalid_times = BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope()?,
        observation_id: observation_id(10),
        observed_at_ms: OBSERVED_AT_MS,
        fresh_until_ms: OBSERVED_AT_MS - 1,
        stale_after_ms: OBSERVED_AT_MS + 1,
        completeness: BenefitInventoryCompleteness::Complete,
        lots: Vec::new(),
    });
    assert_eq!(invalid_times, Err(BenefitError::InvalidObservationTimes));

    let aggregate = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: lot_id(7),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 2,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: None,
        expiry: BenefitExpiry::unknown(),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::Medium,
        detail_kind: BenefitDetailKind::ProviderAggregate,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset")?,
    })?;
    assert_eq!(aggregate.quantity(), 2);
    assert_eq!(aggregate.expiry(), &BenefitExpiry::Unknown);
    Ok(())
}

#[test]
fn reminder_profiles_dedupe_sort_and_keep_user_choice_exact() -> Result<(), BenefitError> {
    let revision = ReminderProfileRevision::new(1)?;
    let recommended = ReminderProfile::recommended(revision)?;
    assert_eq!(
        recommended
            .lead_times()
            .iter()
            .map(|value| value.seconds())
            .collect::<Vec<_>>(),
        RECOMMENDED_REMINDER_LEAD_SECONDS
    );

    let custom = ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(2)?,
        lead_times: vec![
            ReminderLeadTime::new(3 * 60 * 60)?,
            ReminderLeadTime::new(6 * 60 * 60)?,
            ReminderLeadTime::new(3 * 60 * 60)?,
        ],
        channels: vec![NotificationChannel::InApp, NotificationChannel::InApp],
    })?;
    assert_eq!(
        custom
            .lead_times()
            .iter()
            .map(|value| value.seconds())
            .collect::<Vec<_>>(),
        vec![6 * 60 * 60, 3 * 60 * 60]
    );
    assert_eq!(custom.channels(), &[NotificationChannel::InApp]);

    let empty = ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(3)?,
        lead_times: Vec::new(),
        channels: vec![NotificationChannel::InApp],
    })?;
    assert!(empty.lead_times().is_empty());

    assert_eq!(
        ReminderLeadTime::new(59),
        Err(BenefitError::InvalidReminderLeadTime)
    );
    assert_eq!(
        ReminderProfileRevision::new(0),
        Err(BenefitError::InvalidReminderProfileRevision)
    );
    let too_many = (0..=MAX_REMINDER_THRESHOLDS)
        .map(|index| ReminderLeadTime::new(60 + (index as u32 * 60)))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        ReminderProfile::new(ReminderProfileParts {
            revision,
            lead_times: too_many,
            channels: vec![NotificationChannel::InApp],
        }),
        Err(BenefitError::CapacityExceeded {
            limit: MAX_REMINDER_THRESHOLDS,
        })
    );
    Ok(())
}

#[test]
fn serde_revalidates_values_and_rejects_unknown_nested_fields() -> Result<(), BenefitError> {
    let value = inventory(vec![lot(
        1,
        BenefitExpiry::bounded_utc(OBSERVED_AT_MS + 100, OBSERVED_AT_MS + 200)?,
    )?])?;
    let encoded = serde_json::to_value(&value).map_err(|_| BenefitError::InvalidSerializedValue)?;
    let decoded: BenefitInventoryObservation = serde_json::from_value(encoded.clone())
        .map_err(|_| BenefitError::InvalidSerializedValue)?;
    assert_eq!(decoded, value);

    let mut unknown = encoded;
    unknown["lots"][0]["privatePayload"] = serde_json::json!("secret");
    assert!(serde_json::from_value::<BenefitInventoryObservation>(unknown).is_err());

    let mut invalid_profile = serde_json::to_value(ReminderProfile::recommended(
        ReminderProfileRevision::new(4)?,
    )?)
    .map_err(|_| BenefitError::InvalidSerializedValue)?;
    invalid_profile["lead_times"][0] = serde_json::json!(1);
    assert!(serde_json::from_value::<ReminderProfile>(invalid_profile).is_err());
    Ok(())
}
