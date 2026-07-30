use std::fs;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{
    BackupDirectory, BackupDirectoryEntry, BackupDirectoryError, MAX_BACKUP_DIRECTORY_FILES,
    MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory,
};

fn fixture() -> (TempDir, BackupDirectory) {
    let root = TempDir::new().expect("temporary root");
    let validated = ValidatedLocalDirectory::new(root.path()).expect("validated root");
    let directory = BackupDirectory::open_or_create(&validated).expect("backup directory");
    (root, directory)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn publish(directory: &BackupDirectory, bytes: &[u8]) -> BackupDirectoryEntry {
    let mut staged = directory
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("create backup stage");
    staged.write_chunk(bytes).expect("write backup bytes");
    staged
        .seal(bytes.len() as u64, digest(bytes))
        .expect("seal backup stage");
    directory.publish(&mut staged).expect("publish backup")
}

#[test]
fn exact_directory_streams_through_opaque_path_private_entries() {
    let (root, directory) = fixture();
    assert_eq!(format!("{directory:?}"), "BackupDirectory([redacted])");

    let empty = directory.scan().expect("empty scan");
    assert!(empty.entries().is_empty());
    assert_eq!(
        format!("{:?}", empty.generation()),
        "BackupDirectoryGeneration([redacted])"
    );

    let published = publish(&directory, b"verified package bytes");
    assert_eq!(published.ordinal(), 0);
    assert_eq!(published.len(), 22);
    let rendered = format!("{published:?}");
    assert_eq!(rendered, "BackupDirectoryEntry { ordinal: 0, len: 22 }");
    assert!(!rendered.contains(root.path().to_string_lossy().as_ref()));
    assert!(!rendered.contains("point-00"));

    let scan = directory.scan().expect("one entry");
    assert_eq!(scan.entries().len(), 1);
    assert_eq!(scan.entries()[0], published);

    let mut reader = directory
        .open_reader(&published, MAX_DURABLE_FILE_BYTES)
        .expect("open exact backup");
    let mut output = Vec::new();
    let mut buffer = [0_u8; 7];
    loop {
        let count = reader.read_chunk(&mut buffer).expect("bounded read");
        if count == 0 {
            break;
        }
        output.extend_from_slice(&buffer[..count]);
    }
    assert_eq!(output, b"verified package bytes");
}

#[test]
fn namespace_is_exact_link_free_and_capped_at_thirty_two_files() {
    let (_root, directory) = fixture();
    for value in 0..MAX_BACKUP_DIRECTORY_FILES {
        let entry = publish(&directory, &[value as u8]);
        assert_eq!(usize::from(entry.ordinal()), value);
    }
    assert_eq!(directory.scan().expect("full scan").entries().len(), 32);
    assert_eq!(
        directory
            .create_staged(1)
            .expect_err("thirty-third point must fail"),
        BackupDirectoryError::CapacityExceeded
    );

    let (unexpected_root, unexpected) = fixture();
    fs::write(
        unexpected_root
            .path()
            .join("backups")
            .join("unexpected.txt"),
        b"unexpected",
    )
    .expect("unexpected child");
    assert_eq!(
        unexpected.scan().expect_err("unexpected name"),
        BackupDirectoryError::UnexpectedEntry
    );

    let (typed_root, typed) = fixture();
    fs::create_dir(typed_root.path().join("backups").join("point-00.tmbackup"))
        .expect("directory in package slot");
    assert_eq!(
        typed.scan().expect_err("unexpected type"),
        BackupDirectoryError::UnexpectedType
    );

    let (linked_root, linked) = fixture();
    let first = linked_root.path().join("backups").join("point-00.tmbackup");
    let second = linked_root.path().join("backups").join("point-01.tmbackup");
    fs::write(&first, b"linked").expect("source file");
    fs::hard_link(&first, &second).expect("hard link");
    assert_eq!(
        linked.scan().expect_err("hard links are forbidden"),
        BackupDirectoryError::LinkedEntry
    );

    let (_staged_root, staged_directory) = fixture();
    let staged = staged_directory.create_staged(1).expect("controlled stage");
    assert_eq!(
        staged_directory
            .scan()
            .expect_err("live or abandoned stage requires recovery"),
        BackupDirectoryError::RecoveryRequired
    );
    drop(staged);
    assert!(
        staged_directory
            .scan()
            .expect("dropped stage is removed")
            .entries()
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn symbolic_link_package_child_is_rejected() {
    use std::os::unix::fs::symlink;

    let (root, directory) = fixture();
    let outside = root.path().join("outside.tmbackup");
    fs::write(&outside, b"outside").expect("outside file");
    symlink(
        &outside,
        root.path().join("backups").join("point-00.tmbackup"),
    )
    .expect("package symlink");
    assert_eq!(
        directory.scan().expect_err("linked child"),
        BackupDirectoryError::LinkedEntry
    );
}

#[cfg(windows)]
#[test]
fn symbolic_or_reparse_package_child_is_rejected() {
    use std::os::windows::fs::symlink_file;

    let (root, directory) = fixture();
    let outside = root.path().join("outside.tmbackup");
    fs::write(&outside, b"outside").expect("outside file");
    symlink_file(
        &outside,
        root.path().join("backups").join("point-00.tmbackup"),
    )
    .expect("package symlink");
    assert_eq!(
        directory.scan().expect_err("linked child"),
        BackupDirectoryError::LinkedEntry
    );
}

#[test]
fn open_and_delete_revalidate_identity_and_prefix_deletion_is_restart_safe() {
    let (root, directory) = fixture();
    let first = publish(&directory, b"first");
    let second = publish(&directory, b"second");
    let third = publish(&directory, b"third");

    directory.delete(&first).expect("delete first point");
    let after_one = directory.scan().expect("scan after one deletion");
    assert_eq!(after_one.entries(), &[second.clone(), third.clone()]);
    assert_eq!(
        directory.delete(&first).expect_err("stale deleted token"),
        BackupDirectoryError::StaleEntry
    );

    let live_path = fs::read_dir(root.path().join("backups"))
        .expect("backup entries")
        .map(|entry| entry.expect("entry").path())
        .find(|path| fs::read(path).expect("entry bytes") == b"second")
        .expect("second path");
    fs::remove_file(&live_path).expect("replace second");
    fs::write(&live_path, b"second").expect("replacement bytes");

    assert_eq!(
        directory
            .open_reader(&second, MAX_DURABLE_FILE_BYTES)
            .expect_err("stale open"),
        BackupDirectoryError::StaleEntry
    );
    assert_eq!(
        directory.delete(&second).expect_err("stale delete"),
        BackupDirectoryError::StaleEntry
    );
    assert!(live_path.exists(), "replacement must not be deleted");

    directory.delete(&third).expect("delete third point");
    let final_scan = directory.scan().expect("final scan");
    assert_eq!(final_scan.entries().len(), 1);
    assert_eq!(final_scan.entries()[0].ordinal(), second.ordinal());
}

#[test]
fn sealed_stage_exposes_no_independent_publication_and_discard_is_irreversible() {
    let (_root, directory) = fixture();
    let mut staged = directory.create_staged(16).expect("backup stage");
    staged.write_chunk(b"discarded").expect("stage bytes");
    assert_eq!(staged.written_len(), 9);
    staged.discard().expect("discard stage");
    assert_eq!(
        directory
            .publish(&mut staged)
            .expect_err("discarded stage cannot publish"),
        BackupDirectoryError::InvalidState
    );
    assert!(
        directory
            .scan()
            .expect("discard leaves no child")
            .entries()
            .is_empty()
    );
}
