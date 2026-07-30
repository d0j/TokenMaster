use std::time::Duration;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    MAX_USAGE_PRICE_BASIS_KEYS, MAX_USAGE_PRICE_BASIS_TARGETS, ScanScope, StoreErrorCode,
    UsageAggregateBucketWidth, UsageAggregateRange, UsageAggregateSegment, UsageAnalyticsQuery,
    UsageBreakdownKind, UsageBreakdownPriceBasisQuery, UsagePriceBasisBatchQuery,
    UsagePriceBasisQuery, UsageQueryDatasetIdentity, UsageReadStore, UsageReportedCostState,
    UsageSessionBreakdownPriceBasisQuery, UsageSessionDetailQuery, UsageSessionPageQuery,
    UsageSessionPriceBasisBatchQuery, UsageSessionPriceBasisQuery, UsageStore,
};

fn seed_current_price_archive() -> (TempDir, std::path::PathBuf) {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("price-query.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut connection = Connection::open(&path).expect("open fixture");
    connection
        .execute(
            "INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 0, 0, 1, 1, NULL)",
            [],
        )
        .expect("current revision");
    connection
        .execute(
            "UPDATE usage_archive_state
             SET current_revision_id = 0, incremental_state = 'partial'
             WHERE singleton_id = 1",
            [],
        )
        .expect("current publication");
    let transaction = connection.transaction().expect("fixture transaction");
    for index in 0_u16..520 {
        let mut fingerprint = [0_u8; 32];
        fingerprint[0..2].copy_from_slice(&index.to_be_bytes());
        let reported = (index % 2 == 0).then_some(i64::from(index) + 1);
        transaction
            .execute(
                "INSERT INTO usage_event(
                   fingerprint, event_id, selected_file_key, selected_generation,
                   selected_source_offset, provider_id, profile_id, session_id, source_id,
                   timestamp_seconds, timestamp_nanos, model, input_tokens, cached_tokens,
                   output_tokens, reasoning_tokens, total_tokens, fallback_model,
                   long_context, service_tier, reported_cost_usd_micros,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 'codex', 'default', 'session', 'fixture',
                   1, 0, ?5, 10, 2, 3, 4, 17, 0, 'no', NULL, ?6,
                   0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    fingerprint.as_slice(),
                    format!("event-{index:03}"),
                    [9_u8; 32].as_slice(),
                    i64::from(index),
                    format!("model-{index:03}"),
                    reported,
                ],
            )
            .expect("price event");
    }
    transaction.commit().expect("commit fixture");
    drop(connection);
    (directory, path)
}

fn minute_range() -> UsageAggregateRange {
    UsageAggregateRange::new(
        vec![
            UsageAggregateSegment::new(UsageAggregateBucketWidth::Minute, 0, 60)
                .expect("minute segment"),
        ]
        .into_boxed_slice(),
    )
    .expect("minute range")
}

#[test]
fn range_price_basis_is_deterministic_capped_and_exact_about_omissions() {
    let (_directory, path) = seed_current_price_archive();
    let mut store = UsageReadStore::open(&path).expect("price read store");
    let capture = store
        .capture_usage_price_basis(
            UsagePriceBasisQuery::new(None, minute_range(), Box::default(), Duration::from_secs(2))
                .expect("price query"),
        )
        .expect("price basis capture");

    assert_eq!(capture.rows().len(), MAX_USAGE_PRICE_BASIS_KEYS);
    assert_eq!(capture.rows()[0].key().model(), "model-518");
    assert_eq!(capture.rows()[259].key().model(), "model-000");
    assert_eq!(capture.rows()[260].key().model(), "model-001");
    assert_eq!(capture.rows()[511].key().model(), "model-503");
    assert_eq!(capture.included().event_count(), 512);
    assert_eq!(capture.omitted().event_count(), 8);
    assert_eq!(capture.omitted().calculable_event_count(), 8);
    assert_eq!(capture.omitted().reported_cost_count(), 0);
    assert_eq!(
        capture.rows()[0].key().reported_cost_state(),
        UsageReportedCostState::Present
    );
    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::ReplayRevision {
            revision_id: 0,
            dataset_generation: 520,
        }
    );
}

#[test]
fn session_price_basis_reuses_exact_session_and_dataset_identity() {
    let (_directory, path) = seed_current_price_archive();
    let mut store = UsageReadStore::open(&path).expect("price read store");
    let page = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(None, None, Box::default(), 1, Duration::from_secs(2))
                .expect("session page query"),
        )
        .expect("session page");
    let session = page.sessions()[0].key().clone();
    let dataset = page.publication().dataset_identity();
    let batch = store
        .capture_usage_session_price_basis_batch(
            UsageSessionPriceBasisBatchQuery::new(
                dataset,
                vec![session.clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("session batch price query"),
        )
        .expect("session batch price basis");
    assert_eq!(batch.publication().dataset_identity(), dataset);
    assert_eq!(batch.targets().len(), 1);
    assert_eq!(batch.targets()[0].rows().len(), MAX_USAGE_PRICE_BASIS_KEYS);
    assert_eq!(batch.targets()[0].omitted().event_count(), 8);

    let detail = store
        .capture_usage_session_detail(
            UsageSessionDetailQuery::new(dataset, session.clone(), Duration::from_secs(2))
                .expect("session detail query"),
        )
        .expect("session detail");
    let model_breakdown = &detail.detail().expect("detail row").breakdowns()[0];
    let targets = model_breakdown
        .items()
        .iter()
        .map(|item| item.identity().clone())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let breakdown = store
        .capture_usage_session_breakdown_price_basis(
            UsageSessionBreakdownPriceBasisQuery::new(
                dataset,
                session.clone(),
                model_breakdown.kind(),
                targets,
                Duration::from_secs(2),
            )
            .expect("session breakdown price query"),
        )
        .expect("session breakdown price basis");
    assert_eq!(breakdown.targets().len(), 256);
    assert!(breakdown.targets().iter().all(|target| {
        target.rows().len() == 1
            && target.included().event_count() == 1
            && target.omitted().event_count() == 0
    }));

    let capture = store
        .capture_usage_session_price_basis(
            UsageSessionPriceBasisQuery::new(dataset, session, Duration::from_secs(2))
                .expect("session price query"),
        )
        .expect("session price basis");

    assert_eq!(capture.publication().dataset_identity(), dataset);
    assert_eq!(capture.rows().len(), MAX_USAGE_PRICE_BASIS_KEYS);
    assert_eq!(capture.omitted().event_count(), 8);
    assert_eq!(capture.omitted().calculable_event_count(), 8);
    assert_eq!(capture.omitted().reported_cost_count(), 0);
}

#[test]
fn price_basis_enforces_scope_bounds_deadline_and_exact_dataset() {
    let (_directory, path) = seed_current_price_archive();
    let mut store = UsageReadStore::open(&path).expect("price read store");
    let matching = ScanScope::new("codex", "default").expect("matching scope");
    let capture = store
        .capture_usage_price_basis(
            UsagePriceBasisQuery::new(
                None,
                minute_range(),
                vec![matching.clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("scoped price query"),
        )
        .expect("scoped price basis");
    assert_eq!(capture.included().event_count(), 512);
    assert_eq!(capture.omitted().event_count(), 8);

    let absent = ScanScope::new("codex", "other").expect("absent scope");
    let empty = store
        .capture_usage_price_basis(
            UsagePriceBasisQuery::new(
                None,
                minute_range(),
                vec![absent].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("absent scope query"),
        )
        .expect("absent scope capture");
    assert!(empty.rows().is_empty());
    assert_eq!(empty.included().event_count(), 0);
    assert_eq!(empty.omitted().event_count(), 0);

    let stale = store
        .capture_usage_price_basis(
            UsagePriceBasisQuery::new(
                Some(UsageQueryDatasetIdentity::ReplayRevision {
                    revision_id: 0,
                    dataset_generation: 519,
                }),
                minute_range(),
                Box::default(),
                Duration::from_secs(2),
            )
            .expect("stale price query"),
        )
        .expect_err("stale dataset must fail");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);

    let duplicate = UsagePriceBasisQuery::new(
        None,
        minute_range(),
        vec![matching.clone(), matching].into_boxed_slice(),
        Duration::from_secs(2),
    )
    .expect_err("duplicate scopes must fail");
    assert_eq!(duplicate.code(), StoreErrorCode::InvalidValue);
    assert!(
        UsagePriceBasisQuery::new(None, minute_range(), Box::default(), Duration::ZERO,).is_err()
    );

    assert!(
        UsagePriceBasisBatchQuery::new(
            None,
            Box::default(),
            Box::default(),
            Duration::from_secs(1),
        )
        .is_err()
    );
    let too_many_ranges = (0..=MAX_USAGE_PRICE_BASIS_TARGETS)
        .map(|_| minute_range())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let error = UsagePriceBasisBatchQuery::new(
        None,
        too_many_ranges,
        Box::default(),
        Duration::from_secs(1),
    )
    .expect_err("too many batch targets");
    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_USAGE_PRICE_BASIS_TARGETS as u64));
}

#[test]
fn range_batch_uses_one_global_key_budget_with_exact_per_target_omissions() {
    let (_directory, path) = seed_current_price_archive();
    let connection = Connection::open(&path).expect("rebucket fixture");
    connection
        .execute(
            "UPDATE usage_event SET timestamp_seconds = 61
             WHERE selected_source_offset % 2 = 1",
            [],
        )
        .expect("rebucket odd events");
    drop(connection);

    let ranges = [
        UsageAggregateRange::new(
            vec![
                UsageAggregateSegment::new(UsageAggregateBucketWidth::Minute, 0, 60)
                    .expect("first segment"),
            ]
            .into_boxed_slice(),
        )
        .expect("first range"),
        UsageAggregateRange::new(
            vec![
                UsageAggregateSegment::new(UsageAggregateBucketWidth::Minute, 60, 120)
                    .expect("second segment"),
            ]
            .into_boxed_slice(),
        )
        .expect("second range"),
    ];
    let mut store = UsageReadStore::open(&path).expect("batch price store");
    let capture = store
        .capture_usage_price_basis_batch(
            UsagePriceBasisBatchQuery::new(
                None,
                ranges.into(),
                Box::default(),
                Duration::from_secs(2),
            )
            .expect("batch price query"),
        )
        .expect("batch price capture");

    assert_eq!(MAX_USAGE_PRICE_BASIS_TARGETS, 401);
    assert_eq!(capture.targets().len(), 2);
    assert_eq!(
        capture
            .targets()
            .iter()
            .map(|target| target.rows().len())
            .sum::<usize>(),
        MAX_USAGE_PRICE_BASIS_KEYS
    );
    for target in capture.targets() {
        assert_eq!(target.rows().len(), 256);
        assert_eq!(target.included().event_count(), 256);
        assert_eq!(target.omitted().event_count(), 4);
        assert_eq!(target.omitted().calculable_event_count(), 4);
    }
    assert_eq!(capture.targets()[0].omitted().reported_cost_count(), 4);
    assert_eq!(capture.targets()[1].omitted().reported_cost_count(), 0);
    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::ReplayRevision {
            revision_id: 0,
            dataset_generation: 780,
        }
    );
}

#[test]
fn range_breakdown_price_batch_matches_only_the_bounded_visible_targets() {
    let (_directory, path) = seed_current_price_archive();
    let mut store = UsageReadStore::open(&path).expect("breakdown price store");
    let analytics = store
        .capture_usage_analytics(
            UsageAnalyticsQuery::new(
                None,
                minute_range(),
                Box::default(),
                vec![UsageBreakdownKind::Model].into_boxed_slice(),
                Box::default(),
                Duration::from_secs(2),
            )
            .expect("analytics query"),
        )
        .expect("analytics capture");
    let visible = &analytics.breakdowns()[0];
    assert!(visible.truncated());
    assert_eq!(visible.items().len(), 256);
    let targets = visible
        .items()
        .iter()
        .map(|item| item.identity().clone())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let prices = store
        .capture_usage_breakdown_price_basis(
            UsageBreakdownPriceBasisQuery::new(
                analytics.publication().dataset_identity(),
                minute_range(),
                Box::default(),
                visible.kind(),
                targets,
                Duration::from_secs(2),
            )
            .expect("breakdown price query"),
        )
        .expect("breakdown price capture");

    assert_eq!(prices.targets().len(), visible.items().len());
    assert!(prices.targets().iter().all(|target| {
        target.rows().len() == 1
            && target.included().event_count() == 1
            && target.omitted().event_count() == 0
    }));
}
