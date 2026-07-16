use tokenmaster_engine::{RefreshOutcome, WorkerSnapshot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitReminderFailure {
    Busy,
    Clock,
    StoreUnavailable,
    InvalidData,
    CapacityExceeded,
    Cancelled,
    DeadlineExceeded,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitReminderRetryMode {
    Normal,
    Accelerated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitReminderRuntimePhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitReminderSchedulePhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenefitReminderScheduleSnapshot {
    pub(super) phase: BenefitReminderSchedulePhase,
    pub(super) reconciliation_pending: bool,
    pub(super) notification_pending: bool,
    pub(super) nearest_due_at_ms: Option<i64>,
    pub(super) retry_at_ms: Option<i64>,
    pub(super) accepted_hint_count: u64,
    pub(super) submitted_count: u64,
}

impl BenefitReminderScheduleSnapshot {
    #[must_use]
    pub const fn phase(self) -> BenefitReminderSchedulePhase {
        self.phase
    }

    #[must_use]
    pub const fn reconciliation_pending(self) -> bool {
        self.reconciliation_pending
    }

    #[must_use]
    pub const fn notification_pending(self) -> bool {
        self.notification_pending
    }

    #[must_use]
    pub const fn nearest_due_at_ms(self) -> Option<i64> {
        self.nearest_due_at_ms
    }

    #[must_use]
    pub const fn retry_at_ms(self) -> Option<i64> {
        self.retry_at_ms
    }

    #[must_use]
    pub const fn accepted_hint_count(self) -> u64 {
        self.accepted_hint_count
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.submitted_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenefitReminderRefreshSnapshot {
    pub(super) attempt_sequence: u64,
    pub(super) outcome: Option<RefreshOutcome>,
    pub(super) failure: Option<BenefitReminderFailure>,
    pub(super) retry_mode: BenefitReminderRetryMode,
    pub(super) examined_count: u16,
    pub(super) expired_count: u16,
    pub(super) suppressed_count: u16,
    pub(super) delivery_count: u16,
    pub(super) pending_due_count: u64,
    pub(super) retained_delivery_count: u64,
    pub(super) nearest_due_at_ms: Option<i64>,
    pub(super) observed_at_ms: Option<i64>,
    pub(super) elapsed_millis: u64,
    pub(super) last_success_observed_at_ms: Option<i64>,
}

impl BenefitReminderRefreshSnapshot {
    pub(super) const fn not_run() -> Self {
        Self {
            attempt_sequence: 0,
            outcome: None,
            failure: None,
            retry_mode: BenefitReminderRetryMode::Normal,
            examined_count: 0,
            expired_count: 0,
            suppressed_count: 0,
            delivery_count: 0,
            pending_due_count: 0,
            retained_delivery_count: 0,
            nearest_due_at_ms: None,
            observed_at_ms: None,
            elapsed_millis: 0,
            last_success_observed_at_ms: None,
        }
    }

    #[must_use]
    pub const fn attempt_sequence(self) -> u64 {
        self.attempt_sequence
    }

    #[must_use]
    pub const fn outcome(self) -> Option<RefreshOutcome> {
        self.outcome
    }

    #[must_use]
    pub const fn failure(self) -> Option<BenefitReminderFailure> {
        self.failure
    }

    #[must_use]
    pub const fn retry_mode(self) -> BenefitReminderRetryMode {
        self.retry_mode
    }

    #[must_use]
    pub const fn examined_count(self) -> u16 {
        self.examined_count
    }

    #[must_use]
    pub const fn expired_count(self) -> u16 {
        self.expired_count
    }

    #[must_use]
    pub const fn suppressed_count(self) -> u16 {
        self.suppressed_count
    }

    #[must_use]
    pub const fn delivery_count(self) -> u16 {
        self.delivery_count
    }

    #[must_use]
    pub const fn pending_due_count(self) -> u64 {
        self.pending_due_count
    }

    #[must_use]
    pub const fn retained_delivery_count(self) -> u64 {
        self.retained_delivery_count
    }

    #[must_use]
    pub const fn nearest_due_at_ms(self) -> Option<i64> {
        self.nearest_due_at_ms
    }

    #[must_use]
    pub const fn observed_at_ms(self) -> Option<i64> {
        self.observed_at_ms
    }

    #[must_use]
    pub const fn elapsed_millis(self) -> u64 {
        self.elapsed_millis
    }

    #[must_use]
    pub const fn last_success_observed_at_ms(self) -> Option<i64> {
        self.last_success_observed_at_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenefitReminderRuntimeSnapshot {
    pub(super) phase: BenefitReminderRuntimePhase,
    pub(super) schedule: BenefitReminderScheduleSnapshot,
    pub(super) worker: WorkerSnapshot,
    pub(super) refresh: BenefitReminderRefreshSnapshot,
}

impl BenefitReminderRuntimeSnapshot {
    #[must_use]
    pub const fn phase(self) -> BenefitReminderRuntimePhase {
        self.phase
    }

    #[must_use]
    pub const fn schedule(self) -> BenefitReminderScheduleSnapshot {
        self.schedule
    }

    #[must_use]
    pub const fn worker(self) -> WorkerSnapshot {
        self.worker
    }

    #[must_use]
    pub const fn refresh(self) -> BenefitReminderRefreshSnapshot {
        self.refresh
    }
}
