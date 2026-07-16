use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{StoreErrorCode, USAGE_SCHEMA_VERSION, UsageStore};

const QUOTA_TABLES: [&str; 7] = [
    "quota_state",
    "quota_window_definition",
    "quota_sample",
    "quota_epoch_current",
    "quota_epoch_history",
    "quota_transition",
    "quota_window_current",
];

const QUOTA_INDEXES: [&str; 5] = [
    "quota_definition_scope_revision",
    "quota_epoch_history_retention",
    "quota_sample_retention",
    "quota_transition_window_sequence",
    "quota_window_current_scope",
];

const QUOTA_TRIGGERS: [&str; 5] = [
    "quota_epoch_history_no_update",
    "quota_sample_no_update",
    "quota_state_no_delete",
    "quota_transition_no_update",
    "quota_window_definition_no_update",
];

fn table_sql(connection: &Connection, table: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get::<_, String>(0),
        )
        .expect("table SQL")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn raw_connection(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("open raw archive");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
}

fn strip_quota_schema_to_exact_v9(connection: &Connection) {
    git_schema_v13::strip_git_schema(connection);
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS benefit_ack_no_update;
             DROP TRIGGER IF EXISTS benefit_change_no_update;
             DROP TRIGGER IF EXISTS benefit_delivery_no_update;
             DROP TRIGGER IF EXISTS benefit_lot_revision_no_update;
             DROP TRIGGER IF EXISTS benefit_state_no_delete;
             DROP INDEX IF EXISTS benefit_change_scope_sequence;
             DROP INDEX IF EXISTS benefit_delivery_scope_time;
             DROP INDEX IF EXISTS benefit_due_next;
             DROP INDEX IF EXISTS benefit_lot_current_expiry;
             DROP INDEX IF EXISTS benefit_lot_revision_retention;
             DROP INDEX IF EXISTS benefit_profile_scope;
             DROP TABLE IF EXISTS benefit_reminder_ack;
             DROP TABLE IF EXISTS benefit_reminder_delivery;
             DROP TABLE IF EXISTS benefit_reminder_due;
             DROP TABLE IF EXISTS benefit_reminder_threshold;
             DROP TABLE IF EXISTS benefit_reminder_profile;
             DROP TABLE IF EXISTS benefit_change;
             DROP TABLE IF EXISTS benefit_lot_current;
             DROP TABLE IF EXISTS benefit_lot_revision;
             DROP TABLE IF EXISTS benefit_scope;
             DROP TABLE IF EXISTS benefit_state;
             DROP TRIGGER IF EXISTS quota_epoch_history_no_update;
             DROP TRIGGER IF EXISTS quota_sample_no_update;
             DROP TRIGGER IF EXISTS quota_state_no_delete;
             DROP TRIGGER IF EXISTS quota_transition_no_update;
             DROP TRIGGER IF EXISTS quota_window_definition_no_update;
             DROP INDEX IF EXISTS quota_definition_scope_revision;
             DROP INDEX IF EXISTS quota_epoch_history_retention;
             DROP INDEX IF EXISTS quota_sample_retention;
             DROP INDEX IF EXISTS quota_transition_window_sequence;
             DROP INDEX IF EXISTS quota_window_current_scope;
             DROP TABLE IF EXISTS quota_window_current;
             DROP TABLE IF EXISTS quota_transition;
             DROP TABLE IF EXISTS quota_epoch_history;
             DROP TABLE IF EXISTS quota_epoch_current;
             DROP TABLE IF EXISTS quota_sample;
             DROP TABLE IF EXISTS quota_window_definition;
             DROP TABLE IF EXISTS quota_state;",
        )
        .expect("strip quota schema");
    connection
        .pragma_update(None, "user_version", 9_i64)
        .expect("set exact v9");
}

fn insert_quota_definition(connection: &Connection, scope_id: &[u8], window_id: &str) {
    connection
        .execute(
            "INSERT INTO quota_window_definition(
               scope_id, window_id, revision, provider_id, account_id,
               workspace_id, label_key, presentation, semantics,
               nominal_duration_seconds, maximum_post_reset_used_ppm,
               minimum_post_reset_remaining_ppm, minimum_used_ratio_drop_ppm
             ) VALUES (?1, ?2, 1, 'codex', 'default', NULL, 'quota.test',
                       'remaining', 'unknown', NULL, NULL, NULL, NULL)",
            params![scope_id, window_id],
        )
        .expect("insert quota definition");
}

fn insert_quota_sample(
    connection: &Connection,
    observation_id: &[u8],
    scope_id: &[u8],
    window_id: &str,
    used_ratio_ppm: i64,
) {
    connection
        .execute(
            "INSERT INTO quota_sample(
               observation_id, scope_id, window_id, definition_revision,
               observed_at_ms, fresh_until_ms, stale_after_ms,
               used_ratio_ppm, quality, source, confidence, reset_evidence
             ) VALUES (?1, ?2, ?3, 1, 100, 200, 300, ?4,
                       'authoritative', 'provider_local', 'high', 'none')",
            params![observation_id, scope_id, window_id, used_ratio_ppm],
        )
        .expect("insert quota sample");
}

fn insert_quota_epoch(
    connection: &Connection,
    epoch_id: &[u8],
    observation_id: &[u8],
    scope_id: &[u8],
    window_id: &str,
    maximum_used_ratio_ppm: i64,
) {
    connection
        .execute(
            "INSERT INTO quota_epoch_current(
               scope_id, window_id, epoch_id, epoch_definition_revision,
               definition_revision, first_observation_id, last_observation_id,
               first_observed_at_ms, last_observed_at_ms,
               maximum_used_ratio_ppm, maximum_used_ratio_observation_id,
               last_transition_sequence
             ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?4, 100, 100, ?5, ?4, 0)",
            params![
                scope_id,
                window_id,
                epoch_id,
                observation_id,
                maximum_used_ratio_ppm
            ],
        )
        .expect("insert quota epoch");
}

#[test]
fn fresh_schema_has_exact_strict_bounded_quota_objects() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-schema.sqlite3");
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
                "SELECT revision, retained_sample_count, retained_epoch_count,
                        retained_transition_count, last_published_at_ms
                 FROM quota_state WHERE singleton_id = 1",
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
            .expect("quota state"),
        (0, 0, 0, 0, None)
    );

    let mut observed_tables = connection
        .prepare(
            "SELECT name, strict FROM pragma_table_list
             WHERE schema = 'main' AND name LIKE 'quota_%'
             ORDER BY name",
        )
        .expect("quota table list")
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("quota table rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect quota tables");
    let mut expected_tables = QUOTA_TABLES.map(str::to_owned);
    observed_tables.sort();
    expected_tables.sort();
    assert_eq!(
        observed_tables
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
        expected_tables
    );
    assert!(
        observed_tables.iter().all(|(_, strict)| *strict == 1),
        "every quota table must be STRICT"
    );

    let definition_sql = table_sql(&connection, "quota_window_definition");
    for required in [
        "CHECK(length(scope_id) = 32)",
        "CHECK(revision > 0)",
        "presentation TEXT NOT NULL CHECK(presentation IN ('used','remaining','pace'))",
        "semantics TEXT NOT NULL CHECK(semantics IN ('fixed','rolling','credit','unknown'))",
        "CHECK((maximum_post_reset_used_ppm IS NULL AND minimum_post_reset_remaining_ppm IS NULL AND minimum_used_ratio_drop_ppm IS NULL) OR (semantics = 'fixed' AND (maximum_post_reset_used_ppm IS NOT NULL OR minimum_post_reset_remaining_ppm IS NOT NULL)))",
    ] {
        assert!(
            definition_sql.contains(required),
            "missing definition constraint: {required}"
        );
    }

    let sample_sql = table_sql(&connection, "quota_sample");
    for required in [
        "observation_id BLOB PRIMARY KEY NOT NULL CHECK(length(observation_id) = 32)",
        "CHECK(observed_at_ms > 0 AND observed_at_ms <= fresh_until_ms AND fresh_until_ms <= stale_after_ms)",
        "quality TEXT NOT NULL CHECK(quality IN ('authoritative','partial','conflict','unknown'))",
        "source TEXT NOT NULL CHECK(source IN ('provider_local','provider_official','local_reset_event','manual','unknown'))",
        "reset_evidence TEXT NOT NULL CHECK(reset_evidence IN ('none','explicit_provider','explicit_local','manual_or_banked'))",
        "FOREIGN KEY(scope_id, window_id, definition_revision) REFERENCES quota_window_definition(scope_id, window_id, revision)",
        "UNIQUE(scope_id, window_id, observation_id)",
        "UNIQUE(scope_id, window_id, definition_revision, observation_id)",
    ] {
        assert!(
            sample_sql.contains(required),
            "missing sample constraint: {required}"
        );
    }

    let current_epoch_sql = table_sql(&connection, "quota_epoch_current");
    for required in [
        "maximum_used_ratio_observation_id BLOB",
        "maximum_used_units_observation_id BLOB",
        "CHECK(epoch_definition_revision > 0 AND definition_revision >= epoch_definition_revision)",
        "UNIQUE(scope_id, window_id, definition_revision, epoch_id)",
        "FOREIGN KEY(scope_id, window_id, epoch_definition_revision, first_observation_id) REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id)",
        "FOREIGN KEY(scope_id, window_id, maximum_used_ratio_observation_id) REFERENCES quota_sample(scope_id, window_id, observation_id)",
    ] {
        assert!(
            current_epoch_sql.contains(required),
            "missing current epoch constraint: {required}"
        );
    }

    let transition_sql = table_sql(&connection, "quota_transition");
    for required in [
        "transition_id BLOB PRIMARY KEY NOT NULL CHECK(length(transition_id) = 32)",
        "kind TEXT NOT NULL CHECK(kind IN ('scheduled_reset','early_reset','manual_or_banked_reset','unknown_reset','allowance_changed'))",
        "detection_time_kind TEXT NOT NULL CHECK(detection_time_kind IN ('exact','interval'))",
        "maximum_used_ratio_observation_id BLOB",
        "maximum_used_units_observation_id BLOB",
        "FOREIGN KEY(scope_id, window_id, definition_revision, post_observation_id) REFERENCES quota_sample(scope_id, window_id, definition_revision, observation_id)",
        "allowance_old_unit_id IS NOT NULL",
        "allowance_new_unit_id IS NOT NULL",
        "allowance_change_kind = 'increased' AND allowance_old_unit_id = allowance_new_unit_id AND allowance_new_capacity_units > allowance_old_capacity_units",
    ] {
        assert!(
            transition_sql.contains(required),
            "missing transition constraint: {required}"
        );
    }

    let indexes = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'index' AND name LIKE 'quota_%'
             ORDER BY name",
        )
        .expect("quota indexes")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("index rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect indexes");
    assert_eq!(indexes, QUOTA_INDEXES);

    let triggers = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'trigger' AND name LIKE 'quota_%'
             ORDER BY name",
        )
        .expect("quota triggers")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("trigger rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect triggers");
    assert_eq!(triggers, QUOTA_TRIGGERS);

    for table in QUOTA_TABLES {
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
            ] {
                assert!(
                    !column.contains(forbidden),
                    "forbidden quota column {table}.{column}"
                );
            }
        }
    }
}

#[test]
fn exact_v9_migration_adds_empty_quota_state_without_touching_usage_or_prices() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-migration.sqlite3");
    drop(UsageStore::open(&path).expect("create base archive"));
    {
        let connection = raw_connection(&path);
        connection
            .execute(
                "UPDATE usage_archive_state
                 SET archive_generation = 7, dataset_generation = 11
                 WHERE singleton_id = 1",
                [],
            )
            .expect("archive fixture");
        connection
            .execute(
                "UPDATE usage_aggregate_state
                 SET expected_dataset_generation = 11,
                     active_aggregate_generation = 5
                 WHERE singleton_id = 1",
                [],
            )
            .expect("aggregate fixture");
        connection
            .execute(
                "INSERT INTO usage_price_time_rollup(
                   aggregate_generation, dataset_kind, bucket_width,
                   bucket_start_seconds, provider_id, profile_id, model, project_key,
                   service_tier, long_context, reported_state, event_count,
                   calculable_event_count, uncached_input_sum, cached_input_sum,
                   billable_output_sum, reported_cost_count, reported_cost_sum
                 ) VALUES (
                   5, 'current', 'minute', 0, 'codex', 'default', 'gpt-test', '',
                   'standard_assumed', 'no', 'missing', 1, 1, 1, 0, 1, 0, 0
                 )",
                [],
            )
            .expect("price fixture");
        strip_quota_schema_to_exact_v9(&connection);
    }

    let before = {
        let connection = raw_connection(&path);
        connection
            .query_row(
                "SELECT archive.archive_generation, archive.dataset_generation,
                        aggregate.expected_dataset_generation,
                        aggregate.active_aggregate_generation,
                        (SELECT count(*) FROM usage_event),
                        (SELECT count(*) FROM usage_price_time_rollup)
                 FROM usage_archive_state AS archive
                 JOIN usage_aggregate_state AS aggregate ON aggregate.singleton_id = 1
                 WHERE archive.singleton_id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .expect("pre-migration snapshot")
    };

    drop(UsageStore::open(&path).expect("migrate exact v9"));
    let connection = raw_connection(&path);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("user version"),
        13
    );
    let after = connection
        .query_row(
            "SELECT archive.archive_generation, archive.dataset_generation,
                    aggregate.expected_dataset_generation,
                    aggregate.active_aggregate_generation,
                    (SELECT count(*) FROM usage_event),
                    (SELECT count(*) FROM usage_price_time_rollup)
             FROM usage_archive_state AS archive
             JOIN usage_aggregate_state AS aggregate ON aggregate.singleton_id = 1
             WHERE archive.singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .expect("post-migration snapshot");
    assert_eq!(after, before);
    assert_eq!(
        connection
            .query_row(
                "SELECT revision, retained_sample_count, retained_epoch_count,
                        retained_transition_count
                 FROM quota_state WHERE singleton_id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .expect("empty quota state"),
        (0, 0, 0, 0)
    );
}

#[test]
fn weakened_quota_schema_is_rejected_on_reopen() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-malformed.sqlite3");
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
                   'CHECK(length(observation_id) = 32)',
                   'CHECK(length(observation_id) = 31)'
                 )
                 WHERE type = 'table' AND name = 'quota_sample'",
                [],
            )
            .expect("weaken quota schema");
        assert_eq!(changed, 1);
        connection
            .pragma_update(None, "writable_schema", "OFF")
            .expect("disable writable schema");
    }

    let error = match UsageStore::open(&path) {
        Ok(_) => panic!("weakened quota schema unexpectedly reopened"),
        Err(error) => error,
    };
    assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
}

#[test]
fn current_projection_rejects_cross_window_sample_and_epoch_links() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-cross-window.sqlite3");
    drop(UsageStore::open(&path).expect("create current schema"));
    let connection = raw_connection(&path);
    let scope_a = [1_u8; 32];
    let scope_b = [2_u8; 32];
    let observation_a = [3_u8; 32];
    let observation_b = [4_u8; 32];
    let epoch_a = [5_u8; 32];
    let epoch_b = [6_u8; 32];
    insert_quota_definition(&connection, &scope_a, "window-a");
    insert_quota_definition(&connection, &scope_b, "window-b");
    insert_quota_sample(&connection, &observation_a, &scope_a, "window-a", 100);
    insert_quota_sample(&connection, &observation_b, &scope_b, "window-b", 200);
    insert_quota_epoch(
        &connection,
        &epoch_a,
        &observation_a,
        &scope_a,
        "window-a",
        100,
    );
    insert_quota_epoch(
        &connection,
        &epoch_b,
        &observation_b,
        &scope_b,
        "window-b",
        200,
    );

    let insert_projection = |sample: &[u8], epoch: &[u8]| {
        connection.execute(
            "INSERT INTO quota_window_current(
               scope_id, window_id, definition_revision, sample_observation_id,
               epoch_id, observed_at_ms, fresh_until_ms, stale_after_ms,
               quality, source, confidence, last_transition_sequence
             ) VALUES (?1, 'window-a', 1, ?2, ?3, 100, 200, 300,
                       'authoritative', 'provider_local', 'high', 0)",
            params![scope_a.as_slice(), sample, epoch],
        )
    };

    assert!(
        insert_projection(&observation_b, &epoch_a).is_err(),
        "projection must not accept another window's sample"
    );
    assert!(
        insert_projection(&observation_a, &epoch_b).is_err(),
        "projection must not accept another window's epoch"
    );
}

#[test]
fn allowance_change_requires_complete_units_and_matching_direction() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-allowance.sqlite3");
    drop(UsageStore::open(&path).expect("create current schema"));
    let connection = raw_connection(&path);
    let scope = [7_u8; 32];
    let observation = [8_u8; 32];
    let epoch = [9_u8; 32];
    insert_quota_definition(&connection, &scope, "weekly");
    insert_quota_sample(&connection, &observation, &scope, "weekly", 500);

    let insert_transition = |transition_id: [u8; 32],
                             kind: &str,
                             old_unit_id: Option<&str>,
                             old_capacity: i64,
                             new_unit_id: Option<&str>,
                             new_capacity: i64| {
        connection.execute(
            "INSERT INTO quota_transition(
                   transition_id, scope_id, window_id, definition_revision,
                   sequence, kind, previous_epoch_id, current_epoch_id,
                   pre_observation_id, post_observation_id,
                   allowance_change_kind, allowance_old_unit_id,
                   allowance_old_capacity_units, allowance_new_unit_id,
                   allowance_new_capacity_units, source, confidence,
                   detection_time_kind, exact_at_ms
                 ) VALUES (
                   ?1, ?2, 'weekly', 1, 1, 'allowance_changed', ?3, ?3, ?4, ?4,
                   ?5, ?6, ?7, ?8, ?9, 'provider_local', 'high', 'exact', 100
                 )",
            params![
                transition_id.as_slice(),
                scope.as_slice(),
                epoch.as_slice(),
                observation.as_slice(),
                kind,
                old_unit_id,
                old_capacity,
                new_unit_id,
                new_capacity
            ],
        )
    };

    assert!(
        insert_transition([10_u8; 32], "increased", None, 100, None, 200).is_err(),
        "allowance facts must include both unit IDs"
    );
    assert!(
        insert_transition(
            [11_u8; 32],
            "increased",
            Some("requests"),
            200,
            Some("requests"),
            100,
        )
        .is_err(),
        "increased allowance must have a larger capacity"
    );
}
mod git_schema_v13 {
    include!("support/git_schema_v13.rs");
}
