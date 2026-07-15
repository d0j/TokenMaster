use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, TokenCount, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterBatchParts, AdapterCheckpoint, AdapterCompletion,
    AdapterCounters, AdapterDiagnostics, Archive, ArchiveEpoch, ArchiveReplay, ArchiveRevisionId,
    ArchiveScanSetId, ArchiveSourceCursor, BatchState, CanonicalBatch, Clock, CompletionQuality,
    DiscoveredSource, ExecutionCounts, MAX_REPLAY_CONTINUATIONS_PER_RUN, MonotonicTime,
    OneShotExecutor, OperationControl, PortError, PortErrorCode, RefreshAdmission,
    RefreshCoordinator, RefreshDeadline, RefreshOutcome, RefreshRequestId, RefreshUrgency,
    ReplayCleanup, ReplayContinuation, ReplayContinuationState, ReplaySource, ReplaySourcePage,
    ScopeIdentity, ScopeManifest, ScopeSink, SourceIdentity, SourceKind, SourceSink, WriterLease,
    WriterLeaseGuard,
};

type Log = Arc<Mutex<Vec<&'static str>>>;

fn record(log: &Log, event: &'static str) {
    log.lock().expect("log lock").push(event);
}

struct FakeClock(AtomicU64);

impl FakeClock {
    fn new(now: u64) -> Self {
        Self(AtomicU64::new(now))
    }
}

impl Clock for FakeClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(self.0.load(Ordering::Acquire))
    }
}

struct ExpiringClock {
    calls: AtomicU64,
    expire_on: u64,
}

impl ExpiringClock {
    fn new(expire_on: u64) -> Self {
        Self {
            calls: AtomicU64::new(0),
            expire_on,
        }
    }
}

impl Clock for ExpiringClock {
    fn now(&self) -> MonotonicTime {
        let call = self.calls.fetch_add(1, Ordering::AcqRel) + 1;
        MonotonicTime::from_millis(u64::from(call >= self.expire_on))
    }
}

struct CancellingClock {
    calls: AtomicU64,
    cancel_on: u64,
    coordinator: Arc<Mutex<RefreshCoordinator>>,
    request_id: RefreshRequestId,
}

impl Clock for CancellingClock {
    fn now(&self) -> MonotonicTime {
        let call = self.calls.fetch_add(1, Ordering::AcqRel) + 1;
        if call == self.cancel_on {
            self.coordinator
                .lock()
                .expect("coordinator lock")
                .cancel(self.request_id)
                .expect("active request cancellation");
        }
        MonotonicTime::from_millis(0)
    }
}

struct FakeLease {
    log: Log,
    error: Option<PortErrorCode>,
}

struct FakeLeaseGuard {
    log: Log,
}

impl Drop for FakeLeaseGuard {
    fn drop(&mut self) {
        record(&self.log, "lease_drop");
    }
}

impl WriterLeaseGuard for FakeLeaseGuard {}

impl WriterLease for FakeLease {
    fn try_acquire(&mut self) -> Result<Box<dyn WriterLeaseGuard>, PortError> {
        record(&self.log, "lease");
        if let Some(code) = self.error {
            return Err(PortError::new(code));
        }
        Ok(Box::new(FakeLeaseGuard {
            log: self.log.clone(),
        }))
    }
}

fn source_identity() -> SourceIdentity {
    SourceIdentity::new(
        ScopeIdentity::new("codex", "profile-a").expect("scope"),
        "source-a",
    )
    .expect("source")
}

fn checkpoint(value: u8) -> AdapterCheckpoint {
    AdapterCheckpoint::new(vec![value].into_boxed_slice()).expect("checkpoint")
}

fn usage() -> TokenUsage {
    TokenUsage::new(
        TokenCount::Available(2),
        TokenCount::Available(0),
        TokenCount::Available(3),
        TokenCount::Available(0),
        TokenCount::Available(5),
    )
}

fn observation(source: &SourceIdentity) -> ObservationDraft {
    ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new(source.scope().provider_id()).expect("provider"),
        profile_id: UsageProfileId::new(source.scope().profile_id()).expect("profile"),
        session_id: UsageSessionId::new("session-a").expect("session"),
        parent_session_id: None,
        session_ordinal: 0,
        lineage_conflict: false,
        source_id: UsageSourceId::new(source.source_id()).expect("source"),
        source_offset: 1,
        source_verification: ObservationVerification::FullPrefix,
        timestamp: UtcTimestamp::new(1_720_000_000, 0).expect("timestamp"),
        model: ModelKey::new("gpt-5").expect("model"),
        raw_model: None,
        delta_usage: usage(),
        cumulative_usage: Some(usage()),
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        project: None,
        originator: None,
        activity: ActivityCounts::default(),
    })
    .expect("observation")
}

struct FakeAdapter {
    log: Log,
    source: Option<SourceIdentity>,
    extra_scope: Option<ScopeIdentity>,
    source_quality: CompletionQuality,
    source_error: Option<PortErrorCode>,
}

impl Adapter for FakeAdapter {
    fn visit_scopes(
        &mut self,
        control: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError> {
        record(&self.log, "scopes");
        control.check()?;
        let scope = self.source.as_ref().map_or_else(
            || source_identity().scope().clone(),
            |source| source.scope().clone(),
        );
        let _ = sink.on_scope(scope)?;
        if let Some(scope) = &self.extra_scope {
            let _ = sink.on_scope(scope.clone())?;
        }
        AdapterCompletion::new(
            CompletionQuality::Complete,
            AdapterCounters::default(),
            AdapterDiagnostics::default(),
        )
        .map_err(PortError::from)
    }

    fn visit_sources(
        &mut self,
        _scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        record(&self.log, "sources");
        control.check()?;
        let files_read = if self
            .source
            .as_ref()
            .is_some_and(|source| source.scope() == _scope)
        {
            let source = self.source.as_ref().expect("checked source");
            let discovered = DiscoveredSource::new(source.clone(), SourceKind::Active, [7; 32]);
            let _ = sink.on_source(discovered, checkpoint(1))?;
            1
        } else {
            0
        };
        if let Some(code) = self.source_error {
            return Err(PortError::new(code));
        }
        AdapterCompletion::new(
            self.source_quality,
            AdapterCounters::new(files_read, 0, 0, 0).map_err(PortError::from)?,
            AdapterDiagnostics::default(),
        )
        .map_err(PortError::from)
    }

    fn read_batch(
        &mut self,
        source: &SourceIdentity,
        _checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        record(&self.log, "read");
        control.check()?;
        AdapterBatch::new(
            source,
            AdapterBatchParts {
                observations: vec![observation(source)].into_boxed_slice(),
                relations: Box::default(),
                chunk_proofs: tokenmaster_engine::ChunkProofBatch::new(None, Box::default())
                    .map_err(PortError::from)?,
                next_checkpoint: checkpoint(2),
                state: BatchState::SnapshotEnd,
                counters: AdapterCounters::new(1, 80, 1, 0).map_err(PortError::from)?,
                diagnostics: AdapterDiagnostics::default(),
            },
        )
        .map_err(PortError::from)
    }
}

struct RepeatingCheckpointAdapter {
    inner: FakeAdapter,
    reads: u64,
}

impl Adapter for RepeatingCheckpointAdapter {
    fn visit_scopes(
        &mut self,
        control: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError> {
        self.inner.visit_scopes(control, sink)
    }

    fn visit_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        self.inner.visit_sources(scope, control, sink)
    }

    fn read_batch(
        &mut self,
        source: &SourceIdentity,
        current_checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        if self.reads != 0 {
            return self.inner.read_batch(source, current_checkpoint, control);
        }
        self.reads += 1;
        record(&self.inner.log, "read");
        control.check()?;
        AdapterBatch::new(
            source,
            AdapterBatchParts {
                observations: vec![observation(source)].into_boxed_slice(),
                relations: Box::default(),
                chunk_proofs: tokenmaster_engine::ChunkProofBatch::new(None, Box::default())
                    .map_err(PortError::from)?,
                next_checkpoint: current_checkpoint.clone(),
                state: BatchState::More,
                counters: AdapterCounters::new(1, 80, 1, 0).map_err(PortError::from)?,
                diagnostics: AdapterDiagnostics::default(),
            },
        )
        .map_err(PortError::from)
    }
}

struct CrossScopeDiscoveryAdapter {
    inner: FakeAdapter,
    foreign_source: SourceIdentity,
}

impl Adapter for CrossScopeDiscoveryAdapter {
    fn visit_scopes(
        &mut self,
        control: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError> {
        self.inner.visit_scopes(control, sink)
    }

    fn visit_sources(
        &mut self,
        _scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        record(&self.inner.log, "sources");
        control.check()?;
        let discovered =
            DiscoveredSource::new(self.foreign_source.clone(), SourceKind::Active, [7; 32]);
        let _ = sink.on_source(discovered, checkpoint(1))?;
        AdapterCompletion::new(
            CompletionQuality::Complete,
            AdapterCounters::new(1, 0, 0, 0).map_err(PortError::from)?,
            AdapterDiagnostics::default(),
        )
        .map_err(PortError::from)
    }

    fn read_batch(
        &mut self,
        source: &SourceIdentity,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        self.inner.read_batch(source, checkpoint, control)
    }
}

struct FakeArchive {
    log: Log,
    source: Option<ReplaySource>,
    observed_sources: u64,
    appended_events: u64,
    finished_scopes: Vec<CompletionQuality>,
    fail_at: Option<&'static str>,
    discard_error: Option<PortErrorCode>,
    discarded: Option<ArchiveReplay>,
}

impl FakeArchive {
    fn advance(replay: ArchiveReplay) -> ArchiveReplay {
        ArchiveReplay::new(
            replay.revision_id(),
            ArchiveEpoch::new(replay.epoch().get() + 1).expect("next epoch"),
        )
    }

    fn fail_if_configured(&self, step: &'static str) -> Result<(), PortError> {
        if self.fail_at == Some("append_stale") && step == "append" {
            return Err(PortError::new(PortErrorCode::StaleState));
        }
        if self.fail_at == Some("append_busy") && step == "append" {
            return Err(PortError::new(PortErrorCode::Busy));
        }
        if self.fail_at == Some(step) {
            return Err(PortError::new(PortErrorCode::Failed));
        }
        Ok(())
    }
}

impl Archive for FakeArchive {
    fn begin_scan_set(&mut self, _manifest: &ScopeManifest) -> Result<ArchiveScanSetId, PortError> {
        record(&self.log, "begin_scan");
        self.fail_if_configured("begin_scan")?;
        ArchiveScanSetId::new(1).map_err(PortError::from)
    }

    fn observe_source(
        &mut self,
        _scan_set: ArchiveScanSetId,
        _source: &DiscoveredSource,
        _initial_checkpoint: &AdapterCheckpoint,
    ) -> Result<(), PortError> {
        record(&self.log, "observe");
        self.fail_if_configured("observe")?;
        self.observed_sources += 1;
        Ok(())
    }

    fn finish_scope(
        &mut self,
        _scan_set: ArchiveScanSetId,
        _scope: &ScopeIdentity,
        completion: AdapterCompletion,
    ) -> Result<(), PortError> {
        record(&self.log, "finish_scope");
        self.fail_if_configured("finish_scope")?;
        self.finished_scopes.push(completion.quality());
        Ok(())
    }

    fn finish_scan_set(
        &mut self,
        _scan_set: ArchiveScanSetId,
    ) -> Result<CompletionQuality, PortError> {
        record(&self.log, "finish_scan");
        self.fail_if_configured("finish_scan")?;
        if self
            .finished_scopes
            .iter()
            .all(|quality| *quality == CompletionQuality::Complete)
        {
            Ok(CompletionQuality::Complete)
        } else {
            Ok(CompletionQuality::Partial)
        }
    }

    fn begin_replay(&mut self, _scan_set: ArchiveScanSetId) -> Result<ArchiveReplay, PortError> {
        record(&self.log, "begin_replay");
        self.fail_if_configured("begin_replay")?;
        Ok(ArchiveReplay::new(
            ArchiveRevisionId::new(1).map_err(PortError::from)?,
            ArchiveEpoch::new(1).map_err(PortError::from)?,
        ))
    }

    fn replay_source_page(
        &mut self,
        _replay: ArchiveReplay,
        _after: Option<&ArchiveSourceCursor>,
    ) -> Result<ReplaySourcePage, PortError> {
        record(&self.log, "page");
        self.fail_if_configured("page")?;
        let sources = self.source.clone().into_iter().collect::<Vec<_>>();
        let next_cursor = if self.fail_at == Some("repeat_cursor") {
            let page_calls = self
                .log
                .lock()
                .expect("log lock")
                .iter()
                .filter(|event| **event == "page")
                .count();
            (page_calls <= 2).then(|| ArchiveSourceCursor::new([9; 32]))
        } else {
            None
        };
        ReplaySourcePage::new(sources.into_boxed_slice(), next_cursor).map_err(PortError::from)
    }

    fn prepare_replay_source(
        &mut self,
        replay: ArchiveReplay,
        _source: &ReplaySource,
    ) -> Result<ArchiveReplay, PortError> {
        record(&self.log, "prepare");
        self.fail_if_configured("prepare")?;
        if self.fail_at == Some("switch_revision") {
            return Ok(ArchiveReplay::new(
                ArchiveRevisionId::new(replay.revision_id().get() + 1).map_err(PortError::from)?,
                ArchiveEpoch::new(replay.epoch().get() + 1).map_err(PortError::from)?,
            ));
        }
        Ok(Self::advance(replay))
    }

    fn append_replay_batch(
        &mut self,
        replay: ArchiveReplay,
        _source: &SourceIdentity,
        batch: CanonicalBatch,
    ) -> Result<ArchiveReplay, PortError> {
        record(&self.log, "append");
        self.fail_if_configured("append")?;
        self.appended_events += batch.events().len() as u64;
        Ok(Self::advance(replay))
    }

    fn continue_replay(&mut self, replay: ArchiveReplay) -> Result<ReplayContinuation, PortError> {
        record(&self.log, "continue");
        self.fail_if_configured("continue")?;
        let continuation_calls = self
            .log
            .lock()
            .expect("log lock")
            .iter()
            .filter(|event| **event == "continue")
            .count();
        let state = if self.fail_at == Some("continuation_bound")
            && continuation_calls <= MAX_REPLAY_CONTINUATIONS_PER_RUN
        {
            ReplayContinuationState::Pending
        } else {
            ReplayContinuationState::Complete
        };
        Ok(ReplayContinuation::new(Self::advance(replay), state))
    }

    fn seal_replay(&mut self, replay: ArchiveReplay) -> Result<ArchiveReplay, PortError> {
        record(&self.log, "seal");
        self.fail_if_configured("seal")?;
        Ok(Self::advance(replay))
    }

    fn promote_replay(&mut self, _replay: ArchiveReplay) -> Result<(), PortError> {
        record(&self.log, "promote");
        self.fail_if_configured("promote")?;
        Ok(())
    }

    fn discard_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError> {
        record(&self.log, "discard");
        self.discarded = Some(replay);
        if let Some(code) = self.discard_error {
            return Err(PortError::new(code));
        }
        Ok(())
    }
}

fn started_permit(
    coordinator: &mut RefreshCoordinator,
    clock: &FakeClock,
) -> tokenmaster_engine::RefreshPermit {
    match coordinator
        .submit(RefreshUrgency::Interactive, None, clock.now())
        .expect("admission")
    {
        RefreshAdmission::Started(permit) => permit,
        admission => panic!("unexpected admission: {admission:?}"),
    }
}

fn run_complete_fixture(
    permit: &tokenmaster_engine::RefreshPermit,
    clock: &dyn Clock,
) -> (tokenmaster_engine::OneShotResult, FakeArchive, Log) {
    let log = Log::default();
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result = OneShotExecutor::new().run(permit, clock, &mut lease, &mut adapter, &mut archive);
    (result, archive, log)
}

#[test]
fn deadline_is_enforced_at_every_execution_control_boundary() {
    const COMPLETE_PATH_CONTROL_CHECKS: u64 = 14;

    for expire_on in 1..=COMPLETE_PATH_CONTROL_CHECKS + 1 {
        let submit_clock = FakeClock::new(0);
        let mut coordinator = RefreshCoordinator::new();
        let permit = match coordinator
            .submit(
                RefreshUrgency::Interactive,
                Some(RefreshDeadline::from_millis(1)),
                submit_clock.now(),
            )
            .expect("admission")
        {
            RefreshAdmission::Started(permit) => permit,
            admission => panic!("unexpected admission: {admission:?}"),
        };
        let clock = ExpiringClock::new(expire_on);

        let (result, _archive, log) = run_complete_fixture(&permit, &clock);
        let events = log.lock().expect("log lock").clone();

        if expire_on > COMPLETE_PATH_CONTROL_CHECKS {
            assert_eq!(result.outcome(), RefreshOutcome::Completed);
            assert!(events.contains(&"promote"));
            continue;
        }

        assert_eq!(result.outcome(), RefreshOutcome::DeadlineExceeded);
        assert_eq!(result.quality(), CompletionQuality::TimedOut);
        assert_eq!(result.error(), Some(PortErrorCode::DeadlineExceeded));
        assert!(!events.contains(&"promote"));
        if events.contains(&"begin_scan") && !events.contains(&"begin_replay") {
            assert!(
                events.contains(&"finish_scan"),
                "scan was left open when deadline expired at control check {expire_on}: {events:?}"
            );
        }
        assert_eq!(
            result.cleanup(),
            if events.contains(&"begin_replay") {
                ReplayCleanup::Discarded
            } else {
                ReplayCleanup::NotRequired
            }
        );
    }
}

#[test]
fn cancellation_is_observed_between_every_execution_phase() {
    const LAST_CANCELLABLE_INTERVAL: u64 = 13;

    for cancel_on in 1..=LAST_CANCELLABLE_INTERVAL {
        let coordinator = Arc::new(Mutex::new(RefreshCoordinator::new()));
        let permit = match coordinator
            .lock()
            .expect("coordinator lock")
            .submit(
                RefreshUrgency::Interactive,
                Some(RefreshDeadline::from_millis(100)),
                MonotonicTime::from_millis(0),
            )
            .expect("admission")
        {
            RefreshAdmission::Started(permit) => permit,
            admission => panic!("unexpected admission: {admission:?}"),
        };
        let clock = CancellingClock {
            calls: AtomicU64::new(0),
            cancel_on,
            coordinator: coordinator.clone(),
            request_id: permit.id(),
        };

        let (result, _archive, log) = run_complete_fixture(&permit, &clock);
        let events = log.lock().expect("log lock").clone();

        assert_eq!(result.outcome(), RefreshOutcome::Cancelled);
        assert_eq!(result.quality(), CompletionQuality::Cancelled);
        assert_eq!(result.error(), Some(PortErrorCode::Cancelled));
        assert!(!events.contains(&"promote"));
        if events.contains(&"begin_scan") && !events.contains(&"begin_replay") {
            assert!(
                events.contains(&"finish_scan"),
                "scan was left open when cancellation arrived between phases {cancel_on} and {}: {events:?}",
                cancel_on + 1
            );
        }
        assert_eq!(
            result.cleanup(),
            if events.contains(&"begin_replay") {
                ReplayCleanup::Discarded
            } else {
                ReplayCleanup::NotRequired
            }
        );
    }
}

#[test]
fn cancellation_before_execution_stops_before_the_writer_lease() {
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    coordinator
        .cancel(permit.id())
        .expect("cancel active request");

    let (result, _archive, log) = run_complete_fixture(&permit, &clock);

    assert_eq!(result.outcome(), RefreshOutcome::Cancelled);
    assert_eq!(result.quality(), CompletionQuality::Cancelled);
    assert_eq!(result.error(), Some(PortErrorCode::Cancelled));
    assert_eq!(result.cleanup(), ReplayCleanup::NotRequired);
    assert!(log.lock().expect("log lock").is_empty());
}

#[test]
fn complete_execution_acquires_lease_then_publishes_one_canonical_result() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.request_id(), permit.id());
    assert_eq!(result.outcome(), RefreshOutcome::Completed);
    assert_eq!(result.quality(), CompletionQuality::Complete);
    assert_eq!(result.scan_set_id().map(ArchiveScanSetId::get), Some(1));
    assert_eq!(
        result.published_revision_id().map(ArchiveRevisionId::get),
        Some(1)
    );
    assert_eq!(
        result.counts(),
        ExecutionCounts::new(1, 1, 1, 1).expect("counts")
    );
    assert_eq!(result.cleanup(), ReplayCleanup::NotRequired);
    assert_eq!(result.error(), None);
    assert_eq!(archive.observed_sources, 1);
    assert_eq!(archive.appended_events, 1);
    assert_eq!(archive.finished_scopes, vec![CompletionQuality::Complete]);
    assert_eq!(
        *log.lock().expect("log lock"),
        vec![
            "lease",
            "scopes",
            "begin_scan",
            "sources",
            "observe",
            "finish_scope",
            "finish_scan",
            "begin_replay",
            "page",
            "prepare",
            "read",
            "append",
            "continue",
            "seal",
            "promote",
            "lease_drop",
        ]
    );

    let transition = coordinator
        .finish(permit.id(), result.outcome(), clock.now())
        .expect("finish");
    assert_eq!(transition.completed().outcome(), RefreshOutcome::Completed);
}

#[test]
fn complete_zero_source_scan_publishes_retention_without_adapter_reads() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: None,
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: None,
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Completed);
    assert_eq!(result.quality(), CompletionQuality::Complete);
    assert_eq!(
        result.counts(),
        ExecutionCounts::new(0, 0, 0, 1).expect("counts")
    );
    assert_eq!(result.cleanup(), ReplayCleanup::NotRequired);
    assert_eq!(archive.observed_sources, 0);
    assert_eq!(archive.appended_events, 0);
    let events = log.lock().expect("log lock").clone();
    assert!(!events.contains(&"prepare"));
    assert!(!events.contains(&"read"));
    assert!(!events.contains(&"append"));
    assert!(events.contains(&"promote"));
}

#[test]
fn busy_lease_returns_before_provider_io_or_archive_mutation() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: Some(PortErrorCode::Busy),
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Busy);
    assert_eq!(result.quality(), CompletionQuality::Failed);
    assert_eq!(result.scan_set_id(), None);
    assert_eq!(result.cleanup(), ReplayCleanup::NotRequired);
    assert_eq!(result.error(), Some(PortErrorCode::Busy));
    assert_eq!(*log.lock().expect("log lock"), vec!["lease"]);
}

#[test]
fn partial_discovery_closes_remaining_scopes_without_starting_replay() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: Some(ScopeIdentity::new("codex", "profile-b").expect("scope")),
        source_quality: CompletionQuality::Partial,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Completed);
    assert_eq!(result.quality(), CompletionQuality::Partial);
    assert_eq!(result.error(), None);
    assert_eq!(result.published_revision_id(), None);
    assert_eq!(result.cleanup(), ReplayCleanup::NotRequired);
    assert_eq!(
        archive.finished_scopes,
        vec![CompletionQuality::Partial, CompletionQuality::Partial]
    );
    let events = log.lock().expect("log lock").clone();
    assert_eq!(
        events.iter().filter(|event| **event == "sources").count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == "finish_scope")
            .count(),
        2
    );
    assert!(events.contains(&"finish_scan"));
    assert!(!events.contains(&"begin_replay"));
}

#[test]
fn adapter_failure_closes_the_scan_failed_without_starting_replay() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: Some(PortErrorCode::Failed),
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.quality(), CompletionQuality::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::Failed));
    assert_eq!(archive.finished_scopes, vec![CompletionQuality::Failed]);
    let events = log.lock().expect("log lock").clone();
    assert!(events.contains(&"finish_scan"));
    assert!(!events.contains(&"begin_replay"));
}

#[test]
fn archive_observation_failure_still_closes_failed_scan_state() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("observe"),
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.counts(), ExecutionCounts::default());
    assert_eq!(archive.finished_scopes, vec![CompletionQuality::Failed]);
    assert!(log.lock().expect("log lock").contains(&"finish_scan"));
}

#[test]
fn cross_scope_discovery_is_rejected_before_archive_observation() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let expected_source = source_identity();
    let foreign_source = SourceIdentity::new(
        ScopeIdentity::new("codex", "profile-b").expect("foreign scope"),
        "source-b",
    )
    .expect("foreign source");
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = CrossScopeDiscoveryAdapter {
        inner: FakeAdapter {
            log: log.clone(),
            source: Some(expected_source),
            extra_scope: None,
            source_quality: CompletionQuality::Complete,
            source_error: None,
        },
        foreign_source,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: None,
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::InvalidData));
    assert_eq!(archive.observed_sources, 0);
    assert!(!log.lock().expect("log lock").contains(&"observe"));
    assert_eq!(archive.finished_scopes, vec![CompletionQuality::Failed]);
}

#[test]
fn replay_fault_discards_only_the_latest_unpublished_epoch() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("append"),
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.cleanup(), ReplayCleanup::Discarded);
    assert_eq!(result.error(), Some(PortErrorCode::Failed));
    assert_eq!(
        archive.discarded,
        Some(ArchiveReplay::new(
            ArchiveRevisionId::new(1).expect("revision"),
            ArchiveEpoch::new(2).expect("epoch")
        ))
    );
    assert_eq!(
        result.counts(),
        ExecutionCounts::new(1, 0, 0, 0).expect("counts")
    );
    assert_eq!(archive.appended_events, 0);
}

#[test]
fn non_progressing_adapter_checkpoint_fails_before_archive_append() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = RepeatingCheckpointAdapter {
        inner: FakeAdapter {
            log: log.clone(),
            source: Some(source.clone()),
            extra_scope: None,
            source_quality: CompletionQuality::Complete,
            source_error: None,
        },
        reads: 0,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: None,
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::InvalidData));
    assert_eq!(result.cleanup(), ReplayCleanup::Discarded);
    assert_eq!(archive.appended_events, 0);
    assert!(!log.lock().expect("log lock").contains(&"append"));
}

#[test]
fn repeated_archive_cursor_fails_without_reprocessing_the_page() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("repeat_cursor"),
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::InvalidData));
    assert_eq!(result.cleanup(), ReplayCleanup::Discarded);
    let events = log.lock().expect("log lock").clone();
    assert_eq!(events.iter().filter(|event| **event == "page").count(), 2);
    assert_eq!(events.iter().filter(|event| **event == "append").count(), 1);
}

#[test]
fn replay_continuation_work_is_hard_bounded_per_execution() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: None,
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: None,
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("continuation_bound"),
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::CapacityExceeded));
    assert_eq!(result.cleanup(), ReplayCleanup::Discarded);
    assert_eq!(
        result.counts().replay_continuations(),
        MAX_REPLAY_CONTINUATIONS_PER_RUN as u64
    );
    assert_eq!(
        log.lock()
            .expect("log lock")
            .iter()
            .filter(|event| **event == "continue")
            .count(),
        MAX_REPLAY_CONTINUATIONS_PER_RUN
    );
}

#[test]
fn stale_epoch_error_discards_the_last_confirmed_epoch_only() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log: log.clone(),
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("append_stale"),
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.error(), Some(PortErrorCode::StaleState));
    assert_eq!(result.cleanup(), ReplayCleanup::Discarded);
    assert_eq!(
        archive.discarded.map(|replay| replay.epoch().get()),
        Some(2)
    );
}

#[test]
fn archive_cannot_switch_the_replay_revision_mid_execution() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log,
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("switch_revision"),
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::InvalidData));
    assert_eq!(result.cleanup(), ReplayCleanup::Discarded);
    assert_eq!(
        archive.discarded,
        Some(ArchiveReplay::new(
            ArchiveRevisionId::new(1).expect("revision"),
            ArchiveEpoch::new(1).expect("epoch")
        ))
    );
    assert_eq!(archive.appended_events, 0);
}

#[test]
fn failed_replay_discard_is_reported_without_masking_the_execution_error() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log,
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("append_stale"),
        discard_error: Some(PortErrorCode::StaleState),
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::StaleState));
    assert_eq!(result.cleanup(), ReplayCleanup::Failed);
}

#[test]
fn busy_from_a_non_lease_port_is_a_failure_not_false_admission_backpressure() {
    let log = Log::default();
    let clock = FakeClock::new(0);
    let mut coordinator = RefreshCoordinator::new();
    let permit = started_permit(&mut coordinator, &clock);
    let source = source_identity();
    let mut lease = FakeLease {
        log: log.clone(),
        error: None,
    };
    let mut adapter = FakeAdapter {
        log: log.clone(),
        source: Some(source.clone()),
        extra_scope: None,
        source_quality: CompletionQuality::Complete,
        source_error: None,
    };
    let mut archive = FakeArchive {
        log,
        source: Some(ReplaySource::new(source, checkpoint(1))),
        observed_sources: 0,
        appended_events: 0,
        finished_scopes: Vec::new(),
        fail_at: Some("append_busy"),
        discard_error: None,
        discarded: None,
    };

    let result =
        OneShotExecutor::new().run(&permit, &clock, &mut lease, &mut adapter, &mut archive);

    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.quality(), CompletionQuality::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::Busy));
    assert_eq!(result.cleanup(), ReplayCleanup::Discarded);
}
