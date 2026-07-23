use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
};

use crate::{
    MainWindow,
    bridge::{EventScheduler, ScheduleError, SlintEventScheduler, saturating_increment},
    ui::apply_in_app_notification_batch,
};

pub const MAX_DESKTOP_IN_APP_NOTIFICATIONS: usize = 256;
const MAX_NOTIFICATION_LABEL_KEY_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopNotificationKind {
    BankedRateLimitReset,
    UsageCredit,
    TemporaryUsage,
    Unknown,
}

impl DesktopNotificationKind {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::BankedRateLimitReset => "banked_rate_limit_reset",
            Self::UsageCredit => "usage_credit",
            Self::TemporaryUsage => "temporary_usage",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::BankedRateLimitReset => "Banked rate-limit reset",
            Self::UsageCredit => "Usage credit",
            Self::TemporaryUsage => "Temporary usage",
            Self::Unknown => "Benefit",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct DesktopInAppNotification {
    kind: DesktopNotificationKind,
    quantity: u64,
    label_key: Arc<str>,
    lead_seconds: u32,
    due_at_ms: i64,
    expiry_at_ms: i64,
    delivered_at_ms: i64,
}

impl DesktopInAppNotification {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: DesktopNotificationKind,
        quantity: u64,
        label_key: &str,
        lead_seconds: u32,
        due_at_ms: i64,
        expiry_at_ms: i64,
        delivered_at_ms: i64,
    ) -> Result<Self, DesktopNotificationError> {
        let expected_lead_millis = i64::from(lead_seconds)
            .checked_mul(1_000)
            .ok_or_else(DesktopNotificationError::invalid_value)?;
        if quantity == 0
            || !valid_label_key(label_key)
            || lead_seconds == 0
            || due_at_ms <= 0
            || expiry_at_ms <= due_at_ms
            || delivered_at_ms < due_at_ms
            || delivered_at_ms >= expiry_at_ms
            || expiry_at_ms.checked_sub(due_at_ms) != Some(expected_lead_millis)
        {
            return Err(DesktopNotificationError::invalid_value());
        }
        Ok(Self {
            kind,
            quantity,
            label_key: Arc::from(label_key),
            lead_seconds,
            due_at_ms,
            expiry_at_ms,
            delivered_at_ms,
        })
    }

    #[must_use]
    pub const fn kind(&self) -> DesktopNotificationKind {
        self.kind
    }

    #[must_use]
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }

    #[must_use]
    pub fn label_key(&self) -> &str {
        &self.label_key
    }

    #[must_use]
    pub const fn lead_seconds(&self) -> u32 {
        self.lead_seconds
    }

    #[must_use]
    pub const fn due_at_ms(&self) -> i64 {
        self.due_at_ms
    }

    #[must_use]
    pub const fn expiry_at_ms(&self) -> i64 {
        self.expiry_at_ms
    }

    #[must_use]
    pub const fn delivered_at_ms(&self) -> i64 {
        self.delivered_at_ms
    }
}

impl fmt::Debug for DesktopInAppNotification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopInAppNotification")
            .field("kind", &self.kind)
            .field("quantity", &self.quantity)
            .field("label_key", &"[redacted]")
            .field("lead_seconds", &self.lead_seconds)
            .field("due_at_ms", &self.due_at_ms)
            .field("expiry_at_ms", &self.expiry_at_ms)
            .field("delivered_at_ms", &self.delivered_at_ms)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct DesktopInAppNotificationBatch {
    rows: Box<[DesktopInAppNotification]>,
}

pub(crate) type SharedInAppNotificationBatch = Arc<Mutex<Option<DesktopInAppNotificationBatch>>>;

impl DesktopInAppNotificationBatch {
    pub fn new(rows: Vec<DesktopInAppNotification>) -> Result<Self, DesktopNotificationError> {
        if rows.is_empty() {
            return Err(DesktopNotificationError::invalid_batch());
        }
        if rows.len() > MAX_DESKTOP_IN_APP_NOTIFICATIONS {
            return Err(DesktopNotificationError::capacity_exceeded());
        }
        Ok(Self {
            rows: rows.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn rows(&self) -> &[DesktopInAppNotification] {
        &self.rows
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl fmt::Debug for DesktopInAppNotificationBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopInAppNotificationBatch")
            .field("row_count", &self.rows.len())
            .finish()
    }
}

fn valid_label_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NOTIFICATION_LABEL_KEY_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub trait DesktopNotificationPresentationReceipt: Send + Sync + 'static {
    fn presented(&self);
    fn failed(&self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopNotificationErrorCode {
    InvalidValue,
    InvalidBatch,
    CapacityExceeded,
    Busy,
    Closed,
    StateUnavailable,
}

impl DesktopNotificationErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidValue => "invalid_value",
            Self::InvalidBatch => "invalid_batch",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::Busy => "busy",
            Self::Closed => "closed",
            Self::StateUnavailable => "state_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopNotificationError {
    code: DesktopNotificationErrorCode,
}

impl DesktopNotificationError {
    const fn invalid_value() -> Self {
        Self {
            code: DesktopNotificationErrorCode::InvalidValue,
        }
    }

    const fn invalid_batch() -> Self {
        Self {
            code: DesktopNotificationErrorCode::InvalidBatch,
        }
    }

    const fn capacity_exceeded() -> Self {
        Self {
            code: DesktopNotificationErrorCode::CapacityExceeded,
        }
    }

    const fn busy() -> Self {
        Self {
            code: DesktopNotificationErrorCode::Busy,
        }
    }

    const fn closed() -> Self {
        Self {
            code: DesktopNotificationErrorCode::Closed,
        }
    }

    const fn state_unavailable() -> Self {
        Self {
            code: DesktopNotificationErrorCode::StateUnavailable,
        }
    }

    #[must_use]
    pub const fn code(self) -> DesktopNotificationErrorCode {
        self.code
    }

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        self.code.stable_code()
    }
}

impl fmt::Display for DesktopNotificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

impl std::error::Error for DesktopNotificationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopNotificationBridgePhase {
    Running,
    Closed,
    Faulted,
}

impl DesktopNotificationBridgePhase {
    const fn encoded(self) -> u8 {
        match self {
            Self::Running => 0,
            Self::Closed => 1,
            Self::Faulted => 2,
        }
    }

    const fn decode(value: u8) -> Self {
        match value {
            0 => Self::Running,
            1 => Self::Closed,
            _ => Self::Faulted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopNotificationBridgeFailureCode {
    EventLoopUnavailable,
    EventLoopTerminated,
    StaleEpoch,
    WindowClosed,
    StateUnavailable,
}

impl DesktopNotificationBridgeFailureCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::EventLoopUnavailable => "event_loop_unavailable",
            Self::EventLoopTerminated => "event_loop_terminated",
            Self::StaleEpoch => "stale_epoch",
            Self::WindowClosed => "window_closed",
            Self::StateUnavailable => "state_unavailable",
        }
    }

    const fn encoded(self) -> u8 {
        match self {
            Self::EventLoopUnavailable => 1,
            Self::EventLoopTerminated => 2,
            Self::StaleEpoch => 3,
            Self::WindowClosed => 4,
            Self::StateUnavailable => 5,
        }
    }

    const fn decode(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::EventLoopUnavailable),
            2 => Some(Self::EventLoopTerminated),
            3 => Some(Self::StaleEpoch),
            4 => Some(Self::WindowClosed),
            5 => Some(Self::StateUnavailable),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopNotificationBridgeSnapshot {
    phase: DesktopNotificationBridgePhase,
    scheduled: bool,
    scheduled_count: u64,
    presented_count: u64,
    failed_count: u64,
    scheduling_failure_count: u64,
    last_failure: Option<DesktopNotificationBridgeFailureCode>,
}

impl DesktopNotificationBridgeSnapshot {
    #[must_use]
    pub const fn phase(self) -> DesktopNotificationBridgePhase {
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
    pub const fn presented_count(self) -> u64 {
        self.presented_count
    }

    #[must_use]
    pub const fn failed_count(self) -> u64 {
        self.failed_count
    }

    #[must_use]
    pub const fn scheduling_failure_count(self) -> u64 {
        self.scheduling_failure_count
    }

    #[must_use]
    pub const fn last_failure(self) -> Option<DesktopNotificationBridgeFailureCode> {
        self.last_failure
    }
}

pub(crate) struct NotificationEpochState {
    next: AtomicU64,
    active: AtomicU64,
}

impl NotificationEpochState {
    pub(crate) const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
            active: AtomicU64::new(0),
        }
    }

    fn activate(&self) -> Result<u64, DesktopNotificationError> {
        let epoch = self
            .next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map_err(|_| DesktopNotificationError::state_unavailable())?;
        if epoch == 0 {
            return Err(DesktopNotificationError::state_unavailable());
        }
        self.active.store(epoch, Ordering::Release);
        Ok(epoch)
    }

    fn deactivate(&self, epoch: u64) {
        let _ = self
            .active
            .compare_exchange(epoch, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

enum NotificationDeliveryOutcome {
    Presented,
    Stale,
    WindowClosed,
    StateUnavailable,
}

trait NotificationDelivery: Send + Sync + 'static {
    fn deliver(&self, batch: &DesktopInAppNotificationBatch) -> NotificationDeliveryOutcome;
}

struct SlintNotificationDelivery {
    epoch: u64,
    epochs: Arc<NotificationEpochState>,
    window: slint::Weak<MainWindow>,
    latest_batch: SharedInAppNotificationBatch,
}

impl NotificationDelivery for SlintNotificationDelivery {
    fn deliver(&self, batch: &DesktopInAppNotificationBatch) -> NotificationDeliveryOutcome {
        if self.epochs.active.load(Ordering::Acquire) != self.epoch {
            return NotificationDeliveryOutcome::Stale;
        }
        let Some(window) = self.window.upgrade() else {
            return NotificationDeliveryOutcome::WindowClosed;
        };
        let Ok(mut latest_batch) = self.latest_batch.lock() else {
            return NotificationDeliveryOutcome::StateUnavailable;
        };
        if !apply_in_app_notification_batch(&window, batch) {
            return NotificationDeliveryOutcome::StateUnavailable;
        }
        *latest_batch = Some(batch.clone());
        NotificationDeliveryOutcome::Presented
    }
}

struct NotificationBridgeInner {
    scheduler: Arc<dyn EventScheduler>,
    delivery: Arc<dyn NotificationDelivery>,
    phase: AtomicU8,
    scheduled: AtomicBool,
    scheduled_count: AtomicU64,
    presented_count: AtomicU64,
    failed_count: AtomicU64,
    scheduling_failure_count: AtomicU64,
    last_failure: AtomicU8,
}

impl NotificationBridgeInner {
    fn new(
        scheduler: Arc<dyn EventScheduler>,
        delivery: Arc<dyn NotificationDelivery>,
    ) -> Arc<Self> {
        Arc::new(Self {
            scheduler,
            delivery,
            phase: AtomicU8::new(DesktopNotificationBridgePhase::Running.encoded()),
            scheduled: AtomicBool::new(false),
            scheduled_count: AtomicU64::new(0),
            presented_count: AtomicU64::new(0),
            failed_count: AtomicU64::new(0),
            scheduling_failure_count: AtomicU64::new(0),
            last_failure: AtomicU8::new(0),
        })
    }

    fn present(
        self: &Arc<Self>,
        batch: DesktopInAppNotificationBatch,
        receipt: Arc<dyn DesktopNotificationPresentationReceipt>,
    ) -> Result<(), DesktopNotificationError> {
        if self.phase() != DesktopNotificationBridgePhase::Running {
            return Err(DesktopNotificationError::closed());
        }
        if self
            .scheduled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(DesktopNotificationError::busy());
        }
        let inner = Arc::clone(self);
        match self
            .scheduler
            .schedule(Box::new(move || inner.run(batch, receipt)))
        {
            Ok(()) => {
                saturating_increment(&self.scheduled_count);
                Ok(())
            }
            Err(error) => {
                self.scheduled.store(false, Ordering::Release);
                saturating_increment(&self.scheduling_failure_count);
                self.record_schedule_error(error);
                Err(DesktopNotificationError::state_unavailable())
            }
        }
    }

    fn run(
        &self,
        batch: DesktopInAppNotificationBatch,
        receipt: Arc<dyn DesktopNotificationPresentationReceipt>,
    ) {
        if self.phase() != DesktopNotificationBridgePhase::Running {
            saturating_increment(&self.failed_count);
            self.scheduled.store(false, Ordering::Release);
            receipt.failed();
            return;
        }
        let presented = match self.delivery.deliver(&batch) {
            NotificationDeliveryOutcome::Presented => {
                self.last_failure.store(0, Ordering::Release);
                saturating_increment(&self.presented_count);
                true
            }
            NotificationDeliveryOutcome::Stale => {
                self.fail(DesktopNotificationBridgeFailureCode::StaleEpoch, false);
                false
            }
            NotificationDeliveryOutcome::WindowClosed => {
                self.fail(DesktopNotificationBridgeFailureCode::WindowClosed, true);
                false
            }
            NotificationDeliveryOutcome::StateUnavailable => {
                self.fail(DesktopNotificationBridgeFailureCode::StateUnavailable, true);
                false
            }
        };
        self.scheduled.store(false, Ordering::Release);
        if presented {
            receipt.presented();
        } else {
            receipt.failed();
        }
    }

    fn record_schedule_error(&self, error: ScheduleError) {
        match error {
            ScheduleError::Unavailable => {
                self.last_failure.store(
                    DesktopNotificationBridgeFailureCode::EventLoopUnavailable.encoded(),
                    Ordering::Release,
                );
            }
            ScheduleError::Terminated => self.fail(
                DesktopNotificationBridgeFailureCode::EventLoopTerminated,
                true,
            ),
            ScheduleError::Internal => {
                self.fail(DesktopNotificationBridgeFailureCode::StateUnavailable, true)
            }
        }
    }

    fn fail(&self, failure: DesktopNotificationBridgeFailureCode, close: bool) {
        self.last_failure
            .store(failure.encoded(), Ordering::Release);
        saturating_increment(&self.failed_count);
        if close {
            self.phase.store(
                DesktopNotificationBridgePhase::Closed.encoded(),
                Ordering::Release,
            );
        }
    }

    fn close(&self) {
        self.phase.store(
            DesktopNotificationBridgePhase::Closed.encoded(),
            Ordering::Release,
        );
    }

    fn phase(&self) -> DesktopNotificationBridgePhase {
        DesktopNotificationBridgePhase::decode(self.phase.load(Ordering::Acquire))
    }

    fn snapshot(&self) -> DesktopNotificationBridgeSnapshot {
        DesktopNotificationBridgeSnapshot {
            phase: self.phase(),
            scheduled: self.scheduled.load(Ordering::Acquire),
            scheduled_count: self.scheduled_count.load(Ordering::Acquire),
            presented_count: self.presented_count.load(Ordering::Acquire),
            failed_count: self.failed_count.load(Ordering::Acquire),
            scheduling_failure_count: self.scheduling_failure_count.load(Ordering::Acquire),
            last_failure: DesktopNotificationBridgeFailureCode::decode(
                self.last_failure.load(Ordering::Acquire),
            ),
        }
    }
}

pub struct DesktopInAppNotificationBridge {
    epoch: u64,
    epochs: Arc<NotificationEpochState>,
    inner: Arc<NotificationBridgeInner>,
}

impl DesktopInAppNotificationBridge {
    pub(crate) fn new(
        epochs: Arc<NotificationEpochState>,
        window: slint::Weak<MainWindow>,
        latest_batch: SharedInAppNotificationBatch,
    ) -> Result<Self, DesktopNotificationError> {
        let epoch = epochs.activate()?;
        let delivery = Arc::new(SlintNotificationDelivery {
            epoch,
            epochs: Arc::clone(&epochs),
            window,
            latest_batch,
        });
        Ok(Self {
            epoch,
            epochs,
            inner: NotificationBridgeInner::new(Arc::new(SlintEventScheduler), delivery),
        })
    }

    pub fn present(
        &self,
        batch: DesktopInAppNotificationBatch,
        receipt: Arc<dyn DesktopNotificationPresentationReceipt>,
    ) -> Result<(), DesktopNotificationError> {
        self.inner.present(batch, receipt)
    }

    #[must_use]
    pub fn snapshot(&self) -> DesktopNotificationBridgeSnapshot {
        self.inner.snapshot()
    }
}

impl Drop for DesktopInAppNotificationBridge {
    fn drop(&mut self) {
        self.epochs.deactivate(self.epoch);
        self.inner.close();
    }
}
