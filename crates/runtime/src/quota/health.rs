use tokenmaster_codex::{CodexQuotaErrorCode, MAX_CODEX_QUOTA_WINDOWS};
use tokenmaster_engine::{PortErrorCode, RefreshOutcome, WorkerSnapshot};

use crate::SchedulerPhase;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexQuotaClockErrorCode {
    Unavailable,
    InvalidTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexQuotaPublicationErrorCode {
    Busy,
    Cancelled,
    DeadlineExceeded,
    StoreUnavailable,
    InvalidData,
    CapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexQuotaRefreshStage {
    Discovery,
    Clock,
    Transport,
    Publication,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexQuotaRefreshFailure {
    Discovery(super::CodexExecutableDiscoveryErrorCode),
    Clock(CodexQuotaClockErrorCode),
    Transport(CodexQuotaErrorCode),
    Publication(CodexQuotaPublicationErrorCode),
    Control(PortErrorCode),
}

impl CodexQuotaRefreshFailure {
    #[must_use]
    pub const fn stage(self) -> CodexQuotaRefreshStage {
        match self {
            Self::Discovery(_) => CodexQuotaRefreshStage::Discovery,
            Self::Clock(_) => CodexQuotaRefreshStage::Clock,
            Self::Transport(_) => CodexQuotaRefreshStage::Transport,
            Self::Publication(_) => CodexQuotaRefreshStage::Publication,
            Self::Control(_) => CodexQuotaRefreshStage::Control,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexQuotaRetryMode {
    Normal,
    Accelerated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexQuotaRuntimePhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodexQuotaScheduleSnapshot {
    pub(super) phase: SchedulerPhase,
    pub(super) retry_mode: CodexQuotaRetryMode,
    pub(super) refresh_pending: bool,
    pub(super) accepted_refresh_count: u64,
    pub(super) submitted_count: u64,
}

impl CodexQuotaScheduleSnapshot {
    #[must_use]
    pub const fn phase(self) -> SchedulerPhase {
        self.phase
    }

    #[must_use]
    pub const fn retry_mode(self) -> CodexQuotaRetryMode {
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
pub struct CodexQuotaRefreshSnapshot {
    pub(super) attempt_sequence: u64,
    pub(super) outcome: Option<RefreshOutcome>,
    pub(super) failure: Option<CodexQuotaRefreshFailure>,
    pub(super) retry_mode: CodexQuotaRetryMode,
    pub(super) observation_count: u16,
    pub(super) processed_count: u16,
    pub(super) changed_count: u16,
    pub(super) started_count: u16,
    pub(super) advanced_count: u16,
    pub(super) duplicate_count: u16,
    pub(super) stale_count: u16,
    pub(super) allowance_change_count: u16,
    pub(super) reset_count: u16,
    pub(super) observed_at_ms: Option<i64>,
    pub(super) elapsed_millis: u64,
    pub(super) last_success_observed_at_ms: Option<i64>,
}

impl CodexQuotaRefreshSnapshot {
    pub(crate) const fn not_run() -> Self {
        Self {
            attempt_sequence: 0,
            outcome: None,
            failure: None,
            retry_mode: CodexQuotaRetryMode::Normal,
            observation_count: 0,
            processed_count: 0,
            changed_count: 0,
            started_count: 0,
            advanced_count: 0,
            duplicate_count: 0,
            stale_count: 0,
            allowance_change_count: 0,
            reset_count: 0,
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
    pub const fn failure(self) -> Option<CodexQuotaRefreshFailure> {
        self.failure
    }

    #[must_use]
    pub const fn retry_mode(self) -> CodexQuotaRetryMode {
        self.retry_mode
    }

    #[must_use]
    pub const fn observation_count(self) -> u16 {
        self.observation_count
    }

    #[must_use]
    pub const fn processed_count(self) -> u16 {
        self.processed_count
    }

    #[must_use]
    pub const fn changed_count(self) -> u16 {
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
pub struct CodexQuotaRuntimeSnapshot {
    pub(super) phase: CodexQuotaRuntimePhase,
    pub(super) schedule: CodexQuotaScheduleSnapshot,
    pub(super) worker: WorkerSnapshot,
    pub(super) refresh: CodexQuotaRefreshSnapshot,
}

impl CodexQuotaRuntimeSnapshot {
    #[must_use]
    pub const fn phase(self) -> CodexQuotaRuntimePhase {
        self.phase
    }

    #[must_use]
    pub const fn schedule(self) -> CodexQuotaScheduleSnapshot {
        self.schedule
    }

    #[must_use]
    pub const fn worker(self) -> WorkerSnapshot {
        self.worker
    }

    #[must_use]
    pub const fn refresh(self) -> CodexQuotaRefreshSnapshot {
        self.refresh
    }
}

const _: () = assert!(MAX_CODEX_QUOTA_WINDOWS <= u16::MAX as usize);
