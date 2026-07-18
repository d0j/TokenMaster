use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use rusqlite::Connection;
use sha2::Digest;
use tempfile::TempDir;
use tokenmaster_platform::{DurableFileTarget, ValidatedLocalDirectory};
use tokenmaster_store::{
    BackupControl, BackupStaging, StoreErrorCode, USAGE_SCHEMA_VERSION, UsageStore,
    verify_recovery_archive,
};

struct Fixture {
    _root: TempDir,
    source: ValidatedLocalDirectory,
    staging: BackupStaging,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary root");
        let source_path = root.path().join("source");
        let staging_path = root.path().join("staging");
        fs::create_dir(&source_path).expect("source directory");
        fs::create_dir(&staging_path).expect("staging directory");
        let source = ValidatedLocalDirectory::new(&source_path).expect("validated source");
        let staging_root = ValidatedLocalDirectory::new(&staging_path).expect("validated staging");
        let staging = BackupStaging::new(&staging_root).expect("backup staging");
        Self {
            _root: root,
            source,
            staging,
        }
    }

    fn target(&self) -> DurableFileTarget {
        DurableFileTarget::exact_child(&self.source, "candidate.sqlite3").expect("source target")
    }

    fn control() -> BackupControl {
        BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("backup control")
    }
}

fn create_current_archive(path: &std::path::Path) {
    drop(UsageStore::open(path).expect("create current archive"));
    let connection = Connection::open(path).expect("checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

#[test]
fn path_free_reader_receives_the_complete_sqlite_verifier_and_bounded_proof() {
    let fixture = Fixture::new();
    let archive = fixture.source.as_path().join("candidate.sqlite3");
    create_current_archive(&archive);
    let expected_len = fs::metadata(&archive).expect("archive metadata").len();
    let reader = fixture
        .target()
        .open_reader(expected_len)
        .expect("reader open")
        .expect("reader present");

    let verified = verify_recovery_archive(reader, &fixture.staging, &Fixture::control())
        .expect("verified recovery archive");

    assert_eq!(verified.schema_version(), USAGE_SCHEMA_VERSION as u32);
    assert_eq!(verified.len(), expected_len);
    assert!(!verified.is_empty());
    assert!(verified.integrity_verified());
    assert!(verified.foreign_keys_verified());
    assert!(verified.schema_verified());
    assert!(verified.semantics_verified());
    assert_ne!(verified.sha256(), &[0_u8; 32]);
    assert_eq!(
        format!("{verified:?}"),
        "VerifiedRecoveryArchive([redacted])"
    );
}

#[test]
fn malformed_input_is_rejected_without_leaving_a_store_candidate() {
    let fixture = Fixture::new();
    let bytes = b"not sqlite";
    let mut stage = fixture
        .target()
        .create_staged(bytes.len() as u64)
        .expect("source stage");
    stage.write_chunk(bytes).expect("source bytes");
    let digest: [u8; 32] = sha2::Sha256::digest(bytes).into();
    stage.seal(bytes.len() as u64, digest).expect("source seal");
    stage
        .publish_new(&fixture.target())
        .expect("source publish");
    let reader = fixture
        .target()
        .open_reader(bytes.len() as u64)
        .expect("reader")
        .expect("present");

    let error = verify_recovery_archive(reader, &fixture.staging, &Fixture::control())
        .expect_err("malformed input");
    assert_eq!(error.code(), StoreErrorCode::BackupHeaderCorrupt);
    assert_eq!(
        fixture
            .staging
            .recover_abandoned_candidates()
            .expect("cleanup scan"),
        0
    );
}

#[test]
fn cancelled_copy_fails_without_retaining_unbounded_state() {
    let fixture = Fixture::new();
    let archive = fixture.source.as_path().join("candidate.sqlite3");
    create_current_archive(&archive);
    let len = fs::metadata(&archive).expect("metadata").len();
    let reader = fixture
        .target()
        .open_reader(len)
        .expect("reader")
        .expect("present");
    let cancelled = Arc::new(AtomicBool::new(true));
    let control = BackupControl::new(cancelled, Duration::from_secs(30)).expect("control");

    let error =
        verify_recovery_archive(reader, &fixture.staging, &control).expect_err("cancelled copy");
    assert_eq!(error.code(), StoreErrorCode::Cancelled);
    assert_eq!(
        fixture
            .staging
            .recover_abandoned_candidates()
            .expect("cleanup scan"),
        0
    );
}

#[test]
fn recovery_verifier_refuses_to_exceed_the_shared_staging_cap() {
    let fixture = Fixture::new();
    let archive = fixture.source.as_path().join("candidate.sqlite3");
    create_current_archive(&archive);
    let len = fs::metadata(&archive).expect("metadata").len();
    let reader = fixture
        .target()
        .open_reader(len)
        .expect("reader")
        .expect("present");
    for index in 0..3 {
        fs::write(
            fixture
                ._root
                .path()
                .join("staging")
                .join(format!("occupied-{index}")),
            b"occupied",
        )
        .expect("occupied staging artifact");
    }

    let error = verify_recovery_archive(reader, &fixture.staging, &Fixture::control())
        .expect_err("the fourth recovery artifact must be rejected");

    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        fs::read_dir(fixture._root.path().join("staging"))
            .expect("staging scan")
            .count(),
        3
    );
}
