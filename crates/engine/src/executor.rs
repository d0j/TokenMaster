use tokenmaster_accounting::Canonicalizer;

use crate::{
    Adapter, AdapterCompletion, AdapterCounters, AdapterDiagnostics, Archive, ArchiveReplay,
    ArchiveRevisionId, ArchiveScanSetId, BatchState, CanonicalBatch, CanonicalBatchParts, Clock,
    CompletionQuality, DiscoveredSource, EngineError, EngineErrorCode, MAX_SCOPE_MANIFEST_ENTRIES,
    OperationControl, PortError, PortErrorCode, RefreshOutcome, RefreshPermit, RefreshRequestId,
    ReplayContinuationState, ReplaySourceSink, ScopeIdentity, ScopeManifest, ScopeSink,
    SinkControl, SourceBatchReader, SourceSink, WriterLease,
};

pub const MAX_REPLAY_CONTINUATIONS_PER_RUN: usize = 4_096;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutionCounts {
    observed_sources: u64,
    canonical_events: u64,
    adapter_batches: u64,
    replay_continuations: u64,
}

impl ExecutionCounts {
    pub const fn new(
        observed_sources: u64,
        canonical_events: u64,
        adapter_batches: u64,
        replay_continuations: u64,
    ) -> Result<Self, EngineError> {
        if observed_sources > i64::MAX as u64
            || canonical_events > i64::MAX as u64
            || adapter_batches > i64::MAX as u64
            || replay_continuations > i64::MAX as u64
        {
            return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
        }
        Ok(Self {
            observed_sources,
            canonical_events,
            adapter_batches,
            replay_continuations,
        })
    }

    #[must_use]
    pub const fn observed_sources(self) -> u64 {
        self.observed_sources
    }

    #[must_use]
    pub const fn canonical_events(self) -> u64 {
        self.canonical_events
    }

    #[must_use]
    pub const fn adapter_batches(self) -> u64 {
        self.adapter_batches
    }

    #[must_use]
    pub const fn replay_continuations(self) -> u64 {
        self.replay_continuations
    }

    fn add_observed_source(&mut self) -> Result<(), PortError> {
        self.observed_sources = checked_count_add(self.observed_sources, 1)?;
        Ok(())
    }

    fn add_canonical_events(&mut self, count: usize) -> Result<(), PortError> {
        let count = u64::try_from(count).map_err(|_| capacity_error())?;
        self.canonical_events = checked_count_add(self.canonical_events, count)?;
        Ok(())
    }

    fn add_adapter_batch(&mut self) -> Result<(), PortError> {
        self.adapter_batches = checked_count_add(self.adapter_batches, 1)?;
        Ok(())
    }

    fn add_replay_continuation(&mut self) -> Result<(), PortError> {
        self.replay_continuations = checked_count_add(self.replay_continuations, 1)?;
        Ok(())
    }
}

fn checked_count_add(value: u64, amount: u64) -> Result<u64, PortError> {
    value
        .checked_add(amount)
        .filter(|next| *next <= i64::MAX as u64)
        .ok_or_else(capacity_error)
}

fn capacity_error() -> PortError {
    PortError::new(PortErrorCode::CapacityExceeded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayCleanup {
    NotRequired,
    Discarded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OneShotResult {
    request_id: RefreshRequestId,
    outcome: RefreshOutcome,
    quality: CompletionQuality,
    scan_set_id: Option<ArchiveScanSetId>,
    published_revision_id: Option<ArchiveRevisionId>,
    counts: ExecutionCounts,
    cleanup: ReplayCleanup,
    error: Option<PortErrorCode>,
}

impl OneShotResult {
    #[must_use]
    pub const fn request_id(self) -> RefreshRequestId {
        self.request_id
    }

    #[must_use]
    pub const fn outcome(self) -> RefreshOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn quality(self) -> CompletionQuality {
        self.quality
    }

    #[must_use]
    pub const fn scan_set_id(self) -> Option<ArchiveScanSetId> {
        self.scan_set_id
    }

    #[must_use]
    pub const fn published_revision_id(self) -> Option<ArchiveRevisionId> {
        self.published_revision_id
    }

    #[must_use]
    pub const fn counts(self) -> ExecutionCounts {
        self.counts
    }

    #[must_use]
    pub const fn cleanup(self) -> ReplayCleanup {
        self.cleanup
    }

    #[must_use]
    pub const fn error(self) -> Option<PortErrorCode> {
        self.error
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ExecutionState {
    scan_set_id: Option<ArchiveScanSetId>,
    replay: Option<ArchiveReplay>,
    counts: ExecutionCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionTerminal {
    Published(ArchiveRevisionId),
    ScanOnly(CompletionQuality),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExecutionFailure {
    error: PortError,
    quality: CompletionQuality,
}

impl From<PortError> for ExecutionFailure {
    fn from(error: PortError) -> Self {
        Self {
            quality: quality_for_error(error.code()),
            error,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OneShotExecutor;

impl OneShotExecutor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn run(
        &self,
        permit: &RefreshPermit,
        clock: &dyn Clock,
        lease: &mut dyn WriterLease,
        adapter: &mut dyn Adapter,
        archive: &mut dyn Archive,
    ) -> OneShotResult {
        let control = OperationControl::new(permit, clock);
        if let Err(error) = control.check() {
            return failure_result(
                permit.id(),
                ExecutionState::default(),
                error.into(),
                ReplayCleanup::NotRequired,
            );
        }
        let guard = match lease.try_acquire() {
            Ok(guard) => guard,
            Err(error) => {
                return lease_failure_result(
                    permit.id(),
                    ExecutionState::default(),
                    error,
                    ReplayCleanup::NotRequired,
                );
            }
        };
        let mut state = ExecutionState::default();
        let execution = self.run_with_lease(&control, adapter, archive, &mut state);
        let result = match execution {
            Ok(ExecutionTerminal::Published(published_revision_id)) => OneShotResult {
                request_id: permit.id(),
                outcome: RefreshOutcome::Completed,
                quality: CompletionQuality::Complete,
                scan_set_id: state.scan_set_id,
                published_revision_id: Some(published_revision_id),
                counts: state.counts,
                cleanup: ReplayCleanup::NotRequired,
                error: None,
            },
            Ok(ExecutionTerminal::ScanOnly(quality)) => OneShotResult {
                request_id: permit.id(),
                outcome: outcome_for_quality(quality),
                quality,
                scan_set_id: state.scan_set_id,
                published_revision_id: None,
                counts: state.counts,
                cleanup: ReplayCleanup::NotRequired,
                error: None,
            },
            Err(failure) => {
                let cleanup =
                    state.replay.map_or(ReplayCleanup::NotRequired, |replay| {
                        match archive.discard_replay(replay) {
                            Ok(()) => ReplayCleanup::Discarded,
                            Err(_) => ReplayCleanup::Failed,
                        }
                    });
                failure_result(permit.id(), state, failure, cleanup)
            }
        };
        drop(guard);
        result
    }

    fn run_with_lease(
        &self,
        control: &OperationControl<'_>,
        adapter: &mut dyn Adapter,
        archive: &mut dyn Archive,
        state: &mut ExecutionState,
    ) -> Result<ExecutionTerminal, ExecutionFailure> {
        let mut scope_sink = BoundedScopeSink::default();
        let scope_completion = adapter.visit_scopes(control, &mut scope_sink)?;
        if scope_completion.quality() != CompletionQuality::Complete {
            return Ok(ExecutionTerminal::ScanOnly(scope_completion.quality()));
        }
        let manifest =
            ScopeManifest::new(scope_sink.scopes.into_boxed_slice()).map_err(PortError::from)?;

        control.check()?;
        let scan_set_id = archive.begin_scan_set(&manifest)?;
        state.scan_set_id = Some(scan_set_id);

        for (scope_index, scope) in manifest.scopes().iter().enumerate() {
            if let Err(error) = control.check() {
                close_failed_scan(
                    archive,
                    scan_set_id,
                    &manifest.scopes()[scope_index..],
                    quality_for_error(error.code()),
                );
                return Err(error.into());
            }
            let completion = {
                let mut source_sink = ArchiveSourceSink {
                    archive,
                    scan_set_id,
                    expected_scope: scope,
                    counts: &mut state.counts,
                };
                adapter.visit_sources(scope, control, &mut source_sink)
            };
            let completion = match completion {
                Ok(completion) => completion,
                Err(error) => {
                    close_failed_scan(
                        archive,
                        scan_set_id,
                        &manifest.scopes()[scope_index..],
                        quality_for_error(error.code()),
                    );
                    return Err(error.into());
                }
            };
            if let Err(error) = archive.finish_scope(scan_set_id, scope, completion) {
                close_failed_scan(
                    archive,
                    scan_set_id,
                    &manifest.scopes()[scope_index + 1..],
                    CompletionQuality::Failed,
                );
                return Err(error.into());
            }
            if completion.quality() != CompletionQuality::Complete {
                close_scopes(
                    archive,
                    scan_set_id,
                    &manifest.scopes()[scope_index + 1..],
                    completion.quality(),
                )?;
                let aggregate = archive.finish_scan_set(scan_set_id)?;
                if aggregate == CompletionQuality::Complete {
                    return Err(PortError::new(PortErrorCode::InvalidData).into());
                }
                return Ok(ExecutionTerminal::ScanOnly(completion.quality()));
            }
        }

        if let Err(error) = control.check() {
            close_failed_scan(archive, scan_set_id, &[], quality_for_error(error.code()));
            return Err(error.into());
        }
        let scan_quality = archive.finish_scan_set(scan_set_id)?;
        if scan_quality != CompletionQuality::Complete {
            return Ok(ExecutionTerminal::ScanOnly(scan_quality));
        }

        control.check()?;
        let replay = archive.begin_replay(scan_set_id)?;
        state.replay = Some(replay);
        self.replay_sources(control, adapter, archive, &manifest, state)?;
        self.finish_replay(control, archive, state)
            .map(ExecutionTerminal::Published)
            .map_err(ExecutionFailure::from)
    }

    fn replay_sources(
        &self,
        control: &OperationControl<'_>,
        adapter: &mut dyn Adapter,
        archive: &mut dyn Archive,
        manifest: &ScopeManifest,
        state: &mut ExecutionState,
    ) -> Result<(), ExecutionFailure> {
        for scope in manifest.scopes() {
            control.check()?;
            let completion = {
                let mut sink = ArchiveReplaySourceSink {
                    executor: self,
                    control,
                    archive,
                    expected_scope: scope,
                    state,
                };
                adapter.visit_replay_sources(scope, control, &mut sink)?
            };
            if completion.quality() != CompletionQuality::Complete {
                return Err(ExecutionFailure {
                    error: error_for_quality(completion.quality()),
                    quality: completion.quality(),
                });
            }
        }
        Ok(())
    }

    fn replay_source(
        &self,
        control: &OperationControl<'_>,
        archive: &mut dyn Archive,
        source: &DiscoveredSource,
        initial_state: crate::AdapterSourceState,
        reader: &mut dyn SourceBatchReader,
        state: &mut ExecutionState,
    ) -> Result<(), PortError> {
        control.check()?;
        let current = state.replay.ok_or_else(stale_error)?;
        let replay = validate_replay_transition(
            current,
            archive.prepare_replay_source(current, source, &initial_state)?,
        )?;
        state.replay = Some(replay);
        let mut checkpoint = initial_state.checkpoint().clone();

        loop {
            control.check()?;
            let batch = reader.read_batch(&checkpoint, control)?;
            if batch.source_identity() != source.identity() {
                return Err(PortError::new(PortErrorCode::InvalidData));
            }
            let batch_state = batch.state();
            let next_checkpoint = batch.next_checkpoint().clone();
            if batch_state == BatchState::More && next_checkpoint == checkpoint {
                return Err(PortError::new(PortErrorCode::InvalidData));
            }
            let canonical = canonicalize_batch(source.identity(), batch)?;
            let event_count = canonical.events().len();
            let current = state.replay.ok_or_else(stale_error)?;
            let replay = validate_replay_transition(
                current,
                archive.append_replay_batch(current, source.identity(), canonical)?,
            )?;
            state.replay = Some(replay);
            state.counts.add_adapter_batch()?;
            state.counts.add_canonical_events(event_count)?;
            if batch_state == BatchState::SnapshotEnd {
                return Ok(());
            }
            checkpoint = next_checkpoint;
        }
    }

    fn finish_replay(
        &self,
        control: &OperationControl<'_>,
        archive: &mut dyn Archive,
        state: &mut ExecutionState,
    ) -> Result<ArchiveRevisionId, PortError> {
        loop {
            control.check()?;
            if state.counts.replay_continuations() >= MAX_REPLAY_CONTINUATIONS_PER_RUN as u64 {
                return Err(PortError::new(PortErrorCode::CapacityExceeded));
            }
            let current = state.replay.ok_or_else(stale_error)?;
            let continuation = archive.continue_replay(current)?;
            let replay = validate_replay_transition(current, continuation.replay())?;
            state.replay = Some(replay);
            state.counts.add_replay_continuation()?;
            if continuation.state() == ReplayContinuationState::Complete {
                break;
            }
        }

        control.check()?;
        let current = state.replay.ok_or_else(stale_error)?;
        let replay = validate_replay_transition(current, archive.seal_replay(current)?)?;
        state.replay = Some(replay);
        control.check()?;
        archive.promote_replay(replay)?;
        Ok(replay.revision_id())
    }
}

#[derive(Default)]
struct BoundedScopeSink {
    scopes: Vec<ScopeIdentity>,
}

impl ScopeSink for BoundedScopeSink {
    fn on_scope(&mut self, scope: ScopeIdentity) -> Result<SinkControl, PortError> {
        if self.scopes.len() >= MAX_SCOPE_MANIFEST_ENTRIES {
            return Err(capacity_error());
        }
        self.scopes.push(scope);
        Ok(SinkControl::Continue)
    }
}

struct ArchiveSourceSink<'a> {
    archive: &'a mut dyn Archive,
    scan_set_id: ArchiveScanSetId,
    expected_scope: &'a ScopeIdentity,
    counts: &'a mut ExecutionCounts,
}

impl SourceSink for ArchiveSourceSink<'_> {
    fn on_source(
        &mut self,
        source: crate::DiscoveredSource,
        initial_state: crate::AdapterSourceState,
    ) -> Result<SinkControl, PortError> {
        if source.identity().scope() != self.expected_scope {
            return Err(PortError::new(PortErrorCode::InvalidData));
        }
        self.archive
            .observe_source(self.scan_set_id, &source, &initial_state)?;
        self.counts.add_observed_source()?;
        Ok(SinkControl::Continue)
    }
}

struct ArchiveReplaySourceSink<'a> {
    executor: &'a OneShotExecutor,
    control: &'a OperationControl<'a>,
    archive: &'a mut dyn Archive,
    expected_scope: &'a ScopeIdentity,
    state: &'a mut ExecutionState,
}

impl ReplaySourceSink for ArchiveReplaySourceSink<'_> {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_state: crate::AdapterSourceState,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        if source.identity().scope() != self.expected_scope {
            return Err(PortError::new(PortErrorCode::InvalidData));
        }
        self.executor.replay_source(
            self.control,
            self.archive,
            &source,
            initial_state,
            reader,
            self.state,
        )?;
        Ok(SinkControl::Continue)
    }
}

pub fn canonicalize_batch(
    source: &crate::SourceIdentity,
    batch: crate::AdapterBatch,
) -> Result<CanonicalBatch, PortError> {
    let parts = batch.into_parts();
    let events = parts
        .observations
        .iter()
        .map(|observation| {
            Canonicalizer::new()
                .canonicalize(observation)
                .map_err(|_| PortError::new(PortErrorCode::InvalidData))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    CanonicalBatch::new(
        source,
        CanonicalBatchParts {
            events,
            relations: parts.relations,
            chunk_proofs: parts.chunk_proofs,
            next_checkpoint: parts.next_checkpoint,
            next_progress: parts.next_progress,
            state: parts.state,
            counters: parts.counters,
            diagnostics: parts.diagnostics,
        },
    )
    .map_err(PortError::from)
}

fn close_scopes(
    archive: &mut dyn Archive,
    scan_set_id: ArchiveScanSetId,
    scopes: &[ScopeIdentity],
    quality: CompletionQuality,
) -> Result<(), PortError> {
    for scope in scopes {
        archive.finish_scope(scan_set_id, scope, completion_for_quality(quality)?)?;
    }
    Ok(())
}

fn close_failed_scan(
    archive: &mut dyn Archive,
    scan_set_id: ArchiveScanSetId,
    scopes: &[ScopeIdentity],
    quality: CompletionQuality,
) {
    let _ = close_scopes(archive, scan_set_id, scopes, quality);
    let _ = archive.finish_scan_set(scan_set_id);
}

fn completion_for_quality(quality: CompletionQuality) -> Result<AdapterCompletion, PortError> {
    AdapterCompletion::new(
        quality,
        AdapterCounters::default(),
        AdapterDiagnostics::default(),
    )
    .map_err(PortError::from)
}

fn failure_result(
    request_id: RefreshRequestId,
    state: ExecutionState,
    failure: ExecutionFailure,
    cleanup: ReplayCleanup,
) -> OneShotResult {
    let outcome = match failure.error.code() {
        PortErrorCode::Cancelled => RefreshOutcome::Cancelled,
        PortErrorCode::DeadlineExceeded => RefreshOutcome::DeadlineExceeded,
        PortErrorCode::Busy
        | PortErrorCode::InvalidData
        | PortErrorCode::CapacityExceeded
        | PortErrorCode::StaleState
        | PortErrorCode::RebuildRequired
        | PortErrorCode::Unavailable
        | PortErrorCode::Failed => RefreshOutcome::Failed,
    };
    OneShotResult {
        request_id,
        outcome,
        quality: failure.quality,
        scan_set_id: state.scan_set_id,
        published_revision_id: None,
        counts: state.counts,
        cleanup,
        error: Some(failure.error.code()),
    }
}

fn lease_failure_result(
    request_id: RefreshRequestId,
    state: ExecutionState,
    error: PortError,
    cleanup: ReplayCleanup,
) -> OneShotResult {
    let is_busy = error.code() == PortErrorCode::Busy;
    let mut result = failure_result(request_id, state, error.into(), cleanup);
    if is_busy {
        result.outcome = RefreshOutcome::Busy;
    }
    result
}

fn outcome_for_quality(quality: CompletionQuality) -> RefreshOutcome {
    match quality {
        CompletionQuality::Complete | CompletionQuality::Partial => RefreshOutcome::Completed,
        CompletionQuality::Cancelled => RefreshOutcome::Cancelled,
        CompletionQuality::Failed => RefreshOutcome::Failed,
        CompletionQuality::TimedOut => RefreshOutcome::DeadlineExceeded,
    }
}

fn quality_for_error(code: PortErrorCode) -> CompletionQuality {
    match code {
        PortErrorCode::Cancelled => CompletionQuality::Cancelled,
        PortErrorCode::DeadlineExceeded => CompletionQuality::TimedOut,
        PortErrorCode::Busy
        | PortErrorCode::InvalidData
        | PortErrorCode::CapacityExceeded
        | PortErrorCode::StaleState
        | PortErrorCode::RebuildRequired
        | PortErrorCode::Unavailable
        | PortErrorCode::Failed => CompletionQuality::Failed,
    }
}

fn error_for_quality(quality: CompletionQuality) -> PortError {
    PortError::new(match quality {
        CompletionQuality::Complete => PortErrorCode::InvalidData,
        CompletionQuality::Partial => PortErrorCode::Unavailable,
        CompletionQuality::Cancelled => PortErrorCode::Cancelled,
        CompletionQuality::Failed => PortErrorCode::Failed,
        CompletionQuality::TimedOut => PortErrorCode::DeadlineExceeded,
    })
}

fn stale_error() -> PortError {
    PortError::new(PortErrorCode::StaleState)
}

fn validate_replay_transition(
    current: ArchiveReplay,
    next: ArchiveReplay,
) -> Result<ArchiveReplay, PortError> {
    if next.revision_id() != current.revision_id() || next.epoch() < current.epoch() {
        return Err(PortError::new(PortErrorCode::InvalidData));
    }
    Ok(next)
}
