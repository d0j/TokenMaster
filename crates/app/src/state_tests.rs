use std::ffi::OsString;
use std::fs;

use tempfile::TempDir;
use tokenmaster_platform::{
    DurableFileTarget, ExclusiveFileLease, ExclusiveFileLeaseError, ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupPolicy, BootstrapOutcome, ConfigPackage, MAX_CONFIG_PACKAGE_BYTES, PortableSettings,
    PortableSettingsCandidate, ReminderPolicy, SettingsChangeCategory, SettingsStore,
    SettingsValue,
};

use crate::command::{
    ApplicationCommand, ApplicationCommandAdmission, ApplicationCommandCoordinator,
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
    let target =
        DurableFileTarget::exact_child(&export_directory, "settings.tmconfig").expect("target");
    let permit = command_permit(ApplicationCommand::ExportConfig);

    let receipt = owner
        .export_config(&permit, &target, 1_721_234_567_890)
        .expect("config export");
    assert!(!permit.is_cancelled());
    assert!(!permit.clone().begin_irreversible().is_ok());
    assert_eq!(receipt.created_at_utc_ms(), 1_721_234_567_890);
    assert!(receipt.package_bytes() > 0);
    assert!(receipt.package_bytes() <= MAX_CONFIG_PACKAGE_BYTES);
    assert!(!format!("{receipt:?}").contains(&temporary.path().display().to_string()));

    let mut reader = target
        .open_reader(MAX_CONFIG_PACKAGE_BYTES)
        .expect("open export")
        .expect("export exists");
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
    let occupied = DurableFileTarget::exact_child(&export_directory, "occupied.tmconfig")
        .expect("occupied target descriptor");
    let second = command_permit(ApplicationCommand::ExportConfig);
    assert!(
        owner
            .export_config(&second, &occupied, 1_721_234_567_891)
            .is_err()
    );
    assert_eq!(
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
    let source = target
        .open_reader(MAX_CONFIG_PACKAGE_BYTES)
        .expect("open import")
        .expect("import exists");
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
