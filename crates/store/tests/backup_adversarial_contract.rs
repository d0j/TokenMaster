use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tempfile::{TempDir, tempdir};
use tokenmaster_platform::ValidatedLocalDirectory;
use tokenmaster_store::{
    BackupCandidate, BackupControl, BackupSource, BackupStaging, StoreErrorCode, UsageStore,
    VerifiedBackupCandidate, create_compact_snapshot, create_online_snapshot,
    verify_backup_candidate,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct CandidateFixture {
    _root: TempDir,
    staging_path: std::path::PathBuf,
    staging: BackupStaging,
    control_cancelled: Arc<AtomicBool>,
    candidate: Option<BackupCandidate>,
}

impl CandidateFixture {
    fn current() -> TestResult<Self> {
        let root = tempdir()?;
        let archive = root.path().join("tokenmaster.sqlite3");
        drop(UsageStore::open(&archive)?);
        let staging_path = root.path().join("staging");
        fs::create_dir(&staging_path)?;
        let data_root = ValidatedLocalDirectory::new(root.path())?;
        let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
        let source = BackupSource::new(&data_root)?;
        let staging = BackupStaging::new(&staging_root)?;
        let control_cancelled = Arc::new(AtomicBool::new(false));
        let control = BackupControl::new(Arc::clone(&control_cancelled), Duration::from_secs(5))?;
        let candidate = create_online_snapshot(&source, &staging, &control)?;
        Ok(Self {
            _root: root,
            staging_path,
            staging,
            control_cancelled,
            candidate: Some(candidate),
        })
    }

    fn path(&self) -> std::path::PathBuf {
        self.staging_path.join(".tokenmaster-snapshot-00.sqlite3")
    }

    fn take(&mut self) -> BackupCandidate {
        self.candidate
            .take()
            .expect("candidate fixture is single-use")
    }

    fn control(&self) -> TestResult<BackupControl> {
        Ok(BackupControl::new(
            Arc::clone(&self.control_cancelled),
            Duration::from_secs(5),
        )?)
    }

    fn verify(&mut self) -> Result<VerifiedBackupCandidate, tokenmaster_store::StoreError> {
        let control =
            BackupControl::new(Arc::clone(&self.control_cancelled), Duration::from_secs(5))?;
        let candidate = self.take();
        verify_backup_candidate(candidate, &control)
    }
}

#[test]
fn cancellation_and_deadline_remove_every_unpublished_snapshot_file() -> TestResult {
    for cancelled_before_start in [true, false] {
        let root = tempdir()?;
        let archive = root.path().join("tokenmaster.sqlite3");
        let connection = Connection::open(&archive)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE large_payload(value BLOB NOT NULL) STRICT;
             INSERT INTO large_payload VALUES(zeroblob(16777216));",
        )?;
        let staging_path = root.path().join("staging");
        fs::create_dir(&staging_path)?;
        let data_root = ValidatedLocalDirectory::new(root.path())?;
        let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
        let source = BackupSource::new(&data_root)?;
        let staging = BackupStaging::new(&staging_root)?;
        let cancelled = Arc::new(AtomicBool::new(cancelled_before_start));
        let duration = if cancelled_before_start {
            Duration::from_secs(5)
        } else {
            Duration::from_millis(1)
        };
        let control = BackupControl::new(Arc::clone(&cancelled), duration)?;
        let error = create_online_snapshot(&source, &staging, &control)
            .expect_err("copy must stop before publication");
        assert_eq!(
            error.code(),
            if cancelled_before_start {
                StoreErrorCode::Cancelled
            } else {
                StoreErrorCode::DeadlineExceeded
            }
        );
        assert_eq!(fs::read_dir(&staging_path)?.count(), 0);
    }
    Ok(())
}

#[test]
fn busy_source_fails_with_a_bounded_stable_category() -> TestResult {
    let root = tempdir()?;
    let archive = root.path().join("tokenmaster.sqlite3");
    let lock = Connection::open(&archive)?;
    lock.execute_batch(
        "PRAGMA journal_mode=DELETE;
         CREATE TABLE item(value INTEGER NOT NULL) STRICT;
         BEGIN EXCLUSIVE;
         INSERT INTO item VALUES(1);",
    )?;
    let staging_path = root.path().join("staging");
    fs::create_dir(&staging_path)?;
    let data_root = ValidatedLocalDirectory::new(root.path())?;
    let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
    let source = BackupSource::new(&data_root)?;
    let staging = BackupStaging::new(&staging_root)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(2))?;
    let started = Instant::now();
    let error = create_online_snapshot(&source, &staging, &control)
        .expect_err("exclusive source lock must not spin forever");
    assert_eq!(error.code(), StoreErrorCode::Busy);
    assert_eq!(error.limit(), Some(8));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(fs::read_dir(&staging_path)?.count(), 0);
    Ok(())
}

#[test]
fn non_transient_source_failure_is_immediate_and_never_retried() -> TestResult {
    let root = tempdir()?;
    let archive = root.path().join("tokenmaster.sqlite3");
    fs::write(&archive, b"not-a-sqlite-database")?;
    let staging_path = root.path().join("staging");
    fs::create_dir(&staging_path)?;
    let data_root = ValidatedLocalDirectory::new(root.path())?;
    let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
    let source = BackupSource::new(&data_root)?;
    let staging = BackupStaging::new(&staging_root)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(2))?;
    let started = Instant::now();
    let error = create_online_snapshot(&source, &staging, &control)
        .expect_err("invalid source header must fail immediately");
    assert_eq!(error.code(), StoreErrorCode::BackupHeaderCorrupt);
    assert_eq!(error.limit(), None);
    assert!(started.elapsed() < Duration::from_millis(250));
    assert_eq!(fs::read_dir(&staging_path)?.count(), 0);
    Ok(())
}

#[test]
fn header_page_and_index_corruption_have_distinct_stable_categories() -> TestResult {
    let mut header = CandidateFixture::current()?;
    OpenOptions::new()
        .write(true)
        .open(header.path())?
        .write_all(b"NotSQLiteFormat!")?;
    assert_eq!(
        header.verify().expect_err("header corruption").code(),
        StoreErrorCode::BackupHeaderCorrupt
    );

    let mut page = CandidateFixture::current()?;
    let connection = Connection::open(page.path())?;
    let page_size =
        u64::try_from(
            connection.query_row::<i64, _, _>("PRAGMA page_size", [], |row| row.get(0))?,
        )?;
    let root_page = u64::try_from(connection.query_row::<i64, _, _>(
        "SELECT rootpage FROM sqlite_schema WHERE type='table' AND name='benefit_reminder_profile'",
        [],
        |row| row.get(0),
    )?)?;
    drop(connection);
    let mut file = OpenOptions::new().write(true).open(page.path())?;
    file.seek(SeekFrom::Start((root_page - 1) * page_size))?;
    file.write_all(&[0xA5])?;
    assert_eq!(
        page.verify().expect_err("page corruption").code(),
        StoreErrorCode::BackupPageCorrupt
    );

    let mut index = CandidateFixture::current()?;
    let connection = Connection::open(index.path())?;
    let index_name: String = connection.query_row(
        "SELECT name FROM sqlite_schema
         WHERE type='index' AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    connection.execute_batch(&format!("DROP INDEX \"{index_name}\""))?;
    drop(connection);
    assert_eq!(
        index.verify().expect_err("index corruption").code(),
        StoreErrorCode::BackupIndexCorrupt
    );
    Ok(())
}

#[test]
fn foreign_key_schema_count_generation_and_semantic_failures_are_independent() -> TestResult {
    let mut foreign_key = CandidateFixture::current()?;
    let connection = Connection::open(foreign_key.path())?;
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    connection.execute(
        "DELETE FROM benefit_reminder_profile
         WHERE profile_kind='global' AND length(profile_scope_id)=0",
        [],
    )?;
    drop(connection);
    assert_eq!(
        foreign_key
            .verify()
            .expect_err("foreign-key corruption")
            .code(),
        StoreErrorCode::BackupForeignKeyCorrupt
    );

    let mut schema = CandidateFixture::current()?;
    Connection::open(schema.path())?
        .execute_batch("CREATE TABLE injected(value INTEGER) STRICT;")?;
    assert_eq!(
        schema.verify().expect_err("schema corruption").code(),
        StoreErrorCode::SchemaMismatch
    );

    let mut count = CandidateFixture::current()?;
    Connection::open(count.path())?.execute(
        "UPDATE benefit_state SET current_lot_count=current_lot_count+1 WHERE singleton_id=1",
        [],
    )?;
    assert_eq!(
        count.verify().expect_err("count corruption").code(),
        StoreErrorCode::BackupCountCorrupt
    );

    let mut generation = CandidateFixture::current()?;
    Connection::open(generation.path())?.execute(
        "UPDATE usage_aggregate_state
         SET expected_dataset_generation=expected_dataset_generation+1 WHERE singleton_id=1",
        [],
    )?;
    assert_eq!(
        generation
            .verify()
            .expect_err("generation corruption")
            .code(),
        StoreErrorCode::BackupGenerationCorrupt
    );

    let mut semantic = CandidateFixture::current()?;
    let connection = Connection::open(semantic.path())?;
    connection.pragma_update(None, "ignore_check_constraints", "ON")?;
    connection.execute(
        "UPDATE git_installation_state SET installation_salt=x'01' WHERE singleton_id=1",
        [],
    )?;
    drop(connection);
    assert_eq!(
        semantic.verify().expect_err("semantic corruption").code(),
        StoreErrorCode::BackupSemanticCorrupt
    );
    Ok(())
}

#[test]
fn cancelled_compaction_is_not_accepted_and_leaves_no_compact_output() -> TestResult {
    let mut fixture = CandidateFixture::current()?;
    let snapshot = fixture.verify()?;
    fixture
        .control_cancelled
        .store(true, std::sync::atomic::Ordering::Release);
    let error = create_compact_snapshot(&snapshot, &fixture.staging, &fixture.control()?)
        .expect_err("cancelled compact snapshot");
    assert_eq!(error.code(), StoreErrorCode::Cancelled);
    assert!(
        !fixture
            .staging_path
            .join(".tokenmaster-compact-00.sqlite3")
            .exists()
    );
    Ok(())
}

#[test]
fn every_backup_error_is_path_and_sqlite_text_private() -> TestResult {
    let mut fixture = CandidateFixture::current()?;
    let marker = "tm-secret-path-canary";
    OpenOptions::new()
        .write(true)
        .open(fixture.path())?
        .write_all(marker.as_bytes())?;
    let error = fixture.verify().expect_err("corrupt header");
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(!rendered.contains(marker));
        assert!(!rendered.contains("database disk image is malformed"));
        assert!(!rendered.contains("staging"));
        assert!(!rendered.contains("sqlite3"));
    }
    Ok(())
}

#[test]
fn candidate_verification_honors_cancellation_and_discards_the_candidate() -> TestResult {
    let mut fixture = CandidateFixture::current()?;
    let candidate_path = fixture.path();
    fixture
        .control_cancelled
        .store(true, std::sync::atomic::Ordering::Release);
    let error = fixture.verify().expect_err("cancelled verification");
    assert_eq!(error.code(), StoreErrorCode::Cancelled);
    assert!(!candidate_path.exists());
    Ok(())
}

#[test]
fn verified_candidate_rejects_path_replacement_before_compaction() -> TestResult {
    let mut fixture = CandidateFixture::current()?;
    let snapshot = fixture.verify()?;
    let candidate_path = fixture.path();
    let retired_path = fixture.staging_path.join("retired.sqlite3");
    let replacement_path = fixture.staging_path.join("replacement.sqlite3");
    fs::copy(&candidate_path, &replacement_path)?;
    fs::rename(&candidate_path, &retired_path)?;
    fs::rename(&replacement_path, &candidate_path)?;

    let error = create_compact_snapshot(&snapshot, &fixture.staging, &fixture.control()?)
        .expect_err("a verified path must remain bound to the same physical content");
    assert_eq!(error.code(), StoreErrorCode::StaleBackupCandidate);
    assert!(
        !fixture
            .staging_path
            .join(".tokenmaster-compact-00.sqlite3")
            .exists()
    );
    fs::remove_file(retired_path)?;
    Ok(())
}

#[test]
fn cleanup_failure_is_observable_and_abandoned_candidates_are_recoverable() -> TestResult {
    let mut fixture = CandidateFixture::current()?;
    let candidate_path = fixture.path();
    fs::remove_file(&candidate_path)?;
    fs::create_dir(&candidate_path)?;

    let error = fixture
        .take()
        .discard()
        .expect_err("a directory cannot be discarded as a candidate file");
    assert_eq!(error.code(), StoreErrorCode::BackupIo);
    assert!(fixture.staging.cleanup_failure_count() >= 1);
    assert_eq!(
        fixture
            .staging
            .recover_abandoned_candidates()
            .expect_err("recovery must report the undeletable exact child")
            .code(),
        StoreErrorCode::BackupIo
    );

    fs::remove_dir(&candidate_path)?;
    fs::write(&candidate_path, b"abandoned")?;
    assert_eq!(fixture.staging.recover_abandoned_candidates()?, 1);
    assert_eq!(fixture.staging.cleanup_failure_count(), 0);
    assert!(!candidate_path.exists());
    Ok(())
}
