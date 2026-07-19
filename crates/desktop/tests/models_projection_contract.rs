mod support;

use std::sync::Arc;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopCostComposition, DesktopCostMode, DesktopDashboardSectionState, DesktopRouteKey,
    DesktopState, DesktopValueAvailability, MAX_MODEL_ROWS,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{
    QueryErrorCode, QueryService, UsageAnalyticsRequest, UsageBreakdownKind, UsageRange,
    UsageSeriesSelection, UsageTimeZone, WeekStart,
};

use support::dashboard_fixture::{
    FixedClock, add_distinct_usage_rows, clear_usage_rows, make_partial_model_usage, seed,
};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

fn recent_request(breakdowns: Vec<UsageBreakdownKind>) -> UsageAnalyticsRequest {
    UsageAnalyticsRequest::new(
        UsageRange::recent_days(30).expect("recent range"),
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        breakdowns,
    )
    .expect("recent analytics request")
}

#[test]
fn initial_models_is_bounded_waiting_truth_without_fabricated_zeroes() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Models);
    let models = state.projection().models();

    assert_eq!(MAX_MODEL_ROWS, 64);
    assert_eq!(models.state(), DesktopDashboardSectionState::Waiting);
    assert_eq!(models.rows().len(), 0);
    assert_eq!(
        models.total_tokens().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(
        models.cost().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(models.event_count(), None);
    assert_eq!(models.range(), None);
    assert_eq!(models.time_zone_id(), None);
    assert!(!models.truncated());
}

#[test]
fn ten_thousand_snapshot_replacements_release_the_old_models_list() {
    let mut reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    let mut state = DesktopState::new(&initial, DesktopRouteKey::Models);
    let old_rows = Arc::clone(state.projection().models().rows());
    let old_rows_weak = Arc::downgrade(&old_rows);
    drop(old_rows);

    for generation in 1..=10_000 {
        reducer
            .fail_data_status(attempt(generation), QueryErrorCode::Unavailable)
            .expect("new product generation");
        let snapshot = reducer.snapshot();
        state.apply_snapshot(&snapshot);
    }

    assert!(old_rows_weak.upgrade().is_none());
    assert_eq!(state.projection().generation().get(), 10_000);
    assert_eq!(state.projection().selected(), DesktopRouteKey::Models);
    assert_eq!(state.projection().models().rows().len(), 0);
}

#[test]
fn ready_models_maps_complete_recent_token_mix_and_cost_evidence() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("models-projection.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let recent = service
        .usage_analytics(recent_request(vec![
            UsageBreakdownKind::Model,
            UsageBreakdownKind::Project,
        ]))
        .expect("recent analytics");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_history(attempt(1), recent)
        .expect("publish recent analytics");

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Models);
    let models = state.projection().models();

    assert_eq!(models.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(models.range(), Some(((2026, 6, 17), (2026, 7, 17))));
    assert_eq!(models.time_zone_id(), Some("UTC"));
    assert_eq!(models.event_count(), Some(1));
    assert_eq!(models.total_tokens().known_sum(), Some(140));
    assert_eq!(models.cost().micros(), Some(10_000));
    assert_eq!(models.cost().mode(), Some(DesktopCostMode::Auto));
    assert_eq!(
        models.cost().composition(),
        Some(DesktopCostComposition::Reported)
    );
    assert_eq!(models.rows().len(), 1);
    assert_eq!(models.token_maximum(), Some(140));
    assert!(!models.truncated());

    let row = &models.rows()[0];
    assert_eq!(row.model(), "gpt-5.6");
    assert_eq!(row.event_count(), 1);
    assert_eq!(row.input().known_sum(), Some(100));
    assert_eq!(row.cached().known_sum(), Some(20));
    assert_eq!(row.output().known_sum(), Some(30));
    assert_eq!(row.reasoning().known_sum(), Some(10));
    assert_eq!(row.total_tokens().known_sum(), Some(140));
    assert_eq!(row.cost().micros(), Some(10_000));
    assert_eq!(row.cost().mode(), Some(DesktopCostMode::Auto));
    assert_eq!(
        row.cost().composition(),
        Some(DesktopCostComposition::Reported)
    );

    let debug = format!("{models:?}");
    for forbidden in [
        path.to_string_lossy().as_ref(),
        "dashboard-private-source",
        "dashboard-private-session",
        "dashboard-private-event",
        "dashboard-private-account",
        "SELECT ",
    ] {
        assert!(!debug.contains(forbidden), "models exposed {forbidden}");
    }

    reducer
        .fail_history(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain recent analytics");
    let retained = reducer.snapshot();
    let retained_state = DesktopState::new(&retained, DesktopRouteKey::Models);
    let retained_models = retained_state.projection().models();
    assert_eq!(
        retained_models.state(),
        DesktopDashboardSectionState::Degraded
    );
    assert_eq!(retained_models.rows().len(), 1);
    assert!(
        retained_models
            .reason_codes()
            .iter()
            .any(|reason| reason == "deadline_exceeded")
    );
}

#[test]
fn ready_empty_models_is_explicit_without_fabricating_token_evidence() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("models-empty.sqlite3");
    seed(&path);
    clear_usage_rows(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let recent = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Model]))
        .expect("empty recent analytics");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_history(attempt(1), recent)
        .expect("publish empty analytics");

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Models);
    let models = state.projection().models();

    assert_eq!(models.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(models.rows().len(), 0);
    assert_eq!(models.event_count(), Some(0));
    assert_eq!(
        models.total_tokens().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(models.total_tokens().known_sum(), None);
    assert_eq!(
        models.cost().availability(),
        DesktopValueAvailability::LegitimateZero
    );
    assert_eq!(models.cost().micros(), Some(0));
    assert_eq!(models.cost().mode(), Some(DesktopCostMode::Auto));
    assert_eq!(
        models.cost().composition(),
        Some(DesktopCostComposition::None)
    );
    assert!(!models.truncated());
}

#[test]
fn partial_models_preserve_component_cost_and_provenance_truth() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("models-partial.sqlite3");
    seed(&path);
    make_partial_model_usage(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let recent = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Model]))
        .expect("partial recent analytics");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_history(attempt(1), recent)
        .expect("publish partial analytics");

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Models);
    let models = state.projection().models();

    assert_eq!(models.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(models.rows().len(), 1);
    assert_eq!(
        models.total_tokens().availability(),
        DesktopValueAvailability::Known
    );
    assert_eq!(
        models.cost().availability(),
        DesktopValueAvailability::Partial
    );
    assert_eq!(models.cost().micros(), Some(10_000));
    assert_eq!(models.cost().mode(), Some(DesktopCostMode::Auto));
    assert_eq!(
        models.cost().composition(),
        Some(DesktopCostComposition::Reported)
    );

    let row = &models.rows()[0];
    assert_eq!(row.model(), "fixture-unpriced-model");
    assert_eq!(row.event_count(), 2);
    assert_eq!(
        row.input().availability(),
        DesktopValueAvailability::Partial
    );
    assert_eq!(row.input().known_sum(), Some(50));
    assert_eq!(row.input().known_count(), 1);
    assert_eq!(row.input().event_count(), 2);
    assert_eq!(row.cost().availability(), DesktopValueAvailability::Partial);
    assert_eq!(row.cost().micros(), Some(10_000));
    assert_eq!(row.cost().mode(), Some(DesktopCostMode::Auto));
    assert_eq!(
        row.cost().composition(),
        Some(DesktopCostComposition::Reported)
    );
}

#[test]
fn models_cap_and_missing_breakdown_are_explicit_and_local() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("models-cap.sqlite3");
    seed(&path);
    add_distinct_usage_rows(&path, 70);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let recent = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Model]))
        .expect("recent model analytics");
    assert_eq!(recent.payload().breakdowns()[0].items().len(), 71);
    assert!(!recent.payload().breakdowns()[0].truncated());

    let mut reducer = ProductReducer::new();
    reducer
        .publish_history(attempt(1), recent)
        .expect("publish recent analytics");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Models);
    let models = state.projection().models();

    assert_eq!(models.rows().len(), MAX_MODEL_ROWS);
    assert_eq!(models.rows()[0].model(), "gpt-5.6");
    assert!(models.truncated());
    assert_eq!(models.state(), DesktopDashboardSectionState::Degraded);
    assert!(
        models
            .reason_codes()
            .iter()
            .any(|reason| reason == "models_truncated")
    );

    let project_only = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Project]))
        .expect("recent project analytics");
    reducer
        .publish_history(attempt(2), project_only)
        .expect("publish project-only analytics");
    let missing = reducer.snapshot();
    let missing_state = DesktopState::new(&missing, DesktopRouteKey::Models);
    let missing_models = missing_state.projection().models();
    assert_eq!(missing_models.rows().len(), 0);
    assert!(!missing_models.truncated());
    assert_eq!(
        missing_models.state(),
        DesktopDashboardSectionState::Degraded
    );
    assert!(
        missing_models
            .reason_codes()
            .iter()
            .any(|reason| reason == "models_breakdown_unavailable")
    );
    assert!(
        missing_models
            .rows()
            .iter()
            .all(|row| row.model() != "tokenmaster")
    );
}

#[test]
fn backend_model_lookahead_truncation_survives_the_desktop_cap() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("models-backend-truncation.sqlite3");
    seed(&path);
    add_distinct_usage_rows(&path, 256);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let recent = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Model]))
        .expect("recent model analytics");
    let breakdown = &recent.payload().breakdowns()[0];
    assert_eq!(breakdown.items().len(), 256);
    assert!(breakdown.truncated());

    let mut reducer = ProductReducer::new();
    reducer
        .publish_history(attempt(1), recent)
        .expect("publish recent analytics");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Models);
    let models = state.projection().models();

    assert_eq!(models.rows().len(), MAX_MODEL_ROWS);
    assert!(models.truncated());
    assert!(
        models
            .reason_codes()
            .iter()
            .any(|reason| reason == "models_truncated")
    );
}
