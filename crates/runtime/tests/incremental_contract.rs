use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterCheckpoint, AdapterCompletion, AdapterSourceProgress,
    AdapterSourceState, Clock, CompletionQuality, DiscoveredSource, MonotonicTime, OneShotExecutor,
    OneShotResult, OperationControl, PortError, RefreshAdmission, RefreshCoordinator,
    RefreshDeadline, RefreshOutcome, RefreshUrgency, ReplaySourceSink, ScopeIdentity, ScopeSink,
    SinkControl, SourceBatchReader, SourceSink, WriterLease, WriterLeaseGuard,
};
use tokenmaster_runtime::{
    CodexAdapter, IncrementalRefreshOutcome, IncrementalRefreshReport, StoreArchive,
    refresh_incremental,
};
use tokenmaster_store::{ArchivePublicationQuality, UsageStore};

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

struct OpenLease;
struct OpenGuard;

impl WriterLeaseGuard for OpenGuard {}

impl WriterLease for OpenLease {
    fn try_acquire(&mut self) -> Result<Box<dyn WriterLeaseGuard>, tokenmaster_engine::PortError> {
        Ok(Box::new(OpenGuard))
    }
}

fn adapter(root: &Path) -> CodexAdapter {
    adapter_roots(&[root])
}

fn adapter_roots(roots: &[&Path]) -> CodexAdapter {
    let configured = roots
        .iter()
        .map(|root| ConfiguredCodexRoot::new(*root, None, true))
        .collect::<Vec<_>>();
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request");
    CodexAdapter::new(request).expect("Codex adapter")
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

fn deadline_permit() -> (RefreshCoordinator, tokenmaster_engine::RefreshPermit) {
    let mut coordinator = RefreshCoordinator::new();
    let RefreshAdmission::Started(permit) = coordinator
        .submit(
            RefreshUrgency::Recovery,
            Some(RefreshDeadline::from_millis(5)),
            MonotonicTime::from_millis(0),
        )
        .expect("refresh admission")
    else {
        panic!("refresh must start");
    };
    (coordinator, permit)
}

fn bootstrap(root: &Path, archive: &mut StoreArchive) -> OneShotResult {
    bootstrap_with_adapter(&mut adapter(root), archive)
}

fn bootstrap_with_adapter(adapter: &mut dyn Adapter, archive: &mut StoreArchive) -> OneShotResult {
    let (_coordinator, permit) = permit();
    OneShotExecutor::new().run(&permit, &FixedClock, &mut OpenLease, adapter, archive)
}

fn incremental(root: &Path, archive: &mut StoreArchive) -> IncrementalRefreshReport {
    let (_coordinator, permit) = permit();
    let control = OperationControl::new(&permit, &FixedClock);
    refresh_incremental(&mut adapter(root), archive, &control).expect("incremental refresh")
}

fn usage_line(index: u64) -> String {
    let minute = (index / 60) % 60;
    let second = index % 60;
    let input = index + 3;
    format!(
        "{{\"timestamp\":\"2026-07-15T00:{minute:02}:{second:02}Z\",\"model\":\"gpt-5\",\"usage\":{{\"input_tokens\":{input},\"output_tokens\":2,\"total_tokens\":{}}}}}\n",
        input + 2
    )
}

fn append(path: &Path, payload: &str) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open source for append");
    file.write_all(payload.as_bytes()).expect("append source");
    file.flush().expect("flush source");
}

fn assert_bootstrapped(result: OneShotResult) {
    assert_eq!(result.outcome(), RefreshOutcome::Completed);
    assert_eq!(result.quality(), CompletionQuality::Complete);
    assert!(result.published_revision_id().is_some());
    assert!(result.error().is_none());
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
    // SAFETY: both path buffers are NUL-terminated and remain alive for the call.
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
    .expect("atomically replace fixture");
}

#[cfg(not(windows))]
fn atomic_replace(replaced: &Path, replacement: &Path) {
    std::fs::rename(replacement, replaced).expect("atomically replace fixture");
}

#[test]
fn unchanged_refresh_reads_no_payload_and_only_advances_freshness() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    let before = archive.store().archive_publication().expect("publication");

    let report = incremental(root.path(), &mut archive);

    assert_eq!(report.outcome(), IncrementalRefreshOutcome::Complete);
    assert_eq!(report.files_examined(), 1);
    assert_eq!(report.bytes_read(), 0);
    assert_eq!(report.events_observed(), 0);
    assert_eq!(report.batches_committed(), 0);
    assert_eq!(report.diagnostics(), 0);
    assert_eq!(report.archive_generation(), before.generation().get() + 1);
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        1
    );
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("publication")
            .quality(),
        ArchivePublicationQuality::Complete
    );
}

#[test]
fn one_line_append_reads_and_commits_only_the_tail_then_restarts_cleanly() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    let tail = usage_line(2);
    append(&path, &tail);

    let report = incremental(root.path(), &mut archive);

    assert_eq!(report.outcome(), IncrementalRefreshOutcome::Complete);
    assert_eq!(report.files_examined(), 1);
    assert_eq!(report.bytes_read(), tail.len() as u64);
    assert_eq!(report.events_observed(), 1);
    assert_eq!(report.batches_committed(), 1);
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        2
    );

    let restart = incremental(root.path(), &mut archive);
    assert_eq!(restart.outcome(), IncrementalRefreshOutcome::Complete);
    assert_eq!(restart.bytes_read(), 0);
    assert_eq!(restart.events_observed(), 0);
    assert_eq!(restart.batches_committed(), 0);
}

#[test]
fn large_append_is_applied_as_multiple_bounded_batches() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    let tail = (2..302).map(usage_line).collect::<String>();
    append(&path, &tail);

    let report = incremental(root.path(), &mut archive);

    assert_eq!(report.outcome(), IncrementalRefreshOutcome::Complete);
    assert_eq!(report.bytes_read(), tail.len() as u64);
    assert_eq!(report.events_observed(), 300);
    assert!(report.batches_committed() > 1);
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        301
    );
}

#[test]
fn replacement_durably_requests_rebuild_without_changing_canonical_usage() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));

    let replacement = root.path().join("replacement.jsonl");
    std::fs::write(&replacement, usage_line(2)).expect("write replacement");
    atomic_replace(&path, &replacement);
    let replaced = incremental(root.path(), &mut archive);
    assert_eq!(
        replaced.outcome(),
        IncrementalRefreshOutcome::RebuildRequired
    );
    assert_eq!(replaced.bytes_read(), 0);
    assert_eq!(replaced.batches_committed(), 0);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("publication")
            .quality(),
        ArchivePublicationQuality::RecoveryPending
    );
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        1
    );

    let generation = replaced.archive_generation();
    let repeated = incremental(root.path(), &mut archive);
    assert_eq!(
        repeated.outcome(),
        IncrementalRefreshOutcome::RebuildRequired
    );
    assert_eq!(repeated.archive_generation(), generation);
}

#[test]
fn truncation_durably_requests_rebuild_without_changing_canonical_usage() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));

    std::fs::write(&path, "").expect("truncate source");
    let report = incremental(root.path(), &mut archive);

    assert_eq!(report.outcome(), IncrementalRefreshOutcome::RebuildRequired);
    assert_eq!(report.bytes_read(), 0);
    assert_eq!(report.batches_committed(), 0);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("publication")
            .quality(),
        ArchivePublicationQuality::RecoveryPending
    );
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        1
    );
}

#[test]
fn exact_scan_admits_new_sources_but_retains_missing_historical_sources() {
    let root = TempDir::new().expect("source root");
    let first = root.path().join("first.jsonl");
    let second = root.path().join("second.jsonl");
    let third = root.path().join("third.jsonl");
    let empty = root.path().join("empty.jsonl");
    std::fs::write(&first, usage_line(1)).expect("write first source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    std::fs::write(&second, usage_line(2)).expect("write second source");
    std::fs::write(&third, usage_line(3)).expect("write third source");
    std::fs::write(&empty, "").expect("write empty source");

    let admitted = incremental(root.path(), &mut archive);
    assert_eq!(admitted.outcome(), IncrementalRefreshOutcome::Complete);
    assert_eq!(admitted.files_examined(), 4);
    assert_eq!(admitted.events_observed(), 2);
    let counts = archive.store().counts().expect("admitted counts");
    assert_eq!(counts.sources(), 4);
    assert_eq!(counts.canonical_events(), 3);

    std::fs::remove_file(&first).expect("remove historical source");
    let missing = incremental(root.path(), &mut archive);
    assert_eq!(missing.outcome(), IncrementalRefreshOutcome::Complete);
    assert_eq!(missing.files_examined(), 3);
    assert_eq!(missing.bytes_read(), 0);
    let counts = archive.store().counts().expect("retained counts");
    assert_eq!(counts.sources(), 4);
    assert_eq!(counts.canonical_events(), 3);
}

#[test]
fn exact_scan_applies_existing_tail_and_new_source_in_one_publication() {
    let root = TempDir::new().expect("source root");
    let first = root.path().join("first.jsonl");
    std::fs::write(&first, usage_line(1)).expect("write first source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));

    append(&first, &usage_line(2));
    std::fs::write(root.path().join("second.jsonl"), usage_line(3)).expect("write new source");
    let (_coordinator, permit) = permit();
    let control = OperationControl::new(&permit, &FixedClock);
    let result = refresh_incremental(&mut adapter(root.path()), &mut archive, &control);
    assert!(
        result.is_ok(),
        "combined refresh: {result:?}, publication: {:?}, counts: {:?}, revision: {:?}",
        archive.store().archive_publication(),
        archive.store().counts(),
        archive.store().current_replay_revision()
    );
    let report = result.expect("combined incremental refresh");

    assert_eq!(report.outcome(), IncrementalRefreshOutcome::Complete);
    let counts = archive.store().counts().expect("combined counts");
    assert_eq!(counts.sources(), 2);
    assert_eq!(counts.canonical_events(), 3);
}

#[test]
fn full_rebuild_admits_a_source_added_after_the_current_revision() {
    let root = TempDir::new().expect("source root");
    std::fs::write(root.path().join("first.jsonl"), usage_line(1)).expect("write first source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    std::fs::write(root.path().join("second.jsonl"), usage_line(2)).expect("write second source");

    let rebuilt = bootstrap(root.path(), &mut archive);

    assert_bootstrapped(rebuilt);
    let counts = archive.store().counts().expect("rebuilt counts");
    assert_eq!(counts.sources(), 2);
    assert_eq!(counts.canonical_events(), 2);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("publication")
            .quality(),
        ArchivePublicationQuality::Complete
    );
}

#[test]
fn changed_profile_scope_requests_rebuild_and_full_rebuild_recovers_provisional_source() {
    let first_root = TempDir::new().expect("first source root");
    let second_root = TempDir::new().expect("second source root");
    std::fs::write(first_root.path().join("first.jsonl"), usage_line(1))
        .expect("write first source");
    std::fs::write(second_root.path().join("second.jsonl"), usage_line(2))
        .expect("write second source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(first_root.path(), &mut archive));

    let mut expanded_adapter = adapter_roots(&[first_root.path(), second_root.path()]);
    let (_coordinator, permit) = permit();
    let control = OperationControl::new(&permit, &FixedClock);
    let refresh = refresh_incremental(&mut expanded_adapter, &mut archive, &control)
        .expect("scope-changing refresh");

    assert_eq!(
        refresh.outcome(),
        IncrementalRefreshOutcome::RebuildRequired
    );
    assert_eq!(refresh.bytes_read(), 0);
    assert_eq!(refresh.batches_committed(), 0);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("recovery publication")
            .quality(),
        ArchivePublicationQuality::RecoveryPending
    );
    assert_eq!(
        archive
            .store()
            .counts()
            .expect("preserved counts")
            .canonical_events(),
        1
    );

    let rebuilt = bootstrap_with_adapter(
        &mut adapter_roots(&[first_root.path(), second_root.path()]),
        &mut archive,
    );

    assert_bootstrapped(rebuilt);
    let counts = archive.store().counts().expect("rebuilt counts");
    assert_eq!(counts.sources(), 2);
    assert_eq!(counts.canonical_events(), 2);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("rebuilt publication")
            .quality(),
        ArchivePublicationQuality::Complete
    );
}

#[test]
fn excessive_new_source_admission_requests_rebuild_before_exceeding_retained_bounds() {
    let root = TempDir::new().expect("source root");
    std::fs::write(root.path().join("baseline.jsonl"), usage_line(1))
        .expect("write baseline source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    for index in 0..=tokenmaster_store::MAX_REPLAY_SOURCES {
        std::fs::write(root.path().join(format!("new-{index:03}.jsonl")), "")
            .expect("write new source");
    }

    let mut expanded_adapter = adapter(root.path());
    let (_coordinator, permit) = permit();
    let control = OperationControl::new(&permit, &FixedClock);
    let refresh = refresh_incremental(&mut expanded_adapter, &mut archive, &control)
        .expect("bounded refresh outcome");

    assert_eq!(
        refresh.outcome(),
        IncrementalRefreshOutcome::RebuildRequired
    );
    assert_eq!(refresh.bytes_read(), 0);
    assert_eq!(refresh.batches_committed(), 0);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("recovery publication")
            .quality(),
        ArchivePublicationQuality::RecoveryPending
    );
    assert_eq!(
        archive
            .store()
            .counts()
            .expect("preserved counts")
            .canonical_events(),
        1
    );

    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    let counts = archive.store().counts().expect("rebuilt counts");
    assert_eq!(counts.sources(), 258);
    assert_eq!(counts.canonical_events(), 1);
}

#[test]
fn cancellation_before_incremental_work_preserves_the_exact_publication() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    let before = archive.store().archive_publication().expect("publication");
    let (mut coordinator, permit) = permit();
    coordinator.cancel(permit.id()).expect("cancel refresh");
    let control = OperationControl::new(&permit, &FixedClock);

    let error = refresh_incremental(&mut adapter(root.path()), &mut archive, &control)
        .expect_err("cancelled refresh");

    assert_eq!(error.code(), tokenmaster_engine::PortErrorCode::Cancelled);
    assert_eq!(
        archive.store().archive_publication().expect("publication"),
        before
    );
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        1
    );
}

#[test]
fn deadline_after_first_batch_leaves_resumable_partial_state_without_duplicates() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_bootstrapped(bootstrap(root.path(), &mut archive));
    let tail = (2..302).map(usage_line).collect::<String>();
    append(&path, &tail);
    let expired = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let mut interrupting = InterruptAfterFirstBatch {
        inner: adapter(root.path()),
        expired: Arc::clone(&expired),
        reads: Arc::clone(&reads),
    };
    let (_coordinator, permit) = deadline_permit();
    let clock = PhaseClock { expired };
    let control = OperationControl::new(&permit, &clock);

    let error = refresh_incremental(&mut interrupting, &mut archive, &control)
        .expect_err("deadline after first batch");

    assert_eq!(
        error.code(),
        tokenmaster_engine::PortErrorCode::DeadlineExceeded
    );
    assert!(reads.load(Ordering::Acquire) >= 2);
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("publication")
            .quality(),
        ArchivePublicationQuality::Partial
    );
    let partial_events = archive
        .store()
        .counts()
        .expect("partial counts")
        .canonical_events();
    assert!(partial_events > 1 && partial_events < 301);

    let resumed = incremental(root.path(), &mut archive);
    assert_eq!(resumed.outcome(), IncrementalRefreshOutcome::Complete);
    assert!(resumed.events_observed() > 0);
    assert_eq!(
        archive
            .store()
            .counts()
            .expect("resumed counts")
            .canonical_events(),
        301
    );
    assert_eq!(
        archive
            .store()
            .archive_publication()
            .expect("publication")
            .quality(),
        ArchivePublicationQuality::Complete
    );
}
