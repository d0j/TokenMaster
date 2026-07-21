use std::{thread, time::Duration};
use tokenmaster_desktop::{
    DesktopController, DesktopHistoryRangeGeneration, DesktopHistoryRangeIntent,
    DesktopHistoryRangePreset, DesktopQueryPlan, DesktopQuerySource, DesktopRefreshUrgency,
    DesktopSessionDetailIntent, DesktopSessionNavigationGeneration, DesktopSessionPageDirection,
    DesktopSessionPageIntent, DesktopSnapshotEpoch,
};
use tokenmaster_product::{
    ProductGeneration, ProductSessionDetailSelection, ProductSessionDetailSelectionGeneration,
};
use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewRequest, BenefitOverviewSnapshot, GitEnvelope,
    GitOutputRequest, GitOutputSnapshot, LatestActivityPage, LatestActivityRequest,
    ProductDataStatusEnvelope, QueryEnvelope, QueryError, QuotaCurrentSnapshot, QuotaEnvelope,
    UsageAnalytics, UsageAnalyticsRequest, UsageSessionDetailResult, UsageSessionKey,
    UsageSessionPage, UsageSessionPageRequest,
};

#[test]
fn history_range_presets_are_closed_and_default_to_thirty_days() {
    assert_eq!(DesktopHistoryRangePreset::Recent1Day.day_count(), 1);
    assert_eq!(DesktopHistoryRangePreset::Recent7Days.day_count(), 7);
    assert_eq!(DesktopHistoryRangePreset::Recent30Days.day_count(), 30);
    assert_eq!(
        DesktopHistoryRangePreset::Recent1Day.stable_code(),
        "recent_1_day"
    );
    assert_eq!(
        DesktopHistoryRangePreset::Recent7Days.stable_code(),
        "recent_7_days"
    );
    assert_eq!(
        DesktopHistoryRangePreset::Recent30Days.stable_code(),
        "recent_30_days"
    );
    assert_eq!(
        DesktopQueryPlan::default_history_range_preset(),
        DesktopHistoryRangePreset::Recent30Days
    );
}

#[test]
fn history_range_generation_is_checked_and_intent_is_path_free() {
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let generation = DesktopHistoryRangeGeneration::new(7).expect("nonzero generation");
    let intent = DesktopHistoryRangeIntent::new(
        epoch,
        ProductGeneration::INITIAL,
        generation,
        DesktopHistoryRangePreset::Recent7Days,
    );

    assert!(DesktopHistoryRangeGeneration::new(0).is_none());
    assert_eq!(intent.snapshot_epoch(), epoch);
    assert_eq!(intent.product_generation(), ProductGeneration::INITIAL);
    assert_eq!(intent.generation(), generation);
    assert_eq!(intent.preset(), DesktopHistoryRangePreset::Recent7Days);
    let debug = format!("{intent:?}");
    assert!(!debug.contains("path"));
    assert!(!debug.contains("scope"));
    assert!(!debug.contains("date"));
}

struct UnavailableSource;

impl DesktopQuerySource for UnavailableSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        Err(query_failure())
    }
    fn usage_analytics(
        &mut self,
        _: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        Err(query_failure())
    }
    fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        Err(query_failure())
    }
    fn benefit_overview(
        &mut self,
        _: BenefitOverviewRequest,
    ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
        Err(query_failure())
    }
    fn git_output(
        &mut self,
        _: GitOutputRequest,
    ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
        Err(query_failure())
    }
    fn latest_activity(
        &mut self,
        _: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
        Err(query_failure())
    }
    fn usage_sessions(
        &mut self,
        _: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
        Err(query_failure())
    }
    fn usage_session_detail(
        &mut self,
        _: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        Err(query_failure())
    }
}

fn query_failure() -> QueryError {
    tokenmaster_query::PageSize::new(0).expect_err("zero page size is invalid")
}

fn ready_controller() -> (DesktopController, DesktopSnapshotEpoch, ProductGeneration) {
    let mut controller = DesktopController::spawn(
        UnavailableSource,
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while controller.try_completion().expect("completion").is_none() {
        assert!(std::time::Instant::now() < deadline, "refresh timed out");
        thread::yield_now();
    }
    let generation = controller
        .published_product_generation()
        .expect("published generation")
        .expect("initial publication");
    (controller, epoch, generation)
}

#[test]
fn history_range_admission_rejects_stale_same_and_non_newer_intents() {
    let (mut controller, epoch, generation) = ready_controller();
    let same = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent30Days,
    );
    assert_eq!(
        controller
            .request_history_range(same)
            .expect_err("same preset")
            .stable_code(),
        "stale_history_range"
    );
    let stale_epoch = DesktopHistoryRangeIntent::new(
        DesktopSnapshotEpoch::new(2).expect("epoch"),
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(stale_epoch)
            .expect_err("stale epoch")
            .stable_code(),
        "stale_history_range"
    );
    let accepted = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(accepted)
        .expect("first range admission");
    let non_newer = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent7Days,
    );
    assert_eq!(
        controller
            .request_history_range(non_newer)
            .expect_err("non-newer generation")
            .stable_code(),
        "stale_history_range"
    );
    controller.shutdown().expect("shutdown");
}

#[test]
fn range_and_sessions_admissions_are_mutually_exclusive_without_displacement() {
    let (mut controller, epoch, generation) = ready_controller();
    let range = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(range)
        .expect("range admission");
    let page = DesktopSessionPageIntent::new(
        epoch,
        generation,
        DesktopSessionNavigationGeneration::new(1).expect("generation"),
        DesktopSessionPageDirection::Newest,
    );
    assert_eq!(
        controller
            .request_session_page(page)
            .expect_err("range blocks page")
            .stable_code(),
        "busy"
    );
    controller.shutdown().expect("shutdown");

    let (mut controller, epoch, generation) = ready_controller();
    let selection = ProductSessionDetailSelection::new(
        ProductSessionDetailSelectionGeneration::new(1).expect("generation"),
        0,
    );
    controller
        .request_session_detail(DesktopSessionDetailIntent::new(
            epoch, generation, selection,
        ))
        .expect("detail admission");
    let range = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(range)
            .expect_err("detail blocks range")
            .stable_code(),
        "busy"
    );
    controller.shutdown().expect("shutdown");
}
