use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_domain::{UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId};
use tokenmaster_git::{
    GitCancellation, GitExecutable, GitIdentitySalt, GitProcess, GitRefreshKind,
    GitRepositoryCandidate, GitRunControl, derive_activity_association_id,
};

mod support;

fn native_name() -> &'static str {
    if cfg!(windows) { "git.exe" } else { "git" }
}

#[test]
fn activity_association_identity_is_salted_scope_exact_and_redacted() {
    let provider = UsageProviderId::new("codex").expect("provider");
    let profile = UsageProfileId::new("default").expect("profile");
    let source = UsageSourceId::new("source-1").expect("source");
    let session = UsageSessionId::new("session-1").expect("session");
    let first = derive_activity_association_id(
        &GitIdentitySalt::from_bytes([1; 32]),
        &provider,
        &profile,
        &source,
        &session,
    )
    .expect("association");
    let other_salt = derive_activity_association_id(
        &GitIdentitySalt::from_bytes([2; 32]),
        &provider,
        &profile,
        &source,
        &session,
    )
    .expect("salted association");
    let other_session = derive_activity_association_id(
        &GitIdentitySalt::from_bytes([1; 32]),
        &provider,
        &profile,
        &source,
        &UsageSessionId::new("session-2").expect("other session"),
    )
    .expect("scoped association");
    assert_ne!(first, other_salt);
    assert_ne!(first, other_session);
    assert_eq!(format!("{first:?}"), "GitActivityAssociationId([redacted])");
}

struct Fixture {
    _temporary: TempDir,
    directory: PathBuf,
    repository: PathBuf,
    executable: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temporary = TempDir::new().expect("fixture root");
        let directory = temporary.path().join("incremental");
        let repository = directory.join("repository");
        fs::create_dir_all(repository.join(".git")).expect("repository fixture");
        let executable = directory.join(native_name());
        fs::copy(support::fixture_path(), &executable).expect("copy fixture executable");
        fs::write(directory.join("phase.txt"), "initial").expect("initial phase");
        Self {
            _temporary: temporary,
            directory,
            repository,
            executable,
        }
    }

    fn set_phase(&self, value: &str) {
        fs::write(self.directory.join("phase.txt"), value).expect("set phase");
    }

    fn process(&self) -> GitProcess {
        let executable = GitExecutable::new(self.executable.clone()).expect("fixture executable");
        let control = GitRunControl::new(Duration::from_secs(5), GitCancellation::new())
            .expect("run control");
        GitProcess::new(executable, control)
    }

    fn candidate(&self) -> GitRepositoryCandidate {
        GitRepositoryCandidate::new(self.repository.clone()).expect("repository candidate")
    }

    fn receipt(&self) -> String {
        fs::read_to_string(self.directory.join("receipt.txt")).expect("receipt")
    }
}

fn log_count(receipt: &str) -> usize {
    receipt
        .lines()
        .filter(|line| line.starts_with("argv=") && line.contains("|log|"))
        .count()
}

#[test]
fn unchanged_skips_history_and_append_scans_only_new_reachable_commits() {
    let fixture = Fixture::new();
    let salt = GitIdentitySalt::from_bytes([7; 32]);
    let first = fixture
        .process()
        .refresh(&fixture.candidate(), salt, None)
        .expect("initial refresh");
    assert_eq!(first.kind(), GitRefreshKind::Rebuild);
    assert_eq!(
        first.summary().expect("rebuild summary").totals().commits(),
        1
    );
    assert_eq!(log_count(&fixture.receipt()), 1);

    let unchanged = fixture
        .process()
        .refresh(&fixture.candidate(), salt, Some(first.frontier()))
        .expect("unchanged refresh");
    assert_eq!(unchanged.kind(), GitRefreshKind::Unchanged);
    assert!(unchanged.summary().is_none());
    assert_eq!(log_count(&fixture.receipt()), 1, "unchanged ran git log");

    fixture.set_phase("append");
    let appended = fixture
        .process()
        .refresh(&fixture.candidate(), salt, Some(unchanged.frontier()))
        .expect("append refresh");
    assert_eq!(appended.kind(), GitRefreshKind::Append);
    assert_eq!(
        appended
            .summary()
            .expect("append summary")
            .totals()
            .commits(),
        1
    );
    let receipt = fixture.receipt();
    assert_eq!(log_count(&receipt), 2);
    assert!(receipt.contains("|--not|1111111111111111111111111111111111111111"));
    assert!(!format!("{appended:?}").contains(&fixture.repository.to_string_lossy()[..]));
}

#[test]
fn rewritten_history_falls_back_to_one_authoritative_rebuild() {
    let fixture = Fixture::new();
    let salt = GitIdentitySalt::from_bytes([8; 32]);
    let first = fixture
        .process()
        .refresh(&fixture.candidate(), salt, None)
        .expect("initial refresh");
    fixture.set_phase("rewrite");
    let rebuilt = fixture
        .process()
        .refresh(&fixture.candidate(), salt, Some(first.frontier()))
        .expect("rewrite rebuild");
    assert_eq!(rebuilt.kind(), GitRefreshKind::Rebuild);
    assert_eq!(
        rebuilt
            .summary()
            .expect("rebuild summary")
            .totals()
            .commits(),
        1
    );
    assert_eq!(log_count(&fixture.receipt()), 2);
}

#[test]
fn linked_cancellation_probe_stops_the_exact_child_without_a_monitor_thread() {
    let fixture_root = TempDir::new().expect("fixture root");
    let directory = fixture_root.path().join("hang");
    fs::create_dir(&directory).expect("fixture directory");
    let executable_path = directory.join(native_name());
    fs::copy(support::fixture_path(), &executable_path).expect("copy fixture executable");
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let probe = std::sync::Arc::clone(&cancelled);
    let cancellation =
        GitCancellation::linked(move || probe.load(std::sync::atomic::Ordering::Acquire));
    let executable = GitExecutable::new(executable_path).expect("fixture executable");
    let process = GitProcess::new(
        executable,
        GitRunControl::new(Duration::from_secs(2), cancellation).expect("run control"),
    );
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        cancelled.store(true, std::sync::atomic::Ordering::Release);
    });
    let error = process.version().expect_err("linked cancellation");
    setter.join().expect("setter");
    assert_eq!(
        error.code(),
        tokenmaster_git::GitBackendErrorCode::Cancelled
    );
    assert!(!format!("{error:?}").contains(Path::new(&directory).to_string_lossy().as_ref()));
}
