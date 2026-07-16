use std::sync::atomic::{AtomicU64, Ordering};

use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterBatchParts, AdapterCheckpoint, AdapterCompletion,
    AdapterCounters, AdapterDiagnostics, Archive, ArchiveEpoch, ArchiveReplay, ArchiveRevisionId,
    ArchiveScanSetId, BatchState, Clock, CompletionQuality, DiscoveredSource, MonotonicTime,
    OperationControl, OperationStop, PortError, PortErrorCode, RefreshAdmission,
    RefreshCoordinator, RefreshDeadline, RefreshUrgency, ReplayContinuation,
    ReplayContinuationState, ReplaySourceSink, ScopeIdentity, ScopeManifest, ScopeSink,
    SinkControl, SourceBatchReader, SourceIdentity, SourceKind, SourceSink, WriterLease,
    WriterLeaseGuard,
};

fn source_identity() -> SourceIdentity {
    SourceIdentity::new(
        ScopeIdentity::new("codex", "profile-a").expect("scope"),
        "source-a",
        [7; 32],
    )
    .expect("source")
}

struct FakeClock(AtomicU64);

impl FakeClock {
    fn set(&self, value: u64) {
        self.0.store(value, Ordering::Release);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(self.0.load(Ordering::Acquire))
    }
}

#[test]
fn operation_control_uses_only_monotonic_time_and_cancellation_wins() {
    let clock = FakeClock(AtomicU64::new(4));
    let mut coordinator = RefreshCoordinator::new();
    let permit = match coordinator
        .submit(
            RefreshUrgency::Interactive,
            Some(RefreshDeadline::from_millis(5)),
            clock.now(),
        )
        .expect("admission")
    {
        RefreshAdmission::Started(permit) => permit,
        admission => panic!("unexpected admission: {admission:?}"),
    };
    let control = OperationControl::new(&permit, &clock);
    assert_eq!(control.stop_reason(), None);

    clock.set(5);
    assert_eq!(control.stop_reason(), Some(OperationStop::DeadlineExceeded));
    assert_eq!(
        control.check().expect_err("deadline").code(),
        PortErrorCode::DeadlineExceeded
    );

    coordinator.cancel(permit.id()).expect("cancel");
    assert_eq!(control.stop_reason(), Some(OperationStop::Cancelled));
    assert_eq!(
        control.check().expect_err("cancel").code(),
        PortErrorCode::Cancelled
    );
}

#[derive(Default)]
struct CollectedScopes(Vec<ScopeIdentity>);

impl ScopeSink for CollectedScopes {
    fn on_scope(&mut self, scope: ScopeIdentity) -> Result<SinkControl, PortError> {
        self.0.push(scope);
        Ok(SinkControl::Continue)
    }
}

#[derive(Default)]
struct CollectedSources(Vec<(DiscoveredSource, AdapterCheckpoint)>);

impl SourceSink for CollectedSources {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_checkpoint: AdapterCheckpoint,
    ) -> Result<SinkControl, PortError> {
        self.0.push((source, initial_checkpoint));
        Ok(SinkControl::Continue)
    }
}

struct FakeAdapter {
    source: SourceIdentity,
}

struct FakeSourceReader {
    source: SourceIdentity,
}

impl SourceBatchReader for FakeSourceReader {
    fn read_batch(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        control.check()?;
        AdapterBatch::new(
            &self.source,
            AdapterBatchParts {
                observations: Box::default(),
                relations: Box::default(),
                chunk_proofs: tokenmaster_engine::ChunkProofBatch::new(None, Box::default())
                    .map_err(PortError::from)?,
                next_checkpoint: checkpoint.clone(),
                state: BatchState::SnapshotEnd,
                counters: AdapterCounters::default(),
                diagnostics: AdapterDiagnostics::default(),
            },
        )
        .map_err(PortError::from)
    }
}

impl Adapter for FakeAdapter {
    fn visit_scopes(
        &mut self,
        control: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        let _ = sink.on_scope(self.source.scope().clone())?;
        AdapterCompletion::new(
            CompletionQuality::Complete,
            AdapterCounters::default(),
            AdapterDiagnostics::default(),
        )
        .map_err(PortError::from)
    }

    fn visit_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        if scope != self.source.scope() {
            return Err(PortError::new(PortErrorCode::InvalidData));
        }
        let discovered = DiscoveredSource::new(self.source.clone(), SourceKind::Active);
        let checkpoint =
            AdapterCheckpoint::new(vec![1, 2, 3].into_boxed_slice()).map_err(PortError::from)?;
        let _ = sink.on_source(discovered, checkpoint)?;
        AdapterCompletion::new(
            CompletionQuality::Complete,
            AdapterCounters::new(1, 0, 0, 0).map_err(PortError::from)?,
            AdapterDiagnostics::default(),
        )
        .map_err(PortError::from)
    }

    fn visit_replay_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        if scope != self.source.scope() {
            return Err(PortError::new(PortErrorCode::InvalidData));
        }
        let source = DiscoveredSource::new(self.source.clone(), SourceKind::Active);
        let checkpoint =
            AdapterCheckpoint::new(vec![1, 2, 3].into_boxed_slice()).map_err(PortError::from)?;
        let mut reader = FakeSourceReader {
            source: self.source.clone(),
        };
        let _ = sink.on_source(source, checkpoint, &mut reader)?;
        AdapterCompletion::new(
            CompletionQuality::Complete,
            AdapterCounters::new(1, 0, 0, 0).map_err(PortError::from)?,
            AdapterDiagnostics::default(),
        )
        .map_err(PortError::from)
    }
}

struct CollectedReplaySources<'a> {
    control: &'a OperationControl<'a>,
    sources: Vec<SourceIdentity>,
    batch_count: usize,
}

impl ReplaySourceSink for CollectedReplaySources<'_> {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_checkpoint: AdapterCheckpoint,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        let batch = reader.read_batch(&initial_checkpoint, self.control)?;
        assert!(reader.take_repository_activity_hint().is_none());
        assert_eq!(batch.source_identity(), source.identity());
        self.sources.push(source.identity().clone());
        self.batch_count += 1;
        Ok(SinkControl::Continue)
    }
}

#[test]
fn adapter_streams_owned_normalized_values_through_object_safe_callbacks() {
    let clock = FakeClock(AtomicU64::new(0));
    let mut coordinator = RefreshCoordinator::new();
    let permit = match coordinator
        .submit(RefreshUrgency::Hint, None, clock.now())
        .expect("admission")
    {
        RefreshAdmission::Started(permit) => permit,
        admission => panic!("unexpected admission: {admission:?}"),
    };
    let control = OperationControl::new(&permit, &clock);
    let mut adapter: Box<dyn Adapter> = Box::new(FakeAdapter {
        source: source_identity(),
    });
    let mut scopes = CollectedScopes::default();
    let completion = adapter
        .visit_scopes(&control, &mut scopes)
        .expect("scope discovery");
    assert_eq!(completion.quality(), CompletionQuality::Complete);
    assert_eq!(scopes.0.len(), 1);

    let mut sources = CollectedSources::default();
    adapter
        .visit_sources(&scopes.0[0], &control, &mut sources)
        .expect("source discovery");
    assert_eq!(sources.0.len(), 1);
    assert_eq!(sources.0[0].0.identity(), &source_identity());
    assert_eq!(sources.0[0].1.as_bytes(), &[1, 2, 3]);
    let mut replay = CollectedReplaySources {
        control: &control,
        sources: Vec::new(),
        batch_count: 0,
    };
    adapter
        .visit_replay_sources(&scopes.0[0], &control, &mut replay)
        .expect("replay discovery");
    assert_eq!(replay.sources, vec![source_identity()]);
    assert_eq!(replay.batch_count, 1);
}

struct Guard;

impl WriterLeaseGuard for Guard {}

struct FakeLease;

impl WriterLease for FakeLease {
    fn try_acquire(&mut self) -> Result<Box<dyn WriterLeaseGuard>, PortError> {
        Ok(Box::new(Guard))
    }
}

struct FakeArchive;

impl Archive for FakeArchive {
    fn begin_scan_set(&mut self, _manifest: &ScopeManifest) -> Result<ArchiveScanSetId, PortError> {
        ArchiveScanSetId::new(1).map_err(PortError::from)
    }

    fn observe_source(
        &mut self,
        _scan_set: ArchiveScanSetId,
        _source: &DiscoveredSource,
        _initial_checkpoint: &AdapterCheckpoint,
    ) -> Result<(), PortError> {
        Ok(())
    }

    fn finish_scope(
        &mut self,
        _scan_set: ArchiveScanSetId,
        _scope: &ScopeIdentity,
        _completion: AdapterCompletion,
    ) -> Result<(), PortError> {
        Ok(())
    }

    fn finish_scan_set(
        &mut self,
        _scan_set: ArchiveScanSetId,
    ) -> Result<CompletionQuality, PortError> {
        Ok(CompletionQuality::Complete)
    }

    fn begin_replay(&mut self, _scan_set: ArchiveScanSetId) -> Result<ArchiveReplay, PortError> {
        Ok(ArchiveReplay::new(
            ArchiveRevisionId::new(1).map_err(PortError::from)?,
            ArchiveEpoch::new(1).map_err(PortError::from)?,
        ))
    }

    fn prepare_replay_source(
        &mut self,
        replay: ArchiveReplay,
        _source: &DiscoveredSource,
        _initial_checkpoint: &AdapterCheckpoint,
    ) -> Result<ArchiveReplay, PortError> {
        Ok(replay)
    }

    fn append_replay_batch(
        &mut self,
        replay: ArchiveReplay,
        _source: &SourceIdentity,
        _batch: tokenmaster_engine::CanonicalBatch,
    ) -> Result<ArchiveReplay, PortError> {
        Ok(replay)
    }

    fn continue_replay(&mut self, replay: ArchiveReplay) -> Result<ReplayContinuation, PortError> {
        Ok(ReplayContinuation::new(
            replay,
            ReplayContinuationState::Complete,
        ))
    }

    fn seal_replay(&mut self, replay: ArchiveReplay) -> Result<ArchiveReplay, PortError> {
        Ok(replay)
    }

    fn promote_replay(&mut self, _replay: ArchiveReplay) -> Result<(), PortError> {
        Ok(())
    }

    fn discard_replay(&mut self, _replay: ArchiveReplay) -> Result<(), PortError> {
        Ok(())
    }
}

#[test]
fn all_ports_are_object_safe_and_errors_are_stable_codes_only() {
    let mut lease: Box<dyn WriterLease> = Box::new(FakeLease);
    let _guard = lease.try_acquire().expect("lease");
    let mut archive: Box<dyn Archive> = Box::new(FakeArchive);
    let manifest = ScopeManifest::new(vec![source_identity().scope().clone()].into_boxed_slice())
        .expect("manifest");
    assert_eq!(
        archive.begin_scan_set(&manifest).expect("scan set").get(),
        1
    );

    let error = PortError::new(PortErrorCode::Unavailable);
    assert_eq!(error.to_string(), "unavailable");
    assert_eq!(format!("{error:?}"), "PortError { code: Unavailable }");
}
