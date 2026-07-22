use std::sync::{Arc, Mutex};

use tempfile::TempDir;
use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterBatchParts, AdapterCheckpoint, AdapterCompletion,
    AdapterCounters, AdapterDiagnostics, AdapterSourceProgress, AdapterSourceProgressParts,
    AdapterSourceState, AdapterVerification, BatchState, ChunkProof, ChunkProofBatch, Clock,
    CompletionQuality, DiscoveredSource, MonotonicTime, OneShotExecutor, OperationControl,
    PortError, RefreshAdmission, RefreshCoordinator, RefreshUrgency, ReplaySourceSink,
    ScopeIdentity, ScopeSink, SourceBatchReader, SourceIdentity, SourceKind, SourceSink,
    WriterLease, WriterLeaseGuard,
};
use tokenmaster_runtime::{StoreArchive, refresh_incremental};
use tokenmaster_store::UsageStore;

const CHECKPOINT: &[u8] = b"synthetic-provider-v1\0page-0001";

#[derive(Clone, Copy)]
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(1)
    }
}

struct OpenLease;
struct OpenGuard;
impl WriterLeaseGuard for OpenGuard {}
impl WriterLease for OpenLease {
    fn try_acquire(&mut self) -> Result<Box<dyn WriterLeaseGuard>, PortError> {
        Ok(Box::new(OpenGuard))
    }
}

fn checkpoint() -> AdapterCheckpoint {
    AdapterCheckpoint::new(CHECKPOINT.to_vec().into_boxed_slice()).expect("bounded checkpoint")
}

fn progress() -> AdapterSourceProgress {
    AdapterSourceProgress::new(AdapterSourceProgressParts {
        schema_version: 1,
        physical_identity: None,
        logical_identity: [7; 32],
        committed_offset: 1,
        scan_offset: 1,
        observed_extent: 1,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [0; 32],
        provider_resume: b"page-0001".to_vec().into_boxed_slice(),
        discarding_oversized_record: false,
        incomplete_tail: false,
        verification: AdapterVerification::Full,
    })
    .expect("valid progress")
}

fn source() -> DiscoveredSource {
    let scope = ScopeIdentity::new("synthetic", "profile").expect("scope");
    let identity = SourceIdentity::new(scope, "source", [7; 32]).expect("source");
    DiscoveredSource::new(identity, SourceKind::Active)
}

fn permit() -> tokenmaster_engine::RefreshPermit {
    let mut coordinator = RefreshCoordinator::new();
    let RefreshAdmission::Started(permit) = coordinator
        .submit(
            RefreshUrgency::Recovery,
            None,
            MonotonicTime::from_millis(0),
        )
        .expect("admission")
    else {
        panic!("started");
    };
    permit
}

struct SyntheticAdapter {
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl SyntheticAdapter {
    fn state() -> AdapterSourceState {
        AdapterSourceState::new(checkpoint(), progress()).expect("state")
    }
    fn completion() -> Result<AdapterCompletion, PortError> {
        AdapterCompletion::new(
            CompletionQuality::Complete,
            AdapterCounters::new(1, 0, 0, 0).map_err(PortError::from)?,
            AdapterDiagnostics::default(),
        )
        .map_err(PortError::from)
    }
}
impl Adapter for SyntheticAdapter {
    fn visit_scopes(
        &mut self,
        _: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError> {
        let _ = sink.on_scope(source().identity().scope().clone())?;
        Self::completion()
    }
    fn visit_sources(
        &mut self,
        _: &ScopeIdentity,
        _: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        let _ = sink.on_source(source(), Self::state())?;
        Self::completion()
    }
    fn visit_replay_sources(
        &mut self,
        _: &ScopeIdentity,
        _: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        let mut reader = SyntheticReader {
            seen: Arc::clone(&self.seen),
        };
        let _ = sink.on_source(source(), Self::state(), &mut reader)?;
        Self::completion()
    }
}

struct SyntheticReader {
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl SourceBatchReader for SyntheticReader {
    fn restore_checkpoint(
        &mut self,
        progress: &AdapterSourceProgress,
        _: &OperationControl<'_>,
    ) -> Result<AdapterCheckpoint, PortError> {
        if progress.provider_resume() != b"page-0001" {
            return Err(PortError::new(
                tokenmaster_engine::PortErrorCode::InvalidData,
            ));
        }
        Ok(checkpoint())
    }
    fn validate_checkpoint(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        _: &OperationControl<'_>,
    ) -> Result<(), PortError> {
        self.seen
            .lock()
            .expect("seen")
            .push(checkpoint.as_bytes().to_vec());
        Ok(())
    }
    fn read_batch(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        _: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        self.seen
            .lock()
            .expect("seen")
            .push(checkpoint.as_bytes().to_vec());
        let batch = AdapterBatch::new(
            source().identity(),
            AdapterBatchParts {
                observations: Box::default(),
                relations: Box::default(),
                chunk_proofs: ChunkProofBatch::new(
                    None,
                    vec![ChunkProof::new(0, 1, [0; 32]).map_err(PortError::from)?]
                        .into_boxed_slice(),
                )
                .map_err(PortError::from)?,
                next_checkpoint: checkpoint.clone(),
                next_progress: progress(),
                state: BatchState::SnapshotEnd,
                counters: AdapterCounters::default(),
                diagnostics: AdapterDiagnostics::default(),
            },
        )
        .expect("synthetic batch valid");
        Ok(batch)
    }
}

#[test]
fn full_rebuild_reopen_and_incremental_reuse_exact_opaque_provider_checkpoint() {
    let directory = TempDir::new().expect("temporary store");
    let path = directory.path().join("usage.sqlite");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = SyntheticAdapter {
        seen: Arc::clone(&seen),
    };
    let mut archive = StoreArchive::new(UsageStore::open(&path).expect("store"));
    let result = OneShotExecutor::new().run(
        &permit(),
        &FixedClock,
        &mut OpenLease,
        &mut adapter,
        &mut archive,
    );
    assert_eq!(result.quality(), CompletionQuality::Complete, "{result:?}");
    drop(archive);

    let mut archive = StoreArchive::new(UsageStore::open(&path).expect("reopen"));
    let control = OperationControl::new(&permit(), &FixedClock);
    refresh_incremental(&mut adapter, &mut archive, &control).expect("incremental");
    let seen = seen.lock().expect("seen");
    assert!(
        seen.iter()
            .all(|checkpoint| checkpoint.as_slice() == CHECKPOINT)
    );
    assert!(
        !format!(
            "{:?}",
            archive
                .store()
                .generation_snapshot(tokenmaster_store::SourceKey::from_bytes([7; 32]))
                .expect("snapshot")
        )
        .contains("synthetic-provider-v1")
    );
}
