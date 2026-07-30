use core::fmt;

use tokenmaster_platform::{
    ArchiveRecoveryError, ArchiveRecoveryScope, BackupDirectory, BackupDirectoryError,
    ExclusiveFileLeaseGuard, ValidatedLocalDirectory,
};
use tokenmaster_store::{
    BackupControl, BackupStaging, StartupArchiveStatus, StartupValidationMode, StoreErrorCode,
    inspect_startup_archive,
};

use crate::{
    BackupCatalog, BackupPackage, CatalogHealth, CatalogSelection, PriorRunCondition,
    RecoveryCoordinator, RecoveryJournalStore, RecoveryLaunchDecision, RecoveryReceipt,
    RestoreMode, RunSession, RunStateInspection, RunStateStore, SettingsStore, StateError,
    StateErrorCode,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapOutcome {
    Healthy,
    FirstInstall,
    MigrationRequired,
    UpgradeRequired,
    RecoveryRequired,
    Unavailable,
    SafeMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapReport {
    outcome: BootstrapOutcome,
    prior_run: RunStateInspection,
    quick_check_performed: bool,
    recovery_resumed: bool,
    recovery_launch: RecoveryLaunchDecision,
}

impl BootstrapReport {
    #[must_use]
    pub const fn outcome(self) -> BootstrapOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn prior_run(self) -> RunStateInspection {
        self.prior_run
    }

    #[must_use]
    pub const fn quick_check_performed(self) -> bool {
        self.quick_check_performed
    }

    #[must_use]
    pub const fn recovery_resumed(self) -> bool {
        self.recovery_resumed
    }

    #[must_use]
    pub const fn recovery_launch(self) -> RecoveryLaunchDecision {
        self.recovery_launch
    }
}

pub struct PreparedBootstrap {
    report: BootstrapReport,
    session: RunSession,
}

impl PreparedBootstrap {
    #[must_use]
    pub const fn report(&self) -> BootstrapReport {
        self.report
    }

    pub fn session_mut(&mut self) -> &mut RunSession {
        &mut self.session
    }
}

impl fmt::Debug for PreparedBootstrap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedBootstrap")
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

pub struct StateBootstrap<'a> {
    data_root: &'a ValidatedLocalDirectory,
    scope: &'a ArchiveRecoveryScope,
    verification_staging: &'a BackupStaging,
    journal: &'a RecoveryJournalStore,
    settings: &'a SettingsStore,
    run_state: &'a RunStateStore,
    backups: &'a BackupDirectory,
}

impl<'a> StateBootstrap<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data_root: &'a ValidatedLocalDirectory,
        scope: &'a ArchiveRecoveryScope,
        verification_staging: &'a BackupStaging,
        journal: &'a RecoveryJournalStore,
        settings: &'a SettingsStore,
        run_state: &'a RunStateStore,
        backups: &'a BackupDirectory,
    ) -> Result<Self, StateError> {
        let value = Self {
            data_root,
            scope,
            verification_staging,
            journal,
            settings,
            run_state,
            backups,
        };
        value.validate_bindings()?;
        Ok(value)
    }

    pub fn prepare(
        &self,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
    ) -> Result<PreparedBootstrap, StateError> {
        let recovery = self.validate_bindings()?;
        self.scope
            .authorize_data_root(self.data_root, guard)
            .map_err(map_platform_error)?;
        let archive_before = self.scope.observe(guard).map_err(map_platform_error)?;
        let recovery_evidence = self
            .scope
            .has_recovery_evidence(guard)
            .map_err(map_platform_error)?;
        let prior_run = self.run_state.inspect()?;
        let settings_evidence = self.settings.has_any_artifact()?;
        let backup_evidence = !self
            .backups
            .scan()
            .map_err(map_backup_directory_error)?
            .entries()
            .is_empty();
        let prior_durable_evidence = archive_before.has_any_archive_artifact()
            || recovery_evidence
            || settings_evidence
            || backup_evidence
            || prior_run.condition() != PriorRunCondition::Missing;

        // This durable write and reread is deliberately before catalog/package/SQLite work.
        let mut session = self.run_state.begin()?;
        let mut catalog = match BackupCatalog::rebuild(self.backups, None) {
            Ok(catalog) => catalog,
            Err(error) => {
                return Ok(prepared_from_state_error(session, error));
            }
        };
        let mut receipt = match recovery.resume(self.backups, &catalog, guard, control) {
            Ok(receipt) => receipt,
            Err(error) => {
                return Ok(prepared_from_state_error(session, error));
            }
        };
        if receipt.is_some_and(|receipt| {
            prior_run.last_recovery_generation() == Some(receipt.operation_generation())
        }) {
            receipt = None;
        }
        let mode = if session.prior().requires_quick_check() {
            StartupValidationMode::Quick
        } else {
            StartupValidationMode::Normal
        };
        let mut inspection = match inspect_startup_archive(self.data_root, mode) {
            Ok(inspection) => inspection,
            Err(error) if is_definitive_corruption(error.code()) && receipt.is_none() => {
                receipt = match self.automatic_recover(&recovery, &mut catalog, guard, control) {
                    Ok(receipt) => receipt,
                    Err(error) => return Ok(prepared_from_state_error(session, error)),
                };
                if receipt.is_none() {
                    return Ok(PreparedBootstrap {
                        report: BootstrapReport {
                            outcome: BootstrapOutcome::RecoveryRequired,
                            prior_run: session.prior(),
                            quick_check_performed: false,
                            recovery_resumed: false,
                            recovery_launch: RecoveryLaunchDecision::NotTracked,
                        },
                        session,
                    });
                }
                match inspect_startup_archive(self.data_root, mode) {
                    Ok(inspection) => inspection,
                    Err(error) => {
                        return Ok(PreparedBootstrap {
                            report: BootstrapReport {
                                outcome: map_store_outcome(error.code()),
                                prior_run: session.prior(),
                                quick_check_performed: false,
                                recovery_resumed: true,
                                recovery_launch: RecoveryLaunchDecision::NotTracked,
                            },
                            session,
                        });
                    }
                }
            }
            Err(error) => {
                return Ok(PreparedBootstrap {
                    report: BootstrapReport {
                        outcome: map_store_outcome(error.code()),
                        prior_run: session.prior(),
                        quick_check_performed: false,
                        recovery_resumed: receipt.is_some(),
                        recovery_launch: RecoveryLaunchDecision::NotTracked,
                    },
                    session,
                });
            }
        };
        if inspection.status() == StartupArchiveStatus::Missing
            && prior_durable_evidence
            && receipt.is_none()
        {
            receipt = match self.automatic_recover(&recovery, &mut catalog, guard, control) {
                Ok(receipt) => receipt,
                Err(error) => return Ok(prepared_from_state_error(session, error)),
            };
            if receipt.is_some() {
                inspection = match inspect_startup_archive(self.data_root, mode) {
                    Ok(inspection) => inspection,
                    Err(error) => {
                        return Ok(PreparedBootstrap {
                            report: BootstrapReport {
                                outcome: map_store_outcome(error.code()),
                                prior_run: session.prior(),
                                quick_check_performed: false,
                                recovery_resumed: true,
                                recovery_launch: RecoveryLaunchDecision::NotTracked,
                            },
                            session,
                        });
                    }
                };
            }
        }
        let outcome = match inspection.status() {
            StartupArchiveStatus::Missing if prior_durable_evidence || receipt.is_some() => {
                BootstrapOutcome::RecoveryRequired
            }
            StartupArchiveStatus::Missing => BootstrapOutcome::FirstInstall,
            StartupArchiveStatus::SupportedLegacy => BootstrapOutcome::MigrationRequired,
            StartupArchiveStatus::Current => BootstrapOutcome::Healthy,
        };
        let recovery_launch = if outcome == BootstrapOutcome::Healthy {
            match receipt {
                Some(receipt) => session.start_recovered_candidate(
                    receipt.operation_generation(),
                    receipt.candidate(),
                )?,
                None if session.prior().recovery_launches().is_some() => {
                    RecoveryLaunchDecision::SafeMode {
                        failed_launches: session.prior().recovery_launches().unwrap_or(2),
                    }
                }
                None => {
                    session.authorize_healthy_launch();
                    RecoveryLaunchDecision::NotTracked
                }
            }
        } else {
            RecoveryLaunchDecision::NotTracked
        };
        let outcome = if matches!(recovery_launch, RecoveryLaunchDecision::SafeMode { .. }) {
            BootstrapOutcome::SafeMode
        } else {
            outcome
        };
        Ok(PreparedBootstrap {
            report: BootstrapReport {
                outcome,
                prior_run: session.prior(),
                quick_check_performed: inspection.quick_check_performed(),
                recovery_resumed: receipt.is_some(),
                recovery_launch,
            },
            session,
        })
    }

    fn automatic_recover(
        &self,
        recovery: &RecoveryCoordinator<'_>,
        catalog: &mut BackupCatalog,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
    ) -> Result<Option<RecoveryReceipt>, StateError> {
        let selections: Vec<CatalogSelection> = catalog
            .points()
            .iter()
            .filter(|point| point.health() != CatalogHealth::Corrupt)
            .map(|point| point.selection())
            .collect();
        for selection in selections {
            let mut reader = match catalog.open_recovery_selection(self.backups, selection) {
                Ok(reader) => reader,
                Err(error) if is_skippable_package_error(error.code()) => continue,
                Err(error) => return Err(error),
            };
            let package = match BackupPackage::inspect(&mut reader) {
                Ok(package) => package,
                Err(error) if is_skippable_package_error(error.code()) => continue,
                Err(error) => return Err(error),
            };
            if let Err(error) = catalog.bind_verified(selection, &package) {
                if is_skippable_package_error(error.code()) {
                    continue;
                }
                return Err(error);
            }
            match recovery.restore_definitively_corrupt_selected(
                self.backups,
                catalog,
                selection,
                RestoreMode::AutomaticDataOnly,
                guard,
                control,
            ) {
                Ok(receipt) => return Ok(Some(receipt)),
                Err(error)
                    if matches!(
                        error.code(),
                        StateErrorCode::Integrity | StateErrorCode::UnsupportedVersion
                    ) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }

    fn validate_bindings(&self) -> Result<RecoveryCoordinator<'a>, StateError> {
        let reliable_state = self
            .scope
            .reliable_state_root()
            .map_err(map_platform_error)?;
        self.run_state.authorize_directory(&reliable_state)?;
        self.backups
            .authorize_parent(&reliable_state)
            .map_err(map_backup_directory_error)?;
        RecoveryCoordinator::new(
            self.scope,
            self.verification_staging,
            self.journal,
            self.settings,
        )
    }
}

impl fmt::Debug for StateBootstrap<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StateBootstrap([redacted])")
    }
}

fn prepared_from_state_error(session: RunSession, error: StateError) -> PreparedBootstrap {
    let prior_run = session.prior();
    PreparedBootstrap {
        report: BootstrapReport {
            outcome: match error.code() {
                StateErrorCode::Unavailable
                | StateErrorCode::Busy
                | StateErrorCode::DiskCapacity => BootstrapOutcome::Unavailable,
                StateErrorCode::RecoveryRequired
                | StateErrorCode::Integrity
                | StateErrorCode::CapacityExceeded
                | StateErrorCode::InternalInvariant
                | StateErrorCode::InvalidInput
                | StateErrorCode::UnsupportedVersion => BootstrapOutcome::SafeMode,
            },
            prior_run,
            quick_check_performed: false,
            recovery_resumed: false,
            recovery_launch: RecoveryLaunchDecision::NotTracked,
        },
        session,
    }
}

const fn map_store_outcome(code: StoreErrorCode) -> BootstrapOutcome {
    match code {
        StoreErrorCode::SchemaTooNew => BootstrapOutcome::UpgradeRequired,
        StoreErrorCode::SchemaMismatch
        | StoreErrorCode::BackupHeaderCorrupt
        | StoreErrorCode::BackupPageCorrupt
        | StoreErrorCode::BackupIndexCorrupt
        | StoreErrorCode::BackupForeignKeyCorrupt
        | StoreErrorCode::BackupCountCorrupt
        | StoreErrorCode::BackupGenerationCorrupt
        | StoreErrorCode::BackupSemanticCorrupt => BootstrapOutcome::RecoveryRequired,
        StoreErrorCode::Busy
        | StoreErrorCode::BackupIo
        | StoreErrorCode::Database
        | StoreErrorCode::DeadlineExceeded
        | StoreErrorCode::Cancelled => BootstrapOutcome::Unavailable,
        StoreErrorCode::VersionMismatch
        | StoreErrorCode::PolicyMismatch
        | StoreErrorCode::InvalidValue
        | StoreErrorCode::CapacityExceeded
        | StoreErrorCode::InvalidStoredValue
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
        | StoreErrorCode::ArchiveModeMismatch
        | StoreErrorCode::StaleBackupCandidate => BootstrapOutcome::SafeMode,
    }
}

const fn is_definitive_corruption(code: StoreErrorCode) -> bool {
    matches!(
        code,
        StoreErrorCode::BackupHeaderCorrupt
            | StoreErrorCode::BackupPageCorrupt
            | StoreErrorCode::BackupIndexCorrupt
            | StoreErrorCode::BackupForeignKeyCorrupt
            | StoreErrorCode::BackupCountCorrupt
            | StoreErrorCode::BackupGenerationCorrupt
            | StoreErrorCode::BackupSemanticCorrupt
    )
}

const fn is_skippable_package_error(code: StateErrorCode) -> bool {
    matches!(
        code,
        StateErrorCode::InvalidInput
            | StateErrorCode::UnsupportedVersion
            | StateErrorCode::CapacityExceeded
            | StateErrorCode::Integrity
    )
}

const fn map_platform_error(error: ArchiveRecoveryError) -> StateError {
    match error {
        ArchiveRecoveryError::CapacityExceeded | ArchiveRecoveryError::CollisionLimit => {
            StateError::capacity_exceeded()
        }
        ArchiveRecoveryError::DiskCapacity => StateError::from_code(StateErrorCode::DiskCapacity),
        ArchiveRecoveryError::ArtifactMismatch | ArchiveRecoveryError::UnexpectedArtifact => {
            StateError::integrity()
        }
        ArchiveRecoveryError::RecoveryRequired => StateError::recovery_required(),
        ArchiveRecoveryError::InvalidState => StateError::internal_invariant(),
        ArchiveRecoveryError::UnsupportedLocation
        | ArchiveRecoveryError::WrongLease
        | ArchiveRecoveryError::Unavailable => StateError::unavailable(),
    }
}

const fn map_backup_directory_error(error: BackupDirectoryError) -> StateError {
    match error {
        BackupDirectoryError::CapacityExceeded => StateError::capacity_exceeded(),
        BackupDirectoryError::UnexpectedEntry
        | BackupDirectoryError::UnexpectedType
        | BackupDirectoryError::LinkedEntry
        | BackupDirectoryError::AmbiguousIdentity
        | BackupDirectoryError::StaleEntry => StateError::integrity(),
        BackupDirectoryError::RecoveryRequired => StateError::recovery_required(),
        BackupDirectoryError::InvalidState => StateError::internal_invariant(),
        BackupDirectoryError::UnsupportedLocation | BackupDirectoryError::Unavailable => {
            StateError::unavailable()
        }
    }
}
