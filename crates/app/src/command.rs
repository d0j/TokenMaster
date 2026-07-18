#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 12B command execution is composed before Task 15 binds UI intents"
    )
)]

use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const COMMAND_RUNNING: u8 = 0;
const COMMAND_CANCELLED: u8 = 1;
const COMMAND_IRREVERSIBLE: u8 = 2;

/// Path-free application intents. Native file selection is a later sealed platform step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommand {
    ExportConfig,
    ImportConfig,
    Backup,
    Verify,
    RestoreData(ApplicationBackupSelection),
    RestoreDataAndPortableSettings(ApplicationBackupSelection),
    Rebuild,
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
}

impl ApplicationCommandPermit {
    fn new(id: ApplicationCommandId, command: ApplicationCommand) -> Self {
        Self {
            id,
            command,
            state: Arc::new(AtomicU8::new(COMMAND_RUNNING)),
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
        self.state.load(Ordering::Acquire) == COMMAND_CANCELLED
    }

    #[must_use]
    fn cancel(&self) -> bool {
        self.state
            .compare_exchange(
                COMMAND_RUNNING,
                COMMAND_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
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
    }
}

impl Eq for ApplicationCommandPermit {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationCommandRejection {
    Busy,
    Closed,
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
    pub(crate) const fn pending_request_id(self) -> Option<ApplicationCommandId> {
        self.pending_request_id
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
                self.last_retryable = Some(active.permit.command());
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
