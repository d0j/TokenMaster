use std::fmt;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokenmaster_desktop::{
    DesktopBackupHealth, DesktopBackupPolicy, DesktopBoardPreferences, DesktopBoardSectionKey,
    DesktopBoardSectionPreference, DesktopConfigImportPreview, DesktopOperationSnapshot,
    DesktopPresentationSettings, DesktopRecoveryReceipt, DesktopReliableStateHealth,
    DesktopReliableStateInput, DesktopReliableStateProjection, DesktopReliableStateSummary,
    DesktopReminderPolicy, DesktopReminderSyncState, DesktopRestorePointInput,
    DesktopRestoreSelection,
};
use tokenmaster_domain::{
    NotificationChannel, ReminderLeadTime, ReminderProfile, ReminderProfileParts,
    ReminderProfileRevision,
};
use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, BackupDirectoryError, ExclusiveFileLease,
    ExclusiveFileLeaseGuard, MAX_DURABLE_FILE_BYTES, SelectedInputFile, SelectedOutputFile,
    ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCatalog, BackupCompression, BackupEncryptionContext, BackupMaintenanceRuntime,
    BackupMetadata, BackupPackage, BackupPassphrase, BackupPolicy, BackupPurpose, BootstrapOutcome,
    BootstrapReport, CatalogHealth, CatalogSelectionBinding, ConfigPackage, EncryptedBackupPackage,
    MAX_CONFIG_PACKAGE_BYTES, MaintenanceExecution, MaintenancePermit, MaintenanceSourceState,
    PendingMigration, PortableSettings, PreparedBootstrap, PresentationColorScheme,
    PresentationDensity, PresentationLayout, PresentationSettings, PresentationSkin,
    RecoveryBoundary, RecoveryCoordinator, RecoveryJournalLoad, RecoveryJournalStore,
    RecoveryLaunchDecision, RecoveryPhase, RecoveryReceipt, RestoreMode, RestoreSafety,
    RetentionAdmission, RetentionPolicy, RunSession, RunStateStore, SettingsCommitReceipt,
    SettingsImportPreview, SettingsStore, SettingsValue, StateBootstrap, StateErrorCode,
    SystemMaintenanceClock,
};

#[cfg(test)]
use tokenmaster_state::SettingsChangeCategory;
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, StartupArchiveStatus, StartupValidationMode,
    StoreErrorCode, USAGE_SCHEMA_VERSION, UsageStore, create_compact_snapshot,
    create_online_snapshot, inspect_startup_archive, verify_backup_candidate,
};

use crate::command::{
    ApplicationBackupPolicyUpdate, ApplicationBackupSelection, ApplicationCommand,
    ApplicationCommandPermit,
};
use crate::{ApplicationError, DataRoot};

const STARTUP_RECOVERY_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const REMINDER_SYNC_PENDING: u8 = 0;
const REMINDER_SYNC_SYNCHRONIZED: u8 = 1;

pub(crate) struct ApplicationStateOwner {
    scope: ArchiveRecoveryScope,
    backups: BackupDirectory,
    verification_staging: BackupStaging,
    settings: SettingsStore,
    journal: RecoveryJournalStore,
    run_state: RunStateStore,
    catalog: Arc<Mutex<Option<Arc<BackupCatalog>>>>,
    restore_pin: Arc<Mutex<Option<CatalogSelectionBinding>>>,
    pending_config_import: Mutex<Option<ApplicationConfigImportPreview>>,
    reminder_sync_state: AtomicU8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationConfigExportReceipt {
    created_at_utc_ms: i64,
    package_bytes: u64,
}

#[cfg_attr(not(test), allow(dead_code))]
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

pub(crate) struct ApplicationConfigImportPreview {
    settings: SettingsImportPreview,
    created_at_utc_ms: i64,
    package_bytes: u64,
}

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
    #[cfg(test)]
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
            pending_config_import: Mutex::new(None),
            reminder_sync_state: AtomicU8::new(REMINDER_SYNC_PENDING),
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
        let recovery_launch = bootstrap.report().recovery_launch();
        let source_reconciliation_required = matches!(
            recovery_launch,
            RecoveryLaunchDecision::Start { .. } | RecoveryLaunchDecision::SafeMode { .. }
        ) && matches!(
            self.journal.load().map_err(|_| ApplicationError::state())?,
            RecoveryJournalLoad::Pending(journal)
                if journal.phase() == RecoveryPhase::Complete && journal.backup().is_none()
        );
        let effective_outcome = match (source_reconciliation_required, recovery_launch) {
            (true, RecoveryLaunchDecision::Start { .. }) => BootstrapOutcome::RecoveryRequired,
            _ => bootstrap.report().outcome(),
        };
        Ok(ApplicationPreflight {
            bootstrap,
            startup_guard: Some(guard),
            effective_outcome,
            source_reconciliation_required,
        })
    }

    #[cfg(test)]
    pub(crate) fn reliable_state_projection(
        &self,
        report: BootstrapReport,
    ) -> Result<DesktopReliableStateProjection, ApplicationError> {
        self.reliable_state_projection_for_outcome(report.outcome(), None)
    }

    pub(crate) fn reliable_state_projection_for_outcome(
        &self,
        outcome: BootstrapOutcome,
        operation: Option<DesktopOperationSnapshot>,
    ) -> Result<DesktopReliableStateProjection, ApplicationError> {
        let settings = self
            .settings
            .load()
            .map_err(|_| ApplicationError::state())?;
        let policy = settings.value().portable().backup();
        let reminders = settings.value().portable().reminders();
        let reminder_policy = DesktopReminderPolicy::new(
            reminders.enabled(),
            reminders.lead_seconds(),
            self.reminder_sync_state()?,
        )
        .ok_or_else(ApplicationError::state)?;
        let previous = self
            .catalog
            .lock()
            .map_err(|_| ApplicationError::state())?
            .clone();
        let catalog = Arc::new(
            BackupCatalog::rebuild(&self.backups, previous.as_deref())
                .map_err(|_| ApplicationError::state())?,
        );
        *self.catalog.lock().map_err(|_| ApplicationError::state())? = Some(Arc::clone(&catalog));

        let corrupt_count = catalog
            .points()
            .iter()
            .filter(|point| point.health() == CatalogHealth::Corrupt)
            .count() as u64;
        let usable_count = catalog.points().len() as u64 - corrupt_count;
        let usable_bytes = catalog
            .points()
            .iter()
            .filter(|point| point.health() != CatalogHealth::Corrupt)
            .try_fold(0_u64, |total, point| total.checked_add(point.size_bytes()))
            .ok_or_else(ApplicationError::state)?;
        let latest_success = catalog
            .points()
            .iter()
            .find(|point| point.health() != CatalogHealth::Corrupt)
            .and_then(|point| point.created_at_utc_ms());
        let recovery_receipt = match self.journal.load().map_err(|_| ApplicationError::state())? {
            RecoveryJournalLoad::Pending(journal) if journal.phase() == RecoveryPhase::Complete => {
                Some(if journal.backup().is_some() {
                    DesktopRecoveryReceipt::restored_from_verified_backup()
                } else {
                    DesktopRecoveryReceipt::reconstructed_from_authoritative_source()
                })
            }
            RecoveryJournalLoad::Absent | RecoveryJournalLoad::Pending(_) => None,
            RecoveryJournalLoad::Invalid => return Err(ApplicationError::state()),
        };
        let health = reliable_health(outcome, corrupt_count != 0, settings.health_code());
        let state_presentation = settings.value().portable().presentation();
        let density = match state_presentation.density() {
            PresentationDensity::Comfortable => tokenmaster_desktop::DesktopDensity::Comfortable,
            PresentationDensity::Compact => tokenmaster_desktop::DesktopDensity::Compact,
            PresentationDensity::UltraCompact => tokenmaster_desktop::DesktopDensity::UltraCompact,
        };
        let skin = match state_presentation.skin() {
            PresentationSkin::Refined => tokenmaster_desktop::DesktopSkin::Refined,
            PresentationSkin::Graphite => tokenmaster_desktop::DesktopSkin::Graphite,
            PresentationSkin::Ember => tokenmaster_desktop::DesktopSkin::Ember,
        };
        let color_scheme = match state_presentation.color_scheme() {
            PresentationColorScheme::System => tokenmaster_desktop::DesktopColorScheme::System,
            PresentationColorScheme::Light => tokenmaster_desktop::DesktopColorScheme::Light,
            PresentationColorScheme::Dark => tokenmaster_desktop::DesktopColorScheme::Dark,
        };
        let layout = match state_presentation.layout() {
            PresentationLayout::Refined => tokenmaster_desktop::DesktopLayout::Refined,
            PresentationLayout::ControlCenter => tokenmaster_desktop::DesktopLayout::ControlCenter,
            PresentationLayout::Workbench => tokenmaster_desktop::DesktopLayout::Workbench,
        };
        let rows = state_presentation.board().rows().map(|row| {
            let key = match row.key() {
                tokenmaster_state::BoardSectionKey::PlanUsage => DesktopBoardSectionKey::PlanUsage,
                tokenmaster_state::BoardSectionKey::CodeOutput => {
                    DesktopBoardSectionKey::CodeOutput
                }
                tokenmaster_state::BoardSectionKey::Trend => DesktopBoardSectionKey::Trend,
                tokenmaster_state::BoardSectionKey::Sessions => DesktopBoardSectionKey::Sessions,
                tokenmaster_state::BoardSectionKey::Activity => DesktopBoardSectionKey::Activity,
                tokenmaster_state::BoardSectionKey::Models => DesktopBoardSectionKey::Models,
            };
            DesktopBoardSectionPreference::new(key, row.visible(), row.collapsed())
        });
        let board = match DesktopBoardPreferences::new(rows) {
            Some(board) => board,
            None => unreachable!("state board preferences are validated before projection"),
        };
        let presentation =
            DesktopPresentationSettings::new(density, skin, color_scheme, layout).with_board(board);
        let summary = DesktopReliableStateSummary::new_with_settings(
            health,
            matches!(
                outcome,
                BootstrapOutcome::SafeMode | BootstrapOutcome::RecoveryRequired
            ),
            settings.health_code().as_str(),
            DesktopBackupPolicy::new(
                policy.periodic_enabled(),
                policy.quiet_seconds(),
                policy.interval_seconds(),
                policy.retention_budget_bytes(),
            ),
            reminder_policy,
            presentation,
            latest_success,
            catalog
                .points()
                .first()
                .and_then(|point| point.created_at_utc_ms()),
            Some(usable_count),
            Some(corrupt_count),
            Some(usable_bytes),
            (corrupt_count != 0).then_some("integrity"),
            recovery_receipt,
            operation,
            self.config_import_preview()?,
        );
        let restore_points = catalog
            .points()
            .iter()
            .filter_map(|point| {
                Some(DesktopRestorePointInput::new(
                    DesktopRestoreSelection::new(
                        point.selection().generation().get(),
                        point.selection().ordinal(),
                    )?,
                    point.created_at_utc_ms(),
                    point.size_bytes(),
                    map_catalog_health(point.health()),
                    point.purpose().map_or("unavailable", backup_purpose_code),
                    point.database_schema_version(),
                    point
                        .compression()
                        .map_or("unavailable", backup_compression_code),
                ))
            })
            .collect();
        Ok(DesktopReliableStateProjection::from_input(
            DesktopReliableStateInput::new(catalog.generation().get(), summary, restore_points),
        ))
    }

    pub(crate) fn export_config(
        &self,
        permit: &ApplicationCommandPermit,
        mut target: SelectedOutputFile,
        created_at_utc_ms: i64,
        mut on_irreversible: impl FnMut(),
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
        on_irreversible();
        let published = target
            .publish(&mut staged)
            .map_err(|_| ApplicationError::state())?;
        if published.len() != package.package_len() {
            return Err(ApplicationError::state());
        }
        let mut reader = target
            .open_reader(MAX_CONFIG_PACKAGE_BYTES)
            .map_err(|_| ApplicationError::state())?
            .into_reader();
        let verified = ConfigPackage::read(&mut reader).map_err(|_| ApplicationError::state())?;
        if verified.receipt() != package || verified.settings().digest() != settings.digest() {
            return Err(ApplicationError::state());
        }
        Ok(ApplicationConfigExportReceipt {
            created_at_utc_ms,
            package_bytes: package.package_len(),
        })
    }

    pub(crate) fn export_compact_backup(
        &self,
        root: &DataRoot,
        permit: &ApplicationCommandPermit,
        mut target: SelectedOutputFile,
        mut on_irreversible: impl FnMut(),
    ) -> Result<(), ApplicationError> {
        if permit.command() != ApplicationCommand::BackupCompact || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let (mut package_stage, verified) =
            self.prepare_manual_export(root, permit, true, BackupCompression::Compact)?;
        let expected = verified.receipt();
        let mut output_stage = target
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .map_err(|_| ApplicationError::state())?;
        BackupPackage::copy_verified_stage_to_durable(&package_stage, &verified, &mut output_stage)
            .map_err(|_| ApplicationError::state())?;
        package_stage
            .discard()
            .map_err(|_| ApplicationError::state())?;
        if permit.is_cancelled() {
            output_stage
                .discard()
                .map_err(|_| ApplicationError::state())?;
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        on_irreversible();
        let published = target
            .publish(&mut output_stage)
            .map_err(|_| ApplicationError::state())?;
        if published.len() != expected.package_len() {
            return Err(ApplicationError::state());
        }
        let mut reader = target
            .open_reader(MAX_DURABLE_FILE_BYTES)
            .map_err(|_| ApplicationError::state())?
            .into_reader();
        let reread = BackupPackage::inspect(&mut reader).map_err(|_| ApplicationError::state())?;
        if reread.receipt() != expected || reread.compression() != BackupCompression::Compact {
            return Err(ApplicationError::state());
        }
        Ok(())
    }

    pub(crate) fn export_encrypted_backup(
        &self,
        root: &DataRoot,
        permit: &ApplicationCommandPermit,
        mut target: SelectedOutputFile,
        passphrase: BackupPassphrase,
        mut on_irreversible: impl FnMut(),
    ) -> Result<(), ApplicationError> {
        if permit.command() != ApplicationCommand::BackupEncrypted || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let (mut package_stage, verified) =
            self.prepare_manual_export(root, permit, false, BackupCompression::Normal)?;
        let mut output_stage = target
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .map_err(|_| ApplicationError::state())?;
        let protected = {
            let mut reader = package_stage
                .open_reader()
                .map_err(|_| ApplicationError::state())?;
            EncryptedBackupPackage::encrypt(
                BackupEncryptionContext::ManualExport,
                &mut reader,
                &verified,
                passphrase,
                &mut output_stage,
            )
            .map_err(|_| ApplicationError::state())?
        };
        package_stage
            .discard()
            .map_err(|_| ApplicationError::state())?;
        if permit.is_cancelled() {
            output_stage
                .discard()
                .map_err(|_| ApplicationError::state())?;
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        on_irreversible();
        let published = target
            .publish(&mut output_stage)
            .map_err(|_| ApplicationError::state())?;
        if published.len() != protected.output_len()
            || published.sha256() != protected.output_sha256()
        {
            return Err(ApplicationError::state());
        }
        Ok(())
    }

    fn prepare_manual_export(
        &self,
        root: &DataRoot,
        permit: &ApplicationCommandPermit,
        compact: bool,
        compression: BackupCompression,
    ) -> Result<
        (
            tokenmaster_platform::BackupStagedFile,
            tokenmaster_state::VerifiedBackupPackage,
        ),
        ApplicationError,
    > {
        let control = BackupControl::new(permit.cancellation_flag(), STARTUP_RECOVERY_TIMEOUT)
            .map_err(|_| ApplicationError::state())?;
        let source =
            BackupSource::new(root.validated_directory()).map_err(|_| ApplicationError::state())?;
        let snapshot = create_online_snapshot(&source, &self.verification_staging, &control)
            .and_then(|candidate| verify_backup_candidate(candidate, &control))
            .map_err(|_| ApplicationError::state())?;
        let compact_candidate = compact
            .then(|| create_compact_snapshot(&snapshot, &self.verification_staging, &control))
            .transpose()
            .map_err(|_| ApplicationError::state())?;
        let candidate = compact_candidate.as_ref().unwrap_or(&snapshot);
        let settings = self
            .settings
            .full_backup_candidate()
            .map_err(|_| ApplicationError::state())?;
        let mut stage = self
            .backups
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .map_err(|_| ApplicationError::state())?;
        let reader = candidate
            .open_reader(&control)
            .map_err(|_| ApplicationError::state())?;
        let metadata = BackupMetadata::new(
            current_utc_millis().map_err(|_| ApplicationError::state())?,
            BackupPurpose::Manual,
        )
        .map_err(|_| ApplicationError::state())?;
        BackupPackage::write_verified_candidate_to_backup_stage(
            &settings,
            reader,
            compression,
            metadata,
            &mut stage,
        )
        .map_err(|_| ApplicationError::state())?;
        let verified =
            BackupPackage::verify_backup_stage(&stage).map_err(|_| ApplicationError::state())?;
        if permit.is_cancelled() {
            stage.discard().map_err(|_| ApplicationError::state())?;
            return Err(ApplicationError::invalid_lifecycle());
        }
        Ok((stage, verified))
    }

    pub(crate) fn update_backup_policy(
        &self,
        permit: &ApplicationCommandPermit,
        update: ApplicationBackupPolicyUpdate,
        mut on_irreversible: impl FnMut(),
    ) -> Result<BackupPolicy, ApplicationError> {
        if permit.command() != ApplicationCommand::UpdateBackupPolicy || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let retention_budget_bytes = u64::from(update.retention_budget_mib()) * 1024 * 1024;
        let policy = BackupPolicy::new(
            update.periodic_enabled(),
            update.quiet_seconds(),
            update.interval_seconds(),
            retention_budget_bytes,
        )
        .map_err(|_| ApplicationError::state())?;
        let current = self
            .settings
            .load()
            .map_err(|_| ApplicationError::state())?;
        let value = SettingsValue::new(
            PortableSettings::new(
                current.value().portable().reminders().clone(),
                policy.clone(),
                *current.value().portable().presentation(),
            ),
            current.value().device().clone(),
        );
        if permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        on_irreversible();
        self.settings
            .save(&value)
            .map_err(|_| ApplicationError::state())?;
        Ok(policy)
    }

    pub(crate) fn update_presentation(
        &self,
        permit: &ApplicationCommandPermit,
        presentation: PresentationSettings,
        mut on_irreversible: impl FnMut(),
    ) -> Result<(), ApplicationError> {
        if permit.command() != ApplicationCommand::UpdatePresentation || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let current = self
            .settings
            .load()
            .map_err(|_| ApplicationError::state())?;
        if current.value().portable().presentation() == &presentation {
            return Ok(());
        }
        let value = SettingsValue::new(
            PortableSettings::new(
                current.value().portable().reminders().clone(),
                current.value().portable().backup().clone(),
                presentation,
            ),
            current.value().device().clone(),
        );
        if permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        on_irreversible();
        self.settings
            .save(&value)
            .map_err(|_| ApplicationError::state())?;
        Ok(())
    }

    pub(crate) fn update_reminder_policy(
        &self,
        permit: &ApplicationCommandPermit,
        policy: tokenmaster_state::ReminderPolicy,
        mut on_irreversible: impl FnMut() -> Result<(), ApplicationError>,
    ) -> Result<(), ApplicationError> {
        if permit.command() != ApplicationCommand::UpdateReminderPolicy || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let current = self
            .settings
            .load()
            .map_err(|_| ApplicationError::state())?;
        if current.value().portable().reminders() == &policy {
            return Ok(());
        }
        let value = SettingsValue::new(
            PortableSettings::new(
                policy.clone(),
                current.value().portable().backup().clone(),
                *current.value().portable().presentation(),
            ),
            current.value().device().clone(),
        );
        if permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        let previous_sync_state = self
            .reminder_sync_state
            .swap(REMINDER_SYNC_PENDING, Ordering::AcqRel);
        if on_irreversible().is_err() {
            self.reminder_sync_state
                .store(previous_sync_state, Ordering::Release);
            return Err(ApplicationError::state());
        }
        self.settings
            .save(&value)
            .map_err(|_| ApplicationError::state())?;
        Ok(())
    }

    pub(crate) fn synchronize_reminder_profile(
        &self,
        root: &DataRoot,
    ) -> Result<ReminderProfile, ApplicationError> {
        self.reminder_sync_state
            .store(REMINDER_SYNC_PENDING, Ordering::Release);
        let settings = self
            .settings
            .load()
            .map_err(|_| ApplicationError::state())?;
        let profile = reminder_profile_from_settings(
            settings.generation(),
            settings.value().portable().reminders(),
        )?;
        let mut store =
            UsageStore::open_current(root.archive_path()).map_err(|_| ApplicationError::state())?;
        store
            .set_benefit_reminder_global_profile(&profile)
            .map_err(|_| ApplicationError::state())?;
        self.reminder_sync_state
            .store(REMINDER_SYNC_SYNCHRONIZED, Ordering::Release);
        Ok(profile)
    }

    fn reminder_sync_state(&self) -> Result<DesktopReminderSyncState, ApplicationError> {
        match self.reminder_sync_state.load(Ordering::Acquire) {
            REMINDER_SYNC_PENDING => Ok(DesktopReminderSyncState::Pending),
            REMINDER_SYNC_SYNCHRONIZED => Ok(DesktopReminderSyncState::Synchronized),
            _ => Err(ApplicationError::state()),
        }
    }

    pub(crate) fn preview_config_import(
        &self,
        permit: &ApplicationCommandPermit,
        source: SelectedInputFile,
    ) -> Result<ApplicationConfigImportPreview, ApplicationError> {
        if permit.command() != ApplicationCommand::ImportConfig || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let mut source = source.into_reader();
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

    pub(crate) fn stage_config_import_preview(
        &self,
        permit: &ApplicationCommandPermit,
        source: SelectedInputFile,
    ) -> Result<(), ApplicationError> {
        if self
            .pending_config_import
            .lock()
            .map_err(|_| ApplicationError::state())?
            .is_some()
        {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let preview = self.preview_config_import(permit, source)?;
        let mut pending = self
            .pending_config_import
            .lock()
            .map_err(|_| ApplicationError::state())?;
        if pending.is_some() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        *pending = Some(preview);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn commit_config_import(
        &self,
        permit: &ApplicationCommandPermit,
        preview: ApplicationConfigImportPreview,
    ) -> Result<SettingsCommitReceipt, ApplicationError> {
        if !matches!(
            permit.command(),
            ApplicationCommand::ImportConfig | ApplicationCommand::ConfirmConfigImport
        ) || permit.is_cancelled()
        {
            return Err(ApplicationError::invalid_lifecycle());
        }
        permit
            .begin_irreversible()
            .map_err(|_| ApplicationError::invalid_lifecycle())?;
        self.settings
            .commit_import(&preview.settings)
            .map_err(|_| ApplicationError::state())
    }

    pub(crate) fn commit_pending_config_import(
        &self,
        permit: &ApplicationCommandPermit,
        mut on_irreversible: impl FnMut() -> Result<(), ApplicationError>,
    ) -> Result<SettingsCommitReceipt, ApplicationError> {
        if permit.command() != ApplicationCommand::ConfirmConfigImport || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let preview = self
            .pending_config_import
            .lock()
            .map_err(|_| ApplicationError::state())?
            .take()
            .ok_or_else(ApplicationError::invalid_lifecycle)?;
        if permit.begin_irreversible().is_err() {
            let mut pending = self
                .pending_config_import
                .lock()
                .map_err(|_| ApplicationError::state())?;
            if pending.is_none() {
                *pending = Some(preview);
            }
            return Err(ApplicationError::invalid_lifecycle());
        }
        let previous_sync_state = self
            .reminder_sync_state
            .swap(REMINDER_SYNC_PENDING, Ordering::AcqRel);
        {
            let mut pending = self
                .pending_config_import
                .lock()
                .map_err(|_| ApplicationError::state())?;
            if pending.is_some() {
                self.reminder_sync_state
                    .store(previous_sync_state, Ordering::Release);
                return Err(ApplicationError::state());
            }
            *pending = Some(preview);
        }
        if on_irreversible().is_err() {
            self.reminder_sync_state
                .store(previous_sync_state, Ordering::Release);
            return Err(ApplicationError::state());
        }
        let preview = self
            .pending_config_import
            .lock()
            .map_err(|_| ApplicationError::state())?
            .take()
            .ok_or_else(ApplicationError::state)?;
        self.settings
            .commit_import(&preview.settings)
            .map_err(|_| ApplicationError::state())
    }

    pub(crate) fn cancel_pending_config_import(
        &self,
        permit: &ApplicationCommandPermit,
    ) -> Result<(), ApplicationError> {
        if permit.command() != ApplicationCommand::CancelConfigImport || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        self.pending_config_import
            .lock()
            .map_err(|_| ApplicationError::state())?
            .take()
            .ok_or_else(ApplicationError::invalid_lifecycle)?;
        Ok(())
    }

    fn config_import_preview(
        &self,
    ) -> Result<Option<DesktopConfigImportPreview>, ApplicationError> {
        let pending = self
            .pending_config_import
            .lock()
            .map_err(|_| ApplicationError::state())?;
        pending
            .as_ref()
            .map(|preview| {
                Ok(DesktopConfigImportPreview::new(
                    preview.created_at_utc_ms(),
                    preview.package_bytes(),
                    u8::try_from(preview.changed_category_count())
                        .map_err(|_| ApplicationError::state())?,
                    u16::try_from(preview.changed_field_count())
                        .map_err(|_| ApplicationError::state())?,
                ))
            })
            .transpose()
    }

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

    pub(crate) fn verify_backups(
        &self,
        permit: &ApplicationCommandPermit,
    ) -> Result<(), ApplicationError> {
        if permit.command() != ApplicationCommand::Verify || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let previous = self.catalog_snapshot().ok();
        let mut catalog = BackupCatalog::rebuild(&self.backups, previous.as_deref())
            .map_err(|_| ApplicationError::state())?;
        catalog
            .verify_all_packages(&self.backups)
            .map_err(|_| ApplicationError::state())?;
        if permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        *self.catalog.lock().map_err(|_| ApplicationError::state())? = Some(Arc::new(catalog));
        Ok(())
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

    pub(crate) fn reconstruct_definitively_corrupt<F>(
        &self,
        permit: &ApplicationCommandPermit,
        guard: &ExclusiveFileLeaseGuard,
        mut on_irreversible: F,
    ) -> Result<RecoveryReceipt, ApplicationError>
    where
        F: FnMut(),
    {
        if permit.command() != ApplicationCommand::Rebuild || permit.is_cancelled() {
            return Err(ApplicationError::invalid_lifecycle());
        }
        let previous = self.catalog_snapshot().ok();
        let mut catalog = BackupCatalog::rebuild(&self.backups, previous.as_deref())
            .map_err(|_| ApplicationError::state())?;
        let control = BackupControl::new(permit.cancellation_flag(), STARTUP_RECOVERY_TIMEOUT)
            .map_err(|_| ApplicationError::state())?;
        let result = RecoveryCoordinator::new(
            &self.scope,
            &self.verification_staging,
            &self.journal,
            &self.settings,
        )
        .and_then(|recovery| {
            recovery.reconstruct_definitively_corrupt_with_observer(
                &self.backups,
                &mut catalog,
                guard,
                &control,
                |boundary| {
                    if boundary == RecoveryBoundary::BeforeJournalPublication {
                        permit.begin_irreversible().map_err(|_| {
                            tokenmaster_state::StateError::from_code(StateErrorCode::Unavailable)
                        })?;
                        on_irreversible();
                    }
                    Ok(())
                },
            )
        });
        *self.catalog.lock().map_err(|_| ApplicationError::state())? = Some(Arc::new(catalog));
        result.map_err(|_| ApplicationError::state())
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

fn reminder_profile_from_settings(
    generation: Option<u64>,
    policy: &tokenmaster_state::ReminderPolicy,
) -> Result<ReminderProfile, ApplicationError> {
    let revision = generation
        .unwrap_or(0)
        .checked_add(1)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(ApplicationError::state)?;
    let lead_times = policy
        .lead_seconds()
        .iter()
        .copied()
        .map(ReminderLeadTime::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ApplicationError::state())?;
    let channels = policy
        .enabled()
        .then_some(NotificationChannel::InApp)
        .into_iter()
        .collect();
    ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(revision).map_err(|_| ApplicationError::state())?,
        lead_times,
        channels,
    })
    .map_err(|_| ApplicationError::state())
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
    effective_outcome: BootstrapOutcome,
    source_reconciliation_required: bool,
}

impl ApplicationPreflight {
    pub(crate) fn report(&self) -> BootstrapReport {
        self.bootstrap.report()
    }

    pub(crate) const fn effective_outcome(&self) -> BootstrapOutcome {
        self.effective_outcome
    }

    pub(crate) fn mark_live_healthy(&mut self) {
        self.effective_outcome = BootstrapOutcome::Healthy;
        self.source_reconciliation_required = false;
    }

    pub(crate) const fn requires_source_reconciliation(&self) -> bool {
        self.source_reconciliation_required
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
        let decision = self
            .bootstrap
            .session_mut()
            .start_recovered_candidate(receipt.operation_generation(), receipt.candidate())
            .map_err(|_| ApplicationError::state())?;
        match decision {
            RecoveryLaunchDecision::Start { .. }
            | RecoveryLaunchDecision::AlreadyAccepted { .. } => {
                self.source_reconciliation_required =
                    receipt.reconstructed_from_authoritative_source();
                Ok(())
            }
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
            .field(
                "source_reconciliation_required",
                &self.source_reconciliation_required,
            )
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
            match permit.purpose() {
                tokenmaster_state::MaintenancePurpose::Manual => BackupCompression::Normal,
                tokenmaster_state::MaintenancePurpose::Periodic
                | tokenmaster_state::MaintenancePurpose::PreMigration
                | tokenmaster_state::MaintenancePurpose::PostMigration
                | tokenmaster_state::MaintenancePurpose::PreRestore
                | tokenmaster_state::MaintenancePurpose::PreDestructiveMaintenance => {
                    BackupCompression::Automatic
                }
            },
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

const fn reliable_health(
    outcome: BootstrapOutcome,
    has_corrupt_backup: bool,
    settings_health: tokenmaster_state::SettingsHealthCode,
) -> DesktopReliableStateHealth {
    if matches!(
        outcome,
        BootstrapOutcome::SafeMode | BootstrapOutcome::RecoveryRequired
    ) {
        return DesktopReliableStateHealth::RecoveryRequired;
    }
    if matches!(
        outcome,
        BootstrapOutcome::UpgradeRequired | BootstrapOutcome::Unavailable
    ) {
        return DesktopReliableStateHealth::Unavailable;
    }
    let settings_degraded = !matches!(
        settings_health,
        tokenmaster_state::SettingsHealthCode::Healthy
    ) && !(matches!(outcome, BootstrapOutcome::FirstInstall)
        && matches!(
            settings_health,
            tokenmaster_state::SettingsHealthCode::DefaultsNoValidRecord
        ));
    if has_corrupt_backup || settings_degraded {
        DesktopReliableStateHealth::Degraded
    } else {
        DesktopReliableStateHealth::Healthy
    }
}

const fn map_catalog_health(health: CatalogHealth) -> DesktopBackupHealth {
    match health {
        CatalogHealth::Corrupt => DesktopBackupHealth::Corrupt,
        CatalogHealth::HeaderValid => DesktopBackupHealth::HeaderValid,
        CatalogHealth::Verified => DesktopBackupHealth::Verified,
    }
}

const fn backup_purpose_code(purpose: BackupPurpose) -> &'static str {
    match purpose {
        BackupPurpose::Periodic => "periodic",
        BackupPurpose::Manual => "manual",
        BackupPurpose::PreMigration => "pre_migration",
        BackupPurpose::PostMigration => "post_migration",
        BackupPurpose::PreRestore => "pre_restore",
        BackupPurpose::PreDestructiveMaintenance => "pre_destructive_maintenance",
    }
}

const fn backup_compression_code(compression: BackupCompression) -> &'static str {
    match compression {
        BackupCompression::Automatic => "automatic",
        BackupCompression::Normal => "normal",
        BackupCompression::Compact => "compact",
    }
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
