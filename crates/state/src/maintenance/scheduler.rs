use core::fmt;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, Instant};

use crate::{BackupPolicy, StateError};

use super::worker::MaintenanceSubmitter;
use super::{MaintenanceAdmission, MaintenancePurpose, MaintenanceSourceState};

const MAX_SCHEDULER_SLEEP_MILLIS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MaintenanceTick(u64);

impl MaintenanceTick {
    #[must_use]
    pub const fn from_millis(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceScheduleSnapshot {
    periodic_enabled: bool,
    healthy_publication_seen: bool,
    dirty: bool,
    paused: bool,
    catch_up_due: bool,
}

impl MaintenanceScheduleSnapshot {
    #[must_use]
    pub const fn periodic_enabled(self) -> bool {
        self.periodic_enabled
    }

    #[must_use]
    pub const fn healthy_publication_seen(self) -> bool {
        self.healthy_publication_seen
    }

    #[must_use]
    pub const fn dirty(self) -> bool {
        self.dirty
    }

    #[must_use]
    pub const fn paused(self) -> bool {
        self.paused
    }

    #[must_use]
    pub const fn catch_up_due(self) -> bool {
        self.catch_up_due
    }
}

/// Constant-size automatic-backup scheduling state.
pub struct MaintenanceSchedule {
    periodic_enabled: bool,
    quiet_millis: u64,
    interval_millis: u64,
    healthy_publication_seen: bool,
    last_submission: Option<u64>,
    last_change: Option<u64>,
    last_observed: u64,
    pause_started: Option<u64>,
    catch_up_due: bool,
    paused: bool,
}

impl MaintenanceSchedule {
    #[must_use]
    pub fn new(
        policy: &BackupPolicy,
        now: MaintenanceTick,
        source_state: MaintenanceSourceState,
    ) -> Self {
        let healthy_publication_seen = source_state == MaintenanceSourceState::Healthy;
        Self {
            periodic_enabled: policy.periodic_enabled(),
            quiet_millis: u64::from(policy.quiet_seconds()) * 1_000,
            interval_millis: u64::from(policy.interval_seconds()) * 1_000,
            healthy_publication_seen,
            last_submission: healthy_publication_seen.then_some(now.as_millis()),
            last_change: None,
            last_observed: now.as_millis(),
            pause_started: None,
            catch_up_due: false,
            paused: false,
        }
    }

    pub fn update_policy(&mut self, policy: &BackupPolicy) {
        self.periodic_enabled = policy.periodic_enabled();
        self.quiet_millis = u64::from(policy.quiet_seconds()) * 1_000;
        self.interval_millis = u64::from(policy.interval_seconds()) * 1_000;
        if !self.periodic_enabled {
            self.catch_up_due = false;
        }
    }

    pub fn mark_healthy_publication(&mut self, now: MaintenanceTick) {
        self.last_observed = now.as_millis();
        self.last_submission = Some(now.as_millis());
        self.last_change = None;
        self.catch_up_due = false;
        self.healthy_publication_seen = true;
    }

    pub fn record_durable_change(&mut self, now: MaintenanceTick) {
        if now.as_millis() < self.last_observed {
            self.catch_up_due = true;
        }
        self.last_observed = now.as_millis();
        self.last_change = Some(now.as_millis());
    }

    pub fn pause(&mut self, now: MaintenanceTick) {
        self.observe_without_submission(now);
        self.paused = true;
        self.pause_started = Some(now.as_millis());
    }

    pub fn resume(&mut self, now: MaintenanceTick) {
        let now_millis = now.as_millis();
        if now_millis < self.last_observed
            || self
                .last_submission
                .is_some_and(|last| now_millis.saturating_sub(last) >= self.interval_millis)
        {
            self.catch_up_due = true;
        }
        self.last_observed = now_millis;
        self.pause_started = None;
        self.paused = false;
    }

    pub fn poll(&mut self, now: MaintenanceTick) -> Option<MaintenancePurpose> {
        let now_millis = now.as_millis();
        if now_millis < self.last_observed {
            self.last_observed = now_millis;
            self.catch_up_due = true;
            return None;
        }
        self.last_observed = now_millis;
        if self.paused || !self.periodic_enabled || !self.healthy_publication_seen {
            return None;
        }
        if self.catch_up_due {
            self.catch_up_due = false;
            self.note_submission(now_millis);
            return Some(MaintenancePurpose::Periodic);
        }
        let last_submission = self.last_submission?;
        if now_millis.saturating_sub(last_submission) < self.interval_millis {
            return None;
        }
        let quiet = self
            .last_change
            .is_none_or(|changed| now_millis.saturating_sub(changed) >= self.quiet_millis);
        if !quiet {
            return None;
        }
        self.note_submission(now_millis);
        Some(MaintenancePurpose::Periodic)
    }

    #[must_use]
    pub fn next_wake_millis(&self, now: MaintenanceTick) -> u64 {
        if self.paused
            || !self.periodic_enabled
            || !self.healthy_publication_seen
            || self.catch_up_due
        {
            return if self.catch_up_due {
                0
            } else {
                MAX_SCHEDULER_SLEEP_MILLIS
            };
        }
        let now_millis = now.as_millis();
        let interval_due = self
            .last_submission
            .and_then(|last| last.checked_add(self.interval_millis))
            .unwrap_or(now_millis);
        let quiet_due = self
            .last_change
            .and_then(|last| last.checked_add(self.quiet_millis))
            .unwrap_or(now_millis);
        interval_due
            .max(quiet_due)
            .saturating_sub(now_millis)
            .min(MAX_SCHEDULER_SLEEP_MILLIS)
    }

    #[must_use]
    pub const fn snapshot(&self) -> MaintenanceScheduleSnapshot {
        MaintenanceScheduleSnapshot {
            periodic_enabled: self.periodic_enabled,
            healthy_publication_seen: self.healthy_publication_seen,
            dirty: self.last_change.is_some(),
            paused: self.paused,
            catch_up_due: self.catch_up_due,
        }
    }

    fn note_submission(&mut self, now_millis: u64) {
        self.last_submission = Some(now_millis);
        self.last_change = None;
    }

    fn observe_without_submission(&mut self, now: MaintenanceTick) {
        if now.as_millis() < self.last_observed {
            self.catch_up_due = true;
        }
        self.last_observed = now.as_millis();
    }
}

impl fmt::Debug for MaintenanceSchedule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaintenanceSchedule")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

pub trait MaintenanceClock: Send + Sync + 'static {
    fn now(&self) -> MaintenanceTick;
}

pub struct SystemMaintenanceClock {
    started: Instant,
}

impl SystemMaintenanceClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Default for SystemMaintenanceClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MaintenanceClock for SystemMaintenanceClock {
    fn now(&self) -> MaintenanceTick {
        let elapsed = self.started.elapsed().as_millis();
        MaintenanceTick::from_millis(u64::try_from(elapsed).unwrap_or(u64::MAX))
    }
}

impl fmt::Debug for SystemMaintenanceClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SystemMaintenanceClock([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceSchedulerPhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceSchedulerSnapshot {
    phase: MaintenanceSchedulerPhase,
    schedule: MaintenanceScheduleSnapshot,
    submitted_count: u64,
}

impl MaintenanceSchedulerSnapshot {
    #[must_use]
    pub const fn phase(self) -> MaintenanceSchedulerPhase {
        self.phase
    }

    #[must_use]
    pub const fn schedule(self) -> MaintenanceScheduleSnapshot {
        self.schedule
    }

    #[must_use]
    pub const fn submitted_count(self) -> u64 {
        self.submitted_count
    }
}

pub(crate) type SharedMaintenanceSchedule = Arc<Mutex<MaintenanceSchedule>>;

struct SchedulerRuntimeState {
    phase: MaintenanceSchedulerPhase,
    submitted_count: u64,
}

#[derive(Clone, Copy)]
enum SchedulerWake {
    Signal,
}

pub struct MaintenanceScheduler {
    clock: Arc<dyn MaintenanceClock>,
    schedule: SharedMaintenanceSchedule,
    state: Arc<Mutex<SchedulerRuntimeState>>,
    wake_sender: SyncSender<SchedulerWake>,
    thread: Option<JoinHandle<()>>,
}

impl MaintenanceScheduler {
    pub(crate) fn spawn(
        clock: Arc<dyn MaintenanceClock>,
        schedule: SharedMaintenanceSchedule,
        submitter: MaintenanceSubmitter,
    ) -> Result<Self, StateError> {
        let state = Arc::new(Mutex::new(SchedulerRuntimeState {
            phase: MaintenanceSchedulerPhase::Running,
            submitted_count: 0,
        }));
        let (wake_sender, wake_receiver) = sync_channel(1);
        let thread_clock = Arc::clone(&clock);
        let thread_schedule = Arc::clone(&schedule);
        let thread_state = Arc::clone(&state);
        let thread = Builder::new()
            .name(String::from("tokenmaster-backup-scheduler"))
            .spawn(move || {
                run_scheduler(
                    thread_clock,
                    thread_schedule,
                    thread_state,
                    wake_receiver,
                    submitter,
                );
            })
            .map_err(|_| StateError::unavailable())?;
        Ok(Self {
            clock,
            schedule,
            state,
            wake_sender,
            thread: Some(thread),
        })
    }

    pub fn record_durable_change(&self) -> Result<(), StateError> {
        self.schedule
            .lock()
            .map_err(|_| StateError::internal_invariant())?
            .record_durable_change(self.clock.now());
        self.wake()
    }

    pub fn update_policy(&self, policy: &BackupPolicy) -> Result<(), StateError> {
        self.schedule
            .lock()
            .map_err(|_| StateError::internal_invariant())?
            .update_policy(policy);
        self.wake()
    }

    #[must_use]
    pub fn snapshot(&self) -> MaintenanceSchedulerSnapshot {
        let schedule = self.schedule.lock().map_or(
            MaintenanceScheduleSnapshot {
                periodic_enabled: false,
                healthy_publication_seen: false,
                dirty: false,
                paused: false,
                catch_up_due: false,
            },
            |schedule| schedule.snapshot(),
        );
        self.state.lock().map_or(
            MaintenanceSchedulerSnapshot {
                phase: MaintenanceSchedulerPhase::Faulted,
                schedule,
                submitted_count: 0,
            },
            |state| MaintenanceSchedulerSnapshot {
                phase: state.phase,
                schedule,
                submitted_count: state.submitted_count,
            },
        )
    }

    pub fn pause(&self) -> Result<MaintenanceSchedulerPhase, StateError> {
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| StateError::internal_invariant())?;
            match state.phase {
                MaintenanceSchedulerPhase::Running => {
                    state.phase = MaintenanceSchedulerPhase::Paused;
                }
                MaintenanceSchedulerPhase::Paused => return Ok(MaintenanceSchedulerPhase::Paused),
                MaintenanceSchedulerPhase::Faulted => {
                    return Err(StateError::internal_invariant());
                }
                MaintenanceSchedulerPhase::Stopping | MaintenanceSchedulerPhase::Stopped => {
                    return Err(StateError::unavailable());
                }
            }
        }
        self.schedule
            .lock()
            .map_err(|_| StateError::internal_invariant())?
            .pause(self.clock.now());
        self.wake()?;
        Ok(MaintenanceSchedulerPhase::Paused)
    }

    pub fn resume(&self) -> Result<MaintenanceSchedulerPhase, StateError> {
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| StateError::internal_invariant())?;
            match state.phase {
                MaintenanceSchedulerPhase::Paused => {
                    state.phase = MaintenanceSchedulerPhase::Running;
                }
                MaintenanceSchedulerPhase::Running => {
                    return Ok(MaintenanceSchedulerPhase::Running);
                }
                MaintenanceSchedulerPhase::Faulted => {
                    return Err(StateError::internal_invariant());
                }
                MaintenanceSchedulerPhase::Stopping | MaintenanceSchedulerPhase::Stopped => {
                    return Err(StateError::unavailable());
                }
            }
        }
        self.schedule
            .lock()
            .map_err(|_| StateError::internal_invariant())?
            .resume(self.clock.now());
        self.wake()?;
        Ok(MaintenanceSchedulerPhase::Running)
    }

    pub fn shutdown(&mut self) -> Result<MaintenanceSchedulerPhase, StateError> {
        let Some(thread) = self.thread.take() else {
            return Ok(self.snapshot().phase());
        };
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| StateError::internal_invariant())?;
            if state.phase != MaintenanceSchedulerPhase::Faulted {
                state.phase = MaintenanceSchedulerPhase::Stopping;
            }
        }
        let _ = self.wake_sender.try_send(SchedulerWake::Signal);
        thread
            .join()
            .map_err(|_| StateError::internal_invariant())?;
        let phase = self.snapshot().phase();
        if phase == MaintenanceSchedulerPhase::Faulted {
            Err(StateError::internal_invariant())
        } else {
            Ok(phase)
        }
    }

    fn wake(&self) -> Result<(), StateError> {
        match self.wake_sender.try_send(SchedulerWake::Signal) {
            Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
            Err(TrySendError::Disconnected(_)) => Err(StateError::unavailable()),
        }
    }
}

impl fmt::Debug for MaintenanceScheduler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaintenanceScheduler")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl Drop for MaintenanceScheduler {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_scheduler(
    clock: Arc<dyn MaintenanceClock>,
    schedule: SharedMaintenanceSchedule,
    state: Arc<Mutex<SchedulerRuntimeState>>,
    wake_receiver: Receiver<SchedulerWake>,
    submitter: MaintenanceSubmitter,
) {
    loop {
        let phase = match state.lock() {
            Ok(state) => state.phase,
            Err(_) => return,
        };
        match phase {
            MaintenanceSchedulerPhase::Stopping => {
                if let Ok(mut state) = state.lock() {
                    state.phase = MaintenanceSchedulerPhase::Stopped;
                }
                return;
            }
            MaintenanceSchedulerPhase::Stopped | MaintenanceSchedulerPhase::Faulted => return,
            MaintenanceSchedulerPhase::Paused => {
                if wake_receiver.recv().is_err() {
                    set_scheduler_phase(&state, MaintenanceSchedulerPhase::Stopped);
                    return;
                }
                continue;
            }
            MaintenanceSchedulerPhase::Running => {}
        }

        let now = clock.now();
        let (purpose, wait_millis) = match schedule.lock() {
            Ok(mut schedule) => {
                let purpose = schedule.poll(now);
                let wait = schedule.next_wake_millis(now);
                (purpose, wait)
            }
            Err(_) => {
                set_scheduler_phase(&state, MaintenanceSchedulerPhase::Faulted);
                return;
            }
        };
        if let Some(purpose) = purpose {
            match submitter.submit(purpose) {
                MaintenanceAdmission::Rejected(super::MaintenanceRejection::Closed) => {
                    set_scheduler_phase(&state, MaintenanceSchedulerPhase::Faulted);
                    return;
                }
                MaintenanceAdmission::Started(_)
                | MaintenanceAdmission::Coalesced { .. }
                | MaintenanceAdmission::Rejected(_)
                | MaintenanceAdmission::BypassedEmptyInstallation
                | MaintenanceAdmission::BypassedCorruptQuarantine => {}
            }
            let Ok(mut state) = state.lock() else {
                return;
            };
            let Some(next) = state.submitted_count.checked_add(1) else {
                state.phase = MaintenanceSchedulerPhase::Faulted;
                return;
            };
            state.submitted_count = next;
            continue;
        }
        let wait = Duration::from_millis(wait_millis.max(1));
        match wake_receiver.recv_timeout(wait) {
            Ok(SchedulerWake::Signal) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                set_scheduler_phase(&state, MaintenanceSchedulerPhase::Stopped);
                return;
            }
        }
    }
}

fn set_scheduler_phase(state: &Mutex<SchedulerRuntimeState>, phase: MaintenanceSchedulerPhase) {
    if let Ok(mut state) = state.lock() {
        state.phase = phase;
    }
}
