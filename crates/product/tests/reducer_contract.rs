use tempfile::TempDir;
use tokenmaster_product::{
    ProductGeneration, ProductPublishOutcome, ProductReducer, ProductSectionKind,
};
use tokenmaster_query::{
    QueryClock, QueryError, QueryErrorCode, QueryService, QueryTimeSample, UsageAnalyticsRequest,
    UsageRange, UsageSeriesSelection, UsageTimeZone, WeekStart,
};
use tokenmaster_store::UsageStore;

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

#[test]
fn reducer_accepts_only_newer_sections_and_keeps_faults_independent() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("product-reducer.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let first_status = service.product_data_status().expect("first status");
    let second_status = service.product_data_status().expect("second status");
    let analytics = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::today(),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::None,
                Vec::new(),
                Vec::new(),
            )
            .expect("request"),
        )
        .expect("analytics");

    let mut reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    assert_eq!(initial.generation(), ProductGeneration::INITIAL);
    assert_eq!(initial.data_status().kind(), ProductSectionKind::Waiting);
    assert_eq!(initial.analytics().kind(), ProductSectionKind::Waiting);

    assert_eq!(
        reducer
            .publish_data_status(second_status.clone())
            .expect("publish"),
        ProductPublishOutcome::Accepted
    );
    let after_status = reducer.snapshot();
    assert_eq!(after_status.generation().get(), 1);
    assert_eq!(after_status.data_status().kind(), ProductSectionKind::Ready);
    assert_eq!(after_status.analytics().kind(), ProductSectionKind::Waiting);
    assert_eq!(initial.data_status().kind(), ProductSectionKind::Waiting);

    assert_eq!(
        reducer
            .publish_data_status(first_status)
            .expect("older result"),
        ProductPublishOutcome::RejectedOlder
    );
    assert_eq!(
        reducer
            .publish_data_status(second_status)
            .expect("equal result"),
        ProductPublishOutcome::Coalesced
    );
    assert_eq!(reducer.snapshot().generation().get(), 1);

    assert_eq!(
        reducer
            .fail_analytics(
                tokenmaster_query::SnapshotGeneration::new(2).expect("attempt"),
                QueryErrorCode::DeadlineExceeded,
            )
            .expect("independent failure"),
        ProductPublishOutcome::Accepted
    );
    let failed = reducer.snapshot();
    assert_eq!(failed.generation().get(), 2);
    assert_eq!(failed.data_status().kind(), ProductSectionKind::Ready);
    assert_eq!(failed.analytics().kind(), ProductSectionKind::Unavailable);
    assert_eq!(
        failed.analytics().failure().expect("failure").code(),
        QueryErrorCode::DeadlineExceeded
    );

    assert_eq!(
        reducer
            .publish_analytics(analytics)
            .expect("newer analytics"),
        ProductPublishOutcome::Accepted
    );
    let ready = reducer.snapshot();
    assert_eq!(ready.generation().get(), 3);
    assert_eq!(ready.analytics().kind(), ProductSectionKind::Ready);
    assert_eq!(failed.analytics().kind(), ProductSectionKind::Unavailable);
}
