use std::fmt;
use std::path::Path;

use rusqlite::Connection;

use crate::{EXPECTED_SQLITE_VERSION, StoreError, StoreErrorCode};

mod aggregate;
mod incremental;
mod migration;
mod price_schema;
mod query;
mod read;
mod replay;
mod replay_manifest;
mod scan;
mod schema;
mod types;
mod write;

pub use query::{
    MAX_USAGE_BREAKDOWN_ITEMS, MAX_USAGE_BREAKDOWNS, MAX_USAGE_OVERVIEW_SEGMENTS,
    MAX_USAGE_PRICE_BASIS_KEYS, MAX_USAGE_PRICE_BASIS_TARGETS, MAX_USAGE_QUERY_SCOPES,
    MAX_USAGE_SERIES_POINTS, MAX_USAGE_SESSION_DETAIL_ITEMS, MAX_USAGE_SESSION_PAGE_SIZE,
    UsageActivityQuery, UsageAggregateActivity, UsageAggregateBucketWidth, UsageAggregateMetrics,
    UsageAggregateRange, UsageAggregateSegment, UsageAnalyticsCapture, UsageAnalyticsQuery,
    UsageBreakdown, UsageBreakdownIdentity, UsageBreakdownItem, UsageBreakdownKind,
    UsageBreakdownPriceBasisQuery, UsageOverviewCapture, UsageOverviewQuery,
    UsagePriceBasisBatchCapture, UsagePriceBasisBatchQuery, UsagePriceBasisCapture,
    UsagePriceBasisKey, UsagePriceBasisMetrics, UsagePriceBasisQuery, UsagePriceBasisRow,
    UsagePriceBasisTargetCapture, UsagePriceLongContext, UsagePriceTier, UsageQueryCapture,
    UsageQueryDatasetIdentity, UsageQueryEvent, UsageQueryPublication, UsageReadRuntimePolicy,
    UsageReadStore, UsageReportedCostState, UsageSeriesPoint, UsageSeriesPointCapture,
    UsageSessionBreakdownPriceBasisQuery, UsageSessionCursor, UsageSessionDetail,
    UsageSessionDetailCapture, UsageSessionDetailQuery, UsageSessionKey, UsageSessionPageCapture,
    UsageSessionPageQuery, UsageSessionPriceBasisBatchQuery, UsageSessionPriceBasisQuery,
    UsageSessionSummary, UsageTokenAggregate,
};
pub use schema::USAGE_SCHEMA_VERSION;
pub use types::{
    AccountingVersions, AggregateRebuildProgress, AggregateRebuildStatus, AppendBatch,
    AppendBatchParts, ArchiveGeneration, ArchiveMode, ArchivePublication,
    ArchivePublicationQuality, ArchiveState, CurrentReplayAppendBatch,
    CurrentReplayAppendBatchParts, CurrentReplayCommit, CurrentScanPublication,
    CurrentScanPublicationParts, DatasetGeneration, EventCursor, GenerationSnapshot,
    GenerationStatus, MAX_AGGREGATE_REBUILD_PAGE_SIZE, MAX_APPEND_CHUNK_UPDATES, MAX_APPEND_EVENTS,
    MAX_APPEND_RELATIONS, MAX_REPLAY_SOURCES, MAX_RESUME_BYTES, MAX_SCAN_SCOPES,
    MAX_USAGE_EVENT_PAGE_SIZE, ReplayAppendBatch, ReplayAppendBatchParts, ReplayContinuationResult,
    ReplayEpoch, ReplayManifest, ReplayQualityCounts, ReplayRelation, ReplayRevisionId,
    ReplayRevisionSnapshot, ReplayRevisionStatus, SCAN_HISTORY_PER_SCOPE, SCAN_PRUNE_BATCH_SIZE,
    SOURCE_CHUNK_BYTES, ScanCounters, ScanId, ScanOutcome, ScanScope, ScanSetId, ScanSetManifest,
    ScanSetSnapshot, ScanSnapshot, SourceKey, SourceKind, SourceRegistration,
    SourceRegistrationParts, StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk,
    StoredUsageEvent, StoredVerification, UsageStoreCounts,
};

use migration::migrate_schema;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalMode {
    Wal,
    Memory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePolicy {
    journal_mode: JournalMode,
    synchronous: i64,
    foreign_keys: bool,
    busy_timeout_ms: u64,
    wal_autocheckpoint_pages: u64,
    journal_size_limit_bytes: u64,
    cache_size_kib: u64,
    temp_store: i64,
    mmap_size_bytes: u64,
}

impl RuntimePolicy {
    #[must_use]
    pub const fn journal_mode(&self) -> JournalMode {
        self.journal_mode
    }

    #[must_use]
    pub const fn synchronous(&self) -> i64 {
        self.synchronous
    }

    #[must_use]
    pub const fn foreign_keys(&self) -> bool {
        self.foreign_keys
    }

    #[must_use]
    pub const fn busy_timeout_ms(&self) -> u64 {
        self.busy_timeout_ms
    }

    #[must_use]
    pub const fn wal_autocheckpoint_pages(&self) -> u64 {
        self.wal_autocheckpoint_pages
    }

    #[must_use]
    pub const fn journal_size_limit_bytes(&self) -> u64 {
        self.journal_size_limit_bytes
    }

    #[must_use]
    pub const fn cache_size_kib(&self) -> u64 {
        self.cache_size_kib
    }

    #[must_use]
    pub const fn temp_store(&self) -> i64 {
        self.temp_store
    }

    #[must_use]
    pub const fn mmap_size_bytes(&self) -> u64 {
        self.mmap_size_bytes
    }
}

pub struct UsageStore {
    pub(super) connection: Connection,
    in_memory: bool,
}

impl UsageStore {
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::initialize(Connection::open_in_memory()?, true)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::initialize(Connection::open(path)?, false)
    }

    fn initialize(mut connection: Connection, in_memory: bool) -> Result<Self, StoreError> {
        let actual: String =
            connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
        if actual != EXPECTED_SQLITE_VERSION {
            return Err(StoreError::new(StoreErrorCode::VersionMismatch));
        }
        apply_runtime_policy(&connection, in_memory)?;
        migrate_schema(&mut connection)?;
        let store = Self {
            connection,
            in_memory,
        };
        store.runtime_policy()?;
        Ok(store)
    }

    pub fn sqlite_version(&self) -> Result<String, StoreError> {
        Ok(self
            .connection
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))?)
    }

    pub fn runtime_policy(&self) -> Result<RuntimePolicy, StoreError> {
        let journal_mode_text: String =
            self.connection
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        let journal_mode = match journal_mode_text.to_ascii_lowercase().as_str() {
            "wal" => JournalMode::Wal,
            "memory" => JournalMode::Memory,
            _ => return Err(StoreError::new(StoreErrorCode::PolicyMismatch)),
        };
        let synchronous = pragma_i64(&self.connection, "PRAGMA synchronous")?;
        let foreign_keys = pragma_i64(&self.connection, "PRAGMA foreign_keys")? == 1;
        let busy_timeout = pragma_nonnegative(&self.connection, "PRAGMA busy_timeout")?;
        let wal_autocheckpoint = pragma_nonnegative(&self.connection, "PRAGMA wal_autocheckpoint")?;
        let journal_size_limit = pragma_nonnegative(&self.connection, "PRAGMA journal_size_limit")?;
        let cache_size = pragma_i64(&self.connection, "PRAGMA cache_size")?;
        let temp_store = pragma_i64(&self.connection, "PRAGMA temp_store")?;
        let mmap_size = pragma_nonnegative_or_zero(&self.connection, "PRAGMA mmap_size")?;
        let cache_size_kib = cache_size
            .checked_neg()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| StoreError::new(StoreErrorCode::PolicyMismatch))?;
        let policy = RuntimePolicy {
            journal_mode,
            synchronous,
            foreign_keys,
            busy_timeout_ms: busy_timeout,
            wal_autocheckpoint_pages: wal_autocheckpoint,
            journal_size_limit_bytes: journal_size_limit,
            cache_size_kib,
            temp_store,
            mmap_size_bytes: mmap_size,
        };
        let expected_journal = if self.in_memory {
            JournalMode::Memory
        } else {
            JournalMode::Wal
        };
        if policy.journal_mode != expected_journal
            || policy.synchronous != 2
            || !policy.foreign_keys
            || policy.busy_timeout_ms != 250
            || policy.wal_autocheckpoint_pages != 1_000
            || policy.journal_size_limit_bytes != 16 * 1024 * 1024
            || policy.cache_size_kib != 8 * 1024
            || policy.temp_store != 1
            || policy.mmap_size_bytes != 0
        {
            return Err(StoreError::new(StoreErrorCode::PolicyMismatch));
        }
        Ok(policy)
    }
}

impl fmt::Debug for UsageStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsageStore")
            .field("in_memory", &self.in_memory)
            .finish_non_exhaustive()
    }
}

fn apply_runtime_policy(connection: &Connection, in_memory: bool) -> Result<(), StoreError> {
    let requested_journal = if in_memory { "MEMORY" } else { "WAL" };
    let _: String = connection.query_row(
        &format!("PRAGMA journal_mode = {requested_journal}"),
        [],
        |row| row.get(0),
    )?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "busy_timeout", 250_i64)?;
    connection.pragma_update(None, "wal_autocheckpoint", 1_000_i64)?;
    connection.pragma_update(None, "journal_size_limit", 16_777_216_i64)?;
    connection.pragma_update(None, "cache_size", -8_192_i64)?;
    connection.pragma_update(None, "temp_store", "FILE")?;
    connection.pragma_update(None, "mmap_size", 0_i64)?;
    Ok(())
}

fn pragma_i64(connection: &Connection, sql: &str) -> Result<i64, StoreError> {
    Ok(connection.query_row(sql, [], |row| row.get(0))?)
}

fn pragma_nonnegative(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    let value = pragma_i64(connection, sql)?;
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::PolicyMismatch))
}

fn pragma_nonnegative_or_zero(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    match connection.query_row(sql, [], |row| row.get::<_, i64>(0)) {
        Ok(value) => {
            u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::PolicyMismatch))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(error) => Err(error.into()),
    }
}
