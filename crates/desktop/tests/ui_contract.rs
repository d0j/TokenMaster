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
    BenefitOverviewRequest, GitOutputRequest, PageSize, QueryErrorCode, QueryService,
    UsageAnalyticsRequest, UsageBreakdownKind, UsageRange, UsageSeriesSelection,
    UsageSessionPageRequest, UsageTimeZone, WeekStart,
};

use support::dashboard_fixture::{
    FixedClock, add_distinct_usage_rows, add_quota_windows, range, seed,
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
    additional_usage_rows: u8,
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
                Vec::new(),
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
        .publish_sessions(attempt, sessions)
        .expect("publish sessions");
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
    assert_eq!(window.get_history_day_rows().row_count(), 0);
    assert_eq!(window.get_history_total_tokens(), "—");
    assert_eq!(window.get_session_list_rows().row_count(), 0);
    assert_eq!(window.get_sessions_loaded_label(), "Unavailable");
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
    assert_compiled_session_selection_is_immediate_correlated_and_bounded_in_place();
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
