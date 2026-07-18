use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitObservationId, BenefitScope, QuotaAccountId, UsageProviderId,
};
use tokenmaster_product::{
    ProductAttemptGeneration, ProductPublishOutcome, ProductReducer, ProductRoute,
    ProductRouteReason, ProductRouteState, ProductSectionKind,
};
use tokenmaster_query::{
    BenefitOverviewRequest, DatasetIdentity, LatestActivityPage, PublicationGeneration, QueryClock,
    QueryEnvelope, QueryError, QueryFreshness, QueryHeader, QueryHeaderParts, QueryQuality,
    QueryService, QueryTimeSample, SnapshotGeneration, UsageAnalyticsRequest, UsageRange,
    UsageSeriesSelection, UsageTimeZone, WeekStart,
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

#[test]
fn history_route_depends_on_its_own_recent_analytics_section() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-route.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let request = |range| {
        UsageAnalyticsRequest::new(
            range,
            UsageTimeZone::iana("UTC").expect("UTC"),
            WeekStart::Monday,
            UsageSeriesSelection::Daily,
            Vec::new(),
            Vec::new(),
        )
        .expect("analytics request")
    };
    let status = service.product_data_status().expect("status");
    let today = service
        .usage_analytics(request(UsageRange::today()))
        .expect("today analytics");
    let recent = service
        .usage_analytics(request(UsageRange::recent_days(30).expect("recent range")))
        .expect("recent analytics");
    let mut reducer = ProductReducer::new();

    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_analytics(attempt(1), today)
        .expect("publish dashboard analytics");
    assert_eq!(
        reducer.snapshot().route(ProductRoute::Dashboard).state(),
        ProductRouteState::Degraded
    );
    assert_eq!(
        reducer.snapshot().route(ProductRoute::History).state(),
        ProductRouteState::Degraded
    );
    assert!(
        reducer
            .snapshot()
            .route(ProductRoute::History)
            .reasons()
            .contains(ProductRouteReason::UsageUnavailable)
    );

    reducer
        .publish_history(attempt(1), recent)
        .expect("publish history");
    assert_eq!(
        reducer.snapshot().route(ProductRoute::History).state(),
        ProductRouteState::Ready
    );
    assert!(
        !reducer
            .snapshot()
            .route(ProductRoute::History)
            .reasons()
            .contains(ProductRouteReason::UsageUnavailable)
    );
}

fn activity(generation: u64, identity: DatasetIdentity) -> QueryEnvelope<LatestActivityPage> {
    QueryEnvelope::new(
        QueryHeader::new(QueryHeaderParts {
            snapshot_generation: SnapshotGeneration::new(generation).expect("generation"),
            publication_generation: PublicationGeneration::new(0).expect("publication"),
            dataset_identity: identity,
            generated_at_ms: 1_800_000_000_000,
            data_through_ms: None,
            freshness: QueryFreshness::Unavailable,
            quality: QueryQuality::Authoritative,
            scopes: Vec::new(),
            warnings: Vec::new(),
        })
        .expect("header"),
        LatestActivityPage::new(Vec::new(), None, false).expect("page"),
    )
}

#[test]
fn routes_are_fixed_bounded_and_degrade_without_hiding_healthy_sections() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("routes.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("service");
    let mut reducer = ProductReducer::new();

    assert_eq!(ProductRoute::ALL.len(), 11);
    assert_eq!(
        reducer.snapshot().route(ProductRoute::DataHealth).state(),
        ProductRouteState::Unavailable
    );
    assert_eq!(
        reducer.snapshot().route(ProductRoute::Settings).state(),
        ProductRouteState::Ready
    );
    assert_eq!(
        reducer.snapshot().route(ProductRoute::HelpAbout).state(),
        ProductRouteState::Ready
    );

    reducer
        .publish_data_status(attempt(1), service.product_data_status().expect("status"))
        .expect("publish status");
    let snapshot = reducer.snapshot();
    assert_eq!(
        snapshot.route(ProductRoute::DataHealth).state(),
        ProductRouteState::Ready
    );
    assert_eq!(
        snapshot.route(ProductRoute::Dashboard).state(),
        ProductRouteState::Degraded
    );
    assert!(
        snapshot
            .route(ProductRoute::Dashboard)
            .reasons()
            .contains(ProductRouteReason::UsageUnavailable)
    );
    assert!(
        snapshot
            .route(ProductRoute::Dashboard)
            .reasons()
            .contains(ProductRouteReason::QuotaUnavailable)
    );
    assert!(
        ProductRoute::ALL
            .into_iter()
            .all(|route| snapshot.route(route).reasons().count() <= 8)
    );
}

#[test]
fn aggregate_rebuild_is_visible_only_on_routes_that_depend_on_aggregates() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("route-rebuild.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    Connection::open(&path)
        .expect("open fixture")
        .execute_batch(
            "UPDATE usage_aggregate_state
             SET state = 'rebuilding', failure_code = NULL,
                 rebuild_aggregate_generation = active_aggregate_generation + 1,
                 rebuild_dataset_kind = 'cleanup', rebuild_cursor_fingerprint = NULL,
                 rebuild_processed_events = 0, rebuild_total_events = 0
             WHERE singleton_id = 1;",
        )
        .expect("mark aggregate rebuilding");
    let mut service = QueryService::open(&path, FixedClock).expect("service");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), service.product_data_status().expect("status"))
        .expect("publish status");

    let snapshot = reducer.snapshot();
    assert_eq!(
        snapshot.route(ProductRoute::DataHealth).state(),
        ProductRouteState::Ready
    );
    assert!(
        !snapshot
            .route(ProductRoute::DataHealth)
            .reasons()
            .contains(ProductRouteReason::AggregateRebuilding)
    );
    assert!(
        snapshot
            .route(ProductRoute::Dashboard)
            .reasons()
            .contains(ProductRouteReason::AggregateRebuilding)
    );
    assert_eq!(
        snapshot.route(ProductRoute::History).state(),
        ProductRouteState::Unavailable
    );
    assert!(
        snapshot
            .route(ProductRoute::History)
            .reasons()
            .contains(ProductRouteReason::AggregateRebuilding)
    );
    assert!(
        !snapshot
            .route(ProductRoute::Activity)
            .reasons()
            .contains(ProductRouteReason::AggregateRebuilding)
    );
}

#[test]
fn incompatible_async_results_are_rejected_and_new_status_invalidates_old_payloads() {
    let directory = TempDir::new().expect("temporary directory");
    let empty_path = directory.path().join("empty.sqlite3");
    drop(UsageStore::open(&empty_path).expect("create empty archive"));
    let mut empty_service = QueryService::open(&empty_path, FixedClock).expect("empty service");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(
            attempt(1),
            empty_service.product_data_status().expect("empty status"),
        )
        .expect("publish empty status");

    assert_eq!(
        reducer
            .publish_activity(attempt(1), activity(2, DatasetIdentity::LegacySnapshotV1),)
            .expect("mismatched activity"),
        ProductPublishOutcome::RejectedIncompatible
    );
    assert_eq!(
        reducer.snapshot().activity().kind(),
        ProductSectionKind::Waiting
    );
    assert_eq!(
        reducer
            .publish_activity(attempt(2), activity(2, DatasetIdentity::Empty))
            .expect("matching activity"),
        ProductPublishOutcome::Accepted
    );

    let legacy_path = directory.path().join("legacy.sqlite3");
    Connection::open(&legacy_path)
        .expect("create legacy archive")
        .execute_batch(include_str!("../../store/tests/fixtures/usage_v1.sql"))
        .expect("create legacy schema");
    drop(UsageStore::open(&legacy_path).expect("migrate legacy fixture"));
    let mut legacy_service = QueryService::open(&legacy_path, FixedClock).expect("legacy service");
    let _ = legacy_service
        .product_data_status()
        .expect("legacy generation one");
    let legacy_status = legacy_service.product_data_status().expect("legacy status");
    assert_eq!(
        reducer
            .publish_data_status(attempt(2), legacy_status)
            .expect("publish changed identity"),
        ProductPublishOutcome::Accepted
    );
    let changed = reducer.snapshot();
    assert_eq!(changed.activity().kind(), ProductSectionKind::Unavailable);
    assert_eq!(
        changed.activity().failure().expect("stale failure").code(),
        tokenmaster_query::QueryErrorCode::StaleSnapshot
    );
}

#[test]
fn failed_status_retains_last_truth_but_marks_data_health_degraded() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("retained-status.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("service");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), service.product_data_status().expect("status"))
        .expect("publish status");
    reducer
        .fail_data_status(
            attempt(2),
            tokenmaster_query::QueryErrorCode::DeadlineExceeded,
        )
        .expect("fail status");

    let snapshot = reducer.snapshot();
    assert_eq!(
        snapshot.data_status().kind(),
        ProductSectionKind::Unavailable
    );
    assert!(snapshot.data_status().retains_payload());
    assert_eq!(
        snapshot.route(ProductRoute::DataHealth).state(),
        ProductRouteState::Degraded
    );
    assert!(
        snapshot
            .route(ProductRoute::DataHealth)
            .reasons()
            .contains(ProductRouteReason::DataStatusUnavailable)
    );
}

#[test]
fn published_benefit_overview_clears_only_the_benefit_route_reason() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("route-benefit-overview.sqlite3");
    let scope = BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new("route-private-account").expect("account"),
        None,
    );
    UsageStore::open(&path)
        .expect("writer")
        .apply_benefit_observation(
            &BenefitInventoryObservation::new(BenefitInventoryObservationParts {
                scope,
                observation_id: BenefitObservationId::from_bytes([7; 32]),
                observed_at_ms: 1_800_000_000_000,
                fresh_until_ms: 1_800_000_001_000,
                stale_after_ms: 1_800_000_002_000,
                completeness: BenefitInventoryCompleteness::Complete,
                lots: Vec::new(),
            })
            .expect("benefit observation"),
        )
        .expect("publish benefit observation");
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let overview = service
        .benefit_overview(BenefitOverviewRequest::new())
        .expect("benefit overview");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    assert!(
        reducer
            .snapshot()
            .route(ProductRoute::Dashboard)
            .reasons()
            .contains(ProductRouteReason::BenefitUnavailable)
    );

    reducer
        .publish_benefit(attempt(1), overview)
        .expect("publish overview");
    let dashboard = reducer.snapshot().route(ProductRoute::Dashboard);
    assert!(
        !dashboard
            .reasons()
            .contains(ProductRouteReason::BenefitUnavailable)
    );
    assert!(
        dashboard
            .reasons()
            .contains(ProductRouteReason::UsageUnavailable)
    );
    assert!(
        dashboard
            .reasons()
            .contains(ProductRouteReason::QuotaUnavailable)
    );
}
