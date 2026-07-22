use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use tempfile::TempDir;
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, TokenCount, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterBatchParts, AdapterCheckpoint, AdapterCompletion,
    AdapterCounters, AdapterDiagnostics, AdapterSourceProgress, AdapterSourceProgressParts,
    AdapterSourceState, AdapterVerification, BatchState, CompletionQuality, DiscoveredSource,
    OperationControl, PortError, ReplaySourceSink, ScopeIdentity, ScopeSink, SourceBatchReader,
    SourceIdentity, SourceKind, SourceSink,
};
use tokenmaster_provider::{ProviderCapability, ProviderDescriptor, ProviderId};
use tokenmaster_runtime::{
    GitRepositoryHintIngress, LiveProviderAdapter, LiveRuntime, ProviderWatchRoots, RuntimeError,
    UsageProviderFactory,
};
use tokenmaster_store::{UsageActivityQuery, UsageReadStore, UsageStore};

const INITIAL_CHECKPOINT: &[u8] = b"synthetic-provider-v1\0page-0001";
const NEXT_CHECKPOINT: &[u8] = b"synthetic-provider-v1\0page-0002";
const INITIAL_RESUME: &[u8] = b"page-0001";
const NEXT_RESUME: &[u8] = b"page-0002";

fn source() -> DiscoveredSource {
    let scope = ScopeIdentity::new("synthetic", "profile").expect("scope");
    let identity = SourceIdentity::new(scope, "source", [9; 32]).expect("source");
    DiscoveredSource::new(identity, SourceKind::Active)
}

fn checkpoint(value: &[u8]) -> AdapterCheckpoint {
    AdapterCheckpoint::new(value.to_vec().into_boxed_slice()).expect("checkpoint")
}

fn progress(source: &DiscoveredSource, offset: u64, resume: &[u8]) -> AdapterSourceProgress {
    AdapterSourceProgress::new(AdapterSourceProgressParts {
        schema_version: 1,
        physical_identity: None,
        logical_identity: *source.identity().logical_file_key(),
        committed_offset: offset,
        scan_offset: offset,
        observed_extent: offset,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [0; 32],
        provider_resume: resume.to_vec().into_boxed_slice(),
        discarding_oversized_record: false,
        incomplete_tail: false,
        verification: AdapterVerification::Full,
    })
    .expect("progress")
}

fn state(source: &DiscoveredSource) -> AdapterSourceState {
    AdapterSourceState::new(
        checkpoint(INITIAL_CHECKPOINT),
        progress(source, 0, INITIAL_RESUME),
    )
    .expect("state")
}

fn observation(source: &DiscoveredSource) -> ObservationDraft {
    let usage = TokenUsage::new(
        TokenCount::Available(2),
        TokenCount::Available(0),
        TokenCount::Available(3),
        TokenCount::Available(0),
        TokenCount::Available(5),
    );
    ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new(source.identity().scope().provider_id())
            .expect("provider"),
        profile_id: UsageProfileId::new(source.identity().scope().profile_id()).expect("profile"),
        session_id: UsageSessionId::new("session-a").expect("session"),
        parent_session_id: None,
        session_ordinal: 0,
        lineage_conflict: false,
        source_id: UsageSourceId::new(source.identity().source_id()).expect("source"),
        source_offset: 1,
        source_verification: ObservationVerification::FullPrefix,
        timestamp: UtcTimestamp::new(1_720_000_000, 0).expect("timestamp"),
        model: ModelKey::new("synthetic-model").expect("model"),
        raw_model: None,
        delta_usage: usage,
        cumulative_usage: Some(usage),
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        reported_cost: None,
        project: None,
        originator: None,
        activity: ActivityCounts::default(),
    })
    .expect("observation")
}

fn batch(source: &DiscoveredSource) -> Result<AdapterBatch, PortError> {
    AdapterBatch::new(
        source.identity(),
        AdapterBatchParts {
            observations: vec![observation(source)].into_boxed_slice(),
            relations: Box::default(),
            chunk_proofs: tokenmaster_engine::ChunkProofBatch::new(
                None,
                vec![tokenmaster_engine::ChunkProof::new(0, 2, [0; 32]).map_err(PortError::from)?]
                    .into_boxed_slice(),
            )
            .map_err(PortError::from)?,
            next_checkpoint: checkpoint(NEXT_CHECKPOINT),
            next_progress: progress(source, 2, NEXT_RESUME),
            state: BatchState::SnapshotEnd,
            counters: AdapterCounters::new(1, 1, 1, 0).map_err(PortError::from)?,
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .map_err(PortError::from)
}

fn completion(files_read: u64) -> Result<AdapterCompletion, PortError> {
    AdapterCompletion::new(
        CompletionQuality::Complete,
        AdapterCounters::new(files_read, 0, 0, 0).map_err(PortError::from)?,
        AdapterDiagnostics::default(),
    )
    .map_err(PortError::from)
}

struct SyntheticFactory {
    descriptor: ProviderDescriptor,
    record: Arc<SyntheticFactoryRecord>,
}

#[derive(Default)]
struct SyntheticFactoryRecord {
    build_count: AtomicUsize,
    received_repository_hints: AtomicBool,
}

impl SyntheticFactory {
    fn new(repository_activity: bool) -> (Self, Arc<SyntheticFactoryRecord>) {
        let record = Arc::new(SyntheticFactoryRecord::default());
        let capabilities = repository_activity
            .then_some(ProviderCapability::RepositoryActivity)
            .into_iter()
            .collect::<Vec<_>>();
        let factory = Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new("synthetic").expect("provider id"),
                "Synthetic",
                capabilities,
            )
            .expect("descriptor"),
            record: Arc::clone(&record),
        };
        (factory, record)
    }
}

impl UsageProviderFactory for SyntheticFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn build(
        self: Box<Self>,
        repository_hints: Option<GitRepositoryHintIngress>,
    ) -> Result<Box<dyn LiveProviderAdapter>, RuntimeError> {
        self.record.build_count.fetch_add(1, Ordering::SeqCst);
        self.record
            .received_repository_hints
            .store(repository_hints.is_some(), Ordering::SeqCst);
        Ok(Box::new(SyntheticAdapter))
    }
}

struct SyntheticAdapter;

impl Adapter for SyntheticAdapter {
    fn visit_scopes(
        &mut self,
        control: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        let _ = sink.on_scope(source().identity().scope().clone())?;
        completion(0)
    }

    fn visit_sources(
        &mut self,
        _scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        let source = source();
        let _ = sink.on_source(source.clone(), state(&source))?;
        completion(1)
    }

    fn visit_replay_sources(
        &mut self,
        _scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        let source = source();
        let mut reader = SyntheticReader {
            source: source.clone(),
        };
        let _ = sink.on_source(source.clone(), state(&source), &mut reader)?;
        completion(1)
    }
}

impl LiveProviderAdapter for SyntheticAdapter {
    fn watch_roots(&self) -> ProviderWatchRoots {
        ProviderWatchRoots::empty()
    }
}

struct SyntheticReader {
    source: DiscoveredSource,
}

impl SourceBatchReader for SyntheticReader {
    fn restore_checkpoint(
        &mut self,
        progress: &AdapterSourceProgress,
        control: &OperationControl<'_>,
    ) -> Result<AdapterCheckpoint, PortError> {
        control.check()?;
        match progress.provider_resume() {
            INITIAL_RESUME => Ok(checkpoint(INITIAL_CHECKPOINT)),
            NEXT_RESUME => Ok(checkpoint(NEXT_CHECKPOINT)),
            _ => Err(PortError::new(
                tokenmaster_engine::PortErrorCode::InvalidData,
            )),
        }
    }

    fn validate_checkpoint(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<(), PortError> {
        control.check()?;
        if checkpoint.as_bytes() == INITIAL_CHECKPOINT || checkpoint.as_bytes() == NEXT_CHECKPOINT {
            Ok(())
        } else {
            Err(PortError::new(
                tokenmaster_engine::PortErrorCode::InvalidData,
            ))
        }
    }

    fn read_batch(
        &mut self,
        _checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        control.check()?;
        batch(&self.source)
    }
}

#[test]
fn injected_synthetic_provider_publishes_usage_without_codex_discovery() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("usage.sqlite3");
    let (factory, record) = SyntheticFactory::new(false);
    let mut runtime = LiveRuntime::start_with_provider(&archive, Box::new(factory))
        .expect("start synthetic provider runtime");

    let completion = runtime
        .wait_for_completion(Duration::from_secs(10))
        .expect("worker completion")
        .expect("initial publication");
    assert_eq!(
        completion.outcome(),
        tokenmaster_engine::RefreshOutcome::Completed
    );
    assert_eq!(
        runtime.shutdown().expect("shutdown"),
        tokenmaster_runtime::LivePhase::Stopped
    );

    let store = UsageStore::open(&archive).expect("store");
    assert_eq!(store.counts().expect("counts").canonical_events(), 1);

    let mut reader = UsageReadStore::open(&archive).expect("read store");
    let capture = reader
        .capture_activity_page(
            UsageActivityQuery::new(None, None, 10, Duration::from_secs(2))
                .expect("activity query"),
        )
        .expect("activity capture");
    assert_eq!(capture.events().len(), 1);
    assert_eq!(capture.events()[0].provider_id(), "synthetic");
    assert_eq!(record.build_count.load(Ordering::SeqCst), 1);
    assert!(
        !record.received_repository_hints.load(Ordering::SeqCst),
        "factory without RepositoryActivity must receive no Git ingress"
    );
}

#[test]
fn repository_activity_capable_factory_receives_git_ingress() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("usage.sqlite3");
    let (factory, record) = SyntheticFactory::new(true);
    let mut runtime = LiveRuntime::start_with_provider(&archive, Box::new(factory))
        .expect("start repository-capable runtime");

    runtime
        .wait_for_completion(Duration::from_secs(10))
        .expect("worker completion")
        .expect("initial publication");
    assert_eq!(
        runtime.shutdown().expect("shutdown"),
        tokenmaster_runtime::LivePhase::Stopped
    );

    assert_eq!(record.build_count.load(Ordering::SeqCst), 1);
    assert!(
        record.received_repository_hints.load(Ordering::SeqCst),
        "factory with RepositoryActivity must receive Git ingress"
    );
}

#[test]
fn provider_watch_roots_debug_does_not_expose_absolute_paths() {
    let temporary = TempDir::new().expect("temporary directory");
    let root = temporary.path().to_path_buf();
    let roots = ProviderWatchRoots::try_new(vec![root.clone()]).expect("bounded roots");

    assert!(!format!("{roots:?}").contains(&format!("{root:?}")));
}
