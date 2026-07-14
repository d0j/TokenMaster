use rusqlite::{Connection, TransactionBehavior};

use crate::{StoreError, StoreErrorCode};

use super::schema::{
    IndexContract, LEGACY_COPY_SQL, LEGACY_IMMUTABILITY_TRIGGERS, TableContract, TriggerContract,
    USAGE_INDEX_CONTRACTS, USAGE_SCHEMA_VERSION, USAGE_TABLE_CONTRACTS, USAGE_TRIGGER_CONTRACTS,
    V1_INDEX_CONTRACTS, V1_SCHEMA, V1_SCHEMA_VERSION, V1_TABLE_COUNT, V2_REPLAY_SCHEMA,
};

pub(super) fn migrate_schema(connection: &mut Connection) -> Result<(), StoreError> {
    let version = pragma_i64(connection, "PRAGMA user_version")?;
    if version > USAGE_SCHEMA_VERSION {
        return Err(StoreError::new(StoreErrorCode::SchemaTooNew));
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match version {
        0 => create_fresh_v2(&transaction)?,
        V1_SCHEMA_VERSION => migrate_v1(&transaction)?,
        USAGE_SCHEMA_VERSION => validate_v2(&transaction)?,
        _ => return Err(StoreError::new(StoreErrorCode::SchemaMismatch)),
    }
    transaction.commit()?;
    Ok(())
}

fn create_fresh_v2(connection: &Connection) -> Result<(), StoreError> {
    if count_application_objects(connection)? != 0 {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    connection.execute_batch(V1_SCHEMA)?;
    connection.execute_batch(V2_REPLAY_SCHEMA)?;
    connection.execute_batch(LEGACY_IMMUTABILITY_TRIGGERS)?;
    connection.pragma_update(None, "user_version", USAGE_SCHEMA_VERSION)?;
    validate_v2(connection)
}

fn migrate_v1(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        &USAGE_TABLE_CONTRACTS[..V1_TABLE_COUNT],
        V1_INDEX_CONTRACTS,
        &[],
        &[V1_SCHEMA],
    )?;
    connection.execute_batch(V2_REPLAY_SCHEMA)?;
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
    connection.pragma_update(None, "user_version", USAGE_SCHEMA_VERSION)?;
    validate_v2(connection)
}

fn validate_v2(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        USAGE_TRIGGER_CONTRACTS,
        &[V1_SCHEMA, V2_REPLAY_SCHEMA],
    )?;
    validate_legacy_snapshot(connection)
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
        let sql = format!("SELECT * FROM {} LIMIT 0", contract.name);
        let statement = connection.prepare(&sql)?;
        if statement.column_names().as_slice() != contract.columns {
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
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
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
