use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    ArchiveGeneration, ArchivePublicationQuality, EXPECTED_SQLITE_VERSION, JournalMode,
    MAX_RESUME_BYTES, MAX_USAGE_EVENT_PAGE_SIZE, SourceKey, StoreErrorCode, StoredCheckpoint,
    StoredCheckpointParts, StoredVerification, USAGE_SCHEMA_VERSION, UsageStore,
};

const APPLICATION_TABLES: [&str; 28] = [
    "quota_epoch_current",
    "quota_epoch_history",
    "quota_sample",
    "quota_state",
    "quota_transition",
    "quota_window_current",
    "quota_window_definition",
    "usage_aggregate_state",
    "usage_archive_state",
    "usage_scan_set",
    "usage_source",
    "usage_generation",
    "usage_source_chunk",
    "usage_observation",
    "usage_event",
    "usage_scan",
    "usage_session_rollup",
    "usage_time_rollup",
    "usage_price_session_rollup",
    "usage_price_time_rollup",
    "usage_legacy_snapshot",
    "usage_legacy_event",
    "usage_replay_revision",
    "usage_replay_source",
    "usage_replay_session",
    "usage_replay_observation",
    "usage_replay_selection",
    "usage_replay_work",
];
const APPLICATION_TRIGGERS: [&str; 23] = [
    "quota_epoch_history_no_update",
    "quota_sample_no_update",
    "quota_state_no_delete",
    "quota_transition_no_update",
    "quota_window_definition_no_update",
    "usage_event_aggregate_session_after_delete",
    "usage_event_aggregate_session_after_insert",
    "usage_event_aggregate_session_after_update",
    "usage_event_aggregate_time_after_delete",
    "usage_event_aggregate_time_after_insert",
    "usage_event_aggregate_time_after_update",
    "usage_event_dataset_generation_after_delete",
    "usage_event_dataset_generation_after_insert",
    "usage_event_dataset_generation_after_update",
    "usage_event_price_session_after_delete",
    "usage_event_price_session_after_insert",
    "usage_event_price_session_after_update",
    "usage_event_price_time_after_delete",
    "usage_event_price_time_after_insert",
    "usage_event_price_time_after_update",
    "usage_legacy_event_no_delete",
    "usage_legacy_event_no_insert",
    "usage_legacy_event_no_update",
];
const FIXTURE_SOURCE_KEY: [u8; 32] = [7; 32];

fn checkpoint_parts() -> StoredCheckpointParts {
    StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([1; 32]),
        logical_identity: [2; 32],
        committed_offset: 100,
        scan_offset: 100,
        observed_file_length: 100,
        modified_time_ns: Some(123),
        anchor_start: 0,
        anchor_len: 100,
        anchor_sha256: [3; 32],
        resume: vec![4, 5].into_boxed_slice(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_aggregate_event(
    connection: &Connection,
    fingerprint_byte: u8,
    event_id: &str,
    timestamp_seconds: i64,
    model: &str,
    project_alias: Option<&str>,
    input_tokens: Option<i64>,
    cached_tokens: Option<i64>,
    fallback_model: i64,
    long_context: &str,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO usage_event(
           fingerprint, event_id, selected_file_key, selected_generation,
           selected_source_offset, provider_id, profile_id, session_id, source_id,
           timestamp_seconds, timestamp_nanos, model, project_alias,
           input_tokens, cached_tokens, output_tokens, reasoning_tokens, total_tokens,
           fallback_model, long_context, activity_read, activity_edit_write,
           activity_search, activity_git, activity_build_test, activity_web,
           activity_subagents, activity_terminal
         ) VALUES (
           ?1, ?2, ?3, 0, ?4, 'codex', 'default', 'session', 'source',
           ?5, 7, ?6, ?7, ?8, ?9, 2, 1,
           CASE WHEN ?8 IS NULL THEN NULL ELSE ?8 + coalesce(?9, 0) + 3 END,
           ?10, ?11, 1, 2, 3, 4, 5, 6, 7, 8
         )",
        params![
            [fingerprint_byte; 32].as_slice(),
            event_id,
            [42_u8; 32].as_slice(),
            i64::from(fingerprint_byte),
            timestamp_seconds,
            model,
            project_alias,
            input_tokens,
            cached_tokens,
            fallback_model,
            long_context,
        ],
    )
}

fn fresh_usage_connection(file_name: &str) -> (TempDir, Connection) {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join(file_name);
    drop(UsageStore::open(&path).expect("create usage schema"));
    let connection = Connection::open(path).expect("open usage schema");
    (directory, connection)
}

fn seed_usage_fixture(path: &Path, event_count: u32) {
    drop(UsageStore::open(path).expect("create usage schema"));
    seed_existing_usage_fixture(path, event_count, true);
}

fn seed_existing_usage_fixture(path: &Path, event_count: u32, provider_self_contained: bool) {
    let mut connection = Connection::open(path).expect("open usage fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable fixture foreign keys");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity
             ) VALUES (?1, 'codex', 'default', 'fixture', 'active', ?2, ?3)",
            params![
                FIXTURE_SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [1_u8; 32].as_slice()
            ],
        )
        .expect("fixture source");
    transaction
        .execute(
            "INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version,
               physical_identity, logical_identity, committed_offset, scan_offset,
               observed_file_length, modified_time_ns, anchor_start, anchor_len,
               anchor_sha256, resume_payload, discarding_oversized_line,
               incomplete_tail, verification_level
             ) VALUES (
               ?1, 0, 'current', 1, ?2, ?3, 100, 100, 100, 123, 0, 100,
               ?4, ?5, 0, 0, 'incremental'
             )",
            params![
                FIXTURE_SOURCE_KEY.as_slice(),
                [1_u8; 32].as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice(),
                [4_u8, 5_u8].as_slice()
            ],
        )
        .expect("fixture generation");
    transaction
        .execute(
            "UPDATE usage_source SET current_generation = 0 WHERE file_key = ?1",
            params![FIXTURE_SOURCE_KEY.as_slice()],
        )
        .expect("select fixture generation");

    for event_index in 0..event_count {
        let source_offset = i64::from(event_index);
        let mut fingerprint = [0_u8; 32];
        fingerprint[..4].copy_from_slice(&event_index.to_be_bytes());
        let event_id = if event_index < 2 {
            "event-short-id-collision".to_owned()
        } else {
            format!("event-{event_index:04}")
        };
        transaction
            .execute(
                "INSERT INTO usage_observation(
                   file_key, generation, source_offset, fingerprint, event_id,
                   profile_id, session_id, source_id, timestamp_seconds,
                   timestamp_nanos, model, raw_model, input_tokens, cached_tokens,
                   output_tokens, reasoning_tokens, total_tokens, fallback_model,
                   long_context, service_tier, project_alias, originator,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, 0, ?2, ?3, ?4, 'default', 'session', 'fixture', ?2,
                   0, 'gpt-test', NULL, 1, 2, 3, 4, 10, 0, 'no', NULL, NULL,
                   NULL, 0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    FIXTURE_SOURCE_KEY.as_slice(),
                    source_offset,
                    fingerprint.as_slice(),
                    event_id
                ],
            )
            .expect("fixture observation");
        let (event_sql, event_parameters) = if provider_self_contained {
            (
                "INSERT INTO usage_event(
                   fingerprint, event_id, selected_file_key, selected_generation,
                   selected_source_offset, provider_id, profile_id, session_id, source_id,
                   timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
                   cached_tokens, output_tokens, reasoning_tokens, total_tokens,
                   fallback_model, long_context, service_tier, project_alias, originator,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 'codex', 'default', 'session', 'fixture', ?4,
                   0, 'gpt-test', NULL, 1, 2, 3, 4, 10, 0, 'no', NULL, NULL,
                   NULL, 0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    fingerprint.as_slice(),
                    event_id.as_str(),
                    FIXTURE_SOURCE_KEY.as_slice(),
                    source_offset
                ],
            )
        } else {
            (
                "INSERT INTO usage_event(
                   fingerprint, event_id, selected_file_key, selected_generation,
                   selected_source_offset, profile_id, session_id, source_id,
                   timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
                   cached_tokens, output_tokens, reasoning_tokens, total_tokens,
                   fallback_model, long_context, service_tier, project_alias, originator,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 'default', 'session', 'fixture', ?4,
                   0, 'gpt-test', NULL, 1, 2, 3, 4, 10, 0, 'no', NULL, NULL,
                   NULL, 0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    fingerprint.as_slice(),
                    event_id.as_str(),
                    FIXTURE_SOURCE_KEY.as_slice(),
                    source_offset
                ],
            )
        };
        transaction
            .execute(event_sql, event_parameters)
            .expect("fixture canonical event");
    }
    transaction.commit().expect("commit usage fixture");
}

fn create_v1_fixture(path: &Path, event_count: u32) {
    let connection = Connection::open(path).expect("create v1 fixture");
    connection
        .execute_batch(include_str!("fixtures/usage_v1.sql"))
        .expect("create exact v1 schema");
    drop(connection);
    seed_existing_usage_fixture(path, event_count, false);
}

fn rewrite_table_schema(path: &Path, table: &str, from: &str, to: &str) {
    let connection = Connection::open(path).expect("open schema rewrite fixture");
    connection
        .pragma_update(None, "writable_schema", "ON")
        .expect("enable fixture schema rewrite");
    let changed = connection
        .execute(
            "UPDATE sqlite_schema SET sql = replace(sql, ?2, ?3)
             WHERE type = 'table' AND name = ?1 AND instr(sql, ?2) > 0",
            params![table, from, to],
        )
        .expect("rewrite fixture table schema");
    assert_eq!(changed, 1, "fixture rewrite must change exactly one table");
    connection
        .pragma_update(None, "writable_schema", "OFF")
        .expect("disable fixture schema rewrite");
}

fn table_sql(path: &Path, table: &str) -> String {
    let connection = Connection::open(path).expect("open schema inspection fixture");
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .expect("read table SQL")
}

#[test]
fn file_store_enforces_exact_runtime_policy_and_reopens() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("usage-private.sqlite3");
    let key = SourceKey::from_slice(&[7; 32]).expect("source key");
    {
        let store = UsageStore::open(&path).expect("first usage store open");
        assert_eq!(
            store.sqlite_version().expect("SQLite version"),
            EXPECTED_SQLITE_VERSION
        );
        let policy = store.runtime_policy().expect("runtime policy");
        assert_eq!(policy.journal_mode(), JournalMode::Wal);
        assert_eq!(policy.synchronous(), 2);
        assert!(policy.foreign_keys());
        assert_eq!(policy.busy_timeout_ms(), 250);
        assert_eq!(policy.wal_autocheckpoint_pages(), 1_000);
        assert_eq!(policy.journal_size_limit_bytes(), 16 * 1024 * 1024);
        assert_eq!(policy.cache_size_kib(), 8 * 1024);
        assert_eq!(policy.temp_store(), 1);
        assert_eq!(policy.mmap_size_bytes(), 0);
        assert_eq!(store.counts().expect("empty counts").total(), 0);
        assert!(
            store
                .generation_snapshot(key)
                .expect("empty snapshot")
                .is_none()
        );
        assert!(
            store
                .event_page_before(None, usize::MAX)
                .expect("empty event page")
                .is_empty()
        );
        assert_eq!(MAX_USAGE_EVENT_PAGE_SIZE, 256);
    }
    let reopened = UsageStore::open(&path).expect("reopen usage store");
    assert_eq!(reopened.counts().expect("reopened counts").total(), 0);
}

#[test]
fn in_memory_store_uses_supported_equivalent_policy() {
    let store = UsageStore::in_memory().expect("in-memory usage store");
    let policy = store.runtime_policy().expect("runtime policy");
    assert_eq!(policy.journal_mode(), JournalMode::Memory);
    assert_eq!(policy.synchronous(), 2);
    assert!(policy.foreign_keys());
    assert_eq!(policy.busy_timeout_ms(), 250);
    assert_eq!(policy.cache_size_kib(), 8 * 1024);
    assert_eq!(policy.temp_store(), 1);
    assert_eq!(policy.mmap_size_bytes(), 0);
}

#[test]
fn schema_is_strict_path_free_and_has_exact_usage_tables() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("schema-private.sqlite3");
    drop(UsageStore::open(&path).expect("create schema"));
    let connection = Connection::open(&path).expect("inspect schema");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user version");
    assert_eq!(USAGE_SCHEMA_VERSION, 12);
    assert_eq!(version, 12);
    assert_eq!(version, USAGE_SCHEMA_VERSION);

    let publication_sql = table_sql(&path, "usage_archive_state")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1)",
        "archive_generation INTEGER NOT NULL CHECK(archive_generation >= 0)",
        "dataset_generation INTEGER NOT NULL DEFAULT 0 CHECK(dataset_generation >= 0)",
        "incremental_state TEXT NOT NULL CHECK(incremental_state IN ('empty','complete','partial','recovery_pending'))",
        "FOREIGN KEY(current_revision_id) REFERENCES usage_replay_revision(revision_id)",
        "FOREIGN KEY(latest_complete_scan_set_id) REFERENCES usage_scan_set(scan_set_id)",
    ] {
        assert!(
            publication_sql.contains(required),
            "missing archive-publication contract: {required}"
        );
    }

    let scan_set_sql = table_sql(&path, "usage_scan_set")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "completion_state TEXT NOT NULL CHECK(completion_state IN ('running','complete','partial','cancelled','failed','timed_out'))",
        "expected_scope_count INTEGER NOT NULL CHECK(expected_scope_count BETWEEN 1 AND 256)",
        "CHECK((completion_state = 'running' AND completed_at_ms IS NULL) OR (completion_state <> 'running' AND completed_at_ms IS NOT NULL))",
    ] {
        assert!(
            scan_set_sql.contains(required),
            "missing scan-set contract: {required}"
        );
    }

    let scan_sql = table_sql(&path, "usage_scan")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "scan_set_id INTEGER NOT NULL CHECK(scan_set_id >= 0)",
        "provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64)",
        "UNIQUE(scan_set_id, provider_id, profile_id)",
        "FOREIGN KEY(scan_set_id) REFERENCES usage_scan_set(scan_set_id)",
    ] {
        assert!(
            scan_sql.contains(required),
            "missing scoped scan contract: {required}"
        );
    }

    let revision_sql = table_sql(&path, "usage_replay_revision");
    assert!(
        revision_sql
            .contains("expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 0)")
    );
    assert!(!revision_sql.contains("expected_source_count BETWEEN 1 AND 256"));

    let event_sql = table_sql(&path, "usage_event");
    let normalized_event_sql = event_sql.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "projection_revision_id INTEGER",
        "origin_revision_id INTEGER",
        "retained INTEGER NOT NULL CHECK(retained IN (0,1))",
        "provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 64)",
        "FOREIGN KEY(projection_revision_id) REFERENCES usage_replay_revision(revision_id) DEFERRABLE INITIALLY DEFERRED",
    ] {
        assert!(
            normalized_event_sql.contains(required),
            "missing usage_event contract: {required}"
        );
    }
    assert!(
        !normalized_event_sql.contains("REFERENCES usage_observation"),
        "canonical projection must not retain a foreign key to a deletable generation"
    );

    let aggregate_state: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT state, expected_dataset_generation,
                    current_event_count, legacy_event_count
             FROM usage_aggregate_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("fresh aggregate state");
    assert_eq!(aggregate_state, ("ready".to_owned(), 0, 0, 0));

    let mut table_list = connection
        .prepare("PRAGMA table_list")
        .expect("prepare table list");
    let rows = table_list
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })
        .expect("query table list");
    let mut observed = Vec::new();
    for row in rows {
        let (name, strict) = row.expect("table row");
        if APPLICATION_TABLES.contains(&name.as_str()) {
            assert_eq!(strict, 1, "{name} must be STRICT");
            observed.push(name);
        }
    }
    observed.sort();
    let mut expected = APPLICATION_TABLES.map(str::to_owned);
    expected.sort();
    assert_eq!(observed, expected);

    let mut trigger_statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'trigger'
               AND (name LIKE 'quota_%' OR name LIKE 'usage_%')
             ORDER BY name",
        )
        .expect("prepare trigger list");
    let trigger_rows = trigger_statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query trigger list");
    let triggers = trigger_rows
        .collect::<Result<Vec<_>, _>>()
        .expect("collect trigger names");
    assert_eq!(triggers, APPLICATION_TRIGGERS);

    for table in APPLICATION_TABLES {
        let pragma = format!("PRAGMA table_info({table})");
        let mut columns = connection.prepare(&pragma).expect("prepare table info");
        let names = columns
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query columns");
        for name in names {
            let name = name.expect("column name").to_ascii_lowercase();
            for forbidden in [
                "path",
                "prompt",
                "response",
                "reasoning_text",
                "tool_argument",
                "command",
                "command_output",
                "raw_json",
                "credential",
                "token_value",
            ] {
                assert!(!name.contains(forbidden), "forbidden column {table}.{name}");
            }
        }
    }

    let foreign_key_failures: i64 = connection
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign key check");
    assert_eq!(foreign_key_failures, 0);
    let partial_indexes: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name IN ('usage_generation_one_current', 'usage_generation_one_staging', 'usage_scan_set_one_running', 'usage_scan_one_running_scope') AND sql LIKE '% WHERE %'",
            [],
            |row| row.get(0),
        )
        .expect("partial indexes");
    assert_eq!(partial_indexes, 4);

    let source_schema: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'usage_source'",
            [],
            |row| row.get(0),
        )
        .expect("usage_source schema");
    assert!(
        !source_schema
            .to_ascii_uppercase()
            .contains("ON DELETE SET NULL"),
        "generation deletion must not rewrite source identity columns"
    );
}

#[test]
fn dataset_generation_changes_only_with_canonical_event_mutations() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("dataset-generation-private.sqlite3");
    drop(UsageStore::open(&path).expect("create schema"));
    let mut connection = Connection::open(&path).expect("open schema");

    let generation = |connection: &Connection| -> i64 {
        connection
            .query_row(
                "SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("dataset generation")
    };
    assert_eq!(generation(&connection), 0);

    connection
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) VALUES (?1, 'event', ?2, 0, 0, 'unknown', 'default', 'session', 'source',
                       1, 0, 'model', 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0)",
            params![[1_u8; 32].as_slice(), [2_u8; 32].as_slice()],
        )
        .expect("insert canonical event");
    assert_eq!(generation(&connection), 1);

    connection
        .execute(
            "UPDATE usage_archive_state SET archive_generation = 1 WHERE singleton_id = 1",
            [],
        )
        .expect("freshness-only publication");
    assert_eq!(generation(&connection), 1);

    connection
        .execute(
            "UPDATE usage_event SET timestamp_seconds = 2 WHERE event_id = 'event'",
            [],
        )
        .expect("update canonical event");
    assert_eq!(generation(&connection), 2);

    let transaction = connection.transaction().expect("begin rollback proof");
    transaction
        .execute("DELETE FROM usage_event WHERE event_id = 'event'", [])
        .expect("delete inside transaction");
    assert_eq!(generation(&transaction), 3);
    transaction.rollback().expect("rollback event mutation");
    assert_eq!(generation(&connection), 2);

    connection
        .execute("DELETE FROM usage_event WHERE event_id = 'event'", [])
        .expect("delete canonical event");
    assert_eq!(generation(&connection), 3);
}

#[test]
fn ready_aggregates_track_insert_update_delete_and_availability_exactly() {
    let (_directory, connection) = fresh_usage_connection("aggregate-mutations.sqlite3");
    insert_aggregate_event(
        &connection,
        1,
        "first",
        61,
        "model-a",
        None,
        Some(10),
        None,
        1,
        "yes",
    )
    .expect("first aggregate event");
    insert_aggregate_event(
        &connection,
        2,
        "second",
        121,
        "model-b",
        Some("project-b"),
        None,
        Some(3),
        0,
        "unavailable",
    )
    .expect("second aggregate event");

    let hour_all: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT event_count, input_known_count, input_known_sum,
                    cached_known_count, cached_known_sum, fallback_model_count,
                    long_context_yes_count, long_context_unavailable_count,
                    activity_terminal
             FROM usage_time_rollup
             WHERE dataset_kind = 'current' AND bucket_width = 'hour'
               AND bucket_start_seconds = 0 AND provider_id = 'codex'
               AND profile_id = 'default' AND dimension_kind = 'all'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .expect("hour all rollup");
    assert_eq!(hour_all, (2, 1, 10, 1, 3, 1, 1, 1, 16));
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM usage_time_rollup", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("time rollup count"),
        11
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM usage_session_rollup", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("session rollup count"),
        5
    );

    connection
        .execute(
            "UPDATE usage_event
             SET timestamp_seconds = 3661, model = 'model-b',
                 project_alias = 'project-b', input_tokens = 20,
                 cached_tokens = 4, total_tokens = 27, fallback_model = 0,
                 long_context = 'no'
             WHERE event_id = 'first'",
            [],
        )
        .expect("move aggregate contribution");
    let session_all: (i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT event_count, first_timestamp_seconds, last_timestamp_seconds,
                    input_known_count, input_known_sum, cached_known_sum
             FROM usage_session_rollup
             WHERE dataset_kind = 'current' AND provider_id = 'codex'
               AND profile_id = 'default' AND session_id = 'session'
               AND dimension_kind = 'all'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("updated session rollup");
    assert_eq!(session_all, (2, 121, 3661, 1, 20, 7));
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM usage_session_rollup
                 WHERE dimension_kind = 'model' AND dimension_value = 'model-a'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("removed old model dimension"),
        0
    );

    connection
        .execute("DELETE FROM usage_event WHERE event_id = 'first'", [])
        .expect("delete moved event");
    let remaining_session: (i64, i64, i64) = connection
        .query_row(
            "SELECT event_count, first_timestamp_seconds, last_timestamp_seconds
             FROM usage_session_rollup
             WHERE dataset_kind = 'current' AND dimension_kind = 'all'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("remaining session");
    assert_eq!(remaining_session, (1, 121, 121));

    connection
        .execute("DELETE FROM usage_event WHERE event_id = 'second'", [])
        .expect("delete final event");
    assert_eq!(
        connection
            .query_row(
                "SELECT (SELECT count(*) FROM usage_time_rollup)
                      + (SELECT count(*) FROM usage_session_rollup)",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("empty aggregate rows"),
        0
    );
    let state: (String, i64, i64) = connection
        .query_row(
            "SELECT state, expected_dataset_generation, current_event_count
             FROM usage_aggregate_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("final aggregate state");
    assert_eq!(state, ("ready".to_owned(), 5, 0));
}

#[test]
fn unavailable_aggregates_never_publish_partial_rows() {
    let (_directory, connection) = fresh_usage_connection("aggregate-unavailable.sqlite3");
    connection
        .execute(
            "UPDATE usage_aggregate_state
             SET state = 'rebuild_required', rebuild_total_events = 0
             WHERE singleton_id = 1",
            [],
        )
        .expect("require rebuild");

    insert_aggregate_event(
        &connection,
        3,
        "pending",
        1,
        "model",
        None,
        Some(1),
        None,
        0,
        "no",
    )
    .expect("event while aggregate unavailable");

    let state: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT state, expected_dataset_generation,
                    current_event_count, rebuild_total_events
             FROM usage_aggregate_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("pending aggregate state");
    assert_eq!(state, ("rebuild_required".to_owned(), 1, 1, 1));
    assert_eq!(
        connection
            .query_row(
                "SELECT (SELECT count(*) FROM usage_time_rollup)
                      + (SELECT count(*) FROM usage_session_rollup)",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("no partial rollups"),
        0
    );
}

#[test]
fn aggregate_overflow_rolls_back_event_generation_state_and_rollups() {
    let (_directory, connection) = fresh_usage_connection("aggregate-overflow.sqlite3");
    insert_aggregate_event(
        &connection,
        4,
        "base",
        1,
        "model",
        None,
        Some(1),
        None,
        0,
        "no",
    )
    .expect("base aggregate event");
    connection
        .execute(
            "UPDATE usage_time_rollup SET input_known_sum = ?1
             WHERE bucket_width = 'hour' AND dimension_kind = 'all'",
            [i64::MAX],
        )
        .expect("seed aggregate overflow boundary");
    let before: (i64, i64, i64) = connection
        .query_row(
            "SELECT archive.dataset_generation, aggregate.current_event_count,
                    (SELECT count(*) FROM usage_time_rollup)
             FROM usage_archive_state AS archive
             JOIN usage_aggregate_state AS aggregate ON aggregate.singleton_id = 1
             WHERE archive.singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("overflow baseline");

    let result = insert_aggregate_event(
        &connection,
        5,
        "overflow",
        2,
        "model",
        None,
        Some(1),
        None,
        0,
        "no",
    );
    assert!(
        result.is_err(),
        "aggregate integer overflow must fail closed"
    );
    let after: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT archive.dataset_generation, aggregate.current_event_count,
                    (SELECT count(*) FROM usage_time_rollup),
                    (SELECT count(*) FROM usage_event)
             FROM usage_archive_state AS archive
             JOIN usage_aggregate_state AS aggregate ON aggregate.singleton_id = 1
             WHERE archive.singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("overflow rollback state");
    assert_eq!((after.0, after.1, after.2), before);
    assert_eq!(after.3, 1);
}

#[test]
fn dataset_generation_overflow_aborts_the_event_mutation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("dataset-generation-overflow-private.sqlite3");
    drop(UsageStore::open(&path).expect("create schema"));
    let connection = Connection::open(&path).expect("open schema");
    connection
        .execute(
            "UPDATE usage_archive_state SET dataset_generation = ?1 WHERE singleton_id = 1",
            [i64::MAX],
        )
        .expect("seed exhausted generation");

    let result = connection.execute(
        "INSERT INTO usage_event(
           fingerprint, event_id, selected_file_key, selected_generation,
           selected_source_offset, provider_id, profile_id, session_id, source_id,
           timestamp_seconds, timestamp_nanos, model, fallback_model, long_context,
           activity_read, activity_edit_write, activity_search, activity_git,
           activity_build_test, activity_web, activity_subagents, activity_terminal
         ) VALUES (?1, 'overflow', ?2, 0, 0, 'unknown', 'default', 'session', 'source',
                   1, 0, 'model', 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0)",
        params![[3_u8; 32].as_slice(), [4_u8; 32].as_slice()],
    );
    assert!(result.is_err(), "overflow must fail closed");
    let event_count: i64 = connection
        .query_row("SELECT count(*) FROM usage_event", [], |row| row.get(0))
        .expect("event count");
    assert_eq!(event_count, 0);
}

#[test]
fn missing_dataset_state_aborts_the_event_mutation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("missing-dataset-state-private.sqlite3");
    drop(UsageStore::open(&path).expect("create schema"));
    let connection = Connection::open(&path).expect("open schema");
    connection
        .execute("DELETE FROM usage_archive_state WHERE singleton_id = 1", [])
        .expect("remove state to simulate corruption");

    let result = connection.execute(
        "INSERT INTO usage_event(
           fingerprint, event_id, selected_file_key, selected_generation,
           selected_source_offset, provider_id, profile_id, session_id, source_id,
           timestamp_seconds, timestamp_nanos, model, fallback_model, long_context,
           activity_read, activity_edit_write, activity_search, activity_git,
           activity_build_test, activity_web, activity_subagents, activity_terminal
         ) VALUES (?1, 'missing-state', ?2, 0, 0, 'unknown', 'default', 'session', 'source',
                   1, 0, 'model', 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0)",
        params![[5_u8; 32].as_slice(), [6_u8; 32].as_slice()],
    );
    assert!(result.is_err(), "missing state must fail closed");
    let event_count: i64 = connection
        .query_row("SELECT count(*) FROM usage_event", [], |row| row.get(0))
        .expect("event count");
    assert_eq!(event_count, 0);
}

#[test]
fn missing_aggregate_state_aborts_event_and_dataset_generation() {
    let (_directory, connection) = fresh_usage_connection("missing-aggregate-state.sqlite3");
    connection
        .execute(
            "DELETE FROM usage_aggregate_state WHERE singleton_id = 1",
            [],
        )
        .expect("remove aggregate state to simulate corruption");

    let result = insert_aggregate_event(
        &connection,
        6,
        "missing-aggregate-state",
        1,
        "model",
        None,
        Some(1),
        None,
        0,
        "no",
    );
    assert!(result.is_err(), "missing aggregate state must fail closed");
    assert_eq!(
        connection
            .query_row(
                "SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("unchanged dataset generation"),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM usage_event", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("no event committed"),
        0
    );
}

#[test]
fn missing_published_rollup_aborts_event_mutation_and_all_trigger_side_effects() {
    let (_directory, connection) = fresh_usage_connection("missing-published-rollup.sqlite3");
    insert_aggregate_event(
        &connection,
        7,
        "published",
        1,
        "model",
        None,
        Some(1),
        None,
        0,
        "no",
    )
    .expect("published aggregate event");
    connection
        .execute(
            "DELETE FROM usage_time_rollup
             WHERE bucket_width = 'minute' AND dimension_kind = 'model'",
            [],
        )
        .expect("simulate missing published rollup");
    let before: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT archive.dataset_generation, aggregate.current_event_count,
                    (SELECT count(*) FROM usage_time_rollup),
                    (SELECT count(*) FROM usage_session_rollup)
             FROM usage_archive_state AS archive
             JOIN usage_aggregate_state AS aggregate ON aggregate.singleton_id = 1
             WHERE archive.singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("corrupt aggregate baseline");

    let result = connection.execute("DELETE FROM usage_event WHERE event_id = 'published'", []);
    assert!(result.is_err(), "missing published rollup must fail closed");
    let after: (i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT archive.dataset_generation, aggregate.current_event_count,
                    (SELECT count(*) FROM usage_time_rollup),
                    (SELECT count(*) FROM usage_session_rollup),
                    (SELECT count(*) FROM usage_event)
             FROM usage_archive_state AS archive
             JOIN usage_aggregate_state AS aggregate ON aggregate.singleton_id = 1
             WHERE archive.singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("rolled-back aggregate state");
    assert_eq!((after.0, after.1, after.2, after.3), before);
    assert_eq!(after.4, 1);
}

#[test]
fn fresh_archive_has_one_empty_generation_zero_publication() {
    let store = UsageStore::in_memory().expect("in-memory usage store");
    let publication = store
        .archive_publication()
        .expect("read archive publication");
    assert_eq!(publication.generation(), ArchiveGeneration::new(0).unwrap());
    assert_eq!(publication.current_revision(), None);
    assert_eq!(publication.latest_complete_scan_set(), None);
    assert_eq!(publication.quality(), ArchivePublicationQuality::Empty);
}

#[test]
fn exact_v1_migration_preserves_an_immutable_legacy_snapshot() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("v1-migration-private.sqlite3");
    create_v1_fixture(&path, 2);

    let before = Connection::open(&path).expect("inspect v1 fixture");
    let before_events: Vec<(Vec<u8>, String)> = before
        .prepare("SELECT fingerprint, event_id FROM usage_event ORDER BY fingerprint")
        .expect("prepare v1 event read")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query v1 events")
        .collect::<Result<_, _>>()
        .expect("collect v1 events");
    drop(before);

    drop(UsageStore::open(&path).expect("migrate exact v1 archive"));
    let connection = Connection::open(&path).expect("inspect migrated archive");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("migrated version");
    assert_eq!(version, USAGE_SCHEMA_VERSION);
    let snapshot: (i64, String, i64) = connection
        .query_row(
            "SELECT source_schema_version, quality_state, event_count
             FROM usage_legacy_snapshot WHERE snapshot_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("legacy snapshot metadata");
    assert_eq!(snapshot, (1, "legacy_unverified".to_owned(), 2));
    let legacy_events: Vec<(Vec<u8>, String)> = connection
        .prepare(
            "SELECT fingerprint, event_id
             FROM usage_legacy_event WHERE snapshot_id = 1 ORDER BY fingerprint",
        )
        .expect("prepare legacy event read")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query legacy events")
        .collect::<Result<_, _>>()
        .expect("collect legacy events");
    assert_eq!(legacy_events, before_events);
    let live_event_count: i64 = connection
        .query_row("SELECT count(*) FROM usage_event", [], |row| row.get(0))
        .expect("preserved live event count");
    assert_eq!(live_event_count, 2);

    for statement in [
        "INSERT INTO usage_legacy_event DEFAULT VALUES",
        "UPDATE usage_legacy_event SET event_id = event_id WHERE snapshot_id = 1",
        "DELETE FROM usage_legacy_event WHERE snapshot_id = 1",
    ] {
        let error = connection
            .execute(statement, [])
            .expect_err("legacy event mutation must fail");
        assert!(
            error.to_string().contains("immutable legacy snapshot"),
            "immutability trigger must reject mutation"
        );
    }
    drop(connection);

    let reopened = UsageStore::open(&path).expect("reopen migrated archive");
    assert_eq!(
        reopened
            .counts()
            .expect("reopened counts")
            .canonical_events(),
        2
    );
}

#[test]
fn malformed_v1_rolls_back_without_creating_v2_objects() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("malformed-v1-private.sqlite3");
    create_v1_fixture(&path, 1);
    let connection = Connection::open(&path).expect("damage v1 fixture");
    connection
        .execute("DROP INDEX usage_observation_fingerprint", [])
        .expect("drop required v1 index");
    drop(connection);

    let error = UsageStore::open(&path).expect_err("malformed v1 must fail closed");
    assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);

    let connection = Connection::open(&path).expect("inspect failed migration");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("rolled-back version");
    assert_eq!(version, 1);
    let replay_objects: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE name IN (
               'usage_legacy_snapshot', 'usage_legacy_event',
               'usage_replay_revision', 'usage_replay_source',
               'usage_replay_session', 'usage_replay_observation',
               'usage_replay_selection', 'usage_replay_work'
             )",
            [],
            |row| row.get(0),
        )
        .expect("count rolled-back v2 objects");
    assert_eq!(replay_objects, 0);
}

#[test]
fn v1_with_weakened_deferred_foreign_key_rolls_back() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("weakened-v1-fk-private.sqlite3");
    create_v1_fixture(&path, 1);
    rewrite_table_schema(
        &path,
        "usage_event",
        "DEFERRABLE INITIALLY DEFERRED",
        "NOT DEFERRABLE INITIALLY IMMEDIATE",
    );

    let error = UsageStore::open(&path).expect_err("weakened v1 foreign key must fail");
    assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
    let connection = Connection::open(&path).expect("inspect rejected v1 archive");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("preserved v1 version");
    assert_eq!(version, 1);
    let legacy_table_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name = 'usage_legacy_snapshot'",
            [],
            |row| row.get(0),
        )
        .expect("count rejected migration tables");
    assert_eq!(legacy_table_count, 0);
}

#[test]
fn current_v5_with_weakened_constraint_fails_closed() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("weakened-v5-check-private.sqlite3");
    drop(UsageStore::open(&path).expect("create valid v5 schema"));
    rewrite_table_schema(
        &path,
        "usage_replay_revision",
        "expected_source_count >= 0",
        "expected_source_count >= -1",
    );

    let error = UsageStore::open(&path).expect_err("weakened v5 constraint must fail");
    assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
}

#[test]
fn legacy_snapshot_count_tampering_fails_closed_on_reopen() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("legacy-count-tamper-private.sqlite3");
    create_v1_fixture(&path, 2);
    drop(UsageStore::open(&path).expect("migrate v1 fixture"));
    let connection = Connection::open(&path).expect("tamper legacy metadata");
    connection
        .execute(
            "UPDATE usage_legacy_snapshot SET event_count = 99 WHERE snapshot_id = 1",
            [],
        )
        .expect("change legacy event count");
    drop(connection);

    let error = UsageStore::open(&path).expect_err("legacy count mismatch must fail");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
}

#[test]
fn schema_rejects_invalid_keys_kinds_and_multibyte_byte_overflow() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("strict-values-private.sqlite3");
    drop(UsageStore::open(&path).expect("create strict usage schema"));
    let connection = Connection::open(&path).expect("open strict schema fixture");
    let invalid_key = connection.execute(
        "INSERT INTO usage_source(
           file_key, provider_id, profile_id, source_id, source_kind, logical_identity
         ) VALUES (?1, 'codex', 'default', 'fixture', 'active', ?2)",
        params![[0_u8; 31].as_slice(), [2_u8; 32].as_slice()],
    );
    assert!(invalid_key.is_err());

    let invalid_kind = connection.execute(
        "INSERT INTO usage_source(
           file_key, provider_id, profile_id, source_id, source_kind, logical_identity
         ) VALUES (?1, 'codex', 'default', 'fixture', 'unknown', ?2)",
        params![[8_u8; 32].as_slice(), [2_u8; 32].as_slice()],
    );
    assert!(invalid_kind.is_err());

    let oversized_utf8 = "é".repeat(33);
    let invalid_provider = connection.execute(
        "INSERT INTO usage_source(
           file_key, provider_id, profile_id, source_id, source_kind, logical_identity
         ) VALUES (?1, ?2, 'default', 'fixture', 'active', ?3)",
        params![[9_u8; 32].as_slice(), oversized_utf8, [2_u8; 32].as_slice()],
    );
    assert!(invalid_provider.is_err());
}

#[test]
fn bounded_reads_return_checkpoint_and_complete_keyset_pages() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("bounded-read-private.sqlite3");
    seed_usage_fixture(&path, 300);
    let store = UsageStore::open(&path).expect("open seeded usage store");

    let counts = store.counts().expect("usage counts");
    assert_eq!(counts.sources(), 1);
    assert_eq!(counts.generations(), 1);
    assert_eq!(counts.observations(), 300);
    assert_eq!(counts.canonical_events(), 300);
    assert_eq!(counts.chunks(), 0);
    assert_eq!(counts.scans(), 0);

    let key = SourceKey::from_bytes(FIXTURE_SOURCE_KEY);
    let snapshot = store
        .generation_snapshot(key)
        .expect("generation snapshot")
        .expect("current generation");
    assert_eq!(snapshot.source_key(), key);
    assert_eq!(snapshot.generation(), 0);
    assert_eq!(snapshot.checkpoint().committed_offset(), 100);
    assert_eq!(snapshot.checkpoint().resume(), [4, 5]);

    let first = store
        .event_page_before(None, usize::MAX)
        .expect("first event page");
    assert_eq!(first.len(), MAX_USAGE_EVENT_PAGE_SIZE);
    assert_eq!(first.first().expect("first event").timestamp_seconds(), 299);
    assert_eq!(
        first
            .last()
            .expect("last first-page event")
            .timestamp_seconds(),
        44
    );

    let second = store
        .event_page_before(
            Some(first.last().expect("first page cursor").cursor()),
            usize::MAX,
        )
        .expect("second event page");
    assert_eq!(second.len(), 44);
    assert_eq!(
        second
            .first()
            .expect("second-page first event")
            .timestamp_seconds(),
        43
    );
    assert_eq!(second.last().expect("final event").timestamp_seconds(), 0);
    assert!(
        store
            .event_page_before(
                Some(second.last().expect("final cursor").cursor()),
                usize::MAX
            )
            .expect("terminal event page")
            .is_empty()
    );
    assert_eq!(
        store
            .event_page_before(None, 0)
            .expect("minimum bounded page")
            .len(),
        1
    );
}

#[test]
fn malformed_current_schema_fails_closed() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("malformed-schema-private.sqlite3");
    let connection = Connection::open(&path).expect("create malformed schema fixture");
    connection
        .execute("CREATE TABLE usage_scan(scan_id INTEGER) STRICT", [])
        .expect("create malformed current table");
    connection
        .pragma_update(None, "user_version", USAGE_SCHEMA_VERSION)
        .expect("mark malformed schema current");
    drop(connection);

    let error = UsageStore::open(&path).expect_err("malformed current schema must fail");
    assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
    let text = format!("{error:?} {error}");
    assert!(!text.contains(directory.path().to_string_lossy().as_ref()));
    assert!(!text.contains("malformed-schema-private.sqlite3"));

    let connection = Connection::open(&path).expect("inspect rolled-back migration");
    let usage_tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name LIKE 'usage_%'",
            [],
            |row| row.get(0),
        )
        .expect("count rolled-back usage tables");
    assert_eq!(
        usage_tables, 1,
        "failed validation must not partially repair the schema"
    );
}

#[test]
fn newer_schema_fails_closed_without_exposing_the_path() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("newer-schema-private.sqlite3");
    let connection = Connection::open(&path).expect("create newer schema fixture");
    connection
        .pragma_update(None, "user_version", USAGE_SCHEMA_VERSION + 1)
        .expect("set newer schema");
    drop(connection);

    let error = UsageStore::open(&path).expect_err("newer schema must fail");
    assert_eq!(error.code(), StoreErrorCode::SchemaTooNew);
    let text = format!("{error:?} {error}");
    assert!(!text.contains(directory.path().to_string_lossy().as_ref()));
    assert!(!text.contains("newer-schema-private.sqlite3"));
}

#[test]
fn malformed_current_index_fails_closed() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("malformed-index-private.sqlite3");
    drop(UsageStore::open(&path).expect("create valid schema"));
    let connection = Connection::open(&path).expect("open index fixture");
    connection
        .execute_batch(
            "DROP INDEX usage_generation_one_current;
             CREATE INDEX usage_generation_one_current
               ON usage_generation(generation) WHERE status = 'current';",
        )
        .expect("replace unique partial index with malformed index");
    drop(connection);

    let error = UsageStore::open(&path).expect_err("malformed current index must fail");
    assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
}

#[test]
fn source_keys_and_checkpoints_are_strict_and_debug_private() {
    assert_eq!(
        SourceKey::from_slice(&[0; 31])
            .expect_err("short key")
            .code(),
        StoreErrorCode::InvalidValue
    );
    let key = SourceKey::from_slice(&[9; 32]).expect("valid source key");
    assert_eq!(format!("{key:?}"), "SourceKey([redacted])");

    let checkpoint = StoredCheckpoint::new(checkpoint_parts()).expect("valid checkpoint");
    let debug = format!("{checkpoint:?}");
    assert!(!debug.contains("[1, 1, 1"));
    assert!(!debug.contains("[2, 2, 2"));
    assert!(!debug.contains("[3, 3, 3"));
    assert!(!debug.contains("[4, 5]"));

    let mut too_large = checkpoint_parts();
    too_large.resume = vec![0; MAX_RESUME_BYTES + 1].into_boxed_slice();
    let error = StoredCheckpoint::new(too_large).expect_err("oversized resume must fail");
    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_RESUME_BYTES as u64));

    let mut invalid_offset = checkpoint_parts();
    invalid_offset.scan_offset = 99;
    assert_eq!(
        StoredCheckpoint::new(invalid_offset)
            .expect_err("backward scan offset")
            .code(),
        StoreErrorCode::InvalidValue
    );

    let mut invalid_flags = checkpoint_parts();
    invalid_flags.scan_offset = 101;
    invalid_flags.observed_file_length = 101;
    assert_eq!(
        StoredCheckpoint::new(invalid_flags)
            .expect_err("non-discard scan divergence")
            .code(),
        StoreErrorCode::InvalidValue
    );
}
