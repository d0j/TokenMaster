use std::path::Path;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_store::{StoreErrorCode, USAGE_SCHEMA_VERSION, UsageStore};

const BENEFIT_TABLES: [&str; 9] = [
    "benefit_change",
    "benefit_lot_current",
    "benefit_lot_revision",
    "benefit_reminder_delivery",
    "benefit_reminder_due",
    "benefit_reminder_profile",
    "benefit_reminder_threshold",
    "benefit_scope",
    "benefit_state",
];

const BENEFIT_INDEXES: [&str; 6] = [
    "benefit_change_scope_sequence",
    "benefit_delivery_scope_time",
    "benefit_due_next",
    "benefit_lot_current_expiry",
    "benefit_lot_revision_retention",
    "benefit_profile_scope",
];

const BENEFIT_TRIGGERS: [&str; 4] = [
    "benefit_change_no_update",
    "benefit_delivery_no_update",
    "benefit_lot_revision_no_update",
    "benefit_state_no_delete",
];

fn raw_connection(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("open raw archive");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
}

fn strip_benefit_schema_to_exact_v10(connection: &Connection) {
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS benefit_change_no_update;
             DROP TRIGGER IF EXISTS benefit_delivery_no_update;
             DROP TRIGGER IF EXISTS benefit_lot_revision_no_update;
             DROP TRIGGER IF EXISTS benefit_state_no_delete;
             DROP INDEX IF EXISTS benefit_change_scope_sequence;
             DROP INDEX IF EXISTS benefit_delivery_scope_time;
             DROP INDEX IF EXISTS benefit_due_next;
             DROP INDEX IF EXISTS benefit_lot_current_expiry;
             DROP INDEX IF EXISTS benefit_lot_revision_retention;
             DROP INDEX IF EXISTS benefit_profile_scope;
             DROP TABLE IF EXISTS benefit_reminder_delivery;
             DROP TABLE IF EXISTS benefit_reminder_due;
             DROP TABLE IF EXISTS benefit_reminder_threshold;
             DROP TABLE IF EXISTS benefit_reminder_profile;
             DROP TABLE IF EXISTS benefit_change;
             DROP TABLE IF EXISTS benefit_lot_current;
             DROP TABLE IF EXISTS benefit_lot_revision;
             DROP TABLE IF EXISTS benefit_scope;
             DROP TABLE IF EXISTS benefit_state;",
        )
        .expect("strip benefit schema");
    connection
        .pragma_update(None, "user_version", 10_i64)
        .expect("set exact v10");
}

#[test]
fn fresh_schema_has_strict_bounded_benefit_objects_and_recommended_profile() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-schema.sqlite3");
    drop(UsageStore::open(&path).expect("create current schema"));
    let connection = raw_connection(&path);

    assert_eq!(USAGE_SCHEMA_VERSION, 11);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("user version"),
        11
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT revision, current_lot_count, retained_change_count,
                        pending_due_count, retained_delivery_count, last_published_at_ms
                 FROM benefit_state WHERE singleton_id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                    ))
                },
            )
            .expect("benefit state"),
        (0, 0, 0, 0, 0, None)
    );

    let tables = connection
        .prepare(
            "SELECT name, strict FROM pragma_table_list
             WHERE schema = 'main' AND name LIKE 'benefit_%'
             ORDER BY name",
        )
        .expect("benefit tables")
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("table rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    assert_eq!(
        tables
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
        BENEFIT_TABLES
    );
    assert!(tables.iter().all(|(_, strict)| *strict == 1));

    let indexes = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'index' AND name LIKE 'benefit_%'
             ORDER BY name",
        )
        .expect("benefit indexes")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("index rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect indexes");
    assert_eq!(indexes, BENEFIT_INDEXES);

    let triggers = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'trigger' AND name LIKE 'benefit_%'
             ORDER BY name",
        )
        .expect("benefit triggers")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("trigger rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect triggers");
    assert_eq!(triggers, BENEFIT_TRIGGERS);

    let thresholds = connection
        .prepare(
            "SELECT threshold_seconds
             FROM benefit_reminder_threshold
             WHERE profile_kind = 'global' AND length(profile_scope_id) = 0
             ORDER BY threshold_seconds DESC",
        )
        .expect("recommended thresholds")
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("threshold rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect thresholds");
    assert_eq!(thresholds, vec![604_800, 86_400, 43_200, 21_600, 3_600]);

    for table in BENEFIT_TABLES {
        let pragma = format!("PRAGMA table_info({table})");
        let mut statement = connection.prepare(&pragma).expect("table info");
        for column in statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("column rows")
        {
            let column = column.expect("column").to_ascii_lowercase();
            for forbidden in [
                "path",
                "prompt",
                "response",
                "reasoning",
                "command",
                "raw",
                "credential",
                "cookie",
                "header",
                "url",
                "email",
                "title",
                "description",
            ] {
                assert!(
                    !column.contains(forbidden),
                    "forbidden benefit column {table}.{column}"
                );
            }
        }
    }
}

#[test]
fn exact_v10_migration_adds_empty_benefits_without_touching_existing_facts() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-migration.sqlite3");
    drop(UsageStore::open(&path).expect("create current archive"));
    {
        let connection = raw_connection(&path);
        connection
            .execute(
                "UPDATE usage_archive_state
                 SET archive_generation = 7, dataset_generation = 11
                 WHERE singleton_id = 1",
                [],
            )
            .expect("usage fixture");
        connection
            .execute(
                "UPDATE usage_aggregate_state
                 SET expected_dataset_generation = 11
                 WHERE singleton_id = 1",
                [],
            )
            .expect("aggregate fixture");
        connection
            .execute(
                "UPDATE quota_state
                 SET revision = 1, last_published_at_ms = 100
                 WHERE singleton_id = 1",
                [],
            )
            .expect("quota fixture");
        strip_benefit_schema_to_exact_v10(&connection);
    }

    let before = {
        let connection = raw_connection(&path);
        connection
            .query_row(
                "SELECT archive_generation, dataset_generation,
                        (SELECT revision FROM quota_state WHERE singleton_id = 1),
                        (SELECT count(*) FROM usage_event),
                        (SELECT count(*) FROM quota_transition)
                 FROM usage_archive_state WHERE singleton_id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .expect("pre-migration facts")
    };

    drop(UsageStore::open(&path).expect("migrate exact v10"));
    let connection = raw_connection(&path);
    let after = connection
        .query_row(
            "SELECT archive_generation, dataset_generation,
                    (SELECT revision FROM quota_state WHERE singleton_id = 1),
                    (SELECT count(*) FROM usage_event),
                    (SELECT count(*) FROM quota_transition)
             FROM usage_archive_state WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .expect("post-migration facts");
    assert_eq!(after, before);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("current version"),
        11
    );
}

#[test]
fn weakened_benefit_schema_is_rejected_on_reopen() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-malformed.sqlite3");
    drop(UsageStore::open(&path).expect("create current schema"));
    {
        let connection = raw_connection(&path);
        connection
            .pragma_update(None, "writable_schema", "ON")
            .expect("enable writable schema");
        let changed = connection
            .execute(
                "UPDATE sqlite_schema
                 SET sql = replace(
                   sql,
                   'CHECK(length(lot_id) = 32)',
                   'CHECK(length(lot_id) = 31)'
                 )
                 WHERE type = 'table' AND name = 'benefit_lot_revision'",
                [],
            )
            .expect("weaken schema");
        assert_eq!(changed, 1);
        connection
            .pragma_update(None, "writable_schema", "OFF")
            .expect("disable writable schema");
    }

    let error = match UsageStore::open(&path) {
        Ok(_) => panic!("weakened benefit schema unexpectedly reopened"),
        Err(error) => error,
    };
    assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
}
