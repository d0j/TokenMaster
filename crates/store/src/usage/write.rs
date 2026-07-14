use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use tokenmaster_accounting::CanonicalUsageEvent;
use tokenmaster_domain::{LongContextState, TokenCount};

use super::{AppendBatch, SourceRegistration, StoredCheckpoint, StoredSourceChunk, UsageStore};
use crate::{StoreError, StoreErrorCode};

const REFRESH_CANONICAL_SQL: &str = r#"
INSERT INTO usage_event(
  fingerprint, event_id, selected_file_key, selected_generation,
  selected_source_offset, projection_revision_id, origin_revision_id, retained,
  profile_id, session_id, source_id,
  timestamp_seconds, timestamp_nanos, model, raw_model, input_tokens,
  cached_tokens, output_tokens, reasoning_tokens, total_tokens,
  fallback_model, long_context, service_tier, project_alias, originator,
  activity_read, activity_edit_write, activity_search, activity_git,
  activity_build_test, activity_web, activity_subagents, activity_terminal
)
SELECT
  o.fingerprint, o.event_id, o.file_key, o.generation, o.source_offset,
  current_revision.revision_id, current_revision.revision_id, 0,
  o.profile_id, o.session_id, o.source_id, o.timestamp_seconds,
  o.timestamp_nanos, o.model, o.raw_model, o.input_tokens, o.cached_tokens,
  o.output_tokens, o.reasoning_tokens, o.total_tokens, o.fallback_model,
  o.long_context, o.service_tier, o.project_alias, o.originator,
  o.activity_read, o.activity_edit_write, o.activity_search, o.activity_git,
  o.activity_build_test, o.activity_web, o.activity_subagents, o.activity_terminal
FROM usage_observation AS o
JOIN usage_generation AS g
  ON g.file_key = o.file_key AND g.generation = o.generation
LEFT JOIN usage_replay_revision AS current_revision
  ON current_revision.status = 'current'
WHERE o.fingerprint = ?1 AND g.status = 'current'
ORDER BY o.profile_id, o.file_key, o.generation, o.source_offset
LIMIT 1
"#;

impl UsageStore {
    pub fn register_source(&mut self, registration: &SourceRegistration) -> Result<(), StoreError> {
        let parts = registration.parts();
        let checkpoint = &parts.initial_checkpoint;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, current_generation,
               missing, verification_level, diagnostic_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 0, ?8, 0)",
            params![
                parts.source_key.as_bytes().as_slice(),
                parts.provider_id.as_ref(),
                parts.profile_id.as_ref(),
                parts.source_id.as_ref(),
                parts.source_kind.as_sql(),
                parts.logical_identity.as_slice(),
                parts.physical_identity.as_ref().map(<[u8; 32]>::as_slice),
                checkpoint.verification().as_sql(),
            ],
        )?;
        insert_generation(&transaction, parts.source_key, 0, "current", checkpoint)?;
        let updated = transaction.execute(
            "UPDATE usage_source SET current_generation = 0 WHERE file_key = ?1",
            params![parts.source_key.as_bytes().as_slice()],
        )?;
        if updated != 1 {
            return Err(StoreError::new(StoreErrorCode::Database));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn apply_append_batch(&mut self, batch: &AppendBatch) -> Result<(), StoreError> {
        self.apply_append_batch_inner(batch, ApplyFault::None)
    }

    fn apply_append_batch_inner(
        &mut self,
        batch: &AppendBatch,
        _fault: ApplyFault,
    ) -> Result<(), StoreError> {
        let parts = batch.parts();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = transaction
            .query_row(
                "SELECT
                   s.current_generation, g.committed_offset, g.scan_offset,
                   g.logical_identity, g.physical_identity, s.provider_id,
                   s.profile_id, s.source_id
                 FROM usage_source AS s
                 JOIN usage_generation AS g
                   ON g.file_key = s.file_key AND g.generation = s.current_generation
                 WHERE s.file_key = ?1 AND g.status = 'current'",
                params![parts.source_key.as_bytes().as_slice()],
                |row| {
                    Ok(CurrentSource {
                        generation: row.get(0)?,
                        committed_offset: row.get(1)?,
                        scan_offset: row.get(2)?,
                        logical_identity: row.get(3)?,
                        physical_identity: row.get(4)?,
                        provider_id: row.get(5)?,
                        profile_id: row.get(6)?,
                        source_id: row.get(7)?,
                    })
                },
            )
            .optional()?;
        let Some(current) = current else {
            return Err(StoreError::new(StoreErrorCode::StaleCheckpoint));
        };
        current.matches_batch(parts)?;
        verify_chunk_proof(
            &transaction,
            parts.source_key,
            parts.expected_generation,
            parts.previous_partial_chunk,
        )?;
        verify_chunk_conflicts(
            &transaction,
            parts.source_key,
            parts.expected_generation,
            parts.previous_partial_chunk,
            &parts.chunk_updates,
        )?;

        for event in &parts.events {
            if event.provider_id().as_str() != current.provider_id
                || event.profile_id().as_str() != current.profile_id
                || event.source_id().as_str() != current.source_id
            {
                return Err(StoreError::new(StoreErrorCode::InvalidValue));
            }
            insert_observation(
                &transaction,
                parts.source_key,
                parts.expected_generation,
                event,
            )?;
            refresh_canonical(&transaction, event.fingerprint().as_bytes())?;
        }
        for chunk in &parts.chunk_updates {
            upsert_chunk(
                &transaction,
                parts.source_key,
                parts.expected_generation,
                *chunk,
            )?;
        }
        update_checkpoint(
            &transaction,
            parts.source_key,
            parts.expected_generation,
            parts.expected_committed_offset,
            parts.expected_scan_offset,
            &parts.next_checkpoint,
        )?;
        update_source_metadata(
            &transaction,
            parts.source_key,
            parts.last_seen_scan_id,
            parts.diagnostic_count_delta,
            &parts.next_checkpoint,
        )?;
        #[cfg(test)]
        if _fault == ApplyFault::ProjectionIntegrity {
            let event = parts
                .events
                .first()
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?;
            transaction.execute(
                "UPDATE usage_event SET selected_file_key = zeroblob(32)
                 WHERE fingerprint = ?1",
                params![event.fingerprint().as_bytes().as_slice()],
            )?;
        }
        for event in &parts.events {
            validate_direct_projection(&transaction, event.fingerprint().as_bytes())?;
        }
        transaction.commit()?;
        Ok(())
    }

    #[cfg(test)]
    fn apply_append_batch_with_projection_integrity_failure(
        &mut self,
        batch: &AppendBatch,
    ) -> Result<(), StoreError> {
        self.apply_append_batch_inner(batch, ApplyFault::ProjectionIntegrity)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplyFault {
    None,
    #[cfg(test)]
    ProjectionIntegrity,
}

struct CurrentSource {
    generation: i64,
    committed_offset: i64,
    scan_offset: i64,
    logical_identity: Vec<u8>,
    physical_identity: Option<Vec<u8>>,
    provider_id: String,
    profile_id: String,
    source_id: String,
}

impl CurrentSource {
    fn matches_batch(&self, parts: &super::AppendBatchParts) -> Result<(), StoreError> {
        let generation = stored_u64(self.generation)?;
        let committed_offset = stored_u64(self.committed_offset)?;
        let scan_offset = stored_u64(self.scan_offset)?;
        let logical_identity = stored_digest(&self.logical_identity)?;
        let physical_identity = self
            .physical_identity
            .as_deref()
            .map(stored_digest)
            .transpose()?;
        if generation != parts.expected_generation
            || committed_offset != parts.expected_committed_offset
            || scan_offset != parts.expected_scan_offset
            || logical_identity != *parts.next_checkpoint.logical_identity()
            || physical_identity.as_ref() != parts.next_checkpoint.physical_identity()
        {
            return Err(StoreError::new(StoreErrorCode::StaleCheckpoint));
        }
        Ok(())
    }
}

fn insert_generation(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    status: &str,
    checkpoint: &StoredCheckpoint,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO usage_generation(
           file_key, generation, status, parser_schema_version, physical_identity,
           logical_identity, committed_offset, scan_offset, observed_file_length,
           modified_time_ns, anchor_start, anchor_len, anchor_sha256, resume_payload,
           discarding_oversized_line, incomplete_tail, verification_level
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
           ?15, ?16, ?17
         )",
        params![
            source_key.as_bytes().as_slice(),
            sql_u64(generation)?,
            status,
            i64::from(checkpoint.parser_schema_version()),
            checkpoint.physical_identity().map(<[u8; 32]>::as_slice),
            checkpoint.logical_identity().as_slice(),
            sql_u64(checkpoint.committed_offset())?,
            sql_u64(checkpoint.scan_offset())?,
            sql_u64(checkpoint.observed_file_length())?,
            checkpoint.modified_time_ns(),
            sql_u64(checkpoint.anchor_start())?,
            i64::from(checkpoint.anchor_len()),
            checkpoint.anchor_sha256().as_slice(),
            checkpoint.resume(),
            sql_bool(checkpoint.discarding_oversized_line()),
            sql_bool(checkpoint.incomplete_tail()),
            checkpoint.verification().as_sql(),
        ],
    )?;
    Ok(())
}

pub(super) fn verify_chunk_proof(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    proof: Option<StoredSourceChunk>,
) -> Result<(), StoreError> {
    let Some(proof) = proof else {
        return Ok(());
    };
    let stored = read_chunk(transaction, source_key, generation, proof.index())?;
    if stored != Some(proof) {
        return Err(StoreError::new(StoreErrorCode::StaleCheckpoint));
    }
    Ok(())
}

pub(super) fn verify_chunk_conflicts(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    proof: Option<StoredSourceChunk>,
    updates: &[StoredSourceChunk],
) -> Result<(), StoreError> {
    for update in updates {
        let stored = read_chunk(transaction, source_key, generation, update.index())?;
        if let Some(stored) = stored
            && stored != *update
            && proof.is_none_or(|value| value != stored || value.index() != update.index())
        {
            return Err(StoreError::new(StoreErrorCode::StaleCheckpoint));
        }
    }
    Ok(())
}

fn read_chunk(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    chunk_index: u64,
) -> Result<Option<StoredSourceChunk>, StoreError> {
    let raw = transaction
        .query_row(
            "SELECT covered_len, sha256 FROM usage_source_chunk
             WHERE file_key = ?1 AND generation = ?2 AND chunk_index = ?3",
            params![
                source_key.as_bytes().as_slice(),
                sql_u64(generation)?,
                sql_u64(chunk_index)?,
            ],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .optional()?;
    raw.map(|(covered_len, sha256)| {
        StoredSourceChunk::new(
            chunk_index,
            u32::try_from(covered_len)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
            stored_digest(&sha256)?,
        )
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
    })
    .transpose()
}

pub(super) fn insert_observation(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    event: &CanonicalUsageEvent,
) -> Result<(), StoreError> {
    let activity = event.activity().as_array();
    transaction.execute(
        "INSERT OR IGNORE INTO usage_observation(
           file_key, generation, source_offset, fingerprint, event_id, profile_id,
           session_id, source_id, timestamp_seconds, timestamp_nanos, model, raw_model,
           input_tokens, cached_tokens, output_tokens, reasoning_tokens, total_tokens,
           fallback_model, long_context, service_tier, project_alias, originator,
           activity_read, activity_edit_write, activity_search, activity_git,
           activity_build_test, activity_web, activity_subagents, activity_terminal
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
           ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26,
           ?27, ?28, ?29, ?30
         )",
        params![
            source_key.as_bytes().as_slice(),
            sql_u64(generation)?,
            sql_u64(event.source_offset())?,
            event.fingerprint().as_bytes().as_slice(),
            event.id().as_str(),
            event.profile_id().as_str(),
            event.session_id().as_str(),
            event.source_id().as_str(),
            event.timestamp().unix_seconds(),
            i64::from(event.timestamp().subsec_nanos()),
            event.model().as_str(),
            event.raw_model().map(|value| value.as_str()),
            sql_token(event.usage().input())?,
            sql_token(event.usage().cached())?,
            sql_token(event.usage().output())?,
            sql_token(event.usage().reasoning())?,
            sql_token(event.usage().total())?,
            sql_bool(event.fallback_model()),
            long_context_sql(event.long_context()),
            event.service_tier().map(|value| value.as_str()),
            event.project().map(|value| value.as_str()),
            event.originator().map(|value| value.as_str()),
            sql_u64(activity[0])?,
            sql_u64(activity[1])?,
            sql_u64(activity[2])?,
            sql_u64(activity[3])?,
            sql_u64(activity[4])?,
            sql_u64(activity[5])?,
            sql_u64(activity[6])?,
            sql_u64(activity[7])?,
        ],
    )?;
    Ok(())
}

fn refresh_canonical(
    transaction: &Transaction<'_>,
    fingerprint: &[u8; 32],
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM usage_event WHERE fingerprint = ?1",
        params![fingerprint.as_slice()],
    )?;
    let inserted = transaction.execute(REFRESH_CANONICAL_SQL, params![fingerprint.as_slice()])?;
    if inserted != 1 {
        return Err(StoreError::new(StoreErrorCode::Database));
    }
    Ok(())
}

fn validate_direct_projection(
    transaction: &Transaction<'_>,
    fingerprint: &[u8; 32],
) -> Result<(), StoreError> {
    let valid: i64 = transaction.query_row(
        "SELECT count(*)
         FROM usage_event AS event
         JOIN usage_observation AS observation
           ON observation.file_key = event.selected_file_key
          AND observation.generation = event.selected_generation
          AND observation.source_offset = event.selected_source_offset
          AND observation.fingerprint = event.fingerprint
         WHERE event.fingerprint = ?1
           AND event.retained = 0
           AND (
             (event.projection_revision_id IS NULL
              AND event.origin_revision_id IS NULL
              AND NOT EXISTS(
                SELECT 1 FROM usage_replay_revision WHERE status = 'current'
              ))
             OR
             (event.projection_revision_id = event.origin_revision_id
              AND event.projection_revision_id = (
                SELECT revision_id FROM usage_replay_revision WHERE status = 'current'
              ))
           )",
        [fingerprint.as_slice()],
        |row| row.get(0),
    )?;
    if valid != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

pub(super) fn upsert_chunk(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    chunk: StoredSourceChunk,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO usage_source_chunk(
           file_key, generation, chunk_index, covered_len, sha256
         ) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(file_key, generation, chunk_index) DO UPDATE SET
           covered_len = excluded.covered_len,
           sha256 = excluded.sha256",
        params![
            source_key.as_bytes().as_slice(),
            sql_u64(generation)?,
            sql_u64(chunk.index())?,
            i64::from(chunk.covered_len()),
            chunk.sha256().as_slice(),
        ],
    )?;
    Ok(())
}

fn update_checkpoint(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    expected_committed_offset: u64,
    expected_scan_offset: u64,
    checkpoint: &StoredCheckpoint,
) -> Result<(), StoreError> {
    update_checkpoint_for_status(
        transaction,
        source_key,
        generation,
        expected_committed_offset,
        expected_scan_offset,
        checkpoint,
        "current",
    )
}

pub(super) fn update_checkpoint_for_status(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    generation: u64,
    expected_committed_offset: u64,
    expected_scan_offset: u64,
    checkpoint: &StoredCheckpoint,
    status: &str,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE usage_generation SET
           parser_schema_version = ?1,
           physical_identity = ?2,
           logical_identity = ?3,
           committed_offset = ?4,
           scan_offset = ?5,
           observed_file_length = ?6,
           modified_time_ns = ?7,
           anchor_start = ?8,
           anchor_len = ?9,
           anchor_sha256 = ?10,
           resume_payload = ?11,
           discarding_oversized_line = ?12,
           incomplete_tail = ?13,
           verification_level = ?14
         WHERE file_key = ?15 AND generation = ?16 AND status = ?19
           AND committed_offset = ?17 AND scan_offset = ?18",
        params![
            i64::from(checkpoint.parser_schema_version()),
            checkpoint.physical_identity().map(<[u8; 32]>::as_slice),
            checkpoint.logical_identity().as_slice(),
            sql_u64(checkpoint.committed_offset())?,
            sql_u64(checkpoint.scan_offset())?,
            sql_u64(checkpoint.observed_file_length())?,
            checkpoint.modified_time_ns(),
            sql_u64(checkpoint.anchor_start())?,
            i64::from(checkpoint.anchor_len()),
            checkpoint.anchor_sha256().as_slice(),
            checkpoint.resume(),
            sql_bool(checkpoint.discarding_oversized_line()),
            sql_bool(checkpoint.incomplete_tail()),
            checkpoint.verification().as_sql(),
            source_key.as_bytes().as_slice(),
            sql_u64(generation)?,
            sql_u64(expected_committed_offset)?,
            sql_u64(expected_scan_offset)?,
            status,
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::StaleCheckpoint));
    }
    Ok(())
}

pub(super) fn update_source_metadata(
    transaction: &Transaction<'_>,
    source_key: super::SourceKey,
    last_seen_scan_id: Option<u64>,
    diagnostic_count_delta: u64,
    checkpoint: &StoredCheckpoint,
) -> Result<(), StoreError> {
    let delta = sql_u64(diagnostic_count_delta)?;
    let updated = transaction.execute(
        "UPDATE usage_source SET
           last_seen_scan_id = COALESCE(?1, last_seen_scan_id),
           missing = 0,
           verification_level = ?2,
           diagnostic_count = CASE
             WHEN diagnostic_count > 9223372036854775807 - ?3
               THEN 9223372036854775807
             ELSE diagnostic_count + ?3
           END
         WHERE file_key = ?4",
        params![
            last_seen_scan_id.map(sql_u64).transpose()?,
            checkpoint.verification().as_sql(),
            delta,
            source_key.as_bytes().as_slice(),
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::StaleCheckpoint));
    }
    Ok(())
}

pub(super) fn sql_u64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))
}

pub(super) const fn sql_bool(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub(super) fn sql_token(value: TokenCount) -> Result<Option<i64>, StoreError> {
    match value {
        TokenCount::Available(value) => sql_u64(value).map(Some),
        TokenCount::Unavailable => Ok(None),
    }
}

pub(super) const fn long_context_sql(value: LongContextState) -> &'static str {
    match value {
        LongContextState::Yes => "yes",
        LongContextState::No => "no",
        LongContextState::Unavailable => "unavailable",
    }
}

fn stored_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

pub(super) fn stored_digest(value: &[u8]) -> Result<[u8; 32], StoreError> {
    <[u8; 32]>::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

#[cfg(test)]
mod tests {
    use tokenmaster_accounting::Canonicalizer;
    use tokenmaster_domain::{
        ActivityCounts, MetadataValue, ModelKey, ObservationDraft, ObservationDraftParts,
        ObservationVerification, ProjectAlias, TokenUsage, UsageProfileId, UsageProviderId,
        UsageSessionId, UsageSourceId, UtcTimestamp,
    };

    use super::*;
    use crate::{
        AppendBatchParts, SourceKey, SourceKind, SourceRegistrationParts, StoredCheckpointParts,
        StoredVerification,
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn checkpoint(seed: u8, offset: u64) -> Result<StoredCheckpoint, StoreError> {
        StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some([seed; 32]),
            logical_identity: [seed.wrapping_add(1); 32],
            committed_offset: offset,
            scan_offset: offset,
            observed_file_length: offset,
            modified_time_ns: Some(i64::from(seed)),
            anchor_start: 0,
            anchor_len: u16::try_from(offset.min(100))
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?,
            anchor_sha256: [seed.wrapping_add(2); 32],
            resume: vec![seed].into_boxed_slice(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::Incremental,
        })
    }

    fn registration(seed: u8) -> Result<SourceRegistration, StoreError> {
        SourceRegistration::new(SourceRegistrationParts {
            source_key: SourceKey::from_bytes([seed; 32]),
            provider_id: "codex".into(),
            profile_id: "default".into(),
            source_id: "fixture".into(),
            source_kind: SourceKind::Active,
            logical_identity: [seed.wrapping_add(1); 32],
            physical_identity: Some([seed; 32]),
            initial_checkpoint: checkpoint(seed, 0)?,
        })
    }

    fn event(fingerprint: u8) -> TestResult<CanonicalUsageEvent> {
        let draft = ObservationDraft::new(ObservationDraftParts {
            provider_id: UsageProviderId::new("codex")?,
            profile_id: UsageProfileId::new("default")?,
            session_id: UsageSessionId::new("session")?,
            parent_session_id: None,
            session_ordinal: u64::from(fingerprint),
            lineage_conflict: false,
            source_id: UsageSourceId::new("fixture")?,
            source_offset: 10,
            source_verification: ObservationVerification::Incremental,
            timestamp: UtcTimestamp::new(100, 0)?,
            model: ModelKey::new("gpt-test")?,
            raw_model: Some(MetadataValue::new("gpt-test")?),
            delta_usage: TokenUsage::new(
                TokenCount::Available(10),
                TokenCount::Unavailable,
                TokenCount::Available(2),
                TokenCount::Unavailable,
                TokenCount::Available(12),
            ),
            cumulative_usage: None,
            fallback_model: false,
            long_context: LongContextState::No,
            service_tier: None,
            project: Some(ProjectAlias::new("tokenmaster")?),
            originator: None,
            activity: ActivityCounts::default(),
        })?;
        Ok(Canonicalizer::new().canonicalize(&draft)?)
    }

    fn batch(seed: u8, fingerprint: u8) -> TestResult<AppendBatch> {
        Ok(AppendBatch::new(AppendBatchParts {
            source_key: SourceKey::from_bytes([seed; 32]),
            expected_generation: 0,
            expected_committed_offset: 0,
            expected_scan_offset: 0,
            events: vec![event(fingerprint)?].into_boxed_slice(),
            previous_partial_chunk: None,
            chunk_updates: vec![StoredSourceChunk::new(0, 100, [8; 32])?].into_boxed_slice(),
            next_checkpoint: checkpoint(seed, 100)?,
            last_seen_scan_id: None,
            diagnostic_count_delta: 0,
        })?)
    }

    #[test]
    fn projection_integrity_fault_rolls_back_every_append_write() -> TestResult {
        let mut store = UsageStore::in_memory()?;
        store.register_source(&registration(7)?)?;
        let before = store.counts()?;
        let batch = batch(7, 1)?;

        let error = match store.apply_append_batch_with_projection_integrity_failure(&batch) {
            Ok(()) => return Err("invalid canonical projection unexpectedly committed".into()),
            Err(error) => error,
        };
        assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
        assert_eq!(store.counts()?, before);
        let snapshot = store
            .generation_snapshot(SourceKey::from_bytes([7; 32]))?
            .ok_or("missing rollback snapshot")?;
        assert_eq!(snapshot.checkpoint().committed_offset(), 0);

        store.apply_append_batch(&batch)?;
        assert_eq!(store.counts()?.canonical_events(), 1);
        Ok(())
    }

    #[test]
    fn canonical_rebuild_selects_remaining_smallest_current_observation() -> TestResult {
        let mut store = UsageStore::in_memory()?;
        let fingerprint = *event(3)?.fingerprint().as_bytes();
        for seed in [9_u8, 1_u8] {
            store.register_source(&registration(seed)?)?;
            store.apply_append_batch(&batch(seed, 3)?)?;
        }
        let transaction = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "DELETE FROM usage_event WHERE fingerprint = ?1",
            params![fingerprint.as_slice()],
        )?;
        transaction.execute(
            "DELETE FROM usage_observation
             WHERE file_key = ?1 AND generation = 0 AND fingerprint = ?2",
            params![[1_u8; 32].as_slice(), fingerprint.as_slice()],
        )?;
        refresh_canonical(&transaction, &fingerprint)?;
        transaction.commit()?;

        let selected: Vec<u8> = store.connection.query_row(
            "SELECT selected_file_key FROM usage_event WHERE fingerprint = ?1",
            params![fingerprint.as_slice()],
            |row| row.get(0),
        )?;
        assert_eq!(selected, [9_u8; 32]);
        let counts = store.counts()?;
        assert_eq!(counts.observations(), 1);
        assert_eq!(counts.canonical_events(), 1);
        Ok(())
    }
}
