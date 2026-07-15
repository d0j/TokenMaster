use std::time::{SystemTime, UNIX_EPOCH};

use tokenmaster_codex::{CodexCheckpointV1, LogicalFileIdentity, VerificationLevel};
use tokenmaster_engine::{
    AdapterCheckpoint, AdapterCompletion, Archive, ArchiveEpoch, ArchiveReplay, ArchiveRevisionId,
    ArchiveScanSetId, CanonicalBatch, CompletionQuality, DiscoveredSource, PortError,
    PortErrorCode, ReplayContinuation, ReplayContinuationState, ScopeIdentity, ScopeManifest,
    SourceIdentity,
};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, ReplayAppendBatch, ReplayAppendBatchParts, ReplayEpoch,
    ReplayRevisionId, ScanCounters, ScanId, ScanOutcome, ScanScope, ScanSetId, ScanSetManifest,
    SourceKey, SourceRegistration, SourceRegistrationParts, StoredCheckpoint,
    StoredCheckpointParts, StoredSourceChunk, StoredVerification, UsageStore,
};

use crate::error::store_port_error;

pub struct StoreArchive {
    store: UsageStore,
    last_timestamp_ms: i64,
}

impl StoreArchive {
    #[must_use]
    pub const fn new(store: UsageStore) -> Self {
        Self {
            store,
            last_timestamp_ms: 0,
        }
    }

    #[must_use]
    pub const fn store(&self) -> &UsageStore {
        &self.store
    }

    #[must_use]
    pub const fn store_mut(&mut self) -> &mut UsageStore {
        &mut self.store
    }

    #[must_use]
    pub fn into_store(self) -> UsageStore {
        self.store
    }

    fn timestamp_ms(&mut self) -> Result<i64, PortError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PortError::new(PortErrorCode::Unavailable))?;
        let observed = i64::try_from(elapsed.as_millis())
            .map_err(|_| PortError::new(PortErrorCode::CapacityExceeded))?;
        self.last_timestamp_ms = self.last_timestamp_ms.max(observed);
        Ok(self.last_timestamp_ms)
    }

    fn scan_for_scope(
        &self,
        scan_set: ArchiveScanSetId,
        scope: &ScopeIdentity,
    ) -> Result<ScanId, PortError> {
        let stored_set = store_scan_set_id(scan_set)?;
        let page = self
            .store
            .scan_page(stored_set, None, tokenmaster_store::MAX_SCAN_SCOPES)
            .map_err(|error| store_port_error(&error))?;
        page.iter()
            .find(|scan| {
                scan.scope().provider_id() == scope.provider_id()
                    && scan.scope().profile_id() == scope.profile_id()
            })
            .map(tokenmaster_store::ScanSnapshot::id)
            .ok_or_else(|| PortError::new(PortErrorCode::StaleState))
    }
}

impl Archive for StoreArchive {
    fn begin_scan_set(&mut self, manifest: &ScopeManifest) -> Result<ArchiveScanSetId, PortError> {
        let scopes = manifest
            .scopes()
            .iter()
            .map(|scope| ScanScope::new(scope.provider_id(), scope.profile_id()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| store_port_error(&error))?
            .into_boxed_slice();
        let manifest = ScanSetManifest::new(scopes).map_err(|error| store_port_error(&error))?;
        let started_at = self.timestamp_ms()?;
        let snapshot = self
            .store
            .begin_scan_set(&manifest, started_at)
            .map_err(|error| store_port_error(&error))?;
        engine_scan_set_id(snapshot.id())
    }

    fn observe_source(
        &mut self,
        scan_set: ArchiveScanSetId,
        source: &DiscoveredSource,
        initial_checkpoint: &AdapterCheckpoint,
    ) -> Result<(), PortError> {
        let source_key = source_key(source.identity());
        let reader = decode_checkpoint(initial_checkpoint, source.identity())?;
        let checkpoint = stored_checkpoint(&reader, true)?;
        if self
            .store
            .generation_snapshot(source_key)
            .map_err(|error| store_port_error(&error))?
            .is_none()
        {
            let registration = SourceRegistration::new(SourceRegistrationParts {
                source_key,
                provider_id: source.identity().scope().provider_id().into(),
                profile_id: source.identity().scope().profile_id().into(),
                source_id: source.identity().source_id().into(),
                source_kind: stored_source_kind(source.kind()),
                logical_identity: *source.identity().logical_file_key(),
                physical_identity: reader
                    .physical_identity()
                    .map(|identity| *identity.as_bytes()),
                initial_checkpoint: checkpoint,
            })
            .map_err(|error| store_port_error(&error))?;
            self.store
                .register_source(&registration)
                .map_err(|error| store_port_error(&error))?;
        }
        let scan_id = self.scan_for_scope(scan_set, source.identity().scope())?;
        self.store
            .observe_scan_source(scan_id, source_key)
            .map_err(|error| store_port_error(&error))
    }

    fn finish_scope(
        &mut self,
        scan_set: ArchiveScanSetId,
        scope: &ScopeIdentity,
        completion: AdapterCompletion,
    ) -> Result<(), PortError> {
        let scan_id = self.scan_for_scope(scan_set, scope)?;
        let counters = completion.counters();
        let counters = ScanCounters::new(
            counters.files_read(),
            counters.bytes_read(),
            counters.events_observed(),
            counters.diagnostics(),
        )
        .map_err(|error| store_port_error(&error))?;
        let completed_at = self.timestamp_ms()?;
        self.store
            .finish_scan(
                scan_id,
                scan_outcome(completion.quality()),
                completed_at,
                counters,
            )
            .map(|_| ())
            .map_err(|error| store_port_error(&error))
    }

    fn finish_scan_set(
        &mut self,
        scan_set: ArchiveScanSetId,
    ) -> Result<CompletionQuality, PortError> {
        let completed_at = self.timestamp_ms()?;
        let snapshot = self
            .store
            .finish_scan_set(store_scan_set_id(scan_set)?, completed_at)
            .map_err(|error| store_port_error(&error))?;
        snapshot
            .outcome()
            .map(completion_quality)
            .ok_or_else(|| PortError::new(PortErrorCode::StaleState))
    }

    fn begin_replay(&mut self, scan_set: ArchiveScanSetId) -> Result<ArchiveReplay, PortError> {
        let snapshot = self
            .store
            .begin_replay_revision_for_scan_set(store_scan_set_id(scan_set)?)
            .map_err(|error| store_port_error(&error))?;
        archive_replay(snapshot.id(), snapshot.epoch())
    }

    fn prepare_replay_source(
        &mut self,
        replay: ArchiveReplay,
        source: &DiscoveredSource,
        initial_checkpoint: &AdapterCheckpoint,
    ) -> Result<ArchiveReplay, PortError> {
        let reader = decode_checkpoint(initial_checkpoint, source.identity())?;
        let checkpoint = stored_checkpoint(&reader, true)?;
        let revision = store_revision_id(replay.revision_id())?;
        let next_epoch = self
            .store
            .prepare_replay_source(
                revision,
                store_epoch(replay.epoch())?,
                source_key(source.identity()),
                &checkpoint,
            )
            .map_err(|error| store_port_error(&error))?;
        archive_replay(revision, next_epoch)
    }

    fn append_replay_batch(
        &mut self,
        replay: ArchiveReplay,
        source: &SourceIdentity,
        batch: CanonicalBatch,
    ) -> Result<ArchiveReplay, PortError> {
        let source_key = source_key(source);
        let revision = store_revision_id(replay.revision_id())?;
        let current = self
            .store
            .replay_generation_snapshot(revision, source_key)
            .map_err(|error| store_port_error(&error))?;
        let parts = batch.into_parts();
        let next_reader = decode_checkpoint(&parts.next_checkpoint, source)?;
        let next_checkpoint = stored_checkpoint(&next_reader, false)?;
        let previous_partial_chunk = parts
            .chunk_proofs
            .previous_partial()
            .map(stored_chunk)
            .transpose()?;
        let chunk_updates = parts
            .chunk_proofs
            .updates()
            .iter()
            .map(stored_chunk)
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let append = AppendBatch::new(AppendBatchParts {
            source_key,
            expected_generation: current.generation(),
            expected_committed_offset: current.checkpoint().committed_offset(),
            expected_scan_offset: current.checkpoint().scan_offset(),
            events: parts.events,
            previous_partial_chunk,
            chunk_updates,
            next_checkpoint,
            diagnostic_count_delta: 0,
        })
        .map_err(|error| store_port_error(&error))?;
        let replay_batch = ReplayAppendBatch::new(ReplayAppendBatchParts {
            revision_id: revision,
            expected_epoch: store_epoch(replay.epoch())?,
            append_batch: append,
            relations: parts.relations,
        })
        .map_err(|error| store_port_error(&error))?;
        let next_epoch = self
            .store
            .apply_replay_append_batch(&replay_batch)
            .map_err(|error| store_port_error(&error))?;
        archive_replay(revision, next_epoch)
    }

    fn continue_replay(&mut self, replay: ArchiveReplay) -> Result<ReplayContinuation, PortError> {
        let revision = store_revision_id(replay.revision_id())?;
        let continuation = self
            .store
            .continue_replay(revision, store_epoch(replay.epoch())?)
            .map_err(|error| store_port_error(&error))?;
        if continuation.remaining_work() && continuation.processed_count() == 0 {
            return Err(PortError::new(PortErrorCode::StaleState));
        }
        let replay = archive_replay(revision, continuation.epoch())?;
        let state = if continuation.remaining_work() {
            ReplayContinuationState::Pending
        } else {
            ReplayContinuationState::Complete
        };
        Ok(ReplayContinuation::new(replay, state))
    }

    fn seal_replay(&mut self, replay: ArchiveReplay) -> Result<ArchiveReplay, PortError> {
        let revision = store_revision_id(replay.revision_id())?;
        let snapshot = self
            .store
            .seal_replay_revision(revision, store_epoch(replay.epoch())?)
            .map_err(|error| store_port_error(&error))?;
        archive_replay(snapshot.id(), snapshot.epoch())
    }

    fn promote_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError> {
        self.store
            .promote_replay_revision(
                store_revision_id(replay.revision_id())?,
                store_epoch(replay.epoch())?,
            )
            .map(|_| ())
            .map_err(|error| store_port_error(&error))
    }

    fn discard_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError> {
        self.store
            .discard_replay_revision(
                store_revision_id(replay.revision_id())?,
                store_epoch(replay.epoch())?,
            )
            .map_err(|error| store_port_error(&error))
    }
}

impl core::fmt::Debug for StoreArchive {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("StoreArchive")
            .finish_non_exhaustive()
    }
}

fn source_key(source: &SourceIdentity) -> SourceKey {
    SourceKey::from_bytes(*source.logical_file_key())
}

fn decode_checkpoint(
    checkpoint: &AdapterCheckpoint,
    source: &SourceIdentity,
) -> Result<tokenmaster_codex::ReaderCheckpointV1, PortError> {
    CodexCheckpointV1::decode(
        checkpoint.as_bytes(),
        LogicalFileIdentity::from_bytes(*source.logical_file_key()),
    )
    .map(CodexCheckpointV1::into_reader)
    .map_err(|_| PortError::new(PortErrorCode::InvalidData))
}

fn stored_checkpoint(
    checkpoint: &tokenmaster_codex::ReaderCheckpointV1,
    empty: bool,
) -> Result<StoredCheckpoint, PortError> {
    let (committed_offset, scan_offset, observed_file_length, modified_time_ns, anchor) = if empty {
        (0, 0, 0, None, (0, 0, [0; 32]))
    } else {
        (
            checkpoint.committed_offset(),
            checkpoint.scan_offset(),
            checkpoint.observed_file_length(),
            checkpoint.modified_time_ns(),
            (
                checkpoint.anchor().start(),
                checkpoint.anchor().len(),
                *checkpoint.anchor().sha256(),
            ),
        )
    };
    let verification = if empty || checkpoint.verification() == VerificationLevel::Incremental {
        StoredVerification::Incremental
    } else {
        StoredVerification::FullPrefix
    };
    let resume = serde_json::to_vec(checkpoint.resume())
        .map_err(|_| PortError::new(PortErrorCode::InvalidData))?
        .into_boxed_slice();
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: checkpoint.parser_schema_version(),
        physical_identity: checkpoint
            .physical_identity()
            .map(|identity| *identity.as_bytes()),
        logical_identity: *checkpoint.logical_identity().as_bytes(),
        committed_offset,
        scan_offset,
        observed_file_length,
        modified_time_ns,
        anchor_start: anchor.0,
        anchor_len: anchor.1,
        anchor_sha256: anchor.2,
        resume,
        discarding_oversized_line: !empty && checkpoint.discarding_oversized_line(),
        incomplete_tail: !empty && checkpoint.incomplete_tail(),
        verification,
    })
    .map_err(|error| store_port_error(&error))
}

fn stored_chunk(proof: &tokenmaster_engine::ChunkProof) -> Result<StoredSourceChunk, PortError> {
    StoredSourceChunk::new(proof.index(), proof.covered_len(), *proof.sha256())
        .map_err(|error| store_port_error(&error))
}

const fn stored_source_kind(kind: tokenmaster_engine::SourceKind) -> tokenmaster_store::SourceKind {
    match kind {
        tokenmaster_engine::SourceKind::Active => tokenmaster_store::SourceKind::Active,
        tokenmaster_engine::SourceKind::Direct => tokenmaster_store::SourceKind::Direct,
        tokenmaster_engine::SourceKind::Archived => tokenmaster_store::SourceKind::Archived,
    }
}

const fn scan_outcome(quality: CompletionQuality) -> ScanOutcome {
    match quality {
        CompletionQuality::Complete => ScanOutcome::Complete,
        CompletionQuality::Partial => ScanOutcome::Partial,
        CompletionQuality::Cancelled => ScanOutcome::Cancelled,
        CompletionQuality::Failed => ScanOutcome::Failed,
        CompletionQuality::TimedOut => ScanOutcome::TimedOut,
    }
}

const fn completion_quality(outcome: ScanOutcome) -> CompletionQuality {
    match outcome {
        ScanOutcome::Complete => CompletionQuality::Complete,
        ScanOutcome::Partial => CompletionQuality::Partial,
        ScanOutcome::Cancelled => CompletionQuality::Cancelled,
        ScanOutcome::Failed => CompletionQuality::Failed,
        ScanOutcome::TimedOut => CompletionQuality::TimedOut,
    }
}

fn engine_scan_set_id(stored: ScanSetId) -> Result<ArchiveScanSetId, PortError> {
    let value = stored
        .get()
        .checked_add(1)
        .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
    ArchiveScanSetId::new(value).map_err(PortError::from)
}

fn store_scan_set_id(engine: ArchiveScanSetId) -> Result<ScanSetId, PortError> {
    ScanSetId::new(engine.get().saturating_sub(1)).map_err(|error| store_port_error(&error))
}

fn engine_revision_id(stored: ReplayRevisionId) -> Result<ArchiveRevisionId, PortError> {
    let value = stored
        .get()
        .checked_add(1)
        .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
    ArchiveRevisionId::new(value).map_err(PortError::from)
}

fn store_revision_id(engine: ArchiveRevisionId) -> Result<ReplayRevisionId, PortError> {
    ReplayRevisionId::new(engine.get().saturating_sub(1)).map_err(|error| store_port_error(&error))
}

fn engine_epoch(stored: ReplayEpoch) -> Result<ArchiveEpoch, PortError> {
    let value = stored
        .get()
        .checked_add(1)
        .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
    ArchiveEpoch::new(value).map_err(PortError::from)
}

fn store_epoch(engine: ArchiveEpoch) -> Result<ReplayEpoch, PortError> {
    ReplayEpoch::new(engine.get().saturating_sub(1)).map_err(|error| store_port_error(&error))
}

fn archive_replay(
    revision: ReplayRevisionId,
    epoch: ReplayEpoch,
) -> Result<ArchiveReplay, PortError> {
    Ok(ArchiveReplay::new(
        engine_revision_id(revision)?,
        engine_epoch(epoch)?,
    ))
}
