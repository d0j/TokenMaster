use std::path::Path;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_store::{StoreErrorCode, USAGE_SCHEMA_VERSION, UsageStore};

const GIT_TABLES: [&str; 7] = [
    "git_activity_association",
    "git_category_aggregate",
    "git_day_aggregate",
    "git_day_category_aggregate",
    "git_installation_state",
    "git_repository",
    "git_warning",
];

const GIT_INDEXES: [&str; 4] = [
    "git_association_repository_activity",
    "git_day_category_repository_range",
    "git_day_repository_range",
    "git_repository_observed",
];

const GIT_TRIGGERS: [&str; 5] = [
    "git_category_no_update",
    "git_day_category_no_update",
    "git_day_no_update",
    "git_installation_state_no_delete",
    "git_warning_no_update",
];

fn raw_connection(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("open raw archive");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
}

fn strip_git_schema_to_exact_v12(connection: &Connection) {
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS git_category_no_update;
             DROP TRIGGER IF EXISTS git_day_no_update;
             DROP TRIGGER IF EXISTS git_day_category_no_update;
             DROP TRIGGER IF EXISTS git_installation_state_no_delete;
             DROP TRIGGER IF EXISTS git_warning_no_update;
             DROP INDEX IF EXISTS git_association_repository_activity;
             DROP INDEX IF EXISTS git_day_repository_range;
             DROP INDEX IF EXISTS git_day_category_repository_range;
             DROP INDEX IF EXISTS git_repository_observed;
             DROP TABLE IF EXISTS git_warning;
             DROP TABLE IF EXISTS git_category_aggregate;
             DROP TABLE IF EXISTS git_day_aggregate;
             DROP TABLE IF EXISTS git_day_category_aggregate;
             DROP TABLE IF EXISTS git_activity_association;
             DROP TABLE IF EXISTS git_repository;
             DROP TABLE IF EXISTS git_installation_state;",
        )
        .expect("strip Git schema");
    connection
        .pragma_update(None, "user_version", 12_i64)
        .expect("set exact v12");
}

#[test]
fn fresh_v13_schema_has_private_strict_bounded_git_objects() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-schema.sqlite3");
    drop(UsageStore::open(&path).expect("create current schema"));
    let connection = raw_connection(&path);

    assert_eq!(USAGE_SCHEMA_VERSION, 13);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("user version"),
        13
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT length(installation_salt), publication_revision,
                        repository_count, association_count, last_published_at_ms
                 FROM git_installation_state WHERE singleton_id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .expect("Git installation state"),
        (32, 0, 0, 0, None)
    );

    let tables = connection
        .prepare(
            "SELECT name, strict FROM pragma_table_list
             WHERE schema = 'main' AND name LIKE 'git_%'
             ORDER BY name",
        )
        .expect("Git tables")
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("Git table rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect Git tables");
    assert_eq!(
        tables
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
        GIT_TABLES
    );
    assert!(tables.iter().all(|(_, strict)| *strict == 1));

    let indexes = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'index' AND name LIKE 'git_%'
             ORDER BY name",
        )
        .expect("Git indexes")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("Git index rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect Git indexes");
    assert_eq!(indexes, GIT_INDEXES);

    let triggers = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'trigger' AND name LIKE 'git_%'
             ORDER BY name",
        )
        .expect("Git triggers")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("Git trigger rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect Git triggers");
    assert_eq!(triggers, GIT_TRIGGERS);

    for table in GIT_TABLES {
        let pragma = format!("PRAGMA table_info({table})");
        let mut statement = connection.prepare(&pragma).expect("table info");
        for column in statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("column rows")
        {
            let column = column.expect("column").to_ascii_lowercase();
            for forbidden in [
                "repository_path",
                "executable_path",
                "file_path",
                "absolute_path",
                "email",
                "commit_id",
                "commit_message",
                "ref_name",
                "object_id",
                "prompt",
                "response",
                "reasoning",
                "command",
                "raw",
                "credential",
            ] {
                assert!(
                    !column.contains(forbidden),
                    "private Git column {table}.{column}"
                );
            }
        }
    }
}

#[test]
fn exact_v12_migrates_transactionally_without_touching_existing_products() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-migration.sqlite3");
    drop(UsageStore::open(&path).expect("create current schema"));
    let connection = raw_connection(&path);
    let before: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_archive_state),
               (SELECT count(*) FROM usage_aggregate_state),
               (SELECT count(*) FROM quota_state),
               (SELECT count(*) FROM benefit_state)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("pre-migration products");
    strip_git_schema_to_exact_v12(&connection);
    drop(connection);

    drop(UsageStore::open(&path).expect("migrate exact v12"));
    let connection = raw_connection(&path);
    let after: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_archive_state),
               (SELECT count(*) FROM usage_aggregate_state),
               (SELECT count(*) FROM quota_state),
               (SELECT count(*) FROM benefit_state)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("post-migration products");
    assert_eq!(after, before);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("user version"),
        13
    );
}

#[test]
fn malformed_v13_git_state_fails_closed() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-corrupt.sqlite3");
    drop(UsageStore::open(&path).expect("create current schema"));
    let connection = raw_connection(&path);
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER git_installation_state_no_delete;
             DELETE FROM git_installation_state;
             CREATE TRIGGER git_installation_state_no_delete
             BEFORE DELETE ON git_installation_state
             BEGIN
               SELECT RAISE(ABORT, 'Git installation state is required');
             END;",
        )
        .expect("corrupt Git singleton");
    drop(connection);

    let error = UsageStore::open(&path).expect_err("missing Git state must fail closed");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
}
