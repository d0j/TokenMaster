mod support;

use slint::{ComponentHandle, Model, SharedString};
use tokenmaster_desktop::{DesktopApplyOutcome, DesktopShell};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{
    BenefitOverviewRequest, GitOutputRequest, PageSize, QueryErrorCode, QueryService,
    UsageAnalyticsRequest, UsageBreakdownKind, UsageSeriesSelection, UsageSessionPageRequest,
    UsageTimeZone, WeekStart,
};

use support::dashboard_fixture::{FixedClock, add_quota_windows, range, seed};

fn ready_reducer(path: &std::path::Path, additional_quota_windows: u8) -> ProductReducer {
    seed(path);
    add_quota_windows(path, additional_quota_windows);
    let mut service = QueryService::open(path, FixedClock).expect("query service");
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
        .expect("benefits");
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

    let attempt = ProductAttemptGeneration::new(1).expect("attempt");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt, status)
        .expect("publish status");
    reducer
        .publish_analytics(attempt, analytics)
        .expect("publish analytics");
    reducer
        .publish_quota(attempt, quota)
        .expect("publish quota");
    reducer
        .publish_benefit(attempt, benefits)
        .expect("publish benefits");
    reducer.publish_git(attempt, git).expect("publish Git");
    reducer
        .publish_sessions(attempt, sessions)
        .expect("publish sessions");
    reducer
}

#[test]
fn compiled_shell_renders_exact_route_model_and_switches_in_place() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();

    assert_eq!(window.get_route_rows().row_count(), 11);
    assert_eq!(
        window.get_product_generation(),
        snapshot.generation().get().to_string()
    );
    assert_eq!(window.get_active_route_key(), "dashboard");
    assert_eq!(window.get_active_route_state(), "unavailable");
    assert_eq!(window.get_active_route_reasons(), "data_status_unavailable");
    assert!(window.get_dashboard_visible());
    assert_eq!(window.get_dashboard_section_rows().row_count(), 6);
    assert_eq!(window.get_dashboard_header_tokens(), "—");
    assert_eq!(window.get_dashboard_header_cost(), "—");
    assert_eq!(window.get_dashboard_quota_rows().row_count(), 0);
    assert_eq!(window.get_dashboard_benefit_rows().row_count(), 0);
    assert_eq!(window.get_dashboard_trend_points().row_count(), 0);
    assert_eq!(window.get_dashboard_session_rows().row_count(), 0);
    assert_eq!(window.get_dashboard_activity_rows().row_count(), 8);
    assert_eq!(window.get_dashboard_model_rows().row_count(), 0);

    window.invoke_select_route(SharedString::from("settings"));
    assert_eq!(window.get_active_route_key(), "settings");
    assert_eq!(window.get_active_route_state(), "ready");
    assert!(!window.get_dashboard_visible());
    assert_eq!(
        window
            .get_route_rows()
            .iter()
            .filter(|row| row.selected)
            .count(),
        1
    );

    window.invoke_select_route(SharedString::from("not-a-route"));
    assert_eq!(window.get_active_route_key(), "settings");

    let attempt = ProductAttemptGeneration::new(1).expect("attempt");
    let mut reducer = reducer;
    reducer
        .fail_data_status(attempt, QueryErrorCode::Unavailable)
        .expect("new product generation");
    let newer = reducer.snapshot();
    assert_eq!(
        shell
            .apply_snapshot(&newer)
            .expect("shared presentation state remains available"),
        DesktopApplyOutcome::Accepted
    );
    assert_eq!(
        window.get_product_generation(),
        newer.generation().get().to_string()
    );
    assert_eq!(
        shell
            .apply_snapshot(&newer)
            .expect("shared presentation state remains available"),
        DesktopApplyOutcome::IgnoredNotNewer
    );
    assert_eq!(window.get_active_route_key(), "settings");

    assert_compiled_dashboard_renders_real_bounded_models_and_switches_layout_in_place();
}

fn assert_compiled_dashboard_renders_real_bounded_models_and_switches_layout_in_place() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-dashboard.sqlite3");
    let reducer = ready_reducer(&path, 0);
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();

    assert!(window.get_dashboard_visible());
    let sections = window.get_dashboard_section_rows();
    assert_eq!(sections.row_count(), 6);
    assert_eq!(sections.row_data(0).expect("plan").key, "plan_usage");
    assert_eq!(sections.row_data(5).expect("models").key, "models");
    assert!(sections.iter().all(|section| section.state == "ready"));
    assert_eq!(window.get_dashboard_header_tokens(), "140");
    assert_eq!(window.get_dashboard_header_cost(), "$0.010000");
    assert_eq!(window.get_dashboard_header_events(), "1 event");
    assert_eq!(
        window.get_dashboard_header_evidence(),
        "Fresh · Authoritative"
    );

    let quotas = window.get_dashboard_quota_rows();
    assert_eq!(quotas.row_count(), 1);
    let quota = quotas.row_data(0).expect("quota");
    assert_eq!(quota.label_key, "quota.dynamic_weekly");
    assert!(quota.ratio_known);
    assert!((quota.used_ratio - 0.7).abs() < f32::EPSILON);
    assert_eq!(quota.usage_label, "70.0% used");
    assert_eq!(quota.units_label, "700 / 1,000 tokens");
    assert!(quota.reset_label.starts_with("Resets "));

    let benefits = window.get_dashboard_benefit_rows();
    assert_eq!(benefits.row_count(), 1);
    let benefit = benefits.row_data(0).expect("benefit");
    assert_eq!(benefit.reset_quantity_label, "2");
    assert_eq!(benefit.credit_quantity_label, "4");
    assert_eq!(benefit.temporary_quantity_label, "0");
    assert_eq!(benefit.unavailable_quantity_label, "10");
    assert_eq!(benefit.reminder_label, "In-app reminders");

    assert_eq!(window.get_dashboard_code_commits(), "1 commit");
    assert_eq!(window.get_dashboard_code_added(), "+200");
    assert_eq!(window.get_dashboard_code_removed(), "−20");
    assert_eq!(window.get_dashboard_code_net(), "+180");
    assert_eq!(
        window.get_dashboard_code_efficiency(),
        "$0.005000 / 100 lines"
    );

    let trend = window.get_dashboard_trend_points();
    assert_eq!(trend.row_count(), 1);
    assert_eq!(trend.row_data(0).expect("trend").tokens_label, "140");
    let sessions = window.get_dashboard_session_rows();
    assert_eq!(sessions.row_count(), 1);
    assert_eq!(sessions.row_data(0).expect("session").tokens_label, "140");
    let activity = window.get_dashboard_activity_rows();
    assert_eq!(activity.row_count(), 8);
    assert_eq!(activity.row_data(0).expect("read").count_label, "1");
    assert_eq!(activity.row_data(7).expect("terminal").count_label, "8");
    let models = window.get_dashboard_model_rows();
    assert_eq!(models.row_count(), 1);
    assert_eq!(models.row_data(0).expect("model").label, "gpt-5.6");

    window.window().set_size(slint::PhysicalSize::new(760, 720));
    assert_eq!(window.get_dashboard_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert_eq!(window.get_dashboard_layout_mode(), "wide");

    let component_address = window as *const _;
    window.invoke_select_route(SharedString::from("settings"));
    assert!(!window.get_dashboard_visible());
    window.invoke_select_route(SharedString::from("dashboard"));
    assert!(window.get_dashboard_visible());
    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), 1);

    drop(shell);
    let scale_directory = tempfile::TempDir::new().expect("scale directory");
    let scale_path = scale_directory.path().join("ui-dashboard-32-quota.sqlite3");
    let scale_reducer = ready_reducer(&scale_path, 31);
    let scale_snapshot = scale_reducer.snapshot();
    let scale_shell = DesktopShell::new(&scale_snapshot).expect("scale desktop shell");
    let scale_quotas = scale_shell.window().get_dashboard_quota_rows();
    assert_eq!(scale_quotas.row_count(), 32);
    assert!(scale_quotas.iter().all(|quota| quota.ratio_known));
    let label_keys = scale_quotas
        .iter()
        .map(|quota| quota.label_key.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(label_keys.len(), 32);
}
