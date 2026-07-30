#![allow(clippy::expect_used)]

mod package_support;

use std::fs;

use tempfile::TempDir;
use tokenmaster_platform::{
    ArchiveRecoveryError, ArchiveRecoveryScope, ExclusiveFileLease, ValidatedLocalDirectory,
};

use package_support::{backup_bytes, config_bytes, read_backup_bytes, read_config_bytes};

const MAIN: &str = "tokenmaster.sqlite3";
const WAL: &str = "tokenmaster.sqlite3-wal";
const SHM: &str = "tokenmaster.sqlite3-shm";

fn assert_every_prefix_and_one_bit_mutation_fails(
    bytes: &[u8],
    mut rejects: impl FnMut(&[u8]) -> bool,
) {
    for length in 0..bytes.len() {
        assert!(rejects(&bytes[..length]), "accepted truncation at {length}");
    }
    for offset in 0..bytes.len() {
        let mut mutated = bytes.to_vec();
        mutated[offset] ^= 1;
        assert!(rejects(&mutated), "accepted one-bit mutation at {offset}");
    }
}

#[test]
fn every_package_prefix_and_one_bit_mutation_fails_closed() {
    let config = config_bytes();
    assert_every_prefix_and_one_bit_mutation_fails(&config, |bytes| {
        read_config_bytes(bytes).is_err()
    });

    let backup = backup_bytes();
    assert_every_prefix_and_one_bit_mutation_fails(&backup, |bytes| {
        read_backup_bytes(bytes).is_err()
    });
}

struct RecoveryFixture {
    _root: TempDir,
    data: ValidatedLocalDirectory,
    reliable: ValidatedLocalDirectory,
}

impl RecoveryFixture {
    fn new(wal: Option<&[u8]>, shm: Option<&[u8]>) -> Self {
        let root = tempfile::tempdir().expect("temporary recovery root");
        let reliable_path = root.path().join("reliable-state");
        fs::create_dir(&reliable_path).expect("reliable-state directory");
        fs::write(root.path().join(MAIN), b"main-before").expect("active main");
        if let Some(bytes) = wal {
            fs::write(root.path().join(WAL), bytes).expect("active WAL");
        }
        if let Some(bytes) = shm {
            fs::write(root.path().join(SHM), bytes).expect("active SHM");
        }
        Self {
            data: ValidatedLocalDirectory::new(root.path()).expect("validated data root"),
            reliable: ValidatedLocalDirectory::new(&reliable_path)
                .expect("validated reliable-state root"),
            _root: root,
        }
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.data.as_path().join(name)
    }

    fn scope(&self) -> ArchiveRecoveryScope {
        ArchiveRecoveryScope::new(&self.data, &self.reliable).expect("recovery scope")
    }

    fn guard(&self) -> tokenmaster_platform::ExclusiveFileLeaseGuard {
        ExclusiveFileLease::for_archive(&self.path(MAIN))
            .expect("archive lease")
            .try_acquire()
            .expect("held archive lease")
    }
}

fn read_optional(path: &std::path::Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("read archive fixture: {error}"),
    }
}

fn operation_directory(
    fixture: &RecoveryFixture,
    operation: &tokenmaster_platform::RecoveryOperation,
) -> std::path::PathBuf {
    let operation_hex = operation
        .id()
        .to_persisted_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    fixture
        .reliable
        .as_path()
        .join("quarantine")
        .join(format!("op-{operation_hex}"))
}

#[derive(Clone, Copy)]
enum SidecarMutation {
    RemoveWal,
    AddWal,
    ReplaceWal,
    RemoveShm,
    AddShm,
    ReplaceShm,
}

#[test]
fn preexisting_wal_and_shm_drift_fails_before_any_archive_move() {
    for mutation in [
        SidecarMutation::RemoveWal,
        SidecarMutation::AddWal,
        SidecarMutation::ReplaceWal,
        SidecarMutation::RemoveShm,
        SidecarMutation::AddShm,
        SidecarMutation::ReplaceShm,
    ] {
        let wal = match mutation {
            SidecarMutation::AddWal => None,
            _ => Some(b"wal-before".as_slice()),
        };
        let shm = match mutation {
            SidecarMutation::AddShm => None,
            _ => Some(b"shm-before".as_slice()),
        };
        let fixture = RecoveryFixture::new(wal, shm);
        let scope = fixture.scope();
        let guard = fixture.guard();
        let before = scope
            .observe(&guard)
            .expect("archive observation")
            .expectation();
        let operation = scope.reserve_operation(&guard).expect("recovery operation");

        match mutation {
            SidecarMutation::RemoveWal => fs::remove_file(fixture.path(WAL)).expect("remove WAL"),
            SidecarMutation::AddWal => fs::write(fixture.path(WAL), b"wal-added").expect("add WAL"),
            SidecarMutation::ReplaceWal => {
                fs::write(fixture.path(WAL), b"wal-changed").expect("replace WAL")
            }
            SidecarMutation::RemoveShm => fs::remove_file(fixture.path(SHM)).expect("remove SHM"),
            SidecarMutation::AddShm => fs::write(fixture.path(SHM), b"shm-added").expect("add SHM"),
            SidecarMutation::ReplaceShm => {
                fs::write(fixture.path(SHM), b"shm-changed").expect("replace SHM")
            }
        }
        let expected_main = read_optional(&fixture.path(MAIN));
        let expected_wal = read_optional(&fixture.path(WAL));
        let expected_shm = read_optional(&fixture.path(SHM));

        assert_eq!(
            scope
                .quarantine_sidecars(&operation, &guard, before)
                .expect_err("sidecar drift must fail closed"),
            ArchiveRecoveryError::ArtifactMismatch
        );
        assert_eq!(read_optional(&fixture.path(MAIN)), expected_main);
        assert_eq!(read_optional(&fixture.path(WAL)), expected_wal);
        assert_eq!(read_optional(&fixture.path(SHM)), expected_shm);
    }
}

#[test]
fn prepared_resume_completes_an_exact_partially_moved_sidecar_set() {
    let fixture = RecoveryFixture::new(Some(b"wal-before"), Some(b"shm-before"));
    let scope = fixture.scope();
    let guard = fixture.guard();
    let before = scope
        .observe(&guard)
        .expect("archive observation")
        .expectation();
    let operation = scope.reserve_operation(&guard).expect("recovery operation");
    scope
        .quarantine_sidecars(&operation, &guard, before)
        .expect("initial sidecar quarantine");
    let set = operation_directory(&fixture, &operation);
    fs::rename(set.join(SHM), fixture.path(SHM)).expect("simulate death between moves");

    let resumed = scope
        .resume_operation(operation.id().to_persisted_bytes(), &guard)
        .expect("resume exact operation");
    scope
        .quarantine_sidecars(&resumed, &guard, before)
        .expect("complete partial sidecar quarantine");

    assert_eq!(read_optional(&fixture.path(WAL)), None);
    assert_eq!(read_optional(&fixture.path(SHM)), None);
    assert_eq!(read_optional(&set.join(WAL)), Some(b"wal-before".to_vec()));
    assert_eq!(read_optional(&set.join(SHM)), Some(b"shm-before".to_vec()));
}

#[test]
fn conflicting_resumed_sidecar_target_fails_before_any_active_move() {
    let fixture = RecoveryFixture::new(Some(b"wal-before"), Some(b"shm-before"));
    let scope = fixture.scope();
    let guard = fixture.guard();
    let before = scope
        .observe(&guard)
        .expect("archive observation")
        .expectation();
    let operation = scope.reserve_operation(&guard).expect("recovery operation");
    let set = operation_directory(&fixture, &operation);
    fs::create_dir(&set).expect("resumed operation directory");
    fs::write(set.join(SHM), b"conflicting-shm").expect("conflicting quarantine target");
    let resumed = scope
        .resume_operation(operation.id().to_persisted_bytes(), &guard)
        .expect("resume exact operation");

    assert_eq!(
        scope
            .quarantine_sidecars(&resumed, &guard, before)
            .expect_err("target conflict must fail closed"),
        ArchiveRecoveryError::ArtifactMismatch
    );
    assert_eq!(
        read_optional(&fixture.path(WAL)),
        Some(b"wal-before".to_vec())
    );
    assert_eq!(
        read_optional(&fixture.path(SHM)),
        Some(b"shm-before".to_vec())
    );
    assert_eq!(read_optional(&set.join(WAL)), None);
    assert_eq!(
        read_optional(&set.join(SHM)),
        Some(b"conflicting-shm".to_vec())
    );
}
