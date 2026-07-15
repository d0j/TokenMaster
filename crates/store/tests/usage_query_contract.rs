use std::{fs, path::Path, time::Duration};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    EXPECTED_SQLITE_VERSION, EventCursor, JournalMode, StoreErrorCode, USAGE_SCHEMA_VERSION,
    UsageActivityQuery, UsageQueryDatasetIdentity, UsageReadStore, UsageStore,
};

const SOURCE_KEY: [u8; 32] = [7; 32];

fn create_empty_archive(path: &Path) {
    drop(UsageStore::open(path).expect("create archive"));
    checkpoint(path);
}

fn checkpoint(path: &Path) {
    let connection = Connection::open(path).expect("open checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint archive");
}

fn seed_current_archive(path: &Path) {
    create_empty_archive(path);
    let mut connection = Connection::open(path).expect("open fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'fixture', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("source");
    transaction
        .execute(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completed_at_ms, completion_state,
               expected_scope_count
             ) VALUES (1, 1000, 2000, 'complete', 1)",
            [],
        )
        .expect("scan set");
    transaction
        .execute(
            "INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1000, 1900, 'complete')",
            [],
        )
        .expect("scan");
    transaction
        .execute(
            "INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 1, 1, 1, 1, 1)",
            [],
        )
        .expect("revision");
    transaction
        .execute(
            "UPDATE usage_archive_state
             SET archive_generation = 4, current_revision_id = 0,
                 latest_complete_scan_set_id = 1, incremental_state = 'complete'
             WHERE singleton_id = 1",
            [],
        )
        .expect("publication");
    for index in 0_u8..3 {
        transaction
            .execute(
                "INSERT INTO usage_event(
                   fingerprint, event_id, selected_file_key, selected_generation,
                   selected_source_offset, projection_revision_id, origin_revision_id,
                   retained, profile_id, session_id, source_id, timestamp_seconds,
                   timestamp_nanos, model, input_tokens, cached_tokens, output_tokens,
                   reasoning_tokens, total_tokens, fallback_model, long_context,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'default', 'session', 'fixture',
                   ?5, ?6, 'gpt-5.6', ?7, NULL, 1, NULL, ?8, 0, 'no',
                   0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    [index + 1; 32].as_slice(),
                    format!("event-{index}"),
                    SOURCE_KEY.as_slice(),
                    i64::from(index),
                    1_000_i64 + i64::from(index),
                    i64::from(index),
                    i64::from(index) + 10,
                    i64::from(index) + 11,
                ],
            )
            .expect("event");
    }
    transaction.commit().expect("commit fixture");
    drop(connection);
    checkpoint(path);
}

fn seed_legacy_archive(path: &Path) {
    let mut connection = Connection::open(path).expect("create v1 fixture");
    connection
        .execute_batch(include_str!("fixtures/usage_v1.sql"))
        .expect("exact v1 schema");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("legacy transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity
             ) VALUES (?1, 'codex', 'legacy', 'fixture', 'archived', ?2, ?3)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("legacy source");
    transaction
        .execute(
            "INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version, physical_identity,
               logical_identity, committed_offset, scan_offset, observed_file_length,
               modified_time_ns, anchor_start, anchor_len, anchor_sha256, resume_payload,
               discarding_oversized_line, incomplete_tail, verification_level
             ) VALUES (?1, 0, 'current', 1, ?2, ?3, 0, 0, 0, NULL, 0, 0, ?4, X'',
                       0, 0, 'full_prefix')",
            params![
                SOURCE_KEY.as_slice(),
                [3_u8; 32].as_slice(),
                [2_u8; 32].as_slice(),
                [4_u8; 32].as_slice()
            ],
        )
        .expect("legacy generation");
    transaction
        .execute(
            "INSERT INTO usage_observation(
               file_key, generation, source_offset, fingerprint, event_id, profile_id,
               session_id, source_id, timestamp_seconds, timestamp_nanos, model,
               input_tokens, cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (?1, 0, 0, ?2, 'event-legacy', 'legacy', 'session', 'fixture',
                       900, 0, 'gpt-5.6', 4, NULL, 1, NULL, 5, 0, 'no',
                       0, 0, 0, 0, 0, 0, 0, 0)",
            params![SOURCE_KEY.as_slice(), [9_u8; 32].as_slice()],
        )
        .expect("legacy observation");
    transaction
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens, cached_tokens,
               output_tokens, reasoning_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) SELECT fingerprint, event_id, file_key, generation, source_offset,
                      profile_id, session_id, source_id, timestamp_seconds, timestamp_nanos,
                      model, input_tokens, cached_tokens, output_tokens, reasoning_tokens,
                      total_tokens, fallback_model, long_context, activity_read,
                      activity_edit_write, activity_search, activity_git,
                      activity_build_test, activity_web, activity_subagents,
                      activity_terminal
               FROM usage_observation",
            [],
        )
        .expect("legacy event");
    transaction.commit().expect("legacy commit");
    drop(connection);
    drop(UsageStore::open(path).expect("migrate legacy archive"));
    checkpoint(path);
}

fn query(
    expected: Option<UsageQueryDatasetIdentity>,
    before: Option<tokenmaster_store::EventCursor>,
    page_size: usize,
) -> UsageActivityQuery {
    UsageActivityQuery::new(expected, before, page_size, Duration::from_secs(2))
        .expect("valid query")
}

#[test]
fn read_store_is_query_only_bounded_and_does_not_modify_archive() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("usage.sqlite3");
    create_empty_archive(&path);
    let before = fs::read(&path).expect("archive bytes before");

    let mut store = UsageReadStore::open(&path).expect("read store");
    assert_eq!(
        store.sqlite_version().expect("SQLite version"),
        EXPECTED_SQLITE_VERSION
    );
    let policy = store.runtime_policy().expect("read policy");
    assert!(policy.query_only());
    assert!(policy.foreign_keys());
    assert!(!policy.trusted_schema());
    assert!(policy.defensive());
    assert!(policy.no_checkpoint_on_close());
    assert_eq!(policy.journal_mode(), JournalMode::Wal);
    assert_eq!(policy.busy_timeout_ms(), 250);
    assert_eq!(policy.cache_size_kib(), 4 * 1024);
    assert_eq!(policy.mmap_size_bytes(), 0);

    let capture = store
        .capture_activity_page(query(None, None, 256))
        .expect("empty capture");
    assert_eq!(capture.publication().generation(), 0);
    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::Empty
    );
    assert_eq!(capture.publication().data_through_ms(), None);
    assert!(capture.publication().scopes().is_empty());
    assert!(capture.events().is_empty());
    assert!(!capture.has_more());
    assert_eq!(format!("{store:?}"), "UsageReadStore([redacted])");
    drop(store);

    assert_eq!(fs::read(&path).expect("archive bytes after"), before);
    assert_eq!(
        UsageActivityQuery::new(None, None, 0, Duration::from_secs(2))
            .expect_err("zero page")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        UsageActivityQuery::new(None, None, 257, Duration::from_secs(2))
            .expect_err("oversized page")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        UsageActivityQuery::new(
            None,
            Some(EventCursor::new(0, 0, [0; 32]).expect("cursor")),
            1,
            Duration::from_secs(2),
        )
        .expect_err("continuation requires dataset identity")
        .code(),
        StoreErrorCode::InvalidValue
    );
}

#[test]
fn read_store_rejects_missing_old_new_and_malformed_archives_without_migration() {
    let directory = TempDir::new().expect("temporary directory");
    let missing = directory.path().join("missing.sqlite3");
    assert_eq!(
        UsageReadStore::open(&missing)
            .expect_err("missing archive")
            .code(),
        StoreErrorCode::Database
    );
    assert!(!missing.exists());

    for (version, expected) in [
        (USAGE_SCHEMA_VERSION - 1, StoreErrorCode::SchemaMismatch),
        (USAGE_SCHEMA_VERSION + 1, StoreErrorCode::SchemaTooNew),
    ] {
        let path = directory.path().join(format!("version-{version}.sqlite3"));
        create_empty_archive(&path);
        let connection = Connection::open(&path).expect("open version fixture");
        connection
            .pragma_update(None, "user_version", version)
            .expect("set fixture version");
        drop(connection);
        checkpoint(&path);
        let before = fs::read(&path).expect("version bytes before");
        assert_eq!(
            UsageReadStore::open(&path)
                .expect_err("version must fail")
                .code(),
            expected
        );
        assert_eq!(fs::read(&path).expect("version bytes after"), before);
    }

    let malformed = directory.path().join("malformed.sqlite3");
    create_empty_archive(&malformed);
    let connection = Connection::open(&malformed).expect("open malformed fixture");
    connection
        .execute("DROP INDEX usage_event_time_desc", [])
        .expect("damage index");
    drop(connection);
    checkpoint(&malformed);
    let before = fs::read(&malformed).expect("malformed bytes before");
    assert_eq!(
        UsageReadStore::open(&malformed)
            .expect_err("malformed archive")
            .code(),
        StoreErrorCode::SchemaMismatch
    );
    assert_eq!(fs::read(&malformed).expect("malformed bytes after"), before);
}

#[test]
fn capture_is_exact_keyset_bounded_and_rejects_stale_dataset() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("usage.sqlite3");
    seed_current_archive(&path);
    let mut store = UsageReadStore::open(&path).expect("read store");
    let identity = UsageQueryDatasetIdentity::ReplayRevision(0);

    let first = store
        .capture_activity_page(query(Some(identity), None, 2))
        .expect("first page");
    assert_eq!(first.publication().generation(), 4);
    assert_eq!(first.publication().dataset_identity(), identity);
    assert_eq!(first.publication().data_through_ms(), Some(2_000));
    assert!(first.publication().accounting_versions_current());
    assert_eq!(first.publication().scopes().len(), 1);
    assert_eq!(first.events().len(), 2);
    assert_eq!(first.events()[0].event_id(), "event-2");
    assert_eq!(first.events()[0].provider_id(), "codex");
    assert_eq!(first.events()[0].profile_id(), "default");
    assert_eq!(first.events()[0].input_tokens(), Some(12));
    assert_eq!(first.events()[0].cached_tokens(), None);
    assert!(first.has_more());
    let cursor = first.next_cursor().expect("continuation cursor");

    let second = store
        .capture_activity_page(query(Some(identity), Some(cursor), 2))
        .expect("second page");
    assert_eq!(second.events().len(), 1);
    assert_eq!(second.events()[0].event_id(), "event-0");
    assert!(!second.has_more());
    assert!(second.next_cursor().is_none());

    let stale = store
        .capture_activity_page(query(
            Some(UsageQueryDatasetIdentity::ReplayRevision(1)),
            Some(cursor),
            2,
        ))
        .expect_err("stale dataset");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
}

#[test]
fn migrated_legacy_page_is_explicit_and_owned() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("legacy.sqlite3");
    seed_legacy_archive(&path);
    let mut store = UsageReadStore::open(&path).expect("legacy read store");
    let capture = store
        .capture_activity_page(query(
            Some(UsageQueryDatasetIdentity::LegacySnapshotV1),
            None,
            1,
        ))
        .expect("legacy capture");
    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::LegacySnapshotV1
    );
    assert_eq!(capture.publication().data_through_ms(), None);
    assert_eq!(capture.events().len(), 1);
    assert_eq!(capture.events()[0].event_id(), "event-legacy");
    assert_eq!(capture.events()[0].provider_id(), "codex");
    assert_eq!(capture.events()[0].profile_id(), "legacy");
    drop(store);
    assert_eq!(capture.events()[0].total_tokens(), Some(5));
}
