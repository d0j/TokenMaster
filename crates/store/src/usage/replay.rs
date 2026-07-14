use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use tokenmaster_accounting::{
    CanonicalUsageEvent, MAX_REPLAY_DEPTH, MAX_REPLAY_FANOUT, ParentOrdinal,
    ReplayClassificationInput, ReplayClassifier, ReplayDisposition, ReplayEventFacts,
    ReplayEvidence, ReplayTraversalFacts, SessionReplayState,
};

use crate::{StoreError, StoreErrorCode};

use super::{
    UsageStore,
    types::*,
    write::{
        insert_observation, long_context_sql, sql_bool, sql_token, sql_u64, stored_digest,
        update_checkpoint_for_status, upsert_chunk, verify_chunk_conflicts, verify_chunk_proof,
    },
};

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

    pub fn apply_replay_append_batch(
        &mut self,
        batch: &ReplayAppendBatch,
    ) -> Result<ReplayEpoch, StoreError> {
        let replay_parts = batch.parts();
        let append = replay_parts.append_batch.parts();
        if append.last_seen_scan_id.is_some() || append.diagnostic_count_delta != 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision = load_staging_revision(&transaction, replay_parts.revision_id)?;
        if revision.versions != AccountingVersions::compiled() {
            return Err(StoreError::new(StoreErrorCode::AccountingVersionMismatch));
        }
        if revision.epoch != replay_parts.expected_epoch {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        if revision.sealed {
            return Err(StoreError::new(StoreErrorCode::ArchiveModeMismatch));
        }
        let next_epoch_value = revision
            .epoch
            .get()
            .checked_add(1)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let next_epoch = ReplayEpoch::new(next_epoch_value)?;
        let source = load_replay_source(&transaction, replay_parts.revision_id, append.source_key)?;
        source.matches(append)?;
        verify_chunk_proof(
            &transaction,
            append.source_key,
            append.expected_generation,
            append.previous_partial_chunk,
        )?;
        verify_chunk_conflicts(
            &transaction,
            append.source_key,
            append.expected_generation,
            append.previous_partial_chunk,
            &append.chunk_updates,
        )?;

        for event in &append.events {
            validate_event_scope(event, &source, revision.versions)?;
            insert_observation(
                &transaction,
                append.source_key,
                append.expected_generation,
                event,
            )?;
            if !observation_matches(
                &transaction,
                append.source_key,
                append.expected_generation,
                event,
            )? {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }

            let relation = reconcile_session_relation(
                &transaction,
                replay_parts.revision_id,
                append.source_key,
                event,
                next_epoch,
            )?;
            let parent = load_parent_facts(
                &transaction,
                replay_parts.revision_id,
                event,
                revision.versions,
            )?;
            let parent_missing = event.lineage().parent_session_id().is_some() && parent.is_none();
            let parent_ordinal = match parent.as_ref() {
                Some(parent) => ParentOrdinal::Present(parent.as_facts()),
                None if event.lineage().parent_session_id().is_some() => ParentOrdinal::MissingOpen,
                None => ParentOrdinal::NotApplicable,
            };
            let traversal = replay_traversal(
                &transaction,
                replay_parts.revision_id,
                event,
                relation.relation_conflict,
            )?;
            let classification = ReplayClassifier::new().classify(ReplayClassificationInput::new(
                relation.prior_state,
                ReplayEventFacts::from_event(event),
                parent_ordinal,
                traversal.facts,
            ));
            upsert_replay_observation(
                &transaction,
                replay_parts.revision_id,
                append.source_key,
                append.expected_generation,
                event,
                classification.disposition(),
                next_epoch,
            )?;
            update_session_classification(
                &transaction,
                replay_parts.revision_id,
                event,
                classification.next_state(),
                next_epoch,
            )?;
            refresh_replay_selection(
                &transaction,
                replay_parts.revision_id,
                event.fingerprint().as_bytes(),
            )?;
            if parent_missing && classification.disposition() == ReplayDisposition::Pending {
                enqueue_missing_parent(&transaction, replay_parts.revision_id, event, next_epoch)?;
            } else if traversal.depth_exhausted {
                enqueue_classification(
                    &transaction,
                    replay_parts.revision_id,
                    event.provider_id().as_str(),
                    event.profile_id().as_str(),
                    event.session_id().as_str(),
                    "depth_bound",
                    event.lineage().session_ordinal(),
                    next_epoch,
                )?;
            }
            if replay_session_has_children(
                &transaction,
                replay_parts.revision_id,
                event.provider_id().as_str(),
                event.profile_id().as_str(),
                event.session_id().as_str(),
            )? {
                enqueue_child_scan(
                    &transaction,
                    replay_parts.revision_id,
                    event.provider_id().as_str(),
                    event.profile_id().as_str(),
                    event.session_id().as_str(),
                    next_epoch,
                )?;
            }
        }

        for chunk in &append.chunk_updates {
            upsert_chunk(
                &transaction,
                append.source_key,
                append.expected_generation,
                *chunk,
            )?;
        }
        update_checkpoint_for_status(
            &transaction,
            append.source_key,
            append.expected_generation,
            append.expected_committed_offset,
            append.expected_scan_offset,
            &append.next_checkpoint,
            "staging",
        )?;
        let source_state = if checkpoint_is_complete(&append.next_checkpoint) {
            "complete"
        } else {
            "pending"
        };
        let source_updated = transaction.execute(
            "UPDATE usage_replay_source SET state = ?1
             WHERE revision_id = ?2 AND file_key = ?3 AND generation = ?4",
            params![
                source_state,
                replay_parts.revision_id.as_sql()?,
                append.source_key.as_bytes().as_slice(),
                sql_u64(append.expected_generation)?,
            ],
        )?;
        if source_updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        transaction.execute(
            "UPDATE usage_replay_work SET expected_evidence_epoch = ?1
             WHERE revision_id = ?2",
            params![next_epoch.as_sql()?, replay_parts.revision_id.as_sql()?],
        )?;
        let revision_updated = transaction.execute(
            "UPDATE usage_replay_revision SET evidence_epoch = ?1
             WHERE revision_id = ?2 AND status = 'staging' AND sealed = 0
               AND evidence_epoch = ?3",
            params![
                next_epoch.as_sql()?,
                replay_parts.revision_id.as_sql()?,
                replay_parts.expected_epoch.as_sql()?,
            ],
        )?;
        if revision_updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        let foreign_key_failures: i64 =
            transaction.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if foreign_key_failures != 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        transaction.commit()?;
        Ok(next_epoch)
    }

    pub fn apply_replay_relation(
        &mut self,
        relation: &ReplayRelation,
    ) -> Result<ReplayEpoch, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision = load_staging_revision(&transaction, relation.revision_id)?;
        validate_replay_revision(
            &revision,
            relation.expected_epoch,
            AccountingVersions::compiled(),
        )?;
        let source = load_replay_source(&transaction, relation.revision_id, relation.source_key)?;
        if source.provider_id != relation.provider_id.as_ref()
            || source.profile_id != relation.profile_id.as_ref()
            || source.source_id != relation.source_id.as_ref()
            || relation.source_offset >= source.committed_offset
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let next_epoch = next_replay_epoch(revision.epoch)?;
        persist_late_relation(&transaction, relation, next_epoch)?;
        invalidate_session_selections(
            &transaction,
            relation.revision_id,
            &relation.provider_id,
            &relation.profile_id,
            &relation.session_id,
        )?;
        enqueue_classification(
            &transaction,
            relation.revision_id,
            &relation.provider_id,
            &relation.profile_id,
            &relation.session_id,
            "late_relation",
            0,
            next_epoch,
        )?;
        synchronize_work_epochs(&transaction, relation.revision_id, next_epoch)?;
        advance_revision_epoch(
            &transaction,
            relation.revision_id,
            relation.expected_epoch,
            next_epoch,
        )?;
        validate_foreign_keys(&transaction)?;
        transaction.commit()?;
        Ok(next_epoch)
    }

    pub fn continue_replay(
        &mut self,
        revision_id: ReplayRevisionId,
        expected_epoch: ReplayEpoch,
    ) -> Result<ReplayContinuationResult, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision = load_staging_revision(&transaction, revision_id)?;
        validate_replay_revision(&revision, expected_epoch, AccountingVersions::compiled())?;
        reject_stale_work(&transaction, revision_id, expected_epoch)?;
        let Some(work) = load_next_actionable_work(&transaction, revision_id)? else {
            let remaining_work = replay_work_exists(&transaction, revision_id)?;
            return Ok(ReplayContinuationResult {
                processed_count: 0,
                remaining_work,
                epoch: expected_epoch,
            });
        };
        let next_epoch = next_replay_epoch(revision.epoch)?;
        let processed_count = match work.kind.as_str() {
            "classify_session" => process_session_classification(
                &transaction,
                revision_id,
                revision.versions,
                &work,
                next_epoch,
            )?,
            "scan_children" => process_child_scan(&transaction, revision_id, &work, next_epoch)?,
            _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        };
        synchronize_work_epochs(&transaction, revision_id, next_epoch)?;
        advance_revision_epoch(&transaction, revision_id, expected_epoch, next_epoch)?;
        let remaining_work = replay_work_exists(&transaction, revision_id)?;
        validate_foreign_keys(&transaction)?;
        transaction.commit()?;
        Ok(ReplayContinuationResult {
            processed_count,
            remaining_work,
            epoch: next_epoch,
        })
    }
}

struct StoredRevision {
    epoch: ReplayEpoch,
    versions: AccountingVersions,
    sealed: bool,
}

fn validate_replay_revision(
    revision: &StoredRevision,
    expected_epoch: ReplayEpoch,
    expected_versions: AccountingVersions,
) -> Result<(), StoreError> {
    if revision.versions != expected_versions {
        return Err(StoreError::new(StoreErrorCode::AccountingVersionMismatch));
    }
    if revision.epoch != expected_epoch {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    if revision.sealed {
        return Err(StoreError::new(StoreErrorCode::ArchiveModeMismatch));
    }
    Ok(())
}

fn next_replay_epoch(epoch: ReplayEpoch) -> Result<ReplayEpoch, StoreError> {
    ReplayEpoch::new(
        epoch
            .get()
            .checked_add(1)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?,
    )
}

fn advance_revision_epoch(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    expected_epoch: ReplayEpoch,
    next_epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE usage_replay_revision SET evidence_epoch = ?1
         WHERE revision_id = ?2 AND status = 'staging' AND sealed = 0
           AND evidence_epoch = ?3",
        params![
            next_epoch.as_sql()?,
            revision_id.as_sql()?,
            expected_epoch.as_sql()?,
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    Ok(())
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

fn load_staging_revision(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
) -> Result<StoredRevision, StoreError> {
    let raw = transaction
        .query_row(
            "SELECT
               status, canonicalizer_version, fingerprint_version,
               replay_signature_version, evidence_epoch, sealed
             FROM usage_replay_revision WHERE revision_id = ?1",
            [revision_id.as_sql()?],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
    if raw.0 != "staging" {
        return Err(StoreError::new(StoreErrorCode::ArchiveModeMismatch));
    }
    Ok(StoredRevision {
        versions: AccountingVersions::from_stored(raw.1, raw.2, raw.3)?,
        epoch: ReplayEpoch::new(
            u64::try_from(raw.4)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        )
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        sealed: stored_bool(raw.5)?,
    })
}

struct ReplaySource {
    generation: u64,
    committed_offset: u64,
    scan_offset: u64,
    logical_identity: [u8; 32],
    physical_identity: Option<[u8; 32]>,
    provider_id: String,
    profile_id: String,
    source_id: String,
}

impl ReplaySource {
    fn matches(&self, append: &AppendBatchParts) -> Result<(), StoreError> {
        if self.generation != append.expected_generation
            || self.committed_offset != append.expected_committed_offset
            || self.scan_offset != append.expected_scan_offset
            || self.logical_identity != *append.next_checkpoint.logical_identity()
            || self.physical_identity.as_ref() != append.next_checkpoint.physical_identity()
        {
            return Err(StoreError::new(StoreErrorCode::StaleCheckpoint));
        }
        Ok(())
    }
}

fn load_replay_source(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    source_key: SourceKey,
) -> Result<ReplaySource, StoreError> {
    let raw = transaction
        .query_row(
            "SELECT
               rs.generation, g.committed_offset, g.scan_offset,
               g.logical_identity, g.physical_identity,
               s.provider_id, s.profile_id, s.source_id
             FROM usage_replay_source AS rs
             JOIN usage_generation AS g
               ON g.file_key = rs.file_key AND g.generation = rs.generation
             JOIN usage_source AS s ON s.file_key = rs.file_key
             WHERE rs.revision_id = ?1 AND rs.file_key = ?2
               AND g.status = 'staging'",
            params![revision_id.as_sql()?, source_key.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Option<Vec<u8>>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
    Ok(ReplaySource {
        generation: stored_nonnegative(raw.0)?,
        committed_offset: stored_nonnegative(raw.1)?,
        scan_offset: stored_nonnegative(raw.2)?,
        logical_identity: stored_digest(&raw.3)?,
        physical_identity: raw.4.as_deref().map(stored_digest).transpose()?,
        provider_id: raw.5,
        profile_id: raw.6,
        source_id: raw.7,
    })
}

fn validate_event_scope(
    event: &CanonicalUsageEvent,
    source: &ReplaySource,
    versions: AccountingVersions,
) -> Result<(), StoreError> {
    if event.provider_id().as_str() != source.provider_id
        || event.profile_id().as_str() != source.profile_id
        || event.source_id().as_str() != source.source_id
        || event.canonicalizer_version() != versions.canonicalizer()
        || event.fingerprint_version() != versions.fingerprint()
        || event.replay_signature_version() != versions.replay_signature()
    {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    Ok(())
}

fn observation_matches(
    transaction: &Transaction<'_>,
    source_key: SourceKey,
    generation: u64,
    event: &CanonicalUsageEvent,
) -> Result<bool, StoreError> {
    let activity = event.activity().as_array();
    let matched: i64 = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM usage_observation
           WHERE file_key = ?1 AND generation = ?2 AND source_offset = ?3
             AND fingerprint = ?4 AND event_id = ?5 AND profile_id = ?6
             AND session_id = ?7 AND source_id = ?8 AND timestamp_seconds = ?9
             AND timestamp_nanos = ?10 AND model = ?11 AND raw_model IS ?12
             AND input_tokens IS ?13 AND cached_tokens IS ?14
             AND output_tokens IS ?15 AND reasoning_tokens IS ?16
             AND total_tokens IS ?17 AND fallback_model = ?18
             AND long_context = ?19 AND service_tier IS ?20
             AND project_alias IS ?21 AND originator IS ?22
             AND activity_read = ?23 AND activity_edit_write = ?24
             AND activity_search = ?25 AND activity_git = ?26
             AND activity_build_test = ?27 AND activity_web = ?28
             AND activity_subagents = ?29 AND activity_terminal = ?30
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
        |row| row.get(0),
    )?;
    stored_bool(matched)
}

struct SessionRelation {
    prior_state: SessionReplayState,
    relation_conflict: bool,
}

fn reconcile_session_relation(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    source_key: SourceKey,
    event: &CanonicalUsageEvent,
    epoch: ReplayEpoch,
) -> Result<SessionRelation, StoreError> {
    let provider = event.provider_id().as_str();
    let profile = event.profile_id().as_str();
    let session = event.session_id().as_str();
    let parent = event
        .lineage()
        .parent_session_id()
        .map(|value| value.as_str());
    let existing = transaction
        .query_row(
            "SELECT parent_session_id, relation_conflict, state,
                    first_relation_file_key, first_relation_source_offset
             FROM usage_replay_session
             WHERE revision_id = ?1 AND provider_id = ?2
               AND profile_id = ?3 AND session_id = ?4",
            params![revision_id.as_sql()?, provider, profile, session],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()?;
    let declared_conflict = event.lineage().declared_conflict();
    let incoming_identity = (source_key, event.source_offset());
    let (stored_parent, relation_conflict, state, first_identity) = match existing {
        None => {
            let conflict = declared_conflict || parent == Some(session);
            let state = if conflict {
                SessionReplayState::Conflict
            } else if parent.is_some() {
                SessionReplayState::Matching
            } else {
                SessionReplayState::Root
            };
            let first = (parent.is_some() || declared_conflict).then_some(incoming_identity);
            (parent.map(str::to_owned), conflict, state, first)
        }
        Some((stored_parent, stored_conflict, stored_state, first_key, first_offset)) => {
            let mut conflict = stored_bool(stored_conflict)? || declared_conflict;
            let stored_identity = match (first_key, first_offset) {
                (Some(key), Some(offset)) => Some((
                    SourceKey::from_bytes(stored_digest(&key)?),
                    stored_nonnegative(offset)?,
                )),
                (None, None) => None,
                _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
            };
            let incoming_is_relation = parent.is_some() || declared_conflict;
            let incoming_is_first = incoming_is_relation
                && stored_identity.as_ref().is_none_or(|stored| {
                    (incoming_identity.0.as_bytes(), incoming_identity.1)
                        < (stored.0.as_bytes(), stored.1)
                });
            let parent_disagrees = stored_parent
                .as_deref()
                .zip(parent)
                .is_some_and(|(left, right)| left != right);
            conflict |= parent_disagrees;
            let mut resolved_parent = if incoming_is_first {
                parent.map(str::to_owned)
            } else {
                stored_parent
            };
            match (resolved_parent.as_deref(), parent) {
                (None, Some(value)) => resolved_parent = Some(value.to_owned()),
                (Some(left), Some(right)) if left != right => conflict = true,
                _ => {}
            }
            if resolved_parent.as_deref() == Some(session) {
                conflict = true;
            }
            let state = if conflict {
                SessionReplayState::Conflict
            } else if stored_state == "root" && resolved_parent.is_some() {
                SessionReplayState::Matching
            } else {
                session_state_from_sql(&stored_state)?
            };
            let first = if incoming_is_first {
                Some(incoming_identity)
            } else {
                stored_identity
            };
            (resolved_parent, conflict, state, first)
        }
    };
    let first_key = first_identity.map(|identity| *identity.0.as_bytes());
    let first_offset = first_identity.map(|identity| identity.1);
    transaction.execute(
        "INSERT INTO usage_replay_session(
           revision_id, provider_id, profile_id, session_id, parent_session_id,
           relation_conflict, state, completion_state, first_relation_file_key,
           first_relation_source_offset, last_classified_ordinal, evidence_epoch
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', ?8, ?9, NULL, ?10)
         ON CONFLICT(revision_id, provider_id, profile_id, session_id) DO UPDATE SET
           parent_session_id = excluded.parent_session_id,
           relation_conflict = excluded.relation_conflict,
           state = excluded.state,
           first_relation_file_key = excluded.first_relation_file_key,
           first_relation_source_offset = excluded.first_relation_source_offset,
           evidence_epoch = excluded.evidence_epoch",
        params![
            revision_id.as_sql()?,
            provider,
            profile,
            session,
            stored_parent.as_deref(),
            sql_bool(relation_conflict),
            session_state_sql(state),
            first_key.as_ref().map(|value| value.as_slice()),
            first_offset.map(sql_u64).transpose()?,
            epoch.as_sql()?,
        ],
    )?;
    Ok(SessionRelation {
        prior_state: state,
        relation_conflict,
    })
}

struct StoredReplayFacts {
    provider_id: String,
    profile_id: String,
    session_id: String,
    parent_session_id: Option<String>,
    session_ordinal: u64,
    replay_signature: [u8; 32],
    evidence: ReplayEvidence,
    declared_conflict: bool,
}

impl StoredReplayFacts {
    fn as_facts(&self) -> ReplayEventFacts<'_> {
        ReplayEventFacts::new(
            &self.provider_id,
            &self.profile_id,
            &self.session_id,
            self.parent_session_id.as_deref(),
            self.session_ordinal,
            &self.replay_signature,
            self.evidence,
            self.declared_conflict,
        )
    }
}

fn load_parent_facts(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    event: &CanonicalUsageEvent,
    expected_versions: AccountingVersions,
) -> Result<Option<StoredReplayFacts>, StoreError> {
    load_parent_facts_for_session(
        transaction,
        revision_id,
        expected_versions,
        event.provider_id().as_str(),
        event.profile_id().as_str(),
        event
            .lineage()
            .parent_session_id()
            .map(|value| value.as_str()),
        event.lineage().session_ordinal(),
    )
}

fn replay_traversal(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    event: &CanonicalUsageEvent,
    relation_conflict: bool,
) -> Result<StoredTraversal, StoreError> {
    traversal_for_session(
        transaction,
        revision_id,
        event.provider_id().as_str(),
        event.profile_id().as_str(),
        event.session_id().as_str(),
        event
            .lineage()
            .parent_session_id()
            .map(|value| value.as_str()),
        relation_conflict,
    )
}

fn upsert_replay_observation(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    source_key: SourceKey,
    generation: u64,
    event: &CanonicalUsageEvent,
    disposition: ReplayDisposition,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    let signature = event.lineage().signature();
    let parent = event
        .lineage()
        .parent_session_id()
        .map(|value| value.as_str());
    let changed = transaction.execute(
        "INSERT INTO usage_replay_observation(
           revision_id, file_key, generation, source_offset, fingerprint,
           provider_id, profile_id, session_id, parent_session_id, session_ordinal,
           canonicalizer_version, fingerprint_version, replay_signature_version,
           replay_signature, evidence, disposition, declared_conflict, evidence_epoch
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
         )
         ON CONFLICT(revision_id, file_key, generation, source_offset, fingerprint)
         DO UPDATE SET
           disposition = excluded.disposition,
           evidence_epoch = excluded.evidence_epoch
         WHERE provider_id = excluded.provider_id
           AND profile_id = excluded.profile_id
           AND session_id = excluded.session_id
           AND parent_session_id IS excluded.parent_session_id
           AND session_ordinal = excluded.session_ordinal
           AND canonicalizer_version = excluded.canonicalizer_version
           AND fingerprint_version = excluded.fingerprint_version
           AND replay_signature_version = excluded.replay_signature_version
           AND replay_signature = excluded.replay_signature
           AND evidence = excluded.evidence
           AND declared_conflict = excluded.declared_conflict",
        params![
            revision_id.as_sql()?,
            source_key.as_bytes().as_slice(),
            sql_u64(generation)?,
            sql_u64(event.source_offset())?,
            event.fingerprint().as_bytes().as_slice(),
            event.provider_id().as_str(),
            event.profile_id().as_str(),
            event.session_id().as_str(),
            parent,
            sql_u64(event.lineage().session_ordinal())?,
            i64::from(event.canonicalizer_version()),
            i64::from(event.fingerprint_version()),
            i64::from(event.replay_signature_version()),
            signature.as_bytes().as_slice(),
            replay_evidence_sql(event.lineage().evidence()),
            replay_disposition_sql(disposition),
            sql_bool(event.lineage().declared_conflict()),
            epoch.as_sql()?,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn update_session_classification(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    event: &CanonicalUsageEvent,
    state: SessionReplayState,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE usage_replay_session SET
           state = ?1, last_classified_ordinal = ?2, evidence_epoch = ?3
         WHERE revision_id = ?4 AND provider_id = ?5
           AND profile_id = ?6 AND session_id = ?7",
        params![
            session_state_sql(state),
            sql_u64(event.lineage().session_ordinal())?,
            epoch.as_sql()?,
            revision_id.as_sql()?,
            event.provider_id().as_str(),
            event.profile_id().as_str(),
            event.session_id().as_str(),
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn refresh_replay_selection(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    fingerprint: &[u8; 32],
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM usage_replay_selection
         WHERE revision_id = ?1 AND fingerprint = ?2",
        params![revision_id.as_sql()?, fingerprint.as_slice()],
    )?;
    transaction.execute(
        "INSERT INTO usage_replay_selection(
           revision_id, fingerprint, file_key, generation, source_offset,
           canonicalizer_version, fingerprint_version, replay_signature_version
         )
         SELECT
           revision_id, fingerprint, file_key, generation, source_offset,
           canonicalizer_version, fingerprint_version, replay_signature_version
         FROM usage_replay_observation
         WHERE revision_id = ?1 AND fingerprint = ?2 AND disposition = 'eligible'
         ORDER BY profile_id, file_key, generation, source_offset
         LIMIT 1",
        params![revision_id.as_sql()?, fingerprint.as_slice()],
    )?;
    Ok(())
}

fn enqueue_missing_parent(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    event: &CanonicalUsageEvent,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO usage_replay_work(
           revision_id, work_kind, provider_id, profile_id, session_id,
           reason, next_ordinal, child_session_cursor, expected_evidence_epoch
         ) VALUES (?1, 'classify_session', ?2, ?3, ?4, 'missing_parent', ?5, NULL, ?6)
         ON CONFLICT(revision_id, work_kind, provider_id, profile_id, session_id)
         DO UPDATE SET
           reason = excluded.reason,
           next_ordinal = min(next_ordinal, excluded.next_ordinal),
           expected_evidence_epoch = excluded.expected_evidence_epoch",
        params![
            revision_id.as_sql()?,
            event.provider_id().as_str(),
            event.profile_id().as_str(),
            event.session_id().as_str(),
            sql_u64(event.lineage().session_ordinal())?,
            epoch.as_sql()?,
        ],
    )?;
    Ok(())
}

struct RelationTraversal {
    depth: usize,
    cycle: bool,
    ancestor_conflict: bool,
    exhausted: bool,
}

fn relation_traversal(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    provider: &str,
    profile: &str,
    session: &str,
    parent: Option<&str>,
) -> Result<RelationTraversal, StoreError> {
    let Some(parent) = parent else {
        return Ok(RelationTraversal {
            depth: 0,
            cycle: false,
            ancestor_conflict: false,
            exhausted: false,
        });
    };
    let mut current = parent.to_owned();
    let mut visited = Vec::with_capacity(MAX_REPLAY_DEPTH);
    let mut depth = 0_usize;
    loop {
        depth = depth.saturating_add(1);
        if current == session || visited.iter().any(|seen| seen == &current) {
            return Ok(RelationTraversal {
                depth,
                cycle: true,
                ancestor_conflict: false,
                exhausted: false,
            });
        }
        if depth > MAX_REPLAY_DEPTH {
            return Ok(RelationTraversal {
                depth,
                cycle: false,
                ancestor_conflict: false,
                exhausted: true,
            });
        }
        visited.push(current.clone());
        let ancestor = transaction
            .query_row(
                "SELECT parent_session_id, relation_conflict
                 FROM usage_replay_session
                 WHERE revision_id = ?1 AND provider_id = ?2
                   AND profile_id = ?3 AND session_id = ?4",
                params![revision_id.as_sql()?, provider, profile, current],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let Some((next_parent, conflict)) = ancestor else {
            return Ok(RelationTraversal {
                depth,
                cycle: false,
                ancestor_conflict: false,
                exhausted: false,
            });
        };
        if stored_bool(conflict)? {
            return Ok(RelationTraversal {
                depth,
                cycle: false,
                ancestor_conflict: true,
                exhausted: false,
            });
        }
        let Some(next_parent) = next_parent else {
            return Ok(RelationTraversal {
                depth,
                cycle: false,
                ancestor_conflict: false,
                exhausted: false,
            });
        };
        validate_replay_text(&next_parent, 512)?;
        current = next_parent;
    }
}

fn persist_late_relation(
    transaction: &Transaction<'_>,
    relation: &ReplayRelation,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    let existing = transaction
        .query_row(
            "SELECT parent_session_id, relation_conflict, state,
                    first_relation_file_key, first_relation_source_offset
             FROM usage_replay_session
             WHERE revision_id = ?1 AND provider_id = ?2
               AND profile_id = ?3 AND session_id = ?4",
            params![
                relation.revision_id.as_sql()?,
                relation.provider_id.as_ref(),
                relation.profile_id.as_ref(),
                relation.session_id.as_ref(),
            ],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()?;
    let incoming_parent = relation.parent_session_id.as_deref();
    let incoming_key = *relation.source_key.as_bytes();
    let incoming_offset = relation.source_offset;
    let (stored_parent, stored_conflict, stored_state, stored_identity) = match existing {
        Some((parent, conflict, state, key, offset)) => {
            let identity = match (key, offset) {
                (Some(key), Some(offset)) => {
                    Some((stored_digest(&key)?, stored_nonnegative(offset)?))
                }
                (None, None) => None,
                _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
            };
            (
                parent,
                stored_bool(conflict)?,
                session_state_from_sql(&state)?,
                identity,
            )
        }
        None => (None, false, SessionReplayState::Root, None),
    };
    let incoming_is_first = stored_identity
        .as_ref()
        .is_none_or(|stored| (incoming_key, incoming_offset) < *stored);
    let parent_disagrees = stored_parent
        .as_deref()
        .zip(incoming_parent)
        .is_some_and(|(left, right)| left != right);
    let resolved_parent = if incoming_is_first || stored_parent.is_none() {
        incoming_parent.map(str::to_owned)
    } else {
        stored_parent
    };
    let first_identity = if incoming_is_first {
        (incoming_key, incoming_offset)
    } else {
        stored_identity.ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?
    };
    let traversal = relation_traversal(
        transaction,
        relation.revision_id,
        &relation.provider_id,
        &relation.profile_id,
        &relation.session_id,
        resolved_parent.as_deref(),
    )?;
    let conflict = stored_conflict
        || relation.declared_conflict
        || parent_disagrees
        || traversal.cycle
        || traversal.ancestor_conflict;
    let state = if conflict {
        SessionReplayState::Conflict
    } else if stored_state == SessionReplayState::Diverged {
        SessionReplayState::Diverged
    } else if traversal.exhausted {
        SessionReplayState::Pending
    } else if resolved_parent.is_some() {
        SessionReplayState::Matching
    } else {
        SessionReplayState::Root
    };
    let changed = transaction.execute(
        "INSERT INTO usage_replay_session(
           revision_id, provider_id, profile_id, session_id, parent_session_id,
           relation_conflict, state, completion_state, first_relation_file_key,
           first_relation_source_offset, last_classified_ordinal, evidence_epoch
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', ?8, ?9, NULL, ?10)
         ON CONFLICT(revision_id, provider_id, profile_id, session_id) DO UPDATE SET
           parent_session_id = excluded.parent_session_id,
           relation_conflict = excluded.relation_conflict,
           state = excluded.state,
           first_relation_file_key = excluded.first_relation_file_key,
           first_relation_source_offset = excluded.first_relation_source_offset,
           last_classified_ordinal = NULL,
           evidence_epoch = excluded.evidence_epoch",
        params![
            relation.revision_id.as_sql()?,
            relation.provider_id.as_ref(),
            relation.profile_id.as_ref(),
            relation.session_id.as_ref(),
            resolved_parent.as_deref(),
            sql_bool(conflict),
            session_state_sql(state),
            first_identity.0.as_slice(),
            sql_u64(first_identity.1)?,
            epoch.as_sql()?,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn invalidate_session_selections(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    provider: &str,
    profile: &str,
    session: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM usage_replay_selection AS selection
         WHERE selection.revision_id = ?1
           AND EXISTS(
             SELECT 1 FROM usage_replay_observation AS observation
             WHERE observation.revision_id = selection.revision_id
               AND observation.fingerprint = selection.fingerprint
               AND observation.file_key = selection.file_key
               AND observation.generation = selection.generation
               AND observation.source_offset = selection.source_offset
               AND observation.provider_id = ?2
               AND observation.profile_id = ?3
               AND observation.session_id = ?4
           )",
        params![revision_id.as_sql()?, provider, profile, session],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn enqueue_classification(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    provider: &str,
    profile: &str,
    session: &str,
    reason: &str,
    next_ordinal: u64,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    if !matches!(
        reason,
        "late_relation" | "missing_parent" | "parent_changed" | "depth_bound" | "fanout_bound"
    ) {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    transaction.execute(
        "INSERT INTO usage_replay_work(
           revision_id, work_kind, provider_id, profile_id, session_id,
           reason, next_ordinal, child_session_cursor, expected_evidence_epoch
         ) VALUES (?1, 'classify_session', ?2, ?3, ?4, ?5, ?6, NULL, ?7)
         ON CONFLICT(revision_id, work_kind, provider_id, profile_id, session_id)
         DO UPDATE SET
           reason = excluded.reason,
           next_ordinal = min(next_ordinal, excluded.next_ordinal),
           child_session_cursor = NULL,
           expected_evidence_epoch = excluded.expected_evidence_epoch",
        params![
            revision_id.as_sql()?,
            provider,
            profile,
            session,
            reason,
            sql_u64(next_ordinal)?,
            epoch.as_sql()?,
        ],
    )?;
    Ok(())
}

fn enqueue_child_scan(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    provider: &str,
    profile: &str,
    session: &str,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO usage_replay_work(
           revision_id, work_kind, provider_id, profile_id, session_id,
           reason, next_ordinal, child_session_cursor, expected_evidence_epoch
         ) VALUES (?1, 'scan_children', ?2, ?3, ?4, 'parent_changed', 0, NULL, ?5)
         ON CONFLICT(revision_id, work_kind, provider_id, profile_id, session_id)
         DO UPDATE SET
           reason = 'parent_changed',
           child_session_cursor = NULL,
           expected_evidence_epoch = excluded.expected_evidence_epoch",
        params![
            revision_id.as_sql()?,
            provider,
            profile,
            session,
            epoch.as_sql()?,
        ],
    )?;
    Ok(())
}

fn synchronize_work_epochs(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    transaction.execute(
        "UPDATE usage_replay_work SET expected_evidence_epoch = ?1
         WHERE revision_id = ?2",
        params![epoch.as_sql()?, revision_id.as_sql()?],
    )?;
    Ok(())
}

fn reject_stale_work(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    let stale: i64 = transaction.query_row(
        "SELECT count(*) FROM usage_replay_work
         WHERE revision_id = ?1 AND expected_evidence_epoch <> ?2",
        params![revision_id.as_sql()?, epoch.as_sql()?],
        |row| row.get(0),
    )?;
    if stale != 0 {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    Ok(())
}

struct ReplayWork {
    kind: String,
    provider: String,
    profile: String,
    session: String,
    next_ordinal: u64,
    child_cursor: Option<String>,
}

fn load_next_actionable_work(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
) -> Result<Option<ReplayWork>, StoreError> {
    transaction
        .query_row(
            "SELECT work_kind, provider_id, profile_id, session_id,
                    next_ordinal, child_session_cursor
             FROM usage_replay_work
             WHERE revision_id = ?1
               AND (work_kind = 'scan_children'
                    OR reason IN ('late_relation','parent_changed'))
             ORDER BY CASE work_kind WHEN 'scan_children' THEN 0 ELSE 1 END,
                      provider_id, profile_id, session_id
             LIMIT 1",
            [revision_id.as_sql()?],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?
        .map(|raw| {
            validate_replay_text(&raw.1, 64)?;
            validate_replay_text(&raw.2, 128)?;
            validate_replay_text(&raw.3, 512)?;
            if let Some(cursor) = raw.5.as_deref() {
                validate_replay_text(cursor, 512)?;
            }
            Ok(ReplayWork {
                kind: raw.0,
                provider: raw.1,
                profile: raw.2,
                session: raw.3,
                next_ordinal: stored_nonnegative(raw.4)?,
                child_cursor: raw.5,
            })
        })
        .transpose()
}

fn replay_work_exists(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
) -> Result<bool, StoreError> {
    let exists: i64 = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM usage_replay_work WHERE revision_id = ?1)",
        [revision_id.as_sql()?],
        |row| row.get(0),
    )?;
    stored_bool(exists)
}

fn replay_session_has_children(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    provider: &str,
    profile: &str,
    session: &str,
) -> Result<bool, StoreError> {
    let exists: i64 = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM usage_replay_session
           WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
             AND parent_session_id = ?4
         )",
        params![revision_id.as_sql()?, provider, profile, session],
        |row| row.get(0),
    )?;
    stored_bool(exists)
}

struct StoredReplayObservation {
    file_key: [u8; 32],
    generation: u64,
    source_offset: u64,
    fingerprint: [u8; 32],
    provider: String,
    profile: String,
    session: String,
    ordinal: u64,
    versions: AccountingVersions,
    signature: [u8; 32],
    evidence: ReplayEvidence,
    declared_conflict: bool,
}

fn process_session_classification(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    versions: AccountingVersions,
    work: &ReplayWork,
    epoch: ReplayEpoch,
) -> Result<u16, StoreError> {
    let session = load_replay_session(transaction, revision_id, work)?;
    let next_ordinal = transaction
        .query_row(
            "SELECT min(session_ordinal) FROM usage_replay_observation
             WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
               AND session_id = ?4 AND session_ordinal >= ?5",
            params![
                revision_id.as_sql()?,
                work.provider,
                work.profile,
                work.session,
                sql_u64(work.next_ordinal)?,
            ],
            |row| row.get::<_, Option<i64>>(0),
        )?
        .map(stored_nonnegative)
        .transpose()?;
    let Some(ordinal) = next_ordinal else {
        delete_work(transaction, revision_id, work)?;
        enqueue_child_scan(
            transaction,
            revision_id,
            &work.provider,
            &work.profile,
            &work.session,
            epoch,
        )?;
        return Ok(0);
    };
    let observations = load_replay_ordinal(transaction, revision_id, work, ordinal)?;
    if observations.len() > MAX_REPLAY_FANOUT {
        return Err(StoreError::with_limit(
            StoreErrorCode::CapacityExceeded,
            MAX_REPLAY_FANOUT as u64,
        ));
    }
    let traversal = traversal_for_session(
        transaction,
        revision_id,
        &work.provider,
        &work.profile,
        &work.session,
        session.parent.as_deref(),
        session.conflict,
    )?;
    let parent = load_parent_facts_for_session(
        transaction,
        revision_id,
        versions,
        &work.provider,
        &work.profile,
        session.parent.as_deref(),
        ordinal,
    )?;
    let missing_parent = session.parent.is_some() && parent.is_none();
    let parent_ordinal = match parent.as_ref() {
        Some(parent) => ParentOrdinal::Present(parent.as_facts()),
        None if session.parent.is_some() => ParentOrdinal::MissingOpen,
        None => ParentOrdinal::NotApplicable,
    };
    let mut state = if session.conflict {
        SessionReplayState::Conflict
    } else if session.state == SessionReplayState::Diverged {
        SessionReplayState::Diverged
    } else if session.parent.is_some() {
        SessionReplayState::Matching
    } else {
        SessionReplayState::Root
    };
    for observation in &observations {
        if observation.versions != versions
            || observation.provider != work.provider
            || observation.profile != work.profile
            || observation.session != work.session
        {
            return Err(StoreError::new(StoreErrorCode::AccountingVersionMismatch));
        }
        let child = ReplayEventFacts::new(
            &observation.provider,
            &observation.profile,
            &observation.session,
            session.parent.as_deref(),
            observation.ordinal,
            &observation.signature,
            observation.evidence,
            observation.declared_conflict,
        );
        let classification = ReplayClassifier::new().classify(ReplayClassificationInput::new(
            state,
            child,
            parent_ordinal,
            traversal.facts,
        ));
        state = merge_session_state(state, classification.next_state());
        update_persisted_classification(
            transaction,
            revision_id,
            observation,
            session.parent.as_deref(),
            classification.disposition(),
            epoch,
        )?;
        refresh_replay_selection(transaction, revision_id, &observation.fingerprint)?;
    }
    update_persisted_session_state(transaction, revision_id, work, state, ordinal, epoch)?;
    if missing_parent && state != SessionReplayState::Conflict {
        update_work_position(transaction, revision_id, work, "missing_parent", ordinal)?;
    } else if traversal.depth_exhausted {
        update_work_position(transaction, revision_id, work, "depth_bound", ordinal)?;
    } else if replay_ordinal_exists_after(transaction, revision_id, work, ordinal)? {
        update_work_position(
            transaction,
            revision_id,
            work,
            "parent_changed",
            ordinal
                .checked_add(1)
                .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?,
        )?;
    } else {
        delete_work(transaction, revision_id, work)?;
        enqueue_child_scan(
            transaction,
            revision_id,
            &work.provider,
            &work.profile,
            &work.session,
            epoch,
        )?;
    }
    u16::try_from(observations.len()).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

struct StoredReplaySession {
    parent: Option<String>,
    conflict: bool,
    state: SessionReplayState,
}

fn load_replay_session(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    work: &ReplayWork,
) -> Result<StoredReplaySession, StoreError> {
    let raw = transaction
        .query_row(
            "SELECT parent_session_id, relation_conflict, state
             FROM usage_replay_session
             WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
               AND session_id = ?4",
            params![
                revision_id.as_sql()?,
                work.provider,
                work.profile,
                work.session
            ],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if let Some(parent) = raw.0.as_deref() {
        validate_replay_text(parent, 512)?;
    }
    Ok(StoredReplaySession {
        parent: raw.0,
        conflict: stored_bool(raw.1)?,
        state: session_state_from_sql(&raw.2)?,
    })
}

fn load_replay_ordinal(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    work: &ReplayWork,
    ordinal: u64,
) -> Result<Vec<StoredReplayObservation>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT file_key, generation, source_offset, fingerprint,
                provider_id, profile_id, session_id, session_ordinal,
                canonicalizer_version, fingerprint_version, replay_signature_version,
                replay_signature, evidence, declared_conflict
         FROM usage_replay_observation
         WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
           AND session_id = ?4 AND session_ordinal = ?5
         ORDER BY file_key, generation, source_offset, fingerprint
         LIMIT 257",
    )?;
    let rows = statement.query_map(
        params![
            revision_id.as_sql()?,
            work.provider,
            work.profile,
            work.session,
            sql_u64(ordinal)?,
        ],
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, Vec<u8>>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, i64>(13)?,
            ))
        },
    )?;
    rows.map(|row| {
        let raw = row?;
        Ok(StoredReplayObservation {
            file_key: stored_digest(&raw.0)?,
            generation: stored_nonnegative(raw.1)?,
            source_offset: stored_nonnegative(raw.2)?,
            fingerprint: stored_digest(&raw.3)?,
            provider: raw.4,
            profile: raw.5,
            session: raw.6,
            ordinal: stored_nonnegative(raw.7)?,
            versions: AccountingVersions::from_stored(raw.8, raw.9, raw.10)?,
            signature: stored_digest(&raw.11)?,
            evidence: replay_evidence_from_sql(&raw.12)?,
            declared_conflict: stored_bool(raw.13)?,
        })
    })
    .collect()
}

struct StoredTraversal {
    facts: ReplayTraversalFacts,
    depth_exhausted: bool,
}

#[allow(clippy::too_many_arguments)]
fn traversal_for_session(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    provider: &str,
    profile: &str,
    session: &str,
    parent: Option<&str>,
    relation_conflict: bool,
) -> Result<StoredTraversal, StoreError> {
    let relation =
        relation_traversal(transaction, revision_id, provider, profile, session, parent)?;
    let direct_children: i64 = transaction.query_row(
        "SELECT count(*) FROM (
           SELECT session_id FROM usage_replay_session
           WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
             AND parent_session_id = ?4
           ORDER BY session_id LIMIT 257
         )",
        params![revision_id.as_sql()?, provider, profile, session],
        |row| row.get(0),
    )?;
    let direct_children = usize::try_from(direct_children)
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    Ok(StoredTraversal {
        facts: ReplayTraversalFacts::new(
            relation.depth,
            direct_children.min(MAX_REPLAY_FANOUT),
            relation.cycle,
            relation_conflict || relation.ancestor_conflict,
        ),
        depth_exhausted: relation.exhausted,
    })
}

#[allow(clippy::too_many_arguments)]
fn load_parent_facts_for_session(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    expected_versions: AccountingVersions,
    provider: &str,
    profile: &str,
    parent: Option<&str>,
    ordinal: u64,
) -> Result<Option<StoredReplayFacts>, StoreError> {
    let Some(parent) = parent else {
        return Ok(None);
    };
    let raw = transaction
        .query_row(
            "SELECT
               observation.provider_id, observation.profile_id,
               observation.session_id, session.parent_session_id,
               observation.session_ordinal, observation.replay_signature,
               observation.evidence,
               max(observation.declared_conflict, session.relation_conflict),
               observation.canonicalizer_version, observation.fingerprint_version,
               observation.replay_signature_version
             FROM usage_replay_observation AS observation
             JOIN usage_replay_session AS session
               ON session.revision_id = observation.revision_id
              AND session.provider_id = observation.provider_id
              AND session.profile_id = observation.profile_id
              AND session.session_id = observation.session_id
             WHERE observation.revision_id = ?1
               AND observation.provider_id = ?2 AND observation.profile_id = ?3
               AND observation.session_id = ?4 AND observation.session_ordinal = ?5
             ORDER BY observation.file_key, observation.generation,
                      observation.source_offset, observation.fingerprint
             LIMIT 1",
            params![
                revision_id.as_sql()?,
                provider,
                profile,
                parent,
                sql_u64(ordinal)?
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                ))
            },
        )
        .optional()?;
    raw.map(|raw| stored_replay_facts(raw, expected_versions))
        .transpose()
}

type StoredReplayFactsRow = (
    String,
    String,
    String,
    Option<String>,
    i64,
    Vec<u8>,
    String,
    i64,
    i64,
    i64,
    i64,
);

fn stored_replay_facts(
    raw: StoredReplayFactsRow,
    expected_versions: AccountingVersions,
) -> Result<StoredReplayFacts, StoreError> {
    if AccountingVersions::from_stored(raw.8, raw.9, raw.10)? != expected_versions {
        return Err(StoreError::new(StoreErrorCode::AccountingVersionMismatch));
    }
    validate_replay_text(&raw.0, 64)?;
    validate_replay_text(&raw.1, 128)?;
    validate_replay_text(&raw.2, 512)?;
    if let Some(parent) = raw.3.as_deref() {
        validate_replay_text(parent, 512)?;
    }
    Ok(StoredReplayFacts {
        provider_id: raw.0,
        profile_id: raw.1,
        session_id: raw.2,
        parent_session_id: raw.3,
        session_ordinal: stored_nonnegative(raw.4)?,
        replay_signature: stored_digest(&raw.5)?,
        evidence: replay_evidence_from_sql(&raw.6)?,
        declared_conflict: stored_bool(raw.7)?,
    })
}

fn merge_session_state(
    current: SessionReplayState,
    next: SessionReplayState,
) -> SessionReplayState {
    use SessionReplayState::{Conflict, Diverged, Matching, Pending, Root};
    match (current, next) {
        (Conflict, _) | (_, Conflict) => Conflict,
        (Diverged, _) | (_, Diverged) => Diverged,
        (Matching, _) | (_, Matching) => Matching,
        (Pending, _) | (_, Pending) => Pending,
        _ => Root,
    }
}

fn update_persisted_classification(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    observation: &StoredReplayObservation,
    parent: Option<&str>,
    disposition: ReplayDisposition,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE usage_replay_observation SET
           parent_session_id = ?1, disposition = ?2, evidence_epoch = ?3
         WHERE revision_id = ?4 AND file_key = ?5 AND generation = ?6
           AND source_offset = ?7 AND fingerprint = ?8",
        params![
            parent,
            replay_disposition_sql(disposition),
            epoch.as_sql()?,
            revision_id.as_sql()?,
            observation.file_key.as_slice(),
            sql_u64(observation.generation)?,
            sql_u64(observation.source_offset)?,
            observation.fingerprint.as_slice(),
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn update_persisted_session_state(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    work: &ReplayWork,
    state: SessionReplayState,
    ordinal: u64,
    epoch: ReplayEpoch,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE usage_replay_session SET state = ?1,
           last_classified_ordinal = ?2, evidence_epoch = ?3
         WHERE revision_id = ?4 AND provider_id = ?5 AND profile_id = ?6
           AND session_id = ?7",
        params![
            session_state_sql(state),
            sql_u64(ordinal)?,
            epoch.as_sql()?,
            revision_id.as_sql()?,
            work.provider,
            work.profile,
            work.session,
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn replay_ordinal_exists_after(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    work: &ReplayWork,
    ordinal: u64,
) -> Result<bool, StoreError> {
    let exists: i64 = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM usage_replay_observation
           WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
             AND session_id = ?4 AND session_ordinal > ?5
         )",
        params![
            revision_id.as_sql()?,
            work.provider,
            work.profile,
            work.session,
            sql_u64(ordinal)?,
        ],
        |row| row.get(0),
    )?;
    stored_bool(exists)
}

fn update_work_position(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    work: &ReplayWork,
    reason: &str,
    next_ordinal: u64,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE usage_replay_work SET reason = ?1, next_ordinal = ?2
         WHERE revision_id = ?3 AND work_kind = ?4 AND provider_id = ?5
           AND profile_id = ?6 AND session_id = ?7",
        params![
            reason,
            sql_u64(next_ordinal)?,
            revision_id.as_sql()?,
            work.kind,
            work.provider,
            work.profile,
            work.session,
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn delete_work(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    work: &ReplayWork,
) -> Result<(), StoreError> {
    let deleted = transaction.execute(
        "DELETE FROM usage_replay_work
         WHERE revision_id = ?1 AND work_kind = ?2 AND provider_id = ?3
           AND profile_id = ?4 AND session_id = ?5",
        params![
            revision_id.as_sql()?,
            work.kind,
            work.provider,
            work.profile,
            work.session,
        ],
    )?;
    if deleted != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn process_child_scan(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    work: &ReplayWork,
    epoch: ReplayEpoch,
) -> Result<u16, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT session_id, state FROM usage_replay_session
         WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
           AND parent_session_id = ?4
           AND (?5 IS NULL OR session_id > ?5)
         ORDER BY session_id LIMIT 257",
    )?;
    let children = statement
        .query_map(
            params![
                revision_id.as_sql()?,
                work.provider,
                work.profile,
                work.session,
                work.child_cursor.as_deref(),
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = children.len() > MAX_REPLAY_FANOUT;
    let page = &children[..children.len().min(MAX_REPLAY_FANOUT)];
    for (child, state) in page {
        validate_replay_text(child, 512)?;
        if session_state_from_sql(state)? == SessionReplayState::Conflict {
            continue;
        }
        invalidate_session_selections(
            transaction,
            revision_id,
            &work.provider,
            &work.profile,
            child,
        )?;
        enqueue_classification(
            transaction,
            revision_id,
            &work.provider,
            &work.profile,
            child,
            "parent_changed",
            0,
            epoch,
        )?;
    }
    if has_more {
        let cursor = page
            .last()
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        let updated = transaction.execute(
            "UPDATE usage_replay_work SET reason = 'fanout_bound',
               child_session_cursor = ?1
             WHERE revision_id = ?2 AND work_kind = 'scan_children'
               AND provider_id = ?3 AND profile_id = ?4 AND session_id = ?5",
            params![
                &cursor.0,
                revision_id.as_sql()?,
                work.provider,
                work.profile,
                work.session,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
    } else {
        delete_work(transaction, revision_id, work)?;
    }
    u16::try_from(page.len()).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn checkpoint_is_complete(checkpoint: &StoredCheckpoint) -> bool {
    checkpoint.verification() == StoredVerification::FullPrefix
        && !checkpoint.incomplete_tail()
        && !checkpoint.discarding_oversized_line()
        && checkpoint.committed_offset() == checkpoint.scan_offset()
        && checkpoint.scan_offset() == checkpoint.observed_file_length()
}

const fn replay_evidence_sql(evidence: ReplayEvidence) -> &'static str {
    match evidence {
        ReplayEvidence::StrongCumulative => "strong_cumulative",
        ReplayEvidence::WeakUsageOnly => "weak_usage_only",
    }
}

fn replay_evidence_from_sql(value: &str) -> Result<ReplayEvidence, StoreError> {
    match value {
        "strong_cumulative" => Ok(ReplayEvidence::StrongCumulative),
        "weak_usage_only" => Ok(ReplayEvidence::WeakUsageOnly),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

const fn replay_disposition_sql(disposition: ReplayDisposition) -> &'static str {
    match disposition {
        ReplayDisposition::Eligible => "eligible",
        ReplayDisposition::Replay => "replay",
        ReplayDisposition::Pending => "pending",
        ReplayDisposition::Conflict => "conflict",
    }
}

const fn session_state_sql(state: SessionReplayState) -> &'static str {
    match state {
        SessionReplayState::Root => "root",
        SessionReplayState::Matching => "matching",
        SessionReplayState::Diverged => "diverged",
        SessionReplayState::Pending => "pending",
        SessionReplayState::Conflict => "conflict",
    }
}

fn session_state_from_sql(value: &str) -> Result<SessionReplayState, StoreError> {
    match value {
        "root" => Ok(SessionReplayState::Root),
        "matching" => Ok(SessionReplayState::Matching),
        "diverged" => Ok(SessionReplayState::Diverged),
        "pending" => Ok(SessionReplayState::Pending),
        "conflict" => Ok(SessionReplayState::Conflict),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn validate_replay_text(value: &str, max_bytes: usize) -> Result<(), StoreError> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
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
