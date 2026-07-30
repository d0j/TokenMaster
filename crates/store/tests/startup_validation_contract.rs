use std::fs;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_platform::ValidatedLocalDirectory;
use tokenmaster_store::{
    StartupArchiveStatus, StartupValidationMode, StoreErrorCode, USAGE_SCHEMA_VERSION, UsageStore,
    inspect_startup_archive,
};

struct Fixture {
    root: TempDir,
    data: ValidatedLocalDirectory,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary data root");
        let data = ValidatedLocalDirectory::new(root.path()).expect("validated data root");
        Self { root, data }
    }

    fn archive(&self) -> std::path::PathBuf {
        self.root.path().join("tokenmaster.sqlite3")
    }

    fn create_current(&self) {
        drop(UsageStore::open(self.archive()).expect("current archive"));
    }
}

#[test]
fn missing_archive_is_observed_without_creating_empty_truth() {
    let fixture = Fixture::new();

    let inspection = inspect_startup_archive(&fixture.data, StartupValidationMode::Quick)
        .expect("missing inspection");

    assert_eq!(inspection.status(), StartupArchiveStatus::Missing);
    assert_eq!(inspection.schema_version(), None);
    assert!(!inspection.quick_check_performed());
    assert!(!fixture.archive().exists());
}

#[test]
fn clean_current_archive_uses_bounded_schema_and_semantic_validation() {
    let fixture = Fixture::new();
    fixture.create_current();

    let inspection = inspect_startup_archive(&fixture.data, StartupValidationMode::Normal)
        .expect("normal startup validation");

    assert_eq!(inspection.status(), StartupArchiveStatus::Current);
    assert_eq!(
        inspection.schema_version(),
        Some(USAGE_SCHEMA_VERSION as u32)
    );
    assert!(!inspection.quick_check_performed());
    assert_eq!(
        format!("{inspection:?}"),
        format!(
            "StartupArchiveInspection {{ status: Current, schema_version: Some({}), quick_check_performed: false }}",
            USAGE_SCHEMA_VERSION
        )
    );
}

#[test]
fn unclean_startup_adds_quick_check_without_full_integrity_history() {
    let fixture = Fixture::new();
    fixture.create_current();

    let inspection = inspect_startup_archive(&fixture.data, StartupValidationMode::Quick)
        .expect("quick startup validation");

    assert_eq!(inspection.status(), StartupArchiveStatus::Current);
    assert!(inspection.quick_check_performed());
}

#[test]
fn malformed_header_is_definitive_corruption_but_does_not_get_rewritten() {
    let fixture = Fixture::new();
    let corrupt = b"not-a-sqlite-archive";
    fs::write(fixture.archive(), corrupt).expect("corrupt archive");

    let error = inspect_startup_archive(&fixture.data, StartupValidationMode::Quick)
        .expect_err("corrupt header");

    assert_eq!(error.code(), StoreErrorCode::BackupHeaderCorrupt);
    assert_eq!(
        fs::read(fixture.archive()).expect("unchanged bytes"),
        corrupt
    );
}

#[test]
fn newer_schema_is_upgrade_required_and_never_corruption_authority() {
    let fixture = Fixture::new();
    fixture.create_current();
    let connection = Connection::open(fixture.archive()).expect("newer schema connection");
    connection
        .pragma_update(None, "user_version", USAGE_SCHEMA_VERSION + 1)
        .expect("newer user version");
    drop(connection);

    let error = inspect_startup_archive(&fixture.data, StartupValidationMode::Quick)
        .expect_err("newer schema");

    assert_eq!(error.code(), StoreErrorCode::SchemaTooNew);
    let connection = Connection::open(fixture.archive()).expect("inspect preserved archive");
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .expect("preserved version"),
        USAGE_SCHEMA_VERSION + 1
    );
}
