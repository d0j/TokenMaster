//! Provider-neutral, bounded refresh coordination and runtime ports.
//!
//! Identity internals are sealed so callers cannot bypass validation:
//!
//! ```compile_fail
//! use tokenmaster_engine::ScopeIdentity;
//!
//! let _ = ScopeIdentity {
//!     provider_id: "codex".into(),
//!     profile_id: "default".into(),
//! };
//! ```
//!
//! Provider paths cannot be substituted for normalized source identities:
//!
//! ```compile_fail
//! use std::path::Path;
//! use tokenmaster_engine::{Adapter, AdapterCheckpoint, OperationControl};
//!
//! fn provider_coupled(
//!     adapter: &mut dyn Adapter,
//!     path: &Path,
//!     checkpoint: &AdapterCheckpoint,
//!     control: &OperationControl<'_>,
//! ) {
//!     let _ = adapter.read_batch(path, checkpoint, control);
//! }
//! ```
//!
//! Archive writes reject raw provider bytes at compile time:
//!
//! ```compile_fail
//! use tokenmaster_engine::{Archive, ArchiveReplay, SourceIdentity};
//!
//! fn raw_archive_write(
//!     archive: &mut dyn Archive,
//!     replay: ArchiveReplay,
//!     source: &SourceIdentity,
//!     raw_source_bytes: Box<[u8]>,
//! ) {
//!     let _ = archive.append_replay_batch(replay, source, raw_source_bytes);
//! }
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod archive;
mod batch;
mod coordinator;
mod error;
mod executor;
mod ports;
mod time;
mod values;

pub use archive::{
    Archive, ArchiveEpoch, ArchiveReplay, ArchiveRevisionId, ArchiveScanSetId, ArchiveSourceCursor,
    MAX_REPLAY_SOURCES_PER_PAGE, ReplayContinuation, ReplayContinuationState, ReplaySource,
    ReplaySourcePage,
};
pub use batch::{
    AdapterBatch, AdapterBatchParts, BatchState, CanonicalBatch, CanonicalBatchParts,
    MAX_OBSERVATIONS_PER_BATCH, MAX_RELATIONS_PER_BATCH,
};
pub use coordinator::{
    CancellationToken, FinishTransition, RefreshAdmission, RefreshCoordinator, RefreshOutcome,
    RefreshPermit, RefreshResult, RefreshUrgency,
};
pub use error::{EngineError, EngineErrorCode};
pub use executor::{
    ExecutionCounts, MAX_REPLAY_CONTINUATIONS_PER_RUN, OneShotExecutor, OneShotResult,
    ReplayCleanup,
};
pub use ports::{
    Adapter, AdapterCompletion, Clock, OperationControl, OperationStop, PortError, PortErrorCode,
    ScopeSink, SinkControl, SourceSink, WriterLease, WriterLeaseGuard,
};
pub use time::{MonotonicTime, RefreshDeadline, RefreshRequestId};
pub use values::{
    AdapterCheckpoint, AdapterCounters, AdapterDiagnosticCode, AdapterDiagnostics, ChunkProof,
    ChunkProofBatch, CompletionQuality, DiscoveredSource, MAX_ADAPTER_CHECKPOINT_BYTES,
    MAX_CHUNK_PROOFS_PER_BATCH, MAX_PROFILE_ID_BYTES, MAX_PROVIDER_ID_BYTES,
    MAX_SCOPE_MANIFEST_ENTRIES, MAX_SOURCE_ID_BYTES, SOURCE_CHUNK_BYTES, ScopeIdentity,
    ScopeManifest, SourceIdentity, SourceKind,
};
