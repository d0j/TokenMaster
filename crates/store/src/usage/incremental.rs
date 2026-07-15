use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{UsageStore, types::*, write::insert_generation};
use crate::{StoreError, StoreErrorCode};

impl UsageStore {
    pub fn register_rebuild_source(
        &mut self,
        registration: &SourceRegistration,
    ) -> Result<(), StoreError> {
        let parts = registration.parts();
        let checkpoint = &parts.initial_checkpoint;
        if checkpoint.committed_offset() != 0
            || checkpoint.scan_offset() != 0
            || checkpoint.observed_file_length() != 0
            || checkpoint.modified_time_ns().is_some()
            || checkpoint.anchor_start() != 0
            || checkpoint.anchor_len() != 0
            || checkpoint.discarding_oversized_line()
            || checkpoint.incomplete_tail()
            || checkpoint.verification() != StoredVerification::Incremental
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, current_generation,
               missing, verification_level, diagnostic_count
             ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL,
               CASE WHEN EXISTS(
                 SELECT 1 FROM usage_scan
                 WHERE provider_id = ?2 AND profile_id = ?3
                   AND completion_state = 'complete'
               ) THEN 1 ELSE 0 END,
               'incremental', 0
             )",
            params![
                parts.source_key.as_bytes().as_slice(),
                parts.provider_id.as_ref(),
                parts.profile_id.as_ref(),
                parts.source_id.as_ref(),
                parts.source_kind.as_sql(),
                parts.logical_identity.as_slice(),
                parts.physical_identity.as_ref().map(<[u8; 32]>::as_slice),
            ],
        )?;
        if inserted == 0 {
            let recoverable: i64 = transaction.query_row(
                "SELECT count(*) FROM usage_source AS source
                 WHERE source.file_key = ?1 AND source.current_generation IS NULL
                   AND source.provider_id = ?2 AND source.profile_id = ?3
                   AND source.source_id = ?4 AND source.source_kind = ?5
                   AND source.logical_identity = ?6
                   AND NOT EXISTS(
                     SELECT 1 FROM usage_replay_source
                     WHERE file_key = source.file_key
                   )
                   AND NOT EXISTS(
                     SELECT 1 FROM usage_observation
                     WHERE file_key = source.file_key
                   )
                   AND NOT EXISTS(
                     SELECT 1 FROM usage_source_chunk
                     WHERE file_key = source.file_key
                   )
                   AND (SELECT count(*) FROM usage_generation
                        WHERE file_key = source.file_key) = 1
                   AND EXISTS(
                     SELECT 1 FROM usage_generation
                     WHERE file_key = source.file_key AND generation = 0
                       AND status = 'current'
                   )",
                params![
                    parts.source_key.as_bytes().as_slice(),
                    parts.provider_id.as_ref(),
                    parts.profile_id.as_ref(),
                    parts.source_id.as_ref(),
                    parts.source_kind.as_sql(),
                    parts.logical_identity.as_slice(),
                ],
                |row| row.get(0),
            )?;
            if recoverable != 1 {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
            transaction.execute(
                "DELETE FROM usage_generation WHERE file_key = ?1 AND generation = 0",
                [parts.source_key.as_bytes().as_slice()],
            )?;
            let updated = transaction.execute(
                "UPDATE usage_source SET physical_identity = ?1,
                   verification_level = 'incremental', diagnostic_count = 0
                 WHERE file_key = ?2 AND current_generation IS NULL",
                params![
                    parts.physical_identity.as_ref().map(<[u8; 32]>::as_slice),
                    parts.source_key.as_bytes().as_slice(),
                ],
            )?;
            if updated != 1 {
                return Err(StoreError::new(StoreErrorCode::StaleRevision));
            }
        }
        insert_generation(&transaction, parts.source_key, 0, "current", checkpoint)?;
        let admitted = transaction.execute(
            "UPDATE usage_source SET current_generation = 0
             WHERE file_key = ?1 AND current_generation IS NULL",
            [parts.source_key.as_bytes().as_slice()],
        )?;
        if admitted != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        validate_foreign_keys(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn mark_current_rebuild_required(
        &mut self,
        revision_id: ReplayRevisionId,
        expected_archive_generation: ArchiveGeneration,
    ) -> Result<ArchiveGeneration, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let next_generation = ArchiveGeneration::new(
            expected_archive_generation
                .get()
                .checked_add(1)
                .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?,
        )?;
        let updated = transaction.execute(
            "UPDATE usage_archive_state SET
               archive_generation = ?1, incremental_state = 'recovery_pending'
             WHERE singleton_id = 1 AND archive_generation = ?2
               AND current_revision_id = ?3
               AND incremental_state IN ('complete','partial')",
            params![
                next_generation.as_sql()?,
                expected_archive_generation.as_sql()?,
                revision_id.as_sql()?,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        validate_foreign_keys(&transaction)?;
        transaction.commit()?;
        Ok(next_generation)
    }

    pub fn register_scan_discovered_source(
        &mut self,
        scan_id: ScanId,
        registration: &SourceRegistration,
    ) -> Result<(), StoreError> {
        let parts = registration.parts();
        if parts.initial_checkpoint.verification() != StoredVerification::Incremental
            || parts.initial_checkpoint.committed_offset() != 0
            || parts.initial_checkpoint.scan_offset() != 0
            || parts.initial_checkpoint.anchor_start() != 0
            || parts.initial_checkpoint.anchor_len() != 0
            || parts.initial_checkpoint.discarding_oversized_line()
            || parts.initial_checkpoint.incomplete_tail()
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let observed_file_length = i64::try_from(parts.initial_checkpoint.observed_file_length())
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let authority: Option<(String, String)> = transaction
            .query_row(
                "SELECT scan.provider_id, scan.profile_id
                 FROM usage_scan AS scan
                 JOIN usage_archive_state AS archive ON archive.singleton_id = 1
                 JOIN usage_replay_revision AS revision
                   ON revision.revision_id = archive.current_revision_id
                  AND revision.status = 'current'
                 WHERE scan.scan_id = ?1 AND scan.completion_state = 'running'
                   AND archive.incremental_state = 'complete'",
                [scan_id.as_sql()?],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((provider, profile)) = authority else {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        };
        if provider != parts.provider_id.as_ref() || profile != parts.profile_id.as_ref() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }

        let existing: Option<(Option<i64>, i64, i64)> = transaction
            .query_row(
                "SELECT source.current_generation,
                        (SELECT count(*) FROM usage_replay_source AS replay
                         WHERE replay.file_key = source.file_key),
                        (SELECT count(*) FROM usage_generation AS generation
                         WHERE generation.file_key = source.file_key
                           AND generation.generation = 0
                           AND generation.status = 'current'
                           AND generation.logical_identity = ?2
                           AND generation.physical_identity IS ?3
                           AND generation.parser_schema_version = ?8
                           AND generation.committed_offset = 0
                           AND generation.scan_offset = 0
                           AND generation.observed_file_length = ?9
                           AND generation.modified_time_ns IS ?10
                           AND generation.anchor_start = 0
                           AND generation.anchor_len = 0
                           AND generation.anchor_sha256 = ?11
                           AND generation.resume_payload = ?12
                           AND generation.discarding_oversized_line = 0
                           AND generation.incomplete_tail = 0
                           AND generation.verification_level = 'incremental')
                 FROM usage_source AS source
                 WHERE source.file_key = ?1
                   AND source.provider_id = ?4 AND source.profile_id = ?5
                   AND source.source_id = ?6 AND source.source_kind = ?7
                   AND source.logical_identity = ?2
                   AND source.physical_identity IS ?3
                   AND source.verification_level = 'incremental'
                   AND source.diagnostic_count = 0",
                params![
                    parts.source_key.as_bytes().as_slice(),
                    parts.logical_identity.as_slice(),
                    parts.physical_identity.as_ref().map(<[u8; 32]>::as_slice),
                    parts.provider_id.as_ref(),
                    parts.profile_id.as_ref(),
                    parts.source_id.as_ref(),
                    parts.source_kind.as_sql(),
                    i64::from(parts.initial_checkpoint.parser_schema_version()),
                    observed_file_length,
                    parts.initial_checkpoint.modified_time_ns(),
                    parts.initial_checkpoint.anchor_sha256().as_slice(),
                    parts.initial_checkpoint.resume(),
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        match existing {
            Some((None, 0, 1)) => {
                let updated = transaction.execute(
                    "UPDATE usage_source SET last_seen_scan_id = ?1, missing = 1
                     WHERE file_key = ?2 AND current_generation IS NULL",
                    params![scan_id.as_sql()?, parts.source_key.as_bytes().as_slice()],
                )?;
                if updated != 1 {
                    return Err(StoreError::new(StoreErrorCode::StaleScan));
                }
            }
            Some(_) => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
            None => {
                let source_key_exists: i64 = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM usage_source WHERE file_key = ?1)",
                    [parts.source_key.as_bytes().as_slice()],
                    |row| row.get(0),
                )?;
                if source_key_exists != 0 {
                    return Err(StoreError::new(StoreErrorCode::RebuildRequired));
                }
                transaction.execute(
                    "INSERT INTO usage_source(
                       file_key, provider_id, profile_id, source_id, source_kind,
                       logical_identity, physical_identity, current_generation,
                       last_seen_scan_id, missing, verification_level, diagnostic_count
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, 1, ?9, 0)",
                    params![
                        parts.source_key.as_bytes().as_slice(),
                        parts.provider_id.as_ref(),
                        parts.profile_id.as_ref(),
                        parts.source_id.as_ref(),
                        parts.source_kind.as_sql(),
                        parts.logical_identity.as_slice(),
                        parts.physical_identity.as_ref().map(<[u8; 32]>::as_slice),
                        scan_id.as_sql()?,
                        parts.initial_checkpoint.verification().as_sql(),
                    ],
                )?;
                insert_generation(
                    &transaction,
                    parts.source_key,
                    0,
                    "current",
                    &parts.initial_checkpoint,
                )?;
            }
        }
        validate_foreign_keys(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn publish_current_scan(
        &mut self,
        publication: &CurrentScanPublication,
    ) -> Result<CurrentReplayCommit, StoreError> {
        let parts = publication.parts();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision = load_current_publication(&transaction, parts)?;
        validate_complete_current_scan(&transaction, parts, revision.expected_source_count)?;
        let discovered_count = u64::try_from(parts.discovered_sources.len())
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let next_expected_source_count = revision
            .expected_source_count
            .checked_add(discovered_count)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let next_expected_source_count = i64::try_from(next_expected_source_count)
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let next_epoch = ReplayEpoch::new(
            parts
                .expected_epoch
                .get()
                .checked_add(1)
                .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?,
        )?;
        let next_generation = ArchiveGeneration::new(
            parts
                .expected_archive_generation
                .get()
                .checked_add(1)
                .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?,
        )?;
        for source_key in &parts.discovered_sources {
            validate_discovered_source(&transaction, parts, *source_key)?;
            let source_state = if discovered_source_is_caught_up(&transaction, *source_key)? {
                "complete"
            } else {
                "pending"
            };
            let inserted = transaction.execute(
                "INSERT INTO usage_replay_source(revision_id, file_key, generation, state)
                 VALUES (?1, ?2, 0, ?3)",
                params![
                    parts.revision_id.as_sql()?,
                    source_key.as_bytes().as_slice(),
                    source_state,
                ],
            )?;
            let admitted = transaction.execute(
                "UPDATE usage_source SET current_generation = 0
                 WHERE file_key = ?1 AND current_generation IS NULL AND missing = 0",
                [source_key.as_bytes().as_slice()],
            )?;
            if inserted != 1 || admitted != 1 {
                return Err(StoreError::new(StoreErrorCode::StaleRevision));
            }
        }
        let revision_updated = transaction.execute(
            "UPDATE usage_replay_revision SET scan_set_id = ?1, evidence_epoch = ?2,
                    expected_source_count = ?5
             WHERE revision_id = ?3 AND status = 'current' AND sealed = 1
               AND promoted = 1 AND evidence_epoch = ?4",
            params![
                parts.scan_set_id.as_sql()?,
                next_epoch.as_sql()?,
                parts.revision_id.as_sql()?,
                parts.expected_epoch.as_sql()?,
                next_expected_source_count,
            ],
        )?;
        if revision_updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        let pending_sources: i64 = transaction.query_row(
            "SELECT count(*) FROM usage_replay_source
             WHERE revision_id = ?1 AND state <> 'complete'",
            [parts.revision_id.as_sql()?],
            |row| row.get(0),
        )?;
        let quality = if pending_sources == 0 {
            ArchivePublicationQuality::Complete
        } else {
            ArchivePublicationQuality::Partial
        };
        let archive_updated = transaction.execute(
            "UPDATE usage_archive_state SET
               archive_generation = ?1,
               latest_complete_scan_set_id = ?2,
               incremental_state = ?5
             WHERE singleton_id = 1 AND archive_generation = ?3
               AND current_revision_id = ?4 AND incremental_state = 'complete'",
            params![
                next_generation.as_sql()?,
                parts.scan_set_id.as_sql()?,
                parts.expected_archive_generation.as_sql()?,
                parts.revision_id.as_sql()?,
                quality.as_sql(),
            ],
        )?;
        if archive_updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        validate_foreign_keys(&transaction)?;
        transaction.commit()?;
        Ok(CurrentReplayCommit {
            processed_count: u16::try_from(parts.discovered_sources.len())
                .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?,
            remaining_work: false,
            epoch: next_epoch,
            archive_generation: next_generation,
            quality,
        })
    }
}

struct CurrentPublicationRevision {
    expected_source_count: u64,
}

struct CurrentPublicationRow {
    canonicalizer_version: i64,
    fingerprint_version: i64,
    replay_signature_version: i64,
    expected_source_count: i64,
    epoch: i64,
    archive_generation: i64,
    current_revision_id: Option<i64>,
    quality: String,
}

fn load_current_publication(
    transaction: &Transaction<'_>,
    parts: &CurrentScanPublicationParts,
) -> Result<CurrentPublicationRevision, StoreError> {
    let raw: Option<CurrentPublicationRow> = transaction
        .query_row(
            "SELECT revision.canonicalizer_version, revision.fingerprint_version,
                    revision.replay_signature_version, revision.expected_source_count,
                    revision.evidence_epoch, archive.archive_generation,
                    archive.current_revision_id, archive.incremental_state
             FROM usage_replay_revision AS revision
             JOIN usage_archive_state AS archive ON archive.singleton_id = 1
             WHERE revision.revision_id = ?1 AND revision.status = 'current'
               AND revision.sealed = 1 AND revision.promoted = 1",
            [parts.revision_id.as_sql()?],
            |row| {
                Ok(CurrentPublicationRow {
                    canonicalizer_version: row.get(0)?,
                    fingerprint_version: row.get(1)?,
                    replay_signature_version: row.get(2)?,
                    expected_source_count: row.get(3)?,
                    epoch: row.get(4)?,
                    archive_generation: row.get(5)?,
                    current_revision_id: row.get(6)?,
                    quality: row.get(7)?,
                })
            },
        )
        .optional()?;
    let raw = raw.ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
    if AccountingVersions::from_stored(
        raw.canonicalizer_version,
        raw.fingerprint_version,
        raw.replay_signature_version,
    )? != AccountingVersions::compiled()
    {
        return Err(StoreError::new(StoreErrorCode::AccountingVersionMismatch));
    }
    if raw.epoch != parts.expected_epoch.as_sql()?
        || raw.archive_generation != parts.expected_archive_generation.as_sql()?
        || raw.current_revision_id != Some(parts.revision_id.as_sql()?)
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    if ArchivePublicationQuality::from_sql(&raw.quality)? != ArchivePublicationQuality::Complete {
        return Err(StoreError::new(StoreErrorCode::PendingContinuation));
    }
    Ok(CurrentPublicationRevision {
        expected_source_count: u64::try_from(raw.expected_source_count)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
    })
}

fn validate_complete_current_scan(
    transaction: &Transaction<'_>,
    parts: &CurrentScanPublicationParts,
    expected_source_count: u64,
) -> Result<(), StoreError> {
    let expected_source_count = i64::try_from(expected_source_count)
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let invalid: (i64, i64, i64, i64, i64, i64) = transaction.query_row(
        "SELECT
           (SELECT count(*) FROM usage_scan_set AS scan_set
            WHERE scan_set.scan_set_id = ?1
              AND scan_set.completion_state = 'complete'
              AND (SELECT count(*) FROM usage_scan
                   WHERE scan_set_id = scan_set.scan_set_id)
                  = scan_set.expected_scope_count
              AND NOT EXISTS(
                SELECT 1 FROM usage_scan
                WHERE scan_set_id = scan_set.scan_set_id
                  AND completion_state <> 'complete'
              )),
           (SELECT count(*) FROM (
              SELECT provider_id, profile_id FROM usage_scan WHERE scan_set_id = ?1
              EXCEPT
              SELECT scan.provider_id, scan.profile_id
              FROM usage_archive_state AS archive
              JOIN usage_scan AS scan
                ON scan.scan_set_id = archive.latest_complete_scan_set_id
              WHERE archive.singleton_id = 1
            )),
           (SELECT count(*) FROM (
              SELECT scan.provider_id, scan.profile_id
              FROM usage_archive_state AS archive
              JOIN usage_scan AS scan
                ON scan.scan_set_id = archive.latest_complete_scan_set_id
              WHERE archive.singleton_id = 1
              EXCEPT
              SELECT provider_id, profile_id FROM usage_scan WHERE scan_set_id = ?1
            )),
           (SELECT count(*) FROM usage_replay_source
            WHERE revision_id = ?2),
           (SELECT count(*) FROM usage_replay_source AS replay
            JOIN usage_generation AS generation
              ON generation.file_key = replay.file_key
             AND generation.generation = replay.generation
            JOIN usage_source AS source
              ON source.file_key = replay.file_key
             AND source.current_generation = replay.generation
            WHERE replay.revision_id = ?2 AND replay.state = 'complete'
              AND generation.status = 'current'),
           (SELECT count(*) FROM usage_replay_source AS replay
            JOIN usage_source AS source ON source.file_key = replay.file_key
            WHERE replay.revision_id = ?2 AND source.missing = 0
              AND NOT EXISTS(
                SELECT 1 FROM usage_scan AS scope
                WHERE scope.scan_set_id = ?1
                  AND scope.provider_id = source.provider_id
                  AND scope.profile_id = source.profile_id
                  AND scope.scan_id = source.last_seen_scan_id
              ))",
        params![parts.scan_set_id.as_sql()?, parts.revision_id.as_sql()?],
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
    )?;
    if invalid.1 != 0 || invalid.2 != 0 {
        return Err(StoreError::new(StoreErrorCode::RebuildRequired));
    }
    if invalid != (1, 0, 0, expected_source_count, expected_source_count, 0) {
        return Err(StoreError::new(StoreErrorCode::IncompleteManifest));
    }
    let mut statement = transaction.prepare(
        "SELECT source.file_key
         FROM usage_source AS source
         JOIN usage_scan AS scope
           ON scope.scan_set_id = ?1
          AND scope.provider_id = source.provider_id
          AND scope.profile_id = source.profile_id
          AND scope.scan_id = source.last_seen_scan_id
         WHERE source.missing = 0 AND NOT EXISTS(
           SELECT 1 FROM usage_replay_source AS replay
           WHERE replay.revision_id = ?2 AND replay.file_key = source.file_key
         )
         ORDER BY source.file_key LIMIT 257",
    )?;
    let unadmitted = statement
        .query_map(
            params![parts.scan_set_id.as_sql()?, parts.revision_id.as_sql()?],
            |row| row.get::<_, Vec<u8>>(0),
        )?
        .map(|row| {
            row.map_err(StoreError::from).and_then(|value| {
                SourceKey::from_slice(&value)
                    .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if unadmitted.as_slice() != parts.discovered_sources.as_ref() {
        return Err(StoreError::new(StoreErrorCode::IncompleteManifest));
    }
    Ok(())
}

fn validate_discovered_source(
    transaction: &Transaction<'_>,
    parts: &CurrentScanPublicationParts,
    source_key: SourceKey,
) -> Result<(), StoreError> {
    let valid: i64 = transaction.query_row(
        "SELECT count(*)
         FROM usage_source AS source
         JOIN usage_generation AS generation
           ON generation.file_key = source.file_key AND generation.generation = 0
         JOIN usage_scan AS scan
           ON scan.scan_set_id = ?1
          AND scan.scan_id = source.last_seen_scan_id
          AND scan.provider_id = source.provider_id
          AND scan.profile_id = source.profile_id
         WHERE source.file_key = ?2 AND source.current_generation IS NULL
           AND source.missing = 0 AND generation.status = 'current'
           AND generation.committed_offset = 0 AND generation.scan_offset = 0
           AND generation.discarding_oversized_line = 0
           AND generation.incomplete_tail = 0
           AND generation.verification_level = 'incremental'
           AND NOT EXISTS(
             SELECT 1 FROM usage_observation
             WHERE file_key = source.file_key AND generation = 0
           )
           AND NOT EXISTS(
             SELECT 1 FROM usage_source_chunk
             WHERE file_key = source.file_key AND generation = 0
           )
           AND NOT EXISTS(
             SELECT 1 FROM usage_replay_source WHERE file_key = source.file_key
           )",
        params![
            parts.scan_set_id.as_sql()?,
            source_key.as_bytes().as_slice()
        ],
        |row| row.get(0),
    )?;
    if valid != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn discovered_source_is_caught_up(
    transaction: &Transaction<'_>,
    source_key: SourceKey,
) -> Result<bool, StoreError> {
    let caught_up: i64 = transaction.query_row(
        "SELECT count(*) FROM usage_generation
         WHERE file_key = ?1 AND generation = 0 AND status = 'current'
           AND committed_offset = scan_offset
           AND scan_offset = observed_file_length
           AND discarding_oversized_line = 0 AND incomplete_tail = 0",
        [source_key.as_bytes().as_slice()],
        |row| row.get(0),
    )?;
    Ok(caught_up == 1)
}

fn validate_foreign_keys(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let failures: i64 =
        transaction.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if failures != 0 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}
