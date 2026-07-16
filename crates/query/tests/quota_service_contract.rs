use std::path::Path;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample,
    QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaWindowDefinition,
    QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey, QuotaWindowSemantics,
    UsageProviderId,
};
use tokenmaster_query::{
    PageSize, QueryClock, QueryError, QueryErrorCode, QueryFreshness, QueryQuality, QueryService,
    QueryTimeSample, QuotaCurrentRequest, QuotaTransitionPageRequest, QuotaWarningCode,
};
use tokenmaster_store::UsageStore;

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

fn key(window_id: &str) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("personal").expect("account"),
            None,
        ),
        QuotaWindowId::new(window_id).expect("window"),
    )
}

fn definition(window_id: &str) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: key(window_id),
        revision: 1,
        label_key: format!("quota.{window_id}"),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(3_600),
        reset_thresholds: None,
    })
    .expect("definition")
}

fn observation_id(value: u64) -> QuotaObservationId {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    QuotaObservationId::from_bytes(bytes)
}

#[allow(clippy::too_many_arguments)]
fn sample(
    key: &QuotaWindowKey,
    observation: u64,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    epoch: &str,
    quality: QuotaSampleQuality,
) -> QuotaSample {
    QuotaSample::new(QuotaSampleParts {
        key: key.clone(),
        observation_id: observation_id(observation),
        observed_at_ms,
        fresh_until_ms,
        stale_after_ms,
        provider_epoch_id: Some(QuotaProviderEpochId::new(epoch).expect("epoch")),
        used_ratio: Some(QuotaRatio::new(100_000).expect("used")),
        remaining_ratio: Some(QuotaRatio::new(900_000).expect("remaining")),
        units: None,
        advertised_resets_at_ms: Some(10_000),
        quality,
        source: QuotaEvidenceSource::ProviderOfficial,
        confidence: QuotaConfidence::High,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    })
    .expect("sample")
}

fn checkpoint(path: &Path) {
    Connection::open(path)
        .expect("checkpoint connection")
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

#[test]
fn quota_header_uses_provider_freshness_and_strongest_truthful_quality() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-service-quality.sqlite3");
    let fixtures = [
        (
            "fresh",
            900,
            1_100,
            1_200,
            QuotaSampleQuality::Authoritative,
        ),
        ("aging", 800, 900, 1_100, QuotaSampleQuality::Partial),
        ("stale", 700, 800, 900, QuotaSampleQuality::Conflict),
    ];
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for (index, (window, observed, fresh, stale, quality)) in fixtures.iter().enumerate() {
            let definition = definition(window);
            writer
                .apply_quota_observation(
                    &definition,
                    &sample(
                        definition.key(),
                        u64::try_from(index + 1).expect("observation"),
                        *observed,
                        *fresh,
                        *stale,
                        window,
                        *quality,
                    ),
                )
                .expect("observation");
        }
    }
    checkpoint(&path);

    let mut service = QueryService::open(
        &path,
        FixedClock {
            wall_time_ms: 1_000,
            monotonic_ms: 10,
        },
    )
    .expect("service");
    let snapshot = service
        .quota_windows(
            QuotaCurrentRequest::new(fixtures.iter().map(|value| key(value.0)).collect())
                .expect("request"),
        )
        .expect("snapshot");
    assert_eq!(snapshot.header().snapshot_generation().get(), 1);
    assert_eq!(snapshot.header().generated_at_ms(), 1_000);
    assert_eq!(snapshot.header().data_through_ms(), Some(700));
    assert_eq!(snapshot.header().freshness(), QueryFreshness::Stale);
    assert_eq!(snapshot.header().quality(), QueryQuality::Conflict);
    assert!(
        snapshot
            .header()
            .warnings()
            .contains(&QuotaWarningCode::PartialEvidence)
    );
    assert!(
        snapshot
            .header()
            .warnings()
            .contains(&QuotaWarningCode::ConflictingEvidence)
    );
    assert_eq!(
        snapshot.payload().windows()[0]
            .snapshot()
            .expect("fresh")
            .freshness(),
        QueryFreshness::Fresh
    );
    assert_eq!(
        snapshot.payload().windows()[1]
            .snapshot()
            .expect("aging")
            .freshness(),
        QueryFreshness::Aging
    );
    assert_eq!(
        snapshot.payload().windows()[2]
            .snapshot()
            .expect("stale")
            .freshness(),
        QueryFreshness::Stale
    );

    let unavailable = service
        .quota_windows(QuotaCurrentRequest::new(vec![key("missing")]).expect("missing request"))
        .expect("missing snapshot");
    assert_eq!(unavailable.header().snapshot_generation().get(), 2);
    assert_eq!(
        unavailable.header().freshness(),
        QueryFreshness::Unavailable
    );
    assert_eq!(unavailable.header().quality(), QueryQuality::Unknown);
    assert_eq!(unavailable.header().data_through_ms(), None);
    assert_eq!(unavailable.payload().windows().len(), 1);
    assert!(unavailable.payload().windows()[0].snapshot().is_none());
    assert!(
        unavailable
            .header()
            .warnings()
            .contains(&QuotaWarningCode::WindowUnavailable)
    );
}

#[test]
fn quota_clock_rollback_is_unavailable_and_stale_cursor_does_not_consume_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-service-generation.sqlite3");
    let weekly = definition("weekly");
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for (observation, epoch) in [(1, "epoch-1"), (2, "epoch-2"), (3, "epoch-3")] {
            writer
                .apply_quota_observation(
                    &weekly,
                    &sample(
                        weekly.key(),
                        observation,
                        i64::try_from(observation * 100).expect("time"),
                        i64::try_from(observation * 100 + 100).expect("fresh"),
                        i64::try_from(observation * 100 + 200).expect("stale"),
                        epoch,
                        QuotaSampleQuality::Authoritative,
                    ),
                )
                .expect("observation");
        }
    }
    checkpoint(&path);

    let mut rollback_service = QueryService::open(
        &path,
        FixedClock {
            wall_time_ms: 50,
            monotonic_ms: 1,
        },
    )
    .expect("rollback service");
    let rollback = rollback_service
        .quota_windows(QuotaCurrentRequest::new(vec![weekly.key().clone()]).expect("request"))
        .expect("rollback snapshot");
    assert_eq!(rollback.header().freshness(), QueryFreshness::Unavailable);
    assert!(
        rollback
            .header()
            .warnings()
            .contains(&QuotaWarningCode::ClockDiscontinuity)
    );

    let mut service = QueryService::open(
        &path,
        FixedClock {
            wall_time_ms: 250,
            monotonic_ms: 2,
        },
    )
    .expect("service");
    let first = service
        .quota_transitions(
            QuotaTransitionPageRequest::first(
                weekly.key().clone(),
                PageSize::new(1).expect("page size"),
            )
            .expect("first request"),
        )
        .expect("first page");
    assert_eq!(first.header().snapshot_generation().get(), 1);
    let cursor = first
        .payload()
        .next_cursor()
        .cloned()
        .expect("continuation cursor");

    {
        let mut writer = UsageStore::open(&path).expect("writer reopen");
        writer
            .apply_quota_observation(
                &weekly,
                &sample(
                    weekly.key(),
                    4,
                    400,
                    500,
                    600,
                    "epoch-4",
                    QuotaSampleQuality::Authoritative,
                ),
            )
            .expect("new reset");
    }
    checkpoint(&path);
    let stale = service
        .quota_transitions(
            QuotaTransitionPageRequest::continuation(
                weekly.key().clone(),
                PageSize::new(1).expect("page size"),
                cursor,
            )
            .expect("continuation request"),
        )
        .expect_err("stale revision");
    assert_eq!(stale.code(), QueryErrorCode::StaleSnapshot);

    let next = service
        .quota_windows(
            QuotaCurrentRequest::new(vec![weekly.key().clone()]).expect("current request"),
        )
        .expect("next successful snapshot");
    assert_eq!(next.header().snapshot_generation().get(), 2);
}
