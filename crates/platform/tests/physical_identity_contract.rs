use std::fs::{File, OpenOptions};
use std::io::Write;

use tempfile::tempdir;
use tokenmaster_platform::PhysicalFileIdentity;

#[test]
fn identity_tracks_the_open_file_not_its_path_or_length() {
    let root = tempdir().expect("temporary directory");
    let first = root.path().join("first.jsonl");
    let renamed = root.path().join("renamed.jsonl");
    std::fs::write(&first, b"one\n").expect("write fixture");

    let open = File::open(&first).expect("open fixture");
    let before = PhysicalFileIdentity::from_file(&open).expect("identity");
    OpenOptions::new()
        .append(true)
        .open(&first)
        .expect("open append")
        .write_all(b"two\n")
        .expect("append fixture");
    let after_append = PhysicalFileIdentity::from_file(&open).expect("identity");
    assert_eq!(before, after_append);

    std::fs::rename(&first, &renamed).expect("rename fixture");
    let after_rename =
        PhysicalFileIdentity::from_file(&File::open(&renamed).expect("open renamed fixture"))
            .expect("identity");
    assert_eq!(before, after_rename);

    std::fs::write(&first, b"replacement\n").expect("write replacement");
    let replacement =
        PhysicalFileIdentity::from_file(&File::open(&first).expect("open replacement"))
            .expect("identity");
    assert_ne!(before, replacement);
    assert_eq!(format!("{before:?}"), "PhysicalFileIdentity([redacted])");
}
