use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, RecvTimeoutError, channel},
};
use std::time::Duration;

use tokenmaster_engine::{
    Clock, MonotonicTime, RefreshOutcome, RefreshUrgency, RefreshWorker, WorkerPhase,
};
use tokenmaster_runtime::{
    DEGRADED_POLL_MILLIS, HEALTHY_POLL_MILLIS, QUIET_WINDOW_MILLIS, RefreshScheduler,
    SchedulerPhase, WatcherHealth,
};

#[derive(Default)]
struct FakeClock {
    millis: AtomicU64,
}

impl FakeClock {
    fn set(&self, millis: u64) {
        self.millis.store(millis, Ordering::Release);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(self.millis.load(Ordering::Acquire))
    }
}

#[derive(Default)]
struct GatedClock {
    millis: AtomicU64,
    gate: Mutex<GatedClockState>,
    captured: Condvar,
    released: Condvar,
}

#[derive(Default)]
struct GatedClockState {
    armed: bool,
    captured: bool,
    released: bool,
}

struct SchedulerReadGate<'a> {
    clock: &'a GatedClock,
}

impl Drop for SchedulerReadGate<'_> {
    fn drop(&mut self) {
        self.clock.release_scheduler_read();
    }
}

impl GatedClock {
    fn set(&self, millis: u64) {
        self.millis.store(millis, Ordering::Release);
    }

    fn arm_scheduler_read(&self) -> SchedulerReadGate<'_> {
        let mut gate = self.gate.lock().expect("clock gate");
        *gate = GatedClockState {
            armed: true,
            captured: false,
            released: false,
        };
        SchedulerReadGate { clock: self }
    }

    fn wait_for_scheduler_read(&self) {
        let gate = self.gate.lock().expect("clock gate");
        let (gate, timeout) = self
            .captured
            .wait_timeout_while(gate, Duration::from_secs(2), |gate| !gate.captured)
            .expect("clock gate wait");
        assert!(!timeout.timed_out(), "scheduler clock read");
        assert!(gate.captured);
    }

    fn release_scheduler_read(&self) {
        let mut gate = self.gate.lock().unwrap_or_else(|error| error.into_inner());
        gate.released = true;
        self.released.notify_all();
    }
}

impl Clock for GatedClock {
    fn now(&self) -> MonotonicTime {
        let millis = self.millis.load(Ordering::Acquire);
        if std::thread::current().name() == Some("tokenmaster-scheduler") {
            let mut gate = self.gate.lock().expect("clock gate");
            if gate.armed && !gate.captured {
                gate.captured = true;
                self.captured.notify_all();
                gate = self
                    .released
                    .wait_while(gate, |gate| !gate.released)
                    .unwrap_or_else(|error| error.into_inner());
                gate.armed = false;
            }
        }
        MonotonicTime::from_millis(millis)
    }
}

fn receive(receiver: &Receiver<RefreshUrgency>) -> RefreshUrgency {
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("scheduled refresh")
}

fn assert_no_refresh(receiver: &Receiver<RefreshUrgency>) {
    assert_eq!(
        receiver.recv_timeout(Duration::from_millis(40)),
        Err(RecvTimeoutError::Timeout)
    );
}

#[test]
fn startup_and_ten_thousand_hints_use_one_fixed_aggregate_and_one_submission() {
    let clock = Arc::new(FakeClock::default());
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(clock.clone(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    let hints = scheduler.hints();

    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);
    clock.set(100);
    for _ in 0..10_000 {
        assert!(hints.filesystem_changed());
    }
    assert_no_refresh(&receiver);

    clock.set(100 + QUIET_WINDOW_MILLIS - 1);
    assert!(hints.watcher_healthy());
    assert_no_refresh(&receiver);
    clock.set(100 + QUIET_WINDOW_MILLIS);
    assert!(hints.watcher_healthy());
    assert_eq!(receive(&receiver), RefreshUrgency::Hint);
    assert_no_refresh(&receiver);

    let snapshot = scheduler.snapshot();
    assert_eq!(snapshot.phase(), SchedulerPhase::Running);
    assert!(!snapshot.dirty());
    assert!(!snapshot.force_reconcile());
    assert_eq!(snapshot.accepted_hint_count(), 10_000);
    assert_eq!(snapshot.submitted_count(), 2);
    assert_eq!(
        scheduler.shutdown().expect("shutdown"),
        SchedulerPhase::Stopped
    );
    assert!(!hints.filesystem_changed());
}

#[test]
fn submit_failure_faults_and_joins_without_retry_or_payload() {
    let clock = Arc::new(FakeClock::default());
    let mut scheduler = RefreshScheduler::spawn(clock, |_urgency| Err::<(), _>(())).expect("spawn");

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if scheduler.snapshot().phase() == SchedulerPhase::Faulted {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(scheduler.snapshot().phase(), SchedulerPhase::Faulted);
    assert_eq!(scheduler.snapshot().submitted_count(), 0);
    assert_eq!(
        scheduler.shutdown().expect("join faulted"),
        SchedulerPhase::Faulted
    );
}

#[test]
fn ten_thousand_scheduler_hints_create_at_most_one_engine_follow_up() {
    let clock = Arc::new(FakeClock::default());
    let (started_sender, started_receiver) = channel();
    let (release_sender, release_receiver) = channel();
    let mut first = true;
    let worker = Arc::new(
        RefreshWorker::spawn(clock.clone(), move |permit| {
            started_sender
                .send(permit.urgency())
                .expect("record execution");
            if first {
                first = false;
                release_receiver.recv().expect("release first execution");
            }
            RefreshOutcome::Completed
        })
        .expect("worker"),
    );
    let submit_worker = worker.clone();
    let mut scheduler = RefreshScheduler::spawn(clock.clone(), move |urgency| {
        submit_worker
            .submit(urgency, None)
            .map(|_admission| ())
            .map_err(|_error| ())
    })
    .expect("scheduler");
    let hints = scheduler.hints();
    assert_eq!(receive(&started_receiver), RefreshUrgency::Recovery);

    clock.set(100);
    for _ in 0..10_000 {
        assert!(hints.filesystem_changed());
    }
    clock.set(100 + QUIET_WINDOW_MILLIS);
    assert!(hints.watcher_healthy());
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if worker.snapshot().expect("worker snapshot").pending_count() == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        worker.snapshot().expect("worker snapshot").pending_count(),
        1
    );
    assert_eq!(scheduler.snapshot().submitted_count(), 2);

    release_sender.send(()).expect("release worker");
    assert_eq!(receive(&started_receiver), RefreshUrgency::Hint);
    assert_no_refresh(&started_receiver);

    assert_eq!(
        scheduler.shutdown().expect("scheduler shutdown"),
        SchedulerPhase::Stopped
    );
    let mut worker = match Arc::try_unwrap(worker) {
        Ok(worker) => worker,
        Err(_) => panic!("scheduler retained the refresh worker"),
    };
    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        WorkerPhase::Stopped
    );
}

#[test]
fn healthy_and_degraded_periods_are_checked_against_monotonic_time() {
    let clock = Arc::new(FakeClock::default());
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(clock.clone(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    let hints = scheduler.hints();
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

    clock.set(HEALTHY_POLL_MILLIS - 1);
    assert!(hints.watcher_healthy());
    assert_no_refresh(&receiver);
    clock.set(HEALTHY_POLL_MILLIS);
    assert!(hints.watcher_healthy());
    assert_eq!(receive(&receiver), RefreshUrgency::Periodic);

    clock.set(HEALTHY_POLL_MILLIS + 1);
    assert!(hints.watcher_error());
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);
    assert_eq!(
        scheduler.snapshot().watcher_health(),
        WatcherHealth::Degraded
    );

    clock.set(HEALTHY_POLL_MILLIS + DEGRADED_POLL_MILLIS);
    assert!(hints.filesystem_changed());
    assert_no_refresh(&receiver);
    clock.set(HEALTHY_POLL_MILLIS + 1 + DEGRADED_POLL_MILLIS);
    assert!(hints.filesystem_changed());
    assert_eq!(receive(&receiver), RefreshUrgency::Periodic);

    clock.set(HEALTHY_POLL_MILLIS + 2 + DEGRADED_POLL_MILLIS);
    assert!(hints.watcher_rescan_required());
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

    assert_eq!(
        scheduler.shutdown().expect("shutdown"),
        SchedulerPhase::Stopped
    );
}

#[test]
fn concurrent_hint_after_scheduler_clock_sample_is_not_a_clock_rollback() {
    let clock = Arc::new(GatedClock::default());
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(clock.clone(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    let hints = scheduler.hints();
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

    clock.set(100);
    let scheduler_read = clock.arm_scheduler_read();
    assert!(hints.watcher_healthy());
    clock.wait_for_scheduler_read();
    clock.set(101);
    let accepted = hints.filesystem_changed();
    drop(scheduler_read);
    assert!(accepted);

    assert_no_refresh(&receiver);
    clock.set(101 + QUIET_WINDOW_MILLIS);
    assert!(hints.watcher_healthy());
    assert_eq!(receive(&receiver), RefreshUrgency::Hint);
    assert_eq!(
        scheduler.shutdown().expect("shutdown"),
        SchedulerPhase::Stopped
    );
}

#[test]
fn clock_rollback_between_hint_and_scheduler_resample_fails_closed() {
    let clock = Arc::new(GatedClock::default());
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(clock.clone(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    let hints = scheduler.hints();
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

    clock.set(100);
    let scheduler_read = clock.arm_scheduler_read();
    assert!(hints.watcher_healthy());
    clock.wait_for_scheduler_read();
    clock.set(101);
    let accepted = hints.filesystem_changed();
    clock.set(100);
    drop(scheduler_read);

    assert!(accepted);
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);
    assert_eq!(
        scheduler.shutdown().expect("shutdown"),
        SchedulerPhase::Stopped
    );
}

#[test]
fn clock_rollback_pause_resume_and_shutdown_fail_closed() {
    let clock = Arc::new(FakeClock::default());
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(clock.clone(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    let hints = scheduler.hints();
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

    clock.set(1_000);
    assert!(hints.filesystem_changed());
    assert_no_refresh(&receiver);
    clock.set(900);
    assert!(hints.watcher_healthy());
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

    assert_eq!(scheduler.pause().expect("pause"), SchedulerPhase::Paused);
    for _ in 0..10_000 {
        assert!(!hints.filesystem_changed());
    }
    assert_no_refresh(&receiver);
    assert_eq!(scheduler.resume().expect("resume"), SchedulerPhase::Running);
    assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

    assert_eq!(
        scheduler.shutdown().expect("shutdown"),
        SchedulerPhase::Stopped
    );
    assert_eq!(
        scheduler.shutdown().expect("idempotent shutdown"),
        SchedulerPhase::Stopped
    );
    assert!(!hints.watcher_error());
}
