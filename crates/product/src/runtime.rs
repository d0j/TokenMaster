use std::num::NonZeroU64;

use tokenmaster_engine::{RefreshOutcome, WorkerPhase, WorkerSnapshot};
use tokenmaster_runtime::{
    BenefitReminderFailure, BenefitReminderRetryMode, BenefitReminderRuntimePhase,
    BenefitReminderRuntimeSnapshot, BenefitReminderSchedulePhase, EnginePublicationQuality,
    GitRefreshFailure, GitRuntimePhase, GitRuntimeSnapshot, LivePhase, LiveRefreshKind,
    LiveRuntimeSnapshot, ProviderQuotaRefreshStage, ProviderQuotaRetryMode,
    ProviderQuotaRuntimePhase, ProviderQuotaRuntimeSnapshot, RuntimeErrorCode, SchedulerPhase,
    WatcherHealth,
};

use crate::ProductSectionKind;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductRuntimeGeneration(NonZeroU64);

impl ProductRuntimeGeneration {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductRuntimeLifecycle {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductSchedulerLifecycle {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductWorkerLifecycle {
    Running,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductRefreshOutcome {
    Completed,
    Busy,
    Cancelled,
    DeadlineExceeded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductRetryMode {
    Normal,
    Accelerated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductUsageRefreshKind {
    None,
    Incremental,
    FullRebuild,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductRuntimeFailureCode {
    UsageRefresh,
    QuotaDiscovery,
    QuotaClock,
    QuotaTransport,
    QuotaPublication,
    BenefitPublication,
    RuntimeControl,
    ReminderBusy,
    ReminderClock,
    ReminderStoreUnavailable,
    ReminderInvalidData,
    ReminderCapacityExceeded,
    ReminderCancelled,
    ReminderDeadlineExceeded,
    ReminderControl,
    GitBackend,
    GitPublication,
    GitControl,
    Unclassified,
}

impl ProductRuntimeFailureCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::UsageRefresh => "usage_refresh",
            Self::QuotaDiscovery => "quota_discovery",
            Self::QuotaClock => "quota_clock",
            Self::QuotaTransport => "quota_transport",
            Self::QuotaPublication => "quota_publication",
            Self::BenefitPublication => "benefit_publication",
            Self::RuntimeControl => "runtime_control",
            Self::ReminderBusy => "reminder_busy",
            Self::ReminderClock => "reminder_clock",
            Self::ReminderStoreUnavailable => "reminder_store_unavailable",
            Self::ReminderInvalidData => "reminder_invalid_data",
            Self::ReminderCapacityExceeded => "reminder_capacity_exceeded",
            Self::ReminderCancelled => "reminder_cancelled",
            Self::ReminderDeadlineExceeded => "reminder_deadline_exceeded",
            Self::ReminderControl => "reminder_control",
            Self::GitBackend => "git_backend",
            Self::GitPublication => "git_publication",
            Self::GitControl => "git_control",
            Self::Unclassified => "unclassified",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductRuntimeObservationError {
    InvalidConfiguration,
    ProviderUnavailable,
    StoreUnavailable,
    Busy,
    Closed,
    Faulted,
    Internal,
}

impl ProductRuntimeObservationError {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "invalid_configuration",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::StoreUnavailable => "store_unavailable",
            Self::Busy => "busy",
            Self::Closed => "closed",
            Self::Faulted => "faulted",
            Self::Internal => "internal",
        }
    }
}

impl From<RuntimeErrorCode> for ProductRuntimeObservationError {
    fn from(value: RuntimeErrorCode) -> Self {
        match value {
            RuntimeErrorCode::InvalidConfiguration => Self::InvalidConfiguration,
            RuntimeErrorCode::ProviderUnavailable => Self::ProviderUnavailable,
            RuntimeErrorCode::StoreUnavailable => Self::StoreUnavailable,
            RuntimeErrorCode::Busy => Self::Busy,
            RuntimeErrorCode::Closed => Self::Closed,
            RuntimeErrorCode::Faulted => Self::Faulted,
            RuntimeErrorCode::Internal => Self::Internal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductRuntimeCore {
    lifecycle: ProductRuntimeLifecycle,
    scheduler: ProductSchedulerLifecycle,
    worker: ProductWorkerLifecycle,
    recovery_pending: bool,
    pending_work_count: usize,
    accepted_request_count: u64,
    submitted_count: u64,
}

impl ProductRuntimeCore {
    const fn is_degraded(self) -> bool {
        !matches!(self.lifecycle, ProductRuntimeLifecycle::Running)
            || !matches!(self.scheduler, ProductSchedulerLifecycle::Running)
            || !matches!(self.worker, ProductWorkerLifecycle::Running)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductUsageRuntimeHealth {
    core: ProductRuntimeCore,
    watcher_degraded: bool,
    watcher_root_count: usize,
    engine_generation: u64,
    refresh_kind: ProductUsageRefreshKind,
    outcome: Option<ProductRefreshOutcome>,
    failure: Option<ProductRuntimeFailureCode>,
    initial_import_in_progress: bool,
    completed_refresh_count: u64,
    busy_refresh_count: u64,
    cancelled_refresh_count: u64,
    deadline_exceeded_refresh_count: u64,
    failed_refresh_count: u64,
    counter_overflowed: bool,
}

impl ProductUsageRuntimeHealth {
    #[must_use]
    pub const fn lifecycle(self) -> ProductRuntimeLifecycle {
        self.core.lifecycle
    }

    #[must_use]
    pub const fn scheduler(self) -> ProductSchedulerLifecycle {
        self.core.scheduler
    }

    #[must_use]
    pub const fn worker(self) -> ProductWorkerLifecycle {
        self.core.worker
    }

    #[must_use]
    pub const fn recovery_pending(self) -> bool {
        self.core.recovery_pending
    }

    #[must_use]
    pub const fn pending_work_count(self) -> usize {
        self.core.pending_work_count
    }

    #[must_use]
    pub const fn accepted_request_count(self) -> u64 {
        self.core.accepted_request_count
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.core.submitted_count
    }

    #[must_use]
    pub const fn watcher_degraded(self) -> bool {
        self.watcher_degraded
    }

    #[must_use]
    pub const fn watcher_root_count(self) -> usize {
        self.watcher_root_count
    }

    #[must_use]
    pub const fn engine_generation(self) -> u64 {
        self.engine_generation
    }

    #[must_use]
    pub const fn refresh_kind(self) -> ProductUsageRefreshKind {
        self.refresh_kind
    }

    #[must_use]
    pub const fn outcome(self) -> Option<ProductRefreshOutcome> {
        self.outcome
    }

    #[must_use]
    pub const fn failure(self) -> Option<ProductRuntimeFailureCode> {
        self.failure
    }

    #[must_use]
    pub const fn initial_import_in_progress(self) -> bool {
        self.initial_import_in_progress
    }

    #[must_use]
    pub const fn completed_refresh_count(self) -> u64 {
        self.completed_refresh_count
    }

    #[must_use]
    pub const fn busy_refresh_count(self) -> u64 {
        self.busy_refresh_count
    }

    #[must_use]
    pub const fn cancelled_refresh_count(self) -> u64 {
        self.cancelled_refresh_count
    }

    #[must_use]
    pub const fn deadline_exceeded_refresh_count(self) -> u64 {
        self.deadline_exceeded_refresh_count
    }

    #[must_use]
    pub const fn failed_refresh_count(self) -> u64 {
        self.failed_refresh_count
    }

    #[must_use]
    pub const fn counter_overflowed(self) -> bool {
        self.counter_overflowed
    }

    pub(crate) const fn is_degraded(self) -> bool {
        self.core.is_degraded()
            || self.watcher_degraded
            || self.failure.is_some()
            || matches!(
                self.outcome,
                Some(
                    ProductRefreshOutcome::Busy
                        | ProductRefreshOutcome::Cancelled
                        | ProductRefreshOutcome::DeadlineExceeded
                        | ProductRefreshOutcome::Failed
                )
            )
    }
}

impl From<LiveRuntimeSnapshot> for ProductUsageRuntimeHealth {
    fn from(value: LiveRuntimeSnapshot) -> Self {
        let scheduler = value.scheduler();
        let worker = value.worker();
        let refresh = value.refresh();
        let diagnostics = value.engine().diagnostics();
        Self {
            core: ProductRuntimeCore {
                lifecycle: map_live_lifecycle(value.phase()),
                scheduler: map_scheduler(scheduler.phase()),
                worker: map_worker(worker.phase()),
                recovery_pending: scheduler.dirty() || scheduler.force_reconcile(),
                pending_work_count: pending_work(worker),
                accepted_request_count: scheduler.accepted_hint_count(),
                submitted_count: scheduler.submitted_count(),
            },
            watcher_degraded: scheduler.watcher_health() == WatcherHealth::Degraded,
            watcher_root_count: value.watcher().root_count(),
            engine_generation: value.engine().generation().get(),
            refresh_kind: match refresh.kind() {
                LiveRefreshKind::None => ProductUsageRefreshKind::None,
                LiveRefreshKind::Incremental => ProductUsageRefreshKind::Incremental,
                LiveRefreshKind::FullRebuild => ProductUsageRefreshKind::FullRebuild,
            },
            outcome: refresh.outcome().map(map_outcome),
            failure: refresh
                .error()
                .map(|_| ProductRuntimeFailureCode::UsageRefresh),
            initial_import_in_progress: refresh.kind() == LiveRefreshKind::FullRebuild
                && refresh.outcome().is_none()
                && refresh.error().is_none()
                && value.engine().quality() == EnginePublicationQuality::Empty,
            completed_refresh_count: diagnostics.completed_refreshes(),
            busy_refresh_count: diagnostics.busy_refreshes(),
            cancelled_refresh_count: diagnostics.cancelled_refreshes(),
            deadline_exceeded_refresh_count: diagnostics.deadline_exceeded_refreshes(),
            failed_refresh_count: diagnostics.failed_refreshes(),
            counter_overflowed: diagnostics.counter_overflowed(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductQuotaRuntimeHealth {
    core: ProductRuntimeCore,
    source_attempt_sequence: u64,
    retry_mode: ProductRetryMode,
    outcome: Option<ProductRefreshOutcome>,
    quota_failure: Option<ProductRuntimeFailureCode>,
    benefit_failure: Option<ProductRuntimeFailureCode>,
    quota_observation_count: u16,
    quota_processed_count: u16,
    quota_changed_count: u16,
    started_count: u16,
    advanced_count: u16,
    duplicate_count: u16,
    stale_count: u16,
    allowance_change_count: u16,
    reset_count: u16,
    benefit_observation_count: u8,
    benefit_processed_count: u8,
    benefit_changed_count: u8,
    benefit_freshness_only_count: u8,
    benefit_duplicate_count: u8,
    benefit_stale_count: u8,
    benefit_lot_change_count: u16,
    benefit_pending_due_count: u16,
}

impl ProductQuotaRuntimeHealth {
    #[must_use]
    pub const fn lifecycle(self) -> ProductRuntimeLifecycle {
        self.core.lifecycle
    }

    #[must_use]
    pub const fn recovery_pending(self) -> bool {
        self.core.recovery_pending
    }

    #[must_use]
    pub const fn scheduler(self) -> ProductSchedulerLifecycle {
        self.core.scheduler
    }

    #[must_use]
    pub const fn worker(self) -> ProductWorkerLifecycle {
        self.core.worker
    }

    #[must_use]
    pub const fn pending_work_count(self) -> usize {
        self.core.pending_work_count
    }

    #[must_use]
    pub const fn accepted_request_count(self) -> u64 {
        self.core.accepted_request_count
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.core.submitted_count
    }

    #[must_use]
    pub const fn source_attempt_sequence(self) -> u64 {
        self.source_attempt_sequence
    }

    #[must_use]
    pub const fn retry_mode(self) -> ProductRetryMode {
        self.retry_mode
    }

    #[must_use]
    pub const fn outcome(self) -> Option<ProductRefreshOutcome> {
        self.outcome
    }

    #[must_use]
    pub const fn quota_failure(self) -> Option<ProductRuntimeFailureCode> {
        self.quota_failure
    }

    #[must_use]
    pub const fn benefit_failure(self) -> Option<ProductRuntimeFailureCode> {
        self.benefit_failure
    }

    #[must_use]
    pub const fn quota_observation_count(self) -> u16 {
        self.quota_observation_count
    }

    #[must_use]
    pub const fn quota_processed_count(self) -> u16 {
        self.quota_processed_count
    }

    #[must_use]
    pub const fn quota_changed_count(self) -> u16 {
        self.quota_changed_count
    }

    #[must_use]
    pub const fn started_count(self) -> u16 {
        self.started_count
    }

    #[must_use]
    pub const fn advanced_count(self) -> u16 {
        self.advanced_count
    }

    #[must_use]
    pub const fn duplicate_count(self) -> u16 {
        self.duplicate_count
    }

    #[must_use]
    pub const fn stale_count(self) -> u16 {
        self.stale_count
    }

    #[must_use]
    pub const fn allowance_change_count(self) -> u16 {
        self.allowance_change_count
    }

    #[must_use]
    pub const fn reset_count(self) -> u16 {
        self.reset_count
    }

    #[must_use]
    pub const fn benefit_observation_count(self) -> u8 {
        self.benefit_observation_count
    }

    #[must_use]
    pub const fn benefit_processed_count(self) -> u8 {
        self.benefit_processed_count
    }

    #[must_use]
    pub const fn benefit_changed_count(self) -> u8 {
        self.benefit_changed_count
    }

    #[must_use]
    pub const fn benefit_freshness_only_count(self) -> u8 {
        self.benefit_freshness_only_count
    }

    #[must_use]
    pub const fn benefit_duplicate_count(self) -> u8 {
        self.benefit_duplicate_count
    }

    #[must_use]
    pub const fn benefit_stale_count(self) -> u8 {
        self.benefit_stale_count
    }

    #[must_use]
    pub const fn benefit_lot_change_count(self) -> u16 {
        self.benefit_lot_change_count
    }

    #[must_use]
    pub const fn benefit_pending_due_count(self) -> u16 {
        self.benefit_pending_due_count
    }

    pub(crate) const fn quota_is_degraded(self) -> bool {
        self.core.is_degraded() || self.quota_failure.is_some()
    }

    pub(crate) const fn benefit_is_degraded(self) -> bool {
        self.core.is_degraded() || self.benefit_failure.is_some()
    }
}

impl From<ProviderQuotaRuntimeSnapshot> for ProductQuotaRuntimeHealth {
    fn from(value: ProviderQuotaRuntimeSnapshot) -> Self {
        let schedule = value.schedule();
        let worker = value.worker();
        let refresh = value.refresh();
        let source_failure = refresh.failure().map(|failure| match failure.stage() {
            ProviderQuotaRefreshStage::Discovery => ProductRuntimeFailureCode::QuotaDiscovery,
            ProviderQuotaRefreshStage::Clock => ProductRuntimeFailureCode::QuotaClock,
            ProviderQuotaRefreshStage::Transport => ProductRuntimeFailureCode::QuotaTransport,
            ProviderQuotaRefreshStage::Publication
            | ProviderQuotaRefreshStage::QuotaPublication => {
                ProductRuntimeFailureCode::QuotaPublication
            }
            ProviderQuotaRefreshStage::BenefitPublication => {
                ProductRuntimeFailureCode::BenefitPublication
            }
            ProviderQuotaRefreshStage::Control => ProductRuntimeFailureCode::RuntimeControl,
        });
        let mut quota_failure = refresh
            .quota_failure()
            .map(|_| ProductRuntimeFailureCode::QuotaPublication);
        let mut benefit_failure = refresh
            .benefit_failure()
            .map(|_| ProductRuntimeFailureCode::BenefitPublication);
        if let Some(failure) = source_failure {
            match failure {
                ProductRuntimeFailureCode::BenefitPublication => {
                    benefit_failure = Some(failure);
                }
                ProductRuntimeFailureCode::QuotaPublication => {
                    quota_failure = Some(failure);
                }
                _ => {
                    quota_failure = Some(failure);
                    benefit_failure = Some(failure);
                }
            }
        }
        if refresh
            .outcome()
            .is_some_and(|outcome| outcome != RefreshOutcome::Completed)
            && quota_failure.is_none()
            && benefit_failure.is_none()
        {
            quota_failure = Some(ProductRuntimeFailureCode::Unclassified);
            benefit_failure = Some(ProductRuntimeFailureCode::Unclassified);
        }
        Self {
            core: ProductRuntimeCore {
                lifecycle: map_quota_lifecycle(value.phase()),
                scheduler: map_scheduler(schedule.phase()),
                worker: map_worker(worker.phase()),
                recovery_pending: schedule.refresh_pending(),
                pending_work_count: pending_work(worker),
                accepted_request_count: schedule.accepted_refresh_count(),
                submitted_count: schedule.submitted_count(),
            },
            source_attempt_sequence: refresh.attempt_sequence(),
            retry_mode: map_quota_retry(refresh.retry_mode()),
            outcome: refresh.outcome().map(map_outcome),
            quota_failure,
            benefit_failure,
            quota_observation_count: refresh.quota_observation_count(),
            quota_processed_count: refresh.quota_processed_count(),
            quota_changed_count: refresh.quota_changed_count(),
            started_count: refresh.started_count(),
            advanced_count: refresh.advanced_count(),
            duplicate_count: refresh.duplicate_count(),
            stale_count: refresh.stale_count(),
            allowance_change_count: refresh.allowance_change_count(),
            reset_count: refresh.reset_count(),
            benefit_observation_count: refresh.benefit_observation_count(),
            benefit_processed_count: refresh.benefit_processed_count(),
            benefit_changed_count: refresh.benefit_changed_count(),
            benefit_freshness_only_count: refresh.benefit_freshness_only_count(),
            benefit_duplicate_count: refresh.benefit_duplicate_count(),
            benefit_stale_count: refresh.benefit_stale_count(),
            benefit_lot_change_count: refresh.benefit_lot_change_count(),
            benefit_pending_due_count: refresh.benefit_pending_due_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductReminderRuntimeHealth {
    core: ProductRuntimeCore,
    source_attempt_sequence: u64,
    retry_mode: ProductRetryMode,
    outcome: Option<ProductRefreshOutcome>,
    failure: Option<ProductRuntimeFailureCode>,
    notification_pending: bool,
    examined_count: u16,
    expired_count: u16,
    suppressed_count: u16,
    delivery_count: u16,
    pending_due_count: u64,
    retained_delivery_count: u64,
    nearest_due_at_ms: Option<i64>,
}

impl ProductReminderRuntimeHealth {
    #[must_use]
    pub const fn lifecycle(self) -> ProductRuntimeLifecycle {
        self.core.lifecycle
    }

    #[must_use]
    pub const fn recovery_pending(self) -> bool {
        self.core.recovery_pending
    }

    #[must_use]
    pub const fn scheduler(self) -> ProductSchedulerLifecycle {
        self.core.scheduler
    }

    #[must_use]
    pub const fn worker(self) -> ProductWorkerLifecycle {
        self.core.worker
    }

    #[must_use]
    pub const fn pending_work_count(self) -> usize {
        self.core.pending_work_count
    }

    #[must_use]
    pub const fn accepted_request_count(self) -> u64 {
        self.core.accepted_request_count
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.core.submitted_count
    }

    #[must_use]
    pub const fn failure(self) -> Option<ProductRuntimeFailureCode> {
        self.failure
    }

    #[must_use]
    pub const fn notification_pending(self) -> bool {
        self.notification_pending
    }

    #[must_use]
    pub const fn source_attempt_sequence(self) -> u64 {
        self.source_attempt_sequence
    }

    #[must_use]
    pub const fn retry_mode(self) -> ProductRetryMode {
        self.retry_mode
    }

    #[must_use]
    pub const fn outcome(self) -> Option<ProductRefreshOutcome> {
        self.outcome
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

    pub(crate) const fn is_degraded(self) -> bool {
        self.core.is_degraded() || self.failure.is_some()
    }
}

impl From<BenefitReminderRuntimeSnapshot> for ProductReminderRuntimeHealth {
    fn from(value: BenefitReminderRuntimeSnapshot) -> Self {
        let schedule = value.schedule();
        let worker = value.worker();
        let refresh = value.refresh();
        let mut failure = refresh.failure().map(map_reminder_failure);
        if refresh
            .outcome()
            .is_some_and(|outcome| outcome != RefreshOutcome::Completed)
            && failure.is_none()
        {
            failure = Some(ProductRuntimeFailureCode::Unclassified);
        }
        Self {
            core: ProductRuntimeCore {
                lifecycle: map_reminder_lifecycle(value.phase()),
                scheduler: map_reminder_scheduler(schedule.phase()),
                worker: map_worker(worker.phase()),
                recovery_pending: schedule.reconciliation_pending(),
                pending_work_count: pending_work(worker),
                accepted_request_count: schedule.accepted_hint_count(),
                submitted_count: schedule.submitted_count(),
            },
            source_attempt_sequence: refresh.attempt_sequence(),
            retry_mode: map_reminder_retry(refresh.retry_mode()),
            outcome: refresh.outcome().map(map_outcome),
            failure,
            notification_pending: schedule.notification_pending(),
            examined_count: refresh.examined_count(),
            expired_count: refresh.expired_count(),
            suppressed_count: refresh.suppressed_count(),
            delivery_count: refresh.delivery_count(),
            pending_due_count: refresh.pending_due_count(),
            retained_delivery_count: refresh.retained_delivery_count(),
            nearest_due_at_ms: refresh.nearest_due_at_ms(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductGitRuntimeHealth {
    core: ProductRuntimeCore,
    source_attempt_sequence: u64,
    outcome: Option<ProductRefreshOutcome>,
    failure: Option<ProductRuntimeFailureCode>,
    scanned_count: u64,
    published_count: u64,
    rebuild_count: u64,
    append_count: u64,
    unchanged_count: u64,
    partial_count: u64,
    unavailable_count: u64,
    cancelled_count: u64,
    stale_count: u64,
    retained_hint_count: usize,
    dropped_hint_count: u64,
}

impl ProductGitRuntimeHealth {
    #[must_use]
    pub const fn lifecycle(self) -> ProductRuntimeLifecycle {
        self.core.lifecycle
    }

    #[must_use]
    pub const fn recovery_pending(self) -> bool {
        self.core.recovery_pending
    }

    #[must_use]
    pub const fn scheduler(self) -> ProductSchedulerLifecycle {
        self.core.scheduler
    }

    #[must_use]
    pub const fn worker(self) -> ProductWorkerLifecycle {
        self.core.worker
    }

    #[must_use]
    pub const fn pending_work_count(self) -> usize {
        self.core.pending_work_count
    }

    #[must_use]
    pub const fn accepted_request_count(self) -> u64 {
        self.core.accepted_request_count
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.core.submitted_count
    }

    #[must_use]
    pub const fn failure(self) -> Option<ProductRuntimeFailureCode> {
        self.failure
    }

    #[must_use]
    pub const fn source_attempt_sequence(self) -> u64 {
        self.source_attempt_sequence
    }

    #[must_use]
    pub const fn outcome(self) -> Option<ProductRefreshOutcome> {
        self.outcome
    }

    #[must_use]
    pub const fn scanned_count(self) -> u64 {
        self.scanned_count
    }

    #[must_use]
    pub const fn published_count(self) -> u64 {
        self.published_count
    }

    #[must_use]
    pub const fn rebuild_count(self) -> u64 {
        self.rebuild_count
    }

    #[must_use]
    pub const fn append_count(self) -> u64 {
        self.append_count
    }

    #[must_use]
    pub const fn unchanged_count(self) -> u64 {
        self.unchanged_count
    }

    #[must_use]
    pub const fn partial_count(self) -> u64 {
        self.partial_count
    }

    #[must_use]
    pub const fn unavailable_count(self) -> u64 {
        self.unavailable_count
    }

    #[must_use]
    pub const fn cancelled_count(self) -> u64 {
        self.cancelled_count
    }

    #[must_use]
    pub const fn stale_count(self) -> u64 {
        self.stale_count
    }

    #[must_use]
    pub const fn retained_hint_count(self) -> usize {
        self.retained_hint_count
    }

    #[must_use]
    pub const fn dropped_hint_count(self) -> u64 {
        self.dropped_hint_count
    }

    pub(crate) const fn is_degraded(self) -> bool {
        self.core.is_degraded() || self.failure.is_some()
    }
}

impl From<GitRuntimeSnapshot> for ProductGitRuntimeHealth {
    fn from(value: GitRuntimeSnapshot) -> Self {
        let scheduler = value.scheduler();
        let worker = value.worker();
        let refresh = value.refresh();
        let mut failure = refresh.failure().map(|failure| match failure {
            GitRefreshFailure::Git(_) => ProductRuntimeFailureCode::GitBackend,
            GitRefreshFailure::Publication(_) => ProductRuntimeFailureCode::GitPublication,
            GitRefreshFailure::Control => ProductRuntimeFailureCode::GitControl,
        });
        if refresh
            .outcome()
            .is_some_and(|outcome| outcome != RefreshOutcome::Completed)
            && failure.is_none()
        {
            failure = Some(ProductRuntimeFailureCode::Unclassified);
        }
        Self {
            core: ProductRuntimeCore {
                lifecycle: map_git_lifecycle(value.phase()),
                scheduler: map_scheduler(scheduler.phase()),
                worker: map_worker(worker.phase()),
                recovery_pending: scheduler.dirty() || scheduler.force_reconcile(),
                pending_work_count: pending_work(worker),
                accepted_request_count: scheduler.accepted_hint_count(),
                submitted_count: scheduler.submitted_count(),
            },
            source_attempt_sequence: refresh.attempt_sequence(),
            outcome: refresh.outcome().map(map_outcome),
            failure,
            scanned_count: refresh.scanned_count(),
            published_count: refresh.published_count(),
            rebuild_count: refresh.rebuild_count(),
            append_count: refresh.append_count(),
            unchanged_count: refresh.unchanged_count(),
            partial_count: refresh.partial_count(),
            unavailable_count: refresh.unavailable_count(),
            cancelled_count: refresh.cancelled_count(),
            stale_count: refresh.stale_count(),
            retained_hint_count: value.retained_hint_count(),
            dropped_hint_count: value.dropped_hint_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductRuntimeSection<T> {
    generation: Option<ProductRuntimeGeneration>,
    health: Option<T>,
    observation_error: Option<ProductRuntimeObservationError>,
}

impl<T: Copy> ProductRuntimeSection<T> {
    pub(crate) const fn waiting() -> Self {
        Self {
            generation: None,
            health: None,
            observation_error: None,
        }
    }

    pub(crate) const fn ready(generation: ProductRuntimeGeneration, health: T) -> Self {
        Self {
            generation: Some(generation),
            health: Some(health),
            observation_error: None,
        }
    }

    pub(crate) const fn unavailable_retaining(
        generation: ProductRuntimeGeneration,
        error: ProductRuntimeObservationError,
        current: Self,
    ) -> Self {
        Self {
            generation: Some(generation),
            health: current.health,
            observation_error: Some(error),
        }
    }

    #[must_use]
    pub const fn generation(self) -> Option<ProductRuntimeGeneration> {
        self.generation
    }

    #[must_use]
    pub const fn kind(self) -> ProductSectionKind {
        match (self.health.is_some(), self.observation_error.is_some()) {
            (false, false) => ProductSectionKind::Waiting,
            (true, false) => ProductSectionKind::Ready,
            (_, true) => ProductSectionKind::Unavailable,
        }
    }

    #[must_use]
    pub const fn health(self) -> Option<T> {
        self.health
    }

    #[must_use]
    pub const fn observation_error(self) -> Option<ProductRuntimeObservationError> {
        self.observation_error
    }

    #[must_use]
    pub const fn retains_health(self) -> bool {
        self.health.is_some() && self.observation_error.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductRuntimeStatus {
    pub(crate) usage: ProductRuntimeSection<ProductUsageRuntimeHealth>,
    pub(crate) quota: ProductRuntimeSection<ProductQuotaRuntimeHealth>,
    pub(crate) reminder: ProductRuntimeSection<ProductReminderRuntimeHealth>,
    pub(crate) git: ProductRuntimeSection<ProductGitRuntimeHealth>,
}

impl ProductRuntimeStatus {
    pub(crate) const fn waiting() -> Self {
        Self {
            usage: ProductRuntimeSection::waiting(),
            quota: ProductRuntimeSection::waiting(),
            reminder: ProductRuntimeSection::waiting(),
            git: ProductRuntimeSection::waiting(),
        }
    }

    #[must_use]
    pub const fn usage(&self) -> ProductRuntimeSection<ProductUsageRuntimeHealth> {
        self.usage
    }

    #[must_use]
    pub const fn quota(&self) -> ProductRuntimeSection<ProductQuotaRuntimeHealth> {
        self.quota
    }

    #[must_use]
    pub const fn reminder(&self) -> ProductRuntimeSection<ProductReminderRuntimeHealth> {
        self.reminder
    }

    #[must_use]
    pub const fn git(&self) -> ProductRuntimeSection<ProductGitRuntimeHealth> {
        self.git
    }

    pub(crate) const fn usage_is_degraded(self) -> bool {
        self.usage.observation_error.is_some()
            || match self.usage.health {
                Some(health) => health.is_degraded(),
                None => false,
            }
    }

    pub(crate) const fn quota_is_degraded(self) -> bool {
        self.quota.observation_error.is_some()
            || match self.quota.health {
                Some(health) => health.quota_is_degraded(),
                None => false,
            }
    }

    pub(crate) const fn benefit_is_degraded(self) -> bool {
        self.quota.observation_error.is_some()
            || self.reminder.observation_error.is_some()
            || match self.quota.health {
                Some(health) => health.benefit_is_degraded(),
                None => false,
            }
            || match self.reminder.health {
                Some(health) => health.is_degraded(),
                None => false,
            }
    }

    pub(crate) const fn git_is_degraded(self) -> bool {
        self.git.observation_error.is_some()
            || match self.git.health {
                Some(health) => health.is_degraded(),
                None => false,
            }
    }
}

fn pending_work(worker: WorkerSnapshot) -> usize {
    worker
        .pending_count()
        .saturating_add(usize::from(worker.active_request_id().is_some()))
}

const fn map_outcome(value: RefreshOutcome) -> ProductRefreshOutcome {
    match value {
        RefreshOutcome::Completed => ProductRefreshOutcome::Completed,
        RefreshOutcome::Busy => ProductRefreshOutcome::Busy,
        RefreshOutcome::Cancelled => ProductRefreshOutcome::Cancelled,
        RefreshOutcome::DeadlineExceeded => ProductRefreshOutcome::DeadlineExceeded,
        RefreshOutcome::Failed => ProductRefreshOutcome::Failed,
    }
}

const fn map_quota_retry(value: ProviderQuotaRetryMode) -> ProductRetryMode {
    match value {
        ProviderQuotaRetryMode::Normal => ProductRetryMode::Normal,
        ProviderQuotaRetryMode::Accelerated => ProductRetryMode::Accelerated,
    }
}

const fn map_reminder_retry(value: BenefitReminderRetryMode) -> ProductRetryMode {
    match value {
        BenefitReminderRetryMode::Normal => ProductRetryMode::Normal,
        BenefitReminderRetryMode::Accelerated => ProductRetryMode::Accelerated,
    }
}

const fn map_scheduler(value: SchedulerPhase) -> ProductSchedulerLifecycle {
    match value {
        SchedulerPhase::Running => ProductSchedulerLifecycle::Running,
        SchedulerPhase::Paused => ProductSchedulerLifecycle::Paused,
        SchedulerPhase::Stopping => ProductSchedulerLifecycle::Stopping,
        SchedulerPhase::Stopped => ProductSchedulerLifecycle::Stopped,
        SchedulerPhase::Faulted => ProductSchedulerLifecycle::Faulted,
    }
}

const fn map_reminder_scheduler(value: BenefitReminderSchedulePhase) -> ProductSchedulerLifecycle {
    match value {
        BenefitReminderSchedulePhase::Running => ProductSchedulerLifecycle::Running,
        BenefitReminderSchedulePhase::Paused => ProductSchedulerLifecycle::Paused,
        BenefitReminderSchedulePhase::Stopping => ProductSchedulerLifecycle::Stopping,
        BenefitReminderSchedulePhase::Stopped => ProductSchedulerLifecycle::Stopped,
        BenefitReminderSchedulePhase::Faulted => ProductSchedulerLifecycle::Faulted,
    }
}

const fn map_worker(value: WorkerPhase) -> ProductWorkerLifecycle {
    match value {
        WorkerPhase::Running => ProductWorkerLifecycle::Running,
        WorkerPhase::ShuttingDown => ProductWorkerLifecycle::Stopping,
        WorkerPhase::Stopped => ProductWorkerLifecycle::Stopped,
        WorkerPhase::Faulted => ProductWorkerLifecycle::Faulted,
    }
}

const fn map_live_lifecycle(value: LivePhase) -> ProductRuntimeLifecycle {
    match value {
        LivePhase::Running => ProductRuntimeLifecycle::Running,
        LivePhase::Paused => ProductRuntimeLifecycle::Paused,
        LivePhase::Stopping => ProductRuntimeLifecycle::Stopping,
        LivePhase::Stopped => ProductRuntimeLifecycle::Stopped,
        LivePhase::Faulted => ProductRuntimeLifecycle::Faulted,
    }
}

const fn map_quota_lifecycle(value: ProviderQuotaRuntimePhase) -> ProductRuntimeLifecycle {
    match value {
        ProviderQuotaRuntimePhase::Running => ProductRuntimeLifecycle::Running,
        ProviderQuotaRuntimePhase::Paused => ProductRuntimeLifecycle::Paused,
        ProviderQuotaRuntimePhase::Stopping => ProductRuntimeLifecycle::Stopping,
        ProviderQuotaRuntimePhase::Stopped => ProductRuntimeLifecycle::Stopped,
        ProviderQuotaRuntimePhase::Faulted => ProductRuntimeLifecycle::Faulted,
    }
}

const fn map_reminder_lifecycle(value: BenefitReminderRuntimePhase) -> ProductRuntimeLifecycle {
    match value {
        BenefitReminderRuntimePhase::Running => ProductRuntimeLifecycle::Running,
        BenefitReminderRuntimePhase::Paused => ProductRuntimeLifecycle::Paused,
        BenefitReminderRuntimePhase::Stopping => ProductRuntimeLifecycle::Stopping,
        BenefitReminderRuntimePhase::Stopped => ProductRuntimeLifecycle::Stopped,
        BenefitReminderRuntimePhase::Faulted => ProductRuntimeLifecycle::Faulted,
    }
}

const fn map_git_lifecycle(value: GitRuntimePhase) -> ProductRuntimeLifecycle {
    match value {
        GitRuntimePhase::Running => ProductRuntimeLifecycle::Running,
        GitRuntimePhase::Paused => ProductRuntimeLifecycle::Paused,
        GitRuntimePhase::Stopping => ProductRuntimeLifecycle::Stopping,
        GitRuntimePhase::Stopped => ProductRuntimeLifecycle::Stopped,
        GitRuntimePhase::Faulted => ProductRuntimeLifecycle::Faulted,
    }
}

const fn map_reminder_failure(value: BenefitReminderFailure) -> ProductRuntimeFailureCode {
    match value {
        BenefitReminderFailure::Busy => ProductRuntimeFailureCode::ReminderBusy,
        BenefitReminderFailure::Clock => ProductRuntimeFailureCode::ReminderClock,
        BenefitReminderFailure::StoreUnavailable => {
            ProductRuntimeFailureCode::ReminderStoreUnavailable
        }
        BenefitReminderFailure::InvalidData => ProductRuntimeFailureCode::ReminderInvalidData,
        BenefitReminderFailure::CapacityExceeded => {
            ProductRuntimeFailureCode::ReminderCapacityExceeded
        }
        BenefitReminderFailure::Cancelled => ProductRuntimeFailureCode::ReminderCancelled,
        BenefitReminderFailure::DeadlineExceeded => {
            ProductRuntimeFailureCode::ReminderDeadlineExceeded
        }
        BenefitReminderFailure::Control => ProductRuntimeFailureCode::ReminderControl,
    }
}
