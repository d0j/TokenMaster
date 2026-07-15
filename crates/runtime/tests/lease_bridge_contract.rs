use std::path::Path;

use tempfile::TempDir;
use tokenmaster_engine::{PortErrorCode, WriterLease};
use tokenmaster_platform::WRITER_LEASE_SUFFIX;
use tokenmaster_runtime::{RuntimeErrorCode, RuntimeWriterLease};

fn fixture() -> (TempDir, std::path::PathBuf) {
    let root = TempDir::new().expect("temporary lease root");
    let archive = root.path().join("usage.sqlite3");
    std::fs::write(&archive, []).expect("create archive fixture");
    (root, archive)
}

#[test]
fn runtime_bridge_maps_only_contention_to_busy_and_releases_on_drop() {
    let (_root, archive) = fixture();
    let mut first = RuntimeWriterLease::new(&archive).expect("first runtime lease");
    let mut second = RuntimeWriterLease::new(&archive).expect("second runtime lease");

    let guard = first.try_acquire().expect("first acquisition");
    let error = match second.try_acquire() {
        Ok(_) => panic!("second acquisition must be busy"),
        Err(error) => error,
    };
    assert_eq!(error.code(), PortErrorCode::Busy);
    drop(guard);
    drop(second.try_acquire().expect("reacquire after guard drop"));
}

#[test]
fn runtime_bridge_keeps_paths_and_platform_errors_private() {
    let (_root, archive) = fixture();
    let lease = RuntimeWriterLease::new(&archive).expect("runtime lease");
    assert_eq!(format!("{lease:?}"), "RuntimeWriterLease([redacted])");

    let mut sidecar_name = archive.file_name().expect("archive name").to_os_string();
    sidecar_name.push(WRITER_LEASE_SUFFIX);
    std::fs::write(archive.with_file_name(sidecar_name), b"private").expect("tamper sidecar");
    let mut tampered = RuntimeWriterLease::new(&archive).expect("tampered runtime lease");
    let error = match tampered.try_acquire() {
        Ok(_) => panic!("tampered sidecar must fail closed"),
        Err(error) => error,
    };
    assert_eq!(error.code(), PortErrorCode::InvalidData);
    assert!(!format!("{error:?}").contains(archive.to_string_lossy().as_ref()));

    let invalid = RuntimeWriterLease::new(Path::new("relative.sqlite3"))
        .expect_err("relative path must be invalid configuration");
    assert_eq!(invalid.code(), RuntimeErrorCode::InvalidConfiguration);
}
