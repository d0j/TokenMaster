use std::{
    fmt,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokenmaster_product::ProductSnapshot;

use crate::{
    DashboardActivityRow, DashboardBenefitRow, DashboardModelRow, DashboardQuotaRow,
    DashboardSectionRow, DashboardSessionRow, DashboardTrendPoint, DesktopActivityKey,
    DesktopCostValue, DesktopDashboardProjection, DesktopDashboardSectionKey, DesktopFreshness,
    DesktopQuality, DesktopSnapshotBridge, DesktopSnapshotReceiver, DesktopTokenValue,
    DesktopValueAvailability, MainWindow, RouteRow,
    presentation::{DesktopApplyOutcome, DesktopProjection, DesktopRouteKey, DesktopState},
};

pub struct DesktopShell {
    window: MainWindow,
    state: SharedDesktopState,
}

pub(crate) type SharedDesktopState = Arc<Mutex<DesktopState>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopUiErrorCode {
    StateUnavailable,
}

impl DesktopUiErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::StateUnavailable => "state_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopUiError {
    code: DesktopUiErrorCode,
}

impl DesktopUiError {
    const fn state_unavailable() -> Self {
        Self {
            code: DesktopUiErrorCode::StateUnavailable,
        }
    }

    #[must_use]
    pub const fn code(self) -> DesktopUiErrorCode {
        self.code
    }

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        self.code.stable_code()
    }
}

impl fmt::Display for DesktopUiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

impl std::error::Error for DesktopUiError {}

impl DesktopShell {
    pub fn new(snapshot: &ProductSnapshot) -> Result<Self, slint::PlatformError> {
        let window = MainWindow::new()?;
        let initial_state = DesktopState::new(snapshot, DesktopRouteKey::Dashboard);
        apply_projection(&window, initial_state.projection());
        let state = Arc::new(Mutex::new(initial_state));
        wire_route_selection(&window, state.clone());
        Ok(Self { window, state })
    }

    #[must_use]
    pub const fn window(&self) -> &MainWindow {
        &self.window
    }

    pub fn apply_snapshot(
        &self,
        snapshot: &ProductSnapshot,
    ) -> Result<DesktopApplyOutcome, DesktopUiError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?;
        let outcome = state.apply_snapshot(snapshot);
        if outcome == DesktopApplyOutcome::Accepted {
            apply_projection(&self.window, state.projection());
        }
        Ok(outcome)
    }

    pub(crate) fn state_handle(&self) -> SharedDesktopState {
        self.state.clone()
    }

    #[must_use]
    pub fn snapshot_bridge(&self, receiver: DesktopSnapshotReceiver) -> DesktopSnapshotBridge {
        DesktopSnapshotBridge::new(self.window.as_weak(), self.state_handle(), receiver)
    }
}

fn wire_route_selection(window: &MainWindow, state: SharedDesktopState) {
    let weak = window.as_weak();
    window.on_select_route(move |key| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let Ok(mut state) = state.lock() else {
            return;
        };
        if state.select_stable_key(key.as_str()).is_ok() {
            apply_route_projection(&window, state.projection());
        }
    });
}

pub(crate) fn apply_projection(window: &MainWindow, projection: &DesktopProjection) {
    apply_route_projection(window, projection);
    apply_dashboard_projection(window, projection.dashboard());
}

fn apply_route_projection(window: &MainWindow, projection: &DesktopProjection) {
    let rows = projection
        .routes()
        .iter()
        .map(|route| RouteRow {
            key: SharedString::from(route.key().stable_key()),
            label_key: SharedString::from(route.key().label_key()),
            label: SharedString::from(route.key().english_label()),
            state: SharedString::from(route.state().stable_code()),
            reasons: SharedString::from(join_reasons(route.reason_codes().iter())),
            selected: route.key() == projection.selected(),
        })
        .collect::<Vec<_>>();
    let active = projection.route(projection.selected());

    window.set_route_rows(ModelRc::new(VecModel::from(rows)));
    window.set_active_route_key(SharedString::from(projection.selected().stable_key()));
    window.set_active_route_label(SharedString::from(projection.selected().english_label()));
    window.set_active_route_state(SharedString::from(active.state().stable_code()));
    window.set_active_route_reasons(SharedString::from(join_reasons(
        active.reason_codes().iter(),
    )));
    window.set_product_generation(SharedString::from(
        projection.generation().get().to_string(),
    ));
}

fn join_reasons(reasons: impl Iterator<Item = &'static str>) -> String {
    reasons.collect::<Vec<_>>().join(", ")
}

fn apply_dashboard_projection(window: &MainWindow, dashboard: &DesktopDashboardProjection) {
    let sections = dashboard
        .sections()
        .iter()
        .map(|section| DashboardSectionRow {
            key: section.key().stable_key().into(),
            label_key: section.key().label_key().into(),
            label: section_label(section.key()).into(),
            state: section.state().stable_code().into(),
            reasons: join_reasons(section.reason_codes().iter()).into(),
            has_data: section.has_data(),
        })
        .collect::<Vec<_>>();
    window.set_dashboard_section_rows(model(sections));

    let header = dashboard.header();
    window.set_dashboard_header_tokens(format_tokens(header.tokens()).into());
    window.set_dashboard_header_cost(format_cost(header.cost()).into());
    window.set_dashboard_header_events(format_optional_events(header.event_count()).into());
    window.set_dashboard_header_evidence(
        format_evidence(header.freshness(), header.quality()).into(),
    );

    let quota_rows = dashboard
        .quota_rows()
        .iter()
        .map(|row| {
            let used_ppm = row.used_ppm().or_else(|| {
                row.remaining_ppm()
                    .and_then(|value| 1_000_000_u32.checked_sub(value))
            });
            DashboardQuotaRow {
                ordinal: i32::from(row.ordinal()),
                label_key: row.label_key().into(),
                label: humanize_key(row.label_key()).into(),
                ratio_known: used_ppm.is_some(),
                used_ratio: used_ppm.map_or(0.0, ppm_ratio),
                usage_label: format_ratio(used_ppm, "used").into(),
                remaining_label: format_ratio(row.remaining_ppm(), "remaining").into(),
                units_label: format_quota_units(row).into(),
                reset_label: row
                    .advertised_reset_at_ms()
                    .map_or_else(
                        || "Reset time unavailable".to_owned(),
                        |value| format!("Resets {}", format_timestamp_ms(value)),
                    )
                    .into(),
                evidence_label: format!(
                    "{} · {} · {} confidence",
                    freshness_label(row.freshness()),
                    quality_label(row.quality()),
                    row.confidence()
                )
                .into(),
            }
        })
        .collect::<Vec<_>>();
    window.set_dashboard_quota_rows(model(quota_rows));

    let benefit_rows = dashboard
        .benefit_scopes()
        .iter()
        .map(|scope| DashboardBenefitRow {
            ordinal: i32::from(scope.ordinal()),
            reset_quantity_label: format_integer(scope.available_reset_quantity()).into(),
            credit_quantity_label: format_integer(scope.available_credit_quantity()).into(),
            temporary_quantity_label: format_integer(scope.available_temporary_quantity()).into(),
            other_quantity_label: format_integer(scope.available_unknown_quantity()).into(),
            unavailable_quantity_label: format_integer(scope.non_available_quantity()).into(),
            expiry_label: scope
                .nearest_reset_expiry_at_ms()
                .map_or_else(
                    || "Expiry unavailable".to_owned(),
                    |value| format!("Expires {}", format_timestamp_ms(value)),
                )
                .into(),
            reminder_label: reminder_label(scope.reminder_coverage()).into(),
            evidence_label: format!(
                "{} · {}",
                freshness_label(scope.freshness()),
                quality_label(scope.quality())
            )
            .into(),
        })
        .collect::<Vec<_>>();
    window.set_dashboard_benefit_rows(model(benefit_rows));

    apply_code_output(window, dashboard);
    apply_trend(window, dashboard);
    apply_sessions(window, dashboard);
    apply_activity(window, dashboard);
    apply_models(window, dashboard);
}

fn apply_code_output(window: &MainWindow, dashboard: &DesktopDashboardProjection) {
    let section = dashboard.section(DesktopDashboardSectionKey::CodeOutput);
    if !section.has_data() {
        for setter in [
            MainWindow::set_dashboard_code_commits,
            MainWindow::set_dashboard_code_added,
            MainWindow::set_dashboard_code_removed,
            MainWindow::set_dashboard_code_net,
            MainWindow::set_dashboard_code_efficiency,
            MainWindow::set_dashboard_code_evidence,
        ] {
            setter(window, "—".into());
        }
        return;
    }
    let code = dashboard.code_output();
    window.set_dashboard_code_commits(format_counted(code.commits(), "commit", "commits").into());
    window.set_dashboard_code_added(format!("+{}", format_integer(code.added_lines())).into());
    window.set_dashboard_code_removed(format!("−{}", format_integer(code.removed_lines())).into());
    window.set_dashboard_code_net(format_signed(code.net_lines()).into());
    window.set_dashboard_code_efficiency(
        code.cost_per_100_added_lines_micros()
            .map_or_else(
                || "Efficiency unavailable".to_owned(),
                |value| format!("{} / 100 lines", format_usd_micros(value)),
            )
            .into(),
    );
    window.set_dashboard_code_evidence(
        format!(
            "{} · {} · {}",
            freshness_label(code.freshness()),
            quality_label(code.quality()),
            if code.complete() {
                "complete"
            } else {
                "incomplete"
            }
        )
        .into(),
    );
}

fn apply_trend(window: &MainWindow, dashboard: &DesktopDashboardProjection) {
    let max_tokens = dashboard.trend_max_tokens();
    let max_cost = dashboard.trend_max_cost_micros();
    let rows = dashboard
        .trend_points()
        .iter()
        .map(|point| {
            let (start_year, start_month, start_day) = point.start_date();
            let (end_year, end_month, end_day) = point.end_date();
            DashboardTrendPoint {
                date_label: format!(
                    "{start_year:04}-{start_month:02}-{start_day:02}–{end_year:04}-{end_month:02}-{end_day:02}"
                ).into(),
                tokens_availability: availability_code(point.tokens().availability()).into(),
                tokens_label: format_tokens(point.tokens()).into(),
                tokens_ratio: ratio(point.tokens().known_sum(), max_tokens),
                cost_availability: availability_code(point.cost().availability()).into(),
                cost_label: format_cost(point.cost()).into(),
                cost_ratio: ratio(point.cost().micros(), max_cost),
            }
        })
        .collect::<Vec<_>>();
    window.set_dashboard_trend_points(model(rows));
}

fn apply_sessions(window: &MainWindow, dashboard: &DesktopDashboardProjection) {
    let rows = dashboard
        .sessions()
        .iter()
        .map(|session| DashboardSessionRow {
            ordinal: i32::from(session.ordinal()),
            time_label: format!(
                "{}–{}",
                format_timestamp_seconds(session.first_timestamp_seconds()),
                format_timestamp_seconds(session.last_timestamp_seconds())
            )
            .into(),
            tokens_availability: availability_code(session.tokens().availability()).into(),
            tokens_label: format_tokens(session.tokens()).into(),
            cost_availability: availability_code(session.cost().availability()).into(),
            cost_label: format_cost(session.cost()).into(),
        })
        .collect::<Vec<_>>();
    window.set_dashboard_session_rows(model(rows));
}

fn apply_activity(window: &MainWindow, dashboard: &DesktopDashboardProjection) {
    let has_data = dashboard
        .section(DesktopDashboardSectionKey::Activity)
        .has_data();
    let maximum = has_data
        .then(|| dashboard.activity().iter().map(|row| row.count()).max())
        .flatten();
    let rows = dashboard
        .activity()
        .iter()
        .map(|row| DashboardActivityRow {
            key: row.key().stable_key().into(),
            label: activity_label(row.key()).into(),
            count_label: if has_data {
                format_integer(row.count()).into()
            } else {
                "—".into()
            },
            ratio: if has_data {
                ratio(Some(row.count()), maximum)
            } else {
                0.0
            },
        })
        .collect::<Vec<_>>();
    window.set_dashboard_activity_rows(model(rows));
}

fn apply_models(window: &MainWindow, dashboard: &DesktopDashboardProjection) {
    let maximum = dashboard
        .models()
        .iter()
        .filter_map(|row| row.tokens().known_sum())
        .max();
    let rows = dashboard
        .models()
        .iter()
        .map(|row| DashboardModelRow {
            ordinal: i32::from(row.ordinal()),
            label: row.model().into(),
            tokens_availability: availability_code(row.tokens().availability()).into(),
            tokens_label: format_tokens(row.tokens()).into(),
            tokens_ratio: ratio(row.tokens().known_sum(), maximum),
            cost_availability: availability_code(row.cost().availability()).into(),
            cost_label: format_cost(row.cost()).into(),
        })
        .collect::<Vec<_>>();
    window.set_dashboard_model_rows(model(rows));
}

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(rows))
}

const fn section_label(key: DesktopDashboardSectionKey) -> &'static str {
    match key {
        DesktopDashboardSectionKey::PlanUsage => "Plan Usage",
        DesktopDashboardSectionKey::CodeOutput => "Code Output",
        DesktopDashboardSectionKey::Trend => "Usage and Cost Trend",
        DesktopDashboardSectionKey::Sessions => "Sessions",
        DesktopDashboardSectionKey::Activity => "Activity",
        DesktopDashboardSectionKey::Models => "Model Usage",
    }
}

const fn activity_label(key: DesktopActivityKey) -> &'static str {
    match key {
        DesktopActivityKey::Read => "Read",
        DesktopActivityKey::EditWrite => "Edit / Write",
        DesktopActivityKey::Search => "Search",
        DesktopActivityKey::Git => "Git",
        DesktopActivityKey::BuildTest => "Build / Test",
        DesktopActivityKey::Web => "Web",
        DesktopActivityKey::Subagents => "Subagents",
        DesktopActivityKey::Terminal => "Terminal",
    }
}

const fn availability_code(value: DesktopValueAvailability) -> &'static str {
    match value {
        DesktopValueAvailability::Unavailable => "unavailable",
        DesktopValueAvailability::Known => "known",
        DesktopValueAvailability::Partial => "partial",
        DesktopValueAvailability::Complete => "complete",
        DesktopValueAvailability::LegitimateZero => "zero",
    }
}

fn format_tokens(value: DesktopTokenValue) -> String {
    match value.availability() {
        DesktopValueAvailability::Unavailable => "—".to_owned(),
        DesktopValueAvailability::Partial => value.known_sum().map_or_else(
            || "—".to_owned(),
            |known| {
                format!(
                    "{} ({}/{})",
                    format_integer(known),
                    format_integer(value.known_count()),
                    format_integer(value.event_count())
                )
            },
        ),
        DesktopValueAvailability::Known
        | DesktopValueAvailability::Complete
        | DesktopValueAvailability::LegitimateZero => value
            .known_sum()
            .map_or_else(|| "—".to_owned(), format_integer),
    }
}

fn format_cost(value: DesktopCostValue) -> String {
    value
        .micros()
        .map_or_else(|| "—".to_owned(), format_usd_micros)
}

fn format_optional_events(value: Option<u64>) -> String {
    value.map_or_else(
        || "—".to_owned(),
        |count| format_counted(count, "event", "events"),
    )
}

fn format_counted(value: u64, singular: &str, plural: &str) -> String {
    format!(
        "{} {}",
        format_integer(value),
        if value == 1 { singular } else { plural }
    )
}

fn format_integer(value: u64) -> String {
    format_unsigned(u128::from(value))
}

fn format_unsigned(value: u128) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len().saturating_add(digits.len() / 3));
    for (index, byte) in digits.bytes().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(char::from(byte));
    }
    result
}

fn format_signed(value: i128) -> String {
    if value > 0 {
        format!("+{}", format_unsigned(value.unsigned_abs()))
    } else if value < 0 {
        format!("−{}", format_unsigned(value.unsigned_abs()))
    } else {
        "0".to_owned()
    }
}

fn format_usd_micros(value: u64) -> String {
    let dollars = value / 1_000_000;
    let micros = value % 1_000_000;
    format!("${}.{:06}", format_integer(dollars), micros)
}

fn format_ratio(value: Option<u32>, kind: &str) -> String {
    value.map_or_else(
        || "—".to_owned(),
        |ppm| format!("{:.1}% {kind}", f64::from(ppm) / 10_000.0),
    )
}

fn ppm_ratio(value: u32) -> f32 {
    (f64::from(value) / 1_000_000.0) as f32
}

fn ratio(value: Option<u64>, maximum: Option<u64>) -> f32 {
    match value.zip(maximum) {
        Some((value, maximum)) if maximum > 0 => {
            ((value as f64) / (maximum as f64)).clamp(0.0, 1.0) as f32
        }
        _ => 0.0,
    }
}

fn format_quota_units(row: &crate::DesktopQuotaRow) -> String {
    let unit = row.unit_key().map_or("units", |value| value);
    match (
        row.used_units(),
        row.remaining_units(),
        row.capacity_units(),
    ) {
        (Some(used), _, Some(capacity)) => format!(
            "{} / {} {}",
            format_integer(used),
            format_integer(capacity),
            unit
        ),
        (_, Some(remaining), Some(capacity)) => format!(
            "{} / {} {} remaining",
            format_integer(remaining),
            format_integer(capacity),
            unit
        ),
        (Some(used), _, None) => format!("{} {} used", format_integer(used), unit),
        (_, Some(remaining), None) => {
            format!("{} {} remaining", format_integer(remaining), unit)
        }
        (_, _, Some(capacity)) => format!("{} {} capacity", format_integer(capacity), unit),
        (None, None, None) => String::new(),
    }
}

fn humanize_key(value: &str) -> String {
    let tail = value.rsplit('.').next().map_or(value, |part| part);
    let mut result = String::with_capacity(tail.len());
    let mut capitalize = true;
    for character in tail.chars() {
        if matches!(character, '_' | '-') {
            if !result.ends_with(' ') {
                result.push(' ');
            }
            capitalize = true;
        } else if capitalize {
            result.extend(character.to_uppercase());
            capitalize = false;
        } else {
            result.push(character);
        }
    }
    if result.is_empty() {
        "Quota window".to_owned()
    } else {
        result
    }
}

fn format_evidence(freshness: Option<DesktopFreshness>, quality: Option<DesktopQuality>) -> String {
    match freshness.zip(quality) {
        Some((freshness, quality)) => format!(
            "{} · {}",
            freshness_label(freshness),
            quality_label(quality)
        ),
        None => "Evidence unavailable".to_owned(),
    }
}

const fn freshness_label(value: DesktopFreshness) -> &'static str {
    match value {
        DesktopFreshness::Fresh => "Fresh",
        DesktopFreshness::Aging => "Aging",
        DesktopFreshness::Stale => "Stale",
        DesktopFreshness::Unavailable => "Unavailable",
    }
}

const fn quality_label(value: DesktopQuality) -> &'static str {
    match value {
        DesktopQuality::Authoritative => "Authoritative",
        DesktopQuality::Derived => "Derived",
        DesktopQuality::Estimated => "Estimated",
        DesktopQuality::Partial => "Partial",
        DesktopQuality::Conflict => "Conflict",
        DesktopQuality::Unknown => "Unknown",
    }
}

fn reminder_label(value: &str) -> &'static str {
    match value {
        "in_app_only" => "In-app reminders",
        "disabled" => "Reminders disabled",
        _ => "Reminder state unavailable",
    }
}

fn format_timestamp_ms(value: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(value).map_or_else(
        || "at an unknown time".to_owned(),
        |value| value.format("%Y-%m-%d %H:%M UTC").to_string(),
    )
}

fn format_timestamp_seconds(value: i64) -> String {
    DateTime::<Utc>::from_timestamp(value, 0).map_or_else(
        || "unknown".to_owned(),
        |value| value.format("%H:%M:%S").to_string(),
    )
}
