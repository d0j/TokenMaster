use std::fmt;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, BackupDirectoryError, DurableFileReader,
    DurableFileTarget, ExclusiveFileLease, ExclusiveFileLeaseGuard, MAX_DURABLE_FILE_BYTES,
    ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCatalog, BackupCompression, BackupMaintenanceRuntime, BackupMetadata, BackupPackage,
    BootstrapReport, CatalogSelectionBinding, ConfigPackage, MAX_CONFIG_PACKAGE_BYTES,
    MaintenanceExecution, MaintenancePermit, MaintenanceSourceState, PendingMigration,
    PreparedBootstrap, RecoveryCoordinator, RecoveryJournalStore, RecoveryLaunchDecision,
    RecoveryReceipt, RestoreMode, RestoreSafety, RetentionAdmission, RetentionPolicy, RunSession,
    RunStateStore, SettingsChangeCategory, SettingsCommitReceipt, SettingsImportPreview,
    SettingsStore, StateBootstrap, StateErrorCode, SystemMaintenanceClock,
};
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, StartupArchiveStatus, StartupValidationMode,
    StoreErrorCode, USAGE_SCHEMA_VERSION, create_online_snapshot, inspect_startup_archive,
    verify_backup_candidate,
};

use crate::command::{ApplicationBackupSelection, ApplicationCommand, ApplicationCommandPermit};
use crate::{ApplicationError, DataRoot};

#[cfg(test)]
use tokenmaster_state::BackupPurpose;

const STARTUP_RECOVERY_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(crate) struct ApplicationStateOwner {
    scope: ArchiveRecoveryScope,
    backups: BackupDirectory,
    verification_staging: BackupStaging,
    settings: SettingsStore,
    journal: RecoveryJournalStore,
    run_state: RunStateStore,
    catalog: Arc<Mutex<Option<Arc<BackupCatalog>>>>,
    restore_pin: Arc<Mutex<Option<CatalogSelectionBinding>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 12B.2b config worker binding follows the sealed config operations"
    )
)]
pub(crate) struct ApplicationConfigExportReceipt {
    created_at_utc_ms: i64,
    package_bytes: u64,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 12B.2b config worker binding follows the sealed config operations"
    )
)]
impl ApplicationConfigExportReceipt {
    #[must_use]
    pub(crate) const fn created_at_utc_ms(self) -> i64 {
        self.created_at_utc_ms
    }

    #[must_use]
    pub(crate) const fn package_bytes(self) -> u64 {
        self.package_bytes
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 12B.2b config worker binding follows the sealed config operations"
    )
)]
pub(crate) struct ApplicationConfigImportPreview {
    settings: SettingsImportPreview,
    created_at_utc_ms: i64,
    package_bytes: u64,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 12B.2b config worker binding follows the sealed config operations"
    )
)]
impl ApplicationConfigImportPreview {
    #[must_use]
    pub(crate) fn changed_category_count(&self) -> usize {
        self.settings.changed_category_count()
    }

    #[must_use]
    pub(crate) const fn changed_field_count(&self) -> usize {
        self.settings.changed_field_count()
    }

    #[must_use]
    pub(crate) fn categories(&self) -> &[SettingsChangeCategory] {
        self.settings.categories()
    }

    #[must_use]
    pub(crate) const fn created_at_utc_ms(&self) -> i64 {
        self.created_at_utc_ms
    }

    #[must_use]
    pub(crate) const fn package_bytes(&self) -> u64 {
        self.package_bytes
    }
}

impl fmt::Debug for ApplicationConfigImportPreview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationConfigImportPreview")
            .field("changed_category_count", &self.changed_category_count())
            .field("changed_field_count", &self.changed_field_count())
            .field("created_at_utc_ms", &self.created_at_utc_ms)
            .field("package_bytes", &self.package_bytes)
            .finish()
    }
}

impl ApplicationStateOwner {
    pub(crate) fn open(root: &DataRoot) -> Result<Self, ApplicationError> {
        let scope = ArchiveRecoveryScope::new(root.validated_directory(), root.reliable_state())
            .map_err(|_| ApplicationError::state())?;
        let staging = scope
            .staging_root()
            .map_err(|_| ApplicationError::state())?;
        let verification_staging =
            BackupStaging::new(&staging).map_err(|_| ApplicationError::state())?;
        let backups = BackupDirectory::open_or_create(root.reliable_state())
            .map_err(|_| ApplicationError::state())?;
        let settings =
            SettingsStore::new(root.reliable_state()).map_err(|_| ApplicationError::state())?;
        let journal = RecoveryJournalStore::new(root.reliable_state())
            .map_err(|_| ApplicationError::state())?;
        let run_state =
            RunStateStore::new(root.reliable_state()).map_err(|_| ApplicationError::state())?;
        Ok(Self {
            scope,
            backups,
            verification_staging,
            settings,
            journal,
            run_state,
            catalog: Arc::new(Mutex::new(None)),
            restore_pin: Arc::new(Mutex::new(None)),
        })
    }

    pub(crate) fn prepare(
        &self,
        root: &DataRoot,
    ) -> Result<ApplicationPreflight, ApplicationError> {
        let lease = ExclusiveFileLease::for_archive(root.archive_path())
            .map_err(|_| ApplicationError::state())?;
        let guard = lease.try_acquire().map_err(|_| ApplicationError::state())?;
        let control =
            BackupControl::new(Arc::new(AtomicBool::new(false)), STARTUP_RECOVERY_TIMEOUT)
                .map_err(|_| ApplicationError::state())?;
        let bootstrap = StateBootstrap::new(
            root.validated_directory(),
            &self.scope,
            &self.verification_staging,
            &self.journal,
            &self.settings,
            &self.run_state,
            &self.backups,
        )
        .map_err(|_| ApplicationError::state())?
        .prepare(&guard, &control)
        .map_err(|_| ApplicationError::state())?;
        Ok(ApplicationPreflight {
            bootstrap,
            startup_guard: Some(guard),
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Task 12B.2b config worker binding follows the sealed config operations"
        )
    )]
    pub(crate) fn export_config(
        &self,
        permit: &ApplicationCommandPermit,
        target: &DurableFileTarget,
        created_at_utc_ms: i64,
    ) -> Result<ApplicationConfigExportReceipt, ApplicationError> {
        if permit.command() != ApplicationCommand::ExportConfig || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let settings = self
            .settings
            .full_backup_candidate()
            .map_err(|_| ApplicationError::state())?;
        let mut staged = target
            .create_staged(MAX_CONFIG_PACKAGE_BYTES)
            .map_err(|_| ApplicationError::state())?;
        let package = ConfigPackage::write(&settings, created_at_utc_ms, &mut staged)
            .map_err(|_| ApplicationError::state())?;
        if permit.is_cancelled() {
            staged.discard().map_err(|_| ApplicationError::state())?;
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        let published = staged
            .publish_new(target)
            .map_err(|_| ApplicationError::state())?;
        if published.len() != package.package_len() {
            return Err(ApplicationError::state());
        }
        let mut reader = target
            .open_reader(MAX_CONFIG_PACKAGE_BYTES)
            .map_err(|_| ApplicationError::state())?
            .ok_or_else(ApplicationError::state)?;
        let verified = ConfigPackage::read(&mut reader).map_err(|_| ApplicationError::state())?;
        if verified.receipt() != package || verified.settings().digest() != settings.digest() {
            return Err(ApplicationError::state());
        }
        Ok(ApplicationConfigExportReceipt {
            created_at_utc_ms,
            package_bytes: package.package_len(),
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Task 12B.2b config worker binding follows the sealed config operations"
        )
    )]
    pub(crate) fn preview_config_import(
        &self,
        permit: &ApplicationCommandPermit,
        mut source: DurableFileReader,
    ) -> Result<ApplicationConfigImportPreview, ApplicationError> {
        if permit.command() != ApplicationCommand::ImportConfig || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let verified = ConfigPackage::read(&mut source).map_err(|_| ApplicationError::state())?;
        if permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let settings = self
            .settings
            .preview_candidate(verified.settings().clone())
            .map_err(|_| ApplicationError::state())?;
        Ok(ApplicationConfigImportPreview {
            settings,
            created_at_utc_ms: verified.created_at_utc_ms(),
            package_bytes: verified.receipt().package_len(),
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Task 12B.2b config worker binding follows the sealed config operations"
        )
    )]
    pub(crate) fn commit_config_import(
        &self,
        permit: &ApplicationCommandPermit,
        preview: ApplicationConfigImportPreview,
    ) -> Result<SettingsCommitReceipt, ApplicationError> {
        if permit.command() != ApplicationCommand::ImportConfig || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        self.settings
            .commit_import(&preview.settings)
            .map_err(|_| ApplicationError::state())
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 12B command worker binds controlled restart")
    )]
    pub(crate) fn acquire_runtime_guard(
        &self,
        root: &DataRoot,
    ) -> Result<ExclusiveFileLeaseGuard, ApplicationError> {
        ExclusiveFileLease::for_archive(root.archive_path())
            .and_then(|lease| lease.try_acquire())
            .map_err(|_| ApplicationError::state())
    }

    pub(crate) fn start_maintenance(
        &self,
        root: &DataRoot,
        source_state: MaintenanceSourceState,
    ) -> Result<BackupMaintenanceRuntime, ApplicationError> {
        self.start_maintenance_inner(root, source_state, None)
    }

    pub(crate) fn start_protected_maintenance(
        &self,
        root: &DataRoot,
        source_state: MaintenanceSourceState,
        protected: CatalogSelectionBinding,
    ) -> Result<BackupMaintenanceRuntime, ApplicationError> {
        self.start_maintenance_inner(root, source_state, Some(protected))
    }

    fn start_maintenance_inner(
        &self,
        root: &DataRoot,
        source_state: MaintenanceSourceState,
        protected: Option<CatalogSelectionBinding>,
    ) -> Result<BackupMaintenanceRuntime, ApplicationError> {
        let settings = self
            .settings
            .load()
            .map_err(|_| ApplicationError::state())?;
        let policy = settings.value().portable().backup().clone();
        let retention = RetentionPolicy::new(policy.retention_budget_bytes())
            .map_err(|_| ApplicationError::state())?;
        let mut operation = ApplicationBackupOperation::open(
            root,
            retention,
            Arc::clone(&self.catalog),
            Arc::clone(&self.restore_pin),
            protected,
        )?;
        BackupMaintenanceRuntime::spawn(
            Arc::new(SystemMaintenanceClock::new()),
            policy,
            source_state,
            move |permit| operation.execute(permit),
        )
        .map_err(|_| ApplicationError::state())
    }

    #[cfg(test)]
    pub(crate) fn oldest_verified_backup_selection(
        &self,
    ) -> Result<ApplicationBackupSelection, ApplicationError> {
        let catalog = self.catalog_snapshot()?;
        let point = catalog
            .points()
            .iter()
            .rev()
            .find(|point| point.health() == tokenmaster_state::CatalogHealth::Verified)
            .ok_or_else(ApplicationError::state)?;
        ApplicationBackupSelection::new(
            point.selection().generation().get(),
            point.selection().ordinal(),
        )
        .ok_or_else(ApplicationError::state)
    }

    #[cfg(test)]
    pub(crate) fn verified_backup_selection(
        &self,
        purpose: BackupPurpose,
        schema_version: u16,
    ) -> Result<ApplicationBackupSelection, ApplicationError> {
        let catalog = self.catalog_snapshot()?;
        let point = catalog
            .points()
            .iter()
            .find(|point| {
                point.health() == tokenmaster_state::CatalogHealth::Verified
                    && point.purpose() == Some(purpose)
                    && point.database_schema_version() == Some(schema_version)
            })
            .ok_or_else(ApplicationError::state)?;
        ApplicationBackupSelection::new(
            point.selection().generation().get(),
            point.selection().ordinal(),
        )
        .ok_or_else(ApplicationError::state)
    }

    pub(crate) fn bind_backup_selection(
        &self,
        selection: ApplicationBackupSelection,
    ) -> Result<ApplicationBackupSelectionPin, ApplicationError> {
        let mut restore_pin = self
            .restore_pin
            .lock()
            .map_err(|_| ApplicationError::state())?;
        if restore_pin.is_some() {
            return Err(ApplicationError::state());
        }
        let catalog = self.catalog_snapshot()?;
        let point = catalog
            .points()
            .get(usize::from(selection.ordinal()))
            .filter(|point| {
                point.selection().generation().get() == selection.catalog_generation()
                    && point.selection().ordinal() == selection.ordinal()
            })
            .ok_or_else(ApplicationError::state)?;
        let binding = catalog
            .bind_current_selection(&self.backups, point.selection())
            .map_err(|_| ApplicationError::state())?;
        *restore_pin = Some(binding);
        Ok(ApplicationBackupSelectionPin {
            binding,
            shared: Arc::clone(&self.restore_pin),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore_selected(
        &self,
        binding: CatalogSelectionBinding,
        mode: RestoreMode,
        safety: RestoreSafety,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
    ) -> Result<RecoveryReceipt, ApplicationError> {
        let previous = self.catalog_snapshot()?;
        let mut catalog = BackupCatalog::rebuild(&self.backups, Some(previous.as_ref()))
            .map_err(|_| ApplicationError::state())?;
        catalog
            .verify_all_packages(&self.backups)
            .map_err(|_| ApplicationError::state())?;
        let selection = catalog
            .resolve_binding(binding)
            .map_err(|_| ApplicationError::state())?;
        let catalog = Arc::new(catalog);
        *self.catalog.lock().map_err(|_| ApplicationError::state())? = Some(Arc::clone(&catalog));
        RecoveryCoordinator::new(
            &self.scope,
            &self.verification_staging,
            &self.journal,
            &self.settings,
        )
        .and_then(|recovery| {
            recovery.restore_selected(
                &self.backups,
                catalog.as_ref(),
                selection,
                mode,
                safety,
                guard,
                control,
            )
        })
        .map_err(|_| ApplicationError::state())
    }

    fn catalog_snapshot(&self) -> Result<Arc<BackupCatalog>, ApplicationError> {
        self.catalog
            .lock()
            .map_err(|_| ApplicationError::state())?
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(ApplicationError::state)
    }

    pub(crate) fn migration_versions(
        &self,
        root: &DataRoot,
    ) -> Result<(u16, u16), ApplicationError> {
        let inspection =
            inspect_startup_archive(root.validated_directory(), StartupValidationMode::Normal)
                .map_err(|_| ApplicationError::state())?;
        if inspection.status() != StartupArchiveStatus::SupportedLegacy {
            return Err(ApplicationError::state());
        }
        let from = inspection
            .schema_version()
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(ApplicationError::state)?;
        let to = u16::try_from(USAGE_SCHEMA_VERSION).map_err(|_| ApplicationError::state())?;
        if from >= to {
            return Err(ApplicationError::state());
        }
        Ok((from, to))
    }

    pub(crate) fn restored_archive_requires_migration(
        &self,
        root: &DataRoot,
    ) -> Result<bool, ApplicationError> {
        let inspection =
            inspect_startup_archive(root.validated_directory(), StartupValidationMode::Normal)
                .map_err(|_| ApplicationError::state())?;
        match inspection.status() {
            StartupArchiveStatus::Current => Ok(false),
            StartupArchiveStatus::SupportedLegacy => Ok(true),
            StartupArchiveStatus::Missing => Err(ApplicationError::state()),
        }
    }

    pub(crate) fn validate_pending_migration(
        &self,
        pending: PendingMigration,
    ) -> Result<(), ApplicationError> {
        let current = u16::try_from(USAGE_SCHEMA_VERSION).map_err(|_| ApplicationError::state())?;
        if pending.to_schema_version() != current {
            return Err(ApplicationError::state());
        }
        Ok(())
    }
}

impl fmt::Debug for ApplicationStateOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationStateOwner([redacted])")
    }
}

pub(crate) struct ApplicationBackupSelectionPin {
    binding: CatalogSelectionBinding,
    shared: Arc<Mutex<Option<CatalogSelectionBinding>>>,
}

impl ApplicationBackupSelectionPin {
    pub(crate) const fn binding(&self) -> CatalogSelectionBinding {
        self.binding
    }
}

impl Drop for ApplicationBackupSelectionPin {
    fn drop(&mut self) {
        let Ok(mut shared) = self.shared.lock() else {
            return;
        };
        if *shared == Some(self.binding) {
            *shared = None;
        }
    }
}

impl fmt::Debug for ApplicationBackupSelectionPin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationBackupSelectionPin([redacted])")
    }
}

pub(crate) struct ApplicationPreflight {
    bootstrap: PreparedBootstrap,
    startup_guard: Option<ExclusiveFileLeaseGuard>,
}

impl ApplicationPreflight {
    pub(crate) fn report(&self) -> BootstrapReport {
        self.bootstrap.report()
    }

    pub(crate) fn take_startup_guard(
        &mut self,
    ) -> Result<ExclusiveFileLeaseGuard, ApplicationError> {
        self.startup_guard
            .take()
            .ok_or_else(ApplicationError::state)
    }

    pub(crate) fn session_mut(&mut self) -> &mut RunSession {
        self.bootstrap.session_mut()
    }

    pub(crate) fn bind_recovery_launch(
        &mut self,
        receipt: RecoveryReceipt,
    ) -> Result<(), ApplicationError> {
        match self
            .bootstrap
            .session_mut()
            .start_recovered_candidate(receipt.operation_generation(), receipt.candidate())
            .map_err(|_| ApplicationError::state())?
        {
            RecoveryLaunchDecision::Start { .. }
            | RecoveryLaunchDecision::AlreadyAccepted { .. } => Ok(()),
            RecoveryLaunchDecision::NotTracked | RecoveryLaunchDecision::SafeMode { .. } => {
                Err(ApplicationError::state())
            }
        }
    }

    pub(crate) fn release_startup_guard(&mut self) {
        drop(self.startup_guard.take());
    }
}

impl fmt::Debug for ApplicationPreflight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationPreflight")
            .field("report", &self.bootstrap.report())
            .field("startup_guard", &"[redacted]")
            .finish()
    }
}

struct ApplicationBackupOperation {
    data_root: ValidatedLocalDirectory,
    staging: BackupStaging,
    backups: BackupDirectory,
    settings: SettingsStore,
    retention: RetentionPolicy,
    catalog: Arc<Mutex<Option<Arc<BackupCatalog>>>>,
    restore_pin: Arc<Mutex<Option<CatalogSelectionBinding>>>,
    protected: Option<CatalogSelectionBinding>,
}

impl ApplicationBackupOperation {
    fn open(
        root: &DataRoot,
        retention: RetentionPolicy,
        catalog: Arc<Mutex<Option<Arc<BackupCatalog>>>>,
        restore_pin: Arc<Mutex<Option<CatalogSelectionBinding>>>,
        protected: Option<CatalogSelectionBinding>,
    ) -> Result<Self, ApplicationError> {
        let scope = ArchiveRecoveryScope::new(root.validated_directory(), root.reliable_state())
            .map_err(|_| ApplicationError::state())?;
        let staging = BackupStaging::new(
            &scope
                .staging_root()
                .map_err(|_| ApplicationError::state())?,
        )
        .map_err(|_| ApplicationError::state())?;
        let backups = BackupDirectory::open_or_create(root.reliable_state())
            .map_err(|_| ApplicationError::state())?;
        let settings =
            SettingsStore::new(root.reliable_state()).map_err(|_| ApplicationError::state())?;
        Ok(Self {
            data_root: root.validated_directory().clone(),
            staging,
            backups,
            settings,
            retention,
            catalog,
            restore_pin,
            protected,
        })
    }

    fn execute(&mut self, permit: &MaintenancePermit) -> MaintenanceExecution {
        match self.try_execute(permit) {
            Ok(bytes) => MaintenanceExecution::Published { bytes },
            Err(_) if permit.is_cancelled() => MaintenanceExecution::Cancelled,
            Err(code) => MaintenanceExecution::Failed(code),
        }
    }

    fn try_execute(&mut self, permit: &MaintenancePermit) -> Result<u64, StateErrorCode> {
        let previous = self
            .catalog
            .lock()
            .map_err(|_| StateErrorCode::InternalInvariant)?
            .clone();
        let mut catalog = BackupCatalog::rebuild(&self.backups, previous.as_deref())
            .map_err(|error| error.code())?;
        catalog
            .verify_all_packages(&self.backups)
            .map_err(|error| error.code())?;
        let catalog = Arc::new(catalog);
        *self
            .catalog
            .lock()
            .map_err(|_| StateErrorCode::InternalInvariant)? = Some(Arc::clone(&catalog));
        let control = permit.backup_control().map_err(|error| error.code())?;
        let source = BackupSource::new(&self.data_root).map_err(map_store_error)?;
        let candidate = create_online_snapshot(&source, &self.staging, &control)
            .and_then(|candidate| verify_backup_candidate(candidate, &control))
            .map_err(map_store_error)?;
        let settings = self
            .settings
            .full_backup_candidate()
            .map_err(|error| error.code())?;
        let mut stage = self
            .backups
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .map_err(map_directory_error)?;
        let reader = candidate.open_reader(&control).map_err(map_store_error)?;
        let metadata =
            BackupMetadata::new(current_utc_millis()?, permit.purpose().backup_purpose())
                .map_err(|error| error.code())?;
        let receipt = BackupPackage::write_verified_candidate_to_backup_stage(
            &settings,
            reader,
            BackupCompression::Automatic,
            metadata,
            &mut stage,
        )
        .map_err(|error| error.code())?;
        let verified = BackupPackage::verify_backup_stage(&stage).map_err(|error| error.code())?;
        let admission = match self.protected {
            Some(protected) => RetentionAdmission::preflight_protected(
                catalog.as_ref(),
                &verified,
                self.retention,
                protected,
            ),
            None => RetentionAdmission::preflight(catalog.as_ref(), &verified, self.retention),
        }
        .map_err(|error| error.code())?;

        permit.begin_publication().map_err(|error| error.code())?;
        self.backups
            .publish(&mut stage)
            .map_err(map_directory_error)?;
        let mut published = BackupCatalog::rebuild(&self.backups, Some(catalog.as_ref()))
            .map_err(|error| error.code())?;
        let selection = published
            .bind_published(&verified)
            .map_err(|error| error.code())?;
        let retention = admission
            .confirm_published(&published, selection)
            .map_err(|error| error.code())?;
        loop {
            let restore_pin = self
                .restore_pin
                .lock()
                .map_err(|_| StateErrorCode::InternalInvariant)?;
            let deleted = match *restore_pin {
                Some(protected) => {
                    retention.delete_next_protected(&published, &self.backups, protected)
                }
                None => retention.delete_next(&published, &self.backups),
            }
            .map_err(|error| error.code())?;
            drop(restore_pin);
            if !deleted {
                break;
            }
            published = BackupCatalog::rebuild(&self.backups, Some(&published))
                .map_err(|error| error.code())?;
        }
        *self
            .catalog
            .lock()
            .map_err(|_| StateErrorCode::InternalInvariant)? = Some(Arc::new(published));
        Ok(receipt.package_len())
    }
}

fn current_utc_millis() -> Result<i64, StateErrorCode> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StateErrorCode::Unavailable)?
        .as_millis();
    i64::try_from(millis).map_err(|_| StateErrorCode::CapacityExceeded)
}

const fn map_store_error(error: tokenmaster_store::StoreError) -> StateErrorCode {
    match error.code() {
        StoreErrorCode::CapacityExceeded => StateErrorCode::CapacityExceeded,
        StoreErrorCode::Busy => StateErrorCode::Busy,
        StoreErrorCode::StaleBackupCandidate
        | StoreErrorCode::BackupHeaderCorrupt
        | StoreErrorCode::BackupPageCorrupt
        | StoreErrorCode::BackupIndexCorrupt
        | StoreErrorCode::BackupForeignKeyCorrupt
        | StoreErrorCode::BackupCountCorrupt
        | StoreErrorCode::BackupGenerationCorrupt
        | StoreErrorCode::BackupSemanticCorrupt => StateErrorCode::Integrity,
        StoreErrorCode::InvalidValue => StateErrorCode::InvalidInput,
        StoreErrorCode::VersionMismatch
        | StoreErrorCode::SchemaTooNew
        | StoreErrorCode::SchemaMismatch
        | StoreErrorCode::PolicyMismatch => StateErrorCode::UnsupportedVersion,
        StoreErrorCode::Cancelled
        | StoreErrorCode::DeadlineExceeded
        | StoreErrorCode::BackupIo
        | StoreErrorCode::Database => StateErrorCode::Unavailable,
        StoreErrorCode::InvalidStoredValue
        | StoreErrorCode::StaleCheckpoint
        | StoreErrorCode::RebuildRequired
        | StoreErrorCode::StaleRevision
        | StoreErrorCode::AccountingVersionMismatch
        | StoreErrorCode::IncompleteManifest
        | StoreErrorCode::UnsealedRevision
        | StoreErrorCode::PendingContinuation
        | StoreErrorCode::ScanInProgress
        | StoreErrorCode::StaleScan
        | StoreErrorCode::PendingScan
        | StoreErrorCode::ArchiveModeMismatch => StateErrorCode::InternalInvariant,
    }
}

const fn map_directory_error(error: BackupDirectoryError) -> StateErrorCode {
    match error {
        BackupDirectoryError::UnexpectedEntry
        | BackupDirectoryError::UnexpectedType
        | BackupDirectoryError::LinkedEntry
        | BackupDirectoryError::AmbiguousIdentity => StateErrorCode::Integrity,
        BackupDirectoryError::CapacityExceeded => StateErrorCode::CapacityExceeded,
        BackupDirectoryError::RecoveryRequired => StateErrorCode::RecoveryRequired,
        BackupDirectoryError::UnsupportedLocation
        | BackupDirectoryError::StaleEntry
        | BackupDirectoryError::InvalidState
        | BackupDirectoryError::Unavailable => StateErrorCode::Unavailable,
    }
}
