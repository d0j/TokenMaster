use tokenmaster_engine::{
    Adapter, AdapterCompletion, AdapterCounters, AdapterDiagnostics, Archive, BatchState,
    CompletionQuality, DiscoveredSource, MAX_REPLAY_CONTINUATIONS_PER_RUN, OperationControl,
    PortError, PortErrorCode, ReplaySourceSink, ScopeIdentity, ScopeManifest, ScopeSink,
    SinkControl, SourceBatchReader, SourceSink, canonicalize_batch,
};
use tokenmaster_store::{ArchivePublicationQuality, MAX_SCAN_SCOPES};

use crate::StoreArchive;
use crate::store_archive::CurrentCursor;

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
}

#[derive(Default)]
struct RefreshCounts {
    files_examined: u64,
    bytes_read: u64,
    events_observed: u64,
    batches_committed: u64,
    diagnostics: u64,
}

pub fn refresh_incremental(
    adapter: &mut dyn Adapter,
    archive: &mut StoreArchive,
    control: &OperationControl<'_>,
) -> Result<IncrementalRefreshReport, PortError> {
    control.check()?;
    let mut scope_sink = ScopeCollector::default();
    let scope_completion = adapter.visit_scopes(control, &mut scope_sink)?;
    if scope_completion.quality() != CompletionQuality::Complete || scope_sink.scopes.is_empty() {
        return report(
            archive,
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
        return report(archive, IncrementalRefreshOutcome::RebuildRequired, counts);
    }
    if publication.quality() == ArchivePublicationQuality::Partial {
        let settled = settle_current(archive, &mut cursor)?;
        if !settled {
            let outcome =
                run_tail_passes(adapter, archive, control, &scopes, &mut cursor, &mut counts)?;
            return report(archive, outcome, counts);
        }
        return report(archive, IncrementalRefreshOutcome::Complete, counts);
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
                    return report(archive, IncrementalRefreshOutcome::RebuildRequired, counts);
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
        return report(archive, IncrementalRefreshOutcome::Partial, counts);
    }
    cursor = match archive.publish_current_scan(scan_set) {
        Ok(cursor) => cursor,
        Err(error) if error.code() == PortErrorCode::RebuildRequired => {
            archive.mark_rebuild_required(cursor)?;
            return report(archive, IncrementalRefreshOutcome::RebuildRequired, counts);
        }
        Err(error) => return Err(error),
    };
    let outcome = run_tail_passes(adapter, archive, control, &scopes, &mut cursor, &mut counts)?;
    report(archive, outcome, counts)
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
        };
        let completion = match adapter.visit_replay_sources(scope, control, &mut sink) {
            Ok(completion) => completion,
            Err(error) if error.code() == PortErrorCode::RebuildRequired => {
                *cursor = archive.mark_rebuild_required(*cursor)?;
                return Ok(IncrementalRefreshOutcome::RebuildRequired);
            }
            Err(error) => return Err(error),
        };
        *cursor = sink.cursor;
        if completion.quality() != CompletionQuality::Complete {
            return Ok(IncrementalRefreshOutcome::Partial);
        }
    }
    if settle_current(archive, cursor)? {
        Ok(IncrementalRefreshOutcome::Complete)
    } else {
        Ok(IncrementalRefreshOutcome::Partial)
    }
}

fn settle_current(
    archive: &mut StoreArchive,
    cursor: &mut CurrentCursor,
) -> Result<bool, PortError> {
    for _ in 0..MAX_REPLAY_CONTINUATIONS_PER_RUN {
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
        checkpoint: tokenmaster_engine::AdapterCheckpoint,
    ) -> Result<SinkControl, PortError> {
        self.archive
            .observe_source(self.scan_set, &source, &checkpoint)?;
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
        _initial_checkpoint: tokenmaster_engine::AdapterCheckpoint,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        let checkpoint = self.archive.current_checkpoint(source.identity())?;
        reader.validate_checkpoint(&checkpoint, self.control)?;
        self.files_examined = checked_add(self.files_examined, 1)?;
        Ok(SinkControl::Continue)
    }
}

struct ApplySink<'a> {
    archive: &'a mut StoreArchive,
    control: &'a OperationControl<'a>,
    cursor: CurrentCursor,
    counts: &'a mut RefreshCounts,
}

impl ReplaySourceSink for ApplySink<'_> {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        _initial_checkpoint: tokenmaster_engine::AdapterCheckpoint,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        loop {
            self.control.check()?;
            let checkpoint = self.archive.current_checkpoint(source.identity())?;
            let batch = reader.read_batch(&checkpoint, self.control)?;
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
                break;
            }
            self.counts.bytes_read = checked_add(self.counts.bytes_read, counters.bytes_read())?;
            self.counts.events_observed =
                checked_add(self.counts.events_observed, counters.events_observed())?;
            self.counts.diagnostics = checked_add(self.counts.diagnostics, counters.diagnostics())?;
            let canonical = canonicalize_batch(source.identity(), batch)?;
            let (next_cursor, remaining_work, source_caught_up) = self
                .archive
                .append_current_batch(self.cursor, source.identity(), canonical)?;
            self.cursor = next_cursor;
            self.counts.batches_committed = checked_add(self.counts.batches_committed, 1)?;
            if remaining_work {
                let _ = settle_current(self.archive, &mut self.cursor)?;
            }
            if state == BatchState::SnapshotEnd {
                if !source_caught_up {
                    return Ok(SinkControl::Stop);
                }
                break;
            }
        }
        Ok(SinkControl::Continue)
    }
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

fn report(
    archive: &StoreArchive,
    outcome: IncrementalRefreshOutcome,
    counts: RefreshCounts,
) -> Result<IncrementalRefreshReport, PortError> {
    let generation = archive
        .store()
        .archive_publication()
        .map_err(|_| PortError::new(PortErrorCode::Unavailable))?
        .generation()
        .get();
    Ok(IncrementalRefreshReport {
        outcome,
        files_examined: counts.files_examined,
        bytes_read: counts.bytes_read,
        events_observed: counts.events_observed,
        batches_committed: counts.batches_committed,
        diagnostics: counts.diagnostics,
        archive_generation: generation,
    })
}

fn checked_add(left: u64, right: u64) -> Result<u64, PortError> {
    left.checked_add(right)
        .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))
}
