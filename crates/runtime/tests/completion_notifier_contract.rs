use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{RefreshUrgency, WorkerCompletion, WorkerCompletionNotifier};
use tokenmaster_runtime::{
    BenefitReminderRuntime, BenefitReminderRuntimeConfig, CodexQuotaRuntime,
    CodexQuotaRuntimeConfig, GitRuntime, GitRuntimeConfig, LiveRuntime,
};
use tokenmaster_store::UsageStore;

#[derive(Default)]
struct CountingNotifier(AtomicU64);

impl CountingNotifier {
    fn count(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

impl WorkerCompletionNotifier for CountingNotifier {
    fn completion_ready(&self, _completion: WorkerCompletion) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

fn wait_for_notification(notifier: &CountingNotifier) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while notifier.count() == 0 {
        assert!(
            Instant::now() < deadline,
            "completion notification timed out"
        );
        std::thread::yield_now();
    }
}

fn request(root: &std::path::Path) -> tokenmaster_provider::DiscoveryRequest {
    let configured = [ConfiguredCodexRoot::new(root, None, true)];
    build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request")
}

#[test]
fn live_runtime_emits_completion_through_supplied_notifier() {
    let temporary = TempDir::new().expect("temporary directory");
    let codex_root = temporary.path().join("codex");
    std::fs::create_dir(&codex_root).expect("Codex root");
    let archive = temporary.path().join("live.sqlite3");
    let notifier = Arc::new(CountingNotifier::default());
    let mut runtime = LiveRuntime::start_notified(&archive, request(&codex_root), notifier.clone())
        .expect("start notified live runtime");
    runtime
        .refresh_now(RefreshUrgency::Interactive)
        .expect("force live refresh");

    wait_for_notification(&notifier);
    runtime.shutdown().expect("shutdown live runtime");
}

#[test]
fn git_runtime_emits_completion_through_supplied_notifier() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("git.sqlite3");
    UsageStore::open(&archive).expect("create archive");
    let notifier = Arc::new(CountingNotifier::default());
    let mut runtime = GitRuntime::start_notified(
        GitRuntimeConfig::new(archive).expect("Git config"),
        notifier.clone(),
    )
    .expect("start notified Git runtime");
    runtime.refresh_now().expect("force Git refresh");

    wait_for_notification(&notifier);
    runtime.shutdown().expect("shutdown Git runtime");
}

#[cfg(windows)]
#[test]
fn quota_runtime_emits_completion_through_supplied_notifier() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("quota.sqlite3");
    let executable =
        std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("Windows root"))
            .join("System32")
            .join("where.exe");
    let config = CodexQuotaRuntimeConfig::new(archive)
        .expect("quota config")
        .with_executable(executable)
        .expect("fixed executable")
        .with_transport_timeout(Duration::from_secs(1))
        .expect("bounded timeout");
    let notifier = Arc::new(CountingNotifier::default());
    let mut runtime = CodexQuotaRuntime::start_notified(config, notifier.clone())
        .expect("start notified quota runtime");
    runtime.refresh_now().expect("force quota refresh");

    wait_for_notification(&notifier);
    runtime.shutdown().expect("shutdown quota runtime");
}

#[test]
fn reminder_runtime_emits_completion_through_supplied_notifier() {
    let temporary = TempDir::new().expect("temporary directory");
    let archive = temporary.path().join("reminder.sqlite3");
    UsageStore::open(&archive).expect("create archive");
    let notifier = Arc::new(CountingNotifier::default());
    let mut runtime = BenefitReminderRuntime::start_notified(
        BenefitReminderRuntimeConfig::new(archive).expect("reminder config"),
        notifier.clone(),
    )
    .expect("start notified reminder runtime");
    runtime.reconcile_now().expect("force reminder refresh");

    wait_for_notification(&notifier);
    runtime.shutdown().expect("shutdown reminder runtime");
}
