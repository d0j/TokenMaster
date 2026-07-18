use tempfile::TempDir;
use tokenmaster_domain::{
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitObservationId, BenefitScope, QuotaAccountId, UsageProviderId,
};
use tokenmaster_product::{
    ProductAttemptGeneration, ProductGeneration, ProductPublishOutcome, ProductReducer,
    ProductSectionKind,
};
use tokenmaster_query::{
    BenefitOverviewRequest, QueryClock, QueryError, QueryErrorCode, QueryService, QueryTimeSample,
    UsageAnalyticsRequest, UsageRange, UsageSeriesSelection, UsageTimeZone, WeekStart,
};
use tokenmaster_store::UsageStore;

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("non-zero attempt")
}

fn analytics_request(range: UsageRange) -> UsageAnalyticsRequest {
    UsageAnalyticsRequest::new(
        range,
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        Vec::new(),
    )
    .expect("analytics request")
}

#[test]
fn history_is_independent_and_retains_only_compatible_recent_truth() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("product-history.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let today = service
        .usage_analytics(analytics_request(UsageRange::today()))
        .expect("today analytics");
    let recent = service
        .usage_analytics(analytics_request(
            UsageRange::recent_days(30).expect("recent range"),
        ))
        .expect("recent analytics");

    let mut reducer = ProductReducer::new();
    assert_eq!(
        reducer.snapshot().history().kind(),
        ProductSectionKind::Waiting
    );
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_analytics(attempt(1), today)
        .expect("publish dashboard analytics");
    assert_eq!(
        reducer.snapshot().history().kind(),
        ProductSectionKind::Waiting
    );

    assert_eq!(
        reducer
            .publish_history(attempt(1), recent.clone())
            .expect("publish history"),
        ProductPublishOutcome::Accepted
    );
    let ready = reducer.snapshot();
    assert_eq!(ready.analytics().kind(), ProductSectionKind::Ready);
    assert_eq!(ready.history().kind(), ProductSectionKind::Ready);

    reducer
        .fail_history(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("fail newer history refresh");
    let retained = reducer.snapshot();
    assert_eq!(retained.analytics().kind(), ProductSectionKind::Ready);
    assert_eq!(retained.history().kind(), ProductSectionKind::Unavailable);
    assert!(retained.history().retains_payload());
    assert_eq!(
        reducer
            .publish_history(attempt(1), recent.clone())
            .expect("reject older history"),
        ProductPublishOutcome::RejectedOlder
    );
    assert_eq!(
        reducer
            .publish_history(attempt(3), recent)
            .expect("recover history"),
        ProductPublishOutcome::Accepted
    );
    assert_eq!(
        reducer.snapshot().history().kind(),
        ProductSectionKind::Ready
    );
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
            .publish_data_status(attempt(2), second_status.clone())
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
            .publish_data_status(attempt(1), first_status)
            .expect("older result"),
        ProductPublishOutcome::RejectedOlder
    );
    assert_eq!(
        reducer
            .publish_data_status(attempt(2), second_status)
            .expect("equal result"),
        ProductPublishOutcome::Coalesced
    );
    assert_eq!(reducer.snapshot().generation().get(), 1);

    assert_eq!(
        reducer
            .fail_analytics(attempt(1), QueryErrorCode::DeadlineExceeded,)
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
            .publish_analytics(attempt(2), analytics)
            .expect("newer analytics"),
        ProductPublishOutcome::Accepted
    );
    let ready = reducer.snapshot();
    assert_eq!(ready.generation().get(), 3);
    assert_eq!(ready.analytics().kind(), ProductSectionKind::Ready);
    assert_eq!(failed.analytics().kind(), ProductSectionKind::Unavailable);
}

#[test]
fn newer_attempt_recovers_a_retained_status_after_refresh_failure() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("product-recovery.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let first = service.product_data_status().expect("first status");
    let second = service.product_data_status().expect("second status");
    let mut reducer = ProductReducer::new();

    reducer
        .publish_data_status(attempt(1), first)
        .expect("publish status");
    reducer
        .fail_data_status(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("fail refresh");
    let retained = reducer.snapshot();
    assert_eq!(
        retained.data_status().kind(),
        ProductSectionKind::Unavailable
    );
    assert!(retained.data_status().retains_payload());

    assert_eq!(
        reducer
            .publish_data_status(attempt(2), second.clone())
            .expect("same attempt"),
        ProductPublishOutcome::Coalesced
    );
    assert_eq!(
        reducer
            .publish_data_status(attempt(3), second)
            .expect("newer successful refresh"),
        ProductPublishOutcome::Accepted
    );
    let recovered = reducer.snapshot();
    assert_eq!(recovered.data_status().kind(), ProductSectionKind::Ready);
    assert!(!recovered.data_status().retains_payload());
    assert!(recovered.data_status().failure().is_none());
}

#[test]
fn benefit_overview_is_revision_compatible_and_retained_only_as_degraded_truth() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("product-benefit-overview.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let empty_status = service.product_data_status().expect("empty status");
    UsageStore::open(&path)
        .expect("writer")
        .apply_benefit_observation(
            &BenefitInventoryObservation::new(BenefitInventoryObservationParts {
                scope: BenefitScope::new(
                    UsageProviderId::new("codex").expect("provider"),
                    QuotaAccountId::new("reducer-private-account").expect("account"),
                    None,
                ),
                observation_id: BenefitObservationId::from_bytes([8; 32]),
                observed_at_ms: 1_800_000_000_000,
                fresh_until_ms: 1_800_000_001_000,
                stale_after_ms: 1_800_000_002_000,
                completeness: BenefitInventoryCompleteness::Complete,
                lots: Vec::new(),
            })
            .expect("benefit observation"),
        )
        .expect("publish benefit observation");
    let overview = service
        .benefit_overview(BenefitOverviewRequest::new())
        .expect("benefit overview");
    let current_status = service.product_data_status().expect("current status");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), empty_status)
        .expect("publish empty status");
    assert_eq!(
        reducer
            .publish_benefit(attempt(1), overview.clone())
            .expect("reject incompatible overview"),
        ProductPublishOutcome::RejectedIncompatible
    );
    reducer
        .publish_data_status(attempt(2), current_status)
        .expect("publish current status");
    assert_eq!(
        reducer
            .publish_benefit(attempt(2), overview)
            .expect("publish overview"),
        ProductPublishOutcome::Accepted
    );
    assert_eq!(
        reducer.snapshot().benefit().kind(),
        ProductSectionKind::Ready
    );

    reducer
        .fail_benefit(attempt(3), QueryErrorCode::DeadlineExceeded)
        .expect("fail newer benefit attempt");
    let degraded = reducer.snapshot();
    assert_eq!(degraded.benefit().kind(), ProductSectionKind::Unavailable);
    assert!(degraded.benefit().retains_payload());
    assert!(degraded.benefit().payload().is_some());
}
