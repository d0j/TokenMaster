use std::path::Path;

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{
    AdapterCompletion, AdapterSourceState, Archive, ArchiveReplay, ArchiveScanSetId,
    CanonicalBatch, Clock, CompletionQuality, DiscoveredSource, MonotonicTime, OneShotExecutor,
    PortError, PortErrorCode, RefreshAdmission, RefreshCoordinator, RefreshOutcome, RefreshUrgency,
    ReplayCleanup, ReplayContinuation, ScopeIdentity, ScopeManifest, SourceIdentity, WriterLease,
    WriterLeaseGuard,
};
use tokenmaster_platform::ExclusiveFileLease;
use tokenmaster_provider::DiscoveryRequest;
use tokenmaster_runtime::{
    CodexAdapter, LiveRuntime, RuntimeErrorCode, RuntimeWriterLease, StagingRecoveryOutcome,
    StoreArchive,
};

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

struct PreserveCompleteStaging {
    inner: StoreArchive,
    fail_continuation: bool,
}

impl Archive for PreserveCompleteStaging {
    fn begin_scan_set(&mut self, manifest: &ScopeManifest) -> Result<ArchiveScanSetId, PortError> {
        self.inner.begin_scan_set(manifest)
    }

    fn observe_source(
        &mut self,
        scan_set: ArchiveScanSetId,
        source: &DiscoveredSource,
        initial_state: &AdapterSourceState,
    ) -> Result<(), PortError> {
        self.inner.observe_source(scan_set, source, initial_state)
    }

    fn finish_scope(
        &mut self,
        scan_set: ArchiveScanSetId,
        scope: &ScopeIdentity,
        completion: AdapterCompletion,
    ) -> Result<(), PortError> {
        self.inner.finish_scope(scan_set, scope, completion)
    }

    fn finish_scan_set(
        &mut self,
        scan_set: ArchiveScanSetId,
    ) -> Result<CompletionQuality, PortError> {
        self.inner.finish_scan_set(scan_set)
    }

    fn begin_replay(&mut self, scan_set: ArchiveScanSetId) -> Result<ArchiveReplay, PortError> {
        self.inner.begin_replay(scan_set)
    }

    fn prepare_replay_source(
        &mut self,
        replay: ArchiveReplay,
        source: &DiscoveredSource,
        initial_state: &AdapterSourceState,
    ) -> Result<ArchiveReplay, PortError> {
        self.inner
            .prepare_replay_source(replay, source, initial_state)
    }

    fn append_replay_batch(
        &mut self,
        replay: ArchiveReplay,
        source: &SourceIdentity,
        batch: CanonicalBatch,
    ) -> Result<ArchiveReplay, PortError> {
        self.inner.append_replay_batch(replay, source, batch)
    }

    fn continue_replay(&mut self, replay: ArchiveReplay) -> Result<ReplayContinuation, PortError> {
        if self.fail_continuation {
            self.fail_continuation = false;
            Err(PortError::new(PortErrorCode::Unavailable))
        } else {
            self.inner.continue_replay(replay)
        }
    }

    fn seal_replay(&mut self, replay: ArchiveReplay) -> Result<ArchiveReplay, PortError> {
        self.inner.seal_replay(replay)
    }

    fn promote_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError> {
        self.inner.promote_replay(replay)
    }

    fn discard_replay(&mut self, _replay: ArchiveReplay) -> Result<(), PortError> {
        Err(PortError::new(PortErrorCode::Unavailable))
    }
}

fn permit() -> (RefreshCoordinator, tokenmaster_engine::RefreshPermit) {
    let mut coordinator = RefreshCoordinator::new();
    let RefreshAdmission::Started(permit) = coordinator
        .submit(
            RefreshUrgency::Recovery,
            None,
            MonotonicTime::from_millis(0),
        )
        .expect("refresh admission")
    else {
        panic!("refresh must start");
    };
    (coordinator, permit)
}
use tokenmaster_store::{
    ArchiveMode, ScanCounters, ScanOutcome, ScanScope, ScanSetManifest, SourceKey, SourceKind,
    SourceRegistration, SourceRegistrationParts, StoredCheckpoint, StoredCheckpointParts,
    StoredVerification, UsageStore,
};

fn request(root: &Path) -> DiscoveryRequest {
    let configured = [ConfiguredCodexRoot::new(root, None, true)];
    build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request")
}

fn manifest() -> ScanSetManifest {
    ScanSetManifest::new(
        vec![ScanScope::new("codex", "fixture").expect("scope")].into_boxed_slice(),
    )
    .expect("manifest")
}

fn registration(seed: u8) -> SourceRegistration {
    SourceRegistration::new(SourceRegistrationParts {
        source_key: SourceKey::from_bytes([seed; 32]),
        provider_id: "codex".into(),
        profile_id: "fixture".into(),
        source_id: "fixture-source".into(),
        source_kind: SourceKind::Active,
        logical_identity: [seed.wrapping_add(1); 32],
        physical_identity: Some([seed; 32]),
        initial_checkpoint: StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some([seed; 32]),
            logical_identity: [seed.wrapping_add(1); 32],
            committed_offset: 0,
            scan_offset: 0,
            observed_file_length: 0,
            modified_time_ns: None,
            anchor_start: 0,
            anchor_len: 0,
            anchor_sha256: [seed.wrapping_add(2); 32],
            resume: Box::default(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::Incremental,
        })
        .expect("checkpoint"),
    })
    .expect("registration")
}

fn complete_scan(store: &mut UsageStore, source: Option<SourceKey>, started: i64) {
    let set = store
        .begin_scan_set(&manifest(), started)
        .expect("begin scan");
    let scan = store.scan_page(set.id(), None, 1).expect("scan page")[0].id();
    if let Some(source) = source {
        store
            .observe_scan_source(scan, source)
            .expect("observe source");
    }
    store
        .finish_scan(
            scan,
            ScanOutcome::Complete,
            started + 1,
            ScanCounters::default(),
        )
        .expect("finish scan");
    store
        .finish_scan_set(set.id(), started + 2)
        .expect("finish scan set");
    store
        .begin_replay_revision_for_scan_set(set.id())
        .expect("begin staging replay");
}

#[test]
fn startup_closes_orphan_running_scan_before_async_admission() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let mut store = UsageStore::open(&archive_path).expect("open store");
    store
        .begin_scan_set(&manifest(), 1)
        .expect("orphan running scan");
    drop(store);

    let mut runtime =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("start with orphan");
    let recovery = runtime.startup_recovery();
    assert_eq!(recovery.orphan_scan_sets_closed(), 1);
    assert_eq!(recovery.orphan_scans_closed(), 1);
    runtime.shutdown().expect("shutdown");
    assert!(
        UsageStore::open(&archive_path)
            .expect("reopen")
            .running_scan_set()
            .expect("running scan query")
            .is_none()
    );
}

#[test]
fn startup_lease_contention_prevents_store_open_and_retries_after_release() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let mut external = RuntimeWriterLease::new(&archive_path).expect("external lease");
    let guard = external.try_acquire().expect("hold external lease");

    let error = LiveRuntime::start(&archive_path, request(source_root.path()))
        .expect_err("contended startup must fail before opening SQLite");
    assert_eq!(error.code(), RuntimeErrorCode::Busy);
    assert!(!archive_path.exists());

    drop(guard);
    let mut runtime = LiveRuntime::start(&archive_path, request(source_root.path()))
        .expect("start after lease release");
    runtime.shutdown().expect("shutdown after retry");
}

#[test]
fn guarded_start_adopts_the_already_held_writer_guard_without_reacquiring() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let guard = ExclusiveFileLease::for_archive(&archive_path)
        .expect("platform lease")
        .try_acquire()
        .expect("startup guard");

    let mut runtime = LiveRuntime::start_guarded(&archive_path, request(source_root.path()), guard)
        .expect("guarded start");
    assert!(archive_path.exists());
    runtime.shutdown().expect("guarded shutdown");
}

#[test]
fn guarded_start_rejects_a_guard_for_another_archive_before_store_open() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let target = archive_root.path().join("usage.sqlite3");
    let other = archive_root.path().join("other.sqlite3");
    let guard = ExclusiveFileLease::for_archive(&other)
        .expect("other platform lease")
        .try_acquire()
        .expect("other startup guard");

    let error = LiveRuntime::start_guarded(&target, request(source_root.path()), guard)
        .expect_err("wrong guard");
    assert_eq!(error.code(), RuntimeErrorCode::StoreUnavailable);
    assert!(!target.exists());
}

#[test]
fn startup_resumes_exact_zero_source_staging_and_discards_incomplete_staging() {
    let source_root = TempDir::new().expect("source root");

    let resumable_root = TempDir::new().expect("resumable archive root");
    let resumable_path = resumable_root.path().join("usage.sqlite3");
    let mut resumable = UsageStore::open(&resumable_path).expect("resumable store");
    complete_scan(&mut resumable, None, 10);
    drop(resumable);
    let mut runtime =
        LiveRuntime::start(&resumable_path, request(source_root.path())).expect("resume staging");
    assert_eq!(
        runtime.startup_recovery().staging(),
        StagingRecoveryOutcome::Resumed
    );
    runtime.shutdown().expect("resume shutdown");
    assert_eq!(
        UsageStore::open(&resumable_path)
            .expect("reopen resumed")
            .archive_state()
            .expect("resumed state")
            .mode(),
        ArchiveMode::ReplayVerified
    );

    let discarded_root = TempDir::new().expect("discarded archive root");
    let discarded_path = discarded_root.path().join("usage.sqlite3");
    let mut discarded = UsageStore::open(&discarded_path).expect("discarded store");
    let registration = registration(7);
    let source_key = SourceKey::from_bytes([7; 32]);
    discarded
        .register_source(&registration)
        .expect("register source");
    complete_scan(&mut discarded, Some(source_key), 20);
    drop(discarded);
    let mut runtime = LiveRuntime::start(&discarded_path, request(source_root.path()))
        .expect("discard incomplete staging");
    assert_eq!(
        runtime.startup_recovery().staging(),
        StagingRecoveryOutcome::Discarded
    );
    runtime.shutdown().expect("discard shutdown");
    assert!(
        !UsageStore::open(&discarded_path)
            .expect("reopen discarded")
            .archive_state()
            .expect("discarded state")
            .rebuild_staging()
    );
}

#[test]
fn startup_resumes_nonempty_staging_after_exact_replay_append() {
    let source_root = TempDir::new().expect("source root");
    std::fs::write(
        source_root.path().join("session.jsonl"),
        "{\"timestamp\":\"2026-07-15T00:00:01Z\",\"model\":\"gpt-5\",\"usage\":{\"input_tokens\":3,\"output_tokens\":2,\"total_tokens\":5}}\n",
    )
    .expect("source fixture");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let mut preserved = PreserveCompleteStaging {
        inner: StoreArchive::new(UsageStore::open(&archive_path).expect("open store")),
        fail_continuation: true,
    };
    let (_coordinator, permit) = permit();
    let result = OneShotExecutor::new().run(
        &permit,
        &FixedClock,
        &mut OpenLease,
        &mut CodexAdapter::new(request(source_root.path())).expect("adapter"),
        &mut preserved,
    );
    assert_eq!(result.outcome(), RefreshOutcome::Failed);
    assert_eq!(result.cleanup(), ReplayCleanup::Failed);
    assert_eq!(result.error(), Some(PortErrorCode::Unavailable));
    drop(preserved);

    let mut runtime = LiveRuntime::start(&archive_path, request(source_root.path()))
        .expect("resume nonempty staging");
    assert_eq!(
        runtime.startup_recovery().staging(),
        StagingRecoveryOutcome::Resumed
    );
    runtime.shutdown().expect("shutdown");
    let store = UsageStore::open(&archive_path).expect("reopen resumed archive");
    assert_eq!(
        store.archive_state().expect("archive state").mode(),
        ArchiveMode::ReplayVerified
    );
    assert_eq!(store.counts().expect("counts").canonical_events(), 1);
}
