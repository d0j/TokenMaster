#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod aggregate;
mod classify;
mod command;
mod discovery;
mod identity;
mod process;
mod protocol;
mod scan;

pub const MAX_GIT_AUTHOR_BYTES: usize = 4 * 1024;
pub const MAX_GIT_PATH_BYTES: usize = 32 * 1024;
pub const MAX_GIT_REF_NAME_BYTES: usize = 4 * 1024;
pub const MAX_GIT_REFS: usize = 512;
pub const MAX_GIT_PATHS_PER_COMMIT: usize = 4_096;
pub const MAX_GIT_COMMITS_PER_BATCH: usize = 256;
pub const MAX_GIT_SCANNED_COMMITS: usize = 200_000;
pub const MAX_GIT_MAILMAP_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitBackendErrorCode {
    AuthorIdentityMissing,
    Cancelled,
    CapacityExceeded,
    DeadlineExceeded,
    HistoryChangedDuringScan,
    InvalidExecutable,
    InvalidTime,
    ProcessCleanupFailed,
    ProcessFailed,
    ProtocolError,
    RepositoryNotFound,
    RepositoryPathRejected,
    SpawnFailed,
    StderrLimitExceeded,
    StdoutLimitExceeded,
    TooManyRefs,
    Unavailable,
    UnsupportedObjectFormat,
    UnsupportedVersion,
}

impl GitBackendErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::AuthorIdentityMissing => "author_identity_missing",
            Self::Cancelled => "cancelled",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::HistoryChangedDuringScan => "history_changed_during_scan",
            Self::InvalidExecutable => "invalid_executable",
            Self::InvalidTime => "invalid_time",
            Self::ProcessCleanupFailed => "process_cleanup_failed",
            Self::ProcessFailed => "process_failed",
            Self::ProtocolError => "protocol_error",
            Self::RepositoryNotFound => "repository_not_found",
            Self::RepositoryPathRejected => "repository_path_rejected",
            Self::SpawnFailed => "spawn_failed",
            Self::StderrLimitExceeded => "stderr_limit_exceeded",
            Self::StdoutLimitExceeded => "stdout_limit_exceeded",
            Self::TooManyRefs => "too_many_refs",
            Self::Unavailable => "unavailable",
            Self::UnsupportedObjectFormat => "unsupported_object_format",
            Self::UnsupportedVersion => "unsupported_version",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitBackendError {
    code: GitBackendErrorCode,
    limit: Option<usize>,
}

impl GitBackendError {
    pub(crate) const fn new(code: GitBackendErrorCode) -> Self {
        Self { code, limit: None }
    }

    pub(crate) const fn with_limit(code: GitBackendErrorCode, limit: usize) -> Self {
        Self {
            code,
            limit: Some(limit),
        }
    }

    #[must_use]
    pub const fn code(self) -> GitBackendErrorCode {
        self.code
    }

    #[must_use]
    pub const fn limit(self) -> Option<usize> {
        self.limit
    }
}

impl std::fmt::Display for GitBackendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code.stable_code())
    }
}

impl std::error::Error for GitBackendError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GitCoreError {
    #[error("value exceeds capacity {limit}")]
    CapacityExceeded { limit: usize },
    #[error("Git value contains a duplicate")]
    DuplicateValue,
    #[error("Git author identity is invalid")]
    InvalidAuthor,
    #[error("Git limit is invalid")]
    InvalidLimit,
    #[error("Git object identity is invalid")]
    InvalidObjectId,
    #[error("Git output path is invalid")]
    InvalidPath,
    #[error("Git protocol is invalid")]
    InvalidProtocol,
    #[error("Git ref is invalid")]
    InvalidRef,
    #[error("Git timestamp is invalid")]
    InvalidTimestamp,
    #[error("Git protocol ended before a complete record")]
    IncompleteProtocol,
    #[error("Git value is incoherent")]
    IncoherentState,
    #[error("Git counter overflow")]
    Overflow,
    #[error("Git raw and numstat records disagree")]
    ProtocolMismatch,
}

impl GitCoreError {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::CapacityExceeded { .. } => "capacity_exceeded",
            Self::DuplicateValue => "duplicate_value",
            Self::InvalidAuthor => "invalid_author",
            Self::InvalidLimit => "invalid_limit",
            Self::InvalidObjectId => "invalid_object_id",
            Self::InvalidPath => "invalid_path",
            Self::InvalidProtocol => "invalid_protocol",
            Self::InvalidRef => "invalid_ref",
            Self::InvalidTimestamp => "invalid_timestamp",
            Self::IncompleteProtocol => "incomplete_protocol",
            Self::IncoherentState => "incoherent_state",
            Self::Overflow => "overflow",
            Self::ProtocolMismatch => "protocol_mismatch",
        }
    }
}

pub use aggregate::{
    GitAggregateBatch, GitCommitAccumulator, GitCommitAggregate, GitCommitSink,
    GitDayCategoryAggregate, GitPathStat, GitScanAccumulator, GitScanSummary,
};
pub use classify::classify_destination_path;
pub use command::{GitExecutable, GitRepositoryCandidate};
pub use discovery::{
    GitExecutableSearchPath, MAX_GIT_EXECUTABLE_SEARCH_DIRS, MAX_GIT_EXECUTABLE_SEARCH_PATH_BYTES,
};
pub use identity::{
    GitAuthorFingerprint, GitCommitFingerprint, GitIdentitySalt, GitMailmapFingerprint,
    GitProjectFingerprint, GitRefFingerprint, GitRefHead, derive_activity_association_id,
    derive_author_fingerprint, derive_commit_fingerprint, derive_mailmap_fingerprint,
    derive_project_fingerprint, derive_ref_fingerprint, derive_repository_id,
};
pub use process::{
    GitCancellation, GitProcess, GitRunControl, GitVersion, MAX_GIT_LOG_STDOUT_BYTES,
    MAX_GIT_PROCESS_TIMEOUT, MAX_GIT_STDERR_BYTES,
};
pub use protocol::{GitLogParseConfig, GitLogStreamParser, GitStreamLimits};
pub use scan::{
    GitAuthorSource, GitObjectFormat, GitRefreshKind, GitRepositoryFrontier, GitRepositoryRefresh,
    GitRepositoryScan,
};
