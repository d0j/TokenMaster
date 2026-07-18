mod support;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopDashboardSectionState, DesktopFreshness, DesktopQuality, DesktopRouteKey, DesktopState,
    MAX_SESSION_ROWS,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{PageSize, QueryService, UsageSessionPageRequest};

use support::dashboard_fixture::{DAY_START_SECONDS, FixedClock, add_distinct_usage_rows, seed};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

#[test]
fn initial_sessions_are_bounded_waiting_truth_without_fabricated_completion() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Sessions);
    let sessions = state.projection().sessions();

    assert_eq!(MAX_SESSION_ROWS, 64);
    assert_eq!(sessions.state(), DesktopDashboardSectionState::Waiting);
    assert_eq!(sessions.rows().len(), 0);
    assert_eq!(sessions.has_more(), None);
    assert_eq!(sessions.freshness(), None);
    assert_eq!(sessions.quality(), None);
}

#[test]
fn ready_sessions_map_one_newest_first_page_and_preserve_has_more() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("sessions-projection.sqlite3");
    seed(&path);
    add_distinct_usage_rows(&path, 64);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let sessions = service
        .usage_sessions(
            UsageSessionPageRequest::first(
                PageSize::new(MAX_SESSION_ROWS).expect("page size"),
                Vec::new(),
            )
            .expect("session request"),
        )
        .expect("sessions");
    assert_eq!(sessions.payload().sessions().len(), MAX_SESSION_ROWS);
    assert!(sessions.payload().has_more());

    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_sessions(attempt(1), sessions)
        .expect("publish sessions");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Sessions);
    let sessions = state.projection().sessions();

    assert_eq!(sessions.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(sessions.freshness(), Some(DesktopFreshness::Fresh));
    assert_eq!(sessions.quality(), Some(DesktopQuality::Authoritative));
    assert_eq!(sessions.rows().len(), MAX_SESSION_ROWS);
    assert_eq!(sessions.has_more(), Some(true));
    let newest = &sessions.rows()[0];
    assert_eq!(newest.last_timestamp_seconds(), DAY_START_SECONDS + 4_063);
    assert_eq!(newest.event_count(), 1);
    assert_eq!(newest.total_tokens().known_sum(), Some(2));
    assert_eq!(newest.cost().micros(), Some(1));
    assert!(
        sessions
            .rows()
            .windows(2)
            .all(|pair| { pair[0].last_timestamp_seconds() >= pair[1].last_timestamp_seconds() })
    );
}
