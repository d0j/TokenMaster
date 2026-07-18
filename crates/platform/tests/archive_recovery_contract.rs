use std::fs;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{
    ArchiveRecoveryError, ArchiveRecoveryScope, ExclusiveFileLease, MAX_QUARANTINE_SETS,
    MAX_RECOVERY_STAGING_ARTIFACTS, RecoveryMainMode, ValidatedLocalDirectory,
};

const MAIN: &str = "tokenmaster.sqlite3";
const WAL: &str = "tokenmaster.sqlite3-wal";
const SHM: &str = "tokenmaster.sqlite3-shm";

struct Fixture {
    _root: TempDir,
    data: ValidatedLocalDirectory,
    reliable: ValidatedLocalDirectory,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary data root");
        let reliable_path = root.path().join("reliable-state");
        fs::create_dir(&reliable_path).expect("reliable-state directory");
        Self {
            data: ValidatedLocalDirectory::new(root.path()).expect("validated data root"),
            reliable: ValidatedLocalDirectory::new(&reliable_path)
                .expect("validated reliable-state root"),
            _root: root,
        }
    }

    fn archive(&self, name: &str) -> std::path::PathBuf {
        self.data.as_path().join(name)
    }

    fn lease(&self) -> tokenmaster_platform::ExclusiveFileLeaseGuard {
        ExclusiveFileLease::for_archive(&self.archive(MAIN))
            .expect("archive lease")
            .try_acquire()
            .expect("held archive lease")
    }
}

fn write(path: &std::path::Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("synthetic file");
}

#[test]
fn recovery_scope_is_fixed_redacted_and_creates_only_controlled_children() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");

    assert!(fixture.reliable.as_path().join("staging").is_dir());
    assert!(fixture.reliable.as_path().join("quarantine").is_dir());
    assert_eq!(format!("{scope:?}"), "ArchiveRecoveryScope([redacted])");
    assert!(!format!("{scope:?}").contains(fixture.data.as_path().to_string_lossy().as_ref()));
}

#[test]
fn recovery_preflights_actual_available_staging_capacity() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    scope
        .require_available_staging_bytes(&guard, 1)
        .expect("one available byte");
    assert_eq!(
        scope
            .require_available_staging_bytes(&guard, u64::MAX)
            .expect_err("impossible capacity request"),
        ArchiveRecoveryError::DiskCapacity
    );
}

#[test]
fn a_lease_for_another_archive_cannot_observe_or_mutate_this_scope() {
    let fixture = Fixture::new();
    write(&fixture.archive(MAIN), b"active");
    write(&fixture.archive("other.sqlite3"), b"other");
    let wrong = ExclusiveFileLease::for_archive(&fixture.archive("other.sqlite3"))
        .expect("other lease")
        .try_acquire()
        .expect("held other lease");
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");

    assert_eq!(
        scope.observe(&wrong).expect_err("wrong guard must fail"),
        ArchiveRecoveryError::WrongLease
    );
    assert_eq!(
        scope
            .reserve_operation(&wrong)
            .expect_err("wrong guard must not reserve"),
        ArchiveRecoveryError::WrongLease
    );
}

#[test]
fn replacing_the_locked_sidecar_invalidates_the_archive_authority() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let sidecar = fixture
        .data
        .as_path()
        .join("tokenmaster.sqlite3.tokenmaster-writer.lock");
    let displaced = fixture.data.as_path().join("displaced-writer.lock");
    fs::rename(&sidecar, &displaced).expect("displace locked namespace entry");
    write(&sidecar, b"");

    assert_eq!(
        scope
            .observe(&guard)
            .expect_err("a different physical sidecar cannot authorize recovery"),
        ArchiveRecoveryError::WrongLease
    );
}

#[test]
fn existing_main_and_sidecars_move_as_one_reversible_quarantine_set() {
    let fixture = Fixture::new();
    let old_main = b"old-main";
    let old_wal = b"old-wal";
    let old_shm = b"old-shm";
    let new_main = b"new-main";
    write(&fixture.archive(MAIN), old_main);
    write(&fixture.archive(WAL), old_wal);
    write(&fixture.archive(SHM), old_shm);

    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let before = scope.observe(&guard).expect("active observation");
    let operation = scope.reserve_operation(&guard).expect("operation");
    scope
        .quarantine_sidecars(&operation, &guard, before.expectation())
        .expect("sidecar quarantine");
    assert!(!fixture.archive(WAL).exists());
    assert!(!fixture.archive(SHM).exists());

    let mut candidate = scope
        .create_candidate(&operation, new_main.len() as u64)
        .expect("candidate stage");
    candidate.write_chunk(new_main).expect("candidate bytes");
    let digest: [u8; 32] = Sha256::digest(new_main).into();
    let receipt = candidate
        .seal(new_main.len() as u64, digest)
        .expect("sealed candidate");
    scope
        .promote_main(
            &operation,
            &guard,
            &mut candidate,
            receipt.len(),
            *receipt.sha256(),
            before.expectation(),
            RecoveryMainMode::ReplaceExisting,
        )
        .expect("main promotion");

    assert_eq!(
        fs::read(fixture.archive(MAIN)).expect("new active"),
        new_main
    );
    let quarantine = fixture.reliable.as_path().join("quarantine");
    let sets = fs::read_dir(&quarantine)
        .expect("quarantine scan")
        .collect::<Result<Vec<_>, _>>()
        .expect("quarantine entries");
    assert_eq!(sets.len(), 1);
    let set = sets[0].path();
    assert_eq!(fs::read(set.join(MAIN)).expect("old main"), old_main);
    assert_eq!(fs::read(set.join(WAL)).expect("old wal"), old_wal);
    assert_eq!(fs::read(set.join(SHM)).expect("old shm"), old_shm);

    scope
        .rollback(&operation, &guard, before.expectation())
        .expect("rollback");
    assert_eq!(
        fs::read(fixture.archive(MAIN)).expect("rolled main"),
        old_main
    );
    assert_eq!(fs::read(fixture.archive(WAL)).expect("rolled wal"), old_wal);
    assert_eq!(fs::read(fixture.archive(SHM)).expect("rolled shm"), old_shm);
    assert_eq!(
        fs::read(set.join("failed-main.sqlite3")).expect("failed new main"),
        new_main
    );
}

#[test]
fn rollback_resumes_after_main_and_one_sidecar_were_already_restored() {
    let fixture = Fixture::new();
    let old_main = b"old-main";
    let old_wal = b"old-wal";
    let old_shm = b"old-shm";
    let new_main = b"new-main";
    write(&fixture.archive(MAIN), old_main);
    write(&fixture.archive(WAL), old_wal);
    write(&fixture.archive(SHM), old_shm);
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let before = scope.observe(&guard).expect("before");
    let operation = scope.reserve_operation(&guard).expect("operation");
    scope
        .quarantine_sidecars(&operation, &guard, before.expectation())
        .expect("sidecars quarantined");
    let mut candidate = scope
        .create_candidate(&operation, new_main.len() as u64)
        .expect("candidate");
    candidate.write_chunk(new_main).expect("candidate bytes");
    let digest: [u8; 32] = Sha256::digest(new_main).into();
    let receipt = candidate
        .seal(new_main.len() as u64, digest)
        .expect("candidate seal");
    scope
        .promote_main(
            &operation,
            &guard,
            &mut candidate,
            receipt.len(),
            *receipt.sha256(),
            before.expectation(),
            RecoveryMainMode::ReplaceExisting,
        )
        .expect("main promoted");
    let operation_hex = operation
        .id()
        .to_persisted_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let set = fixture
        .reliable
        .as_path()
        .join("quarantine")
        .join(format!("op-{operation_hex}"));
    fs::rename(fixture.archive(MAIN), set.join("failed-main.sqlite3"))
        .expect("published failed-main boundary");
    fs::rename(set.join(MAIN), fixture.archive(MAIN)).expect("published old-main boundary");
    fs::rename(set.join(WAL), fixture.archive(WAL)).expect("published WAL boundary");

    scope
        .rollback(&operation, &guard, before.expectation())
        .expect("idempotent rollback continuation");
    assert_eq!(fs::read(fixture.archive(MAIN)).expect("old main"), old_main);
    assert_eq!(fs::read(fixture.archive(WAL)).expect("old wal"), old_wal);
    assert_eq!(fs::read(fixture.archive(SHM)).expect("old shm"), old_shm);
    assert_eq!(
        fs::read(set.join("failed-main.sqlite3")).expect("failed candidate retained"),
        new_main
    );
}

#[test]
fn missing_damaged_main_uses_promotion_but_replace_mode_fails_closed() {
    let fixture = Fixture::new();
    write(&fixture.archive(WAL), b"prior-wal");
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let before = scope.observe(&guard).expect("observation");
    assert!(before.main().is_none());
    assert!(before.has_any_archive_artifact());
    let operation = scope.reserve_operation(&guard).expect("operation");
    scope
        .quarantine_sidecars(&operation, &guard, before.expectation())
        .expect("quarantine");

    let bytes = b"replacement";
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    let mut wrong_mode = scope
        .create_candidate(&operation, bytes.len() as u64)
        .expect("stage");
    wrong_mode.write_chunk(bytes).expect("bytes");
    let receipt = wrong_mode.seal(bytes.len() as u64, digest).expect("seal");
    assert_eq!(
        scope
            .promote_main(
                &operation,
                &guard,
                &mut wrong_mode,
                receipt.len(),
                *receipt.sha256(),
                before.expectation(),
                RecoveryMainMode::ReplaceExisting,
            )
            .expect_err("missing main cannot be replaced"),
        ArchiveRecoveryError::ArtifactMismatch
    );

    scope
        .promote_main(
            &operation,
            &guard,
            &mut wrong_mode,
            receipt.len(),
            *receipt.sha256(),
            before.expectation(),
            RecoveryMainMode::PromoteMissing,
        )
        .expect("missing-main promotion");
    assert_eq!(fs::read(fixture.archive(MAIN)).expect("active"), bytes);
}

#[test]
fn operation_identity_round_trips_without_accepting_a_name() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let operation = scope.reserve_operation(&guard).expect("operation");
    let persisted = operation.id().to_persisted_bytes();

    let resumed = scope
        .resume_operation(persisted, &guard)
        .expect("resumed operation");
    assert_eq!(resumed.id(), operation.id());
    assert_eq!(
        format!("{:?}", resumed.id()),
        "RecoveryOperationId([redacted])"
    );
}

#[test]
fn quarantine_capacity_is_three_and_a_fourth_reservation_changes_nothing() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let before = scope
        .observe(&guard)
        .expect("empty observation")
        .expectation();
    for _ in 0..MAX_QUARANTINE_SETS {
        let operation = scope.reserve_operation(&guard).expect("bounded operation");
        scope
            .quarantine_sidecars(&operation, &guard, before)
            .expect("reserve quarantine set");
    }
    let quarantine = fixture.reliable.as_path().join("quarantine");
    let before = fs::read_dir(&quarantine).expect("scan").count();
    assert_eq!(before, MAX_QUARANTINE_SETS);
    assert_eq!(
        scope
            .reserve_operation(&guard)
            .expect_err("fourth set must fail"),
        ArchiveRecoveryError::CapacityExceeded
    );
    assert_eq!(fs::read_dir(quarantine).expect("rescan").count(), before);
}

#[test]
fn unexpected_quarantine_entry_fails_closed_and_is_not_deleted() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let unexpected = fixture
        .reliable
        .as_path()
        .join("quarantine")
        .join("operator.txt");
    write(&unexpected, b"preserve");

    assert_eq!(
        scope
            .reserve_operation(&guard)
            .expect_err("unexpected entry must fail"),
        ArchiveRecoveryError::UnexpectedArtifact
    );
    assert_eq!(
        fs::read(unexpected).expect("evidence retained"),
        b"preserve"
    );
}

#[test]
fn journal_absence_allows_only_bounded_exact_abandoned_candidate_cleanup() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let operation = scope.reserve_operation(&guard).expect("operation");
    let bytes = b"sealed-but-unjournaled";
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    let mut candidate = scope
        .create_candidate(&operation, bytes.len() as u64)
        .expect("candidate");
    candidate.write_chunk(bytes).expect("candidate bytes");
    candidate
        .seal(bytes.len() as u64, digest)
        .expect("sealed candidate");
    drop(candidate);

    assert_eq!(
        scope
            .discard_abandoned_staging(&guard)
            .expect("abandoned cleanup"),
        2
    );
    assert_eq!(
        fs::read_dir(fixture.reliable.as_path().join("staging"))
            .expect("clean staging")
            .count(),
        0
    );

    let unexpected = fixture.reliable.as_path().join("staging").join("keep.bin");
    write(&unexpected, b"unrecognized evidence");
    assert_eq!(
        scope
            .discard_abandoned_staging(&guard)
            .expect_err("unexpected staging artifact must fail closed"),
        ArchiveRecoveryError::UnexpectedArtifact
    );
    assert_eq!(
        fs::read(unexpected).expect("unexpected evidence retained"),
        b"unrecognized evidence"
    );
}

#[test]
fn recovery_staging_has_one_global_fixed_artifact_cap() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let operation = scope.reserve_operation(&guard).expect("bounded operation");
    let candidate = scope
        .create_candidate(&operation, 0)
        .expect("bounded candidate");
    let second_reservation = scope
        .reserve_operation(&guard)
        .expect("final bounded reservation");
    assert_eq!(
        scope
            .reserve_operation(&guard)
            .expect_err("staging cap must fail closed"),
        ArchiveRecoveryError::CapacityExceeded
    );
    assert_eq!(
        fs::read_dir(fixture.reliable.as_path().join("staging"))
            .expect("bounded staging")
            .count(),
        MAX_RECOVERY_STAGING_ARTIFACTS
    );
    drop((candidate, second_reservation));
}

#[test]
fn post_reservation_operation_directory_collision_fails_before_sidecars_move() {
    let fixture = Fixture::new();
    let scope = ArchiveRecoveryScope::new(&fixture.data, &fixture.reliable).expect("scope");
    let guard = fixture.lease();
    let main = fixture.data.as_path().join("tokenmaster.sqlite3");
    let wal = fixture.data.as_path().join("tokenmaster.sqlite3-wal");
    let shm = fixture.data.as_path().join("tokenmaster.sqlite3-shm");
    write(&main, b"old-main");
    write(&wal, b"old-wal");
    write(&shm, b"old-shm");
    let before = scope.observe(&guard).expect("before").expectation();
    let operation = scope.reserve_operation(&guard).expect("reservation");
    let operation_hex = operation
        .id()
        .to_persisted_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    fs::create_dir(
        fixture
            .reliable
            .as_path()
            .join("quarantine")
            .join(format!("op-{operation_hex}")),
    )
    .expect("colliding operation directory");

    assert_eq!(
        scope
            .quarantine_sidecars(&operation, &guard, before)
            .expect_err("reserved name collision must fail closed"),
        ArchiveRecoveryError::UnexpectedArtifact
    );
    assert_eq!(fs::read(main).expect("main retained"), b"old-main");
    assert_eq!(fs::read(wal).expect("wal retained"), b"old-wal");
    assert_eq!(fs::read(shm).expect("shm retained"), b"old-shm");
}
