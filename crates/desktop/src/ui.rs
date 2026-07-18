use std::{
    cell::RefCell,
    fmt,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use chrono::{DateTime, Utc};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokenmaster_product::ProductSnapshot;

use crate::{
    DashboardActivityRow, DashboardBenefitRow, DashboardModelRow, DashboardQuotaRow,
    DashboardSectionRow, DashboardSessionRow, DashboardTrendPoint, DesktopActivityKey,
    DesktopCostValue, DesktopDashboardProjection, DesktopDashboardSectionKey, DesktopFreshness,
    DesktopIntent, DesktopIntentSink, DesktopOperationSnapshot, DesktopQuality,
    DesktopReliableStateProjection, DesktopSnapshotBridge, DesktopSnapshotReceiver,
    DesktopTokenValue, DesktopValueAvailability, MainWindow, RestorePointRow, RouteRow,
    UnavailableDesktopIntentSink,
    presentation::{DesktopApplyOutcome, DesktopProjection, DesktopRouteKey, DesktopState},
};

pub struct DesktopShell {
    window: MainWindow,
    state: SharedDesktopState,
    reliable_state: SharedReliableState,
}

pub(crate) type SharedDesktopState = Arc<Mutex<DesktopState>>;
type SharedReliableState = Arc<Mutex<DesktopReliableStateProjection>>;

#[derive(Clone)]
pub struct DesktopReliableStateNotifier {
    inner: Arc<ReliableStateNotifierInner>,
}

#[derive(Clone)]
pub struct DesktopBridgeFactory {
    window: slint::Weak<MainWindow>,
    state: SharedDesktopState,
}

impl DesktopBridgeFactory {
    #[must_use]
    pub fn snapshot_bridge(&self, receiver: DesktopSnapshotReceiver) -> DesktopSnapshotBridge {
        DesktopSnapshotBridge::new(self.window.clone(), Arc::clone(&self.state), receiver)
    }
}

struct ReliableStateNotifierInner {
    window: slint::Weak<MainWindow>,
    state: SharedReliableState,
    latest: Mutex<Option<DesktopReliableStateProjection>>,
    scheduled: AtomicBool,
    closed: AtomicBool,
}

impl DesktopReliableStateNotifier {
    pub fn publish(
        &self,
        projection: DesktopReliableStateProjection,
    ) -> Result<(), DesktopUiError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(DesktopUiError::state_unavailable());
        }
        *self
            .inner
            .latest
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())? = Some(projection);
        self.inner.request_delivery()
    }

    pub fn publish_operation(
        &self,
        operation: Option<DesktopOperationSnapshot>,
    ) -> Result<(), DesktopUiError> {
        {
            let mut latest = self
                .inner
                .latest
                .lock()
                .map_err(|_| DesktopUiError::state_unavailable())?;
            if let Some(projection) = latest.as_mut() {
                projection.set_operation(operation);
                drop(latest);
                return self.inner.request_delivery();
            }
        }
        let projection = self
            .inner
            .state
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?
            .clone()
            .with_operation(operation);
        self.publish(projection)
    }
}

impl ReliableStateNotifierInner {
    fn request_delivery(self: &Arc<Self>) -> Result<(), DesktopUiError> {
        if self.scheduled.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let inner = Arc::clone(self);
        if slint::invoke_from_event_loop(move || inner.deliver_latest()).is_err() {
            self.scheduled.store(false, Ordering::Release);
            return Err(DesktopUiError::state_unavailable());
        }
        Ok(())
    }

    fn deliver_latest(self: &Arc<Self>) {
        let projection = self.latest.lock().ok().and_then(|mut latest| latest.take());
        let delivered = projection.is_none_or(|projection| {
            let Some(window) = self.window.upgrade() else {
                return false;
            };
            let Ok(mut state) = self.state.lock() else {
                return false;
            };
            apply_reliable_state_projection(&window, &projection);
            *state = projection;
            true
        });
        if !delivered {
            self.closed.store(true, Ordering::Release);
        }
        self.scheduled.store(false, Ordering::Release);
        if !self.closed.load(Ordering::Acquire)
            && self.latest.lock().is_ok_and(|latest| latest.is_some())
        {
            let _ = self.request_delivery();
        }
    }
}

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
        Self::new_with_reliable_state_unbound(
            snapshot,
            DesktopReliableStateProjection::unavailable(),
        )
    }

    pub fn new_with_reliable_state_unbound(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
    ) -> Result<Self, slint::PlatformError> {
        Self::new_with_reliable_state(
            snapshot,
            reliable_state,
            Rc::new(UnavailableDesktopIntentSink),
        )
    }

    pub fn new_with_reliable_state(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
    ) -> Result<Self, slint::PlatformError> {
        let window = MainWindow::new()?;
        let initial_state = DesktopState::new(snapshot, DesktopRouteKey::Dashboard);
        apply_projection(&window, initial_state.projection());
        apply_reliable_state_projection(&window, &reliable_state);
        let state = Arc::new(Mutex::new(initial_state));
        let reliable_state = Arc::new(Mutex::new(reliable_state));
        wire_route_selection(&window, state.clone());
        wire_reliable_state_intents(&window, reliable_state.clone(), intent_sink);
        Ok(Self {
            window,
            state,
            reliable_state,
        })
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

    pub fn apply_reliable_state(
        &self,
        projection: DesktopReliableStateProjection,
    ) -> Result<(), DesktopUiError> {
        let mut reliable_state = self
            .reliable_state
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?;
        apply_reliable_state_projection(&self.window, &projection);
        *reliable_state = projection;
        Ok(())
    }

    pub(crate) fn state_handle(&self) -> SharedDesktopState {
        self.state.clone()
    }

    #[must_use]
    pub fn reliable_state_notifier(&self) -> DesktopReliableStateNotifier {
        DesktopReliableStateNotifier {
            inner: Arc::new(ReliableStateNotifierInner {
                window: self.window.as_weak(),
                state: Arc::clone(&self.reliable_state),
                latest: Mutex::new(None),
                scheduled: AtomicBool::new(false),
                closed: AtomicBool::new(false),
            }),
        }
    }

    #[must_use]
    pub fn snapshot_bridge(&self, receiver: DesktopSnapshotReceiver) -> DesktopSnapshotBridge {
        self.bridge_factory().snapshot_bridge(receiver)
    }

    #[must_use]
    pub fn bridge_factory(&self) -> DesktopBridgeFactory {
        DesktopBridgeFactory {
            window: self.window.as_weak(),
            state: self.state_handle(),
        }
    }
}

fn wire_reliable_state_intents(
    window: &MainWindow,
    reliable_state: SharedReliableState,
    intent_sink: Rc<dyn DesktopIntentSink>,
) {
    let reviewed_restore_selection = Rc::new(RefCell::new(None));
    let sink = intent_sink.clone();
    window.on_export_config(move || {
        let _ = sink.submit(DesktopIntent::ExportConfig);
    });
    let sink = intent_sink.clone();
    window.on_import_config(move || {
        let _ = sink.submit(DesktopIntent::ImportConfig);
    });
    let sink = intent_sink.clone();
    window.on_confirm_config_import(move || {
        let _ = sink.submit(DesktopIntent::ConfirmConfigImport);
    });
    let sink = intent_sink.clone();
    window.on_cancel_config_import(move || {
        let _ = sink.submit(DesktopIntent::CancelConfigImport);
    });
    let sink = intent_sink.clone();
    window.on_backup_normal(move || {
        let _ = sink.submit(DesktopIntent::BackupNormal);
    });
    let sink = intent_sink.clone();
    window.on_backup_compact(move || {
        let _ = sink.submit(DesktopIntent::BackupCompact);
    });
    let sink = intent_sink.clone();
    window.on_backup_encrypted(move |passphrase, confirmation| {
        if let Ok(intent) = DesktopIntent::encrypted_backup(&passphrase, &confirmation) {
            let _ = sink.submit(intent);
        }
    });
    let sink = intent_sink.clone();
    window.on_verify_backups(move || {
        let _ = sink.submit(DesktopIntent::VerifyBackups);
    });
    let sink = intent_sink.clone();
    let state = reliable_state.clone();
    let reviewed_selection = Rc::clone(&reviewed_restore_selection);
    let weak = window.as_weak();
    window.on_preview_restore(move |row| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let Ok(row) = usize::try_from(row) else {
            return;
        };
        let selection_and_point = state.lock().ok().and_then(|state| {
            Some((
                state.restore_selection(row)?,
                state.restore_points().get(row)?.clone(),
            ))
        });
        if let Some((selection, point)) = selection_and_point {
            let admission = sink.submit(DesktopIntent::PreviewRestore(selection));
            if admission != crate::DesktopIntentAdmission::Rejected {
                reviewed_selection.replace(Some(selection));
                window.set_restore_confirmation_visible(true);
                window.set_restore_confirmation_row(saturating_i32(row as u64));
                window.set_restore_confirmation_detail(
                    format!(
                        "{} · {} · {}",
                        format_restore_age(point.created_at_utc_ms()),
                        format_bytes(point.size_bytes()),
                        point.health().stable_code()
                    )
                    .into(),
                );
            }
        }
    });
    let sink = intent_sink.clone();
    let reviewed_selection = Rc::clone(&reviewed_restore_selection);
    let weak = window.as_weak();
    window.on_confirm_restore(move |_row, portable_settings| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let selection = *reviewed_selection.borrow();
        if let Some(selection) = selection {
            let admission = sink.submit(DesktopIntent::ConfirmRestore {
                selection,
                portable_settings,
            });
            if admission != crate::DesktopIntentAdmission::Rejected {
                reviewed_selection.replace(None);
                window.set_restore_confirmation_visible(false);
                window.set_restore_confirmation_row(-1);
                window.set_restore_confirmation_detail("".into());
            }
        }
    });
    let sink = intent_sink.clone();
    window.on_rebuild_data(move || {
        let _ = sink.submit(DesktopIntent::RebuildData);
    });
    let sink = intent_sink.clone();
    window.on_retry_operation(move || {
        let _ = sink.submit(DesktopIntent::RetryOperation);
    });
    let sink = intent_sink.clone();
    window.on_cancel_operation(move || {
        let _ = sink.submit(DesktopIntent::CancelOperation);
    });
    window.on_update_backup_policy(move |enabled, quiet, interval, budget| {
        let (Ok(quiet_seconds), Ok(interval_seconds), Ok(retention_budget_mib)) = (
            u32::try_from(quiet),
            u32::try_from(interval),
            u32::try_from(budget),
        ) else {
            return;
        };
        let _ = intent_sink.submit(DesktopIntent::UpdateBackupPolicy {
            periodic_enabled: enabled,
            quiet_seconds,
            interval_seconds,
            retention_budget_mib,
        });
    });
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

fn apply_reliable_state_projection(
    window: &MainWindow,
    projection: &DesktopReliableStateProjection,
) {
    window.set_reliable_state_generation(projection.generation().to_string().into());
    window.set_reliable_state_health(projection.health().stable_code().into());
    window.set_reliable_state_safe_mode(projection.safe_mode());
    let recovery_receipt = projection.recovery_receipt();
    window.set_reliable_recovery_kind(
        recovery_receipt
            .map_or("", |receipt| receipt.kind().stable_code())
            .into(),
    );
    window.set_reliable_non_reconstructible_domains_lost(
        recovery_receipt.is_some_and(|receipt| receipt.non_reconstructible_domains_lost()),
    );
    window.set_settings_health(projection.settings_health_code().into());
    let config_preview = projection.config_import_preview();
    window.set_config_import_preview_visible(config_preview.is_some());
    window.set_config_import_created_label(
        config_preview
            .map_or_else(
                || "Unavailable".to_owned(),
                |preview| format_timestamp_ms(preview.created_at_utc_ms()),
            )
            .into(),
    );
    window.set_config_import_bytes_label(
        config_preview
            .map_or_else(
                || "0 B".to_owned(),
                |preview| format_bytes(preview.package_bytes()),
            )
            .into(),
    );
    window.set_config_import_changes_label(
        config_preview
            .map_or_else(
                || "No pending changes".to_owned(),
                |preview| {
                    format!(
                        "{} categories · {} fields",
                        preview.changed_category_count(),
                        preview.changed_field_count()
                    )
                },
            )
            .into(),
    );
    window.set_reliable_latest_success_label(
        projection
            .latest_success_at_utc_ms()
            .map_or_else(|| "Unavailable".to_owned(), format_timestamp_ms)
            .into(),
    );
    window.set_reliable_latest_attempt_label(
        projection
            .latest_attempt_at_utc_ms()
            .map_or_else(|| "Unavailable".to_owned(), format_timestamp_ms)
            .into(),
    );
    window.set_reliable_successful_count_label(
        projection
            .successful_count()
            .map_or_else(|| "Unavailable".to_owned(), |count| count.to_string())
            .into(),
    );
    window.set_reliable_failure_count_label(
        projection
            .failure_count()
            .map_or_else(|| "Unavailable".to_owned(), |count| count.to_string())
            .into(),
    );
    window.set_reliable_published_bytes_label(
        projection
            .published_bytes()
            .map_or_else(|| "Unavailable".to_owned(), format_bytes)
            .into(),
    );
    window.set_reliable_latest_failure_code(
        projection.latest_failure_code().unwrap_or_default().into(),
    );
    let operation = projection.operation();
    window.set_reliable_operation_kind(
        operation
            .map_or("idle", |value| value.kind().stable_code())
            .into(),
    );
    window.set_reliable_operation_phase(
        operation
            .map_or("idle", |value| value.phase().stable_code())
            .into(),
    );
    window.set_reliable_operation_failure_code(
        operation
            .and_then(|value| value.failure_code())
            .unwrap_or_default()
            .into(),
    );
    window
        .set_reliable_operation_cancel_enabled(operation.is_some_and(|value| value.cancellable()));

    let policy = projection.policy();
    window.set_backup_periodic_enabled(policy.periodic_enabled());
    window.set_backup_quiet_seconds(saturating_i32(u64::from(policy.quiet_seconds())));
    window.set_backup_interval_seconds(saturating_i32(u64::from(policy.interval_seconds())));
    window.set_backup_retention_budget_mib(saturating_i32(
        policy.retention_budget_bytes() / 1_048_576,
    ));

    let rows = projection
        .restore_points()
        .iter()
        .enumerate()
        .map(|(index, point)| RestorePointRow {
            row_index: saturating_i32(index as u64),
            created_label: point
                .created_at_utc_ms()
                .map_or_else(|| "Time unavailable".to_owned(), format_timestamp_ms)
                .into(),
            size_label: format_bytes(point.size_bytes()).into(),
            health: point.health().stable_code().into(),
            purpose_label: humanize_key(point.purpose_code()).into(),
            schema_label: point
                .database_schema_version()
                .map_or_else(
                    || "Schema unavailable".to_owned(),
                    |version| format!("Schema {version}"),
                )
                .into(),
            compression_label: humanize_key(point.compression_code()).into(),
        })
        .collect::<Vec<_>>();
    window.set_restore_point_rows(model(rows));
}

fn saturating_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn format_bytes(value: u64) -> String {
    const MIB: u64 = 1_048_576;
    const KIB: u64 = 1_024;
    if value >= MIB {
        format!("{:.1} MiB", value as f64 / MIB as f64)
    } else if value >= KIB {
        format!("{:.1} KiB", value as f64 / KIB as f64)
    } else {
        format!("{value} B")
    }
}

fn format_restore_age(created_at_utc_ms: Option<i64>) -> String {
    let Some(created_at_utc_ms) = created_at_utc_ms else {
        return "Age unavailable".to_owned();
    };
    let now = Utc::now().timestamp_millis();
    let elapsed = now.saturating_sub(created_at_utc_ms).max(0) as u64;
    let hours = elapsed / 3_600_000;
    if hours < 24 {
        format!("{hours} h old")
    } else {
        format!("{} d old", hours / 24)
    }
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
