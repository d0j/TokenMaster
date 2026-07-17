use tokenmaster_engine::{RefreshOutcome, WorkerSnapshot};
use tokenmaster_git::GitBackendErrorCode;

use crate::SchedulerSnapshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitPublicationErrorCode {
    Busy,
    StoreUnavailable,
    InvalidData,
    CapacityExceeded,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitRefreshFailure {
    Git(GitBackendErrorCode),
    Publication(GitPublicationErrorCode),
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitRuntimePhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitRefreshSnapshot {
    pub(super) attempt_sequence: u64,
    pub(super) outcome: Option<RefreshOutcome>,
    pub(super) failure: Option<GitRefreshFailure>,
    pub(super) scanned_count: u64,
    pub(super) published_count: u64,
    pub(super) rebuild_count: u64,
    pub(super) append_count: u64,
    pub(super) unchanged_count: u64,
    pub(super) partial_count: u64,
    pub(super) unavailable_count: u64,
    pub(super) cancelled_count: u64,
    pub(super) stale_count: u64,
    pub(super) elapsed_millis: u64,
}

impl GitRefreshSnapshot {
    pub(super) const fn not_run() -> Self {
        Self {
            attempt_sequence: 0,
            outcome: None,
            failure: None,
            scanned_count: 0,
            published_count: 0,
            rebuild_count: 0,
            append_count: 0,
            unchanged_count: 0,
            partial_count: 0,
            unavailable_count: 0,
            cancelled_count: 0,
            stale_count: 0,
            elapsed_millis: 0,
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
    pub const fn failure(self) -> Option<GitRefreshFailure> {
        self.failure
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
    pub const fn elapsed_millis(self) -> u64 {
        self.elapsed_millis
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitRuntimeSnapshot {
    pub(super) phase: GitRuntimePhase,
    pub(super) scheduler: SchedulerSnapshot,
    pub(super) worker: WorkerSnapshot,
    pub(super) refresh: GitRefreshSnapshot,
    pub(super) retained_hint_count: usize,
    pub(super) dropped_hint_count: u64,
}

impl GitRuntimeSnapshot {
    #[must_use]
    pub const fn phase(self) -> GitRuntimePhase {
        self.phase
    }
    #[must_use]
    pub const fn scheduler(self) -> SchedulerSnapshot {
        self.scheduler
    }
    #[must_use]
    pub const fn worker(self) -> WorkerSnapshot {
        self.worker
    }
    #[must_use]
    pub const fn refresh(self) -> GitRefreshSnapshot {
        self.refresh
    }
    #[must_use]
    pub const fn retained_hint_count(self) -> usize {
        self.retained_hint_count
    }
    #[must_use]
    pub const fn dropped_hint_count(self) -> u64 {
        self.dropped_hint_count
    }
}
