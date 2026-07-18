//! Usage append batches accept only canonical accounting output.
//!
//! Canonical events cannot be constructed by store callers:
//!
//! ```compile_fail
//! let _ = tokenmaster_accounting::CanonicalUsageEvent::new();
//! ```

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod backup;
mod error;
mod schema;
mod session_store;
mod usage;

pub use backup::{
    ArchiveVersionInspection, ArchiveVersionStatus, BackupCandidate, BackupControl,
    BackupRuntimePolicy, BackupSource, BackupStaging, MAX_VERIFIED_BACKUP_READ_CHUNK_BYTES,
    RecoveryVerificationBoundary, StartupArchiveInspection, StartupArchiveStatus,
    StartupValidationMode, VerifiedBackupCandidate, VerifiedBackupCandidateReader,
    VerifiedRecoveryArchive, create_compact_snapshot, create_fresh_recovery_archive,
    create_online_snapshot, inspect_archive_version, inspect_startup_archive,
    verify_backup_candidate, verify_recovery_archive, verify_recovery_archive_with_observer,
};
pub use error::{StoreError, StoreErrorCode};
pub use session_store::{EXPECTED_SQLITE_VERSION, MAX_PAGE_SIZE, MAX_SEED_SESSIONS, ProbeStore};
pub use usage::{
    AccountingVersions, AggregateRebuildProgress, AggregateRebuildStatus, AppendBatch,
    AppendBatchParts, ArchiveGeneration, ArchiveMode, ArchivePublication,
    ArchivePublicationQuality, ArchiveState, BenefitApplyResult, BenefitApplyStatus,
    BenefitChangeCursor, BenefitChangePageCapture, BenefitChangePageQuery, BenefitChangeRecord,
    BenefitCurrentCapture, BenefitCurrentQuery, BenefitDueSnapshot, BenefitInventoryRevision,
    BenefitMaintenanceResult, BenefitOverviewCapture, BenefitOverviewQuery,
    BenefitOverviewScopeCapture, BenefitProfileApplyResult, BenefitReminderAcknowledgeResult,
    BenefitReminderDelivery, BenefitReminderProcessResult, BenefitReminderProfileSnapshot,
    BenefitScopeSnapshot, CurrentReplayAppendBatch, CurrentReplayAppendBatchParts,
    CurrentReplayCommit, CurrentScanPublication, CurrentScanPublicationParts,
    DEFAULT_BENEFIT_CHANGES_PER_SCOPE, DEFAULT_BENEFIT_DELIVERIES_PER_SCOPE,
    DEFAULT_QUOTA_EPOCHS_PER_WINDOW, DEFAULT_QUOTA_SAMPLES_PER_WINDOW,
    DEFAULT_QUOTA_TRANSITIONS_PER_WINDOW, DatasetGeneration, EventCursor, GenerationSnapshot,
    GenerationStatus, GitCacheIdentity, GitIncrementalAuthority, GitOutputCapture, GitOutputQuery,
    GitOutputRepositoryCapture, GitProjectKey, GitProjectMatchCapture, GitProjectMatchQuery,
    GitProjectionInput, GitProjectionInputParts, GitPublication, GitRangeMetrics, GitRefreshInput,
    GitRefreshInputParts, JournalMode, MAX_AGGREGATE_REBUILD_PAGE_SIZE, MAX_APPEND_CHUNK_UPDATES,
    MAX_APPEND_EVENTS, MAX_APPEND_RELATIONS, MAX_BENEFIT_CHANGE_PAGE_SIZE,
    MAX_BENEFIT_CHANGES_PER_SCOPE, MAX_BENEFIT_CURRENT_LOTS, MAX_BENEFIT_DELIVERIES_PER_SCOPE,
    MAX_BENEFIT_MAINTENANCE_PAGE_SIZE, MAX_BENEFIT_OVERVIEW_LOTS, MAX_BENEFIT_OVERVIEW_SCOPES,
    MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE, MAX_QUOTA_CURRENT_WINDOWS, MAX_QUOTA_EPOCHS_PER_WINDOW,
    MAX_QUOTA_MAINTENANCE_PAGE_SIZE, MAX_QUOTA_SAMPLES_PER_WINDOW, MAX_QUOTA_TRANSITION_PAGE_SIZE,
    MAX_QUOTA_TRANSITIONS_PER_WINDOW, MAX_REPLAY_SOURCES, MAX_RESUME_BYTES, MAX_SCAN_SCOPES,
    MAX_USAGE_BREAKDOWN_ITEMS, MAX_USAGE_BREAKDOWNS, MAX_USAGE_EVENT_PAGE_SIZE,
    MAX_USAGE_OVERVIEW_SEGMENTS, MAX_USAGE_PRICE_BASIS_KEYS, MAX_USAGE_PRICE_BASIS_TARGETS,
    MAX_USAGE_QUERY_SCOPES, MAX_USAGE_SERIES_POINTS, MAX_USAGE_SESSION_DETAIL_ITEMS,
    MAX_USAGE_SESSION_PAGE_SIZE, ProductAggregateProgress, ProductAggregateState,
    ProductAggregateStatus, ProductBenefitStatus, ProductDataStatusCapture, ProductDataStatusQuery,
    ProductGitStatus, ProductQuotaStatus, ProductUsageStatus, QuotaApplyResult, QuotaApplyStatus,
    QuotaCurrentCapture, QuotaCurrentEpoch, QuotaCurrentQuery, QuotaCurrentWindow,
    QuotaMaintenanceResult, QuotaOverviewQuery, QuotaRevision, QuotaTransitionCursor,
    QuotaTransitionPageCapture, QuotaTransitionPageQuery, QuotaTransitionRecord, ReplayAppendBatch,
    ReplayAppendBatchParts, ReplayContinuationResult, ReplayEpoch, ReplayManifest,
    ReplayQualityCounts, ReplayRelation, ReplayRevisionId, ReplayRevisionSnapshot,
    ReplayRevisionStatus, RuntimePolicy, SCAN_HISTORY_PER_SCOPE, SCAN_PRUNE_BATCH_SIZE,
    SOURCE_CHUNK_BYTES, ScanCounters, ScanId, ScanOutcome, ScanScope, ScanSetId, ScanSetManifest,
    ScanSetSnapshot, ScanSnapshot, SourceKey, SourceKind, SourceRegistration,
    SourceRegistrationParts, StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk,
    StoredUsageEvent, StoredVerification, USAGE_SCHEMA_VERSION, UsageActivityQuery,
    UsageAggregateActivity, UsageAggregateBucketWidth, UsageAggregateMetrics, UsageAggregateRange,
    UsageAggregateSegment, UsageAnalyticsCapture, UsageAnalyticsQuery, UsageBreakdown,
    UsageBreakdownIdentity, UsageBreakdownItem, UsageBreakdownKind, UsageBreakdownPriceBasisQuery,
    UsageOverviewCapture, UsageOverviewQuery, UsagePriceBasisBatchCapture,
    UsagePriceBasisBatchQuery, UsagePriceBasisCapture, UsagePriceBasisKey, UsagePriceBasisMetrics,
    UsagePriceBasisQuery, UsagePriceBasisRow, UsagePriceBasisTargetCapture, UsagePriceLongContext,
    UsagePriceTier, UsageQueryCapture, UsageQueryDatasetIdentity, UsageQueryEvent,
    UsageQueryPublication, UsageReadRuntimePolicy, UsageReadStore, UsageReportedCostState,
    UsageSeriesPoint, UsageSeriesPointCapture, UsageSessionBreakdownPriceBasisQuery,
    UsageSessionCursor, UsageSessionDetail, UsageSessionDetailCapture, UsageSessionDetailQuery,
    UsageSessionKey, UsageSessionPageCapture, UsageSessionPageQuery,
    UsageSessionPriceBasisBatchQuery, UsageSessionPriceBasisQuery, UsageSessionSummary, UsageStore,
    UsageStoreCounts, UsageTokenAggregate,
};
