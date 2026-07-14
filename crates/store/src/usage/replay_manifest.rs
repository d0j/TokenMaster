use rusqlite::{TransactionBehavior, params};

use crate::{StoreError, StoreErrorCode};

use super::{UsageStore, types::*};

pub(super) const EMPTY_SHA256: [u8; 32] = [
    0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
    0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
];

impl UsageStore {
    pub fn begin_replay_revision_all_sources(
        &mut self,
    ) -> Result<ReplayRevisionSnapshot, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let staging: (i64, i64) = transaction.query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision WHERE status = 'staging'),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if staging != (0, 0) {
            return Err(StoreError::new(StoreErrorCode::ArchiveModeMismatch));
        }

        let source_counts: (i64, i64, i64) = transaction.query_row(
            "SELECT
               (SELECT count(*) FROM usage_source),
               (SELECT count(*) FROM usage_source AS source
                JOIN usage_generation AS current
                  ON current.file_key = source.file_key
                 AND current.generation = source.current_generation
                WHERE current.status = 'current'),
               (SELECT count(*) FROM usage_source AS source
                WHERE (SELECT max(previous.generation)
                       FROM usage_generation AS previous
                       WHERE previous.file_key = source.file_key) = ?1)",
            [i64::MAX],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let expected_source_count = stored_count(source_counts.0)?;
        if expected_source_count == 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if stored_count(source_counts.1)? != expected_source_count || source_counts.2 != 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }

        let max_revision: Option<i64> = transaction.query_row(
            "SELECT max(revision_id) FROM usage_replay_revision",
            [],
            |row| row.get(0),
        )?;
        let revision_value = max_revision
            .map_or(Some(0), |value| value.checked_add(1))
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let revision_id = ReplayRevisionId::from_stored(revision_value)?;
        let epoch = ReplayEpoch::new(0)?;
        let versions = AccountingVersions::compiled();
        let status = ReplayRevisionStatus::Staging;
        let inserted_revision = transaction.execute(
            "INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0)",
            params![
                revision_id.as_sql()?,
                status.as_sql(),
                i64::from(versions.canonicalizer()),
                i64::from(versions.fingerprint()),
                i64::from(versions.replay_signature()),
                sql_count(expected_source_count)?,
                epoch.as_sql()?,
            ],
        )?;
        if inserted_revision != 1 {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }

        let inserted_generations = transaction.execute(
            "INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version,
               physical_identity, logical_identity, committed_offset, scan_offset,
               observed_file_length, modified_time_ns, anchor_start, anchor_len,
               anchor_sha256, resume_payload, discarding_oversized_line,
               incomplete_tail, verification_level
             )
             SELECT
               source.file_key,
               (SELECT max(previous.generation) + 1
                FROM usage_generation AS previous
                WHERE previous.file_key = source.file_key),
               'staging', current.parser_schema_version, current.physical_identity,
               current.logical_identity, 0, 0, 0, NULL, 0, 0, ?1, zeroblob(0),
               0, 0, 'incremental'
             FROM usage_source AS source
             JOIN usage_generation AS current
               ON current.file_key = source.file_key
              AND current.generation = source.current_generation
             WHERE current.status = 'current'
             ORDER BY source.file_key",
            [EMPTY_SHA256.as_slice()],
        )?;
        if mutation_count(inserted_generations)? != expected_source_count {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }

        let inserted_sources = transaction.execute(
            "INSERT INTO usage_replay_source(revision_id, file_key, generation, state)
             SELECT ?1, generation.file_key, generation.generation, 'pending'
             FROM usage_generation AS generation
             WHERE generation.status = 'staging'
             ORDER BY generation.file_key",
            [revision_id.as_sql()?],
        )?;
        if mutation_count(inserted_sources)? != expected_source_count {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }

        let stored: (i64, i64, i64) = transaction.query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision
                WHERE revision_id = ?1 AND status = 'staging'),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
               (SELECT count(*) FROM usage_replay_source WHERE revision_id = ?1)",
            [revision_id.as_sql()?],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if stored_count(stored.0)? != 1
            || stored_count(stored.1)? != expected_source_count
            || stored_count(stored.2)? != expected_source_count
        {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        let foreign_key_failures: i64 =
            transaction.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if foreign_key_failures != 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }

        transaction.commit()?;
        Ok(ReplayRevisionSnapshot {
            id: revision_id,
            epoch,
            status,
            versions,
            expected_source_count,
            sealed: false,
            promoted: false,
        })
    }
}

fn stored_count(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn mutation_count(value: usize) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn sql_count(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}
