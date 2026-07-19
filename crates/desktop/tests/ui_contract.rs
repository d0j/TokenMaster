mod support;

use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{
    ComponentHandle, Model, SharedString,
    platform::{Key, PointerEventButton, WindowEvent},
};
use tokenmaster_desktop::{
    DesktopApplyOutcome, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopReliableStateProjection, DesktopSessionDetailIntent,
    DesktopSessionDetailIntentAdmission, DesktopSessionDetailIntentSink, DesktopShell,
    DesktopSnapshotEpoch,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{
    BenefitOverviewRequest, GitOutputRequest, LatestActivityRequest, PageSize, QueryErrorCode,
    QueryService, UsageAnalyticsRequest, UsageBreakdownKind, UsageRange, UsageSeriesSelection,
    UsageSessionPageRequest, UsageTimeZone, WeekStart,
};

use support::dashboard_fixture::{
    FixedClock, add_distinct_usage_rows, add_quota_windows, clear_usage_rows,
    make_partial_model_usage, range, seed, set_usage_project_alias,
};

struct RejectingIntentSink;

impl DesktopIntentSink for RejectingIntentSink {
    fn submit(&self, _intent: DesktopIntent) -> DesktopIntentAdmission {
        DesktopIntentAdmission::Rejected
    }
}

#[derive(Default)]
struct RecordingSessionDetailSink {
    intents: RefCell<Vec<DesktopSessionDetailIntent>>,
}

impl DesktopSessionDetailIntentSink for RecordingSessionDetailSink {
    fn submit(&self, intent: DesktopSessionDetailIntent) -> DesktopSessionDetailIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopSessionDetailIntentAdmission::Accepted
    }
}

fn ready_reducer(path: &std::path::Path, additional_quota_windows: u8) -> ProductReducer {
    ready_reducer_with_usage(path, additional_quota_windows, 0)
}

fn ready_reducer_with_usage(
    path: &std::path::Path,
    additional_quota_windows: u8,
    additional_usage_rows: u16,
) -> ProductReducer {
    seed(path);
    add_quota_windows(path, additional_quota_windows);
    add_distinct_usage_rows(path, additional_usage_rows);
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
    let history = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::recent_days(30).expect("recent history range"),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![UsageBreakdownKind::Model, UsageBreakdownKind::Project],
            )
            .expect("history request"),
        )
        .expect("history");
    let quota = service.quota_overview().expect("quota overview");
    let benefits = service
        .benefit_overview(BenefitOverviewRequest::new())
        .expect("benefits");
    let git = service
        .git_output(
            GitOutputRequest::new(range(), WeekStart::Monday, Vec::new(), 32).expect("Git request"),
        )
        .expect("Git output");
    let activity = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(12).expect("activity page size"),
        ))
        .expect("activity");
    let sessions = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(64).expect("page size"), Vec::new())
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
        .publish_history(attempt, history)
        .expect("publish history");
    reducer
        .publish_quota(attempt, quota)
        .expect("publish quota");
    reducer
        .publish_benefit(attempt, benefits)
        .expect("publish benefits");
    reducer.publish_git(attempt, git).expect("publish Git");
    reducer
        .publish_activity(attempt, activity)
        .expect("publish activity");
    reducer
        .publish_sessions(attempt, sessions)
        .expect("publish sessions");
    reducer
}

fn partial_models_reducer(path: &std::path::Path) -> ProductReducer {
    seed(path);
    make_partial_model_usage(path);
    let mut service = QueryService::open(path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let history = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::recent_days(30).expect("recent history range"),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![UsageBreakdownKind::Model],
            )
            .expect("history request"),
        )
        .expect("history");
    let attempt = ProductAttemptGeneration::new(1).expect("attempt");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt, status)
        .expect("publish status");
    reducer
        .publish_history(attempt, history)
        .expect("publish history");
    reducer
}

fn projects_without_git_reducer(path: &std::path::Path) -> ProductReducer {
    seed(path);
    let mut service = QueryService::open(path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let history = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::recent_days(30).expect("recent history range"),
                UsageTimeZone::iana("Asia/Jerusalem").expect("fixture timezone"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![UsageBreakdownKind::Project],
            )
            .expect("history request"),
        )
        .expect("history");
    let attempt = ProductAttemptGeneration::new(1).expect("attempt");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt, status)
        .expect("publish status");
    reducer
        .publish_history(attempt, history)
        .expect("publish history");
    reducer
        .fail_git(attempt, QueryErrorCode::DeadlineExceeded)
        .expect("fail Git");
    reducer
}

fn projects_without_linked_repository_reducer(path: &std::path::Path) -> ProductReducer {
    seed(path);
    set_usage_project_alias(path, "usage-only-project");
    let mut service = QueryService::open(path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let history = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::recent_days(30).expect("recent history range"),
                UsageTimeZone::iana("Asia/Jerusalem").expect("fixture timezone"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![UsageBreakdownKind::Project],
            )
            .expect("history request"),
        )
        .expect("history");
    let git = service
        .git_output(
            GitOutputRequest::new(range(), WeekStart::Monday, Vec::new(), 32).expect("Git request"),
        )
        .expect("Git output");
    let attempt = ProductAttemptGeneration::new(1).expect("attempt");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt, status)
        .expect("publish status");
    reducer
        .publish_history(attempt, history)
        .expect("publish history");
    reducer.publish_git(attempt, git).expect("publish Git");
    reducer
}

#[test]
fn compiled_shell_renders_exact_route_model_and_switches_in_place() {
    i_slint_backend_testing::init_no_event_loop();

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
    assert!(!window.get_history_visible());
    assert!(!window.get_sessions_visible());
    assert!(!window.get_models_visible());
    assert!(!window.get_projects_visible());
    assert!(!window.get_activity_visible());
    assert!(!window.get_help_about_visible());
    assert_eq!(window.get_history_day_rows().row_count(), 0);
    assert_eq!(window.get_history_total_tokens(), "—");
    assert_eq!(window.get_session_list_rows().row_count(), 0);
    assert_eq!(window.get_sessions_loaded_label(), "Unavailable");
    assert_eq!(window.get_model_usage_rows().row_count(), 0);
    assert_eq!(window.get_models_total_tokens(), "—");
    assert_eq!(window.get_models_total_availability(), "unavailable");
    assert_eq!(window.get_models_cost_availability(), "unavailable");
    assert_eq!(window.get_models_cost_evidence_label(), "Unavailable");
    assert_eq!(window.get_models_loaded_label(), "Unavailable");
    assert_eq!(window.get_projects_total_tokens(), "—");
    assert_eq!(window.get_projects_usage_range_label(), "Range unavailable");
    assert_eq!(window.get_projects_code_range_label(), "Range unavailable");
    assert_eq!(window.get_project_usage_rows().row_count(), 0);
    assert_eq!(window.get_recent_activity_rows().row_count(), 0);
    assert!(!window.get_activity_page_available());
    assert_eq!(window.get_activity_loaded_label(), "Unavailable");
    assert_eq!(
        window.get_activity_page_status_label(),
        "Page status unavailable"
    );
    assert_eq!(window.get_reminder_scope_rows().row_count(), 0);
    assert_eq!(window.get_benefit_lot_rows().row_count(), 0);
    assert_eq!(window.get_notifications_loaded_label(), "Waiting");
    assert_eq!(
        window.get_notifications_completeness_label(),
        "Waiting for benefit inventory"
    );
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
    assert_compiled_sessions_render_one_bounded_page_without_recreating_the_window();
    assert_compiled_models_render_complete_bounded_mix_without_recreating_the_window();
    assert_compiled_models_render_partial_cost_evidence();
    assert_compiled_projects_keep_recent_usage_and_today_code_separate_in_place();
    assert_compiled_activity_renders_bounded_safe_events_in_place();
    assert_compiled_notifications_render_expiry_truth_in_place();
    assert_compiled_help_about_is_static_truthful_and_responsive();
    assert_compiled_session_selection_is_immediate_correlated_and_bounded_in_place();
}

fn assert_compiled_help_about_is_static_truthful_and_responsive() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();
    let component_address = window as *const _;

    window.invoke_select_route(SharedString::from("help_about"));
    assert!(window.get_help_about_visible());
    assert!(!window.get_dashboard_visible());
    assert!(!window.get_settings_visible());
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(window.get_help_product_version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(window.get_help_about_section_count(), 6);
    window.show().expect("show Help/About window");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert_eq!(window.get_help_about_layout_mode(), "wide");

    window.window().set_size(slint::PhysicalSize::new(700, 720));
    assert_eq!(window.get_help_about_layout_mode(), "narrow");
    window.window().set_size(slint::PhysicalSize::new(900, 720));
    assert_eq!(window.get_help_about_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 1200));
    assert_eq!(window.get_help_about_layout_mode(), "wide");

    let labels = ElementQuery::from_root(window)
        .match_predicate(|element| element.accessible_label().is_some())
        .find_all()
        .into_iter()
        .filter_map(|element| element.accessible_label())
        .collect::<Vec<_>>();
    let region_labels = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Region)
        .find_all()
        .into_iter()
        .filter_map(|element| element.accessible_label())
        .collect::<Vec<_>>();
    for required in [
        "Start here",
        "Data sources and truth",
        "Privacy by design",
        "Health and recovery",
        "Automation status",
        "About and licenses",
    ] {
        assert_eq!(
            region_labels
                .iter()
                .filter(|label| label.contains(required))
                .count(),
            1,
            "accessible Help/About section must appear exactly once: {required}"
        );
    }
    assert!(
        region_labels
            .iter()
            .any(|label| label.contains("No prompts, responses, reasoning, commands")),
        "privacy boundary must be accessible"
    );
    assert!(
        region_labels
            .iter()
            .any(|label| label.contains("CLI and stdio MCP are not available")),
        "automation availability must be truthful"
    );
    assert_eq!(
        labels
            .iter()
            .filter(|label| label.as_str() == "#MadeWithSlint")
            .count(),
        1
    );

    window.invoke_select_route(SharedString::from("dashboard"));
    assert!(!window.get_help_about_visible());
    window.invoke_select_route(SharedString::from("help_about"));
    assert!(window.get_help_about_visible());
    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_help_product_version(), env!("CARGO_PKG_VERSION"));
}

fn assert_compiled_notifications_render_expiry_truth_in_place() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-notifications.sqlite3");
    let reducer = ready_reducer(&path, 0);
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();
    let component_address = window as *const _;

    window.invoke_select_route(SharedString::from("notifications"));
    assert!(window.get_notifications_visible());
    assert!(!window.get_dashboard_visible());
    assert!(!window.get_activity_visible());
    assert_eq!(window.get_notifications_state(), "degraded");
    assert_eq!(
        window.get_notifications_evidence_label(),
        "Fresh · Authoritative"
    );
    assert_eq!(
        window.get_notifications_loaded_label(),
        "1 reminder profile · 4 current benefits"
    );
    assert_eq!(
        window.get_notifications_completeness_label(),
        "Current inventory · warnings present"
    );

    let scopes = window.get_reminder_scope_rows();
    assert_eq!(scopes.row_count(), 1);
    let scope = scopes.row_data(0).expect("reminder scope");
    assert_eq!(scope.scope_label, "Scope 1");
    assert_eq!(scope.lot_count_label, "4 benefits");
    assert_eq!(scope.coverage_label, "In-app only");
    assert_eq!(scope.source_label, "Inherited");
    assert_eq!(scope.leads_label, "7d · 24h · 12h · 6h · 1h");
    assert_eq!(scope.completeness_label, "Complete");
    assert!(scope.nearest_expiry_label.contains("UTC"));
    assert_eq!(scope.evidence_label, "Fresh · Authoritative");
    assert!(scope.warning_label.contains("Unknown expiry"));

    let lots = window.get_benefit_lot_rows();
    assert_eq!(lots.row_count(), 4);
    let expired = lots.row_data(0).expect("expired lot");
    assert_eq!(expired.scope_label, "Scope 1");
    assert_eq!(expired.kind_label, "Banked rate limit reset");
    assert_eq!(expired.quantity_label, "7");
    assert_eq!(expired.state_label, "Expired");
    assert_eq!(expired.expiry_label, "Expired 2026-07-16 11:59:59.999 UTC");
    assert_eq!(
        expired.evidence_label,
        "Provider official · High · Provider detail"
    );
    let unknown = lots.row_data(2).expect("unknown-expiry lot");
    assert_eq!(unknown.kind_label, "Usage credit");
    assert_eq!(unknown.expiry_label, "Expiry unknown");

    window.window().set_size(slint::PhysicalSize::new(700, 720));
    assert_eq!(window.get_notifications_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert_eq!(window.get_notifications_layout_mode(), "wide");

    window.show().expect("show notifications window");
    let accessible_rows = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::ListItem)
        .match_predicate(|element| {
            element.accessible_label().is_some_and(|label| {
                label.contains("Scope 1")
                    && label.contains("Banked rate limit reset")
                    && label.contains("quantity 7")
                    && label.contains("Expired")
                    && label.contains("Provider official")
            })
        })
        .find_all();
    assert_eq!(accessible_rows.len(), 1);

    window.invoke_select_route(SharedString::from("dashboard"));
    assert!(!window.get_notifications_visible());
    window.invoke_select_route(SharedString::from("notifications"));
    assert!(window.get_notifications_visible());
    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_reminder_scope_rows().row_count(), 1);
    assert_eq!(window.get_benefit_lot_rows().row_count(), 4);
}

fn assert_compiled_activity_renders_bounded_safe_events_in_place() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-activity.sqlite3");
    let reducer = ready_reducer(&path, 0);
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();
    let component_address = window as *const _;

    window.invoke_select_route(SharedString::from("activity"));
    assert!(window.get_activity_visible());
    assert!(!window.get_dashboard_visible());
    assert!(!window.get_history_visible());
    assert!(!window.get_sessions_visible());
    assert!(!window.get_models_visible());
    assert!(!window.get_projects_visible());
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(window.get_activity_state(), "ready");
    assert_eq!(window.get_activity_context_label(), "UTC timestamps");
    assert_eq!(
        window.get_activity_evidence_label(),
        "Fresh · Authoritative"
    );
    assert_eq!(window.get_activity_loaded_label(), "1 event loaded");
    assert!(window.get_activity_page_available());
    assert_eq!(
        window.get_activity_page_status_label(),
        "First page complete"
    );

    let rows = window.get_recent_activity_rows();
    assert_eq!(rows.row_count(), 1);
    let row = rows.row_data(0).expect("recent activity row");
    assert_eq!(row.time_label, "2026-07-16 01:00:00 UTC");
    assert_eq!(row.model_label, "gpt-5.6");
    assert_eq!(row.input_label, "100");
    assert_eq!(row.cached_label, "20");
    assert_eq!(row.output_label, "30");
    assert_eq!(row.reasoning_label, "10");
    assert_eq!(row.total_label, "140");

    window.window().set_size(slint::PhysicalSize::new(700, 720));
    assert_eq!(window.get_activity_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert_eq!(window.get_activity_layout_mode(), "wide");

    window.show().expect("show activity window");
    let accessible_rows = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::ListItem)
        .match_predicate(|element| {
            element.accessible_label().is_some_and(|label| {
                label.contains("2026-07-16 01:00:00 UTC")
                    && label.contains("model gpt-5.6")
                    && label.contains("input 100")
                    && label.contains("cached 20")
                    && label.contains("output 30")
                    && label.contains("reasoning 10")
                    && label.contains("total 140")
            })
        })
        .find_all();
    assert_eq!(accessible_rows.len(), 1);

    window.invoke_select_route(SharedString::from("dashboard"));
    assert!(!window.get_activity_visible());
    window.invoke_select_route(SharedString::from("activity"));
    assert!(window.get_activity_visible());
    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_recent_activity_rows().row_count(), 1);

    drop(shell);
    let scale_directory = tempfile::TempDir::new().expect("scale directory");
    let scale_path = scale_directory.path().join("ui-activity-scale.sqlite3");
    let scale_reducer = ready_reducer_with_usage(&scale_path, 0, 64);
    let scale_shell = DesktopShell::new(&scale_reducer.snapshot()).expect("scale shell");
    let scale_window = scale_shell.window();
    scale_window.invoke_select_route(SharedString::from("activity"));
    assert_eq!(scale_window.get_recent_activity_rows().row_count(), 12);
    assert_eq!(scale_window.get_activity_loaded_label(), "12 events loaded");
    assert_eq!(
        scale_window.get_activity_page_status_label(),
        "More activity available"
    );

    drop(scale_shell);
    let retained_directory = tempfile::TempDir::new().expect("retained directory");
    let retained_path = retained_directory
        .path()
        .join("ui-activity-retained-empty.sqlite3");
    seed(&retained_path);
    clear_usage_rows(&retained_path);
    let mut service = QueryService::open(&retained_path, FixedClock).expect("query service");
    let page = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(12).expect("activity page size"),
        ))
        .expect("empty activity");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_activity(ProductAttemptGeneration::new(1).expect("attempt"), page)
        .expect("publish empty activity");
    reducer
        .fail_activity(
            ProductAttemptGeneration::new(2).expect("attempt"),
            QueryErrorCode::DeadlineExceeded,
        )
        .expect("retain empty activity");
    let retained_shell = DesktopShell::new(&reducer.snapshot()).expect("retained shell");
    let retained_window = retained_shell.window();
    retained_window.invoke_select_route(SharedString::from("activity"));
    assert_eq!(retained_window.get_activity_state(), "degraded");
    assert!(retained_window.get_activity_page_available());
    assert_eq!(
        retained_window.get_activity_loaded_label(),
        "0 events loaded"
    );
    assert_eq!(
        retained_window.get_activity_page_status_label(),
        "First page complete"
    );
    retained_window
        .show()
        .expect("show retained activity window");
    let retained_empty_table = ElementQuery::from_root(retained_window)
        .match_accessible_role(AccessibleRole::Table)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label.contains("No activity events in the available page"))
        })
        .find_all();
    assert_eq!(retained_empty_table.len(), 1);
}

fn assert_compiled_projects_keep_recent_usage_and_today_code_separate_in_place() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-projects.sqlite3");
    let reducer = ready_reducer(&path, 0);
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();
    let component_address = window as *const _;

    window.invoke_select_route(SharedString::from("projects"));
    assert!(window.get_projects_visible());
    assert!(!window.get_dashboard_visible());
    assert!(!window.get_history_visible());
    assert!(!window.get_sessions_visible());
    assert!(!window.get_models_visible());
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(window.get_projects_state(), "ready");
    assert_eq!(
        window.get_projects_usage_range_label(),
        "2026-06-17 – before 2026-07-17"
    );
    assert_eq!(window.get_projects_usage_time_zone_label(), "UTC");
    assert_eq!(
        window.get_projects_usage_evidence_label(),
        "Fresh · Authoritative"
    );
    assert_eq!(
        window.get_projects_code_range_label(),
        "2026-07-16 – before 2026-07-17"
    );
    assert_eq!(window.get_projects_code_time_zone_label(), "UTC");
    assert_eq!(
        window.get_projects_code_evidence_label(),
        "Fresh · Authoritative"
    );
    assert_eq!(window.get_projects_total_tokens(), "140");
    assert_eq!(window.get_projects_total_availability(), "known");
    assert_eq!(window.get_projects_cost(), "$0.010000");
    assert_eq!(window.get_projects_cost_availability(), "complete");
    assert_eq!(
        window.get_projects_cost_evidence_label(),
        "Complete · reported"
    );
    assert_eq!(window.get_projects_events(), "1 event");
    assert_eq!(window.get_projects_loaded_label(), "1 project loaded");
    assert_eq!(window.get_projects_completeness_label(), "Complete range");
    assert_eq!(
        window.get_projects_code_coverage_label(),
        "1 repository loaded"
    );
    assert_eq!(
        window.get_projects_code_completeness_label(),
        "Complete code range"
    );

    let rows = window.get_project_usage_rows();
    assert_eq!(rows.row_count(), 1);
    let row = rows.row_data(0).expect("project row");
    assert_eq!(row.project_label, "tokenmaster");
    assert!(!row.unassociated);
    assert_eq!(row.event_label, "1");
    assert_eq!(row.input_label, "100");
    assert_eq!(row.cached_label, "20");
    assert_eq!(row.output_label, "30");
    assert_eq!(row.reasoning_label, "10");
    assert_eq!(row.total_label, "140");
    assert_eq!(row.cost_label, "$0.010000");
    assert_eq!(row.cost_evidence_label, "Complete · reported");
    assert_eq!(row.token_ratio, 1.0);
    assert!(row.code_available);
    assert!(row.code_complete);
    assert_eq!(row.repository_label, "1 repository");
    assert_eq!(row.commits_label, "1");
    assert_eq!(row.added_label, "+200");
    assert_eq!(row.removed_label, "-20");
    assert_eq!(row.net_label, "+180");
    assert_eq!(
        row.efficiency_label,
        "$0.005000 / 100 added product-code lines"
    );
    assert_eq!(row.code_status_label, "Complete code");
    assert_eq!(row.code_evidence_label, "Fresh · Authoritative");

    window.window().set_size(slint::PhysicalSize::new(700, 720));
    assert_eq!(window.get_projects_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert_eq!(window.get_projects_layout_mode(), "wide");

    window.show().expect("show projects window");
    let accessible_rows = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::ListItem)
        .match_predicate(|element| {
            element.accessible_label().is_some_and(|label| {
                label.contains("Recent usage tokenmaster")
                    && label.contains("Today code Complete code 1 repository")
                    && label.contains("+200")
                    && label.contains("-20")
            })
        })
        .find_all();
    assert_eq!(accessible_rows.len(), 1);

    window.invoke_select_route(SharedString::from("dashboard"));
    assert!(!window.get_projects_visible());
    window.invoke_select_route(SharedString::from("projects"));
    assert!(window.get_projects_visible());
    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_project_usage_rows().row_count(), 1);

    drop(shell);
    let unavailable_directory = tempfile::TempDir::new().expect("unavailable directory");
    let unavailable_path = unavailable_directory
        .path()
        .join("ui-projects-git-unavailable.sqlite3");
    let unavailable_reducer = projects_without_git_reducer(&unavailable_path);
    let unavailable_shell =
        DesktopShell::new(&unavailable_reducer.snapshot()).expect("unavailable shell");
    let unavailable_window = unavailable_shell.window();
    unavailable_window.invoke_select_route(SharedString::from("projects"));
    let unavailable_row = unavailable_window
        .get_project_usage_rows()
        .row_data(0)
        .expect("usage row without Git");
    assert_eq!(unavailable_row.repository_label, "Git unavailable");
    assert_eq!(unavailable_row.code_status_label, "Git unavailable");
    assert_eq!(unavailable_row.commits_label, "—");
    unavailable_window
        .show()
        .expect("show unavailable projects");
    let unavailable_accessible_rows = ElementQuery::from_root(unavailable_window)
        .match_accessible_role(AccessibleRole::ListItem)
        .match_predicate(|element| {
            element.accessible_label().is_some_and(|label| {
                label.contains("Today code Git unavailable") && !label.contains("0 repositories")
            })
        })
        .find_all();
    assert_eq!(unavailable_accessible_rows.len(), 1);

    drop(unavailable_shell);
    let not_linked_directory = tempfile::TempDir::new().expect("not-linked directory");
    let not_linked_path = not_linked_directory
        .path()
        .join("ui-projects-not-linked.sqlite3");
    let not_linked_reducer = projects_without_linked_repository_reducer(&not_linked_path);
    let not_linked_shell =
        DesktopShell::new(&not_linked_reducer.snapshot()).expect("not-linked shell");
    let not_linked_window = not_linked_shell.window();
    not_linked_window.invoke_select_route(SharedString::from("projects"));
    let not_linked_row = not_linked_window
        .get_project_usage_rows()
        .row_data(0)
        .expect("usage row without linked repository");
    assert_eq!(not_linked_row.project_label, "usage-only-project");
    assert_eq!(not_linked_row.repository_label, "Not linked");
    assert_eq!(not_linked_row.code_status_label, "Not linked");
    assert_eq!(not_linked_row.commits_label, "—");
    not_linked_window.show().expect("show not-linked projects");
    for (width, layout) in [(700, "narrow"), (1120, "wide")] {
        not_linked_window
            .window()
            .set_size(slint::PhysicalSize::new(width, 720));
        assert_eq!(not_linked_window.get_projects_layout_mode(), layout);
        let rows = ElementQuery::from_root(not_linked_window)
            .match_accessible_role(AccessibleRole::ListItem)
            .match_predicate(|element| {
                element.accessible_label().is_some_and(|label| {
                    label.contains("Today code Not linked") && !label.contains("0 repositories")
                })
            })
            .find_all();
        assert_eq!(rows.len(), 1);
    }
}

fn assert_compiled_session_selection_is_immediate_correlated_and_bounded_in_place() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-session-detail.sqlite3");
    let mut reducer = ready_reducer(&path, 0);
    let snapshot = reducer.snapshot();
    let sink = Rc::new(RecordingSessionDetailSink::default());
    let shell = DesktopShell::new_with_reliable_state_and_session_sink(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        sink.clone(),
    )
    .expect("desktop shell");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    assert_eq!(
        shell
            .apply_snapshot_for_epoch(epoch, &snapshot)
            .expect("bind backend epoch"),
        DesktopApplyOutcome::Accepted
    );
    let window = shell.window();

    window.invoke_select_route(SharedString::from("sessions"));
    window.show().expect("show sessions window");
    let session_rows = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label.starts_with("From "))
        })
        .find_all();
    assert_eq!(session_rows.len(), 1);
    session_rows[0].mock_single_click(PointerEventButton::Left);
    assert_eq!(window.get_sessions_selected_row(), 0);
    assert_eq!(window.get_session_detail_state(), "loading");
    assert_eq!(window.get_session_detail_breakdown_rows().row_count(), 0);
    let intent = sink
        .intents
        .borrow()
        .first()
        .copied()
        .expect("one identity-free selection intent");
    assert_eq!(intent.snapshot_epoch(), epoch);
    assert_eq!(intent.product_generation(), snapshot.generation());
    assert_eq!(intent.selection().row_ordinal(), 0);
    dispatch_key(window, Key::Return);
    dispatch_key(window, Key::Space);
    assert_eq!(
        sink.intents.borrow().len(),
        3,
        "pointer, Enter, and Space each traverse the real session-row bindings"
    );
    let latest_intent = sink
        .intents
        .borrow()
        .last()
        .copied()
        .expect("latest keyboard selection intent");

    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let sessions = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(1).expect("page size"), Vec::new())
                .expect("session request"),
        )
        .expect("sessions");
    let detail = service
        .usage_session_detail(sessions.payload().sessions()[0].key().clone())
        .expect("session detail");
    reducer
        .publish_session_detail(
            ProductAttemptGeneration::new(2).expect("attempt"),
            latest_intent.selection(),
            detail,
        )
        .expect("publish detail");
    assert_eq!(
        shell
            .apply_snapshot_for_epoch(epoch, &reducer.snapshot())
            .expect("apply exact detail"),
        DesktopApplyOutcome::Accepted
    );
    assert_eq!(window.get_session_detail_state(), "ready");
    assert_eq!(
        window.get_session_detail_evidence_label(),
        "Fresh · Authoritative"
    );
    assert_eq!(window.get_sessions_selected_row(), 0);
    assert_eq!(window.get_session_detail_event_label(), "1");
    assert_eq!(window.get_session_detail_breakdown_rows().row_count(), 2);

    let unavailable = DesktopShell::new(&snapshot).expect("unavailable shell");
    unavailable
        .apply_snapshot_for_epoch(epoch, &snapshot)
        .expect("bind unavailable shell epoch");
    unavailable.window().invoke_select_session(0);
    assert_eq!(
        unavailable.window().get_session_detail_state(),
        "unavailable"
    );
    assert_eq!(
        unavailable.window().get_session_detail_status_label(),
        "Unavailable · Request Rejected"
    );
}

fn dispatch_key(window: &tokenmaster_desktop::MainWindow, key: Key) {
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
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

    window.invoke_select_route(SharedString::from("history"));
    assert!(window.get_history_visible());
    assert!(!window.get_dashboard_visible());
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(window.get_history_state(), "ready");
    assert_eq!(window.get_history_range_label(), "2026-06-17 – 2026-07-16");
    assert_eq!(window.get_history_time_zone_label(), "UTC");
    assert_eq!(window.get_history_evidence_label(), "Fresh · Authoritative");
    assert_eq!(window.get_history_total_tokens(), "140");
    assert_eq!(window.get_history_cost(), "$0.010000");
    assert_eq!(window.get_history_events(), "1 event");
    let history = window.get_history_day_rows();
    assert_eq!(history.row_count(), 30);
    let newest = history.row_data(0).expect("newest history day");
    assert_eq!(newest.date_label, "2026-07-16");
    assert_eq!(newest.total_label, "140");
    assert_eq!(newest.cost_label, "$0.010000");

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

fn assert_compiled_models_render_complete_bounded_mix_without_recreating_the_window() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-models.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 64);
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();
    let component_address = window as *const _;

    window.invoke_select_route(SharedString::from("models"));
    assert!(window.get_models_visible());
    assert!(!window.get_dashboard_visible());
    assert!(!window.get_history_visible());
    assert!(!window.get_sessions_visible());
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(window.get_models_state(), "degraded");
    assert_eq!(
        window.get_models_range_label(),
        "2026-06-17 – before 2026-07-17"
    );
    assert_eq!(window.get_models_time_zone_label(), "UTC");
    assert_eq!(window.get_models_evidence_label(), "Fresh · Authoritative");
    assert_eq!(window.get_models_total_tokens(), "268");
    assert_eq!(window.get_models_total_availability(), "known");
    assert_eq!(window.get_models_cost(), "$0.010064");
    assert_eq!(window.get_models_cost_availability(), "complete");
    assert_eq!(
        window.get_models_cost_evidence_label(),
        "Complete · reported"
    );
    assert_eq!(window.get_models_events(), "65 events");
    assert_eq!(window.get_models_loaded_label(), "64 models loaded");
    assert_eq!(
        window.get_models_completeness_label(),
        "More models available"
    );

    let rows = window.get_model_usage_rows();
    assert_eq!(rows.row_count(), 64);
    let primary = rows.row_data(0).expect("primary model");
    assert_eq!(primary.model_label, "gpt-5.6");
    assert_eq!(primary.event_label, "1");
    assert_eq!(primary.input_label, "100");
    assert_eq!(primary.cached_label, "20");
    assert_eq!(primary.output_label, "30");
    assert_eq!(primary.reasoning_label, "10");
    assert_eq!(primary.total_label, "140");
    assert_eq!(primary.cost_label, "$0.010000");
    assert_eq!(primary.cost_availability, "complete");
    assert_eq!(primary.cost_evidence_label, "Complete · reported");
    assert_eq!(primary.token_ratio, 1.0);

    window.window().set_size(slint::PhysicalSize::new(700, 720));
    assert_eq!(window.get_models_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert_eq!(window.get_models_layout_mode(), "wide");

    window.invoke_select_route(SharedString::from("dashboard"));
    assert!(!window.get_models_visible());
    window.invoke_select_route(SharedString::from("models"));
    assert!(window.get_models_visible());
    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_model_usage_rows().row_count(), 64);
}

fn assert_compiled_models_render_partial_cost_evidence() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-models-partial.sqlite3");
    let reducer = partial_models_reducer(&path);
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();

    window.invoke_select_route(SharedString::from("models"));
    assert_eq!(window.get_models_state(), "ready");
    assert_eq!(window.get_models_total_tokens(), "205");
    assert_eq!(window.get_models_total_availability(), "known");
    assert_eq!(window.get_models_cost(), "$0.010000");
    assert_eq!(window.get_models_cost_availability(), "partial");
    assert_eq!(
        window.get_models_cost_evidence_label(),
        "Partial · reported"
    );

    let rows = window.get_model_usage_rows();
    assert_eq!(rows.row_count(), 1);
    let row = rows.row_data(0).expect("partial model");
    assert_eq!(row.model_label, "fixture-unpriced-model");
    assert_eq!(row.input_availability, "partial");
    assert_eq!(row.input_label, "50 (1/2)");
    assert_eq!(row.cost_availability, "partial");
    assert_eq!(row.cost_label, "$0.010000");
    assert_eq!(row.cost_evidence_label, "Partial · reported");

    window.show().expect("show partial models window");
    let accessible_rows = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::ListItem)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label.contains("cost $0.010000 Partial · reported"))
        })
        .find_all();
    assert_eq!(accessible_rows.len(), 1);
}

fn assert_compiled_sessions_render_one_bounded_page_without_recreating_the_window() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-sessions.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 64);
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();
    let component_address = window as *const _;

    window.invoke_select_route(SharedString::from("sessions"));
    assert!(window.get_sessions_visible());
    assert!(!window.get_dashboard_visible());
    assert!(!window.get_history_visible());
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(window.get_sessions_state(), "ready");
    assert_eq!(window.get_sessions_loaded_label(), "64 loaded");
    assert_eq!(
        window.get_sessions_page_status_label(),
        "More sessions available"
    );
    assert_eq!(
        window.get_sessions_evidence_label(),
        "Fresh · Authoritative"
    );
    let rows = window.get_session_list_rows();
    assert_eq!(rows.row_count(), 64);
    let newest = rows.row_data(0).expect("newest session");
    assert_eq!(newest.last_label, "2026-07-16 01:07:43 UTC");
    assert_eq!(newest.event_label, "1");
    assert_eq!(newest.input_label, "1");
    assert_eq!(newest.total_label, "2");
    assert_eq!(newest.cost_label, "$0.000001");

    window.window().set_size(slint::PhysicalSize::new(760, 720));
    assert_eq!(window.get_sessions_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert_eq!(window.get_sessions_layout_mode(), "wide");

    window.invoke_select_route(SharedString::from("dashboard"));
    assert!(!window.get_sessions_visible());
    window.invoke_select_route(SharedString::from("sessions"));
    assert!(window.get_sessions_visible());
    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_session_list_rows().row_count(), 64);
}
