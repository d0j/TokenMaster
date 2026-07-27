use tokenmaster_engine::{
    Adapter, AdapterCompletion, AdapterCounters, AdapterDiagnostics, AdapterSourceProgress,
    AdapterSourceProgressParts, Archive, BatchState, CompletionQuality, DiscoveredSource,
    MAX_REPLAY_CONTINUATIONS_PER_RUN, OperationControl, PortError, PortErrorCode, ReplaySourceSink,
    ScopeIdentity, ScopeManifest, ScopeSink, SinkControl, SourceBatchReader, SourceSink,
    canonicalize_batch,
};
use tokenmaster_store::{
    ArchivePublicationQuality, MAX_APPEND_EVENTS, MAX_APPEND_RELATIONS, MAX_SCAN_SCOPES,
};

use crate::StoreArchive;
use crate::store_archive::{CurrentCursor, PreparedCurrentBatch};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncrementalRefreshOutcome {
    Complete,
    Partial,
    RebuildRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncrementalRefreshReport {
    outcome: IncrementalRefreshOutcome,
    files_examined: u64,
    bytes_read: u64,
    events_observed: u64,
    batches_committed: u64,
    diagnostics: u64,
    starting_current_generation: u64,
    current_generation: u64,
    archive_generation: u64,
}

impl IncrementalRefreshReport {
    #[must_use]
    pub const fn outcome(self) -> IncrementalRefreshOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn files_examined(self) -> u64 {
        self.files_examined
    }

    #[must_use]
    pub const fn bytes_read(self) -> u64 {
        self.bytes_read
    }

    #[must_use]
    pub const fn events_observed(self) -> u64 {
        self.events_observed
    }

    #[must_use]
    pub const fn batches_committed(self) -> u64 {
        self.batches_committed
    }

    #[must_use]
    pub const fn diagnostics(self) -> u64 {
        self.diagnostics
    }

    #[must_use]
    pub const fn archive_generation(self) -> u64 {
        self.archive_generation
    }

    #[must_use]
    pub const fn made_progress(self) -> bool {
        self.current_generation > self.starting_current_generation
    }
}

#[derive(Default)]
struct RefreshCounts {
    files_examined: u64,
    bytes_read: u64,
    events_observed: u64,
    batches_committed: u64,
    diagnostics: u64,
}

#[cfg(test)]
mod tests {
    use super::{IncrementalRefreshOutcome, IncrementalRefreshReport};

    #[test]
    fn only_durable_archive_generation_advance_counts_as_continuation_progress() {
        let stalled = IncrementalRefreshReport {
            outcome: IncrementalRefreshOutcome::Partial,
            files_examined: 0,
            bytes_read: 0,
            events_observed: 0,
            batches_committed: 0,
            diagnostics: 1,
            starting_current_generation: 7,
            current_generation: 7,
            archive_generation: 7,
        };
        assert!(!stalled.made_progress());
        assert!(
            !IncrementalRefreshReport {
                files_examined: 1,
                bytes_read: 1,
                events_observed: 1,
                ..stalled
            }
            .made_progress()
        );
        assert!(
            IncrementalRefreshReport {
                current_generation: 8,
                ..stalled
            }
            .made_progress()
        );
    }
}

pub fn refresh_incremental(
    adapter: &mut dyn Adapter,
    archive: &mut StoreArchive,
    control: &OperationControl<'_>,
) -> Result<IncrementalRefreshReport, PortError> {
    control.check()?;
    let starting_current_generation = archive.current_cursor()?.archive_generation.get();
    let mut scope_sink = ScopeCollector::default();
    let scope_completion = adapter.visit_scopes(control, &mut scope_sink)?;
    if scope_completion.quality() != CompletionQuality::Complete || scope_sink.scopes.is_empty() {
        return report(
            archive,
            starting_current_generation,
            IncrementalRefreshOutcome::Partial,
            RefreshCounts::default(),
        );
    }
    let scopes = scope_sink.scopes;
    let publication = archive
        .store()
        .archive_publication()
        .map_err(|_| PortError::new(PortErrorCode::Unavailable))?;
    let mut counts = RefreshCounts::default();
    let mut cursor = archive.current_cursor()?;

    if publication.quality() == ArchivePublicationQuality::RecoveryPending {
        return report(
            archive,
            starting_current_generation,
            IncrementalRefreshOutcome::RebuildRequired,
            counts,
        );
    }
    if publication.quality() == ArchivePublicationQuality::Partial {
        let settled = settle_current(archive, &mut cursor, control)?;
        if !settled {
            let outcome =
                match run_tail_passes(adapter, archive, control, &scopes, &mut cursor, &mut counts)
                {
                    Ok(outcome) => outcome,
                    Err(error) if requires_rebuild(error.code()) => {
                        archive.mark_rebuild_required(cursor)?;
                        IncrementalRefreshOutcome::RebuildRequired
                    }
                    Err(error) => return Err(error),
                };
            return report(archive, starting_current_generation, outcome, counts);
        }
        return report(
            archive,
            starting_current_generation,
            IncrementalRefreshOutcome::Complete,
            counts,
        );
    }
    if publication.quality() != ArchivePublicationQuality::Complete {
        return Err(PortError::new(PortErrorCode::StaleState));
    }

    let manifest =
        ScopeManifest::new(scopes.clone().into_boxed_slice()).map_err(PortError::from)?;
    let scan_set = archive.begin_incremental_scan_set(&manifest)?;
    let mut scan_quality = CompletionQuality::Complete;
    for (index, scope) in scopes.iter().enumerate() {
        if let Err(error) = control.check() {
            close_scan(
                archive,
                scan_set,
                &scopes[index..],
                quality_for_error(error.code()),
            );
            return Err(error);
        }
        let completion = {
            let mut sink = ScanSink { archive, scan_set };
            match adapter.visit_sources(scope, control, &mut sink) {
                Ok(completion) => completion,
                Err(error) if error.code() == PortErrorCode::RebuildRequired => {
                    close_scan(
                        archive,
                        scan_set,
                        &scopes[index..],
                        quality_for_error(error.code()),
                    );
                    archive.mark_rebuild_required(cursor)?;
                    return report(
                        archive,
                        starting_current_generation,
                        IncrementalRefreshOutcome::RebuildRequired,
                        counts,
                    );
                }
                Err(error) => {
                    close_scan(
                        archive,
                        scan_set,
                        &scopes[index..],
                        quality_for_error(error.code()),
                    );
                    return Err(error);
                }
            }
        };
        if let Err(error) = archive.finish_scope(scan_set, scope, completion) {
            close_scan(
                archive,
                scan_set,
                &scopes[index..],
                quality_for_error(error.code()),
            );
            return Err(error);
        }
        if completion.quality() != CompletionQuality::Complete {
            scan_quality = CompletionQuality::Partial;
        }
    }
    let finished_quality = archive.finish_scan_set(scan_set)?;
    if scan_quality != CompletionQuality::Complete
        || finished_quality != CompletionQuality::Complete
    {
        return report(
            archive,
            starting_current_generation,
            IncrementalRefreshOutcome::Partial,
            counts,
        );
    }
    cursor = match archive.publish_current_scan(scan_set) {
        Ok(cursor) => cursor,
        Err(error) if error.code() == PortErrorCode::RebuildRequired => {
            archive.mark_rebuild_required(cursor)?;
            return report(
                archive,
                starting_current_generation,
                IncrementalRefreshOutcome::RebuildRequired,
                counts,
            );
        }
        Err(error) => return Err(error),
    };
    let outcome = run_tail_passes(adapter, archive, control, &scopes, &mut cursor, &mut counts)?;
    report(archive, starting_current_generation, outcome, counts)
}

fn run_tail_passes(
    adapter: &mut dyn Adapter,
    archive: &mut StoreArchive,
    control: &OperationControl<'_>,
    scopes: &[ScopeIdentity],
    cursor: &mut CurrentCursor,
    counts: &mut RefreshCounts,
) -> Result<IncrementalRefreshOutcome, PortError> {
    for scope in scopes {
        let mut sink = PreflightSink {
            archive,
            control,
            files_examined: 0,
        };
        let completion = match adapter.visit_replay_sources(scope, control, &mut sink) {
            Ok(completion) => completion,
            Err(error) if error.code() == PortErrorCode::RebuildRequired => {
                *cursor = archive.mark_rebuild_required(*cursor)?;
                return Ok(IncrementalRefreshOutcome::RebuildRequired);
            }
            Err(error) => return Err(error),
        };
        counts.files_examined = checked_add(counts.files_examined, sink.files_examined)?;
        if completion.quality() != CompletionQuality::Complete {
            return Ok(IncrementalRefreshOutcome::Partial);
        }
    }

    for scope in scopes {
        let mut sink = ApplySink {
            archive,
            control,
            cursor: *cursor,
            counts,
            pending: Vec::new(),
            pending_events: 0,
            pending_relations: 0,
        };
        let completion = match adapter.visit_replay_sources(scope, control, &mut sink) {
            Ok(completion) => completion,
            Err(error) if error.code() == PortErrorCode::RebuildRequired => {
                *cursor = archive.mark_rebuild_required(*cursor)?;
                return Ok(IncrementalRefreshOutcome::RebuildRequired);
            }
            Err(error) => return Err(error),
        };
        sink.flush_pending()?;
        *cursor = sink.cursor;
        if completion.quality() != CompletionQuality::Complete {
            return Ok(IncrementalRefreshOutcome::Partial);
        }
    }
    if settle_current(archive, cursor, control)? {
        Ok(IncrementalRefreshOutcome::Complete)
    } else {
        Ok(IncrementalRefreshOutcome::Partial)
    }
}

fn settle_current(
    archive: &mut StoreArchive,
    cursor: &mut CurrentCursor,
    control: &OperationControl<'_>,
) -> Result<bool, PortError> {
    for _ in 0..MAX_REPLAY_CONTINUATIONS_PER_RUN {
        control.check()?;
        let quality = archive
            .store()
            .archive_publication()
            .map_err(|_| PortError::new(PortErrorCode::Unavailable))?
            .quality();
        if quality == ArchivePublicationQuality::Complete {
            return Ok(true);
        }
        if quality != ArchivePublicationQuality::Partial {
            return Err(PortError::new(PortErrorCode::StaleState));
        }
        let (next, remaining, _processed, complete) = archive.continue_current(*cursor)?;
        *cursor = next;
        if complete {
            return Ok(true);
        }
        if !remaining {
            return Ok(false);
        }
    }
    Err(PortError::new(PortErrorCode::CapacityExceeded))
}

#[derive(Default)]
struct ScopeCollector {
    scopes: Vec<ScopeIdentity>,
}

impl ScopeSink for ScopeCollector {
    fn on_scope(&mut self, scope: ScopeIdentity) -> Result<SinkControl, PortError> {
        if self.scopes.len() == MAX_SCAN_SCOPES {
            return Err(PortError::new(PortErrorCode::CapacityExceeded));
        }
        self.scopes.push(scope);
        Ok(SinkControl::Continue)
    }
}

struct ScanSink<'a> {
    archive: &'a mut StoreArchive,
    scan_set: tokenmaster_engine::ArchiveScanSetId,
}

impl SourceSink for ScanSink<'_> {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        state: tokenmaster_engine::AdapterSourceState,
    ) -> Result<SinkControl, PortError> {
        self.archive
            .observe_source(self.scan_set, &source, &state)?;
        Ok(SinkControl::Continue)
    }
}

struct PreflightSink<'a> {
    archive: &'a StoreArchive,
    control: &'a OperationControl<'a>,
    files_examined: u64,
}

impl ReplaySourceSink for PreflightSink<'_> {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_state: tokenmaster_engine::AdapterSourceState,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        validate_checkpoint_pair(reader, &initial_state, self.control)?;
        let progress = self.archive.current_progress(source.identity())?;
        validate_current_checkpoint(reader, &initial_state, &progress, self.control)?;
        self.files_examined = checked_add(self.files_examined, 1)?;
        Ok(SinkControl::Continue)
    }
}

struct ApplySink<'a> {
    archive: &'a mut StoreArchive,
    control: &'a OperationControl<'a>,
    cursor: CurrentCursor,
    counts: &'a mut RefreshCounts,
    pending: Vec<PreparedCurrentBatch>,
    pending_events: usize,
    pending_relations: usize,
}

impl ApplySink<'_> {
    fn flush_pending(&mut self) -> Result<(), PortError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let (cursor, remaining_work) = self
            .archive
            .apply_prepared_current_batches(self.cursor, &self.pending)?;
        self.cursor = cursor;
        self.pending.clear();
        self.pending_events = 0;
        self.pending_relations = 0;
        if remaining_work {
            let _ = settle_current(self.archive, &mut self.cursor, self.control)?;
        }
        Ok(())
    }

    fn queue_final_batch(&mut self, batch: PreparedCurrentBatch) -> Result<(), PortError> {
        let next_events = self
            .pending_events
            .checked_add(batch.event_count())
            .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
        let next_relations = self
            .pending_relations
            .checked_add(batch.relation_count())
            .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
        if self.pending.len() == MAX_APPEND_EVENTS
            || next_events > MAX_APPEND_EVENTS
            || next_relations > MAX_APPEND_RELATIONS
        {
            self.flush_pending()?;
        }
        self.pending_events = self
            .pending_events
            .checked_add(batch.event_count())
            .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
        self.pending_relations = self
            .pending_relations
            .checked_add(batch.relation_count())
            .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
        self.pending.push(batch);
        Ok(())
    }
}

impl ReplaySourceSink for ApplySink<'_> {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_state: tokenmaster_engine::AdapterSourceState,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        validate_checkpoint_pair(reader, &initial_state, self.control)?;
        loop {
            self.control.check()?;
            let progress = self.archive.current_progress(source.identity())?;
            let (checkpoint, fresh_start_repair) = match reader
                .restore_checkpoint(&progress, self.control)
            {
                Ok(checkpoint) => (checkpoint, None),
                Err(error) if error.code() == PortErrorCode::InvalidData => {
                    if is_uninitialized_replay_start(&progress) {
                        reader.validate_checkpoint(initial_state.checkpoint(), self.control)?;
                        let repair =
                            reset_resume_progress(reader, &initial_state, &progress, self.control)
                                .ok();
                        (initial_state.checkpoint().clone(), repair)
                    } else {
                        let repaired =
                            reset_resume_progress(reader, &initial_state, &progress, self.control)?;
                        self.flush_pending()?;
                        let (next_cursor, remaining_work) =
                            self.archive.repair_current_source_resume(
                                self.cursor,
                                source.identity(),
                                &repaired,
                            )?;
                        self.cursor = next_cursor;
                        if remaining_work {
                            let _ = settle_current(self.archive, &mut self.cursor, self.control)?;
                        }
                        break;
                    }
                }
                Err(error) => return Err(error),
            };
            let batch = reader.read_batch(&checkpoint, self.control)?;
            let restored_next = reader.restore_checkpoint(batch.next_progress(), self.control)?;
            if restored_next != *batch.next_checkpoint() {
                return Err(PortError::new(PortErrorCode::InvalidData));
            }
            let state = batch.state();
            let counters = batch.counters();
            let unchanged = state == BatchState::SnapshotEnd
                && batch.observations().is_empty()
                && batch.relations().is_empty()
                && batch.chunk_proofs().previous_partial().is_none()
                && batch.chunk_proofs().updates().is_empty()
                && batch.next_checkpoint() == &checkpoint
                && counters.bytes_read() == 0
                && counters.events_observed() == 0
                && counters.diagnostics() == 0;
            if unchanged {
                self.flush_pending()?;
                let (next_cursor, remaining_work) = if let Some(repaired) = fresh_start_repair {
                    self.archive.repair_current_source_resume(
                        self.cursor,
                        source.identity(),
                        &repaired,
                    )?
                } else {
                    self.archive
                        .complete_current_source(self.cursor, source.identity())?
                };
                self.cursor = next_cursor;
                if remaining_work {
                    let _ = settle_current(self.archive, &mut self.cursor, self.control)?;
                }
                break;
            }
            self.counts.bytes_read = checked_add(self.counts.bytes_read, counters.bytes_read())?;
            self.counts.events_observed =
                checked_add(self.counts.events_observed, counters.events_observed())?;
            self.counts.diagnostics = checked_add(self.counts.diagnostics, counters.diagnostics())?;
            let canonical = canonicalize_batch(source.identity(), batch)?;
            self.counts.batches_committed = checked_add(self.counts.batches_committed, 1)?;
            if state == BatchState::SnapshotEnd {
                let prepared = self.archive.prepare_current_batch(
                    self.cursor,
                    source.identity(),
                    canonical,
                )?;
                if !prepared.source_caught_up() {
                    return Ok(SinkControl::Stop);
                }
                self.queue_final_batch(prepared)?;
                break;
            }
            self.flush_pending()?;
            let (next_cursor, remaining_work, source_caught_up) = self
                .archive
                .append_current_batch(self.cursor, source.identity(), canonical)?;
            self.cursor = next_cursor;
            if remaining_work {
                let _ = settle_current(self.archive, &mut self.cursor, self.control)?;
            }
            if source_caught_up {
                return Err(PortError::new(PortErrorCode::InvalidData));
            }
        }
        Ok(SinkControl::Continue)
    }
}

fn validate_checkpoint_pair(
    reader: &mut dyn SourceBatchReader,
    state: &tokenmaster_engine::AdapterSourceState,
    control: &OperationControl<'_>,
) -> Result<(), PortError> {
    let restored = reader.restore_checkpoint(state.progress(), control)?;
    if restored != *state.checkpoint() {
        return Err(PortError::new(PortErrorCode::InvalidData));
    }
    Ok(())
}

fn validate_current_checkpoint(
    reader: &mut dyn SourceBatchReader,
    initial_state: &tokenmaster_engine::AdapterSourceState,
    progress: &AdapterSourceProgress,
    control: &OperationControl<'_>,
) -> Result<(), PortError> {
    match reader.restore_checkpoint(progress, control) {
        Ok(checkpoint) => reader.validate_checkpoint(&checkpoint, control),
        Err(error) if error.code() == PortErrorCode::InvalidData => {
            if is_uninitialized_replay_start(progress) {
                return Ok(());
            }
            let _ = reset_resume_progress(reader, initial_state, progress, control)?;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn is_uninitialized_replay_start(progress: &AdapterSourceProgress) -> bool {
    progress.committed_offset() == 0
        && progress.scan_offset() == 0
        && progress.observed_extent() == 0
        && progress.anchor_start() == 0
        && progress.anchor_len() == 0
        && progress.anchor_sha256() == &[0; 32]
        && progress.provider_resume().is_empty()
        && !progress.discarding_oversized_record()
        && !progress.incomplete_tail()
}

fn reset_resume_progress(
    reader: &mut dyn SourceBatchReader,
    initial_state: &tokenmaster_engine::AdapterSourceState,
    progress: &AdapterSourceProgress,
    control: &OperationControl<'_>,
) -> Result<AdapterSourceProgress, PortError> {
    let initial = initial_state.progress();
    if progress.schema_version() != initial.schema_version()
        || progress.physical_identity() != initial.physical_identity()
        || progress.logical_identity() != initial.logical_identity()
        || progress.observed_extent() != initial.observed_extent()
        || progress.modified_time_ns() != initial.modified_time_ns()
        || !progress_is_caught_up(progress)
    {
        return Err(PortError::new(PortErrorCode::InvalidData));
    }
    let repaired = AdapterSourceProgress::new(AdapterSourceProgressParts {
        schema_version: progress.schema_version(),
        physical_identity: progress.physical_identity().copied(),
        logical_identity: *progress.logical_identity(),
        committed_offset: progress.committed_offset(),
        scan_offset: progress.scan_offset(),
        observed_extent: progress.observed_extent(),
        modified_time_ns: progress.modified_time_ns(),
        anchor_start: progress.anchor_start(),
        anchor_len: progress.anchor_len(),
        anchor_sha256: *progress.anchor_sha256(),
        provider_resume: initial.provider_resume().to_vec().into_boxed_slice(),
        discarding_oversized_record: progress.discarding_oversized_record(),
        incomplete_tail: progress.incomplete_tail(),
        verification: progress.verification(),
    })
    .map_err(PortError::from)?;
    let checkpoint = reader.restore_checkpoint(&repaired, control)?;
    reader.validate_checkpoint(&checkpoint, control)?;
    Ok(repaired)
}

fn progress_is_caught_up(progress: &AdapterSourceProgress) -> bool {
    !progress.discarding_oversized_record()
        && !progress.incomplete_tail()
        && progress.committed_offset() == progress.scan_offset()
        && progress.scan_offset() == progress.observed_extent()
}

fn close_scan(
    archive: &mut StoreArchive,
    scan_set: tokenmaster_engine::ArchiveScanSetId,
    scopes: &[ScopeIdentity],
    quality: CompletionQuality,
) {
    if let Ok(completion) = AdapterCompletion::new(
        quality,
        AdapterCounters::default(),
        AdapterDiagnostics::default(),
    ) {
        for scope in scopes {
            let _ = archive.finish_scope(scan_set, scope, completion);
        }
    }
    let _ = archive.finish_scan_set(scan_set);
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

const fn requires_rebuild(code: PortErrorCode) -> bool {
    matches!(
        code,
        PortErrorCode::InvalidData | PortErrorCode::StaleState | PortErrorCode::RebuildRequired
    )
}

fn report(
    archive: &StoreArchive,
    starting_current_generation: u64,
    outcome: IncrementalRefreshOutcome,
    counts: RefreshCounts,
) -> Result<IncrementalRefreshReport, PortError> {
    let generation = archive
        .store()
        .archive_publication()
        .map_err(|_| PortError::new(PortErrorCode::Unavailable))?
        .generation()
        .get();
    let current_generation = archive.current_cursor()?.archive_generation.get();
    Ok(IncrementalRefreshReport {
        outcome,
        files_examined: counts.files_examined,
        bytes_read: counts.bytes_read,
        events_observed: counts.events_observed,
        batches_committed: counts.batches_committed,
        diagnostics: counts.diagnostics,
        starting_current_generation,
        current_generation,
        archive_generation: generation,
    })
}

fn checked_add(left: u64, right: u64) -> Result<u64, PortError> {
    left.checked_add(right)
        .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))
}
