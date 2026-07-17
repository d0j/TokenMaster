use std::ffi::OsString;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::config::DbConfig;
use rusqlite::limits::Limit;
use rusqlite::{Connection, ErrorCode, OpenFlags};
use sha2::{Digest, Sha256};
use tokenmaster_platform::{MAX_DURABLE_FILE_BYTES, PhysicalFileIdentity, ValidatedLocalDirectory};

use crate::usage::migration::{
    validate_archive_version, validate_current_indexes, validate_current_schema_only,
    validate_current_semantics,
};
use crate::usage::query::{
    PROGRESS_OP_INTERVAL, READ_BUSY_TIMEOUT_MS, READ_CACHE_SIZE_KIB, apply_read_policy,
};
use crate::{EXPECTED_SQLITE_VERSION, StoreError, StoreErrorCode, USAGE_SCHEMA_VERSION};

const ARCHIVE_FILE_NAME: &str = "tokenmaster.sqlite3";
const SNAPSHOT_ATTEMPTS: usize = 32;
const SNAPSHOT_PAGES_PER_STEP: i32 = 64;
const MAX_TRANSIENT_RETRIES: usize = 8;
const STEP_PAUSE: Duration = Duration::from_millis(1);
const TRANSIENT_PAUSE: Duration = Duration::from_millis(5);
const MAX_OPERATION_DURATION: Duration = Duration::from_secs(60 * 60);
const SNAPSHOT_BUSY_TIMEOUT_MS: u64 = 50;
const SQLITE_HEADER_BYTES: usize = 100;
const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";
const MAX_INTEGRITY_ROWS: usize = 100;
const IDENTITY_HASH_BUFFER_BYTES: usize = 64 * 1024;
const SQLITE_LENGTH_LIMIT_BYTES: i32 = 16 * 1024 * 1024;
const SQLITE_SQL_LENGTH_LIMIT_BYTES: i32 = 256 * 1024;
const SQLITE_COLUMN_LIMIT: i32 = 256;

#[derive(Clone, Eq, PartialEq)]
pub struct BackupSource {
    path: PathBuf,
}

impl BackupSource {
    pub fn new(data_root: &ValidatedLocalDirectory) -> Result<Self, StoreError> {
        let path = data_root.as_path().join(ARCHIVE_FILE_NAME);
        require_regular_file(&path)?;
        Ok(Self { path })
    }
}

impl fmt::Debug for BackupSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupSource([redacted])")
    }
}

#[derive(Clone)]
pub struct BackupStaging {
    directory: PathBuf,
    cleanup_failures: Arc<AtomicU64>,
}

impl BackupStaging {
    pub fn new(directory: &ValidatedLocalDirectory) -> Result<Self, StoreError> {
        let revalidated = ValidatedLocalDirectory::new(directory.as_path())
            .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
        if revalidated.as_path() != directory.as_path() {
            return Err(StoreError::new(StoreErrorCode::BackupIo));
        }
        Ok(Self {
            directory: directory.as_path().to_path_buf(),
            cleanup_failures: Arc::new(AtomicU64::new(0)),
        })
    }

    #[must_use]
    pub fn cleanup_failure_count(&self) -> u64 {
        self.cleanup_failures.load(Ordering::Acquire)
    }

    pub fn recover_abandoned_candidates(&self) -> Result<u64, StoreError> {
        let revalidated = ValidatedLocalDirectory::new(&self.directory)
            .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
        if revalidated.as_path() != self.directory {
            return Err(StoreError::new(StoreErrorCode::BackupIo));
        }
        let mut removed = 0_u64;
        for kind in ["snapshot", "compact"] {
            for attempt in 0..SNAPSHOT_ATTEMPTS {
                let path = self
                    .directory
                    .join(format!(".tokenmaster-{kind}-{attempt:02}.sqlite3"));
                removed = removed
                    .checked_add(try_remove_candidate_files(&path)?)
                    .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
            }
        }
        self.cleanup_failures.store(0, Ordering::Release);
        Ok(removed)
    }
}

impl fmt::Debug for BackupStaging {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupStaging([redacted])")
    }
}

#[derive(Clone)]
pub struct BackupControl {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}

impl BackupControl {
    pub fn new(cancelled: Arc<AtomicBool>, duration: Duration) -> Result<Self, StoreError> {
        if duration.is_zero() || duration > MAX_OPERATION_DURATION {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let deadline = Instant::now()
            .checked_add(duration)
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?;
        Ok(Self {
            cancelled,
            deadline,
        })
    }

    fn check(&self) -> Result<(), StoreError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(StoreError::new(StoreErrorCode::Cancelled));
        }
        if Instant::now() >= self.deadline {
            return Err(StoreError::new(StoreErrorCode::DeadlineExceeded));
        }
        Ok(())
    }

    fn interrupted_code(&self) -> StoreErrorCode {
        if self.cancelled.load(Ordering::Acquire) {
            StoreErrorCode::Cancelled
        } else if Instant::now() >= self.deadline {
            StoreErrorCode::DeadlineExceeded
        } else {
            StoreErrorCode::Database
        }
    }
}

impl fmt::Debug for BackupControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupControl([redacted])")
    }
}

pub struct BackupCandidate {
    path: Option<PathBuf>,
    cleanup_failures: Arc<AtomicU64>,
}

impl BackupCandidate {
    fn path(&self) -> Result<&Path, StoreError> {
        self.path
            .as_deref()
            .ok_or_else(|| StoreError::new(StoreErrorCode::BackupIo))
    }

    pub fn discard(mut self) -> Result<(), StoreError> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        match try_remove_candidate_files(path) {
            Ok(_) => {
                self.path = None;
                Ok(())
            }
            Err(error) => {
                record_cleanup_failure(&self.cleanup_failures);
                Err(error)
            }
        }
    }
}

impl fmt::Debug for BackupCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupCandidate([redacted])")
    }
}

impl Drop for BackupCandidate {
    fn drop(&mut self) {
        if let Some(path) = self.path.take()
            && try_remove_candidate_files(&path).is_err()
        {
            record_cleanup_failure(&self.cleanup_failures);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveVersionStatus {
    SupportedLegacy,
    Current,
    Newer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveVersionInspection {
    version: u32,
    status: ArchiveVersionStatus,
}

impl ArchiveVersionInspection {
    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn status(self) -> ArchiveVersionStatus {
        self.status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupRuntimePolicy {
    query_only: bool,
    foreign_keys: bool,
    trusted_schema: bool,
    defensive: bool,
    no_checkpoint_on_close: bool,
    query_planner_stability: bool,
    double_quoted_dml: bool,
    double_quoted_ddl: bool,
    cell_size_check: bool,
    mmap_size_bytes: u64,
    sqlite_length_limit_bytes: i32,
    sqlite_sql_length_limit_bytes: i32,
    sqlite_column_limit: i32,
}

impl BackupRuntimePolicy {
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
    pub const fn query_planner_stability(self) -> bool {
        self.query_planner_stability
    }

    #[must_use]
    pub const fn double_quoted_dml(self) -> bool {
        self.double_quoted_dml
    }

    #[must_use]
    pub const fn double_quoted_ddl(self) -> bool {
        self.double_quoted_ddl
    }

    #[must_use]
    pub const fn cell_size_check(self) -> bool {
        self.cell_size_check
    }

    #[must_use]
    pub const fn mmap_size_bytes(self) -> u64 {
        self.mmap_size_bytes
    }

    #[must_use]
    pub const fn sqlite_version(self) -> &'static str {
        EXPECTED_SQLITE_VERSION
    }

    #[must_use]
    pub const fn sqlite_length_limit_bytes(self) -> i32 {
        self.sqlite_length_limit_bytes
    }

    #[must_use]
    pub const fn sqlite_sql_length_limit_bytes(self) -> i32 {
        self.sqlite_sql_length_limit_bytes
    }

    #[must_use]
    pub const fn sqlite_column_limit(self) -> i32 {
        self.sqlite_column_limit
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct CandidateIdentity {
    physical: PhysicalFileIdentity,
    len: u64,
    sha256: [u8; 32],
}

pub struct VerifiedBackupCandidate {
    candidate: BackupCandidate,
    schema_version: u32,
    len: u64,
    runtime_policy: BackupRuntimePolicy,
    identity: CandidateIdentity,
}

impl VerifiedBackupCandidate {
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn runtime_policy(&self) -> BackupRuntimePolicy {
        self.runtime_policy
    }

    #[must_use]
    pub const fn integrity_verified(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn foreign_keys_verified(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn schema_verified(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn semantics_verified(&self) -> bool {
        true
    }

    pub fn revalidate_identity(&self, control: &BackupControl) -> Result<(), StoreError> {
        let observed = capture_candidate_identity(self.candidate.path()?, control)?;
        if observed == self.identity {
            Ok(())
        } else {
            Err(StoreError::new(StoreErrorCode::StaleBackupCandidate))
        }
    }
}

impl fmt::Debug for VerifiedBackupCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VerifiedBackupCandidate([redacted])")
    }
}

pub fn create_online_snapshot(
    source: &BackupSource,
    staging: &BackupStaging,
    control: &BackupControl,
) -> Result<BackupCandidate, StoreError> {
    create_online_snapshot_with_step_hook(source, staging, control, || Ok(()))
}

fn create_online_snapshot_with_step_hook<F>(
    source: &BackupSource,
    staging: &BackupStaging,
    control: &BackupControl,
    mut after_more: F,
) -> Result<BackupCandidate, StoreError>
where
    F: FnMut() -> Result<(), StoreError>,
{
    control.check()?;
    require_regular_file(&source.path)?;
    validate_sqlite_header(&source.path)?;
    let source_connection = Connection::open_with_flags(
        &source.path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    apply_snapshot_source_policy(&source_connection)?;
    require_bundled_sqlite_identity()?;

    let candidate = create_candidate(staging, "snapshot")?;
    let path = candidate.path()?;
    let mut destination = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    {
        let backup = Backup::new(&source_connection, &mut destination).map_err(map_sql)?;
        let mut transient_retries = 0_usize;
        loop {
            control.check()?;
            match backup.step(SNAPSHOT_PAGES_PER_STEP).map_err(map_sql)? {
                StepResult::Done => break,
                StepResult::More => {
                    transient_retries = 0;
                    after_more()?;
                    thread::sleep(STEP_PAUSE);
                }
                StepResult::Busy | StepResult::Locked => {
                    transient_retries = transient_retries.saturating_add(1);
                    if transient_retries > MAX_TRANSIENT_RETRIES {
                        return Err(StoreError::with_limit(
                            StoreErrorCode::Busy,
                            MAX_TRANSIENT_RETRIES as u64,
                        ));
                    }
                    thread::sleep(TRANSIENT_PAUSE);
                }
                _ => return Err(StoreError::new(StoreErrorCode::Database)),
            }
        }
    }
    drop(destination);
    validate_candidate_length(path)?;
    Ok(candidate)
}

pub fn inspect_archive_version(
    candidate: &BackupCandidate,
) -> Result<ArchiveVersionInspection, StoreError> {
    validate_sqlite_header(candidate.path()?)?;
    let connection = open_candidate(candidate.path()?)?;
    let _policy = apply_and_capture_verify_policy(&connection)?;
    require_expected_sqlite(&connection)?;
    inspect_connection_version(&connection)
}

pub fn verify_backup_candidate(
    candidate: BackupCandidate,
    control: &BackupControl,
) -> Result<VerifiedBackupCandidate, StoreError> {
    control.check()?;
    let path = candidate.path()?;
    let identity_before = capture_candidate_identity(path, control)?;
    let len = identity_before.len;
    validate_sqlite_header(path)?;
    reject_sqlite_sidecars(path)?;
    let connection = open_candidate(path)?;
    let runtime_policy = apply_and_capture_verify_policy(&connection)?;
    require_expected_sqlite(&connection)?;
    control.check()?;
    let cancelled = Arc::clone(&control.cancelled);
    let deadline = control.deadline;
    connection
        .progress_handler(
            PROGRESS_OP_INTERVAL,
            Some(move || cancelled.load(Ordering::Acquire) || Instant::now() >= deadline),
        )
        .map_err(map_sql)?;
    let verification = (|| {
        let inspection = inspect_connection_version(&connection)?;
        if inspection.status == ArchiveVersionStatus::Newer {
            return Err(StoreError::new(StoreErrorCode::SchemaTooNew));
        }
        verify_integrity(&connection)?;
        verify_foreign_keys(&connection)?;
        let version = i64::from(inspection.version);
        if inspection.status == ArchiveVersionStatus::Current {
            validate_current_indexes(&connection).map_err(map_index_validation_error)?;
            validate_current_schema_only(&connection).map_err(map_validation_error)?;
            verify_current_counts(&connection)?;
            verify_current_generations(&connection)?;
            validate_current_semantics(&connection).map_err(map_semantic_validation_error)?;
        } else {
            validate_archive_version(&connection, version).map_err(map_validation_error)?;
        }
        Ok(inspection)
    })();
    let clear = connection.progress_handler(0, None::<fn() -> bool>);
    if clear.is_err() {
        return Err(StoreError::new(StoreErrorCode::Database));
    }
    let inspection = verification.map_err(|error| normalize_control_error(error, control))?;
    control.check()?;
    drop(connection);
    let identity = capture_candidate_identity(path, control)?;
    if identity != identity_before {
        return Err(StoreError::new(StoreErrorCode::StaleBackupCandidate));
    }
    Ok(VerifiedBackupCandidate {
        candidate,
        schema_version: inspection.version,
        len,
        runtime_policy,
        identity,
    })
}

pub fn create_compact_snapshot(
    snapshot: &VerifiedBackupCandidate,
    staging: &BackupStaging,
    control: &BackupControl,
) -> Result<VerifiedBackupCandidate, StoreError> {
    control.check()?;
    snapshot.revalidate_identity(control)?;
    let source_path = snapshot.candidate.path()?;
    require_regular_file(source_path)?;
    let candidate = create_candidate(staging, "compact")?;
    let destination_path = candidate.path()?.to_path_buf();
    let source = Connection::open_with_flags(
        source_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql)?;
    apply_compact_source_policy(&source)?;
    let cancelled = Arc::clone(&control.cancelled);
    let deadline = control.deadline;
    source
        .progress_handler(
            1_000,
            Some(move || cancelled.load(Ordering::Acquire) || Instant::now() >= deadline),
        )
        .map_err(map_sql)?;
    let destination_text = destination_path
        .to_str()
        .ok_or_else(|| StoreError::new(StoreErrorCode::BackupIo))?;
    let vacuum = source.execute("VACUUM INTO ?1", [destination_text]);
    let clear = source.progress_handler(0, None::<fn() -> bool>);
    if clear.is_err() {
        return Err(StoreError::new(StoreErrorCode::Database));
    }
    if let Err(error) = vacuum {
        return Err(match error {
            rusqlite::Error::SqliteFailure(details, _)
                if details.code == ErrorCode::OperationInterrupted =>
            {
                StoreError::new(control.interrupted_code())
            }
            other => map_sql(other),
        });
    }
    control.check()?;
    drop(source);
    snapshot.revalidate_identity(control)?;
    let compact = verify_backup_candidate(candidate, control)?;
    if compact.len > snapshot.len {
        return Err(StoreError::new(StoreErrorCode::BackupSemanticCorrupt));
    }
    Ok(compact)
}

fn apply_snapshot_source_policy(connection: &Connection) -> Result<(), StoreError> {
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
        .map_err(map_sql)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
        .map_err(map_sql)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_DQS_DML, false)
        .map_err(map_sql)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_DQS_DDL, false)
        .map_err(map_sql)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)
        .map_err(map_sql)?;
    connection
        .pragma_update(None, "busy_timeout", SNAPSHOT_BUSY_TIMEOUT_MS as i64)
        .map_err(map_sql)?;
    connection
        .pragma_update(None, "mmap_size", 0_i64)
        .map_err(map_sql)?;
    connection
        .pragma_update(None, "query_only", "ON")
        .map_err(map_sql)?;
    Ok(())
}

fn apply_compact_source_policy(connection: &Connection) -> Result<(), StoreError> {
    apply_candidate_limits(connection)?;
    apply_read_policy(connection)?;
    connection
        .pragma_update(None, "query_only", "OFF")
        .map_err(map_sql)?;
    Ok(())
}

fn apply_and_capture_verify_policy(
    connection: &Connection,
) -> Result<BackupRuntimePolicy, StoreError> {
    apply_candidate_limits(connection)?;
    apply_read_policy(connection)?;

    let policy = BackupRuntimePolicy {
        query_only: pragma_i64(connection, "PRAGMA query_only")? == 1,
        foreign_keys: pragma_i64(connection, "PRAGMA foreign_keys")? == 1,
        trusted_schema: pragma_i64(connection, "PRAGMA trusted_schema")? == 1,
        defensive: connection
            .db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE)
            .map_err(map_sql)?,
        no_checkpoint_on_close: connection
            .db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE)
            .map_err(map_sql)?,
        query_planner_stability: connection
            .db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_ENABLE_QPSG)
            .map_err(map_sql)?,
        double_quoted_dml: connection
            .db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DQS_DML)
            .map_err(map_sql)?,
        double_quoted_ddl: connection
            .db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DQS_DDL)
            .map_err(map_sql)?,
        cell_size_check: pragma_i64(connection, "PRAGMA cell_size_check")? == 1,
        mmap_size_bytes: pragma_u64_or_zero(connection, "PRAGMA mmap_size")?,
        sqlite_length_limit_bytes: connection
            .limit(Limit::SQLITE_LIMIT_LENGTH)
            .map_err(map_sql)?,
        sqlite_sql_length_limit_bytes: connection
            .limit(Limit::SQLITE_LIMIT_SQL_LENGTH)
            .map_err(map_sql)?,
        sqlite_column_limit: connection
            .limit(Limit::SQLITE_LIMIT_COLUMN)
            .map_err(map_sql)?,
    };
    if !policy.query_only
        || !policy.foreign_keys
        || policy.trusted_schema
        || !policy.defensive
        || !policy.no_checkpoint_on_close
        || !policy.query_planner_stability
        || policy.double_quoted_dml
        || policy.double_quoted_ddl
        || !policy.cell_size_check
        || policy.mmap_size_bytes != 0
        || pragma_u64(connection, "PRAGMA busy_timeout")? != READ_BUSY_TIMEOUT_MS
        || negative_pragma_u64(connection, "PRAGMA cache_size")? != READ_CACHE_SIZE_KIB
        || pragma_i64(connection, "PRAGMA temp_store")? != 1
        || policy.sqlite_length_limit_bytes != SQLITE_LENGTH_LIMIT_BYTES
        || policy.sqlite_sql_length_limit_bytes != SQLITE_SQL_LENGTH_LIMIT_BYTES
        || policy.sqlite_column_limit != SQLITE_COLUMN_LIMIT
    {
        return Err(StoreError::new(StoreErrorCode::PolicyMismatch));
    }
    Ok(policy)
}

fn apply_candidate_limits(connection: &Connection) -> Result<(), StoreError> {
    connection
        .set_limit(Limit::SQLITE_LIMIT_LENGTH, SQLITE_LENGTH_LIMIT_BYTES)
        .map_err(map_sql)?;
    connection
        .set_limit(
            Limit::SQLITE_LIMIT_SQL_LENGTH,
            SQLITE_SQL_LENGTH_LIMIT_BYTES,
        )
        .map_err(map_sql)?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_COLUMN, SQLITE_COLUMN_LIMIT)
        .map_err(map_sql)?;
    Ok(())
}

fn create_candidate(staging: &BackupStaging, kind: &str) -> Result<BackupCandidate, StoreError> {
    let revalidated = ValidatedLocalDirectory::new(&staging.directory)
        .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
    if revalidated.as_path() != staging.directory {
        return Err(StoreError::new(StoreErrorCode::BackupIo));
    }
    for attempt in 0..SNAPSHOT_ATTEMPTS {
        let path = staging
            .directory
            .join(format!(".tokenmaster-{kind}-{attempt:02}.sqlite3"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                let metadata = match file.metadata() {
                    Ok(metadata) => metadata,
                    Err(_) => {
                        drop(file);
                        if try_remove_candidate_files(&path).is_err() {
                            record_cleanup_failure(&staging.cleanup_failures);
                        }
                        return Err(StoreError::new(StoreErrorCode::BackupIo));
                    }
                };
                if !metadata.is_file() || is_reparse_point(&metadata) {
                    drop(file);
                    if try_remove_candidate_files(&path).is_err() {
                        record_cleanup_failure(&staging.cleanup_failures);
                    }
                    return Err(StoreError::new(StoreErrorCode::BackupIo));
                }
                drop(file);
                return Ok(BackupCandidate {
                    path: Some(path),
                    cleanup_failures: Arc::clone(&staging.cleanup_failures),
                });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(StoreError::new(StoreErrorCode::BackupIo)),
        }
    }
    Err(StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn open_candidate(path: &Path) -> Result<Connection, StoreError> {
    require_regular_file(path)?;
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_candidate_open_error)
}

fn validate_sqlite_header(path: &Path) -> Result<(), StoreError> {
    let metadata =
        std::fs::metadata(path).map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
    let mut file = File::open(path).map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
    let mut header = [0_u8; SQLITE_HEADER_BYTES];
    file.read_exact(&mut header)
        .map_err(|_| StoreError::new(StoreErrorCode::BackupHeaderCorrupt))?;
    if &header[..SQLITE_MAGIC.len()] != SQLITE_MAGIC
        || !matches!(header[18], 1 | 2)
        || !matches!(header[19], 1 | 2)
        || header[20] != 0
        || header[21..24] != [64, 32, 32]
        || u32::from_be_bytes(
            header[44..48]
                .try_into()
                .map_err(|_| StoreError::new(StoreErrorCode::BackupHeaderCorrupt))?,
        ) != 4
    {
        return Err(StoreError::new(StoreErrorCode::BackupHeaderCorrupt));
    }
    let encoded_page_size = u16::from_be_bytes([header[16], header[17]]);
    let page_size = if encoded_page_size == 1 {
        65_536_u64
    } else {
        u64::from(encoded_page_size)
    };
    if !(512..=65_536).contains(&page_size)
        || !page_size.is_power_of_two()
        || metadata.len() < SQLITE_HEADER_BYTES as u64
        || metadata.len() % page_size != 0
    {
        return Err(StoreError::new(StoreErrorCode::BackupHeaderCorrupt));
    }
    Ok(())
}

fn verify_integrity(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection
        .prepare("PRAGMA integrity_check(100)")
        .map_err(map_integrity_error)?;
    let mut rows = statement.query([]).map_err(map_integrity_error)?;
    let mut count = 0_usize;
    let mut only_ok = true;
    let mut index_failure = false;
    while let Some(row) = rows.next().map_err(map_integrity_error)? {
        count = count.saturating_add(1);
        if count > MAX_INTEGRITY_ROWS {
            return Err(StoreError::new(StoreErrorCode::BackupPageCorrupt));
        }
        let message: String = row.get(0).map_err(map_integrity_error)?;
        if message != "ok" {
            only_ok = false;
            index_failure |= message.to_ascii_lowercase().contains("index");
        }
    }
    if count == 1 && only_ok {
        Ok(())
    } else if index_failure {
        Err(StoreError::new(StoreErrorCode::BackupIndexCorrupt))
    } else {
        Err(StoreError::new(StoreErrorCode::BackupPageCorrupt))
    }
}

fn verify_foreign_keys(connection: &Connection) -> Result<(), StoreError> {
    let failures: i64 = connection
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(map_integrity_error)?;
    if failures == 0 {
        Ok(())
    } else {
        Err(StoreError::new(StoreErrorCode::BackupForeignKeyCorrupt))
    }
}

fn verify_current_counts(connection: &Connection) -> Result<(), StoreError> {
    let valid: bool = connection
        .query_row(
            "SELECT
               coalesce((SELECT event_count FROM usage_legacy_snapshot WHERE snapshot_id=1), 0)
                 = (SELECT count(*) FROM usage_legacy_event WHERE snapshot_id=1)
               AND (SELECT retained_sample_count FROM quota_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM quota_sample)
               AND (SELECT retained_epoch_count FROM quota_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM quota_epoch_history)
               AND (SELECT retained_transition_count FROM quota_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM quota_transition)
               AND (SELECT current_lot_count FROM benefit_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM benefit_lot_current)
               AND (SELECT retained_change_count FROM benefit_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM benefit_change)
               AND (SELECT pending_due_count FROM benefit_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM benefit_reminder_due)
               AND (SELECT retained_delivery_count FROM benefit_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM benefit_reminder_delivery)
               AND (SELECT repository_count FROM git_installation_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM git_repository)
               AND (SELECT association_count FROM git_installation_state WHERE singleton_id=1)
                 = (SELECT count(*) FROM git_activity_association)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_stage_query_error(error, StoreErrorCode::BackupCountCorrupt))?;
    if valid {
        Ok(())
    } else {
        Err(StoreError::new(StoreErrorCode::BackupCountCorrupt))
    }
}

fn verify_current_generations(connection: &Connection) -> Result<(), StoreError> {
    let valid: bool = connection
        .query_row(
            "SELECT
               (SELECT expected_dataset_generation FROM usage_aggregate_state WHERE singleton_id=1)
                 = (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id=1)
               AND NOT EXISTS(
                 SELECT 1 FROM usage_time_rollup AS item, usage_aggregate_state AS state
                 WHERE state.singleton_id=1
                   AND item.aggregate_generation <> state.active_aggregate_generation
               )
               AND NOT EXISTS(
                 SELECT 1 FROM usage_session_rollup AS item, usage_aggregate_state AS state
                 WHERE state.singleton_id=1
                   AND item.aggregate_generation <> state.active_aggregate_generation
               )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_stage_query_error(error, StoreErrorCode::BackupGenerationCorrupt))?;
    if valid {
        Ok(())
    } else {
        Err(StoreError::new(StoreErrorCode::BackupGenerationCorrupt))
    }
}

fn inspect_connection_version(
    connection: &Connection,
) -> Result<ArchiveVersionInspection, StoreError> {
    let version = pragma_i64(connection, "PRAGMA user_version")?;
    let version =
        u32::try_from(version).map_err(|_| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let current = u32::try_from(USAGE_SCHEMA_VERSION)
        .map_err(|_| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let status = if version == current {
        ArchiveVersionStatus::Current
    } else if version >= 1 && version < current {
        ArchiveVersionStatus::SupportedLegacy
    } else if version > current {
        ArchiveVersionStatus::Newer
    } else {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    };
    Ok(ArchiveVersionInspection { version, status })
}

fn require_expected_sqlite(connection: &Connection) -> Result<(), StoreError> {
    let actual: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(map_sql)?;
    if actual == EXPECTED_SQLITE_VERSION {
        Ok(())
    } else {
        Err(StoreError::new(StoreErrorCode::VersionMismatch))
    }
}

fn require_bundled_sqlite_identity() -> Result<(), StoreError> {
    if rusqlite::version() == EXPECTED_SQLITE_VERSION {
        Ok(())
    } else {
        Err(StoreError::new(StoreErrorCode::VersionMismatch))
    }
}

fn validate_candidate_length(path: &Path) -> Result<u64, StoreError> {
    let length = std::fs::metadata(path)
        .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?
        .len();
    if length == 0 || length > MAX_DURABLE_FILE_BYTES {
        Err(StoreError::new(StoreErrorCode::CapacityExceeded))
    } else {
        Ok(length)
    }
}

fn capture_candidate_identity(
    path: &Path,
    control: &BackupControl,
) -> Result<CandidateIdentity, StoreError> {
    control.check()?;
    require_regular_file(path)?;
    let mut file = File::open(path).map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
    let metadata = file
        .metadata()
        .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
    if !metadata.is_file() || is_reparse_point(&metadata) {
        return Err(StoreError::new(StoreErrorCode::BackupIo));
    }
    let len = metadata.len();
    if len == 0 || len > MAX_DURABLE_FILE_BYTES {
        return Err(StoreError::new(StoreErrorCode::CapacityExceeded));
    }
    let physical = PhysicalFileIdentity::from_file(&file)
        .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
    let mut hasher = Sha256::new();
    let mut observed_len = 0_u64;
    let mut buffer = [0_u8; IDENTITY_HASH_BUFFER_BYTES];
    loop {
        control.check()?;
        let read = file
            .read(&mut buffer)
            .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
        if read == 0 {
            break;
        }
        observed_len = observed_len
            .checked_add(
                u64::try_from(read).map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?,
            )
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        if observed_len > len || observed_len > MAX_DURABLE_FILE_BYTES {
            return Err(StoreError::new(StoreErrorCode::StaleBackupCandidate));
        }
        hasher.update(&buffer[..read]);
    }
    if observed_len != len {
        return Err(StoreError::new(StoreErrorCode::StaleBackupCandidate));
    }
    Ok(CandidateIdentity {
        physical,
        len,
        sha256: hasher.finalize().into(),
    })
}

fn reject_sqlite_sidecars(path: &Path) -> Result<(), StoreError> {
    for suffix in ["-journal", "-wal", "-shm"] {
        let sidecar = suffixed_path(path, suffix);
        match std::fs::symlink_metadata(sidecar) {
            Ok(_) => return Err(StoreError::new(StoreErrorCode::BackupPageCorrupt)),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(StoreError::new(StoreErrorCode::BackupIo)),
        }
    }
    Ok(())
}

fn require_regular_file(path: &Path) -> Result<(), StoreError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(StoreError::new(StoreErrorCode::BackupIo));
    }
    Ok(())
}

fn try_remove_candidate_files(path: &Path) -> Result<u64, StoreError> {
    let mut removed = 0_u64;
    for candidate in [
        path.to_path_buf(),
        suffixed_path(path, "-journal"),
        suffixed_path(path, "-wal"),
        suffixed_path(path, "-shm"),
    ] {
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                return Err(StoreError::new(StoreErrorCode::BackupIo));
            }
            Ok(_) => {
                std::fs::remove_file(&candidate)
                    .map_err(|_| StoreError::new(StoreErrorCode::BackupIo))?;
                match std::fs::symlink_metadata(&candidate) {
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        removed = removed
                            .checked_add(1)
                            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
                    }
                    _ => return Err(StoreError::new(StoreErrorCode::BackupIo)),
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(StoreError::new(StoreErrorCode::BackupIo)),
        }
    }
    Ok(removed)
}

fn record_cleanup_failure(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
        Some(value.saturating_add(1))
    });
}

fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

fn pragma_i64(connection: &Connection, sql: &str) -> Result<i64, StoreError> {
    connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(map_integrity_error)
}

fn pragma_u64(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    u64::try_from(pragma_i64(connection, sql)?)
        .map_err(|_| StoreError::new(StoreErrorCode::PolicyMismatch))
}

fn pragma_u64_or_zero(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    match connection.query_row(sql, [], |row| row.get::<_, i64>(0)) {
        Ok(value) => {
            u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::PolicyMismatch))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(error) => Err(map_integrity_error(error)),
    }
}

fn negative_pragma_u64(connection: &Connection, sql: &str) -> Result<u64, StoreError> {
    pragma_i64(connection, sql)?
        .checked_neg()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| StoreError::new(StoreErrorCode::PolicyMismatch))
}

fn map_validation_error(error: StoreError) -> StoreError {
    match error.code() {
        StoreErrorCode::InvalidStoredValue => {
            StoreError::new(StoreErrorCode::BackupSemanticCorrupt)
        }
        _ => error,
    }
}

fn map_index_validation_error(error: StoreError) -> StoreError {
    if error.code() == StoreErrorCode::SchemaMismatch {
        StoreError::new(StoreErrorCode::BackupIndexCorrupt)
    } else {
        error
    }
}

fn map_semantic_validation_error(error: StoreError) -> StoreError {
    if matches!(
        error.code(),
        StoreErrorCode::InvalidStoredValue | StoreErrorCode::SchemaMismatch
    ) {
        StoreError::new(StoreErrorCode::BackupSemanticCorrupt)
    } else {
        error
    }
}

fn normalize_control_error(error: StoreError, control: &BackupControl) -> StoreError {
    if error.code() == StoreErrorCode::DeadlineExceeded {
        StoreError::new(control.interrupted_code())
    } else {
        error
    }
}

fn map_candidate_open_error(error: rusqlite::Error) -> StoreError {
    match sqlite_code(&error) {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
            StoreError::new(StoreErrorCode::Busy)
        }
        Some(ErrorCode::NotADatabase) => StoreError::new(StoreErrorCode::BackupHeaderCorrupt),
        Some(ErrorCode::DatabaseCorrupt) => StoreError::new(StoreErrorCode::BackupPageCorrupt),
        Some(code) if is_io_code(code) => StoreError::new(StoreErrorCode::BackupIo),
        _ => StoreError::new(StoreErrorCode::Database),
    }
}

fn map_stage_query_error(error: rusqlite::Error, stage: StoreErrorCode) -> StoreError {
    match sqlite_code(&error) {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
            StoreError::new(StoreErrorCode::Busy)
        }
        Some(ErrorCode::OperationInterrupted) => StoreError::new(StoreErrorCode::DeadlineExceeded),
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => {
            StoreError::new(StoreErrorCode::BackupPageCorrupt)
        }
        Some(code) if is_io_code(code) => StoreError::new(StoreErrorCode::BackupIo),
        Some(ErrorCode::ConstraintViolation | ErrorCode::TypeMismatch) => StoreError::new(stage),
        _ => StoreError::new(StoreErrorCode::Database),
    }
}

fn map_integrity_error(error: rusqlite::Error) -> StoreError {
    match sqlite_code(&error) {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
            StoreError::new(StoreErrorCode::Busy)
        }
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => {
            StoreError::new(StoreErrorCode::BackupPageCorrupt)
        }
        Some(ErrorCode::OperationInterrupted) => StoreError::new(StoreErrorCode::DeadlineExceeded),
        Some(code) if is_io_code(code) => StoreError::new(StoreErrorCode::BackupIo),
        _ => StoreError::new(StoreErrorCode::Database),
    }
}

fn map_sql(error: rusqlite::Error) -> StoreError {
    match sqlite_code(&error) {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
            StoreError::new(StoreErrorCode::Busy)
        }
        Some(ErrorCode::OperationInterrupted) => StoreError::new(StoreErrorCode::DeadlineExceeded),
        Some(code) if is_io_code(code) => StoreError::new(StoreErrorCode::BackupIo),
        _ => StoreError::new(StoreErrorCode::Database),
    }
}

fn is_io_code(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::CannotOpen
            | ErrorCode::DiskFull
            | ErrorCode::FileLockingProtocolFailed
            | ErrorCode::NoLargeFileSupport
            | ErrorCode::PermissionDenied
            | ErrorCode::ReadOnly
            | ErrorCode::SystemIoFailure
    )
}

fn sqlite_code(error: &rusqlite::Error) -> Option<ErrorCode> {
    match error {
        rusqlite::Error::SqliteFailure(details, _) => Some(details.code),
        _ => None,
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use tempfile::tempdir;

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    #[test]
    fn writer_commit_occurs_between_online_backup_page_steps() -> TestResult {
        let root = tempdir()?;
        let archive = root.path().join(ARCHIVE_FILE_NAME);
        let connection = Connection::open(&archive)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE item(id INTEGER PRIMARY KEY, value BLOB NOT NULL) STRICT;
             INSERT INTO item(value) VALUES(zeroblob(16777216));",
        )?;
        drop(connection);
        let staging_path = root.path().join("staging");
        std::fs::create_dir(&staging_path)?;
        let source = BackupSource::new(&ValidatedLocalDirectory::new(root.path())?)?;
        let staging = BackupStaging::new(&ValidatedLocalDirectory::new(&staging_path)?)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let control = BackupControl::new(cancelled, Duration::from_secs(5))?;
        let (more_tx, more_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);

        let backup = thread::spawn(move || {
            let mut paused = false;
            create_online_snapshot_with_step_hook(&source, &staging, &control, || {
                if !paused {
                    paused = true;
                    more_tx
                        .send(())
                        .map_err(|_| StoreError::new(StoreErrorCode::Database))?;
                    release_rx
                        .recv_timeout(Duration::from_secs(2))
                        .map_err(|_| StoreError::new(StoreErrorCode::DeadlineExceeded))?;
                }
                Ok(())
            })
        });

        more_rx.recv_timeout(Duration::from_secs(2))?;
        let writer = Connection::open(&archive)?;
        writer.execute("UPDATE item SET value=zeroblob(1024) WHERE id=1", [])?;
        drop(writer);
        release_tx.send(())?;
        let candidate = backup
            .join()
            .map_err(|_| std::io::Error::other("backup panicked"))??;
        candidate.discard()?;
        Ok(())
    }
}
