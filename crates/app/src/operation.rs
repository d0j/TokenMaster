#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 12B.2b composes the operation worker before Task 15 binds UI intents"
    )
)]

use core::cell::Cell;
use core::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind, set_hook, take_hook};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, Once};
use std::thread::{Builder, JoinHandle};

use crate::command::{
    ApplicationCommand, ApplicationCommandAdmission, ApplicationCommandCompletion,
    ApplicationCommandCoordinator, ApplicationCommandExecution, ApplicationCommandFailure,
    ApplicationCommandId, ApplicationCommandPermit, ApplicationCommandRejection,
};

thread_local! {
    static REDACT_OPERATION_PANIC: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_OPERATION_PANIC_REDACTION: Once = Once::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationOperationWorkerPhase {
    Running,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationOperationWorkerSnapshot {
    phase: ApplicationOperationWorkerPhase,
    active_count: usize,
    pending_count: usize,
    latest_completion: Option<ApplicationCommandCompletion>,
}

impl ApplicationOperationWorkerSnapshot {
    #[must_use]
    pub(crate) const fn phase(self) -> ApplicationOperationWorkerPhase {
        self.phase
    }

    #[must_use]
    pub(crate) const fn active_count(self) -> usize {
        self.active_count
    }

    #[must_use]
    pub(crate) const fn pending_count(self) -> usize {
        self.pending_count
    }

    #[must_use]
    pub(crate) const fn latest_completion(self) -> Option<ApplicationCommandCompletion> {
        self.latest_completion
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplicationOperationWorkerErrorCode {
    Unavailable,
    Internal,
}

impl ApplicationOperationWorkerErrorCode {
    #[must_use]
    pub(crate) const fn stable_code(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationOperationWorkerError {
    code: ApplicationOperationWorkerErrorCode,
}

impl ApplicationOperationWorkerError {
    pub(crate) const fn unavailable() -> Self {
        Self {
            code: ApplicationOperationWorkerErrorCode::Unavailable,
        }
    }

    pub(crate) const fn internal() -> Self {
        Self {
            code: ApplicationOperationWorkerErrorCode::Internal,
        }
    }

    #[must_use]
    pub(crate) const fn code(self) -> ApplicationOperationWorkerErrorCode {
        self.code
    }
}

impl fmt::Display for ApplicationOperationWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.stable_code())
    }
}

impl std::error::Error for ApplicationOperationWorkerError {}

#[derive(Clone, Copy)]
enum WorkerWake {
    Work,
    Stop,
}

struct WorkerState {
    coordinator: ApplicationCommandCoordinator,
    pending_start: Option<ApplicationCommandPermit>,
    latest_completion: Option<ApplicationCommandCompletion>,
    phase: ApplicationOperationWorkerPhase,
}

pub(crate) struct ApplicationOperationWorker {
    state: Arc<Mutex<WorkerState>>,
    wake_sender: SyncSender<WorkerWake>,
    thread: Option<JoinHandle<()>>,
}

impl ApplicationOperationWorker {
    pub(crate) fn spawn<F>(execute: F) -> Result<Self, ApplicationOperationWorkerError>
    where
        F: FnMut(&ApplicationCommandPermit) -> ApplicationCommandExecution + Send + 'static,
    {
        Self::spawn_inner(execute, |_| {})
    }

    #[cfg(test)]
    pub(crate) fn spawn_observed<F, H>(
        execute: F,
        before_finish: H,
    ) -> Result<Self, ApplicationOperationWorkerError>
    where
        F: FnMut(&ApplicationCommandPermit) -> ApplicationCommandExecution + Send + 'static,
        H: FnMut(ApplicationCommandId) + Send + 'static,
    {
        Self::spawn_inner(execute, before_finish)
    }

    fn spawn_inner<F, H>(
        execute: F,
        before_finish: H,
    ) -> Result<Self, ApplicationOperationWorkerError>
    where
        F: FnMut(&ApplicationCommandPermit) -> ApplicationCommandExecution + Send + 'static,
        H: FnMut(ApplicationCommandId) + Send + 'static,
    {
        install_panic_redaction();
        let state = Arc::new(Mutex::new(WorkerState {
            coordinator: ApplicationCommandCoordinator::new(),
            pending_start: None,
            latest_completion: None,
            phase: ApplicationOperationWorkerPhase::Running,
        }));
        let (wake_sender, wake_receiver) = sync_channel(1);
        let worker_state = Arc::clone(&state);
        let mut execute = execute;
        let mut before_finish = before_finish;
        let thread = Builder::new()
            .name(String::from("tokenmaster-operation-worker"))
            .spawn(move || {
                REDACT_OPERATION_PANIC.with(|redact| redact.set(true));
                run_worker(
                    worker_state,
                    wake_receiver,
                    &mut execute,
                    &mut before_finish,
                );
            })
            .map_err(|_| ApplicationOperationWorkerError::unavailable())?;
        Ok(Self {
            state,
            wake_sender,
            thread: Some(thread),
        })
    }

    pub(crate) fn submit(&self, command: ApplicationCommand) -> ApplicationCommandAdmission {
        let admission = {
            let Ok(mut state) = self.state.lock() else {
                return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed);
            };
            if state.phase != ApplicationOperationWorkerPhase::Running {
                return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed);
            }
            let admission = state.coordinator.submit(command);
            if let ApplicationCommandAdmission::Started(permit) = &admission {
                state.pending_start = Some(permit.clone());
            }
            admission
        };
        if matches!(admission, ApplicationCommandAdmission::Started(_)) {
            match self.wake_sender.try_send(WorkerWake::Work) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {
                    self.fault();
                    return ApplicationCommandAdmission::Rejected(
                        ApplicationCommandRejection::Closed,
                    );
                }
            }
        }
        admission
    }

    pub(crate) fn retry_last(&self) -> ApplicationCommandAdmission {
        let admission = {
            let Ok(mut state) = self.state.lock() else {
                return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed);
            };
            if state.phase != ApplicationOperationWorkerPhase::Running {
                return ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed);
            }
            let admission = state.coordinator.retry_last();
            if let ApplicationCommandAdmission::Started(permit) = &admission {
                state.pending_start = Some(permit.clone());
            }
            admission
        };
        if matches!(admission, ApplicationCommandAdmission::Started(_)) {
            match self.wake_sender.try_send(WorkerWake::Work) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {
                    self.fault();
                    return ApplicationCommandAdmission::Rejected(
                        ApplicationCommandRejection::Closed,
                    );
                }
            }
        }
        admission
    }

    pub(crate) fn pause_admission(&self) -> Result<(), ApplicationOperationWorkerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationOperationWorkerError::internal())?;
        if state.phase != ApplicationOperationWorkerPhase::Running {
            return Err(ApplicationOperationWorkerError::internal());
        }
        state.coordinator.pause_admission();
        Ok(())
    }

    pub(crate) fn resume_admission(&self) -> Result<(), ApplicationOperationWorkerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationOperationWorkerError::internal())?;
        if state.phase != ApplicationOperationWorkerPhase::Running {
            return Err(ApplicationOperationWorkerError::internal());
        }
        state.coordinator.resume_admission();
        Ok(())
    }

    #[must_use]
    pub(crate) fn cancel(&self, request_id: ApplicationCommandId) -> bool {
        self.state
            .lock()
            .is_ok_and(|mut state| state.coordinator.cancel(request_id))
    }

    pub(crate) fn try_completion(
        &self,
    ) -> Result<Option<ApplicationCommandCompletion>, ApplicationOperationWorkerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationOperationWorkerError::internal())?;
        Ok(state.latest_completion.take())
    }

    pub(crate) fn snapshot(
        &self,
    ) -> Result<ApplicationOperationWorkerSnapshot, ApplicationOperationWorkerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ApplicationOperationWorkerError::internal())?;
        let coordinator = state.coordinator.snapshot();
        Ok(ApplicationOperationWorkerSnapshot {
            phase: state.phase,
            active_count: coordinator.active_count(),
            pending_count: coordinator.pending_count(),
            latest_completion: state.latest_completion,
        })
    }

    pub(crate) fn shutdown(
        &mut self,
    ) -> Result<ApplicationOperationWorkerPhase, ApplicationOperationWorkerError> {
        let Some(thread) = self.thread.take() else {
            return self
                .state
                .lock()
                .map(|state| state.phase)
                .map_err(|_| ApplicationOperationWorkerError::internal());
        };
        let mut internal_failure = false;
        match self.state.lock() {
            Ok(mut state) => {
                if state.phase == ApplicationOperationWorkerPhase::Running {
                    state.phase = ApplicationOperationWorkerPhase::Stopping;
                }
                state.coordinator.close();
            }
            Err(_) => internal_failure = true,
        }
        match self.wake_sender.try_send(WorkerWake::Stop) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
        if thread.join().is_err() {
            self.fault();
            return Err(ApplicationOperationWorkerError::internal());
        }
        let Ok(mut state) = self.state.lock() else {
            return Err(ApplicationOperationWorkerError::internal());
        };
        if state.phase != ApplicationOperationWorkerPhase::Faulted {
            state.phase = ApplicationOperationWorkerPhase::Stopped;
        }
        if internal_failure {
            Err(ApplicationOperationWorkerError::internal())
        } else {
            Ok(state.phase)
        }
    }

    fn fault(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.phase = ApplicationOperationWorkerPhase::Faulted;
            state.pending_start = None;
            state.coordinator.close();
        }
    }
}

impl Drop for ApplicationOperationWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_worker<F, H>(
    state: Arc<Mutex<WorkerState>>,
    wake_receiver: Receiver<WorkerWake>,
    execute: &mut F,
    before_finish: &mut H,
) where
    F: FnMut(&ApplicationCommandPermit) -> ApplicationCommandExecution,
    H: FnMut(ApplicationCommandId),
{
    loop {
        let permit = match state.lock() {
            Ok(mut state) => {
                if let Some(permit) = state.pending_start.take() {
                    Some(permit)
                } else if state.phase == ApplicationOperationWorkerPhase::Running {
                    None
                } else {
                    break;
                }
            }
            Err(_) => break,
        };
        let Some(permit) = permit else {
            match wake_receiver.recv() {
                Ok(WorkerWake::Work) => continue,
                Ok(WorkerWake::Stop) | Err(_) => break,
            }
        };

        let execution = if permit.is_cancelled() {
            Ok(ApplicationCommandExecution::Cancelled)
        } else {
            catch_unwind(AssertUnwindSafe(|| execute(&permit)))
        };
        let panicked = execution.is_err();
        let execution = execution.unwrap_or(ApplicationCommandExecution::Failed(
            ApplicationCommandFailure::Internal,
        ));
        before_finish(permit.id());

        let Ok(mut state) = state.lock() else {
            break;
        };
        // Cancellation and shutdown use this same mutex. Normalize the final
        // execution while holding it so the permit cannot change between the
        // cancellation observation and the coordinator transition.
        let execution = if permit.is_cancelled() {
            ApplicationCommandExecution::Cancelled
        } else {
            execution
        };
        let Ok(transition) = state.coordinator.finish(permit.id(), execution) else {
            state.phase = ApplicationOperationWorkerPhase::Faulted;
            state.pending_start = None;
            state.coordinator.close();
            break;
        };
        state.latest_completion = Some(transition.completion());
        if panicked {
            // A cancellation that won the mutex race remains the command
            // outcome, but an executor panic always faults the worker.
            state.phase = ApplicationOperationWorkerPhase::Faulted;
            state.pending_start = None;
            state.coordinator.close();
            break;
        }
        if state.phase == ApplicationOperationWorkerPhase::Running {
            state.pending_start = transition.follow_up().cloned();
        }
    }

    if let Ok(mut state) = state.lock() {
        if state.phase != ApplicationOperationWorkerPhase::Faulted {
            state.phase = ApplicationOperationWorkerPhase::Stopped;
        }
        state.pending_start = None;
        state.coordinator.close();
    }
}

fn install_panic_redaction() {
    INSTALL_OPERATION_PANIC_REDACTION.call_once(|| {
        let previous = take_hook();
        set_hook(Box::new(move |information| {
            let redacted = REDACT_OPERATION_PANIC.with(Cell::get);
            if !redacted {
                previous(information);
            }
        }));
    });
}
