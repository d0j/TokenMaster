use tokenmaster_codex::MAX_CODEX_QUOTA_WINDOWS;
use tokenmaster_engine::{PortErrorCode, RefreshOutcome, WorkerSnapshot};

use crate::ProviderPollErrorCode;
use crate::SchedulerPhase;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderQuotaClockErrorCode {
    Unavailable,
    InvalidTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderQuotaPublicationErrorCode {
    Busy,
    Cancelled,
    DeadlineExceeded,
    StoreUnavailable,
    InvalidData,
    CapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderQuotaRefreshStage {
    Discovery,
    Clock,
    Transport,
    Publication,
    QuotaPublication,
    BenefitPublication,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderQuotaRefreshFailure {
    Discovery(super::CodexExecutableDiscoveryErrorCode),
    Clock(ProviderQuotaClockErrorCode),
    Transport(ProviderPollErrorCode),
    Publication(ProviderQuotaPublicationErrorCode),
    QuotaPublication(ProviderQuotaPublicationErrorCode),
    BenefitPublication(ProviderQuotaPublicationErrorCode),
    Control(PortErrorCode),
}

impl ProviderQuotaRefreshFailure {
    #[must_use]
    pub const fn stage(self) -> ProviderQuotaRefreshStage {
        match self {
            Self::Discovery(_) => ProviderQuotaRefreshStage::Discovery,
            Self::Clock(_) => ProviderQuotaRefreshStage::Clock,
            Self::Transport(_) => ProviderQuotaRefreshStage::Transport,
            Self::Publication(_) => ProviderQuotaRefreshStage::Publication,
            Self::QuotaPublication(_) => ProviderQuotaRefreshStage::QuotaPublication,
            Self::BenefitPublication(_) => ProviderQuotaRefreshStage::BenefitPublication,
            Self::Control(_) => ProviderQuotaRefreshStage::Control,
        }
    }

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Discovery(_) => "unavailable",
            Self::Clock(ProviderQuotaClockErrorCode::Unavailable) => "unavailable",
            Self::Clock(ProviderQuotaClockErrorCode::InvalidTime) => "invalid_time",
            Self::Transport(error) => error.stable_code(),
            Self::Publication(ProviderQuotaPublicationErrorCode::Busy)
            | Self::QuotaPublication(ProviderQuotaPublicationErrorCode::Busy)
            | Self::BenefitPublication(ProviderQuotaPublicationErrorCode::Busy)
            | Self::Control(PortErrorCode::Busy) => "busy",
            Self::Publication(ProviderQuotaPublicationErrorCode::Cancelled)
            | Self::QuotaPublication(ProviderQuotaPublicationErrorCode::Cancelled)
            | Self::BenefitPublication(ProviderQuotaPublicationErrorCode::Cancelled)
            | Self::Control(PortErrorCode::Cancelled) => "cancelled",
            Self::Publication(ProviderQuotaPublicationErrorCode::DeadlineExceeded)
            | Self::QuotaPublication(ProviderQuotaPublicationErrorCode::DeadlineExceeded)
            | Self::BenefitPublication(ProviderQuotaPublicationErrorCode::DeadlineExceeded)
            | Self::Control(PortErrorCode::DeadlineExceeded) => "deadline_exceeded",
            Self::Publication(ProviderQuotaPublicationErrorCode::StoreUnavailable)
            | Self::QuotaPublication(ProviderQuotaPublicationErrorCode::StoreUnavailable)
            | Self::BenefitPublication(ProviderQuotaPublicationErrorCode::StoreUnavailable)
            | Self::Control(PortErrorCode::Unavailable) => "unavailable",
            Self::Publication(ProviderQuotaPublicationErrorCode::CapacityExceeded)
            | Self::QuotaPublication(ProviderQuotaPublicationErrorCode::CapacityExceeded)
            | Self::BenefitPublication(ProviderQuotaPublicationErrorCode::CapacityExceeded)
            | Self::Control(PortErrorCode::CapacityExceeded) => "capacity_exceeded",
            Self::Publication(ProviderQuotaPublicationErrorCode::InvalidData)
            | Self::QuotaPublication(ProviderQuotaPublicationErrorCode::InvalidData)
            | Self::BenefitPublication(ProviderQuotaPublicationErrorCode::InvalidData)
            | Self::Control(PortErrorCode::InvalidData)
            | Self::Control(PortErrorCode::StaleState)
            | Self::Control(PortErrorCode::RebuildRequired)
            | Self::Control(PortErrorCode::Failed) => "invalid_data",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderQuotaRetryMode {
    Normal,
    Accelerated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderQuotaRuntimePhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderQuotaScheduleSnapshot {
    pub(super) phase: SchedulerPhase,
    pub(super) retry_mode: ProviderQuotaRetryMode,
    pub(super) refresh_pending: bool,
    pub(super) accepted_refresh_count: u64,
    pub(super) submitted_count: u64,
}

impl ProviderQuotaScheduleSnapshot {
    #[must_use]
    pub const fn phase(self) -> SchedulerPhase {
        self.phase
    }

    #[must_use]
    pub const fn retry_mode(self) -> ProviderQuotaRetryMode {
        self.retry_mode
    }

    #[must_use]
    pub const fn refresh_pending(self) -> bool {
        self.refresh_pending
    }

    #[must_use]
    pub const fn accepted_refresh_count(self) -> u64 {
        self.accepted_refresh_count
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.submitted_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderQuotaRefreshSnapshot {
    pub(super) attempt_sequence: u64,
    pub(super) outcome: Option<RefreshOutcome>,
    pub(super) failure: Option<ProviderQuotaRefreshFailure>,
    pub(super) retry_mode: ProviderQuotaRetryMode,
    pub(super) observation_count: u16,
    pub(super) processed_count: u16,
    pub(super) changed_count: u16,
    pub(super) started_count: u16,
    pub(super) advanced_count: u16,
    pub(super) duplicate_count: u16,
    pub(super) stale_count: u16,
    pub(super) allowance_change_count: u16,
    pub(super) reset_count: u16,
    pub(super) quota_failure: Option<ProviderQuotaPublicationErrorCode>,
    pub(super) benefit_observation_count: u8,
    pub(super) benefit_processed_count: u8,
    pub(super) benefit_changed_count: u8,
    pub(super) benefit_freshness_only_count: u8,
    pub(super) benefit_duplicate_count: u8,
    pub(super) benefit_stale_count: u8,
    pub(super) benefit_lot_change_count: u16,
    pub(super) benefit_pending_due_count: u16,
    pub(super) benefit_failure: Option<ProviderQuotaPublicationErrorCode>,
    pub(super) observed_at_ms: Option<i64>,
    pub(super) elapsed_millis: u64,
    pub(super) last_success_observed_at_ms: Option<i64>,
    pub(super) last_quota_success_observed_at_ms: Option<i64>,
    pub(super) last_benefit_success_observed_at_ms: Option<i64>,
}

impl ProviderQuotaRefreshSnapshot {
    pub(crate) const fn not_run() -> Self {
        Self {
            attempt_sequence: 0,
            outcome: None,
            failure: None,
            retry_mode: ProviderQuotaRetryMode::Normal,
            observation_count: 0,
            processed_count: 0,
            changed_count: 0,
            started_count: 0,
            advanced_count: 0,
            duplicate_count: 0,
            stale_count: 0,
            allowance_change_count: 0,
            reset_count: 0,
            quota_failure: None,
            benefit_observation_count: 0,
            benefit_processed_count: 0,
            benefit_changed_count: 0,
            benefit_freshness_only_count: 0,
            benefit_duplicate_count: 0,
            benefit_stale_count: 0,
            benefit_lot_change_count: 0,
            benefit_pending_due_count: 0,
            benefit_failure: None,
            observed_at_ms: None,
            elapsed_millis: 0,
            last_success_observed_at_ms: None,
            last_quota_success_observed_at_ms: None,
            last_benefit_success_observed_at_ms: None,
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
    pub const fn failure(self) -> Option<ProviderQuotaRefreshFailure> {
        self.failure
    }

    #[must_use]
    pub const fn retry_mode(self) -> ProviderQuotaRetryMode {
        self.retry_mode
    }

    #[must_use]
    pub const fn observation_count(self) -> u16 {
        self.observation_count
    }

    #[must_use]
    pub const fn quota_observation_count(self) -> u16 {
        self.observation_count
    }

    #[must_use]
    pub const fn processed_count(self) -> u16 {
        self.processed_count
    }

    #[must_use]
    pub const fn quota_processed_count(self) -> u16 {
        self.processed_count
    }

    #[must_use]
    pub const fn changed_count(self) -> u16 {
        self.changed_count
    }

    #[must_use]
    pub const fn quota_changed_count(self) -> u16 {
        self.changed_count
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
    pub const fn quota_failure_count(self) -> u8 {
        if self.quota_failure.is_some() { 1 } else { 0 }
    }

    #[must_use]
    pub const fn quota_failure(self) -> Option<ProviderQuotaPublicationErrorCode> {
        self.quota_failure
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

    #[must_use]
    pub const fn benefit_failure_count(self) -> u8 {
        if self.benefit_failure.is_some() { 1 } else { 0 }
    }

    #[must_use]
    pub const fn benefit_failure(self) -> Option<ProviderQuotaPublicationErrorCode> {
        self.benefit_failure
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

    #[must_use]
    pub const fn last_quota_success_observed_at_ms(self) -> Option<i64> {
        self.last_quota_success_observed_at_ms
    }

    #[must_use]
    pub const fn last_benefit_success_observed_at_ms(self) -> Option<i64> {
        self.last_benefit_success_observed_at_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderQuotaRuntimeSnapshot {
    pub(super) phase: ProviderQuotaRuntimePhase,
    pub(super) schedule: ProviderQuotaScheduleSnapshot,
    pub(super) worker: WorkerSnapshot,
    pub(super) refresh: ProviderQuotaRefreshSnapshot,
}

impl ProviderQuotaRuntimeSnapshot {
    #[must_use]
    pub const fn phase(self) -> ProviderQuotaRuntimePhase {
        self.phase
    }

    #[must_use]
    pub const fn schedule(self) -> ProviderQuotaScheduleSnapshot {
        self.schedule
    }

    #[must_use]
    pub const fn worker(self) -> WorkerSnapshot {
        self.worker
    }

    #[must_use]
    pub const fn refresh(self) -> ProviderQuotaRefreshSnapshot {
        self.refresh
    }
}

const _: () = assert!(MAX_CODEX_QUOTA_WINDOWS <= u16::MAX as usize);

pub type CodexQuotaClockErrorCode = ProviderQuotaClockErrorCode;
pub type CodexQuotaPublicationErrorCode = ProviderQuotaPublicationErrorCode;
pub type CodexQuotaRefreshFailure = ProviderQuotaRefreshFailure;
pub type CodexQuotaRefreshSnapshot = ProviderQuotaRefreshSnapshot;
pub type CodexQuotaRefreshStage = ProviderQuotaRefreshStage;
pub type CodexQuotaRetryMode = ProviderQuotaRetryMode;
pub type CodexQuotaRuntimePhase = ProviderQuotaRuntimePhase;
pub type CodexQuotaRuntimeSnapshot = ProviderQuotaRuntimeSnapshot;
pub type CodexQuotaScheduleSnapshot = ProviderQuotaScheduleSnapshot;
