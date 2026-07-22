use core::fmt;
use std::cell::RefCell;
use std::rc::Rc;

use crate::presentation_style::{DesktopColorScheme, DesktopDensity, DesktopPresentationSelection};
use crate::skin::DesktopSkin;

pub const MAX_DESKTOP_RESTORE_POINTS: usize = 15;
pub const MIN_BACKUP_PASSPHRASE_SCALARS: usize = 12;
pub const MAX_BACKUP_PASSPHRASE_SCALARS: usize = 128;
pub const MAX_DESKTOP_REMINDER_LEADS: usize = 8;
pub const MIN_DESKTOP_REMINDER_LEAD_SECONDS: u32 = 60;
pub const MAX_DESKTOP_REMINDER_LEAD_SECONDS: u32 = 31_536_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopReliableStateHealth {
    Healthy,
    Degraded,
    RecoveryRequired,
    Unavailable,
}

impl DesktopReliableStateHealth {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::RecoveryRequired => "recovery_required",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopRecoveryKind {
    VerifiedBackup,
    AuthoritativeSource,
}

impl DesktopRecoveryKind {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::VerifiedBackup => "verified_backup",
            Self::AuthoritativeSource => "authoritative_source",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopRecoveryReceipt {
    kind: DesktopRecoveryKind,
    non_reconstructible_domains_lost: bool,
}

impl DesktopRecoveryReceipt {
    #[must_use]
    pub const fn restored_from_verified_backup() -> Self {
        Self {
            kind: DesktopRecoveryKind::VerifiedBackup,
            non_reconstructible_domains_lost: false,
        }
    }

    #[must_use]
    pub const fn reconstructed_from_authoritative_source() -> Self {
        Self {
            kind: DesktopRecoveryKind::AuthoritativeSource,
            non_reconstructible_domains_lost: true,
        }
    }

    #[must_use]
    pub const fn kind(self) -> DesktopRecoveryKind {
        self.kind
    }

    #[must_use]
    pub const fn non_reconstructible_domains_lost(self) -> bool {
        self.non_reconstructible_domains_lost
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopBackupHealth {
    Corrupt,
    HeaderValid,
    Verified,
}

impl DesktopBackupHealth {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Corrupt => "corrupt",
            Self::HeaderValid => "header_valid",
            Self::Verified => "verified",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopBackupPolicy {
    periodic_enabled: bool,
    quiet_seconds: u32,
    interval_seconds: u32,
    retention_budget_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopPresentationSettings {
    density: DesktopDensity,
    skin: DesktopSkin,
    color_scheme: DesktopColorScheme,
    layout: crate::DesktopLayout,
}

impl DesktopPresentationSettings {
    #[must_use]
    pub const fn new(
        density: DesktopDensity,
        skin: DesktopSkin,
        color_scheme: DesktopColorScheme,
        layout: crate::DesktopLayout,
    ) -> Self {
        Self {
            density,
            skin,
            color_scheme,
            layout,
        }
    }

    #[must_use]
    pub const fn comfortable() -> Self {
        Self::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            crate::DesktopLayout::Refined,
        )
    }

    #[must_use]
    pub const fn density(self) -> DesktopDensity {
        self.density
    }

    #[must_use]
    pub const fn skin(self) -> DesktopSkin {
        self.skin
    }

    #[must_use]
    pub const fn color_scheme(self) -> DesktopColorScheme {
        self.color_scheme
    }

    #[must_use]
    pub const fn layout(self) -> crate::DesktopLayout {
        self.layout
    }

    #[must_use]
    pub const fn selection(self) -> DesktopPresentationSelection {
        DesktopPresentationSelection::new(self.density, self.skin, self.color_scheme, self.layout)
    }
}

impl DesktopBackupPolicy {
    #[must_use]
    pub const fn new(
        periodic_enabled: bool,
        quiet_seconds: u32,
        interval_seconds: u32,
        retention_budget_bytes: u64,
    ) -> Self {
        Self {
            periodic_enabled,
            quiet_seconds,
            interval_seconds,
            retention_budget_bytes,
        }
    }

    #[must_use]
    pub const fn disabled() -> Self {
        Self::new(false, 0, 0, 0)
    }

    #[must_use]
    pub const fn periodic_enabled(self) -> bool {
        self.periodic_enabled
    }

    #[must_use]
    pub const fn quiet_seconds(self) -> u32 {
        self.quiet_seconds
    }

    #[must_use]
    pub const fn interval_seconds(self) -> u32 {
        self.interval_seconds
    }

    #[must_use]
    pub const fn retention_budget_bytes(self) -> u64 {
        self.retention_budget_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopReminderSyncState {
    Pending,
    Synchronized,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopReminderPolicy {
    enabled: bool,
    lead_seconds: [u32; MAX_DESKTOP_REMINDER_LEADS],
    lead_count: u8,
    sync_state: DesktopReminderSyncState,
}

impl DesktopReminderPolicy {
    #[must_use]
    pub fn new(
        enabled: bool,
        lead_seconds: &[u32],
        sync_state: DesktopReminderSyncState,
    ) -> Option<Self> {
        if lead_seconds.len() > MAX_DESKTOP_REMINDER_LEADS
            || (enabled && lead_seconds.is_empty())
            || (!enabled && !lead_seconds.is_empty())
            || (matches!(sync_state, DesktopReminderSyncState::Unavailable)
                && (enabled || !lead_seconds.is_empty()))
        {
            return None;
        }

        let mut normalized = [0; MAX_DESKTOP_REMINDER_LEADS];
        for (index, lead) in lead_seconds.iter().copied().enumerate() {
            if !(MIN_DESKTOP_REMINDER_LEAD_SECONDS..=MAX_DESKTOP_REMINDER_LEAD_SECONDS)
                .contains(&lead)
                || normalized[..index].contains(&lead)
            {
                return None;
            }
            normalized[index] = lead;
        }
        normalized[..lead_seconds.len()].sort_unstable_by(|left, right| right.cmp(left));

        Some(Self {
            enabled,
            lead_seconds: normalized,
            lead_count: lead_seconds.len() as u8,
            sync_state,
        })
    }

    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            enabled: false,
            lead_seconds: [0; MAX_DESKTOP_REMINDER_LEADS],
            lead_count: 0,
            sync_state: DesktopReminderSyncState::Unavailable,
        }
    }

    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn lead_seconds(&self) -> &[u32] {
        &self.lead_seconds[..usize::from(self.lead_count)]
    }

    #[must_use]
    pub const fn sync_state(self) -> DesktopReminderSyncState {
        self.sync_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopOperationKind {
    ExportConfig,
    ImportConfig,
    Backup,
    Verify,
    Restore,
    RestoreWithPortableSettings,
    Rebuild,
    UpdatePolicy,
    ApplyConfig,
    UpdatePresentation,
}

impl DesktopOperationKind {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::ExportConfig => "export_config",
            Self::ImportConfig => "import_config",
            Self::Backup => "backup",
            Self::Verify => "verify",
            Self::Restore => "restore",
            Self::RestoreWithPortableSettings => "restore_with_portable_settings",
            Self::Rebuild => "rebuild",
            Self::UpdatePolicy => "update_policy",
            Self::ApplyConfig => "apply_config",
            Self::UpdatePresentation => "update_presentation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopOperationPhase {
    Queued,
    Running,
    AtomicPromotion,
    Succeeded,
    Failed,
    Cancelled,
}

impl DesktopOperationPhase {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::AtomicPromotion => "atomic_promotion",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopOperationSnapshot {
    kind: DesktopOperationKind,
    phase: DesktopOperationPhase,
    cancellable: bool,
    failure_code: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopConfigImportPreview {
    created_at_utc_ms: i64,
    package_bytes: u64,
    changed_category_count: u8,
    changed_field_count: u16,
}

impl DesktopConfigImportPreview {
    #[must_use]
    pub const fn new(
        created_at_utc_ms: i64,
        package_bytes: u64,
        changed_category_count: u8,
        changed_field_count: u16,
    ) -> Self {
        Self {
            created_at_utc_ms,
            package_bytes,
            changed_category_count,
            changed_field_count,
        }
    }

    #[must_use]
    pub const fn created_at_utc_ms(self) -> i64 {
        self.created_at_utc_ms
    }

    #[must_use]
    pub const fn package_bytes(self) -> u64 {
        self.package_bytes
    }

    #[must_use]
    pub const fn changed_category_count(self) -> u8 {
        self.changed_category_count
    }

    #[must_use]
    pub const fn changed_field_count(self) -> u16 {
        self.changed_field_count
    }
}

impl DesktopOperationSnapshot {
    #[must_use]
    pub const fn new(
        kind: DesktopOperationKind,
        phase: DesktopOperationPhase,
        cancellable: bool,
        failure_code: Option<&'static str>,
    ) -> Self {
        Self {
            kind,
            phase,
            cancellable: cancellable && !matches!(phase, DesktopOperationPhase::AtomicPromotion),
            failure_code,
        }
    }

    #[must_use]
    pub const fn kind(self) -> DesktopOperationKind {
        self.kind
    }

    #[must_use]
    pub const fn phase(self) -> DesktopOperationPhase {
        self.phase
    }

    #[must_use]
    pub const fn cancellable(self) -> bool {
        self.cancellable
    }

    #[must_use]
    pub const fn failure_code(self) -> Option<&'static str> {
        self.failure_code
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopRestoreSelection {
    catalog_generation: u64,
    ordinal: u8,
}

impl DesktopRestoreSelection {
    #[must_use]
    pub const fn new(catalog_generation: u64, ordinal: u8) -> Option<Self> {
        if catalog_generation == 0 {
            None
        } else {
            Some(Self {
                catalog_generation,
                ordinal,
            })
        }
    }

    #[must_use]
    pub const fn catalog_generation(self) -> u64 {
        self.catalog_generation
    }

    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopRestorePointInput {
    selection: DesktopRestoreSelection,
    created_at_utc_ms: Option<i64>,
    size_bytes: u64,
    health: DesktopBackupHealth,
    purpose_code: &'static str,
    database_schema_version: Option<u16>,
    compression_code: &'static str,
}

impl DesktopRestorePointInput {
    #[must_use]
    pub const fn new(
        selection: DesktopRestoreSelection,
        created_at_utc_ms: Option<i64>,
        size_bytes: u64,
        health: DesktopBackupHealth,
        purpose_code: &'static str,
        database_schema_version: Option<u16>,
        compression_code: &'static str,
    ) -> Self {
        Self {
            selection,
            created_at_utc_ms,
            size_bytes,
            health,
            purpose_code,
            database_schema_version,
            compression_code,
        }
    }

    #[must_use]
    pub const fn selection(&self) -> DesktopRestoreSelection {
        self.selection
    }

    #[must_use]
    pub const fn created_at_utc_ms(&self) -> Option<i64> {
        self.created_at_utc_ms
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    #[must_use]
    pub const fn health(&self) -> DesktopBackupHealth {
        self.health
    }

    #[must_use]
    pub const fn purpose_code(&self) -> &'static str {
        self.purpose_code
    }

    #[must_use]
    pub const fn database_schema_version(&self) -> Option<u16> {
        self.database_schema_version
    }

    #[must_use]
    pub const fn compression_code(&self) -> &'static str {
        self.compression_code
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopReliableStateSummary {
    health: DesktopReliableStateHealth,
    safe_mode: bool,
    settings_health_code: &'static str,
    policy: DesktopBackupPolicy,
    reminder_policy: DesktopReminderPolicy,
    presentation: DesktopPresentationSettings,
    latest_success_at_utc_ms: Option<i64>,
    latest_attempt_at_utc_ms: Option<i64>,
    successful_count: Option<u64>,
    failure_count: Option<u64>,
    published_bytes: Option<u64>,
    latest_failure_code: Option<&'static str>,
    recovery_receipt: Option<DesktopRecoveryReceipt>,
    operation: Option<DesktopOperationSnapshot>,
    config_import_preview: Option<DesktopConfigImportPreview>,
}

impl DesktopReliableStateSummary {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        health: DesktopReliableStateHealth,
        safe_mode: bool,
        settings_health_code: &'static str,
        policy: DesktopBackupPolicy,
        latest_success_at_utc_ms: Option<i64>,
        latest_attempt_at_utc_ms: Option<i64>,
        successful_count: Option<u64>,
        failure_count: Option<u64>,
        published_bytes: Option<u64>,
        latest_failure_code: Option<&'static str>,
        recovery_receipt: Option<DesktopRecoveryReceipt>,
        operation: Option<DesktopOperationSnapshot>,
        config_import_preview: Option<DesktopConfigImportPreview>,
    ) -> Self {
        Self::new_with_reminder_policy(
            health,
            safe_mode,
            settings_health_code,
            policy,
            DesktopReminderPolicy::unavailable(),
            latest_success_at_utc_ms,
            latest_attempt_at_utc_ms,
            successful_count,
            failure_count,
            published_bytes,
            latest_failure_code,
            recovery_receipt,
            operation,
            config_import_preview,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new_with_reminder_policy(
        health: DesktopReliableStateHealth,
        safe_mode: bool,
        settings_health_code: &'static str,
        policy: DesktopBackupPolicy,
        reminder_policy: DesktopReminderPolicy,
        latest_success_at_utc_ms: Option<i64>,
        latest_attempt_at_utc_ms: Option<i64>,
        successful_count: Option<u64>,
        failure_count: Option<u64>,
        published_bytes: Option<u64>,
        latest_failure_code: Option<&'static str>,
        recovery_receipt: Option<DesktopRecoveryReceipt>,
        operation: Option<DesktopOperationSnapshot>,
        config_import_preview: Option<DesktopConfigImportPreview>,
    ) -> Self {
        Self::new_with_settings(
            health,
            safe_mode,
            settings_health_code,
            policy,
            reminder_policy,
            DesktopPresentationSettings::comfortable(),
            latest_success_at_utc_ms,
            latest_attempt_at_utc_ms,
            successful_count,
            failure_count,
            published_bytes,
            latest_failure_code,
            recovery_receipt,
            operation,
            config_import_preview,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new_with_settings(
        health: DesktopReliableStateHealth,
        safe_mode: bool,
        settings_health_code: &'static str,
        policy: DesktopBackupPolicy,
        reminder_policy: DesktopReminderPolicy,
        presentation: DesktopPresentationSettings,
        latest_success_at_utc_ms: Option<i64>,
        latest_attempt_at_utc_ms: Option<i64>,
        successful_count: Option<u64>,
        failure_count: Option<u64>,
        published_bytes: Option<u64>,
        latest_failure_code: Option<&'static str>,
        recovery_receipt: Option<DesktopRecoveryReceipt>,
        operation: Option<DesktopOperationSnapshot>,
        config_import_preview: Option<DesktopConfigImportPreview>,
    ) -> Self {
        Self {
            health,
            safe_mode,
            settings_health_code,
            policy,
            reminder_policy,
            presentation,
            latest_success_at_utc_ms,
            latest_attempt_at_utc_ms,
            successful_count,
            failure_count,
            published_bytes,
            latest_failure_code,
            recovery_receipt,
            operation,
            config_import_preview,
        }
    }
}

pub struct DesktopReliableStateInput {
    generation: u64,
    summary: DesktopReliableStateSummary,
    restore_points: Vec<DesktopRestorePointInput>,
}

impl DesktopReliableStateInput {
    #[must_use]
    pub fn new(
        generation: u64,
        summary: DesktopReliableStateSummary,
        restore_points: Vec<DesktopRestorePointInput>,
    ) -> Self {
        Self {
            generation,
            summary,
            restore_points,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct DesktopReliableStateProjection {
    generation: u64,
    summary: DesktopReliableStateSummary,
    restore_points: Vec<DesktopRestorePointInput>,
}

impl DesktopReliableStateProjection {
    #[must_use]
    pub fn from_input(input: DesktopReliableStateInput) -> Self {
        Self {
            generation: input.generation,
            summary: input.summary,
            restore_points: input
                .restore_points
                .into_iter()
                .take(MAX_DESKTOP_RESTORE_POINTS)
                .collect(),
        }
    }

    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            generation: 0,
            summary: DesktopReliableStateSummary::new(
                DesktopReliableStateHealth::Unavailable,
                false,
                "unavailable",
                DesktopBackupPolicy::disabled(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            restore_points: Vec::new(),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn health(&self) -> DesktopReliableStateHealth {
        self.summary.health
    }

    #[must_use]
    pub const fn safe_mode(&self) -> bool {
        self.summary.safe_mode
    }

    #[must_use]
    pub const fn settings_health_code(&self) -> &'static str {
        self.summary.settings_health_code
    }

    #[must_use]
    pub const fn policy(&self) -> DesktopBackupPolicy {
        self.summary.policy
    }

    #[must_use]
    pub const fn reminder_policy(&self) -> DesktopReminderPolicy {
        self.summary.reminder_policy
    }

    #[must_use]
    pub const fn presentation(&self) -> DesktopPresentationSettings {
        self.summary.presentation
    }

    #[must_use]
    pub const fn latest_success_at_utc_ms(&self) -> Option<i64> {
        self.summary.latest_success_at_utc_ms
    }

    #[must_use]
    pub const fn latest_attempt_at_utc_ms(&self) -> Option<i64> {
        self.summary.latest_attempt_at_utc_ms
    }

    #[must_use]
    pub const fn successful_count(&self) -> Option<u64> {
        self.summary.successful_count
    }

    #[must_use]
    pub const fn failure_count(&self) -> Option<u64> {
        self.summary.failure_count
    }

    #[must_use]
    pub const fn published_bytes(&self) -> Option<u64> {
        self.summary.published_bytes
    }

    #[must_use]
    pub const fn latest_failure_code(&self) -> Option<&'static str> {
        self.summary.latest_failure_code
    }

    #[must_use]
    pub const fn recovery_receipt(&self) -> Option<DesktopRecoveryReceipt> {
        self.summary.recovery_receipt
    }

    #[must_use]
    pub const fn operation(&self) -> Option<DesktopOperationSnapshot> {
        self.summary.operation
    }

    #[must_use]
    pub const fn config_import_preview(&self) -> Option<DesktopConfigImportPreview> {
        self.summary.config_import_preview
    }

    #[must_use]
    pub fn restore_points(&self) -> &[DesktopRestorePointInput] {
        &self.restore_points
    }

    #[must_use]
    pub fn restore_selection(&self, row: usize) -> Option<DesktopRestoreSelection> {
        self.restore_points
            .get(row)
            .map(DesktopRestorePointInput::selection)
    }

    #[must_use]
    pub fn with_operation(mut self, operation: Option<DesktopOperationSnapshot>) -> Self {
        self.set_operation(operation);
        self
    }

    pub fn set_operation(&mut self, operation: Option<DesktopOperationSnapshot>) {
        self.summary.operation = operation;
    }

    pub fn set_reminder_policy(&mut self, reminder_policy: DesktopReminderPolicy) {
        self.summary.reminder_policy = reminder_policy;
    }

    #[must_use]
    pub fn with_reminder_policy(mut self, reminder_policy: DesktopReminderPolicy) -> Self {
        self.set_reminder_policy(reminder_policy);
        self
    }
}

impl fmt::Debug for DesktopReliableStateProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DesktopReliableStateProjection([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopIntentAdmission {
    Started,
    Queued,
    Coalesced,
    Rejected,
}

#[derive(Eq, PartialEq)]
pub enum DesktopIntent {
    ExportConfig,
    ImportConfig,
    ConfirmConfigImport,
    CancelConfigImport,
    BackupNormal,
    BackupCompact,
    BackupEncrypted {
        passphrase: DesktopPassphrase,
    },
    VerifyBackups,
    PreviewRestore(DesktopRestoreSelection),
    ConfirmRestore {
        selection: DesktopRestoreSelection,
        portable_settings: bool,
    },
    RebuildData,
    RetryOperation,
    CancelOperation,
    UpdateBackupPolicy {
        periodic_enabled: bool,
        quiet_seconds: u32,
        interval_seconds: u32,
        retention_budget_mib: u32,
    },
    UpdateReminderPolicy(DesktopReminderPolicyUpdate),
    UpdatePresentation(DesktopPresentationSelection),
    EnableCurrentUserStartup,
    RepairCurrentUserStartup,
    DisableCurrentUserStartup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopCurrentUserStartupStatus {
    Disabled,
    EnabledVerified,
    StaleRelocation,
    Conflict,
    AccessDenied,
    Unavailable,
}

impl DesktopCurrentUserStartupStatus {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::EnabledVerified => "enabled_verified",
            Self::StaleRelocation => "stale_relocation",
            Self::Conflict => "conflict",
            Self::AccessDenied => "access_denied",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Eq, PartialEq)]
pub struct DesktopReminderPolicyUpdate {
    enabled: bool,
    lead_seconds: Box<[u32]>,
}

impl DesktopReminderPolicyUpdate {
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn lead_seconds(&self) -> &[u32] {
        &self.lead_seconds
    }
}

impl DesktopIntent {
    pub fn encrypted_backup(
        passphrase: &str,
        confirmation: &str,
    ) -> Result<Self, DesktopIntentValidationError> {
        let scalar_count = passphrase.chars().count();
        if !(MIN_BACKUP_PASSPHRASE_SCALARS..=MAX_BACKUP_PASSPHRASE_SCALARS).contains(&scalar_count)
            || passphrase != confirmation
        {
            return Err(DesktopIntentValidationError);
        }
        Ok(Self::BackupEncrypted {
            passphrase: DesktopPassphrase(passphrase.to_owned()),
        })
    }

    pub fn update_reminder_policy(
        enabled: bool,
        lead_seconds: &[u32],
    ) -> Result<Self, DesktopIntentValidationError> {
        let policy =
            DesktopReminderPolicy::new(enabled, lead_seconds, DesktopReminderSyncState::Pending)
                .ok_or(DesktopIntentValidationError)?;

        Ok(Self::UpdateReminderPolicy(DesktopReminderPolicyUpdate {
            enabled,
            lead_seconds: policy.lead_seconds().into(),
        }))
    }
}

#[derive(Eq, PartialEq)]
pub struct DesktopPassphrase(String);

impl DesktopPassphrase {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(mut self) -> String {
        core::mem::take(&mut self.0)
    }
}

impl fmt::Debug for DesktopPassphrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DesktopPassphrase([redacted])")
    }
}

impl Drop for DesktopPassphrase {
    fn drop(&mut self) {
        let mut bytes = core::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}

impl fmt::Debug for DesktopIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExportConfig => formatter.write_str("DesktopIntent::ExportConfig"),
            Self::ImportConfig => formatter.write_str("DesktopIntent::ImportConfig"),
            Self::ConfirmConfigImport => formatter.write_str("DesktopIntent::ConfirmConfigImport"),
            Self::CancelConfigImport => formatter.write_str("DesktopIntent::CancelConfigImport"),
            Self::BackupNormal => formatter.write_str("DesktopIntent::BackupNormal"),
            Self::BackupCompact => formatter.write_str("DesktopIntent::BackupCompact"),
            Self::BackupEncrypted { .. } => {
                formatter.write_str("DesktopIntent::BackupEncrypted([redacted])")
            }
            Self::VerifyBackups => formatter.write_str("DesktopIntent::VerifyBackups"),
            Self::PreviewRestore(selection) => formatter
                .debug_tuple("DesktopIntent::PreviewRestore")
                .field(selection)
                .finish(),
            Self::ConfirmRestore {
                selection,
                portable_settings,
            } => formatter
                .debug_struct("DesktopIntent::ConfirmRestore")
                .field("selection", selection)
                .field("portable_settings", portable_settings)
                .finish(),
            Self::RebuildData => formatter.write_str("DesktopIntent::RebuildData"),
            Self::RetryOperation => formatter.write_str("DesktopIntent::RetryOperation"),
            Self::CancelOperation => formatter.write_str("DesktopIntent::CancelOperation"),
            Self::UpdateBackupPolicy {
                periodic_enabled,
                quiet_seconds,
                interval_seconds,
                retention_budget_mib,
            } => formatter
                .debug_struct("DesktopIntent::UpdateBackupPolicy")
                .field("periodic_enabled", periodic_enabled)
                .field("quiet_seconds", quiet_seconds)
                .field("interval_seconds", interval_seconds)
                .field("retention_budget_mib", retention_budget_mib)
                .finish(),
            Self::UpdateReminderPolicy(_) => {
                formatter.write_str("DesktopIntent::UpdateReminderPolicy([redacted])")
            }
            Self::UpdatePresentation(selection) => formatter
                .debug_tuple("DesktopIntent::UpdatePresentation")
                .field(selection)
                .finish(),
            Self::EnableCurrentUserStartup => {
                formatter.write_str("DesktopIntent::EnableCurrentUserStartup")
            }
            Self::RepairCurrentUserStartup => {
                formatter.write_str("DesktopIntent::RepairCurrentUserStartup")
            }
            Self::DisableCurrentUserStartup => {
                formatter.write_str("DesktopIntent::DisableCurrentUserStartup")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopIntentValidationError;

pub trait DesktopIntentSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission;
}

#[derive(Default)]
pub struct DesktopIntentRouter {
    sink: RefCell<Option<Rc<dyn DesktopIntentSink>>>,
}

impl DesktopIntentRouter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sink: RefCell::new(None),
        }
    }

    pub fn install(&self, sink: Rc<dyn DesktopIntentSink>) -> Result<(), DesktopIntentRouterError> {
        let mut slot = self
            .sink
            .try_borrow_mut()
            .map_err(|_| DesktopIntentRouterError)?;
        if slot.is_some() {
            return Err(DesktopIntentRouterError);
        }
        *slot = Some(sink);
        Ok(())
    }
}

impl DesktopIntentSink for DesktopIntentRouter {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        let Ok(slot) = self.sink.try_borrow() else {
            return DesktopIntentAdmission::Rejected;
        };
        slot.as_ref()
            .map_or(DesktopIntentAdmission::Rejected, |sink| sink.submit(intent))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopIntentRouterError;

pub(crate) struct UnavailableDesktopIntentSink;

impl DesktopIntentSink for UnavailableDesktopIntentSink {
    fn submit(&self, _intent: DesktopIntent) -> DesktopIntentAdmission {
        DesktopIntentAdmission::Rejected
    }
}
