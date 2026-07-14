use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    EXPECTED_SQLITE_VERSION, JournalMode, MAX_RESUME_BYTES, MAX_USAGE_EVENT_PAGE_SIZE, SourceKey,
    StoreErrorCode, StoredCheckpoint, StoredCheckpointParts, StoredVerification,
    USAGE_SCHEMA_VERSION, UsageStore,
};

const USAGE_TABLES: [&str; 6] = [
    "usage_source",
    "usage_generation",
    "usage_source_chunk",
    "usage_observation",
    "usage_event",
    "usage_scan",
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

fn seed_usage_fixture(path: &Path, event_count: u32) {
    drop(UsageStore::open(path).expect("create usage schema"));
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
        transaction
            .execute(
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
                    event_id,
                    FIXTURE_SOURCE_KEY.as_slice(),
                    source_offset
                ],
            )
            .expect("fixture canonical event");
    }
    transaction.commit().expect("commit usage fixture");
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
    assert_eq!(version, USAGE_SCHEMA_VERSION);

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
        if USAGE_TABLES.contains(&name.as_str()) {
            assert_eq!(strict, 1, "{name} must be STRICT");
            observed.push(name);
        }
    }
    observed.sort();
    let mut expected = USAGE_TABLES.map(str::to_owned);
    expected.sort();
    assert_eq!(observed, expected);

    for table in USAGE_TABLES {
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
            "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name IN ('usage_generation_one_current', 'usage_generation_one_staging') AND sql LIKE '% WHERE status = %'",
            [],
            |row| row.get(0),
        )
        .expect("partial indexes");
    assert_eq!(partial_indexes, 2);

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
