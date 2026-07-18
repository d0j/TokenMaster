use std::time::{Duration, Instant};

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_platform::BackupDirectory;
use tokenmaster_product::ProductSectionKind;
use tokenmaster_state::{
    BackupCatalog, BackupMaintenanceRuntime, BackupPolicy, BackupPurpose, BootstrapOutcome,
    MaintenanceExecution, MaintenanceSourceState, PortableSettings, PriorRunCondition, RestoreMode,
    RunStateStore, SettingsStore, SettingsValue, SystemMaintenanceClock,
};
use tokenmaster_store::{USAGE_SCHEMA_VERSION, UsageStore};

use super::*;
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
        reminder: OptionalRuntime {
            owner: None,
            failure: Some(RuntimeErrorCode::StoreUnavailable),
        },
        controller,
        maintenance,
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

    let root = DataRoot::resolve(&environment).expect("data root");
    assert!(root.archive_path().exists());
    let bundle_slot = application.bundle.lock().expect("bundle slot");
    let maintenance = &bundle_slot.as_ref().expect("healthy bundle").maintenance;
    assert_eq!(
        maintenance.snapshot().worker().source_state(),
        MaintenanceSourceState::HealthyUnpublished
    );
    let manual = wait_for_mandatory_backup(maintenance, MaintenancePurpose::Manual);
    assert!(
        manual.is_ok(),
        "manual backup publication failed: {:?}",
        maintenance.snapshot()
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

    application
        .restart_services()
        .expect("controlled service restart");
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
    application
        .restore_selected(selected_restore, RestoreMode::DataOnly)
        .expect("identity-bound selected restore");
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
            point.purpose(),
            Some(BackupPurpose::Manual | BackupPurpose::PreRestore)
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
}

fn assert_migrated_archive_retries_pending_post_backup_before_live_restart() {
    let temporary = TempDir::new().expect("post-migration temporary directory");
    let environment = application_environment(&temporary);
    let root = DataRoot::resolve(&environment).expect("post-migration data root");
    create_exact_v12_archive(root.archive_path());
    disable_periodic_backups(&root);

    let mut failed = Application::start_with_observer(&environment, |boundary| match boundary {
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
