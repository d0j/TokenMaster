use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterCheckpoint, AdapterCompletion, AdapterSourceProgress,
    AdapterSourceState, Clock, CompletionQuality, DiscoveredSource, MonotonicTime, OneShotExecutor,
    OperationControl, PortError, RefreshAdmission, RefreshCoordinator, RefreshDeadline,
    RefreshOutcome, RefreshUrgency, ReplaySourceSink, ScopeIdentity, ScopeSink, SinkControl,
    SourceBatchReader, SourceSink, WorkerCompletion, WriterLease,
};
use tokenmaster_provider::DiscoveryRequest;
use tokenmaster_runtime::{
    CodexAdapter, LivePhase, LiveRefreshKind, LiveRuntime, RuntimeWriterLease, StoreArchive,
    refresh_incremental,
};
use tokenmaster_store::{ArchivePublicationQuality, UsageStore};

static LIVE_TEST_LOCK: Mutex<()> = Mutex::new(());

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
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(completion) = runtime.try_completion().expect("completion") {
            return completion;
        }
        assert!(Instant::now() < deadline, "live refresh did not complete");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_quiescent(runtime: &LiveRuntime) {
    let deadline = Instant::now() + Duration::from_secs(10);
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
fn startup_resumes_a_current_partial_publication_without_duplicates() {
    let _serial = serial();
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let source = source_root.path().join("session.jsonl");
    std::fs::write(&source, indexed_usage_line(1)).expect("baseline source");

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
    assert!(partial_events > 1 && partial_events < 301);
    drop(archive);
    drop(lease);

    let mut runtime = LiveRuntime::start(&archive_path, request(source_root.path()))
        .expect("resume live runtime");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
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
    assert_eq!(store.counts().expect("counts").canonical_events(), 301);
}

#[cfg(windows)]
fn assert_resources_return(baseline: (u32, u32)) {
    let deadline = Instant::now() + Duration::from_secs(5);
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
