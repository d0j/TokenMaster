use std::time::{SystemTime, UNIX_EPOCH};

use tokenmaster_engine::{
    AdapterCompletion, AdapterSourceProgress, AdapterSourceState, Archive, ArchiveEpoch,
    ArchiveReplay, ArchiveRevisionId, ArchiveScanSetId, CanonicalBatch, CompletionQuality,
    DiscoveredSource, PortError, PortErrorCode, ReplayContinuation, ReplayContinuationState,
    ScopeIdentity, ScopeManifest, SourceIdentity,
};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, ArchiveGeneration, ArchiveMode, CurrentReplayAppendBatch,
    CurrentReplayAppendBatchParts, CurrentScanPublication, CurrentScanPublicationParts,
    ReplayAppendBatch, ReplayAppendBatchParts, ReplayEpoch, ReplayRevisionId, ScanCounters, ScanId,
    ScanOutcome, ScanScope, ScanSetId, ScanSetManifest, SourceKey, SourceRegistration,
    SourceRegistrationParts, StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk,
    StoredVerification, UsageStore,
};

use crate::error::store_port_error;

pub struct StoreArchive {
    store: UsageStore,
    last_timestamp_ms: i64,
    pending_discovered: Vec<SourceKey>,
    scan_kind: Option<ScanKind>,
}

impl StoreArchive {
    #[must_use]
    pub const fn new(store: UsageStore) -> Self {
        Self {
            store,
            last_timestamp_ms: 0,
            pending_discovered: Vec::new(),
            scan_kind: None,
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

    pub(crate) fn recovery_timestamp_ms(&mut self, floor: i64) -> Result<i64, PortError> {
        let observed = self.timestamp_ms()?;
        self.last_timestamp_ms = observed.max(floor);
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

    pub(crate) fn begin_incremental_scan_set(
        &mut self,
        manifest: &ScopeManifest,
    ) -> Result<ArchiveScanSetId, PortError> {
        self.begin_scan_set_with_kind(manifest, ScanKind::Incremental)
    }

    fn begin_scan_set_with_kind(
        &mut self,
        manifest: &ScopeManifest,
        scan_kind: ScanKind,
    ) -> Result<ArchiveScanSetId, PortError> {
        self.pending_discovered.clear();
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
        self.scan_kind = Some(scan_kind);
        engine_scan_set_id(snapshot.id())
    }

    pub(crate) fn publish_current_scan(
        &mut self,
        scan_set: ArchiveScanSetId,
    ) -> Result<CurrentCursor, PortError> {
        let revision = self
            .store
            .current_replay_revision()
            .map_err(|error| store_port_error(&error))?
            .ok_or_else(|| PortError::new(PortErrorCode::StaleState))?;
        let publication = self
            .store
            .archive_publication()
            .map_err(|error| store_port_error(&error))?;
        let command = CurrentScanPublication::new(CurrentScanPublicationParts {
            revision_id: revision.id(),
            expected_epoch: revision.epoch(),
            expected_archive_generation: publication.generation(),
            scan_set_id: store_scan_set_id(scan_set)?,
            discovered_sources: self.pending_discovered.clone().into_boxed_slice(),
        })
        .map_err(|error| store_port_error(&error))?;
        let committed = self
            .store
            .publish_current_scan(&command)
            .map_err(|error| store_port_error(&error))?;
        self.pending_discovered.clear();
        Ok(CurrentCursor {
            revision_id: revision.id(),
            epoch: committed.epoch(),
            archive_generation: committed.archive_generation(),
        })
    }

    pub(crate) fn current_cursor(&self) -> Result<CurrentCursor, PortError> {
        let revision = self
            .store
            .current_replay_revision()
            .map_err(|error| store_port_error(&error))?
            .ok_or_else(|| PortError::new(PortErrorCode::StaleState))?;
        let publication = self
            .store
            .archive_publication()
            .map_err(|error| store_port_error(&error))?;
        if publication.current_revision() != Some(revision.id()) {
            return Err(PortError::new(PortErrorCode::StaleState));
        }
        Ok(CurrentCursor {
            revision_id: revision.id(),
            epoch: revision.epoch(),
            archive_generation: publication.generation(),
        })
    }

    pub(crate) fn current_progress(
        &self,
        source: &SourceIdentity,
    ) -> Result<AdapterSourceProgress, PortError> {
        let snapshot = self
            .store
            .generation_snapshot(source_key(source))
            .map_err(|error| store_port_error(&error))?
            .ok_or_else(|| PortError::new(PortErrorCode::StaleState))?;
        progress_from_stored(snapshot.checkpoint(), source)
    }

    pub(crate) fn append_current_batch(
        &mut self,
        cursor: CurrentCursor,
        source: &SourceIdentity,
        batch: CanonicalBatch,
    ) -> Result<(CurrentCursor, bool, bool), PortError> {
        let source_key = source_key(source);
        let current = self
            .store
            .generation_snapshot(source_key)
            .map_err(|error| store_port_error(&error))?
            .ok_or_else(|| PortError::new(PortErrorCode::StaleState))?;
        let parts = batch.into_parts();
        let next_checkpoint =
            stored_checkpoint_from_progress(&parts.next_progress, CheckpointStorage::Progress)?;
        let source_caught_up = progress_is_caught_up(&parts.next_progress);
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
            diagnostic_count_delta: parts.counters.diagnostics(),
        })
        .map_err(|error| store_port_error(&error))?;
        let command = CurrentReplayAppendBatch::new(CurrentReplayAppendBatchParts {
            revision_id: cursor.revision_id,
            expected_epoch: cursor.epoch,
            expected_archive_generation: cursor.archive_generation,
            append_batch: append,
            relations: parts.relations,
        })
        .map_err(|error| store_port_error(&error))?;
        let committed = self
            .store
            .apply_current_replay_append_batch(&command)
            .map_err(|error| store_port_error(&error))?;
        Ok((
            CurrentCursor {
                revision_id: cursor.revision_id,
                epoch: committed.epoch(),
                archive_generation: committed.archive_generation(),
            },
            committed.remaining_work(),
            source_caught_up,
        ))
    }

    pub(crate) fn continue_current(
        &mut self,
        cursor: CurrentCursor,
    ) -> Result<(CurrentCursor, bool, u16, bool), PortError> {
        let committed = self
            .store
            .continue_current_replay(cursor.revision_id, cursor.epoch, cursor.archive_generation)
            .map_err(|error| store_port_error(&error))?;
        Ok((
            CurrentCursor {
                revision_id: cursor.revision_id,
                epoch: committed.epoch(),
                archive_generation: committed.archive_generation(),
            },
            committed.remaining_work(),
            committed.processed_count(),
            committed.quality() == tokenmaster_store::ArchivePublicationQuality::Complete,
        ))
    }

    pub(crate) fn mark_rebuild_required(
        &mut self,
        cursor: CurrentCursor,
    ) -> Result<CurrentCursor, PortError> {
        let archive_generation = self
            .store
            .mark_current_rebuild_required(cursor.revision_id, cursor.archive_generation)
            .map_err(|error| store_port_error(&error))?;
        Ok(CurrentCursor {
            archive_generation,
            ..cursor
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CurrentCursor {
    pub(crate) revision_id: ReplayRevisionId,
    pub(crate) epoch: ReplayEpoch,
    pub(crate) archive_generation: ArchiveGeneration,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ScanKind {
    FullRebuild,
    Incremental,
}

impl Archive for StoreArchive {
    fn begin_scan_set(&mut self, manifest: &ScopeManifest) -> Result<ArchiveScanSetId, PortError> {
        self.begin_scan_set_with_kind(manifest, ScanKind::FullRebuild)
    }

    fn observe_source(
        &mut self,
        scan_set: ArchiveScanSetId,
        source: &DiscoveredSource,
        initial_state: &AdapterSourceState,
    ) -> Result<(), PortError> {
        let source_key = source_key(source.identity());
        let progress = initial_state.progress();
        let is_new = self
            .store
            .generation_snapshot(source_key)
            .map_err(|error| store_port_error(&error))?
            .is_none();
        let scan_id = self.scan_for_scope(scan_set, source.identity().scope())?;
        if is_new {
            let archive_mode = self
                .store
                .archive_state()
                .map_err(|error| store_port_error(&error))?
                .mode();
            let checkpoint = stored_checkpoint_from_progress(
                initial_state.progress(),
                if self.scan_kind == Some(ScanKind::Incremental)
                    && archive_mode == ArchiveMode::ReplayVerified
                {
                    CheckpointStorage::IncrementalStart
                } else {
                    CheckpointStorage::ReplayStart
                },
            )?;
            let registration = SourceRegistration::new(SourceRegistrationParts {
                source_key,
                provider_id: source.identity().scope().provider_id().into(),
                profile_id: source.identity().scope().profile_id().into(),
                source_id: source.identity().source_id().into(),
                source_kind: stored_source_kind(source.kind()),
                logical_identity: *source.identity().logical_file_key(),
                physical_identity: progress.physical_identity().copied(),
                initial_checkpoint: checkpoint,
            })
            .map_err(|error| store_port_error(&error))?;
            if self.scan_kind == Some(ScanKind::Incremental)
                && archive_mode == ArchiveMode::ReplayVerified
            {
                if !self.pending_discovered.contains(&source_key)
                    && self.pending_discovered.len() == tokenmaster_store::MAX_REPLAY_SOURCES
                {
                    return Err(PortError::new(PortErrorCode::RebuildRequired));
                }
                self.store
                    .register_scan_discovered_source(scan_id, &registration)
                    .map_err(|error| store_port_error(&error))?;
                if !self.pending_discovered.contains(&source_key) {
                    self.pending_discovered.push(source_key);
                }
            } else {
                self.store
                    .register_rebuild_source(&registration)
                    .map_err(|error| store_port_error(&error))?;
            }
        }
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
        self.scan_kind = None;
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
        initial_state: &AdapterSourceState,
    ) -> Result<ArchiveReplay, PortError> {
        let checkpoint = stored_checkpoint_from_progress(
            initial_state.progress(),
            CheckpointStorage::ReplayStart,
        )?;
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
        let next_checkpoint =
            stored_checkpoint_from_progress(&parts.next_progress, CheckpointStorage::Progress)?;
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
        let next = archive_replay(revision, continuation.epoch())?;
        if continuation.remaining_work()
            && continuation.processed_count() == 0
            && next.epoch() == replay.epoch()
        {
            return Err(PortError::new(PortErrorCode::StaleState));
        }
        let state = if continuation.remaining_work() {
            ReplayContinuationState::Pending
        } else {
            ReplayContinuationState::Complete
        };
        Ok(ReplayContinuation::new(next, state))
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

#[derive(Clone, Copy)]
enum CheckpointStorage {
    ReplayStart,
    IncrementalStart,
    Progress,
}

fn stored_checkpoint_from_progress(
    progress: &AdapterSourceProgress,
    storage: CheckpointStorage,
) -> Result<StoredCheckpoint, PortError> {
    let (
        committed_offset,
        scan_offset,
        observed_file_length,
        modified_time_ns,
        anchor_start,
        anchor_len,
        anchor_sha256,
    ) = match storage {
        CheckpointStorage::ReplayStart => (0, 0, 0, None, 0, 0, [0; 32]),
        CheckpointStorage::IncrementalStart => (
            0,
            0,
            progress.observed_extent(),
            progress.modified_time_ns(),
            0,
            0,
            [0; 32],
        ),
        CheckpointStorage::Progress => (
            progress.committed_offset(),
            progress.scan_offset(),
            progress.observed_extent(),
            progress.modified_time_ns(),
            progress.anchor_start(),
            progress.anchor_len(),
            *progress.anchor_sha256(),
        ),
    };
    let verification = if matches!(
        storage,
        CheckpointStorage::ReplayStart | CheckpointStorage::IncrementalStart
    ) || matches!(
        progress.verification(),
        tokenmaster_engine::AdapterVerification::Incremental
    ) {
        StoredVerification::Incremental
    } else {
        StoredVerification::FullPrefix
    };
    let stored = StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: progress.schema_version(),
        physical_identity: progress.physical_identity().copied(),
        logical_identity: *progress.logical_identity(),
        committed_offset,
        scan_offset,
        observed_file_length,
        modified_time_ns,
        anchor_start,
        anchor_len,
        anchor_sha256,
        resume: progress.provider_resume().to_vec().into_boxed_slice(),
        discarding_oversized_line: matches!(storage, CheckpointStorage::Progress)
            && progress.discarding_oversized_record(),
        incomplete_tail: matches!(storage, CheckpointStorage::Progress)
            && progress.incomplete_tail(),
        verification,
    });
    stored.map_err(|error| store_port_error(&error))
}

fn progress_from_stored(
    checkpoint: &StoredCheckpoint,
    source: &SourceIdentity,
) -> Result<AdapterSourceProgress, PortError> {
    if checkpoint.logical_identity() != source.logical_file_key() {
        return Err(PortError::new(PortErrorCode::InvalidData));
    }
    AdapterSourceProgress::new(tokenmaster_engine::AdapterSourceProgressParts {
        schema_version: checkpoint.parser_schema_version(),
        physical_identity: checkpoint.physical_identity().copied(),
        logical_identity: *checkpoint.logical_identity(),
        committed_offset: checkpoint.committed_offset(),
        scan_offset: checkpoint.scan_offset(),
        observed_extent: checkpoint.observed_file_length(),
        modified_time_ns: checkpoint.modified_time_ns(),
        anchor_start: checkpoint.anchor_start(),
        anchor_len: checkpoint.anchor_len(),
        anchor_sha256: *checkpoint.anchor_sha256(),
        provider_resume: checkpoint.resume().to_vec().into_boxed_slice(),
        discarding_oversized_record: checkpoint.discarding_oversized_line(),
        incomplete_tail: checkpoint.incomplete_tail(),
        verification: match checkpoint.verification() {
            StoredVerification::Incremental => tokenmaster_engine::AdapterVerification::Incremental,
            StoredVerification::FullPrefix => tokenmaster_engine::AdapterVerification::Full,
        },
    })
    .map_err(PortError::from)
}

fn progress_is_caught_up(progress: &AdapterSourceProgress) -> bool {
    !progress.discarding_oversized_record()
        && !progress.incomplete_tail()
        && progress.committed_offset() == progress.scan_offset()
        && progress.scan_offset() == progress.observed_extent()
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

pub(crate) fn archive_replay(
    revision: ReplayRevisionId,
    epoch: ReplayEpoch,
) -> Result<ArchiveReplay, PortError> {
    Ok(ArchiveReplay::new(
        engine_revision_id(revision)?,
        engine_epoch(epoch)?,
    ))
}
