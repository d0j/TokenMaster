use core::cell::Cell;
use core::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind, set_hook, take_hook};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, Once};
use std::thread::{Builder, JoinHandle};

use crate::{StateError, StateErrorCode};

use super::{
    MaintenanceAdmission, MaintenanceCompletion, MaintenanceCoordinator, MaintenanceExecution,
    MaintenancePermit, MaintenancePurpose, MaintenanceRejection, MaintenanceSourceState,
};

thread_local! {
    static REDACT_MAINTENANCE_PANIC: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_MAINTENANCE_PANIC_REDACTION: Once = Once::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceWorkerPhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceWorkerSnapshot {
    phase: MaintenanceWorkerPhase,
    source_state: MaintenanceSourceState,
    active_purpose: Option<MaintenancePurpose>,
    pending_purpose: Option<MaintenancePurpose>,
    latest_completion: Option<MaintenanceCompletion>,
    latest_guard_completion: Option<MaintenanceCompletion>,
    successful_count: u64,
    failure_count: u64,
    published_bytes: u64,
}

impl MaintenanceWorkerSnapshot {
    #[must_use]
    pub const fn phase(self) -> MaintenanceWorkerPhase {
        self.phase
    }

    #[must_use]
    pub const fn source_state(self) -> MaintenanceSourceState {
        self.source_state
    }

    #[must_use]
    pub const fn active_purpose(self) -> Option<MaintenancePurpose> {
        self.active_purpose
    }

    #[must_use]
    pub const fn pending_purpose(self) -> Option<MaintenancePurpose> {
        self.pending_purpose
    }

    #[must_use]
    pub const fn latest_completion(self) -> Option<MaintenanceCompletion> {
        self.latest_completion
    }

    #[must_use]
    pub const fn latest_guard_completion(self) -> Option<MaintenanceCompletion> {
        self.latest_guard_completion
    }

    #[must_use]
    pub const fn successful_count(self) -> u64 {
        self.successful_count
    }

    #[must_use]
    pub const fn failure_count(self) -> u64 {
        self.failure_count
    }

    #[must_use]
    pub const fn published_bytes(self) -> u64 {
        self.published_bytes
    }
}

#[derive(Clone, Copy)]
enum WorkerWake {
    Work,
}

struct WorkerState {
    coordinator: MaintenanceCoordinator,
    pending_start: Option<MaintenancePermit>,
    phase: MaintenanceWorkerPhase,
    latest_completion: Option<MaintenanceCompletion>,
    latest_guard_completion: Option<MaintenanceCompletion>,
    successful_count: u64,
    failure_count: u64,
    published_bytes: u64,
}

#[derive(Clone)]
pub(crate) struct MaintenanceSubmitter {
    state: Arc<Mutex<WorkerState>>,
    wake_sender: SyncSender<WorkerWake>,
}

impl MaintenanceSubmitter {
    pub(crate) fn submit(&self, purpose: MaintenancePurpose) -> MaintenanceAdmission {
        let admission = {
            let Ok(mut state) = self.state.lock() else {
                return MaintenanceAdmission::Rejected(MaintenanceRejection::Closed);
            };
            if state.phase != MaintenanceWorkerPhase::Running {
                return MaintenanceAdmission::Rejected(MaintenanceRejection::Closed);
            }
            let admission = state.coordinator.submit(purpose);
            if let MaintenanceAdmission::Started(permit) = &admission {
                state.pending_start = Some(permit.clone());
            }
            admission
        };
        if matches!(admission, MaintenanceAdmission::Started(_)) {
            match self.wake_sender.try_send(WorkerWake::Work) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {
                    if let Ok(mut state) = self.state.lock() {
                        state.phase = MaintenanceWorkerPhase::Faulted;
                    }
                    return MaintenanceAdmission::Rejected(MaintenanceRejection::Closed);
                }
            }
        }
        admission
    }
}

pub struct MaintenanceWorker {
    submitter: MaintenanceSubmitter,
    thread: Option<JoinHandle<()>>,
}

impl MaintenanceWorker {
    pub fn spawn<F>(
        source_state: MaintenanceSourceState,
        periodic_enabled: bool,
        execute: F,
    ) -> Result<Self, StateError>
    where
        F: FnMut(&MaintenancePermit) -> MaintenanceExecution + Send + 'static,
    {
        install_panic_redaction();
        let state = Arc::new(Mutex::new(WorkerState {
            coordinator: MaintenanceCoordinator::new(source_state, periodic_enabled),
            pending_start: None,
            phase: MaintenanceWorkerPhase::Running,
            latest_completion: None,
            latest_guard_completion: None,
            successful_count: 0,
            failure_count: 0,
            published_bytes: 0,
        }));
        let (wake_sender, wake_receiver) = sync_channel(1);
        let thread_state = Arc::clone(&state);
        let thread = Builder::new()
            .name(String::from("tokenmaster-backup-worker"))
            .spawn(move || {
                REDACT_MAINTENANCE_PANIC.with(|redact| redact.set(true));
                run_worker(thread_state, wake_receiver, execute);
            })
            .map_err(|_| StateError::unavailable())?;
        Ok(Self {
            submitter: MaintenanceSubmitter { state, wake_sender },
            thread: Some(thread),
        })
    }

    pub fn submit(&self, purpose: MaintenancePurpose) -> MaintenanceAdmission {
        self.submitter.submit(purpose)
    }

    #[must_use]
    pub fn snapshot(&self) -> MaintenanceWorkerSnapshot {
        let Ok(state) = self.submitter.state.lock() else {
            return MaintenanceWorkerSnapshot {
                phase: MaintenanceWorkerPhase::Faulted,
                source_state: MaintenanceSourceState::Suspect,
                active_purpose: None,
                pending_purpose: None,
                latest_completion: None,
                latest_guard_completion: None,
                successful_count: 0,
                failure_count: 0,
                published_bytes: 0,
            };
        };
        let coordinator = state.coordinator.snapshot();
        MaintenanceWorkerSnapshot {
            phase: state.phase,
            source_state: coordinator.source_state(),
            active_purpose: coordinator.active_purpose(),
            pending_purpose: coordinator.pending_purpose(),
            latest_completion: state.latest_completion,
            latest_guard_completion: state.latest_guard_completion,
            successful_count: state.successful_count,
            failure_count: state.failure_count,
            published_bytes: state.published_bytes,
        }
    }

    pub fn set_periodic_enabled(&self, enabled: bool) -> Result<(), StateError> {
        let mut state = self
            .submitter
            .state
            .lock()
            .map_err(|_| StateError::internal_invariant())?;
        state.coordinator.set_periodic_enabled(enabled);
        Ok(())
    }

    pub fn pause(&self) -> Result<MaintenanceWorkerPhase, StateError> {
        let mut state = self
            .submitter
            .state
            .lock()
            .map_err(|_| StateError::internal_invariant())?;
        match state.phase {
            MaintenanceWorkerPhase::Running => {
                state.phase = MaintenanceWorkerPhase::Paused;
                state.coordinator.cancel_active();
                Ok(MaintenanceWorkerPhase::Paused)
            }
            MaintenanceWorkerPhase::Paused => Ok(MaintenanceWorkerPhase::Paused),
            MaintenanceWorkerPhase::Faulted => Err(StateError::internal_invariant()),
            MaintenanceWorkerPhase::Stopping | MaintenanceWorkerPhase::Stopped => {
                Err(StateError::unavailable())
            }
        }
    }

    pub fn resume(&self) -> Result<MaintenanceWorkerPhase, StateError> {
        let should_wake = {
            let mut state = self
                .submitter
                .state
                .lock()
                .map_err(|_| StateError::internal_invariant())?;
            match state.phase {
                MaintenanceWorkerPhase::Paused => {
                    state.phase = MaintenanceWorkerPhase::Running;
                    state.pending_start.is_some()
                }
                MaintenanceWorkerPhase::Running => return Ok(MaintenanceWorkerPhase::Running),
                MaintenanceWorkerPhase::Faulted => {
                    return Err(StateError::internal_invariant());
                }
                MaintenanceWorkerPhase::Stopping | MaintenanceWorkerPhase::Stopped => {
                    return Err(StateError::unavailable());
                }
            }
        };
        if should_wake {
            let _ = self.submitter.wake_sender.try_send(WorkerWake::Work);
        }
        Ok(MaintenanceWorkerPhase::Running)
    }

    pub fn shutdown(&mut self) -> Result<MaintenanceWorkerPhase, StateError> {
        let Some(thread) = self.thread.take() else {
            return Ok(self.snapshot().phase());
        };
        {
            let mut state = self
                .submitter
                .state
                .lock()
                .map_err(|_| StateError::internal_invariant())?;
            if state.phase != MaintenanceWorkerPhase::Faulted {
                state.phase = MaintenanceWorkerPhase::Stopping;
            }
            state.coordinator.cancel_active();
        }
        let _ = self.submitter.wake_sender.try_send(WorkerWake::Work);
        thread
            .join()
            .map_err(|_| StateError::internal_invariant())?;
        let phase = self.snapshot().phase();
        if phase == MaintenanceWorkerPhase::Faulted {
            Err(StateError::internal_invariant())
        } else {
            Ok(phase)
        }
    }

    pub(crate) fn submitter(&self) -> MaintenanceSubmitter {
        self.submitter.clone()
    }
}

impl fmt::Debug for MaintenanceWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaintenanceWorker")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl Drop for MaintenanceWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_worker<F>(
    state: Arc<Mutex<WorkerState>>,
    wake_receiver: Receiver<WorkerWake>,
    mut execute: F,
) where
    F: FnMut(&MaintenancePermit) -> MaintenanceExecution,
{
    while wake_receiver.recv().is_ok() {
        let mut next = match take_next(&state) {
            NextWork::Execute(permit) => Some(permit),
            NextWork::Wait => continue,
            NextWork::Stop => return,
        };
        while let Some(permit) = next.take() {
            let stopping = state.lock().map_or(true, |worker| {
                worker.phase == MaintenanceWorkerPhase::Stopping
            });
            let execution = if stopping || permit.is_cancelled() {
                MaintenanceExecution::Cancelled
            } else {
                match catch_unwind(AssertUnwindSafe(|| execute(&permit))) {
                    Ok(execution) => execution,
                    Err(_) => MaintenanceExecution::Failed(StateErrorCode::InternalInvariant),
                }
            };
            let Ok(mut worker) = state.lock() else {
                return;
            };
            let transition = match worker.coordinator.finish(permit.id(), execution) {
                Ok(transition) => transition,
                Err(_) => {
                    worker.phase = MaintenanceWorkerPhase::Faulted;
                    return;
                }
            };
            let completion = transition.completion();
            if completion.outcome() == super::MaintenanceOutcome::Published {
                let Some(successful_count) = worker.successful_count.checked_add(1) else {
                    worker.phase = MaintenanceWorkerPhase::Faulted;
                    return;
                };
                let Some(published_bytes) = worker
                    .published_bytes
                    .checked_add(completion.published_bytes())
                else {
                    worker.phase = MaintenanceWorkerPhase::Faulted;
                    return;
                };
                worker.successful_count = successful_count;
                worker.published_bytes = published_bytes;
            } else {
                let Some(failure_count) = worker.failure_count.checked_add(1) else {
                    worker.phase = MaintenanceWorkerPhase::Faulted;
                    return;
                };
                worker.failure_count = failure_count;
            }
            worker.latest_completion = Some(completion);
            if completion.purpose().blocks_mutation() {
                worker.latest_guard_completion = Some(completion);
            }
            if worker.phase == MaintenanceWorkerPhase::Stopping {
                worker.pending_start = None;
                worker.phase = MaintenanceWorkerPhase::Stopped;
                return;
            }
            if let Some(follow_up) = transition.follow_up().cloned() {
                if worker.phase == MaintenanceWorkerPhase::Running {
                    next = Some(follow_up);
                } else {
                    worker.pending_start = Some(follow_up);
                }
            }
            if worker.phase == MaintenanceWorkerPhase::Paused {
                break;
            }
        }
    }
    if let Ok(mut worker) = state.lock()
        && worker.phase != MaintenanceWorkerPhase::Faulted
    {
        worker.phase = MaintenanceWorkerPhase::Stopped;
    }
}

enum NextWork {
    Execute(MaintenancePermit),
    Wait,
    Stop,
}

fn take_next(state: &Mutex<WorkerState>) -> NextWork {
    let Ok(mut worker) = state.lock() else {
        return NextWork::Stop;
    };
    match worker.phase {
        MaintenanceWorkerPhase::Running => worker
            .pending_start
            .take()
            .map_or(NextWork::Wait, NextWork::Execute),
        MaintenanceWorkerPhase::Paused => NextWork::Wait,
        MaintenanceWorkerPhase::Stopping => {
            if let Some(permit) = worker.pending_start.take() {
                NextWork::Execute(permit)
            } else {
                worker.phase = MaintenanceWorkerPhase::Stopped;
                NextWork::Stop
            }
        }
        MaintenanceWorkerPhase::Stopped | MaintenanceWorkerPhase::Faulted => NextWork::Stop,
    }
}

fn install_panic_redaction() {
    INSTALL_MAINTENANCE_PANIC_REDACTION.call_once(|| {
        let previous = take_hook();
        set_hook(Box::new(move |information| {
            let redact = REDACT_MAINTENANCE_PANIC
                .try_with(Cell::get)
                .unwrap_or(false);
            if !redact {
                previous(information);
            }
        }));
    });
}
