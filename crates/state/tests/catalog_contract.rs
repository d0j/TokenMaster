mod package_support;

use std::fs;

use tempfile::TempDir;
use tokenmaster_platform::{
    BackupDirectory, BackupDirectoryEntry, MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCatalog, BackupCompression, BackupMetadata, BackupPackage, BackupPurpose, CatalogHealth,
    RetentionAdmission, RetentionPolicy, StateErrorCode,
};

use package_support::{ControlledRoot, backup_bytes_at, digest, read_backup_bytes, settings};

fn fixture() -> (TempDir, BackupDirectory) {
    let root = TempDir::new().expect("catalog root");
    let validated = ValidatedLocalDirectory::new(root.path()).expect("validated root");
    let directory = BackupDirectory::open_or_create(&validated).expect("backup directory");
    (root, directory)
}

fn publish(directory: &BackupDirectory, bytes: &[u8]) -> BackupDirectoryEntry {
    let mut staged = directory
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("backup stage");
    staged.write_chunk(bytes).expect("write backup");
    staged
        .seal(bytes.len() as u64, digest(bytes))
        .expect("seal backup");
    directory.publish(&mut staged).expect("publish backup")
}

#[test]
fn typed_package_writer_composes_with_backup_directory_and_catalog_verification() {
    let (_root, directory) = fixture();
    let catalog = BackupCatalog::rebuild(&directory, None).expect("empty catalog");
    let package_root = ControlledRoot::new();
    let database = b"SQLite format 3\0typed backup directory package";
    let database_target = package_root.publish_bytes("snapshot.sqlite3", database);
    let mut database_reader = package_root.open(&database_target);
    let mut package_stage = directory
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("backup directory stage");
    let receipt = BackupPackage::write_to_backup_stage(
        &settings(),
        &mut database_reader,
        database.len() as u64,
        digest(database),
        13,
        BackupCompression::Normal,
        BackupMetadata::new(1_735_689_600_000, BackupPurpose::Periodic).expect("backup metadata"),
        &mut package_stage,
    )
    .expect("typed backup package write");
    let verified = BackupPackage::verify_backup_stage(&package_stage)
        .expect("verify unpublished typed package");
    assert_eq!(verified.receipt(), receipt);
    let admission = RetentionAdmission::preflight(&catalog, &verified, RetentionPolicy::default())
        .expect("pre-publication retention admission");
    let entry = directory
        .publish(&mut package_stage)
        .expect("publish typed package");
    assert_eq!(entry.len(), receipt.package_len());

    let mut published =
        BackupCatalog::rebuild(&directory, Some(&catalog)).expect("published catalog rebuild");
    assert_eq!(published.points().len(), 1);
    let selection = published.points()[0].selection();
    published
        .bind_verified(selection, &verified)
        .expect("bind exact verification proof");
    assert_eq!(published.points()[0].health(), CatalogHealth::Verified);
    let cycle = admission
        .confirm_published(&published, selection)
        .expect("confirm exact publication");
    assert_eq!(
        cycle.next_deletion(&published).expect("retention plan"),
        None
    );
}

#[test]
fn typed_package_writer_discards_a_failed_backup_directory_stage() {
    let (_root, directory) = fixture();
    let package_root = ControlledRoot::new();
    let database = b"SQLite format 3\0failed typed package";
    let database_target = package_root.publish_bytes("snapshot.sqlite3", database);
    let mut database_reader = package_root.open(&database_target);
    let mut package_stage = directory.create_staged(1).expect("small backup stage");
    let error = BackupPackage::write_to_backup_stage(
        &settings(),
        &mut database_reader,
        database.len() as u64,
        digest(database),
        13,
        BackupCompression::Normal,
        BackupMetadata::new(1_735_689_600_000, BackupPurpose::Periodic).expect("backup metadata"),
        &mut package_stage,
    )
    .expect_err("bounded destination must reject package");
    assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    assert!(
        directory
            .scan()
            .expect("failed stage removed")
            .entries()
            .is_empty()
    );

    let database_target = package_root.publish_bytes("snapshot-2.sqlite3", database);
    let mut database_reader = package_root.open(&database_target);
    let mut discarded_stage = directory
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("discarded backup stage");
    discarded_stage.discard().expect("discard backup stage");
    let error = BackupPackage::write_to_backup_stage(
        &settings(),
        &mut database_reader,
        database.len() as u64,
        digest(database),
        13,
        BackupCompression::Normal,
        BackupMetadata::new(1_735_689_600_000, BackupPurpose::Periodic).expect("backup metadata"),
        &mut discarded_stage,
    )
    .expect_err("discarded stage is an invariant violation");
    assert_eq!(error.code(), StateErrorCode::InternalInvariant);
}

#[test]
fn rebuild_uses_self_describing_headers_and_only_current_proof_marks_verified() {
    let (root, directory) = fixture();
    let created_at = 1_735_689_600_000;
    let (bytes, _) = backup_bytes_at(
        b"SQLite format 3\0catalog fixture",
        BackupCompression::Compact,
        BackupPurpose::PreMigration,
        created_at,
    );
    publish(&directory, &bytes);

    let mut catalog = BackupCatalog::rebuild(&directory, None).expect("cold rebuild");
    assert_eq!(catalog.generation().get(), 1);
    assert_eq!(catalog.points().len(), 1);
    let point = &catalog.points()[0];
    assert_eq!(point.selection().ordinal(), 0);
    assert_eq!(point.created_at_utc_ms(), Some(created_at));
    assert_eq!(point.size_bytes(), bytes.len() as u64);
    assert_eq!(point.purpose(), Some(BackupPurpose::PreMigration));
    assert_eq!(point.health(), CatalogHealth::HeaderValid);

    let rendered = format!("{point:?}");
    assert!(rendered.contains("ordinal: 0"));
    assert!(rendered.contains("HeaderValid"));
    assert!(!rendered.contains(root.path().to_string_lossy().as_ref()));
    assert!(!rendered.contains("point-00"));
    assert!(!rendered.contains("sha256"));

    let (verified, _) = read_backup_bytes(&bytes).expect("full package verification");
    let selection = point.selection();
    catalog
        .bind_verified(selection, &verified)
        .expect("bind exact proof");
    assert_eq!(catalog.points()[0].health(), CatalogHealth::Verified);

    let warm = BackupCatalog::rebuild(&directory, Some(&catalog)).expect("warm rebuild");
    assert_eq!(warm.generation().get(), 2);
    assert_eq!(warm.points()[0].health(), CatalogHealth::Verified);

    let package_path = fs::read_dir(root.path().join("backups"))
        .expect("package directory")
        .next()
        .expect("one package")
        .expect("package entry")
        .path();
    let mut changed = fs::read(&package_path).expect("package bytes");
    let body_index = changed.len() / 2;
    changed[body_index] ^= 1;
    fs::write(&package_path, &changed).expect("replace changed package");

    let changed_catalog = BackupCatalog::rebuild(&directory, Some(&warm)).expect("changed rebuild");
    assert_eq!(
        changed_catalog.points()[0].health(),
        CatalogHealth::HeaderValid
    );
}

#[test]
fn corrupt_headers_are_visible_but_duplicates_and_stale_generations_fail_closed() {
    let (_root, directory) = fixture();
    let (valid, _) = backup_bytes_at(
        b"SQLite format 3\0duplicate fixture",
        BackupCompression::Normal,
        BackupPurpose::Periodic,
        1_735_776_000_000,
    );
    let mut corrupt = valid.clone();
    corrupt[0] ^= 1;
    publish(&directory, &corrupt);

    let corrupt_catalog = BackupCatalog::rebuild(&directory, None).expect("corrupt catalog row");
    assert_eq!(corrupt_catalog.points().len(), 1);
    assert_eq!(corrupt_catalog.points()[0].health(), CatalogHealth::Corrupt);
    assert_eq!(corrupt_catalog.points()[0].created_at_utc_ms(), None);
    assert_eq!(corrupt_catalog.points()[0].purpose(), None);

    let (duplicate_root, duplicate_directory) = fixture();
    publish(&duplicate_directory, &valid);
    publish(&duplicate_directory, &valid);
    let error = BackupCatalog::rebuild(&duplicate_directory, None).expect_err("duplicate bytes");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert!(!format!("{error:?}").contains(duplicate_root.path().to_string_lossy().as_ref()));

    let (stale_root, stale_directory) = fixture();
    publish(&stale_directory, &valid);
    let stale = BackupCatalog::rebuild(&stale_directory, None).expect("generation one");
    let stale_selection = stale.points()[0].selection();
    let mut current =
        BackupCatalog::rebuild(&stale_directory, Some(&stale)).expect("generation two");
    let (verified, _) = read_backup_bytes(&valid).expect("verification proof");
    let error = current
        .bind_verified(stale_selection, &verified)
        .expect_err("stale catalog generation");
    assert_eq!(error.code(), StateErrorCode::InvalidInput);
    assert!(!format!("{current:?}").contains(stale_root.path().to_string_lossy().as_ref()));
}
