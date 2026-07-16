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
    AccountingVersions, AggregateRebuildProgress, AggregateRebuildStatus, AppendBatch,
    AppendBatchParts, ArchiveGeneration, ArchiveMode, ArchivePublication,
    ArchivePublicationQuality, ArchiveState, CurrentReplayAppendBatch,
    CurrentReplayAppendBatchParts, CurrentReplayCommit, CurrentScanPublication,
    CurrentScanPublicationParts, DatasetGeneration, EventCursor, GenerationSnapshot,
    GenerationStatus, JournalMode, MAX_AGGREGATE_REBUILD_PAGE_SIZE, MAX_APPEND_CHUNK_UPDATES,
    MAX_APPEND_EVENTS, MAX_APPEND_RELATIONS, MAX_REPLAY_SOURCES, MAX_RESUME_BYTES, MAX_SCAN_SCOPES,
    MAX_USAGE_EVENT_PAGE_SIZE, MAX_USAGE_OVERVIEW_SEGMENTS, MAX_USAGE_QUERY_SCOPES,
    ReplayAppendBatch, ReplayAppendBatchParts, ReplayContinuationResult, ReplayEpoch,
    ReplayManifest, ReplayQualityCounts, ReplayRelation, ReplayRevisionId, ReplayRevisionSnapshot,
    ReplayRevisionStatus, RuntimePolicy, SCAN_HISTORY_PER_SCOPE, SCAN_PRUNE_BATCH_SIZE,
    SOURCE_CHUNK_BYTES, ScanCounters, ScanId, ScanOutcome, ScanScope, ScanSetId, ScanSetManifest,
    ScanSetSnapshot, ScanSnapshot, SourceKey, SourceKind, SourceRegistration,
    SourceRegistrationParts, StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk,
    StoredUsageEvent, StoredVerification, USAGE_SCHEMA_VERSION, UsageActivityQuery,
    UsageAggregateActivity, UsageAggregateBucketWidth, UsageAggregateMetrics,
    UsageAggregateSegment, UsageOverviewCapture, UsageOverviewQuery, UsageQueryCapture,
    UsageQueryDatasetIdentity, UsageQueryEvent, UsageQueryPublication, UsageReadRuntimePolicy,
    UsageReadStore, UsageStore, UsageStoreCounts, UsageTokenAggregate,
};
