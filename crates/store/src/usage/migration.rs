use rusqlite::{Connection, TransactionBehavior};

use crate::{StoreError, StoreErrorCode};

use super::schema::{
    IndexContract, LEGACY_COPY_SQL, LEGACY_IMMUTABILITY_TRIGGERS, PRE_V4_USAGE_EVENT_CONTRACT,
    REPLAY_AUX_SCHEMA, REPLAY_CHILD_SCHEMA, TableContract, TriggerContract, USAGE_INDEX_CONTRACTS,
    USAGE_SCHEMA_VERSION, USAGE_TABLE_CONTRACTS, USAGE_TRIGGER_CONTRACTS, V1_INDEX_CONTRACTS,
    V1_SCHEMA, V1_SCHEMA_VERSION, V1_TABLE_COUNT, V2_REPLAY_REVISION_SCHEMA, V2_SCHEMA_VERSION,
    V3_REPLAY_REVISION_SCHEMA, V3_SCHEMA_VERSION, V4_USAGE_EVENT_SCHEMA,
};

pub(super) fn migrate_schema(connection: &mut Connection) -> Result<(), StoreError> {
    let version = pragma_i64(connection, "PRAGMA user_version")?;
    if version > USAGE_SCHEMA_VERSION {
        return Err(StoreError::new(StoreErrorCode::SchemaTooNew));
    }

    match version {
        V2_SCHEMA_VERSION => migrate_v2(connection),
        V3_SCHEMA_VERSION => migrate_v3(connection),
        0 | V1_SCHEMA_VERSION | USAGE_SCHEMA_VERSION => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            match version {
                0 => create_fresh_v4(&transaction)?,
                V1_SCHEMA_VERSION => migrate_v1(&transaction)?,
                USAGE_SCHEMA_VERSION => validate_v4(&transaction)?,
                _ => return Err(StoreError::new(StoreErrorCode::SchemaMismatch)),
            }
            transaction.commit()?;
            Ok(())
        }
        _ => Err(StoreError::new(StoreErrorCode::SchemaMismatch)),
    }
}

fn create_fresh_v4(connection: &Connection) -> Result<(), StoreError> {
    if count_application_objects(connection)? != 0 {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    connection.execute_batch(V1_SCHEMA)?;
    connection.execute_batch(REPLAY_AUX_SCHEMA)?;
    connection.execute_batch(V3_REPLAY_REVISION_SCHEMA)?;
    connection.execute_batch(REPLAY_CHILD_SCHEMA)?;
    connection.execute_batch(LEGACY_IMMUTABILITY_TRIGGERS)?;
    migrate_usage_event_v4(connection, MigrationFault::None)?;
    connection.pragma_update(None, "user_version", USAGE_SCHEMA_VERSION)?;
    validate_v4(connection)
}

fn migrate_v1(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        &USAGE_TABLE_CONTRACTS[..V1_TABLE_COUNT],
        V1_INDEX_CONTRACTS,
        &[],
        &[V1_SCHEMA],
        &[PRE_V4_USAGE_EVENT_CONTRACT],
    )?;
    connection.execute_batch(REPLAY_AUX_SCHEMA)?;
    connection.execute_batch(V3_REPLAY_REVISION_SCHEMA)?;
    connection.execute_batch(REPLAY_CHILD_SCHEMA)?;
    connection.execute_batch(LEGACY_COPY_SQL)?;
    let counts: (i64, i64) = connection.query_row(
        "SELECT
           (SELECT event_count FROM usage_legacy_snapshot WHERE snapshot_id = 1),
           (SELECT count(*) FROM usage_legacy_event WHERE snapshot_id = 1)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if counts.0 < 0 || counts.0 != counts.1 {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    connection.execute_batch(LEGACY_IMMUTABILITY_TRIGGERS)?;
    migrate_usage_event_v4(connection, MigrationFault::None)?;
    connection.pragma_update(None, "user_version", USAGE_SCHEMA_VERSION)?;
    validate_v4(connection)
}

fn validate_v2(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        USAGE_TRIGGER_CONTRACTS,
        &[
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            V2_REPLAY_REVISION_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[PRE_V4_USAGE_EVENT_CONTRACT],
    )?;
    validate_legacy_snapshot(connection)
}

fn validate_v3(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        USAGE_TRIGGER_CONTRACTS,
        &[
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            V3_REPLAY_REVISION_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[PRE_V4_USAGE_EVENT_CONTRACT],
    )?;
    validate_legacy_snapshot(connection)
}

fn validate_v4(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        USAGE_TRIGGER_CONTRACTS,
        &[
            V4_USAGE_EVENT_SCHEMA,
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            V3_REPLAY_REVISION_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[],
    )?;
    validate_legacy_snapshot(connection)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MigrationFault {
    None,
    #[cfg(test)]
    AfterCreateRevision,
    #[cfg(test)]
    AfterCopyRevision,
    #[cfg(test)]
    AfterDropRevision,
    #[cfg(test)]
    AfterCreateEvent,
    #[cfg(test)]
    AfterCopyEvent,
    #[cfg(test)]
    AfterDropEvent,
}

fn migrate_v2(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v2_with_fault(connection, MigrationFault::None)
}

fn migrate_v2_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v2(connection)?;
    if connection
        .pragma_update(None, "foreign_keys", "OFF")
        .is_err()
    {
        let _restore_attempt = restore_foreign_keys(connection);
        return Err(StoreError::new(StoreErrorCode::Database));
    }
    match pragma_i64(connection, "PRAGMA foreign_keys") {
        Ok(0) => {}
        Ok(_) => {
            let _restore_attempt = restore_foreign_keys(connection);
            return Err(StoreError::new(StoreErrorCode::PolicyMismatch));
        }
        Err(error) => {
            let _restore_attempt = restore_foreign_keys(connection);
            return Err(error);
        }
    }

    let migration: Result<(), StoreError> = (|| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        migrate_v2_revision_table(&transaction, fault)?;
        transaction.commit()?;
        Ok(())
    })();
    let restored = restore_foreign_keys(connection);
    match (migration, restored) {
        (Ok(()), Ok(())) => {
            validate_v3(connection)?;
            migrate_v3(connection)
        }
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(_), Err(_)) => Err(StoreError::new(StoreErrorCode::PolicyMismatch)),
    }
}

fn restore_foreign_keys(connection: &Connection) -> Result<(), StoreError> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    if pragma_i64(connection, "PRAGMA foreign_keys")? == 1 {
        Ok(())
    } else {
        Err(StoreError::new(StoreErrorCode::PolicyMismatch))
    }
}

fn migrate_v2_revision_table(
    connection: &Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    let old_count = pragma_i64(connection, "SELECT count(*) FROM usage_replay_revision")?;
    connection.execute_batch(
        "CREATE TABLE usage_replay_revision_v3 (
           revision_id INTEGER PRIMARY KEY CHECK(revision_id >= 0),
           status TEXT NOT NULL CHECK(status IN ('staging','current')),
           canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
           fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
           replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
           expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 1),
           evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
           sealed INTEGER NOT NULL CHECK(sealed IN (0,1)),
           promoted INTEGER NOT NULL CHECK(promoted IN (0,1)),
           CHECK((status = 'staging' AND promoted = 0) OR
                 (status = 'current' AND sealed = 1 AND promoted = 1))
         ) STRICT;",
    )?;
    migration_fault(fault, MigrationBoundary::Created)?;
    connection.execute_batch(
        "INSERT INTO usage_replay_revision_v3(
           revision_id, status, canonicalizer_version, fingerprint_version,
           replay_signature_version, expected_source_count, evidence_epoch,
           sealed, promoted
         )
         SELECT
           revision_id, status, canonicalizer_version, fingerprint_version,
           replay_signature_version, expected_source_count, evidence_epoch,
           sealed, promoted
         FROM usage_replay_revision;",
    )?;
    let new_count = pragma_i64(connection, "SELECT count(*) FROM usage_replay_revision_v3")?;
    if old_count < 0 || old_count != new_count {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    migration_fault(fault, MigrationBoundary::Copied)?;
    connection.execute_batch("DROP TABLE usage_replay_revision;")?;
    migration_fault(fault, MigrationBoundary::Dropped)?;
    connection.execute_batch(
        "ALTER TABLE usage_replay_revision_v3 RENAME TO usage_replay_revision;
         CREATE UNIQUE INDEX usage_replay_revision_one_current
           ON usage_replay_revision(status) WHERE status = 'current';
         CREATE UNIQUE INDEX usage_replay_revision_one_staging
           ON usage_replay_revision(status) WHERE status = 'staging';",
    )?;
    let foreign_key_failures =
        pragma_i64(connection, "SELECT count(*) FROM pragma_foreign_key_check")?;
    if foreign_key_failures != 0 {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    connection.pragma_update(None, "user_version", V3_SCHEMA_VERSION)?;
    validate_v3(connection)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MigrationBoundary {
    Created,
    Copied,
    Dropped,
    EventCreated,
    EventCopied,
    EventDropped,
}

fn migration_fault(fault: MigrationFault, boundary: MigrationBoundary) -> Result<(), StoreError> {
    #[cfg(test)]
    let triggered = matches!(
        (fault, boundary),
        (
            MigrationFault::AfterCreateRevision,
            MigrationBoundary::Created
        ) | (MigrationFault::AfterCopyRevision, MigrationBoundary::Copied)
            | (
                MigrationFault::AfterDropRevision,
                MigrationBoundary::Dropped
            )
            | (
                MigrationFault::AfterCreateEvent,
                MigrationBoundary::EventCreated
            )
            | (
                MigrationFault::AfterCopyEvent,
                MigrationBoundary::EventCopied
            )
            | (
                MigrationFault::AfterDropEvent,
                MigrationBoundary::EventDropped
            )
    );
    #[cfg(not(test))]
    let triggered = {
        let _ = (fault, boundary);
        false
    };
    if triggered {
        Err(StoreError::new(StoreErrorCode::Database))
    } else {
        Ok(())
    }
}

fn migrate_v3(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v3_with_fault(connection, MigrationFault::None)
}

fn migrate_v3_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v3(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    migrate_usage_event_v4(&transaction, fault)?;
    transaction.pragma_update(None, "user_version", USAGE_SCHEMA_VERSION)?;
    validate_v4(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_usage_event_v4(
    connection: &Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    let old_count = pragma_i64(connection, "SELECT count(*) FROM usage_event")?;
    let temporary_schema = V4_USAGE_EVENT_SCHEMA.replacen(
        "CREATE TABLE usage_event (",
        "CREATE TABLE usage_event_v4 (",
        1,
    );
    connection.execute_batch(&temporary_schema)?;
    migration_fault(fault, MigrationBoundary::EventCreated)?;
    connection.execute_batch(
        "INSERT INTO usage_event_v4(
           fingerprint, event_id, selected_file_key, selected_generation,
           selected_source_offset, projection_revision_id, origin_revision_id,
           retained, profile_id, session_id, source_id, timestamp_seconds,
           timestamp_nanos, model, raw_model, input_tokens, cached_tokens,
           output_tokens, reasoning_tokens, total_tokens, fallback_model,
           long_context, service_tier, project_alias, originator, activity_read,
           activity_edit_write, activity_search, activity_git,
           activity_build_test, activity_web, activity_subagents, activity_terminal
         )
         SELECT
           fingerprint, event_id, selected_file_key, selected_generation,
           selected_source_offset,
           (SELECT revision_id FROM usage_replay_revision WHERE status = 'current'),
           (SELECT revision_id FROM usage_replay_revision WHERE status = 'current'),
           0, profile_id, session_id, source_id, timestamp_seconds,
           timestamp_nanos, model, raw_model, input_tokens, cached_tokens,
           output_tokens, reasoning_tokens, total_tokens, fallback_model,
           long_context, service_tier, project_alias, originator, activity_read,
           activity_edit_write, activity_search, activity_git,
           activity_build_test, activity_web, activity_subagents, activity_terminal
         FROM usage_event;",
    )?;
    let new_count = pragma_i64(connection, "SELECT count(*) FROM usage_event_v4")?;
    let logical_difference = pragma_i64(
        connection,
        "SELECT count(*) FROM (
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
                  cached_tokens, output_tokens, reasoning_tokens, total_tokens,
                  fallback_model, long_context, service_tier, project_alias,
                  originator, activity_read, activity_edit_write, activity_search,
                  activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event
           EXCEPT
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
                  cached_tokens, output_tokens, reasoning_tokens, total_tokens,
                  fallback_model, long_context, service_tier, project_alias,
                  originator, activity_read, activity_edit_write, activity_search,
                  activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event_v4
           UNION ALL
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
                  cached_tokens, output_tokens, reasoning_tokens, total_tokens,
                  fallback_model, long_context, service_tier, project_alias,
                  originator, activity_read, activity_edit_write, activity_search,
                  activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event_v4
           EXCEPT
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
                  cached_tokens, output_tokens, reasoning_tokens, total_tokens,
                  fallback_model, long_context, service_tier, project_alias,
                  originator, activity_read, activity_edit_write, activity_search,
                  activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event
         )",
    )?;
    if old_count < 0 || old_count != new_count || logical_difference != 0 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    migration_fault(fault, MigrationBoundary::EventCopied)?;
    connection.execute_batch("DROP TABLE usage_event;")?;
    migration_fault(fault, MigrationBoundary::EventDropped)?;
    connection.execute_batch(
        "ALTER TABLE usage_event_v4 RENAME TO usage_event;
         CREATE INDEX usage_event_time_desc
           ON usage_event(timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC);
         CREATE INDEX usage_event_model_time
           ON usage_event(model, timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC);",
    )?;
    Ok(())
}

fn validate_legacy_snapshot(connection: &Connection) -> Result<(), StoreError> {
    let (snapshot_count, recorded_count, event_count): (i64, Option<i64>, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_legacy_snapshot),
               (SELECT max(event_count) FROM usage_legacy_snapshot),
               (SELECT count(*) FROM usage_legacy_event)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    let valid = match (snapshot_count, recorded_count, event_count) {
        (0, None, 0) => true,
        (1, Some(recorded), observed) => recorded >= 0 && recorded == observed,
        _ => false,
    };
    if !valid {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn validate_schema(
    connection: &Connection,
    table_contracts: &[TableContract],
    index_contracts: &[IndexContract],
    trigger_contracts: &[TriggerContract],
    table_schema_sources: &[&str],
    column_overrides: &[TableContract],
) -> Result<(), StoreError> {
    let mut table_list = connection.prepare("PRAGMA table_list")?;
    let rows = table_list.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    let mut actual_tables = Vec::new();
    for row in rows {
        let (schema, name, kind, strict) = row?;
        if schema == "main" && kind == "table" && !name.starts_with("sqlite_") {
            if strict != 1 {
                return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
            }
            actual_tables.push(name);
        }
    }
    actual_tables.sort_unstable();
    let mut expected_tables = table_contracts
        .iter()
        .map(|contract| contract.name)
        .collect::<Vec<_>>();
    expected_tables.sort_unstable();
    if actual_tables != expected_tables {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }

    for contract in table_contracts {
        let column_contract = column_overrides
            .iter()
            .find(|override_contract| override_contract.name == contract.name)
            .unwrap_or(contract);
        let sql = format!("SELECT * FROM {} LIMIT 0", contract.name);
        let statement = connection.prepare(&sql)?;
        if statement.column_names().as_slice() != column_contract.columns {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
        let actual_sql: String = connection.query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [contract.name],
            |row| row.get(0),
        )?;
        let expected_sql = expected_table_sql(table_schema_sources, contract.name)
            .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
        if normalize_schema_sql(&actual_sql) != expected_sql {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
    }

    validate_named_sql(connection, "index", index_contracts)?;
    validate_triggers(connection, trigger_contracts)?;
    let foreign_key_failures: i64 =
        connection.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if foreign_key_failures != 0 {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    Ok(())
}

fn validate_named_sql(
    connection: &Connection,
    kind: &str,
    contracts: &[IndexContract],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(
        "SELECT name, sql FROM sqlite_schema
         WHERE type = ?1 AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let rows = statement.query_map([kind], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let actual = rows.collect::<Result<Vec<_>, _>>()?;
    if actual.len() != contracts.len() {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    for ((actual_name, actual_sql), expected) in actual.iter().zip(contracts) {
        if actual_name != expected.name || normalize_schema_sql(actual_sql) != expected.sql {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
    }
    Ok(())
}

fn validate_triggers(
    connection: &Connection,
    contracts: &[TriggerContract],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(
        "SELECT name, sql FROM sqlite_schema
         WHERE type = 'trigger' AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let actual = rows.collect::<Result<Vec<_>, _>>()?;
    if actual.len() != contracts.len() {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    for ((actual_name, actual_sql), expected) in actual.iter().zip(contracts) {
        if actual_name != expected.name || normalize_schema_sql(actual_sql) != expected.sql {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
    }
    Ok(())
}

fn count_application_objects(connection: &Connection) -> Result<i64, StoreError> {
    Ok(connection.query_row(
        "SELECT count(*) FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%' AND type IN ('table','index','trigger','view')",
        [],
        |row| row.get(0),
    )?)
}

fn normalize_schema_sql(sql: &str) -> String {
    let normalized = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some(quoted) = normalized.strip_prefix("CREATE TABLE \"") else {
        return normalized;
    };
    let Some((table_name, suffix)) = quoted.split_once("\" ") else {
        return normalized;
    };
    format!("CREATE TABLE {table_name} {suffix}")
}

fn expected_table_sql(schema_sources: &[&str], table_name: &str) -> Option<String> {
    let prefix = format!("CREATE TABLE {table_name} ");
    for source in schema_sources {
        for statement in source.split(';') {
            let normalized = normalize_schema_sql(statement);
            let canonical = normalized.replacen("CREATE TABLE IF NOT EXISTS ", "CREATE TABLE ", 1);
            if canonical.starts_with(&prefix) {
                return Some(canonical);
            }
        }
    }
    None
}

fn pragma_i64(connection: &Connection, sql: &str) -> Result<i64, StoreError> {
    Ok(connection.query_row(sql, [], |row| row.get(0))?)
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};

    use super::{
        MigrationFault, migrate_schema, migrate_v2_revision_table, migrate_v2_with_fault,
        migrate_v3_with_fault, pragma_i64, validate_v2, validate_v3, validate_v4,
    };
    use crate::{StoreErrorCode, usage::schema};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    const APPLICATION_TABLES: [&str; 14] = [
        "usage_scan",
        "usage_source",
        "usage_generation",
        "usage_source_chunk",
        "usage_observation",
        "usage_event",
        "usage_legacy_snapshot",
        "usage_legacy_event",
        "usage_replay_revision",
        "usage_replay_source",
        "usage_replay_session",
        "usage_replay_observation",
        "usage_replay_selection",
        "usage_replay_work",
    ];

    #[derive(Debug, Eq, PartialEq)]
    struct FixtureSnapshot {
        row_counts: Vec<(&'static str, i64)>,
        revision: (i64, String, i64, i64, i64, i64, i64, i64, i64),
        source: (Vec<u8>, Option<i64>, String),
        generations: Vec<(i64, String, String)>,
        replay_source: (Vec<u8>, i64, String),
        replay_session: (String, String, String, i64),
        replay_observation: (i64, String, String, i64),
        replay_selection: (Vec<u8>, i64, i64),
        replay_work: (String, String, i64, i64),
        legacy_event: (i64, Vec<u8>, String),
    }

    fn exact_v2_fixture(current_revision: bool) -> TestResult<Connection> {
        let mut connection = Connection::open_in_memory()?;
        connection.execute_batch(schema::V1_SCHEMA)?;
        connection.execute_batch(schema::REPLAY_AUX_SCHEMA)?;
        connection.execute_batch(schema::V2_REPLAY_REVISION_SCHEMA)?;
        connection.execute_batch(schema::REPLAY_CHILD_SCHEMA)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;

        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, verification_level
             ) VALUES (?1, 'codex', 'default', 'fixture', 'active', ?2, ?3, 'full_prefix')",
            params![
                [7_u8; 32].as_slice(),
                [8_u8; 32].as_slice(),
                [9_u8; 32].as_slice()
            ],
        )?;
        for (generation, status) in [(0_i64, "current"), (1_i64, "staging")] {
            transaction.execute(
                "INSERT INTO usage_generation(
                   file_key, generation, status, parser_schema_version,
                   physical_identity, logical_identity, committed_offset, scan_offset,
                   observed_file_length, modified_time_ns, anchor_start, anchor_len,
                   anchor_sha256, resume_payload, discarding_oversized_line,
                   incomplete_tail, verification_level
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, 1, 1, 1, 10, 0, 1,
                           ?6, zeroblob(0), 0, 0, 'full_prefix')",
                params![
                    [7_u8; 32].as_slice(),
                    generation,
                    status,
                    [9_u8; 32].as_slice(),
                    [8_u8; 32].as_slice(),
                    [10_u8; 32].as_slice(),
                ],
            )?;
            transaction.execute(
                "INSERT INTO usage_source_chunk(
                   file_key, generation, chunk_index, covered_len, sha256
                 ) VALUES (?1, ?2, 0, 1, ?3)",
                params![[7_u8; 32].as_slice(), generation, [11_u8; 32].as_slice()],
            )?;
            transaction.execute(
                "INSERT INTO usage_observation(
                   file_key, generation, source_offset, fingerprint, event_id,
                   profile_id, session_id, source_id, timestamp_seconds,
                   timestamp_nanos, model, raw_model, input_tokens, cached_tokens,
                   output_tokens, reasoning_tokens, total_tokens, fallback_model,
                   long_context, service_tier, project_alias, originator,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, ?2, 0, ?3, 'event', 'default', 'session', 'fixture', 1,
                   2, 'gpt-test', NULL, 3, 4, 5, 6, 18, 0, 'no', NULL, NULL,
                   NULL, 1, 2, 3, 4, 5, 6, 7, 8
                 )",
                params![[7_u8; 32].as_slice(), generation, [12_u8; 32].as_slice()],
            )?;
        }
        transaction.execute(
            "UPDATE usage_source SET current_generation = 0 WHERE file_key = ?1",
            [[7_u8; 32].as_slice()],
        )?;
        transaction.execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, service_tier, project_alias, originator,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) SELECT
               fingerprint, event_id, file_key, generation, source_offset, profile_id,
               session_id, source_id, timestamp_seconds, timestamp_nanos, model,
               raw_model, input_tokens, cached_tokens, output_tokens, reasoning_tokens,
               total_tokens, fallback_model, long_context, service_tier, project_alias,
               originator, activity_read, activity_edit_write, activity_search,
               activity_git, activity_build_test, activity_web, activity_subagents,
               activity_terminal
             FROM usage_observation WHERE generation = 0",
            [],
        )?;

        let (status, generation, sealed, promoted) = if current_revision {
            ("current", 0_i64, 1_i64, 1_i64)
        } else {
            ("staging", 1_i64, 0_i64, 0_i64)
        };
        transaction.execute(
            "INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted
             ) VALUES (5, ?1, 1, 1, 1, 1, 7, ?2, ?3)",
            params![status, sealed, promoted],
        )?;
        transaction.execute(
            "INSERT INTO usage_replay_source(revision_id, file_key, generation, state)
             VALUES (5, ?1, ?2, 'complete')",
            params![[7_u8; 32].as_slice(), generation],
        )?;
        transaction.execute(
            "INSERT INTO usage_replay_session(
               revision_id, provider_id, profile_id, session_id, parent_session_id,
               relation_conflict, state, completion_state, first_relation_file_key,
               first_relation_source_offset, last_classified_ordinal, evidence_epoch
             ) VALUES (
               5, 'codex', 'default', 'session', NULL, 0, 'root',
               'sealed_complete', NULL, NULL, 0, 7
             )",
            [],
        )?;
        transaction.execute(
            "INSERT INTO usage_replay_observation(
               revision_id, file_key, generation, source_offset, fingerprint,
               provider_id, profile_id, session_id, parent_session_id, session_ordinal,
               canonicalizer_version, fingerprint_version, replay_signature_version,
               replay_signature, evidence, disposition, declared_conflict, evidence_epoch
             ) VALUES (
               5, ?1, ?2, 0, ?3, 'codex', 'default', 'session', NULL, 0,
               1, 1, 1, ?4, 'strong_cumulative', 'eligible', 0, 7
             )",
            params![
                [7_u8; 32].as_slice(),
                generation,
                [12_u8; 32].as_slice(),
                [13_u8; 32].as_slice(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO usage_replay_selection(
               revision_id, fingerprint, file_key, generation, source_offset,
               canonicalizer_version, fingerprint_version, replay_signature_version
             ) VALUES (5, ?1, ?2, ?3, 0, 1, 1, 1)",
            params![[12_u8; 32].as_slice(), [7_u8; 32].as_slice(), generation],
        )?;
        transaction.execute(
            "INSERT INTO usage_replay_work(
               revision_id, work_kind, provider_id, profile_id, session_id, reason,
               next_ordinal, child_session_cursor, expected_evidence_epoch
             ) VALUES (
               5, 'classify_session', 'codex', 'default', 'session',
               'parent_changed', 1, NULL, 7
             )",
            [],
        )?;
        transaction.execute_batch(schema::LEGACY_COPY_SQL)?;
        transaction.execute_batch(schema::LEGACY_IMMUTABILITY_TRIGGERS)?;
        transaction.pragma_update(None, "user_version", schema::V2_SCHEMA_VERSION)?;
        transaction.commit()?;
        Ok(connection)
    }

    fn fixture_snapshot(connection: &Connection) -> TestResult<FixtureSnapshot> {
        let mut row_counts = Vec::with_capacity(APPLICATION_TABLES.len());
        for table in APPLICATION_TABLES {
            let count =
                connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
            row_counts.push((table, count));
        }
        let revision = connection.query_row(
            "SELECT revision_id, status, canonicalizer_version, fingerprint_version,
                    replay_signature_version, expected_source_count, evidence_epoch,
                    sealed, promoted
             FROM usage_replay_revision",
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
        )?;
        let source = connection.query_row(
            "SELECT file_key, current_generation, verification_level FROM usage_source",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let generations = connection
            .prepare(
                "SELECT generation, status, verification_level
                 FROM usage_generation ORDER BY generation",
            )?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        let replay_source = connection.query_row(
            "SELECT file_key, generation, state FROM usage_replay_source",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let replay_session = connection.query_row(
            "SELECT session_id, state, completion_state, evidence_epoch
             FROM usage_replay_session",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let replay_observation = connection.query_row(
            "SELECT generation, evidence, disposition, evidence_epoch
             FROM usage_replay_observation",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let replay_selection = connection.query_row(
            "SELECT fingerprint, generation, source_offset FROM usage_replay_selection",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let replay_work = connection.query_row(
            "SELECT work_kind, reason, next_ordinal, expected_evidence_epoch
             FROM usage_replay_work",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let legacy_event = connection.query_row(
            "SELECT snapshot_id, fingerprint, event_id FROM usage_legacy_event",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(FixtureSnapshot {
            row_counts,
            revision,
            source,
            generations,
            replay_source,
            replay_session,
            replay_observation,
            replay_selection,
            replay_work,
            legacy_event,
        })
    }

    fn exact_v3_fixture(current_revision: bool) -> TestResult<Connection> {
        let mut connection = exact_v2_fixture(current_revision)?;
        connection.pragma_update(None, "foreign_keys", "OFF")?;
        let transaction = connection.transaction()?;
        migrate_v2_revision_table(&transaction, MigrationFault::None)?;
        transaction.commit()?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        validate_v3(&connection)?;
        Ok(connection)
    }

    fn event_provenance(connection: &Connection) -> TestResult<(Option<i64>, Option<i64>, i64)> {
        Ok(connection.query_row(
            "SELECT projection_revision_id, origin_revision_id, retained FROM usage_event",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    }

    #[test]
    fn exact_v2_migrates_to_v4_and_preserves_all_rows() -> TestResult {
        let mut connection = exact_v2_fixture(true)?;
        let before = fixture_snapshot(&connection)?;
        migrate_schema(&mut connection)?;
        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 4);
        assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
        assert_eq!(fixture_snapshot(&connection)?, before);
        assert_eq!(event_provenance(&connection)?, (Some(5), Some(5), 0));
        validate_v4(&connection)?;
        let temporary_names: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE instr(sql, 'usage_replay_revision_v3') > 0
                OR instr(sql, 'usage_event_v4') > 0",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(temporary_names, 0);
        for statement in [
            "INSERT INTO usage_legacy_event DEFAULT VALUES",
            "UPDATE usage_legacy_event SET event_id = event_id WHERE snapshot_id = 1",
            "DELETE FROM usage_legacy_event WHERE snapshot_id = 1",
        ] {
            assert!(connection.execute(statement, []).is_err());
        }
        Ok(())
    }

    #[test]
    fn exact_v3_migrates_legacy_and_current_projection_provenance() -> TestResult {
        for (current_revision, expected) in [
            (false, (None, None, 0_i64)),
            (true, (Some(5_i64), Some(5_i64), 0_i64)),
        ] {
            let mut connection = exact_v3_fixture(current_revision)?;
            let before = fixture_snapshot(&connection)?;
            migrate_schema(&mut connection)?;
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 4);
            assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
            assert_eq!(fixture_snapshot(&connection)?, before);
            assert_eq!(event_provenance(&connection)?, expected);
            validate_v4(&connection)?;
        }
        Ok(())
    }

    #[test]
    fn every_v3_event_migration_fault_rolls_back_exactly() -> TestResult {
        for fault in [
            MigrationFault::AfterCreateEvent,
            MigrationFault::AfterCopyEvent,
            MigrationFault::AfterDropEvent,
        ] {
            let mut connection = exact_v3_fixture(true)?;
            let before = fixture_snapshot(&connection)?;
            let error = match migrate_v3_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted event migration unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 3);
            assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
            validate_v3(&connection)?;
            assert_eq!(fixture_snapshot(&connection)?, before);
            let temporary_names: i64 = connection.query_row(
                "SELECT count(*) FROM sqlite_schema
                 WHERE name = 'usage_event_v4' OR instr(sql, 'usage_event_v4') > 0",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(temporary_names, 0);
        }
        Ok(())
    }

    #[test]
    fn every_v2_migration_fault_rolls_back_and_restores_foreign_keys() -> TestResult {
        for fault in [
            MigrationFault::AfterCreateRevision,
            MigrationFault::AfterCopyRevision,
            MigrationFault::AfterDropRevision,
        ] {
            let mut connection = exact_v2_fixture(false)?;
            let before = fixture_snapshot(&connection)?;
            let error = match migrate_v2_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted migration unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 2);
            assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
            validate_v2(&connection)?;
            assert_eq!(fixture_snapshot(&connection)?, before);
        }
        Ok(())
    }

    #[test]
    fn malformed_v2_is_rejected_before_foreign_keys_are_disabled() -> TestResult {
        let mut connection = exact_v2_fixture(false)?;
        connection.execute("DROP INDEX usage_replay_revision_one_staging", [])?;
        let error = match migrate_v2_with_fault(&mut connection, MigrationFault::None) {
            Ok(()) => return Err("malformed v2 unexpectedly migrated".into()),
            Err(error) => error,
        };
        assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 2);
        assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
        Ok(())
    }
}
