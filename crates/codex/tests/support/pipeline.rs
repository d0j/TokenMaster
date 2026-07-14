use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tokenmaster_accounting::Canonicalizer;
use tokenmaster_codex::{
    BoundaryAnchor, CodexProvider, CodexRootInput, ConfiguredCodexRoot, EnumerationCompletion,
    IntegrityReport, LogicalFileIdentity, PARSER_SCHEMA_VERSION, ParserDiagnosticCode,
    ParserResumeState, ParserState, ReadBatch, ReaderCheckpointParts, ReaderCheckpointV1,
    ReaderError, ReaderErrorCode, ReaderOutcome, RebuildReason, SinkDecision, SourceChunkDigest,
    SourceFileDescriptor, VerificationLevel, build_discovery_request, enumerate_profile_sources,
    logical_file_identity, read_source_batch, verify_full_prefix,
};
use tokenmaster_platform::PhysicalFileIdentity;
use tokenmaster_provider::{DiscoveryProvider, SourceKind as ProviderSourceKind};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, EventCursor, ReplayAppendBatch, ReplayAppendBatchParts,
    ReplayEpoch, ReplayQualityCounts, ReplayRelation, ReplayRevisionId, SourceKey,
    SourceKind as StoreSourceKind, SourceRegistration, SourceRegistrationParts, StoreError,
    StoreErrorCode, StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk, StoredUsageEvent,
    StoredVerification, UsageStore,
};

const CONTINUATION_LIMIT: usize = 16_384;

#[derive(Clone, Copy, Debug)]
pub struct PipelineOptions {
    pub collect_event_ids: bool,
    pub restart_after_batches: Option<u64>,
    pub cancel_reader_after_batches: Option<u64>,
    pub cancel_enumeration_after_files: Option<u64>,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            collect_event_ids: true,
            restart_after_batches: None,
            cancel_reader_after_batches: None,
            cancel_enumeration_after_files: None,
        }
    }
}

#[derive(Debug)]
pub struct PipelineResult {
    pub registered_files: u64,
    pub visible_before_promotion: u64,
    pub visible_events: u64,
    pub visible_total_tokens: u64,
    pub visible_event_ids: BTreeSet<String>,
    pub quality: ReplayQualityCounts,
    pub max_reader_batch: usize,
    pub max_event_page: usize,
    pub restarts: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipelineError {
    Provider,
    Enumeration,
    EnumerationIncomplete,
    Reader(ReaderErrorCode),
    UnexpectedReaderOutcome,
    ReaderCheckpoint,
    Canonicalization,
    Store(StoreErrorCode),
    MalformedInput,
    IncompleteTail,
    Integrity,
    Cancelled,
    ContinuationStalled,
    ContinuationLimit,
    Capacity,
}

impl From<StoreError> for PipelineError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.code())
    }
}

impl From<ReaderError> for PipelineError {
    fn from(error: ReaderError) -> Self {
        Self::Reader(error.code())
    }
}

struct ReopenableStore {
    path: PathBuf,
    store: UsageStore,
    restart_after_batches: Option<u64>,
    applied_batches: u64,
    restarts: u64,
}

impl ReopenableStore {
    fn open(path: &Path, restart_after_batches: Option<u64>) -> Result<Self, PipelineError> {
        Ok(Self {
            path: path.to_path_buf(),
            store: UsageStore::open(path)?,
            restart_after_batches,
            applied_batches: 0,
            restarts: 0,
        })
    }

    fn record_batch_and_maybe_reopen(&mut self) -> Result<(), PipelineError> {
        self.applied_batches = self
            .applied_batches
            .checked_add(1)
            .ok_or(PipelineError::Capacity)?;
        if self.restart_after_batches == Some(self.applied_batches) {
            self.store = UsageStore::open(&self.path)?;
            self.restarts = self
                .restarts
                .checked_add(1)
                .ok_or(PipelineError::Capacity)?;
        }
        Ok(())
    }

    fn reopen(&mut self) -> Result<(), PipelineError> {
        self.store = UsageStore::open(&self.path)?;
        self.restarts = self
            .restarts
            .checked_add(1)
            .ok_or(PipelineError::Capacity)?;
        Ok(())
    }
}

struct PipelineState {
    revision_id: ReplayRevisionId,
    epoch: ReplayEpoch,
    enumerated_files: u64,
    max_reader_batch: usize,
    cancel_reader_after_batches: Option<u64>,
}

pub fn run_pipeline(
    root: &Path,
    database: &Path,
    options: PipelineOptions,
) -> Result<PipelineResult, PipelineError> {
    let configured = [ConfiguredCodexRoot::new(
        root,
        Some("Pipeline fixture".to_owned()),
        true,
    )];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .map_err(|_| PipelineError::Provider)?;
    let provider = CodexProvider::new().map_err(|_| PipelineError::Provider)?;
    let discovery = provider
        .discover(&request)
        .map_err(|_| PipelineError::Provider)?;
    if discovery.sources().is_empty() {
        return Err(PipelineError::EnumerationIncomplete);
    }

    let mut archive = ReopenableStore::open(database, options.restart_after_batches)?;
    let registered_files = register_complete_source_set(
        &discovery,
        &mut archive.store,
        options.cancel_enumeration_after_files,
    )?;
    let revision = archive.store.begin_replay_revision_all_sources()?;
    let mut state = PipelineState {
        revision_id: revision.id(),
        epoch: revision.epoch(),
        enumerated_files: 0,
        max_reader_batch: 0,
        cancel_reader_after_batches: options.cancel_reader_after_batches,
    };

    let rebuild = rebuild_complete_source_set(&discovery, &mut archive, &mut state)
        .and_then(|()| finish_replay(&mut archive.store, &mut state));
    if let Err(error) = rebuild {
        archive
            .store
            .discard_replay_revision(state.revision_id, state.epoch)
            .map_err(PipelineError::from)?;
        return Err(error);
    }

    let before = visible_summary(&archive.store, false)?;
    let sealed = archive
        .store
        .seal_replay_revision(state.revision_id, state.epoch)?;
    state.epoch = sealed.epoch();
    if let Err(error) = archive
        .store
        .promote_replay_revision(state.revision_id, state.epoch)
    {
        archive
            .store
            .discard_replay_revision(state.revision_id, state.epoch)
            .map_err(PipelineError::from)?;
        return Err(error.into());
    }
    archive.reopen()?;

    let visible = visible_summary(&archive.store, options.collect_event_ids)?;
    let quality = archive.store.replay_quality(state.revision_id)?;
    Ok(PipelineResult {
        registered_files,
        visible_before_promotion: before.count,
        visible_events: visible.count,
        visible_total_tokens: visible.total_tokens,
        visible_event_ids: visible.event_ids,
        quality,
        max_reader_batch: state.max_reader_batch,
        max_event_page: before.max_page.max(visible.max_page),
        restarts: archive.restarts,
    })
}

pub fn probe_current_rebuild_reason(
    root: &Path,
    database: &Path,
    relative_path: &Path,
) -> Result<Option<RebuildReason>, PipelineError> {
    let configured = [ConfiguredCodexRoot::new(root, None, true)];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .map_err(|_| PipelineError::Provider)?;
    let discovery = CodexProvider::new()
        .map_err(|_| PipelineError::Provider)?
        .discover(&request)
        .map_err(|_| PipelineError::Provider)?;
    let store = UsageStore::open(database)?;
    let mut result = None;
    let mut callback_error = None;
    let report = enumerate_profile_sources(
        discovery.sources(),
        || false,
        |descriptor| {
            if descriptor.relative_path() != relative_path {
                return SinkDecision::Continue;
            }
            let source_key = SourceKey::from_bytes(*logical_file_identity(&descriptor).as_bytes());
            let probe = store
                .generation_snapshot(source_key)
                .map_err(PipelineError::from)
                .and_then(|snapshot| {
                    let snapshot = snapshot.ok_or(PipelineError::UnexpectedReaderOutcome)?;
                    let checkpoint = reader_checkpoint(snapshot.checkpoint())?;
                    read_source_batch(&descriptor, Some(&checkpoint), || false)
                        .map_err(PipelineError::from)
                });
            match probe {
                Ok(ReaderOutcome::RebuildRequired(reason)) => {
                    result = Some(reason);
                    SinkDecision::Continue
                }
                Ok(ReaderOutcome::Batch(_) | ReaderOutcome::Unchanged(_)) => SinkDecision::Continue,
                Err(error) => {
                    callback_error = Some(error);
                    SinkDecision::Fail
                }
            }
        },
    )
    .map_err(|_| callback_error.unwrap_or(PipelineError::Enumeration))?;
    if let Some(error) = callback_error {
        return Err(error);
    }
    if report.completion() != EnumerationCompletion::Complete {
        return Err(PipelineError::EnumerationIncomplete);
    }
    Ok(result)
}

fn register_complete_source_set(
    discovery: &tokenmaster_provider::DiscoverySnapshot,
    store: &mut UsageStore,
    cancel_after_files: Option<u64>,
) -> Result<u64, PipelineError> {
    let mut emitted = 0_u64;
    let mut callback_error = None;
    let report = enumerate_profile_sources(
        discovery.sources(),
        || false,
        |descriptor| {
            if cancel_after_files.is_some_and(|limit| emitted >= limit) {
                return SinkDecision::Cancel;
            }
            match register_descriptor(store, &descriptor) {
                Ok(()) => match emitted.checked_add(1) {
                    Some(next) => {
                        emitted = next;
                        SinkDecision::Continue
                    }
                    None => {
                        callback_error = Some(PipelineError::Capacity);
                        SinkDecision::Fail
                    }
                },
                Err(error) => {
                    callback_error = Some(error);
                    SinkDecision::Fail
                }
            }
        },
    )
    .map_err(|_| callback_error.unwrap_or(PipelineError::Enumeration))?;
    if let Some(error) = callback_error {
        return Err(error);
    }
    if report.completion() != EnumerationCompletion::Complete || emitted == 0 {
        return Err(PipelineError::EnumerationIncomplete);
    }
    Ok(emitted)
}

fn register_descriptor(
    store: &mut UsageStore,
    descriptor: &SourceFileDescriptor,
) -> Result<(), PipelineError> {
    let logical = logical_file_identity(descriptor);
    let source_key = SourceKey::from_bytes(*logical.as_bytes());
    if store.generation_snapshot(source_key)?.is_some() {
        return Ok(());
    }
    let batch = expect_batch(read_source_batch(descriptor, None, || false)?)?;
    let initial_checkpoint = empty_stored_checkpoint(batch.checkpoint())?;
    let physical_identity = batch
        .checkpoint()
        .physical_identity()
        .map(|identity| *identity.as_bytes());
    let registration = SourceRegistration::new(SourceRegistrationParts {
        source_key,
        provider_id: descriptor.provider_id().into(),
        profile_id: descriptor.profile_id().as_str().into(),
        source_id: descriptor.source_id().as_str().into(),
        source_kind: store_source_kind(descriptor.source_kind()),
        logical_identity: *logical.as_bytes(),
        physical_identity,
        initial_checkpoint,
    })?;
    store.register_source(&registration)?;
    Ok(())
}

fn rebuild_complete_source_set(
    discovery: &tokenmaster_provider::DiscoverySnapshot,
    archive: &mut ReopenableStore,
    state: &mut PipelineState,
) -> Result<(), PipelineError> {
    let mut callback_error = None;
    let report = enumerate_profile_sources(
        discovery.sources(),
        || false,
        |descriptor| match rebuild_descriptor(archive, state, &descriptor) {
            Ok(()) => SinkDecision::Continue,
            Err(error) => {
                callback_error = Some(error);
                SinkDecision::Fail
            }
        },
    )
    .map_err(|_| callback_error.unwrap_or(PipelineError::Enumeration))?;
    if let Some(error) = callback_error {
        return Err(error);
    }
    if report.completion() != EnumerationCompletion::Complete || state.enumerated_files == 0 {
        return Err(PipelineError::EnumerationIncomplete);
    }
    Ok(())
}

fn rebuild_descriptor(
    archive: &mut ReopenableStore,
    state: &mut PipelineState,
    descriptor: &SourceFileDescriptor,
) -> Result<(), PipelineError> {
    state.enumerated_files = state
        .enumerated_files
        .checked_add(1)
        .ok_or(PipelineError::Capacity)?;
    let source_key = SourceKey::from_bytes(*logical_file_identity(descriptor).as_bytes());
    let first = expect_batch(read_source_batch(descriptor, None, || false)?)?;
    state.epoch = archive.store.prepare_replay_source(
        state.revision_id,
        state.epoch,
        source_key,
        &empty_stored_checkpoint(first.checkpoint())?,
    )?;
    let mut reached_end = first.reached_snapshot_end();
    apply_reader_batch(archive, state, source_key, &first)?;

    while !reached_end {
        if state
            .cancel_reader_after_batches
            .is_some_and(|limit| archive.applied_batches >= limit)
        {
            return Err(PipelineError::Cancelled);
        }
        let snapshot = archive
            .store
            .replay_generation_snapshot(state.revision_id, source_key)?;
        let checkpoint = reader_checkpoint(snapshot.checkpoint())?;
        let batch = expect_batch(read_source_batch(descriptor, Some(&checkpoint), || false)?)?;
        reached_end = batch.reached_snapshot_end();
        apply_reader_batch(archive, state, source_key, &batch)?;
    }

    let snapshot = archive
        .store
        .replay_generation_snapshot(state.revision_id, source_key)?;
    if snapshot.checkpoint().incomplete_tail() {
        return Err(PipelineError::IncompleteTail);
    }
    let incremental = reader_checkpoint(snapshot.checkpoint())?;
    let mut chunk_error = None;
    let report = verify_full_prefix(
        descriptor,
        &incremental,
        |index| match archive
            .store
            .source_chunk(source_key, snapshot.generation(), index)
        {
            Ok(Some(chunk)) => match SourceChunkDigest::from_persisted_parts(
                chunk.index(),
                chunk.covered_len(),
                *chunk.sha256(),
            ) {
                Ok(chunk) => Some(chunk),
                Err(error) => {
                    chunk_error = Some(PipelineError::Reader(error.code()));
                    None
                }
            },
            Ok(None) => None,
            Err(error) => {
                chunk_error = Some(error.into());
                None
            }
        },
        || false,
    )?;
    if let Some(error) = chunk_error {
        return Err(error);
    }
    if !matches!(report, IntegrityReport::Verified { .. }) {
        return Err(PipelineError::Integrity);
    }

    let verified = checkpoint_with_verification(&incremental, VerificationLevel::FullPrefix)?;
    let append = AppendBatch::new(AppendBatchParts {
        source_key,
        expected_generation: snapshot.generation(),
        expected_committed_offset: snapshot.checkpoint().committed_offset(),
        expected_scan_offset: snapshot.checkpoint().scan_offset(),
        events: Box::default(),
        previous_partial_chunk: None,
        chunk_updates: Box::default(),
        next_checkpoint: stored_checkpoint(&verified)?,
        last_seen_scan_id: None,
        diagnostic_count_delta: 0,
    })?;
    state.epoch = archive
        .store
        .apply_replay_append_batch(&ReplayAppendBatch::new(ReplayAppendBatchParts {
            revision_id: state.revision_id,
            expected_epoch: state.epoch,
            append_batch: append,
        }))?;
    archive.record_batch_and_maybe_reopen()?;
    Ok(())
}

fn apply_reader_batch(
    archive: &mut ReopenableStore,
    state: &mut PipelineState,
    source_key: SourceKey,
    batch: &ReadBatch,
) -> Result<(), PipelineError> {
    if batch
        .parser_diagnostics()
        .count(ParserDiagnosticCode::MalformedJson)
        != 0
    {
        return Err(PipelineError::MalformedInput);
    }
    state.max_reader_batch = state.max_reader_batch.max(batch.events().len());
    let snapshot = archive
        .store
        .replay_generation_snapshot(state.revision_id, source_key)?;
    let canonicalizer = Canonicalizer::new();
    let events = batch
        .events()
        .iter()
        .map(|draft| {
            canonicalizer
                .canonicalize(draft)
                .map_err(|_| PipelineError::Canonicalization)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let chunk_updates = batch
        .source_chunks()
        .iter()
        .map(|chunk| {
            StoredSourceChunk::new(chunk.index(), chunk.covered_len(), *chunk.sha256())
                .map_err(PipelineError::from)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let previous_partial_chunk = batch
        .previous_partial_chunk()
        .map(|chunk| StoredSourceChunk::new(chunk.index(), chunk.covered_len(), *chunk.sha256()))
        .transpose()?;
    let append = AppendBatch::new(AppendBatchParts {
        source_key,
        expected_generation: snapshot.generation(),
        expected_committed_offset: snapshot.checkpoint().committed_offset(),
        expected_scan_offset: snapshot.checkpoint().scan_offset(),
        events,
        previous_partial_chunk,
        chunk_updates,
        next_checkpoint: stored_checkpoint(batch.checkpoint())?,
        last_seen_scan_id: None,
        diagnostic_count_delta: 0,
    })?;
    state.epoch = archive
        .store
        .apply_replay_append_batch(&ReplayAppendBatch::new(ReplayAppendBatchParts {
            revision_id: state.revision_id,
            expected_epoch: state.epoch,
            append_batch: append,
        }))?;
    archive.record_batch_and_maybe_reopen()?;

    for relation in batch.relations() {
        let relation = ReplayRelation::new(state.revision_id, state.epoch, source_key, relation)?;
        state.epoch = archive.store.apply_replay_relation(&relation)?;
    }
    Ok(())
}

fn finish_replay(store: &mut UsageStore, state: &mut PipelineState) -> Result<(), PipelineError> {
    for _ in 0..CONTINUATION_LIMIT {
        let continuation = store.continue_replay(state.revision_id, state.epoch)?;
        state.epoch = continuation.epoch();
        if !continuation.remaining_work() {
            return Ok(());
        }
        if continuation.processed_count() == 0 {
            return Err(PipelineError::ContinuationStalled);
        }
    }
    Err(PipelineError::ContinuationLimit)
}

fn empty_stored_checkpoint(
    checkpoint: &ReaderCheckpointV1,
) -> Result<StoredCheckpoint, PipelineError> {
    let resume = serde_json::to_vec(&ParserState::new().snapshot())
        .map_err(|_| PipelineError::ReaderCheckpoint)?
        .into_boxed_slice();
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: PARSER_SCHEMA_VERSION,
        physical_identity: checkpoint
            .physical_identity()
            .map(|identity| *identity.as_bytes()),
        logical_identity: *checkpoint.logical_identity().as_bytes(),
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: 0,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [0; 32],
        resume,
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    })
    .map_err(PipelineError::from)
}

fn stored_checkpoint(checkpoint: &ReaderCheckpointV1) -> Result<StoredCheckpoint, PipelineError> {
    let verification = match checkpoint.verification() {
        VerificationLevel::Incremental => StoredVerification::Incremental,
        VerificationLevel::FullPrefix => StoredVerification::FullPrefix,
    };
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: checkpoint.parser_schema_version(),
        physical_identity: checkpoint
            .physical_identity()
            .map(|identity| *identity.as_bytes()),
        logical_identity: *checkpoint.logical_identity().as_bytes(),
        committed_offset: checkpoint.committed_offset(),
        scan_offset: checkpoint.scan_offset(),
        observed_file_length: checkpoint.observed_file_length(),
        modified_time_ns: checkpoint.modified_time_ns(),
        anchor_start: checkpoint.anchor().start(),
        anchor_len: checkpoint.anchor().len(),
        anchor_sha256: *checkpoint.anchor().sha256(),
        resume: serde_json::to_vec(checkpoint.resume())
            .map_err(|_| PipelineError::ReaderCheckpoint)?
            .into_boxed_slice(),
        discarding_oversized_line: checkpoint.discarding_oversized_line(),
        incomplete_tail: checkpoint.incomplete_tail(),
        verification,
    })
    .map_err(PipelineError::from)
}

fn reader_checkpoint(checkpoint: &StoredCheckpoint) -> Result<ReaderCheckpointV1, PipelineError> {
    let physical_identity = checkpoint
        .physical_identity()
        .copied()
        .map(PhysicalFileIdentity::from_persisted_bytes);
    let resume: ParserResumeState =
        serde_json::from_slice(checkpoint.resume()).map_err(|_| PipelineError::ReaderCheckpoint)?;
    let verification = match checkpoint.verification() {
        StoredVerification::Incremental => VerificationLevel::Incremental,
        StoredVerification::FullPrefix => VerificationLevel::FullPrefix,
    };
    ReaderCheckpointV1::new(ReaderCheckpointParts {
        parser_schema_version: checkpoint.parser_schema_version(),
        physical_identity,
        logical_identity: LogicalFileIdentity::from_bytes(*checkpoint.logical_identity()),
        committed_offset: checkpoint.committed_offset(),
        scan_offset: checkpoint.scan_offset(),
        observed_file_length: checkpoint.observed_file_length(),
        modified_time_ns: checkpoint.modified_time_ns(),
        anchor: BoundaryAnchor::new(
            checkpoint.anchor_start(),
            checkpoint.anchor_len(),
            *checkpoint.anchor_sha256(),
        )
        .map_err(|_| PipelineError::ReaderCheckpoint)?,
        resume,
        discarding_oversized_line: checkpoint.discarding_oversized_line(),
        incomplete_tail: checkpoint.incomplete_tail(),
        verification,
    })
    .map_err(|_| PipelineError::ReaderCheckpoint)
}

fn checkpoint_with_verification(
    checkpoint: &ReaderCheckpointV1,
    verification: VerificationLevel,
) -> Result<ReaderCheckpointV1, PipelineError> {
    ReaderCheckpointV1::new(ReaderCheckpointParts {
        parser_schema_version: checkpoint.parser_schema_version(),
        physical_identity: checkpoint.physical_identity(),
        logical_identity: checkpoint.logical_identity(),
        committed_offset: checkpoint.committed_offset(),
        scan_offset: checkpoint.scan_offset(),
        observed_file_length: checkpoint.observed_file_length(),
        modified_time_ns: checkpoint.modified_time_ns(),
        anchor: checkpoint.anchor(),
        resume: checkpoint.resume().clone(),
        discarding_oversized_line: checkpoint.discarding_oversized_line(),
        incomplete_tail: checkpoint.incomplete_tail(),
        verification,
    })
    .map_err(|_| PipelineError::ReaderCheckpoint)
}

fn expect_batch(outcome: ReaderOutcome) -> Result<ReadBatch, PipelineError> {
    match outcome {
        ReaderOutcome::Batch(batch) => Ok(batch),
        ReaderOutcome::Unchanged(_) | ReaderOutcome::RebuildRequired(_) => {
            Err(PipelineError::UnexpectedReaderOutcome)
        }
    }
}

fn store_source_kind(kind: ProviderSourceKind) -> StoreSourceKind {
    match kind {
        ProviderSourceKind::Active => StoreSourceKind::Active,
        ProviderSourceKind::Direct => StoreSourceKind::Direct,
        ProviderSourceKind::Archived => StoreSourceKind::Archived,
    }
}

struct VisibleSummary {
    count: u64,
    total_tokens: u64,
    event_ids: BTreeSet<String>,
    max_page: usize,
}

fn visible_summary(store: &UsageStore, collect_ids: bool) -> Result<VisibleSummary, PipelineError> {
    let mut count = 0_u64;
    let mut total_tokens = 0_u64;
    let mut event_ids = BTreeSet::new();
    let mut max_page = 0_usize;
    let mut before: Option<EventCursor> = None;
    loop {
        let page = store.event_page_before(before, 256)?;
        if page.is_empty() {
            break;
        }
        max_page = max_page.max(page.len());
        for event in &page {
            count = count.checked_add(1).ok_or(PipelineError::Capacity)?;
            if let Some(total) = event.total_tokens() {
                total_tokens = total_tokens
                    .checked_add(total)
                    .ok_or(PipelineError::Capacity)?;
            }
            if collect_ids {
                if event_ids.len() == 256 {
                    return Err(PipelineError::Capacity);
                }
                event_ids.insert(event.event_id().to_owned());
            }
        }
        before = page.last().map(StoredUsageEvent::cursor);
    }
    Ok(VisibleSummary {
        count,
        total_tokens,
        event_ids,
        max_page,
    })
}
