use core::fmt;

use tokenmaster_platform::{
    ArchiveRecoveryError, ArchiveRecoveryScope, ArchiveSetObservation, BackupDirectory,
    BackupDirectoryError, DurableFileReader, ExclusiveFileLeaseGuard, RecoveryMainMode,
    RecoveryOperation, RecoveryStagedArchive, ValidatedLocalDirectory,
};
use tokenmaster_store::{
    BackupControl, BackupStaging, RecoveryVerificationBoundary, StoreErrorCode,
    VerifiedRecoveryArchive, create_fresh_recovery_archive, verify_recovery_archive_with_observer,
};

use super::{
    RecoveryArchiveFacts, RecoveryBackupIdentity, RecoveryCandidateIdentity, RecoveryJournal,
    RecoveryJournalLoad, RecoveryJournalStore, RecoveryPhase, RecoverySettingsMode,
};
use crate::settings::SettingsRestoreBoundary;
use crate::{
    BackupCatalog, BackupPackage, CatalogHealth, CatalogSelection, MaintenanceCompletion,
    MaintenanceOutcome, MaintenancePurpose, PortableSettingsCandidate, PreparedSettingsRestore,
    SettingsStore, StateError, StateErrorCode, VerifiedBackupPackage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreMode {
    DataOnly,
    DataAndPortableSettings,
    AutomaticDataOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Opaque proof that a healthy archive received its mandatory pre-restore backup.
///
/// Corruption is intentionally not a caller-constructible variant:
///
/// ```compile_fail
/// let _ = tokenmaster_state::RestoreSafety::DefinitiveCorruption;
/// ```
pub enum RestoreSafety {
    PreRestoreBackupPublished(MaintenanceCompletion),
}

#[derive(Clone, Copy)]
enum RestoreAuthority {
    Safety(RestoreSafety),
    DefinitiveCorruption,
}

/// Bounded crash-test observation with no filesystem, store, or mutation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum RecoveryBoundary {
    /// All cancellable preparation is complete; the next action publishes the journal.
    BeforeJournalPublication,
    /// The normal store constructor and complete verifier produced a fresh archive.
    ReconstructionCandidateCreated,
    /// The fresh verified archive reached the sealed platform recovery stage.
    ReconstructionCandidateStaged,
    /// The named recovery phase has been durably published and reread.
    JournalDurable(RecoveryPhase),
    /// The exact sidecars reached quarantine before their phase advanced.
    SidecarsQuarantinedBeforeJournal,
    /// The verified candidate reached the active main before its phase advanced.
    MainPromotedBeforeJournal,
    /// Portable settings reached their journaled target before the phase advanced.
    SettingsCommittedBeforeJournal,
    /// A settings slot was published before the typed record reread completed.
    SettingsRecordPublishedBeforeReread,
    /// The store-owned candidate verifier file exists before a journal exists.
    CandidateVerifierFileCreated,
    /// The store-owned active verifier file exists during journal replay.
    ActiveVerifierFileCreated,
    /// The store-owned corruption verifier file exists before journal publication.
    CorruptionVerifierFileCreated,
    /// The first prepared journal slot exists before redundant publication.
    FirstPreparedJournalSlotPublished,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RecoveryReceipt {
    operation_generation: u64,
    candidate: RecoveryCandidateIdentity,
    settings_mode: RecoverySettingsMode,
    reconstructed_from_authoritative_source: bool,
}

impl RecoveryReceipt {
    #[must_use]
    pub const fn operation_generation(self) -> u64 {
        self.operation_generation
    }

    #[must_use]
    pub const fn candidate(self) -> RecoveryCandidateIdentity {
        self.candidate
    }

    #[must_use]
    pub const fn settings_mode(self) -> RecoverySettingsMode {
        self.settings_mode
    }

    #[must_use]
    pub const fn reconstructed_from_authoritative_source(self) -> bool {
        self.reconstructed_from_authoritative_source
    }

    #[must_use]
    pub const fn non_reconstructible_domains_lost(self) -> bool {
        self.reconstructed_from_authoritative_source
    }
}

impl fmt::Debug for RecoveryReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryReceipt([redacted])")
    }
}

/// Single-threaded restore orchestration over sealed platform/store capabilities.
pub struct RecoveryCoordinator<'a> {
    scope: &'a ArchiveRecoveryScope,
    verification_staging: &'a BackupStaging,
    journal: &'a RecoveryJournalStore,
    settings: &'a SettingsStore,
    reliable_state: ValidatedLocalDirectory,
}

impl<'a> RecoveryCoordinator<'a> {
    pub fn new(
        scope: &'a ArchiveRecoveryScope,
        verification_staging: &'a BackupStaging,
        journal: &'a RecoveryJournalStore,
        settings: &'a SettingsStore,
    ) -> Result<Self, StateError> {
        let reliable_state = scope.reliable_state_root().map_err(map_platform_error)?;
        let staging = scope.staging_root().map_err(map_platform_error)?;
        verification_staging
            .authorize_directory(&staging)
            .map_err(|error| map_store_error(error.code()))?;
        journal.authorize_directory(&reliable_state)?;
        settings.authorize_directory(&reliable_state)?;
        Ok(Self {
            scope,
            verification_staging,
            journal,
            settings,
            reliable_state,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore_selected(
        &self,
        directory: &BackupDirectory,
        catalog: &BackupCatalog,
        selection: CatalogSelection,
        mode: RestoreMode,
        safety: RestoreSafety,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
    ) -> Result<RecoveryReceipt, StateError> {
        self.restore_selected_with_observer(
            directory,
            catalog,
            selection,
            mode,
            safety,
            guard,
            control,
            |_| Ok(()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore_definitively_corrupt_selected(
        &self,
        directory: &BackupDirectory,
        catalog: &BackupCatalog,
        selection: CatalogSelection,
        mode: RestoreMode,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
    ) -> Result<RecoveryReceipt, StateError> {
        self.restore_definitively_corrupt_selected_with_observer(
            directory,
            catalog,
            selection,
            mode,
            guard,
            control,
            |_| Ok(()),
        )
    }

    pub fn reconstruct_definitively_corrupt(
        &self,
        directory: &BackupDirectory,
        catalog: &mut BackupCatalog,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
    ) -> Result<RecoveryReceipt, StateError> {
        self.reconstruct_definitively_corrupt_with_observer(
            directory,
            catalog,
            guard,
            control,
            |_| Ok(()),
        )
    }

    #[doc(hidden)]
    pub fn reconstruct_definitively_corrupt_with_observer<F>(
        &self,
        directory: &BackupDirectory,
        catalog: &mut BackupCatalog,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
        mut observer: F,
    ) -> Result<RecoveryReceipt, StateError>
    where
        F: FnMut(RecoveryBoundary) -> Result<(), StateError>,
    {
        self.scope
            .authorize_guard(guard)
            .map_err(map_platform_error)?;
        self.authorize_backup_directory(directory)?;
        catalog.verify_all_packages(directory)?;
        if catalog
            .points()
            .iter()
            .any(|point| point.health() == CatalogHealth::Verified)
        {
            return Err(StateError::invalid_input());
        }
        let has_backup_evidence = !catalog.points().is_empty();
        let operation_generation = self.journal.next_operation_generation()?;
        self.recover_verifier_staging()?;
        self.scope
            .discard_abandoned_staging(guard)
            .map_err(map_platform_error)?;
        let preflight_observation = self.scope.observe(guard).map_err(map_platform_error)?;
        if !preflight_observation.has_any_archive_artifact() && !has_backup_evidence {
            return Err(StateError::invalid_input());
        }

        let operation = self
            .scope
            .reserve_operation(guard)
            .map_err(map_platform_error)?;
        let mut candidate_stage = self
            .scope
            .create_candidate(&operation, crate::MAX_DATABASE_PACKAGE_BYTES)
            .map_err(map_platform_error)?;
        let fresh = create_fresh_recovery_archive(self.verification_staging, control)
            .map_err(|error| map_store_error(error.code()))?;
        observer(RecoveryBoundary::ReconstructionCandidateCreated)?;
        self.scope
            .require_available_staging_bytes(
                guard,
                required_recovery_capacity(
                    fresh.len(),
                    preflight_observation.main().map_or(0, |main| main.len()),
                )?,
            )
            .map_err(map_platform_error)?;
        let candidate = recovery_candidate_identity(&fresh)?;
        fresh
            .stage_for_recovery(&mut candidate_stage, control)
            .map_err(|error| map_store_error(error.code()))?;
        observer(RecoveryBoundary::ReconstructionCandidateStaged)?;
        drop(fresh);
        self.verify_reconstruction_candidate(&candidate_stage, candidate, control, &mut observer)?;

        let before_observation = self.scope.observe(guard).map_err(map_platform_error)?;
        if before_observation != preflight_observation {
            return Err(StateError::integrity());
        }
        self.require_definitive_corruption(before_observation, guard, control, &mut observer)?;
        if self.scope.observe(guard).map_err(map_platform_error)? != before_observation {
            return Err(StateError::integrity());
        }
        let prepared = RecoveryJournal::reconstruction(
            operation_generation,
            operation.id(),
            candidate,
            RecoveryArchiveFacts::from_observation(before_observation)?,
        )?;
        observer(RecoveryBoundary::BeforeJournalPublication)?;
        self.journal.begin_with_observer(&prepared, || {
            observer(RecoveryBoundary::FirstPreparedJournalSlotPublished)
        })?;
        observer(RecoveryBoundary::JournalDurable(RecoveryPhase::Prepared))?;
        self.continue_restore(
            directory,
            catalog,
            prepared,
            operation,
            Some(candidate_stage),
            None,
            guard,
            control,
            &mut observer,
        )
    }

    /// Reports only durable phases and the settings-commit crash boundary.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn restore_selected_with_observer<F>(
        &self,
        directory: &BackupDirectory,
        catalog: &BackupCatalog,
        selection: CatalogSelection,
        mode: RestoreMode,
        safety: RestoreSafety,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
        observer: F,
    ) -> Result<RecoveryReceipt, StateError>
    where
        F: FnMut(RecoveryBoundary) -> Result<(), StateError>,
    {
        self.restore_selected_authorized(
            directory,
            catalog,
            selection,
            mode,
            RestoreAuthority::Safety(safety),
            guard,
            control,
            observer,
        )
    }

    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn restore_definitively_corrupt_selected_with_observer<F>(
        &self,
        directory: &BackupDirectory,
        catalog: &BackupCatalog,
        selection: CatalogSelection,
        mode: RestoreMode,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
        observer: F,
    ) -> Result<RecoveryReceipt, StateError>
    where
        F: FnMut(RecoveryBoundary) -> Result<(), StateError>,
    {
        self.restore_selected_authorized(
            directory,
            catalog,
            selection,
            mode,
            RestoreAuthority::DefinitiveCorruption,
            guard,
            control,
            observer,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn restore_selected_authorized<F>(
        &self,
        directory: &BackupDirectory,
        catalog: &BackupCatalog,
        selection: CatalogSelection,
        mode: RestoreMode,
        authority: RestoreAuthority,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
        mut observer: F,
    ) -> Result<RecoveryReceipt, StateError>
    where
        F: FnMut(RecoveryBoundary) -> Result<(), StateError>,
    {
        self.scope
            .authorize_guard(guard)
            .map_err(map_platform_error)?;
        self.authorize_backup_directory(directory)?;
        let operation_generation = self.journal.next_operation_generation()?;
        self.recover_verifier_staging()?;
        self.scope
            .discard_abandoned_staging(guard)
            .map_err(map_platform_error)?;
        let preflight_observation = self.scope.observe(guard).map_err(map_platform_error)?;
        validate_mode_authority(mode, authority)?;
        let (backup_slot, package_len, package_sha256) =
            catalog.selected_package_identity(selection)?;
        let backup =
            RecoveryBackupIdentity::from_persisted(backup_slot, package_len, package_sha256)?;
        let mut package_probe = catalog.open_verified_selection(directory, selection)?;
        let inspected = BackupPackage::inspect(&mut package_probe)?;
        require_package_identity(&inspected, backup)?;
        self.scope
            .require_available_staging_bytes(
                guard,
                required_recovery_capacity(
                    inspected.database_len(),
                    preflight_observation.main().map_or(0, |main| main.len()),
                )?,
            )
            .map_err(map_platform_error)?;
        let mut package_reader = catalog.open_verified_selection(directory, selection)?;
        let operation = self
            .scope
            .reserve_operation(guard)
            .map_err(map_platform_error)?;
        let mut candidate_stage = self
            .scope
            .create_candidate(&operation, crate::MAX_DATABASE_PACKAGE_BYTES)
            .map_err(map_platform_error)?;
        let package = BackupPackage::read_for_recovery(&mut package_reader, &mut candidate_stage)?;
        require_package_identity(&package, backup)?;
        let verified = self.verify_candidate(&candidate_stage, &package, control, &mut observer)?;
        let candidate = recovery_candidate_identity(&verified)?;
        require_package_database(&package, candidate)?;
        drop(verified);
        // Recheck the selected catalog generation and complete package identity after
        // the recovery-only SQLite verification pass.
        let mut recheck = catalog.open_verified_selection(directory, selection)?;
        let reverified = BackupPackage::inspect(&mut recheck)?;
        require_package_identity(&reverified, backup)?;
        require_package_database(&reverified, candidate)?;

        let prepared_settings = self.prepare_settings(mode, package.settings())?;
        let before_observation = self.scope.observe(guard).map_err(map_platform_error)?;
        if before_observation != preflight_observation {
            return Err(StateError::integrity());
        }
        match authority {
            RestoreAuthority::Safety(safety) => {
                validate_safety_for_archive(safety, before_observation.has_any_archive_artifact())?;
            }
            RestoreAuthority::DefinitiveCorruption => {
                self.require_definitive_corruption(
                    before_observation,
                    guard,
                    control,
                    &mut observer,
                )?;
            }
        }
        if self.scope.observe(guard).map_err(map_platform_error)? != before_observation {
            return Err(StateError::integrity());
        }
        let before = RecoveryArchiveFacts::from_observation(before_observation)?;
        let prepared = match mode {
            RestoreMode::AutomaticDataOnly => RecoveryJournal::automatic(
                operation_generation,
                operation.id(),
                backup,
                candidate,
                before,
                1,
            )?,
            RestoreMode::DataOnly | RestoreMode::DataAndPortableSettings => {
                RecoveryJournal::manual(
                    operation_generation,
                    operation.id(),
                    backup,
                    candidate,
                    before,
                    prepared_settings
                        .as_ref()
                        .map(PreparedSettingsRestore::target),
                    1,
                )?
            }
        };
        self.journal.begin_with_observer(&prepared, || {
            observer(RecoveryBoundary::FirstPreparedJournalSlotPublished)
        })?;
        observer(RecoveryBoundary::JournalDurable(RecoveryPhase::Prepared))?;
        self.continue_restore(
            directory,
            catalog,
            prepared,
            operation,
            Some(candidate_stage),
            prepared_settings,
            guard,
            control,
            &mut observer,
        )
    }

    pub fn resume(
        &self,
        directory: &BackupDirectory,
        catalog: &BackupCatalog,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
    ) -> Result<Option<RecoveryReceipt>, StateError> {
        self.scope
            .authorize_guard(guard)
            .map_err(map_platform_error)?;
        self.authorize_backup_directory(directory)?;
        self.recover_verifier_staging()?;
        let journal = match self.journal.load()? {
            RecoveryJournalLoad::Absent => {
                self.scope
                    .discard_abandoned_staging(guard)
                    .map_err(map_platform_error)?;
                return Ok(None);
            }
            RecoveryJournalLoad::Invalid => return Err(StateError::recovery_required()),
            RecoveryJournalLoad::Pending(journal) => journal,
        };
        if journal.phase() == RecoveryPhase::Complete {
            self.scope
                .discard_abandoned_staging(guard)
                .map_err(map_platform_error)?;
            return Ok(Some(receipt_from_journal(&journal)));
        }
        if journal.phase() == RecoveryPhase::Prepared {
            self.journal.begin(&journal)?;
        }
        let package = match journal.backup() {
            Some(backup) => {
                let selection = catalog.selection_for_package_identity(
                    backup.backup_slot(),
                    backup.package_len(),
                    *backup.package_sha256(),
                )?;
                let mut package_reader = catalog.open_recovery_selection(directory, selection)?;
                let package = BackupPackage::inspect(&mut package_reader)?;
                require_package_identity(&package, backup)?;
                require_package_database(&package, journal.candidate())?;
                Some(package)
            }
            None if journal.settings_mode() == RecoverySettingsMode::ReconstructionDataOnly => None,
            None => return Err(StateError::integrity()),
        };
        let prepared_settings = match (journal.settings_mode(), package.as_ref()) {
            (RecoverySettingsMode::DataAndPortableSettings, Some(package)) => {
                let prepared = self.settings.prepare_restore(package.settings())?;
                let expected = journal
                    .settings_target()
                    .ok_or_else(StateError::internal_invariant)?
                    .to_portable()?;
                if prepared.target() != expected {
                    return Err(StateError::integrity());
                }
                Some(prepared)
            }
            (
                RecoverySettingsMode::DataOnly
                | RecoverySettingsMode::AutomaticDataOnly
                | RecoverySettingsMode::ReconstructionDataOnly,
                _,
            ) => None,
            _ => return Err(StateError::integrity()),
        };
        let operation = self
            .scope
            .resume_operation(journal.operation_id(), guard)
            .map_err(map_platform_error)?;
        let candidate = journal.candidate();
        let mut observer = |_| Ok(());
        let candidate_stage = if matches!(
            journal.phase(),
            RecoveryPhase::Prepared | RecoveryPhase::SidecarsQuarantined
        ) {
            let stage = self
                .scope
                .resume_candidate(&operation, candidate.len(), *candidate.sha256())
                .map_err(map_platform_error)?;
            if stage.is_staged() {
                match package.as_ref() {
                    Some(package) => {
                        let verified =
                            self.verify_candidate(&stage, package, control, &mut observer)?;
                        if recovery_candidate_identity(&verified)? != candidate {
                            return Err(StateError::integrity());
                        }
                    }
                    None => self.verify_reconstruction_candidate(
                        &stage,
                        candidate,
                        control,
                        &mut observer,
                    )?,
                }
            } else {
                self.verify_active(candidate, guard, control, &mut observer)?;
            }
            Some(stage)
        } else {
            None
        };
        self.continue_restore(
            directory,
            catalog,
            journal,
            operation,
            candidate_stage,
            prepared_settings,
            guard,
            control,
            &mut observer,
        )
        .map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    fn continue_restore(
        &self,
        _directory: &BackupDirectory,
        _catalog: &BackupCatalog,
        mut journal: RecoveryJournal,
        operation: RecoveryOperation,
        mut candidate_stage: Option<RecoveryStagedArchive>,
        prepared_settings: Option<PreparedSettingsRestore>,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
        observer: &mut dyn FnMut(RecoveryBoundary) -> Result<(), StateError>,
    ) -> Result<RecoveryReceipt, StateError> {
        let before = journal.before().to_platform()?;
        let candidate = journal.candidate();
        if journal.phase() == RecoveryPhase::Prepared {
            self.scope
                .quarantine_sidecars(&operation, guard, before)
                .map_err(map_platform_error)?;
            observer(RecoveryBoundary::SidecarsQuarantinedBeforeJournal)?;
            journal = self
                .journal
                .advance(&journal, RecoveryPhase::SidecarsQuarantined)?;
            observer(RecoveryBoundary::JournalDurable(
                RecoveryPhase::SidecarsQuarantined,
            ))?;
        }
        if journal.phase() == RecoveryPhase::SidecarsQuarantined {
            let stage = candidate_stage
                .as_mut()
                .ok_or_else(StateError::recovery_required)?;
            let mode = if before.main().is_some() {
                RecoveryMainMode::ReplaceExisting
            } else {
                RecoveryMainMode::PromoteMissing
            };
            self.scope
                .promote_main(
                    &operation,
                    guard,
                    stage,
                    candidate.len(),
                    *candidate.sha256(),
                    before,
                    mode,
                )
                .map_err(map_platform_error)?;
            observer(RecoveryBoundary::MainPromotedBeforeJournal)?;
            journal = self
                .journal
                .advance(&journal, RecoveryPhase::MainReplaced)?;
            observer(RecoveryBoundary::JournalDurable(
                RecoveryPhase::MainReplaced,
            ))?;
        }
        if journal.phase() == RecoveryPhase::MainReplaced {
            if let Err(error) = self.verify_active(candidate, guard, control, observer) {
                self.scope
                    .rollback(&operation, guard, before)
                    .map_err(map_platform_error)?;
                return Err(error);
            }
            journal = self
                .journal
                .advance(&journal, RecoveryPhase::ReopenedVerified)?;
            observer(RecoveryBoundary::JournalDurable(
                RecoveryPhase::ReopenedVerified,
            ))?;
        }
        if journal.phase() == RecoveryPhase::ReopenedVerified {
            if let Err(error) = self.verify_active(candidate, guard, control, observer) {
                self.scope
                    .rollback(&operation, guard, before)
                    .map_err(map_platform_error)?;
                return Err(error);
            }
            let settings_result = match journal.settings_mode() {
                RecoverySettingsMode::DataOnly
                | RecoverySettingsMode::AutomaticDataOnly
                | RecoverySettingsMode::ReconstructionDataOnly => Ok(()),
                RecoverySettingsMode::DataAndPortableSettings => {
                    let prepared = prepared_settings
                        .as_ref()
                        .ok_or_else(StateError::recovery_required)?;
                    let expected = journal
                        .settings_target()
                        .ok_or_else(StateError::internal_invariant)?
                        .to_portable()?;
                    if prepared.target() != expected {
                        Err(StateError::integrity())
                    } else {
                        self.settings
                            .commit_prepared_restore_with_observer(prepared, |boundary| {
                                if boundary == SettingsRestoreBoundary::RecordPublishedBeforeReread
                                {
                                    observer(
                                        RecoveryBoundary::SettingsRecordPublishedBeforeReread,
                                    )?;
                                }
                                Ok(())
                            })
                            .and_then(|receipt| {
                                if receipt.target() == expected {
                                    Ok(())
                                } else {
                                    Err(StateError::recovery_required())
                                }
                            })
                    }
                }
            };
            if let Err(error) = settings_result {
                let expected = journal.settings_target().map(|target| target.to_portable());
                match expected {
                    Some(Ok(target)) => match self.settings.verify_target(target) {
                        Ok(true) => {}
                        Ok(false) => {
                            self.scope
                                .rollback(&operation, guard, before)
                                .map_err(map_platform_error)?;
                            return Err(error);
                        }
                        Err(_) => return Err(StateError::recovery_required()),
                    },
                    None if journal.settings_mode()
                        != RecoverySettingsMode::DataAndPortableSettings => {}
                    _ => return Err(StateError::recovery_required()),
                }
            }
            if journal.settings_mode() == RecoverySettingsMode::DataAndPortableSettings {
                observer(RecoveryBoundary::SettingsCommittedBeforeJournal)?;
            }
            journal = self
                .journal
                .advance(&journal, RecoveryPhase::SettingsPublished)?;
            observer(RecoveryBoundary::JournalDurable(
                RecoveryPhase::SettingsPublished,
            ))?;
        }
        if journal.phase() == RecoveryPhase::SettingsPublished {
            self.verify_active(candidate, guard, control, observer)?;
            if let Some(target) = journal.settings_target()
                && !self.settings.verify_target(target.to_portable()?)?
            {
                self.scope
                    .rollback(&operation, guard, before)
                    .map_err(map_platform_error)?;
                return Err(StateError::recovery_required());
            }
            journal = self.journal.advance(&journal, RecoveryPhase::Complete)?;
            observer(RecoveryBoundary::JournalDurable(RecoveryPhase::Complete))?;
        }
        if journal.phase() != RecoveryPhase::Complete {
            return Err(StateError::internal_invariant());
        }
        Ok(receipt_from_journal(&journal))
    }

    fn verify_candidate(
        &self,
        candidate: &RecoveryStagedArchive,
        package: &VerifiedBackupPackage,
        control: &BackupControl,
        observer: &mut dyn FnMut(RecoveryBoundary) -> Result<(), StateError>,
    ) -> Result<VerifiedRecoveryArchive, StateError> {
        let reader = self
            .scope
            .open_candidate_reader(candidate)
            .map_err(map_platform_error)?;
        let verified = self.verify_reader(
            reader,
            control,
            RecoveryBoundary::CandidateVerifierFileCreated,
            observer,
        )?;
        let identity = recovery_candidate_identity(&verified)?;
        require_package_database(package, identity)?;
        Ok(verified)
    }

    fn verify_reconstruction_candidate(
        &self,
        candidate: &RecoveryStagedArchive,
        expected: RecoveryCandidateIdentity,
        control: &BackupControl,
        observer: &mut dyn FnMut(RecoveryBoundary) -> Result<(), StateError>,
    ) -> Result<(), StateError> {
        let reader = self
            .scope
            .open_candidate_reader(candidate)
            .map_err(map_platform_error)?;
        let verified = self.verify_reader(
            reader,
            control,
            RecoveryBoundary::CandidateVerifierFileCreated,
            observer,
        )?;
        if recovery_candidate_identity(&verified)? != expected {
            return Err(StateError::integrity());
        }
        Ok(())
    }

    fn verify_active(
        &self,
        candidate: RecoveryCandidateIdentity,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
        observer: &mut dyn FnMut(RecoveryBoundary) -> Result<(), StateError>,
    ) -> Result<(), StateError> {
        let reader = self
            .scope
            .open_active_reader(guard, candidate.len(), *candidate.sha256())
            .map_err(map_platform_error)?;
        let verified = self.verify_reader(
            reader,
            control,
            RecoveryBoundary::ActiveVerifierFileCreated,
            observer,
        )?;
        if recovery_candidate_identity(&verified)? != candidate {
            return Err(StateError::integrity());
        }
        Ok(())
    }

    fn prepare_settings(
        &self,
        mode: RestoreMode,
        candidate: &PortableSettingsCandidate,
    ) -> Result<Option<PreparedSettingsRestore>, StateError> {
        match mode {
            RestoreMode::DataAndPortableSettings => {
                self.settings.prepare_restore(candidate).map(Some)
            }
            RestoreMode::DataOnly | RestoreMode::AutomaticDataOnly => Ok(None),
        }
    }

    fn recover_verifier_staging(&self) -> Result<(), StateError> {
        self.verification_staging
            .recover_abandoned_candidates()
            .map(|_| ())
            .map_err(|error| map_store_error(error.code()))
    }

    fn authorize_backup_directory(&self, directory: &BackupDirectory) -> Result<(), StateError> {
        directory
            .authorize_parent(&self.reliable_state)
            .map_err(map_backup_directory_error)
    }

    fn require_definitive_corruption(
        &self,
        observation: ArchiveSetObservation,
        guard: &ExclusiveFileLeaseGuard,
        control: &BackupControl,
        observer: &mut dyn FnMut(RecoveryBoundary) -> Result<(), StateError>,
    ) -> Result<(), StateError> {
        let Some(main) = observation.main() else {
            // The already revalidated, fully verified backup selection is durable
            // prior-installation evidence even when the active main is missing.
            return Ok(());
        };
        let reader = self
            .scope
            .open_active_reader(guard, main.len(), *main.sha256())
            .map_err(map_platform_error)?;
        let mut observer_error = None;
        let result = verify_recovery_archive_with_observer(
            reader,
            self.verification_staging,
            control,
            |boundary| {
                if boundary == RecoveryVerificationBoundary::CandidateCreated
                    && observer_error.is_none()
                {
                    observer_error =
                        observer(RecoveryBoundary::CorruptionVerifierFileCreated).err();
                }
            },
        );
        if let Some(error) = observer_error {
            return Err(error);
        }
        match result {
            Ok(_) => Err(StateError::invalid_input()),
            Err(error) if is_definitive_corruption(error.code()) => Ok(()),
            Err(error) => Err(map_store_error(error.code())),
        }
    }

    fn verify_reader(
        &self,
        reader: DurableFileReader,
        control: &BackupControl,
        boundary: RecoveryBoundary,
        observer: &mut dyn FnMut(RecoveryBoundary) -> Result<(), StateError>,
    ) -> Result<VerifiedRecoveryArchive, StateError> {
        let mut observer_error = None;
        let result = verify_recovery_archive_with_observer(
            reader,
            self.verification_staging,
            control,
            |store_boundary| {
                if store_boundary == RecoveryVerificationBoundary::CandidateCreated
                    && observer_error.is_none()
                {
                    observer_error = observer(boundary).err();
                }
            },
        );
        if let Some(error) = observer_error {
            return Err(error);
        }
        result.map_err(|error| map_store_error(error.code()))
    }
}

impl fmt::Debug for RecoveryCoordinator<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryCoordinator([redacted])")
    }
}

fn validate_safety(safety: RestoreSafety) -> Result<(), StateError> {
    match safety {
        RestoreSafety::PreRestoreBackupPublished(completion)
            if completion.purpose() == MaintenancePurpose::PreRestore
                && completion.outcome() == MaintenanceOutcome::Published
                && completion.allows_mutation() =>
        {
            Ok(())
        }
        RestoreSafety::PreRestoreBackupPublished(_) => Err(StateError::invalid_input()),
    }
}

fn validate_mode_authority(
    mode: RestoreMode,
    authority: RestoreAuthority,
) -> Result<(), StateError> {
    match (mode, authority) {
        (RestoreMode::AutomaticDataOnly, RestoreAuthority::DefinitiveCorruption) => Ok(()),
        (RestoreMode::AutomaticDataOnly, RestoreAuthority::Safety(_)) => {
            Err(StateError::invalid_input())
        }
        (_, RestoreAuthority::Safety(safety)) => validate_safety(safety),
        (_, RestoreAuthority::DefinitiveCorruption) => Ok(()),
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

fn validate_safety_for_archive(
    safety: RestoreSafety,
    has_prior_artifact: bool,
) -> Result<(), StateError> {
    if has_prior_artifact {
        validate_safety(safety)
    } else {
        Err(StateError::invalid_input())
    }
}

fn require_package_identity(
    package: &VerifiedBackupPackage,
    backup: RecoveryBackupIdentity,
) -> Result<(), StateError> {
    let receipt = package.receipt();
    if receipt.package_len() == backup.package_len()
        && receipt.file_sha256() == backup.package_sha256()
    {
        Ok(())
    } else {
        Err(StateError::integrity())
    }
}

fn require_package_database(
    package: &VerifiedBackupPackage,
    candidate: RecoveryCandidateIdentity,
) -> Result<(), StateError> {
    if u32::from(package.database_schema_version()) == candidate.schema_version()
        && package.database_len() == candidate.len()
        && package.database_sha256() == candidate.sha256()
    {
        Ok(())
    } else {
        Err(StateError::integrity())
    }
}

fn recovery_candidate_identity(
    verified: &VerifiedRecoveryArchive,
) -> Result<RecoveryCandidateIdentity, StateError> {
    RecoveryCandidateIdentity::from_persisted(
        verified.schema_version(),
        verified.len(),
        *verified.sha256(),
    )
}

fn receipt_from_journal(journal: &RecoveryJournal) -> RecoveryReceipt {
    RecoveryReceipt {
        operation_generation: journal.operation_generation(),
        candidate: journal.candidate(),
        settings_mode: journal.settings_mode(),
        reconstructed_from_authoritative_source: journal.backup().is_none(),
    }
}

fn required_recovery_capacity(database_len: u64, active_len: u64) -> Result<u64, StateError> {
    const OPERATION_RESERVE_BYTES: u64 = 8 * 1024 * 1024;
    let candidate_verification = database_len
        .checked_mul(2)
        .ok_or_else(StateError::capacity_exceeded)?;
    let corruption_verification = database_len
        .checked_add(active_len)
        .ok_or_else(StateError::capacity_exceeded)?;
    candidate_verification
        .max(corruption_verification)
        .checked_add(OPERATION_RESERVE_BYTES)
        .ok_or_else(StateError::capacity_exceeded)
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

const fn map_store_error(code: StoreErrorCode) -> StateError {
    match code {
        StoreErrorCode::CapacityExceeded => StateError::capacity_exceeded(),
        StoreErrorCode::Busy => StateError::from_code(StateErrorCode::Busy),
        StoreErrorCode::StaleBackupCandidate
        | StoreErrorCode::BackupHeaderCorrupt
        | StoreErrorCode::BackupPageCorrupt
        | StoreErrorCode::BackupIndexCorrupt
        | StoreErrorCode::BackupForeignKeyCorrupt
        | StoreErrorCode::BackupCountCorrupt
        | StoreErrorCode::BackupGenerationCorrupt
        | StoreErrorCode::BackupSemanticCorrupt => StateError::integrity(),
        StoreErrorCode::InvalidValue => StateError::invalid_input(),
        StoreErrorCode::VersionMismatch
        | StoreErrorCode::SchemaTooNew
        | StoreErrorCode::SchemaMismatch
        | StoreErrorCode::PolicyMismatch => StateError::unsupported_version(),
        StoreErrorCode::Cancelled
        | StoreErrorCode::DeadlineExceeded
        | StoreErrorCode::BackupIo
        | StoreErrorCode::Database => StateError::unavailable(),
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
        | StoreErrorCode::ArchiveModeMismatch => StateError::internal_invariant(),
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

#[cfg(test)]
mod capacity_tests {
    use super::required_recovery_capacity;
    use crate::{StateError, StateErrorCode};

    const RESERVE: u64 = 8 * 1024 * 1024;

    #[test]
    fn capacity_covers_the_larger_live_recovery_window() {
        assert_eq!(required_recovery_capacity(10, 100), Ok(110 + RESERVE));
        assert_eq!(required_recovery_capacity(100, 10), Ok(200 + RESERVE));
    }

    #[test]
    fn capacity_arithmetic_fails_closed_on_overflow() {
        assert_eq!(
            required_recovery_capacity(u64::MAX, 1),
            Err(StateError::from_code(StateErrorCode::CapacityExceeded))
        );
    }
}
