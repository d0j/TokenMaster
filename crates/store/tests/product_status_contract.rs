use std::time::Duration;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_store::{
    ArchivePublicationQuality, ProductAggregateState, ProductDataStatusQuery, StoreErrorCode,
    UsageQueryDatasetIdentity, UsageReadStore, UsageStore,
};

#[test]
fn fresh_archive_status_is_exact_empty_and_independently_zero_revisioned() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("product-status.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));

    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let capture = reader
        .capture_product_data_status(
            ProductDataStatusQuery::new(Duration::from_secs(2)).expect("query"),
        )
        .expect("status capture");

    let usage = capture.usage();
    assert_eq!(usage.publication_generation(), 0);
    assert_eq!(usage.dataset_identity(), UsageQueryDatasetIdentity::Empty);
    assert!(usage.accounting_versions_current());
    assert_eq!(usage.data_through_ms(), None);
    assert_eq!(usage.quality(), ArchivePublicationQuality::Empty);
    assert_eq!(usage.scope_count(), 0);
    assert!(!usage.replay_staging());
    assert_eq!(usage.aggregate().state(), ProductAggregateState::Ready);
    assert_eq!(usage.aggregate().expected_dataset_generation(), 0);
    assert_eq!(usage.aggregate().active_generation(), 0);
    assert_eq!(usage.aggregate().current_event_count(), 0);
    assert_eq!(usage.aggregate().legacy_event_count(), 0);
    assert_eq!(usage.aggregate().progress(), None);

    assert_eq!(capture.quota().revision(), 0);
    assert_eq!(capture.quota().retained_sample_count(), 0);
    assert_eq!(capture.quota().retained_epoch_count(), 0);
    assert_eq!(capture.quota().retained_transition_count(), 0);
    assert_eq!(capture.quota().last_published_at_ms(), None);

    assert_eq!(capture.benefit().revision(), 0);
    assert_eq!(capture.benefit().current_lot_count(), 0);
    assert_eq!(capture.benefit().retained_change_count(), 0);
    assert_eq!(capture.benefit().pending_due_count(), 0);
    assert_eq!(capture.benefit().retained_delivery_count(), 0);
    assert_eq!(capture.benefit().last_published_at_ms(), None);

    assert_eq!(capture.git().publication_revision(), 0);
    assert_eq!(capture.git().repository_count(), 0);
    assert_eq!(capture.git().association_count(), 0);
    assert_eq!(capture.git().last_published_at_ms(), None);

    let debug = format!("{capture:?}");
    assert!(!debug.contains(path.to_string_lossy().as_ref()));
    assert!(!debug.contains("installation_salt"));
    assert!(!debug.contains("SELECT "));
}

#[test]
fn status_query_rejects_zero_and_overlong_deadlines_before_sql() {
    assert_eq!(
        ProductDataStatusQuery::new(Duration::ZERO)
            .expect_err("zero deadline")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        ProductDataStatusQuery::new(Duration::from_millis(2_001))
            .expect_err("overlong deadline")
            .code(),
        StoreErrorCode::InvalidValue
    );
}

#[test]
fn every_aggregate_state_is_visible_without_fabricating_progress() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("aggregate-status.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let query = || ProductDataStatusQuery::new(Duration::from_secs(2)).expect("query");

    let connection = Connection::open(&path).expect("writer connection");
    connection
        .execute_batch(
            "UPDATE usage_aggregate_state
             SET state = 'rebuild_required', failure_code = NULL,
                 rebuild_aggregate_generation = NULL, rebuild_dataset_kind = NULL,
                 rebuild_cursor_fingerprint = NULL, rebuild_processed_events = 0,
                 rebuild_total_events = current_event_count + legacy_event_count
             WHERE singleton_id = 1;",
        )
        .expect("rebuild required");
    let required = reader
        .capture_product_data_status(query())
        .expect("required status");
    assert_eq!(
        required.usage().aggregate().state(),
        ProductAggregateState::RebuildRequired
    );
    assert_eq!(required.usage().aggregate().progress(), None);

    connection
        .execute_batch(
            "UPDATE usage_aggregate_state
             SET state = 'rebuilding', failure_code = NULL,
                 rebuild_aggregate_generation = active_aggregate_generation + 1,
                 rebuild_dataset_kind = 'cleanup', rebuild_cursor_fingerprint = NULL,
                 rebuild_processed_events = 0, rebuild_total_events = 0
             WHERE singleton_id = 1;",
        )
        .expect("rebuilding");
    let rebuilding = reader
        .capture_product_data_status(query())
        .expect("rebuilding status");
    let aggregate = rebuilding.usage().aggregate();
    assert_eq!(aggregate.state(), ProductAggregateState::Rebuilding);
    let progress = aggregate.progress().expect("explicit progress");
    assert_eq!(progress.processed_events(), 0);
    assert_eq!(progress.total_events(), 0);

    connection
        .execute_batch(
            "UPDATE usage_aggregate_state
             SET state = 'failed', failure_code = 'aggregate_failed'
             WHERE singleton_id = 1;",
        )
        .expect("failed");
    let failed = reader
        .capture_product_data_status(query())
        .expect("failed status");
    assert_eq!(
        failed.usage().aggregate().state(),
        ProductAggregateState::Failed
    );
    assert_eq!(failed.usage().aggregate().progress(), None);
    assert!(!format!("{failed:?}").contains("aggregate_failed"));
}

#[test]
fn independent_product_revisions_and_counts_are_preserved() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("independent-status.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let connection = Connection::open(&path).expect("writer connection");
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             UPDATE quota_state
             SET revision = 3, retained_sample_count = 7, retained_epoch_count = 2,
                 retained_transition_count = 4, last_published_at_ms = 1800000000001
             WHERE singleton_id = 1;
             UPDATE benefit_state
             SET revision = 5, current_lot_count = 2, retained_change_count = 9,
                 pending_due_count = 1, retained_delivery_count = 3,
                 last_published_at_ms = 1800000000002
             WHERE singleton_id = 1;
             UPDATE git_installation_state
             SET publication_revision = 8, repository_count = 4, association_count = 6,
                 last_published_at_ms = 1800000000003
             WHERE singleton_id = 1;
             COMMIT;",
        )
        .expect("publish independent status");

    let capture = reader
        .capture_product_data_status(
            ProductDataStatusQuery::new(Duration::from_secs(2)).expect("query"),
        )
        .expect("capture");
    assert_eq!(capture.quota().revision(), 3);
    assert_eq!(capture.quota().retained_sample_count(), 7);
    assert_eq!(capture.quota().retained_epoch_count(), 2);
    assert_eq!(capture.quota().retained_transition_count(), 4);
    assert_eq!(
        capture.quota().last_published_at_ms(),
        Some(1_800_000_000_001)
    );
    assert_eq!(capture.benefit().revision(), 5);
    assert_eq!(capture.benefit().current_lot_count(), 2);
    assert_eq!(capture.benefit().retained_change_count(), 9);
    assert_eq!(capture.benefit().pending_due_count(), 1);
    assert_eq!(capture.benefit().retained_delivery_count(), 3);
    assert_eq!(capture.git().publication_revision(), 8);
    assert_eq!(capture.git().repository_count(), 4);
    assert_eq!(capture.git().association_count(), 6);
}

#[test]
fn post_open_corruption_fails_closed_and_deadline_state_is_cleared() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("status-corrupt.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut reader = UsageReadStore::open(&path).expect("open reader");

    let timeout = reader
        .capture_product_data_status(
            ProductDataStatusQuery::new(Duration::from_nanos(1)).expect("tiny query"),
        )
        .expect_err("tiny deadline");
    assert_eq!(timeout.code(), StoreErrorCode::DeadlineExceeded);
    reader
        .capture_product_data_status(
            ProductDataStatusQuery::new(Duration::from_secs(2)).expect("normal query"),
        )
        .expect("handler cleared");

    let connection = Connection::open(&path).expect("corrupting connection");
    connection
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
             UPDATE quota_state SET last_published_at_ms = 1800000000000
             WHERE singleton_id = 1;",
        )
        .expect("inject invalid zero-revision publication time");
    let error = reader
        .capture_product_data_status(
            ProductDataStatusQuery::new(Duration::from_secs(2)).expect("query"),
        )
        .expect_err("corruption must fail closed");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
}
