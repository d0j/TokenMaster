mod support;

use tempfile::TempDir;
use tokenmaster_query::{
    CalendarDate, GitEfficiency, GitEfficiencyUnavailableReason, GitOutputRequest, UsageRange,
    WeekStart,
};

use support::git_output::{
    DAY_INDEX, WALL_TIME_MS, request, seed_current_usage, seed_repository, seed_repository_at,
    service, summary, summary_range,
};

#[test]
fn zero_product_code_lines_are_explicitly_unavailable() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("zero-lines.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 0, 20));

    let snapshot = service(&path).git_output(request(32)).expect("Git output");
    assert_eq!(
        snapshot.payload().repositories()[0].efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::ZeroProductCodeLines)
    );
}

#[test]
fn unmatched_project_is_not_guessed_from_repository_identity() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("unmatched.sqlite3");
    seed_current_usage(&path, "other-project", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));

    let snapshot = service(&path).git_output(request(32)).expect("Git output");
    let repository = &snapshot.payload().repositories()[0];
    assert!(repository.project_alias().is_none());
    assert_eq!(
        repository.efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::ProjectNotInUsageSnapshot)
    );
}

#[test]
fn conflicting_repository_associations_disable_the_join() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("ambiguous.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    seed_repository_at(
        &path,
        1,
        3,
        "different-project",
        summary(DAY_INDEX, 200, 20),
        WALL_TIME_MS,
    );

    let snapshot = service(&path).git_output(request(32)).expect("Git output");
    let repository = &snapshot.payload().repositories()[0];
    assert!(repository.project_alias().is_none());
    assert_eq!(
        repository.efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::GitQualityIncomplete)
    );
}

#[test]
fn stale_git_scan_disables_the_join_without_hiding_metrics() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("stale.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository_at(
        &path,
        1,
        2,
        "tokenmaster",
        summary(DAY_INDEX, 200, 20),
        WALL_TIME_MS - tokenmaster_query::QUERY_STALE_MIN_AGE_MS - 1,
    );

    let snapshot = service(&path).git_output(request(32)).expect("Git output");
    let repository = &snapshot.payload().repositories()[0];
    assert_eq!(repository.range_totals().lines().added(), 200);
    assert_eq!(
        repository.efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::GitStale)
    );
}

#[test]
fn unavailable_usage_projection_does_not_hide_independent_git_facts() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("usage-unavailable.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    let mut service = service(&path);
    let connection = rusqlite::Connection::open(&path).expect("fixture connection");
    connection
        .execute(
            "UPDATE usage_aggregate_state
             SET state = 'rebuild_required'
             WHERE singleton_id = 1",
            [],
        )
        .expect("make usage projection unavailable");

    let snapshot = service
        .git_output(request(32))
        .expect("Git facts remain available");
    let repository = &snapshot.payload().repositories()[0];
    assert_eq!(repository.range_totals().lines().added(), 200);
    assert!(repository.project_alias().is_none());
    assert_eq!(
        repository.efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::UsageEvidenceUnavailable)
    );
}

#[test]
fn unknown_usage_cost_is_explicitly_unavailable() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("unknown-cost.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    let connection = rusqlite::Connection::open(&path).expect("fixture connection");
    connection
        .execute(
            "UPDATE usage_event
             SET model = 'unpriced-private-model',
                 reported_cost_usd_micros = NULL
             WHERE event_id = 'event-1'",
            [],
        )
        .expect("remove exact cost");

    let snapshot = service(&path).git_output(request(32)).expect("Git output");
    assert_eq!(
        snapshot.payload().repositories()[0].efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::UsageCostUnavailable)
    );
}

#[test]
fn stale_usage_publication_disables_only_efficiency() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("usage-stale.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    let connection = rusqlite::Connection::open(&path).expect("fixture connection");
    let completed_at_ms = WALL_TIME_MS - tokenmaster_query::QUERY_STALE_MIN_AGE_MS - 1;
    connection
        .execute_batch(&format!(
            "UPDATE usage_scan_set
             SET started_at_ms = {started_at_ms}, completed_at_ms = {completed_at_ms}
             WHERE scan_set_id = 1;
             UPDATE usage_scan
             SET started_at_ms = {started_at_ms}, completed_at_ms = {completed_at_ms}
             WHERE scan_id = 1;",
            started_at_ms = completed_at_ms - 1_000
        ))
        .expect("age usage publication");

    let snapshot = service(&path).git_output(request(32)).expect("Git output");
    let repository = &snapshot.payload().repositories()[0];
    assert_eq!(
        repository.project_alias().expect("exact alias").as_str(),
        "tokenmaster"
    );
    assert_eq!(
        repository.efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::UsageStale)
    );
}

#[test]
fn truncated_daily_history_keeps_totals_but_disables_range_efficiency() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("retention.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(
        &path,
        1,
        2,
        "tokenmaster",
        summary_range(DAY_INDEX - 400, 401),
    );
    let request = GitOutputRequest::new(
        UsageRange::custom(
            CalendarDate::new(2025, 6, 11).expect("start"),
            CalendarDate::new(2026, 7, 16).expect("end"),
        )
        .expect("400-day range"),
        WeekStart::Monday,
        Vec::new(),
        32,
    )
    .expect("request");

    let snapshot = service(&path).git_output(request).expect("Git output");
    let repository = &snapshot.payload().repositories()[0];
    assert!(repository.daily_history_truncated());
    assert!(!repository.range_complete());
    assert_eq!(repository.all_time_totals().commits(), 401);
    assert_eq!(
        repository.efficiency(),
        &GitEfficiency::Unavailable(GitEfficiencyUnavailableReason::GitRangeIncomplete)
    );
}
