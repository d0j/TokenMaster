use std::ffi::OsString;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_desktop::{DesktopDensity, DesktopReliableStateHealth, DesktopReminderSyncState};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    UsageProviderId,
};
use tokenmaster_platform::{
    ControlledFileDialog, DurableFileTarget, ExclusiveFileLease, ExclusiveFileLeaseError,
    FileDialogFileType, FileDialogResult, FileDialogSelector, MAX_DURABLE_FILE_BYTES,
    ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCompression, BackupPackage, BackupPassphrase, BackupPolicy, BootstrapOutcome,
    ConfigPackage, EncryptedBackupPackage, MAX_CONFIG_PACKAGE_BYTES, PortableSettings,
    PortableSettingsCandidate, PresentationDensity, PresentationLayout, PresentationLocale,
    PresentationSettings, PresentationSkin, ReminderPolicy, SettingsChangeCategory, SettingsStore,
    SettingsValue,
};
use tokenmaster_store::UsageStore;

use crate::command::{
    ApplicationBackupPolicyUpdate, ApplicationCommand, ApplicationCommandAdmission,
    ApplicationCommandCoordinator,
};
use crate::state::ApplicationStateOwner;
use crate::{ApplicationEnvironment, ApplicationError, ApplicationErrorCode, DataRoot};

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
    PortableSettingsCandidate::new(PortableSettings::new(
        reminders,
        backup,
        PresentationSettings::new(
            PresentationDensity::UltraCompact,
            PresentationSkin::Refined,
            tokenmaster_state::PresentationColorScheme::System,
            PresentationLayout::Refined,
            PresentationLocale::English,
        ),
    ))
    .expect("portable candidate")
}

fn reminder_policy_update(enabled: bool, lead_seconds: &[u32]) -> ReminderPolicy {
    ReminderPolicy::new(enabled, lead_seconds).expect("reminder policy")
}

fn seed_real_reminder_archive(path: &std::path::Path) {
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_millis(),
    )
    .expect("wall clock");
    let scope = BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new("private-account").expect("account"),
        None,
    );
    let lot = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([7; 32]),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 2,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(now - 1_000),
        expiry: BenefitExpiry::exact_utc(now + 30 * 60 * 1_000).expect("expiry"),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot");
    let observation = BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id: BenefitObservationId::from_bytes([1; 32]),
        observed_at_ms: now,
        fresh_until_ms: now + 1_000,
        stale_after_ms: now + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots: vec![lot],
    })
    .expect("observation");
    UsageStore::open(path)
        .expect("store")
        .apply_benefit_observation(&observation)
        .expect("seed benefit");
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

fn persisted_reminder_leads(root: &DataRoot) -> Vec<u32> {
    SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("persisted settings")
        .value()
        .portable()
        .reminders()
        .lead_seconds()
        .to_vec()
}

#[test]
fn reminder_explicit_save_reuses_generation_for_an_identical_retry() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let mut irreversible_calls = 0;

    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600, 3_600]),
            || {
                irreversible_calls += 1;
                Ok(())
            },
        )
        .expect("first reminder save");
    let first_generation = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("saved settings")
        .generation()
        .expect("explicit generation");

    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[3_600, 21_600]),
            || {
                irreversible_calls += 1;
                Ok(())
            },
        )
        .expect("identical reminder retry");

    let persisted = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("saved settings");
    assert_eq!(persisted.generation(), Some(first_generation));
    assert_eq!(irreversible_calls, 1);
}

#[test]
fn reminder_save_marks_pending_before_the_settings_generation_changes_and_reverts_on_failed_publication()
 {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600]),
            || Ok(()),
        )
        .expect("initial reminder save");
    seed_real_reminder_archive(root.archive_path());
    owner
        .synchronize_reminder_profile(&root)
        .expect("initial reminder synchronization");
    let store = SettingsStore::new(root.reliable_state()).expect("settings store");
    let before = store.load().expect("settings before changed save");
    let before_generation = before.generation();
    let before_leads = before
        .value()
        .portable()
        .reminders()
        .lead_seconds()
        .to_vec();
    let mut observed_pending_before_save = false;

    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[10_800]),
            || {
                let projection = owner
                    .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
                    .expect("pending projection before save");
                assert_eq!(
                    projection.reminder_policy().sync_state(),
                    DesktopReminderSyncState::Pending
                );
                let persisted = store.load().expect("settings before irreversible save");
                assert_eq!(persisted.generation(), before_generation);
                assert_eq!(
                    persisted.value().portable().reminders().lead_seconds(),
                    before_leads
                );
                observed_pending_before_save = true;
                Ok(())
            },
        )
        .expect("changed reminder save");
    assert!(observed_pending_before_save);
    let after = store.load().expect("settings after changed save");
    assert_ne!(after.generation(), before_generation);
    assert_eq!(
        after.value().portable().reminders().lead_seconds(),
        &[10_800]
    );
    owner
        .synchronize_reminder_profile(&root)
        .expect("changed reminder synchronization");

    let failed_before = store.load().expect("settings before failed publication");
    let error = owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[3_600]),
            || Err(ApplicationError::state()),
        )
        .expect_err("failed publication must stop the settings save");
    assert_eq!(error.code(), ApplicationErrorCode::StateUnavailable);
    let failed_after = store.load().expect("settings after failed publication");
    assert_eq!(failed_after.generation(), failed_before.generation());
    assert_eq!(
        failed_after.value().portable().reminders(),
        failed_before.value().portable().reminders()
    );
    assert_eq!(
        owner
            .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
            .expect("restored projection")
            .reminder_policy()
            .sync_state(),
        DesktopReminderSyncState::Synchronized
    );
}

#[test]
fn reminder_defaults_without_a_record_project_pending_and_synchronize_at_revision_one() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");

    let before = owner
        .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
        .expect("pending reliable projection");
    assert_eq!(
        before.reminder_policy().sync_state(),
        DesktopReminderSyncState::Pending
    );
    assert!(before.reminder_policy().enabled());
    assert_eq!(
        before.reminder_policy().lead_seconds(),
        &[604_800, 86_400, 43_200, 21_600, 3_600]
    );

    seed_real_reminder_archive(root.archive_path());
    let profile = owner
        .synchronize_reminder_profile(&root)
        .expect("default reminder synchronization");
    assert_eq!(profile.revision().get(), 1);
    assert_eq!(
        profile
            .lead_times()
            .iter()
            .map(|lead| lead.seconds())
            .collect::<Vec<_>>(),
        vec![604_800, 86_400, 43_200, 21_600, 3_600]
    );
}

#[test]
fn reminder_synchronization_projects_settings_generation_to_exact_sqlite_profile() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600, 3_600]),
            || Ok(()),
        )
        .expect("reminder save");
    seed_real_reminder_archive(root.archive_path());

    let profile = owner
        .synchronize_reminder_profile(&root)
        .expect("reminder synchronization");
    assert_eq!(profile.revision().get(), 2);
    assert_eq!(
        profile
            .lead_times()
            .iter()
            .map(|lead| lead.seconds())
            .collect::<Vec<_>>(),
        vec![21_600, 3_600]
    );
    assert_eq!(
        profile.channels(),
        &[tokenmaster_domain::NotificationChannel::InApp]
    );

    let connection = rusqlite::Connection::open(root.archive_path()).expect("sqlite archive");
    let global = connection
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
    assert_eq!(global, (2, 1, 0));
    assert_eq!(leads, vec![21_600, 3_600]);

    let projection = owner
        .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
        .expect("reliable projection");
    assert_eq!(
        projection.reminder_policy().sync_state(),
        DesktopReminderSyncState::Synchronized
    );
    assert!(projection.reminder_policy().enabled());
    assert_eq!(
        projection.reminder_policy().lead_seconds(),
        &[21_600, 3_600]
    );
}

#[test]
fn reminder_disabled_policy_synchronizes_without_channels_or_leads() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(false, &[]),
            || Ok(()),
        )
        .expect("disabled reminder save");
    seed_real_reminder_archive(root.archive_path());

    let profile = owner
        .synchronize_reminder_profile(&root)
        .expect("disabled synchronization");
    assert!(profile.channels().is_empty());
    assert!(profile.lead_times().is_empty());

    let projection = owner
        .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
        .expect("reliable projection");
    assert!(!projection.reminder_policy().enabled());
    assert!(projection.reminder_policy().lead_seconds().is_empty());
    assert_eq!(
        projection.reminder_policy().sync_state(),
        DesktopReminderSyncState::Synchronized
    );
}

#[test]
fn reminder_failed_archive_sync_preserves_durable_settings_and_projects_pending() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600]),
            || Ok(()),
        )
        .expect("reminder save");
    fs::create_dir(root.archive_path()).expect("unusable archive path");

    let error = owner
        .synchronize_reminder_profile(&root)
        .expect_err("archive synchronization must fail");
    assert_eq!(error.code(), ApplicationErrorCode::StateUnavailable);
    assert_eq!(error.to_string(), "state_unavailable");

    let persisted = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("durable settings");
    assert!(persisted.value().portable().reminders().enabled());
    assert_eq!(
        persisted.value().portable().reminders().lead_seconds(),
        &[21_600]
    );
    let projection = owner
        .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
        .expect("reliable projection");
    assert_eq!(
        projection.reminder_policy().sync_state(),
        DesktopReminderSyncState::Pending
    );
    assert!(projection.reminder_policy().enabled());
    assert_eq!(projection.reminder_policy().lead_seconds(), &[21_600]);
}

#[test]
fn reminder_changed_save_failure_after_synchronization_reopens_as_pending() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    seed_real_reminder_archive(root.archive_path());
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600]),
            || Ok(()),
        )
        .expect("initial reminder save");
    owner
        .synchronize_reminder_profile(&root)
        .expect("initial synchronization");
    assert_eq!(
        owner
            .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
            .expect("synchronized projection")
            .reminder_policy()
            .sync_state(),
        DesktopReminderSyncState::Synchronized
    );

    let blocked_slot = root.reliable_state().as_path().join("settings-b.tms");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[10_800]),
            || {
                fs::create_dir(&blocked_slot).expect("block next redundant slot");
                Ok(())
            },
        )
        .expect_err("blocked changed save");

    fs::remove_dir(&blocked_slot).expect("remove test fault");
    let projection = owner
        .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
        .expect("failed-save projection");
    assert_eq!(
        projection.reminder_policy().sync_state(),
        DesktopReminderSyncState::Pending
    );
    assert_eq!(projection.reminder_policy().lead_seconds(), &[21_600]);

    let reopened = ApplicationStateOwner::open(&root).expect("reopen state owner");
    let reopened_projection = reopened
        .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
        .expect("reopened projection");
    assert_eq!(
        reopened_projection.reminder_policy().sync_state(),
        DesktopReminderSyncState::Pending
    );
    assert_eq!(
        reopened_projection.reminder_policy().lead_seconds(),
        &[21_600]
    );
}

#[test]
fn reminder_missing_archive_is_not_created_or_synchronized() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600]),
            || Ok(()),
        )
        .expect("reminder save");

    owner
        .synchronize_reminder_profile(&root)
        .expect_err("missing archive must be unavailable");

    assert!(!root.archive_path().exists());
    assert!(!root.archive_path().with_extension("sqlite3-wal").exists());
    assert!(!root.archive_path().with_extension("sqlite3-shm").exists());
    assert_eq!(persisted_reminder_leads(&root), vec![21_600]);
    assert_eq!(
        owner
            .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
            .expect("pending projection")
            .reminder_policy()
            .sync_state(),
        DesktopReminderSyncState::Pending
    );
}

#[test]
fn reminder_supported_legacy_archive_is_not_migrated_or_synchronized() {
    let (_temporary, root) = fixture();
    create_exact_v12_archive(root.archive_path());
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600]),
            || Ok(()),
        )
        .expect("reminder save");
    let before = fs::read(root.archive_path()).expect("legacy archive bytes");
    let before_version = Connection::open(root.archive_path())
        .expect("legacy archive")
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .expect("legacy schema version");

    owner
        .synchronize_reminder_profile(&root)
        .expect_err("legacy archive must be unavailable");

    assert_eq!(
        fs::read(root.archive_path()).expect("legacy archive bytes"),
        before
    );
    let after_version = Connection::open(root.archive_path())
        .expect("legacy archive")
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .expect("legacy schema version");
    assert_eq!(before_version, 12);
    assert_eq!(after_version, before_version);
    assert_eq!(persisted_reminder_leads(&root), vec![21_600]);
}

#[test]
fn reminder_current_archive_write_contention_preserves_profile_and_projects_pending() {
    let (_temporary, root) = fixture();
    seed_real_reminder_archive(root.archive_path());
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder_policy_update(true, &[21_600]),
            || Ok(()),
        )
        .expect("reminder save");
    let before_profile = Connection::open(root.archive_path())
        .expect("current archive")
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
        .expect("global profile");
    let writer = Connection::open(root.archive_path()).expect("writer connection");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("writer transaction");

    owner
        .synchronize_reminder_profile(&root)
        .expect_err("contended write must fail");

    let during_profile = Connection::open(root.archive_path())
        .expect("current archive")
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
        .expect("unchanged global profile");
    assert_eq!(during_profile, before_profile);
    assert_eq!(persisted_reminder_leads(&root), vec![21_600]);
    assert_eq!(
        owner
            .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
            .expect("pending projection")
            .reminder_policy()
            .sync_state(),
        DesktopReminderSyncState::Pending
    );
    writer.execute_batch("ROLLBACK").expect("release writer");
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
    assert_eq!(preview.changed_category_count(), 2);
    assert_eq!(preview.changed_field_count(), 2);
    assert_eq!(
        preview.categories(),
        [
            SettingsChangeCategory::ReminderProfile,
            SettingsChangeCategory::Presentation,
        ]
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
    assert_eq!(
        owner
            .reliable_state_projection_for_outcome(BootstrapOutcome::FirstInstall, None)
            .expect("post-import projection")
            .presentation()
            .density(),
        DesktopDensity::UltraCompact
    );
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
    assert_eq!(preview.changed_category_count(), 2);
    assert_eq!(preview.changed_field_count(), 2);
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
    let before_generation = before.generation();
    let before_digest = PortableSettingsCandidate::new(before.value().portable().clone())
        .expect("before candidate")
        .digest();
    let mut observed_pending_before_commit = false;
    let committed = owner
        .commit_pending_config_import(
            &command_permit(ApplicationCommand::ConfirmConfigImport),
            || {
                let projection = owner
                    .reliable_state_projection(preflight.report())
                    .expect("pending projection before import commit");
                assert_eq!(
                    projection.reminder_policy().sync_state(),
                    DesktopReminderSyncState::Pending
                );
                assert!(projection.config_import_preview().is_some());
                let persisted = SettingsStore::new(root.reliable_state())
                    .expect("settings store")
                    .load()
                    .expect("settings before irreversible import");
                assert_eq!(persisted.generation(), before_generation);
                assert_eq!(
                    PortableSettingsCandidate::new(persisted.value().portable().clone())
                        .expect("persisted candidate")
                        .digest(),
                    before_digest
                );
                observed_pending_before_commit = true;
                Ok(())
            },
        )
        .expect("commit pending import");
    assert!(observed_pending_before_commit);
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

    owner
        .stage_config_import_preview(
            &command_permit(ApplicationCommand::ImportConfig),
            select_input(),
        )
        .expect("stage import for failed publication");
    let failure_before = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings before failed import publication");
    let error = owner
        .commit_pending_config_import(
            &command_permit(ApplicationCommand::ConfirmConfigImport),
            || Err(ApplicationError::state()),
        )
        .expect_err("failed publication must preserve pending import");
    assert_eq!(error.code(), ApplicationErrorCode::StateUnavailable);
    let failure_after = SettingsStore::new(root.reliable_state())
        .expect("settings store")
        .load()
        .expect("settings after failed import publication");
    assert_eq!(failure_after.generation(), failure_before.generation());
    assert!(
        owner
            .reliable_state_projection(preflight.report())
            .expect("failed pending import projection")
            .config_import_preview()
            .is_some()
    );
    owner
        .commit_pending_config_import(
            &command_permit(ApplicationCommand::ConfirmConfigImport),
            || Ok(()),
        )
        .expect("preserved pending import remains confirmable");
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

#[test]
fn presentation_update_preserves_every_other_settings_class() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    owner
        .update_presentation(
            &command_permit(ApplicationCommand::UpdatePresentation),
            PresentationSettings::new(
                PresentationDensity::UltraCompact,
                PresentationSkin::Refined,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::Refined,
                PresentationLocale::English,
            ),
            || {},
        )
        .expect("seed ultra compact density");
    owner
        .update_backup_policy(
            &command_permit(ApplicationCommand::UpdateBackupPolicy),
            ApplicationBackupPolicyUpdate::new(true, 300, 21_600, 256),
            || {},
        )
        .expect("save backup policy");
    let reminder = ReminderPolicy::new(true, &[21_600, 3_600]).expect("reminder policy");
    owner
        .update_reminder_policy(
            &command_permit(ApplicationCommand::UpdateReminderPolicy),
            reminder.clone(),
            || Ok(()),
        )
        .expect("save reminder policy");

    let store = SettingsStore::new(root.reliable_state()).expect("settings store");
    let before = store.load().expect("settings before density update");
    assert_eq!(
        before.value().portable().presentation().density(),
        PresentationDensity::UltraCompact
    );
    let before_reminders = before.value().portable().reminders().clone();
    let before_backup = before.value().portable().backup().clone();
    let before_device = before.value().device().clone();

    owner
        .update_presentation(
            &command_permit(ApplicationCommand::UpdatePresentation),
            PresentationSettings::new(
                PresentationDensity::Compact,
                PresentationSkin::Ember,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::Workbench,
                PresentationLocale::English,
            ),
            || {},
        )
        .expect("save compact density");
    let after = store.load().expect("settings after density update");
    assert_eq!(
        after.value().portable().presentation().density(),
        PresentationDensity::Compact
    );
    assert_eq!(
        after.value().portable().presentation().skin(),
        PresentationSkin::Ember
    );
    assert_eq!(
        after.value().portable().presentation().layout(),
        PresentationLayout::Workbench
    );
    assert_eq!(after.value().portable().reminders(), &before_reminders);
    assert_eq!(after.value().portable().backup(), &before_backup);
    assert_eq!(after.value().device(), &before_device);
}

#[test]
fn presentation_update_replaces_all_axes_atomically() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let store = SettingsStore::new(root.reliable_state()).expect("settings store");
    let current = store.load().expect("initial settings");
    let graphite = SettingsValue::new(
        PortableSettings::new(
            current.value().portable().reminders().clone(),
            current.value().portable().backup().clone(),
            PresentationSettings::new(
                PresentationDensity::Comfortable,
                PresentationSkin::Graphite,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::Refined,
                PresentationLocale::English,
            ),
        ),
        current.value().device().clone(),
    );
    store.save(&graphite).expect("seed graphite skin");

    owner
        .update_presentation(
            &command_permit(ApplicationCommand::UpdatePresentation),
            PresentationSettings::new(
                PresentationDensity::Compact,
                PresentationSkin::Ember,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::ControlCenter,
                PresentationLocale::English,
            ),
            || {},
        )
        .expect("save compact density");

    let saved = store.load().expect("saved settings");
    assert_eq!(
        saved.value().portable().presentation().density(),
        PresentationDensity::Compact
    );
    assert_eq!(
        saved.value().portable().presentation().skin(),
        PresentationSkin::Ember
    );
    assert_eq!(
        saved.value().portable().presentation().layout(),
        PresentationLayout::ControlCenter
    );
}

#[test]
fn presentation_save_rejects_wrong_or_cancelled_permits_and_is_idempotent_only_for_exact_pair() {
    let (_temporary, root) = fixture();
    let owner = ApplicationStateOwner::open(&root).expect("state owner");
    let store = SettingsStore::new(root.reliable_state()).expect("settings store");
    let before = store.load().expect("initial settings");
    let mut callback_count = 0;

    owner
        .update_presentation(
            &command_permit(ApplicationCommand::UpdateBackupPolicy),
            PresentationSettings::new(
                PresentationDensity::Compact,
                PresentationSkin::Graphite,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::Refined,
                PresentationLocale::English,
            ),
            || callback_count += 1,
        )
        .expect_err("wrong permit");
    assert_eq!(callback_count, 0);
    assert_eq!(
        store.load().expect("wrong permit settings").generation(),
        before.generation()
    );

    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(cancelled) =
        coordinator.submit(ApplicationCommand::UpdatePresentation)
    else {
        panic!("presentation permit");
    };
    assert!(coordinator.cancel(cancelled.id()));
    owner
        .update_presentation(
            &cancelled,
            PresentationSettings::new(
                PresentationDensity::Compact,
                PresentationSkin::Graphite,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::Refined,
                PresentationLocale::English,
            ),
            || {
                callback_count += 1;
            },
        )
        .expect_err("cancelled permit");
    assert_eq!(callback_count, 0);
    assert_eq!(
        store.load().expect("cancelled settings").generation(),
        before.generation()
    );

    owner
        .update_presentation(
            &command_permit(ApplicationCommand::UpdatePresentation),
            PresentationSettings::new(
                PresentationDensity::Comfortable,
                PresentationSkin::Refined,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::Refined,
                PresentationLocale::English,
            ),
            || callback_count += 1,
        )
        .expect("exact equal selection is idempotent");
    assert_eq!(callback_count, 0);
    assert_eq!(
        store.load().expect("equal settings").generation(),
        before.generation()
    );

    owner
        .update_presentation(
            &command_permit(ApplicationCommand::UpdatePresentation),
            PresentationSettings::new(
                PresentationDensity::Comfortable,
                PresentationSkin::Graphite,
                tokenmaster_state::PresentationColorScheme::System,
                PresentationLayout::Refined,
                PresentationLocale::English,
            ),
            || callback_count += 1,
        )
        .expect("changed skin is durable mutation");
    assert_eq!(callback_count, 1);
    assert_eq!(
        store
            .load()
            .expect("changed settings")
            .value()
            .portable()
            .presentation()
            .density(),
        PresentationDensity::Comfortable
    );
    assert_eq!(
        store
            .load()
            .expect("changed settings")
            .value()
            .portable()
            .presentation()
            .skin(),
        PresentationSkin::Graphite
    );
}
