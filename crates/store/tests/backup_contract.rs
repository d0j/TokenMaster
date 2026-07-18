use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tempfile::tempdir;
use tokenmaster_platform::ValidatedLocalDirectory;
use tokenmaster_store::{
    ArchiveVersionStatus, BackupControl, BackupSource, BackupStaging, UsageStore,
    create_compact_snapshot, create_online_snapshot, inspect_archive_version,
    verify_backup_candidate,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn online_snapshot_includes_committed_wal_state_that_main_file_copy_misses() -> TestResult {
    let root = tempdir()?;
    let archive = root.path().join("tokenmaster.sqlite3");
    drop(UsageStore::open(&archive)?);

    let writer = Connection::open(&archive)?;
    writer.pragma_update(None, "wal_autocheckpoint", 0_i64)?;
    let before: i64 = writer.query_row(
        "SELECT channel_os_scheduled FROM benefit_reminder_profile
         WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
        [],
        |row| row.get(0),
    )?;
    writer.execute(
        "UPDATE benefit_reminder_profile SET channel_os_scheduled = ?1
         WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
        [1_i64 - before],
    )?;
    assert!(archive.with_extension("sqlite3-wal").is_file());

    let copied_main = root.path().join("copied-main.sqlite3");
    fs::copy(&archive, &copied_main)?;
    let copied_value: i64 = Connection::open(&copied_main)?.query_row(
        "SELECT channel_os_scheduled FROM benefit_reminder_profile
         WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(copied_value, before);

    let staging_path = root.path().join("staging");
    fs::create_dir(&staging_path)?;
    let data_root = ValidatedLocalDirectory::new(root.path())?;
    let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
    let source = BackupSource::new(&data_root)?;
    let staging = BackupStaging::new(&staging_root)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(5))?;
    let _candidate = create_online_snapshot(&source, &staging, &control)?;

    let snapshot = staging_path.join(".tokenmaster-snapshot-00.sqlite3");
    let snapshot_value: i64 = Connection::open(snapshot)?.query_row(
        "SELECT channel_os_scheduled FROM benefit_reminder_profile
         WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(snapshot_value, 1_i64 - before);
    Ok(())
}

#[test]
fn online_snapshot_is_reopenable_and_verified_during_bounded_concurrent_writes() -> TestResult {
    let root = tempdir()?;
    let archive = root.path().join("tokenmaster.sqlite3");
    drop(UsageStore::open(&archive)?);
    let filler = Connection::open(&archive)?;
    filler.execute_batch(
        "CREATE TABLE backup_step_filler(value BLOB NOT NULL) STRICT;
         INSERT INTO backup_step_filler VALUES(zeroblob(16777216));
         DROP TABLE backup_step_filler;
         PRAGMA wal_checkpoint(TRUNCATE);",
    )?;
    drop(filler);
    let staging_path = root.path().join("staging");
    fs::create_dir(&staging_path)?;

    let data_root = ValidatedLocalDirectory::new(root.path())?;
    let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
    let source = BackupSource::new(&data_root)?;
    let staging = BackupStaging::new(&staging_root)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(5))?;
    let backup_done = Arc::new(AtomicBool::new(false));
    let worker_done = Arc::clone(&backup_done);
    let worker_source = source.clone();
    let worker_staging = staging.clone();
    let worker_control = control.clone();
    let backup = thread::spawn(move || {
        let result = create_online_snapshot(&worker_source, &worker_staging, &worker_control);
        worker_done.store(true, Ordering::Release);
        result
    });

    let snapshot_path = staging_path.join(".tokenmaster-snapshot-00.sqlite3");
    let wait_deadline = Instant::now() + Duration::from_secs(2);
    let mut observed_active_step = false;
    while Instant::now() < wait_deadline {
        if fs::metadata(&snapshot_path).is_ok_and(|metadata| metadata.len() > 0)
            && !backup_done.load(Ordering::Acquire)
        {
            observed_active_step = true;
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        observed_active_step,
        "destination growth must be observed while page stepping is active"
    );
    Connection::open(&archive)?.execute(
        "UPDATE benefit_reminder_profile SET channel_os_scheduled = 1-channel_os_scheduled
         WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
        [],
    )?;
    let candidate = backup
        .join()
        .map_err(|_| std::io::Error::other("backup panicked"))??;

    let verified = verify_backup_candidate(candidate, &control)?;
    assert_eq!(verified.schema_version(), 13);
    assert!(!verified.is_empty());
    assert!(verified.integrity_verified());
    assert!(verified.foreign_keys_verified());
    assert!(verified.schema_verified());
    assert!(verified.semantics_verified());
    assert_eq!(
        Connection::open(snapshot_path)?.query_row::<i64, _, _>(
            "PRAGMA user_version",
            [],
            |row| row.get(0)
        )?,
        13
    );
    Ok(())
}

#[test]
fn candidate_verifier_applies_the_complete_defensive_policy() -> TestResult {
    let root = tempdir()?;
    let archive = root.path().join("tokenmaster.sqlite3");
    drop(UsageStore::open(&archive)?);
    let staging_path = root.path().join("staging");
    fs::create_dir(&staging_path)?;
    let data_root = ValidatedLocalDirectory::new(root.path())?;
    let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
    let source = BackupSource::new(&data_root)?;
    let staging = BackupStaging::new(&staging_root)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(5))?;

    let verified = verify_backup_candidate(
        create_online_snapshot(&source, &staging, &control)?,
        &control,
    )?;
    let policy = verified.runtime_policy();
    assert!(policy.query_only());
    assert!(policy.foreign_keys());
    assert!(!policy.trusted_schema());
    assert!(policy.defensive());
    assert!(policy.no_checkpoint_on_close());
    assert!(policy.query_planner_stability());
    assert!(!policy.double_quoted_dml());
    assert!(!policy.double_quoted_ddl());
    assert!(policy.cell_size_check());
    assert_eq!(policy.mmap_size_bytes(), 0);
    assert_eq!(policy.sqlite_version(), "3.53.2");
    assert_eq!(policy.sqlite_length_limit_bytes(), 16 * 1024 * 1024);
    assert_eq!(policy.sqlite_sql_length_limit_bytes(), 256 * 1024);
    assert_eq!(policy.sqlite_column_limit(), 256);
    Ok(())
}

#[test]
fn archive_version_inspection_is_non_mutating_for_supported_old_and_newer_versions() -> TestResult {
    for (version, expected) in [
        (12_i64, ArchiveVersionStatus::SupportedLegacy),
        (14_i64, ArchiveVersionStatus::Newer),
    ] {
        let root = tempdir()?;
        let archive = root.path().join("tokenmaster.sqlite3");
        drop(UsageStore::open(&archive)?);
        let staging_path = root.path().join("staging");
        fs::create_dir(&staging_path)?;
        let data_root = ValidatedLocalDirectory::new(root.path())?;
        let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
        let source = BackupSource::new(&data_root)?;
        let staging = BackupStaging::new(&staging_root)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(5))?;
        let candidate = create_online_snapshot(&source, &staging, &control)?;
        let candidate_path = staging_path.join(".tokenmaster-snapshot-00.sqlite3");
        Connection::open(&candidate_path)?.pragma_update(None, "user_version", version)?;
        let before = fs::read(&candidate_path)?;

        let inspection = inspect_archive_version(&candidate)?;
        assert_eq!(inspection.version(), version as u32);
        assert_eq!(inspection.status(), expected);
        assert_eq!(fs::read(&candidate_path)?, before);
    }
    Ok(())
}

#[test]
fn compact_snapshot_is_isolated_not_larger_and_reverified() -> TestResult {
    let root = tempdir()?;
    let archive = root.path().join("tokenmaster.sqlite3");
    drop(UsageStore::open(&archive)?);
    let staging_path = root.path().join("staging");
    fs::create_dir(&staging_path)?;
    let data_root = ValidatedLocalDirectory::new(root.path())?;
    let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
    let source = BackupSource::new(&data_root)?;
    let staging = BackupStaging::new(&staging_root)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(5))?;

    let snapshot = verify_backup_candidate(
        create_online_snapshot(&source, &staging, &control)?,
        &control,
    )?;
    let compact = create_compact_snapshot(&snapshot, &staging, &control)?;
    assert!(compact.len() <= snapshot.len());
    assert_eq!(compact.schema_version(), snapshot.schema_version());
    assert!(compact.integrity_verified());
    Ok(())
}

#[test]
fn verified_candidate_reader_streams_exact_identity_and_finishes_once() -> TestResult {
    let root = tempdir()?;
    let archive = root.path().join("tokenmaster.sqlite3");
    drop(UsageStore::open(&archive)?);
    let staging_path = root.path().join("staging");
    fs::create_dir(&staging_path)?;
    let data_root = ValidatedLocalDirectory::new(root.path())?;
    let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
    let source = BackupSource::new(&data_root)?;
    let staging = BackupStaging::new(&staging_root)?;
    let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(5))?;
    let verified = verify_backup_candidate(
        create_online_snapshot(&source, &staging, &control)?,
        &control,
    )?;
    let expected = fs::read(staging_path.join(".tokenmaster-snapshot-00.sqlite3"))?;

    let mut reader = verified.open_reader(&control)?;
    assert_eq!(reader.len(), expected.len() as u64);
    assert_eq!(reader.schema_version(), verified.schema_version());
    let mut observed = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let read = reader.read_chunk(&mut chunk)?;
        if read == 0 {
            break;
        }
        observed.extend_from_slice(&chunk[..read]);
    }
    reader.finish()?;
    assert_eq!(observed, expected);
    Ok(())
}

#[test]
fn verified_candidate_reader_rejects_replacement_truncation_and_append() -> TestResult {
    for mutation in ["replace", "truncate", "append"] {
        let root = tempdir()?;
        let archive = root.path().join("tokenmaster.sqlite3");
        drop(UsageStore::open(&archive)?);
        let staging_path = root.path().join("staging");
        fs::create_dir(&staging_path)?;
        let data_root = ValidatedLocalDirectory::new(root.path())?;
        let staging_root = ValidatedLocalDirectory::new(&staging_path)?;
        let source = BackupSource::new(&data_root)?;
        let staging = BackupStaging::new(&staging_root)?;
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(5))?;
        let verified = verify_backup_candidate(
            create_online_snapshot(&source, &staging, &control)?,
            &control,
        )?;
        let candidate_path = staging_path.join(".tokenmaster-snapshot-00.sqlite3");
        let mut reader = verified.open_reader(&control)?;

        match mutation {
            "replace" => {
                let moved = staging_path.join("moved.sqlite3");
                fs::rename(&candidate_path, &moved)?;
                fs::copy(&moved, &candidate_path)?;
            }
            "truncate" => {
                OpenOptions::new()
                    .write(true)
                    .open(&candidate_path)?
                    .set_len(512)?;
            }
            "append" => {
                OpenOptions::new()
                    .append(true)
                    .open(&candidate_path)?
                    .write_all(&[0_u8; 512])?;
            }
            _ => unreachable!(),
        }

        let mut chunk = [0_u8; 4096];
        let stream_error = loop {
            match reader.read_chunk(&mut chunk) {
                Ok(0) => break None,
                Ok(_) => {}
                Err(error) => break Some(error),
            }
        };
        let error = match stream_error {
            Some(error) => error,
            None => reader.finish().expect_err("changed candidate must fail"),
        };
        assert_eq!(
            error.code(),
            tokenmaster_store::StoreErrorCode::StaleBackupCandidate,
            "mutation={mutation}"
        );
    }
    Ok(())
}
