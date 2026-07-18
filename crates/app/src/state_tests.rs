use std::ffi::OsString;
use std::fs;

use tempfile::TempDir;
use tokenmaster_desktop::DesktopReliableStateHealth;
use tokenmaster_platform::{
    ControlledFileDialog, DurableFileTarget, ExclusiveFileLease, ExclusiveFileLeaseError,
    FileDialogFileType, FileDialogResult, FileDialogSelector, MAX_DURABLE_FILE_BYTES,
    ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCompression, BackupPackage, BackupPassphrase, BackupPolicy, BootstrapOutcome,
    ConfigPackage, EncryptedBackupPackage, MAX_CONFIG_PACKAGE_BYTES, PortableSettings,
    PortableSettingsCandidate, ReminderPolicy, SettingsChangeCategory, SettingsStore,
    SettingsValue,
};
use tokenmaster_store::UsageStore;

use crate::command::{
    ApplicationBackupPolicyUpdate, ApplicationCommand, ApplicationCommandAdmission,
    ApplicationCommandCoordinator,
};
use crate::state::ApplicationStateOwner;
use crate::{ApplicationEnvironment, DataRoot};

fn fixture() -> (TempDir, DataRoot) {
    let temporary = tempfile::tempdir().expect("temporary application root");
    let executable = temporary.path().join("TokenMaster.exe");
    fs::write(&executable, b"fixture").expect("fixture executable");
    let environment = ApplicationEnvironment::new(
        executable,
        Some(temporary.path().to_path_buf()),
        None,
        None::<OsString>,
    );
    let root = DataRoot::resolve(&environment).expect("data root");
    (temporary, root)
}

#[test]
fn state_owner_creates_only_the_fixed_reliable_state_tree() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let reliable = root.directory().join("reliable-state");

    for child in ["staging", "quarantine", "backups"] {
        let metadata = fs::symlink_metadata(reliable.join(child)).expect("fixed child");
        assert!(metadata.is_dir());
        assert!(!metadata.file_type().is_symlink());
    }
    assert!(!root.archive_path().exists());
    assert!(!format!("{owner:?}").contains(&root.directory().display().to_string()));
}

#[test]
fn first_install_preflight_publishes_unclean_and_holds_the_startup_guard() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let preflight = owner.prepare(&root).expect("state preflight");

    assert_eq!(preflight.report().outcome(), BootstrapOutcome::FirstInstall);
    assert!(!root.archive_path().exists());
    let reliable = root.directory().join("reliable-state");
    let run_records = ["run-a.tms", "run-b.tms"]
        .into_iter()
        .filter(|name| reliable.join(name).exists())
        .count();
    assert_eq!(run_records, 1);

    let competing = ExclusiveFileLease::for_archive(root.archive_path())
        .expect("same archive lease")
        .try_acquire()
        .expect_err("preflight keeps the startup guard");
    assert_eq!(competing, ExclusiveFileLeaseError::Contended);

    drop(preflight);
    ExclusiveFileLease::for_archive(root.archive_path())
        .expect("same archive lease")
        .try_acquire()
        .expect("guard released with preflight");
}

#[test]
fn first_install_reliable_state_projection_is_bounded_truth_without_fabricated_history() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let preflight = owner.prepare(&root).expect("state preflight");
    let projection = owner
        .reliable_state_projection(preflight.report())
        .expect("reliable-state projection");

    assert_eq!(projection.health(), DesktopReliableStateHealth::Healthy);
    assert!(!projection.safe_mode());
    assert_eq!(
        projection.settings_health_code(),
        "defaults_no_valid_record"
    );
    assert_eq!(projection.successful_count(), Some(0));
    assert_eq!(projection.failure_count(), Some(0));
    assert_eq!(projection.published_bytes(), Some(0));
    assert_eq!(projection.latest_success_at_utc_ms(), None);
    assert_eq!(projection.latest_attempt_at_utc_ms(), None);
    assert!(projection.restore_points().is_empty());
}

fn command_permit(command: ApplicationCommand) -> crate::command::ApplicationCommandPermit {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(permit) = coordinator.submit(command) else {
        panic!("command must start");
    };
    permit
}

fn changed_portable_settings() -> PortableSettingsCandidate {
    let defaults = SettingsValue::safe_defaults();
    let reminders = ReminderPolicy::new(true, &[10_800]).expect("reminder policy");
    let backup = BackupPolicy::new(
        defaults.portable().backup().periodic_enabled(),
        defaults.portable().backup().quiet_seconds(),
        defaults.portable().backup().interval_seconds(),
        defaults.portable().backup().retention_budget_bytes(),
    )
    .expect("backup policy");
    PortableSettingsCandidate::new(PortableSettings::new(reminders, backup))
        .expect("portable candidate")
}

#[test]
fn config_export_is_create_new_verified_and_path_private() {
    let (temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let export_directory = temporary.path().join("exports");
    fs::create_dir(&export_directory).expect("export directory");
    let export_directory =
        ValidatedLocalDirectory::new(&export_directory).expect("validated export directory");
    let dialog = ControlledFileDialog::selected(&export_directory, "settings.tmconfig")
        .expect("export dialog");
    let FileDialogResult::Selected(target) = dialog.select_output(FileDialogFileType::Config)
    else {
        panic!("export selection");
    };
    let permit = command_permit(ApplicationCommand::ExportConfig);

    let receipt = owner
        .export_config(&permit, target, 1_721_234_567_890, || {})
        .expect("config export");
    assert!(!permit.is_cancelled());
    assert!(!permit.clone().begin_irreversible().is_ok());
    assert_eq!(receipt.created_at_utc_ms(), 1_721_234_567_890);
    assert!(receipt.package_bytes() > 0);
    assert!(receipt.package_bytes() <= MAX_CONFIG_PACKAGE_BYTES);
    assert!(!format!("{receipt:?}").contains(&temporary.path().display().to_string()));

    let input_dialog = ControlledFileDialog::selected(&export_directory, "settings.tmconfig")
        .expect("input dialog");
    let FileDialogResult::Selected(input) =
        input_dialog.select_input(FileDialogFileType::Config, MAX_CONFIG_PACKAGE_BYTES)
    else {
        panic!("input selection");
    };
    let mut reader = input.into_reader();
    let verified = ConfigPackage::read(&mut reader).expect("verify export");
    let expected = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .full_backup_candidate()
        .expect("portable settings");
    assert_eq!(verified.settings().digest(), expected.digest());
    assert_eq!(verified.created_at_utc_ms(), receipt.created_at_utc_ms());

    fs::write(
        export_directory.as_path().join("occupied.tmconfig"),
        b"keep",
    )
    .expect("occupied target");
    let occupied_dialog = ControlledFileDialog::selected(&export_directory, "occupied.tmconfig")
        .expect("occupied output dialog");
    let FileDialogResult::Selected(occupied) =
        occupied_dialog.select_output(FileDialogFileType::Config)
    else {
        panic!("occupied output selection");
    };
    let second = command_permit(ApplicationCommand::ExportConfig);
    owner
        .export_config(&second, occupied, 1_721_234_567_891, || {})
        .expect("confirmed existing output is atomically replaced");
    assert_ne!(
        fs::read(export_directory.as_path().join("occupied.tmconfig")).expect("occupied bytes"),
        b"keep"
    );
}

#[test]
fn config_import_preview_is_bounded_and_commit_preserves_device_settings() {
    let (temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let import_directory = temporary.path().join("imports");
    fs::create_dir(&import_directory).expect("import directory");
    let import_directory =
        ValidatedLocalDirectory::new(&import_directory).expect("validated import directory");
    let target =
        DurableFileTarget::exact_child(&import_directory, "settings.tmconfig").expect("target");
    let mut staged = target
        .create_staged(MAX_CONFIG_PACKAGE_BYTES)
        .expect("config stage");
    let changed = changed_portable_settings();
    ConfigPackage::write(&changed, 1_721_234_567_890, &mut staged).expect("config package");
    staged.publish_new(&target).expect("publish config");
    let input_dialog = ControlledFileDialog::selected(&import_directory, "settings.tmconfig")
        .expect("input dialog");
    let FileDialogResult::Selected(source) =
        input_dialog.select_input(FileDialogFileType::Config, MAX_CONFIG_PACKAGE_BYTES)
    else {
        panic!("input selection");
    };
    let permit = command_permit(ApplicationCommand::ImportConfig);

    let preview = owner
        .preview_config_import(&permit, source)
        .expect("config preview");
    assert_eq!(preview.changed_category_count(), 1);
    assert_eq!(preview.changed_field_count(), 1);
    assert_eq!(
        preview.categories(),
        [SettingsChangeCategory::ReminderProfile]
    );
    assert_eq!(preview.created_at_utc_ms(), 1_721_234_567_890);
    assert!(preview.package_bytes() <= MAX_CONFIG_PACKAGE_BYTES);
    assert!(!format!("{preview:?}").contains(&temporary.path().display().to_string()));
    let before = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings before commit");
    assert_ne!(
        PortableSettingsCandidate::new(before.value().portable().clone())
            .expect("before candidate")
            .digest(),
        changed.digest()
    );

    let committed = owner
        .commit_config_import(&permit, preview)
        .expect("config commit");
    assert_eq!(committed.portable_digest(), changed.digest());
    assert!(!permit.is_cancelled());
    let after = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings after commit");
    assert_eq!(
        PortableSettingsCandidate::new(after.value().portable().clone())
            .expect("after candidate")
            .digest(),
        changed.digest()
    );
    assert_eq!(after.value().device(), before.value().device());
}

#[test]
fn pending_config_import_is_projected_cancelled_or_committed_without_paths() {
    let (temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let preflight = owner.prepare(&root).expect("state preflight");
    let import_directory = temporary.path().join("pending-import");
    fs::create_dir(&import_directory).expect("import directory");
    let import_directory =
        ValidatedLocalDirectory::new(&import_directory).expect("validated import directory");
    let target = DurableFileTarget::exact_child(&import_directory, "settings.tmconfig")
        .expect("config target");
    let mut staged = target
        .create_staged(MAX_CONFIG_PACKAGE_BYTES)
        .expect("config stage");
    let changed = changed_portable_settings();
    ConfigPackage::write(&changed, 1_721_234_567_890, &mut staged).expect("config package");
    staged.publish_new(&target).expect("publish config");

    let select_input = || {
        let dialog = ControlledFileDialog::selected(&import_directory, "settings.tmconfig")
            .expect("input dialog");
        let FileDialogResult::Selected(input) =
            dialog.select_input(FileDialogFileType::Config, MAX_CONFIG_PACKAGE_BYTES)
        else {
            panic!("input selection");
        };
        input
    };

    owner
        .stage_config_import_preview(
            &command_permit(ApplicationCommand::ImportConfig),
            select_input(),
        )
        .expect("stage pending import");
    let projected = owner
        .reliable_state_projection(preflight.report())
        .expect("project pending import");
    let preview = projected.config_import_preview().expect("pending preview");
    assert_eq!(preview.changed_category_count(), 1);
    assert_eq!(preview.changed_field_count(), 1);
    assert_eq!(preview.created_at_utc_ms(), 1_721_234_567_890);
    assert!(preview.package_bytes() <= MAX_CONFIG_PACKAGE_BYTES);
    assert!(!format!("{projected:?}").contains(&temporary.path().display().to_string()));

    owner
        .cancel_pending_config_import(&command_permit(ApplicationCommand::CancelConfigImport))
        .expect("cancel pending import");
    assert!(
        owner
            .reliable_state_projection(preflight.report())
            .expect("project cancelled import")
            .config_import_preview()
            .is_none()
    );

    let before = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings before commit");
    owner
        .stage_config_import_preview(
            &command_permit(ApplicationCommand::ImportConfig),
            select_input(),
        )
        .expect("restage pending import");
    let committed = owner
        .commit_pending_config_import(
            &command_permit(ApplicationCommand::ConfirmConfigImport),
            || {},
        )
        .expect("commit pending import");
    assert_eq!(committed.portable_digest(), changed.digest());
    let after = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings after commit");
    assert_eq!(
        PortableSettingsCandidate::new(after.value().portable().clone())
            .expect("after candidate")
            .digest(),
        changed.digest()
    );
    assert_eq!(after.value().device(), before.value().device());
    assert!(
        owner
            .reliable_state_projection(preflight.report())
            .expect("project committed import")
            .config_import_preview()
            .is_none()
    );
}

#[test]
fn compact_and_encrypted_exports_are_verified_bounded_and_path_private() {
    let (temporary, root) = fixture();
    drop(UsageStore::open(root.archive_path()).expect("create archive"));
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let export_directory = temporary.path().join("backup-exports");
    fs::create_dir(&export_directory).expect("export directory");
    let export_directory =
        ValidatedLocalDirectory::new(&export_directory).expect("validated export directory");

    let compact_dialog = ControlledFileDialog::selected(&export_directory, "compact.tmbackup")
        .expect("compact dialog");
    let FileDialogResult::Selected(compact_output) =
        compact_dialog.select_output(FileDialogFileType::Backup)
    else {
        panic!("compact output");
    };
    owner
        .export_compact_backup(
            &root,
            &command_permit(ApplicationCommand::BackupCompact),
            compact_output,
            || {},
        )
        .expect("compact export");
    let compact_input = ControlledFileDialog::selected(&export_directory, "compact.tmbackup")
        .expect("compact input");
    let FileDialogResult::Selected(compact_input) =
        compact_input.select_input(FileDialogFileType::Backup, MAX_DURABLE_FILE_BYTES)
    else {
        panic!("compact input selection");
    };
    let mut compact_reader = compact_input.into_reader();
    let compact = BackupPackage::inspect(&mut compact_reader).expect("verify compact export");
    assert_eq!(compact.compression(), BackupCompression::Compact);

    let encrypted_dialog =
        ControlledFileDialog::selected(&export_directory, "protected.tmbackup.age")
            .expect("encrypted dialog");
    let FileDialogResult::Selected(encrypted_output) =
        encrypted_dialog.select_output(FileDialogFileType::EncryptedBackup)
    else {
        panic!("encrypted output");
    };
    let mut passphrase_input = String::from("correct horse battery staple");
    let mut confirmation = passphrase_input.clone();
    let passphrase =
        BackupPassphrase::new(&mut passphrase_input, &mut confirmation).expect("backup passphrase");
    assert!(passphrase_input.is_empty());
    assert!(confirmation.is_empty());
    owner
        .export_encrypted_backup(
            &root,
            &command_permit(ApplicationCommand::BackupEncrypted),
            encrypted_output,
            passphrase,
            || {},
        )
        .expect("encrypted export");

    let encrypted_input =
        ControlledFileDialog::selected(&export_directory, "protected.tmbackup.age")
            .expect("encrypted input");
    let FileDialogResult::Selected(encrypted_input) =
        encrypted_input.select_input(FileDialogFileType::EncryptedBackup, MAX_DURABLE_FILE_BYTES)
    else {
        panic!("encrypted input selection");
    };
    let database_target = DurableFileTarget::exact_child(&export_directory, "decrypted.sqlite3")
        .expect("database target");
    let mut database_stage = database_target
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("database stage");
    let mut encrypted_reader = encrypted_input.into_reader();
    let mut existing = String::from("correct horse battery staple");
    let decrypted = EncryptedBackupPackage::decrypt(
        &mut encrypted_reader,
        BackupPassphrase::existing(&mut existing).expect("existing passphrase"),
        &mut database_stage,
    )
    .expect("decrypt exported backup");
    assert!(existing.is_empty());
    assert_eq!(decrypted.compression(), BackupCompression::Normal);
    database_stage
        .discard()
        .expect("discard decrypted database");

    let private_path = temporary.path().display().to_string();
    assert!(!format!("{compact:?}{decrypted:?}").contains(&private_path));
}

#[test]
fn backup_policy_update_accepts_only_exact_product_ranges() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let update = ApplicationBackupPolicyUpdate::new(true, 300, 21_600, 256);
    let policy = owner
        .update_backup_policy(
            &command_permit(ApplicationCommand::UpdateBackupPolicy),
            update,
            || {},
        )
        .expect("minimum valid policy");
    assert!(policy.periodic_enabled());
    assert_eq!(policy.quiet_seconds(), 300);
    assert_eq!(policy.interval_seconds(), 21_600);
    assert_eq!(policy.retention_budget_bytes(), 256 * 1024 * 1024);

    let persisted = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("persisted settings");
    assert_eq!(persisted.value().portable().backup(), &policy);

    for invalid in [
        ApplicationBackupPolicyUpdate::new(true, 299, 21_600, 256),
        ApplicationBackupPolicyUpdate::new(true, 300, 21_599, 256),
        ApplicationBackupPolicyUpdate::new(true, 300, 21_600, 255),
        ApplicationBackupPolicyUpdate::new(true, 3_601, 604_800, 65_536),
        ApplicationBackupPolicyUpdate::new(true, 3_600, 604_801, 65_536),
        ApplicationBackupPolicyUpdate::new(true, 3_600, 604_800, 65_537),
    ] {
        owner
            .update_backup_policy(
                &command_permit(ApplicationCommand::UpdateBackupPolicy),
                invalid,
                || {},
            )
            .expect_err("out-of-range policy must fail");
    }
}
