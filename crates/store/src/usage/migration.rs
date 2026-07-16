use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{StoreError, StoreErrorCode};

use super::benefit_schema::{
    V11_BENEFIT_INDEX_CONTRACTS, V11_BENEFIT_SCHEMA, V11_BENEFIT_TABLE_CONTRACTS,
    V11_BENEFIT_TRIGGER_CONTRACTS,
};
use super::price_schema::{
    V9_PRICE_ROLLUP_SCHEMA, price_session_delete_trigger, price_session_insert_trigger,
    price_session_update_trigger, price_time_delete_trigger, price_time_insert_trigger,
    price_time_update_trigger,
};
use super::quota_schema::{
    V10_QUOTA_INDEX_CONTRACTS, V10_QUOTA_SCHEMA, V10_QUOTA_TABLE_CONTRACTS,
    V10_QUOTA_TRIGGER_CONTRACTS,
};
use super::schema::{
    IndexContract, LEGACY_COPY_SQL, LEGACY_IMMUTABILITY_TRIGGERS, LEGACY_TRIGGER_CONTRACTS,
    PRE_V4_USAGE_EVENT_CONTRACT, REPLAY_AUX_SCHEMA, REPLAY_CHILD_SCHEMA, TableContract,
    TriggerContract, USAGE_INDEX_CONTRACTS, USAGE_SCHEMA_VERSION, USAGE_TABLE_CONTRACTS,
    USAGE_TRIGGER_CONTRACTS, V1_INDEX_CONTRACTS, V1_SCHEMA, V1_SCHEMA_VERSION, V1_TABLE_COUNT,
    V2_REPLAY_REVISION_SCHEMA, V2_SCHEMA_VERSION, V3_REPLAY_REVISION_SCHEMA, V3_SCHEMA_VERSION,
    V4_SCHEMA_VERSION, V4_USAGE_EVENT_SCHEMA, V5_INDEX_CONTRACTS, V5_REPLAY_REVISION_CONTRACT,
    V5_REPLAY_REVISION_SCHEMA, V5_SCAN_SET_CONTRACT, V5_SCAN_SET_SCHEMA, V5_SCHEMA_VERSION,
    V5_USAGE_SCAN_CONTRACT, V5_USAGE_SCAN_SCHEMA, V6_ARCHIVE_STATE_CONTRACT,
    V6_ARCHIVE_STATE_SCHEMA, V6_SCHEMA_VERSION, V7_ARCHIVE_STATE_CONTRACT, V7_ARCHIVE_STATE_SCHEMA,
    V7_DATASET_GENERATION_TRIGGERS, V7_SCHEMA_VERSION, V8_AGGREGATE_SCHEMA,
    V8_AGGREGATE_STATE_CONTRACT, V8_DATASET_DELETE_TRIGGER, V8_DATASET_INSERT_TRIGGER,
    V8_DATASET_UPDATE_TRIGGER, V8_INDEX_CONTRACTS, V8_SCHEMA_VERSION, V8_SESSION_DELETE_TRIGGER,
    V8_SESSION_INSERT_TRIGGER, V8_SESSION_ROLLUP_CONTRACT, V8_TIME_DELETE_TRIGGER,
    V8_TIME_INSERT_TRIGGER, V8_TIME_ROLLUP_CONTRACT, V8_USAGE_EVENT_CONTRACT,
    V8_USAGE_EVENT_SCHEMA, V9_AGGREGATE_STATE_CONTRACT, V9_AGGREGATE_STATE_SCHEMA,
    V9_INDEX_CONTRACTS, V9_LEGACY_EVENT_CONTRACT, V9_OBSERVATION_CONTRACT,
    V9_PRICE_SESSION_ROLLUP_CONTRACT, V9_PRICE_TIME_ROLLUP_CONTRACT, V9_REPORTED_COST_TABLE_SCHEMA,
    V9_SCHEMA_VERSION, V9_USAGE_EVENT_CONTRACT, V10_SCHEMA_VERSION, v8_session_update_trigger,
    v8_time_update_trigger,
};
use super::{
    MAX_QUOTA_EPOCHS_PER_WINDOW, MAX_QUOTA_SAMPLES_PER_WINDOW, MAX_QUOTA_TRANSITIONS_PER_WINDOW,
};

pub(super) fn migrate_schema(connection: &mut Connection) -> Result<(), StoreError> {
    let version = pragma_i64(connection, "PRAGMA user_version")?;
    if version > USAGE_SCHEMA_VERSION {
        return Err(StoreError::new(StoreErrorCode::SchemaTooNew));
    }

    match version {
        V2_SCHEMA_VERSION => {
            migrate_v2(connection)?;
            migrate_v4(connection)?;
            migrate_v5(connection)?;
            migrate_v6(connection)?;
            migrate_v7(connection)?;
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V3_SCHEMA_VERSION => {
            migrate_v3(connection)?;
            migrate_v4(connection)?;
            migrate_v5(connection)?;
            migrate_v6(connection)?;
            migrate_v7(connection)?;
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V4_SCHEMA_VERSION => {
            migrate_v4(connection)?;
            migrate_v5(connection)?;
            migrate_v6(connection)?;
            migrate_v7(connection)?;
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        0 | V1_SCHEMA_VERSION => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            match version {
                0 => create_fresh_v4(&transaction)?,
                V1_SCHEMA_VERSION => migrate_v1(&transaction)?,
                _ => return Err(StoreError::new(StoreErrorCode::SchemaMismatch)),
            }
            transaction.commit()?;
            migrate_v4(connection)?;
            migrate_v5(connection)?;
            migrate_v6(connection)?;
            migrate_v7(connection)?;
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V5_SCHEMA_VERSION => {
            migrate_v5(connection)?;
            migrate_v6(connection)?;
            migrate_v7(connection)?;
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V6_SCHEMA_VERSION => {
            migrate_v6(connection)?;
            migrate_v7(connection)?;
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V7_SCHEMA_VERSION => {
            migrate_v7(connection)?;
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V8_SCHEMA_VERSION => {
            migrate_v8(connection)?;
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V9_SCHEMA_VERSION => {
            migrate_v9(connection)?;
            migrate_v10(connection)
        }
        V10_SCHEMA_VERSION => migrate_v10(connection),
        USAGE_SCHEMA_VERSION => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            validate_v11(&transaction)?;
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
    connection.pragma_update(None, "user_version", V4_SCHEMA_VERSION)?;
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
        SchemaExtensions::EMPTY,
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
    connection.pragma_update(None, "user_version", V4_SCHEMA_VERSION)?;
    validate_v4(connection)
}

fn validate_v2(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        LEGACY_TRIGGER_CONTRACTS,
        &[
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            V2_REPLAY_REVISION_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[PRE_V4_USAGE_EVENT_CONTRACT],
        SchemaExtensions::EMPTY,
    )?;
    validate_legacy_snapshot(connection)
}

fn validate_v3(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        LEGACY_TRIGGER_CONTRACTS,
        &[
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            V3_REPLAY_REVISION_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[PRE_V4_USAGE_EVENT_CONTRACT],
        SchemaExtensions::EMPTY,
    )?;
    validate_legacy_snapshot(connection)
}

fn validate_v4(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        LEGACY_TRIGGER_CONTRACTS,
        &[
            V4_USAGE_EVENT_SCHEMA,
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            V3_REPLAY_REVISION_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[],
        SchemaExtensions::EMPTY,
    )?;
    validate_legacy_snapshot(connection)
}

fn validate_v5(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        LEGACY_TRIGGER_CONTRACTS,
        &[
            V5_SCAN_SET_SCHEMA,
            V5_USAGE_SCAN_SCHEMA,
            V5_REPLAY_REVISION_SCHEMA,
            V4_USAGE_EVENT_SCHEMA,
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[V5_USAGE_SCAN_CONTRACT, V5_REPLAY_REVISION_CONTRACT],
        SchemaExtensions {
            tables: std::slice::from_ref(&V5_SCAN_SET_CONTRACT),
            indexes: V5_INDEX_CONTRACTS,
        },
    )?;
    validate_legacy_snapshot(connection)
}

pub(super) fn validate_v6(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        LEGACY_TRIGGER_CONTRACTS,
        &[
            V6_ARCHIVE_STATE_SCHEMA,
            V5_SCAN_SET_SCHEMA,
            V5_USAGE_SCAN_SCHEMA,
            V5_REPLAY_REVISION_SCHEMA,
            V4_USAGE_EVENT_SCHEMA,
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[V5_USAGE_SCAN_CONTRACT, V5_REPLAY_REVISION_CONTRACT],
        SchemaExtensions {
            tables: &[V5_SCAN_SET_CONTRACT, V6_ARCHIVE_STATE_CONTRACT],
            indexes: V5_INDEX_CONTRACTS,
        },
    )?;
    validate_legacy_snapshot(connection)?;
    validate_archive_publication(connection)
}

pub(super) fn validate_v7(connection: &Connection) -> Result<(), StoreError> {
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        USAGE_TRIGGER_CONTRACTS,
        &[
            V7_ARCHIVE_STATE_SCHEMA,
            V5_SCAN_SET_SCHEMA,
            V5_USAGE_SCAN_SCHEMA,
            V5_REPLAY_REVISION_SCHEMA,
            V4_USAGE_EVENT_SCHEMA,
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[V5_USAGE_SCAN_CONTRACT, V5_REPLAY_REVISION_CONTRACT],
        SchemaExtensions {
            tables: &[V5_SCAN_SET_CONTRACT, V7_ARCHIVE_STATE_CONTRACT],
            indexes: V5_INDEX_CONTRACTS,
        },
    )?;
    validate_legacy_snapshot(connection)?;
    validate_archive_publication(connection)
}

pub(super) fn validate_v8(connection: &Connection) -> Result<(), StoreError> {
    let session_update = v8_session_update_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let time_update =
        v8_time_update_trigger().ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let mut trigger_contracts = vec![
        TriggerContract {
            name: "usage_event_aggregate_session_after_delete",
            sql: V8_SESSION_DELETE_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_session_after_insert",
            sql: V8_SESSION_INSERT_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_session_after_update",
            sql: session_update,
        },
        TriggerContract {
            name: "usage_event_aggregate_time_after_delete",
            sql: V8_TIME_DELETE_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_time_after_insert",
            sql: V8_TIME_INSERT_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_time_after_update",
            sql: time_update,
        },
        TriggerContract {
            name: "usage_event_dataset_generation_after_delete",
            sql: V8_DATASET_DELETE_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_dataset_generation_after_insert",
            sql: V8_DATASET_INSERT_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_dataset_generation_after_update",
            sql: V8_DATASET_UPDATE_TRIGGER,
        },
    ];
    trigger_contracts.extend(LEGACY_TRIGGER_CONTRACTS.iter().copied());
    let mut indexes = V5_INDEX_CONTRACTS.to_vec();
    indexes.extend_from_slice(V8_INDEX_CONTRACTS);
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        &trigger_contracts,
        &[
            V8_AGGREGATE_SCHEMA,
            V8_USAGE_EVENT_SCHEMA,
            V7_ARCHIVE_STATE_SCHEMA,
            V5_SCAN_SET_SCHEMA,
            V5_USAGE_SCAN_SCHEMA,
            V5_REPLAY_REVISION_SCHEMA,
            V1_SCHEMA,
            REPLAY_AUX_SCHEMA,
            REPLAY_CHILD_SCHEMA,
        ],
        &[
            V5_USAGE_SCAN_CONTRACT,
            V5_REPLAY_REVISION_CONTRACT,
            V8_USAGE_EVENT_CONTRACT,
        ],
        SchemaExtensions {
            tables: &[
                V5_SCAN_SET_CONTRACT,
                V7_ARCHIVE_STATE_CONTRACT,
                V8_AGGREGATE_STATE_CONTRACT,
                V8_TIME_ROLLUP_CONTRACT,
                V8_SESSION_ROLLUP_CONTRACT,
            ],
            indexes: &indexes,
        },
    )?;
    validate_legacy_snapshot(connection)?;
    validate_archive_publication(connection)?;
    validate_aggregate_state(connection, 1)
}

pub(super) fn validate_v9(connection: &Connection) -> Result<(), StoreError> {
    validate_extended_schema(connection, false, false)
}

pub(super) fn validate_v10(connection: &Connection) -> Result<(), StoreError> {
    validate_extended_schema(connection, true, false)?;
    validate_quota_state(connection)
}

pub(super) fn validate_v11(connection: &Connection) -> Result<(), StoreError> {
    validate_extended_schema(connection, true, true)?;
    validate_quota_state(connection)?;
    validate_benefit_state(connection)
}

fn validate_extended_schema(
    connection: &Connection,
    include_quota: bool,
    include_benefit: bool,
) -> Result<(), StoreError> {
    let session_update = v8_session_update_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let time_update =
        v8_time_update_trigger().ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let price_session_delete = price_session_delete_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let price_session_insert = price_session_insert_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let price_session_update = price_session_update_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let price_time_delete = price_time_delete_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let price_time_insert = price_time_insert_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let price_time_update = price_time_update_trigger()
        .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?;
    let mut trigger_contracts = vec![
        TriggerContract {
            name: "usage_event_aggregate_session_after_delete",
            sql: V8_SESSION_DELETE_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_session_after_insert",
            sql: V8_SESSION_INSERT_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_session_after_update",
            sql: session_update,
        },
        TriggerContract {
            name: "usage_event_aggregate_time_after_delete",
            sql: V8_TIME_DELETE_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_time_after_insert",
            sql: V8_TIME_INSERT_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_aggregate_time_after_update",
            sql: time_update,
        },
        TriggerContract {
            name: "usage_event_dataset_generation_after_delete",
            sql: V8_DATASET_DELETE_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_dataset_generation_after_insert",
            sql: V8_DATASET_INSERT_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_dataset_generation_after_update",
            sql: V8_DATASET_UPDATE_TRIGGER,
        },
        TriggerContract {
            name: "usage_event_price_session_after_delete",
            sql: price_session_delete,
        },
        TriggerContract {
            name: "usage_event_price_session_after_insert",
            sql: price_session_insert,
        },
        TriggerContract {
            name: "usage_event_price_session_after_update",
            sql: price_session_update,
        },
        TriggerContract {
            name: "usage_event_price_time_after_delete",
            sql: price_time_delete,
        },
        TriggerContract {
            name: "usage_event_price_time_after_insert",
            sql: price_time_insert,
        },
        TriggerContract {
            name: "usage_event_price_time_after_update",
            sql: price_time_update,
        },
    ];
    trigger_contracts.extend(LEGACY_TRIGGER_CONTRACTS.iter().copied());
    if include_quota {
        trigger_contracts.extend(V10_QUOTA_TRIGGER_CONTRACTS.iter().copied());
    }
    if include_benefit {
        trigger_contracts.extend(V11_BENEFIT_TRIGGER_CONTRACTS.iter().copied());
    }
    trigger_contracts.sort_unstable_by_key(|contract| contract.name);
    let mut indexes = V5_INDEX_CONTRACTS.to_vec();
    indexes.extend_from_slice(V8_INDEX_CONTRACTS);
    indexes.extend_from_slice(V9_INDEX_CONTRACTS);
    if include_quota {
        indexes.extend_from_slice(V10_QUOTA_INDEX_CONTRACTS);
    }
    if include_benefit {
        indexes.extend_from_slice(V11_BENEFIT_INDEX_CONTRACTS);
    }
    let mut schema_sources = vec![
        V9_REPORTED_COST_TABLE_SCHEMA,
        V9_AGGREGATE_STATE_SCHEMA,
        V9_PRICE_ROLLUP_SCHEMA,
        V8_AGGREGATE_SCHEMA,
        V7_ARCHIVE_STATE_SCHEMA,
        V5_SCAN_SET_SCHEMA,
        V5_USAGE_SCAN_SCHEMA,
        V5_REPLAY_REVISION_SCHEMA,
        V1_SCHEMA,
        REPLAY_AUX_SCHEMA,
        REPLAY_CHILD_SCHEMA,
    ];
    let mut extension_tables = vec![
        V5_SCAN_SET_CONTRACT,
        V7_ARCHIVE_STATE_CONTRACT,
        V9_AGGREGATE_STATE_CONTRACT,
        V8_TIME_ROLLUP_CONTRACT,
        V8_SESSION_ROLLUP_CONTRACT,
        V9_PRICE_TIME_ROLLUP_CONTRACT,
        V9_PRICE_SESSION_ROLLUP_CONTRACT,
    ];
    if include_quota {
        schema_sources.push(V10_QUOTA_SCHEMA);
        extension_tables.extend_from_slice(V10_QUOTA_TABLE_CONTRACTS);
    }
    if include_benefit {
        schema_sources.push(V11_BENEFIT_SCHEMA);
        extension_tables.extend_from_slice(V11_BENEFIT_TABLE_CONTRACTS);
    }
    validate_schema(
        connection,
        USAGE_TABLE_CONTRACTS,
        USAGE_INDEX_CONTRACTS,
        &trigger_contracts,
        &schema_sources,
        &[
            V5_USAGE_SCAN_CONTRACT,
            V5_REPLAY_REVISION_CONTRACT,
            V9_OBSERVATION_CONTRACT,
            V9_USAGE_EVENT_CONTRACT,
            V9_LEGACY_EVENT_CONTRACT,
        ],
        SchemaExtensions {
            tables: &extension_tables,
            indexes: &indexes,
        },
    )?;
    validate_legacy_snapshot(connection)?;
    validate_archive_publication(connection)?;
    validate_aggregate_state(connection, 2)
}

fn validate_quota_state(connection: &Connection) -> Result<(), StoreError> {
    let valid = connection
        .query_row(
            "SELECT revision >= 0
                    AND retained_sample_count = (SELECT count(*) FROM quota_sample)
                    AND retained_epoch_count = (SELECT count(*) FROM quota_epoch_history)
                    AND retained_transition_count = (SELECT count(*) FROM quota_transition)
                    AND NOT EXISTS (
                      SELECT 1 FROM quota_sample
                      GROUP BY scope_id, window_id
                      HAVING count(*) > ?1
                    )
                    AND NOT EXISTS (
                      SELECT 1 FROM quota_epoch_history
                      GROUP BY scope_id, window_id
                      HAVING count(*) > ?2
                    )
                    AND NOT EXISTS (
                      SELECT 1 FROM quota_transition
                      GROUP BY scope_id, window_id
                      HAVING count(*) > ?3
                    )
                    AND NOT EXISTS (
                      SELECT 1
                      FROM quota_epoch_current AS epoch
                      LEFT JOIN quota_window_current AS current
                        ON current.scope_id = epoch.scope_id
                       AND current.window_id = epoch.window_id
                      LEFT JOIN quota_sample AS sample
                        ON sample.scope_id = epoch.scope_id
                       AND sample.window_id = epoch.window_id
                       AND sample.definition_revision = epoch.definition_revision
                       AND sample.observation_id = epoch.last_observation_id
                      WHERE current.scope_id IS NULL
                         OR sample.observation_id IS NULL
                         OR current.definition_revision <> epoch.definition_revision
                         OR current.sample_observation_id <> epoch.last_observation_id
                         OR current.epoch_id <> epoch.epoch_id
                         OR current.observed_at_ms <> sample.observed_at_ms
                         OR current.fresh_until_ms <> sample.fresh_until_ms
                         OR current.stale_after_ms <> sample.stale_after_ms
                         OR current.quality <> sample.quality
                         OR current.source <> sample.source
                         OR current.confidence <> sample.confidence
                         OR current.last_transition_sequence <> epoch.last_transition_sequence
                    )
                    AND ((revision = 0
                          AND last_published_at_ms IS NULL
                          AND NOT EXISTS (SELECT 1 FROM quota_window_current)
                          AND NOT EXISTS (SELECT 1 FROM quota_epoch_current))
                         OR (revision > 0 AND last_published_at_ms IS NOT NULL))
             FROM quota_state
             WHERE singleton_id = 1
               AND (SELECT count(*) FROM quota_state) = 1",
            params![
                MAX_QUOTA_SAMPLES_PER_WINDOW as i64,
                MAX_QUOTA_EPOCHS_PER_WINDOW as i64,
                MAX_QUOTA_TRANSITIONS_PER_WINDOW as i64,
            ],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false);
    if !valid {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn validate_benefit_state(connection: &Connection) -> Result<(), StoreError> {
    let valid = connection
        .query_row(
            "SELECT revision >= 0
                    AND current_lot_count = (SELECT count(*) FROM benefit_lot_current)
                    AND retained_change_count = (SELECT count(*) FROM benefit_change)
                    AND pending_due_count = (SELECT count(*) FROM benefit_reminder_due)
                    AND retained_delivery_count = (SELECT count(*) FROM benefit_reminder_delivery)
                    AND ((revision = 0 AND last_published_at_ms IS NULL)
                      OR (revision > 0 AND last_published_at_ms IS NOT NULL))
             FROM benefit_state WHERE singleton_id = 1",
            [],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false);
    if !valid {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let profiles_valid = connection.query_row(
        "SELECT
           (SELECT count(*) FROM benefit_reminder_profile
            WHERE profile_kind = 'global' AND length(profile_scope_id) = 0) = 1
           AND NOT EXISTS(
             SELECT 1 FROM benefit_reminder_profile AS profile
             WHERE (SELECT count(*) FROM benefit_reminder_threshold AS threshold
                    WHERE threshold.profile_kind = profile.profile_kind
                      AND threshold.profile_scope_id = profile.profile_scope_id) > 8
           )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !profiles_valid {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
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
    #[cfg(test)]
    AfterCreateScanAuthority,
    #[cfg(test)]
    AfterCopyScanAuthority,
    #[cfg(test)]
    AfterDropScanAuthority,
    #[cfg(test)]
    AfterCreateArchiveState,
    #[cfg(test)]
    AfterSeedArchiveState,
    #[cfg(test)]
    AfterCreateDatasetState,
    #[cfg(test)]
    AfterSeedDatasetState,
    #[cfg(test)]
    AfterCreateDatasetTriggers,
    #[cfg(test)]
    AfterCreateProviderEvent,
    #[cfg(test)]
    AfterCopyProviderEvent,
    #[cfg(test)]
    AfterDropProviderEvent,
    #[cfg(test)]
    AfterCreateAggregateSchema,
    #[cfg(test)]
    AfterSeedAggregateState,
    #[cfg(test)]
    AfterCreateAggregateTriggers,
    #[cfg(test)]
    AfterAddReportedCost,
    #[cfg(test)]
    AfterUpgradeAggregateState,
    #[cfg(test)]
    AfterCreatePriceSchema,
    #[cfg(test)]
    AfterCreatePriceTriggers,
    #[cfg(test)]
    AfterCreateQuotaSchema,
    #[cfg(test)]
    AfterCreateBenefitSchema,
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
    ScanAuthorityCreated,
    ScanAuthorityCopied,
    ScanAuthorityDropped,
    ArchiveStateCreated,
    ArchiveStateSeeded,
    DatasetStateCreated,
    DatasetStateSeeded,
    DatasetTriggersCreated,
    ProviderEventCreated,
    ProviderEventCopied,
    ProviderEventDropped,
    AggregateSchemaCreated,
    AggregateStateSeeded,
    AggregateTriggersCreated,
    ReportedCostAdded,
    AggregateStateUpgraded,
    PriceSchemaCreated,
    PriceTriggersCreated,
    QuotaSchemaCreated,
    BenefitSchemaCreated,
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
            | (
                MigrationFault::AfterCreateScanAuthority,
                MigrationBoundary::ScanAuthorityCreated
            )
            | (
                MigrationFault::AfterCopyScanAuthority,
                MigrationBoundary::ScanAuthorityCopied
            )
            | (
                MigrationFault::AfterDropScanAuthority,
                MigrationBoundary::ScanAuthorityDropped
            )
            | (
                MigrationFault::AfterCreateArchiveState,
                MigrationBoundary::ArchiveStateCreated
            )
            | (
                MigrationFault::AfterSeedArchiveState,
                MigrationBoundary::ArchiveStateSeeded
            )
            | (
                MigrationFault::AfterCreateDatasetState,
                MigrationBoundary::DatasetStateCreated
            )
            | (
                MigrationFault::AfterSeedDatasetState,
                MigrationBoundary::DatasetStateSeeded
            )
            | (
                MigrationFault::AfterCreateDatasetTriggers,
                MigrationBoundary::DatasetTriggersCreated
            )
            | (
                MigrationFault::AfterCreateProviderEvent,
                MigrationBoundary::ProviderEventCreated
            )
            | (
                MigrationFault::AfterCopyProviderEvent,
                MigrationBoundary::ProviderEventCopied
            )
            | (
                MigrationFault::AfterDropProviderEvent,
                MigrationBoundary::ProviderEventDropped
            )
            | (
                MigrationFault::AfterCreateAggregateSchema,
                MigrationBoundary::AggregateSchemaCreated
            )
            | (
                MigrationFault::AfterSeedAggregateState,
                MigrationBoundary::AggregateStateSeeded
            )
            | (
                MigrationFault::AfterCreateAggregateTriggers,
                MigrationBoundary::AggregateTriggersCreated
            )
            | (
                MigrationFault::AfterAddReportedCost,
                MigrationBoundary::ReportedCostAdded
            )
            | (
                MigrationFault::AfterUpgradeAggregateState,
                MigrationBoundary::AggregateStateUpgraded
            )
            | (
                MigrationFault::AfterCreatePriceSchema,
                MigrationBoundary::PriceSchemaCreated
            )
            | (
                MigrationFault::AfterCreatePriceTriggers,
                MigrationBoundary::PriceTriggersCreated
            )
            | (
                MigrationFault::AfterCreateQuotaSchema,
                MigrationBoundary::QuotaSchemaCreated
            )
            | (
                MigrationFault::AfterCreateBenefitSchema,
                MigrationBoundary::BenefitSchemaCreated
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
    transaction.pragma_update(None, "user_version", V4_SCHEMA_VERSION)?;
    validate_v4(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v4(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v4_with_fault(connection, MigrationFault::None)
}

fn migrate_v5(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v5_with_fault(connection, MigrationFault::None)
}

fn migrate_v6(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v6_with_fault(connection, MigrationFault::None)
}

fn migrate_v7(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v7_with_fault(connection, MigrationFault::None)
}

fn migrate_v8(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v8_with_fault(connection, MigrationFault::None)
}

fn migrate_v9(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v9_with_fault(connection, MigrationFault::None)
}

fn migrate_v10(connection: &mut Connection) -> Result<(), StoreError> {
    migrate_v10_with_fault(connection, MigrationFault::None)
}

fn migrate_v10_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v10(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(V11_BENEFIT_SCHEMA)?;
    migration_fault(fault, MigrationBoundary::BenefitSchemaCreated)?;
    transaction.pragma_update(None, "user_version", USAGE_SCHEMA_VERSION)?;
    validate_v11(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v9_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v9(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(V10_QUOTA_SCHEMA)?;
    migration_fault(fault, MigrationBoundary::QuotaSchemaCreated)?;
    transaction.pragma_update(None, "user_version", V10_SCHEMA_VERSION)?;
    validate_v10(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v8_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v8(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "DROP TRIGGER usage_event_aggregate_session_after_delete;
         DROP TRIGGER usage_event_aggregate_session_after_insert;
         DROP TRIGGER usage_event_aggregate_session_after_update;
         DROP TRIGGER usage_event_aggregate_time_after_delete;
         DROP TRIGGER usage_event_aggregate_time_after_insert;
         DROP TRIGGER usage_event_aggregate_time_after_update;
         DROP TRIGGER usage_event_dataset_generation_after_delete;
         DROP TRIGGER usage_event_dataset_generation_after_insert;
         DROP TRIGGER usage_event_dataset_generation_after_update;
         ALTER TABLE usage_observation ADD COLUMN
           reported_cost_usd_micros INTEGER
           CHECK(reported_cost_usd_micros IS NULL OR reported_cost_usd_micros >= 0);
         ALTER TABLE usage_event ADD COLUMN
           reported_cost_usd_micros INTEGER
           CHECK(reported_cost_usd_micros IS NULL OR reported_cost_usd_micros >= 0);
         ALTER TABLE usage_legacy_event ADD COLUMN
           reported_cost_usd_micros INTEGER
           CHECK(reported_cost_usd_micros IS NULL OR reported_cost_usd_micros >= 0);
         ALTER TABLE usage_aggregate_state RENAME TO usage_aggregate_state_v8;",
    )?;
    migration_fault(fault, MigrationBoundary::ReportedCostAdded)?;
    transaction.execute_batch(V9_AGGREGATE_STATE_SCHEMA)?;
    transaction.execute(
        "INSERT INTO usage_aggregate_state(
           singleton_id, aggregate_schema_version, state,
           expected_dataset_generation, active_aggregate_generation,
           current_event_count, legacy_event_count, rebuild_total_events
         )
         SELECT old.singleton_id, 2,
                CASE WHEN old.current_event_count = 0 AND old.legacy_event_count = 0
                     THEN 'ready' ELSE 'rebuild_required' END,
                archive.dataset_generation, old.active_aggregate_generation,
                old.current_event_count, old.legacy_event_count,
                old.current_event_count + old.legacy_event_count
         FROM usage_aggregate_state_v8 AS old
         JOIN usage_archive_state AS archive ON archive.singleton_id = old.singleton_id",
        [],
    )?;
    transaction.execute_batch("DROP TABLE usage_aggregate_state_v8;")?;
    migration_fault(fault, MigrationBoundary::AggregateStateUpgraded)?;
    transaction.execute_batch(V9_PRICE_ROLLUP_SCHEMA)?;
    migration_fault(fault, MigrationBoundary::PriceSchemaCreated)?;

    transaction.execute_batch(V8_SESSION_DELETE_TRIGGER)?;
    transaction.execute_batch(V8_SESSION_INSERT_TRIGGER)?;
    transaction.execute_batch(
        v8_session_update_trigger()
            .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?,
    )?;
    transaction.execute_batch(V8_TIME_DELETE_TRIGGER)?;
    transaction.execute_batch(V8_TIME_INSERT_TRIGGER)?;
    transaction.execute_batch(
        v8_time_update_trigger().ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?,
    )?;
    transaction.execute_batch(V8_DATASET_DELETE_TRIGGER)?;
    transaction.execute_batch(V8_DATASET_INSERT_TRIGGER)?;
    transaction.execute_batch(V8_DATASET_UPDATE_TRIGGER)?;
    for trigger in [
        price_session_delete_trigger(),
        price_session_insert_trigger(),
        price_session_update_trigger(),
        price_time_delete_trigger(),
        price_time_insert_trigger(),
        price_time_update_trigger(),
    ] {
        transaction.execute_batch(
            trigger.ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?,
        )?;
    }
    migration_fault(fault, MigrationBoundary::PriceTriggersCreated)?;
    transaction.pragma_update(None, "user_version", V9_SCHEMA_VERSION)?;
    validate_v9(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v7_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v7(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    migrate_usage_event_v8(&transaction, fault)?;
    transaction.execute_batch(V8_AGGREGATE_SCHEMA)?;
    migration_fault(fault, MigrationBoundary::AggregateSchemaCreated)?;
    transaction.execute(
        "INSERT INTO usage_aggregate_state(
           singleton_id, aggregate_schema_version, state,
           expected_dataset_generation, active_aggregate_generation,
           current_event_count, legacy_event_count,
           rebuild_total_events
         )
         SELECT 1, 1,
                CASE WHEN current_count = 0 AND legacy_count = 0
                     THEN 'ready' ELSE 'rebuild_required' END,
                dataset_generation, 0, current_count, legacy_count,
                current_count + legacy_count
         FROM (
           SELECT archive.dataset_generation,
                  (SELECT count(*) FROM usage_event) AS current_count,
                  (SELECT count(*) FROM usage_legacy_event
                   WHERE snapshot_id = 1) AS legacy_count
           FROM usage_archive_state AS archive WHERE archive.singleton_id = 1
         )",
        [],
    )?;
    migration_fault(fault, MigrationBoundary::AggregateStateSeeded)?;
    transaction.execute_batch(V8_SESSION_DELETE_TRIGGER)?;
    transaction.execute_batch(V8_SESSION_INSERT_TRIGGER)?;
    transaction.execute_batch(
        v8_session_update_trigger()
            .ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?,
    )?;
    transaction.execute_batch(V8_TIME_DELETE_TRIGGER)?;
    transaction.execute_batch(V8_TIME_INSERT_TRIGGER)?;
    transaction.execute_batch(
        v8_time_update_trigger().ok_or_else(|| StoreError::new(StoreErrorCode::SchemaMismatch))?,
    )?;
    transaction.execute_batch(V8_DATASET_DELETE_TRIGGER)?;
    transaction.execute_batch(V8_DATASET_INSERT_TRIGGER)?;
    transaction.execute_batch(V8_DATASET_UPDATE_TRIGGER)?;
    migration_fault(fault, MigrationBoundary::AggregateTriggersCreated)?;
    transaction.pragma_update(None, "user_version", V8_SCHEMA_VERSION)?;
    validate_v8(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v6_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v6(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction
        .execute_batch("ALTER TABLE usage_archive_state RENAME TO usage_archive_state_v6;")?;
    transaction.execute_batch(V7_ARCHIVE_STATE_SCHEMA)?;
    migration_fault(fault, MigrationBoundary::DatasetStateCreated)?;
    transaction.execute(
        "INSERT INTO usage_archive_state(
           singleton_id, archive_generation, dataset_generation, current_revision_id,
           latest_complete_scan_set_id, incremental_state
         )
         SELECT singleton_id, archive_generation,
                CASE WHEN current_revision_id IS NULL
                     THEN 0 ELSE (SELECT count(*) FROM usage_event) END,
                current_revision_id, latest_complete_scan_set_id, incremental_state
         FROM usage_archive_state_v6",
        [],
    )?;
    transaction.execute_batch("DROP TABLE usage_archive_state_v6;")?;
    migration_fault(fault, MigrationBoundary::DatasetStateSeeded)?;
    transaction.execute_batch(V7_DATASET_GENERATION_TRIGGERS)?;
    migration_fault(fault, MigrationBoundary::DatasetTriggersCreated)?;
    transaction.pragma_update(None, "user_version", V7_SCHEMA_VERSION)?;
    validate_v7(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v5_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v5(connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(V6_ARCHIVE_STATE_SCHEMA)?;
    migration_fault(fault, MigrationBoundary::ArchiveStateCreated)?;

    let current: Option<(i64, Option<i64>)> = transaction
        .query_row(
            "SELECT revision_id, scan_set_id
             FROM usage_replay_revision WHERE status = 'current'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (current_revision, complete_scan_set, quality) = match current {
        None => (None, None, "empty"),
        Some((revision_id, scan_set_id)) => {
            let exact_complete = match scan_set_id {
                Some(scan_set_id) => transaction
                    .query_row(
                        "SELECT completion_state = 'complete'
                            AND (SELECT count(*) FROM usage_scan
                                 WHERE scan_set_id = usage_scan_set.scan_set_id)
                                = expected_scope_count
                            AND NOT EXISTS (
                              SELECT 1 FROM usage_scan
                              WHERE scan_set_id = usage_scan_set.scan_set_id
                                AND completion_state <> 'complete'
                            )
                     FROM usage_scan_set WHERE scan_set_id = ?1",
                        [scan_set_id],
                        |row| row.get::<_, bool>(0),
                    )
                    .optional()?
                    .unwrap_or(false),
                None => false,
            };
            if exact_complete {
                (Some(revision_id), scan_set_id, "complete")
            } else {
                (Some(revision_id), None, "recovery_pending")
            }
        }
    };
    transaction.execute(
        "INSERT INTO usage_archive_state(
           singleton_id, archive_generation, current_revision_id,
           latest_complete_scan_set_id, incremental_state
         ) VALUES (1, 0, ?1, ?2, ?3)",
        rusqlite::params![current_revision, complete_scan_set, quality],
    )?;
    migration_fault(fault, MigrationBoundary::ArchiveStateSeeded)?;
    transaction.pragma_update(None, "user_version", V6_SCHEMA_VERSION)?;
    validate_v6(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_v4_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v4(connection)?;
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
        migrate_scan_authority_v5(&transaction, fault)?;
        transaction.commit()?;
        Ok(())
    })();
    let restored = restore_foreign_keys(connection);
    match (migration, restored) {
        (Ok(()), Ok(())) => validate_v5(connection),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(_), Err(_)) => Err(StoreError::new(StoreErrorCode::PolicyMismatch)),
    }
}

fn migrate_scan_authority_v5(
    connection: &Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    let invalid_scan_state: i64 = connection.query_row(
        "SELECT count(*) FROM usage_scan
         WHERE (completion_state = 'running' AND completed_at_ms IS NOT NULL)
            OR (completion_state <> 'running' AND completed_at_ms IS NULL)
            OR (completed_at_ms IS NOT NULL AND completed_at_ms < started_at_ms)
            OR EXISTS (
              SELECT 1 FROM usage_source AS source
              WHERE source.last_seen_scan_id = usage_scan.scan_id
                AND source.profile_id <> usage_scan.profile_id
            )
            OR 1 < (
              SELECT count(DISTINCT source.provider_id)
              FROM usage_source AS source
              WHERE source.last_seen_scan_id = usage_scan.scan_id
            )",
        [],
        |row| row.get(0),
    )?;
    let running_scans: i64 = connection.query_row(
        "SELECT count(*) FROM usage_scan WHERE completion_state = 'running'",
        [],
        |row| row.get(0),
    )?;
    if invalid_scan_state != 0 || running_scans > 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }

    let scan_count = pragma_i64(connection, "SELECT count(*) FROM usage_scan")?;
    let revision_count = pragma_i64(connection, "SELECT count(*) FROM usage_replay_revision")?;
    let temporary_scan_schema = V5_USAGE_SCAN_SCHEMA.replacen(
        "CREATE TABLE usage_scan (",
        "CREATE TABLE usage_scan_v5 (",
        1,
    );
    let temporary_revision_schema = V5_REPLAY_REVISION_SCHEMA.replacen(
        "CREATE TABLE usage_replay_revision (",
        "CREATE TABLE usage_replay_revision_v5 (",
        1,
    );
    connection.execute_batch(V5_SCAN_SET_SCHEMA)?;
    connection.execute_batch(&temporary_scan_schema)?;
    connection.execute_batch(&temporary_revision_schema)?;
    migration_fault(fault, MigrationBoundary::ScanAuthorityCreated)?;

    connection.execute_batch(
        "INSERT INTO usage_scan_set(
           scan_set_id, started_at_ms, completed_at_ms, completion_state,
           expected_scope_count
         )
         SELECT scan_id, started_at_ms, completed_at_ms, completion_state, 1
         FROM usage_scan;

         INSERT INTO usage_scan_v5(
           scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
           completed_at_ms, completion_state, sources_seen, files_read,
           bytes_read, events_observed, diagnostics
         )
         SELECT
           scan.scan_id, scan.scan_id,
           COALESCE((
             SELECT min(source.provider_id)
             FROM usage_source AS source
             WHERE source.last_seen_scan_id = scan.scan_id
           ), 'legacy-unverified'),
           scan.profile_id, scan.started_at_ms, scan.completed_at_ms,
           scan.completion_state, scan.sources_seen, scan.files_read,
           scan.bytes_read, scan.events_observed, scan.diagnostics
         FROM usage_scan AS scan;

         INSERT INTO usage_replay_revision_v5(
           revision_id, status, canonicalizer_version, fingerprint_version,
           replay_signature_version, expected_source_count, evidence_epoch,
           sealed, promoted, scan_set_id
         )
         SELECT
           revision_id, status, canonicalizer_version, fingerprint_version,
           replay_signature_version, expected_source_count, evidence_epoch,
           sealed, promoted, NULL
         FROM usage_replay_revision;",
    )?;
    let copied: (i64, i64, i64) = connection.query_row(
        "SELECT
           (SELECT count(*) FROM usage_scan_set),
           (SELECT count(*) FROM usage_scan_v5),
           (SELECT count(*) FROM usage_replay_revision_v5)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let scan_difference = pragma_i64(
        connection,
        "SELECT count(*) FROM (
           SELECT scan_id, profile_id, started_at_ms, completed_at_ms,
                  completion_state, sources_seen, files_read, bytes_read,
                  events_observed, diagnostics
           FROM usage_scan
           EXCEPT
           SELECT scan_id, profile_id, started_at_ms, completed_at_ms,
                  completion_state, sources_seen, files_read, bytes_read,
                  events_observed, diagnostics
           FROM usage_scan_v5
         )",
    )?;
    let revision_difference = pragma_i64(
        connection,
        "SELECT count(*) FROM (
           SELECT revision_id, status, canonicalizer_version,
                  fingerprint_version, replay_signature_version,
                  expected_source_count, evidence_epoch, sealed, promoted
           FROM usage_replay_revision
           EXCEPT
           SELECT revision_id, status, canonicalizer_version,
                  fingerprint_version, replay_signature_version,
                  expected_source_count, evidence_epoch, sealed, promoted
           FROM usage_replay_revision_v5
         )",
    )?;
    if scan_count < 0
        || revision_count < 0
        || copied != (scan_count, scan_count, revision_count)
        || scan_difference != 0
        || revision_difference != 0
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    migration_fault(fault, MigrationBoundary::ScanAuthorityCopied)?;

    connection.execute_batch(
        "DROP TABLE usage_scan;
         DROP TABLE usage_replay_revision;",
    )?;
    migration_fault(fault, MigrationBoundary::ScanAuthorityDropped)?;
    connection.execute_batch(
        "ALTER TABLE usage_scan_v5 RENAME TO usage_scan;
         ALTER TABLE usage_replay_revision_v5 RENAME TO usage_replay_revision;
         CREATE UNIQUE INDEX usage_replay_revision_one_current
           ON usage_replay_revision(status) WHERE status = 'current';
         CREATE UNIQUE INDEX usage_replay_revision_one_staging
           ON usage_replay_revision(status) WHERE status = 'staging';
         CREATE UNIQUE INDEX usage_scan_one_running_scope
           ON usage_scan(provider_id, profile_id) WHERE completion_state = 'running';
         CREATE UNIQUE INDEX usage_scan_set_one_running
           ON usage_scan_set(completion_state) WHERE completion_state = 'running';
         CREATE INDEX usage_source_scope_missing
           ON usage_source(provider_id, profile_id, missing, file_key);",
    )?;
    connection.pragma_update(None, "user_version", V5_SCHEMA_VERSION)?;
    validate_v5(connection)
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

fn migrate_usage_event_v8(
    connection: &Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    let source_profile_mismatches = pragma_i64(
        connection,
        "SELECT count(*)
         FROM usage_event AS event
         JOIN usage_source AS source ON source.file_key = event.selected_file_key
         WHERE source.profile_id <> event.profile_id",
    )?;
    if source_profile_mismatches != 0 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }

    let old_count = pragma_i64(connection, "SELECT count(*) FROM usage_event")?;
    let temporary_schema = V8_USAGE_EVENT_SCHEMA.replacen(
        "CREATE TABLE usage_event (",
        "CREATE TABLE usage_event_v8 (",
        1,
    );
    connection.execute_batch(&temporary_schema)?;
    migration_fault(fault, MigrationBoundary::ProviderEventCreated)?;
    connection.execute_batch(
        "INSERT INTO usage_event_v8(
           fingerprint, event_id, selected_file_key, selected_generation,
           selected_source_offset, projection_revision_id, origin_revision_id,
           retained, provider_id, profile_id, session_id, source_id,
           timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
           cached_tokens, output_tokens, reasoning_tokens, total_tokens,
           fallback_model, long_context, service_tier, project_alias, originator,
           activity_read, activity_edit_write, activity_search, activity_git,
           activity_build_test, activity_web, activity_subagents, activity_terminal
         )
         SELECT
           event.fingerprint, event.event_id, event.selected_file_key,
           event.selected_generation, event.selected_source_offset,
           event.projection_revision_id, event.origin_revision_id, event.retained,
           coalesce(source.provider_id, 'unknown'), event.profile_id,
           event.session_id, event.source_id, event.timestamp_seconds,
           event.timestamp_nanos, event.model, event.raw_model, event.input_tokens,
           event.cached_tokens, event.output_tokens, event.reasoning_tokens,
           event.total_tokens, event.fallback_model, event.long_context,
           event.service_tier, event.project_alias, event.originator,
           event.activity_read, event.activity_edit_write, event.activity_search,
           event.activity_git, event.activity_build_test, event.activity_web,
           event.activity_subagents, event.activity_terminal
         FROM usage_event AS event
         LEFT JOIN usage_source AS source ON source.file_key = event.selected_file_key;",
    )?;
    let new_count = pragma_i64(connection, "SELECT count(*) FROM usage_event_v8")?;
    let logical_difference = pragma_i64(
        connection,
        "SELECT count(*) FROM (
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, projection_revision_id,
                  origin_revision_id, retained, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model,
                  input_tokens, cached_tokens, output_tokens, reasoning_tokens,
                  total_tokens, fallback_model, long_context, service_tier,
                  project_alias, originator, activity_read, activity_edit_write,
                  activity_search, activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event
           EXCEPT
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, projection_revision_id,
                  origin_revision_id, retained, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model,
                  input_tokens, cached_tokens, output_tokens, reasoning_tokens,
                  total_tokens, fallback_model, long_context, service_tier,
                  project_alias, originator, activity_read, activity_edit_write,
                  activity_search, activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event_v8
           UNION ALL
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, projection_revision_id,
                  origin_revision_id, retained, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model,
                  input_tokens, cached_tokens, output_tokens, reasoning_tokens,
                  total_tokens, fallback_model, long_context, service_tier,
                  project_alias, originator, activity_read, activity_edit_write,
                  activity_search, activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event_v8
           EXCEPT
           SELECT fingerprint, event_id, selected_file_key, selected_generation,
                  selected_source_offset, projection_revision_id,
                  origin_revision_id, retained, profile_id, session_id, source_id,
                  timestamp_seconds, timestamp_nanos, model, raw_model,
                  input_tokens, cached_tokens, output_tokens, reasoning_tokens,
                  total_tokens, fallback_model, long_context, service_tier,
                  project_alias, originator, activity_read, activity_edit_write,
                  activity_search, activity_git, activity_build_test, activity_web,
                  activity_subagents, activity_terminal
           FROM usage_event
         )",
    )?;
    let provider_difference = pragma_i64(
        connection,
        "SELECT count(*)
         FROM usage_event_v8 AS event
         LEFT JOIN usage_source AS source ON source.file_key = event.selected_file_key
         WHERE event.provider_id <> coalesce(source.provider_id, 'unknown')",
    )?;
    if old_count < 0
        || old_count != new_count
        || logical_difference != 0
        || provider_difference != 0
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    migration_fault(fault, MigrationBoundary::ProviderEventCopied)?;
    connection.execute_batch("DROP TABLE usage_event;")?;
    migration_fault(fault, MigrationBoundary::ProviderEventDropped)?;
    connection.execute_batch(
        "ALTER TABLE usage_event_v8 RENAME TO usage_event;
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

fn validate_archive_publication(connection: &Connection) -> Result<(), StoreError> {
    let row_count = pragma_i64(
        connection,
        "SELECT count(*) FROM usage_archive_state WHERE singleton_id = 1",
    )?;
    if row_count != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let invalid = pragma_i64(
        connection,
        "SELECT count(*)
         FROM usage_archive_state AS state
         LEFT JOIN usage_replay_revision AS revision
           ON revision.revision_id = state.current_revision_id
          AND revision.status = 'current'
         LEFT JOIN usage_scan_set AS scan_set
           ON scan_set.scan_set_id = state.latest_complete_scan_set_id
         WHERE state.singleton_id <> 1
            OR (state.current_revision_id IS NOT NULL AND revision.revision_id IS NULL)
            OR (state.latest_complete_scan_set_id IS NOT NULL AND (
                 scan_set.scan_set_id IS NULL
                 OR scan_set.completion_state <> 'complete'
                 OR revision.scan_set_id <> state.latest_complete_scan_set_id
                 OR (SELECT count(*) FROM usage_scan
                     WHERE scan_set_id = scan_set.scan_set_id)
                    <> scan_set.expected_scope_count
                 OR EXISTS (
                   SELECT 1 FROM usage_scan
                   WHERE scan_set_id = scan_set.scan_set_id
                     AND completion_state <> 'complete'
                 )
               ))
            OR (state.incremental_state = 'empty' AND (
                 state.current_revision_id IS NOT NULL
                 OR state.latest_complete_scan_set_id IS NOT NULL
               ))
            OR (state.incremental_state = 'complete' AND (
                 state.current_revision_id IS NULL
                 OR state.latest_complete_scan_set_id IS NULL
               ))
            OR (state.incremental_state IN ('partial','recovery_pending')
                AND state.current_revision_id IS NULL)",
    )?;
    if invalid != 0 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn validate_aggregate_state(
    connection: &Connection,
    expected_schema_version: i64,
) -> Result<(), StoreError> {
    let state: Option<(i64, String, i64, i64, i64, i64)> = connection
        .query_row(
            "SELECT aggregate.aggregate_schema_version, aggregate.state,
                    aggregate.expected_dataset_generation,
                    aggregate.current_event_count, aggregate.legacy_event_count,
                    aggregate.rebuild_total_events
             FROM usage_aggregate_state AS aggregate
             JOIN usage_archive_state AS archive ON archive.singleton_id = 1
             WHERE aggregate.singleton_id = 1
               AND aggregate.expected_dataset_generation = archive.dataset_generation",
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
        .optional()?;
    let Some((schema_version, state, generation, current_count, legacy_count, total_count)) = state
    else {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    };
    let expected_total = current_count
        .checked_add(legacy_count)
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if schema_version != expected_schema_version
        || generation < 0
        || current_count < 0
        || legacy_count < 0
        || total_count != expected_total
        || !matches!(
            state.as_str(),
            "ready" | "rebuild_required" | "rebuilding" | "failed"
        )
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let legacy_snapshot_count: Option<i64> = connection
        .query_row(
            "SELECT event_count FROM usage_legacy_snapshot WHERE snapshot_id = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if legacy_count != legacy_snapshot_count.unwrap_or(0) {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SchemaExtensions<'a> {
    tables: &'a [TableContract],
    indexes: &'a [IndexContract],
}

impl SchemaExtensions<'_> {
    const EMPTY: Self = Self {
        tables: &[],
        indexes: &[],
    };
}

fn validate_schema(
    connection: &Connection,
    table_contracts: &[TableContract],
    index_contracts: &[IndexContract],
    trigger_contracts: &[TriggerContract<'_>],
    table_schema_sources: &[&str],
    column_overrides: &[TableContract],
    extensions: SchemaExtensions<'_>,
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
        .chain(extensions.tables)
        .map(|contract| contract.name)
        .collect::<Vec<_>>();
    expected_tables.sort_unstable();
    if actual_tables != expected_tables {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }

    for contract in table_contracts.iter().chain(extensions.tables) {
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
        if !normalized_schema_sql_equal(&actual_sql, expected_sql) {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
    }

    validate_named_sql(connection, "index", index_contracts, extensions.indexes)?;
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
    extra_contracts: &[IndexContract],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(
        "SELECT name, sql FROM sqlite_schema
         WHERE type = ?1 AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let rows = statement.query_map([kind], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut expected = contracts.iter().chain(extra_contracts).collect::<Vec<_>>();
    expected.sort_unstable_by_key(|contract| contract.name);
    let mut actual = rows;
    for expected in expected {
        let Some(row) = actual.next() else {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        };
        let (actual_name, actual_sql) = row?;
        if actual_name != expected.name || !normalized_schema_sql_equal(&actual_sql, expected.sql) {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
    }
    if actual.next().transpose()?.is_some() {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
    }
    Ok(())
}

fn validate_triggers(
    connection: &Connection,
    contracts: &[TriggerContract<'_>],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(
        "SELECT name, sql FROM sqlite_schema
         WHERE type = 'trigger' AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut actual = rows;
    for expected in contracts {
        let Some(row) = actual.next() else {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        };
        let (actual_name, actual_sql) = row?;
        if actual_name != expected.name || !normalized_schema_sql_equal(&actual_sql, expected.sql) {
            return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
        }
    }
    if actual.next().transpose()?.is_some() {
        return Err(StoreError::new(StoreErrorCode::SchemaMismatch));
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

struct NormalizedSchemaTokens<'a> {
    tokens: std::iter::Peekable<std::str::SplitWhitespace<'a>>,
    index: usize,
}

impl<'a> NormalizedSchemaTokens<'a> {
    fn new(sql: &'a str) -> Self {
        Self {
            tokens: sql.split_whitespace().peekable(),
            index: 0,
        }
    }

    fn next(&mut self) -> Option<&'a str> {
        let mut token = self.tokens.next()?;
        if self.index == 2 && token == "IF" {
            if self.tokens.next()? != "NOT" || self.tokens.next()? != "EXISTS" {
                return None;
            }
            token = self.tokens.next()?;
        }
        if self.index == 2 {
            token = token
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(token);
        }
        if self.tokens.peek().is_none() {
            token = token.trim_end_matches(';');
        }
        self.index += 1;
        Some(token)
    }
}

fn normalized_schema_sql_equal(left: &str, right: &str) -> bool {
    let mut left = NormalizedSchemaTokens::new(left);
    let mut right = NormalizedSchemaTokens::new(right);
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) if left == right => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn table_statement_name(statement: &str) -> Option<&str> {
    let mut tokens = statement.split_whitespace();
    if tokens.next()? != "CREATE" || tokens.next()? != "TABLE" {
        return None;
    }
    let mut name = tokens.next()?;
    if name == "IF" {
        if tokens.next()? != "NOT" || tokens.next()? != "EXISTS" {
            return None;
        }
        name = tokens.next()?;
    }
    Some(name.trim_matches('"'))
}

fn expected_table_sql<'a>(schema_sources: &[&'a str], table_name: &str) -> Option<&'a str> {
    for source in schema_sources {
        for statement in source.split(';') {
            if table_statement_name(statement) == Some(table_name) {
                return Some(statement);
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
        migrate_v3_with_fault, migrate_v4_with_fault, migrate_v5_with_fault, migrate_v6,
        migrate_v6_with_fault, migrate_v7_with_fault, migrate_v8_with_fault, migrate_v9_with_fault,
        migrate_v10_with_fault, pragma_i64, validate_v2, validate_v3, validate_v4, validate_v5,
        validate_v6, validate_v7, validate_v8, validate_v9, validate_v10, validate_v11,
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

    fn exact_v4_fixture_with_scan() -> TestResult<Connection> {
        let mut connection = exact_v3_fixture(true)?;
        migrate_v3_with_fault(&mut connection, MigrationFault::None)?;
        connection.execute(
            "INSERT INTO usage_scan(
               scan_id, profile_id, started_at_ms, completed_at_ms,
               completion_state, sources_seen, files_read, bytes_read,
               events_observed, diagnostics
             ) VALUES (9, 'default', 100, 200, 'complete', 1, 2, 3, 4, 5)",
            [],
        )?;
        connection.execute(
            "UPDATE usage_source SET last_seen_scan_id = 9, missing = 1
             WHERE file_key = ?1",
            [[7_u8; 32].as_slice()],
        )?;
        validate_v4(&connection)?;
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
    fn exact_v2_migrates_to_current_schema_and_preserves_all_rows() -> TestResult {
        let mut connection = exact_v2_fixture(true)?;
        let before = fixture_snapshot(&connection)?;
        migrate_schema(&mut connection)?;
        assert_eq!(
            pragma_i64(&connection, "PRAGMA user_version")?,
            schema::USAGE_SCHEMA_VERSION
        );
        assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
        assert_eq!(fixture_snapshot(&connection)?, before);
        assert_eq!(event_provenance(&connection)?, (Some(5), Some(5), 0));
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state"
            )?,
            1
        );
        validate_v11(&connection)?;
        let temporary_names: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE instr(sql, 'usage_replay_revision_v3') > 0
                OR instr(sql, 'usage_event_v4') > 0
                OR instr(sql, 'usage_event_v8') > 0",
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
    fn exact_v3_migrates_to_current_schema_with_projection_provenance() -> TestResult {
        for (current_revision, expected) in [
            (false, (None, None, 0_i64)),
            (true, (Some(5_i64), Some(5_i64), 0_i64)),
        ] {
            let mut connection = exact_v3_fixture(current_revision)?;
            let before = fixture_snapshot(&connection)?;
            migrate_schema(&mut connection)?;
            assert_eq!(
                pragma_i64(&connection, "PRAGMA user_version")?,
                schema::USAGE_SCHEMA_VERSION
            );
            assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
            assert_eq!(fixture_snapshot(&connection)?, before);
            assert_eq!(event_provenance(&connection)?, expected);
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT dataset_generation FROM usage_archive_state"
                )?,
                i64::from(current_revision)
            );
            validate_v11(&connection)?;
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
    fn exact_v4_scan_and_revision_migrate_to_current_scoped_schema() -> TestResult {
        let mut connection = exact_v4_fixture_with_scan()?;
        migrate_schema(&mut connection)?;
        assert_eq!(
            pragma_i64(&connection, "PRAGMA user_version")?,
            schema::USAGE_SCHEMA_VERSION
        );
        assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
        let scan: (
            i64,
            i64,
            String,
            String,
            i64,
            i64,
            String,
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = connection.query_row(
            "SELECT scan_id, scan_set_id, provider_id, profile_id,
                        started_at_ms, completed_at_ms, completion_state,
                        sources_seen, files_read, bytes_read, events_observed,
                        diagnostics
                 FROM usage_scan WHERE scan_id = 9",
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
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )?;
        assert_eq!(
            scan,
            (
                9,
                9,
                "codex".to_owned(),
                "default".to_owned(),
                100,
                200,
                "complete".to_owned(),
                1,
                2,
                3,
                4,
                5,
            )
        );
        let scan_set: (i64, i64, i64, String, i64) = connection.query_row(
            "SELECT scan_set_id, started_at_ms, completed_at_ms,
                    completion_state, expected_scope_count
             FROM usage_scan_set WHERE scan_set_id = 9",
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
        )?;
        assert_eq!(scan_set, (9, 100, 200, "complete".to_owned(), 1));
        let source: (i64, i64) = connection.query_row(
            "SELECT last_seen_scan_id, missing FROM usage_source",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert_eq!(source, (9, 1));
        let revision_scan_set: Option<i64> =
            connection.query_row("SELECT scan_set_id FROM usage_replay_revision", [], |row| {
                row.get(0)
            })?;
        assert_eq!(revision_scan_set, None);
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state"
            )?,
            1
        );
        validate_v11(&connection)?;
        Ok(())
    }

    fn exact_v5_fixture(complete_publication: bool) -> TestResult<Connection> {
        let mut connection = exact_v4_fixture_with_scan()?;
        migrate_v4_with_fault(&mut connection, MigrationFault::None)?;
        if complete_publication {
            connection.execute(
                "UPDATE usage_replay_revision SET scan_set_id = 9
                 WHERE status = 'current'",
                [],
            )?;
        }
        validate_v5(&connection)?;
        Ok(connection)
    }

    fn exact_v7_fixture() -> TestResult<Connection> {
        let mut connection = exact_v5_fixture(true)?;
        migrate_v5_with_fault(&mut connection, MigrationFault::None)?;
        migrate_v6(&mut connection)?;
        validate_v7(&connection)?;
        Ok(connection)
    }

    fn exact_v8_fixture() -> TestResult<Connection> {
        let mut connection = exact_v7_fixture()?;
        migrate_v7_with_fault(&mut connection, MigrationFault::None)?;
        validate_v8(&connection)?;
        Ok(connection)
    }

    fn exact_v9_fixture() -> TestResult<Connection> {
        let mut connection = exact_v8_fixture()?;
        migrate_v8_with_fault(&mut connection, MigrationFault::None)?;
        validate_v9(&connection)?;
        Ok(connection)
    }

    #[test]
    fn exact_v8_migrates_to_v9_without_inventing_reported_cost() -> TestResult {
        let mut connection = exact_v8_fixture()?;
        let before = fixture_snapshot(&connection)?;
        migrate_v8_with_fault(&mut connection, MigrationFault::None)?;
        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 9);
        assert_eq!(fixture_snapshot(&connection)?, before);
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT count(*) FROM usage_observation
                 WHERE reported_cost_usd_micros IS NOT NULL"
            )?,
            0
        );
        assert_eq!(
            connection.query_row(
                "SELECT aggregate_schema_version, state, current_event_count,
                        legacy_event_count, rebuild_total_events
                 FROM usage_aggregate_state WHERE singleton_id = 1",
                [],
                |row| Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                )),
            )?,
            (2, "rebuild_required".to_owned(), 1, 1, 2)
        );
        validate_v9(&connection)?;
        Ok(())
    }

    #[test]
    fn every_v8_pricing_migration_fault_rolls_back_exactly() -> TestResult {
        for fault in [
            MigrationFault::AfterAddReportedCost,
            MigrationFault::AfterUpgradeAggregateState,
            MigrationFault::AfterCreatePriceSchema,
            MigrationFault::AfterCreatePriceTriggers,
        ] {
            let mut connection = exact_v8_fixture()?;
            let before = fixture_snapshot(&connection)?;
            let error = match migrate_v8_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted pricing migration unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 8);
            assert_eq!(fixture_snapshot(&connection)?, before);
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT count(*) FROM pragma_table_info('usage_event')
                     WHERE name = 'reported_cost_usd_micros'"
                )?,
                0
            );
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT count(*) FROM sqlite_schema
                     WHERE name IN ('usage_aggregate_state_v8',
                                    'usage_price_time_rollup',
                                    'usage_price_session_rollup')"
                )?,
                0
            );
            validate_v8(&connection)?;
        }
        Ok(())
    }

    #[test]
    fn exact_v9_migrates_to_v10_without_touching_usage_rows() -> TestResult {
        let mut connection = exact_v9_fixture()?;
        let before = fixture_snapshot(&connection)?;

        migrate_v9_with_fault(&mut connection, MigrationFault::None)?;

        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 10);
        assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
        assert_eq!(fixture_snapshot(&connection)?, before);
        assert_eq!(
            connection.query_row(
                "SELECT revision, retained_sample_count, retained_epoch_count,
                        retained_transition_count, last_published_at_ms
                 FROM quota_state WHERE singleton_id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                },
            )?,
            (0, 0, 0, 0, None)
        );
        validate_v10(&connection)?;
        Ok(())
    }

    #[test]
    fn benefit_schema_fault_rolls_back_exact_v10_without_residue() -> TestResult {
        let mut connection = exact_v9_fixture()?;
        migrate_v9_with_fault(&mut connection, MigrationFault::None)?;
        let before = fixture_snapshot(&connection)?;

        let error =
            match migrate_v10_with_fault(&mut connection, MigrationFault::AfterCreateBenefitSchema)
            {
                Ok(()) => return Err("faulted benefit migration unexpectedly committed".into()),
                Err(error) => error,
            };

        assert_eq!(error.code(), StoreErrorCode::Database);
        assert_eq!(
            pragma_i64(&connection, "PRAGMA user_version")?,
            schema::V10_SCHEMA_VERSION
        );
        assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
        assert_eq!(fixture_snapshot(&connection)?, before);
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT count(*) FROM sqlite_schema
                 WHERE name LIKE 'benefit_%'"
            )?,
            0
        );
        validate_v10(&connection)?;
        Ok(())
    }

    #[test]
    fn quota_schema_fault_rolls_back_exact_v9_without_residue() -> TestResult {
        let mut connection = exact_v9_fixture()?;
        let before = fixture_snapshot(&connection)?;

        let error =
            match migrate_v9_with_fault(&mut connection, MigrationFault::AfterCreateQuotaSchema) {
                Ok(()) => return Err("faulted quota migration unexpectedly committed".into()),
                Err(error) => error,
            };

        assert_eq!(error.code(), StoreErrorCode::Database);
        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 9);
        assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
        assert_eq!(fixture_snapshot(&connection)?, before);
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT count(*) FROM sqlite_schema
                 WHERE name LIKE 'quota_%'"
            )?,
            0
        );
        validate_v9(&connection)?;
        Ok(())
    }

    #[test]
    fn exact_v5_migration_seeds_complete_or_recovery_publication() -> TestResult {
        for (complete_publication, expected_scan, expected_quality) in [
            (false, None, "recovery_pending"),
            (true, Some(9_i64), "complete"),
        ] {
            let mut connection = exact_v5_fixture(complete_publication)?;
            migrate_v5_with_fault(&mut connection, MigrationFault::None)?;
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 6);
            let state: (i64, Option<i64>, Option<i64>, String) = connection.query_row(
                "SELECT archive_generation, current_revision_id,
                        latest_complete_scan_set_id, incremental_state
                 FROM usage_archive_state WHERE singleton_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            assert_eq!(
                state,
                (0, Some(5), expected_scan, expected_quality.to_owned())
            );
            validate_v6(&connection)?;
        }
        Ok(())
    }

    #[test]
    fn exact_v6_migration_seeds_dataset_generation_and_triggers() -> TestResult {
        let mut connection = exact_v5_fixture(true)?;
        migrate_v5_with_fault(&mut connection, MigrationFault::None)?;
        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 6);
        migrate_v6(&mut connection)?;
        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 7);
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state"
            )?,
            1
        );
        validate_v7(&connection)?;

        connection.execute(
            "UPDATE usage_event SET timestamp_seconds = timestamp_seconds + 1",
            [],
        )?;
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state"
            )?,
            2
        );
        Ok(())
    }

    #[test]
    fn every_v6_dataset_generation_fault_rolls_back_exactly() -> TestResult {
        for fault in [
            MigrationFault::AfterCreateDatasetState,
            MigrationFault::AfterSeedDatasetState,
            MigrationFault::AfterCreateDatasetTriggers,
        ] {
            let mut connection = exact_v5_fixture(true)?;
            migrate_v5_with_fault(&mut connection, MigrationFault::None)?;
            let before: (i64, Option<i64>, Option<i64>, String) = connection.query_row(
                "SELECT archive_generation, current_revision_id,
                        latest_complete_scan_set_id, incremental_state
                 FROM usage_archive_state WHERE singleton_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;

            let error = match migrate_v6_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted dataset migration unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 6);
            validate_v6(&connection)?;
            let after: (i64, Option<i64>, Option<i64>, String) = connection.query_row(
                "SELECT archive_generation, current_revision_id,
                        latest_complete_scan_set_id, incremental_state
                 FROM usage_archive_state WHERE singleton_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            assert_eq!(after, before);
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT count(*) FROM sqlite_schema
                     WHERE name = 'usage_archive_state_v6'
                        OR name LIKE 'usage_event_dataset_generation_%'",
                )?,
                0
            );
        }
        Ok(())
    }

    #[test]
    fn exact_v7_migration_materializes_provider_without_advancing_dataset() -> TestResult {
        let mut connection = exact_v7_fixture()?;
        let generation = pragma_i64(
            &connection,
            "SELECT dataset_generation FROM usage_archive_state",
        )?;

        migrate_v7_with_fault(&mut connection, MigrationFault::None)?;

        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 8);
        assert_eq!(
            connection.query_row("SELECT provider_id FROM usage_event", [], |row| {
                row.get::<_, String>(0)
            })?,
            "codex"
        );
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state"
            )?,
            generation
        );
        assert_eq!(
            connection.query_row(
                "SELECT state, expected_dataset_generation,
                        current_event_count, legacy_event_count, rebuild_total_events
                 FROM usage_aggregate_state WHERE singleton_id = 1",
                [],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                )),
            )?,
            ("rebuild_required".to_owned(), generation, 1, 1, 2)
        );
        validate_v8(&connection)?;
        Ok(())
    }

    #[test]
    fn exact_v7_migration_uses_stable_unknown_for_orphan_event() -> TestResult {
        let mut connection = exact_v7_fixture()?;
        connection.execute(
            "UPDATE usage_event SET selected_file_key = ?1",
            [[42_u8; 32].as_slice()],
        )?;
        let generation = pragma_i64(
            &connection,
            "SELECT dataset_generation FROM usage_archive_state",
        )?;

        migrate_v7_with_fault(&mut connection, MigrationFault::None)?;

        assert_eq!(
            connection.query_row("SELECT provider_id FROM usage_event", [], |row| {
                row.get::<_, String>(0)
            })?,
            "unknown"
        );
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state"
            )?,
            generation
        );
        validate_v8(&connection)?;
        Ok(())
    }

    #[test]
    fn exact_v7_migration_rejects_source_profile_mismatch_without_changes() -> TestResult {
        let mut connection = exact_v7_fixture()?;
        connection.execute("UPDATE usage_event SET profile_id = 'different'", [])?;
        let generation = pragma_i64(
            &connection,
            "SELECT dataset_generation FROM usage_archive_state",
        )?;

        let error = match migrate_v7_with_fault(&mut connection, MigrationFault::None) {
            Ok(()) => return Err("mismatched provider migration unexpectedly committed".into()),
            Err(error) => error,
        };

        assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
        assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 7);
        assert_eq!(
            connection.query_row("SELECT profile_id FROM usage_event", [], |row| {
                row.get::<_, String>(0)
            })?,
            "different"
        );
        assert_eq!(
            pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state"
            )?,
            generation
        );
        validate_v7(&connection)?;
        Ok(())
    }

    #[test]
    fn every_v7_provider_event_fault_rolls_back_exactly() -> TestResult {
        for fault in [
            MigrationFault::AfterCreateProviderEvent,
            MigrationFault::AfterCopyProviderEvent,
            MigrationFault::AfterDropProviderEvent,
        ] {
            let mut connection = exact_v7_fixture()?;
            let generation = pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state",
            )?;
            let event: (Vec<u8>, String) = connection.query_row(
                "SELECT fingerprint, profile_id FROM usage_event",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            let error = match migrate_v7_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted provider migration unexpectedly committed".into()),
                Err(error) => error,
            };

            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 7);
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT dataset_generation FROM usage_archive_state"
                )?,
                generation
            );
            assert_eq!(
                connection.query_row(
                    "SELECT fingerprint, profile_id FROM usage_event",
                    [],
                    |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
                )?,
                event
            );
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT count(*) FROM sqlite_schema
                     WHERE name = 'usage_event_v8' OR instr(sql, 'usage_event_v8') > 0"
                )?,
                0
            );
            validate_v7(&connection)?;
        }
        Ok(())
    }

    #[test]
    fn every_v7_aggregate_migration_fault_rolls_back_exactly() -> TestResult {
        for fault in [
            MigrationFault::AfterCreateAggregateSchema,
            MigrationFault::AfterSeedAggregateState,
            MigrationFault::AfterCreateAggregateTriggers,
        ] {
            let mut connection = exact_v7_fixture()?;
            let generation = pragma_i64(
                &connection,
                "SELECT dataset_generation FROM usage_archive_state",
            )?;

            let error = match migrate_v7_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted aggregate migration unexpectedly committed".into()),
                Err(error) => error,
            };

            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 7);
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT dataset_generation FROM usage_archive_state"
                )?,
                generation
            );
            assert_eq!(
                pragma_i64(
                    &connection,
                    "SELECT count(*) FROM sqlite_schema
                     WHERE name IN (
                       'usage_event_v8', 'usage_aggregate_state',
                       'usage_time_rollup', 'usage_session_rollup'
                     ) OR instr(sql, 'usage_event_v8') > 0
                        OR name LIKE 'usage_event_aggregate_%'"
                )?,
                0
            );
            validate_v7(&connection)?;
        }
        Ok(())
    }

    #[test]
    fn every_v5_archive_state_fault_rolls_back_exactly() -> TestResult {
        for fault in [
            MigrationFault::AfterCreateArchiveState,
            MigrationFault::AfterSeedArchiveState,
        ] {
            let mut connection = exact_v5_fixture(true)?;
            let before = fixture_snapshot(&connection)?;
            let error = match migrate_v5_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted archive-state migration committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 5);
            assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
            validate_v5(&connection)?;
            assert_eq!(fixture_snapshot(&connection)?, before);
            let archive_state_tables = pragma_i64(
                &connection,
                "SELECT count(*) FROM sqlite_schema
                 WHERE name = 'usage_archive_state'",
            )?;
            assert_eq!(archive_state_tables, 0);
        }
        Ok(())
    }

    #[test]
    fn every_v4_scan_authority_fault_rolls_back_and_restores_foreign_keys() -> TestResult {
        for fault in [
            MigrationFault::AfterCreateScanAuthority,
            MigrationFault::AfterCopyScanAuthority,
            MigrationFault::AfterDropScanAuthority,
        ] {
            let mut connection = exact_v4_fixture_with_scan()?;
            let before = fixture_snapshot(&connection)?;
            let error = match migrate_v4_with_fault(&mut connection, fault) {
                Ok(()) => return Err("faulted scan-authority migration committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(pragma_i64(&connection, "PRAGMA user_version")?, 4);
            assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys")?, 1);
            validate_v4(&connection)?;
            assert_eq!(fixture_snapshot(&connection)?, before);
            let temporary_names: i64 = connection.query_row(
                "SELECT count(*) FROM sqlite_schema
                 WHERE name IN (
                   'usage_scan_set', 'usage_scan_v5',
                   'usage_replay_revision_v5'
                 ) OR instr(sql, 'usage_scan_v5') > 0
                    OR instr(sql, 'usage_replay_revision_v5') > 0",
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
