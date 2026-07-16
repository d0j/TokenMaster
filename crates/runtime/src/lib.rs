//! Production composition for bounded TokenMaster runtime operations.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod clock;
mod codex_adapter;
mod error;
mod hints;
mod incremental;
mod lease;
mod lifecycle;
mod live;
mod publication;
mod quota;
mod recovery;
mod scheduler;
mod store_archive;
mod watcher;

pub use clock::SystemClock;
pub use codex_adapter::CodexAdapter;
pub use error::{RuntimeError, RuntimeErrorCode};
pub use hints::{RefreshHintSink, SchedulerPhase, WatcherHealth};
pub use incremental::{IncrementalRefreshOutcome, IncrementalRefreshReport, refresh_incremental};
pub use lease::RuntimeWriterLease;
pub use lifecycle::{LivePhase, LiveRefreshKind, LiveRefreshSnapshot, LiveRuntimeSnapshot};
pub use live::LiveRuntime;
pub use publication::{
    EngineDiagnostics, EnginePublicationQuality, EngineSnapshot, EngineSnapshotGeneration,
};
pub use quota::{
    CodexExecutableDiscoveryError, CodexExecutableDiscoveryErrorCode, CodexExecutableSearchPath,
    CodexQuotaClockErrorCode, CodexQuotaPublicationErrorCode, CodexQuotaRefreshFailure,
    CodexQuotaRefreshSnapshot, CodexQuotaRefreshStage, CodexQuotaRetryMode, CodexQuotaRuntime,
    CodexQuotaRuntimeConfig, CodexQuotaRuntimePhase, CodexQuotaRuntimeSnapshot,
    CodexQuotaScheduleSnapshot, MAX_CODEX_EXECUTABLE_SEARCH_DIRS,
    MAX_CODEX_EXECUTABLE_SEARCH_PATH_BYTES,
};
pub use recovery::{StagingRecoveryOutcome, StartupRecoveryReport};
pub use scheduler::{
    DEGRADED_POLL_MILLIS, HEALTHY_POLL_MILLIS, QUIET_WINDOW_MILLIS, RefreshScheduler,
    SchedulerError, SchedulerErrorCode, SchedulerSnapshot,
};
pub use store_archive::StoreArchive;
pub use watcher::{
    BoundedFilesystemWatcher, MAX_WATCH_ROOTS, WatcherError, WatcherErrorCode, WatcherSnapshot,
};
