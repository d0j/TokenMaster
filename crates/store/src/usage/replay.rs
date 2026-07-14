use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use tokenmaster_accounting::{
    CanonicalUsageEvent, ParentOrdinal, ReplayClassificationInput, ReplayClassifier,
    ReplayDisposition, ReplayEventFacts, ReplayEvidence, ReplayTraversalFacts, SessionReplayState,
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
                traversal,
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
}

struct StoredRevision {
    epoch: ReplayEpoch,
    versions: AccountingVersions,
    sealed: bool,
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
            "SELECT parent_session_id, relation_conflict, state
             FROM usage_replay_session
             WHERE revision_id = ?1 AND provider_id = ?2
               AND profile_id = ?3 AND session_id = ?4",
            params![revision_id.as_sql()?, provider, profile, session],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let declared_conflict = event.lineage().declared_conflict();
    let (stored_parent, relation_conflict, state) = match existing {
        None => {
            let conflict = declared_conflict || parent == Some(session);
            let state = if conflict {
                SessionReplayState::Conflict
            } else if parent.is_some() {
                SessionReplayState::Matching
            } else {
                SessionReplayState::Root
            };
            (parent.map(str::to_owned), conflict, state)
        }
        Some((stored_parent, stored_conflict, stored_state)) => {
            let mut conflict = stored_bool(stored_conflict)? || declared_conflict;
            let mut resolved_parent = stored_parent;
            match (resolved_parent.as_deref(), parent) {
                (None, Some(value)) => resolved_parent = Some(value.to_owned()),
                (Some(left), Some(right)) if left != right => conflict = true,
                (Some(_), None) => conflict = true,
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
            (resolved_parent, conflict, state)
        }
    };
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
           evidence_epoch = excluded.evidence_epoch",
        params![
            revision_id.as_sql()?,
            provider,
            profile,
            session,
            stored_parent.as_deref(),
            sql_bool(relation_conflict),
            session_state_sql(state),
            source_key.as_bytes().as_slice(),
            sql_u64(event.source_offset())?,
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
    let Some(parent_session_id) = event.lineage().parent_session_id() else {
        return Ok(None);
    };
    let raw = transaction
        .query_row(
            "SELECT
               provider_id, profile_id, session_id, parent_session_id,
               session_ordinal, replay_signature, evidence, declared_conflict,
               canonicalizer_version, fingerprint_version, replay_signature_version
             FROM usage_replay_observation
             WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
               AND session_id = ?4 AND session_ordinal = ?5
             ORDER BY file_key, generation, source_offset, fingerprint
             LIMIT 1",
            params![
                revision_id.as_sql()?,
                event.provider_id().as_str(),
                event.profile_id().as_str(),
                parent_session_id.as_str(),
                sql_u64(event.lineage().session_ordinal())?,
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
    raw.map(|raw| {
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
    })
    .transpose()
}

fn replay_traversal(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    event: &CanonicalUsageEvent,
    relation_conflict: bool,
) -> Result<ReplayTraversalFacts, StoreError> {
    let parent = event
        .lineage()
        .parent_session_id()
        .map(|value| value.as_str());
    let direct_children = if let Some(parent) = parent {
        let count: i64 = transaction.query_row(
            "SELECT count(*) FROM (
               SELECT session_id FROM usage_replay_session
               WHERE revision_id = ?1 AND provider_id = ?2 AND profile_id = ?3
                 AND parent_session_id = ?4
               ORDER BY session_id LIMIT 257
             )",
            params![
                revision_id.as_sql()?,
                event.provider_id().as_str(),
                event.profile_id().as_str(),
                parent,
            ],
            |row| row.get(0),
        )?;
        usize::try_from(count).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?
    } else {
        0
    };
    Ok(ReplayTraversalFacts::new(
        usize::from(parent.is_some()),
        direct_children,
        parent == Some(event.session_id().as_str()),
        relation_conflict,
    ))
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
