use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use tempfile::TempDir;
use tokenmaster_platform::{ExclusiveFileLease, ExclusiveFileLeaseError, WRITER_LEASE_SUFFIX};

struct Holder {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Holder {
    fn spawn(archive: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lease_fixture"))
            .arg(archive)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn lease fixture");
        let stdin = child.stdin.take().expect("fixture stdin");
        let stdout = child.stdout.take().expect("fixture stdout");
        let mut ready = String::new();
        BufReader::new(stdout)
            .read_line(&mut ready)
            .expect("read fixture readiness");
        assert_eq!(ready, "acquired\n");
        Self {
            child,
            stdin: Some(stdin),
        }
    }

    fn exit_normally(mut self) {
        self.stdin
            .take()
            .expect("fixture stdin")
            .write_all(b"exit\n")
            .expect("release fixture");
        assert!(self.child.wait().expect("wait fixture").success());
    }

    fn terminate(mut self) {
        self.child.kill().expect("kill fixture");
        let _ = self.child.wait().expect("wait killed fixture");
    }
}

fn fixture() -> (TempDir, PathBuf) {
    let root = TempDir::new().expect("temporary lease root");
    let archive = root.path().join("usage.sqlite3");
    std::fs::write(&archive, []).expect("create archive fixture");
    (root, archive)
}

fn sidecar(archive: &Path) -> PathBuf {
    let mut name = archive.file_name().expect("archive name").to_os_string();
    name.push(WRITER_LEASE_SUFFIX);
    archive.with_file_name(name)
}

#[test]
fn independent_same_process_handles_contend_and_reacquire() {
    let (_root, archive) = fixture();
    let first = ExclusiveFileLease::for_archive(&archive).expect("first lease");
    let second = ExclusiveFileLease::for_archive(&archive).expect("second lease");

    let guard = first.try_acquire().expect("first acquisition");
    let error = second
        .try_acquire()
        .expect_err("second handle must contend");
    assert_eq!(error, ExclusiveFileLeaseError::Contended);

    drop(guard);
    let reacquired = second.try_acquire().expect("reacquire after drop");
    assert_eq!(
        format!("{reacquired:?}"),
        "ExclusiveFileLeaseGuard([redacted])"
    );
}

#[test]
fn canonical_parent_aliases_resolve_to_one_lock_identity() {
    let (_root, archive) = fixture();
    let alias = archive
        .parent()
        .expect("archive parent")
        .join(".")
        .join(archive.file_name().expect("archive name"));
    let canonical = ExclusiveFileLease::for_archive(&archive).expect("canonical lease");
    let aliased = ExclusiveFileLease::for_archive(&alias).expect("aliased lease");

    let _guard = canonical.try_acquire().expect("canonical acquisition");
    assert_eq!(
        aliased.try_acquire().expect_err("alias must contend"),
        ExclusiveFileLeaseError::Contended
    );
}

#[test]
fn an_existing_guard_authorizes_only_its_exact_archive_lease() {
    let (root, archive) = fixture();
    let other = root.path().join("other.sqlite3");
    std::fs::write(&other, []).expect("create other archive fixture");
    let lease = ExclusiveFileLease::for_archive(&archive).expect("archive lease");
    let alias = ExclusiveFileLease::for_archive(
        &archive
            .parent()
            .expect("archive parent")
            .join(".")
            .join(archive.file_name().expect("archive name")),
    )
    .expect("alias lease");
    let other_lease = ExclusiveFileLease::for_archive(&other).expect("other lease");
    let guard = lease.try_acquire().expect("archive guard");

    alias.authorize_guard(&guard).expect("same canonical lease");
    assert_eq!(
        other_lease
            .authorize_guard(&guard)
            .expect_err("other archive must be rejected"),
        ExclusiveFileLeaseError::InvalidSidecar
    );
}

#[test]
fn child_normal_exit_and_forced_termination_release_the_os_lock() {
    let (_root, archive) = fixture();
    let lease = ExclusiveFileLease::for_archive(&archive).expect("parent lease");

    Holder::spawn(&archive).exit_normally();
    drop(lease.try_acquire().expect("reacquire after normal exit"));

    Holder::spawn(&archive).terminate();
    drop(lease.try_acquire().expect("reacquire after forced exit"));
}

#[test]
fn child_contention_is_non_blocking_and_stable() {
    let (_root, archive) = fixture();
    let holder = Holder::spawn(&archive);
    let lease = ExclusiveFileLease::for_archive(&archive).expect("parent lease");

    assert_eq!(
        lease.try_acquire().expect_err("child owns the lock"),
        ExclusiveFileLeaseError::Contended
    );

    holder.exit_normally();
}

#[test]
fn sidecar_is_persistent_empty_and_debug_is_path_private() {
    let (_root, archive) = fixture();
    let lease = ExclusiveFileLease::for_archive(&archive).expect("lease");
    assert_eq!(format!("{lease:?}"), "ExclusiveFileLease([redacted])");

    drop(lease.try_acquire().expect("acquire"));

    let sidecar = sidecar(&archive);
    assert!(sidecar.is_file());
    assert_eq!(
        std::fs::metadata(&sidecar).expect("sidecar metadata").len(),
        0
    );
    drop(lease.try_acquire().expect("persistent sidecar reacquire"));
    assert!(sidecar.is_file());
}

#[test]
fn payload_and_invalid_paths_fail_closed_without_exposing_them() {
    let (_root, archive) = fixture();
    std::fs::write(sidecar(&archive), b"owner=private").expect("tamper sidecar");
    let lease = ExclusiveFileLease::for_archive(&archive).expect("lease");
    let error = lease
        .try_acquire()
        .expect_err("payload sidecar must fail closed");
    assert_eq!(error, ExclusiveFileLeaseError::InvalidSidecar);
    assert_eq!(format!("{error:?}"), "InvalidSidecar");
    assert!(!format!("{error}").contains(archive.to_string_lossy().as_ref()));

    let relative = ExclusiveFileLease::for_archive(Path::new("usage.sqlite3"))
        .expect_err("relative archive path must fail closed");
    assert_eq!(relative, ExclusiveFileLeaseError::InvalidPath);
}

#[cfg(windows)]
#[test]
fn remote_and_device_namespaces_fail_before_filesystem_io() {
    for archive in [
        Path::new(r"\\server\share\usage.sqlite3"),
        Path::new(r"\\?\UNC\server\share\usage.sqlite3"),
        Path::new(r"\\.\C:\private\usage.sqlite3"),
    ] {
        let error = ExclusiveFileLease::for_archive(archive)
            .expect_err("unsupported namespace must fail closed");
        assert_eq!(error, ExclusiveFileLeaseError::UnsupportedLocation);
    }
}

#[cfg(windows)]
#[test]
fn repeated_acquire_drop_does_not_grow_process_handles() {
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

    fn handle_count() -> u32 {
        let mut count = 0_u32;
        // SAFETY: `count` is valid writable storage and the process pseudo-handle is valid.
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &raw mut count) }
            .expect("query process handle count");
        count
    }

    let (_root, archive) = fixture();
    let lease = ExclusiveFileLease::for_archive(&archive).expect("lease");
    drop(lease.try_acquire().expect("warm sidecar"));
    let before = handle_count();

    for _ in 0..4_096 {
        drop(lease.try_acquire().expect("bounded reacquire"));
    }

    let after = handle_count();
    assert!(
        after <= before.saturating_add(1),
        "writer lease handle count grew: before={before}, after={after}"
    );
}
