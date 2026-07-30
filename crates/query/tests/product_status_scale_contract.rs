use std::{path::Path, time::Duration, time::Instant};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_query::{QueryClock, QueryError, QueryService, QueryTimeSample};
use tokenmaster_store::UsageStore;

const EVENT_COUNT: i64 = 100_000;
const SAMPLE_COUNT: usize = 40;
const STATUS_P95_BUDGET: Duration = Duration::from_millis(25);
const SOURCE_KEY: [u8; 32] = [17; 32];
const STATUS_SOURCE: &str = include_str!("../../store/src/usage/query/status.rs");

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

fn checkpoint(path: &Path) {
    Connection::open(path)
        .expect("checkpoint connection")
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
    connection
        .pragma_update(None, "synchronous", "OFF")
        .expect("fixture synchronous mode");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'status-scale-source', 'active', ?2, ?3, 0)",
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
             ) VALUES (1, 1, 1800000000000, 'complete', 1);
             INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1, 1800000000000, 'complete');
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
               retained, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, project_alias, activity_read,
               activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents,
               activity_terminal
             )
             SELECT unhex(printf('%064x', value)), 'event-' || value, ?2, 0, value,
                    0, 0, 0, 'codex', 'default', 'session-' || value,
                    'status-scale-source', 1704067200 + value, 0,
                    'gpt-scale', 10, NULL, 2, NULL, 12, 0, 'no', 'project',
                    1, 0, 0, 0, 0, 0, 0, 0
             FROM series",
            params![EVENT_COUNT, SOURCE_KEY.as_slice()],
        )
        .expect("bulk events");
    transaction.commit().expect("commit fixture");
    drop(connection);
    checkpoint(path);
}

fn percentile_95(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    samples[(samples.len() * 95).div_ceil(100) - 1]
}

#[test]
fn large_archive_product_status_is_constant_plan_and_below_twenty_five_ms_p95() {
    let production_status_source = STATUS_SOURCE
        .split("#[cfg(test)]")
        .next()
        .expect("production status source");
    for forbidden in [
        "FROM usage_event",
        "JOIN usage_event",
        "usage_aggregate_time",
        "usage_aggregate_session",
        "quota_sample",
        "quota_transition",
        "benefit_change",
        "git_repository_day",
        "git_activity_association",
    ] {
        assert!(
            !production_status_source.contains(forbidden),
            "product status source contains forbidden archive scan: {forbidden}"
        );
    }

    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("product-status-scale.sqlite3");
    seed_large_archive(&path);
    let connection = Connection::open(&path).expect("count fixture events");
    let event_count: i64 = connection
        .query_row("SELECT count(*) FROM usage_event", [], |row| row.get(0))
        .expect("event count");
    assert_eq!(event_count, EVENT_COUNT);
    drop(connection);

    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let warm = service.product_data_status().expect("warm status");
    assert_eq!(
        warm.payload().usage().aggregate().current_event_count(),
        EVENT_COUNT as u64
    );
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let status = service.product_data_status().expect("measured status");
        samples.push(started.elapsed());
        assert_eq!(
            status.payload().usage().aggregate().current_event_count(),
            EVENT_COUNT as u64
        );
    }
    let p95 = percentile_95(&mut samples);
    assert!(
        p95 < STATUS_P95_BUDGET,
        "100K product status p95 {p95:?} exceeded {STATUS_P95_BUDGET:?}"
    );
    eprintln!(
        "P2-F status scale events={} samples={} p95_ms={:.3}",
        EVENT_COUNT,
        SAMPLE_COUNT,
        p95.as_secs_f64() * 1_000.0
    );
}
