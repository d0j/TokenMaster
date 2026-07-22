use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{
    AdapterCompletion, AdapterSourceState, Archive, ArchiveReplay, ArchiveScanSetId,
    CanonicalBatch, Clock, CompletionQuality, DiscoveredSource, MonotonicTime, OneShotExecutor,
    OneShotResult, PortError, RefreshAdmission, RefreshCoordinator, RefreshDeadline,
    RefreshOutcome, RefreshUrgency, ReplayContinuation, ScopeIdentity, ScopeManifest,
    SourceIdentity, WriterLease, WriterLeaseGuard,
};
use tokenmaster_runtime::{CodexAdapter, StoreArchive};
use tokenmaster_store::{ArchiveMode, UsageStore};

#[derive(Clone, Copy, Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(10)
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

struct ExpireAfterReplayBegin {
    inner: StoreArchive,
    expired: Arc<AtomicBool>,
}

impl Archive for ExpireAfterReplayBegin {
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
        let replay = self.inner.begin_replay(scan_set)?;
        self.expired.store(true, Ordering::Release);
        Ok(replay)
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
        self.inner.continue_replay(replay)
    }

    fn seal_replay(&mut self, replay: ArchiveReplay) -> Result<ArchiveReplay, PortError> {
        self.inner.seal_replay(replay)
    }

    fn promote_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError> {
        self.inner.promote_replay(replay)
    }

    fn discard_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError> {
        self.inner.discard_replay(replay)
    }
}

fn adapter(root: &Path) -> CodexAdapter {
    let configured = [ConfiguredCodexRoot::new(root, None, true)];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request");
    CodexAdapter::new(request).expect("Codex adapter")
}

fn run(root: &Path, archive: &mut StoreArchive) -> OneShotResult {
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
    OneShotExecutor::new().run(
        &permit,
        &FixedClock,
        &mut OpenLease,
        &mut adapter(root),
        archive,
    )
}

fn usage_line(second: u8, input: u64) -> String {
    format!(
        "{{\"timestamp\":\"2026-07-15T00:00:{second:02}Z\",\"model\":\"gpt-5\",\"usage\":{{\"input_tokens\":{input},\"output_tokens\":2,\"total_tokens\":{}}}}}\n",
        input + 2
    )
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
    // SAFETY: both buffers are NUL-terminated and remain alive for the call;
    // both paths are same-directory temporary test fixtures.
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

fn assert_published(result: OneShotResult) {
    assert_eq!(
        result.outcome(),
        RefreshOutcome::Completed,
        "bootstrap result: {result:?}"
    );
    assert_eq!(result.quality(), CompletionQuality::Complete);
    assert!(result.published_revision_id().is_some());
    assert!(result.error().is_none());
}

#[test]
fn real_codex_bootstrap_publishes_and_reopens_without_private_debug_data() {
    let root = TempDir::new().expect("source root");
    let database = root.path().join("tokenmaster.sqlite3");
    let private_name = "PRIVATE_SESSION_NAME.jsonl";
    std::fs::write(root.path().join(private_name), usage_line(1, 3)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::open(&database).expect("open store"));
    let adapter = adapter(root.path());

    let debug = format!("{adapter:?} {archive:?}");
    assert!(!debug.contains(private_name));
    assert!(!debug.contains(root.path().to_string_lossy().as_ref()));
    assert_published(run(root.path(), &mut archive));
    assert_eq!(archive.store().counts().expect("counts").sources(), 1);
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        1
    );
    assert_eq!(
        archive.store().archive_state().expect("state").mode(),
        ArchiveMode::ReplayVerified
    );
    drop(archive);

    let reopened = UsageStore::open(&database).expect("reopen store");
    assert_eq!(
        reopened
            .counts()
            .expect("reopened counts")
            .canonical_events(),
        1
    );
    assert_eq!(
        reopened.archive_state().expect("reopened state").mode(),
        ArchiveMode::ReplayVerified
    );
}

#[test]
fn bootstrap_streams_three_hundred_logical_files_sharing_one_source_id() {
    let root = TempDir::new().expect("source root");
    for index in 0..300_u16 {
        let directory = root.path().join(format!("bucket-{index:03}"));
        std::fs::create_dir(&directory).expect("create bucket");
        std::fs::write(
            directory.join(format!("session-{index:03}.jsonl")),
            usage_line((index % 60) as u8, u64::from(index) + 1),
        )
        .expect("write source");
    }
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));

    let result = run(root.path(), &mut archive);
    assert_published(result);
    let counts = archive.store().counts().expect("counts");
    assert_eq!(counts.sources(), 300);
    assert_eq!(counts.canonical_events(), 300);
}

#[test]
fn available_empty_profile_is_an_authoritative_zero_source_rebuild() {
    let root = TempDir::new().expect("source root");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));

    assert_published(run(root.path(), &mut archive));
    let counts = archive.store().counts().expect("counts");
    assert_eq!(counts.sources(), 0);
    assert_eq!(counts.canonical_events(), 0);
    assert_eq!(
        archive.store().archive_state().expect("state").mode(),
        ArchiveMode::ReplayVerified
    );
}

#[test]
fn missing_profile_is_partial_and_cannot_replace_prior_published_usage() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("codex-root");
    std::fs::create_dir(&root).expect("create source root");
    std::fs::write(root.join("session.jsonl"), usage_line(1, 3)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_published(run(&root, &mut archive));
    std::fs::remove_dir_all(&root).expect("remove configured profile");

    let result = run(&root, &mut archive);
    assert_eq!(result.quality(), CompletionQuality::Partial);
    assert!(result.published_revision_id().is_none());
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        1
    );
    assert_eq!(
        archive.store().archive_state().expect("state").mode(),
        ArchiveMode::ReplayVerified
    );
}

#[test]
fn append_replacement_and_truncation_publish_complete_revisions_with_prior_usage_retained() {
    let root = TempDir::new().expect("source root");
    let path = root.path().join("session.jsonl");
    std::fs::write(&path, usage_line(1, 3)).expect("write baseline");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
    assert_published(run(root.path(), &mut archive));

    let mut appended = usage_line(1, 3);
    appended.push_str(&usage_line(2, 4));
    std::fs::write(&path, appended).expect("append fixture");
    assert_published(run(root.path(), &mut archive));
    assert_eq!(
        archive.store().counts().expect("counts").canonical_events(),
        2
    );

    let replacement = root.path().join("replacement.jsonl");
    std::fs::write(&replacement, usage_line(3, 9)).expect("write replacement");
    atomic_replace(&path, &replacement);
    assert_published(run(root.path(), &mut archive));
    let events = archive
        .store()
        .event_page_before(None, 10)
        .expect("replacement events");
    assert_eq!(events.len(), 3);

    std::fs::write(&path, usage_line(4, 2)).expect("truncate source");
    assert_published(run(root.path(), &mut archive));
    let events = archive
        .store()
        .event_page_before(None, 10)
        .expect("truncated events");
    assert_eq!(events.len(), 4);
}

#[test]
fn cancellation_before_bootstrap_keeps_the_archive_untouched() {
    let root = TempDir::new().expect("source root");
    std::fs::write(root.path().join("session.jsonl"), usage_line(1, 3)).expect("write source");
    let mut archive = StoreArchive::new(UsageStore::in_memory().expect("store"));
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
    coordinator.cancel(permit.id()).expect("cancel refresh");

    let result = OneShotExecutor::new().run(
        &permit,
        &FixedClock,
        &mut OpenLease,
        &mut adapter(root.path()),
        &mut archive,
    );
    assert_eq!(result.outcome(), RefreshOutcome::Cancelled);
    assert!(result.scan_set_id().is_none());
    assert_eq!(archive.store().counts().expect("counts").total(), 0);
}

#[test]
fn deadline_after_staging_begin_discards_exact_replay_and_keeps_no_staging_state() {
    let root = TempDir::new().expect("source root");
    std::fs::write(root.path().join("session.jsonl"), usage_line(1, 3)).expect("write source");
    let expired = Arc::new(AtomicBool::new(false));
    let clock = PhaseClock {
        expired: Arc::clone(&expired),
    };
    let mut archive = ExpireAfterReplayBegin {
        inner: StoreArchive::new(UsageStore::in_memory().expect("store")),
        expired,
    };
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

    let result = OneShotExecutor::new().run(
        &permit,
        &clock,
        &mut OpenLease,
        &mut adapter(root.path()),
        &mut archive,
    );
    assert_eq!(result.outcome(), RefreshOutcome::DeadlineExceeded);
    assert_eq!(
        result.cleanup(),
        tokenmaster_engine::ReplayCleanup::Discarded
    );
    let state = archive
        .inner
        .store()
        .archive_state()
        .expect("archive state");
    assert_eq!(state.mode(), ArchiveMode::Empty);
    assert!(!state.rebuild_staging());
    assert_eq!(
        archive
            .inner
            .store()
            .counts()
            .expect("counts")
            .canonical_events(),
        0
    );
}
