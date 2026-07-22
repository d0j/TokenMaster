use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use tokenmaster_product::ProductSnapshot;

use crate::{
    BenefitLotRow, DashboardActivityRow, DashboardBenefitRow, DashboardModelRow, DashboardQuotaRow,
    DashboardSectionRow, DashboardSessionRow, DashboardTrendPoint, DesktopActivityKey,
    DesktopActivityProjection, DesktopBenefitExpiry, DesktopCloseEffect, DesktopCostComposition,
    DesktopCostValue, DesktopCurrentUserStartupStatus, DesktopDashboardProjection,
    DesktopDashboardSectionKey, DesktopFreshness, DesktopHistoryProjection,
    DesktopHistoryRangeIntentSink, DesktopInAppNotificationBatch, DesktopInAppNotificationBridge,
    DesktopIntent, DesktopIntentSink, DesktopLifecycleIntentSink, DesktopModelsProjection,
    DesktopNotificationsProjection, DesktopOperationSnapshot, DesktopPresentationApplyOutcome,
    DesktopPresentationStyle, DesktopProjectsProjection, DesktopQuality,
    DesktopReliableStateProjection, DesktopReminderPolicy, DesktopRgb,
    DesktopSessionDetailIntentAdmission, DesktopSessionDetailIntentSink,
    DesktopSessionPageDirection, DesktopSessionPageIntentAdmission, DesktopSessionPageIntentSink,
    DesktopSessionsProjection, DesktopSnapshotBridge, DesktopSnapshotEpoch,
    DesktopSnapshotReceiver, DesktopTokenValue, DesktopTrayAvailability, DesktopValueAvailability,
    HistoryDayRow, InAppNotificationRow, MainWindow, ModelUsageRow, ProjectUsageRow,
    RecentActivityRow, ReminderCustomLeadRow, ReminderScopeRow, RestorePointRow, RouteRow,
    SessionDetailBreakdownRow, SessionListRow, UiPalette, UnavailableDesktopHistoryRangeIntentSink,
    UnavailableDesktopIntentSink, UnavailableDesktopSessionDetailIntentSink,
    UnavailableDesktopSessionPageIntentSink,
    in_app_notification::NotificationEpochState,
    native_tray::DesktopNativeTrayOwner,
    presentation::{DesktopApplyOutcome, DesktopProjection, DesktopRouteKey, DesktopState},
};

pub struct DesktopShell {
    window: MainWindow,
    _presentation_style: Arc<Mutex<DesktopPresentationStyle>>,
    _history_range_sink: Rc<dyn DesktopHistoryRangeIntentSink>,
    _session_page_sink: Rc<dyn DesktopSessionPageIntentSink>,
    tray: RefCell<Option<DesktopNativeTrayOwner>>,
    lifecycle_sink: Option<Rc<dyn DesktopLifecycleIntentSink>>,
    tray_availability: Rc<Cell<DesktopTrayAvailability>>,
    state: SharedDesktopState,
    reliable_state: SharedReliableState,
    snapshot_epochs: Arc<AtomicU64>,
    notification_epochs: Arc<NotificationEpochState>,
}

pub(crate) type SharedDesktopState = Arc<Mutex<DesktopState>>;
type SharedReliableState = Arc<Mutex<DesktopReliableStateProjection>>;
const VISIBLE_REMINDER_PUBLICATION_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_COMMAND_PALETTE_QUERY_SCALARS: usize = 64;
const COMPACT_WINDOW_WIDTH: f32 = 420.0;
const COMPACT_WINDOW_HEIGHT: f32 = 560.0;
const NORMAL_WINDOW_WIDTH: u32 = 1_120;
const NORMAL_WINDOW_HEIGHT: u32 = 720;

#[derive(Default)]
struct CompactWindowMode {
    active: bool,
    normal_size: Option<slint::PhysicalSize>,
}

type SharedCompactWindowMode = Rc<RefCell<CompactWindowMode>>;

struct ReliableStateDelivery {
    id: u64,
    projection: DesktopReliableStateProjection,
    presentation_terminal: Option<DesktopOperationSnapshot>,
    acknowledgement: Option<SyncSender<Result<(), DesktopUiError>>>,
}

#[derive(Clone)]
pub struct DesktopReliableStateNotifier {
    inner: Arc<ReliableStateNotifierInner>,
}

#[derive(Clone)]
pub struct DesktopCurrentUserStartupPresenter {
    window: slint::Weak<MainWindow>,
}

impl DesktopCurrentUserStartupPresenter {
    pub fn present(&self, status: DesktopCurrentUserStartupStatus) -> Result<(), DesktopUiError> {
        let window = self
            .window
            .upgrade()
            .ok_or_else(DesktopUiError::state_unavailable)?;
        apply_current_user_startup(&window, status);
        Ok(())
    }
}

#[derive(Clone)]
pub struct DesktopBridgeFactory {
    window: slint::Weak<MainWindow>,
    state: SharedDesktopState,
    snapshot_epochs: Arc<AtomicU64>,
    notification_epochs: Arc<NotificationEpochState>,
}

impl DesktopBridgeFactory {
    pub fn snapshot_bridge(
        &self,
        receiver: DesktopSnapshotReceiver,
    ) -> Result<DesktopSnapshotBridge, DesktopUiError> {
        let raw_epoch = self
            .snapshot_epochs
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                if current == 0 {
                    None
                } else {
                    Some(current.checked_add(1).unwrap_or(0))
                }
            })
            .map_err(|_| DesktopUiError::state_unavailable())?;
        let epoch =
            DesktopSnapshotEpoch::new(raw_epoch).ok_or_else(DesktopUiError::state_unavailable)?;
        Ok(DesktopSnapshotBridge::new(
            epoch,
            self.window.clone(),
            Arc::clone(&self.state),
            receiver,
        ))
    }

    pub fn in_app_notification_bridge(
        &self,
    ) -> Result<DesktopInAppNotificationBridge, DesktopUiError> {
        DesktopInAppNotificationBridge::new(
            Arc::clone(&self.notification_epochs),
            self.window.clone(),
        )
        .map_err(|_| DesktopUiError::state_unavailable())
    }
}

struct ReliableStateNotifierInner {
    window: slint::Weak<MainWindow>,
    state: SharedReliableState,
    presentation_style: Arc<Mutex<DesktopPresentationStyle>>,
    latest: Mutex<Option<ReliableStateDelivery>>,
    next_delivery_id: AtomicU64,
    scheduled: AtomicBool,
    closed: AtomicBool,
}

impl DesktopReliableStateNotifier {
    pub fn publish(
        &self,
        projection: DesktopReliableStateProjection,
    ) -> Result<(), DesktopUiError> {
        self.publish_delivery(projection, None)
    }

    fn publish_delivery(
        &self,
        projection: DesktopReliableStateProjection,
        acknowledgement: Option<SyncSender<Result<(), DesktopUiError>>>,
    ) -> Result<(), DesktopUiError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(DesktopUiError::state_unavailable());
        }
        let id = self
            .inner
            .next_delivery_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map_err(|_| DesktopUiError::state_unavailable())?;
        let mut latest = self
            .inner
            .latest
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?;
        let presentation_terminal =
            presentation_terminal_from_projection(&projection).or_else(|| {
                latest
                    .as_ref()
                    .and_then(|delivery| delivery.presentation_terminal)
            });
        let displaced = latest.replace(ReliableStateDelivery {
            id,
            projection,
            presentation_terminal,
            acknowledgement,
        });
        drop(latest);
        if let Some(acknowledgement) = displaced.and_then(|delivery| delivery.acknowledgement) {
            let _ = acknowledgement.send(Err(DesktopUiError::state_unavailable()));
        }
        if let Err(error) = self.inner.request_delivery() {
            let failed = self
                .inner
                .latest
                .lock()
                .map_err(|_| DesktopUiError::state_unavailable())?
                .take_if(|delivery| delivery.id == id);
            if let Some(acknowledgement) = failed.and_then(|delivery| delivery.acknowledgement) {
                let _ = acknowledgement.send(Err(error));
            }
            return Err(error);
        }
        Ok(())
    }

    fn publish_visible_reminder_projection(
        &self,
        projection: DesktopReliableStateProjection,
    ) -> Result<(), DesktopUiError> {
        let (sender, receiver) = sync_channel(1);
        self.publish_delivery(projection, Some(sender))?;
        receiver
            .recv_timeout(VISIBLE_REMINDER_PUBLICATION_TIMEOUT)
            .map_err(|_| DesktopUiError::state_unavailable())?
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
            if let Some(delivery) = latest.as_mut() {
                if delivery.acknowledgement.is_some() {
                    return Err(DesktopUiError::state_unavailable());
                }
                delivery.projection.set_operation(operation);
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

    pub fn publish_pending_reminder_policy(
        &self,
        reminder_policy: DesktopReminderPolicy,
        operation: DesktopOperationSnapshot,
    ) -> Result<(), DesktopUiError> {
        let latest = self
            .inner
            .latest
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?
            .as_ref()
            .map(|delivery| delivery.projection.clone());
        let projection = match latest {
            Some(projection) => projection,
            None => self
                .inner
                .state
                .lock()
                .map_err(|_| DesktopUiError::state_unavailable())?
                .clone(),
        }
        .with_reminder_policy(reminder_policy)
        .with_operation(Some(operation));
        self.publish_visible_reminder_projection(projection)
    }

    pub fn publish_pending_reminder_operation(
        &self,
        operation: DesktopOperationSnapshot,
    ) -> Result<(), DesktopUiError> {
        let latest = self
            .inner
            .latest
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?
            .as_ref()
            .map(|delivery| delivery.projection.clone());
        let projection = match latest {
            Some(projection) => projection,
            None => self
                .inner
                .state
                .lock()
                .map_err(|_| DesktopUiError::state_unavailable())?
                .clone(),
        };
        let current = projection.reminder_policy();
        let pending = DesktopReminderPolicy::new(
            current.enabled(),
            current.lead_seconds(),
            crate::DesktopReminderSyncState::Pending,
        )
        .ok_or_else(DesktopUiError::state_unavailable)?;
        self.publish_visible_reminder_projection(
            projection
                .with_reminder_policy(pending)
                .with_operation(Some(operation)),
        )
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
        let delivery = self.latest.lock().ok().and_then(|mut latest| latest.take());
        let delivered = delivery.as_ref().is_none_or(|delivery| {
            let Some(window) = self.window.upgrade() else {
                return false;
            };
            let Ok(style) = reconcile_presentation_style(
                &self.presentation_style,
                &delivery.projection,
                delivery.presentation_terminal,
            ) else {
                return false;
            };
            let Ok(mut state) = self.state.lock() else {
                return false;
            };
            *state = delivery.projection.clone();
            drop(state);
            apply_reliable_state_projection(&window, &delivery.projection);
            apply_presentation_style(&window, style);
            true
        });
        if let Some(acknowledgement) = delivery.and_then(|delivery| delivery.acknowledgement) {
            let _ = acknowledgement.send(if delivered {
                Ok(())
            } else {
                Err(DesktopUiError::state_unavailable())
            });
        }
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
        Self::new_with_reliable_state_and_session_sink(
            snapshot,
            reliable_state,
            intent_sink,
            Rc::new(UnavailableDesktopSessionDetailIntentSink),
        )
    }

    pub fn new_with_reliable_state_and_session_sink(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
        session_sink: Rc<dyn DesktopSessionDetailIntentSink>,
    ) -> Result<Self, slint::PlatformError> {
        Self::new_with_reliable_state_and_session_sinks(
            snapshot,
            reliable_state,
            intent_sink,
            session_sink,
            Rc::new(UnavailableDesktopSessionPageIntentSink),
        )
    }

    pub fn new_with_reliable_state_and_session_sinks(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
        session_sink: Rc<dyn DesktopSessionDetailIntentSink>,
        session_page_sink: Rc<dyn DesktopSessionPageIntentSink>,
    ) -> Result<Self, slint::PlatformError> {
        Self::new_with_optional_lifecycle_sink(
            snapshot,
            reliable_state,
            intent_sink,
            Rc::new(UnavailableDesktopHistoryRangeIntentSink),
            session_sink,
            session_page_sink,
            None,
        )
    }

    pub fn new_with_reliable_state_and_history_and_session_sinks(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
        history_range_sink: Rc<dyn DesktopHistoryRangeIntentSink>,
        session_sink: Rc<dyn DesktopSessionDetailIntentSink>,
        session_page_sink: Rc<dyn DesktopSessionPageIntentSink>,
    ) -> Result<Self, slint::PlatformError> {
        Self::new_with_optional_lifecycle_sink(
            snapshot,
            reliable_state,
            intent_sink,
            history_range_sink,
            session_sink,
            session_page_sink,
            None,
        )
    }

    pub fn new_with_reliable_state_and_all_sinks(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
        session_sink: Rc<dyn DesktopSessionDetailIntentSink>,
        lifecycle_sink: Rc<dyn DesktopLifecycleIntentSink>,
    ) -> Result<Self, slint::PlatformError> {
        Self::new_with_reliable_state_and_all_session_sinks(
            snapshot,
            reliable_state,
            intent_sink,
            session_sink,
            Rc::new(UnavailableDesktopSessionPageIntentSink),
            lifecycle_sink,
        )
    }

    pub fn new_with_reliable_state_and_all_session_sinks(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
        session_sink: Rc<dyn DesktopSessionDetailIntentSink>,
        session_page_sink: Rc<dyn DesktopSessionPageIntentSink>,
        lifecycle_sink: Rc<dyn DesktopLifecycleIntentSink>,
    ) -> Result<Self, slint::PlatformError> {
        Self::new_with_optional_lifecycle_sink(
            snapshot,
            reliable_state,
            intent_sink,
            Rc::new(UnavailableDesktopHistoryRangeIntentSink),
            session_sink,
            session_page_sink,
            Some(lifecycle_sink),
        )
    }

    pub fn new_with_reliable_state_and_all_history_and_session_sinks(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
        history_range_sink: Rc<dyn DesktopHistoryRangeIntentSink>,
        session_sink: Rc<dyn DesktopSessionDetailIntentSink>,
        session_page_sink: Rc<dyn DesktopSessionPageIntentSink>,
        lifecycle_sink: Rc<dyn DesktopLifecycleIntentSink>,
    ) -> Result<Self, slint::PlatformError> {
        Self::new_with_optional_lifecycle_sink(
            snapshot,
            reliable_state,
            intent_sink,
            history_range_sink,
            session_sink,
            session_page_sink,
            Some(lifecycle_sink),
        )
    }

    fn new_with_optional_lifecycle_sink(
        snapshot: &ProductSnapshot,
        reliable_state: DesktopReliableStateProjection,
        intent_sink: Rc<dyn DesktopIntentSink>,
        history_range_sink: Rc<dyn DesktopHistoryRangeIntentSink>,
        session_sink: Rc<dyn DesktopSessionDetailIntentSink>,
        session_page_sink: Rc<dyn DesktopSessionPageIntentSink>,
        lifecycle_sink: Option<Rc<dyn DesktopLifecycleIntentSink>>,
    ) -> Result<Self, slint::PlatformError> {
        let window = MainWindow::new()?;
        let initial_presentation_style =
            DesktopPresentationStyle::from_persisted(reliable_state.presentation().selection());
        let presentation_style = Arc::new(Mutex::new(initial_presentation_style));
        apply_presentation_style(&window, initial_presentation_style);
        let tray_availability = Rc::new(Cell::new(DesktopTrayAvailability::Unavailable));
        if lifecycle_sink.is_some() {
            wire_close_to_tray(&window, Rc::clone(&tray_availability));
        }
        window.set_help_product_version(env!("CARGO_PKG_VERSION").into());
        let initial_state = DesktopState::new(snapshot, DesktopRouteKey::Dashboard);
        apply_projection(&window, initial_state.projection());
        apply_reliable_state_projection(&window, &reliable_state);
        let state = Arc::new(Mutex::new(initial_state));
        let reliable_state = Arc::new(Mutex::new(reliable_state));
        let compact_window_mode = Rc::new(RefCell::new(CompactWindowMode::default()));
        wire_route_selection(&window, state.clone(), compact_window_mode.clone());
        wire_command_palette(&window, state.clone(), compact_window_mode);
        wire_reliable_state_intents(&window, reliable_state.clone(), Rc::clone(&intent_sink));
        wire_session_detail_intents(&window, state.clone(), session_sink);
        wire_session_page_intents(&window, state.clone(), session_page_sink.clone());
        wire_in_app_notification_dismissal(&window);
        wire_presentation_density(
            &window,
            Arc::clone(&presentation_style),
            intent_sink.clone(),
        );
        wire_presentation_skin(&window, Arc::clone(&presentation_style), intent_sink);
        Ok(Self {
            window,
            _presentation_style: presentation_style,
            _history_range_sink: history_range_sink,
            _session_page_sink: session_page_sink,
            tray: RefCell::new(None),
            lifecycle_sink,
            tray_availability,
            state,
            reliable_state,
            snapshot_epochs: Arc::new(AtomicU64::new(1)),
            notification_epochs: Arc::new(NotificationEpochState::new()),
        })
    }

    #[must_use]
    pub const fn window(&self) -> &MainWindow {
        &self.window
    }

    #[must_use]
    pub fn show_lifecycle_surface(&self) -> bool {
        let Some(lifecycle_sink) = self.lifecycle_sink.as_ref() else {
            return false;
        };
        let Ok(mut tray) = self.tray.try_borrow_mut() else {
            return false;
        };
        if tray.is_some() {
            return true;
        }
        match DesktopNativeTrayOwner::new(
            Rc::clone(lifecycle_sink),
            Rc::clone(&self.tray_availability),
        ) {
            Ok(owner) => {
                *tray = Some(owner);
                true
            }
            Err(_) => {
                self.tray_availability
                    .set(DesktopTrayAvailability::Unavailable);
                false
            }
        }
    }

    pub fn apply_snapshot(
        &self,
        snapshot: &ProductSnapshot,
    ) -> Result<DesktopApplyOutcome, DesktopUiError> {
        let (outcome, projection) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| DesktopUiError::state_unavailable())?;
            let outcome = state.apply_snapshot(snapshot);
            let projection =
                (outcome == DesktopApplyOutcome::Accepted).then(|| state.projection().clone());
            (outcome, projection)
        };
        if let Some(projection) = projection {
            apply_projection(&self.window, &projection);
            refresh_command_palette_if_open(&self.window, &projection);
        }
        Ok(outcome)
    }

    pub fn apply_snapshot_for_epoch(
        &self,
        epoch: DesktopSnapshotEpoch,
        snapshot: &ProductSnapshot,
    ) -> Result<DesktopApplyOutcome, DesktopUiError> {
        let (outcome, projection) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| DesktopUiError::state_unavailable())?;
            let outcome = state.apply_snapshot_for_epoch(epoch, snapshot);
            let projection =
                (outcome == DesktopApplyOutcome::Accepted).then(|| state.projection().clone());
            (outcome, projection)
        };
        if let Some(projection) = projection {
            apply_projection(&self.window, &projection);
            refresh_command_palette_if_open(&self.window, &projection);
        }
        Ok(outcome)
    }

    pub fn request_history_range(
        &self,
        preset: crate::DesktopHistoryRangePreset,
    ) -> Result<crate::DesktopHistoryRangeIntent, DesktopUiError> {
        let intent = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| DesktopUiError::state_unavailable())?;
            let intent = state
                .request_history_range(preset)
                .map_err(|_| DesktopUiError::state_unavailable())?;
            apply_history_projection(&self.window, state.projection().history());
            intent
        };
        Ok(intent)
    }

    pub fn history_range_state(
        &self,
    ) -> Result<(crate::DesktopHistoryRangePreset, bool), DesktopUiError> {
        let state = self
            .state
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?;
        let history = state.projection().history();
        Ok((history.range_preset(), history.range_pending()))
    }

    pub fn apply_reliable_state(
        &self,
        projection: DesktopReliableStateProjection,
    ) -> Result<(), DesktopUiError> {
        let style = reconcile_presentation_style(&self._presentation_style, &projection, None)?;
        let mut reliable_state = self
            .reliable_state
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?;
        *reliable_state = projection.clone();
        drop(reliable_state);
        apply_reliable_state_projection(&self.window, &projection);
        apply_presentation_style(&self.window, style);
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
                presentation_style: Arc::clone(&self._presentation_style),
                latest: Mutex::new(None),
                next_delivery_id: AtomicU64::new(1),
                scheduled: AtomicBool::new(false),
                closed: AtomicBool::new(false),
            }),
        }
    }

    #[must_use]
    pub fn current_user_startup_presenter(&self) -> DesktopCurrentUserStartupPresenter {
        DesktopCurrentUserStartupPresenter {
            window: self.window.as_weak(),
        }
    }

    pub fn snapshot_bridge(
        &self,
        receiver: DesktopSnapshotReceiver,
    ) -> Result<DesktopSnapshotBridge, DesktopUiError> {
        self.bridge_factory().snapshot_bridge(receiver)
    }

    #[must_use]
    pub fn bridge_factory(&self) -> DesktopBridgeFactory {
        DesktopBridgeFactory {
            window: self.window.as_weak(),
            state: self.state_handle(),
            snapshot_epochs: Arc::clone(&self.snapshot_epochs),
            notification_epochs: Arc::clone(&self.notification_epochs),
        }
    }
}

fn apply_presentation_style(window: &MainWindow, style: DesktopPresentationStyle) {
    window.set_presentation_palette(ui_palette(style.skin()));
    window.set_presentation_skin_id(style.skin().slint_index());
    window.set_presentation_density_id(style.density().slint_index());
    window.set_presentation_revision(style.revision().get().to_string().into());
    window.set_presentation_persistence_state(style.persistence().stable_code().into());
}

fn ui_palette(skin: crate::DesktopSkin) -> UiPalette {
    let tokens = skin.color_tokens();
    UiPalette {
        background: ui_color(tokens.background()),
        surface: ui_color(tokens.surface()),
        surface_raised: ui_color(tokens.surface_raised()),
        surface_subtle: ui_color(tokens.surface_subtle()),
        border: ui_color(tokens.border()),
        text_primary: ui_color(tokens.text_primary()),
        text_secondary: ui_color(tokens.text_secondary()),
        accent: ui_color(tokens.accent()),
        accent_subtle: ui_color(tokens.accent_subtle()),
        accent_secondary: ui_color(tokens.accent_secondary()),
        accent_tertiary: ui_color(tokens.accent_tertiary()),
        ready: ui_color(tokens.ready()),
        waiting: ui_color(tokens.waiting()),
        degraded: ui_color(tokens.degraded()),
        unavailable: ui_color(tokens.unavailable()),
    }
}

fn ui_color(color: DesktopRgb) -> Color {
    Color::from_rgb_u8(color.red(), color.green(), color.blue())
}

fn reconcile_presentation_style(
    presentation_style: &Arc<Mutex<DesktopPresentationStyle>>,
    projection: &DesktopReliableStateProjection,
    presentation_terminal: Option<DesktopOperationSnapshot>,
) -> Result<DesktopPresentationStyle, DesktopUiError> {
    let mut style = presentation_style
        .lock()
        .map_err(|_| DesktopUiError::state_unavailable())?;
    let projected_selection = projection.presentation().selection();
    match presentation_terminal.or_else(|| presentation_terminal_from_projection(projection)) {
        Some(operation)
            if matches!(
                operation.kind(),
                crate::DesktopOperationKind::ApplyConfig
                    | crate::DesktopOperationKind::RestoreWithPortableSettings
            ) && operation.phase() == crate::DesktopOperationPhase::Succeeded =>
        {
            style.apply_persisted_override(projected_selection);
        }
        Some(operation)
            if operation.kind() == crate::DesktopOperationKind::UpdatePresentation
                && matches!(
                    operation.phase(),
                    crate::DesktopOperationPhase::Failed | crate::DesktopOperationPhase::Cancelled
                ) =>
        {
            style.observe_persisted_unconfirmed(projected_selection);
            style.mark_not_saved();
        }
        Some(operation)
            if operation.kind() == crate::DesktopOperationKind::UpdatePresentation
                && operation.phase() == crate::DesktopOperationPhase::Succeeded =>
        {
            style.observe_persisted(projected_selection);
        }
        _ => {
            style.observe_persisted_unconfirmed(projected_selection);
        }
    }
    Ok(*style)
}

fn presentation_terminal_from_projection(
    projection: &DesktopReliableStateProjection,
) -> Option<DesktopOperationSnapshot> {
    let operation = projection.operation()?;
    match (operation.kind(), operation.phase()) {
        (
            crate::DesktopOperationKind::UpdatePresentation,
            crate::DesktopOperationPhase::Succeeded
            | crate::DesktopOperationPhase::Failed
            | crate::DesktopOperationPhase::Cancelled,
        )
        | (crate::DesktopOperationKind::ApplyConfig, crate::DesktopOperationPhase::Succeeded)
        | (
            crate::DesktopOperationKind::RestoreWithPortableSettings,
            crate::DesktopOperationPhase::Succeeded,
        ) => Some(operation),
        _ => None,
    }
}

fn wire_presentation_density(
    window: &MainWindow,
    presentation_style: Arc<Mutex<DesktopPresentationStyle>>,
    intent_sink: Rc<dyn DesktopIntentSink>,
) {
    let weak_window = window.as_weak();
    window.on_select_presentation_density(move |index| {
        let Some(next_style) =
            select_presentation_density_if_admitted(&presentation_style, index, &intent_sink)
        else {
            return;
        };
        if let Some(window) = weak_window.upgrade() {
            apply_presentation_style(&window, next_style);
        }
    });
}

fn wire_presentation_skin(
    window: &MainWindow,
    presentation_style: Arc<Mutex<DesktopPresentationStyle>>,
    intent_sink: Rc<dyn DesktopIntentSink>,
) {
    let weak_window = window.as_weak();
    window.on_select_presentation_skin(move |index| {
        let Some(next_style) =
            select_presentation_skin_if_admitted(&presentation_style, index, &intent_sink)
        else {
            return;
        };
        if let Some(window) = weak_window.upgrade() {
            apply_presentation_style(&window, next_style);
        }
    });
}

fn select_presentation_density_if_admitted(
    presentation_style: &Arc<Mutex<DesktopPresentationStyle>>,
    index: i32,
    intent_sink: &Rc<dyn DesktopIntentSink>,
) -> Option<DesktopPresentationStyle> {
    let captured = *presentation_style.lock().ok()?;
    let mut selected = captured;
    if selected.select_density_index_if_admitted(index, |selection| {
        matches!(
            intent_sink.submit(DesktopIntent::UpdatePresentation(selection)),
            crate::DesktopIntentAdmission::Started
                | crate::DesktopIntentAdmission::Queued
                | crate::DesktopIntentAdmission::Coalesced
        )
    }) != DesktopPresentationApplyOutcome::Applied
    {
        return None;
    }
    let mut current = presentation_style.lock().ok()?;
    if *current == captured {
        *current = selected;
    }
    Some(*current)
}

fn select_presentation_skin_if_admitted(
    presentation_style: &Arc<Mutex<DesktopPresentationStyle>>,
    index: i32,
    intent_sink: &Rc<dyn DesktopIntentSink>,
) -> Option<DesktopPresentationStyle> {
    let captured = *presentation_style.lock().ok()?;
    let mut selected = captured;
    if selected.select_skin_index_if_admitted(index, |selection| {
        matches!(
            intent_sink.submit(DesktopIntent::UpdatePresentation(selection)),
            crate::DesktopIntentAdmission::Started
                | crate::DesktopIntentAdmission::Queued
                | crate::DesktopIntentAdmission::Coalesced
        )
    }) != DesktopPresentationApplyOutcome::Applied
    {
        return None;
    }
    let mut current = presentation_style.lock().ok()?;
    if *current == captured {
        *current = selected;
    }
    Some(*current)
}

fn wire_in_app_notification_dismissal(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_dismiss_in_app_notifications(move || {
        if let Some(window) = weak.upgrade() {
            window.set_in_app_notification_visible(false);
            window.set_in_app_notification_count_label(SharedString::default());
            window.set_in_app_notification_rows(model(Vec::new()));
        }
    });
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
    let sink = intent_sink.clone();
    window.on_enable_current_user_startup(move || {
        let _ = sink.submit(DesktopIntent::EnableCurrentUserStartup);
    });
    let sink = intent_sink.clone();
    window.on_repair_current_user_startup(move || {
        let _ = sink.submit(DesktopIntent::RepairCurrentUserStartup);
    });
    let sink = intent_sink.clone();
    window.on_disable_current_user_startup(move || {
        let _ = sink.submit(DesktopIntent::DisableCurrentUserStartup);
    });
    let sink = intent_sink.clone();
    window.on_update_backup_policy(move |enabled, quiet, interval, budget| {
        let (Ok(quiet_seconds), Ok(interval_seconds), Ok(retention_budget_mib)) = (
            u32::try_from(quiet),
            u32::try_from(interval),
            u32::try_from(budget),
        ) else {
            return;
        };
        let _ = sink.submit(DesktopIntent::UpdateBackupPolicy {
            periodic_enabled: enabled,
            quiet_seconds,
            interval_seconds,
            retention_budget_mib,
        });
    });
    wire_reminder_policy_editor(window, intent_sink);
}

fn apply_current_user_startup(window: &MainWindow, status: DesktopCurrentUserStartupStatus) {
    window.set_current_user_startup_status(status.stable_code().into());
    window.set_current_user_startup_can_enable(matches!(
        status,
        DesktopCurrentUserStartupStatus::Disabled
    ));
    window.set_current_user_startup_can_disable(matches!(
        status,
        DesktopCurrentUserStartupStatus::EnabledVerified
            | DesktopCurrentUserStartupStatus::StaleRelocation
    ));
    window.set_current_user_startup_can_repair(matches!(
        status,
        DesktopCurrentUserStartupStatus::StaleRelocation
    ));
}

fn wire_reminder_policy_editor(window: &MainWindow, intent_sink: Rc<dyn DesktopIntentSink>) {
    let weak = window.as_weak();
    window.on_reminder_enabled_edited(move |enabled| {
        if let Some(window) = weak.upgrade() {
            window.set_reminder_enabled(enabled);
            mark_reminder_draft_dirty(&window);
        }
    });
    let weak = window.as_weak();
    window.on_reminder_preset_edited(move |index, enabled| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        match index {
            0 => window.set_reminder_preset_seven_days(enabled),
            1 => window.set_reminder_preset_twenty_four_hours(enabled),
            2 => window.set_reminder_preset_twelve_hours(enabled),
            3 => window.set_reminder_preset_six_hours(enabled),
            4 => window.set_reminder_preset_one_hour(enabled),
            _ => return,
        }
        mark_reminder_draft_dirty(&window);
    });
    let weak = window.as_weak();
    window.on_reminder_custom_lead_edited(move |index, enabled, value, unit_index| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let rows = window.get_reminder_custom_lead_rows();
        if index >= rows.row_count() {
            return;
        }
        rows.set_row_data(
            index,
            ReminderCustomLeadRow {
                enabled,
                value,
                unit_index,
            },
        );
        mark_reminder_draft_dirty(&window);
    });
    let weak = window.as_weak();
    window.on_reset_reminder_recommended(move || {
        if let Some(window) = weak.upgrade() {
            apply_recommended_reminder_draft(&window);
        }
    });
    let weak = window.as_weak();
    window.on_save_reminder_policy(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let Some(intent) = reminder_policy_intent(&window) else {
            window.set_reminder_feedback("Reminder profile is invalid".into());
            return;
        };
        match intent_sink.submit(intent) {
            crate::DesktopIntentAdmission::Rejected => {
                window.set_reminder_feedback("Reminder service is busy".into())
            }
            crate::DesktopIntentAdmission::Started
            | crate::DesktopIntentAdmission::Queued
            | crate::DesktopIntentAdmission::Coalesced => {
                window.set_reminder_dirty(false);
                window.set_reminder_feedback("Reminder profile submitted".into());
            }
        }
    });
}

fn mark_reminder_draft_dirty(window: &MainWindow) {
    window.set_reminder_dirty(true);
    window.set_reminder_feedback("Reminder draft changed".into());
}

fn apply_recommended_reminder_draft(window: &MainWindow) {
    window.set_reminder_enabled(true);
    window.set_reminder_preset_seven_days(true);
    window.set_reminder_preset_twenty_four_hours(true);
    window.set_reminder_preset_twelve_hours(true);
    window.set_reminder_preset_six_hours(true);
    window.set_reminder_preset_one_hour(true);
    window.set_reminder_custom_lead_rows(reminder_custom_rows(&[]));
    window.set_reminder_dirty(true);
    window.set_reminder_feedback("Recommended reminder draft ready".into());
}

fn reminder_policy_intent(window: &MainWindow) -> Option<DesktopIntent> {
    const PRESETS: [u32; 5] = [604_800, 86_400, 43_200, 21_600, 3_600];
    let preset_enabled = [
        window.get_reminder_preset_seven_days(),
        window.get_reminder_preset_twenty_four_hours(),
        window.get_reminder_preset_twelve_hours(),
        window.get_reminder_preset_six_hours(),
        window.get_reminder_preset_one_hour(),
    ];
    let mut leads = PRESETS
        .into_iter()
        .zip(preset_enabled)
        .filter_map(|(lead, enabled)| enabled.then_some(lead))
        .collect::<Vec<_>>();
    let rows = window.get_reminder_custom_lead_rows();
    if rows.row_count() != 8 {
        return None;
    }
    for index in 0..rows.row_count() {
        let row = rows.row_data(index)?;
        if !row.enabled {
            continue;
        }
        let unit = match row.unit_index {
            0 => 1_u32,
            1 => 60,
            2 => 3_600,
            3 => 86_400,
            _ => return None,
        };
        let value = u32::try_from(row.value).ok()?;
        let lead = value.checked_mul(unit)?;
        leads.push(lead);
    }
    leads.sort_unstable_by(|left, right| right.cmp(left));
    leads.dedup();
    if leads.len() > 8 {
        return None;
    }
    DesktopIntent::update_reminder_policy(window.get_reminder_enabled(), &leads).ok()
}

fn reminder_custom_rows(leads: &[u32]) -> ModelRc<ReminderCustomLeadRow> {
    let mut rows = Vec::with_capacity(8);
    for lead in leads.iter().copied().take(8) {
        let (value, unit_index) = if lead.is_multiple_of(86_400) {
            (lead / 86_400, 3)
        } else if lead.is_multiple_of(3_600) {
            (lead / 3_600, 2)
        } else if lead.is_multiple_of(60) {
            (lead / 60, 1)
        } else {
            (lead, 0)
        };
        rows.push(ReminderCustomLeadRow {
            enabled: true,
            value: saturating_i32(u64::from(value)),
            unit_index,
        });
    }
    rows.resize(
        8,
        ReminderCustomLeadRow {
            enabled: false,
            value: 1,
            unit_index: 0,
        },
    );
    model(rows)
}

fn wire_close_to_tray(window: &MainWindow, tray_availability: Rc<Cell<DesktopTrayAvailability>>) {
    window
        .window()
        .on_close_requested(move || match tray_availability.get().close_effect() {
            DesktopCloseEffect::HideToTray => slint::CloseRequestResponse::HideWindow,
            DesktopCloseEffect::Quit => {
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::HideWindow
            }
        });
}

fn wire_route_selection(
    window: &MainWindow,
    state: SharedDesktopState,
    compact_window_mode: SharedCompactWindowMode,
) {
    let weak = window.as_weak();
    window.on_select_route(move |key| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let projection = {
            let Ok(mut state) = state.lock() else {
                return;
            };
            if state.select_stable_key(key.as_str()).is_err() {
                return;
            }
            state.projection().clone()
        };
        apply_selected_route(&window, &projection, &compact_window_mode);
    });
}

fn wire_command_palette(
    window: &MainWindow,
    state: SharedDesktopState,
    compact_window_mode: SharedCompactWindowMode,
) {
    let weak = window.as_weak();
    let open_state = state.clone();
    window.on_open_command_palette(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let Ok(projection) = open_state.lock().map(|state| state.projection().clone()) else {
            return;
        };
        window.set_command_palette_visible(true);
        apply_command_palette_rows(&window, &projection, "", None);
    });

    let weak = window.as_weak();
    window.on_dismiss_command_palette(move || {
        if let Some(window) = weak.upgrade() {
            dismiss_command_palette(&window);
        }
    });

    let weak = window.as_weak();
    let query_state = state.clone();
    window.on_command_palette_query_edited(move |query| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let Ok(projection) = query_state.lock().map(|state| state.projection().clone()) else {
            return;
        };
        let query = truncate_command_palette_query(query.as_str());
        apply_command_palette_rows(&window, &projection, &query, None);
    });

    let weak = window.as_weak();
    window.on_move_command_palette_selection(move |delta| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        move_command_palette_selection(&window, delta);
    });

    let weak = window.as_weak();
    let activate_state = state.clone();
    let activate_compact_window_mode = compact_window_mode.clone();
    window.on_activate_command_palette_selection(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let selected = window.get_command_palette_selected_ordinal();
        let Ok(selected) = usize::try_from(selected) else {
            return;
        };
        let Some(row) = window.get_command_palette_rows().row_data(selected) else {
            return;
        };
        activate_command_palette_route(
            &window,
            &activate_state,
            &activate_compact_window_mode,
            row.key.as_str(),
        );
    });

    let weak = window.as_weak();
    window.on_activate_command_palette_route(move |key| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        activate_command_palette_route(&window, &state, &compact_window_mode, key.as_str());
    });
}

fn truncate_command_palette_query(value: &str) -> String {
    value
        .chars()
        .take(MAX_COMMAND_PALETTE_QUERY_SCALARS)
        .collect()
}

fn apply_command_palette_rows(
    window: &MainWindow,
    projection: &DesktopProjection,
    query: &str,
    selected_key: Option<&str>,
) {
    let query = truncate_command_palette_query(query);
    let normalized_query = query.to_lowercase();
    let mut rows = projection
        .routes()
        .iter()
        .filter(|route| {
            normalized_query.is_empty()
                || route
                    .key()
                    .stable_key()
                    .to_lowercase()
                    .contains(&normalized_query)
                || route
                    .key()
                    .english_label()
                    .to_lowercase()
                    .contains(&normalized_query)
        })
        .map(|route| RouteRow {
            key: route.key().stable_key().into(),
            label_key: route.key().label_key().into(),
            label: route.key().english_label().into(),
            state: route.state().stable_code().into(),
            reasons: join_reasons(route.reason_codes().iter()).into(),
            selected: false,
        })
        .collect::<Vec<_>>();
    let selected = selected_key
        .and_then(|key| rows.iter().position(|row| row.key.as_str() == key))
        .unwrap_or(0);
    if let Some(row) = rows.get_mut(selected) {
        row.selected = true;
    }
    window.set_command_palette_query(query.into());
    window.set_command_palette_selected_ordinal(if rows.is_empty() {
        -1
    } else {
        saturating_i32(selected as u64)
    });
    window.set_command_palette_rows(model(rows));
}

fn move_command_palette_selection(window: &MainWindow, delta: i32) {
    let mut rows = window.get_command_palette_rows().iter().collect::<Vec<_>>();
    let Ok(len) = i32::try_from(rows.len()) else {
        return;
    };
    if len == 0 {
        return;
    }
    let current = window
        .get_command_palette_selected_ordinal()
        .clamp(0, len - 1);
    let selected = (current + delta).rem_euclid(len);
    for (index, row) in rows.iter_mut().enumerate() {
        row.selected = index == selected as usize;
    }
    window.set_command_palette_selected_ordinal(selected);
    window.set_command_palette_rows(model(rows));
}

fn refresh_command_palette_if_open(window: &MainWindow, projection: &DesktopProjection) {
    if !window.get_command_palette_visible() {
        return;
    }
    let selected_key = usize::try_from(window.get_command_palette_selected_ordinal())
        .ok()
        .and_then(|index| window.get_command_palette_rows().row_data(index))
        .map(|row| row.key.to_string());
    let query = window.get_command_palette_query();
    apply_command_palette_rows(window, projection, query.as_str(), selected_key.as_deref());
}

fn activate_command_palette_route(
    window: &MainWindow,
    state: &SharedDesktopState,
    compact_window_mode: &SharedCompactWindowMode,
    key: &str,
) {
    let projection = {
        let Ok(mut state) = state.lock() else {
            return;
        };
        if state.select_stable_key(key).is_err() {
            return;
        }
        state.projection().clone()
    };
    apply_selected_route(window, &projection, compact_window_mode);
    dismiss_command_palette(window);
}

fn apply_selected_route(
    window: &MainWindow,
    projection: &DesktopProjection,
    compact_window_mode: &SharedCompactWindowMode,
) {
    apply_route_projection(window, projection);
    update_compact_window_mode(window, projection.selected(), compact_window_mode);
}

fn update_compact_window_mode(
    window: &MainWindow,
    selected: DesktopRouteKey,
    compact_window_mode: &SharedCompactWindowMode,
) {
    let target_size = {
        let mut mode = compact_window_mode.borrow_mut();
        if selected == DesktopRouteKey::CompactWidget {
            if mode.active {
                None
            } else {
                let current = window.window().size();
                mode.normal_size = Some(if current.width >= 560 && current.height >= 480 {
                    current
                } else {
                    slint::PhysicalSize::new(NORMAL_WINDOW_WIDTH, NORMAL_WINDOW_HEIGHT)
                });
                mode.active = true;
                Some(
                    slint::LogicalSize::new(COMPACT_WINDOW_WIDTH, COMPACT_WINDOW_HEIGHT)
                        .to_physical(window.window().scale_factor()),
                )
            }
        } else if mode.active {
            mode.active = false;
            Some(mode.normal_size.take().unwrap_or_else(|| {
                slint::PhysicalSize::new(NORMAL_WINDOW_WIDTH, NORMAL_WINDOW_HEIGHT)
            }))
        } else {
            None
        }
    };
    if let Some(size) = target_size {
        window.window().set_size(size);
    }
}

fn dismiss_command_palette(window: &MainWindow) {
    window.set_command_palette_visible(false);
    window.set_command_palette_query("".into());
    window.set_command_palette_selected_ordinal(-1);
    window.set_command_palette_rows(model(Vec::new()));
}

fn wire_session_detail_intents(
    window: &MainWindow,
    state: SharedDesktopState,
    sink: Rc<dyn DesktopSessionDetailIntentSink>,
) {
    let weak = window.as_weak();
    window.on_select_session(move |row| {
        let Ok(row) = usize::try_from(row) else {
            return;
        };
        let Some(window) = weak.upgrade() else {
            return;
        };
        let intent = {
            let Ok(mut state) = state.lock() else {
                return;
            };
            let Ok(intent) = state.select_session_row(row) else {
                return;
            };
            apply_session_detail_projection(&window, state.projection().sessions());
            intent
        };
        if sink.submit(intent) == DesktopSessionDetailIntentAdmission::Rejected {
            let Ok(mut state) = state.lock() else {
                return;
            };
            state.reject_session_detail(intent);
            apply_session_detail_projection(&window, state.projection().sessions());
        }
    });
}

fn wire_session_page_intents(
    window: &MainWindow,
    state: SharedDesktopState,
    sink: Rc<dyn DesktopSessionPageIntentSink>,
) {
    let bind = |direction| {
        let weak = window.as_weak();
        let state = state.clone();
        let sink = sink.clone();
        move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let intent = {
                let Ok(mut state) = state.lock() else {
                    return;
                };
                let Ok(intent) = state.request_session_page(direction) else {
                    return;
                };
                apply_session_navigation_projection(&window, state.projection().sessions());
                apply_session_detail_projection(&window, state.projection().sessions());
                intent
            };
            if sink.submit(intent) == DesktopSessionPageIntentAdmission::Rejected {
                let Ok(mut state) = state.lock() else {
                    return;
                };
                state.reject_session_page(intent);
                apply_session_navigation_projection(&window, state.projection().sessions());
                apply_session_detail_projection(&window, state.projection().sessions());
            }
        }
    };
    window.on_request_session_page_next(bind(DesktopSessionPageDirection::Next));
    window.on_request_session_page_newest(bind(DesktopSessionPageDirection::Newest));
}

pub(crate) fn apply_projection(window: &MainWindow, projection: &DesktopProjection) {
    apply_route_projection(window, projection);
    apply_dashboard_projection(window, projection.dashboard());
    apply_history_projection(window, projection.history());
    apply_models_projection(window, projection.models());
    apply_projects_projection(window, projection.projects());
    apply_activity_route_projection(window, projection.activity());
    apply_notifications_projection(window, projection.notifications());
    apply_sessions_projection(window, projection.sessions());
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

    let reminder = projection.reminder_policy();
    window.set_reminder_sync_state(
        match reminder.sync_state() {
            crate::DesktopReminderSyncState::Pending => "Pending",
            crate::DesktopReminderSyncState::Synchronized => "Synchronized",
            crate::DesktopReminderSyncState::Unavailable => "Unavailable",
        }
        .into(),
    );
    if !window.get_reminder_dirty() {
        window.set_reminder_enabled(reminder.enabled());
        let leads = reminder.lead_seconds();
        window.set_reminder_preset_seven_days(leads.contains(&604_800));
        window.set_reminder_preset_twenty_four_hours(leads.contains(&86_400));
        window.set_reminder_preset_twelve_hours(leads.contains(&43_200));
        window.set_reminder_preset_six_hours(leads.contains(&21_600));
        window.set_reminder_preset_one_hour(leads.contains(&3_600));
        let custom = leads
            .iter()
            .copied()
            .filter(|lead| ![604_800, 86_400, 43_200, 21_600, 3_600].contains(lead))
            .collect::<Vec<_>>();
        window.set_reminder_custom_lead_rows(reminder_custom_rows(&custom));
        window.set_reminder_feedback("Ready to edit reminder profile".into());
    }

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

pub(crate) fn apply_history_projection(window: &MainWindow, history: &DesktopHistoryProjection) {
    window.set_history_state(history.state().stable_code().into());
    window.set_history_reasons(join_reasons(history.reason_codes().iter()).into());
    window.set_history_range_label(format_history_range(history).into());
    window.set_history_time_zone_label(history.time_zone_id().unwrap_or("Unavailable").into());
    window
        .set_history_evidence_label(format_evidence(history.freshness(), history.quality()).into());
    window.set_history_input_tokens(format_tokens(history.input()).into());
    window.set_history_cached_tokens(format_tokens(history.cached()).into());
    window.set_history_output_tokens(format_tokens(history.output()).into());
    window.set_history_reasoning_tokens(format_tokens(history.reasoning()).into());
    window.set_history_total_tokens(format_tokens(history.total_tokens()).into());
    window.set_history_cost(format_cost(history.cost()).into());
    window.set_history_events(format_optional_events(history.event_count()).into());

    let rows = history
        .rows()
        .iter()
        .map(|row| {
            let (year, month, day) = row.date();
            HistoryDayRow {
                date_label: format_date(year, month, day).into(),
                event_label: format_integer(row.event_count()).into(),
                input_availability: availability_code(row.input().availability()).into(),
                input_label: format_tokens(row.input()).into(),
                cached_availability: availability_code(row.cached().availability()).into(),
                cached_label: format_tokens(row.cached()).into(),
                output_availability: availability_code(row.output().availability()).into(),
                output_label: format_tokens(row.output()).into(),
                reasoning_availability: availability_code(row.reasoning().availability()).into(),
                reasoning_label: format_tokens(row.reasoning()).into(),
                total_availability: availability_code(row.total_tokens().availability()).into(),
                total_label: format_tokens(row.total_tokens()).into(),
                cost_availability: availability_code(row.cost().availability()).into(),
                cost_label: format_cost(row.cost()).into(),
                token_ratio: ratio(row.total_tokens().known_sum(), history.token_maximum()),
                cost_ratio: ratio(row.cost().micros(), history.cost_maximum_micros()),
            }
        })
        .collect::<Vec<_>>();
    window.set_history_day_rows(model(rows));
}

fn format_history_range(history: &DesktopHistoryProjection) -> String {
    if let (Some(oldest), Some(newest)) = (history.rows().last(), history.rows().first()) {
        let (start_year, start_month, start_day) = oldest.date();
        let (end_year, end_month, end_day) = newest.date();
        return format!(
            "{} – {}",
            format_date(start_year, start_month, start_day),
            format_date(end_year, end_month, end_day)
        );
    }
    history.range().map_or_else(
        || "Range unavailable".to_owned(),
        |(start, end)| {
            format!(
                "{} – before {}",
                format_date(start.0, start.1, start.2),
                format_date(end.0, end.1, end.2)
            )
        },
    )
}

fn format_date(year: i16, month: u8, day: u8) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

fn apply_models_projection(window: &MainWindow, models: &DesktopModelsProjection) {
    window.set_models_state(models.state().stable_code().into());
    window.set_models_reasons(join_reasons(models.reason_codes().iter()).into());
    window.set_models_range_label(format_models_range(models).into());
    window.set_models_time_zone_label(models.time_zone_id().unwrap_or("Unavailable").into());
    window.set_models_evidence_label(format_evidence(models.freshness(), models.quality()).into());
    window.set_models_total_tokens(format_tokens(models.total_tokens()).into());
    window.set_models_total_availability(
        availability_code(models.total_tokens().availability()).into(),
    );
    window.set_models_cost(format_cost(models.cost()).into());
    window.set_models_cost_availability(availability_code(models.cost().availability()).into());
    window.set_models_cost_evidence_label(format_cost_evidence(models.cost()).into());
    window.set_models_events(format_optional_events(models.event_count()).into());
    window.set_models_loaded_label(
        models
            .event_count()
            .map_or_else(
                || "Unavailable".to_owned(),
                |_| format_counted(models.rows().len() as u64, "model loaded", "models loaded"),
            )
            .into(),
    );
    window.set_models_completeness_label(
        if models.event_count().is_none() {
            "Completeness unavailable"
        } else if models.truncated() {
            "More models available"
        } else {
            "Complete range"
        }
        .into(),
    );

    let rows = models
        .rows()
        .iter()
        .map(|row| ModelUsageRow {
            model_label: row.model().into(),
            event_label: format_integer(row.event_count()).into(),
            input_availability: availability_code(row.input().availability()).into(),
            input_label: format_tokens(row.input()).into(),
            cached_availability: availability_code(row.cached().availability()).into(),
            cached_label: format_tokens(row.cached()).into(),
            output_availability: availability_code(row.output().availability()).into(),
            output_label: format_tokens(row.output()).into(),
            reasoning_availability: availability_code(row.reasoning().availability()).into(),
            reasoning_label: format_tokens(row.reasoning()).into(),
            total_availability: availability_code(row.total_tokens().availability()).into(),
            total_label: format_tokens(row.total_tokens()).into(),
            cost_availability: availability_code(row.cost().availability()).into(),
            cost_label: format_cost(row.cost()).into(),
            cost_evidence_label: format_cost_evidence(row.cost()).into(),
            token_ratio: ratio(row.total_tokens().known_sum(), models.token_maximum()),
        })
        .collect::<Vec<_>>();
    window.set_model_usage_rows(model(rows));
}

fn format_models_range(models: &DesktopModelsProjection) -> String {
    models.range().map_or_else(
        || "Range unavailable".to_owned(),
        |(start, end)| {
            format!(
                "{} – before {}",
                format_date(start.0, start.1, start.2),
                format_date(end.0, end.1, end.2)
            )
        },
    )
}

fn apply_projects_projection(window: &MainWindow, projects: &DesktopProjectsProjection) {
    window.set_projects_state(projects.state().stable_code().into());
    window.set_projects_reasons(join_reasons(projects.reason_codes().iter()).into());
    window.set_projects_usage_range_label(format_optional_range(projects.usage_range()).into());
    window.set_projects_usage_time_zone_label(
        projects
            .usage_time_zone_id()
            .unwrap_or("Unavailable")
            .into(),
    );
    window.set_projects_usage_evidence_label(
        format_evidence(projects.usage_freshness(), projects.usage_quality()).into(),
    );
    window.set_projects_code_range_label(format_optional_range(projects.code_range()).into());
    window.set_projects_code_time_zone_label(
        projects.code_time_zone_id().unwrap_or("Unavailable").into(),
    );
    window.set_projects_code_evidence_label(
        format_evidence(projects.code_freshness(), projects.code_quality()).into(),
    );
    window.set_projects_total_tokens(format_tokens(projects.total_tokens()).into());
    window.set_projects_total_availability(
        availability_code(projects.total_tokens().availability()).into(),
    );
    window.set_projects_cost(format_cost(projects.cost()).into());
    window.set_projects_cost_availability(availability_code(projects.cost().availability()).into());
    window.set_projects_cost_evidence_label(format_cost_evidence(projects.cost()).into());
    window.set_projects_events(format_optional_events(projects.event_count()).into());
    window.set_projects_loaded_label(
        projects
            .event_count()
            .map_or_else(
                || "Unavailable".to_owned(),
                |_| {
                    format_counted(
                        projects.rows().len() as u64,
                        "project loaded",
                        "projects loaded",
                    )
                },
            )
            .into(),
    );
    window.set_projects_completeness_label(
        if projects.event_count().is_none() {
            "Completeness unavailable"
        } else if projects.usage_truncated() {
            "More projects available"
        } else {
            "Complete range"
        }
        .into(),
    );
    window.set_projects_code_coverage_label(
        projects
            .loaded_repository_count()
            .map_or_else(
                || "Repositories unavailable".to_owned(),
                |count| {
                    format_counted(u64::from(count), "repository loaded", "repositories loaded")
                },
            )
            .into(),
    );
    window.set_projects_code_completeness_label(
        if projects.loaded_repository_count().is_none() {
            "Code completeness unavailable"
        } else if projects.code_truncated() || !projects.code_complete() {
            "Incomplete code range"
        } else {
            "Complete code range"
        }
        .into(),
    );

    let rows = projects
        .rows()
        .iter()
        .map(|row| ProjectUsageRow {
            project_label: row.project().into(),
            unassociated: row.unassociated(),
            event_label: format_integer(row.event_count()).into(),
            input_availability: availability_code(row.input().availability()).into(),
            input_label: format_tokens(row.input()).into(),
            cached_availability: availability_code(row.cached().availability()).into(),
            cached_label: format_tokens(row.cached()).into(),
            output_availability: availability_code(row.output().availability()).into(),
            output_label: format_tokens(row.output()).into(),
            reasoning_availability: availability_code(row.reasoning().availability()).into(),
            reasoning_label: format_tokens(row.reasoning()).into(),
            total_availability: availability_code(row.total_tokens().availability()).into(),
            total_label: format_tokens(row.total_tokens()).into(),
            cost_availability: availability_code(row.cost().availability()).into(),
            cost_label: format_cost(row.cost()).into(),
            cost_evidence_label: format_cost_evidence(row.cost()).into(),
            token_ratio: ratio(row.total_tokens().known_sum(), projects.token_maximum()),
            code_available: row.code_available(),
            code_complete: row.code_complete(),
            code_status_label: format_project_code_status(row).into(),
            repository_label: format_project_repository_label(row).into(),
            commits_label: format_optional_integer(row.commits()).into(),
            added_label: format_optional_prefixed(row.added_lines(), "+").into(),
            removed_label: format_optional_prefixed(row.removed_lines(), "-").into(),
            net_label: row
                .net_lines()
                .map_or_else(|| "—".to_owned(), format_signed)
                .into(),
            efficiency_label: row
                .cost_per_100_added_lines_micros()
                .map_or_else(
                    || "—".to_owned(),
                    |value| {
                        format!(
                            "{} / 100 added product-code lines",
                            format_usd_micros(value)
                        )
                    },
                )
                .into(),
            efficiency_reason_label: row
                .efficiency_unavailable_reason()
                .map_or_else(String::new, humanize_key)
                .into(),
            code_evidence_label: format_evidence(row.code_freshness(), row.code_quality()).into(),
        })
        .collect::<Vec<_>>();
    window.set_project_usage_rows(model(rows));
}

fn format_optional_range(range: Option<crate::DesktopHistoryRange>) -> String {
    range.map_or_else(
        || "Range unavailable".to_owned(),
        |(start, end)| {
            format!(
                "{} – before {}",
                format_date(start.0, start.1, start.2),
                format_date(end.0, end.1, end.2)
            )
        },
    )
}

fn format_optional_integer(value: Option<u64>) -> String {
    value.map_or_else(|| "—".to_owned(), format_integer)
}

fn format_optional_prefixed(value: Option<u64>, prefix: &str) -> String {
    value.map_or_else(
        || "—".to_owned(),
        |value| format!("{prefix}{}", format_integer(value)),
    )
}

fn format_project_code_status(row: &crate::DesktopProjectUsageRow) -> String {
    if row.code_available() {
        if row.code_complete() {
            "Complete code".to_owned()
        } else {
            "Incomplete code".to_owned()
        }
    } else {
        row.efficiency_unavailable_reason()
            .map_or_else(|| "Code unavailable".to_owned(), format_project_reason)
    }
}

fn format_project_repository_label(row: &crate::DesktopProjectUsageRow) -> String {
    if row.code_available() {
        format_counted(
            u64::from(row.repository_count()),
            "repository",
            "repositories",
        )
    } else {
        row.efficiency_unavailable_reason().map_or_else(
            || "Repositories unavailable".to_owned(),
            format_project_reason,
        )
    }
}

fn format_project_reason(reason: &str) -> String {
    match reason {
        "git_unavailable" => "Git unavailable".to_owned(),
        "repository_not_linked" => "Not linked".to_owned(),
        "unassociated_project" => "Unassociated project".to_owned(),
        _ => humanize_key(reason),
    }
}

fn apply_activity_route_projection(window: &MainWindow, activity: &DesktopActivityProjection) {
    window.set_activity_state(activity.state().stable_code().into());
    window.set_activity_reasons(join_reasons(activity.reason_codes().iter()).into());
    window.set_activity_context_label("UTC timestamps".into());
    window.set_activity_page_available(activity.has_more().is_some());
    window.set_activity_evidence_label(
        format_evidence(activity.freshness(), activity.quality()).into(),
    );
    window.set_activity_loaded_label(
        activity
            .has_more()
            .map_or_else(
                || "Unavailable".to_owned(),
                |_| {
                    format_counted(
                        activity.rows().len() as u64,
                        "event loaded",
                        "events loaded",
                    )
                },
            )
            .into(),
    );
    window.set_activity_page_status_label(
        activity
            .has_more()
            .map_or("Page status unavailable", |has_more| {
                if has_more {
                    "More activity available"
                } else {
                    "First page complete"
                }
            })
            .into(),
    );
    let rows = activity
        .rows()
        .iter()
        .map(|row| RecentActivityRow {
            time_label: format_timestamp_utc(row.timestamp_seconds(), row.timestamp_nanos()).into(),
            model_label: row.model().into(),
            input_availability: availability_code(row.input().availability()).into(),
            input_label: format_tokens(row.input()).into(),
            cached_availability: availability_code(row.cached().availability()).into(),
            cached_label: format_tokens(row.cached()).into(),
            output_availability: availability_code(row.output().availability()).into(),
            output_label: format_tokens(row.output()).into(),
            reasoning_availability: availability_code(row.reasoning().availability()).into(),
            reasoning_label: format_tokens(row.reasoning()).into(),
            total_availability: availability_code(row.total_tokens().availability()).into(),
            total_label: format_tokens(row.total_tokens()).into(),
        })
        .collect::<Vec<_>>();
    window.set_recent_activity_rows(model(rows));
}

fn apply_notifications_projection(
    window: &MainWindow,
    notifications: &DesktopNotificationsProjection,
) {
    window.set_notifications_state(notifications.state().stable_code().into());
    window.set_notifications_reasons(join_reasons(notifications.reason_codes().iter()).into());
    window.set_notifications_evidence_label(
        format_evidence(notifications.freshness(), notifications.quality()).into(),
    );
    let state = notifications.state().stable_code();
    window.set_notifications_loaded_label(
        if state == "waiting" {
            "Waiting".to_owned()
        } else if state == "unavailable" {
            "Unavailable".to_owned()
        } else {
            format!(
                "{} · {}",
                format_counted(
                    notifications.scopes().len() as u64,
                    "reminder profile",
                    "reminder profiles",
                ),
                format_counted(
                    notifications.lots().len() as u64,
                    "current benefit",
                    "current benefits",
                ),
            )
        }
        .into(),
    );
    window.set_notifications_completeness_label(
        if state == "waiting" {
            "Waiting for benefit inventory"
        } else if state == "unavailable" {
            "Inventory unavailable"
        } else if notifications.scopes_truncated() || notifications.lots_truncated() {
            "Bounded inventory · more data omitted"
        } else if !notifications.reason_codes().is_empty() {
            "Current inventory · warnings present"
        } else if notifications.lots().is_empty() {
            "No current benefits"
        } else {
            "Current inventory complete"
        }
        .into(),
    );

    let scope_rows = notifications
        .scopes()
        .iter()
        .map(|scope| ReminderScopeRow {
            scope_label: format!("Scope {}", scope.ordinal()).into(),
            lot_count_label: format_counted(
                u64::from(scope.current_lot_count()),
                "benefit",
                "benefits",
            )
            .into(),
            coverage_label: notification_coverage_label(scope.reminder_coverage()).into(),
            source_label: humanize_code(scope.profile_source()).into(),
            leads_label: format_reminder_leads(scope.lead_seconds()).into(),
            next_due_label: scope
                .nearest_due_at_ms()
                .map_or_else(
                    || "Next reminder unavailable".to_owned(),
                    |value| format!("Next reminder {}", format_timestamp_ms(value)),
                )
                .into(),
            nearest_expiry_label: scope
                .nearest_expiry_at_ms()
                .map_or_else(
                    || "Nearest expiry unavailable".to_owned(),
                    |value| format!("Nearest expiry {}", format_timestamp_ms(value)),
                )
                .into(),
            evidence_label: format_evidence(Some(scope.freshness()), Some(scope.quality())).into(),
            warning_label: join_humanized_codes(scope.warning_codes().iter()).into(),
            completeness_label: humanize_code(scope.completeness()).into(),
        })
        .collect::<Vec<_>>();
    window.set_reminder_scope_rows(model(scope_rows));

    let lot_rows = notifications
        .lots()
        .iter()
        .map(|lot| BenefitLotRow {
            scope_label: format!("Scope {}", lot.scope_ordinal()).into(),
            benefit_label: humanize_key(lot.label_key()).into(),
            kind_label: humanize_code(lot.kind()).into(),
            quantity_label: format_integer(lot.quantity()).into(),
            state_label: humanize_code(lot.state()).into(),
            expiry_label: format_benefit_expiry(lot.expiry(), lot.state()).into(),
            granted_label: lot
                .granted_at_ms()
                .map_or_else(
                    || "Grant time unavailable".to_owned(),
                    |value| format!("Granted {}", format_timestamp_ms(value)),
                )
                .into(),
            evidence_label: format!(
                "{} · {} · {}",
                humanize_code(lot.evidence_source()),
                humanize_code(lot.confidence()),
                humanize_code(lot.detail_kind()),
            )
            .into(),
        })
        .collect::<Vec<_>>();
    window.set_benefit_lot_rows(model(lot_rows));
}

pub(crate) fn apply_in_app_notification_batch(
    window: &MainWindow,
    batch: &DesktopInAppNotificationBatch,
) -> bool {
    let rows = batch
        .rows()
        .iter()
        .map(|notification| {
            let benefit_label = humanize_key(notification.label_key());
            let kind_label = notification.kind().display_label();
            let quantity_label = format_integer(notification.quantity());
            let lead_label = format!(
                "Reminder due {} before expiry",
                format_reminder_lead(notification.lead_seconds())
            );
            let due_label = format!(
                "Due {}",
                format_precise_timestamp_ms(notification.due_at_ms())
            );
            let expiry_label = format!(
                "Expires {}",
                format_precise_timestamp_ms(notification.expiry_at_ms())
            );
            let delivered_label = format!(
                "Queued {}",
                format_precise_timestamp_ms(notification.delivered_at_ms())
            );
            let accessible_label = format!(
                "{benefit_label}. {kind_label}, quantity {quantity_label}. {lead_label}. {due_label}. {expiry_label}. {delivered_label}."
            );
            InAppNotificationRow {
                benefit_label: benefit_label.into(),
                kind_label: kind_label.into(),
                quantity_label: quantity_label.into(),
                lead_label: lead_label.into(),
                due_label: due_label.into(),
                expiry_label: expiry_label.into(),
                delivered_label: delivered_label.into(),
                accessible_label: accessible_label.into(),
            }
        })
        .collect::<Vec<_>>();
    let count_label = if batch.len() == 1 {
        "1 expiry reminder".to_owned()
    } else {
        format!("{} expiry reminders", batch.len())
    };
    window.set_in_app_notification_rows(model(rows));
    window.set_in_app_notification_count_label(count_label.into());
    window.set_in_app_notification_visible(true);
    window.get_in_app_notification_visible()
        && window.get_in_app_notification_rows().row_count() == batch.len()
}

fn notification_coverage_label(value: &str) -> &'static str {
    match value {
        "in_app_only" => "In-app only",
        "disabled" => "Disabled",
        _ => "Unavailable",
    }
}

fn format_reminder_leads(values: &[u32]) -> String {
    if values.is_empty() {
        return "Disabled".to_owned();
    }
    values
        .iter()
        .map(|seconds| format_reminder_lead(*seconds))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn format_reminder_lead(seconds: u32) -> String {
    const DAY: u32 = 86_400;
    const HOUR: u32 = 3_600;
    const MINUTE: u32 = 60;
    if seconds >= 2 * DAY && seconds.is_multiple_of(DAY) {
        format!("{}d", seconds / DAY)
    } else if seconds.is_multiple_of(HOUR) {
        format!("{}h", seconds / HOUR)
    } else if seconds.is_multiple_of(MINUTE) {
        format!("{}m", seconds / MINUTE)
    } else {
        format!("{seconds}s")
    }
}

fn join_humanized_codes<'a>(codes: impl Iterator<Item = &'a str>) -> String {
    codes.map(humanize_code).collect::<Vec<_>>().join(" · ")
}

fn humanize_code(value: &str) -> String {
    let mut result = value.replace(['_', '-'], " ");
    if let Some(first) = result.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    result
}

fn format_benefit_expiry(expiry: &DesktopBenefitExpiry, state: &str) -> String {
    let prefix = if state == "expired" {
        "Expired"
    } else {
        "Expires"
    };
    match expiry {
        DesktopBenefitExpiry::ExactUtc { at_ms } => {
            format!("{prefix} {}", format_precise_timestamp_ms(*at_ms))
        }
        DesktopBenefitExpiry::BoundedUtc {
            earliest_at_ms,
            latest_at_ms,
        } => format!(
            "{prefix} between {} and {}",
            format_precise_timestamp_ms(*earliest_at_ms),
            format_precise_timestamp_ms(*latest_at_ms),
        ),
        DesktopBenefitExpiry::ProviderLocal {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
            time_zone,
        } => format!(
            "{prefix} {:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {} (provider local)",
            year, month, day, hour, minute, second, millisecond, time_zone,
        ),
        DesktopBenefitExpiry::ProviderDate {
            year,
            month,
            day,
            time_zone,
        } => format!(
            "{prefix} {:04}-{:02}-{:02}{} (provider date)",
            year,
            month,
            day,
            time_zone
                .as_deref()
                .map_or_else(String::new, |zone| format!(" {zone}")),
        ),
        DesktopBenefitExpiry::Unknown => "Expiry unknown".to_owned(),
    }
}

fn apply_sessions_projection(window: &MainWindow, sessions: &DesktopSessionsProjection) {
    window.set_sessions_state(sessions.state().stable_code().into());
    window.set_sessions_reasons(join_reasons(sessions.reason_codes().iter()).into());
    window.set_sessions_evidence_label(
        format_evidence(sessions.freshness(), sessions.quality()).into(),
    );
    window.set_sessions_loaded_label(
        sessions
            .has_more()
            .map_or_else(
                || "Unavailable".to_owned(),
                |_| format!("{} loaded", format_integer(sessions.rows().len() as u64)),
            )
            .into(),
    );
    apply_session_navigation_projection(window, sessions);

    let rows = sessions
        .rows()
        .iter()
        .enumerate()
        .map(|(index, session)| SessionListRow {
            row_index: i32::try_from(index).unwrap_or(i32::MAX),
            first_label: format_timestamp_seconds_utc(session.first_timestamp_seconds()).into(),
            last_label: format_timestamp_seconds_utc(session.last_timestamp_seconds()).into(),
            duration_label: format_session_duration(
                session.first_timestamp_seconds(),
                session.first_timestamp_nanos(),
                session.last_timestamp_seconds(),
                session.last_timestamp_nanos(),
            )
            .into(),
            event_label: format_integer(session.event_count()).into(),
            input_availability: availability_code(session.input().availability()).into(),
            input_label: format_tokens(session.input()).into(),
            cached_availability: availability_code(session.cached().availability()).into(),
            cached_label: format_tokens(session.cached()).into(),
            output_availability: availability_code(session.output().availability()).into(),
            output_label: format_tokens(session.output()).into(),
            reasoning_availability: availability_code(session.reasoning().availability()).into(),
            reasoning_label: format_tokens(session.reasoning()).into(),
            total_availability: availability_code(session.total_tokens().availability()).into(),
            total_label: format_tokens(session.total_tokens()).into(),
            cost_availability: availability_code(session.cost().availability()).into(),
            cost_label: format_cost(session.cost()).into(),
        })
        .collect::<Vec<_>>();
    window.set_session_list_rows(model(rows));
    apply_session_detail_projection(window, sessions);
}

pub(crate) fn apply_session_navigation_projection(
    window: &MainWindow,
    sessions: &DesktopSessionsProjection,
) {
    let pending = sessions.navigation_pending();
    let continuation = sessions.page_kind() == Some(crate::DesktopSessionPageKind::Continuation);
    let retained_unavailable_page = sessions.state() != crate::DesktopDashboardSectionState::Ready
        && sessions.page_kind().is_some();
    let next_enabled = !pending
        && sessions.state() == crate::DesktopDashboardSectionState::Ready
        && sessions.has_more() == Some(true);
    window.set_sessions_navigation_pending(pending);
    window.set_sessions_next_enabled(next_enabled);
    window.set_sessions_back_to_newest_enabled(
        !pending && (continuation || retained_unavailable_page),
    );
    let status = if pending {
        "Loading sessions…"
    } else {
        match (sessions.page_kind(), sessions.has_more()) {
            (Some(crate::DesktopSessionPageKind::Newest), Some(true)) => {
                "Newest page · More sessions available"
            }
            (Some(crate::DesktopSessionPageKind::Newest), Some(false)) => {
                "Newest page · All sessions loaded"
            }
            (Some(crate::DesktopSessionPageKind::Continuation), Some(true)) => {
                "Older sessions · More sessions available"
            }
            (Some(crate::DesktopSessionPageKind::Continuation), Some(false)) => {
                "Older sessions · Oldest sessions loaded"
            }
            (Some(crate::DesktopSessionPageKind::Newest), None) => {
                "Newest page · Status unavailable"
            }
            (Some(crate::DesktopSessionPageKind::Continuation), None) => {
                "Older sessions · Status unavailable"
            }
            (None, _) => "Page status unavailable",
        }
    };
    window.set_sessions_page_status_label(status.into());
}

pub(crate) fn apply_session_detail_projection(
    window: &MainWindow,
    sessions: &DesktopSessionsProjection,
) {
    let detail = sessions.detail();
    window.set_sessions_selected_row(detail.selected_ordinal().map_or(-1, i32::from));
    window.set_session_detail_state(detail.state().stable_code().into());
    window.set_session_detail_evidence_label(
        format_evidence(detail.freshness(), detail.quality()).into(),
    );
    let status = match detail.state() {
        crate::DesktopSessionDetailState::Idle => "No selection".to_owned(),
        crate::DesktopSessionDetailState::Loading => "Loading".to_owned(),
        crate::DesktopSessionDetailState::Ready if detail.truncated() => {
            "Ready · breakdown limited to 32 models and 32 projects".to_owned()
        }
        crate::DesktopSessionDetailState::Ready => "Ready".to_owned(),
        crate::DesktopSessionDetailState::Missing => "Not found".to_owned(),
        crate::DesktopSessionDetailState::Unavailable => detail.failure_code().map_or_else(
            || "Unavailable".to_owned(),
            |code| format!("Unavailable · {}", humanize_key(code)),
        ),
    };
    window.set_session_detail_status_label(status.into());
    if let Some(summary) = detail.summary() {
        window.set_session_detail_period_label(
            format!(
                "{} → {}",
                format_timestamp_seconds_utc(summary.first_timestamp_seconds()),
                format_timestamp_seconds_utc(summary.last_timestamp_seconds())
            )
            .into(),
        );
        window.set_session_detail_duration_label(
            format_session_duration(
                summary.first_timestamp_seconds(),
                summary.first_timestamp_nanos(),
                summary.last_timestamp_seconds(),
                summary.last_timestamp_nanos(),
            )
            .into(),
        );
        window.set_session_detail_event_label(format_integer(summary.event_count()).into());
        window.set_session_detail_input_availability(
            availability_code(summary.input().availability()).into(),
        );
        window.set_session_detail_input_label(format_tokens(summary.input()).into());
        window.set_session_detail_cached_availability(
            availability_code(summary.cached().availability()).into(),
        );
        window.set_session_detail_cached_label(format_tokens(summary.cached()).into());
        window.set_session_detail_output_availability(
            availability_code(summary.output().availability()).into(),
        );
        window.set_session_detail_output_label(format_tokens(summary.output()).into());
        window.set_session_detail_reasoning_availability(
            availability_code(summary.reasoning().availability()).into(),
        );
        window.set_session_detail_reasoning_label(format_tokens(summary.reasoning()).into());
        window.set_session_detail_total_availability(
            availability_code(summary.total_tokens().availability()).into(),
        );
        window.set_session_detail_total_label(format_tokens(summary.total_tokens()).into());
        window.set_session_detail_cost_availability(
            availability_code(summary.cost().availability()).into(),
        );
        window.set_session_detail_cost_label(format_cost(summary.cost()).into());
    } else {
        window.set_session_detail_period_label("".into());
        window.set_session_detail_duration_label("".into());
        window.set_session_detail_event_label("".into());
        window.set_session_detail_input_availability("unavailable".into());
        window.set_session_detail_input_label("Unavailable".into());
        window.set_session_detail_cached_availability("unavailable".into());
        window.set_session_detail_cached_label("Unavailable".into());
        window.set_session_detail_output_availability("unavailable".into());
        window.set_session_detail_output_label("Unavailable".into());
        window.set_session_detail_reasoning_availability("unavailable".into());
        window.set_session_detail_reasoning_label("Unavailable".into());
        window.set_session_detail_total_availability("unavailable".into());
        window.set_session_detail_total_label("Unavailable".into());
        window.set_session_detail_cost_availability("unavailable".into());
        window.set_session_detail_cost_label("Unavailable".into());
    }
    let rows = detail
        .breakdown_rows()
        .iter()
        .map(|row| SessionDetailBreakdownRow {
            kind: row.kind().stable_code().into(),
            label: row.label().into(),
            event_label: format_integer(row.event_count()).into(),
            input_availability: availability_code(row.input().availability()).into(),
            input_label: format_tokens(row.input()).into(),
            cached_availability: availability_code(row.cached().availability()).into(),
            cached_label: format_tokens(row.cached()).into(),
            output_availability: availability_code(row.output().availability()).into(),
            output_label: format_tokens(row.output()).into(),
            reasoning_availability: availability_code(row.reasoning().availability()).into(),
            reasoning_label: format_tokens(row.reasoning()).into(),
            total_availability: availability_code(row.total_tokens().availability()).into(),
            total_label: format_tokens(row.total_tokens()).into(),
            cost_availability: availability_code(row.cost().availability()).into(),
            cost_label: format_cost(row.cost()).into(),
        })
        .collect::<Vec<_>>();
    window.set_session_detail_breakdown_rows(model(rows));
}

fn format_session_duration(
    first_seconds: i64,
    first_nanos: u32,
    last_seconds: i64,
    last_nanos: u32,
) -> String {
    const NANOS_PER_SECOND: i128 = 1_000_000_000;
    if first_nanos >= NANOS_PER_SECOND as u32 || last_nanos >= NANOS_PER_SECOND as u32 {
        return "Unavailable".to_owned();
    }
    let duration_nanos = (i128::from(last_seconds) - i128::from(first_seconds)) * NANOS_PER_SECOND
        + i128::from(last_nanos)
        - i128::from(first_nanos);
    if duration_nanos < 0 {
        return "Unavailable".to_owned();
    }
    if duration_nanos == 0 {
        return "0s".to_owned();
    }
    if duration_nanos < NANOS_PER_SECOND {
        return "<1s".to_owned();
    }
    let seconds = u64::try_from(duration_nanos / NANOS_PER_SECOND).unwrap_or(u64::MAX);
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
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

fn format_cost_evidence(value: DesktopCostValue) -> String {
    let availability = match value.availability() {
        DesktopValueAvailability::Unavailable => "Unavailable",
        DesktopValueAvailability::Known => "Known",
        DesktopValueAvailability::Partial => "Partial",
        DesktopValueAvailability::Complete => "Complete",
        DesktopValueAvailability::LegitimateZero => "Zero",
    };
    let provenance = match value.composition() {
        None | Some(DesktopCostComposition::None) => None,
        Some(DesktopCostComposition::Calculated) => Some("calculated"),
        Some(DesktopCostComposition::Reported) => Some("reported"),
        Some(DesktopCostComposition::Mixed) => Some("mixed"),
    };
    provenance.map_or_else(
        || availability.to_owned(),
        |provenance| format!("{availability} · {provenance}"),
    )
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

fn format_precise_timestamp_ms(value: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(value).map_or_else(
        || "at an unknown time".to_owned(),
        |value| value.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string(),
    )
}

fn format_timestamp_seconds(value: i64) -> String {
    DateTime::<Utc>::from_timestamp(value, 0).map_or_else(
        || "unknown".to_owned(),
        |value| value.format("%H:%M:%S").to_string(),
    )
}

fn format_timestamp_seconds_utc(value: i64) -> String {
    DateTime::<Utc>::from_timestamp(value, 0).map_or_else(
        || "Unavailable".to_owned(),
        |value| value.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    )
}

fn format_timestamp_utc(seconds: i64, nanos: u32) -> String {
    DateTime::<Utc>::from_timestamp(seconds, nanos).map_or_else(
        || "Unavailable".to_owned(),
        |value| {
            let base = value.format("%Y-%m-%d %H:%M:%S");
            if nanos == 0 {
                format!("{base} UTC")
            } else {
                let fraction = format!("{nanos:09}");
                format!("{base}.{} UTC", fraction.trim_end_matches('0'))
            }
        },
    )
}

#[cfg(test)]
mod duration_tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::mpsc::sync_channel;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{
        DesktopReliableStateProjection, DesktopShell, MainWindow, apply_presentation_style,
        format_benefit_expiry, format_session_duration, format_timestamp_utc,
        wire_presentation_density,
    };
    use crate::{
        DesktopBackupPolicy, DesktopBenefitExpiry, DesktopDensity, DesktopIntent,
        DesktopIntentAdmission, DesktopIntentSink, DesktopOperationKind, DesktopOperationPhase,
        DesktopOperationSnapshot, DesktopPresentationSelection, DesktopPresentationSettings,
        DesktopPresentationStyle, DesktopReliableStateHealth, DesktopReliableStateInput,
        DesktopReliableStateSummary, DesktopReminderPolicy, DesktopReminderSyncState,
        DesktopSessionPageIntent, DesktopSessionPageIntentAdmission, DesktopSessionPageIntentSink,
        DesktopSkin, UnavailableDesktopIntentSink, UnavailableDesktopSessionDetailIntentSink,
        UnavailableDesktopSessionPageIntentSink,
    };
    use tokenmaster_product::ProductReducer;

    struct ReentrantPresentationSink {
        style: Arc<Mutex<DesktopPresentationStyle>>,
        acquired_style: Cell<bool>,
        submissions: Cell<u64>,
    }

    struct AcceptingPresentationSink;

    struct RetainingSessionPageSink {
        _token: Rc<()>,
    }

    impl DesktopSessionPageIntentSink for RetainingSessionPageSink {
        fn submit(&self, _intent: DesktopSessionPageIntent) -> DesktopSessionPageIntentAdmission {
            DesktopSessionPageIntentAdmission::Rejected
        }
    }

    impl DesktopIntentSink for AcceptingPresentationSink {
        fn submit(&self, _: DesktopIntent) -> DesktopIntentAdmission {
            DesktopIntentAdmission::Started
        }
    }

    #[test]
    fn shell_retains_the_supplied_session_page_sink_for_its_lifetime() -> Result<(), String> {
        i_slint_backend_testing::init_no_event_loop();
        let token = Rc::new(());
        let weak = Rc::downgrade(&token);
        let page_sink: Rc<dyn DesktopSessionPageIntentSink> =
            Rc::new(RetainingSessionPageSink { _token: token });
        let shell = DesktopShell::new_with_reliable_state_and_session_sinks(
            &ProductReducer::new().snapshot(),
            DesktopReliableStateProjection::unavailable(),
            Rc::new(UnavailableDesktopIntentSink),
            Rc::new(UnavailableDesktopSessionDetailIntentSink),
            Rc::clone(&page_sink),
        )
        .map_err(|_| String::from("desktop shell"))?;
        drop(page_sink);
        assert!(weak.upgrade().is_some());
        drop(shell);
        assert!(weak.upgrade().is_none());
        Ok(())
    }

    fn reliable_state_with_presentation_and_operation(
        density: DesktopDensity,
        skin: DesktopSkin,
        operation: Option<DesktopOperationSnapshot>,
    ) -> DesktopReliableStateProjection {
        let summary = DesktopReliableStateSummary::new_with_settings(
            DesktopReliableStateHealth::Healthy,
            false,
            "healthy",
            DesktopBackupPolicy::disabled(),
            DesktopReminderPolicy::unavailable(),
            DesktopPresentationSettings::new(density, skin),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            operation,
            None,
        );
        DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
            1,
            summary,
            Vec::new(),
        ))
    }

    fn publish_from_worker(
        notifier: super::DesktopReliableStateNotifier,
        projection: DesktopReliableStateProjection,
    ) -> Result<(), String> {
        thread::spawn(move || notifier.publish(projection))
            .join()
            .map_err(|_| String::from("publisher thread"))?
            .map_err(|_| String::from("publication"))
    }

    impl DesktopIntentSink for ReentrantPresentationSink {
        fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
            if matches!(intent, DesktopIntent::UpdatePresentation(_)) {
                self.submissions.set(self.submissions.get() + 1);
                let Ok(mut style) = self.style.try_lock() else {
                    return DesktopIntentAdmission::Rejected;
                };
                self.acquired_style.set(true);
                assert_eq!(
                    style.select_density_index(2),
                    crate::DesktopPresentationApplyOutcome::Applied
                );
            }
            DesktopIntentAdmission::Started
        }
    }

    #[test]
    fn presentation_submission_does_not_hold_the_style_mutex_or_overwrite_reentry()
    -> Result<(), String> {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().map_err(|_| String::from("window"))?;
        let style = Arc::new(Mutex::new(DesktopPresentationStyle::from_persisted(
            DesktopPresentationSelection::new(DesktopDensity::Comfortable, DesktopSkin::Refined),
        )));
        let initial_style = *style.lock().map_err(|_| String::from("initial style"))?;
        apply_presentation_style(&window, initial_style);
        let sink = Rc::new(ReentrantPresentationSink {
            style: Arc::clone(&style),
            acquired_style: Cell::new(false),
            submissions: Cell::new(0),
        });
        wire_presentation_density(&window, Arc::clone(&style), sink.clone());

        window.invoke_select_presentation_density(1);

        assert!(sink.acquired_style.get());
        assert_eq!(sink.submissions.get(), 1);
        let reentrant_style = *style.lock().map_err(|_| String::from("reentrant style"))?;
        assert_eq!(reentrant_style.density(), DesktopDensity::UltraCompact);
        assert_eq!(
            reentrant_style.persisted_selection(),
            DesktopPresentationSelection::new(DesktopDensity::Comfortable, DesktopSkin::Refined)
        );
        assert_eq!(reentrant_style.revision().get(), 1);
        assert_eq!(
            reentrant_style.persistence(),
            crate::DesktopPresentationPersistence::NotSaved
        );
        assert_eq!(window.get_presentation_density_key(), "ultra_compact");
        assert_eq!(window.get_presentation_revision(), "1");
        assert_eq!(window.get_presentation_persistence_state(), "not_saved");
        Ok(())
    }

    fn terminal_presentation_outcomes_survive_generic_replacement_until_one_delivery()
    -> Result<(), String> {
        for (phase, persisted_density, expected_persistence) in [
            (
                DesktopOperationPhase::Succeeded,
                DesktopDensity::Compact,
                "saved",
            ),
            (
                DesktopOperationPhase::Failed,
                DesktopDensity::Comfortable,
                "not_saved",
            ),
        ] {
            let shell = DesktopShell::new_with_reliable_state(
                &ProductReducer::new().snapshot(),
                reliable_state_with_presentation_and_operation(
                    DesktopDensity::Comfortable,
                    DesktopSkin::Refined,
                    None,
                ),
                Rc::new(AcceptingPresentationSink),
            )
            .map_err(|_| String::from("desktop shell"))?;
            let window = shell.window();
            window.invoke_select_presentation_density(1);
            let notifier = shell.reliable_state_notifier();
            publish_from_worker(
                notifier.clone(),
                reliable_state_with_presentation_and_operation(
                    persisted_density,
                    DesktopSkin::Refined,
                    Some(DesktopOperationSnapshot::new(
                        DesktopOperationKind::UpdatePresentation,
                        phase,
                        false,
                        None,
                    )),
                ),
            )?;
            publish_from_worker(
                notifier.clone(),
                reliable_state_with_presentation_and_operation(
                    persisted_density,
                    DesktopSkin::Refined,
                    Some(DesktopOperationSnapshot::new(
                        DesktopOperationKind::Backup,
                        DesktopOperationPhase::Running,
                        false,
                        None,
                    )),
                ),
            )?;
            notifier
                .publish_operation(Some(DesktopOperationSnapshot::new(
                    DesktopOperationKind::Backup,
                    DesktopOperationPhase::Running,
                    false,
                    None,
                )))
                .map_err(|_| String::from("generic operation publication"))?;
            assert!(
                notifier
                    .inner
                    .latest
                    .lock()
                    .map_err(|_| String::from("latest delivery"))?
                    .is_some(),
                "capacity one retains exactly one pending delivery"
            );

            notifier.inner.deliver_latest();

            assert_eq!(window.get_presentation_density_key(), "compact");
            assert_eq!(
                window.get_presentation_persistence_state(),
                expected_persistence
            );
            assert_eq!(window.get_reliable_operation_kind(), "backup");
            assert!(
                notifier
                    .inner
                    .latest
                    .lock()
                    .map_err(|_| String::from("delivered latest"))?
                    .is_none(),
                "terminal marker is consumed with the sole delivery"
            );
        }
        Ok(())
    }

    fn config_and_portable_restore_terminals_survive_generic_replacement() -> Result<(), String> {
        for kind in [
            DesktopOperationKind::ApplyConfig,
            DesktopOperationKind::RestoreWithPortableSettings,
        ] {
            let shell = DesktopShell::new_with_reliable_state(
                &ProductReducer::new().snapshot(),
                reliable_state_with_presentation_and_operation(
                    DesktopDensity::Comfortable,
                    DesktopSkin::Refined,
                    None,
                ),
                Rc::new(AcceptingPresentationSink),
            )
            .map_err(|_| String::from("desktop shell"))?;
            let window = shell.window();
            window.invoke_select_presentation_density(1);
            let notifier = shell.reliable_state_notifier();
            publish_from_worker(
                notifier.clone(),
                reliable_state_with_presentation_and_operation(
                    DesktopDensity::UltraCompact,
                    DesktopSkin::Graphite,
                    Some(DesktopOperationSnapshot::new(
                        kind,
                        DesktopOperationPhase::Succeeded,
                        false,
                        None,
                    )),
                ),
            )?;
            publish_from_worker(
                notifier.clone(),
                reliable_state_with_presentation_and_operation(
                    DesktopDensity::UltraCompact,
                    DesktopSkin::Graphite,
                    Some(DesktopOperationSnapshot::new(
                        DesktopOperationKind::Backup,
                        DesktopOperationPhase::Running,
                        false,
                        None,
                    )),
                ),
            )?;

            notifier.inner.deliver_latest();

            assert_eq!(window.get_presentation_density_key(), "ultra_compact");
            assert_eq!(window.get_presentation_persistence_state(), "saved");
            assert_eq!(window.get_reliable_operation_kind(), "backup");
        }
        Ok(())
    }

    #[test]
    fn notification_expiry_preserves_exact_millisecond_endpoints() {
        assert_eq!(
            format_benefit_expiry(
                &DesktopBenefitExpiry::ExactUtc {
                    at_ms: 1_784_203_200_001,
                },
                "available",
            ),
            "Expires 2026-07-16 12:00:00.001 UTC"
        );
        assert_eq!(
            format_benefit_expiry(
                &DesktopBenefitExpiry::BoundedUtc {
                    earliest_at_ms: 1_784_203_200_001,
                    latest_at_ms: 1_784_203_200_002,
                },
                "available",
            ),
            "Expires between 2026-07-16 12:00:00.001 UTC and 2026-07-16 12:00:00.002 UTC"
        );
    }

    #[test]
    fn activity_timestamp_preserves_fractional_utc_truth() {
        assert_eq!(
            format_timestamp_utc(1_784_163_600, 123_450_000),
            "2026-07-16 01:00:00.12345 UTC"
        );
        assert_eq!(
            format_timestamp_utc(1_784_163_600, 0),
            "2026-07-16 01:00:00 UTC"
        );
        assert_eq!(
            format_timestamp_utc(1_784_163_600, 1_000_000_000),
            "Unavailable"
        );
    }

    #[test]
    fn duration_borrows_nanoseconds_across_the_second_boundary() {
        assert_eq!(
            format_session_duration(10, 900_000_000, 11, 100_000_000),
            "<1s"
        );
        assert_eq!(
            format_session_duration(10, 100_000_000, 11, 900_000_000),
            "1s"
        );
        assert_eq!(
            format_session_duration(10, 100_000_000, 10, 100_000_000),
            "0s"
        );
        assert_eq!(
            format_session_duration(11, 100_000_000, 10, 900_000_000),
            "Unavailable"
        );
    }

    fn assert_pending_reminder_publication_waits_for_visible_atomic_projection(
        shell: &DesktopShell,
        publish_pending: impl FnOnce(super::DesktopReliableStateNotifier) -> Result<(), String>
        + Send
        + 'static,
        expected_kind: &str,
    ) -> Result<(), String> {
        let window = shell.window();
        let notifier = shell.reliable_state_notifier();
        let coalescer = notifier.clone();
        let inner = std::sync::Arc::clone(&notifier.inner);
        let (completed_sender, completed_receiver) = sync_channel(1);
        let publisher = thread::spawn(move || -> Result<(), String> {
            let result = publish_pending(notifier);
            let _ = completed_sender.send(result);
            Ok(())
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while inner
            .latest
            .lock()
            .map_err(|_| String::from("pending delivery"))?
            .is_none()
        {
            assert!(
                Instant::now() < deadline,
                "pending projection was not queued"
            );
            thread::yield_now();
        }
        assert!(
            completed_receiver
                .recv_timeout(Duration::from_millis(25))
                .is_err(),
            "publication returned before the visible projection"
        );
        assert!(
            coalescer
                .publish_operation(Some(DesktopOperationSnapshot::new(
                    DesktopOperationKind::Backup,
                    DesktopOperationPhase::AtomicPromotion,
                    false,
                    None,
                )))
                .is_err(),
            "queued pending projection must not be coalesced with a different operation"
        );

        inner.deliver_latest();
        completed_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| String::from("publication acknowledgement"))?
            .map_err(|_| String::from("visible pending publication"))?;
        publisher
            .join()
            .map_err(|_| String::from("publisher thread"))??;
        assert_eq!(window.get_reminder_sync_state(), "Pending");
        assert_eq!(window.get_reliable_operation_kind(), expected_kind);
        assert_eq!(window.get_reliable_operation_phase(), "atomic_promotion");
        Ok(())
    }

    fn pending_reminder_publications_wait_for_the_visible_atomic_projection() -> Result<(), String>
    {
        let shell = DesktopShell::new_with_optional_lifecycle_sink(
            &ProductReducer::new().snapshot(),
            DesktopReliableStateProjection::unavailable(),
            Rc::new(UnavailableDesktopIntentSink),
            Rc::new(crate::history::UnavailableDesktopHistoryRangeIntentSink),
            Rc::new(UnavailableDesktopSessionDetailIntentSink),
            Rc::new(UnavailableDesktopSessionPageIntentSink),
            None,
        )
        .map_err(|_| String::from("desktop shell"))?;
        assert_pending_reminder_publication_waits_for_visible_atomic_projection(
            &shell,
            |notifier| {
                notifier
                    .publish_pending_reminder_policy(
                        DesktopReminderPolicy::new(
                            true,
                            &[21_600],
                            DesktopReminderSyncState::Pending,
                        )
                        .ok_or_else(|| String::from("pending reminder policy"))?,
                        DesktopOperationSnapshot::new(
                            DesktopOperationKind::UpdatePolicy,
                            DesktopOperationPhase::AtomicPromotion,
                            false,
                            None,
                        ),
                    )
                    .map_err(|_| String::from("pending reminder publication"))
            },
            "update_policy",
        )?;
        assert_pending_reminder_publication_waits_for_visible_atomic_projection(
            &shell,
            |notifier| {
                notifier
                    .publish_pending_reminder_operation(DesktopOperationSnapshot::new(
                        DesktopOperationKind::ImportConfig,
                        DesktopOperationPhase::AtomicPromotion,
                        false,
                        None,
                    ))
                    .map_err(|_| String::from("pending config import publication"))
            },
            "import_config",
        )
    }

    #[test]
    fn reliable_state_delivery_contracts_share_one_slint_event_loop() -> Result<(), String> {
        i_slint_backend_testing::init_integration_test_with_system_time();
        terminal_presentation_outcomes_survive_generic_replacement_until_one_delivery()?;
        config_and_portable_restore_terminals_survive_generic_replacement()?;
        pending_reminder_publications_wait_for_the_visible_atomic_projection()
    }
}
