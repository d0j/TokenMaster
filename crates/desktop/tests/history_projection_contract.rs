mod support;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopDashboardSectionState, DesktopHistoryRangePreset, DesktopHistoryRangeSelectionError,
    DesktopRouteKey, DesktopSnapshotEpoch, DesktopState, DesktopValueAvailability,
    MAX_HISTORY_DAYS,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{
    QueryService, UsageAnalyticsRequest, UsageRange, UsageSeriesSelection, UsageTimeZone, WeekStart,
};

use support::dashboard_fixture::{FixedClock, seed};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

fn snapshot_for_days(days: u16) -> std::sync::Arc<tokenmaster_product::ProductSnapshot> {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-projection.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let history = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::recent_days(days).expect("recent range"),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                Vec::new(),
            )
            .expect("history request"),
        )
        .expect("history analytics");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_history(attempt(1), history)
        .expect("publish history");
    reducer.snapshot()
}

#[test]
fn initial_history_is_bounded_waiting_truth_without_fabricated_zeroes() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::History);
    let history = state.projection().history();

    assert_eq!(MAX_HISTORY_DAYS, 30);
    assert_eq!(history.state(), DesktopDashboardSectionState::Waiting);
    assert_eq!(history.rows().len(), 0);
    assert_eq!(
        history.total_tokens().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(
        history.cost().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(history.event_count(), None);
    assert_eq!(history.range(), None);
    assert_eq!(
        history.range_preset(),
        DesktopHistoryRangePreset::Recent30Days
    );
    assert!(!history.range_pending());
}

#[test]
fn history_range_request_is_single_pending_and_rejects_the_published_preset() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let mut state = DesktopState::new(&snapshot, DesktopRouteKey::History);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    assert_eq!(
        state.apply_snapshot_for_epoch(epoch, &snapshot),
        tokenmaster_desktop::DesktopApplyOutcome::Accepted
    );

    assert_eq!(
        state.request_history_range(DesktopHistoryRangePreset::Recent30Days),
        Err(DesktopHistoryRangeSelectionError::Unavailable)
    );
    let intent = state
        .request_history_range(DesktopHistoryRangePreset::Recent1Day)
        .expect("accept a different preset");
    assert_eq!(intent.snapshot_epoch(), epoch);
    assert_eq!(intent.preset(), DesktopHistoryRangePreset::Recent1Day);
    assert!(state.projection().history().range_pending());
    assert_eq!(
        state.request_history_range(DesktopHistoryRangePreset::Recent7Days),
        Err(DesktopHistoryRangeSelectionError::Pending)
    );
}

#[test]
fn history_range_preset_is_derived_only_from_exact_daily_series_and_terminal_is_exact() {
    for (days, preset) in [
        (1, DesktopHistoryRangePreset::Recent1Day),
        (7, DesktopHistoryRangePreset::Recent7Days),
        (30, DesktopHistoryRangePreset::Recent30Days),
    ] {
        let snapshot = snapshot_for_days(days);
        let mut state = DesktopState::new(&snapshot, DesktopRouteKey::History);
        state.apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), &snapshot);
        assert_eq!(state.projection().history().range_preset(), preset);
    }

    let initial = ProductReducer::new().snapshot();
    let mut state = DesktopState::new(&initial, DesktopRouteKey::History);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    state.apply_snapshot_for_epoch(epoch, &initial);
    let intent = state
        .request_history_range(DesktopHistoryRangePreset::Recent1Day)
        .expect("pending range");
    let other = tokenmaster_desktop::DesktopHistoryRangeIntent::new(
        epoch,
        intent.product_generation(),
        tokenmaster_desktop::DesktopHistoryRangeGeneration::new(2).expect("generation"),
        DesktopHistoryRangePreset::Recent7Days,
    );
    state.complete_history_range_terminal(other);
    assert!(state.projection().history().range_pending());
    state.complete_history_range_terminal(intent);
    assert!(!state.projection().history().range_pending());
    state.apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(2).expect("new epoch"), &initial);
    assert_eq!(
        state.projection().history().range_preset(),
        DesktopHistoryRangePreset::Recent30Days
    );
}

#[test]
fn ready_history_maps_exact_recent_days_newest_first() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-projection.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let history = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::recent_days(30).expect("recent range"),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                Vec::new(),
            )
            .expect("history request"),
        )
        .expect("history analytics");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_history(attempt(1), history)
        .expect("publish history");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::History);
    let history = state.projection().history();

    assert_eq!(history.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(history.range(), Some(((2026, 6, 17), (2026, 7, 17))));
    assert_eq!(history.time_zone_id(), Some("UTC"));
    assert_eq!(history.rows().len(), MAX_HISTORY_DAYS);
    assert_eq!(
        history.range_preset(),
        DesktopHistoryRangePreset::Recent30Days
    );
    let newest = &history.rows()[0];
    assert_eq!(newest.date(), (2026, 7, 16));
    assert_eq!(newest.event_count(), 1);
    assert_eq!(newest.total_tokens().known_sum(), Some(140));
    assert_eq!(history.event_count(), Some(1));
    assert_eq!(history.total_tokens().known_sum(), Some(140));
    assert_eq!(history.token_maximum(), Some(140));
    assert_eq!(history.cost_maximum_micros(), Some(10_000));
    assert!(
        history
            .rows()
            .windows(2)
            .all(|pair| pair[0].date() > pair[1].date())
    );
}
