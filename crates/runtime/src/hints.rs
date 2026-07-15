use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, Ordering},
    mpsc::{SyncSender, TrySendError},
};

use tokenmaster_engine::{Clock, RefreshUrgency};

pub(crate) const FLAG_DIRTY: u64 = 1 << 0;
pub(crate) const FLAG_FORCE: u64 = 1 << 1;
const FLAG_URGENCY_HINT: u64 = 1 << 2;
const FLAG_URGENCY_PERIODIC: u64 = 1 << 3;
const FLAG_URGENCY_INTERACTIVE: u64 = 1 << 4;
const FLAG_URGENCY_RECOVERY: u64 = 1 << 5;
const FLAG_WATCHER_OVERFLOW: u64 = 1 << 6;
const FLAG_CLOCK_DISCONTINUITY: u64 = 1 << 7;
const INITIAL_FLAGS: u64 = FLAG_DIRTY | FLAG_FORCE | FLAG_URGENCY_RECOVERY;

const PHASE_RUNNING: u8 = 0;
const PHASE_PAUSED: u8 = 1;
const PHASE_STOPPING: u8 = 2;
const PHASE_STOPPED: u8 = 3;
const PHASE_FAULTED: u8 = 4;

const WATCHER_HEALTHY: u8 = 0;
const WATCHER_DEGRADED: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerPhase {
    Running,
    Paused,
    Stopping,
    Stopped,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatcherHealth {
    Healthy,
    Degraded,
}

#[derive(Clone, Copy)]
pub(crate) enum SchedulerWake {
    Signal,
}

pub(crate) struct HintState {
    pub(crate) flags: AtomicU64,
    pub(crate) latest_hint_tick: AtomicU64,
    pub(crate) watcher_health: AtomicU8,
    pub(crate) phase: AtomicU8,
    pub(crate) accepted_hint_count: AtomicU64,
    pub(crate) submitted_count: AtomicU64,
}

impl HintState {
    pub(crate) fn new(phase: SchedulerPhase) -> Self {
        Self {
            flags: AtomicU64::new(INITIAL_FLAGS),
            latest_hint_tick: AtomicU64::new(0),
            watcher_health: AtomicU8::new(WATCHER_HEALTHY),
            phase: AtomicU8::new(encode_phase(phase)),
            accepted_hint_count: AtomicU64::new(0),
            submitted_count: AtomicU64::new(0),
        }
    }

    pub(crate) fn phase(&self) -> SchedulerPhase {
        decode_phase(self.phase.load(Ordering::Acquire))
    }

    pub(crate) fn set_phase(&self, phase: SchedulerPhase) {
        self.phase.store(encode_phase(phase), Ordering::Release);
    }

    pub(crate) fn transition_phase(&self, current: SchedulerPhase, next: SchedulerPhase) -> bool {
        self.phase
            .compare_exchange(
                encode_phase(current),
                encode_phase(next),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub(crate) fn watcher_health(&self) -> WatcherHealth {
        decode_health(self.watcher_health.load(Ordering::Acquire))
    }

    pub(crate) fn force_clock_discontinuity(&self) {
        self.flags.fetch_or(
            FLAG_DIRTY | FLAG_FORCE | FLAG_CLOCK_DISCONTINUITY | FLAG_URGENCY_RECOVERY,
            Ordering::AcqRel,
        );
    }
}

#[derive(Clone)]
pub struct RefreshHintSink {
    state: Arc<HintState>,
    clock: Arc<dyn Clock>,
    wake_sender: SyncSender<SchedulerWake>,
}

impl RefreshHintSink {
    pub(crate) fn new(
        state: Arc<HintState>,
        clock: Arc<dyn Clock>,
        wake_sender: SyncSender<SchedulerWake>,
    ) -> Self {
        Self {
            state,
            clock,
            wake_sender,
        }
    }

    #[must_use]
    pub fn filesystem_changed(&self) -> bool {
        self.signal(FLAG_DIRTY | FLAG_URGENCY_HINT, true)
    }

    #[must_use]
    pub fn force_reconcile(&self, urgency: RefreshUrgency) -> bool {
        self.signal(FLAG_DIRTY | FLAG_FORCE | urgency_flag(urgency), true)
    }

    #[must_use]
    pub fn watcher_error(&self) -> bool {
        if self.state.phase() != SchedulerPhase::Running {
            return false;
        }
        self.state
            .watcher_health
            .store(WATCHER_DEGRADED, Ordering::Release);
        self.signal(FLAG_DIRTY | FLAG_FORCE | FLAG_URGENCY_RECOVERY, true)
    }

    #[must_use]
    pub fn watcher_rescan_required(&self) -> bool {
        if self.state.phase() != SchedulerPhase::Running {
            return false;
        }
        self.state
            .watcher_health
            .store(WATCHER_DEGRADED, Ordering::Release);
        self.signal(
            FLAG_DIRTY | FLAG_FORCE | FLAG_WATCHER_OVERFLOW | FLAG_URGENCY_RECOVERY,
            true,
        )
    }

    #[must_use]
    pub fn watcher_healthy(&self) -> bool {
        if self.state.phase() != SchedulerPhase::Running {
            return false;
        }
        self.state
            .watcher_health
            .store(WATCHER_HEALTHY, Ordering::Release);
        self.wake()
    }

    pub(crate) fn wake(&self) -> bool {
        match self.wake_sender.try_send(SchedulerWake::Signal) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    fn signal(&self, mut flags: u64, record_tick: bool) -> bool {
        if self.state.phase() != SchedulerPhase::Running {
            return false;
        }
        if record_tick {
            let tick = self.clock.now().as_millis();
            let previous = self
                .state
                .latest_hint_tick
                .fetch_max(tick, Ordering::AcqRel);
            if tick < previous {
                flags |= FLAG_FORCE | FLAG_CLOCK_DISCONTINUITY | FLAG_URGENCY_RECOVERY;
            }
        }
        if self
            .state
            .accepted_hint_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .is_err()
        {
            flags |= FLAG_FORCE | FLAG_URGENCY_RECOVERY;
        }
        self.state.flags.fetch_or(flags, Ordering::AcqRel);
        self.wake()
    }
}

impl fmt::Debug for RefreshHintSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RefreshHintSink([redacted])")
    }
}

pub(crate) fn flags_urgency(flags: u64) -> RefreshUrgency {
    if flags & FLAG_URGENCY_RECOVERY != 0 {
        RefreshUrgency::Recovery
    } else if flags & FLAG_URGENCY_INTERACTIVE != 0 {
        RefreshUrgency::Interactive
    } else if flags & FLAG_URGENCY_PERIODIC != 0 {
        RefreshUrgency::Periodic
    } else {
        RefreshUrgency::Hint
    }
}

fn urgency_flag(urgency: RefreshUrgency) -> u64 {
    match urgency {
        RefreshUrgency::Hint => FLAG_URGENCY_HINT,
        RefreshUrgency::Periodic => FLAG_URGENCY_PERIODIC,
        RefreshUrgency::Interactive => FLAG_URGENCY_INTERACTIVE,
        RefreshUrgency::Recovery => FLAG_URGENCY_RECOVERY,
    }
}

fn encode_phase(phase: SchedulerPhase) -> u8 {
    match phase {
        SchedulerPhase::Running => PHASE_RUNNING,
        SchedulerPhase::Paused => PHASE_PAUSED,
        SchedulerPhase::Stopping => PHASE_STOPPING,
        SchedulerPhase::Stopped => PHASE_STOPPED,
        SchedulerPhase::Faulted => PHASE_FAULTED,
    }
}

fn decode_phase(value: u8) -> SchedulerPhase {
    match value {
        PHASE_RUNNING => SchedulerPhase::Running,
        PHASE_PAUSED => SchedulerPhase::Paused,
        PHASE_STOPPING => SchedulerPhase::Stopping,
        PHASE_STOPPED => SchedulerPhase::Stopped,
        _ => SchedulerPhase::Faulted,
    }
}

fn decode_health(value: u8) -> WatcherHealth {
    if value == WATCHER_HEALTHY {
        WatcherHealth::Healthy
    } else {
        WatcherHealth::Degraded
    }
}
