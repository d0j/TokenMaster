//! Usage append batches accept only canonical accounting output.
//!
//! Canonical events cannot be constructed by store callers:
//!
//! ```compile_fail
//! let _ = tokenmaster_accounting::CanonicalUsageEvent::new();
//! ```

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod error;
mod schema;
mod session_store;
mod usage;

pub use error::{StoreError, StoreErrorCode};
pub use session_store::{EXPECTED_SQLITE_VERSION, MAX_PAGE_SIZE, MAX_SEED_SESSIONS, ProbeStore};
pub use usage::{
    AppendBatch, AppendBatchParts, EventCursor, GenerationSnapshot, GenerationStatus, JournalMode,
    MAX_APPEND_CHUNK_UPDATES, MAX_APPEND_EVENTS, MAX_RESUME_BYTES, MAX_USAGE_EVENT_PAGE_SIZE,
    RuntimePolicy, SOURCE_CHUNK_BYTES, SourceKey, SourceKind, SourceRegistration,
    SourceRegistrationParts, StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk,
    StoredUsageEvent, StoredVerification, USAGE_SCHEMA_VERSION, UsageStore, UsageStoreCounts,
};
