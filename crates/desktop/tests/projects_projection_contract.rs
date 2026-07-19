mod support;

use std::sync::Arc;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopCostComposition, DesktopCostMode, DesktopDashboardSectionState, DesktopRouteKey,
    DesktopState, DesktopValueAvailability, MAX_PROJECT_ROWS,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{
    GitOutputRequest, QueryErrorCode, QueryService, UsageAnalyticsRequest, UsageBreakdownKind,
    UsageRange, UsageSeriesSelection, UsageTimeZone, WeekStart,
};

use support::dashboard_fixture::{
    FixedClock, add_distinct_project_usage_rows, add_git_only_project,
    add_same_project_git_repositories, add_same_project_git_repository, clear_usage_rows,
    make_partial_model_usage, make_usage_unassociated, range, seed,
};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

fn recent_request(breakdowns: Vec<UsageBreakdownKind>) -> UsageAnalyticsRequest {
    UsageAnalyticsRequest::new(
        UsageRange::recent_days(30).expect("recent range"),
        UsageTimeZone::iana("Asia/Jerusalem").expect("fixture timezone"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        breakdowns,
    )
    .expect("recent analytics request")
}

fn publish_projects(path: &std::path::Path, reducer: &mut ProductReducer, generation: u64) {
    let mut service = QueryService::open(path, FixedClock).expect("query service");
    let recent = service
        .usage_analytics(recent_request(vec![
            UsageBreakdownKind::Model,
            UsageBreakdownKind::Project,
        ]))
        .expect("recent analytics");
    let git = service
        .git_output(
            GitOutputRequest::new(range(), WeekStart::Monday, Vec::new(), 32).expect("Git request"),
        )
        .expect("Git output");
    reducer
        .publish_history(attempt(generation), recent)
        .expect("publish recent analytics");
    reducer
        .publish_git(attempt(generation), git)
        .expect("publish Git");
}

#[test]
fn initial_projects_is_bounded_waiting_truth_without_fabricated_zeroes() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Projects);
    let projects = state.projection().projects();

    assert_eq!(MAX_PROJECT_ROWS, 32);
    assert_eq!(projects.state(), DesktopDashboardSectionState::Waiting);
    assert_eq!(projects.rows().len(), 0);
    assert_eq!(projects.usage_range(), None);
    assert_eq!(projects.usage_time_zone_id(), None);
    assert_eq!(projects.code_range(), None);
    assert_eq!(projects.code_time_zone_id(), None);
    assert_eq!(projects.loaded_repository_count(), None);
    assert_eq!(
        projects.total_tokens().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(
        projects.cost().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert!(!projects.usage_truncated());
    assert!(!projects.code_truncated());
}

#[test]
fn ready_projects_keeps_recent_usage_and_utc_today_code_explicitly_separate() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("projects-projection.sqlite3");
    seed(&path);
    let mut reducer = ProductReducer::new();
    publish_projects(&path, &mut reducer, 1);

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Projects);
    let projects = state.projection().projects();

    assert_eq!(projects.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(projects.usage_range(), Some(((2026, 6, 17), (2026, 7, 17))));
    assert_eq!(projects.usage_time_zone_id(), Some("Asia/Jerusalem"));
    assert_eq!(projects.code_range(), Some(((2026, 7, 16), (2026, 7, 17))));
    assert_eq!(projects.code_time_zone_id(), Some("UTC"));
    assert_eq!(projects.event_count(), Some(1));
    assert_eq!(projects.total_tokens().known_sum(), Some(140));
    assert_eq!(projects.cost().micros(), Some(10_000));
    assert_eq!(projects.cost().mode(), Some(DesktopCostMode::Auto));
    assert_eq!(
        projects.cost().composition(),
        Some(DesktopCostComposition::Reported)
    );
    assert_eq!(projects.rows().len(), 1);
    assert_eq!(projects.token_maximum(), Some(140));
    assert_eq!(projects.loaded_repository_count(), Some(1));
    assert!(!projects.usage_truncated());
    assert!(!projects.code_truncated());

    let row = &projects.rows()[0];
    assert_eq!(row.project(), "tokenmaster");
    assert!(!row.unassociated());
    assert_eq!(row.event_count(), 1);
    assert_eq!(row.input().known_sum(), Some(100));
    assert_eq!(row.cached().known_sum(), Some(20));
    assert_eq!(row.output().known_sum(), Some(30));
    assert_eq!(row.reasoning().known_sum(), Some(10));
    assert_eq!(row.total_tokens().known_sum(), Some(140));
    assert_eq!(row.cost().micros(), Some(10_000));
    assert!(row.code_available());
    assert!(row.code_complete());
    assert_eq!(row.repository_count(), 1);
    assert_eq!(row.commits(), Some(1));
    assert_eq!(row.added_lines(), Some(200));
    assert_eq!(row.removed_lines(), Some(20));
    assert_eq!(row.net_lines(), Some(180));
    assert_eq!(row.cost_per_100_added_lines_micros(), Some(5_000));
    assert_eq!(row.efficiency_unavailable_reason(), None);

    let debug = format!("{projects:?}");
    for forbidden in [
        path.to_string_lossy().as_ref(),
        "dashboard-private-source",
        "dashboard-private-session",
        "dashboard-private-event",
        "repository_id",
        "association_id",
        "dataset_identity",
        "SELECT ",
    ] {
        assert!(!debug.contains(forbidden), "projects exposed {forbidden}");
    }

    reducer
        .fail_git(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain Git payload");
    let retained = reducer.snapshot();
    let retained_state = DesktopState::new(&retained, DesktopRouteKey::Projects);
    let retained_projects = retained_state.projection().projects();
    assert_eq!(
        retained_projects.state(),
        DesktopDashboardSectionState::Degraded
    );
    assert!(retained_projects.rows()[0].code_available());
    assert_eq!(retained_projects.rows()[0].added_lines(), Some(200));
    assert!(
        retained_projects
            .reason_codes()
            .iter()
            .any(|reason| reason == "deadline_exceeded")
    );
}

#[test]
fn unassociated_usage_is_explicit_and_never_matches_git() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("projects-unassociated.sqlite3");
    seed(&path);
    make_usage_unassociated(&path);
    let mut reducer = ProductReducer::new();
    publish_projects(&path, &mut reducer, 1);

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Projects);
    let projects = state.projection().projects();
    assert_eq!(projects.rows().len(), 1);
    let row = &projects.rows()[0];
    assert_eq!(row.project(), "Unassociated");
    assert!(row.unassociated());
    assert!(!row.code_available());
    assert_eq!(row.repository_count(), 0);
    assert_eq!(row.commits(), None);
    assert_eq!(
        row.efficiency_unavailable_reason(),
        Some("unassociated_project")
    );
}

#[test]
fn same_project_repositories_sum_code_but_count_project_cost_once() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("projects-same-alias.sqlite3");
    seed(&path);
    add_same_project_git_repository(&path);
    let mut reducer = ProductReducer::new();
    publish_projects(&path, &mut reducer, 1);

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Projects);
    let row = &state.projection().projects().rows()[0];
    assert_eq!(row.project(), "tokenmaster");
    assert_eq!(row.repository_count(), 2);
    assert_eq!(row.commits(), Some(2));
    assert_eq!(row.added_lines(), Some(300));
    assert_eq!(row.removed_lines(), Some(30));
    assert_eq!(row.net_lines(), Some(270));
    assert_eq!(row.cost_per_100_added_lines_micros(), Some(3_333));
}

#[test]
fn git_only_projects_are_not_invented_as_zero_usage_rows() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("projects-git-only.sqlite3");
    seed(&path);
    add_git_only_project(&path);
    let mut reducer = ProductReducer::new();
    publish_projects(&path, &mut reducer, 1);

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Projects);
    let projects = state.projection().projects();
    assert_eq!(projects.rows().len(), 1);
    assert_eq!(projects.rows()[0].project(), "tokenmaster");
    assert!(
        projects
            .rows()
            .iter()
            .all(|row| row.project() != "git-only-project")
    );
}

#[test]
fn partial_cost_degrades_projects_while_git_remains_ready() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("projects-degraded.sqlite3");
    seed(&path);
    make_partial_model_usage(&path);
    let mut reducer = ProductReducer::new();
    publish_projects(&path, &mut reducer, 1);

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Projects);
    let projects = state.projection().projects();
    assert_eq!(projects.state(), DesktopDashboardSectionState::Degraded);
    assert_eq!(projects.rows().len(), 1);
    assert_eq!(
        projects.rows()[0].cost().availability(),
        DesktopValueAvailability::Partial
    );
    assert!(projects.rows()[0].code_available());
    assert!(
        projects
            .reason_codes()
            .iter()
            .any(|reason| reason == "cost_partial")
    );
}

#[test]
fn usage_survives_git_failure_without_fabricating_code_zeroes() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("projects-git-failure.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let recent = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Project]))
        .expect("recent analytics");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_history(attempt(1), recent)
        .expect("publish recent analytics");
    reducer
        .fail_git(attempt(1), QueryErrorCode::DeadlineExceeded)
        .expect("fail Git");

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Projects);
    let projects = state.projection().projects();
    assert_eq!(projects.state(), DesktopDashboardSectionState::Degraded);
    assert_eq!(projects.rows().len(), 1);
    assert_eq!(projects.rows()[0].total_tokens().known_sum(), Some(140));
    assert!(!projects.rows()[0].code_available());
    assert_eq!(projects.rows()[0].commits(), None);
    assert_eq!(
        projects.rows()[0].efficiency_unavailable_reason(),
        Some("git_unavailable")
    );
}

#[test]
fn empty_missing_and_capped_project_breakdowns_remain_distinct() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("projects-bounds.sqlite3");
    seed(&path);
    clear_usage_rows(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let empty = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Project]))
        .expect("empty analytics");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_history(attempt(1), empty)
        .expect("publish empty analytics");
    let empty_snapshot = reducer.snapshot();
    let empty_state = DesktopState::new(&empty_snapshot, DesktopRouteKey::Projects);
    let empty_projects = empty_state.projection().projects();
    assert_eq!(empty_projects.rows().len(), 0);
    assert_eq!(empty_projects.event_count(), Some(0));
    assert_eq!(
        empty_projects.cost().availability(),
        DesktopValueAvailability::LegitimateZero
    );

    let missing = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Model]))
        .expect("model-only analytics");
    reducer
        .publish_history(attempt(2), missing)
        .expect("publish missing breakdown");
    let missing_snapshot = reducer.snapshot();
    let missing_state = DesktopState::new(&missing_snapshot, DesktopRouteKey::Projects);
    assert!(
        missing_state
            .projection()
            .projects()
            .reason_codes()
            .iter()
            .any(|reason| reason == "projects_breakdown_unavailable")
    );

    add_distinct_project_usage_rows(&path, 40);
    let capped = service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Project]))
        .expect("capped analytics");
    reducer
        .publish_history(attempt(3), capped)
        .expect("publish capped breakdown");
    let capped_snapshot = reducer.snapshot();
    let capped_state = DesktopState::new(&capped_snapshot, DesktopRouteKey::Projects);
    let capped_projects = capped_state.projection().projects();
    assert_eq!(capped_projects.rows().len(), MAX_PROJECT_ROWS);
    assert!(capped_projects.usage_truncated());
    assert!(
        capped_projects
            .reason_codes()
            .iter()
            .any(|reason| reason == "projects_truncated")
    );
}

#[test]
fn backend_project_and_git_lookahead_truncation_survive_desktop_bounds() {
    let project_directory = TempDir::new().expect("project directory");
    let project_path = project_directory
        .path()
        .join("projects-backend-cap.sqlite3");
    seed(&project_path);
    add_distinct_project_usage_rows(&project_path, 256);
    let mut project_service =
        QueryService::open(&project_path, FixedClock).expect("project query service");
    let recent = project_service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Project]))
        .expect("project analytics");
    let breakdown = &recent.payload().breakdowns()[0];
    assert_eq!(breakdown.items().len(), 256);
    assert!(breakdown.truncated());
    let mut project_reducer = ProductReducer::new();
    project_reducer
        .publish_history(attempt(1), recent)
        .expect("publish truncated projects");
    let project_snapshot = project_reducer.snapshot();
    let project_state = DesktopState::new(&project_snapshot, DesktopRouteKey::Projects);
    let project_projection = project_state.projection().projects();
    assert_eq!(project_projection.rows().len(), MAX_PROJECT_ROWS);
    assert!(project_projection.usage_truncated());

    let git_directory = TempDir::new().expect("Git directory");
    let git_path = git_directory.path().join("projects-git-cap.sqlite3");
    seed(&git_path);
    add_same_project_git_repositories(&git_path, 31);
    let mut git_service = QueryService::open(&git_path, FixedClock).expect("Git query service");
    let git_recent = git_service
        .usage_analytics(recent_request(vec![UsageBreakdownKind::Project]))
        .expect("Git fixture recent analytics");
    let truncated_git = git_service
        .git_output(
            GitOutputRequest::new(range(), WeekStart::Monday, Vec::new(), 16)
                .expect("truncated Git request"),
        )
        .expect("truncated Git output");
    assert_eq!(truncated_git.payload().repositories().len(), 16);
    assert!(truncated_git.payload().has_more_repositories());
    let mut git_reducer = ProductReducer::new();
    git_reducer
        .publish_history(attempt(1), git_recent)
        .expect("publish Git fixture analytics");
    git_reducer
        .publish_git(attempt(1), truncated_git)
        .expect("publish truncated Git");
    let git_snapshot = git_reducer.snapshot();
    let git_state = DesktopState::new(&git_snapshot, DesktopRouteKey::Projects);
    let git_projection = git_state.projection().projects();
    assert_eq!(git_projection.loaded_repository_count(), Some(16));
    assert!(git_projection.code_truncated());
    assert!(!git_projection.code_complete());
    assert!(git_projection.rows()[0].code_available());
    assert!(!git_projection.rows()[0].code_complete());
    assert_eq!(git_projection.rows()[0].repository_count(), 16);
    assert!(
        git_projection
            .reason_codes()
            .iter()
            .any(|reason| reason == "git_repositories_truncated")
    );
}

#[test]
fn ten_thousand_snapshot_replacements_release_the_old_projects_list() {
    let mut reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    let mut state = DesktopState::new(&initial, DesktopRouteKey::Projects);
    let old_rows = Arc::clone(state.projection().projects().rows());
    let old_rows_weak = Arc::downgrade(&old_rows);
    drop(old_rows);

    for generation in 1..=10_000 {
        reducer
            .fail_data_status(attempt(generation), QueryErrorCode::Unavailable)
            .expect("new product generation");
        state.apply_snapshot(&reducer.snapshot());
    }

    assert!(old_rows_weak.upgrade().is_none());
    assert_eq!(state.projection().generation().get(), 10_000);
    assert_eq!(state.projection().projects().rows().len(), 0);
}
