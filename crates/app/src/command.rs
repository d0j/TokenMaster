#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 12B command execution is composed before Task 15 binds UI intents"
    )
)]

use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tokenmaster_desktop::{DesktopPresentationSelection, DesktopReminderPolicyUpdate};
use tokenmaster_platform::{SelectedInputFile, SelectedOutputFile};
use tokenmaster_state::{BackupPassphrase, ReminderPolicy};

const COMMAND_RUNNING: u8 = 0;
const COMMAND_CANCELLED: u8 = 1;
const COMMAND_IRREVERSIBLE: u8 = 2;

/// Path-free application intents. Native file selection is a later sealed platform step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommand {
    ExportConfig,
    ImportConfig,
    ConfirmConfigImport,
    CancelConfigImport,
    Backup,
    BackupCompact,
    BackupEncrypted,
    Verify,
    RestoreData(ApplicationBackupSelection),
    RestoreDataAndPortableSettings(ApplicationBackupSelection),
    Rebuild,
    UpdateBackupPolicy,
    UpdateReminderPolicy,
    UpdatePresentation,
}

impl ApplicationCommand {
    const fn permits_empty_payload(self) -> bool {
        matches!(
            self,
            Self::ConfirmConfigImport
                | Self::CancelConfigImport
                | Self::Backup
                | Self::Verify
                | Self::RestoreData(_)
                | Self::RestoreDataAndPortableSettings(_)
                | Self::Rebuild
        )
    }

    const fn supports_payloadless_retry(self) -> bool {
        matches!(self, Self::Backup | Self::Verify | Self::Rebuild)
    }

    const fn is_exclusive(self) -> bool {
        matches!(
            self,
            Self::RestoreData(_) | Self::RestoreDataAndPortableSettings(_) | Self::Rebuild
        )
    }
}

pub(crate) enum ApplicationOperationPayload {
    Empty,
    ConfigOutput(SelectedOutputFile),
    ConfigInput(SelectedInputFile),
    BackupOutput(SelectedOutputFile),
    EncryptedBackupOutput {
        output: SelectedOutputFile,
        passphrase: BackupPassphrase,
    },
    BackupPolicy(ApplicationBackupPolicyUpdate),
    ReminderPolicy(ApplicationReminderPolicyUpdate),
    Presentation(ApplicationPresentationUpdate),
}

impl fmt::Debug for ApplicationOperationPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("ApplicationOperationPayload::Empty"),
            Self::ConfigOutput(_) => {
                formatter.write_str("ApplicationOperationPayload::ConfigOutput([redacted])")
            }
            Self::ConfigInput(_) => {
                formatter.write_str("ApplicationOperationPayload::ConfigInput([redacted])")
            }
            Self::BackupOutput(_) => {
                formatter.write_str("ApplicationOperationPayload::BackupOutput([redacted])")
            }
            Self::EncryptedBackupOutput { .. } => formatter
                .write_str("ApplicationOperationPayload::EncryptedBackupOutput([redacted])"),
            Self::BackupPolicy(_) => {
                formatter.write_str("ApplicationOperationPayload::BackupPolicy([redacted])")
            }
            Self::ReminderPolicy(_) => {
                formatter.write_str("ApplicationOperationPayload::ReminderPolicy([redacted])")
            }
            Self::Presentation(_) => {
                formatter.write_str("ApplicationOperationPayload::Presentation([redacted])")
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ApplicationBackupPolicyUpdate {
    periodic_enabled: bool,
    quiet_seconds: u32,
    interval_seconds: u32,
    retention_budget_mib: u32,
}

impl ApplicationBackupPolicyUpdate {
    pub(crate) const fn new(
        periodic_enabled: bool,
        quiet_seconds: u32,
        interval_seconds: u32,
        retention_budget_mib: u32,
    ) -> Self {
        Self {
            periodic_enabled,
            quiet_seconds,
            interval_seconds,
            retention_budget_mib,
        }
    }

    pub(crate) const fn periodic_enabled(self) -> bool {
        self.periodic_enabled
    }

    pub(crate) const fn quiet_seconds(self) -> u32 {
        self.quiet_seconds
    }

    pub(crate) const fn interval_seconds(self) -> u32 {
        self.interval_seconds
    }

    pub(crate) const fn retention_budget_mib(self) -> u32 {
        self.retention_budget_mib
    }
}

impl fmt::Debug for ApplicationBackupPolicyUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationBackupPolicyUpdate([redacted])")
    }
}

#[derive(Eq, PartialEq)]
pub(crate) struct ApplicationReminderPolicyUpdate {
    policy: ReminderPolicy,
}

impl ApplicationReminderPolicyUpdate {
    pub(crate) fn new(enabled: bool, lead_seconds: &[u32]) -> Option<Self> {
        ReminderPolicy::new(enabled, lead_seconds)
            .ok()
            .map(|policy| Self { policy })
    }

    pub(crate) fn from_desktop(update: DesktopReminderPolicyUpdate) -> Option<Self> {
        Self::new(update.enabled(), update.lead_seconds())
    }

    pub(crate) const fn enabled(&self) -> bool {
        self.policy.enabled()
    }

    pub(crate) fn lead_seconds(&self) -> &[u32] {
        self.policy.lead_seconds()
    }

    pub(crate) fn into_policy(self) -> ReminderPolicy {
        self.policy
    }
}

impl fmt::Debug for ApplicationReminderPolicyUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationReminderPolicyUpdate([redacted])")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ApplicationPresentationUpdate {
    selection: DesktopPresentationSelection,
}

impl ApplicationPresentationUpdate {
    pub(crate) const fn new(selection: DesktopPresentationSelection) -> Self {
        Self { selection }
    }

    pub(crate) const fn selection(self) -> DesktopPresentationSelection {
        self.selection
    }

    pub(crate) fn into_state_presentation(self) -> tokenmaster_state::PresentationSettings {
        let density = match self.selection.density() {
            tokenmaster_desktop::DesktopDensity::Comfortable => {
                tokenmaster_state::PresentationDensity::Comfortable
            }
            tokenmaster_desktop::DesktopDensity::Compact => {
                tokenmaster_state::PresentationDensity::Compact
            }
            tokenmaster_desktop::DesktopDensity::UltraCompact => {
                tokenmaster_state::PresentationDensity::UltraCompact
            }
        };
        let skin = match self.selection.skin() {
            tokenmaster_desktop::DesktopSkin::Refined => {
                tokenmaster_state::PresentationSkin::Refined
            }
            tokenmaster_desktop::DesktopSkin::Graphite => {
                tokenmaster_state::PresentationSkin::Graphite
            }
            tokenmaster_desktop::DesktopSkin::Ember => tokenmaster_state::PresentationSkin::Ember,
        };
        let color_scheme = match self.selection.color_scheme() {
            tokenmaster_desktop::DesktopColorScheme::System => {
                tokenmaster_state::PresentationColorScheme::System
            }
            tokenmaster_desktop::DesktopColorScheme::Light => {
                tokenmaster_state::PresentationColorScheme::Light
            }
            tokenmaster_desktop::DesktopColorScheme::Dark => {
                tokenmaster_state::PresentationColorScheme::Dark
            }
        };
        let layout = match self.selection.layout() {
            tokenmaster_desktop::DesktopLayout::Refined => {
                tokenmaster_state::PresentationLayout::Refined
            }
            tokenmaster_desktop::DesktopLayout::ControlCenter => {
                tokenmaster_state::PresentationLayout::ControlCenter
            }
            tokenmaster_desktop::DesktopLayout::Workbench => {
                tokenmaster_state::PresentationLayout::Workbench
            }
        };
        let rows = self.selection.board().rows().map(|row| {
            let key = match row.key() {
                tokenmaster_desktop::DesktopBoardSectionKey::PlanUsage => {
                    tokenmaster_state::BoardSectionKey::PlanUsage
                }
                tokenmaster_desktop::DesktopBoardSectionKey::CodeOutput => {
                    tokenmaster_state::BoardSectionKey::CodeOutput
                }
                tokenmaster_desktop::DesktopBoardSectionKey::Trend => {
                    tokenmaster_state::BoardSectionKey::Trend
                }
                tokenmaster_desktop::DesktopBoardSectionKey::Sessions => {
                    tokenmaster_state::BoardSectionKey::Sessions
                }
                tokenmaster_desktop::DesktopBoardSectionKey::Activity => {
                    tokenmaster_state::BoardSectionKey::Activity
                }
                tokenmaster_desktop::DesktopBoardSectionKey::Models => {
                    tokenmaster_state::BoardSectionKey::Models
                }
            };
            tokenmaster_state::BoardSectionPreference::new(key, row.visible(), row.collapsed())
        });
        let board = match tokenmaster_state::BoardPreferences::new(rows) {
            Ok(board) => board,
            Err(_) => unreachable!("Desktop board preferences are validated before mapping"),
        };
        tokenmaster_state::PresentationSettings::new(density, skin, color_scheme, layout)
            .with_board(board)
    }
}

impl fmt::Debug for ApplicationPresentationUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationPresentationUpdate([redacted])")
    }
}

pub(crate) struct ApplicationOperationRequest {
    command: ApplicationCommand,
    payload: ApplicationOperationPayload,
}

impl ApplicationOperationRequest {
    pub(crate) const fn plain(command: ApplicationCommand) -> Option<Self> {
        if command.permits_empty_payload() {
            Some(Self {
                command,
                payload: ApplicationOperationPayload::Empty,
            })
        } else {
            None
        }
    }

    pub(crate) const fn export_config(output: SelectedOutputFile) -> Self {
        Self {
            command: ApplicationCommand::ExportConfig,
            payload: ApplicationOperationPayload::ConfigOutput(output),
        }
    }

    pub(crate) const fn import_config(input: SelectedInputFile) -> Self {
        Self {
            command: ApplicationCommand::ImportConfig,
            payload: ApplicationOperationPayload::ConfigInput(input),
        }
    }

    pub(crate) const fn compact_backup(output: SelectedOutputFile) -> Self {
        Self {
            command: ApplicationCommand::BackupCompact,
            payload: ApplicationOperationPayload::BackupOutput(output),
        }
    }

    pub(crate) const fn encrypted_backup(
        output: SelectedOutputFile,
        passphrase: BackupPassphrase,
    ) -> Self {
        Self {
            command: ApplicationCommand::BackupEncrypted,
            payload: ApplicationOperationPayload::EncryptedBackupOutput { output, passphrase },
        }
    }

    pub(crate) const fn update_backup_policy(update: ApplicationBackupPolicyUpdate) -> Self {
        Self {
            command: ApplicationCommand::UpdateBackupPolicy,
            payload: ApplicationOperationPayload::BackupPolicy(update),
        }
    }

    pub(crate) const fn update_reminder_policy(update: ApplicationReminderPolicyUpdate) -> Self {
        Self {
            command: ApplicationCommand::UpdateReminderPolicy,
            payload: ApplicationOperationPayload::ReminderPolicy(update),
        }
    }

    pub(crate) const fn update_presentation(selection: DesktopPresentationSelection) -> Self {
        Self {
            command: ApplicationCommand::UpdatePresentation,
            payload: ApplicationOperationPayload::Presentation(ApplicationPresentationUpdate::new(
                selection,
            )),
        }
    }

    pub(crate) fn into_parts(self) -> (ApplicationCommand, ApplicationOperationPayload) {
        (self.command, self.payload)
    }
}

impl fmt::Debug for ApplicationOperationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ApplicationOperationRequest({:?}, [redacted])",
            self.command
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationBackupSelection {
    catalog_generation: u64,
    ordinal: u8,
}

impl ApplicationBackupSelection {
    pub(crate) fn new(catalog_generation: u64, ordinal: u8) -> Option<Self> {
        (catalog_generation != 0).then_some(Self {
            catalog_generation,
            ordinal,
        })
    }

    #[must_use]
    pub(crate) const fn catalog_generation(self) -> u64 {
        self.catalog_generation
    }

    #[must_use]
    pub(crate) const fn ordinal(self) -> u8 {
        self.ordinal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationCommandId(u64);

impl ApplicationCommandId {
    #[must_use]
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone)]
pub(crate) struct ApplicationCommandPermit {
    id: ApplicationCommandId,
    command: ApplicationCommand,
    state: Arc<AtomicU8>,
    cancelled: Arc<AtomicBool>,
}

impl ApplicationCommandPermit {
    fn new(id: ApplicationCommandId, command: ApplicationCommand) -> Self {
        Self {
            id,
            command,
            state: Arc::new(AtomicU8::new(COMMAND_RUNNING)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    pub(crate) const fn id(&self) -> ApplicationCommandId {
        self.id
    }

    #[must_use]
    pub(crate) const fn command(&self) -> ApplicationCommand {
        self.command
    }

    #[must_use]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn cancellation_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    #[must_use]
    fn cancel(&self) -> bool {
        let cancelled = self
            .state
            .compare_exchange(
                COMMAND_RUNNING,
                COMMAND_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if cancelled {
            self.cancelled.store(true, Ordering::Release);
        }
        cancelled
    }

    pub(crate) fn begin_irreversible(&self) -> Result<(), ApplicationCommandStateError> {
        self.state
            .compare_exchange(
                COMMAND_RUNNING,
                COMMAND_IRREVERSIBLE,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|_| ApplicationCommandStateError)
    }
}

impl fmt::Debug for ApplicationCommandPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationCommandPermit")
            .field("id", &self.id)
            .field("command", &self.command)
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl PartialEq for ApplicationCommandPermit {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.command == other.command
            && Arc::ptr_eq(&self.state, &other.state)
            && Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

impl Eq for ApplicationCommandPermit {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommandRejection {
    Busy,
    Closed,
    PayloadRequired,
    NoRetryAvailable,
    CapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommandAdmission {
    Started(ApplicationCommandPermit),
    Queued {
        request_id: ApplicationCommandId,
        active_request_id: ApplicationCommandId,
    },
    Coalesced {
        request_id: ApplicationCommandId,
        active_request_id: ApplicationCommandId,
    },
    Rejected(ApplicationCommandRejection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommandFailure {
    Unavailable,
    InvalidSelection,
    Integrity,
    CapacityExceeded,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommandExecution {
    Succeeded,
    Failed(ApplicationCommandFailure),
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommandOutcome {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationCommandCompletion {
    request_id: ApplicationCommandId,
    command: ApplicationCommand,
    outcome: ApplicationCommandOutcome,
    failure: Option<ApplicationCommandFailure>,
}

impl ApplicationCommandCompletion {
    #[must_use]
    pub(crate) const fn request_id(self) -> ApplicationCommandId {
        self.request_id
    }

    #[must_use]
    pub(crate) const fn command(self) -> ApplicationCommand {
        self.command
    }

    #[must_use]
    pub(crate) const fn outcome(self) -> ApplicationCommandOutcome {
        self.outcome
    }

    #[must_use]
    pub(crate) const fn failure(self) -> Option<ApplicationCommandFailure> {
        self.failure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationCommandTransition {
    completion: ApplicationCommandCompletion,
    follow_up: Option<ApplicationCommandPermit>,
}

impl ApplicationCommandTransition {
    #[must_use]
    pub(crate) const fn completion(&self) -> ApplicationCommandCompletion {
        self.completion
    }

    #[must_use]
    pub(crate) const fn follow_up(&self) -> Option<&ApplicationCommandPermit> {
        self.follow_up.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationCommandCoordinatorSnapshot {
    active_request_id: Option<ApplicationCommandId>,
    active_command: Option<ApplicationCommand>,
    pending_request_id: Option<ApplicationCommandId>,
    pending_command: Option<ApplicationCommand>,
    admission_paused: bool,
    closed: bool,
}

impl ApplicationCommandCoordinatorSnapshot {
    #[must_use]
    pub(crate) const fn active_count(self) -> usize {
        if self.active_request_id.is_some() {
            1
        } else {
            0
        }
    }

    #[must_use]
    pub(crate) const fn pending_count(self) -> usize {
        if self.pending_request_id.is_some() {
            1
        } else {
            0
        }
    }

    #[must_use]
    pub(crate) const fn active_request_id(self) -> Option<ApplicationCommandId> {
        self.active_request_id
    }

    #[must_use]
    pub(crate) const fn active_command(self) -> Option<ApplicationCommand> {
        self.active_command
    }

    #[must_use]
    pub(crate) const fn pending_command(self) -> Option<ApplicationCommand> {
        self.pending_command
    }

    #[must_use]
    pub(crate) const fn is_closed(self) -> bool {
        self.closed
    }

    #[must_use]
    pub(crate) const fn admission_paused(self) -> bool {
        self.admission_paused
    }
}

#[derive(Clone, Copy)]
struct PendingCommand {
    id: ApplicationCommandId,
    command: ApplicationCommand,
}

struct ActiveCommand {
    permit: ApplicationCommandPermit,
    pending: Option<PendingCommand>,
}

pub(crate) struct ApplicationCommandCoordinator {
    next_request_id: Option<u64>,
    active: Option<ActiveCommand>,
    last_retryable: Option<ApplicationCommand>,
    admission_paused: bool,
    closed: bool,
}

impl ApplicationCommandCoordinator {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            next_request_id: Some(1),
            active: None,
            last_retryable: None,
            admission_paused: false,
            closed: false,
        }
    }

    pub(crate) fn submit(&mut self, command: ApplicationCommand) -> ApplicationCommandAdmission {
        if self.closed || self.admission_paused {
            return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed);
        }
        if let Some(active) = self.active.as_ref() {
            if active.permit.command().is_exclusive() {
                return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Busy);
            }
            if active.permit.command() == command {
                return ApplicationCommandAdmission::Coalesced {
                    request_id: active.permit.id(),
                    active_request_id: active.permit.id(),
                };
            }
            if let Some(pending) = active.pending {
                return if pending.command == command {
                    ApplicationCommandAdmission::Coalesced {
                        request_id: pending.id,
                        active_request_id: active.permit.id(),
                    }
                } else {
                    ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Busy)
                };
            }
        }

        let Some(id) = self.allocate_request_id() else {
            return ApplicationCommandAdmission::Rejected(
                ApplicationCommandRejection::CapacityExceeded,
            );
        };
        if let Some(active) = self.active.as_mut() {
            let active_request_id = active.permit.id();
            active.pending = Some(PendingCommand { id, command });
            ApplicationCommandAdmission::Queued {
                request_id: id,
                active_request_id,
            }
        } else {
            let permit = ApplicationCommandPermit::new(id, command);
            self.active = Some(ActiveCommand {
                permit: permit.clone(),
                pending: None,
            });
            ApplicationCommandAdmission::Started(permit)
        }
    }

    pub(crate) fn submit_replaceable(
        &mut self,
        command: ApplicationCommand,
    ) -> ApplicationCommandAdmission {
        if self.closed || self.admission_paused {
            return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed);
        }
        if let Some(active) = self.active.as_ref() {
            if active.permit.command().is_exclusive() {
                return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Busy);
            }
            if active.permit.command() == command {
                if let Some(pending) = active.pending {
                    return if pending.command == command {
                        ApplicationCommandAdmission::Coalesced {
                            request_id: pending.id,
                            active_request_id: active.permit.id(),
                        }
                    } else {
                        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Busy)
                    };
                }
                let Some(id) = self.allocate_request_id() else {
                    return ApplicationCommandAdmission::Rejected(
                        ApplicationCommandRejection::CapacityExceeded,
                    );
                };
                let Some(active) = self.active.as_mut() else {
                    return ApplicationCommandAdmission::Rejected(
                        ApplicationCommandRejection::Closed,
                    );
                };
                let active_request_id = active.permit.id();
                active.pending = Some(PendingCommand { id, command });
                return ApplicationCommandAdmission::Queued {
                    request_id: id,
                    active_request_id,
                };
            }
        }
        self.submit(command)
    }

    pub(crate) fn retry_last(&mut self) -> ApplicationCommandAdmission {
        let Some(command) = self.last_retryable else {
            return ApplicationCommandAdmission::Rejected(
                ApplicationCommandRejection::NoRetryAvailable,
            );
        };
        self.submit(command)
    }

    #[must_use]
    pub(crate) fn cancel(&mut self, request_id: ApplicationCommandId) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        if active.permit.id() == request_id {
            return active.permit.cancel();
        }
        if active
            .pending
            .is_some_and(|pending| pending.id == request_id)
        {
            active.pending = None;
            return true;
        }
        false
    }

    pub(crate) fn finish(
        &mut self,
        request_id: ApplicationCommandId,
        execution: ApplicationCommandExecution,
    ) -> Result<ApplicationCommandTransition, ApplicationCommandStateError> {
        let Some(active) = self.active.take() else {
            return Err(ApplicationCommandStateError);
        };
        let reports_cancelled = execution == ApplicationCommandExecution::Cancelled;
        if active.permit.id() != request_id || active.permit.is_cancelled() != reports_cancelled {
            self.active = Some(active);
            return Err(ApplicationCommandStateError);
        }
        let (outcome, failure) = match execution {
            ApplicationCommandExecution::Succeeded => (ApplicationCommandOutcome::Succeeded, None),
            ApplicationCommandExecution::Failed(failure) => {
                self.last_retryable = active
                    .permit
                    .command()
                    .supports_payloadless_retry()
                    .then_some(active.permit.command());
                (ApplicationCommandOutcome::Failed, Some(failure))
            }
            ApplicationCommandExecution::Cancelled => (ApplicationCommandOutcome::Cancelled, None),
        };
        let completion = ApplicationCommandCompletion {
            request_id,
            command: active.permit.command(),
            outcome,
            failure,
        };
        let follow_up = active
            .pending
            .map(|pending| ApplicationCommandPermit::new(pending.id, pending.command));
        self.active = follow_up.clone().map(|permit| ActiveCommand {
            permit,
            pending: None,
        });
        Ok(ApplicationCommandTransition {
            completion,
            follow_up,
        })
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.admission_paused = true;
        if let Some(active) = self.active.as_mut() {
            active.pending = None;
            let _ = active.permit.cancel();
        }
    }

    pub(crate) fn pause_admission(&mut self) {
        self.admission_paused = true;
        if let Some(active) = self.active.as_mut() {
            active.pending = None;
        }
    }

    pub(crate) fn resume_admission(&mut self) {
        if !self.closed {
            self.admission_paused = false;
        }
    }

    #[must_use]
    pub(crate) fn snapshot(&self) -> ApplicationCommandCoordinatorSnapshot {
        let (active_request_id, active_command, pending_request_id, pending_command) = self
            .active
            .as_ref()
            .map_or((None, None, None, None), |active| {
                (
                    Some(active.permit.id()),
                    Some(active.permit.command()),
                    active.pending.map(|pending| pending.id),
                    active.pending.map(|pending| pending.command),
                )
            });
        ApplicationCommandCoordinatorSnapshot {
            active_request_id,
            active_command,
            pending_request_id,
            pending_command,
            admission_paused: self.admission_paused,
            closed: self.closed,
        }
    }

    fn allocate_request_id(&mut self) -> Option<ApplicationCommandId> {
        let id = self.next_request_id?;
        self.next_request_id = id.checked_add(1);
        Some(ApplicationCommandId(id))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationCommandStateError;
