use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

#[cfg(windows)]
use rusqlite::{Connection, OpenFlags};
use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterCheckpoint, AdapterCompletion, AdapterSourceProgress,
    AdapterSourceState, Clock, CompletionQuality, DiscoveredSource, MonotonicTime, OneShotExecutor,
    OperationControl, PortError, RefreshAdmission, RefreshCoordinator, RefreshDeadline,
    RefreshOutcome, RefreshUrgency, ReplaySourceSink, ScopeIdentity, ScopeSink, SinkControl,
    SourceBatchReader, SourceSink, WorkerCompletion, WriterLease,
};
use tokenmaster_provider::{DiscoveryRequest, ProviderDescriptor};
use tokenmaster_runtime::{
    CodexAdapter, CodexUsageProviderFactory, GitRepositoryHintIngress, LivePhase,
    LiveProviderAdapter, LiveRefreshKind, LiveRuntime, ProviderWatchRoots, RuntimeError,
    RuntimeWriterLease, StoreArchive, UsageProviderFactory, refresh_incremental,
};
use tokenmaster_store::{ArchivePublicationQuality, StoreErrorCode, UsageStore};

static LIVE_TEST_LOCK: Mutex<()> = Mutex::new(());

struct IsolatedWatchFactory {
    inner: CodexUsageProviderFactory,
    descriptor: ProviderDescriptor,
    watch_roots: ProviderWatchRoots,
}

impl IsolatedWatchFactory {
    fn new(request: DiscoveryRequest, watch_root: &Path) -> Self {
        let inner = CodexUsageProviderFactory::new(request).expect("Codex provider factory");
        let descriptor = inner.descriptor().clone();
        let watch_roots =
            ProviderWatchRoots::try_new(vec![watch_root.to_path_buf()]).expect("watch roots");
        Self {
            inner,
            descriptor,
            watch_roots,
        }
    }
}

impl UsageProviderFactory for IsolatedWatchFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn build(
        self: Box<Self>,
        repository_hints: Option<GitRepositoryHintIngress>,
    ) -> Result<Box<dyn LiveProviderAdapter>, RuntimeError> {
        let inner = Box::new(self.inner).build(repository_hints)?;
        Ok(Box::new(IsolatedWatchAdapter {
            inner,
            watch_roots: self.watch_roots,
        }))
    }
}

struct IsolatedWatchAdapter {
    inner: Box<dyn LiveProviderAdapter>,
    watch_roots: ProviderWatchRoots,
}

impl Adapter for IsolatedWatchAdapter {
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

    fn visit_replay_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        self.inner.visit_replay_sources(scope, control, sink)
    }
}

impl LiveProviderAdapter for IsolatedWatchAdapter {
    fn watch_roots(&self) -> ProviderWatchRoots {
        self.watch_roots.clone()
    }

    fn visit_changed_replay_sources(
        &mut self,
        paths: &[&Path],
        control: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        self.inner
            .visit_changed_replay_sources(paths, control, sink)
    }
}

fn serial() -> MutexGuard<'static, ()> {
    LIVE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn request(root: &Path) -> DiscoveryRequest {
    let configured = [ConfiguredCodexRoot::new(root, None, true)];
    build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request")
}

fn usage_line(second: u8, input: u64) -> String {
    format!(
        "{{\"timestamp\":\"2026-07-15T00:00:{second:02}Z\",\"model\":\"gpt-5\",\"usage\":{{\"input_tokens\":{input},\"output_tokens\":2,\"total_tokens\":{}}}}}\n",
        input + 2
    )
}

fn indexed_usage_line(index: u64) -> String {
    let minute = (index / 60) % 60;
    let second = index % 60;
    let input = index + 3;
    format!(
        "{{\"timestamp\":\"2026-07-15T00:{minute:02}:{second:02}Z\",\"model\":\"gpt-5\",\"usage\":{{\"input_tokens\":{input},\"output_tokens\":2,\"total_tokens\":{}}}}}\n",
        input + 2
    )
}

#[derive(Clone, Copy, Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(10)
    }
}

struct PhaseClock {
    expired: Arc<AtomicBool>,
}

impl Clock for PhaseClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(if self.expired.load(Ordering::Acquire) {
            10
        } else {
            0
        })
    }
}

struct InterruptAfterFirstBatch {
    inner: CodexAdapter,
    expired: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
}

impl Adapter for InterruptAfterFirstBatch {
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

    fn visit_replay_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        let mut interrupting = InterruptingSink {
            inner: sink,
            expired: Arc::clone(&self.expired),
            reads: Arc::clone(&self.reads),
        };
        self.inner
            .visit_replay_sources(scope, control, &mut interrupting)
    }
}

struct InterruptingSink<'a> {
    inner: &'a mut dyn ReplaySourceSink,
    expired: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
}

impl ReplaySourceSink for InterruptingSink<'_> {
    fn on_source(
        &mut self,
        source: DiscoveredSource,
        initial_checkpoint: AdapterSourceState,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        let mut interrupting = InterruptingReader {
            inner: reader,
            expired: Arc::clone(&self.expired),
            reads: Arc::clone(&self.reads),
        };
        self.inner
            .on_source(source, initial_checkpoint, &mut interrupting)
    }
}

struct InterruptingReader<'a> {
    inner: &'a mut dyn SourceBatchReader,
    expired: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
}

impl SourceBatchReader for InterruptingReader<'_> {
    fn restore_checkpoint(
        &mut self,
        progress: &AdapterSourceProgress,
        control: &OperationControl<'_>,
    ) -> Result<AdapterCheckpoint, PortError> {
        self.inner.restore_checkpoint(progress, control)
    }

    fn validate_checkpoint(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<(), PortError> {
        self.inner.validate_checkpoint(checkpoint, control)
    }

    fn read_batch(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        if self.reads.fetch_add(1, Ordering::AcqRel) != 0 {
            self.expired.store(true, Ordering::Release);
        }
        self.inner.read_batch(checkpoint, control)
    }
}

fn recovery_permit(
    deadline: Option<RefreshDeadline>,
) -> (RefreshCoordinator, tokenmaster_engine::RefreshPermit) {
    let mut coordinator = RefreshCoordinator::new();
    let RefreshAdmission::Started(permit) = coordinator
        .submit(
            RefreshUrgency::Recovery,
            deadline,
            MonotonicTime::from_millis(0),
        )
        .expect("refresh admission")
    else {
        panic!("refresh must start");
    };
    (coordinator, permit)
}

fn append(path: &Path, payload: &str) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open source for append");
    file.write_all(payload.as_bytes()).expect("append source");
    file.flush().expect("flush source");
}

fn wait_completion(runtime: &LiveRuntime) -> WorkerCompletion {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(completion) = runtime.try_completion().expect("completion") {
            return completion;
        }
        assert!(Instant::now() < deadline, "live refresh did not complete");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_quiescent(runtime: &LiveRuntime) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut stable_since = None;
    loop {
        while runtime
            .try_completion()
            .expect("drain completion")
            .is_some()
        {}
        let snapshot = runtime.snapshot().expect("live snapshot");
        let quiet = snapshot.worker().active_request_id().is_none()
            && snapshot.worker().pending_count() == 0
            && !snapshot.scheduler().dirty();
        if quiet {
            let since = stable_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_millis(100) {
                return;
            }
        } else {
            stable_since = None;
        }
        assert!(Instant::now() < deadline, "live runtime did not quiesce");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(windows)]
fn atomic_replace(replaced: &Path, replacement: &Path) {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Storage::FileSystem::{REPLACE_FILE_FLAGS, ReplaceFileW};
    use windows::core::PCWSTR;

    let replaced = replaced
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replacement = replacement
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are NUL-terminated and live for this same-directory call.
    unsafe {
        ReplaceFileW(
            PCWSTR(replaced.as_ptr()),
            PCWSTR(replacement.as_ptr()),
            PCWSTR::null(),
            REPLACE_FILE_FLAGS::default(),
            None,
            None,
        )
    }
    .expect("atomically replace source");
}

#[cfg(not(windows))]
fn atomic_replace(replaced: &Path, replacement: &Path) {
    std::fs::rename(replacement, replaced).expect("atomically replace source");
}

#[test]
fn startup_append_new_source_burst_pause_resume_and_reopen_are_live() {
    let _serial = serial();
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let first = source_root.path().join("first.jsonl");
    std::fs::write(&first, usage_line(1, 3)).expect("initial source");
    #[cfg(windows)]
    let resource_baseline = current_resource_counts();

    let mut runtime =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("start live runtime");
    let startup = wait_completion(&runtime);
    assert_eq!(startup.outcome(), RefreshOutcome::Completed);
    wait_quiescent(&runtime);
    let snapshot = runtime.snapshot().expect("startup snapshot");
    assert_eq!(snapshot.phase(), LivePhase::Running);
    assert_eq!(snapshot.watcher().root_count(), 1);
    let startup_timings = runtime
        .last_full_rebuild_timings()
        .expect("startup timings")
        .expect("completed full rebuild timings");
    assert_eq!(startup_timings.total().samples(), 1);

    append(&first, &usage_line(2, 5));
    std::fs::write(source_root.path().join("second.jsonl"), usage_line(3, 7)).expect("new source");
    let hints = runtime.hints();
    for _ in 0..10_000 {
        assert!(hints.filesystem_changed());
    }
    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("interactive refresh");
    let burst_completion = wait_completion(&runtime);
    assert_eq!(
        burst_completion.outcome(),
        RefreshOutcome::Completed,
        "burst completion: {burst_completion:?}, live: {:?}",
        runtime.snapshot()
    );
    wait_quiescent(&runtime);
    assert_eq!(
        runtime
            .last_full_rebuild_timings()
            .expect("retained startup timings"),
        Some(startup_timings),
        "an incremental refresh must not erase the last cold receipt"
    );
    assert!(
        runtime
            .snapshot()
            .expect("burst snapshot")
            .scheduler()
            .accepted_hint_count()
            >= 10_001
    );

    assert_eq!(runtime.pause().expect("pause"), LivePhase::Paused);
    assert!(!hints.filesystem_changed());
    assert_eq!(runtime.resume().expect("resume"), LivePhase::Running);
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let private_debug = format!("{runtime:?}");
    assert!(!private_debug.contains(source_root.path().to_string_lossy().as_ref()));
    assert!(!private_debug.contains(archive_root.path().to_string_lossy().as_ref()));
    assert_eq!(runtime.shutdown().expect("shutdown"), LivePhase::Stopped);

    let store = UsageStore::open(&archive_path).expect("reopen archive");
    assert_eq!(store.counts().expect("counts").canonical_events(), 3);
    assert_eq!(
        store.archive_publication().expect("publication").quality(),
        ArchivePublicationQuality::Complete
    );
    drop(store);

    let mut reopened =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("restart runtime");
    assert_eq!(
        wait_completion(&reopened).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&reopened);
    assert_eq!(
        reopened.shutdown().expect("restart shutdown"),
        LivePhase::Stopped
    );
    assert_eq!(
        UsageStore::open(&archive_path)
            .expect("final reopen")
            .counts()
            .expect("final counts")
            .canonical_events(),
        3
    );
    drop(reopened);
    #[cfg(windows)]
    assert_resources_return(resource_baseline);
}

#[test]
fn live_replacement_and_truncation_rebuild_without_losing_prior_truth() {
    let _serial = serial();
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let source = source_root.path().join("session.jsonl");
    std::fs::write(&source, usage_line(1, 3)).expect("baseline source");

    let mut runtime =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("start live runtime");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);

    assert_eq!(
        runtime.pause().expect("pause for replacement"),
        LivePhase::Paused
    );
    let replacement = source_root.path().join("replacement.jsonl");
    std::fs::write(&replacement, usage_line(2, 5)).expect("replacement source");
    atomic_replace(&source, &replacement);
    assert_eq!(
        runtime.resume().expect("resume replacement"),
        LivePhase::Running
    );
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);

    assert_eq!(
        runtime.pause().expect("pause for truncation"),
        LivePhase::Paused
    );
    std::fs::write(&source, usage_line(3, 7)).expect("truncated source");
    assert_eq!(
        runtime.resume().expect("resume truncation"),
        LivePhase::Running
    );
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    assert_eq!(runtime.shutdown().expect("shutdown"), LivePhase::Stopped);
    drop(runtime);

    let store = UsageStore::open(&archive_path).expect("reopen archive");
    assert_eq!(
        store.archive_publication().expect("publication").quality(),
        ArchivePublicationQuality::Complete
    );
    assert_eq!(
        store
            .event_page_before(None, 10)
            .expect("retained events")
            .len(),
        3
    );
}

#[test]
fn startup_resumes_a_current_partial_publication_across_sources_without_duplicates() {
    let _serial = serial();
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let source = source_root.path().join("session.jsonl");
    let second_source = source_root.path().join("second-session.jsonl");
    std::fs::write(&source, indexed_usage_line(1)).expect("baseline source");
    std::fs::write(&second_source, indexed_usage_line(2)).expect("second baseline source");

    let mut archive = StoreArchive::new(UsageStore::open(&archive_path).expect("archive"));
    let mut lease = RuntimeWriterLease::new(&archive_path).expect("writer lease");
    let mut initial_adapter = CodexAdapter::new(request(source_root.path())).expect("adapter");
    let (_coordinator, permit) = recovery_permit(None);
    let initial = OneShotExecutor::new().run(
        &permit,
        &FixedClock,
        &mut lease,
        &mut initial_adapter,
        &mut archive,
    );
    assert_eq!(initial.outcome(), RefreshOutcome::Completed);
    assert_eq!(initial.quality(), CompletionQuality::Complete);

    append(
        &source,
        &(2..302).map(indexed_usage_line).collect::<String>(),
    );
    append(
        &second_source,
        &(302..602).map(indexed_usage_line).collect::<String>(),
    );
    let expired = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let mut interrupting = InterruptAfterFirstBatch {
        inner: CodexAdapter::new(request(source_root.path())).expect("interrupting adapter"),
        expired: Arc::clone(&expired),
        reads: Arc::clone(&reads),
    };
    let (_coordinator, permit) = recovery_permit(Some(RefreshDeadline::from_millis(5)));
    let clock = PhaseClock { expired };
    let control = OperationControl::new(&permit, &clock);
    let guard = lease.try_acquire().expect("incremental writer lease");
    let error = refresh_incremental(&mut interrupting, &mut archive, &control)
        .expect_err("deadline after first batch");
    drop(guard);
    assert_eq!(
        error.code(),
        tokenmaster_engine::PortErrorCode::DeadlineExceeded
    );
    assert!(reads.load(Ordering::Acquire) >= 2);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("partial publication")
            .quality(),
        ArchivePublicationQuality::Partial
    );
    let partial_events = archive
        .store()
        .counts()
        .expect("partial counts")
        .canonical_events();
    assert!(partial_events > 2 && partial_events < 602);
    drop(archive);
    drop(lease);

    let mut runtime = LiveRuntime::start(&archive_path, request(source_root.path()))
        .expect("resume live runtime");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    assert!(
        runtime
            .snapshot()
            .expect("resumed scheduler snapshot")
            .scheduler()
            .submitted_count()
            >= 2,
        "partial refresh must schedule a coalesced continuation"
    );
    assert_eq!(
        runtime
            .snapshot()
            .expect("resumed snapshot")
            .refresh()
            .kind(),
        LiveRefreshKind::Incremental
    );
    assert_eq!(runtime.shutdown().expect("shutdown"), LivePhase::Stopped);
    drop(runtime);

    let store = UsageStore::open(&archive_path).expect("reopen resumed archive");
    assert_eq!(
        store.archive_publication().expect("publication").quality(),
        ArchivePublicationQuality::Complete
    );
    assert_eq!(store.counts().expect("counts").canonical_events(), 602);
}

#[cfg(windows)]
#[test]
#[ignore = "release-only real Codex history cold-import acceptance"]
fn real_codex_history_cold_import_completes_and_shuts_down_cleanly() {
    let _serial = serial();
    let source_root = std::env::var_os("TOKENMASTER_REAL_CODEX_HOME")
        .map(std::path::PathBuf::from)
        .expect("TOKENMASTER_REAL_CODEX_HOME");
    assert!(
        source_root.is_absolute() && source_root.is_dir(),
        "real Codex root must be an existing absolute directory"
    );
    let timeout_seconds = std::env::var("TOKENMASTER_REAL_IMPORT_TIMEOUT_SECONDS")
        .ok()
        .map(|value| value.parse::<u64>().expect("real import timeout"))
        .unwrap_or(3_600);
    assert!(
        (60..=3_600).contains(&timeout_seconds),
        "real import timeout must remain between 60 and 3600 seconds"
    );
    let idle_seconds = std::env::var("TOKENMASTER_REAL_IDLE_SECONDS")
        .ok()
        .map(|value| value.parse::<u64>().expect("real idle seconds"))
        .unwrap_or(0);
    assert!(
        matches!(idle_seconds, 0 | 600),
        "real idle receipt must be disabled or exactly 600 seconds"
    );

    let archive_root = TempDir::new().expect("real import archive root");
    let isolated_watch_root = TempDir::new().expect("isolated idle watch root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let baseline_private_bytes = current_private_bytes();
    let mut peak_private_bytes = baseline_private_bytes;
    let started = Instant::now();
    let deadline = started + Duration::from_secs(timeout_seconds);
    let mut completion_count = 0_u64;
    let factory = IsolatedWatchFactory::new(request(&source_root), isolated_watch_root.path());
    let mut runtime = LiveRuntime::start_with_provider(&archive_path, Box::new(factory))
        .expect("start real import runtime");

    let (elapsed, final_events, final_archive_bytes, final_timings) = loop {
        while let Some(completion) = runtime.try_completion().expect("real import completion") {
            completion_count = completion_count.checked_add(1).expect("completion count");
            eprintln!(
                "TM-REAL-IMPORT-COMPLETION|count={completion_count}|outcome={:?}",
                completion.outcome()
            );
        }
        let snapshot = runtime.snapshot().expect("real import snapshot");
        let store = open_real_import_store(&archive_path, started + Duration::from_secs(5));
        let publication = store
            .archive_publication()
            .expect("real import publication");
        let event_count = store
            .counts()
            .expect("real import counts")
            .canonical_events();
        drop(store);
        let diagnostics = real_import_diagnostics(&archive_path);
        peak_private_bytes = peak_private_bytes.max(current_private_bytes());
        let archive_bytes = archive_footprint_bytes(&archive_path);
        let elapsed = started.elapsed();
        eprintln!(
            "TM-REAL-IMPORT-PROGRESS|elapsed_s={}|quality={:?}|events={event_count}|archive_bytes={archive_bytes}|private_bytes={peak_private_bytes}|completions={completion_count}|sources_pending={}|sources_complete={}|observations={}|sessions={}|work_classify={}|work_children={}",
            elapsed.as_secs(),
            publication.quality(),
            diagnostics.0,
            diagnostics.1,
            diagnostics.2,
            diagnostics.3,
            diagnostics.4,
            diagnostics.5,
        );

        if publication.quality() == ArchivePublicationQuality::Complete
            && let Some(timings) = runtime
                .last_full_rebuild_timings()
                .expect("last full rebuild timings")
        {
            break (elapsed, event_count, archive_bytes, timings);
        }
        if Instant::now() >= deadline {
            runtime.shutdown().expect("timeout shutdown");
            panic!(
                "real import did not complete within {timeout_seconds} seconds: quality={:?}, events={event_count}, archive_bytes={archive_bytes}, private_bytes={peak_private_bytes}, snapshot={snapshot:?}",
                publication.quality()
            );
        }
        std::thread::sleep(Duration::from_secs(15));
    };

    if idle_seconds != 0 {
        wait_quiescent(&runtime);
        let idle_before_runtime = runtime.snapshot().expect("idle start snapshot");
        let idle_before = current_idle_sample();
        std::thread::sleep(Duration::from_secs(idle_seconds));
        let idle_after = current_idle_sample();
        let idle_after_runtime = runtime.snapshot().expect("idle end snapshot");
        let cpu_percent = idle_cpu_percent(idle_before, idle_after);
        let read_bytes = idle_after
            .read_bytes
            .checked_sub(idle_before.read_bytes)
            .expect("idle read counter order");
        let write_bytes = idle_after
            .write_bytes
            .checked_sub(idle_before.write_bytes)
            .expect("idle write counter order");

        assert_eq!(
            idle_after_runtime.engine().archive_generation(),
            idle_before_runtime.engine().archive_generation(),
            "idle archive generation must remain unchanged"
        );
        assert_eq!(
            idle_after_runtime.scheduler().submitted_count(),
            idle_before_runtime.scheduler().submitted_count(),
            "idle scheduler must not submit refresh work"
        );
        assert!(!idle_after_runtime.scheduler().dirty());
        assert!(idle_after_runtime.worker().active_request_id().is_none());
        assert_eq!(idle_after_runtime.worker().pending_count(), 0);
        assert!(
            cpu_percent < 0.5,
            "idle CPU {cpu_percent:.4}% must remain below 0.5%"
        );
        assert!(
            read_bytes <= 1_048_576,
            "idle process reads {read_bytes} bytes exceed the fixed 1 MiB noise ceiling"
        );
        assert!(
            write_bytes <= 1_048_576,
            "idle process writes {write_bytes} bytes exceed the fixed 1 MiB noise ceiling"
        );
        assert!(
            idle_after.private_bytes <= 64 * 1024 * 1024,
            "idle private memory {} bytes exceeds 64 MiB",
            idle_after.private_bytes
        );
        println!(
            "TM-REAL-IDLE-RECEIPT|seconds={idle_seconds}|cpu_percent={cpu_percent:.4}|read_bytes={read_bytes}|write_bytes={write_bytes}|private_bytes={}|archive_generation={}|scheduler_submissions={}",
            idle_after.private_bytes,
            idle_after_runtime.engine().archive_generation(),
            idle_after_runtime.scheduler().submitted_count(),
        );
    }

    assert_eq!(
        runtime.shutdown().expect("real import shutdown"),
        LivePhase::Stopped
    );
    let store = UsageStore::open_current(&archive_path).expect("reopen completed real import");
    assert_eq!(
        store
            .archive_publication()
            .expect("final real import publication")
            .quality(),
        ArchivePublicationQuality::Complete
    );
    assert_eq!(
        store
            .counts()
            .expect("final real import counts")
            .canonical_events(),
        final_events
    );
    drop(store);
    let final_private_bytes = current_private_bytes();
    println!(
        "TM-COLD-STAGE-RECEIPT|total_ns={}|discovery_ns={}|discovery_samples={}|read_parse_ns={}|read_parse_samples={}|canonicalize_ns={}|canonicalize_samples={}|fact_write_ns={}|fact_write_samples={}|project_ns={}|project_samples={}|checkpoint_ns={}|checkpoint_samples={}",
        final_timings.total().elapsed_nanos(),
        final_timings.discovery().elapsed_nanos(),
        final_timings.discovery().samples(),
        final_timings.read_parse().elapsed_nanos(),
        final_timings.read_parse().samples(),
        final_timings.canonicalize().elapsed_nanos(),
        final_timings.canonicalize().samples(),
        final_timings.fact_write().elapsed_nanos(),
        final_timings.fact_write().samples(),
        final_timings.project().elapsed_nanos(),
        final_timings.project().samples(),
        final_timings.checkpoint().elapsed_nanos(),
        final_timings.checkpoint().samples(),
    );
    println!(
        "TM-REAL-IMPORT-RECEIPT|elapsed_ms={}|events={final_events}|archive_bytes={final_archive_bytes}|baseline_private_bytes={baseline_private_bytes}|peak_private_bytes={peak_private_bytes}|final_private_bytes={final_private_bytes}|completions={completion_count}",
        elapsed.as_millis()
    );
}

#[cfg(windows)]
fn open_real_import_store(path: &Path, migration_deadline: Instant) -> UsageStore {
    loop {
        match UsageStore::open_current(path) {
            Ok(store) => return store,
            Err(error)
                if error.code() == StoreErrorCode::SchemaMismatch
                    && Instant::now() < migration_deadline =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("open real import archive: {error:?}"),
        }
    }
}

#[cfg(windows)]
fn real_import_diagnostics(path: &Path) -> (u64, u64, u64, u64, u64, u64) {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("open real import diagnostics");
    let scalar = |sql| {
        let value = connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))
            .expect("read real import diagnostic");
        u64::try_from(value).expect("non-negative real import diagnostic")
    };
    (
        scalar("SELECT count(*) FROM usage_replay_source WHERE state = 'pending'"),
        scalar("SELECT count(*) FROM usage_replay_source WHERE state = 'complete'"),
        scalar("SELECT count(*) FROM usage_replay_observation"),
        scalar("SELECT count(*) FROM usage_replay_session"),
        scalar("SELECT count(*) FROM usage_replay_work WHERE work_kind = 'classify_session'"),
        scalar("SELECT count(*) FROM usage_replay_work WHERE work_kind = 'scan_children'"),
    )
}

#[cfg(windows)]
fn assert_resources_return(baseline: (u32, u32)) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let current = current_resource_counts();
        if current.0 <= baseline.0 && current.1 <= baseline.1 {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "live resources did not return to baseline: handles {}->{}, threads {}->{}",
            baseline.0,
            current.0,
            baseline.1,
            current.1
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn archive_footprint_bytes(archive_path: &Path) -> u64 {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let mut path = archive_path.as_os_str().to_os_string();
            path.push(suffix);
            std::fs::metadata(path).map_or(0, |metadata| metadata.len())
        })
        .sum()
}

#[cfg(windows)]
fn current_private_bytes() -> usize {
    use std::mem::size_of;

    use windows::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).expect("counter size"),
        ..Default::default()
    };
    // SAFETY: the pointer targets a live, correctly sized counters value and the
    // current-process pseudo-handle remains valid for the duration of the call.
    unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
            memory.cb,
        )
    }
    .expect("process memory");
    memory.PrivateUsage
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct IdleProcessSample {
    monotonic: Instant,
    private_bytes: usize,
    kernel_time_100ns: u64,
    user_time_100ns: u64,
    read_bytes: u64,
    write_bytes: u64,
}

#[cfg(windows)]
fn current_idle_sample() -> IdleProcessSample {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetProcessIoCounters, GetProcessTimes, IO_COUNTERS,
    };

    let process = unsafe { GetCurrentProcess() };
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(
            process,
            &raw mut creation,
            &raw mut exit,
            &raw mut kernel,
            &raw mut user,
        )
    }
    .expect("process times");
    let mut io = IO_COUNTERS::default();
    unsafe { GetProcessIoCounters(process, &raw mut io) }.expect("process I/O counters");
    IdleProcessSample {
        monotonic: Instant::now(),
        private_bytes: current_private_bytes(),
        kernel_time_100ns: filetime_ticks(kernel),
        user_time_100ns: filetime_ticks(user),
        read_bytes: io.ReadTransferCount,
        write_bytes: io.WriteTransferCount,
    }
}

#[cfg(windows)]
fn idle_cpu_percent(before: IdleProcessSample, after: IdleProcessSample) -> f64 {
    let elapsed = after
        .monotonic
        .duration_since(before.monotonic)
        .as_secs_f64();
    assert!(elapsed > 0.0, "idle elapsed time must be positive");
    let kernel = after
        .kernel_time_100ns
        .checked_sub(before.kernel_time_100ns)
        .expect("kernel time order");
    let user = after
        .user_time_100ns
        .checked_sub(before.user_time_100ns)
        .expect("user time order");
    let processors = std::thread::available_parallelism()
        .expect("available processors")
        .get() as f64;
    ((kernel + user) as f64 / 10_000_000.0) * 100.0 / elapsed / processors
}

#[cfg(windows)]
const fn filetime_ticks(value: windows::Win32::Foundation::FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}

#[cfg(windows)]
fn current_resource_counts() -> (u32, u32) {
    use std::mem::size_of;

    use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetProcessHandleCount,
    };
    use windows::core::HRESULT;

    let mut handles = 0_u32;
    // SAFETY: the count points to writable storage and the process pseudo-handle is valid.
    unsafe { GetProcessHandleCount(GetCurrentProcess(), &raw mut handles) }
        .expect("process handles");
    // SAFETY: fixed flags and no borrowed pointers; the returned owned handle is closed below.
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.expect("thread snapshot");
    // SAFETY: this call takes no pointers and returns this process's numeric identifier.
    let process_id = unsafe { GetCurrentProcessId() };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).expect("thread entry size"),
        ..Default::default()
    };
    let mut threads = 0_u32;
    // SAFETY: the entry is correctly sized and writable; the snapshot remains open.
    let first = unsafe { Thread32First(snapshot, &raw mut entry) };
    if first.is_ok() {
        loop {
            if entry.th32OwnerProcessID == process_id {
                threads = threads.checked_add(1).expect("thread count");
            }
            // SAFETY: same live snapshot and writable entry as the first call.
            match unsafe { Thread32Next(snapshot, &raw mut entry) } {
                Ok(()) => {}
                Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) => break,
                Err(error) => panic!("thread enumeration failed: {error}"),
            }
        }
    } else if first
        .as_ref()
        .is_err_and(|error| error.code() != HRESULT::from_win32(ERROR_NO_MORE_FILES.0))
    {
        panic!("thread enumeration failed");
    }
    // SAFETY: the snapshot is the owned live handle and is closed exactly once.
    unsafe { CloseHandle(snapshot) }.expect("close thread snapshot");
    (handles, threads)
}
