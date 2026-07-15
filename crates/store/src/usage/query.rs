use std::{fmt, path::Path, time::Duration, time::Instant};

use rusqlite::{
    Connection, ErrorCode, OpenFlags, Params, Row, TransactionBehavior, config::DbConfig, params,
};

use super::{
    JournalMode, MAX_SCAN_SCOPES, MAX_USAGE_EVENT_PAGE_SIZE,
    migration::validate_v6,
    schema::USAGE_SCHEMA_VERSION,
    types::{AccountingVersions, ArchivePublicationQuality, EventCursor, ScanScope},
};
use crate::{EXPECTED_SQLITE_VERSION, StoreError, StoreErrorCode};

const READ_CACHE_SIZE_KIB: u64 = 4 * 1024;
const READ_BUSY_TIMEOUT_MS: u64 = 250;
const MAX_QUERY_DURATION: Duration = Duration::from_secs(2);
const PROGRESS_OP_INTERVAL: i32 = 1_000;

const FIRST_CURRENT_ACTIVITY_SQL: &str =
    "SELECT source.provider_id, source.profile_id, event.profile_id,
            event.event_id, event.timestamp_seconds, event.timestamp_nanos, event.model,
            event.input_tokens, event.cached_tokens, event.output_tokens,
            event.reasoning_tokens, event.total_tokens, event.fingerprint
     FROM usage_event AS event
     LEFT JOIN usage_source AS source ON source.file_key = event.selected_file_key
     ORDER BY event.timestamp_seconds DESC, event.timestamp_nanos DESC,
              event.fingerprint DESC
     LIMIT ?1";
const CURSOR_CURRENT_ACTIVITY_SQL: &str =
    "SELECT source.provider_id, source.profile_id, event.profile_id,
            event.event_id, event.timestamp_seconds, event.timestamp_nanos, event.model,
            event.input_tokens, event.cached_tokens, event.output_tokens,
            event.reasoning_tokens, event.total_tokens, event.fingerprint
     FROM usage_event AS event
     LEFT JOIN usage_source AS source ON source.file_key = event.selected_file_key
     WHERE (event.timestamp_seconds, event.timestamp_nanos, event.fingerprint) < (?1, ?2, ?3)
     ORDER BY event.timestamp_seconds DESC, event.timestamp_nanos DESC,
              event.fingerprint DESC
     LIMIT ?4";
const FIRST_LEGACY_ACTIVITY_SQL: &str =
    "SELECT source.provider_id, source.profile_id, event.profile_id,
            event.event_id, event.timestamp_seconds, event.timestamp_nanos, event.model,
            event.input_tokens, event.cached_tokens, event.output_tokens,
            event.reasoning_tokens, event.total_tokens, event.fingerprint
     FROM usage_legacy_event AS event
     LEFT JOIN usage_source AS source ON source.file_key = event.selected_file_key
     WHERE event.snapshot_id = 1
     ORDER BY event.timestamp_seconds DESC, event.timestamp_nanos DESC,
              event.fingerprint DESC
     LIMIT ?1";
const CURSOR_LEGACY_ACTIVITY_SQL: &str =
    "SELECT source.provider_id, source.profile_id, event.profile_id,
            event.event_id, event.timestamp_seconds, event.timestamp_nanos, event.model,
            event.input_tokens, event.cached_tokens, event.output_tokens,
            event.reasoning_tokens, event.total_tokens, event.fingerprint
     FROM usage_legacy_event AS event
     LEFT JOIN usage_source AS source ON source.file_key = event.selected_file_key
     WHERE event.snapshot_id = 1
       AND (event.timestamp_seconds, event.timestamp_nanos, event.fingerprint) < (?1, ?2, ?3)
     ORDER BY event.timestamp_seconds DESC, event.timestamp_nanos DESC,
              event.fingerprint DESC
     LIMIT ?4";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageQueryDatasetIdentity {
    Empty,
    LegacySnapshotV1,
    ReplayRevision(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageActivityQuery {
    expected_dataset: Option<UsageQueryDatasetIdentity>,
    before: Option<EventCursor>,
    page_size: usize,
    deadline: Duration,
}

impl UsageActivityQuery {
    pub fn new(
        expected_dataset: Option<UsageQueryDatasetIdentity>,
        before: Option<EventCursor>,
        page_size: usize,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        if !(1..=MAX_USAGE_EVENT_PAGE_SIZE).contains(&page_size)
            || deadline.is_zero()
            || deadline > MAX_QUERY_DURATION
            || (before.is_some() && expected_dataset.is_none())
            || matches!(
                expected_dataset,
                Some(UsageQueryDatasetIdentity::ReplayRevision(value))
                    if value > i64::MAX as u64
            )
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            expected_dataset,
            before,
            page_size,
            deadline,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageQueryPublication {
    generation: u64,
    dataset_identity: UsageQueryDatasetIdentity,
    accounting_versions_current: bool,
    data_through_ms: Option<i64>,
    quality: ArchivePublicationQuality,
    scopes: Box<[ScanScope]>,
}

impl UsageQueryPublication {
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn dataset_identity(&self) -> UsageQueryDatasetIdentity {
        self.dataset_identity
    }

    #[must_use]
    pub const fn accounting_versions_current(&self) -> bool {
        self.accounting_versions_current
    }

    #[must_use]
    pub const fn data_through_ms(&self) -> Option<i64> {
        self.data_through_ms
    }

    #[must_use]
    pub const fn quality(&self) -> ArchivePublicationQuality {
        self.quality
    }

    #[must_use]
    pub const fn scopes(&self) -> &[ScanScope] {
        &self.scopes
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct UsageQueryEvent {
    provider_id: Box<str>,
    profile_id: Box<str>,
    event_id: Box<str>,
    timestamp_seconds: i64,
    timestamp_nanos: u32,
    model: Box<str>,
    input_tokens: Option<u64>,
    cached_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    total_tokens: Option<u64>,
    fingerprint: [u8; 32],
}

impl UsageQueryEvent {
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    #[must_use]
    pub const fn timestamp_seconds(&self) -> i64 {
        self.timestamp_seconds
    }

    #[must_use]
    pub const fn timestamp_nanos(&self) -> u32 {
        self.timestamp_nanos
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub const fn input_tokens(&self) -> Option<u64> {
        self.input_tokens
    }

    #[must_use]
    pub const fn cached_tokens(&self) -> Option<u64> {
        self.cached_tokens
    }

    #[must_use]
    pub const fn output_tokens(&self) -> Option<u64> {
        self.output_tokens
    }

    #[must_use]
    pub const fn reasoning_tokens(&self) -> Option<u64> {
        self.reasoning_tokens
    }

    #[must_use]
    pub const fn total_tokens(&self) -> Option<u64> {
        self.total_tokens
    }

    #[must_use]
    pub const fn cursor(&self) -> EventCursor {
        EventCursor {
            timestamp_seconds: self.timestamp_seconds,
            timestamp_nanos: self.timestamp_nanos,
            fingerprint: self.fingerprint,
        }
    }
}

impl fmt::Debug for UsageQueryEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsageQueryEvent")
            .field("provider_id", &self.provider_id)
            .field("profile_id", &self.profile_id)
            .field("event_id", &self.event_id)
            .field("timestamp_seconds", &self.timestamp_seconds)
            .field("timestamp_nanos", &self.timestamp_nanos)
            .field("model", &self.model)
            .field("input_tokens", &self.input_tokens)
            .field("cached_tokens", &self.cached_tokens)
            .field("output_tokens", &self.output_tokens)
            .field("reasoning_tokens", &self.reasoning_tokens)
            .field("total_tokens", &self.total_tokens)
            .field("fingerprint", &Redacted)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageQueryCapture {
    publication: UsageQueryPublication,
    events: Box<[UsageQueryEvent]>,
    next_cursor: Option<EventCursor>,
    has_more: bool,
}

impl UsageQueryCapture {
    #[must_use]
    pub const fn publication(&self) -> &UsageQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn events(&self) -> &[UsageQueryEvent] {
        &self.events
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<EventCursor> {
        self.next_cursor
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsageReadRuntimePolicy {
    journal_mode: JournalMode,
    query_only: bool,
    foreign_keys: bool,
    trusted_schema: bool,
    defensive: bool,
    no_checkpoint_on_close: bool,
    query_planner_stability: bool,
    double_quoted_dml: bool,
    double_quoted_ddl: bool,
    busy_timeout_ms: u64,
    cache_size_kib: u64,
    temp_store: i64,
    mmap_size_bytes: u64,
}

impl UsageReadRuntimePolicy {
    #[must_use]
    pub const fn journal_mode(self) -> JournalMode {
        self.journal_mode
    }

    #[must_use]
    pub const fn query_only(self) -> bool {
        self.query_only
    }

    #[must_use]
    pub const fn foreign_keys(self) -> bool {
        self.foreign_keys
    }

    #[must_use]
    pub const fn trusted_schema(self) -> bool {
        self.trusted_schema
    }

    #[must_use]
    pub const fn defensive(self) -> bool {
        self.defensive
    }

    #[must_use]
    pub const fn no_checkpoint_on_close(self) -> bool {
        self.no_checkpoint_on_close
    }

    #[must_use]
    pub const fn busy_timeout_ms(self) -> u64 {
        self.busy_timeout_ms
    }

    #[must_use]
    pub const fn cache_size_kib(self) -> u64 {
        self.cache_size_kib
    }

    #[must_use]
    pub const fn temp_store(self) -> i64 {
        self.temp_store
    }

    #[must_use]
    pub const fn mmap_size_bytes(self) -> u64 {
        self.mmap_size_bytes
    }
}

pub struct UsageReadStore {
    connection: Connection,
}

impl UsageReadStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = map_sql(Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ))?;
        let actual: String =
            map_sql(connection.query_row("SELECT sqlite_version()", [], |row| row.get(0)))?;
        if actual != EXPECTED_SQLITE_VERSION {
            return Err(StoreError::new(StoreErrorCode::VersionMismatch));
        }
        apply_read_policy(&connection)?;
        let version = pragma_i64(&connection, "PRAGMA user_version")?;
        if version > USAGE_SCHEMA_VERSION {
            return Err(StoreError::new(StoreErrorCode::SchemaTooNew));
        }
        if version != USAGE_SCHEMA_VERSION {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
        validate_v6(&connection)?;
        let store = Self { connection };
        store.runtime_policy()?;
        Ok(store)
    }

    pub fn sqlite_version(&self) -> Result<String, StoreError> {
        map_sql(
            self.connection
                .query_row("SELECT sqlite_version()", [], |row| row.get(0)),
        )
    }

    pub fn runtime_policy(&self) -> Result<UsageReadRuntimePolicy, StoreError> {
        let journal_mode_text: String = map_sql(self.connection.query_row(
            "PRAGMA journal_mode",
            [],
            |row| row.get(0),
        ))?;
        let journal_mode = match journal_mode_text.to_ascii_lowercase().as_str() {
            "wal" => JournalMode::Wal,
            "memory" => JournalMode::Memory,
            _ => return Err(StoreError::new(StoreErrorCode::PolicyMismatch)),
        };
        let policy = UsageReadRuntimePolicy {
            journal_mode,
            query_only: pragma_i64(&self.connection, "PRAGMA query_only")? == 1,
            foreign_keys: pragma_i64(&self.connection, "PRAGMA foreign_keys")? == 1,
            trusted_schema: pragma_i64(&self.connection, "PRAGMA trusted_schema")? == 1,
            defensive: map_sql(
                self.connection
                    .db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE),
            )?,
            no_checkpoint_on_close: map_sql(
                self.connection
                    .db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE),
            )?,
            query_planner_stability: map_sql(
                self.connection
                    .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_QPSG),
            )?,
            double_quoted_dml: map_sql(
                self.connection.db_config(DbConfig::SQLITE_DBCONFIG_DQS_DML),
            )?,
            double_quoted_ddl: map_sql(
                self.connection.db_config(DbConfig::SQLITE_DBCONFIG_DQS_DDL),
            )?,
            busy_timeout_ms: pragma_u64(&self.connection, "PRAGMA busy_timeout")?,
            cache_size_kib: negative_pragma_u64(&self.connection, "PRAGMA cache_size")?,
            temp_store: pragma_i64(&self.connection, "PRAGMA temp_store")?,
            mmap_size_bytes: pragma_u64_or_zero(&self.connection, "PRAGMA mmap_size")?,
        };
        if policy.journal_mode != JournalMode::Wal
            || !policy.query_only
            || !policy.foreign_keys
            || policy.trusted_schema
            || !policy.defensive
            || !policy.no_checkpoint_on_close
            || !policy.query_planner_stability
            || policy.double_quoted_dml
            || policy.double_quoted_ddl
            || policy.busy_timeout_ms != READ_BUSY_TIMEOUT_MS
            || policy.cache_size_kib != READ_CACHE_SIZE_KIB
            || policy.temp_store != 1
            || policy.mmap_size_bytes != 0
        {
            return Err(StoreError::new(StoreErrorCode::PolicyMismatch));
        }
        Ok(policy)
    }

    pub fn capture_activity_page(
        &mut self,
        query: UsageActivityQuery,
    ) -> Result<UsageQueryCapture, StoreError> {
        self.capture_activity_page_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_activity_page_with_options<F>(
        &mut self,
        query: UsageActivityQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<UsageQueryCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_activity_page(&mut self.connection, query, after_publication);
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

impl fmt::Debug for UsageReadStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("UsageReadStore([redacted])")
    }
}

fn apply_read_policy(connection: &Connection) -> Result<(), StoreError> {
    map_sql(connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true))?;
    map_sql(connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true))?;
    map_sql(connection.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_QPSG, true))?;
    map_sql(connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DQS_DML, false))?;
    map_sql(connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DQS_DDL, false))?;
    map_sql(connection.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false))?;
    map_sql(connection.pragma_update(None, "foreign_keys", "ON"))?;
    map_sql(connection.pragma_update(None, "busy_timeout", READ_BUSY_TIMEOUT_MS as i64))?;
    map_sql(connection.pragma_update(None, "cache_size", -(READ_CACHE_SIZE_KIB as i64)))?;
    map_sql(connection.pragma_update(None, "temp_store", "FILE"))?;
    map_sql(connection.pragma_update(None, "mmap_size", 0_i64))?;
    map_sql(connection.pragma_update(None, "trusted_schema", "OFF"))?;
    map_sql(connection.pragma_update(None, "query_only", "ON"))?;
    Ok(())
}

fn capture_activity_page<F>(
    connection: &mut Connection,
    query: UsageActivityQuery,
    after_publication: F,
) -> Result<UsageQueryCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let raw_publication = load_raw_publication(&transaction)?;
    after_publication()?;
    let dataset_identity = raw_publication.dataset_identity()?;
    if query
        .expected_dataset
        .is_some_and(|expected| expected != dataset_identity)
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let (data_through_ms, scopes) =
        load_scan_truth(&transaction, raw_publication.latest_complete_scan_set_id)?;
    let lookahead = query
        .page_size
        .checked_add(1)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut events = load_activity_events(&transaction, dataset_identity, query.before, lookahead)?;
    let has_more = events.len() > query.page_size;
    if has_more {
        events.truncate(query.page_size);
    }
    let next_cursor = has_more
        .then(|| events.last().map(UsageQueryEvent::cursor))
        .flatten();
    if has_more && next_cursor.is_none() {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let capture = UsageQueryCapture {
        publication: UsageQueryPublication {
            generation: nonnegative(raw_publication.archive_generation)?,
            dataset_identity,
            accounting_versions_current: raw_publication.accounting_versions_current()?,
            data_through_ms,
            quality: ArchivePublicationQuality::from_sql(&raw_publication.quality)?,
            scopes: scopes.into_boxed_slice(),
        },
        events: events.into_boxed_slice(),
        next_cursor,
        has_more,
    };
    map_sql(transaction.commit())?;
    Ok(capture)
}

struct RawPublication {
    archive_generation: i64,
    current_revision_id: Option<i64>,
    latest_complete_scan_set_id: Option<i64>,
    quality: String,
    canonicalizer_version: Option<i64>,
    fingerprint_version: Option<i64>,
    replay_signature_version: Option<i64>,
    has_legacy: bool,
}

impl RawPublication {
    fn dataset_identity(&self) -> Result<UsageQueryDatasetIdentity, StoreError> {
        match self.current_revision_id {
            Some(revision) => Ok(UsageQueryDatasetIdentity::ReplayRevision(nonnegative(
                revision,
            )?)),
            None if self.has_legacy => Ok(UsageQueryDatasetIdentity::LegacySnapshotV1),
            None => Ok(UsageQueryDatasetIdentity::Empty),
        }
    }

    fn accounting_versions_current(&self) -> Result<bool, StoreError> {
        match (
            self.current_revision_id,
            self.canonicalizer_version,
            self.fingerprint_version,
            self.replay_signature_version,
        ) {
            (None, None, None, None) => Ok(true),
            (Some(_), Some(canonicalizer), Some(fingerprint), Some(replay_signature)) => Ok(
                AccountingVersions::from_stored(canonicalizer, fingerprint, replay_signature)?
                    == AccountingVersions::compiled(),
            ),
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

fn load_raw_publication(connection: &Connection) -> Result<RawPublication, StoreError> {
    map_sql(connection.query_row(
        "SELECT archive.archive_generation, archive.current_revision_id,
                archive.latest_complete_scan_set_id, archive.incremental_state,
                revision.canonicalizer_version, revision.fingerprint_version,
                revision.replay_signature_version,
                EXISTS(SELECT 1 FROM usage_legacy_snapshot WHERE snapshot_id = 1)
         FROM usage_archive_state AS archive
         LEFT JOIN usage_replay_revision AS revision
           ON revision.revision_id = archive.current_revision_id
         WHERE archive.singleton_id = 1",
        [],
        |row| {
            Ok(RawPublication {
                archive_generation: row.get(0)?,
                current_revision_id: row.get(1)?,
                latest_complete_scan_set_id: row.get(2)?,
                quality: row.get(3)?,
                canonicalizer_version: row.get(4)?,
                fingerprint_version: row.get(5)?,
                replay_signature_version: row.get(6)?,
                has_legacy: row.get::<_, i64>(7)? == 1,
            })
        },
    ))
}

fn load_scan_truth(
    connection: &Connection,
    scan_set_id: Option<i64>,
) -> Result<(Option<i64>, Vec<ScanScope>), StoreError> {
    let Some(scan_set_id) = scan_set_id else {
        return Ok((None, Vec::new()));
    };
    let (completed_at_ms, state, expected_count): (Option<i64>, String, i64) =
        map_sql(connection.query_row(
            "SELECT completed_at_ms, completion_state, expected_scope_count
             FROM usage_scan_set WHERE scan_set_id = ?1",
            [scan_set_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ))?;
    let expected_count = usize::try_from(expected_count)
        .ok()
        .filter(|count| *count <= MAX_SCAN_SCOPES)
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if state != "complete" || completed_at_ms.is_none() {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let mut statement = map_sql(connection.prepare_cached(
        "SELECT provider_id, profile_id, completion_state
         FROM usage_scan WHERE scan_set_id = ?1
         ORDER BY provider_id, profile_id",
    ))?;
    let rows = map_sql(statement.query_map([scan_set_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }))?;
    let mut scopes = Vec::with_capacity(expected_count);
    for row in rows {
        let (provider, profile, child_state) = map_sql(row)?;
        if child_state != "complete" {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        scopes.push(
            ScanScope::new(provider, profile)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        );
    }
    if scopes.len() != expected_count {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok((completed_at_ms, scopes))
}

fn load_activity_events(
    connection: &Connection,
    dataset_identity: UsageQueryDatasetIdentity,
    before: Option<EventCursor>,
    limit: usize,
) -> Result<Vec<UsageQueryEvent>, StoreError> {
    let limit =
        i64::try_from(limit).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    match (dataset_identity, before) {
        (UsageQueryDatasetIdentity::Empty, _) => Ok(Vec::new()),
        (UsageQueryDatasetIdentity::ReplayRevision(_), None) => query_events(
            connection,
            FIRST_CURRENT_ACTIVITY_SQL,
            params![limit],
            limit,
        ),
        (UsageQueryDatasetIdentity::LegacySnapshotV1, None) => {
            query_events(connection, FIRST_LEGACY_ACTIVITY_SQL, params![limit], limit)
        }
        (UsageQueryDatasetIdentity::ReplayRevision(_), Some(cursor)) => {
            query_cursor_events(connection, CURSOR_CURRENT_ACTIVITY_SQL, cursor, limit)
        }
        (UsageQueryDatasetIdentity::LegacySnapshotV1, Some(cursor)) => {
            query_cursor_events(connection, CURSOR_LEGACY_ACTIVITY_SQL, cursor, limit)
        }
    }
}

fn query_cursor_events(
    connection: &Connection,
    sql: &'static str,
    cursor: EventCursor,
    limit: i64,
) -> Result<Vec<UsageQueryEvent>, StoreError> {
    let fingerprint = cursor.fingerprint();
    query_events(
        connection,
        sql,
        params![
            cursor.timestamp_seconds(),
            i64::from(cursor.timestamp_nanos()),
            fingerprint.as_slice(),
            limit
        ],
        limit,
    )
}

fn query_events(
    connection: &Connection,
    sql: &'static str,
    parameters: impl Params,
    limit: i64,
) -> Result<Vec<UsageQueryEvent>, StoreError> {
    let mut statement = map_sql(connection.prepare_cached(sql))?;
    let rows = map_sql(statement.query_map(parameters, raw_query_event))?;
    let capacity =
        usize::try_from(limit).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut events = Vec::with_capacity(capacity);
    for row in rows {
        events.push(map_sql(row)?.validate()?);
    }
    Ok(events)
}

fn raw_query_event(row: &Row<'_>) -> rusqlite::Result<RawQueryEvent> {
    Ok(RawQueryEvent {
        provider_id: row.get(0)?,
        source_profile_id: row.get(1)?,
        event_profile_id: row.get(2)?,
        event_id: row.get(3)?,
        timestamp_seconds: row.get(4)?,
        timestamp_nanos: row.get(5)?,
        model: row.get(6)?,
        input_tokens: row.get(7)?,
        cached_tokens: row.get(8)?,
        output_tokens: row.get(9)?,
        reasoning_tokens: row.get(10)?,
        total_tokens: row.get(11)?,
        fingerprint: row.get(12)?,
    })
}

struct RawQueryEvent {
    provider_id: Option<String>,
    source_profile_id: Option<String>,
    event_profile_id: String,
    event_id: String,
    timestamp_seconds: i64,
    timestamp_nanos: i64,
    model: String,
    input_tokens: Option<i64>,
    cached_tokens: Option<i64>,
    output_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    total_tokens: Option<i64>,
    fingerprint: Vec<u8>,
}

impl RawQueryEvent {
    fn validate(self) -> Result<UsageQueryEvent, StoreError> {
        let provider_id = self
            .provider_id
            .filter(|value| valid_ascii_id(value, 64))
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        let source_profile_id = self
            .source_profile_id
            .filter(|value| valid_ascii_id(value, 128))
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        if source_profile_id != self.event_profile_id
            || !valid_ascii_id(&self.event_profile_id, 128)
            || !valid_event_id(&self.event_id)
            || !valid_model(&self.model)
        {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        Ok(UsageQueryEvent {
            provider_id: provider_id.into_boxed_str(),
            profile_id: self.event_profile_id.into_boxed_str(),
            event_id: self.event_id.into_boxed_str(),
            timestamp_seconds: self.timestamp_seconds,
            timestamp_nanos: u32::try_from(self.timestamp_nanos)
                .ok()
                .filter(|value| *value < 1_000_000_000)
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
            model: self.model.into_boxed_str(),
            input_tokens: optional_nonnegative(self.input_tokens)?,
            cached_tokens: optional_nonnegative(self.cached_tokens)?,
            output_tokens: optional_nonnegative(self.output_tokens)?,
            reasoning_tokens: optional_nonnegative(self.reasoning_tokens)?,
            total_tokens: optional_nonnegative(self.total_tokens)?,
            fingerprint: <[u8; 32]>::try_from(self.fingerprint)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        })
    }
}

fn valid_ascii_id(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_event_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

fn valid_model(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/' | b':')
        })
}

fn optional_nonnegative(value: Option<i64>) -> Result<Option<u64>, StoreError> {
    value.map(nonnegative).transpose()
}

fn nonnegative(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn pragma_i64(connection: &Connection, sql: &str) -> Result<i64, StoreError> {
    map_sql(connection.query_row(sql, [], |row| row.get(0)))
}

fn pragma_u64(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    let value = pragma_i64(connection, sql)?;
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::PolicyMismatch))
}

fn pragma_u64_or_zero(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    match connection.query_row(sql, [], |row| row.get::<_, i64>(0)) {
        Ok(value) => {
            u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::PolicyMismatch))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(error) => Err(map_sql_error(error)),
    }
}

fn negative_pragma_u64(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    pragma_i64(connection, sql)?
        .checked_neg()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| StoreError::new(StoreErrorCode::PolicyMismatch))
}

fn map_sql<T>(result: rusqlite::Result<T>) -> Result<T, StoreError> {
    result.map_err(map_sql_error)
}

fn map_sql_error(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::SqliteFailure(details, _)
            if details.code == ErrorCode::OperationInterrupted =>
        {
            StoreError::new(StoreErrorCode::DeadlineExceeded)
        }
        _ => StoreError::new(StoreErrorCode::Database),
    }
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs};

    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::UsageStore;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn empty_archive() -> TestResult<(TempDir, std::path::PathBuf)> {
        let directory = TempDir::new()?;
        let path = directory.path().join("usage.sqlite3");
        drop(UsageStore::open(&path)?);
        Ok((directory, path))
    }

    fn activity_query(deadline: Duration) -> Result<UsageActivityQuery, StoreError> {
        UsageActivityQuery::new(None, None, 16, deadline)
    }

    #[test]
    fn read_transaction_keeps_publication_exact_during_concurrent_commit() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let writer_path = path.clone();
        let capture = store.capture_activity_page_with_options(
            activity_query(Duration::from_secs(2))?,
            PROGRESS_OP_INTERVAL,
            false,
            move || {
                let writer = map_sql(Connection::open(&writer_path))?;
                map_sql(writer.execute(
                    "UPDATE usage_archive_state SET archive_generation = 1
                     WHERE singleton_id = 1",
                    [],
                ))?;
                Ok(())
            },
        )?;
        assert_eq!(capture.publication().generation(), 0);
        assert_eq!(
            store
                .capture_activity_page(activity_query(Duration::from_secs(2))?)?
                .publication()
                .generation(),
            1
        );
        Ok(())
    }

    #[test]
    fn deterministic_progress_cancellation_is_cleared_for_next_query() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let interrupted = match store.capture_activity_page_with_options(
            activity_query(Duration::from_secs(2))?,
            1,
            true,
            || Ok(()),
        ) {
            Err(error) => error,
            Ok(_) => return Err("forced cancellation unexpectedly completed".into()),
        };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_activity_page(activity_query(Duration::from_secs(2))?)?;
        assert_eq!(next.publication().generation(), 0);
        Ok(())
    }

    #[test]
    fn cursor_plan_uses_composite_index_without_offset_or_temp_sort() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let store = UsageReadStore::open(&path)?;
        let fingerprint = [0_u8; 32];
        let explain = format!("EXPLAIN QUERY PLAN {CURSOR_CURRENT_ACTIVITY_SQL}");
        let mut statement = store.connection.prepare(&explain)?;
        let rows = statement.query_map(
            params![0_i64, 0_i64, fingerprint.as_slice(), 257_i64],
            |row| row.get::<_, String>(3),
        )?;
        let mut details = Vec::new();
        for row in rows {
            details.push(row?);
        }
        let joined = details.join("\n");
        assert!(joined.contains("usage_event_time_desc"), "{joined}");
        assert!(!joined.contains("USE TEMP B-TREE"), "{joined}");
        let normalized = CURSOR_CURRENT_ACTIVITY_SQL.to_ascii_lowercase();
        assert!(!normalized.contains(" offset "));
        assert!(!normalized.contains("count("));
        assert!(!fs::read(path)?.is_empty());
        Ok(())
    }
}
