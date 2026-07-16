#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod aggregate;
mod classify;
mod identity;
mod protocol;

pub const MAX_GIT_AUTHOR_BYTES: usize = 4 * 1024;
pub const MAX_GIT_PATH_BYTES: usize = 32 * 1024;
pub const MAX_GIT_REF_NAME_BYTES: usize = 4 * 1024;
pub const MAX_GIT_REFS: usize = 512;
pub const MAX_GIT_PATHS_PER_COMMIT: usize = 4_096;
pub const MAX_GIT_COMMITS_PER_BATCH: usize = 256;

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
    GitAggregateBatch, GitCommitAccumulator, GitCommitAggregate, GitCommitSink, GitPathStat,
    GitScanAccumulator, GitScanSummary,
};
pub use classify::classify_destination_path;
pub use identity::{
    GitAuthorFingerprint, GitCommitFingerprint, GitIdentitySalt, GitRefFingerprint, GitRefHead,
    derive_author_fingerprint, derive_commit_fingerprint, derive_ref_fingerprint,
    derive_repository_id,
};
pub use protocol::{GitLogParseConfig, GitLogStreamParser, GitStreamLimits};
