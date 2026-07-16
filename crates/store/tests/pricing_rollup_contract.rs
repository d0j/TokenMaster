use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{USAGE_SCHEMA_VERSION, UsageStore};

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count rollup")
}

#[test]
fn current_schema_reserves_reported_cost_and_strict_price_rollups() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("pricing-schema.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let connection = Connection::open(&path).expect("inspect archive");

    assert_eq!(USAGE_SCHEMA_VERSION, 11);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("schema version"),
        11
    );
    for table in ["usage_observation", "usage_event", "usage_legacy_event"] {
        let present: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info(?1)
                 WHERE name = 'reported_cost_usd_micros' AND type = 'INTEGER'",
                [table],
                |row| row.get(0),
            )
            .expect("reported cost column");
        assert_eq!(present, 1, "{table}");
    }
    let aggregate_version: i64 = connection
        .query_row(
            "SELECT aggregate_schema_version FROM usage_aggregate_state
             WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("aggregate schema version");
    assert_eq!(aggregate_version, 2);

    for table in ["usage_price_time_rollup", "usage_price_session_rollup"] {
        let strict: i64 = connection
            .query_row(
                "SELECT strict FROM pragma_table_list WHERE name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("strict price table");
        assert_eq!(strict, 1, "{table}");
    }
}

#[test]
fn ready_current_price_rollups_track_insert_update_and_delete_atomically() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("pricing-write.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let connection = Connection::open(&path).expect("open archive");
    connection
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens, cached_tokens,
               output_tokens, reasoning_tokens, total_tokens, fallback_model,
               long_context, service_tier, reported_cost_usd_micros,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) VALUES (
               ?1, 'event', ?2, 0, 0, 'codex', 'default', 'session', 'source',
               121, 0, 'gpt-5.6-sol', 100, 40, 20, 5, 125, 0,
               'yes', 'priority', 12345, 0, 0, 0, 0, 0, 0, 0, 0
             )",
            params![[1_u8; 32].as_slice(), [2_u8; 32].as_slice()],
        )
        .expect("insert event");

    assert_eq!(count(&connection, "usage_price_time_rollup"), 2);
    assert_eq!(count(&connection, "usage_price_session_rollup"), 1);
    let time: (String, String, String, i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT model, service_tier, long_context, event_count,
                    calculable_event_count, uncached_input_sum, cached_input_sum,
                    billable_output_sum, reported_cost_sum
             FROM usage_price_time_rollup WHERE bucket_width = 'minute'",
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
        .expect("minute price basis");
    assert_eq!(
        time,
        (
            "gpt-5.6-sol".to_owned(),
            "priority".to_owned(),
            "yes".to_owned(),
            1,
            1,
            60,
            40,
            25,
            12345,
        )
    );

    connection
        .execute(
            "UPDATE usage_event
             SET service_tier = NULL, long_context = 'no',
                 reported_cost_usd_micros = NULL, cached_tokens = 101
             WHERE event_id = 'event'",
            [],
        )
        .expect("replace price basis");
    assert_eq!(count(&connection, "usage_price_time_rollup"), 2);
    let updated: (String, String, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT service_tier, reported_state, calculable_event_count,
                    uncached_input_sum, billable_output_sum, reported_cost_count
             FROM usage_price_session_rollup",
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
        .expect("updated session price basis");
    assert_eq!(
        updated,
        (
            "standard_assumed".to_owned(),
            "missing".to_owned(),
            0,
            0,
            0,
            0
        )
    );

    connection
        .execute("DELETE FROM usage_event WHERE event_id = 'event'", [])
        .expect("delete event");
    assert_eq!(count(&connection, "usage_price_time_rollup"), 0);
    assert_eq!(count(&connection, "usage_price_session_rollup"), 0);
}

#[test]
fn price_rollups_retain_bounded_project_partition_without_extra_event_rows() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("pricing-project.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let connection = Connection::open(&path).expect("open archive");
    for (index, project) in [(1_u8, None), (2_u8, Some("project-a"))] {
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
                   ?1, ?2, ?3, 0, ?4, 'codex', 'default', 'session', 'source',
                   121, 0, 'gpt-5.6-sol', ?5, 10, 2, 3, 4, 17, 0, 'no',
                   0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    [index; 32].as_slice(),
                    format!("project-event-{index}"),
                    [index + 10; 32].as_slice(),
                    i64::from(index),
                    project,
                ],
            )
            .expect("project price event");
    }

    let projects = connection
        .prepare(
            "SELECT project_key, event_count FROM usage_price_time_rollup
             WHERE bucket_width = 'minute' ORDER BY project_key",
        )
        .expect("project price query")
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("project price rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect project price rows");
    assert_eq!(
        projects,
        vec![(String::new(), 1), ("project-a".to_owned(), 1)]
    );
    assert_eq!(count(&connection, "usage_price_time_rollup"), 4);
    assert_eq!(count(&connection, "usage_price_session_rollup"), 2);
}
