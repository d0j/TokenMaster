use rusqlite::{OptionalExtension, TransactionBehavior, params};

use crate::{StoreError, StoreErrorCode};

use super::{UsageStore, types::*};

const EMPTY_SHA256: [u8; 32] = [
    0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
    0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
];

impl UsageStore {
    pub fn begin_replay_revision(
        &mut self,
        manifest: &ReplayManifest,
    ) -> Result<ReplayRevisionSnapshot, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let staging_revisions: i64 = transaction.query_row(
            "SELECT count(*) FROM usage_replay_revision WHERE status = 'staging'",
            [],
            |row| row.get(0),
        )?;
        let staging_generations: i64 = transaction.query_row(
            "SELECT count(*) FROM usage_generation WHERE status = 'staging'",
            [],
            |row| row.get(0),
        )?;
        if staging_revisions != 0 || staging_generations != 0 {
            return Err(StoreError::new(StoreErrorCode::ArchiveModeMismatch));
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
        let expected_source_count = u16::try_from(manifest.source_count())
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let status = ReplayRevisionStatus::Staging;
        transaction.execute(
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
                i64::from(expected_source_count),
                epoch.as_sql()?,
            ],
        )?;

        for source_key in manifest.source_keys() {
            let current_exists: Option<i64> = transaction
                .query_row(
                    "SELECT s.current_generation
                     FROM usage_source AS s
                     JOIN usage_generation AS g
                       ON g.file_key = s.file_key AND g.generation = s.current_generation
                     WHERE s.file_key = ?1 AND g.status = 'current'",
                    [source_key.as_bytes().as_slice()],
                    |row| row.get(0),
                )
                .optional()?;
            if current_exists.is_none() {
                return Err(StoreError::new(StoreErrorCode::InvalidValue));
            }
            let max_generation: Option<i64> = transaction.query_row(
                "SELECT max(generation) FROM usage_generation WHERE file_key = ?1",
                [source_key.as_bytes().as_slice()],
                |row| row.get(0),
            )?;
            let generation = max_generation
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
            let inserted = transaction.execute(
                "INSERT INTO usage_generation(
                   file_key, generation, status, parser_schema_version,
                   physical_identity, logical_identity, committed_offset, scan_offset,
                   observed_file_length, modified_time_ns, anchor_start, anchor_len,
                   anchor_sha256, resume_payload, discarding_oversized_line,
                   incomplete_tail, verification_level
                 )
                 SELECT
                   g.file_key, ?2, 'staging', g.parser_schema_version,
                   g.physical_identity, g.logical_identity, 0, 0, 0, NULL, 0, 0,
                   ?3, zeroblob(0), 0, 0, 'incremental'
                 FROM usage_source AS s
                 JOIN usage_generation AS g
                   ON g.file_key = s.file_key AND g.generation = s.current_generation
                 WHERE s.file_key = ?1 AND g.status = 'current'",
                params![
                    source_key.as_bytes().as_slice(),
                    generation,
                    EMPTY_SHA256.as_slice(),
                ],
            )?;
            if inserted != 1 {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
            transaction.execute(
                "INSERT INTO usage_replay_source(revision_id, file_key, generation, state)
                 VALUES (?1, ?2, ?3, 'pending')",
                params![
                    revision_id.as_sql()?,
                    source_key.as_bytes().as_slice(),
                    generation,
                ],
            )?;
        }

        let manifest_rows: i64 = transaction.query_row(
            "SELECT count(*) FROM usage_replay_source WHERE revision_id = ?1",
            [revision_id.as_sql()?],
            |row| row.get(0),
        )?;
        let foreign_key_failures: i64 =
            transaction.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if manifest_rows != i64::from(expected_source_count) || foreign_key_failures != 0 {
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
