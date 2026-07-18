use std::fmt;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, BackupDirectoryError, ExclusiveFileLease,
    ExclusiveFileLeaseGuard, MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCatalog, BackupCompression, BackupMaintenanceRuntime, BackupMetadata, BackupPackage,
    BootstrapReport, MaintenanceExecution, MaintenancePermit, MaintenanceSourceState,
    PendingMigration, PreparedBootstrap, RecoveryJournalStore, RetentionAdmission, RetentionPolicy,
    RunSession, RunStateStore, SettingsStore, StateBootstrap, StateErrorCode,
    SystemMaintenanceClock,
};
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, StartupArchiveStatus, StartupValidationMode,
    StoreErrorCode, USAGE_SCHEMA_VERSION, create_online_snapshot, inspect_startup_archive,
    verify_backup_candidate,
};

use crate::{ApplicationError, DataRoot};

const STARTUP_RECOVERY_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(crate) struct ApplicationStateOwner {
    scope: ArchiveRecoveryScope,
    backups: BackupDirectory,
    verification_staging: BackupStaging,
    settings: SettingsStore,
    journal: RecoveryJournalStore,
    run_state: RunStateStore,
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

    pub(crate) fn start_maintenance(
        &self,
        root: &DataRoot,
        source_state: MaintenanceSourceState,
    ) -> Result<BackupMaintenanceRuntime, ApplicationError> {
        let settings = self
            .settings
            .load()
            .map_err(|_| ApplicationError::state())?;
        let policy = settings.value().portable().backup().clone();
        let retention = RetentionPolicy::new(policy.retention_budget_bytes())
            .map_err(|_| ApplicationError::state())?;
        let mut operation = ApplicationBackupOperation::open(root, retention)?;
        BackupMaintenanceRuntime::spawn(
            Arc::new(SystemMaintenanceClock::new()),
            policy,
            source_state,
            move |permit| operation.execute(permit),
        )
        .map_err(|_| ApplicationError::state())
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
    catalog: Option<BackupCatalog>,
}

impl ApplicationBackupOperation {
    fn open(root: &DataRoot, retention: RetentionPolicy) -> Result<Self, ApplicationError> {
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
            catalog: None,
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
        let mut catalog = BackupCatalog::rebuild(&self.backups, self.catalog.as_ref())
            .map_err(|error| error.code())?;
        catalog
            .verify_all_packages(&self.backups)
            .map_err(|error| error.code())?;
        self.catalog = Some(catalog);
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
        let current_catalog = self
            .catalog
            .as_ref()
            .ok_or(StateErrorCode::InternalInvariant)?;
        let admission = RetentionAdmission::preflight(current_catalog, &verified, self.retention)
            .map_err(|error| error.code())?;

        permit.begin_publication().map_err(|error| error.code())?;
        self.backups
            .publish(&mut stage)
            .map_err(map_directory_error)?;
        let mut published = BackupCatalog::rebuild(&self.backups, Some(current_catalog))
            .map_err(|error| error.code())?;
        let selection = published
            .bind_published(&verified)
            .map_err(|error| error.code())?;
        let retention = admission
            .confirm_published(&published, selection)
            .map_err(|error| error.code())?;
        while retention
            .delete_next(&published, &self.backups)
            .map_err(|error| error.code())?
        {
            published = BackupCatalog::rebuild(&self.backups, Some(&published))
                .map_err(|error| error.code())?;
        }
        self.catalog = Some(published);
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
