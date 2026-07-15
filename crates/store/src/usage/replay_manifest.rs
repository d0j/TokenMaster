use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{StoreError, StoreErrorCode};

use super::{
    UsageStore,
    types::*,
    write::{sql_u64, stored_digest},
};

const MANIFEST_VALIDATION_PAGE_SIZE: usize = 256;

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
            scan_set_id: None,
            sealed: false,
            promoted: false,
        })
    }

    pub fn begin_replay_revision_for_scan_set(
        &mut self,
        scan_set_id: ScanSetId,
    ) -> Result<ReplayRevisionSnapshot, StoreError> {
        self.begin_replay_revision_for_scan_set_inner(scan_set_id, ScanBoundBeginFault::None)
    }

    fn begin_replay_revision_for_scan_set_inner(
        &mut self,
        scan_set_id: ScanSetId,
        _fault: ScanBoundBeginFault,
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

        let expected_source_count = complete_scan_set_source_count(&transaction, scan_set_id)?
            .ok_or_else(|| StoreError::new(StoreErrorCode::IncompleteManifest))?;
        let source_state: (i64, i64) = transaction.query_row(
            "SELECT
               (SELECT count(*)
                FROM usage_source AS source
                JOIN usage_scan AS scan
                  ON scan.scan_set_id = ?1
                 AND scan.provider_id = source.provider_id
                 AND scan.profile_id = source.profile_id
                 AND scan.scan_id = source.last_seen_scan_id
                JOIN usage_generation AS current
                  ON current.file_key = source.file_key
                 AND current.generation = source.current_generation
                WHERE scan.completion_state = 'complete' AND source.missing = 0
                  AND current.status = 'current'),
               (SELECT count(*)
                FROM usage_source AS source
                JOIN usage_scan AS scan
                  ON scan.scan_set_id = ?1
                 AND scan.provider_id = source.provider_id
                 AND scan.profile_id = source.profile_id
                 AND scan.scan_id = source.last_seen_scan_id
                WHERE scan.completion_state = 'complete' AND source.missing = 0
                  AND (SELECT max(previous.generation)
                       FROM usage_generation AS previous
                       WHERE previous.file_key = source.file_key) = ?2)",
            params![scan_set_id.as_sql()?, i64::MAX],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if stored_count(source_state.0)? != expected_source_count || source_state.1 != 0 {
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
               sealed, promoted, scan_set_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8)",
            params![
                revision_id.as_sql()?,
                status.as_sql(),
                i64::from(versions.canonicalizer()),
                i64::from(versions.fingerprint()),
                i64::from(versions.replay_signature()),
                sql_count(expected_source_count)?,
                epoch.as_sql()?,
                scan_set_id.as_sql()?,
            ],
        )?;
        if inserted_revision != 1 {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        #[cfg(test)]
        if _fault == ScanBoundBeginFault::AfterRevision {
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
               current.logical_identity, 0, 0, 0, NULL, 0, 0, ?2, zeroblob(0),
               0, 0, 'incremental'
             FROM usage_source AS source
             JOIN usage_scan AS scan
               ON scan.scan_set_id = ?1
              AND scan.provider_id = source.provider_id
              AND scan.profile_id = source.profile_id
              AND scan.scan_id = source.last_seen_scan_id
             JOIN usage_generation AS current
               ON current.file_key = source.file_key
              AND current.generation = source.current_generation
             WHERE scan.completion_state = 'complete' AND source.missing = 0
               AND current.status = 'current'
             ORDER BY source.file_key",
            params![scan_set_id.as_sql()?, EMPTY_SHA256.as_slice()],
        )?;
        if mutation_count(inserted_generations)? != expected_source_count {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        #[cfg(test)]
        if _fault == ScanBoundBeginFault::AfterGenerations {
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
        if mutation_count(inserted_sources)? != expected_source_count
            || !scan_bound_manifest_matches(
                &transaction,
                revision_id,
                scan_set_id,
                expected_source_count,
                "staging",
            )?
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
            scan_set_id: Some(scan_set_id),
            sealed: false,
            promoted: false,
        })
    }

    #[cfg(test)]
    fn begin_replay_revision_for_scan_set_with_fault(
        &mut self,
        scan_set_id: ScanSetId,
        fault: ScanBoundBeginFault,
    ) -> Result<ReplayRevisionSnapshot, StoreError> {
        self.begin_replay_revision_for_scan_set_inner(scan_set_id, fault)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScanBoundBeginFault {
    None,
    #[cfg(test)]
    AfterRevision,
    #[cfg(test)]
    AfterGenerations,
}

fn complete_scan_set_source_count(
    transaction: &Transaction<'_>,
    scan_set_id: ScanSetId,
) -> Result<Option<u64>, StoreError> {
    let state: Option<(String, i64, i64, i64, i64, i64, i64)> = transaction
        .query_row(
            "SELECT
               completion_state, expected_scope_count,
               (SELECT count(*) FROM usage_scan WHERE scan_set_id = ?1),
               (SELECT count(*) FROM usage_scan
                WHERE scan_set_id = ?1 AND completion_state = 'complete'),
               (SELECT count(*)
                FROM usage_source AS source
                JOIN usage_scan AS scan
                  ON scan.scan_set_id = ?1
                 AND scan.provider_id = source.provider_id
                 AND scan.profile_id = source.profile_id
                WHERE source.last_seen_scan_id = scan.scan_id
                  AND source.missing = 0),
               (SELECT count(*)
                FROM usage_source AS source
                JOIN usage_scan AS scan
                  ON scan.scan_set_id = ?1
                 AND scan.provider_id = source.provider_id
                 AND scan.profile_id = source.profile_id
                WHERE source.missing <> CASE
                  WHEN source.last_seen_scan_id = scan.scan_id THEN 0 ELSE 1 END),
               (SELECT count(*) FROM usage_scan AS scan
                WHERE scan.scan_set_id = ?1
                  AND (scan.completed_at_ms IS NULL
                       OR scan.completed_at_ms > usage_scan_set.completed_at_ms))
             FROM usage_scan_set WHERE scan_set_id = ?1",
            [scan_set_id.as_sql()?],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?;
    let Some(state) = state else {
        return Ok(None);
    };
    let expected_scope_count = stored_count(state.1)?;
    if state.0 != "complete"
        || stored_count(state.2)? != expected_scope_count
        || stored_count(state.3)? != expected_scope_count
        || state.5 != 0
        || state.6 != 0
    {
        return Ok(None);
    }
    stored_count(state.4).map(Some)
}

fn scan_bound_manifest_matches(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    scan_set_id: ScanSetId,
    expected_source_count: u64,
    generation_status: &str,
) -> Result<bool, StoreError> {
    if complete_scan_set_source_count(transaction, scan_set_id)? != Some(expected_source_count) {
        return Ok(false);
    }
    let counts: (i64, i64, i64, i64) = transaction.query_row(
        "SELECT
           (SELECT count(*) FROM usage_replay_source WHERE revision_id = ?1),
           (SELECT count(*) FROM usage_replay_source AS replay
            JOIN usage_generation AS generation
              ON generation.file_key = replay.file_key
             AND generation.generation = replay.generation
            WHERE replay.revision_id = ?1 AND generation.status = ?3),
           (SELECT count(*)
            FROM usage_source AS source
            JOIN usage_scan AS scan
              ON scan.scan_set_id = ?2
             AND scan.provider_id = source.provider_id
             AND scan.profile_id = source.profile_id
             AND scan.scan_id = source.last_seen_scan_id
            WHERE source.missing = 0 AND NOT EXISTS(
              SELECT 1 FROM usage_replay_source AS replay
              WHERE replay.revision_id = ?1 AND replay.file_key = source.file_key)),
           (SELECT count(*) FROM usage_replay_source AS replay
            WHERE replay.revision_id = ?1 AND NOT EXISTS(
              SELECT 1 FROM usage_source AS source
              JOIN usage_scan AS scan
                ON scan.scan_set_id = ?2
               AND scan.provider_id = source.provider_id
               AND scan.profile_id = source.profile_id
               AND scan.scan_id = source.last_seen_scan_id
              WHERE source.file_key = replay.file_key AND source.missing = 0))",
        params![
            revision_id.as_sql()?,
            scan_set_id.as_sql()?,
            generation_status
        ],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    Ok(stored_count(counts.0)? == expected_source_count
        && stored_count(counts.1)? == expected_source_count
        && counts.2 == 0
        && counts.3 == 0)
}

pub(super) fn replay_manifest_sources_closed(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    expected_source_count: u64,
    scan_set_id: Option<ScanSetId>,
) -> Result<bool, StoreError> {
    let counts: (i64, i64, i64, i64) = transaction.query_row(
        "SELECT
           (SELECT count(*) FROM usage_source),
           (SELECT count(*) FROM usage_replay_source WHERE revision_id = ?1),
           (SELECT count(*) FROM usage_replay_source
            WHERE revision_id = ?1 AND state = 'complete'),
           (SELECT count(*) FROM usage_replay_source AS replay
            JOIN usage_generation AS generation
              ON generation.file_key = replay.file_key
             AND generation.generation = replay.generation
            WHERE replay.revision_id = ?1 AND generation.status = 'staging')",
        [revision_id.as_sql()?],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let source_membership_matches = if let Some(scan_set_id) = scan_set_id {
        scan_bound_manifest_matches(
            transaction,
            revision_id,
            scan_set_id,
            expected_source_count,
            "staging",
        )?
    } else {
        stored_count(counts.0)? == expected_source_count
    };
    Ok(source_membership_matches
        && stored_count(counts.1)? == expected_source_count
        && stored_count(counts.2)? == expected_source_count
        && stored_count(counts.3)? == expected_source_count)
}

pub(super) fn current_replay_manifest_sources_closed(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    expected_source_count: u64,
    scan_set_id: ScanSetId,
) -> Result<bool, StoreError> {
    if complete_scan_set_source_count(transaction, scan_set_id)?.is_none() {
        return Ok(false);
    }
    let counts: (i64, i64, i64, i64) = transaction.query_row(
        "SELECT
           (SELECT count(*) FROM usage_replay_source WHERE revision_id = ?1),
           (SELECT count(*) FROM usage_replay_source AS replay
            JOIN usage_generation AS generation
              ON generation.file_key = replay.file_key
             AND generation.generation = replay.generation
            JOIN usage_source AS source
              ON source.file_key = replay.file_key
             AND source.current_generation = replay.generation
            WHERE replay.revision_id = ?1 AND generation.status = 'current'),
           (SELECT count(*) FROM usage_replay_source AS replay
            JOIN usage_source AS source ON source.file_key = replay.file_key
            WHERE replay.revision_id = ?1 AND NOT EXISTS(
              SELECT 1 FROM usage_scan AS scope
              WHERE scope.scan_set_id = ?2
                AND scope.provider_id = source.provider_id
                AND scope.profile_id = source.profile_id
            )),
           (SELECT count(*) FROM usage_source AS source
            JOIN usage_scan AS scope
              ON scope.scan_set_id = ?2
             AND scope.provider_id = source.provider_id
             AND scope.profile_id = source.profile_id
            WHERE source.missing = 0 AND (
              source.last_seen_scan_id <> scope.scan_id
              OR NOT EXISTS(
                SELECT 1 FROM usage_replay_source AS replay
                WHERE replay.revision_id = ?1 AND replay.file_key = source.file_key
              )
            ))",
        params![revision_id.as_sql()?, scan_set_id.as_sql()?],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    Ok(stored_count(counts.0)? == expected_source_count
        && stored_count(counts.1)? == expected_source_count
        && counts.2 == 0
        && counts.3 == 0)
}

pub(super) fn validate_complete_manifest(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    expected_source_count: u64,
    scan_set_id: Option<ScanSetId>,
) -> Result<(), StoreError> {
    if !replay_manifest_is_complete(transaction, revision_id, expected_source_count, scan_set_id)? {
        return Err(StoreError::new(StoreErrorCode::IncompleteManifest));
    }
    Ok(())
}

struct ManifestSourceState {
    file_key: [u8; 32],
    generation: u64,
    state: String,
    generation_status: String,
    committed_offset: u64,
    scan_offset: u64,
    observed_file_length: u64,
    discarding: bool,
    incomplete: bool,
    verification: StoredVerification,
}

fn replay_manifest_is_complete(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    expected_source_count: u64,
    scan_set_id: Option<ScanSetId>,
) -> Result<bool, StoreError> {
    let counts: (i64, i64, i64) = transaction.query_row(
        "SELECT
           (SELECT count(*) FROM usage_source),
           (SELECT count(*) FROM usage_replay_source WHERE revision_id = ?1),
           (SELECT count(*) FROM usage_replay_source AS replay
            JOIN usage_generation AS generation
              ON generation.file_key = replay.file_key
             AND generation.generation = replay.generation
            WHERE replay.revision_id = ?1 AND generation.status = 'staging')",
        [revision_id.as_sql()?],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let source_membership_matches = if let Some(scan_set_id) = scan_set_id {
        scan_bound_manifest_matches(
            transaction,
            revision_id,
            scan_set_id,
            expected_source_count,
            "staging",
        )?
    } else {
        stored_count(counts.0)? == expected_source_count
    };
    if !source_membership_matches
        || stored_count(counts.1)? != expected_source_count
        || stored_count(counts.2)? != expected_source_count
    {
        return Ok(false);
    }

    let mut cursor = None;
    let mut visited = 0_u64;
    loop {
        let page = load_manifest_page(transaction, revision_id, cursor.as_ref())?;
        if page.is_empty() {
            break;
        }
        for source in &page {
            if source.state != "complete"
                || source.generation_status != "staging"
                || source.verification != StoredVerification::FullPrefix
                || source.discarding
                || source.incomplete
                || source.committed_offset != source.scan_offset
                || source.scan_offset != source.observed_file_length
                || !source_chunks_cover(
                    transaction,
                    &source.file_key,
                    source.generation,
                    source.committed_offset,
                )?
            {
                return Ok(false);
            }
        }
        visited = visited
            .checked_add(mutation_count(page.len())?)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        cursor = page.last().map(|source| source.file_key);
        if page.len() < MANIFEST_VALIDATION_PAGE_SIZE {
            break;
        }
    }
    Ok(visited == expected_source_count)
}

fn load_manifest_page(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    cursor: Option<&[u8; 32]>,
) -> Result<Vec<ManifestSourceState>, StoreError> {
    const SELECT: &str = "SELECT replay.file_key, replay.generation, replay.state,
                generation.status, generation.committed_offset,
                generation.scan_offset, generation.observed_file_length,
                generation.discarding_oversized_line, generation.incomplete_tail,
                generation.verification_level
         FROM usage_replay_source AS replay
         JOIN usage_generation AS generation
           ON generation.file_key = replay.file_key
          AND generation.generation = replay.generation";
    let mut page = Vec::with_capacity(MANIFEST_VALIDATION_PAGE_SIZE);
    if let Some(cursor) = cursor {
        let sql = format!(
            "{SELECT}
             WHERE replay.revision_id = ?1 AND replay.file_key > ?2
             ORDER BY replay.file_key
             LIMIT ?3"
        );
        let mut statement = transaction.prepare(&sql)?;
        let mut rows = statement.query(params![
            revision_id.as_sql()?,
            cursor.as_slice(),
            i64::try_from(MANIFEST_VALIDATION_PAGE_SIZE)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?,
        ])?;
        while let Some(row) = rows.next()? {
            page.push(manifest_source_from_row(row)?);
        }
    } else {
        let sql = format!(
            "{SELECT}
             WHERE replay.revision_id = ?1
             ORDER BY replay.file_key
             LIMIT ?2"
        );
        let mut statement = transaction.prepare(&sql)?;
        let mut rows = statement.query(params![
            revision_id.as_sql()?,
            i64::try_from(MANIFEST_VALIDATION_PAGE_SIZE)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?,
        ])?;
        while let Some(row) = rows.next()? {
            page.push(manifest_source_from_row(row)?);
        }
    }
    Ok(page)
}

fn manifest_source_from_row(row: &rusqlite::Row<'_>) -> Result<ManifestSourceState, StoreError> {
    Ok(ManifestSourceState {
        file_key: stored_digest(&row.get::<_, Vec<u8>>(0)?)?,
        generation: stored_nonnegative(row.get(1)?)?,
        state: row.get(2)?,
        generation_status: row.get(3)?,
        committed_offset: stored_nonnegative(row.get(4)?)?,
        scan_offset: stored_nonnegative(row.get(5)?)?,
        observed_file_length: stored_nonnegative(row.get(6)?)?,
        discarding: stored_bool(row.get(7)?)?,
        incomplete: stored_bool(row.get(8)?)?,
        verification: StoredVerification::from_sql(&row.get::<_, String>(9)?)?,
    })
}

fn source_chunks_cover(
    transaction: &Transaction<'_>,
    file_key: &[u8; 32],
    generation: u64,
    committed_offset: u64,
) -> Result<bool, StoreError> {
    let (count, minimum, maximum, covered): (i64, Option<i64>, Option<i64>, i64) = transaction
        .query_row(
            "SELECT count(*), min(chunk_index), max(chunk_index),
                    coalesce(sum(covered_len), 0)
             FROM usage_source_chunk
             WHERE file_key = ?1 AND generation = ?2",
            params![file_key.as_slice(), sql_u64(generation)?],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    if committed_offset == 0 {
        return Ok(count == 0 && covered == 0 && minimum.is_none() && maximum.is_none());
    }
    let final_index = (committed_offset - 1) / SOURCE_CHUNK_BYTES;
    let final_length = committed_offset - final_index * SOURCE_CHUNK_BYTES;
    if count
        != i64::try_from(final_index + 1)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?
        || minimum != Some(0)
        || maximum != Some(sql_u64(final_index)?)
        || covered != sql_u64(committed_offset)?
    {
        return Ok(false);
    }
    let invalid: i64 = transaction.query_row(
        "SELECT count(*) FROM usage_source_chunk
         WHERE file_key = ?1 AND generation = ?2
           AND ((chunk_index < ?3 AND covered_len <> ?4)
                OR (chunk_index = ?3 AND covered_len <> ?5)
                OR chunk_index > ?3)",
        params![
            file_key.as_slice(),
            sql_u64(generation)?,
            sql_u64(final_index)?,
            sql_u64(SOURCE_CHUNK_BYTES)?,
            sql_u64(final_length)?,
        ],
        |row| row.get(0),
    )?;
    Ok(invalid == 0)
}

fn stored_count(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn stored_nonnegative(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn stored_bool(value: i64) -> Result<bool, StoreError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn mutation_count(value: usize) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn sql_count(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn registration() -> Result<SourceRegistration, StoreError> {
        let checkpoint = StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some([1; 32]),
            logical_identity: [2; 32],
            committed_offset: 0,
            scan_offset: 0,
            observed_file_length: 0,
            modified_time_ns: None,
            anchor_start: 0,
            anchor_len: 0,
            anchor_sha256: [3; 32],
            resume: Box::default(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::Incremental,
        })?;
        SourceRegistration::new(SourceRegistrationParts {
            source_key: SourceKey::from_bytes([1; 32]),
            provider_id: "codex".into(),
            profile_id: "default".into(),
            source_id: "fixture".into(),
            source_kind: SourceKind::Active,
            logical_identity: [2; 32],
            physical_identity: Some([1; 32]),
            initial_checkpoint: checkpoint,
        })
    }

    fn complete_scan(store: &mut UsageStore) -> Result<ScanSetId, StoreError> {
        let manifest =
            ScanSetManifest::new(vec![ScanScope::new("codex", "default")?].into_boxed_slice())?;
        let scan_set = store.begin_scan_set(&manifest, 1_000)?;
        let scans = store.scan_page(scan_set.id(), None, 1)?;
        let scan = scans
            .first()
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        store.observe_scan_source(scan.id(), SourceKey::from_bytes([1; 32]))?;
        store.finish_scan(
            scan.id(),
            ScanOutcome::Complete,
            1_010,
            ScanCounters::default(),
        )?;
        store.finish_scan_set(scan_set.id(), 1_020)?;
        Ok(scan_set.id())
    }

    #[test]
    fn every_scan_bound_begin_fault_rolls_back_all_staging_state() -> TestResult {
        for fault in [
            ScanBoundBeginFault::AfterRevision,
            ScanBoundBeginFault::AfterGenerations,
        ] {
            let mut store = UsageStore::in_memory()?;
            store.register_source(&registration()?)?;
            let scan_set_id = complete_scan(&mut store)?;
            let error =
                match store.begin_replay_revision_for_scan_set_with_fault(scan_set_id, fault) {
                    Ok(_) => return Err("faulted scan-bound begin unexpectedly committed".into()),
                    Err(error) => error,
                };
            assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
            let staging: (i64, i64, i64) = store.connection.query_row(
                "SELECT
                   (SELECT count(*) FROM usage_replay_revision WHERE status = 'staging'),
                   (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
                   (SELECT count(*) FROM usage_replay_source)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            assert_eq!(staging, (0, 0, 0));
            let current = store
                .generation_snapshot(SourceKey::from_bytes([1; 32]))?
                .ok_or("missing current generation after rollback")?;
            assert_eq!(current.generation(), 0);
            assert_eq!(current.status(), GenerationStatus::Current);
            assert_eq!(
                store
                    .begin_replay_revision_for_scan_set(scan_set_id)?
                    .expected_source_count(),
                1
            );
        }
        Ok(())
    }
}
