use std::cell::Cell;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex, Once};
use std::thread::{Builder, JoinHandle};
use std::time::Duration;

use tokenmaster_engine::{
    Clock, RefreshUrgency, RefreshWorker, WorkerCompletion, WorkerError, WorkerErrorCode,
    WorkerPhase,
};
use tokenmaster_platform::PowerLifecycleEvent;

use super::execution::{
    BenefitReminderAcknowledger, BenefitReminderExecution, BenefitReminderWallClock,
    NotificationSlot, SystemBenefitReminderWallClock,
};
use super::health::{
    BenefitReminderRefreshSnapshot, BenefitReminderRetryMode, BenefitReminderRuntimePhase,
    BenefitReminderRuntimeSnapshot, BenefitReminderSchedulePhase, BenefitReminderScheduleSnapshot,
};
use super::{BenefitReminderDelivery, BenefitReminderRuntimeConfig};
use crate::{RuntimeError, RuntimeErrorCode, SystemClock};

const ACCELERATED_RETRY_MILLIS: i64 = 60_000;

thread_local! {
    static REDACT_REMINDER_SCHEDULER_PANIC: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_REMINDER_SCHEDULER_PANIC_REDACTION: Once = Once::new();

struct ReminderScheduleState {
    phase: BenefitReminderSchedulePhase,
    pending_urgency: Option<RefreshUrgency>,
    in_flight: bool,
    notification_pending: bool,
    nearest_due_at_ms: Option<i64>,
    retry_at_ms: Option<i64>,
    accepted_hint_count: u64,
    submitted_count: u64,
}

pub(super) struct ReminderScheduleControl {
    state: Mutex<ReminderScheduleState>,
    wake: Condvar,
}

impl ReminderScheduleControl {
    fn new() -> Self {
        Self {
            state: Mutex::new(ReminderScheduleState {
                phase: BenefitReminderSchedulePhase::Paused,
                pending_urgency: None,
                in_flight: false,
                notification_pending: false,
                nearest_due_at_ms: None,
                retry_at_ms: None,
                accepted_hint_count: 0,
                submitted_count: 0,
            }),
            wake: Condvar::new(),
        }
    }

    fn force_reconcile(&self, urgency: RefreshUrgency) -> Result<(), RuntimeError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        match state.phase {
            BenefitReminderSchedulePhase::Running | BenefitReminderSchedulePhase::Paused => {}
            BenefitReminderSchedulePhase::Faulted => {
                return Err(RuntimeError::new(RuntimeErrorCode::Faulted));
            }
            BenefitReminderSchedulePhase::Stopping | BenefitReminderSchedulePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        state.pending_urgency = Some(
            state
                .pending_urgency
                .map_or(urgency, |current| current.max(urgency)),
        );
        state.accepted_hint_count = state
            .accepted_hint_count
            .checked_add(1)
            .ok_or_else(|| RuntimeError::new(RuntimeErrorCode::Internal))?;
        self.wake.notify_one();
        Ok(())
    }

    pub(super) fn complete_attempt(
        &self,
        nearest_due_update: Option<Option<i64>>,
        retry_mode: BenefitReminderRetryMode,
        notification_pending: bool,
        observed_at_ms: Option<i64>,
    ) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.in_flight = false;
        if let Some(nearest_due_at_ms) = nearest_due_update {
            state.nearest_due_at_ms = nearest_due_at_ms;
        }
        state.retry_at_ms = match (retry_mode, observed_at_ms) {
            (BenefitReminderRetryMode::Accelerated, Some(now)) => {
                now.checked_add(ACCELERATED_RETRY_MILLIS)
            }
            _ => None,
        };
        state.notification_pending |= notification_pending;
        self.wake.notify_one();
    }

    fn notification_acknowledged(&self) -> Result<(), RuntimeError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        state.notification_pending = false;
        if matches!(
            state.phase,
            BenefitReminderSchedulePhase::Running | BenefitReminderSchedulePhase::Paused
        ) {
            state.pending_urgency = Some(
                state
                    .pending_urgency
                    .map_or(RefreshUrgency::Hint, |current| {
                        current.max(RefreshUrgency::Hint)
                    }),
            );
            state.accepted_hint_count = state
                .accepted_hint_count
                .checked_add(1)
                .ok_or_else(|| RuntimeError::new(RuntimeErrorCode::Internal))?;
        }
        self.wake.notify_one();
        Ok(())
    }

    fn pause(&self) -> Result<(), RuntimeError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        match state.phase {
            BenefitReminderSchedulePhase::Running => {
                state.phase = BenefitReminderSchedulePhase::Paused;
                self.wake.notify_one();
                Ok(())
            }
            BenefitReminderSchedulePhase::Paused => Ok(()),
            BenefitReminderSchedulePhase::Faulted => {
                Err(RuntimeError::new(RuntimeErrorCode::Faulted))
            }
            BenefitReminderSchedulePhase::Stopping | BenefitReminderSchedulePhase::Stopped => {
                Err(RuntimeError::new(RuntimeErrorCode::Closed))
            }
        }
    }

    fn resume(&self) -> Result<(), RuntimeError> {
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
            match state.phase {
                BenefitReminderSchedulePhase::Paused => {
                    state.phase = BenefitReminderSchedulePhase::Running;
                }
                BenefitReminderSchedulePhase::Running => return Ok(()),
                BenefitReminderSchedulePhase::Faulted => {
                    return Err(RuntimeError::new(RuntimeErrorCode::Faulted));
                }
                BenefitReminderSchedulePhase::Stopping | BenefitReminderSchedulePhase::Stopped => {
                    return Err(RuntimeError::new(RuntimeErrorCode::Closed));
                }
            }
        }
        self.force_reconcile(RefreshUrgency::Recovery)
    }

    fn stop(&self) {
        if let Ok(mut state) = self.state.lock() {
            if !matches!(
                state.phase,
                BenefitReminderSchedulePhase::Stopped | BenefitReminderSchedulePhase::Faulted
            ) {
                state.phase = BenefitReminderSchedulePhase::Stopping;
            }
            self.wake.notify_one();
        }
    }

    fn snapshot(&self) -> Result<BenefitReminderScheduleSnapshot, RuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        Ok(BenefitReminderScheduleSnapshot {
            phase: state.phase,
            reconciliation_pending: state.pending_urgency.is_some() || state.in_flight,
            notification_pending: state.notification_pending,
            nearest_due_at_ms: state.nearest_due_at_ms,
            retry_at_ms: state.retry_at_ms,
            accepted_hint_count: state.accepted_hint_count,
            submitted_count: state.submitted_count,
        })
    }

    fn fault(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.phase = BenefitReminderSchedulePhase::Faulted;
            state.in_flight = false;
            self.wake.notify_one();
        }
    }
}

struct ReminderScheduler {
    control: Arc<ReminderScheduleControl>,
    thread: Option<JoinHandle<()>>,
}

impl ReminderScheduler {
    fn spawn<F>(
        control: Arc<ReminderScheduleControl>,
        clock: Arc<dyn BenefitReminderWallClock>,
        submit: F,
    ) -> Result<Self, RuntimeError>
    where
        F: FnMut(RefreshUrgency) -> Result<bool, ()> + Send + 'static,
    {
        install_reminder_scheduler_panic_redaction();
        let thread_control = Arc::clone(&control);
        let recovery = Arc::clone(&control);
        let thread = Builder::new()
            .name(String::from("tokenmaster-reminder-scheduler"))
            .spawn(move || {
                REDACT_REMINDER_SCHEDULER_PANIC.with(|redact| redact.set(true));
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_scheduler(thread_control, clock, submit);
                }))
                .is_err()
                {
                    recovery.fault();
                }
            })
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::ProviderUnavailable))?;
        Ok(Self {
            control,
            thread: Some(thread),
        })
    }

    fn shutdown(&mut self) -> Result<BenefitReminderSchedulePhase, RuntimeError> {
        if self.thread.is_none() {
            return Ok(self.control.snapshot()?.phase());
        }
        self.control.stop();
        let thread = self
            .thread
            .take()
            .ok_or_else(|| RuntimeError::new(RuntimeErrorCode::Internal))?;
        thread
            .join()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        self.control.snapshot().map(|snapshot| snapshot.phase())
    }
}

impl Drop for ReminderScheduler {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_scheduler<F>(
    control: Arc<ReminderScheduleControl>,
    clock: Arc<dyn BenefitReminderWallClock>,
    mut submit: F,
) where
    F: FnMut(RefreshUrgency) -> Result<bool, ()>,
{
    loop {
        let (urgency, should_stop) = {
            let mut state = match control.state.lock() {
                Ok(state) => state,
                Err(_) => {
                    control.fault();
                    return;
                }
            };
            loop {
                match state.phase {
                    BenefitReminderSchedulePhase::Stopping => {
                        state.phase = BenefitReminderSchedulePhase::Stopped;
                        control.wake.notify_one();
                        break (RefreshUrgency::Hint, true);
                    }
                    BenefitReminderSchedulePhase::Stopped
                    | BenefitReminderSchedulePhase::Faulted => {
                        break (RefreshUrgency::Hint, true);
                    }
                    BenefitReminderSchedulePhase::Paused => {
                        state = match control.wake.wait(state) {
                            Ok(state) => state,
                            Err(_) => {
                                control.fault();
                                return;
                            }
                        };
                    }
                    BenefitReminderSchedulePhase::Running
                        if state.notification_pending || state.in_flight =>
                    {
                        state = match control.wake.wait(state) {
                            Ok(state) => state,
                            Err(_) => {
                                control.fault();
                                return;
                            }
                        };
                    }
                    BenefitReminderSchedulePhase::Running => {
                        if let Some(urgency) = state.pending_urgency.take() {
                            state.in_flight = true;
                            break (urgency, false);
                        }
                        let now = match clock.now_millis() {
                            Ok(now) if now > 0 => now,
                            _ => {
                                state.phase = BenefitReminderSchedulePhase::Faulted;
                                return;
                            }
                        };
                        let next_at = match (state.nearest_due_at_ms, state.retry_at_ms) {
                            (Some(due), Some(retry)) => Some(due.min(retry)),
                            (Some(due), None) => Some(due),
                            (None, Some(retry)) => Some(retry),
                            (None, None) => None,
                        };
                        let Some(next_at) = next_at else {
                            state = match control.wake.wait(state) {
                                Ok(state) => state,
                                Err(_) => {
                                    control.fault();
                                    return;
                                }
                            };
                            continue;
                        };
                        if next_at <= now {
                            state.in_flight = true;
                            break (RefreshUrgency::Periodic, false);
                        }
                        let wait_millis = u64::try_from(next_at.saturating_sub(now)).unwrap_or(0);
                        let (next_state, _) = match control
                            .wake
                            .wait_timeout(state, Duration::from_millis(wait_millis))
                        {
                            Ok(result) => result,
                            Err(_) => {
                                control.fault();
                                return;
                            }
                        };
                        state = next_state;
                    }
                }
            }
        };
        if should_stop {
            return;
        }
        match submit(urgency) {
            Ok(true) => {
                if let Ok(mut state) = control.state.lock() {
                    let Some(submitted_count) = state.submitted_count.checked_add(1) else {
                        state.phase = BenefitReminderSchedulePhase::Faulted;
                        return;
                    };
                    state.submitted_count = submitted_count;
                } else {
                    control.fault();
                    return;
                }
            }
            Ok(false) => {
                if let Ok(mut state) = control.state.lock() {
                    state.in_flight = false;
                    control.wake.notify_one();
                } else {
                    control.fault();
                    return;
                }
            }
            Err(()) => {
                control.fault();
                return;
            }
        }
    }
}

pub struct BenefitReminderRuntime {
    phase: BenefitReminderRuntimePhase,
    scheduler: ReminderScheduler,
    control: Arc<ReminderScheduleControl>,
    worker: Arc<RefreshWorker>,
    admission_open: Arc<Mutex<bool>>,
    latest: Arc<Mutex<BenefitReminderRefreshSnapshot>>,
    notifications: Arc<NotificationSlot>,
    acknowledger: Mutex<BenefitReminderAcknowledger>,
}

impl BenefitReminderRuntime {
    pub fn start(config: BenefitReminderRuntimeConfig) -> Result<Self, RuntimeError> {
        let monotonic_clock: Arc<dyn Clock> = SystemClock::shared();
        let wall_clock: Arc<dyn BenefitReminderWallClock> =
            Arc::new(SystemBenefitReminderWallClock);
        let acknowledger = Mutex::new(BenefitReminderAcknowledger::new(
            Arc::clone(&wall_clock),
            config.archive_path(),
        )?);
        let latest = Arc::new(Mutex::new(BenefitReminderRefreshSnapshot::not_run()));
        let notifications = Arc::new(NotificationSlot::new());
        let control = Arc::new(ReminderScheduleControl::new());
        let mut execution = BenefitReminderExecution::new(
            Arc::clone(&monotonic_clock),
            Arc::clone(&wall_clock),
            config.archive_path(),
            Arc::clone(&latest),
            Arc::clone(&notifications),
            Arc::clone(&control),
        )?;
        let worker = Arc::new(
            RefreshWorker::spawn(monotonic_clock, move |permit| execution.run(permit))
                .map_err(runtime_worker_error)?,
        );
        let admission_open = Arc::new(Mutex::new(false));
        let scheduler_worker = Arc::clone(&worker);
        let scheduler_admission = Arc::clone(&admission_open);
        let scheduler =
            ReminderScheduler::spawn(Arc::clone(&control), wall_clock, move |urgency| {
                let admission = scheduler_admission.lock().map_err(|_| ())?;
                if !*admission {
                    return Ok(false);
                }
                scheduler_worker
                    .submit(urgency, None)
                    .map(|_admission| true)
                    .map_err(|_| ())
            })?;
        *admission_open
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = true;
        control.resume()?;
        Ok(Self {
            phase: BenefitReminderRuntimePhase::Running,
            scheduler,
            control,
            worker,
            admission_open,
            latest,
            notifications,
            acknowledger,
        })
    }

    pub fn reconcile_now(&self) -> Result<(), RuntimeError> {
        self.force_reconcile(RefreshUrgency::Interactive)
    }

    pub fn notify_inventory_changed(&self) -> Result<(), RuntimeError> {
        self.force_reconcile(RefreshUrgency::Hint)
    }

    pub fn notify_profile_changed(&self) -> Result<(), RuntimeError> {
        self.force_reconcile(RefreshUrgency::Hint)
    }

    pub fn notify_clock_changed(&self) -> Result<(), RuntimeError> {
        self.force_reconcile(RefreshUrgency::Recovery)
    }

    fn force_reconcile(&self, urgency: RefreshUrgency) -> Result<(), RuntimeError> {
        if self.phase != BenefitReminderRuntimePhase::Running {
            return Err(RuntimeError::new(match self.phase {
                BenefitReminderRuntimePhase::Faulted => RuntimeErrorCode::Faulted,
                BenefitReminderRuntimePhase::Running => RuntimeErrorCode::Internal,
                BenefitReminderRuntimePhase::Paused
                | BenefitReminderRuntimePhase::Stopping
                | BenefitReminderRuntimePhase::Stopped => RuntimeErrorCode::Closed,
            }));
        }
        self.control.force_reconcile(urgency)
    }

    pub fn try_completion(&self) -> Result<Option<WorkerCompletion>, RuntimeError> {
        self.worker.try_completion().map_err(runtime_worker_error)
    }

    pub fn take_notifications(
        &self,
    ) -> Result<Option<Box<[BenefitReminderDelivery]>>, RuntimeError> {
        let batch = self
            .notifications
            .take_for_presentation()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        Ok(batch)
    }

    pub fn acknowledge_notifications(&self) -> Result<bool, RuntimeError> {
        if self.phase != BenefitReminderRuntimePhase::Running {
            return Err(RuntimeError::new(match self.phase {
                BenefitReminderRuntimePhase::Faulted => RuntimeErrorCode::Faulted,
                BenefitReminderRuntimePhase::Running => RuntimeErrorCode::Internal,
                BenefitReminderRuntimePhase::Paused
                | BenefitReminderRuntimePhase::Stopping
                | BenefitReminderRuntimePhase::Stopped => RuntimeErrorCode::Closed,
            }));
        }
        let Some(batch) = self
            .notifications
            .begin_acknowledgement()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?
        else {
            return Ok(false);
        };
        let acknowledgement = self
            .acknowledger
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?
            .acknowledge(&batch);
        let committed = acknowledgement.is_ok();
        self.notifications
            .finish_acknowledgement(committed)
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        acknowledgement?;
        self.control.notification_acknowledged()?;
        Ok(true)
    }

    pub fn release_notifications(&self) -> Result<bool, RuntimeError> {
        self.notifications
            .release_presentation()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))
    }

    pub fn snapshot(&self) -> Result<BenefitReminderRuntimeSnapshot, RuntimeError> {
        let schedule = self.control.snapshot()?;
        let worker = self.worker.snapshot().map_err(runtime_worker_error)?;
        let refresh = *self
            .latest
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        let phase = if schedule.phase() == BenefitReminderSchedulePhase::Faulted
            || worker.phase() == WorkerPhase::Faulted
        {
            BenefitReminderRuntimePhase::Faulted
        } else {
            self.phase
        };
        Ok(BenefitReminderRuntimeSnapshot {
            phase,
            schedule,
            worker,
            refresh,
        })
    }

    pub fn pause(&mut self) -> Result<BenefitReminderRuntimePhase, RuntimeError> {
        match self.phase {
            BenefitReminderRuntimePhase::Paused => return Ok(self.phase),
            BenefitReminderRuntimePhase::Running => {}
            BenefitReminderRuntimePhase::Faulted => {
                return Err(RuntimeError::new(RuntimeErrorCode::Faulted));
            }
            BenefitReminderRuntimePhase::Stopping | BenefitReminderRuntimePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        *self
            .admission_open
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = false;
        self.control.pause()?;
        let worker = self.worker.snapshot().map_err(runtime_worker_error)?;
        if let Some(active) = worker.active_request_id()
            && let Err(error) = self.worker.cancel(active)
            && error.code() != WorkerErrorCode::StaleRequest
        {
            self.phase = BenefitReminderRuntimePhase::Faulted;
            return Err(runtime_worker_error(error));
        }
        self.phase = BenefitReminderRuntimePhase::Paused;
        Ok(self.phase)
    }

    pub fn resume(&mut self) -> Result<BenefitReminderRuntimePhase, RuntimeError> {
        match self.phase {
            BenefitReminderRuntimePhase::Running => return Ok(self.phase),
            BenefitReminderRuntimePhase::Paused => {}
            BenefitReminderRuntimePhase::Faulted => {
                return Err(RuntimeError::new(RuntimeErrorCode::Faulted));
            }
            BenefitReminderRuntimePhase::Stopping | BenefitReminderRuntimePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        *self
            .admission_open
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = true;
        if let Err(error) = self.control.resume() {
            if let Ok(mut admission) = self.admission_open.lock() {
                *admission = false;
            }
            self.phase = BenefitReminderRuntimePhase::Faulted;
            return Err(error);
        }
        self.phase = BenefitReminderRuntimePhase::Running;
        Ok(self.phase)
    }

    pub fn apply_power_event(
        &mut self,
        event: PowerLifecycleEvent,
    ) -> Result<BenefitReminderRuntimePhase, RuntimeError> {
        match event {
            PowerLifecycleEvent::Suspend => self.pause(),
            PowerLifecycleEvent::Resume if self.phase == BenefitReminderRuntimePhase::Running => {
                self.control.force_reconcile(RefreshUrgency::Recovery)?;
                Ok(self.phase)
            }
            PowerLifecycleEvent::Resume => self.resume(),
        }
    }

    pub fn shutdown(&mut self) -> Result<BenefitReminderRuntimePhase, RuntimeError> {
        if self.phase == BenefitReminderRuntimePhase::Stopped {
            return Ok(self.phase);
        }
        self.phase = BenefitReminderRuntimePhase::Stopping;
        let mut failed = false;
        match self.admission_open.lock() {
            Ok(mut admission) => *admission = false,
            Err(poisoned) => {
                *poisoned.into_inner() = false;
                failed = true;
            }
        }
        let scheduler_phase = match self.scheduler.shutdown() {
            Ok(phase) => phase,
            Err(_) => {
                failed = true;
                BenefitReminderSchedulePhase::Faulted
            }
        };
        let worker_phase = match Arc::get_mut(&mut self.worker) {
            Some(worker) => match worker.shutdown() {
                Ok(phase) => phase,
                Err(_) => {
                    failed = true;
                    WorkerPhase::Faulted
                }
            },
            None => {
                failed = true;
                WorkerPhase::Faulted
            }
        };
        if failed
            || scheduler_phase == BenefitReminderSchedulePhase::Faulted
            || worker_phase == WorkerPhase::Faulted
        {
            self.phase = BenefitReminderRuntimePhase::Faulted;
            Err(RuntimeError::new(RuntimeErrorCode::Internal))
        } else {
            self.phase = BenefitReminderRuntimePhase::Stopped;
            Ok(self.phase)
        }
    }
}

impl fmt::Debug for BenefitReminderRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitReminderRuntime")
            .field("snapshot", &self.snapshot().ok())
            .finish()
    }
}

impl Drop for BenefitReminderRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn runtime_worker_error(error: WorkerError) -> RuntimeError {
    let code = match error.code() {
        WorkerErrorCode::Closed | WorkerErrorCode::StaleRequest => RuntimeErrorCode::Closed,
        WorkerErrorCode::Faulted => RuntimeErrorCode::Faulted,
        WorkerErrorCode::CapacityExceeded => RuntimeErrorCode::Busy,
        WorkerErrorCode::Unavailable => RuntimeErrorCode::ProviderUnavailable,
        WorkerErrorCode::Internal => RuntimeErrorCode::Internal,
    };
    RuntimeError::new(code)
}

fn install_reminder_scheduler_panic_redaction() {
    INSTALL_REMINDER_SCHEDULER_PANIC_REDACTION.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |information| {
            let redact = REDACT_REMINDER_SCHEDULER_PANIC
                .try_with(Cell::get)
                .unwrap_or(false);
            if !redact {
                previous(information);
            }
        }));
    });
}

const _: () = assert!(tokenmaster_store::MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE <= u16::MAX as usize);

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, channel},
    };
    use std::time::Duration;

    use tokenmaster_engine::RefreshUrgency;

    use super::*;

    #[derive(Default)]
    struct FakeWallClock {
        now_ms: AtomicI64,
    }

    impl FakeWallClock {
        fn set(&self, now_ms: i64) {
            self.now_ms.store(now_ms, Ordering::Release);
        }
    }

    impl BenefitReminderWallClock for FakeWallClock {
        fn now_millis(&self) -> Result<i64, ()> {
            Ok(self.now_ms.load(Ordering::Acquire))
        }
    }

    fn receive(receiver: &Receiver<RefreshUrgency>) -> RefreshUrgency {
        receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|_| panic!("scheduler submission timed out"))
    }

    fn assert_no_submission(receiver: &Receiver<RefreshUrgency>) {
        assert_eq!(
            receiver.recv_timeout(Duration::from_millis(40)),
            Err(RecvTimeoutError::Timeout)
        );
    }

    #[test]
    fn scheduler_waits_for_nearest_due_and_notification_backpressure() {
        let clock = Arc::new(FakeWallClock::default());
        clock.set(1_000);
        let control = Arc::new(ReminderScheduleControl::new());
        let (sender, receiver) = channel();
        let mut scheduler =
            ReminderScheduler::spawn(Arc::clone(&control), clock.clone(), move |urgency| {
                sender.send(urgency).map(|()| true).map_err(|_| ())
            })
            .expect("scheduler");
        control.resume().expect("resume");
        assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

        control.complete_attempt(
            Some(Some(5_000)),
            BenefitReminderRetryMode::Normal,
            true,
            Some(1_000),
        );
        clock.set(5_000);
        control.wake.notify_one();
        assert_no_submission(&receiver);

        control
            .notification_acknowledged()
            .expect("consume notification");
        assert_eq!(receive(&receiver), RefreshUrgency::Hint);
        control.complete_attempt(
            Some(Some(5_000)),
            BenefitReminderRetryMode::Normal,
            false,
            Some(5_000),
        );
        assert_eq!(receive(&receiver), RefreshUrgency::Periodic);
        control.complete_attempt(
            Some(None),
            BenefitReminderRetryMode::Normal,
            false,
            Some(5_000),
        );
        assert_no_submission(&receiver);
        assert_eq!(
            scheduler.shutdown().expect("shutdown"),
            BenefitReminderSchedulePhase::Stopped
        );
    }

    #[test]
    fn scheduler_coalesces_bursts_and_uses_one_accelerated_retry_deadline() {
        let clock = Arc::new(FakeWallClock::default());
        clock.set(10_000);
        let control = Arc::new(ReminderScheduleControl::new());
        let (sender, receiver) = channel();
        let mut scheduler =
            ReminderScheduler::spawn(Arc::clone(&control), clock.clone(), move |urgency| {
                sender.send(urgency).map(|()| true).map_err(|_| ())
            })
            .expect("scheduler");
        control.resume().expect("resume");
        assert_eq!(receive(&receiver), RefreshUrgency::Recovery);
        for _ in 0..10_000 {
            control
                .force_reconcile(RefreshUrgency::Hint)
                .expect("coalesced hint");
        }
        assert_no_submission(&receiver);
        control.complete_attempt(
            None,
            BenefitReminderRetryMode::Accelerated,
            false,
            Some(10_000),
        );
        assert_eq!(receive(&receiver), RefreshUrgency::Hint);
        control.complete_attempt(
            None,
            BenefitReminderRetryMode::Accelerated,
            false,
            Some(10_000),
        );
        clock.set(69_999);
        control.wake.notify_one();
        assert_no_submission(&receiver);
        clock.set(70_000);
        control.wake.notify_one();
        assert_eq!(receive(&receiver), RefreshUrgency::Periodic);
        control.complete_attempt(
            Some(None),
            BenefitReminderRetryMode::Normal,
            false,
            Some(70_000),
        );
        scheduler.shutdown().expect("shutdown");
    }
}
