use std::path::Path;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
    QuotaResetThresholds, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId,
    QuotaWindowKey, QuotaWindowSemantics, UsageProviderId,
};
use tokenmaster_query::{
    PageSize, QueryClock, QueryError, QueryErrorCode, QueryFreshness, QueryQuality, QueryService,
    QueryTimeSample, QuotaAllowanceChangeKind, QuotaCurrentRequest, QuotaDetectionTime,
    QuotaTransitionKind, QuotaTransitionPageRequest, QuotaWarningCode,
};
use tokenmaster_store::{MAX_QUOTA_CURRENT_WINDOWS, UsageStore};

#[derive(Clone, Copy)]
struct FixedClock {
    wall_time_ms: i64,
    monotonic_ms: u64,
}

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(self.wall_time_ms, self.monotonic_ms))
    }
}

fn window_key(account_id: &str, window_id: &str) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new(account_id).expect("account"),
            None,
        ),
        QuotaWindowId::new(window_id).expect("window"),
    )
}

fn definition(
    account_id: &str,
    window_id: &str,
    thresholds: Option<QuotaResetThresholds>,
) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key(account_id, window_id),
        revision: 1,
        label_key: format!("quota.{window_id}"),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(604_800),
        reset_thresholds: thresholds,
    })
    .expect("definition")
}

#[derive(Clone, Copy)]
struct SampleSpec<'a> {
    observation: u64,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    provider_epoch: &'a str,
    used_ppm: u32,
    used_units: Option<u64>,
    capacity_units: Option<u64>,
    quality: QuotaSampleQuality,
    source: QuotaEvidenceSource,
    confidence: QuotaConfidence,
    reset_evidence: QuotaResetEvidence,
    reset_occurred_at_ms: Option<i64>,
    advertised_resets_at_ms: Option<i64>,
}

fn observation_id(value: u64) -> QuotaObservationId {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    QuotaObservationId::from_bytes(bytes)
}

fn sample(key: &QuotaWindowKey, spec: SampleSpec<'_>) -> QuotaSample {
    let units = spec.capacity_units.map(|capacity| {
        let used = spec.used_units.expect("used with capacity");
        QuotaUnits::new(
            QuotaUnitId::new("requests").expect("unit"),
            Some(used),
            Some(capacity - used),
            Some(capacity),
        )
        .expect("units")
    });
    QuotaSample::new(QuotaSampleParts {
        key: key.clone(),
        observation_id: observation_id(spec.observation),
        observed_at_ms: spec.observed_at_ms,
        fresh_until_ms: spec.fresh_until_ms,
        stale_after_ms: spec.stale_after_ms,
        provider_epoch_id: Some(
            QuotaProviderEpochId::new(spec.provider_epoch).expect("provider epoch"),
        ),
        used_ratio: Some(QuotaRatio::new(spec.used_ppm).expect("used ratio")),
        remaining_ratio: Some(QuotaRatio::new(1_000_000 - spec.used_ppm).expect("remaining ratio")),
        units,
        advertised_resets_at_ms: spec.advertised_resets_at_ms,
        quality: spec.quality,
        source: spec.source,
        confidence: spec.confidence,
        reset_evidence: spec.reset_evidence,
        reset_occurred_at_ms: spec.reset_occurred_at_ms,
    })
    .expect("sample")
}

fn apply(writer: &mut UsageStore, definition: &QuotaWindowDefinition, spec: SampleSpec<'_>) {
    writer
        .apply_quota_observation(definition, &sample(definition.key(), spec))
        .expect("apply observation");
}

fn checkpoint(path: &Path) {
    Connection::open(path)
        .expect("checkpoint connection")
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

#[test]
fn quota_values_preserve_reset_kinds_allowance_boundaries_and_private_debug() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-value-contract.sqlite3");
    let weekly = definition("personal", "weekly", None);
    let scheduled = definition("personal", "scheduled", None);
    let unknown = definition(
        "personal",
        "unknown",
        Some(
            QuotaResetThresholds::new(
                Some(QuotaRatio::new(50_000).expect("post used")),
                Some(QuotaRatio::new(950_000).expect("post remaining")),
                Some(QuotaRatio::new(500_000).expect("drop")),
            )
            .expect("thresholds"),
        ),
    );
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        apply(
            &mut writer,
            &weekly,
            SampleSpec {
                observation: 1,
                observed_at_ms: 100,
                fresh_until_ms: 200,
                stale_after_ms: 300,
                provider_epoch: "private-epoch-1",
                used_ppm: 800_000,
                used_units: Some(800),
                capacity_units: Some(1_000),
                quality: QuotaSampleQuality::Authoritative,
                source: QuotaEvidenceSource::ProviderOfficial,
                confidence: QuotaConfidence::High,
                reset_evidence: QuotaResetEvidence::None,
                reset_occurred_at_ms: None,
                advertised_resets_at_ms: Some(1_000),
            },
        );
        apply(
            &mut writer,
            &weekly,
            SampleSpec {
                observation: 2,
                observed_at_ms: 200,
                fresh_until_ms: 300,
                stale_after_ms: 400,
                provider_epoch: "private-epoch-1",
                used_ppm: 400_000,
                used_units: Some(800),
                capacity_units: Some(2_000),
                quality: QuotaSampleQuality::Authoritative,
                source: QuotaEvidenceSource::ProviderOfficial,
                confidence: QuotaConfidence::High,
                reset_evidence: QuotaResetEvidence::None,
                reset_occurred_at_ms: None,
                advertised_resets_at_ms: Some(1_000),
            },
        );
        apply(
            &mut writer,
            &weekly,
            SampleSpec {
                observation: 3,
                observed_at_ms: 300,
                fresh_until_ms: 400,
                stale_after_ms: 500,
                provider_epoch: "private-epoch-2",
                used_ppm: 50_000,
                used_units: Some(100),
                capacity_units: Some(2_000),
                quality: QuotaSampleQuality::Authoritative,
                source: QuotaEvidenceSource::ProviderOfficial,
                confidence: QuotaConfidence::High,
                reset_evidence: QuotaResetEvidence::None,
                reset_occurred_at_ms: None,
                advertised_resets_at_ms: Some(1_000),
            },
        );
        apply(
            &mut writer,
            &weekly,
            SampleSpec {
                observation: 4,
                observed_at_ms: 400,
                fresh_until_ms: 500,
                stale_after_ms: 600,
                provider_epoch: "private-epoch-3",
                used_ppm: 10_000,
                used_units: Some(20),
                capacity_units: Some(2_000),
                quality: QuotaSampleQuality::Authoritative,
                source: QuotaEvidenceSource::Manual,
                confidence: QuotaConfidence::High,
                reset_evidence: QuotaResetEvidence::ManualOrBanked,
                reset_occurred_at_ms: Some(350),
                advertised_resets_at_ms: Some(1_000),
            },
        );
        for (definition, first, second) in [
            (
                &scheduled,
                SampleSpec {
                    observation: 10,
                    observed_at_ms: 100,
                    fresh_until_ms: 200,
                    stale_after_ms: 300,
                    provider_epoch: "scheduled-1",
                    used_ppm: 900_000,
                    used_units: None,
                    capacity_units: None,
                    quality: QuotaSampleQuality::Authoritative,
                    source: QuotaEvidenceSource::ProviderOfficial,
                    confidence: QuotaConfidence::High,
                    reset_evidence: QuotaResetEvidence::None,
                    reset_occurred_at_ms: None,
                    advertised_resets_at_ms: Some(200),
                },
                SampleSpec {
                    observation: 11,
                    observed_at_ms: 200,
                    fresh_until_ms: 300,
                    stale_after_ms: 400,
                    provider_epoch: "scheduled-2",
                    used_ppm: 20_000,
                    used_units: None,
                    capacity_units: None,
                    quality: QuotaSampleQuality::Authoritative,
                    source: QuotaEvidenceSource::ProviderOfficial,
                    confidence: QuotaConfidence::High,
                    reset_evidence: QuotaResetEvidence::None,
                    reset_occurred_at_ms: None,
                    advertised_resets_at_ms: Some(1_000),
                },
            ),
            (
                &unknown,
                SampleSpec {
                    observation: 20,
                    observed_at_ms: 100,
                    fresh_until_ms: 200,
                    stale_after_ms: 300,
                    provider_epoch: "unknown-epoch",
                    used_ppm: 900_000,
                    used_units: None,
                    capacity_units: None,
                    quality: QuotaSampleQuality::Authoritative,
                    source: QuotaEvidenceSource::ProviderOfficial,
                    confidence: QuotaConfidence::High,
                    reset_evidence: QuotaResetEvidence::None,
                    reset_occurred_at_ms: None,
                    advertised_resets_at_ms: None,
                },
                SampleSpec {
                    observation: 21,
                    observed_at_ms: 200,
                    fresh_until_ms: 300,
                    stale_after_ms: 400,
                    provider_epoch: "unknown-epoch",
                    used_ppm: 10_000,
                    used_units: None,
                    capacity_units: None,
                    quality: QuotaSampleQuality::Authoritative,
                    source: QuotaEvidenceSource::ProviderOfficial,
                    confidence: QuotaConfidence::High,
                    reset_evidence: QuotaResetEvidence::None,
                    reset_occurred_at_ms: None,
                    advertised_resets_at_ms: None,
                },
            ),
        ] {
            apply(&mut writer, definition, first);
            apply(&mut writer, definition, second);
        }
    }
    checkpoint(&path);

    let mut service = QueryService::open(
        &path,
        FixedClock {
            wall_time_ms: 450,
            monotonic_ms: 1,
        },
    )
    .expect("service");
    let current = service
        .quota_windows(
            QuotaCurrentRequest::new(vec![
                weekly.key().clone(),
                scheduled.key().clone(),
                unknown.key().clone(),
            ])
            .expect("current request"),
        )
        .expect("current snapshot");
    assert_eq!(current.header().filters().len(), 3);
    assert_eq!(current.header().freshness(), QueryFreshness::Stale);
    assert_eq!(current.header().quality(), QueryQuality::Authoritative);
    assert_eq!(current.payload().windows().len(), 3);
    let weekly_value = current.payload().windows()[0]
        .snapshot()
        .expect("weekly available");
    assert_eq!(
        weekly_value
            .current_sample()
            .used_ratio()
            .expect("used ratio")
            .parts_per_million(),
        10_000
    );
    assert_eq!(
        weekly_value
            .current_sample()
            .units()
            .expect("units")
            .capacity(),
        Some(2_000)
    );
    assert_eq!(weekly_value.epoch().last_transition_sequence(), 3);
    assert_eq!(
        weekly_value
            .last_transition()
            .expect("last transition")
            .kind(),
        QuotaTransitionKind::ManualOrBankedReset
    );

    let weekly_page = service
        .quota_transitions(
            QuotaTransitionPageRequest::first(
                weekly.key().clone(),
                PageSize::new(3).expect("page size"),
            )
            .expect("transition request"),
        )
        .expect("weekly transitions");
    assert_eq!(
        weekly_page
            .payload()
            .transitions()
            .iter()
            .map(|transition| transition.sequence())
            .collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(
        weekly_page.payload().transitions()[0].kind(),
        QuotaTransitionKind::ManualOrBankedReset
    );
    assert_eq!(
        weekly_page.payload().transitions()[0].detection_time(),
        QuotaDetectionTime::Exact { at_ms: 350 }
    );
    assert_eq!(
        weekly_page.payload().transitions()[1].kind(),
        QuotaTransitionKind::EarlyReset
    );
    assert_eq!(
        weekly_page.payload().transitions()[1].detection_time(),
        QuotaDetectionTime::Interval {
            after_ms: 200,
            at_or_before_ms: 300,
        }
    );
    let allowance = weekly_page.payload().transitions()[2]
        .allowance_change()
        .expect("allowance change");
    assert_eq!(allowance.kind(), QuotaAllowanceChangeKind::Increased);
    assert_eq!(allowance.old_units().capacity(), Some(1_000));
    assert_eq!(allowance.new_units().capacity(), Some(2_000));

    for (definition, expected) in [
        (&scheduled, QuotaTransitionKind::ScheduledReset),
        (&unknown, QuotaTransitionKind::UnknownReset),
    ] {
        let page = service
            .quota_transitions(
                QuotaTransitionPageRequest::first(
                    definition.key().clone(),
                    PageSize::new(16).expect("page size"),
                )
                .expect("page request"),
            )
            .expect("transition page");
        assert_eq!(page.payload().transitions()[0].kind(), expected);
    }

    let first_page = service
        .quota_transitions(
            QuotaTransitionPageRequest::first(
                weekly.key().clone(),
                PageSize::new(1).expect("page size"),
            )
            .expect("first page"),
        )
        .expect("first page capture");
    assert!(first_page.payload().has_more());
    let cursor = first_page.payload().next_cursor().cloned().expect("cursor");
    assert!(format!("{cursor:?}").contains("[redacted]"));
    let changed_filter = QuotaTransitionPageRequest::continuation(
        scheduled.key().clone(),
        PageSize::new(1).expect("page size"),
        cursor.clone(),
    )
    .expect_err("changed filter");
    assert_eq!(changed_filter.code(), QueryErrorCode::InvalidValue);
    let continuation = service
        .quota_transitions(
            QuotaTransitionPageRequest::continuation(
                weekly.key().clone(),
                PageSize::new(1).expect("page size"),
                cursor,
            )
            .expect("continuation"),
        )
        .expect("second page");
    assert_eq!(continuation.payload().transitions()[0].sequence(), 2);

    for private in ["personal", "weekly", "private-epoch-3"] {
        assert!(
            !format!("{current:?}{weekly_page:?}").contains(private),
            "quota Debug exposed {private}"
        );
    }
}

#[test]
fn quota_request_and_warning_values_are_bounded_and_stable() {
    let too_many = (0..=MAX_QUOTA_CURRENT_WINDOWS)
        .map(|index| window_key("personal", &format!("window-{index}")))
        .collect();
    let error = QuotaCurrentRequest::new(too_many).expect_err("window cap");
    assert_eq!(error.code(), QueryErrorCode::CapacityExceeded);
    let key = window_key("personal", "weekly");
    let duplicate = QuotaCurrentRequest::new(vec![key.clone(), key]).expect_err("duplicate window");
    assert_eq!(duplicate.code(), QueryErrorCode::InvalidValue);
    assert_eq!(
        QuotaWarningCode::WindowUnavailable.stable_code(),
        "window_unavailable"
    );
    assert_eq!(
        QuotaWarningCode::ClockDiscontinuity.stable_code(),
        "clock_discontinuity"
    );
}
