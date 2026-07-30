use tokenmaster_benefits::{
    BenefitChangeKind, BenefitInventoryState, BenefitReconciliationStatus, BenefitRevision,
    BenefitSequence, reconcile_inventory,
};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitError, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    UsageProviderId,
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

fn lot(
    id: u8,
    kind: BenefitKind,
    quantity: u64,
    state: BenefitState,
    expiry_at_ms: i64,
) -> Result<BenefitLotObservation, BenefitError> {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([id; 32]),
        kind,
        quantity,
        state,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(OBSERVED_AT_MS - 1_000),
        expiry: BenefitExpiry::exact_utc(expiry_at_ms)?,
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset")?,
    })
}

fn observation(
    id: u8,
    observed_at_ms: i64,
    lots: Vec<BenefitLotObservation>,
) -> Result<BenefitInventoryObservation, BenefitError> {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope()?,
        observation_id: BenefitObservationId::from_bytes([id; 32]),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots,
    })
}

#[test]
fn initial_distinct_lots_are_awarded_without_merging() -> Result<(), Box<dyn std::error::Error>> {
    let current = BenefitInventoryState::empty(scope()?);
    let input = observation(
        1,
        OBSERVED_AT_MS,
        vec![
            lot(
                1,
                BenefitKind::BankedRateLimitReset,
                1,
                BenefitState::Available,
                OBSERVED_AT_MS + 10_000,
            )?,
            lot(
                2,
                BenefitKind::UsageCredit,
                3,
                BenefitState::Available,
                OBSERVED_AT_MS + 20_000,
            )?,
        ],
    )?;

    let result = reconcile_inventory(&current, &input)?;
    assert_eq!(result.status(), BenefitReconciliationStatus::Changed);
    assert_eq!(result.state().revision(), BenefitRevision::new(1)?);
    assert_eq!(
        result.state().last_change_sequence(),
        BenefitSequence::new(2)?
    );
    assert_eq!(result.state().lots().len(), 2);
    assert_eq!(
        result
            .changes()
            .iter()
            .map(|change| change.kind())
            .collect::<Vec<_>>(),
        vec![BenefitChangeKind::Awarded, BenefitChangeKind::Awarded]
    );
    assert_ne!(
        result.state().lots()[0].lot().kind(),
        result.state().lots()[1].lot().kind()
    );
    Ok(())
}

#[test]
fn duplicate_freshness_missing_and_reappearance_are_deterministic()
-> Result<(), Box<dyn std::error::Error>> {
    let initial = observation(
        1,
        OBSERVED_AT_MS,
        vec![lot(
            1,
            BenefitKind::BankedRateLimitReset,
            1,
            BenefitState::Available,
            OBSERVED_AT_MS + 10_000,
        )?],
    )?;
    let first = reconcile_inventory(&BenefitInventoryState::empty(scope()?), &initial)?;

    let duplicate = reconcile_inventory(first.state(), &initial)?;
    assert_eq!(duplicate.status(), BenefitReconciliationStatus::Duplicate);
    assert!(duplicate.changes().is_empty());

    let freshness = observation(2, OBSERVED_AT_MS + 1, vec![initial.lots()[0].clone()])?;
    let refreshed = reconcile_inventory(first.state(), &freshness)?;
    assert_eq!(
        refreshed.status(),
        BenefitReconciliationStatus::FreshnessOnly
    );
    assert_eq!(refreshed.state().revision(), BenefitRevision::new(2)?);
    assert_eq!(
        refreshed.state().last_observed_at_ms(),
        Some(OBSERVED_AT_MS + 1)
    );

    let missing = observation(3, OBSERVED_AT_MS + 2, Vec::new())?;
    let disappeared = reconcile_inventory(refreshed.state(), &missing)?;
    assert_eq!(
        disappeared.changes()[0].kind(),
        BenefitChangeKind::DisappearedAmbiguous
    );
    assert_eq!(
        disappeared.state().lots()[0].lot().state(),
        BenefitState::Ambiguous
    );

    let still_missing = observation(4, OBSERVED_AT_MS + 3, Vec::new())?;
    let unchanged = reconcile_inventory(disappeared.state(), &still_missing)?;
    assert_eq!(
        unchanged.status(),
        BenefitReconciliationStatus::FreshnessOnly
    );
    assert!(unchanged.changes().is_empty());

    let returned = observation(5, OBSERVED_AT_MS + 4, vec![initial.lots()[0].clone()])?;
    let reappeared = reconcile_inventory(unchanged.state(), &returned)?;
    assert_eq!(
        reappeared.changes()[0].kind(),
        BenefitChangeKind::Reappeared
    );
    assert_eq!(
        reappeared.state().lots()[0].lot().state(),
        BenefitState::Available
    );
    Ok(())
}

#[test]
fn quantity_state_expiry_and_multi_field_changes_are_classified()
-> Result<(), Box<dyn std::error::Error>> {
    let original = lot(
        7,
        BenefitKind::BankedRateLimitReset,
        1,
        BenefitState::Available,
        OBSERVED_AT_MS + 10_000,
    )?;
    let initial = observation(1, OBSERVED_AT_MS, vec![original.clone()])?;
    let first = reconcile_inventory(&BenefitInventoryState::empty(scope()?), &initial)?;

    let quantity = BenefitLotObservation::new(BenefitLotObservationParts {
        quantity: 2,
        ..original.clone().into_parts()
    })?;
    let quantity_result = reconcile_inventory(
        first.state(),
        &observation(2, OBSERVED_AT_MS + 1, vec![quantity.clone()])?,
    )?;
    assert_eq!(
        quantity_result.changes()[0].kind(),
        BenefitChangeKind::QuantityChanged
    );

    let state = BenefitLotObservation::new(BenefitLotObservationParts {
        state: BenefitState::ActivationPending,
        ..quantity.clone().into_parts()
    })?;
    let state_result = reconcile_inventory(
        quantity_result.state(),
        &observation(3, OBSERVED_AT_MS + 2, vec![state.clone()])?,
    )?;
    assert_eq!(
        state_result.changes()[0].kind(),
        BenefitChangeKind::StateChanged
    );

    let expiry = BenefitLotObservation::new(BenefitLotObservationParts {
        expiry: BenefitExpiry::exact_utc(OBSERVED_AT_MS + 30_000)?,
        ..state.clone().into_parts()
    })?;
    let expiry_result = reconcile_inventory(
        state_result.state(),
        &observation(4, OBSERVED_AT_MS + 3, vec![expiry.clone()])?,
    )?;
    assert_eq!(
        expiry_result.changes()[0].kind(),
        BenefitChangeKind::ExpiryChanged
    );

    let corrected = BenefitLotObservation::new(BenefitLotObservationParts {
        quantity: 4,
        state: BenefitState::Activated,
        ..expiry.into_parts()
    })?;
    let corrected_result = reconcile_inventory(
        expiry_result.state(),
        &observation(5, OBSERVED_AT_MS + 4, vec![corrected])?,
    )?;
    assert_eq!(
        corrected_result.changes()[0].kind(),
        BenefitChangeKind::Corrected
    );
    Ok(())
}

#[test]
fn partial_observations_preserve_missing_lots_and_complete_terminal_absence_retires_them()
-> Result<(), Box<dyn std::error::Error>> {
    let available = lot(
        1,
        BenefitKind::BankedRateLimitReset,
        1,
        BenefitState::Available,
        OBSERVED_AT_MS + 10_000,
    )?;
    let activated = lot(
        2,
        BenefitKind::BankedRateLimitReset,
        1,
        BenefitState::Activated,
        OBSERVED_AT_MS + 20_000,
    )?;
    let initial = observation(1, OBSERVED_AT_MS, vec![available.clone(), activated])?;
    let first = reconcile_inventory(&BenefitInventoryState::empty(scope()?), &initial)?;

    let partial = BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope()?,
        observation_id: BenefitObservationId::from_bytes([2; 32]),
        observed_at_ms: OBSERVED_AT_MS + 1,
        fresh_until_ms: OBSERVED_AT_MS + 1_001,
        stale_after_ms: OBSERVED_AT_MS + 2_001,
        completeness: BenefitInventoryCompleteness::Partial,
        lots: vec![available],
    })?;
    let preserved = reconcile_inventory(first.state(), &partial)?;
    assert_eq!(
        preserved.status(),
        BenefitReconciliationStatus::FreshnessOnly
    );
    assert_eq!(preserved.state().lots().len(), 2);
    assert!(preserved.changes().is_empty());

    let complete = observation(3, OBSERVED_AT_MS + 2, vec![initial.lots()[0].clone()])?;
    let retired = reconcile_inventory(preserved.state(), &complete)?;
    assert_eq!(
        retired.changes()[0].kind(),
        BenefitChangeKind::RetiredTerminal
    );
    assert_eq!(retired.state().lots().len(), 1);
    Ok(())
}

#[test]
fn stale_and_conflicting_identity_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let original = observation(
        1,
        OBSERVED_AT_MS,
        vec![lot(
            1,
            BenefitKind::TemporaryUsage,
            1,
            BenefitState::Available,
            OBSERVED_AT_MS + 10_000,
        )?],
    )?;
    let first = reconcile_inventory(&BenefitInventoryState::empty(scope()?), &original)?;

    let stale = observation(2, OBSERVED_AT_MS - 1, Vec::new())?;
    assert_eq!(
        reconcile_inventory(first.state(), &stale)?.status(),
        BenefitReconciliationStatus::Stale
    );

    let conflict = observation(1, OBSERVED_AT_MS, Vec::new())?;
    assert!(reconcile_inventory(first.state(), &conflict).is_err());
    assert_eq!(
        format!("{:?}", first.state().scope_id()),
        "BenefitScopeId([redacted])"
    );
    assert_eq!(
        format!("{:?}", first.changes()[0].id()),
        "BenefitChangeId([redacted])"
    );
    Ok(())
}
