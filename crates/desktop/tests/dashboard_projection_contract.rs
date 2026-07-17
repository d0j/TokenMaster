mod support;

use std::sync::Arc;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopActivityKey, DesktopDashboardSectionKey, DesktopDashboardSectionState, DesktopFreshness,
    DesktopQuality, DesktopRouteKey, DesktopState, DesktopValueAvailability,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{
    BenefitOverviewRequest, CalendarDate, GitOutputRequest, PageSize, QueryErrorCode, QueryService,
    UsageAnalyticsRequest, UsageBreakdownKind, UsageRange, UsageSeriesSelection,
    UsageSessionPageRequest, UsageTimeZone, WeekStart,
};

use support::dashboard_fixture::{
    BENEFIT_EXPIRY_AT_MS, FixedClock, RESET_AT_MS, add_distinct_usage_rows, add_second_git_project,
    range, seed,
};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

#[test]
fn initial_dashboard_has_six_exact_bounded_waiting_sections() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Dashboard);
    let dashboard = state.projection().dashboard();

    assert_eq!(
        dashboard
            .sections()
            .iter()
            .map(|section| section.key())
            .collect::<Vec<_>>(),
        vec![
            DesktopDashboardSectionKey::PlanUsage,
            DesktopDashboardSectionKey::CodeOutput,
            DesktopDashboardSectionKey::Trend,
            DesktopDashboardSectionKey::Sessions,
            DesktopDashboardSectionKey::Activity,
            DesktopDashboardSectionKey::Models,
        ]
    );
    assert!(dashboard.sections().iter().all(|section| {
        section.state() == DesktopDashboardSectionState::Waiting && !section.has_data()
    }));
    assert_eq!(
        dashboard.header().tokens().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(dashboard.quota_rows().len(), 0);
    assert_eq!(dashboard.benefit_scopes().len(), 0);
    assert_eq!(dashboard.trend_points().len(), 0);
    assert_eq!(dashboard.sessions().len(), 0);
    assert_eq!(dashboard.activity().len(), 8);
    assert!(dashboard.activity().iter().all(|row| row.count() == 0));
    assert_eq!(dashboard.models().len(), 0);
}

#[test]
fn section_failures_are_local_path_free_and_do_not_become_zero_data() {
    let mut reducer = ProductReducer::new();
    reducer
        .fail_analytics(attempt(1), QueryErrorCode::DeadlineExceeded)
        .expect("analytics failure");
    reducer
        .fail_quota(attempt(1), QueryErrorCode::CapacityExceeded)
        .expect("quota failure");
    reducer
        .fail_benefit(attempt(1), QueryErrorCode::Unavailable)
        .expect("benefit failure");
    reducer
        .fail_git(attempt(1), QueryErrorCode::CorruptArchive)
        .expect("Git failure");
    reducer
        .fail_sessions(attempt(1), QueryErrorCode::StaleSnapshot)
        .expect("session failure");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Dashboard);
    let dashboard = state.projection().dashboard();

    for key in [
        DesktopDashboardSectionKey::PlanUsage,
        DesktopDashboardSectionKey::CodeOutput,
        DesktopDashboardSectionKey::Trend,
        DesktopDashboardSectionKey::Sessions,
        DesktopDashboardSectionKey::Activity,
        DesktopDashboardSectionKey::Models,
    ] {
        let section = dashboard.section(key);
        assert_eq!(section.state(), DesktopDashboardSectionState::Unavailable);
        assert!(!section.has_data());
        assert!(!section.reason_codes().is_empty());
    }
    assert_eq!(
        dashboard.header().tokens().availability(),
        DesktopValueAvailability::Unavailable
    );
    assert_eq!(
        dashboard.header().cost().availability(),
        DesktopValueAvailability::Unavailable
    );
    let debug = format!("{dashboard:?}");
    assert!(!debug.contains("C:\\"));
    assert!(!debug.contains("SELECT "));
}

#[test]
fn ten_thousand_snapshot_replacements_release_the_old_dashboard_models() {
    let mut reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    let mut state = DesktopState::new(&initial, DesktopRouteKey::Dashboard);
    let old_models = Arc::clone(state.projection().dashboard().models());
    let old_models_weak = Arc::downgrade(&old_models);
    drop(old_models);

    for generation in 1..=10_000 {
        reducer
            .fail_data_status(attempt(generation), QueryErrorCode::Unavailable)
            .expect("new product generation");
        let snapshot = reducer.snapshot();
        state.apply_snapshot(&snapshot);
    }

    assert!(old_models_weak.upgrade().is_none());
    assert_eq!(state.projection().generation().get(), 10_000);
    assert_eq!(state.projection().dashboard().models().len(), 0);
}

#[test]
fn production_snapshot_maps_dynamic_values_without_private_identity_leakage() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("dashboard-private-archive.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let analytics = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                range(),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![UsageBreakdownKind::Model],
            )
            .expect("analytics request"),
        )
        .expect("analytics");
    let quota = service.quota_overview().expect("quota overview");
    let benefits = service
        .benefit_overview(BenefitOverviewRequest::new())
        .expect("benefit overview");
    let git = service
        .git_output(
            GitOutputRequest::new(range(), WeekStart::Monday, Vec::new(), 32).expect("Git request"),
        )
        .expect("Git output");
    let sessions = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(12).expect("page size"), Vec::new())
                .expect("session request"),
        )
        .expect("sessions");

    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_analytics(attempt(1), analytics)
        .expect("publish analytics");
    reducer
        .publish_quota(attempt(1), quota)
        .expect("publish quota");
    reducer
        .publish_benefit(attempt(1), benefits)
        .expect("publish benefits");
    reducer.publish_git(attempt(1), git).expect("publish Git");
    reducer
        .publish_sessions(attempt(1), sessions)
        .expect("publish sessions");

    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Dashboard);
    let dashboard = state.projection().dashboard();
    assert!(
        dashboard
            .sections()
            .iter()
            .all(|section| section.state() == DesktopDashboardSectionState::Ready)
    );

    let header = dashboard.header();
    assert_eq!(
        header.tokens().availability(),
        DesktopValueAvailability::Known
    );
    assert_eq!(header.tokens().known_sum(), Some(140));
    assert_eq!(
        header.cost().availability(),
        DesktopValueAvailability::Complete
    );
    assert_eq!(header.cost().micros(), Some(10_000));
    assert_eq!(header.cost().priced_events(), Some(1));
    assert_eq!(header.event_count(), Some(1));

    assert_eq!(dashboard.quota_rows().len(), 1);
    let quota = &dashboard.quota_rows()[0];
    assert_eq!(quota.label_key(), "quota.dynamic_weekly");
    assert_eq!(quota.used_ppm(), Some(700_000));
    assert_eq!(quota.remaining_ppm(), Some(300_000));
    assert_eq!(quota.used_units(), Some(700));
    assert_eq!(quota.remaining_units(), Some(300));
    assert_eq!(quota.capacity_units(), Some(1_000));
    assert_eq!(quota.advertised_reset_at_ms(), Some(RESET_AT_MS));

    assert_eq!(dashboard.benefit_scopes().len(), 1);
    let benefits = &dashboard.benefit_scopes()[0];
    assert_eq!(benefits.available_reset_quantity(), 2);
    assert_eq!(benefits.available_credit_quantity(), 4);
    assert_eq!(benefits.available_temporary_quantity(), 0);
    assert_eq!(benefits.non_available_quantity(), 10);
    assert_eq!(
        benefits.nearest_reset_expiry_at_ms(),
        Some(BENEFIT_EXPIRY_AT_MS)
    );
    assert_eq!(benefits.reminder_coverage(), "in_app_only");

    let code = dashboard.code_output();
    assert_eq!(code.repository_count(), 1);
    assert_eq!(code.commits(), 1);
    assert_eq!(code.added_lines(), 200);
    assert_eq!(code.removed_lines(), 20);
    assert_eq!(code.net_lines(), 180);
    assert!(code.complete());
    assert_eq!(code.freshness(), DesktopFreshness::Fresh);
    assert_eq!(code.quality(), DesktopQuality::Authoritative);
    assert_eq!(code.cost_per_100_added_lines_micros(), Some(5_000));

    assert_eq!(dashboard.trend_points().len(), 1);
    assert_eq!(dashboard.trend_max_tokens(), Some(140));
    assert_eq!(dashboard.trend_max_cost_micros(), Some(10_000));
    assert_eq!(dashboard.sessions().len(), 1);
    assert_eq!(dashboard.sessions()[0].ordinal(), 1);
    assert_eq!(dashboard.sessions()[0].tokens().known_sum(), Some(140));
    assert_eq!(dashboard.models().len(), 1);
    assert_eq!(dashboard.models()[0].model(), "gpt-5.6");
    assert_eq!(dashboard.models()[0].tokens().known_sum(), Some(140));
    assert_eq!(
        dashboard
            .activity()
            .iter()
            .map(|row| (row.key(), row.count()))
            .collect::<Vec<_>>(),
        vec![
            (DesktopActivityKey::Read, 1),
            (DesktopActivityKey::EditWrite, 2),
            (DesktopActivityKey::Search, 3),
            (DesktopActivityKey::Git, 4),
            (DesktopActivityKey::BuildTest, 5),
            (DesktopActivityKey::Web, 6),
            (DesktopActivityKey::Subagents, 7),
            (DesktopActivityKey::Terminal, 8),
        ]
    );

    let debug = format!("{dashboard:?}");
    for private in [
        path.to_string_lossy().as_ref(),
        "dashboard-private-account",
        "dashboard-private-source",
        "dashboard-private-session",
        "dashboard-private-event",
        "dynamic-weekly",
        "src/lib.rs",
    ] {
        assert!(!debug.contains(private), "dashboard exposed {private}");
    }

    reducer
        .fail_analytics(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain analytics");
    reducer
        .fail_quota(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain quota");
    reducer
        .fail_benefit(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain benefits");
    reducer
        .fail_git(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain Git");
    reducer
        .fail_sessions(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain sessions");
    let retained = reducer.snapshot();
    let retained_state = DesktopState::new(&retained, DesktopRouteKey::Dashboard);
    let retained_dashboard = retained_state.projection().dashboard();
    assert!(retained_dashboard.sections().iter().all(|section| {
        section.state() == DesktopDashboardSectionState::Degraded
            && section.has_data()
            && section
                .reason_codes()
                .iter()
                .any(|reason| reason == "deadline_exceeded")
    }));
    assert_eq!(retained_dashboard.header().tokens().known_sum(), Some(140));
    assert_eq!(retained_dashboard.quota_rows().len(), 1);
    assert_eq!(retained_dashboard.code_output().added_lines(), 200);
    assert_eq!(retained_dashboard.sessions().len(), 1);
}

#[test]
fn trend_cap_degrades_only_trend_and_retains_exactly_240_points() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("dashboard-trend-cap.sqlite3");
    seed(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let analytics = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::custom(
                    CalendarDate::new(2025, 11, 18).expect("start"),
                    CalendarDate::new(2026, 7, 17).expect("end"),
                )
                .expect("241-day range"),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![UsageBreakdownKind::Model],
            )
            .expect("analytics request"),
        )
        .expect("analytics");
    assert_eq!(analytics.payload().series().len(), 241);

    let mut reducer = ProductReducer::new();
    reducer
        .publish_analytics(attempt(1), analytics)
        .expect("publish analytics");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Dashboard);
    let dashboard = state.projection().dashboard();
    assert_eq!(dashboard.trend_points().len(), 240);
    let trend = dashboard.section(DesktopDashboardSectionKey::Trend);
    assert_eq!(trend.state(), DesktopDashboardSectionState::Degraded);
    assert!(
        trend
            .reason_codes()
            .iter()
            .any(|reason| reason == "trend_truncated")
    );
    assert_eq!(
        dashboard
            .section(DesktopDashboardSectionKey::Activity)
            .state(),
        DesktopDashboardSectionState::Ready
    );
    assert_eq!(
        dashboard
            .section(DesktopDashboardSectionKey::Models)
            .state(),
        DesktopDashboardSectionState::Ready
    );
}

#[test]
fn model_and_session_caps_are_local_and_retain_only_twelve_rows() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("dashboard-row-caps.sqlite3");
    seed(&path);
    add_distinct_usage_rows(&path, 12);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let analytics = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                range(),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![UsageBreakdownKind::Model],
            )
            .expect("analytics request"),
        )
        .expect("analytics");
    let sessions = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(16).expect("page size"), Vec::new())
                .expect("session request"),
        )
        .expect("sessions");
    assert_eq!(analytics.payload().breakdowns()[0].items().len(), 13);
    assert_eq!(sessions.payload().sessions().len(), 13);

    let mut reducer = ProductReducer::new();
    reducer
        .publish_analytics(attempt(1), analytics)
        .expect("publish analytics");
    reducer
        .publish_sessions(attempt(1), sessions)
        .expect("publish sessions");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Dashboard);
    let dashboard = state.projection().dashboard();

    assert_eq!(dashboard.models().len(), 12);
    assert!(dashboard.models_truncated());
    assert_eq!(
        dashboard
            .section(DesktopDashboardSectionKey::Models)
            .state(),
        DesktopDashboardSectionState::Degraded
    );
    assert_eq!(
        dashboard.section(DesktopDashboardSectionKey::Trend).state(),
        DesktopDashboardSectionState::Ready
    );
    assert_eq!(
        dashboard
            .section(DesktopDashboardSectionKey::Activity)
            .state(),
        DesktopDashboardSectionState::Ready
    );

    assert_eq!(dashboard.sessions().len(), 12);
    assert!(dashboard.sessions_truncated());
    let session_section = dashboard.section(DesktopDashboardSectionKey::Sessions);
    assert_eq!(
        session_section.state(),
        DesktopDashboardSectionState::Degraded
    );
    assert!(
        session_section
            .reason_codes()
            .iter()
            .any(|reason| reason == "sessions_truncated")
    );
}

#[test]
fn git_projection_checked_sums_distinct_exact_project_efficiency() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("dashboard-git-aggregation.sqlite3");
    seed(&path);
    add_second_git_project(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let git = service
        .git_output(
            GitOutputRequest::new(range(), WeekStart::Monday, Vec::new(), 32).expect("Git request"),
        )
        .expect("Git output");
    assert_eq!(git.payload().repositories().len(), 2);

    let mut reducer = ProductReducer::new();
    reducer.publish_git(attempt(1), git).expect("publish Git");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Dashboard);
    let dashboard = state.projection().dashboard();
    let code = dashboard.code_output();
    assert_eq!(code.repository_count(), 2);
    assert_eq!(code.commits(), 2);
    assert_eq!(code.added_lines(), 300);
    assert_eq!(code.removed_lines(), 30);
    assert_eq!(code.net_lines(), 270);
    assert_eq!(code.cost_per_100_added_lines_micros(), Some(5_000));
    assert_eq!(
        dashboard
            .section(DesktopDashboardSectionKey::CodeOutput)
            .state(),
        DesktopDashboardSectionState::Ready
    );
}
