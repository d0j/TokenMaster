use std::time::Duration;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    MAX_USAGE_PRICE_BASIS_KEYS, ScanScope, StoreErrorCode, UsageAggregateBucketWidth,
    UsageAggregateRange, UsageAggregateSegment, UsagePriceBasisQuery, UsageQueryDatasetIdentity,
    UsageReadStore, UsageReportedCostState, UsageSessionPageQuery, UsageSessionPriceBasisQuery,
    UsageStore,
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
}
