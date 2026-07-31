use core::fmt;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, Once,
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::{Builder, JoinHandle},
    time::{Duration, Instant},
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

/// Lossy, nonblocking hint emitted after the latest completion receipt is published.
///
/// Implementations must not block. A panic is contained by the worker and does not
/// fault refresh execution or remove the published completion receipt.
pub trait WorkerCompletionNotifier: Send + Sync + 'static {
    fn completion_ready(&self, completion: WorkerCompletion);
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
    /// A caller argument this API cannot represent -- today, a timeout whose deadline
    /// overflows `Instant`. It is a refusal rather than a panic because every other failure on
    /// this surface arrives through `Result`.
    InvalidValue,
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
            Self::InvalidValue => "invalid_value",
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

/// The one completion receipt a reader can collect, and the wait that does not hold it.
///
/// This was a `sync_channel(1)` whose `Receiver` lived behind a mutex, because a `std`
/// `Receiver` is not `Sync`. Waiting on it therefore held that mutex for the caller's whole
/// timeout, and the same mutex is taken by `try_completion` and by the worker thread itself
/// when it evicts a stale receipt. Measured: a reader arriving behind a two-second wait was
/// blocked for 1.91 of those seconds. Slicing the wait did not help -- the platform mutex is
/// not fair, so a waiter that releases and immediately reacquires is handed it straight back.
///
/// A condition variable is the primitive that fixes it: `wait_timeout` releases the mutex
/// while it waits and reacquires only to return. A one-slot queue with manual eviction is
/// also just a slot whose newest value wins, so saying that directly is simpler than the
/// channel was.
///
/// **Lock order is slot then state, never the reverse.** `publish_latest` is the only place
/// that holds both; every other path takes one.
struct SlotState {
    latest: Option<WorkerCompletion>,
    /// When `latest` was written, on the same monotonic clock a waiter's deadline comes from.
    /// The publisher owns this mutex across the whole of `publish_latest`, so a waiter that
    /// observes a receipt after its deadline cannot tell whether the work met that deadline.
    /// `wait_for_reconstructed_reconciliation` passes its remaining mandatory-backup budget
    /// here and treats a receipt as success, so accepting whatever is present lets
    /// reconstruction succeed after its own deadline.
    latest_at: Option<Instant>,
    closed: bool,
}

struct CompletionSlot {
    state: Mutex<SlotState>,
    ready: Condvar,
}

impl CompletionSlot {
    fn new() -> Self {
        Self {
            state: Mutex::new(SlotState {
                latest: None,
                latest_at: None,
                closed: false,
            }),
            ready: Condvar::new(),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, SlotState>, WorkerError> {
        self.state
            .lock()
            .map_err(|_| WorkerError::new(WorkerErrorCode::Internal))
    }

    /// Marks the slot closed and wakes every waiter, so a worker that has stopped producing
    /// ends their wait instead of leaving them to their timeout. The channel got this from
    /// disconnection; a slot has to say it.
    ///
    /// `closed` lives in the same mutex as `latest` because a waiter checks both and then
    /// waits, and a condition variable only guarantees the wakeup is not lost when the
    /// predicate is read under the very lock it waits on. Held apart, this sequence loses it:
    /// the waiter reads `closed` false and releases that second lock, the worker sets it and
    /// notifies, and only then does the waiter register -- after the notification, sleeping
    /// out its whole timeout for a receipt that can never arrive.
    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
        }
        self.ready.notify_all();
    }
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
    completions: Arc<CompletionSlot>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct RefreshSubmitter {
    clock: Arc<dyn Clock>,
    state: Arc<Mutex<WorkerState>>,
    wake_sender: SyncSender<Wake>,
}

impl RefreshWorker {
    pub fn spawn<F>(clock: Arc<dyn Clock>, execute: F) -> Result<Self, WorkerError>
    where
        F: FnMut(&RefreshPermit) -> RefreshOutcome + Send + 'static,
    {
        Self::spawn_with_notifier(clock, None, execute)
    }

    pub fn spawn_notified<F>(
        clock: Arc<dyn Clock>,
        notifier: Arc<dyn WorkerCompletionNotifier>,
        execute: F,
    ) -> Result<Self, WorkerError>
    where
        F: FnMut(&RefreshPermit) -> RefreshOutcome + Send + 'static,
    {
        Self::spawn_with_notifier(clock, Some(notifier), execute)
    }

    fn spawn_with_notifier<F>(
        clock: Arc<dyn Clock>,
        notifier: Option<Arc<dyn WorkerCompletionNotifier>>,
        execute: F,
    ) -> Result<Self, WorkerError>
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
        let completions = Arc::new(CompletionSlot::new());
        let worker_state = state.clone();
        let worker_clock = clock.clone();
        let worker_completions = completions.clone();
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
                        worker_completions.clone(),
                        notifier,
                        execute,
                    );
                }))
                .is_err()
                {
                    fault_and_abandon(&recovery_state);
                }
                // Whatever ended the thread -- shutdown or a panic already contained above --
                // nothing further will be published. The channel this replaced said that by
                // disconnecting; a slot has to say it, or a waiter sits out its whole timeout
                // for a receipt that can no longer arrive.
                worker_completions.close();
            })
            .map_err(|_| WorkerError::new(WorkerErrorCode::Unavailable))?;

        Ok(Self {
            clock,
            state,
            wake_sender,
            completions,
            thread: Some(thread),
        })
    }

    pub fn submit(
        &self,
        urgency: RefreshUrgency,
        deadline: Option<RefreshDeadline>,
    ) -> Result<RefreshAdmission, WorkerError> {
        self.submitter().submit(urgency, deadline)
    }

    #[must_use]
    pub fn submitter(&self) -> RefreshSubmitter {
        RefreshSubmitter {
            clock: Arc::clone(&self.clock),
            state: Arc::clone(&self.state),
            wake_sender: self.wake_sender.clone(),
        }
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

    /// Takes whatever is in the slot, with no deadline to miss.
    ///
    /// The publication stamp is cleared with the receipt so a later timed wait cannot compare
    /// against a stamp whose receipt is gone.
    pub fn try_completion(&self) -> Result<Option<WorkerCompletion>, WorkerError> {
        let mut slot = self.completions.lock()?;
        let completion = slot.latest.take();
        if completion.is_some() {
            slot.latest_at = None;
        }
        Ok(completion)
    }

    /// Waits for a completion without holding the slot while it waits.
    ///
    /// `Condvar::wait_timeout` releases the mutex for the duration of the wait and reacquires
    /// it only to return, which is the whole reason this is a condition variable and not a
    /// channel behind a lock. The previous shape held that lock for the caller's entire
    /// timeout, and `try_completion` -- sixty-one call sites -- and the worker thread's own
    /// eviction path both need it. Slicing the wait was tried first and measured: a competing
    /// reader still waited 1.91 seconds of a two-second wait, because the platform mutex hands
    /// itself straight back to a thread that releases and reacquires.
    pub fn wait_for_completion(
        &self,
        timeout: Duration,
    ) -> Result<Option<WorkerCompletion>, WorkerError> {
        // Checked: `Instant + Duration` panics on overflow, and a caller expressing "wait
        // effectively forever" as `Duration::MAX` would crash a public API that reports every
        // other failure through `Result`. The `recv_timeout` this replaced accepted it.
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| WorkerError::new(WorkerErrorCode::InvalidValue))?;
        let mut state = self.completions.lock()?;
        loop {
            // Taken only when it was published by the deadline. A receipt written after it is
            // left in the slot for whoever asks next rather than reported as work that met a
            // budget it missed.
            if state
                .latest_at
                .is_some_and(|published| published <= deadline)
                && let Some(completion) = state.latest.take()
            {
                state.latest_at = None;
                return Ok(Some(completion));
            }
            if state.closed {
                return Err(WorkerError::new(WorkerErrorCode::Closed));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            let (guard, _timed_out) = self
                .completions
                .ready
                .wait_timeout(state, remaining)
                .map_err(|_| WorkerError::new(WorkerErrorCode::Internal))?;
            state = guard;
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

impl RefreshSubmitter {
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
    completions: Arc<CompletionSlot>,
    notifier: Option<Arc<dyn WorkerCompletionNotifier>>,
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
                    let _ = publish_latest(&state, &completions, completion);
                    if let Ok(mut worker_state) = state.lock() {
                        worker_state.phase = WorkerPhase::Faulted;
                    }
                    notify_completion(notifier.as_deref(), completion);
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
            if publish_latest(&state, &completions, completion).is_err() {
                return;
            }
            notify_completion(notifier.as_deref(), completion);
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

fn notify_completion(
    notifier: Option<&dyn WorkerCompletionNotifier>,
    completion: WorkerCompletion,
) {
    if let Some(notifier) = notifier {
        let _ = catch_unwind(AssertUnwindSafe(|| notifier.completion_ready(completion)));
    }
}

/// Publishes the newest completion receipt, replacing any the reader has not collected.
///
/// One slot, newest wins. When a receipt is displaced the worker's superseded counter rises
/// and the receipt being published carries the new count, which is the behaviour the previous
/// one-slot channel produced by evicting and retrying -- said here in one step instead of a
/// send-evict-resend loop.
///
/// This is the only place that holds the slot and the state together, and it takes them in
/// that order. Nothing takes them the other way round.
fn publish_latest(
    state: &Arc<Mutex<WorkerState>>,
    completions: &Arc<CompletionSlot>,
    mut completion: WorkerCompletion,
) -> Result<(), WorkerError> {
    let mut slot = completions.lock()?;
    if slot.closed {
        return Err(WorkerError::new(WorkerErrorCode::Closed));
    }
    if slot.latest.is_some() {
        let mut state = lock_state(state)?;
        state.superseded_results = state
            .superseded_results
            .checked_add(1)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or_else(|| WorkerError::new(WorkerErrorCode::CapacityExceeded))?;
        completion = completion.with_superseded_results(state.superseded_results);
    }
    slot.latest = Some(completion);
    slot.latest_at = Some(Instant::now());
    drop(slot);
    completions.ready.notify_all();
    Ok(())
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod completion_slot_tests {
    use super::*;

    /// A receipt is accepted for the deadline it met, not for the moment it was noticed.
    ///
    /// `publish_latest` owns this mutex for its whole body, so a waiter whose timer expires
    /// meanwhile cannot reacquire until the publisher is done. Comparing the publication stamp
    /// against the caller's deadline is what separates a receipt that met the budget from one
    /// written after it: `wait_for_reconstructed_reconciliation` passes its remaining
    /// mandatory-backup budget here and treats a receipt as success, so without the stamp
    /// reconstruction succeeds on evidence that arrived too late.
    fn observed(publish_after: Duration) -> Option<WorkerCompletion> {
        let slot = Arc::new(CompletionSlot::new());
        let completion = WorkerCompletion {
            request_id: RefreshRequestId::new(1).expect("request id"),
            outcome: RefreshOutcome::Completed,
            kind: WorkerCompletionKind::Executed,
            superseded_results: 0,
            follow_up_started: false,
            follow_up_abandoned: false,
            pending_deadline_exceeded: false,
            pending_capacity_exceeded: false,
        };
        let holder = Arc::clone(&slot);
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            let mut state = holder.state.lock().expect("slot");
            std::thread::sleep(publish_after);
            state.latest = Some(completion);
            state.latest_at = Some(Instant::now());
            std::thread::sleep(Duration::from_millis(200));
            drop(state);
            holder.ready.notify_all();
        });

        let deadline = Instant::now() + Duration::from_millis(60);
        let mut state = slot.state.lock().expect("slot");
        let seen = loop {
            if state
                .latest_at
                .is_some_and(|published| published <= deadline)
                && let Some(value) = state.latest.take()
            {
                break Some(value);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break None;
            }
            let (guard, _) = slot.ready.wait_timeout(state, remaining).expect("wait");
            state = guard;
        };
        drop(state);
        writer.join().expect("writer thread");
        seen
    }

    #[test]
    fn a_receipt_published_before_the_deadline_survives_being_seen_after_it() {
        assert!(
            observed(Duration::from_millis(20)).is_some(),
            "a receipt written before the deadline was discarded for arriving late"
        );
    }

    #[test]
    fn a_receipt_published_after_the_deadline_is_not_reported_as_success() {
        assert!(
            observed(Duration::from_millis(180)).is_none(),
            "work that finished after the deadline was reported as meeting it"
        );
    }
}
