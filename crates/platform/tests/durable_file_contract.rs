use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{
    DURABLE_STAGE_ATTEMPTS, DurableFileError, DurableFileTarget, MAX_DURABLE_FILE_BYTES,
    MAX_DURABLE_WRITE_CHUNK_BYTES, ValidatedLocalDirectory,
};

const OLD: &[u8] = b"old-complete-payload";
const NEW: &[u8] = b"new-complete-payload";
const TEST_MAX_BYTES: u64 = 1024 * 1024;

fn fixture() -> (TempDir, ValidatedLocalDirectory) {
    let root = TempDir::new().expect("temporary durable-file root");
    let directory = ValidatedLocalDirectory::new(root.path()).expect("validated root");
    (root, directory)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn descriptor(
    directory: &ValidatedLocalDirectory,
    name: &str,
) -> Result<DurableFileTarget, DurableFileError> {
    DurableFileTarget::exact_child(directory, name)
}

fn sealed(target: &DurableFileTarget, payload: &[u8]) -> tokenmaster_platform::DurableStagedFile {
    let mut staged = target.create_staged(TEST_MAX_BYTES).expect("create stage");
    staged.write_chunk(payload).expect("write stage");
    let receipt = staged
        .seal(
            u64::try_from(payload.len()).expect("fixture length"),
            digest(payload),
        )
        .expect("seal stage");
    assert_eq!(receipt.len(), payload.len() as u64);
    assert_eq!(receipt.sha256(), &digest(payload));
    assert_eq!(format!("{receipt:?}"), "DurableFileReceipt([redacted])");
    staged
}

#[test]
fn exact_child_policy_rejects_traversal_separators_reserved_and_unexpected_types() {
    let (root, directory) = fixture();
    for invalid in [
        "",
        ".",
        "..",
        "nested/file",
        "nested\\file",
        "stream:ads",
        "CON",
        "name with space",
    ] {
        assert_eq!(
            descriptor(&directory, invalid).expect_err("invalid child must fail"),
            DurableFileError::InvalidName
        );
    }
    assert_eq!(
        descriptor(&directory, &"a".repeat(97)).expect_err("long child must fail"),
        DurableFileError::InvalidName
    );

    fs::create_dir(root.path().join("directory.slot")).expect("directory child");
    assert_eq!(
        descriptor(&directory, "directory.slot").expect_err("directory must fail"),
        DurableFileError::UnexpectedType
    );
}

#[cfg(unix)]
#[test]
fn linked_child_is_rejected() {
    use std::os::unix::fs::symlink;

    let (root, directory) = fixture();
    fs::write(root.path().join("outside.slot"), OLD).expect("link target");
    symlink(
        root.path().join("outside.slot"),
        root.path().join("linked.slot"),
    )
    .expect("create symlink");
    assert_eq!(
        descriptor(&directory, "linked.slot").expect_err("link must fail"),
        DurableFileError::UnsupportedLocation
    );
}

#[cfg(windows)]
#[test]
fn linked_or_reparse_child_is_rejected() {
    use std::os::windows::fs::symlink_file;

    let (root, directory) = fixture();
    fs::write(root.path().join("outside.slot"), OLD).expect("link target");
    symlink_file(
        root.path().join("outside.slot"),
        root.path().join("linked.slot"),
    )
    .expect("create local symlink");
    assert_eq!(
        descriptor(&directory, "linked.slot").expect_err("link must fail"),
        DurableFileError::UnsupportedLocation
    );
}

#[test]
fn staging_is_create_new_collision_safe_bounded_and_drop_cleaned() {
    let (root, directory) = fixture();
    let target = descriptor(&directory, "settings.a").expect("target");
    let mut stages = Vec::new();
    for _ in 0..DURABLE_STAGE_ATTEMPTS {
        stages.push(
            target
                .create_staged(TEST_MAX_BYTES)
                .expect("bounded distinct stage"),
        );
    }
    assert_eq!(
        target
            .create_staged(TEST_MAX_BYTES)
            .expect_err("all staging slots occupied"),
        DurableFileError::CollisionLimit
    );
    drop(stages);
    assert_eq!(
        fs::read_dir(root.path()).expect("list root").count(),
        0,
        "ordinary drop must remove unpublished stages"
    );
}

#[test]
fn seal_flushes_reopens_and_rejects_wrong_length_or_digest() {
    let (_root, directory) = fixture();
    let target = descriptor(&directory, "settings.a").expect("target");

    let mut wrong_length = target.create_staged(TEST_MAX_BYTES).expect("length stage");
    wrong_length.write_chunk(NEW).expect("write stage");
    assert_eq!(
        wrong_length
            .seal(NEW.len() as u64 + 1, digest(NEW))
            .expect_err("wrong length"),
        DurableFileError::Integrity
    );

    let mut wrong_digest = target.create_staged(TEST_MAX_BYTES).expect("digest stage");
    wrong_digest.write_chunk(NEW).expect("write stage");
    assert_eq!(
        wrong_digest
            .seal(NEW.len() as u64, digest(OLD))
            .expect_err("wrong digest"),
        DurableFileError::Integrity
    );
}

#[test]
fn publish_new_is_same_volume_verified_and_preserves_source_on_preflight_failure() {
    let (root, directory) = fixture();
    let target = descriptor(&directory, "settings.a").expect("target");
    let mut staged = sealed(&target, NEW);
    let receipt = staged.publish_new(&target).expect("publish new target");
    assert_eq!(receipt.sha256(), &digest(NEW));
    assert_eq!(
        fs::read(root.path().join("settings.a")).expect("read target"),
        NEW
    );

    let blocked = descriptor(&directory, "blocked.slot").expect("blocked target");
    fs::write(root.path().join("blocked.slot"), OLD).expect("existing target");
    let mut source = sealed(&blocked, NEW);
    assert_eq!(
        source
            .publish_new(&blocked)
            .expect_err("existing target must fail"),
        DurableFileError::TargetExists
    );
    assert_eq!(
        fs::read(root.path().join("blocked.slot")).expect("old target"),
        OLD
    );
    assert!(
        fs::read_dir(root.path()).expect("list stages").count() >= 3,
        "failed publication must retain its sealed source until caller drops it"
    );
}

#[test]
fn replace_existing_saves_old_target_and_preserves_all_inputs_on_preflight_failure() {
    let (root, directory) = fixture();
    let target = descriptor(&directory, "tokenmaster.sqlite3").expect("target");
    let backup = descriptor(&directory, "quarantine.main").expect("backup");
    fs::write(root.path().join("tokenmaster.sqlite3"), OLD).expect("old target");
    let mut staged = sealed(&target, NEW);
    staged
        .replace_existing(&target, &backup)
        .expect("replace existing");
    assert_eq!(
        fs::read(root.path().join("tokenmaster.sqlite3")).expect("new target"),
        NEW
    );
    assert_eq!(
        fs::read(root.path().join("quarantine.main")).expect("old backup"),
        OLD
    );

    let failed_target = descriptor(&directory, "failed.sqlite3").expect("failed target");
    let occupied_backup = descriptor(&directory, "occupied.backup").expect("occupied backup");
    fs::write(root.path().join("failed.sqlite3"), OLD).expect("failed old target");
    fs::write(root.path().join("occupied.backup"), b"keep-backup").expect("backup blocker");
    let mut source = sealed(&failed_target, NEW);
    assert_eq!(
        source
            .replace_existing(&failed_target, &occupied_backup)
            .expect_err("occupied backup must fail"),
        DurableFileError::TargetExists
    );
    assert_eq!(
        fs::read(root.path().join("failed.sqlite3")).expect("preserved target"),
        OLD
    );
    assert_eq!(
        fs::read(root.path().join("occupied.backup")).expect("preserved backup"),
        b"keep-backup"
    );
}

#[test]
fn public_errors_and_handles_are_path_private() {
    let (root, directory) = fixture();
    let target = descriptor(&directory, "private.slot").expect("target");
    let staged = target.create_staged(TEST_MAX_BYTES).expect("stage");
    assert_eq!(format!("{target:?}"), "DurableFileTarget([redacted])");
    assert_eq!(format!("{staged:?}"), "DurableStagedFile([redacted])");
    for error in [
        DurableFileError::InvalidName,
        DurableFileError::UnsupportedLocation,
        DurableFileError::CollisionLimit,
        DurableFileError::TargetExists,
        DurableFileError::TargetMissing,
        DurableFileError::UnexpectedType,
        DurableFileError::InvalidState,
        DurableFileError::Integrity,
        DurableFileError::CapacityExceeded,
        DurableFileError::Unavailable,
        DurableFileError::RecoveryRequired,
    ] {
        let display = error.to_string();
        assert!(!display.contains(root.path().to_string_lossy().as_ref()));
        assert!(!format!("{error:?}").contains(root.path().to_string_lossy().as_ref()));
    }
}

#[test]
fn caller_byte_limit_global_limit_and_chunk_limit_fail_before_writing_excess() {
    let (root, directory) = fixture();
    let target = descriptor(&directory, "bounded.slot").expect("target");
    assert_eq!(
        target
            .create_staged(MAX_DURABLE_FILE_BYTES + 1)
            .expect_err("global maximum must fail"),
        DurableFileError::CapacityExceeded
    );

    let mut staged = target.create_staged(4).expect("bounded stage");
    staged.write_chunk(b"1234").expect("exact limit");
    assert_eq!(
        staged.write_chunk(b"5").expect_err("caller limit"),
        DurableFileError::CapacityExceeded
    );
    staged
        .seal(4, digest(b"1234"))
        .expect("excess was not written");

    let mut chunk_limited = target
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("chunk stage");
    let oversized = vec![0_u8; MAX_DURABLE_WRITE_CHUNK_BYTES + 1];
    assert_eq!(
        chunk_limited
            .write_chunk(&oversized)
            .expect_err("chunk limit"),
        DurableFileError::CapacityExceeded
    );
    assert!(
        fs::read_dir(root.path()).expect("bounded files").count() <= 2,
        "bounded failures must not create additional retained files"
    );
}

#[cfg(windows)]
#[test]
fn forty_pre_and_post_publication_kills_leave_exact_complete_files() {
    let (root, _directory) = fixture();
    let target_path = root.path().join("atomic.slot");
    let old = vec![b'A'; 256 * 1024];
    let new = vec![b'B'; 256 * 1024];
    fs::write(&target_path, &old).expect("initial target");

    for round in 0..40 {
        let backup_path = root.path().join("atomic.backup");
        if backup_path.exists() {
            fs::remove_file(&backup_path).expect("remove prior backup");
        }
        let before = fs::read(&target_path).expect("read pre-round target");
        let replacement = if before == old { &new } else { &old };
        let mut child = Command::new(env!("CARGO_BIN_EXE_durable_file_fixture"))
            .arg(root.path())
            .arg("atomic.slot")
            .arg("atomic.backup")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn durable fixture");
        let mut stdin = child.stdin.take().expect("fixture stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("fixture stdout"));
        let mut boundary = String::new();
        stdout
            .read_line(&mut boundary)
            .expect("fixture prepared boundary");
        assert_eq!(boundary, "prepared\n");
        stdin.write_all(b"publish\n").expect("arm publication");
        stdin.flush().expect("flush publication arm");

        boundary.clear();
        stdout
            .read_line(&mut boundary)
            .expect("fixture publication boundary");
        assert_eq!(boundary, "publishing\n");
        if round % 2 == 0 {
            child.kill().expect("kill before replacement call");
            let _ = child.wait().expect("wait pre-publication fixture");
            assert_eq!(fs::read(&target_path).expect("old target remains"), before);
            assert!(!backup_path.exists(), "backup is not published early");
        } else {
            stdin.write_all(b"commit\n").expect("commit publication");
            stdin.flush().expect("flush publication commit");
            boundary.clear();
            stdout
                .read_line(&mut boundary)
                .expect("fixture published boundary");
            assert_eq!(boundary, "published\n");
            child.kill().expect("kill after replacement call");
            let _ = child.wait().expect("wait post-publication fixture");
            assert_eq!(
                fs::read(&target_path).expect("new target published"),
                replacement.as_slice()
            );
            assert_eq!(
                fs::read(&backup_path).expect("old target backed up"),
                before
            );
        }

        let observed = fs::read(&target_path).expect("target always exists");
        assert!(
            observed == old || observed == new,
            "target was a partial mixture"
        );
    }
}

#[cfg(windows)]
#[test]
fn twenty_race_kills_at_replace_entry_leave_only_exact_old_or_new_state() {
    let (root, _directory) = fixture();
    let target_path = root.path().join("atomic.slot");
    let backup_path = root.path().join("atomic.backup");
    let old = vec![b'A'; 256 * 1024];
    let new = vec![b'B'; 256 * 1024];
    fs::write(&target_path, &old).expect("initial target");

    for _ in 0..20 {
        if backup_path.exists() {
            fs::remove_file(&backup_path).expect("remove prior backup");
        }
        let before = fs::read(&target_path).expect("read pre-round target");
        let replacement = if before == old { &new } else { &old };
        let mut child = Command::new(env!("CARGO_BIN_EXE_durable_file_fixture"))
            .arg(root.path())
            .arg("atomic.slot")
            .arg("atomic.backup")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn durable fixture");
        let mut stdin = child.stdin.take().expect("fixture stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("fixture stdout"));
        let mut boundary = String::new();
        stdout.read_line(&mut boundary).expect("prepared boundary");
        assert_eq!(boundary, "prepared\n");
        stdin.write_all(b"publish\n").expect("arm publication");
        stdin.flush().expect("flush publication arm");
        boundary.clear();
        stdout
            .read_line(&mut boundary)
            .expect("publication boundary");
        assert_eq!(boundary, "publishing\n");

        stdin.write_all(b"commit\n").expect("enter replacement");
        stdin.flush().expect("flush replacement entry");
        let _ = child.kill();
        let _ = child.wait().expect("wait race fixture");

        let observed = fs::read(&target_path).expect("target always exists");
        assert!(
            observed == before || observed.as_slice() == replacement.as_slice(),
            "target was a partial mixture"
        );
        if observed.as_slice() == replacement.as_slice() {
            assert_eq!(
                fs::read(&backup_path).expect("published old backup"),
                before
            );
        } else if backup_path.exists() {
            assert_eq!(
                fs::read(&backup_path).expect("retained exact old backup"),
                before
            );
        }
    }
}

#[test]
fn path_constructor_is_not_exposed_by_the_target_api() {
    let (_root, directory) = fixture();
    let _ = descriptor(&directory, "fixed.slot").expect("exact child");
    let _no_arbitrary_path_constructor: fn(
        &ValidatedLocalDirectory,
        &str,
    ) -> Result<DurableFileTarget, DurableFileError> = DurableFileTarget::exact_child;
}
