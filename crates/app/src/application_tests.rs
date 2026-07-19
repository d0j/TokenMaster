use std::sync::mpsc;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_engine::RefreshOutcome;
use tokenmaster_platform::{
    BackupDirectory, ControlledFileDialog, DurableFileTarget, FileDialogFileType, FileDialogResult,
    FileDialogSelector, ValidatedLocalDirectory,
};
use tokenmaster_product::{
    ProductReducer, ProductSectionKind, ProductSessionDetailSelection,
    ProductSessionDetailSelectionGeneration,
};
use tokenmaster_state::{
    BackupCatalog, BackupMaintenanceRuntime, BackupPolicy, BackupPurpose, BootstrapOutcome,
    ConfigPackage, MAX_CONFIG_PACKAGE_BYTES, MaintenanceExecution, MaintenanceSourceState,
    PortableSettings, PortableSettingsCandidate, PriorRunCondition, ReminderPolicy, RestoreMode,
    RunStateStore, SettingsStore, SettingsValue, SystemMaintenanceClock,
};
use tokenmaster_store::{USAGE_SCHEMA_VERSION, UsageStore};

use super::*;
use crate::command::{
    ApplicationCommandCompletion, ApplicationCommandCoordinator, ApplicationCommandOutcome,
};
use crate::state::ApplicationStateOwner;

fn application_environment(temporary: &TempDir) -> ApplicationEnvironment {
    let executable = temporary.path().join("TokenMaster.exe");
    std::fs::write(&executable, b"fixture").expect("fixture executable");
    let codex_home = temporary.path().join("codex");
    std::fs::create_dir(&codex_home).expect("Codex root");
    ApplicationEnvironment::new(
        executable,
        Some(temporary.path().to_path_buf()),
        None,
        Some(codex_home.into_os_string()),
    )
}

fn create_exact_v12_archive(path: &std::path::Path) {
    drop(UsageStore::open(path).expect("create current archive"));
    let connection = Connection::open(path).expect("open exact-v12 fixture");
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS git_category_no_update;
             DROP TRIGGER IF EXISTS git_day_no_update;
             DROP TRIGGER IF EXISTS git_day_category_no_update;
             DROP TRIGGER IF EXISTS git_installation_state_no_delete;
             DROP TRIGGER IF EXISTS git_warning_no_update;
             DROP INDEX IF EXISTS git_association_repository_activity;
             DROP INDEX IF EXISTS git_day_repository_range;
             DROP INDEX IF EXISTS git_day_category_repository_range;
             DROP INDEX IF EXISTS git_repository_observed;
             DROP TABLE IF EXISTS git_warning;
             DROP TABLE IF EXISTS git_category_aggregate;
             DROP TABLE IF EXISTS git_day_aggregate;
             DROP TABLE IF EXISTS git_day_category_aggregate;
             DROP TABLE IF EXISTS git_activity_association;
             DROP TABLE IF EXISTS git_repository;
             DROP TABLE IF EXISTS git_installation_state;",
        )
        .expect("strip v13 Git schema");
    connection
        .pragma_update(None, "user_version", 12_i64)
        .expect("publish exact v12 version");
}

fn disable_periodic_backups(root: &DataRoot) {
    let defaults = SettingsValue::safe_defaults();
    let current = defaults.portable().backup();
    let backup = BackupPolicy::new(
        false,
        current.quiet_seconds(),
        current.interval_seconds(),
        current.retention_budget_bytes(),
    )
    .expect("disabled periodic policy");
    let settings = SettingsValue::new(
        PortableSettings::new(defaults.portable().reminders().clone(), backup),
        defaults.device().clone(),
    );
    SettingsStore::new(root.reliable_state())
        .expect("migration settings store")
        .save(&settings)
        .expect("persist disabled periodic policy");
}

fn wait_for_application_completion(application: &Application) -> ApplicationCommandCompletion {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(completion) = application
            .commands
            .try_completion()
            .expect("operation completion")
        {
            return completion;
        }
        assert!(Instant::now() < deadline, "application operation timed out");
        std::thread::yield_now();
    }
}

fn wait_for_application_completion_with_visible_pending(
    application: &Application,
) -> ApplicationCommandCompletion {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        })
        .expect("schedule desktop event-loop pump");
        slint::run_event_loop_until_quit().expect("desktop event-loop pump");
        if let Some(completion) = application
            .commands
            .try_completion()
            .expect("operation completion")
        {
            return completion;
        }
        assert!(Instant::now() < deadline, "application operation timed out");
        std::thread::yield_now();
    }
}

fn wait_for_initial_live_refresh(application: &Application) {
    let bundle = application.bundle.lock().expect("live bundle");
    bundle
        .as_ref()
        .expect("live application bundle")
        .live
        .wait_for_completion(Duration::from_secs(30))
        .expect("initial live refresh")
        .expect("initial live refresh completion");
}

fn write_reminder_config_package(
    directory: &std::path::Path,
    portable: PortableSettingsCandidate,
) -> tokenmaster_platform::SelectedInputFile {
    std::fs::create_dir(directory).expect("config package directory");
    let directory = ValidatedLocalDirectory::new(directory).expect("validated package directory");
    let target =
        DurableFileTarget::exact_child(&directory, "reminders.tmconfig").expect("config target");
    let mut staged = target
        .create_staged(MAX_CONFIG_PACKAGE_BYTES)
        .expect("config package stage");
    ConfigPackage::write(&portable, 1_721_234_567_890, &mut staged).expect("config package");
    staged.publish_new(&target).expect("publish config package");
    let dialog = ControlledFileDialog::selected(&directory, "reminders.tmconfig")
        .expect("config input dialog");
    let FileDialogResult::Selected(input) =
        dialog.select_input(FileDialogFileType::Config, MAX_CONFIG_PACKAGE_BYTES)
    else {
        panic!("config input selection");
    };
    input
}

fn global_reminder_profile(archive: &std::path::Path) -> (i64, i64, i64, Vec<i64>) {
    let connection = Connection::open(archive).expect("current archive");
    let profile = connection
        .query_row(
            "SELECT revision, channel_in_app, channel_os_scheduled
             FROM benefit_reminder_profile
             WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .expect("global reminder profile");
    let leads = connection
        .prepare(
            "SELECT threshold_seconds FROM benefit_reminder_threshold
             WHERE profile_kind = 'global' AND length(profile_scope_id) = 0
             ORDER BY threshold_seconds DESC",
        )
        .expect("threshold query")
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("threshold rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("threshold values");
    (profile.0, profile.1, profile.2, leads)
}

fn portable_with_reminder_policy(
    settings: &SettingsValue,
    leads: &[u32],
) -> PortableSettingsCandidate {
    let reminders = ReminderPolicy::new(true, leads).expect("reminder policy");
    PortableSettingsCandidate::new(PortableSettings::new(
        reminders,
        settings.portable().backup().clone(),
    ))
    .expect("portable settings candidate")
}

fn assert_reminder_policy_config_import_worker_sync_lifecycle(
    application: &mut Application,
    temporary: &TempDir,
) {
    wait_for_initial_live_refresh(application);
    let root = &application.data_root;
    let success = &*application;
    let success_root = root;
    let success_before = SettingsStore::new(root.reliable_state())
        .expect("success settings store")
        .load()
        .expect("success settings before import");
    let success_portable = portable_with_reminder_policy(success_before.value(), &[21_600, 3_600]);
    let success_input = write_reminder_config_package(
        &temporary.path().join("success-config"),
        success_portable.clone(),
    );
    let ApplicationCommandAdmission::Started(success_preview) =
        success.commands.submitter().submit_request(
            crate::command::ApplicationOperationRequest::import_config(success_input),
        )
    else {
        panic!("success config preview must start");
    };
    let success_preview_completion = wait_for_application_completion(success);
    assert_eq!(
        success_preview_completion.request_id(),
        success_preview.id()
    );
    assert_eq!(
        success_preview_completion.outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    let success_preview = success
        .state
        .reliable_state_projection_for_outcome(BootstrapOutcome::Healthy, None)
        .expect("success preview projection")
        .config_import_preview()
        .expect("staged config preview");
    assert_eq!(success_preview.changed_category_count(), 1);

    let success_counters = {
        let bundle = success.bundle.lock().expect("success bundle");
        Arc::clone(
            &bundle
                .as_ref()
                .expect("success live bundle")
                .reminder_sync_counters,
        )
    };
    let ApplicationCommandAdmission::Started(success_confirm) = success
        .commands
        .submit(ApplicationCommand::ConfirmConfigImport)
    else {
        panic!("success config confirmation must start");
    };
    let success_completion = wait_for_application_completion_with_visible_pending(success);
    assert_eq!(success_completion.request_id(), success_confirm.id());
    assert_eq!(
        success_completion.outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    let success_settings = SettingsStore::new(success_root.reliable_state())
        .expect("success settings store")
        .load()
        .expect("success imported settings");
    assert_eq!(
        PortableSettingsCandidate::new(success_settings.value().portable().clone())
            .expect("success portable settings"),
        success_portable
    );
    let expected_success_revision = i64::try_from(
        success_settings
            .generation()
            .expect("success settings generation"),
    )
    .expect("success revision range")
        + 1;
    assert_eq!(
        global_reminder_profile(success_root.archive_path()),
        (expected_success_revision, 1, 0, vec![21_600, 3_600])
    );
    assert_eq!(
        success
            .state
            .reliable_state_projection_for_outcome(BootstrapOutcome::Healthy, None)
            .expect("success reliable projection")
            .reminder_policy()
            .sync_state(),
        tokenmaster_desktop::DesktopReminderSyncState::Synchronized
    );
    assert_eq!(success_counters.profile_hints.load(Ordering::Acquire), 1);
    assert_eq!(
        success_counters
            .controller_refreshes
            .load(Ordering::Acquire),
        1
    );
    success_counters.profile_hints.store(0, Ordering::Release);
    success_counters
        .controller_refreshes
        .store(0, Ordering::Release);

    let failure = &*application;
    let failure_temporary = temporary;
    let failure_root = root;
    let failure_before = SettingsStore::new(failure_root.reliable_state())
        .expect("failure settings store")
        .load()
        .expect("failure settings before import");
    let failure_portable = portable_with_reminder_policy(failure_before.value(), &[10_800]);
    let failure_input = write_reminder_config_package(
        &failure_temporary.path().join("failure-config"),
        failure_portable.clone(),
    );
    let ApplicationCommandAdmission::Started(failure_preview) =
        failure.commands.submitter().submit_request(
            crate::command::ApplicationOperationRequest::import_config(failure_input),
        )
    else {
        panic!("failure config preview must start");
    };
    let failure_preview_completion = wait_for_application_completion(failure);
    assert_eq!(
        failure_preview_completion.request_id(),
        failure_preview.id()
    );
    assert_eq!(
        failure_preview_completion.outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    let failure_counters = {
        let bundle = failure.bundle.lock().expect("failure bundle");
        Arc::clone(
            &bundle
                .as_ref()
                .expect("failure live bundle")
                .reminder_sync_counters,
        )
    };
    let profile_before_failure = global_reminder_profile(failure_root.archive_path());
    let writer = Connection::open(failure_root.archive_path()).expect("failure archive writer");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("failure archive write contention");

    let ApplicationCommandAdmission::Started(failure_confirm) = failure
        .commands
        .submit(ApplicationCommand::ConfirmConfigImport)
    else {
        panic!("failure config confirmation must start");
    };
    let failure_completion = wait_for_application_completion_with_visible_pending(failure);
    assert_eq!(failure_completion.request_id(), failure_confirm.id());
    assert_eq!(
        failure_completion.outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    writer
        .execute_batch("ROLLBACK")
        .expect("release failure writer");

    let failure_settings = SettingsStore::new(failure_root.reliable_state())
        .expect("failure settings store")
        .load()
        .expect("failure imported settings");
    assert_eq!(
        PortableSettingsCandidate::new(failure_settings.value().portable().clone())
            .expect("failure portable settings"),
        failure_portable
    );
    assert_eq!(
        failure
            .state
            .reliable_state_projection_for_outcome(BootstrapOutcome::Healthy, None)
            .expect("failure reliable projection")
            .reminder_policy()
            .sync_state(),
        tokenmaster_desktop::DesktopReminderSyncState::Pending
    );
    assert_eq!(
        global_reminder_profile(failure_root.archive_path()),
        profile_before_failure
    );
    assert_eq!(failure_counters.profile_hints.load(Ordering::Acquire), 0);
    assert_eq!(
        failure_counters
            .controller_refreshes
            .load(Ordering::Acquire),
        0
    );
}

#[test]
fn session_detail_sink_rejects_when_no_live_bundle_owns_the_controller() {
    let bundle: SharedBundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
    let sink = ApplicationSessionDetailIntentSink::new(Arc::downgrade(&bundle));
    let intent = DesktopSessionDetailIntent::new(
        tokenmaster_desktop::DesktopSnapshotEpoch::new(1).expect("epoch"),
        ProductReducer::new().snapshot().generation(),
        ProductSessionDetailSelection::new(
            ProductSessionDetailSelectionGeneration::new(1).expect("selection generation"),
            0,
        ),
    );
    assert_eq!(
        sink.submit(intent),
        DesktopSessionDetailIntentAdmission::Rejected
    );

    drop(bundle);
    assert_eq!(
        sink.submit(intent),
        DesktopSessionDetailIntentAdmission::Rejected
    );
}

#[test]
fn session_detail_sink_never_waits_for_a_busy_bundle_owner() {
    let bundle: SharedBundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
    let sink = ApplicationSessionDetailIntentSink::new(Arc::downgrade(&bundle));
    let intent = DesktopSessionDetailIntent::new(
        tokenmaster_desktop::DesktopSnapshotEpoch::new(1).expect("epoch"),
        ProductReducer::new().snapshot().generation(),
        ProductSessionDetailSelection::new(
            ProductSessionDetailSelectionGeneration::new(1).expect("selection generation"),
            0,
        ),
    );
    let guard = bundle.lock().expect("busy bundle guard");
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let _ = sender.send(sink.submit(intent));
    });
    let timely = receiver.recv_timeout(Duration::from_millis(100));
    drop(guard);
    worker.join().expect("session admission worker");

    assert_eq!(timely, Ok(DesktopSessionDetailIntentAdmission::Rejected));
}

#[test]
fn operation_projection_is_typed_path_free_and_never_offers_atomic_cancel() {
    let running_restore = application_operation_running(ApplicationCommand::RestoreData(
        ApplicationBackupSelection::new(1, 0).expect("selection"),
    ));
    assert_eq!(running_restore.kind(), DesktopOperationKind::Restore);
    assert_eq!(running_restore.phase(), DesktopOperationPhase::Running);
    assert!(running_restore.cancellable());

    let running_backup = application_operation_running(ApplicationCommand::Backup);
    assert_eq!(running_backup.kind(), DesktopOperationKind::Backup);
    assert!(running_backup.cancellable());

    let running_rebuild = application_operation_running(ApplicationCommand::Rebuild);
    assert_eq!(running_rebuild.kind(), DesktopOperationKind::Rebuild);
    assert!(running_rebuild.cancellable());

    let failed = application_operation_completion(
        ApplicationCommand::BackupCompact,
        ApplicationCommandExecution::Failed(ApplicationCommandFailure::Integrity),
    );
    assert_eq!(failed.phase(), DesktopOperationPhase::Failed);
    assert_eq!(failed.failure_code(), Some("integrity"));
    assert!(!failed.cancellable());
    assert!(!format!("{failed:?}").contains("C:\\private\\canary"));

    let atomic = DesktopOperationSnapshot::new(
        DesktopOperationKind::Restore,
        DesktopOperationPhase::AtomicPromotion,
        true,
        None,
    );
    assert!(!atomic.cancellable());
}

#[test]
fn reminder_policy_intent_admits_one_bounded_update_policy_request() {
    let (observed_sender, observed_receiver) = mpsc::sync_channel(1);
    let mut worker = ApplicationOperationWorker::spawn_with_payload(move |permit, payload| {
        observed_sender
            .send((permit.command(), payload))
            .expect("observed request");
        ApplicationCommandExecution::Succeeded
    })
    .expect("operation worker");
    let sink = ApplicationDesktopIntentSink::new(worker.submitter());

    assert_eq!(
        sink.submit(
            DesktopIntent::update_reminder_policy(true, &[21_600, 3_600])
                .expect("bounded desktop intent"),
        ),
        DesktopIntentAdmission::Started
    );
    let (command, payload) = observed_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("reminder request");
    assert_eq!(command, ApplicationCommand::UpdateReminderPolicy);
    let ApplicationOperationPayload::ReminderPolicy(update) = payload else {
        panic!("reminder payload");
    };
    assert!(update.enabled());
    assert_eq!(update.lead_seconds(), &[21_600, 3_600]);
    assert_eq!(
        application_operation_kind(command),
        DesktopOperationKind::UpdatePolicy
    );
    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn reminder_policy_sync_failure_keeps_the_durable_save_and_returns_success() {
    let temporary = TempDir::new().expect("temporary directory");
    let environment = application_environment(&temporary);
    let root = DataRoot::resolve(&environment).expect("data root");
    let state = ApplicationStateOwner::open(&root).expect("state owner");
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(permit) =
        coordinator.submit(ApplicationCommand::UpdateReminderPolicy)
    else {
        panic!("reminder permit");
    };
    let DesktopIntent::UpdateReminderPolicy(update) =
        DesktopIntent::update_reminder_policy(true, &[21_600]).expect("desktop reminder update")
    else {
        panic!("reminder intent");
    };
    let update = crate::command::ApplicationReminderPolicyUpdate::from_desktop(update)
        .expect("application reminder update");
    state
        .update_reminder_policy(&permit, update.into_policy(), || Ok(()))
        .expect("durable reminder save");
    let bundle: SharedBundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
    synchronize_reminder_policy_after_settings(&state, &root, &bundle)
        .expect("archive failure remains a successful settings operation");
    let projection = state
        .reliable_state_projection_for_outcome(BootstrapOutcome::Healthy, None)
        .expect("pending reliable projection");
    assert_eq!(
        projection.reminder_policy().sync_state(),
        tokenmaster_desktop::DesktopReminderSyncState::Pending
    );
    assert_eq!(projection.reminder_policy().lead_seconds(), &[21_600]);
}

#[test]
fn startup_busy_reminder_store_keeps_the_durable_policy_pending_and_retryable() {
    let temporary = TempDir::new().expect("temporary directory");
    let environment = application_environment(&temporary);
    let root = DataRoot::resolve(&environment).expect("data root");
    let state = ApplicationStateOwner::open(&root).expect("state owner");
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(permit) =
        coordinator.submit(ApplicationCommand::UpdateReminderPolicy)
    else {
        panic!("reminder permit");
    };
    state
        .update_reminder_policy(
            &permit,
            ReminderPolicy::new(true, &[21_600, 3_600]).expect("reminder policy"),
            || Ok(()),
        )
        .expect("durable reminder save");
    let before = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings before startup");
    drop(UsageStore::open(root.archive_path()).expect("current archive"));
    let writer = Connection::open(root.archive_path()).expect("archive writer");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("archive write contention");

    let notifier_port: Arc<dyn WorkerCompletionNotifier> =
        Arc::new(ApplicationRuntimeNotifier::new(Weak::new(), 1));
    let reminder = start_optional_reminder_runtime(
        &root,
        &state,
        root.archive_path().to_path_buf(),
        notifier_port,
    );
    assert!(reminder.owner().is_none());
    assert_eq!(reminder.failure, Some(RuntimeErrorCode::StoreUnavailable));
    let projection = state
        .reliable_state_projection_for_outcome(BootstrapOutcome::Healthy, None)
        .expect("pending reliable projection");
    assert!(projection.reminder_policy().enabled());
    assert_eq!(
        projection.reminder_policy().lead_seconds(),
        &[21_600, 3_600]
    );
    assert_eq!(
        projection.reminder_policy().sync_state(),
        tokenmaster_desktop::DesktopReminderSyncState::Pending
    );
    let after = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings after startup");
    assert_eq!(after.generation(), before.generation());
    assert_eq!(after.value(), before.value());

    writer
        .execute_batch("ROLLBACK")
        .expect("release archive writer");
    state
        .synchronize_reminder_profile(&root)
        .expect("pending profile remains retryable after contention");
}

fn assert_no_backup_rebuild_preserves_corrupt_truth_and_completes_authoritative_reconciliation() {
    let temporary = TempDir::new().expect("rebuild temporary directory");
    let environment = application_environment(&temporary);
    let root = DataRoot::resolve(&environment).expect("rebuild data root");
    let owner = ApplicationStateOwner::open(&root).expect("rebuild state owner");
    let corrupt = b"definitively-corrupt-application-archive";
    std::fs::write(root.archive_path(), corrupt).expect("corrupt active archive");
    std::fs::write(
        temporary.path().join("codex").join("session.jsonl"),
        concat!(
            r#"{"timestamp":"2026-07-18T08:00:00Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12},"total_token_usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}}}}"#,
            "\n",
        ),
    )
    .expect("authoritative Codex fixture");
    drop(owner);

    let mut application = Application::start(&environment).expect("safe-mode rebuild startup");
    assert_eq!(
        application
            .preflight
            .lock()
            .expect("preflight")
            .effective_outcome(),
        BootstrapOutcome::RecoveryRequired
    );
    assert!(!application.live_started.load(Ordering::Acquire));
    assert!(application.bundle.lock().expect("bundle slot").is_none());

    let ApplicationCommandAdmission::Started(request) =
        application.commands.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild command must start");
    };
    let completion = wait_for_application_completion(&application);
    assert_eq!(completion.request_id(), request.id());
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Succeeded);
    assert!(application.live_started.load(Ordering::Acquire));
    assert_eq!(
        application
            .preflight
            .lock()
            .expect("healthy preflight")
            .effective_outcome(),
        BootstrapOutcome::Healthy
    );
    let recovery_projection = application
        .state
        .reliable_state_projection_for_outcome(BootstrapOutcome::Healthy, None)
        .expect("durable recovery projection");
    let recovery_receipt = recovery_projection
        .recovery_receipt()
        .expect("visible recovery receipt");
    assert_eq!(
        recovery_receipt.kind(),
        tokenmaster_desktop::DesktopRecoveryKind::AuthoritativeSource
    );
    assert!(recovery_receipt.non_reconstructible_domains_lost());
    let bundle = application.bundle.lock().expect("rebuilt bundle");
    let rebuilt = bundle.as_ref().expect("rebuilt live bundle");
    let live = rebuilt.live.snapshot().expect("rebuilt live snapshot");
    assert_eq!(live.refresh().outcome(), Some(RefreshOutcome::Completed));
    assert!(live.engine().diagnostics().completed_refreshes() > 0);
    assert_eq!(
        rebuilt.maintenance.snapshot().worker().source_state(),
        MaintenanceSourceState::Healthy
    );
    drop(bundle);

    let readable = Connection::open(root.archive_path()).expect("read rebuilt archive");
    assert!(
        readable
            .query_row("SELECT count(*) FROM usage_event", [], |row| row
                .get::<_, i64>(0))
            .expect("reconstructed usage count")
            > 0
    );
    drop(readable);

    drop(UsageStore::open(root.archive_path()).expect("current rebuilt archive"));
    let quarantine = root.reliable_state().as_path().join("quarantine");
    let quarantine_set = std::fs::read_dir(quarantine)
        .expect("quarantine directory")
        .next()
        .expect("quarantine set")
        .expect("quarantine entry")
        .path();
    assert_eq!(
        std::fs::read(quarantine_set.join("tokenmaster.sqlite3"))
            .expect("quarantined corrupt archive"),
        corrupt
    );

    application
        .shutdown()
        .expect("rebuilt application shutdown");
}

fn assert_reconstruction_reconciliation_survives_restart_and_retries_without_rebuild() {
    let temporary = TempDir::new().expect("reconciliation restart temporary directory");
    let environment = application_environment(&temporary);
    let root = DataRoot::resolve(&environment).expect("reconciliation restart data root");
    let owner = ApplicationStateOwner::open(&root).expect("reconciliation restart owner");
    std::fs::write(root.archive_path(), b"definitively-corrupt-restart-archive")
        .expect("corrupt restart archive");
    std::fs::write(
        temporary.path().join("codex").join("restart-session.jsonl"),
        concat!(
            r#"{"timestamp":"2026-07-18T09:00:00Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":8,"output_tokens":3,"total_tokens":11},"total_token_usage":{"input_tokens":80,"output_tokens":30,"total_tokens":110}}}}"#,
            "\n",
        ),
    )
    .expect("restart authoritative Codex fixture");
    let mut preflight = owner.prepare(&root).expect("corrupt preflight");
    assert_eq!(
        preflight.effective_outcome(),
        BootstrapOutcome::RecoveryRequired
    );
    let guard = preflight
        .take_startup_guard()
        .expect("reconstruction guard");
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(permit) =
        coordinator.submit(ApplicationCommand::Rebuild)
    else {
        panic!("reconstruction permit must start");
    };
    let receipt = owner
        .reconstruct_definitively_corrupt(&permit, &guard, || {})
        .expect("publish reconstructed archive");
    assert!(receipt.reconstructed_from_authoritative_source());
    drop(guard);
    drop(preflight);
    drop(owner);

    let mut observed_reconciliation = false;
    let mut interrupted =
        Application::start_with_observer(&environment, |boundary| match boundary {
            ApplicationStartBoundary::BeforeReconstructionReconciliation => {
                observed_reconciliation = true;
                Err(ApplicationError::live_runtime())
            }
            ApplicationStartBoundary::PreMigrationBackupPublished
            | ApplicationStartBoundary::BeforePostMigrationBackup => Ok(()),
        })
        .expect("interrupted reconciliation opens recovery UI");
    assert!(observed_reconciliation);
    assert!(!interrupted.live_started.load(Ordering::Acquire));
    {
        let preflight = interrupted.preflight.lock().expect("interrupted preflight");
        assert!(preflight.requires_source_reconciliation());
        assert_eq!(
            preflight.effective_outcome(),
            BootstrapOutcome::RecoveryRequired
        );
    }

    let ApplicationCommandAdmission::Started(request) =
        interrupted.commands.submit(ApplicationCommand::Rebuild)
    else {
        panic!("reconciliation retry must start");
    };
    let completion = wait_for_application_completion(&interrupted);
    assert_eq!(completion.request_id(), request.id());
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Succeeded);
    assert!(interrupted.live_started.load(Ordering::Acquire));
    assert!(
        !interrupted
            .preflight
            .lock()
            .expect("reconciled preflight")
            .requires_source_reconciliation()
    );
    let readable = Connection::open(root.archive_path()).expect("reconciled archive");
    assert!(
        readable
            .query_row("SELECT count(*) FROM usage_event", [], |row| row
                .get::<_, i64>(0))
            .expect("reconciled usage count")
            > 0
    );
    drop(readable);
    interrupted
        .shutdown()
        .expect("reconciliation retry shutdown");
}

fn assert_reconstruction_safe_mode_keeps_explicit_reconciliation_retry() {
    let temporary = TempDir::new().expect("reconciliation safe-mode temporary directory");
    let environment = application_environment(&temporary);
    let root = DataRoot::resolve(&environment).expect("reconciliation safe-mode data root");
    let owner = ApplicationStateOwner::open(&root).expect("reconciliation safe-mode owner");
    std::fs::write(
        root.archive_path(),
        b"definitively-corrupt-safe-mode-archive",
    )
    .expect("corrupt safe-mode archive");
    std::fs::write(
        temporary.path().join("codex").join("safe-mode-session.jsonl"),
        concat!(
            r#"{"timestamp":"2026-07-18T10:00:00Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":7,"output_tokens":4,"total_tokens":11},"total_token_usage":{"input_tokens":70,"output_tokens":40,"total_tokens":110}}}}"#,
            "\n",
        ),
    )
    .expect("safe-mode authoritative Codex fixture");
    let mut preflight = owner.prepare(&root).expect("safe-mode corrupt preflight");
    let guard = preflight
        .take_startup_guard()
        .expect("safe-mode reconstruction guard");
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(permit) =
        coordinator.submit(ApplicationCommand::Rebuild)
    else {
        panic!("safe-mode reconstruction permit must start");
    };
    owner
        .reconstruct_definitively_corrupt(&permit, &guard, || {})
        .expect("publish safe-mode reconstructed archive");
    drop(guard);
    drop(preflight);
    drop(owner);

    for expected_launch in 1_u8..=2 {
        let mut observed = false;
        let mut interrupted =
            Application::start_with_observer(&environment, |boundary| match boundary {
                ApplicationStartBoundary::BeforeReconstructionReconciliation => {
                    observed = true;
                    Err(ApplicationError::live_runtime())
                }
                ApplicationStartBoundary::PreMigrationBackupPublished
                | ApplicationStartBoundary::BeforePostMigrationBackup => Ok(()),
            })
            .expect("interrupted recovery launch opens recovery UI");
        assert!(observed);
        let preflight = interrupted.preflight.lock().expect("launch preflight");
        assert!(preflight.requires_source_reconciliation());
        assert_eq!(
            preflight.report().recovery_launch(),
            tokenmaster_state::RecoveryLaunchDecision::Start {
                launch: expected_launch
            }
        );
        drop(preflight);
        interrupted
            .shutdown()
            .expect("interrupted recovery launch shutdown");
    }

    let mut safe_mode = Application::start(&environment).expect("bounded recovery safe mode");
    assert!(!safe_mode.live_started.load(Ordering::Acquire));
    {
        let preflight = safe_mode.preflight.lock().expect("safe-mode preflight");
        assert_eq!(preflight.effective_outcome(), BootstrapOutcome::SafeMode);
        assert!(preflight.requires_source_reconciliation());
        assert!(matches!(
            preflight.report().recovery_launch(),
            tokenmaster_state::RecoveryLaunchDecision::SafeMode { failed_launches: 2 }
        ));
    }
    let ApplicationCommandAdmission::Started(request) =
        safe_mode.commands.submit(ApplicationCommand::Rebuild)
    else {
        panic!("explicit safe-mode reconciliation must start");
    };
    let completion = wait_for_application_completion(&safe_mode);
    assert_eq!(completion.request_id(), request.id());
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Succeeded);
    assert!(safe_mode.live_started.load(Ordering::Acquire));
    safe_mode
        .shutdown()
        .expect("explicit safe-mode reconciliation shutdown");
}

#[test]
fn early_notification_sets_one_pending_bit_without_allocating_generation() {
    let bundle: SharedBundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
    let generation = begin_bundle_generation(&bundle).expect("bundle generation");
    let notifier = ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle), generation);

    notifier.publish().expect("lossy early notification");

    assert!(notifier.pending.load(Ordering::Acquire));
    assert_eq!(notifier.next_generation.load(Ordering::Acquire), 1);
}

#[test]
fn runtime_generation_overflow_is_checked_and_path_free() {
    let bundle: SharedBundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
    let generation = begin_bundle_generation(&bundle).expect("bundle generation");
    let notifier = ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle), generation);
    notifier.next_generation.store(u64::MAX, Ordering::Release);

    let error = notifier
        .next_generation()
        .expect_err("generation must not wrap");
    assert_eq!(error.code(), ApplicationErrorCode::GenerationOverflow);
    assert_eq!(error.to_string(), "generation_overflow");
}

#[test]
fn obsolete_bundle_notifier_cannot_publish_after_generation_replacement() {
    let bundle: SharedBundle = Arc::new(Mutex::new(ApplicationBundleSlot::new()));
    let first = begin_bundle_generation(&bundle).expect("first bundle generation");
    let notifier = ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle), first);

    let second = begin_bundle_generation(&bundle).expect("replacement bundle generation");
    assert!(second > first);
    notifier
        .publish()
        .expect("obsolete notification is discarded");

    assert!(!notifier.pending.load(Ordering::Acquire));
    assert_eq!(notifier.next_generation.load(Ordering::Acquire), 1);
}

#[test]
fn real_bundle_joins_live_health_and_independent_optional_failures_then_shuts_down() {
    let temporary = TempDir::new().expect("temporary directory");
    let codex_root = temporary.path().join("codex");
    std::fs::create_dir(&codex_root).expect("Codex root");
    let configured = [ConfiguredCodexRoot::new(&codex_root, None, true)];
    let discovery = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request");
    let archive = temporary.path().join("application.sqlite3");
    let live = LiveRuntime::start(&archive, discovery).expect("live runtime");
    let controller =
        DesktopController::open(&archive, DesktopQueryPlan::overview().expect("query plan"))
            .expect("desktop controller");
    let refresh_ingress = controller.refresh_ingress();
    let maintenance = BackupMaintenanceRuntime::spawn(
        Arc::new(SystemMaintenanceClock::new()),
        SettingsValue::safe_defaults().portable().backup().clone(),
        MaintenanceSourceState::Healthy,
        |permit| {
            permit.begin_publication().expect("publication boundary");
            MaintenanceExecution::Published { bytes: 1 }
        },
    )
    .expect("maintenance runtime");
    let mut bundle = ApplicationBundle {
        live,
        quota: OptionalRuntime {
            owner: None,
            failure: Some(RuntimeErrorCode::ProviderUnavailable),
        },
        reminder: OptionalReminderRuntime {
            owner: None,
            failure: Some(RuntimeErrorCode::StoreUnavailable),
        },
        notification_presentation: None,
        controller,
        refresh_ingress,
        maintenance,
        reminder_sync_counters: Arc::new(ReminderSyncCounters::default()),
        notifier: Arc::new(ApplicationRuntimeNotifier::new(Weak::new(), 1)),
    };

    bundle
        .publish_runtime(ProductRuntimeGeneration::new(1).expect("generation"))
        .expect("publish runtime health");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if bundle
            .controller
            .try_completion()
            .expect("controller completion")
            .is_some()
        {
            break;
        }
        assert!(Instant::now() < deadline, "controller completion timed out");
        std::thread::yield_now();
    }
    let snapshot = bundle
        .controller
        .take_snapshot()
        .expect("snapshot mailbox")
        .expect("product snapshot");
    assert_eq!(snapshot.runtime().usage().kind(), ProductSectionKind::Ready);
    assert_eq!(snapshot.runtime().git().kind(), ProductSectionKind::Ready);
    assert_eq!(
        snapshot.runtime().quota().observation_error(),
        Some(ProductRuntimeObservationError::ProviderUnavailable)
    );
    assert_eq!(
        snapshot.runtime().reminder().observation_error(),
        Some(ProductRuntimeObservationError::StoreUnavailable)
    );

    bundle.shutdown().expect("bundle shutdown");
}

#[test]
fn application_bootstraps_live_and_safe_mode_then_marks_clean_after_joined_shutdown() {
    let temporary = TempDir::new().expect("temporary directory");
    let environment = application_environment(&temporary);
    let mut application = Application::start(&environment).expect("application startup");
    assert_eq!(
        application.reliable_publish_count.load(Ordering::Acquire),
        1,
        "initial live bundle publishes one current reliable projection"
    );
    assert_eq!(
        application
            .state
            .reliable_state_projection_for_outcome(BootstrapOutcome::Healthy, None)
            .expect("initial reliable projection")
            .reminder_policy()
            .sync_state(),
        tokenmaster_desktop::DesktopReminderSyncState::Synchronized
    );
    assert!(
        application
            .bundle
            .lock()
            .expect("initial bundle")
            .as_ref()
            .expect("initial live bundle")
            .reminder
            .owner()
            .is_some(),
        "synchronization completes before the optional reminder starts"
    );

    let root = DataRoot::resolve(&environment).expect("data root");
    assert!(root.archive_path().exists());
    assert_reminder_policy_config_import_worker_sync_lifecycle(&mut application, &temporary);
    let crate::command::ApplicationCommandAdmission::Started(manual) = application
        .commands
        .submit(crate::command::ApplicationCommand::Backup)
    else {
        panic!("manual backup command must start");
    };
    let manual_completion = wait_for_application_completion(&application);
    assert_eq!(manual_completion.request_id(), manual.id());
    assert_eq!(
        manual_completion.outcome(),
        crate::command::ApplicationCommandOutcome::Succeeded
    );
    let policy_update = crate::command::ApplicationBackupPolicyUpdate::new(
        false,
        tokenmaster_state::BACKUP_QUIET_MIN_SECONDS,
        tokenmaster_state::BACKUP_INTERVAL_MIN_SECONDS,
        256,
    );
    let ApplicationCommandAdmission::Started(policy_request) =
        application.commands.submitter().submit_request(
            crate::command::ApplicationOperationRequest::update_backup_policy(policy_update),
        )
    else {
        panic!("backup policy update must start");
    };
    let policy_completion = wait_for_application_completion(&application);
    assert_eq!(policy_completion.request_id(), policy_request.id());
    assert_eq!(
        policy_completion.outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    let bundle_slot = application.bundle.lock().expect("bundle slot");
    let maintenance = &bundle_slot.as_ref().expect("healthy bundle").maintenance;
    assert!(
        !maintenance
            .snapshot()
            .scheduler()
            .schedule()
            .periodic_enabled()
    );
    assert_eq!(
        maintenance.snapshot().worker().source_state(),
        MaintenanceSourceState::Healthy
    );
    assert_eq!(maintenance.snapshot().worker().successful_count(), 1);
    for _ in 0..18 {
        wait_for_mandatory_backup(maintenance, MaintenancePurpose::Manual)
            .expect("bounded repeated manual backup");
    }
    assert_eq!(maintenance.snapshot().worker().successful_count(), 19);
    let old_bundle_generation = bundle_slot.generation;
    let old_notifier = Arc::clone(&bundle_slot.as_ref().expect("healthy bundle").notifier);
    drop(bundle_slot);

    let direct_restart_publications = application.reliable_publish_count.load(Ordering::Acquire);
    application
        .restart_services()
        .expect("controlled service restart");
    assert_eq!(
        application.reliable_publish_count.load(Ordering::Acquire),
        direct_restart_publications + 1,
        "direct restart publishes exactly one current reliable projection"
    );
    let restarted_slot = application.bundle.lock().expect("restarted bundle slot");
    assert!(restarted_slot.generation > old_bundle_generation);
    assert!(restarted_slot.is_some());
    drop(restarted_slot);
    let obsolete_runtime_generation = old_notifier.next_generation.load(Ordering::Acquire);
    old_notifier
        .publish()
        .expect("obsolete notifier is discarded after real restart");
    assert_eq!(
        old_notifier.next_generation.load(Ordering::Acquire),
        obsolete_runtime_generation
    );

    let selected_restore = application
        .state
        .oldest_verified_backup_selection()
        .expect("oldest verified restore selection");
    let pre_restore_generation = application
        .bundle
        .lock()
        .expect("pre-restore bundle")
        .generation;
    let operation_restore_publications = application.reliable_publish_count.load(Ordering::Acquire);
    let ApplicationCommandAdmission::Started(restore) = application
        .commands
        .submit(ApplicationCommand::RestoreData(selected_restore))
    else {
        panic!("identity-bound selected restore must start");
    };
    let restore_completion = wait_for_application_completion(&application);
    assert_eq!(restore_completion.request_id(), restore.id());
    assert_eq!(
        restore_completion.outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    assert_eq!(
        application.reliable_publish_count.load(Ordering::Acquire),
        operation_restore_publications,
        "operation replacement leaves publication to its one completion projection"
    );
    let restored_slot = application.bundle.lock().expect("restored bundle slot");
    assert!(restored_slot.generation > pre_restore_generation);
    assert!(restored_slot.is_some());
    let restored_generation = restored_slot.generation;
    drop(restored_slot);
    let stale_restore = application
        .restore_selected(selected_restore, RestoreMode::DataOnly)
        .expect_err("stale catalog selection fails before lifecycle mutation");
    assert_eq!(stale_restore.code(), ApplicationErrorCode::StateUnavailable);
    let unchanged_slot = application.bundle.lock().expect("unchanged live bundle");
    assert_eq!(unchanged_slot.generation, restored_generation);
    assert!(unchanged_slot.is_some());
    drop(unchanged_slot);

    let live_backups =
        BackupDirectory::open_or_create(root.reliable_state()).expect("live backup directory");
    let live_catalog = BackupCatalog::rebuild(&live_backups, None).expect("live backup catalog");
    assert!(live_catalog.points().len() <= 15);
    assert!(live_catalog.points().iter().all(|point| {
        matches!(
            (point.purpose(), point.compression()),
            (
                Some(BackupPurpose::Manual),
                Some(tokenmaster_state::BackupCompression::Normal)
            ) | (
                Some(BackupPurpose::PreRestore),
                Some(tokenmaster_state::BackupCompression::Automatic)
            )
        ) && point.database_schema_version() == Some(USAGE_SCHEMA_VERSION as u16)
    }));
    assert!(
        live_catalog
            .points()
            .iter()
            .any(|point| point.purpose() == Some(BackupPurpose::PreRestore))
    );

    let safe_temporary = TempDir::new().expect("safe-mode temporary directory");
    let safe_environment = application_environment(&safe_temporary);
    let safe_root = DataRoot::resolve(&safe_environment).expect("safe-mode data root");
    let safe_owner = ApplicationStateOwner::open(&safe_root).expect("safe-mode state owner");
    std::fs::write(
        safe_root
            .directory()
            .join("reliable-state")
            .join("staging")
            .join("unknown-evidence"),
        b"preserve",
    )
    .expect("unknown evidence");
    drop(safe_owner);
    let mut safe_application =
        Application::start(&safe_environment).expect("safe-mode application");
    assert!(
        safe_application
            .bundle
            .lock()
            .expect("bundle slot")
            .is_none()
    );
    assert!(!safe_root.archive_path().exists());
    let safe_restart = safe_application
        .restart_services()
        .expect_err("safe mode cannot bypass bootstrap through service restart");
    assert_eq!(safe_restart.code(), ApplicationErrorCode::InvalidLifecycle);
    assert!(
        safe_application
            .bundle
            .lock()
            .expect("safe bundle slot")
            .is_none()
    );
    assert!(!safe_root.archive_path().exists());
    safe_application
        .shutdown()
        .expect("safe-mode application shutdown");

    let migration_temporary = TempDir::new().expect("migration temporary directory");
    let migration_environment = application_environment(&migration_temporary);
    let migration_root = DataRoot::resolve(&migration_environment).expect("migration data root");
    create_exact_v12_archive(migration_root.archive_path());
    disable_periodic_backups(&migration_root);
    let mut migration_application =
        Application::start(&migration_environment).expect("guarded migration startup");
    let direct_restore_publications = migration_application
        .reliable_publish_count
        .load(Ordering::Acquire);
    assert!(
        migration_application
            .bundle
            .lock()
            .expect("migration bundle slot")
            .is_some(),
        "supported legacy state must become one live bundle"
    );
    let backups = BackupDirectory::open_or_create(migration_root.reliable_state())
        .expect("migration backups");
    let catalog = BackupCatalog::rebuild(&backups, None).expect("migration backup catalog");
    assert_eq!(catalog.points().len(), 2);
    let migration_points = catalog
        .points()
        .iter()
        .map(|point| (point.purpose(), point.database_schema_version()))
        .collect::<Vec<_>>();
    assert!(migration_points.contains(&(Some(BackupPurpose::PreMigration), Some(12))));
    assert!(migration_points.contains(&(
        Some(BackupPurpose::PostMigration),
        Some(USAGE_SCHEMA_VERSION as u16)
    )));
    let legacy_restore = migration_application
        .state
        .verified_backup_selection(BackupPurpose::PreMigration, 12)
        .expect("legacy pre-migration restore selection");
    let pre_legacy_restore_generation = migration_application
        .bundle
        .lock()
        .expect("pre-legacy-restore bundle")
        .generation;
    migration_application
        .restore_selected(legacy_restore, RestoreMode::DataOnly)
        .expect("legacy selected restore passes guarded migration lifecycle");
    assert_eq!(
        migration_application
            .reliable_publish_count
            .load(Ordering::Acquire),
        direct_restore_publications + 1,
        "direct restored bundle publishes exactly one current reliable projection"
    );
    let restored_legacy_slot = migration_application
        .bundle
        .lock()
        .expect("restored legacy bundle");
    assert!(restored_legacy_slot.generation > pre_legacy_restore_generation);
    assert!(restored_legacy_slot.is_some());
    assert_eq!(
        restored_legacy_slot
            .as_ref()
            .expect("restored legacy live bundle")
            .maintenance
            .snapshot()
            .worker()
            .successful_count(),
        2,
        "restored legacy archive must publish both migration safety points"
    );
    drop(restored_legacy_slot);
    let restored_legacy_catalog =
        BackupCatalog::rebuild(&backups, None).expect("restored legacy backup catalog");
    let restored_legacy_points = restored_legacy_catalog
        .points()
        .iter()
        .map(|point| (point.purpose(), point.database_schema_version()))
        .collect::<Vec<_>>();
    assert!(restored_legacy_points.contains(&(Some(BackupPurpose::PreMigration), Some(12))));
    assert!(restored_legacy_points.contains(&(
        Some(BackupPurpose::PostMigration),
        Some(USAGE_SCHEMA_VERSION as u16)
    )));
    assert!(
        restored_legacy_points
            .iter()
            .any(|point| point.0 == Some(BackupPurpose::PreRestore))
    );
    migration_application
        .shutdown()
        .expect("migration application shutdown");
    let migrated = Connection::open(migration_root.archive_path()).expect("open migrated archive");
    assert_eq!(
        migrated
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("migrated schema version"),
        USAGE_SCHEMA_VERSION
    );

    let failed_migration_temporary = TempDir::new().expect("failed migration temporary directory");
    let failed_migration_environment = application_environment(&failed_migration_temporary);
    let failed_migration_root =
        DataRoot::resolve(&failed_migration_environment).expect("failed migration data root");
    create_exact_v12_archive(failed_migration_root.archive_path());
    let mut failed_migration_application =
        Application::start_with_observer(&failed_migration_environment, |boundary| {
            assert_eq!(
                boundary,
                ApplicationStartBoundary::PreMigrationBackupPublished
            );
            Err(ApplicationError::live_runtime())
        })
        .expect("failed migration opens safe mode");
    assert!(
        failed_migration_application
            .bundle
            .lock()
            .expect("failed migration bundle slot")
            .is_none()
    );
    let unchanged =
        Connection::open(failed_migration_root.archive_path()).expect("unchanged old archive");
    assert_eq!(
        unchanged
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("unchanged old schema version"),
        12
    );
    let failed_backups = BackupDirectory::open_or_create(failed_migration_root.reliable_state())
        .expect("failed migration backups");
    let failed_catalog =
        BackupCatalog::rebuild(&failed_backups, None).expect("failed migration catalog");
    assert_eq!(failed_catalog.points().len(), 1);
    assert_eq!(
        failed_catalog.points()[0].purpose(),
        Some(BackupPurpose::PreMigration)
    );
    assert_eq!(
        failed_catalog.points()[0].database_schema_version(),
        Some(12)
    );
    failed_migration_application
        .shutdown()
        .expect("failed migration safe-mode shutdown");

    application.shutdown().expect("joined application shutdown");
    let final_restart = application
        .restart_services()
        .expect_err("final shutdown cannot resurrect a bundle");
    assert_eq!(final_restart.code(), ApplicationErrorCode::InvalidLifecycle);
    let final_restore = application
        .restore_selected(selected_restore, RestoreMode::DataOnly)
        .expect_err("final shutdown cannot begin selected restore");
    assert_eq!(final_restore.code(), ApplicationErrorCode::InvalidLifecycle);
    assert!(
        application
            .bundle
            .lock()
            .expect("final bundle slot")
            .is_none()
    );
    drop(application);

    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let next = owner.prepare(&root).expect("next preflight");
    assert_eq!(next.report().outcome(), BootstrapOutcome::Healthy);
    assert!(!next.report().recovery_resumed());
    assert_eq!(
        next.report().prior_run().condition(),
        PriorRunCondition::Clean
    );
    assert_migrated_archive_retries_pending_post_backup_before_live_restart();
    assert_no_backup_rebuild_preserves_corrupt_truth_and_completes_authoritative_reconciliation();
    assert_reconstruction_reconciliation_survives_restart_and_retries_without_rebuild();
    assert_reconstruction_safe_mode_keeps_explicit_reconciliation_retry();
}

fn assert_migrated_archive_retries_pending_post_backup_before_live_restart() {
    let temporary = TempDir::new().expect("post-migration temporary directory");
    let environment = application_environment(&temporary);
    let root = DataRoot::resolve(&environment).expect("post-migration data root");
    create_exact_v12_archive(root.archive_path());
    disable_periodic_backups(&root);

    let mut failed = Application::start_with_observer(&environment, |boundary| match boundary {
        ApplicationStartBoundary::BeforeReconstructionReconciliation => Ok(()),
        ApplicationStartBoundary::PreMigrationBackupPublished => Ok(()),
        ApplicationStartBoundary::BeforePostMigrationBackup => {
            Err(ApplicationError::live_runtime())
        }
    })
    .expect("post-migration failure opens safe mode");
    assert!(failed.bundle.lock().expect("failed bundle slot").is_none());
    let migrated = Connection::open(root.archive_path()).expect("migrated current archive");
    assert_eq!(
        migrated
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("current schema version"),
        USAGE_SCHEMA_VERSION
    );
    drop(migrated);
    let run_state = RunStateStore::new(root.reliable_state()).expect("run state");
    let pending = run_state
        .inspect()
        .expect("pending run state")
        .pending_migration()
        .expect("durable post-migration obligation");
    assert_eq!(pending.from_schema_version(), 12);
    assert_eq!(pending.to_schema_version(), USAGE_SCHEMA_VERSION as u16);
    let backups = BackupDirectory::open_or_create(root.reliable_state()).expect("backups");
    let before_restart = BackupCatalog::rebuild(&backups, None).expect("pre-only catalog");
    assert_eq!(before_restart.points().len(), 1);
    assert_eq!(
        before_restart.points()[0].purpose(),
        Some(BackupPurpose::PreMigration)
    );
    failed.shutdown().expect("safe-mode shutdown");

    let mut restarted = Application::start(&environment).expect("restart completes post point");
    assert!(
        restarted
            .bundle
            .lock()
            .expect("restarted bundle slot")
            .is_some()
    );
    assert_eq!(
        run_state
            .inspect()
            .expect("completed run state")
            .pending_migration(),
        None
    );
    let after_restart = BackupCatalog::rebuild(&backups, None).expect("completed catalog");
    assert!(after_restart.points().iter().any(|point| {
        point.purpose() == Some(BackupPurpose::PostMigration)
            && point.database_schema_version() == Some(USAGE_SCHEMA_VERSION as u16)
    }));
    restarted.shutdown().expect("restarted shutdown");
}
