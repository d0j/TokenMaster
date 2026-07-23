mod support;

use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{
    ComponentHandle, Model, ModelRc, SharedString, VecModel,
    platform::{Key, PointerEventButton, WindowEvent},
};
use tokenmaster_desktop::{
    DesktopApplyOutcome, DesktopHistoryRangeIntent, DesktopHistoryRangeIntentAdmission,
    DesktopHistoryRangeIntentSink, DesktopHistoryRangePreset, DesktopIntent,
    DesktopIntentAdmission, DesktopIntentSink, DesktopReliableStateProjection,
    DesktopSessionDetailIntent, DesktopSessionDetailIntentAdmission,
    DesktopSessionDetailIntentSink, DesktopSessionPageDirection, DesktopSessionPageIntent,
    DesktopSessionPageIntentAdmission, DesktopSessionPageIntentSink, DesktopShell,
    DesktopSnapshotEpoch, MAX_HISTORY_DAYS, MAX_MODEL_ROWS, MAX_PROJECT_ROWS,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer, ProductSnapshot};
use tokenmaster_query::{
    BenefitOverviewRequest, GitOutputRequest, LatestActivityRequest, PageSize, QueryErrorCode,
    QueryService, UsageAnalyticsRequest, UsageBreakdownKind, UsageRange, UsageRhythmSelection,
    UsageSeriesSelection, UsageSessionPageRequest, UsageTimeZone, WeekStart,
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

struct AcceptingIntentSink;

impl DesktopIntentSink for AcceptingIntentSink {
    fn submit(&self, _intent: DesktopIntent) -> DesktopIntentAdmission {
        DesktopIntentAdmission::Started
    }
}

#[derive(Default)]
struct RecordingHistoryRangeSink {
    intents: RefCell<Vec<DesktopHistoryRangeIntent>>,
}

impl DesktopHistoryRangeIntentSink for RecordingHistoryRangeSink {
    fn submit(&self, intent: DesktopHistoryRangeIntent) -> DesktopHistoryRangeIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopHistoryRangeIntentAdmission::Accepted
    }
}

#[derive(Default)]
struct RejectingHistoryRangeSink {
    intents: RefCell<Vec<DesktopHistoryRangeIntent>>,
}

impl DesktopHistoryRangeIntentSink for RejectingHistoryRangeSink {
    fn submit(&self, intent: DesktopHistoryRangeIntent) -> DesktopHistoryRangeIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopHistoryRangeIntentAdmission::Rejected
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

#[derive(Default)]
struct RecordingSessionPageSink {
    intents: RefCell<Vec<DesktopSessionPageIntent>>,
}

impl DesktopSessionPageIntentSink for RecordingSessionPageSink {
    fn submit(&self, intent: DesktopSessionPageIntent) -> DesktopSessionPageIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopSessionPageIntentAdmission::Accepted
    }
}

#[derive(Default)]
struct RejectingSessionPageSink {
    intents: RefCell<Vec<DesktopSessionPageIntent>>,
}

struct RetainedSessionPageSink {
    intents: Rc<RefCell<Vec<DesktopSessionPageIntent>>>,
}

impl DesktopSessionPageIntentSink for RetainedSessionPageSink {
    fn submit(&self, intent: DesktopSessionPageIntent) -> DesktopSessionPageIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopSessionPageIntentAdmission::Accepted
    }
}

impl DesktopSessionPageIntentSink for RejectingSessionPageSink {
    fn submit(&self, intent: DesktopSessionPageIntent) -> DesktopSessionPageIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopSessionPageIntentAdmission::Rejected
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
            .and_then(|request| request.with_rhythm(UsageRhythmSelection::HourAndWeekday))
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
    assert!(!window.get_compact_widget_visible());
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

    assert_compiled_command_palette_is_bounded_and_routes_through_desktop_state(window);

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
    assert_open_command_palette_refreshes_from_accepted_snapshot();
    assert_compiled_compact_widget_reuses_quota_snapshot_and_restores_window();
}

#[test]
fn populated_usage_projection_payloads_survive_hot_locale_switch() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let reducer = ready_reducer(&directory.path().join("locale-invariants.sqlite3"), 0);
    let shell = DesktopShell::new_with_reliable_state(
        &reducer.snapshot(),
        DesktopReliableStateProjection::unavailable(),
        Rc::new(AcceptingIntentSink),
    )
    .expect("desktop shell");
    let window = shell.window();
    let history_before = window.get_history_day_rows().iter().collect::<Vec<_>>();
    let models_before = window.get_model_usage_rows().iter().collect::<Vec<_>>();
    let projects_before = window.get_project_usage_rows().iter().collect::<Vec<_>>();
    let activity_before = window.get_recent_activity_rows().iter().collect::<Vec<_>>();
    let hour_before = window
        .get_activity_rhythm_hour_rows()
        .iter()
        .collect::<Vec<_>>();
    let weekday_before = window
        .get_activity_rhythm_weekday_rows()
        .iter()
        .collect::<Vec<_>>();
    let notification_scopes_before = window.get_reminder_scope_rows().iter().collect::<Vec<_>>();
    let notification_lots_before = window.get_benefit_lot_rows().iter().collect::<Vec<_>>();
    let activity_state = window.get_activity_state();
    let activity_reasons = window.get_activity_reasons();
    assert!(!history_before.is_empty());
    assert!(!models_before.is_empty());
    assert!(!projects_before.is_empty());

    window.invoke_select_presentation_locale(1);

    let history_after = window.get_history_day_rows().iter().collect::<Vec<_>>();
    let models_after = window.get_model_usage_rows().iter().collect::<Vec<_>>();
    let projects_after = window.get_project_usage_rows().iter().collect::<Vec<_>>();
    let activity_after = window.get_recent_activity_rows().iter().collect::<Vec<_>>();
    let hour_after = window
        .get_activity_rhythm_hour_rows()
        .iter()
        .collect::<Vec<_>>();
    let weekday_after = window
        .get_activity_rhythm_weekday_rows()
        .iter()
        .collect::<Vec<_>>();
    let notification_scopes_after = window.get_reminder_scope_rows().iter().collect::<Vec<_>>();
    let notification_lots_after = window.get_benefit_lot_rows().iter().collect::<Vec<_>>();
    assert_eq!(history_after.len(), history_before.len());
    assert_eq!(models_after.len(), models_before.len());
    assert_eq!(projects_after.len(), projects_before.len());
    for (before, after) in history_before.iter().zip(&history_after) {
        assert_eq!(after.date_label, before.date_label);
        assert_eq!(after.event_label, before.event_label);
        assert_eq!(after.input_availability, before.input_availability);
        assert_eq!(after.input_label, before.input_label);
        assert_eq!(after.cached_availability, before.cached_availability);
        assert_eq!(after.cached_label, before.cached_label);
        assert_eq!(after.output_availability, before.output_availability);
        assert_eq!(after.output_label, before.output_label);
        assert_eq!(after.reasoning_availability, before.reasoning_availability);
        assert_eq!(after.reasoning_label, before.reasoning_label);
        assert_eq!(after.total_availability, before.total_availability);
        assert_eq!(after.total_label, before.total_label);
        assert_eq!(after.cost_availability, before.cost_availability);
        assert_eq!(after.cost_label, before.cost_label);
    }
    for (before, after) in models_before.iter().zip(&models_after) {
        assert_eq!(after.model_label, before.model_label);
        assert_eq!(after.event_label, before.event_label);
        assert_eq!(after.input_availability, before.input_availability);
        assert_eq!(after.input_label, before.input_label);
        assert_eq!(after.cached_availability, before.cached_availability);
        assert_eq!(after.cached_label, before.cached_label);
        assert_eq!(after.output_availability, before.output_availability);
        assert_eq!(after.output_label, before.output_label);
        assert_eq!(after.reasoning_availability, before.reasoning_availability);
        assert_eq!(after.reasoning_label, before.reasoning_label);
        assert_eq!(after.total_availability, before.total_availability);
        assert_eq!(after.total_label, before.total_label);
        assert_eq!(after.cost_availability, before.cost_availability);
        assert_eq!(after.cost_label, before.cost_label);
    }
    for (before, after) in projects_before.iter().zip(&projects_after) {
        assert_eq!(after.project_label, before.project_label);
        assert_eq!(after.unassociated, before.unassociated);
        assert_eq!(after.event_label, before.event_label);
        assert_eq!(after.input_availability, before.input_availability);
        assert_eq!(after.input_label, before.input_label);
        assert_eq!(after.cached_availability, before.cached_availability);
        assert_eq!(after.cached_label, before.cached_label);
        assert_eq!(after.output_availability, before.output_availability);
        assert_eq!(after.output_label, before.output_label);
        assert_eq!(after.reasoning_availability, before.reasoning_availability);
        assert_eq!(after.reasoning_label, before.reasoning_label);
        assert_eq!(after.total_availability, before.total_availability);
        assert_eq!(after.total_label, before.total_label);
        assert_eq!(after.cost_availability, before.cost_availability);
        assert_eq!(after.cost_label, before.cost_label);
        assert_eq!(after.code_available, before.code_available);
        assert_eq!(after.code_complete, before.code_complete);
        assert_eq!(after.commits_label, before.commits_label);
        assert_eq!(after.added_label, before.added_label);
        assert_eq!(after.removed_label, before.removed_label);
        assert_eq!(after.net_label, before.net_label);
        assert_eq!(
            after.efficiency_label.split(" / ").next(),
            before.efficiency_label.split(" / ").next()
        );
    }
    assert_eq!(window.get_activity_state(), activity_state);
    assert_eq!(window.get_activity_reasons(), activity_reasons);
    assert_eq!(activity_after.len(), activity_before.len());
    assert_eq!(hour_after.len(), hour_before.len());
    assert_eq!(weekday_after.len(), weekday_before.len());
    assert_eq!(window.get_activity_context_label(), "Метки времени UTC");
    assert_eq!(window.get_activity_loaded_label(), "Загружено: 1");
    assert_eq!(
        window.get_activity_page_status_label(),
        "Первая страница полная"
    );
    assert_eq!(
        window.get_activity_evidence_label(),
        "Свежие · Авторитетные"
    );
    assert_eq!(
        window.get_activity_rhythm_evidence_label(),
        "Свежие · Авторитетные"
    );
    assert_eq!(weekday_after[0].label, "Понедельник");
    assert_eq!(weekday_after[0].events_label, "Событий: 0");
    assert_eq!(
        window.get_notifications_evidence_label(),
        "Свежие · Авторитетные"
    );
    assert_eq!(
        window.get_notifications_loaded_label(),
        "Профиль напоминаний: 1 · Текущих преимуществ: 4"
    );
    assert_eq!(
        window.get_notifications_completeness_label(),
        "Текущий инвентарь · есть предупреждения"
    );
    assert_eq!(
        notification_scopes_after.len(),
        notification_scopes_before.len()
    );
    assert_eq!(
        notification_lots_after.len(),
        notification_lots_before.len()
    );
    assert_eq!(notification_scopes_after[0].scope_label, "Область 1");
    assert_eq!(
        notification_scopes_after[0].lot_count_label,
        "Преимуществ: 4"
    );
    assert_eq!(
        notification_scopes_after[0].coverage_label,
        "Только в приложении"
    );
    assert_eq!(notification_scopes_after[0].source_label, "Унаследован");
    assert_eq!(notification_scopes_after[0].completeness_label, "Полные");
    assert_eq!(notification_lots_after[0].scope_label, "Область 1");
    assert_eq!(
        notification_lots_after[0].kind_label,
        "Сброшенный лимит запросов"
    );
    assert_eq!(notification_lots_after[0].state_label, "Истёк");
    assert_eq!(
        notification_lots_after[0].evidence_label,
        "Официальный поставщик · Высокая · Подробность поставщика"
    );
    for (before, after) in activity_before.iter().zip(&activity_after) {
        assert_eq!(after.time_label, before.time_label);
        assert_eq!(after.model_label, before.model_label);
        assert_eq!(after.input_availability, before.input_availability);
        assert_eq!(after.input_label, before.input_label);
        assert_eq!(after.cached_availability, before.cached_availability);
        assert_eq!(after.cached_label, before.cached_label);
        assert_eq!(after.output_availability, before.output_availability);
        assert_eq!(after.output_label, before.output_label);
        assert_eq!(after.reasoning_availability, before.reasoning_availability);
        assert_eq!(after.reasoning_label, before.reasoning_label);
        assert_eq!(after.total_availability, before.total_availability);
        assert_eq!(after.total_label, before.total_label);
    }
    for (before, after) in hour_before.iter().zip(&hour_after) {
        assert_eq!(after.label, before.label);
        assert_eq!(after.tokens_label, before.tokens_label);
        assert_eq!(after.exposure_label, before.exposure_label);
        assert_eq!(after.ratio, before.ratio);
    }
    for (before, after) in weekday_before.iter().zip(&weekday_after) {
        assert_eq!(after.tokens_label, before.tokens_label);
        assert_eq!(after.exposure_label, before.exposure_label);
        assert_eq!(after.ratio, before.ratio);
    }
    for (before, after) in notification_scopes_before
        .iter()
        .zip(&notification_scopes_after)
    {
        assert_eq!(after.leads_label, before.leads_label);
    }
    assert_eq!(
        notification_scopes_after[0].warning_label,
        "Неизвестное истечение"
    );
    for (before, after) in notification_lots_before
        .iter()
        .zip(&notification_lots_after)
    {
        assert_eq!(after.benefit_label, before.benefit_label);
        assert_eq!(after.quantity_label, before.quantity_label);
    }

    window.invoke_select_presentation_locale(2);
    assert_ne!(
        window.get_notifications_loaded_label(),
        "1 reminder profile · 4 current benefits"
    );
    assert_ne!(
        window
            .get_reminder_scope_rows()
            .row_data(0)
            .expect("scope")
            .scope_label,
        "Scope 1"
    );
    assert_ne!(
        window
            .get_benefit_lot_rows()
            .row_data(0)
            .expect("benefit lot")
            .kind_label,
        "Banked rate limit reset"
    );

    window.invoke_select_presentation_locale(0);
}

fn assert_compiled_command_palette_is_bounded_and_routes_through_desktop_state(
    window: &tokenmaster_desktop::MainWindow,
) {
    window.show().expect("show command palette window");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    assert!(!window.get_command_palette_visible());
    assert_eq!(window.get_command_palette_rows().row_count(), 0);

    window.invoke_open_command_palette();
    assert!(window.get_command_palette_visible());
    assert_eq!(window.get_command_palette_query(), "");
    assert_eq!(window.get_command_palette_rows().row_count(), 11);
    assert_eq!(window.get_command_palette_selected_ordinal(), 0);
    assert_eq!(
        window
            .get_command_palette_rows()
            .iter()
            .filter(|row| row.selected)
            .count(),
        1
    );

    dispatch_key(window, Key::DownArrow);
    assert_eq!(window.get_command_palette_selected_ordinal(), 1);
    dispatch_key(window, Key::Return);
    assert!(!window.get_command_palette_visible());
    assert_eq!(window.get_active_route_key(), "history");

    window.invoke_open_command_palette();
    dispatch_key(window, Key::Escape);
    assert!(!window.get_command_palette_visible());

    window.invoke_open_command_palette();
    dispatch_text(window, "help");
    assert_eq!(window.get_command_palette_query(), "help");
    assert_eq!(window.get_command_palette_rows().row_count(), 1);
    let accessible_routes = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Help / About, ready")
        })
        .find_all();
    assert_eq!(accessible_routes.len(), 1);
    accessible_routes[0].invoke_accessible_default_action();
    assert!(!window.get_command_palette_visible());
    assert_eq!(window.get_active_route_key(), "help_about");

    window.invoke_open_command_palette();

    window.invoke_command_palette_query_edited(SharedString::from("hElP"));
    assert_eq!(window.get_command_palette_rows().row_count(), 1);
    assert_eq!(window.get_command_palette_selected_ordinal(), 0);
    assert_eq!(
        window
            .get_command_palette_rows()
            .row_data(0)
            .expect("filtered route")
            .key,
        "help_about"
    );

    window.invoke_command_palette_query_edited(SharedString::from("settings"));
    let pointer_routes = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Settings, ready")
        })
        .find_all();
    assert_eq!(pointer_routes.len(), 1);
    pointer_routes[0].mock_single_click(PointerEventButton::Left);
    assert!(!window.get_command_palette_visible());
    assert_eq!(window.get_active_route_key(), "settings");

    window.invoke_open_command_palette();

    window.invoke_command_palette_query_edited(SharedString::from("🙂".repeat(10_000)));
    assert_eq!(window.get_command_palette_query().chars().count(), 64);
    assert_eq!(window.get_command_palette_rows().row_count(), 0);
    assert_eq!(window.get_command_palette_selected_ordinal(), -1);

    window.invoke_command_palette_query_edited(SharedString::from("data health"));
    window.invoke_activate_command_palette_selection();
    assert!(!window.get_command_palette_visible());
    assert_eq!(window.get_active_route_key(), "data_health");
    assert_eq!(window.get_active_route_state(), "unavailable");
}

fn assert_open_command_palette_refreshes_from_accepted_snapshot() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-command-palette-refresh.sqlite3");
    let mut reducer = ready_reducer(&path, 0);
    let shell = DesktopShell::new(&reducer.snapshot()).expect("desktop shell");
    let window = shell.window();

    window.invoke_open_command_palette();
    window.invoke_command_palette_query_edited(SharedString::from("data health"));
    assert_eq!(window.get_command_palette_rows().row_count(), 1);
    assert_eq!(
        window
            .get_command_palette_rows()
            .row_data(0)
            .expect("ready data-health route")
            .state,
        "ready"
    );

    reducer
        .fail_data_status(
            ProductAttemptGeneration::new(2).expect("attempt"),
            QueryErrorCode::DeadlineExceeded,
        )
        .expect("fail data-status refresh");
    assert_eq!(
        shell
            .apply_snapshot(&reducer.snapshot())
            .expect("apply refreshed snapshot"),
        DesktopApplyOutcome::Accepted
    );

    assert!(window.get_command_palette_visible());
    assert_eq!(window.get_command_palette_query(), "data health");
    assert_eq!(window.get_command_palette_rows().row_count(), 1);
    let refreshed = window
        .get_command_palette_rows()
        .row_data(0)
        .expect("refreshed data-health route");
    assert_eq!(refreshed.key, "data_health");
    assert_eq!(refreshed.state, "degraded");
    assert!(refreshed.selected);
}

fn assert_compiled_compact_widget_reuses_quota_snapshot_and_restores_window() {
    let unavailable_reducer = ProductReducer::new();
    let unavailable_shell =
        DesktopShell::new(&unavailable_reducer.snapshot()).expect("unavailable compact shell");
    let unavailable_window = unavailable_shell.window();
    unavailable_window
        .show()
        .expect("show unavailable compact window");
    unavailable_window
        .window()
        .set_size(slint::PhysicalSize::new(1_000, 700));
    unavailable_window.invoke_select_route(SharedString::from("compact_widget"));
    assert!(unavailable_window.get_compact_widget_visible());
    assert_eq!(unavailable_window.get_active_route_state(), "unavailable");
    assert_eq!(unavailable_window.get_compact_widget_quota_count(), 0);
    let unavailable_message = ElementQuery::from_root(unavailable_window)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Quota evidence unavailable")
        })
        .find_all();
    assert_eq!(unavailable_message.len(), 1);

    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-compact-widget.sqlite3");
    let reducer = ready_reducer(&path, 31);
    let shell = DesktopShell::new(&reducer.snapshot()).expect("compact shell");
    let window = shell.window();
    let component_address = window as *const _;
    window.show().expect("show compact window");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_000, 700));

    window.invoke_select_route(SharedString::from("compact_widget"));
    assert_eq!(window as *const _, component_address);
    assert!(window.get_compact_widget_visible());
    assert!(!window.get_dashboard_visible());
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(window.get_dashboard_quota_rows().row_count(), 32);
    assert_eq!(window.get_compact_widget_quota_count(), 32);
    assert_eq!(window.window().size(), slint::PhysicalSize::new(420, 560));
    assert_eq!(window.get_compact_widget_layout_mode(), "wide");

    let compact_rows = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Groupbox)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label.starts_with("Compact quota window "))
        })
        .find_all();
    assert!(!compact_rows.is_empty());
    assert!(compact_rows.len() <= 32);

    let original_quotas = window.get_dashboard_quota_rows().iter().collect::<Vec<_>>();
    let mut unknown_ratio = original_quotas[0].clone();
    unknown_ratio.ratio_known = false;
    unknown_ratio.usage_label = SharedString::from("Usage unavailable");
    window.set_dashboard_quota_rows(ModelRc::new(VecModel::from(vec![unknown_ratio])));
    assert_eq!(window.get_compact_widget_quota_count(), 1);
    let unknown_ratio_messages = ElementQuery::from_root(window)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Usage ratio unavailable")
        })
        .find_all();
    assert_eq!(unknown_ratio_messages.len(), 1);
    window.set_dashboard_quota_rows(ModelRc::new(VecModel::from(original_quotas)));
    assert_eq!(window.get_compact_widget_quota_count(), 32);

    window.window().set_size(slint::PhysicalSize::new(360, 480));
    assert_eq!(window.get_compact_widget_layout_mode(), "narrow");
    let return_buttons = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Return to Dashboard")
        })
        .find_all();
    assert_eq!(return_buttons.len(), 1);
    return_buttons[0].invoke_accessible_default_action();
    assert!(window.get_dashboard_visible());
    assert!(!window.get_compact_widget_visible());
    assert_eq!(window.window().size(), slint::PhysicalSize::new(1_000, 700));

    window.invoke_select_route(SharedString::from("compact_widget"));
    dispatch_key(window, Key::Return);
    assert!(window.get_dashboard_visible());
    assert_eq!(window.window().size(), slint::PhysicalSize::new(1_000, 700));

    window.invoke_select_route(SharedString::from("compact_widget"));
    let return_buttons = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Return to Dashboard")
        })
        .find_all();
    assert_eq!(return_buttons.len(), 1);
    return_buttons[0].mock_single_click(PointerEventButton::Left);
    assert!(window.get_dashboard_visible());

    for _ in 0..10_000 {
        window.invoke_select_route(SharedString::from("compact_widget"));
        window.invoke_select_route(SharedString::from("dashboard"));
    }
    assert_eq!(window as *const _, component_address);
    assert!(window.get_dashboard_visible());
    assert_eq!(window.get_dashboard_quota_rows().row_count(), 32);
    assert_eq!(window.get_compact_widget_quota_count(), 32);
    assert_eq!(window.window().size(), slint::PhysicalSize::new(1_000, 700));
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
    assert_eq!(window.get_activity_rhythm_state(), "ready");
    assert_eq!(window.get_activity_rhythm_time_zone_label(), "UTC");
    assert_eq!(
        window.get_activity_rhythm_range_label(),
        "2026-06-17 – before 2026-07-17"
    );
    assert_eq!(window.get_activity_rhythm_hour_rows().row_count(), 24);
    assert_eq!(window.get_activity_rhythm_weekday_rows().row_count(), 7);
    assert_eq!(
        window
            .get_activity_rhythm_hour_rows()
            .row_data(1)
            .expect("hour row")
            .exposure_label,
        "1800m/30x"
    );
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

#[test]
fn sessions_pagination_controls_are_accessible_replace_only_and_block_selection_while_pending() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-sessions-pagination.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 65);
    let snapshot = reducer.snapshot();
    let page_sink = Rc::new(RecordingSessionPageSink::default());
    let detail_sink = Rc::new(RecordingSessionDetailSink::default());
    let shell = DesktopShell::new_with_reliable_state_and_session_sinks(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        detail_sink.clone(),
        page_sink.clone(),
    )
    .expect("desktop shell");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    assert_eq!(
        shell
            .apply_snapshot_for_epoch(epoch, &snapshot)
            .expect("bind snapshot"),
        DesktopApplyOutcome::Accepted
    );
    let window = shell.window();
    window.invoke_select_route(SharedString::from("sessions"));
    window.show().expect("show sessions window");

    assert!(!window.get_sessions_navigation_pending());
    assert!(window.get_sessions_next_enabled());
    assert!(!window.get_sessions_back_to_newest_enabled());
    assert_eq!(
        window.get_sessions_page_status_label(),
        "Newest page · More sessions available"
    );
    let initial_rows = window.get_session_list_rows().row_count();
    let next = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Next page")
        })
        .find_all();
    let newest = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Back to newest")
        })
        .find_all();
    assert_eq!(next.len(), 1);
    assert_eq!(newest.len(), 1);

    for (width, layout) in [(700, "narrow"), (1120, "wide")] {
        window
            .window()
            .set_size(slint::PhysicalSize::new(width, 720));
        assert_eq!(window.get_sessions_layout_mode(), layout);
        for label in ["Next page", "Back to newest"] {
            assert_eq!(
                ElementQuery::from_root(window)
                    .match_accessible_role(AccessibleRole::Button)
                    .match_predicate(move |element| {
                        element
                            .accessible_label()
                            .is_some_and(|value| value == label)
                    })
                    .find_all()
                    .len(),
                1,
                "{label} remains accessible in {layout} layout"
            );
        }
    }

    dispatch_key(window, Key::Tab);
    next[0].mock_single_click(PointerEventButton::Left);
    assert_eq!(page_sink.intents.borrow().len(), 1);
    assert_eq!(
        page_sink.intents.borrow()[0].direction(),
        DesktopSessionPageDirection::Next
    );
    assert!(window.get_sessions_navigation_pending());
    assert!(!window.get_sessions_next_enabled());
    assert!(!window.get_sessions_back_to_newest_enabled());
    assert_eq!(window.get_sessions_page_status_label(), "Loading sessions…");
    assert_eq!(window.get_session_list_rows().row_count(), initial_rows);

    let rows = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label.starts_with("From "))
        })
        .find_all();
    rows[0].mock_single_click(PointerEventButton::Left);
    assert!(detail_sink.intents.borrow().is_empty());
}

#[test]
fn rejected_sessions_navigation_restores_controls_and_keyboard_dispatches_both_directions() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("ui-sessions-navigation-rejected.sqlite3");
    let mut reducer = ready_reducer_with_usage(&path, 0, 65);
    let snapshot = reducer.snapshot();
    let next_sink = Rc::new(RejectingSessionPageSink::default());
    let next_shell = DesktopShell::new_with_reliable_state_and_session_sinks(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        Rc::new(RecordingSessionDetailSink::default()),
        next_sink.clone(),
    )
    .expect("next shell");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    next_shell
        .apply_snapshot_for_epoch(epoch, &snapshot)
        .expect("bind snapshot");
    let next_window = next_shell.window();
    next_window.invoke_select_route(SharedString::from("sessions"));
    next_window.show().expect("show next window");
    let next = ElementQuery::from_root(next_window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Next page")
        })
        .find_all();
    next[0].mock_single_click(PointerEventButton::Left);
    dispatch_key(next_window, Key::Return);
    dispatch_key(next_window, Key::Space);
    assert_eq!(next_sink.intents.borrow().len(), 3);
    assert!(
        next_sink
            .intents
            .borrow()
            .iter()
            .all(|intent| intent.direction() == DesktopSessionPageDirection::Next)
    );
    assert!(!next_window.get_sessions_navigation_pending());
    assert!(next_window.get_sessions_next_enabled());
    assert_eq!(
        next_window.get_sessions_page_status_label(),
        "Newest page · More sessions available"
    );

    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let newest = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(64).expect("page"), Vec::new())
                .expect("newest request"),
        )
        .expect("newest page");
    let continuation = service
        .usage_sessions(
            UsageSessionPageRequest::continuation(
                PageSize::new(64).expect("page"),
                newest.payload().next_cursor().expect("cursor").clone(),
                Vec::new(),
            )
            .expect("continuation request"),
        )
        .expect("continuation page");
    reducer
        .publish_sessions(
            ProductAttemptGeneration::new(2).expect("attempt"),
            continuation,
        )
        .expect("publish continuation");
    let continuation_snapshot = reducer.snapshot();
    let newest_sink = Rc::new(RejectingSessionPageSink::default());
    let newest_shell = DesktopShell::new_with_reliable_state_and_session_sinks(
        &continuation_snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        Rc::new(RecordingSessionDetailSink::default()),
        newest_sink.clone(),
    )
    .expect("newest shell");
    newest_shell
        .apply_snapshot_for_epoch(epoch, &continuation_snapshot)
        .expect("bind continuation snapshot");
    let newest_window = newest_shell.window();
    newest_window.invoke_select_route(SharedString::from("sessions"));
    newest_window.show().expect("show newest window");
    assert!(newest_window.get_sessions_back_to_newest_enabled());
    assert_eq!(
        newest_window.get_sessions_page_status_label(),
        "Older sessions · Oldest sessions loaded"
    );
    let newest = ElementQuery::from_root(newest_window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "Back to newest")
        })
        .find_all();
    newest[0].mock_single_click(PointerEventButton::Left);
    dispatch_key(newest_window, Key::Return);
    dispatch_key(newest_window, Key::Space);
    assert_eq!(newest_sink.intents.borrow().len(), 3);
    assert!(
        newest_sink
            .intents
            .borrow()
            .iter()
            .all(|intent| intent.direction() == DesktopSessionPageDirection::Newest)
    );
    assert!(newest_window.get_sessions_back_to_newest_enabled());
}

#[test]
fn retained_unavailable_newest_page_enables_bounded_newest_recovery_only() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-sessions-retained-newest.sqlite3");
    let mut reducer = ready_reducer_with_usage(&path, 0, 65);
    reducer
        .fail_sessions(
            ProductAttemptGeneration::new(2).expect("attempt"),
            QueryErrorCode::DeadlineExceeded,
        )
        .expect("retain newest payload after failed next page");
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new_with_reliable_state_and_session_sinks(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        Rc::new(RecordingSessionDetailSink::default()),
        Rc::new(RecordingSessionPageSink::default()),
    )
    .expect("desktop shell");
    shell
        .apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), &snapshot)
        .expect("bind snapshot");
    let window = shell.window();
    window.invoke_select_route(SharedString::from("sessions"));
    assert!(!window.get_sessions_next_enabled());
    assert!(
        window.get_sessions_back_to_newest_enabled(),
        "retained unavailable newest page exposes only its recovery action"
    );
}

#[test]
fn sessions_pagination_ui_contract_rejects_identity_and_append_paths() {
    let main = include_str!("../ui/main.slint");
    let models = include_str!("../ui/models.slint");
    let view = include_str!("../ui/views/sessions-view.slint");
    let ui = include_str!("../src/ui.rs");
    assert!(!main.to_ascii_lowercase().contains("cursor"));
    assert!(!models.to_ascii_lowercase().contains("cursor"));
    assert!(!view.to_ascii_lowercase().contains("cursor"));
    assert!(main.contains("sessions-navigation-pending"));
    assert!(main.contains("sessions-next-enabled"));
    assert!(main.contains("sessions-back-to-newest-enabled"));
    assert!(view.contains("focus-on-tab-navigation: root.next-enabled"));
    assert!(
        view.contains("focus-on-tab-navigation: !root.next-enabled && root.back-to-newest-enabled")
    );
    assert!(view.contains("forward-focus: next-page-button"));
    assert!(view.contains("forward-focus: back-to-newest-button"));
    assert!(
        !view.contains("Newest all-time session summaries"),
        "session subtitle must not claim a newest page while showing retained/unavailable pages"
    );
    assert!(session_model_replacement_is_pinned(ui));
    let sessions_projection = ui
        .split("fn apply_sessions_projection")
        .nth(1)
        .expect("sessions projection")
        .split("fn apply_session_detail_projection")
        .next()
        .expect("sessions projection body");
    assert!(!sessions_projection.contains(".push("));
    assert!(!sessions_projection.contains(".append("));
    assert!(!sessions_projection.contains(".extend("));
    let navigation_wiring = ui
        .split("fn wire_session_page_intents")
        .nth(1)
        .expect("session navigation wiring")
        .split("pub(crate) fn apply_projection")
        .next()
        .expect("session navigation wiring body");
    assert!(!navigation_wiring.contains("set_session_list_rows"));
    let route_wiring = ui
        .split("fn wire_route_selection")
        .nth(1)
        .expect("route wiring")
        .split("fn wire_command_palette")
        .next()
        .expect("route wiring body");
    assert!(!route_wiring.contains("apply_sessions_projection"));
}

#[test]
fn sessions_pagination_model_identity_does_not_churn_for_pending_or_route_selection() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-sessions-model-identity.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 65);
    let snapshot = reducer.snapshot();
    let page_sink = Rc::new(RecordingSessionPageSink::default());
    let shell = DesktopShell::new_with_reliable_state_and_session_sinks(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        Rc::new(RecordingSessionDetailSink::default()),
        page_sink,
    )
    .expect("desktop shell");
    shell
        .apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), &snapshot)
        .expect("bind snapshot");
    let window = shell.window();
    window.invoke_select_route(SharedString::from("sessions"));
    let initial = window.get_session_list_rows();
    window.invoke_request_session_page_next();
    assert!(window.get_sessions_navigation_pending());
    assert_eq!(window.get_session_list_rows(), initial);
    window.invoke_select_route(SharedString::from("dashboard"));
    window.invoke_select_route(SharedString::from("sessions"));
    assert_eq!(window.get_session_list_rows(), initial);
}

#[test]
fn shell_retains_session_page_sink_after_caller_rc_is_dropped() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-sessions-retained-sink.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 65);
    let snapshot = reducer.snapshot();
    let intents = Rc::new(RefCell::new(Vec::new()));
    let page_sink: Rc<dyn DesktopSessionPageIntentSink> = Rc::new(RetainedSessionPageSink {
        intents: intents.clone(),
    });
    let weak = Rc::downgrade(&page_sink);
    let shell = DesktopShell::new_with_reliable_state_and_session_sinks(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        Rc::new(RecordingSessionDetailSink::default()),
        page_sink.clone(),
    )
    .expect("desktop shell");
    shell
        .apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), &snapshot)
        .expect("bind snapshot");
    drop(page_sink);
    assert!(weak.upgrade().is_some());
    shell.window().invoke_request_session_page_next();
    assert_eq!(intents.borrow().len(), 1);
    drop(shell);
    assert!(weak.upgrade().is_none());
}

fn session_model_replacement_is_pinned(ui: &str) -> bool {
    const REPLACEMENT: &str = "window.set_session_list_rows(model(rows));";
    if ui.matches(REPLACEMENT).count() != 1 {
        return false;
    }
    let sessions_projection = ui
        .split("fn apply_sessions_projection")
        .nth(1)
        .expect("sessions projection")
        .split("fn apply_session_detail_projection")
        .next()
        .expect("sessions projection body");
    if !sessions_projection.contains(REPLACEMENT) {
        return false;
    }
    let route_wiring = ui
        .split("fn wire_route_selection")
        .nth(1)
        .expect("route wiring")
        .split("fn wire_command_palette")
        .next()
        .expect("route wiring body");
    if route_wiring.contains(REPLACEMENT) {
        return false;
    }

    let moved = ui
        .replacen(REPLACEMENT, "", 1)
        .replacen(
            "apply_selected_route(&window, &projection, &compact_window_mode);",
            "apply_selected_route(&window, &projection, &compact_window_mode);\n        window.set_session_list_rows(model(rows));",
            1,
        );
    !moved.is_empty() && !moved.eq(ui) && !session_model_replacement_is_pinned(&moved)
}

#[test]
fn history_range_controls_are_fixed_accessible_and_replace_models_without_route_churn() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-history-range.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 64);
    let snapshot = reducer.snapshot();
    let range_sink = Rc::new(RecordingHistoryRangeSink::default());
    let shell = DesktopShell::new_with_reliable_state_and_history_and_session_sinks(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        range_sink.clone(),
        Rc::new(RecordingSessionDetailSink::default()),
        Rc::new(RecordingSessionPageSink::default()),
    )
    .expect("desktop shell");
    shell
        .apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), &snapshot)
        .expect("bind snapshot");
    let window = shell.window();
    window.invoke_select_route(SharedString::from("history"));
    window.show().expect("show history window");

    assert_eq!(window.get_history_range_preset(), "recent_30_days");
    assert!(!window.get_history_range_pending());
    let initial_rows = window.get_history_day_rows();
    assert_eq!(initial_rows.row_count(), 30);
    let controls = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element.accessible_label().is_some_and(|label| {
                matches!(
                    label.as_str(),
                    "History range 1 day" | "History range 7 days" | "History range 30 days"
                )
            })
        })
        .find_all();
    assert_eq!(controls.len(), 3);

    controls[2].mock_single_click(PointerEventButton::Left);
    assert!(range_sink.intents.borrow().is_empty());
    window.invoke_select_route(SharedString::from("dashboard"));
    window.invoke_select_route(SharedString::from("history"));
    assert_eq!(window.get_history_day_rows(), initial_rows);

    let one_day = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "History range 1 day")
        })
        .find_all();
    assert_eq!(one_day.len(), 1);
    one_day[0].mock_single_click(PointerEventButton::Left);
    assert_eq!(range_sink.intents.borrow().len(), 1);
    assert_eq!(
        range_sink.intents.borrow()[0].preset(),
        DesktopHistoryRangePreset::Recent1Day
    );
    assert_eq!(window.get_history_range_preset(), "recent_30_days");
    assert!(window.get_history_range_pending());
    assert_eq!(window.get_history_day_rows(), initial_rows);
    let seven_days = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .match_predicate(|element| {
            element
                .accessible_label()
                .is_some_and(|label| label == "History range 7 days")
        })
        .find_all();
    assert_eq!(seven_days.len(), 1);
    seven_days[0].mock_single_click(PointerEventButton::Left);
    assert_eq!(range_sink.intents.borrow().len(), 1);
}

#[test]
fn ten_thousand_accepted_history_range_snapshots_replace_shared_bounded_models() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("ui-history-range-replacements.sqlite3");
    let mut reducer = ready_reducer_with_usage(&path, 0, 64);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let histories = [1, 7, 30].map(|days| {
        service
            .usage_analytics(
                UsageAnalyticsRequest::new(
                    UsageRange::recent_days(days).expect("recent history range"),
                    UsageTimeZone::iana("UTC").expect("UTC"),
                    WeekStart::Monday,
                    UsageSeriesSelection::Daily,
                    Vec::new(),
                    vec![UsageBreakdownKind::Model, UsageBreakdownKind::Project],
                )
                .expect("history request"),
            )
            .expect("history analytics")
    });
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    shell
        .apply_snapshot_for_epoch(epoch, &snapshot)
        .expect("bind snapshot");
    let window = shell.window();

    for iteration in 0_u64..10_000 {
        let index = usize::try_from(iteration % 3).expect("preset index");
        reducer
            .publish_history(
                ProductAttemptGeneration::new(iteration + 2).expect("attempt"),
                histories[index].clone(),
            )
            .expect("publish history");
        let snapshot = reducer.snapshot();
        assert_eq!(
            shell
                .apply_snapshot_for_epoch(epoch, &snapshot)
                .expect("apply snapshot"),
            DesktopApplyOutcome::Accepted
        );
        let (preset, expected_days, expected_range) = match index {
            0 => ("recent_1_day", 1, "2026-07-16 – before 2026-07-17"),
            1 => ("recent_7_days", 7, "2026-07-10 – before 2026-07-17"),
            _ => ("recent_30_days", 30, "2026-06-17 – before 2026-07-17"),
        };
        assert_eq!(window.get_history_range_preset(), preset);
        assert!(window.get_history_day_rows().row_count() <= expected_days);
        assert!(window.get_history_day_rows().row_count() <= MAX_HISTORY_DAYS);
        assert_eq!(window.get_models_range_label(), expected_range);
        assert_eq!(window.get_projects_usage_range_label(), expected_range);
        assert!(window.get_model_usage_rows().row_count() <= MAX_MODEL_ROWS);
        assert!(window.get_project_usage_rows().row_count() <= MAX_PROJECT_ROWS);
    }
}

#[test]
fn history_range_rejection_restores_controls_and_tab_reaches_return_and_space() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-history-range-rejection.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 64);
    let snapshot = reducer.snapshot();
    let rejecting = Rc::new(RejectingHistoryRangeSink::default());
    let shell = DesktopShell::new_with_reliable_state_and_history_and_session_sinks(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RejectingIntentSink),
        rejecting.clone(),
        Rc::new(RecordingSessionDetailSink::default()),
        Rc::new(RecordingSessionPageSink::default()),
    )
    .expect("desktop shell");
    shell
        .apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), &snapshot)
        .expect("bind snapshot");
    let window = shell.window();
    window.invoke_select_route(SharedString::from("history"));
    let rows = window.get_history_day_rows();
    window.invoke_request_history_range_1();
    assert_eq!(rejecting.intents.borrow().len(), 1);
    assert!(!window.get_history_range_pending());
    assert_eq!(window.get_history_range_preset(), "recent_30_days");
    assert_eq!(window.get_history_day_rows(), rows);

    for key in [Key::Return, Key::Space] {
        let mut reachable_at = None;
        for tab_count in 1..=32 {
            let sink = Rc::new(RecordingHistoryRangeSink::default());
            let shell = DesktopShell::new_with_reliable_state_and_history_and_session_sinks(
                &snapshot,
                DesktopReliableStateProjection::unavailable(),
                Rc::new(RejectingIntentSink),
                sink.clone(),
                Rc::new(RecordingSessionDetailSink::default()),
                Rc::new(RecordingSessionPageSink::default()),
            )
            .expect("desktop shell");
            shell
                .apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), &snapshot)
                .expect("bind snapshot");
            let window = shell.window();
            window.invoke_select_route(SharedString::from("history"));
            window.show().expect("show history window");
            window
                .window()
                .dispatch_event(WindowEvent::WindowActiveChanged(true));
            for _ in 0..tab_count {
                dispatch_key(window, Key::Tab);
            }
            dispatch_key(window, key);
            if sink
                .intents
                .borrow()
                .first()
                .is_some_and(|intent| intent.preset() == DesktopHistoryRangePreset::Recent1Day)
            {
                reachable_at = Some(tab_count);
                break;
            }
        }
        assert!(
            reachable_at.is_some(),
            "1 day must be reachable by Tab then {key:?}"
        );
    }
}

#[test]
fn history_range_model_replacement_is_bounded_and_has_no_append_or_load_more_path() {
    let ui = include_str!("../src/ui.rs");
    let terminal_projection = ui
        .split("pub(crate) fn apply_history_projection")
        .nth(1)
        .expect("history projection")
        .split("fn apply_history_snapshot_projection")
        .next()
        .expect("terminal history projection body");
    assert!(!terminal_projection.contains("set_history_day_rows"));
    let projection = ui
        .split("fn apply_history_snapshot_projection")
        .nth(1)
        .expect("accepted snapshot history projection")
        .split("fn apply_history_range_state")
        .next()
        .expect("accepted snapshot history projection body");
    assert_eq!(
        projection
            .matches("window.set_history_day_rows(model(rows));")
            .count(),
        1
    );
    assert!(!projection.contains(".append("));
    assert!(!projection.contains(".extend("));
    assert!(!ui.contains("load-more-history"));
}

#[test]
fn tab_reaches_the_enabled_next_button_without_pointer_activation() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-sessions-tab-next.sqlite3");
    let reducer = ready_reducer_with_usage(&path, 0, 65);
    let snapshot = reducer.snapshot();
    assert_session_page_direction_reachable_by_tab(&snapshot, DesktopSessionPageDirection::Next);
}

#[test]
fn tab_reaches_the_enabled_back_to_newest_button_without_pointer_activation() {
    i_slint_backend_testing::init_no_event_loop();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("ui-sessions-tab-newest.sqlite3");
    let mut reducer = ready_reducer_with_usage(&path, 0, 65);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let newest = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(64).expect("page"), Vec::new())
                .expect("newest request"),
        )
        .expect("newest page");
    let continuation = service
        .usage_sessions(
            UsageSessionPageRequest::continuation(
                PageSize::new(64).expect("page"),
                newest.payload().next_cursor().expect("cursor").clone(),
                Vec::new(),
            )
            .expect("continuation request"),
        )
        .expect("continuation page");
    reducer
        .publish_sessions(
            ProductAttemptGeneration::new(2).expect("attempt"),
            continuation,
        )
        .expect("publish continuation");
    let snapshot = reducer.snapshot();
    assert_session_page_direction_reachable_by_tab(&snapshot, DesktopSessionPageDirection::Newest);
}

fn assert_session_page_direction_reachable_by_tab(
    snapshot: &ProductSnapshot,
    direction: DesktopSessionPageDirection,
) {
    let mut reachable_at = None;
    for tab_count in 1..=32 {
        let page_sink = Rc::new(RecordingSessionPageSink::default());
        let shell = DesktopShell::new_with_reliable_state_and_session_sinks(
            snapshot,
            DesktopReliableStateProjection::unavailable(),
            Rc::new(RejectingIntentSink),
            Rc::new(RecordingSessionDetailSink::default()),
            page_sink.clone(),
        )
        .expect("desktop shell");
        shell
            .apply_snapshot_for_epoch(DesktopSnapshotEpoch::new(1).expect("epoch"), snapshot)
            .expect("bind snapshot");
        let window = shell.window();
        window.invoke_select_route(SharedString::from("sessions"));
        window.show().expect("show sessions window");
        window
            .window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        for _ in 0..tab_count {
            dispatch_key(window, Key::Tab);
        }
        dispatch_key(window, Key::Return);
        if page_sink
            .intents
            .borrow()
            .first()
            .is_some_and(|intent| intent.direction() == direction)
        {
            reachable_at = Some(tab_count);
            break;
        }
    }
    assert!(
        reachable_at.is_some(),
        "{direction:?} must be reachable by Tab then Return"
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

fn dispatch_text(window: &tokenmaster_desktop::MainWindow, text: &str) {
    window.window().dispatch_event(WindowEvent::KeyPressed {
        text: SharedString::from(text),
    });
    window.window().dispatch_event(WindowEvent::KeyReleased {
        text: SharedString::from(text),
    });
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
        "Newest page · More sessions available"
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
