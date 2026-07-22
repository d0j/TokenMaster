use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    QuotaConfidence, QuotaEvidenceSource, QuotaObservationId, QuotaPresentationDirection,
    QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample, QuotaSampleParts,
    QuotaSampleQuality, QuotaScope, QuotaUnitId, QuotaUnits, QuotaWindowDefinition,
    QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey, QuotaWindowSemantics,
    UsageProviderId,
};
use tokenmaster_engine::RefreshOutcome;
use tokenmaster_runtime::{
    ProviderPollErrorCode, ProviderQuotaObservation, ProviderQuotaPoll,
    ProviderQuotaRefreshFailure, ProviderQuotaRuntime, ProviderQuotaSource,
};
use tokenmaster_store::{BenefitOverviewQuery, QuotaOverviewQuery, UsageReadStore};

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

fn opaque(value: u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&value.to_be_bytes());
    bytes
}

fn definition() -> QuotaWindowDefinition {
    let key = QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("synthetic").expect("provider"),
            QuotaAccountId::new("synthetic-account").expect("account"),
            None,
        ),
        QuotaWindowId::new("weekly").expect("window"),
    );
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key,
        revision: 1,
        label_key: "quota.synthetic.weekly".to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(604_800),
        reset_thresholds: None,
    })
    .expect("definition")
}

fn sample(definition: &QuotaWindowDefinition) -> QuotaSample {
    QuotaSample::new(QuotaSampleParts {
        key: definition.key().clone(),
        observation_id: QuotaObservationId::from_bytes(opaque(1)),
        observed_at_ms: OBSERVED_AT_MS,
        fresh_until_ms: OBSERVED_AT_MS + 10_000,
        stale_after_ms: OBSERVED_AT_MS + 20_000,
        provider_epoch_id: Some(QuotaProviderEpochId::new("synthetic-epoch").expect("epoch")),
        used_ratio: Some(QuotaRatio::new(420_000).expect("used ratio")),
        remaining_ratio: Some(QuotaRatio::new(580_000).expect("remaining ratio")),
        units: Some(
            QuotaUnits::new(
                QuotaUnitId::new("requests").expect("unit"),
                Some(42),
                Some(58),
                Some(100),
            )
            .expect("units"),
        ),
        advertised_resets_at_ms: Some(OBSERVED_AT_MS + 604_800_000),
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderLocal,
        confidence: QuotaConfidence::High,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    })
    .expect("sample")
}

fn benefits() -> BenefitInventoryObservation {
    let scope = BenefitScope::new(
        UsageProviderId::new("synthetic").expect("provider"),
        QuotaAccountId::new("synthetic-account").expect("account"),
        None,
    );
    let lot = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes(opaque(2)),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 1,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(OBSERVED_AT_MS - 1_000),
        expiry: BenefitExpiry::exact_utc(OBSERVED_AT_MS + 100_000).expect("expiry"),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.synthetic.banked_reset").expect("label"),
    })
    .expect("lot");
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id: BenefitObservationId::from_bytes(opaque(3)),
        observed_at_ms: OBSERVED_AT_MS,
        fresh_until_ms: OBSERVED_AT_MS + 10_000,
        stale_after_ms: OBSERVED_AT_MS + 20_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots: vec![lot],
    })
    .expect("benefits")
}

struct SyntheticSource;

impl ProviderQuotaSource for SyntheticSource {
    fn poll(&mut self, observed_at_ms: i64) -> Result<ProviderQuotaPoll, ProviderPollErrorCode> {
        assert!(
            observed_at_ms > 0,
            "runtime supplies a wall-clock observation time"
        );
        let definition = definition();
        Ok(ProviderQuotaPoll::new(
            observed_at_ms,
            vec![ProviderQuotaObservation::new(
                definition.clone(),
                sample(&definition),
            )],
            Some(benefits()),
        ))
    }
}

struct UnavailableSource;

impl ProviderQuotaSource for UnavailableSource {
    fn poll(&mut self, _observed_at_ms: i64) -> Result<ProviderQuotaPoll, ProviderPollErrorCode> {
        Err(ProviderPollErrorCode::Unavailable)
    }
}

struct OversizedSource;

impl ProviderQuotaSource for OversizedSource {
    fn poll(&mut self, observed_at_ms: i64) -> Result<ProviderQuotaPoll, ProviderPollErrorCode> {
        let definition = definition();
        let quota = (0..=tokenmaster_runtime::MAX_PROVIDER_QUOTA_WINDOWS)
            .map(|_| ProviderQuotaObservation::new(definition.clone(), sample(&definition)))
            .collect();
        Ok(ProviderQuotaPoll::new(observed_at_ms, quota, None))
    }
}

fn wait(runtime: &ProviderQuotaRuntime) -> tokenmaster_engine::WorkerCompletion {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(completion) = runtime.try_completion().expect("completion") {
            return completion;
        }
        assert!(Instant::now() < deadline, "provider poll timed out");
        std::thread::yield_now();
    }
}

#[test]
fn synthetic_provider_poll_publishes_quota_and_benefits_to_the_real_store() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("synthetic-quota.sqlite3");
    let mut runtime =
        ProviderQuotaRuntime::start_with_source(&archive, SyntheticSource).expect("runtime");

    assert_eq!(wait(&runtime).outcome(), RefreshOutcome::Completed);
    assert_eq!(
        runtime
            .snapshot()
            .expect("snapshot")
            .refresh()
            .quota_observation_count(),
        1
    );
    assert_eq!(
        runtime
            .snapshot()
            .expect("snapshot")
            .refresh()
            .benefit_observation_count(),
        1
    );
    assert_eq!(
        runtime.shutdown().expect("shutdown"),
        tokenmaster_runtime::ProviderQuotaRuntimePhase::Stopped
    );

    let mut reader = UsageReadStore::open(&archive).expect("reader");
    assert_eq!(
        reader
            .capture_quota_overview(
                QuotaOverviewQuery::new(Duration::from_secs(2)).expect("quota query")
            )
            .expect("quota overview")
            .windows()
            .len(),
        1
    );
    assert_eq!(
        reader
            .capture_benefit_overview(
                BenefitOverviewQuery::new(Duration::from_secs(2)).expect("benefit query")
            )
            .expect("benefit overview")
            .scopes()
            .len(),
        1
    );
}

#[test]
fn provider_failure_is_stable_and_does_not_expose_provider_error_text() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("unavailable.sqlite3");
    let mut runtime =
        ProviderQuotaRuntime::start_with_source(&archive, UnavailableSource).expect("runtime");

    assert_eq!(wait(&runtime).outcome(), RefreshOutcome::Failed);
    let snapshot = runtime.snapshot().expect("snapshot");
    assert_eq!(
        snapshot.refresh().failure(),
        Some(ProviderQuotaRefreshFailure::Transport(
            ProviderPollErrorCode::Unavailable
        ))
    );
    assert_eq!(
        snapshot.refresh().failure().expect("failure").stable_code(),
        "unavailable"
    );
    assert!(!format!("{runtime:?}").contains("synthetic-account"));
    assert!(
        !archive.exists(),
        "source failure must precede store publication"
    );
    runtime.shutdown().expect("shutdown");
}

#[test]
fn oversized_provider_poll_fails_before_store_publication() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("oversized.sqlite3");
    let mut runtime =
        ProviderQuotaRuntime::start_with_source(&archive, OversizedSource).expect("runtime");

    assert_eq!(wait(&runtime).outcome(), RefreshOutcome::Failed);
    assert_eq!(
        runtime
            .snapshot()
            .expect("snapshot")
            .refresh()
            .failure()
            .expect("failure")
            .stable_code(),
        "capacity_exceeded"
    );
    assert!(
        !archive.exists(),
        "bounded rejection must precede store publication"
    );
    runtime.shutdown().expect("shutdown");
}
