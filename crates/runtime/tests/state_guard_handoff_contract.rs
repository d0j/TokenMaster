use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, ExclusiveFileLease, ValidatedLocalDirectory,
};
use tokenmaster_runtime::LiveRuntime;
use tokenmaster_state::{
    BootstrapOutcome, PriorRunCondition, RecoveryJournalStore, RunStateStore, SettingsStore,
    StateBootstrap,
};
use tokenmaster_store::{BackupControl, BackupStaging};

#[test]
fn state_preflight_hands_the_same_guard_to_live_start_and_clean_follows_joined_shutdown() {
    let root = TempDir::new().expect("application root");
    let source = TempDir::new().expect("Codex source root");
    let reliable_path = root.path().join("reliable-state");
    fs::create_dir(&reliable_path).expect("reliable-state");
    let data = ValidatedLocalDirectory::new(root.path()).expect("data root");
    let reliable = ValidatedLocalDirectory::new(&reliable_path).expect("reliable root");
    let scope = ArchiveRecoveryScope::new(&data, &reliable).expect("recovery scope");
    let backups = BackupDirectory::open_or_create(&reliable).expect("backup directory");
    let staging_root =
        ValidatedLocalDirectory::new(&reliable_path.join("staging")).expect("staging root");
    let staging = BackupStaging::new(&staging_root).expect("verification staging");
    let settings = SettingsStore::new(&reliable).expect("settings store");
    let journal = RecoveryJournalStore::new(&reliable).expect("recovery journal");
    let run_state = RunStateStore::new(&reliable).expect("run-state store");
    let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
        .expect("backup control");
    let archive = data.as_path().join("tokenmaster.sqlite3");
    let guard = ExclusiveFileLease::for_archive(&archive)
        .expect("archive lease")
        .try_acquire()
        .expect("startup guard");
    let bootstrap = StateBootstrap::new(
        &data, &scope, &staging, &journal, &settings, &run_state, &backups,
    )
    .expect("bound bootstrap");
    let mut prepared = bootstrap
        .prepare(&guard, &control)
        .expect("state preflight");
    assert_eq!(prepared.report().outcome(), BootstrapOutcome::FirstInstall);
    assert!(!archive.exists());

    let configured = [ConfiguredCodexRoot::new(source.path(), None, true)];
    let discovery = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request");
    let mut runtime = LiveRuntime::start_guarded(&archive, discovery, guard)
        .expect("continuous guarded live startup");
    assert!(archive.exists());
    runtime.shutdown().expect("joined runtime shutdown");

    prepared.session_mut().authorize_healthy_launch();
    prepared
        .session_mut()
        .mark_clean()
        .expect("post-join clean marker");
    assert_eq!(
        run_state.inspect().expect("clean run state").condition(),
        PriorRunCondition::Clean
    );
}
