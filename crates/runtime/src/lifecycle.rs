use tokenmaster_engine::{PortErrorCode, RefreshOutcome, WorkerSnapshot};

use crate::{SchedulerSnapshot, WatcherSnapshot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRefreshKind {
    None,
    Incremental,
    FullRebuild,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRefreshSnapshot {
    pub(crate) kind: LiveRefreshKind,
    pub(crate) outcome: Option<RefreshOutcome>,
    pub(crate) error: Option<PortErrorCode>,
}

impl LiveRefreshSnapshot {
    pub(crate) const fn not_run() -> Self {
        Self {
            kind: LiveRefreshKind::None,
            outcome: None,
            error: None,
        }
    }

    pub(crate) const fn result(
        kind: LiveRefreshKind,
        outcome: RefreshOutcome,
        error: Option<PortErrorCode>,
    ) -> Self {
        Self {
            kind,
            outcome: Some(outcome),
            error,
        }
    }

    #[must_use]
    pub const fn kind(self) -> LiveRefreshKind {
        self.kind
    }

    #[must_use]
    pub const fn outcome(self) -> Option<RefreshOutcome> {
        self.outcome
    }

    #[must_use]
    pub const fn error(self) -> Option<PortErrorCode> {
        self.error
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRuntimeSnapshot {
    pub(crate) phase: LivePhase,
    pub(crate) scheduler: SchedulerSnapshot,
    pub(crate) worker: WorkerSnapshot,
    pub(crate) watcher: WatcherSnapshot,
    pub(crate) refresh: LiveRefreshSnapshot,
}

impl LiveRuntimeSnapshot {
    #[must_use]
    pub const fn phase(self) -> LivePhase {
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
    pub const fn watcher(self) -> WatcherSnapshot {
        self.watcher
    }

    #[must_use]
    pub const fn refresh(self) -> LiveRefreshSnapshot {
        self.refresh
    }
}
