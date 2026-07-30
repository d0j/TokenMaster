use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
};

use tokenmaster_product::ProductSnapshot;

use crate::{
    MainWindow,
    controller::{
        DesktopHistoryRangeIntent, DesktopSessionPageIntent, DesktopSnapshotNotifier,
        DesktopSnapshotReceiver, DesktopTerminalHistoryRangeNotifier,
        DesktopTerminalNavigationNotifier,
    },
    presentation::{DesktopApplyOutcome, DesktopSnapshotEpoch},
    ui::{
        SharedDesktopState, apply_history_projection, apply_projection,
        apply_session_detail_projection, apply_session_navigation_projection,
    },
};

pub(crate) type EventTask = Box<dyn FnOnce() + Send + 'static>;

pub(crate) trait EventScheduler: Send + Sync + 'static {
    fn schedule(&self, task: EventTask) -> Result<(), ScheduleError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScheduleError {
    Unavailable,
    Terminated,
    Internal,
}

pub(crate) struct SlintEventScheduler;

impl EventScheduler for SlintEventScheduler {
    fn schedule(&self, task: EventTask) -> Result<(), ScheduleError> {
        slint::invoke_from_event_loop(task).map_err(|error| match error {
            slint::EventLoopError::NoEventLoopProvider => ScheduleError::Unavailable,
            slint::EventLoopError::EventLoopTerminated => ScheduleError::Terminated,
            _ => ScheduleError::Internal,
        })
    }
}

trait SnapshotDelivery: Send + Sync + 'static {
    fn deliver(&self, snapshot: Arc<ProductSnapshot>) -> DeliveryOutcome;
}

trait TerminalNavigationDelivery: Send + Sync + 'static {
    fn deliver(&self, intent: DesktopSessionPageIntent) -> TerminalDeliveryOutcome;
}

trait TerminalHistoryRangeDelivery: Send + Sync + 'static {
    fn deliver(&self, intent: DesktopHistoryRangeIntent) -> TerminalDeliveryOutcome;
}

struct SlintSnapshotDelivery {
    epoch: DesktopSnapshotEpoch,
    window: slint::Weak<MainWindow>,
    state: SharedDesktopState,
}

impl SnapshotDelivery for SlintSnapshotDelivery {
    fn deliver(&self, snapshot: Arc<ProductSnapshot>) -> DeliveryOutcome {
        let Some(window) = self.window.upgrade() else {
            return DeliveryOutcome::WindowClosed;
        };
        let Ok(mut state) = self.state.lock() else {
            return DeliveryOutcome::StateUnavailable;
        };
        match state.apply_snapshot_for_epoch(self.epoch, &snapshot) {
            DesktopApplyOutcome::Accepted => {
                apply_projection(&window, state.projection());
                DeliveryOutcome::Delivered(snapshot.generation().get())
            }
            DesktopApplyOutcome::IgnoredNotNewer => DeliveryOutcome::Ignored,
        }
    }
}

struct SlintTerminalNavigationDelivery {
    window: slint::Weak<MainWindow>,
    state: SharedDesktopState,
}

impl TerminalNavigationDelivery for SlintTerminalNavigationDelivery {
    fn deliver(&self, intent: DesktopSessionPageIntent) -> TerminalDeliveryOutcome {
        let Some(window) = self.window.upgrade() else {
            return TerminalDeliveryOutcome::WindowClosed;
        };
        let Ok(mut state) = self.state.lock() else {
            return TerminalDeliveryOutcome::StateUnavailable;
        };
        state.reject_session_page(intent);
        apply_session_navigation_projection(&window, state.projection().sessions());
        apply_session_detail_projection(&window, state.projection().sessions());
        TerminalDeliveryOutcome::Delivered
    }
}

struct SlintTerminalHistoryRangeDelivery {
    window: slint::Weak<MainWindow>,
    state: SharedDesktopState,
}

impl TerminalHistoryRangeDelivery for SlintTerminalHistoryRangeDelivery {
    fn deliver(&self, intent: DesktopHistoryRangeIntent) -> TerminalDeliveryOutcome {
        let Some(window) = self.window.upgrade() else {
            return TerminalDeliveryOutcome::WindowClosed;
        };
        let Ok(mut state) = self.state.lock() else {
            return TerminalDeliveryOutcome::StateUnavailable;
        };
        state.complete_history_range_terminal(intent);
        apply_history_projection(&window, state.projection().history());
        TerminalDeliveryOutcome::Delivered
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeliveryOutcome {
    Delivered(u64),
    Ignored,
    WindowClosed,
    StateUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalDeliveryOutcome {
    Delivered,
    WindowClosed,
    StateUnavailable,
}

#[cfg(test)]
struct NoopTerminalNavigationDelivery;

#[cfg(test)]
impl TerminalNavigationDelivery for NoopTerminalNavigationDelivery {
    fn deliver(&self, _intent: DesktopSessionPageIntent) -> TerminalDeliveryOutcome {
        TerminalDeliveryOutcome::Delivered
    }
}

#[cfg(test)]
struct NoopTerminalHistoryRangeDelivery;

#[cfg(test)]
impl TerminalHistoryRangeDelivery for NoopTerminalHistoryRangeDelivery {
    fn deliver(&self, _intent: DesktopHistoryRangeIntent) -> TerminalDeliveryOutcome {
        TerminalDeliveryOutcome::Delivered
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopBridgeGeneration(u64);

impl DesktopBridgeGeneration {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopBridgePhase {
    Running,
    Closed,
    Faulted,
}

impl DesktopBridgePhase {
    const fn encoded(self) -> u8 {
        match self {
            Self::Running => 0,
            Self::Closed => 1,
            Self::Faulted => 2,
        }
    }

    const fn from_encoded(value: u8) -> Self {
        match value {
            0 => Self::Running,
            1 => Self::Closed,
            _ => Self::Faulted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopBridgeFailureCode {
    EventLoopUnavailable,
    EventLoopTerminated,
    WindowClosed,
    StateUnavailable,
    Internal,
}

impl DesktopBridgeFailureCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::EventLoopUnavailable => "event_loop_unavailable",
            Self::EventLoopTerminated => "event_loop_terminated",
            Self::WindowClosed => "window_closed",
            Self::StateUnavailable => "state_unavailable",
            Self::Internal => "internal",
        }
    }

    const fn encoded(self) -> u8 {
        match self {
            Self::EventLoopUnavailable => 1,
            Self::EventLoopTerminated => 2,
            Self::WindowClosed => 3,
            Self::StateUnavailable => 4,
            Self::Internal => 5,
        }
    }

    const fn from_encoded(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::EventLoopUnavailable),
            2 => Some(Self::EventLoopTerminated),
            3 => Some(Self::WindowClosed),
            4 => Some(Self::StateUnavailable),
            5 => Some(Self::Internal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopBridgeSnapshot {
    phase: DesktopBridgePhase,
    scheduled: bool,
    scheduled_count: u64,
    coalesced_count: u64,
    delivered_count: u64,
    ignored_count: u64,
    scheduling_failure_count: u64,
    last_delivered_generation: Option<DesktopBridgeGeneration>,
    last_failure: Option<DesktopBridgeFailureCode>,
}

impl DesktopBridgeSnapshot {
    #[must_use]
    pub const fn phase(self) -> DesktopBridgePhase {
        self.phase
    }

    #[must_use]
    pub const fn scheduled(self) -> bool {
        self.scheduled
    }

    #[must_use]
    pub const fn scheduled_count(self) -> u64 {
        self.scheduled_count
    }

    #[must_use]
    pub const fn coalesced_count(self) -> u64 {
        self.coalesced_count
    }

    #[must_use]
    pub const fn delivered_count(self) -> u64 {
        self.delivered_count
    }

    #[must_use]
    pub const fn ignored_count(self) -> u64 {
        self.ignored_count
    }

    #[must_use]
    pub const fn scheduling_failure_count(self) -> u64 {
        self.scheduling_failure_count
    }

    #[must_use]
    pub const fn last_delivered_generation(self) -> Option<DesktopBridgeGeneration> {
        self.last_delivered_generation
    }

    #[must_use]
    pub const fn last_failure(self) -> Option<DesktopBridgeFailureCode> {
        self.last_failure
    }
}

struct BridgeInner {
    receiver: DesktopSnapshotReceiver,
    scheduler: Arc<dyn EventScheduler>,
    delivery: Arc<dyn SnapshotDelivery>,
    terminal_delivery: Arc<dyn TerminalNavigationDelivery>,
    history_terminal_delivery: Arc<dyn TerminalHistoryRangeDelivery>,
    terminal_intent: std::sync::Mutex<Option<DesktopSessionPageIntent>>,
    history_terminal_intent: std::sync::Mutex<Option<DesktopHistoryRangeIntent>>,
    phase: AtomicU8,
    scheduled: AtomicBool,
    scheduled_count: AtomicU64,
    coalesced_count: AtomicU64,
    delivered_count: AtomicU64,
    ignored_count: AtomicU64,
    scheduling_failure_count: AtomicU64,
    last_delivered_generation: AtomicU64,
    last_failure: AtomicU8,
}

impl BridgeInner {
    fn new(
        receiver: DesktopSnapshotReceiver,
        scheduler: Arc<dyn EventScheduler>,
        delivery: Arc<dyn SnapshotDelivery>,
        terminal_delivery: Arc<dyn TerminalNavigationDelivery>,
        history_terminal_delivery: Arc<dyn TerminalHistoryRangeDelivery>,
    ) -> Arc<Self> {
        Arc::new(Self {
            receiver,
            scheduler,
            delivery,
            terminal_delivery,
            history_terminal_delivery,
            terminal_intent: std::sync::Mutex::new(None),
            history_terminal_intent: std::sync::Mutex::new(None),
            phase: AtomicU8::new(DesktopBridgePhase::Running.encoded()),
            scheduled: AtomicBool::new(false),
            scheduled_count: AtomicU64::new(0),
            coalesced_count: AtomicU64::new(0),
            delivered_count: AtomicU64::new(0),
            ignored_count: AtomicU64::new(0),
            scheduling_failure_count: AtomicU64::new(0),
            last_delivered_generation: AtomicU64::new(0),
            last_failure: AtomicU8::new(0),
        })
    }

    fn request(self: &Arc<Self>) {
        if self.phase() != DesktopBridgePhase::Running {
            return;
        }
        if self.scheduled.swap(true, Ordering::AcqRel) {
            saturating_increment(&self.coalesced_count);
            return;
        }

        let inner = self.clone();
        match self.scheduler.schedule(Box::new(move || inner.run_once())) {
            Ok(()) => {
                saturating_increment(&self.scheduled_count);
            }
            Err(error) => {
                saturating_increment(&self.scheduling_failure_count);
                self.record_schedule_error(error);
                self.scheduled.store(false, Ordering::Release);
            }
        }
    }

    fn request_terminal(self: &Arc<Self>, intent: DesktopSessionPageIntent) {
        if self.phase() != DesktopBridgePhase::Running {
            return;
        }
        let Ok(mut pending) = self.terminal_intent.lock() else {
            self.fault(DesktopBridgeFailureCode::Internal);
            return;
        };
        if pending
            .is_none_or(|current| intent.navigation_generation() > current.navigation_generation())
        {
            *pending = Some(intent);
        }
        drop(pending);
        self.request();
    }

    fn request_history_terminal(self: &Arc<Self>, intent: DesktopHistoryRangeIntent) {
        if self.phase() != DesktopBridgePhase::Running {
            return;
        }
        let Ok(mut pending) = self.history_terminal_intent.lock() else {
            self.fault(DesktopBridgeFailureCode::Internal);
            return;
        };
        if pending.is_none_or(|current| intent.generation() > current.generation()) {
            *pending = Some(intent);
        }
        drop(pending);
        self.request();
    }

    fn run_once(self: &Arc<Self>) {
        if self.phase() != DesktopBridgePhase::Running {
            self.scheduled.store(false, Ordering::Release);
            return;
        }

        let snapshot = match self.receiver.take_snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => {
                self.fault(DesktopBridgeFailureCode::Internal);
                self.scheduled.store(false, Ordering::Release);
                return;
            }
        };
        if let Some(snapshot) = snapshot {
            match self.delivery.deliver(snapshot) {
                DeliveryOutcome::Delivered(generation) => {
                    self.last_delivered_generation
                        .store(generation, Ordering::Release);
                    saturating_increment(&self.delivered_count);
                    self.last_failure.store(0, Ordering::Release);
                }
                DeliveryOutcome::Ignored => {
                    saturating_increment(&self.ignored_count);
                    self.last_failure.store(0, Ordering::Release);
                }
                DeliveryOutcome::WindowClosed => {
                    self.close(DesktopBridgeFailureCode::WindowClosed);
                }
                DeliveryOutcome::StateUnavailable => {
                    self.fault(DesktopBridgeFailureCode::StateUnavailable);
                }
            }
        }

        let terminal = match self.terminal_intent.lock() {
            Ok(mut pending) => pending.take(),
            Err(_) => {
                self.fault(DesktopBridgeFailureCode::Internal);
                self.scheduled.store(false, Ordering::Release);
                return;
            }
        };
        if let Some(intent) = terminal {
            match self.terminal_delivery.deliver(intent) {
                TerminalDeliveryOutcome::Delivered => {}
                TerminalDeliveryOutcome::WindowClosed => {
                    self.close(DesktopBridgeFailureCode::WindowClosed);
                }
                TerminalDeliveryOutcome::StateUnavailable => {
                    self.fault(DesktopBridgeFailureCode::StateUnavailable);
                }
            }
        }

        let history_terminal = match self.history_terminal_intent.lock() {
            Ok(mut pending) => pending.take(),
            Err(_) => {
                self.fault(DesktopBridgeFailureCode::Internal);
                self.scheduled.store(false, Ordering::Release);
                return;
            }
        };
        if let Some(intent) = history_terminal {
            match self.history_terminal_delivery.deliver(intent) {
                TerminalDeliveryOutcome::Delivered => {}
                TerminalDeliveryOutcome::WindowClosed => {
                    self.close(DesktopBridgeFailureCode::WindowClosed);
                }
                TerminalDeliveryOutcome::StateUnavailable => {
                    self.fault(DesktopBridgeFailureCode::StateUnavailable);
                }
            }
        }

        self.scheduled.store(false, Ordering::Release);
        if self.phase() == DesktopBridgePhase::Running {
            match (
                self.receiver.has_snapshot(),
                self.has_terminal(),
                self.has_history_terminal(),
            ) {
                (Ok(true), _, _) | (_, Ok(true), _) | (_, _, Ok(true)) => self.request(),
                (Ok(false), Ok(false), Ok(false)) => {}
                _ => self.fault(DesktopBridgeFailureCode::Internal),
            }
        }
    }

    fn has_terminal(&self) -> Result<bool, ()> {
        self.terminal_intent
            .lock()
            .map(|pending| pending.is_some())
            .map_err(|_| ())
    }

    fn has_history_terminal(&self) -> Result<bool, ()> {
        self.history_terminal_intent
            .lock()
            .map(|pending| pending.is_some())
            .map_err(|_| ())
    }

    fn record_schedule_error(&self, error: ScheduleError) {
        match error {
            ScheduleError::Unavailable => {
                self.last_failure.store(
                    DesktopBridgeFailureCode::EventLoopUnavailable.encoded(),
                    Ordering::Release,
                );
            }
            ScheduleError::Terminated => {
                self.close(DesktopBridgeFailureCode::EventLoopTerminated);
            }
            ScheduleError::Internal => self.fault(DesktopBridgeFailureCode::Internal),
        }
    }

    fn close(&self, code: DesktopBridgeFailureCode) {
        self.last_failure.store(code.encoded(), Ordering::Release);
        self.phase
            .store(DesktopBridgePhase::Closed.encoded(), Ordering::Release);
    }

    fn fault(&self, code: DesktopBridgeFailureCode) {
        self.last_failure.store(code.encoded(), Ordering::Release);
        self.phase
            .store(DesktopBridgePhase::Faulted.encoded(), Ordering::Release);
    }

    fn phase(&self) -> DesktopBridgePhase {
        DesktopBridgePhase::from_encoded(self.phase.load(Ordering::Acquire))
    }

    fn snapshot(&self) -> DesktopBridgeSnapshot {
        let generation = self.last_delivered_generation.load(Ordering::Acquire);
        DesktopBridgeSnapshot {
            phase: self.phase(),
            scheduled: self.scheduled.load(Ordering::Acquire),
            scheduled_count: self.scheduled_count.load(Ordering::Acquire),
            coalesced_count: self.coalesced_count.load(Ordering::Acquire),
            delivered_count: self.delivered_count.load(Ordering::Acquire),
            ignored_count: self.ignored_count.load(Ordering::Acquire),
            scheduling_failure_count: self.scheduling_failure_count.load(Ordering::Acquire),
            last_delivered_generation: (generation != 0)
                .then_some(DesktopBridgeGeneration(generation)),
            last_failure: DesktopBridgeFailureCode::from_encoded(
                self.last_failure.load(Ordering::Acquire),
            ),
        }
    }
}

struct BridgeNotifier {
    inner: Weak<BridgeInner>,
}

impl DesktopSnapshotNotifier for BridgeNotifier {
    fn snapshot_ready(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.request();
        }
    }
}

struct BridgeTerminalNavigationNotifier {
    inner: Weak<BridgeInner>,
}

impl DesktopTerminalNavigationNotifier for BridgeTerminalNavigationNotifier {
    fn navigation_terminal(&self, intent: DesktopSessionPageIntent) {
        if let Some(inner) = self.inner.upgrade() {
            inner.request_terminal(intent);
        }
    }
}

struct BridgeTerminalHistoryRangeNotifier {
    inner: Weak<BridgeInner>,
}

impl DesktopTerminalHistoryRangeNotifier for BridgeTerminalHistoryRangeNotifier {
    fn history_range_terminal(&self, intent: DesktopHistoryRangeIntent) {
        if let Some(inner) = self.inner.upgrade() {
            inner.request_history_terminal(intent);
        }
    }
}

pub struct DesktopSnapshotBridge {
    epoch: DesktopSnapshotEpoch,
    inner: Arc<BridgeInner>,
}

#[derive(Clone)]
pub struct DesktopBridgeObserver {
    inner: Weak<BridgeInner>,
}

impl DesktopBridgeObserver {
    #[must_use]
    pub fn snapshot(&self) -> Option<DesktopBridgeSnapshot> {
        self.inner.upgrade().map(|inner| inner.snapshot())
    }
}

impl DesktopSnapshotBridge {
    pub(crate) fn new(
        epoch: DesktopSnapshotEpoch,
        window: slint::Weak<MainWindow>,
        state: SharedDesktopState,
        receiver: DesktopSnapshotReceiver,
    ) -> Self {
        Self::with_terminal_parts(
            epoch,
            receiver,
            Arc::new(SlintEventScheduler),
            Arc::new(SlintSnapshotDelivery {
                epoch,
                window: window.clone(),
                state: Arc::clone(&state),
            }),
            Arc::new(SlintTerminalNavigationDelivery {
                window: window.clone(),
                state: Arc::clone(&state),
            }),
            Arc::new(SlintTerminalHistoryRangeDelivery {
                window: window.clone(),
                state: Arc::clone(&state),
            }),
        )
    }

    #[cfg(test)]
    fn with_parts(
        epoch: DesktopSnapshotEpoch,
        receiver: DesktopSnapshotReceiver,
        scheduler: Arc<dyn EventScheduler>,
        delivery: Arc<dyn SnapshotDelivery>,
    ) -> Self {
        Self::with_terminal_parts(
            epoch,
            receiver,
            scheduler,
            delivery,
            Arc::new(NoopTerminalNavigationDelivery),
            Arc::new(NoopTerminalHistoryRangeDelivery),
        )
    }

    fn with_terminal_parts(
        epoch: DesktopSnapshotEpoch,
        receiver: DesktopSnapshotReceiver,
        scheduler: Arc<dyn EventScheduler>,
        delivery: Arc<dyn SnapshotDelivery>,
        terminal_delivery: Arc<dyn TerminalNavigationDelivery>,
        history_terminal_delivery: Arc<dyn TerminalHistoryRangeDelivery>,
    ) -> Self {
        Self {
            epoch,
            inner: BridgeInner::new(
                receiver,
                scheduler,
                delivery,
                terminal_delivery,
                history_terminal_delivery,
            ),
        }
    }

    #[must_use]
    pub const fn epoch(&self) -> DesktopSnapshotEpoch {
        self.epoch
    }

    #[must_use]
    pub fn notifier(&self) -> Arc<dyn DesktopSnapshotNotifier> {
        Arc::new(BridgeNotifier {
            inner: Arc::downgrade(&self.inner),
        })
    }

    #[must_use]
    pub fn terminal_navigation_notifier(&self) -> Arc<dyn DesktopTerminalNavigationNotifier> {
        Arc::new(BridgeTerminalNavigationNotifier {
            inner: Arc::downgrade(&self.inner),
        })
    }

    #[must_use]
    pub fn terminal_history_range_notifier(&self) -> Arc<dyn DesktopTerminalHistoryRangeNotifier> {
        Arc::new(BridgeTerminalHistoryRangeNotifier {
            inner: Arc::downgrade(&self.inner),
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> DesktopBridgeSnapshot {
        self.inner.snapshot()
    }

    #[must_use]
    pub fn observer(&self) -> DesktopBridgeObserver {
        DesktopBridgeObserver {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

impl Drop for DesktopSnapshotBridge {
    fn drop(&mut self) {
        self.inner
            .phase
            .store(DesktopBridgePhase::Closed.encoded(), Ordering::Release);
    }
}

pub(crate) fn saturating_increment(value: &AtomicU64) {
    let _ = value.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
        Some(current.saturating_add(1))
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Mutex, atomic::AtomicBool},
    };

    use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
    use tokenmaster_query::QueryErrorCode;

    use super::*;

    fn test_epoch() -> DesktopSnapshotEpoch {
        DesktopSnapshotEpoch::new(1).expect("nonzero test epoch")
    }

    struct ManualScheduler {
        tasks: Mutex<VecDeque<EventTask>>,
        fail_next: AtomicBool,
    }

    impl ManualScheduler {
        fn new() -> Self {
            Self {
                tasks: Mutex::new(VecDeque::new()),
                fail_next: AtomicBool::new(false),
            }
        }

        fn fail_next(&self) {
            self.fail_next.store(true, Ordering::Release);
        }

        fn len(&self) -> usize {
            self.tasks.lock().expect("tasks").len()
        }

        fn run_one(&self) {
            let task = self.tasks.lock().expect("tasks").pop_front();
            task.expect("scheduled task")();
        }
    }

    impl EventScheduler for ManualScheduler {
        fn schedule(&self, task: EventTask) -> Result<(), ScheduleError> {
            if self.fail_next.swap(false, Ordering::AcqRel) {
                return Err(ScheduleError::Unavailable);
            }
            self.tasks.lock().expect("tasks").push_back(task);
            Ok(())
        }
    }

    struct CallbackDelivery {
        callback: Box<dyn Fn(Arc<ProductSnapshot>) -> DeliveryOutcome + Send + Sync>,
    }

    impl SnapshotDelivery for CallbackDelivery {
        fn deliver(&self, snapshot: Arc<ProductSnapshot>) -> DeliveryOutcome {
            (self.callback)(snapshot)
        }
    }

    struct CallbackTerminalDelivery {
        callback: Box<dyn Fn(DesktopSessionPageIntent) -> TerminalDeliveryOutcome + Send + Sync>,
    }

    impl TerminalNavigationDelivery for CallbackTerminalDelivery {
        fn deliver(&self, intent: DesktopSessionPageIntent) -> TerminalDeliveryOutcome {
            (self.callback)(intent)
        }
    }

    fn terminal_intent(generation: u64) -> DesktopSessionPageIntent {
        DesktopSessionPageIntent::new(
            test_epoch(),
            tokenmaster_product::ProductGeneration::INITIAL,
            crate::DesktopSessionNavigationGeneration::new(generation).expect("generation"),
            crate::DesktopSessionPageDirection::Newest,
        )
    }

    #[test]
    fn terminal_navigation_delivery_is_weak_coalesced_and_keeps_the_latest_intent() {
        let receiver = DesktopSnapshotReceiver::empty_for_test();
        let scheduler = Arc::new(ManualScheduler::new());
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let delivery_log = Arc::clone(&delivered);
        let bridge = DesktopSnapshotBridge::with_terminal_parts(
            test_epoch(),
            receiver,
            scheduler.clone(),
            Arc::new(CallbackDelivery {
                callback: Box::new(|snapshot| {
                    DeliveryOutcome::Delivered(snapshot.generation().get())
                }),
            }),
            Arc::new(CallbackTerminalDelivery {
                callback: Box::new(move |intent| {
                    delivery_log
                        .lock()
                        .expect("terminal deliveries")
                        .push(intent.navigation_generation().get());
                    TerminalDeliveryOutcome::Delivered
                }),
            }),
            Arc::new(NoopTerminalHistoryRangeDelivery),
        );
        let notifier = bridge.terminal_navigation_notifier();
        notifier.navigation_terminal(terminal_intent(2));
        notifier.navigation_terminal(terminal_intent(1));
        assert_eq!(scheduler.len(), 1);
        scheduler.run_one();
        assert_eq!(*delivered.lock().expect("terminal deliveries"), [2]);
        drop(bridge);
        notifier.navigation_terminal(terminal_intent(3));
        assert_eq!(scheduler.len(), 0);
    }

    #[test]
    fn coalesced_snapshot_then_terminal_delivery_cannot_replace_newer_snapshot_state() {
        let receiver = DesktopSnapshotReceiver::empty_for_test();
        let scheduler = Arc::new(ManualScheduler::new());
        let visible_generation = Arc::new(AtomicU64::new(0));
        let snapshot_generation = Arc::clone(&visible_generation);
        let terminal_generation = Arc::clone(&visible_generation);
        let stale_terminal = Arc::new(AtomicBool::new(false));
        let stale_terminal_seen = Arc::clone(&stale_terminal);
        let bridge = DesktopSnapshotBridge::with_terminal_parts(
            test_epoch(),
            receiver.clone(),
            scheduler.clone(),
            Arc::new(CallbackDelivery {
                callback: Box::new(move |snapshot| {
                    snapshot_generation.store(snapshot.generation().get(), Ordering::Release);
                    DeliveryOutcome::Delivered(snapshot.generation().get())
                }),
            }),
            Arc::new(CallbackTerminalDelivery {
                callback: Box::new(move |intent| {
                    stale_terminal_seen.store(
                        terminal_generation.load(Ordering::Acquire)
                            > intent.product_generation().get(),
                        Ordering::Release,
                    );
                    TerminalDeliveryOutcome::Delivered
                }),
            }),
            Arc::new(NoopTerminalHistoryRangeDelivery),
        );
        let mut reducer = ProductReducer::new();
        reducer
            .fail_data_status(
                ProductAttemptGeneration::new(1).expect("attempt"),
                QueryErrorCode::Unavailable,
            )
            .expect("snapshot generation");
        let snapshot = reducer.snapshot();
        let expected_generation = snapshot.generation().get();
        receiver
            .replace_snapshot(snapshot)
            .expect("snapshot queued");
        let snapshot_notifier = bridge.notifier();
        let terminal_notifier = bridge.terminal_navigation_notifier();
        snapshot_notifier.snapshot_ready();
        terminal_notifier.navigation_terminal(terminal_intent(1));
        assert_eq!(scheduler.len(), 1);
        scheduler.run_one();
        assert_eq!(
            visible_generation.load(Ordering::Acquire),
            expected_generation,
            "snapshot remains authoritative when terminal delivery shares its event"
        );
        assert!(stale_terminal.load(Ordering::Acquire));
    }

    #[test]
    fn ten_thousand_notifications_queue_one_event_and_deliver_only_latest() {
        let receiver = DesktopSnapshotReceiver::empty_for_test();
        let scheduler = Arc::new(ManualScheduler::new());
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let delivery_log = delivered.clone();
        let bridge = DesktopSnapshotBridge::with_parts(
            test_epoch(),
            receiver.clone(),
            scheduler.clone(),
            Arc::new(CallbackDelivery {
                callback: Box::new(move |snapshot| {
                    delivery_log
                        .lock()
                        .expect("deliveries")
                        .push(snapshot.generation().get());
                    DeliveryOutcome::Delivered(snapshot.generation().get())
                }),
            }),
        );
        let notifier = bridge.notifier();
        let mut reducer = ProductReducer::new();
        for value in 1..=10_000 {
            let attempt = ProductAttemptGeneration::new(value).expect("attempt");
            reducer
                .fail_data_status(attempt, QueryErrorCode::Unavailable)
                .expect("generation");
            receiver
                .replace_snapshot(reducer.snapshot())
                .expect("mailbox");
            notifier.snapshot_ready();
        }

        assert_eq!(scheduler.len(), 1);
        let pending = bridge.snapshot();
        assert!(pending.scheduled());
        assert_eq!(pending.scheduled_count(), 1);
        assert_eq!(pending.coalesced_count(), 9_999);
        scheduler.run_one();

        assert_eq!(*delivered.lock().expect("deliveries"), [10_000]);
        let complete = bridge.snapshot();
        assert!(!complete.scheduled());
        assert_eq!(complete.delivered_count(), 1);
        assert_eq!(
            complete
                .last_delivered_generation()
                .expect("last generation")
                .get(),
            10_000
        );
        assert!(!receiver.has_snapshot().expect("mailbox"));
    }

    #[test]
    fn publication_during_delivery_queues_exactly_one_follow_up() {
        let receiver = DesktopSnapshotReceiver::empty_for_test();
        let scheduler = Arc::new(ManualScheduler::new());
        let mut reducer = ProductReducer::new();
        reducer
            .fail_data_status(
                ProductAttemptGeneration::new(1).expect("attempt"),
                QueryErrorCode::Unavailable,
            )
            .expect("generation one");
        let first = reducer.snapshot();
        reducer
            .fail_data_status(
                ProductAttemptGeneration::new(2).expect("attempt"),
                QueryErrorCode::Unavailable,
            )
            .expect("generation two");
        let second = reducer.snapshot();
        receiver.replace_snapshot(first).expect("first publication");

        let notifier_slot = Arc::new(Mutex::new(None::<Arc<dyn DesktopSnapshotNotifier>>));
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let callback_receiver = receiver.clone();
        let callback_notifier = notifier_slot.clone();
        let callback_delivered = delivered.clone();
        let bridge = DesktopSnapshotBridge::with_parts(
            test_epoch(),
            receiver.clone(),
            scheduler.clone(),
            Arc::new(CallbackDelivery {
                callback: Box::new(move |snapshot| {
                    callback_delivered
                        .lock()
                        .expect("deliveries")
                        .push(snapshot.generation().get());
                    if snapshot.generation().get() == 1 {
                        callback_receiver
                            .replace_snapshot(second.clone())
                            .expect("racing publication");
                        callback_notifier
                            .lock()
                            .expect("notifier")
                            .as_ref()
                            .expect("notifier installed")
                            .snapshot_ready();
                    }
                    DeliveryOutcome::Delivered(snapshot.generation().get())
                }),
            }),
        );
        let notifier = bridge.notifier();
        *notifier_slot.lock().expect("notifier") = Some(notifier.clone());
        notifier.snapshot_ready();

        scheduler.run_one();
        assert_eq!(scheduler.len(), 1);
        assert_eq!(*delivered.lock().expect("deliveries"), [1]);
        scheduler.run_one();
        assert_eq!(scheduler.len(), 0);
        assert_eq!(*delivered.lock().expect("deliveries"), [1, 2]);
        assert_eq!(bridge.snapshot().scheduled_count(), 2);
    }

    #[test]
    fn failed_schedule_retains_latest_and_later_notification_retries() {
        let receiver = DesktopSnapshotReceiver::empty_for_test();
        let scheduler = Arc::new(ManualScheduler::new());
        let delivered = Arc::new(AtomicU64::new(0));
        let callback_delivered = delivered.clone();
        let bridge = DesktopSnapshotBridge::with_parts(
            test_epoch(),
            receiver.clone(),
            scheduler.clone(),
            Arc::new(CallbackDelivery {
                callback: Box::new(move |snapshot| {
                    callback_delivered.store(snapshot.generation().get(), Ordering::Release);
                    DeliveryOutcome::Delivered(snapshot.generation().get())
                }),
            }),
        );
        let mut reducer = ProductReducer::new();
        reducer
            .fail_data_status(
                ProductAttemptGeneration::new(1).expect("attempt"),
                QueryErrorCode::Unavailable,
            )
            .expect("generation");
        receiver
            .replace_snapshot(reducer.snapshot())
            .expect("publication");
        let notifier = bridge.notifier();
        scheduler.fail_next();
        notifier.snapshot_ready();

        assert_eq!(scheduler.len(), 0);
        assert!(receiver.has_snapshot().expect("mailbox"));
        let failed = bridge.snapshot();
        assert!(!failed.scheduled());
        assert_eq!(failed.scheduling_failure_count(), 1);
        assert_eq!(
            failed.last_failure(),
            Some(DesktopBridgeFailureCode::EventLoopUnavailable)
        );

        notifier.snapshot_ready();
        assert_eq!(scheduler.len(), 1);
        scheduler.run_one();
        assert_eq!(delivered.load(Ordering::Acquire), 1);
        assert!(!receiver.has_snapshot().expect("mailbox"));
    }

    #[test]
    fn dropping_bridge_makes_weak_notifier_a_no_op() {
        let receiver = DesktopSnapshotReceiver::empty_for_test();
        let scheduler = Arc::new(ManualScheduler::new());
        let bridge = DesktopSnapshotBridge::with_parts(
            test_epoch(),
            receiver,
            scheduler.clone(),
            Arc::new(CallbackDelivery {
                callback: Box::new(|snapshot| {
                    DeliveryOutcome::Delivered(snapshot.generation().get())
                }),
            }),
        );
        let notifier = bridge.notifier();
        drop(bridge);
        notifier.snapshot_ready();
        assert_eq!(scheduler.len(), 0);
    }

    #[test]
    fn closed_window_closes_the_bridge_and_stops_future_scheduling() {
        let receiver = DesktopSnapshotReceiver::empty_for_test();
        let scheduler = Arc::new(ManualScheduler::new());
        let bridge = DesktopSnapshotBridge::with_parts(
            test_epoch(),
            receiver.clone(),
            scheduler.clone(),
            Arc::new(CallbackDelivery {
                callback: Box::new(|_| DeliveryOutcome::WindowClosed),
            }),
        );
        let mut reducer = ProductReducer::new();
        reducer
            .fail_data_status(
                ProductAttemptGeneration::new(1).expect("attempt"),
                QueryErrorCode::Unavailable,
            )
            .expect("generation");
        receiver
            .replace_snapshot(reducer.snapshot())
            .expect("publication");
        let notifier = bridge.notifier();
        notifier.snapshot_ready();
        scheduler.run_one();

        let closed = bridge.snapshot();
        assert_eq!(closed.phase(), DesktopBridgePhase::Closed);
        assert_eq!(
            closed.last_failure(),
            Some(DesktopBridgeFailureCode::WindowClosed)
        );
        assert!(!closed.scheduled());
        notifier.snapshot_ready();
        assert_eq!(scheduler.len(), 0);
    }

    #[test]
    fn bridge_and_observer_are_send_sync_without_a_strong_window() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<DesktopSnapshotBridge>();
        assert_send_sync::<DesktopBridgeObserver>();
    }
}
