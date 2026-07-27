use rusqlite::Connection;

fn trigger_sql(connection: &Connection, name: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'trigger' AND name = ?1",
            [name],
            |row| row.get(0),
        )
        .expect("read trigger SQL")
}

fn combine_update(name: &str, delete_trigger: &str, insert_trigger: &str) -> String {
    let delete_body = delete_trigger
        .split_once("\nBEGIN\n")
        .and_then(|(_, body)| body.strip_suffix("END"))
        .expect("delete trigger body");
    let insert_body = insert_trigger
        .split_once("\nBEGIN\n")
        .and_then(|(_, body)| body.strip_suffix("END"))
        .expect("insert trigger body");
    format!(
        "CREATE TRIGGER {name}\nAFTER UPDATE ON usage_event\nWHEN (SELECT state FROM usage_aggregate_state WHERE singleton_id = 1) = 'ready'\nBEGIN\n{delete_body}{insert_body}END;\n"
    )
}

pub fn restore_v13_time_triggers(connection: &Connection) {
    connection
        .execute_batch(
            "CREATE INDEX usage_replay_observation_children ON usage_replay_observation(revision_id, provider_id, profile_id, parent_session_id, session_ordinal, disposition, session_id);",
        )
        .expect("restore pre-v15 replay children index");
    let aggregate_insert = trigger_sql(connection, "usage_event_aggregate_time_after_insert");
    let price_insert = trigger_sql(connection, "usage_event_price_time_after_insert");
    connection
        .execute_batch(
            "DROP TRIGGER usage_event_aggregate_time_after_delete;
             DROP TRIGGER usage_event_aggregate_time_after_update;
             DROP TRIGGER usage_event_price_time_after_delete;
             DROP TRIGGER usage_event_price_time_after_update;",
        )
        .expect("drop v14 time triggers");
    connection
        .execute_batch(include_str!("../fixtures/usage_v13_time_delete_triggers.sql"))
        .expect("restore v13 time delete triggers");
    let aggregate_delete = trigger_sql(connection, "usage_event_aggregate_time_after_delete");
    let price_delete = trigger_sql(connection, "usage_event_price_time_after_delete");
    connection
        .execute_batch(&combine_update(
            "usage_event_aggregate_time_after_update",
            &aggregate_delete,
            &aggregate_insert,
        ))
        .expect("restore v13 aggregate time update trigger");
    connection
        .execute_batch(&combine_update(
            "usage_event_price_time_after_update",
            &price_delete,
            &price_insert,
        ))
        .expect("restore v13 price time update trigger");
}
