use std::{path::Path, time::Instant};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_query::{
    LatestActivityRequest, PageSize, QueryClock, QueryError, QueryService, QueryTimeSample,
};
use tokenmaster_store::UsageStore;

const EVENT_COUNT: i64 = 100_000;
const SOURCE_KEY: [u8; 32] = [7; 32];

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(100_001_000, 1))
    }
}

fn checkpoint(path: &Path) {
    let connection = Connection::open(path).expect("checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

fn seed_large_archive(path: &Path) {
    drop(UsageStore::open(path).expect("create archive"));
    checkpoint(path);
    let mut connection = Connection::open(path).expect("fixture connection");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'performance-private-source', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("source");
    transaction
        .execute_batch(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completed_at_ms, completion_state,
               expected_scope_count
             ) VALUES (1, 1, 100000000, 'complete', 1);
             INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1, 100000000, 'complete');
             INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 1, 1, 1, 1, 1);
             UPDATE usage_archive_state
             SET archive_generation = 1, current_revision_id = 0,
                 latest_complete_scan_set_id = 1, incremental_state = 'complete'
             WHERE singleton_id = 1;",
        )
        .expect("publication metadata");
    transaction
        .execute(
            "WITH RECURSIVE series(value) AS (
               VALUES(1)
               UNION ALL
               SELECT value + 1 FROM series WHERE value < ?1
             )
             INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, profile_id, session_id, source_id, timestamp_seconds,
               timestamp_nanos, model, input_tokens, cached_tokens, output_tokens,
               reasoning_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             )
             SELECT unhex(printf('%064x', value)), 'event-' || value, ?2, 0, value,
                    0, 0, 0, 'default', 'session', 'performance-private-source',
                    value, 0, 'gpt-5.6', value, NULL, 1, NULL, value + 1, 0, 'no',
                    0, 0, 0, 0, 0, 0, 0, 0
             FROM series",
            params![EVENT_COUNT, SOURCE_KEY.as_slice()],
        )
        .expect("bulk events");
    transaction.commit().expect("commit fixture");
    drop(connection);
    checkpoint(path);
}

#[test]
fn hundred_thousand_event_pages_stay_within_latency_budgets() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("performance.sqlite3");
    seed_large_archive(&path);
    let page_size = PageSize::new(256).expect("page size");

    let cold_started = Instant::now();
    let mut service = QueryService::open(&path, FixedClock).expect("open cold query service");
    let first = service
        .latest_activity(LatestActivityRequest::first(page_size))
        .expect("cold first page");
    let cold_elapsed = cold_started.elapsed();
    assert_eq!(first.payload().items().len(), 256);
    assert_eq!(first.payload().items()[0].event_id(), "event-100000");
    assert!(first.payload().has_more());
    assert!(
        cold_elapsed.as_secs_f64() <= 1.0,
        "100K cold open and first page exceeded 1 s: {cold_elapsed:?}"
    );

    let warm_started = Instant::now();
    let second = service
        .latest_activity(LatestActivityRequest::continuation(
            page_size,
            first.header().dataset_identity(),
            first.payload().next_cursor().expect("first cursor"),
        ))
        .expect("warm cursor page");
    let warm_elapsed = warm_started.elapsed();
    assert_eq!(second.payload().items().len(), 256);
    assert_eq!(second.payload().items()[0].event_id(), "event-99744");
    assert!(second.payload().has_more());
    assert!(
        warm_elapsed.as_secs_f64() <= 0.250,
        "100K warm cursor page exceeded 250 ms: {warm_elapsed:?}"
    );

    eprintln!(
        "P2-A 100K evidence: cold_open_first_page={cold_elapsed:?}, warm_cursor_page={warm_elapsed:?}"
    );
}
