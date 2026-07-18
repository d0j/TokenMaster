use std::ffi::OsString;
use std::fs;

use tempfile::TempDir;
use tokenmaster_platform::{ExclusiveFileLease, ExclusiveFileLeaseError};
use tokenmaster_state::BootstrapOutcome;

use crate::state::ApplicationStateOwner;
use crate::{ApplicationEnvironment, DataRoot};

fn fixture() -> (TempDir, DataRoot) {
    let temporary = tempfile::tempdir().expect("temporary application root");
    let executable = temporary.path().join("TokenMaster.exe");
    fs::write(&executable, b"fixture").expect("fixture executable");
    let environment = ApplicationEnvironment::new(
        executable,
        Some(temporary.path().to_path_buf()),
        None,
        None::<OsString>,
    );
    let root = DataRoot::resolve(&environment).expect("data root");
    (temporary, root)
}

#[test]
fn state_owner_creates_only_the_fixed_reliable_state_tree() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let reliable = root.directory().join("reliable-state");

    for child in ["staging", "quarantine", "backups"] {
        let metadata = fs::symlink_metadata(reliable.join(child)).expect("fixed child");
        assert!(metadata.is_dir());
        assert!(!metadata.file_type().is_symlink());
    }
    assert!(!root.archive_path().exists());
    assert!(!format!("{owner:?}").contains(&root.directory().display().to_string()));
}

#[test]
fn first_install_preflight_publishes_unclean_and_holds_the_startup_guard() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let preflight = owner.prepare(&root).expect("state preflight");

    assert_eq!(preflight.report().outcome(), BootstrapOutcome::FirstInstall);
    assert!(!root.archive_path().exists());
    let reliable = root.directory().join("reliable-state");
    let run_records = ["run-a.tms", "run-b.tms"]
        .into_iter()
        .filter(|name| reliable.join(name).exists())
        .count();
    assert_eq!(run_records, 1);

    let competing = ExclusiveFileLease::for_archive(root.archive_path())
        .expect("same archive lease")
        .try_acquire()
        .expect_err("preflight keeps the startup guard");
    assert_eq!(competing, ExclusiveFileLeaseError::Contended);

    drop(preflight);
    ExclusiveFileLease::for_archive(root.archive_path())
        .expect("same archive lease")
        .try_acquire()
        .expect("guard released with preflight");
}
