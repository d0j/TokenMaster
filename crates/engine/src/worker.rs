use core::fmt;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, MutexGuard, Once,
        mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    },
    thread::{Builder, JoinHandle},
};

use crate::{
    Clock, EngineError, EngineErrorCode, RefreshAdmission, RefreshCoordinator, RefreshDeadline,
    RefreshOutcome, RefreshPermit, RefreshRequestId, RefreshUrgency,
};

thread_local! {
    static REDACT_WORKER_PANIC: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_WORKER_PANIC_REDACTION: Once = Once::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerPhase {
    Running,
    ShuttingDown,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerCompletionKind {
    Executed,
    NotStarted,
    Panicked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerCompletion {
    request_id: RefreshRequestId,
    outcome: RefreshOutcome,
    kind: WorkerCompletionKind,
    superseded_results: u64,
    follow_up_started: bool,
    follow_up_abandoned: bool,
    pending_deadline_exceeded: bool,
    pending_capacity_exceeded: bool,
}

impl WorkerCompletion {
    #[must_use]
    pub const fn request_id(self) -> RefreshRequestId {
        self.request_id
    }

    #[must_use]
    pub const fn outcome(self) -> RefreshOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn kind(self) -> WorkerCompletionKind {
        self.kind
    }

    #[must_use]
    pub const fn superseded_results(self) -> u64 {
        self.superseded_results
    }

    #[must_use]
    pub const fn follow_up_started(self) -> bool {
        self.follow_up_started
    }

    #[must_use]
    pub const fn follow_up_abandoned(self) -> bool {
        self.follow_up_abandoned
    }

    #[must_use]
    pub const fn pending_deadline_exceeded(self) -> bool {
        self.pending_deadline_exceeded
    }

    #[must_use]
    pub const fn pending_capacity_exceeded(self) -> bool {
        self.pending_capacity_exceeded
    }

    const fn with_superseded_results(mut self, superseded_results: u64) -> Self {
        self.superseded_results = superseded_results;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerSnapshot {
    phase: WorkerPhase,
    active_request_id: Option<RefreshRequestId>,
    pending_count: usize,
    superseded_results: u64,
}

impl WorkerSnapshot {
    #[must_use]
    pub const fn phase(self) -> WorkerPhase {
        self.phase
    }

    #[must_use]
    pub const fn active_request_id(self) -> Option<RefreshRequestId> {
        self.active_request_id
    }

    #[must_use]
    pub const fn pending_count(self) -> usize {
        self.pending_count
    }

    #[must_use]
    pub const fn superseded_results(self) -> u64 {
        self.superseded_results
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerErrorCode {
    Closed,
    Faulted,
    CapacityExceeded,
    StaleRequest,
    Unavailable,
    Internal,
}

impl fmt::Display for WorkerErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Closed => "closed",
            Self::Faulted => "faulted",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::StaleRequest => "stale_request",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{code}")]
pub struct WorkerError {
    code: WorkerErrorCode,
}

impl WorkerError {
    const fn new(code: WorkerErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> WorkerErrorCode {
        self.code
    }
}

#[derive(Clone, Copy)]
enum Wake {
    Work,
}

struct WorkerState {
    coordinator: RefreshCoordinator,
    pending_start: Option<RefreshPermit>,
    phase: WorkerPhase,
    superseded_results: u64,
}

pub struct RefreshWorker {
    clock: Arc<dyn Clock>,
    state: Arc<Mutex<WorkerState>>,
    wake_sender: SyncSender<Wake>,
    result_receiver: Arc<Mutex<Receiver<WorkerCompletion>>>,
    thread: Option<JoinHandle<()>>,
}

impl RefreshWorker {
    pub fn spawn<F>(clock: Arc<dyn Clock>, execute: F) -> Result<Self, WorkerError>
    where
        F: FnMut(&RefreshPermit) -> RefreshOutcome + Send + 'static,
    {
        install_worker_panic_redaction();
        let state = Arc::new(Mutex::new(WorkerState {
            coordinator: RefreshCoordinator::new(),
            pending_start: None,
            phase: WorkerPhase::Running,
            superseded_results: 0,
        }));
        let (wake_sender, wake_receiver) = sync_channel(1);
        let (result_sender, result_receiver) = sync_channel(1);
        let result_receiver = Arc::new(Mutex::new(result_receiver));
        let worker_state = state.clone();
        let worker_clock = clock.clone();
        let worker_result_receiver = result_receiver.clone();
        let thread = Builder::new()
            .name(String::from("tokenmaster-refresh"))
            .spawn(move || {
                REDACT_WORKER_PANIC.with(|redact| redact.set(true));
                let recovery_state = worker_state.clone();
                if catch_unwind(AssertUnwindSafe(|| {
                    run_worker(
                        worker_clock,
                        worker_state,
                        wake_receiver,
                        result_sender,
                        worker_result_receiver,
                        execute,
                    );
                }))
                .is_err()
                {
                    fault_and_abandon(&recovery_state);
                }
            })
            .map_err(|_| WorkerError::new(WorkerErrorCode::Unavailable))?;

        Ok(Self {
            clock,
            state,
            wake_sender,
            result_receiver,
            thread: Some(thread),
        })
    }

    pub fn submit(
        &self,
        urgency: RefreshUrgency,
        deadline: Option<RefreshDeadline>,
    ) -> Result<RefreshAdmission, WorkerError> {
        ensure_admission_phase(lock_state(&self.state)?.phase)?;
        let now = self.clock.now();
        let admission = {
            let mut state = lock_state(&self.state)?;
            ensure_admission_phase(state.phase)?;
            let admission = state
                .coordinator
                .submit(urgency, deadline, now)
                .map_err(map_engine_error)?;
            if let RefreshAdmission::Started(permit) = &admission {
                if state.pending_start.is_some() {
                    return Err(WorkerError::new(WorkerErrorCode::Internal));
                }
                state.pending_start = Some(permit.clone());
            }
            admission
        };

        if matches!(admission, RefreshAdmission::Started(_)) {
            match self.wake_sender.try_send(Wake::Work) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {
                    return Err(WorkerError::new(WorkerErrorCode::Closed));
                }
            }
        }
        Ok(admission)
    }

    pub fn cancel(&self, request_id: RefreshRequestId) -> Result<(), WorkerError> {
        lock_state(&self.state)?
            .coordinator
            .cancel(request_id)
            .map_err(map_engine_error)
    }

    pub fn snapshot(&self) -> Result<WorkerSnapshot, WorkerError> {
        let state = lock_state(&self.state)?;
        Ok(WorkerSnapshot {
            phase: state.phase,
            active_request_id: state.coordinator.active_request_id(),
            pending_count: state.coordinator.pending_count(),
            superseded_results: state.superseded_results,
        })
    }

    pub fn try_completion(&self) -> Result<Option<WorkerCompletion>, WorkerError> {
        let receiver = self
            .result_receiver
            .lock()
            .map_err(|_| WorkerError::new(WorkerErrorCode::Internal))?;
        match receiver.try_recv() {
            Ok(completion) => Ok(Some(completion)),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }

    pub fn shutdown(&mut self) -> Result<WorkerPhase, WorkerError> {
        if self.thread.is_none() {
            return self.snapshot().map(WorkerSnapshot::phase);
        }
        {
            let mut state = lock_state(&self.state)?;
            if state.phase == WorkerPhase::Running {
                state.phase = WorkerPhase::ShuttingDown;
            }
            if let Some(request_id) = state.coordinator.active_request_id() {
                state
                    .coordinator
                    .cancel(request_id)
                    .map_err(map_engine_error)?;
            }
        }
        let _ = self.wake_sender.try_send(Wake::Work);
        let thread = self
            .thread
            .take()
            .ok_or_else(|| WorkerError::new(WorkerErrorCode::Internal))?;
        thread
            .join()
            .map_err(|_| WorkerError::new(WorkerErrorCode::Internal))?;
        let mut state = lock_state(&self.state)?;
        if state.phase != WorkerPhase::Faulted {
            state.phase = WorkerPhase::Stopped;
        }
        Ok(state.phase)
    }
}

impl Drop for RefreshWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_worker<F>(
    clock: Arc<dyn Clock>,
    state: Arc<Mutex<WorkerState>>,
    wake_receiver: Receiver<Wake>,
    result_sender: SyncSender<WorkerCompletion>,
    result_receiver: Arc<Mutex<Receiver<WorkerCompletion>>>,
    mut execute: F,
) where
    F: FnMut(&RefreshPermit) -> RefreshOutcome,
{
    while wake_receiver.recv().is_ok() {
        let permit = match state.lock() {
            Ok(mut state) => match state.pending_start.take() {
                Some(permit) => permit,
                None if state.phase == WorkerPhase::ShuttingDown => {
                    state.phase = WorkerPhase::Stopped;
                    return;
                }
                None => continue,
            },
            Err(_) => return,
        };
        let mut next = Some(permit);

        while let Some(permit) = next.take() {
            let now = clock.now();
            let preflight = match state.lock() {
                Ok(mut worker_state) if worker_state.phase == WorkerPhase::ShuttingDown => {
                    if !permit.is_cancelled()
                        && worker_state.coordinator.cancel(permit.id()).is_err()
                    {
                        worker_state.phase = WorkerPhase::Stopped;
                        return;
                    }
                    Some((RefreshOutcome::Cancelled, WorkerCompletionKind::NotStarted))
                }
                Ok(_) if permit.is_cancelled() => {
                    Some((RefreshOutcome::Cancelled, WorkerCompletionKind::NotStarted))
                }
                Ok(_) if permit.deadline_exceeded(now) => Some((
                    RefreshOutcome::DeadlineExceeded,
                    WorkerCompletionKind::NotStarted,
                )),
                Ok(_) => None,
                Err(_) => return,
            };
            let execution = match preflight {
                Some(result) => Ok(result),
                None => catch_unwind(AssertUnwindSafe(|| execute(&permit)))
                    .map(|outcome| (outcome, WorkerCompletionKind::Executed)),
            };
            let (outcome, kind) = match execution {
                Ok(result) => result,
                Err(_) => {
                    let failed_at = clock.now();
                    let completion = match state.lock() {
                        Ok(mut worker_state) => {
                            let transition = match worker_state.coordinator.finish(
                                permit.id(),
                                RefreshOutcome::Failed,
                                failed_at,
                            ) {
                                Ok(transition) => transition,
                                Err(_) => {
                                    worker_state.phase = WorkerPhase::Faulted;
                                    return;
                                }
                            };
                            let abandoned = transition.follow_up().cloned();
                            if let Some(follow_up) = &abandoned
                                && (worker_state.coordinator.cancel(follow_up.id()).is_err()
                                    || worker_state
                                        .coordinator
                                        .finish(
                                            follow_up.id(),
                                            RefreshOutcome::Cancelled,
                                            failed_at,
                                        )
                                        .is_err())
                            {
                                worker_state.phase = WorkerPhase::Faulted;
                                return;
                            }
                            worker_state.phase = WorkerPhase::ShuttingDown;
                            WorkerCompletion {
                                request_id: transition.completed().request_id(),
                                outcome: RefreshOutcome::Failed,
                                kind: WorkerCompletionKind::Panicked,
                                superseded_results: worker_state.superseded_results,
                                follow_up_started: false,
                                follow_up_abandoned: abandoned.is_some(),
                                pending_deadline_exceeded: transition.pending_deadline_exceeded(),
                                pending_capacity_exceeded: transition.pending_capacity_exceeded(),
                            }
                        }
                        Err(_) => return,
                    };
                    let _ = publish_latest(&state, &result_sender, &result_receiver, completion);
                    if let Ok(mut worker_state) = state.lock() {
                        worker_state.phase = WorkerPhase::Faulted;
                    }
                    return;
                }
            };
            let finished_at = clock.now();
            let (completion, follow_up) = match state.lock() {
                Ok(mut state) => {
                    match state.coordinator.finish(permit.id(), outcome, finished_at) {
                        Ok(transition) => {
                            let follow_up = transition.follow_up().cloned();
                            let completion = WorkerCompletion {
                                request_id: transition.completed().request_id(),
                                outcome: transition.completed().outcome(),
                                kind,
                                superseded_results: state.superseded_results,
                                follow_up_started: follow_up.is_some(),
                                follow_up_abandoned: false,
                                pending_deadline_exceeded: transition.pending_deadline_exceeded(),
                                pending_capacity_exceeded: transition.pending_capacity_exceeded(),
                            };
                            (completion, follow_up)
                        }
                        Err(_) => {
                            state.phase = WorkerPhase::Stopped;
                            return;
                        }
                    }
                }
                Err(_) => return,
            };
            if publish_latest(&state, &result_sender, &result_receiver, completion).is_err() {
                return;
            }
            next = follow_up;
        }

        match state.lock() {
            Ok(mut state) if state.phase == WorkerPhase::ShuttingDown => {
                state.phase = WorkerPhase::Stopped;
                return;
            }
            Ok(_) => {}
            Err(_) => return,
        }
    }
    if let Ok(mut state) = state.lock() {
        state.phase = WorkerPhase::Stopped;
    }
}

fn publish_latest(
    state: &Arc<Mutex<WorkerState>>,
    sender: &SyncSender<WorkerCompletion>,
    receiver: &Arc<Mutex<Receiver<WorkerCompletion>>>,
    mut completion: WorkerCompletion,
) -> Result<(), WorkerError> {
    loop {
        match sender.try_send(completion) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => {
                return Err(WorkerError::new(WorkerErrorCode::Closed));
            }
            Err(TrySendError::Full(returned)) => {
                completion = returned;
                let removed = receiver
                    .lock()
                    .map_err(|_| WorkerError::new(WorkerErrorCode::Internal))?
                    .try_recv()
                    .is_ok();
                if removed {
                    let mut state = lock_state(state)?;
                    state.superseded_results = state
                        .superseded_results
                        .checked_add(1)
                        .filter(|value| *value <= i64::MAX as u64)
                        .ok_or_else(|| WorkerError::new(WorkerErrorCode::CapacityExceeded))?;
                    completion = completion.with_superseded_results(state.superseded_results);
                }
            }
        }
    }
}

fn lock_state(state: &Arc<Mutex<WorkerState>>) -> Result<MutexGuard<'_, WorkerState>, WorkerError> {
    state
        .lock()
        .map_err(|_| WorkerError::new(WorkerErrorCode::Internal))
}

fn map_engine_error(error: EngineError) -> WorkerError {
    let code = match error.code() {
        EngineErrorCode::CapacityExceeded => WorkerErrorCode::CapacityExceeded,
        EngineErrorCode::StaleRequest => WorkerErrorCode::StaleRequest,
        EngineErrorCode::InvalidValue => WorkerErrorCode::Internal,
    };
    WorkerError::new(code)
}

fn ensure_admission_phase(phase: WorkerPhase) -> Result<(), WorkerError> {
    match phase {
        WorkerPhase::Running => Ok(()),
        WorkerPhase::Faulted => Err(WorkerError::new(WorkerErrorCode::Faulted)),
        WorkerPhase::ShuttingDown | WorkerPhase::Stopped => {
            Err(WorkerError::new(WorkerErrorCode::Closed))
        }
    }
}

fn fault_and_abandon(state: &Arc<Mutex<WorkerState>>) {
    let mut worker_state = match state.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(request_id) = worker_state.coordinator.active_request_id() {
        let _ = worker_state.coordinator.cancel(request_id);
    }
    worker_state.coordinator = RefreshCoordinator::new();
    worker_state.pending_start = None;
    worker_state.phase = WorkerPhase::Faulted;
}

fn install_worker_panic_redaction() {
    INSTALL_WORKER_PANIC_REDACTION.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |information| {
            let redact = REDACT_WORKER_PANIC.try_with(Cell::get).unwrap_or(false);
            if !redact {
                previous(information);
            }
        }));
    });
}
