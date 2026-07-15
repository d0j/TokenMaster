use std::cell::Cell;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, Once,
    atomic::Ordering,
    mpsc::{Receiver, RecvTimeoutError, sync_channel},
};
use std::thread::{Builder, JoinHandle};
use std::time::Duration;

use tokenmaster_engine::{Clock, RefreshUrgency};

use crate::hints::{
    FLAG_DIRTY, FLAG_FORCE, HintState, RefreshHintSink, SchedulerPhase, SchedulerWake,
    WatcherHealth, flags_urgency,
};

pub const QUIET_WINDOW_MILLIS: u64 = 250;
pub const HEALTHY_POLL_MILLIS: u64 = 15 * 60 * 1_000;
pub const DEGRADED_POLL_MILLIS: u64 = 60 * 1_000;

thread_local! {
    static REDACT_SCHEDULER_PANIC: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_SCHEDULER_PANIC_REDACTION: Once = Once::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerErrorCode {
    Closed,
    Faulted,
    CapacityExceeded,
    Unavailable,
    Internal,
}

impl fmt::Display for SchedulerErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Closed => "closed",
            Self::Faulted => "faulted",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerError {
    code: SchedulerErrorCode,
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code.fmt(formatter)
    }
}

impl std::error::Error for SchedulerError {}

impl SchedulerError {
    const fn new(code: SchedulerErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> SchedulerErrorCode {
        self.code
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerSnapshot {
    phase: SchedulerPhase,
    dirty: bool,
    force_reconcile: bool,
    watcher_health: WatcherHealth,
    accepted_hint_count: u64,
    submitted_count: u64,
}

impl SchedulerSnapshot {
    #[must_use]
    pub const fn phase(self) -> SchedulerPhase {
        self.phase
    }

    #[must_use]
    pub const fn dirty(self) -> bool {
        self.dirty
    }

    #[must_use]
    pub const fn force_reconcile(self) -> bool {
        self.force_reconcile
    }

    #[must_use]
    pub const fn watcher_health(self) -> WatcherHealth {
        self.watcher_health
    }

    #[must_use]
    pub const fn accepted_hint_count(self) -> u64 {
        self.accepted_hint_count
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.submitted_count
    }
}

pub struct RefreshScheduler {
    state: Arc<HintState>,
    hints: RefreshHintSink,
    thread: Option<JoinHandle<()>>,
}

impl RefreshScheduler {
    pub fn spawn<F, E>(clock: Arc<dyn Clock>, submit: F) -> Result<Self, SchedulerError>
    where
        F: FnMut(RefreshUrgency) -> Result<(), E> + Send + 'static,
    {
        install_scheduler_panic_redaction();
        let state = Arc::new(HintState::new());
        let (wake_sender, wake_receiver) = sync_channel(1);
        let hints = RefreshHintSink::new(state.clone(), clock.clone(), wake_sender);
        let thread_state = state.clone();
        let thread = Builder::new()
            .name(String::from("tokenmaster-scheduler"))
            .spawn(move || {
                REDACT_SCHEDULER_PANIC.with(|redact| redact.set(true));
                let recovery_state = thread_state.clone();
                if catch_unwind(AssertUnwindSafe(|| {
                    run_scheduler(clock, thread_state, wake_receiver, submit);
                }))
                .is_err()
                {
                    recovery_state.set_phase(SchedulerPhase::Faulted);
                }
            })
            .map_err(|_| SchedulerError::new(SchedulerErrorCode::Unavailable))?;
        Ok(Self {
            state,
            hints,
            thread: Some(thread),
        })
    }

    #[must_use]
    pub fn hints(&self) -> RefreshHintSink {
        self.hints.clone()
    }

    #[must_use]
    pub fn snapshot(&self) -> SchedulerSnapshot {
        let flags = self.state.flags.load(Ordering::Acquire);
        SchedulerSnapshot {
            phase: self.state.phase(),
            dirty: flags & FLAG_DIRTY != 0,
            force_reconcile: flags & FLAG_FORCE != 0,
            watcher_health: self.state.watcher_health(),
            accepted_hint_count: self.state.accepted_hint_count.load(Ordering::Acquire),
            submitted_count: self.state.submitted_count.load(Ordering::Acquire),
        }
    }

    pub fn pause(&self) -> Result<SchedulerPhase, SchedulerError> {
        loop {
            match self.state.phase() {
                SchedulerPhase::Running => {
                    if self
                        .state
                        .transition_phase(SchedulerPhase::Running, SchedulerPhase::Paused)
                    {
                        let _ = self.hints.wake();
                        return Ok(SchedulerPhase::Paused);
                    }
                }
                SchedulerPhase::Paused => return Ok(SchedulerPhase::Paused),
                SchedulerPhase::Faulted => {
                    return Err(SchedulerError::new(SchedulerErrorCode::Faulted));
                }
                SchedulerPhase::Stopping | SchedulerPhase::Stopped => {
                    return Err(SchedulerError::new(SchedulerErrorCode::Closed));
                }
            }
        }
    }

    pub fn resume(&self) -> Result<SchedulerPhase, SchedulerError> {
        loop {
            match self.state.phase() {
                SchedulerPhase::Paused => {
                    if self
                        .state
                        .transition_phase(SchedulerPhase::Paused, SchedulerPhase::Running)
                    {
                        if !self.hints.force_reconcile(RefreshUrgency::Recovery) {
                            return Err(SchedulerError::new(SchedulerErrorCode::Closed));
                        }
                        return Ok(SchedulerPhase::Running);
                    }
                }
                SchedulerPhase::Running => return Ok(SchedulerPhase::Running),
                SchedulerPhase::Faulted => {
                    return Err(SchedulerError::new(SchedulerErrorCode::Faulted));
                }
                SchedulerPhase::Stopping | SchedulerPhase::Stopped => {
                    return Err(SchedulerError::new(SchedulerErrorCode::Closed));
                }
            }
        }
    }

    pub fn shutdown(&mut self) -> Result<SchedulerPhase, SchedulerError> {
        if self.thread.is_none() {
            return Ok(self.state.phase());
        }
        loop {
            match self.state.phase() {
                SchedulerPhase::Running => {
                    if self
                        .state
                        .transition_phase(SchedulerPhase::Running, SchedulerPhase::Stopping)
                    {
                        break;
                    }
                }
                SchedulerPhase::Paused => {
                    if self
                        .state
                        .transition_phase(SchedulerPhase::Paused, SchedulerPhase::Stopping)
                    {
                        break;
                    }
                }
                SchedulerPhase::Stopping | SchedulerPhase::Stopped | SchedulerPhase::Faulted => {
                    break;
                }
            }
        }
        let _ = self.hints.wake();
        let thread = self
            .thread
            .take()
            .ok_or_else(|| SchedulerError::new(SchedulerErrorCode::Internal))?;
        thread
            .join()
            .map_err(|_| SchedulerError::new(SchedulerErrorCode::Internal))?;
        if self.state.phase() != SchedulerPhase::Faulted {
            self.state.set_phase(SchedulerPhase::Stopped);
        }
        Ok(self.state.phase())
    }
}

impl fmt::Debug for RefreshScheduler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RefreshScheduler")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl Drop for RefreshScheduler {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_scheduler<F, E>(
    clock: Arc<dyn Clock>,
    state: Arc<HintState>,
    wake_receiver: Receiver<SchedulerWake>,
    mut submit: F,
) where
    F: FnMut(RefreshUrgency) -> Result<(), E>,
{
    let mut last_now = clock.now().as_millis();
    let mut last_submit = last_now;
    loop {
        match state.phase() {
            SchedulerPhase::Stopping => {
                state.set_phase(SchedulerPhase::Stopped);
                return;
            }
            SchedulerPhase::Stopped | SchedulerPhase::Faulted => return,
            SchedulerPhase::Paused => {
                if wake_receiver.recv().is_err() {
                    state.set_phase(SchedulerPhase::Stopped);
                    return;
                }
                continue;
            }
            SchedulerPhase::Running => {}
        }

        let now = clock.now().as_millis();
        if now < last_now {
            state.force_clock_discontinuity();
        }
        last_now = now;

        let flags = state.flags.load(Ordering::Acquire);
        let mut quiet_deadline = None;
        if flags & FLAG_FORCE != 0 {
            let taken = state.flags.swap(0, Ordering::AcqRel);
            match submit_one(&state, &mut submit, flags_urgency(taken)) {
                SubmitStatus::Submitted => last_submit = now,
                SubmitStatus::Deferred => {
                    state.flags.fetch_or(taken, Ordering::AcqRel);
                }
                SubmitStatus::Faulted => return,
            }
            continue;
        }
        if flags & FLAG_DIRTY != 0 {
            let latest = state.latest_hint_tick.load(Ordering::Acquire);
            if latest > now {
                state.force_clock_discontinuity();
                continue;
            }
            let Some(deadline) = latest.checked_add(QUIET_WINDOW_MILLIS) else {
                state.force_clock_discontinuity();
                continue;
            };
            quiet_deadline = Some(deadline);
            if now >= deadline {
                let taken = state.flags.swap(0, Ordering::AcqRel);
                let latest_after = state.latest_hint_tick.load(Ordering::Acquire);
                if latest_after > latest {
                    state.flags.fetch_or(taken, Ordering::AcqRel);
                    continue;
                }
                match submit_one(&state, &mut submit, flags_urgency(taken)) {
                    SubmitStatus::Submitted => last_submit = now,
                    SubmitStatus::Deferred => {
                        state.flags.fetch_or(taken, Ordering::AcqRel);
                    }
                    SubmitStatus::Faulted => return,
                }
                continue;
            }
        }

        let interval = match state.watcher_health() {
            WatcherHealth::Healthy => HEALTHY_POLL_MILLIS,
            WatcherHealth::Degraded => DEGRADED_POLL_MILLIS,
        };
        let Some(periodic_deadline) = last_submit.checked_add(interval) else {
            state.set_phase(SchedulerPhase::Faulted);
            return;
        };
        if now >= periodic_deadline {
            match submit_one(&state, &mut submit, RefreshUrgency::Periodic) {
                SubmitStatus::Submitted => last_submit = now,
                SubmitStatus::Deferred => {}
                SubmitStatus::Faulted => return,
            }
            continue;
        }

        let next_deadline = quiet_deadline
            .map(|deadline| deadline.min(periodic_deadline))
            .unwrap_or(periodic_deadline);
        let wait = Duration::from_millis(next_deadline.saturating_sub(now));
        match wake_receiver.recv_timeout(wait) {
            Ok(SchedulerWake::Signal) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                state.set_phase(SchedulerPhase::Stopped);
                return;
            }
        }
    }
}

enum SubmitStatus {
    Submitted,
    Deferred,
    Faulted,
}

fn submit_one<F, E>(state: &HintState, submit: &mut F, urgency: RefreshUrgency) -> SubmitStatus
where
    F: FnMut(RefreshUrgency) -> Result<(), E>,
{
    if state.phase() != SchedulerPhase::Running {
        return SubmitStatus::Deferred;
    }
    if submit(urgency).is_err() {
        state.set_phase(SchedulerPhase::Faulted);
        return SubmitStatus::Faulted;
    }
    if state
        .submitted_count
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        })
        .is_err()
    {
        state.set_phase(SchedulerPhase::Faulted);
        return SubmitStatus::Faulted;
    }
    SubmitStatus::Submitted
}

fn install_scheduler_panic_redaction() {
    INSTALL_SCHEDULER_PANIC_REDACTION.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |information| {
            let redact = REDACT_SCHEDULER_PANIC.try_with(Cell::get).unwrap_or(false);
            if !redact {
                previous(information);
            }
        }));
    });
}
