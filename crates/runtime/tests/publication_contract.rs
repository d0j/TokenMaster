use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{RefreshOutcome, RefreshUrgency, WriterLease};
use tokenmaster_provider::DiscoveryRequest;
use tokenmaster_runtime::{EnginePublicationQuality, LiveRuntime, RuntimeWriterLease};
use tokenmaster_store::{ArchivePublicationQuality, UsageStore};

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
        "{{\"timestamp\":\"2026-07-16T00:00:{second:02}Z\",\"model\":\"gpt-5\",\"usage\":{{\"input_tokens\":{input},\"output_tokens\":2,\"total_tokens\":{}}}}}\n",
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

fn wait_completion(runtime: &LiveRuntime) -> tokenmaster_engine::WorkerCompletion {
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

#[test]
fn immutable_engine_snapshot_tracks_exact_archive_publication() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let source = source_root.path().join("session.jsonl");
    std::fs::write(&source, usage_line(1, 3)).expect("initial source");

    let mut runtime =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("live runtime");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let first = runtime.snapshot().expect("first runtime snapshot").engine();
    assert_eq!(first.quality(), EnginePublicationQuality::Complete);
    assert!(first.archive_revision().is_some());
    assert!(first.scan_set_id().is_some());
    assert!(first.data_through_ms().is_some());

    append(&source, &usage_line(2, 5));
    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("append refresh");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let second = runtime
        .snapshot()
        .expect("second runtime snapshot")
        .engine();
    assert!(second.is_newer_than(Some(first)));
    assert!(!first.is_newer_than(Some(second)));
    assert!(second.generation() > first.generation());
    assert!(second.archive_generation() > first.archive_generation());
    assert!(second.data_through_ms() >= first.data_through_ms());
    assert!(second.diagnostics().completed_refreshes() > first.diagnostics().completed_refreshes());

    assert_eq!(
        runtime.shutdown().expect("shutdown"),
        tokenmaster_runtime::LivePhase::Stopped
    );
    let store = UsageStore::open(&archive_path).expect("reopen archive");
    let publication = store.archive_publication().expect("archive publication");
    assert_eq!(publication.quality(), ArchivePublicationQuality::Complete);
    assert_eq!(second.archive_generation(), publication.generation().get());
    assert_eq!(
        second.archive_revision(),
        publication
            .current_revision()
            .map(|revision| revision.get())
    );
    assert_eq!(
        second.scan_set_id(),
        publication
            .latest_complete_scan_set()
            .map(|scan| scan.get())
    );
    let data_through_ms = publication
        .latest_complete_scan_set()
        .and_then(|scan| store.scan_set_snapshot(scan).ok())
        .and_then(|scan| scan.completed_at_ms());
    assert_eq!(second.data_through_ms(), data_through_ms);
}

#[test]
fn busy_refresh_cannot_advance_or_replace_engine_snapshot() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let source = source_root.path().join("session.jsonl");
    std::fs::write(&source, usage_line(1, 3)).expect("initial source");

    let mut runtime =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("live runtime");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let before = runtime.snapshot().expect("before snapshot").engine();

    let mut competing = RuntimeWriterLease::new(&archive_path).expect("competing lease");
    let guard = competing
        .try_acquire()
        .expect("hold competing writer guard");
    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("busy refresh submit");
    assert_eq!(wait_completion(&runtime).outcome(), RefreshOutcome::Busy);
    wait_quiescent(&runtime);
    let busy = runtime.snapshot().expect("busy snapshot").engine();
    assert_eq!(busy.generation(), before.generation());
    assert_eq!(busy.archive_generation(), before.archive_generation());
    drop(guard);

    append(&source, &usage_line(2, 5));
    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("recovery refresh");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let recovered = runtime.snapshot().expect("recovered snapshot").engine();
    assert!(recovered.is_newer_than(Some(before)));
    assert_eq!(
        recovered.diagnostics().busy_refreshes(),
        before.diagnostics().busy_refreshes() + 1
    );
    assert!(
        recovered.diagnostics().completed_refreshes() > before.diagnostics().completed_refreshes()
    );

    let private_debug = format!("{runtime:?}");
    assert!(!private_debug.contains(source_root.path().to_string_lossy().as_ref()));
    assert!(!private_debug.contains(archive_root.path().to_string_lossy().as_ref()));
    runtime.shutdown().expect("shutdown");
}

#[test]
fn no_change_resume_and_restart_keep_persisted_order_separate_from_process_generation() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let source = source_root.path().join("session.jsonl");
    std::fs::write(&source, usage_line(1, 3)).expect("initial source");

    let mut runtime =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("live runtime");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let first = runtime.snapshot().expect("first snapshot").engine();

    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("unchanged refresh");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let unchanged = runtime.snapshot().expect("unchanged snapshot").engine();
    assert!(unchanged.is_newer_than(Some(first)));
    assert_eq!(unchanged.archive_revision(), first.archive_revision());
    assert!(unchanged.scan_set_id() > first.scan_set_id());
    assert!(unchanged.data_through_ms() >= first.data_through_ms());

    assert_eq!(
        runtime.pause().expect("pause"),
        tokenmaster_runtime::LivePhase::Paused
    );
    let paused = runtime.snapshot().expect("paused snapshot").engine();
    assert_eq!(paused.generation(), unchanged.generation());
    assert!(!runtime.hints().filesystem_changed());
    assert_eq!(
        runtime.resume().expect("resume"),
        tokenmaster_runtime::LivePhase::Running
    );
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let resumed = runtime.snapshot().expect("resumed snapshot").engine();
    assert!(resumed.is_newer_than(Some(paused)));
    assert!(resumed.archive_generation() > paused.archive_generation());
    assert_eq!(resumed.archive_revision(), paused.archive_revision());
    runtime.shutdown().expect("first shutdown");

    let mut restarted =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("restart runtime");
    assert_eq!(
        wait_completion(&restarted).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&restarted);
    let after_restart = restarted.snapshot().expect("restart snapshot").engine();
    assert!(after_restart.archive_generation() > resumed.archive_generation());
    assert_eq!(after_restart.archive_revision(), resumed.archive_revision());
    assert!(after_restart.generation() < resumed.generation());
    restarted.shutdown().expect("restart shutdown");
}

#[test]
fn failed_truncation_rebuild_publishes_recovery_pending_then_recovers() {
    let source_root = TempDir::new().expect("source root");
    let archive_root = TempDir::new().expect("archive root");
    let archive_path = archive_root.path().join("usage.sqlite3");
    let source = source_root.path().join("session.jsonl");
    std::fs::write(&source, format!("{}{}", usage_line(1, 3), usage_line(2, 5)))
        .expect("initial source");

    let mut runtime =
        LiveRuntime::start(&archive_path, request(source_root.path())).expect("live runtime");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let complete = runtime.snapshot().expect("complete snapshot").engine();
    assert_eq!(complete.quality(), EnginePublicationQuality::Complete);

    std::fs::write(&source, b"{\"usage\":not-valid-json}\n").expect("truncate to malformed source");
    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("failed rebuild refresh");
    let failed_completion = wait_completion(&runtime);
    assert_eq!(
        failed_completion.outcome(),
        RefreshOutcome::Failed,
        "completion: {failed_completion:?}; runtime: {:?}",
        runtime.snapshot()
    );
    wait_quiescent(&runtime);
    let recovery = runtime.snapshot().expect("recovery snapshot").engine();
    assert!(recovery.is_newer_than(Some(complete)));
    assert_eq!(
        recovery.quality(),
        EnginePublicationQuality::RecoveryPending
    );
    assert_eq!(recovery.archive_revision(), complete.archive_revision());
    assert_eq!(
        UsageStore::open(&archive_path)
            .expect("read recovery archive")
            .counts()
            .expect("recovery counts")
            .canonical_events(),
        2
    );

    std::fs::write(&source, format!("{}{}", usage_line(3, 7), usage_line(4, 9)))
        .expect("repair source");
    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("recovery rebuild");
    assert_eq!(
        wait_completion(&runtime).outcome(),
        RefreshOutcome::Completed
    );
    wait_quiescent(&runtime);
    let repaired = runtime.snapshot().expect("repaired snapshot").engine();
    assert!(repaired.is_newer_than(Some(recovery)));
    assert_eq!(repaired.quality(), EnginePublicationQuality::Complete);
    assert!(repaired.archive_generation() > recovery.archive_generation());
    assert_ne!(repaired.archive_revision(), recovery.archive_revision());
    assert!(repaired.diagnostics().failed_refreshes() >= 1);
    runtime.shutdown().expect("shutdown");
}
