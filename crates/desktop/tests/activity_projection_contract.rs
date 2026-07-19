mod support;

use std::sync::Arc;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopActivityProjection, DesktopDashboardSectionState, DesktopRouteKey, DesktopState,
    DesktopValueAvailability, MAX_ACTIVITY_ROWS,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{
    LatestActivityRequest, PageSize, QueryEnvelope, QueryErrorCode, QueryQuality, QueryService,
};

use support::dashboard_fixture::{FixedClock, seed};
use support::dashboard_fixture::{add_distinct_usage_rows, clear_usage_rows};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

#[test]
fn initial_activity_is_bounded_waiting_truth_without_fabricated_zeroes() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Activity);
    let activity = state.projection().activity();

    assert_eq!(MAX_ACTIVITY_ROWS, 12);
    assert_eq!(activity.state(), DesktopDashboardSectionState::Waiting);
    assert_eq!(activity.rows().len(), 0);
    assert_eq!(activity.freshness(), None);
    assert_eq!(activity.quality(), None);
    assert_eq!(activity.has_more(), None);
}

#[test]
fn unavailable_activity_has_no_payload_or_fabricated_page_state() {
    let mut reducer = ProductReducer::new();
    reducer
        .fail_activity(attempt(1), QueryErrorCode::Unavailable)
        .expect("publish activity failure");

    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Activity);
    let activity = state.projection().activity();

    assert_eq!(activity.state(), DesktopDashboardSectionState::Unavailable);
    assert_eq!(activity.rows().len(), 0);
    assert_eq!(activity.freshness(), None);
    assert_eq!(activity.quality(), None);
    assert_eq!(activity.has_more(), None);
    assert!(
        activity
            .reason_codes()
            .iter()
            .any(|reason| reason == "unavailable")
    );
}

#[test]
fn ready_activity_maps_newest_first_safe_token_facts() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("activity-projection.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let page = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(MAX_ACTIVITY_ROWS).expect("bounded page size"),
        ))
        .expect("latest activity");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_activity(attempt(1), page)
        .expect("publish activity");

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Activity);
    let activity: &DesktopActivityProjection = state.projection().activity();

    assert_eq!(activity.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(activity.rows().len(), 1);
    assert_eq!(activity.has_more(), Some(false));
    let row = &activity.rows()[0];
    assert_eq!(row.timestamp_seconds(), 1_784_163_600);
    assert_eq!(row.timestamp_nanos(), 0);
    assert_eq!(row.model(), "gpt-5.6");
    assert_eq!(row.input().known_sum(), Some(100));
    assert_eq!(row.cached().known_sum(), Some(20));
    assert_eq!(row.output().known_sum(), Some(30));
    assert_eq!(row.reasoning().known_sum(), Some(10));
    assert_eq!(row.total_tokens().known_sum(), Some(140));
    assert_eq!(
        row.total_tokens().availability(),
        DesktopValueAvailability::Known
    );

    let debug = format!("{activity:?}");
    for forbidden in [
        path.to_string_lossy().as_ref(),
        "dashboard-private-source",
        "dashboard-private-session",
        "dashboard-private-event",
        "fingerprint",
        "cursor",
    ] {
        assert!(!debug.contains(forbidden), "activity exposed {forbidden}");
    }
}

#[test]
fn activity_projection_replacements_release_old_rows() {
    let mut reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    let mut state = DesktopState::new(&initial, DesktopRouteKey::Activity);
    let old_rows = Arc::clone(state.projection().activity().rows());
    let old_rows_weak = Arc::downgrade(&old_rows);
    drop(old_rows);

    for generation in 1..=10_000 {
        reducer
            .fail_activity(
                attempt(generation),
                tokenmaster_query::QueryErrorCode::Unavailable,
            )
            .expect("new product generation");
        let snapshot = reducer.snapshot();
        state.apply_snapshot(&snapshot);
    }

    assert!(old_rows_weak.upgrade().is_none());
    assert_eq!(state.projection().activity().rows().len(), 0);
}

#[test]
fn authoritative_empty_activity_remains_ready_and_page_complete() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("activity-empty.sqlite3");
    seed(&path);
    clear_usage_rows(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let page = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(MAX_ACTIVITY_ROWS).expect("bounded page size"),
        ))
        .expect("empty activity");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_activity(attempt(1), page)
        .expect("publish empty activity");

    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Activity);
    let activity = state.projection().activity();
    assert_eq!(activity.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(activity.rows().len(), 0);
    assert_eq!(activity.has_more(), Some(false));
    assert!(activity.reason_codes().is_empty());
}

#[test]
fn empty_partial_activity_degrades_without_becoming_unavailable() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("activity-empty-partial.sqlite3");
    seed(&path);
    clear_usage_rows(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let page = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(MAX_ACTIVITY_ROWS).expect("bounded page size"),
        ))
        .expect("empty activity");
    let (header, payload) = page.into_parts();
    let mut parts = header.into_parts();
    parts.quality = QueryQuality::Partial;
    let page = QueryEnvelope::new(
        tokenmaster_query::QueryHeader::new(parts).expect("partial header"),
        payload,
    );
    let mut reducer = ProductReducer::new();
    reducer
        .publish_activity(attempt(1), page)
        .expect("publish partial empty activity");

    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Activity);
    let activity = state.projection().activity();
    assert_eq!(activity.state(), DesktopDashboardSectionState::Degraded);
    assert_eq!(activity.rows().len(), 0);
    assert_eq!(activity.has_more(), Some(false));
    assert!(
        activity
            .reason_codes()
            .iter()
            .any(|reason| reason == "partial")
    );
}

#[test]
fn retained_activity_failure_keeps_rows_and_current_failure_reason() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("activity-retained.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let page = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(MAX_ACTIVITY_ROWS).expect("bounded page size"),
        ))
        .expect("latest activity");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_activity(attempt(1), page)
        .expect("publish activity");
    reducer
        .fail_activity(
            attempt(2),
            tokenmaster_query::QueryErrorCode::DeadlineExceeded,
        )
        .expect("retain activity");

    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Activity);
    let activity = state.projection().activity();
    assert_eq!(activity.state(), DesktopDashboardSectionState::Degraded);
    assert_eq!(activity.rows().len(), 1);
    assert!(
        activity
            .reason_codes()
            .iter()
            .any(|reason| reason == "deadline_exceeded")
    );
}

#[test]
fn activity_cap_preserves_newest_order_and_page_incompleteness() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("activity-cap.sqlite3");
    seed(&path);
    add_distinct_usage_rows(&path, 70);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let page = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(64).expect("query page size"),
        ))
        .expect("latest activity");
    assert_eq!(page.payload().items().len(), 64);
    assert!(page.payload().has_more());
    let mut reducer = ProductReducer::new();
    reducer
        .publish_activity(attempt(1), page)
        .expect("publish activity");

    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Activity);
    let activity = state.projection().activity();
    assert_eq!(activity.rows().len(), MAX_ACTIVITY_ROWS);
    assert_eq!(activity.rows()[0].model(), "model-069");
    assert_eq!(activity.rows()[1].model(), "model-068");
    assert_eq!(activity.has_more(), Some(true));
    assert_eq!(activity.state(), DesktopDashboardSectionState::Degraded);
    assert!(
        activity
            .reason_codes()
            .iter()
            .any(|reason| reason == "activity_truncated")
    );
}

#[test]
fn partial_evidence_degrades_activity_without_losing_safe_rows() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("activity-partial.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let page = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(MAX_ACTIVITY_ROWS).expect("bounded page size"),
        ))
        .expect("latest activity");
    let (header, payload) = page.into_parts();
    let mut parts = header.into_parts();
    parts.quality = QueryQuality::Partial;
    let page = QueryEnvelope::new(
        tokenmaster_query::QueryHeader::new(parts).expect("partial header"),
        payload,
    );
    let mut reducer = ProductReducer::new();
    reducer
        .publish_activity(attempt(1), page)
        .expect("publish partial activity");

    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Activity);
    let activity = state.projection().activity();
    assert_eq!(activity.state(), DesktopDashboardSectionState::Degraded);
    assert_eq!(activity.rows().len(), 1);
    assert!(
        activity
            .reason_codes()
            .iter()
            .any(|reason| reason == "partial")
    );
}
