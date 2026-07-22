//! Production composition for bounded TokenMaster runtime operations.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod clock;
mod codex_adapter;
mod error;
mod git;
mod hints;
mod incremental;
mod lease;
mod lifecycle;
mod live;
mod provider;
mod provider_quota;
mod publication;
mod quota;
mod recovery;
mod reminder;
mod scheduler;
mod store_archive;
mod watcher;

pub use clock::SystemClock;
pub use codex_adapter::CodexAdapter;
pub use error::{RuntimeError, RuntimeErrorCode};
pub use git::{
    GitPublicationErrorCode, GitRefreshFailure, GitRefreshSnapshot, GitRepositoryHintIngress,
    GitRuntime, GitRuntimeConfig, GitRuntimePhase, GitRuntimeSnapshot,
    MAX_GIT_RUNTIME_REPOSITORIES,
};
pub use hints::{RefreshHintSink, SchedulerPhase, WatcherHealth};
pub use incremental::{IncrementalRefreshOutcome, IncrementalRefreshReport, refresh_incremental};
pub use lease::RuntimeWriterLease;
pub use lifecycle::{LivePhase, LiveRefreshKind, LiveRefreshSnapshot, LiveRuntimeSnapshot};
pub use live::LiveRuntime;
pub use provider::{
    CodexUsageProviderFactory, LiveProviderAdapter, ProviderWatchRoots, UsageProviderFactory,
};
pub use provider_quota::{
    CodexQuotaSource, MAX_PROVIDER_QUOTA_WINDOWS, ProviderPollErrorCode, ProviderQuotaObservation,
    ProviderQuotaPoll, ProviderQuotaSource,
};
pub use publication::{
    EngineDiagnostics, EnginePublicationQuality, EngineSnapshot, EngineSnapshotGeneration,
};
pub use quota::{
    CodexExecutableDiscoveryError, CodexExecutableDiscoveryErrorCode, CodexExecutableSearchPath,
    CodexQuotaClockErrorCode, CodexQuotaPublicationErrorCode, CodexQuotaRefreshFailure,
    CodexQuotaRefreshSnapshot, CodexQuotaRefreshStage, CodexQuotaRetryMode, CodexQuotaRuntime,
    CodexQuotaRuntimeConfig, CodexQuotaRuntimePhase, CodexQuotaRuntimeSnapshot,
    CodexQuotaScheduleSnapshot, MAX_CODEX_EXECUTABLE_SEARCH_DIRS,
    MAX_CODEX_EXECUTABLE_SEARCH_PATH_BYTES, ProviderQuotaClockErrorCode,
    ProviderQuotaPublicationErrorCode, ProviderQuotaRefreshFailure, ProviderQuotaRefreshSnapshot,
    ProviderQuotaRefreshStage, ProviderQuotaRetryMode, ProviderQuotaRuntime,
    ProviderQuotaRuntimePhase, ProviderQuotaRuntimeSnapshot, ProviderQuotaScheduleSnapshot,
};
pub use recovery::{StagingRecoveryOutcome, StartupRecoveryReport};
pub use reminder::{
    BenefitReminderDelivery, BenefitReminderFailure, BenefitReminderRefreshSnapshot,
    BenefitReminderRetryMode, BenefitReminderRuntime, BenefitReminderRuntimeConfig,
    BenefitReminderRuntimePhase, BenefitReminderRuntimeSnapshot, BenefitReminderSchedulePhase,
    BenefitReminderScheduleSnapshot,
};
pub use scheduler::{
    DEGRADED_POLL_MILLIS, HEALTHY_POLL_MILLIS, QUIET_WINDOW_MILLIS, RefreshScheduler,
    SchedulerError, SchedulerErrorCode, SchedulerSnapshot,
};
pub use store_archive::StoreArchive;
pub use watcher::{
    BoundedFilesystemWatcher, MAX_WATCH_ROOTS, WatcherError, WatcherErrorCode, WatcherSnapshot,
};
