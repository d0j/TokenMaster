use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_query::{
    DatasetIdentity, PRODUCT_DATA_STATUS_SCHEMA_VERSION, ProductAggregateState,
    ProductComponentState, ProductDataWarningCode, QueryClock, QueryError, QueryErrorCode,
    QueryFreshness, QueryQuality, QueryService, QueryTimeSample,
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

#[test]
fn fresh_product_status_is_owned_ordered_and_explicitly_empty() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("product-status.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(
        &path,
        FixedClock {
            wall_time_ms: 1_800_000_000_000,
            monotonic_ms: 7,
        },
    )
    .expect("open service");

    let snapshot = service.product_data_status().expect("product status");
    assert_eq!(PRODUCT_DATA_STATUS_SCHEMA_VERSION, 1);
    assert_eq!(snapshot.schema_version(), 1);
    assert_eq!(snapshot.snapshot_generation().get(), 1);
    assert_eq!(snapshot.generated_at_ms(), 1_800_000_000_000);
    assert!(snapshot.is_newer_than(None));

    let usage = snapshot.payload().usage();
    assert_eq!(usage.publication_generation().get(), 0);
    assert_eq!(usage.dataset_identity(), DatasetIdentity::Empty);
    assert_eq!(usage.freshness(), QueryFreshness::Unavailable);
    assert_eq!(usage.quality(), QueryQuality::Authoritative);
    assert_eq!(usage.data_through_ms(), None);
    assert_eq!(usage.scope_count(), 0);
    assert_eq!(usage.aggregate().state(), ProductAggregateState::Ready);
    assert!(usage.warnings().is_empty());

    assert_eq!(
        snapshot.payload().quota().state(),
        ProductComponentState::Empty
    );
    assert_eq!(snapshot.payload().quota().revision().get(), 0);
    assert_eq!(
        snapshot.payload().benefit().state(),
        ProductComponentState::Empty
    );
    assert_eq!(snapshot.payload().benefit().revision().get(), 0);
    assert_eq!(
        snapshot.payload().git().state(),
        ProductComponentState::Empty
    );
    assert_eq!(snapshot.payload().git().revision().get(), 0);

    let debug = format!("{snapshot:?}");
    assert!(!debug.contains(path.to_string_lossy().as_ref()));
    assert!(!debug.contains("SELECT "));
    assert!(!debug.contains("installation_salt"));
}

#[test]
fn aggregate_rebuilding_is_visible_without_hiding_archive_status() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("aggregate-rebuilding.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let connection = Connection::open(&path).expect("fixture connection");
    connection
        .execute_batch(
            "UPDATE usage_aggregate_state
             SET state = 'rebuilding', failure_code = NULL,
                 rebuild_aggregate_generation = active_aggregate_generation + 1,
                 rebuild_dataset_kind = 'cleanup', rebuild_cursor_fingerprint = NULL,
                 rebuild_processed_events = 0, rebuild_total_events = 0
             WHERE singleton_id = 1;",
        )
        .expect("rebuilding fixture");
    drop(connection);
    let mut service = QueryService::open(
        &path,
        FixedClock {
            wall_time_ms: 1_800_000_000_000,
            monotonic_ms: 8,
        },
    )
    .expect("open service");

    let snapshot = service.product_data_status().expect("status");
    let usage = snapshot.payload().usage();
    assert_eq!(usage.dataset_identity(), DatasetIdentity::Empty);
    assert_eq!(usage.quality(), QueryQuality::Authoritative);
    assert_eq!(usage.aggregate().state(), ProductAggregateState::Rebuilding);
    let progress = usage.aggregate().progress().expect("progress");
    assert_eq!(progress.processed_events(), 0);
    assert_eq!(progress.total_events(), 0);
    assert!(
        usage
            .warnings()
            .contains(&ProductDataWarningCode::AggregateRebuilding)
    );
}

#[test]
fn failed_status_mapping_consumes_no_public_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("failed-status.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(
        &path,
        FixedClock {
            wall_time_ms: 1_800_000_000_000,
            monotonic_ms: 9,
        },
    )
    .expect("open service");
    let connection = Connection::open(&path).expect("fixture connection");
    connection
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
             UPDATE quota_state SET last_published_at_ms = 1800000000000
             WHERE singleton_id = 1;",
        )
        .expect("corrupt after open");
    let error = service.product_data_status().expect_err("corrupt status");
    assert_eq!(error.code(), QueryErrorCode::CorruptArchive);

    connection
        .execute(
            "UPDATE quota_state SET last_published_at_ms = NULL WHERE singleton_id = 1",
            [],
        )
        .expect("repair fixture");
    let first = service.product_data_status().expect("first publication");
    assert_eq!(first.snapshot_generation().get(), 1);
    let second = service.product_data_status().expect("second publication");
    assert_eq!(second.snapshot_generation().get(), 2);
    assert!(second.is_newer_than(Some(&first)));
    assert!(!first.is_newer_than(Some(&second)));
}
