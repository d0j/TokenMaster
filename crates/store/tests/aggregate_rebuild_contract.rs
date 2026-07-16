use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    AggregateRebuildStatus, MAX_AGGREGATE_REBUILD_PAGE_SIZE, StoreErrorCode, UsageStore,
};

fn insert_current_event(connection: &Connection, index: u8) {
    connection
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, project_alias,
               input_tokens, cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (
               ?1, ?2, ?3, 0, ?4, 'codex', 'default', 'session', 'fixture',
               ?5, ?4, ?6, ?7, ?4, NULL, 2, NULL, ?8, 0, 'no',
               1, 2, 3, 4, 5, 6, 7, 8
             )",
            params![
                [index; 32].as_slice(),
                format!("event-{index}"),
                [9_u8; 32].as_slice(),
                i64::from(index),
                60_i64 + i64::from(index),
                if index.is_multiple_of(2) {
                    "model-a"
                } else {
                    "model-b"
                },
                if index.is_multiple_of(2) {
                    None
                } else {
                    Some("project-b")
                },
                i64::from(index) + 2,
            ],
        )
        .expect("current event");
}

fn require_rebuild(connection: &Connection) {
    connection
        .execute_batch(
            "UPDATE usage_aggregate_state
             SET state = 'rebuild_required', rebuild_aggregate_generation = NULL,
                 rebuild_dataset_kind = NULL, rebuild_cursor_fingerprint = NULL,
                 rebuild_processed_events = 0,
                 rebuild_total_events = current_event_count + legacy_event_count
             WHERE singleton_id = 1;
             DELETE FROM usage_time_rollup;
             DELETE FROM usage_session_rollup;",
        )
        .expect("require aggregate rebuild");
}

fn seed_v1_event(path: &std::path::Path) {
    let connection = Connection::open(path).expect("create v1 aggregate fixture");
    connection
        .execute_batch(include_str!("fixtures/usage_v1.sql"))
        .expect("exact v1 schema");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity
             ) VALUES (
               zeroblob(32), 'codex', 'default', 'fixture', 'active',
               zeroblob(32), zeroblob(32)
             );
             INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version,
               physical_identity, logical_identity, committed_offset, scan_offset,
               observed_file_length, anchor_start, anchor_len, anchor_sha256,
               resume_payload, discarding_oversized_line, incomplete_tail,
               verification_level
             ) VALUES (
               zeroblob(32), 0, 'current', 1, zeroblob(32), zeroblob(32),
               0, 0, 0, 0, 0, zeroblob(32), zeroblob(0), 0, 0, 'full_prefix'
             );
             UPDATE usage_source SET current_generation = 0;
             INSERT INTO usage_observation(
               file_key, generation, source_offset, fingerprint, event_id,
               profile_id, session_id, source_id, timestamp_seconds,
               timestamp_nanos, model, input_tokens, output_tokens, total_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (
               zeroblob(32), 0, 0, zeroblob(32), 'legacy-event', 'default',
               'legacy-session', 'fixture', 100, 1, 'gpt-test', 1, 2, 3,
               0, 'no', 1, 2, 3, 4, 5, 6, 7, 8
             );
             INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               output_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) VALUES (
               zeroblob(32), 'legacy-event', zeroblob(32), 0, 0, 'default',
               'legacy-session', 'fixture', 100, 1, 'gpt-test', 1, 2, 3,
               0, 'no', 1, 2, 3, 4, 5, 6, 7, 8
             );",
        )
        .expect("seed v1 event");
}

#[test]
fn current_rebuild_is_bounded_resumable_and_generation_published() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("current-rebuild.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let connection = Connection::open(&path).expect("seed archive");
    for index in 1_u8..=5 {
        insert_current_event(&connection, index);
    }
    require_rebuild(&connection);
    drop(connection);

    let mut store = UsageStore::open(&path).expect("reopen rebuild archive");
    let cleanup = store.rebuild_aggregates_page(2).expect("cleanup page");
    assert_eq!(cleanup.status(), AggregateRebuildStatus::Rebuilding);
    assert_eq!(cleanup.processed_events(), 0);
    assert_eq!(cleanup.total_events(), 5);
    let cleanup_complete = store
        .rebuild_aggregates_page(2)
        .expect("finish stale price-row cleanup");
    assert_eq!(
        cleanup_complete.status(),
        AggregateRebuildStatus::Rebuilding
    );
    assert_eq!(cleanup_complete.processed_events(), 0);
    let first = store.rebuild_aggregates_page(2).expect("first data page");
    assert_eq!(first.processed_events(), 2);
    drop(store);

    let mut store = UsageStore::open(&path).expect("resume after reopen");
    let second = store.rebuild_aggregates_page(2).expect("second data page");
    assert_eq!(second.processed_events(), 4);
    let complete = store.rebuild_aggregates_page(2).expect("final data page");
    assert_eq!(complete.status(), AggregateRebuildStatus::Ready);
    assert_eq!(complete.processed_events(), 5);
    assert_eq!(complete.total_events(), 5);
    drop(store);

    let connection = Connection::open(&path).expect("inspect rebuilt archive");
    let state: (String, i64, Option<i64>, i64, i64) = connection
        .query_row(
            "SELECT state, active_aggregate_generation,
                    rebuild_aggregate_generation, expected_dataset_generation,
                    current_event_count
             FROM usage_aggregate_state WHERE singleton_id = 1",
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
        .expect("aggregate state");
    assert_eq!(state, ("ready".to_owned(), 1, None, 5, 5));
    let active_rows: (i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_time_rollup
                WHERE aggregate_generation = 1),
               (SELECT count(*) FROM usage_session_rollup
                WHERE aggregate_generation = 1),
               (SELECT sum(event_count) FROM usage_time_rollup
                WHERE aggregate_generation = 1 AND dataset_kind = 'current'
                  AND bucket_width = 'hour' AND dimension_kind = 'all'),
               (SELECT count(*) FROM usage_price_time_rollup
                WHERE aggregate_generation = 1),
               (SELECT count(*) FROM usage_price_session_rollup
                WHERE aggregate_generation = 1),
               (SELECT sum(event_count) FROM usage_price_time_rollup
                WHERE aggregate_generation = 1 AND dataset_kind = 'current'
                  AND bucket_width = 'hour')",
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
        .expect("active rollups");
    assert!(active_rows.0 > 0);
    assert!(active_rows.1 > 0);
    assert_eq!(active_rows.2, 5);
    assert!(active_rows.3 > 0);
    assert!(active_rows.4 > 0);
    assert_eq!(active_rows.5, 5);
    let project_rows = connection
        .prepare(
            "SELECT project_key, sum(event_count)
             FROM usage_price_time_rollup
             WHERE aggregate_generation = 1 AND dataset_kind = 'current'
               AND bucket_width = 'hour'
             GROUP BY project_key ORDER BY project_key",
        )
        .expect("prepare rebuilt project price rows")
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query rebuilt project price rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect rebuilt project price rows");
    assert_eq!(
        project_rows,
        vec![(String::new(), 2), ("project-b".to_owned(), 3)]
    );
    let stale_price_rows: i64 = connection
        .query_row(
            "SELECT (SELECT count(*) FROM usage_price_time_rollup
                     WHERE aggregate_generation <> 1)
                  + (SELECT count(*) FROM usage_price_session_rollup
                     WHERE aggregate_generation <> 1)",
            [],
            |row| row.get(0),
        )
        .expect("no stale price generations");
    assert_eq!(stale_price_rows, 0);
}

#[test]
fn migrated_current_and_immutable_legacy_build_as_distinct_datasets() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("legacy-rebuild.sqlite3");
    seed_v1_event(&path);
    let mut store = UsageStore::open(&path).expect("migrate v1 archive");
    let mut progress = store
        .rebuild_aggregates_page(1)
        .expect("start legacy rebuild");
    for _ in 0..8 {
        if progress.status() == AggregateRebuildStatus::Ready {
            break;
        }
        progress = store
            .rebuild_aggregates_page(1)
            .expect("continue legacy rebuild");
    }
    assert_eq!(progress.status(), AggregateRebuildStatus::Ready);
    assert_eq!(progress.total_events(), 2);
    drop(store);

    let connection = Connection::open(path).expect("inspect legacy aggregates");
    let datasets = connection
        .prepare(
            "SELECT dataset_kind, provider_id, event_count
             FROM usage_time_rollup
             WHERE aggregate_generation = (
               SELECT active_aggregate_generation FROM usage_aggregate_state
               WHERE singleton_id = 1
             ) AND bucket_width = 'hour' AND dimension_kind = 'all'
             ORDER BY dataset_kind",
        )
        .expect("prepare dataset rows")
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .expect("query dataset rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect dataset rows");
    assert_eq!(
        datasets,
        vec![
            ("current".to_owned(), "codex".to_owned(), 1),
            ("legacy".to_owned(), "codex".to_owned(), 1),
        ]
    );
    let price_datasets = connection
        .prepare(
            "SELECT dataset_kind, provider_id, sum(event_count),
                    sum(calculable_event_count), sum(reported_cost_count)
             FROM usage_price_time_rollup
             WHERE aggregate_generation = (
               SELECT active_aggregate_generation FROM usage_aggregate_state
               WHERE singleton_id = 1
             ) AND bucket_width = 'hour'
             GROUP BY dataset_kind, provider_id ORDER BY dataset_kind",
        )
        .expect("prepare price datasets")
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .expect("query price datasets")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect price datasets");
    assert_eq!(
        price_datasets,
        vec![
            ("current".to_owned(), "codex".to_owned(), 1, 0, 0),
            ("legacy".to_owned(), "codex".to_owned(), 1, 0, 0),
        ]
    );
}

#[test]
fn event_mutation_discards_only_staging_and_restarts_from_new_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("restart-rebuild.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let connection = Connection::open(&path).expect("seed archive");
    for index in 1_u8..=3 {
        insert_current_event(&connection, index);
    }
    require_rebuild(&connection);
    drop(connection);

    let mut store = UsageStore::open(&path).expect("rebuild archive");
    store.rebuild_aggregates_page(1).expect("cleanup page");
    store.rebuild_aggregates_page(1).expect("staging page");
    drop(store);
    let connection = Connection::open(&path).expect("concurrent event fixture");
    insert_current_event(&connection, 4);
    drop(connection);

    let mut store = UsageStore::open(&path).expect("open stale rebuild");
    let restarted = store
        .rebuild_aggregates_page(1)
        .expect("restart stale build");
    assert_eq!(restarted.status(), AggregateRebuildStatus::Rebuilding);
    assert_eq!(restarted.processed_events(), 0);
    let mut progress = restarted;
    for _ in 0..12 {
        if progress.status() == AggregateRebuildStatus::Ready {
            break;
        }
        progress = store.rebuild_aggregates_page(1).expect("continued rebuild");
    }
    assert_eq!(progress.status(), AggregateRebuildStatus::Ready);
    assert_eq!(progress.total_events(), 4);
    drop(store);
    let connection = Connection::open(path).expect("inspect restarted rebuild");
    assert_eq!(
        connection
            .query_row(
                "SELECT sum(event_count) FROM usage_time_rollup
                 WHERE aggregate_generation = (
                   SELECT active_aggregate_generation FROM usage_aggregate_state
                   WHERE singleton_id = 1
                 ) AND dataset_kind = 'current' AND bucket_width = 'hour'
                   AND dimension_kind = 'all'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("rebuilt event count"),
        4
    );
}

#[test]
fn rebuild_page_bounds_fail_before_writes() {
    let mut store = UsageStore::in_memory().expect("in-memory archive");
    for invalid in [0, MAX_AGGREGATE_REBUILD_PAGE_SIZE + 1] {
        let error = store
            .rebuild_aggregates_page(invalid)
            .expect_err("invalid rebuild page");
        assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
        assert_eq!(
            error.limit(),
            Some(u64::try_from(MAX_AGGREGATE_REBUILD_PAGE_SIZE).expect("limit"))
        );
    }
}
