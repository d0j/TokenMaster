use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_domain::{
    GitOutputQuality, ProjectAlias, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId,
    UtcTimestamp,
};
use tokenmaster_engine::{RefreshOutcome, RefreshUrgency, WriterLease};
use tokenmaster_provider::{
    RepositoryActivityHint, RepositoryActivityHintParts, RepositoryCandidatePath,
};
use tokenmaster_runtime::{
    GitPublicationErrorCode, GitRefreshFailure, GitRuntime, GitRuntimeConfig, GitRuntimePhase,
    LiveRuntime, RuntimeWriterLease,
};
use tokenmaster_store::{GitOutputQuery, UsageReadStore};

mod support;

fn git_executable() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(if cfg!(windows) { "git.exe" } else { "git" }))
        .find(|candidate| candidate.is_file())
        .expect("native Git on PATH")
}

fn run_git(repository: &Path, args: &[&str]) {
    let status = Command::new(git_executable())
        .current_dir(repository)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .expect("run Git fixture command");
    assert!(status.success(), "Git fixture command failed: {args:?}");
}

fn repository(root: &Path) -> PathBuf {
    let repository = root.join("PRIVATE_GIT_RUNTIME_REPOSITORY");
    fs::create_dir(&repository).expect("repository directory");
    run_git(&repository, &["init", "-b", "main"]);
    run_git(&repository, &["config", "user.name", "Runtime User"]);
    run_git(
        &repository,
        &["config", "user.email", "runtime@example.com"],
    );
    fs::create_dir(repository.join("src")).expect("source directory");
    fs::write(repository.join("src/main.rs"), "fn main() {}\n").expect("source");
    run_git(&repository, &["add", "-A"]);
    run_git(&repository, &["commit", "-m", "runtime root"]);
    repository
}

fn hint(repository: &Path, session: &str) -> RepositoryActivityHint {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_secs();
    RepositoryActivityHint::new(RepositoryActivityHintParts {
        provider_id: UsageProviderId::new("codex").expect("provider"),
        profile_id: UsageProfileId::new("default").expect("profile"),
        source_id: UsageSourceId::new("source-1").expect("source"),
        session_id: UsageSessionId::new(session).expect("session"),
        observed_at: UtcTimestamp::new(i64::try_from(seconds).expect("seconds"), 0)
            .expect("timestamp"),
        project: Some(ProjectAlias::new("tokenmaster-runtime-test").expect("project")),
        candidate: RepositoryCandidatePath::new(repository.to_path_buf()).expect("candidate"),
    })
}

fn wait_for_publications(runtime: &GitRuntime, minimum: u64) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = runtime.snapshot().expect("runtime snapshot");
        if snapshot.refresh().published_count() >= minimum {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Git runtime publication timed out: {snapshot:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_outcome(runtime: &GitRuntime, expected: RefreshOutcome) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = runtime.snapshot().expect("runtime snapshot");
        if snapshot.refresh().outcome() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Git runtime outcome timed out: {snapshot:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn discovery_request(root: &Path) -> tokenmaster_provider::DiscoveryRequest {
    let configured = [ConfiguredCodexRoot::new(root, None, true)];
    build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request")
}

fn fixture_runtime(temporary: &TempDir, mode: &str) -> (GitRuntime, PathBuf, PathBuf) {
    let directory = temporary.path().join(mode);
    let repository = directory.join("repository");
    fs::create_dir_all(repository.join(".git")).expect("fixture repository");
    let executable = directory.join(if cfg!(windows) { "git.exe" } else { "git" });
    fs::copy(support::git_fixture_path(), &executable).expect("copy Git fixture");
    let archive = temporary.path().join(format!("{mode}.sqlite3"));
    let config = GitRuntimeConfig::new(archive)
        .expect("fixture runtime config")
        .with_executable(executable)
        .expect("fixture Git executable")
        .with_scan_timeout(Duration::from_secs(10))
        .expect("fixture timeout");
    (
        GitRuntime::start(config).expect("start fixture runtime"),
        repository,
        directory,
    )
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.is_file() {
        assert!(Instant::now() < deadline, "fixture marker timeout");
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn hint_drives_rebuild_then_unchanged_publication_without_exposing_private_values() {
    let temporary = TempDir::new().expect("temporary directory");
    let repository = repository(temporary.path());
    let archive = temporary.path().join("git-runtime.sqlite3");
    let config = GitRuntimeConfig::new(archive.clone())
        .expect("runtime config")
        .with_executable(git_executable())
        .expect("explicit Git")
        .with_scan_timeout(Duration::from_secs(10))
        .expect("scan timeout");
    let mut runtime = GitRuntime::start(config).expect("start Git runtime");
    runtime
        .submit_hint(hint(&repository, "session-1"))
        .expect("submit hint");
    runtime.refresh_now().expect("force refresh");
    wait_for_publications(&runtime, 1);

    let first = runtime.snapshot().expect("first snapshot");
    assert_eq!(first.phase(), GitRuntimePhase::Running);
    assert_eq!(first.refresh().rebuild_count(), 1);
    assert_eq!(first.refresh().published_count(), 1);
    assert_eq!(first.retained_hint_count(), 1);
    let private = repository.to_string_lossy();
    assert!(!format!("{runtime:?}{first:?}").contains(private.as_ref()));
    assert!(!format!("{runtime:?}{first:?}").contains("runtime@example.com"));

    runtime.refresh_now().expect("refresh unchanged");
    wait_for_publications(&runtime, 2);
    let second = runtime.snapshot().expect("second snapshot");
    assert_eq!(second.refresh().unchanged_count(), 1);
    assert_eq!(second.refresh().published_count(), 2);

    fs::write(
        repository.join("src/main.rs"),
        "fn main() {}\nfn appended() {}\n",
    )
    .expect("append source");
    run_git(&repository, &["add", "-A"]);
    run_git(&repository, &["commit", "-m", "runtime append"]);
    runtime
        .submit_hint(hint(&repository, "session-1"))
        .expect("submit appended hint");
    runtime.refresh_now().expect("refresh append");
    wait_for_publications(&runtime, 3);
    let third = runtime.snapshot().expect("append snapshot");
    assert_eq!(third.refresh().append_count(), 1);
    assert_eq!(third.refresh().published_count(), 3);

    let now_day = i32::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_secs()
            / 86_400,
    )
    .expect("day index");
    let mut reader = UsageReadStore::open(&archive).expect("read Git projection");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(now_day - 1, now_day, 32, Duration::from_secs(2))
                .expect("Git query"),
        )
        .expect("Git capture");
    assert_eq!(capture.repositories().len(), 1);
    assert_eq!(
        capture.repositories()[0].quality(),
        GitOutputQuality::Complete
    );
    assert_eq!(capture.repositories()[0].all_time_totals().commits(), 2);
    assert_eq!(capture.repositories()[0].scan_revision(), 3);

    runtime.force_recovery().expect("force recovery");
    wait_for_publications(&runtime, 4);
    let recovered = runtime.snapshot().expect("recovery snapshot");
    assert_eq!(recovered.refresh().rebuild_count(), 2);
    assert_eq!(recovered.refresh().published_count(), 4);

    assert_eq!(runtime.pause().expect("pause"), GitRuntimePhase::Paused);
    assert_eq!(
        runtime
            .snapshot()
            .expect("paused snapshot")
            .retained_hint_count(),
        1
    );
    assert_eq!(runtime.resume().expect("resume"), GitRuntimePhase::Running);
    wait_for_publications(&runtime, 5);
    let resumed = runtime.snapshot().expect("resumed snapshot");
    assert_eq!(resumed.refresh().rebuild_count(), 3);
    runtime.shutdown().expect("shutdown");
}

#[test]
fn one_broken_repository_does_not_block_a_valid_sibling_publication() {
    let temporary = TempDir::new().expect("temporary directory");
    let valid = repository(temporary.path());
    let invalid = temporary.path().join("not-a-repository");
    fs::create_dir(&invalid).expect("invalid candidate directory");
    let archive = temporary.path().join("git-runtime-isolation.sqlite3");
    let config = GitRuntimeConfig::new(archive)
        .expect("runtime config")
        .with_executable(git_executable())
        .expect("explicit Git")
        .with_scan_timeout(Duration::from_secs(10))
        .expect("scan timeout");
    let mut runtime = GitRuntime::start(config).expect("start Git runtime");
    runtime
        .submit_hint(hint(&invalid, "invalid-session"))
        .expect("submit invalid hint");
    runtime
        .submit_hint(hint(&valid, "valid-session"))
        .expect("submit valid hint");
    runtime.refresh_now().expect("force refresh");
    wait_for_publications(&runtime, 1);
    let snapshot = runtime.snapshot().expect("isolation snapshot");
    assert_eq!(snapshot.refresh().published_count(), 1);
    assert_eq!(snapshot.refresh().unavailable_count(), 1);
    assert_eq!(snapshot.refresh().scanned_count(), 1);
    runtime.shutdown().expect("shutdown");
}

#[test]
fn hint_ingress_keeps_only_the_latest_thirty_two_candidates() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("git-runtime-capacity.sqlite3");
    let config = GitRuntimeConfig::new(archive)
        .expect("runtime config")
        .with_executable(git_executable())
        .expect("explicit Git");
    let mut runtime = GitRuntime::start(config).expect("start Git runtime");
    for index in 0..33 {
        let candidate = temporary.path().join(format!("candidate-{index}"));
        fs::create_dir(&candidate).expect("candidate directory");
        runtime
            .submit_hint(hint(&candidate, &format!("session-{index}")))
            .expect("submit bounded hint");
    }
    let snapshot = runtime.snapshot().expect("capacity snapshot");
    assert_eq!(snapshot.retained_hint_count(), 32);
    assert_eq!(snapshot.dropped_hint_count(), 1);
    runtime.shutdown().expect("shutdown");
}

#[test]
fn writer_contention_happens_after_git_io_and_before_publication() {
    let temporary = TempDir::new().expect("temporary directory");
    let repository = repository(temporary.path());
    let archive = temporary.path().join("git-runtime-busy.sqlite3");
    let config = GitRuntimeConfig::new(archive.clone())
        .expect("runtime config")
        .with_executable(git_executable())
        .expect("explicit Git")
        .with_scan_timeout(Duration::from_secs(10))
        .expect("scan timeout");
    let mut runtime = GitRuntime::start(config).expect("start Git runtime");
    let mut competing = RuntimeWriterLease::new(&archive).expect("competing lease");
    let guard = competing.try_acquire().expect("hold competing lease");
    runtime
        .submit_hint(hint(&repository, "busy-session"))
        .expect("submit busy hint");
    runtime.refresh_now().expect("force busy refresh");
    wait_for_outcome(&runtime, RefreshOutcome::Busy);
    let busy = runtime.snapshot().expect("busy snapshot").refresh();
    assert_eq!(busy.scanned_count(), 1);
    assert_eq!(busy.published_count(), 0);
    assert_eq!(
        busy.failure(),
        Some(GitRefreshFailure::Publication(
            GitPublicationErrorCode::Busy
        ))
    );
    drop(guard);
    runtime.refresh_now().expect("retry after contention");
    wait_for_publications(&runtime, 1);
    runtime.shutdown().expect("shutdown");
}

#[test]
fn superseded_hint_rejects_the_scanned_result_and_runs_one_follow_up() {
    let temporary = TempDir::new().expect("temporary directory");
    let (mut runtime, repository, fixture) = fixture_runtime(&temporary, "slow_scan");
    runtime
        .submit_hint(hint(&repository, "stale-session-1"))
        .expect("submit first hint");
    runtime.refresh_now().expect("start slow refresh");
    wait_for_file(&fixture.join("log-started.txt"));
    runtime
        .submit_hint(hint(&repository, "stale-session-2"))
        .expect("supersede hint");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let snapshot = runtime.snapshot().expect("stale snapshot");
        if snapshot.refresh().stale_count() >= 1 {
            assert_eq!(snapshot.refresh().published_count(), 0);
            break;
        }
        assert!(Instant::now() < deadline, "stale rejection timeout");
        thread::sleep(Duration::from_millis(2));
    }
    wait_for_publications(&runtime, 1);
    let settled = runtime.snapshot().expect("follow-up snapshot");
    assert_eq!(settled.refresh().stale_count(), 1);
    assert_eq!(settled.refresh().published_count(), 1);
    runtime.shutdown().expect("shutdown");
}

#[test]
fn pause_cancels_and_reaps_the_active_git_child_before_returning() {
    let temporary = TempDir::new().expect("temporary directory");
    let (mut runtime, repository, fixture) = fixture_runtime(&temporary, "hang");
    runtime
        .submit_hint(hint(&repository, "cancel-session"))
        .expect("submit cancellation hint");
    runtime.refresh_now().expect("start hanging refresh");
    wait_for_file(&fixture.join("child-started.txt"));
    let started = Instant::now();
    assert_eq!(
        runtime.pause().expect("pause runtime"),
        GitRuntimePhase::Paused
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "pause waited for the fixture timeout"
    );
    let paused = runtime.snapshot().expect("paused snapshot");
    assert_eq!(paused.retained_hint_count(), 1);
    assert!(paused.refresh().cancelled_count() >= 1);
    assert!(paused.worker().active_request_id().is_none());
    runtime.shutdown().expect("shutdown");
}

#[test]
fn identified_repository_without_author_publishes_explicit_unavailable_truth() {
    let temporary = TempDir::new().expect("temporary directory");
    let (mut runtime, repository, _) = fixture_runtime(&temporary, "missing_author");
    runtime
        .submit_hint(hint(&repository, "missing-author-session"))
        .expect("submit missing-author hint");
    runtime.refresh_now().expect("start missing-author refresh");
    wait_for_publications(&runtime, 1);
    let snapshot = runtime.snapshot().expect("unavailable snapshot");
    assert_eq!(snapshot.refresh().published_count(), 1);
    assert_eq!(snapshot.refresh().unavailable_count(), 1);

    let mut reader = UsageReadStore::open(temporary.path().join("missing_author.sqlite3"))
        .expect("open unavailable projection");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(19_999, 20_001, 32, Duration::from_secs(2))
                .expect("unavailable query"),
        )
        .expect("unavailable capture");
    assert_eq!(capture.repositories().len(), 1);
    assert_eq!(
        capture.repositories()[0].quality(),
        GitOutputQuality::Unavailable
    );
    assert_eq!(
        capture.repositories()[0].unavailable_reason(),
        Some(tokenmaster_domain::GitOutputUnavailableReason::AuthorIdentityMissing)
    );
    runtime.shutdown().expect("shutdown");
}

#[test]
fn live_codex_usage_flow_routes_private_hints_into_the_git_runtime() {
    let temporary = TempDir::new().expect("temporary directory");
    let repository = repository(temporary.path());
    let codex_root = temporary.path().join("codex-root");
    fs::create_dir(&codex_root).expect("Codex root");
    let content = format!(
        "{}\n{}\n",
        serde_json::json!({
            "timestamp": "2026-07-17T01:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": "live-git-session",
                "cwd": repository,
                "requested_model": "gpt-test"
            }
        }),
        serde_json::json!({
            "timestamp": "2026-07-17T01:01:00Z",
            "usage": {"total_tokens": 1}
        })
    );
    fs::write(codex_root.join("session.jsonl"), content).expect("Codex source");
    let archive = temporary.path().join("live-git-runtime.sqlite3");
    let mut runtime =
        LiveRuntime::start(&archive, discovery_request(&codex_root)).expect("start live runtime");
    runtime
        .refresh_now(RefreshUrgency::Recovery)
        .expect("usage recovery");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = runtime.snapshot().expect("live snapshot");
        if snapshot.git().refresh().published_count() >= 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "live Git routing timed out: {snapshot:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        runtime
            .snapshot()
            .expect("routed snapshot")
            .git()
            .retained_hint_count(),
        1
    );
    runtime.shutdown().expect("shutdown live runtime");
}
