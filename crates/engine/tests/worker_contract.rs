use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    time::{Duration, Instant},
};

use tokenmaster_engine::{
    Clock, MonotonicTime, RefreshAdmission, RefreshDeadline, RefreshOutcome, RefreshUrgency,
    RefreshWorker, WorkerCompletion, WorkerCompletionKind, WorkerCompletionNotifier,
    WorkerErrorCode, WorkerPhase,
};

#[derive(Default)]
struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(self.0.load(Ordering::Acquire))
    }
}

impl TestClock {
    fn set(&self, milliseconds: u64) {
        self.0.store(milliseconds, Ordering::Release);
    }
}

struct BlockingClock {
    block_once: AtomicU64,
    entered: SyncSender<()>,
    release: Mutex<Receiver<()>>,
}

#[derive(Default)]
struct CountingClock(AtomicU64);

impl Clock for CountingClock {
    fn now(&self) -> MonotonicTime {
        self.0.fetch_add(1, Ordering::AcqRel);
        MonotonicTime::from_millis(0)
    }
}

#[derive(Default)]
struct WorkerPanicClock(AtomicU64);

impl Clock for WorkerPanicClock {
    fn now(&self) -> MonotonicTime {
        if self.0.fetch_add(1, Ordering::AcqRel) == 1 {
            panic!("fixed worker clock panic");
        }
        MonotonicTime::from_millis(0)
    }
}

impl Clock for BlockingClock {
    fn now(&self) -> MonotonicTime {
        if self.block_once.swap(0, Ordering::AcqRel) == 1 {
            self.entered.send(()).expect("signal blocked clock");
            self.release
                .lock()
                .expect("clock release lock")
                .recv()
                .expect("release blocked clock");
        }
        MonotonicTime::from_millis(0)
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for worker state"
        );
        std::thread::yield_now();
    }
}

fn wait_completion(worker: &RefreshWorker) -> WorkerCompletion {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(completion) = worker.try_completion().expect("completion poll") {
            return completion;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for completion"
        );
        std::thread::yield_now();
    }
}

#[test]
fn completion_wait_is_bounded_and_returns_the_exact_published_receipt() {
    let clock = Arc::new(TestClock::default());
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let mut worker = RefreshWorker::spawn(clock, move |permit| {
        entered_tx.send(permit.id()).expect("signal active task");
        release_rx.recv().expect("release active task");
        RefreshOutcome::Completed
    })
    .expect("spawn worker");
    let request_id = match worker
        .submit(RefreshUrgency::Recovery, None)
        .expect("submit recovery")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected recovery admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("active task entered"),
        request_id
    );
    assert_eq!(
        worker
            .wait_for_completion(Duration::from_millis(1))
            .expect("bounded empty wait"),
        None
    );
    release_tx.send(()).expect("release active task");
    let completion = worker
        .wait_for_completion(Duration::from_secs(2))
        .expect("bounded completion wait")
        .expect("published completion");
    assert_eq!(completion.request_id(), request_id);
    assert_eq!(completion.outcome(), RefreshOutcome::Completed);
    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        WorkerPhase::Stopped
    );
}

#[test]
fn ten_thousand_hints_use_one_follow_up_and_latest_only_result_slot() {
    let clock = Arc::new(TestClock::default());
    let calls = Arc::new(AtomicU64::new(0));
    let task_calls = calls.clone();
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let mut worker = RefreshWorker::spawn(clock, move |permit| {
        let call = task_calls.fetch_add(1, Ordering::AcqRel) + 1;
        if call == 1 {
            entered_tx.send(permit.id()).expect("signal first task");
            release_rx.recv().expect("release first task");
            RefreshOutcome::Failed
        } else {
            RefreshOutcome::Completed
        }
    })
    .expect("spawn worker");

    let first_id = match worker
        .submit(RefreshUrgency::Hint, None)
        .expect("submit first request")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected first admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("first task entered"),
        first_id
    );

    let mut newest_coalesced_id = first_id;
    for index in 0..10_000 {
        match worker
            .submit(RefreshUrgency::Periodic, None)
            .expect("coalesced submission")
        {
            RefreshAdmission::Coalesced {
                request_id,
                active_request_id,
            } => {
                assert_eq!(active_request_id, first_id);
                newest_coalesced_id = request_id;
            }
            admission => panic!("hint {index} was not coalesced: {admission:?}"),
        }
    }

    let active = worker.snapshot().expect("worker snapshot");
    assert_eq!(active.phase(), WorkerPhase::Running);
    assert_eq!(active.active_request_id(), Some(first_id));
    assert_eq!(active.pending_count(), 1);

    release_tx.send(()).expect("release first task");
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.superseded_results() == 1)
    });

    assert_eq!(calls.load(Ordering::Acquire), 2);
    let idle = worker.snapshot().expect("idle snapshot");
    assert_eq!(idle.active_request_id(), None);
    assert_eq!(idle.pending_count(), 0);
    let completion = worker
        .try_completion()
        .expect("completion read")
        .expect("latest completion");
    assert!(completion.request_id() > newest_coalesced_id);
    assert_eq!(completion.outcome(), RefreshOutcome::Completed);
    assert_eq!(completion.kind(), WorkerCompletionKind::Executed);
    assert_eq!(completion.superseded_results(), 1);
    assert!(!completion.follow_up_started());
    assert!(!completion.follow_up_abandoned());
    assert_eq!(worker.try_completion().expect("empty read"), None);

    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        WorkerPhase::Stopped
    );
}

#[test]
fn shutdown_cancels_active_and_coalesced_work_then_joins_without_running_follow_up() {
    let clock = Arc::new(TestClock::default());
    let calls = Arc::new(AtomicU64::new(0));
    let task_calls = calls.clone();
    let (entered_tx, entered_rx) = sync_channel(1);
    let exited = Arc::new(AtomicU64::new(0));
    let task_exited = exited.clone();
    let mut worker = RefreshWorker::spawn(clock, move |permit| {
        let call = task_calls.fetch_add(1, Ordering::AcqRel) + 1;
        if call == 1 {
            entered_tx.send(permit.id()).expect("signal active task");
            while !permit.is_cancelled() {
                std::thread::yield_now();
            }
            task_exited.store(1, Ordering::Release);
        }
        RefreshOutcome::Completed
    })
    .expect("spawn worker");

    let first_id = match worker
        .submit(RefreshUrgency::Interactive, None)
        .expect("submit active task")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected first admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("active task entered"),
        first_id
    );
    let coalesced_id = match worker
        .submit(RefreshUrgency::Recovery, None)
        .expect("submit follow-up")
    {
        RefreshAdmission::Coalesced {
            request_id,
            active_request_id,
        } => {
            assert_eq!(active_request_id, first_id);
            request_id
        }
        admission => panic!("unexpected follow-up admission: {admission:?}"),
    };

    assert_eq!(
        worker.shutdown().expect("joined shutdown"),
        WorkerPhase::Stopped
    );

    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert_eq!(exited.load(Ordering::Acquire), 1);
    let completion = worker
        .try_completion()
        .expect("completion read")
        .expect("cancelled follow-up completion");
    assert!(completion.request_id() > coalesced_id);
    assert_eq!(completion.outcome(), RefreshOutcome::Cancelled);
    assert_eq!(completion.kind(), WorkerCompletionKind::NotStarted);
    assert_eq!(completion.superseded_results(), 1);
    assert!(!completion.follow_up_started());
    assert!(!completion.follow_up_abandoned());
}

#[test]
fn panic_faults_worker_abandons_follow_up_and_exposes_no_payload() {
    let clock = Arc::new(TestClock::default());
    let calls = Arc::new(AtomicU64::new(0));
    let task_calls = calls.clone();
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let mut worker = RefreshWorker::spawn(clock, move |permit| {
        task_calls.fetch_add(1, Ordering::AcqRel);
        entered_tx.send(permit.id()).expect("signal active task");
        release_rx.recv().expect("release active task");
        panic!("fixed test panic")
    })
    .expect("spawn worker");

    let first_id = match worker
        .submit(RefreshUrgency::Interactive, None)
        .expect("submit active task")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected first admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("active task entered"),
        first_id
    );
    assert!(matches!(
        worker
            .submit(RefreshUrgency::Recovery, None)
            .expect("coalesce recovery"),
        RefreshAdmission::Coalesced { .. }
    ));

    release_tx.send(()).expect("release panic");
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.phase() == WorkerPhase::Faulted)
    });

    assert_eq!(calls.load(Ordering::Acquire), 1);
    let faulted = worker.snapshot().expect("faulted snapshot");
    assert_eq!(faulted.active_request_id(), None);
    assert_eq!(faulted.pending_count(), 0);
    let completion = worker
        .try_completion()
        .expect("completion read")
        .expect("panic completion");
    assert_eq!(completion.request_id(), first_id);
    assert_eq!(completion.outcome(), RefreshOutcome::Failed);
    assert_eq!(completion.kind(), WorkerCompletionKind::Panicked);
    assert!(!completion.follow_up_started());
    assert!(completion.follow_up_abandoned());
    assert!(!format!("{completion:?}").contains("fixed test panic"));

    let error = worker
        .submit(RefreshUrgency::Hint, None)
        .expect_err("faulted worker rejects submissions");
    assert_eq!(error.code(), WorkerErrorCode::Faulted);
    assert_eq!(error.to_string(), "faulted");
    assert_eq!(
        worker.shutdown().expect("join faulted worker"),
        WorkerPhase::Faulted
    );
}

#[test]
fn stale_cancel_cannot_touch_newer_work_and_shutdown_is_idempotent() {
    let clock = Arc::new(TestClock::default());
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let mut worker = RefreshWorker::spawn(clock, move |permit| {
        entered_tx.send(permit.id()).expect("signal task");
        release_rx.recv().expect("release task");
        RefreshOutcome::Completed
    })
    .expect("spawn worker");

    let first_id = match worker
        .submit(RefreshUrgency::Hint, None)
        .expect("submit first task")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected first admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("first task entered"),
        first_id
    );
    release_tx.send(()).expect("release first task");
    assert_eq!(
        wait_completion(&worker).outcome(),
        RefreshOutcome::Completed
    );

    let second_id = match worker
        .submit(RefreshUrgency::Interactive, None)
        .expect("submit second task")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected second admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("second task entered"),
        second_id
    );
    let stale = worker.cancel(first_id).expect_err("reject stale cancel");
    assert_eq!(stale.code(), WorkerErrorCode::StaleRequest);
    release_tx.send(()).expect("release second task");
    let second = wait_completion(&worker);
    assert_eq!(second.request_id(), second_id);
    assert_eq!(second.outcome(), RefreshOutcome::Completed);

    assert_eq!(
        worker.shutdown().expect("first shutdown"),
        WorkerPhase::Stopped
    );
    assert_eq!(
        worker.shutdown().expect("second shutdown"),
        WorkerPhase::Stopped
    );
    let closed = worker
        .submit(RefreshUrgency::Hint, None)
        .expect_err("stopped worker rejects submissions");
    assert_eq!(closed.code(), WorkerErrorCode::Closed);
}

#[test]
fn drop_cancels_and_joins_without_detaching_or_starting_follow_up() {
    let clock = Arc::new(TestClock::default());
    let calls = Arc::new(AtomicU64::new(0));
    let task_calls = calls.clone();
    let exited = Arc::new(AtomicU64::new(0));
    let task_exited = exited.clone();
    let (entered_tx, entered_rx) = sync_channel(1);
    let worker = RefreshWorker::spawn(clock, move |permit| {
        task_calls.fetch_add(1, Ordering::AcqRel);
        entered_tx.send(permit.id()).expect("signal active task");
        while !permit.is_cancelled() {
            std::thread::yield_now();
        }
        task_exited.store(1, Ordering::Release);
        RefreshOutcome::Completed
    })
    .expect("spawn worker");

    let first_id = match worker
        .submit(RefreshUrgency::Interactive, None)
        .expect("submit active task")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected first admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("active task entered"),
        first_id
    );
    assert!(matches!(
        worker
            .submit(RefreshUrgency::Recovery, None)
            .expect("coalesce follow-up"),
        RefreshAdmission::Coalesced { .. }
    ));

    drop(worker);

    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert_eq!(exited.load(Ordering::Acquire), 1);
}

#[test]
fn expired_pending_work_is_reported_without_a_second_callback() {
    let clock = Arc::new(TestClock::default());
    let calls = Arc::new(AtomicU64::new(0));
    let task_calls = calls.clone();
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let mut worker = RefreshWorker::spawn(clock.clone(), move |permit| {
        task_calls.fetch_add(1, Ordering::AcqRel);
        entered_tx.send(permit.id()).expect("signal active task");
        release_rx.recv().expect("release active task");
        RefreshOutcome::Completed
    })
    .expect("spawn worker");

    let first_id = match worker
        .submit(RefreshUrgency::Hint, None)
        .expect("submit active task")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected first admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("active task entered"),
        first_id
    );
    assert!(matches!(
        worker
            .submit(
                RefreshUrgency::Periodic,
                Some(RefreshDeadline::from_millis(10)),
            )
            .expect("coalesce expiring work"),
        RefreshAdmission::Coalesced { .. }
    ));
    clock.set(10);
    release_tx.send(()).expect("release active task");
    assert_eq!(calls.load(Ordering::Acquire), 1);
    let completion = wait_completion(&worker);
    assert_eq!(completion.request_id(), first_id);
    assert!(completion.pending_deadline_exceeded());
    assert!(!completion.follow_up_started());
    assert_eq!(worker.shutdown().expect("shutdown"), WorkerPhase::Stopped);
}

#[test]
fn panic_dominates_concurrent_shutdown_and_still_joins_faulted() {
    let clock = Arc::new(TestClock::default());
    let (entered_tx, entered_rx) = sync_channel(1);
    let mut worker = RefreshWorker::spawn(clock, move |permit| {
        entered_tx.send(permit.id()).expect("signal active task");
        while !permit.is_cancelled() {
            std::thread::yield_now();
        }
        panic!("fixed shutdown race panic")
    })
    .expect("spawn worker");

    let first_id = match worker
        .submit(RefreshUrgency::Interactive, None)
        .expect("submit active task")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected first admission: {admission:?}"),
    };
    assert_eq!(
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("active task entered"),
        first_id
    );
    assert!(matches!(
        worker
            .submit(RefreshUrgency::Recovery, None)
            .expect("coalesce recovery"),
        RefreshAdmission::Coalesced { .. }
    ));

    assert_eq!(
        worker.shutdown().expect("joined faulted worker"),
        WorkerPhase::Faulted
    );

    let snapshot = worker.snapshot().expect("faulted snapshot");
    assert_eq!(snapshot.active_request_id(), None);
    assert_eq!(snapshot.pending_count(), 0);
    let completion = worker
        .try_completion()
        .expect("completion read")
        .expect("panic completion");
    assert_eq!(completion.request_id(), first_id);
    assert_eq!(completion.outcome(), RefreshOutcome::Failed);
    assert_eq!(completion.kind(), WorkerCompletionKind::Panicked);
    assert!(completion.follow_up_abandoned());
}

#[test]
fn clock_callback_does_not_run_under_worker_state_lock() {
    let (clock_entered_tx, clock_entered_rx) = sync_channel(1);
    let (clock_release_tx, clock_release_rx) = sync_channel(1);
    let clock = Arc::new(BlockingClock {
        block_once: AtomicU64::new(1),
        entered: clock_entered_tx,
        release: Mutex::new(clock_release_rx),
    });
    let worker =
        Arc::new(RefreshWorker::spawn(clock, |_| RefreshOutcome::Completed).expect("spawn worker"));

    let submit_worker = worker.clone();
    let submit = std::thread::spawn(move || submit_worker.submit(RefreshUrgency::Hint, None));
    clock_entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("clock callback entered");

    let snapshot_worker = worker.clone();
    let (snapshot_tx, snapshot_rx) = sync_channel(1);
    let snapshot = std::thread::spawn(move || {
        snapshot_tx
            .send(snapshot_worker.snapshot())
            .expect("send snapshot");
    });
    let snapshot_was_lock_free = snapshot_rx.recv_timeout(Duration::from_millis(100));

    clock_release_tx.send(()).expect("release clock callback");
    submit
        .join()
        .expect("submit thread join")
        .expect("submit result");
    snapshot.join().expect("snapshot thread join");
    assert!(
        snapshot_was_lock_free.is_ok(),
        "Clock::now held the worker state lock"
    );

    let mut worker = Arc::try_unwrap(worker).unwrap_or_else(|_| panic!("worker still shared"));
    assert_eq!(worker.shutdown().expect("shutdown"), WorkerPhase::Stopped);
}

#[test]
fn stopped_worker_rejects_before_calling_clock() {
    let clock = Arc::new(CountingClock::default());
    let mut worker =
        RefreshWorker::spawn(clock.clone(), |_| RefreshOutcome::Completed).expect("spawn worker");
    assert_eq!(worker.shutdown().expect("shutdown"), WorkerPhase::Stopped);
    assert_eq!(clock.0.load(Ordering::Acquire), 0);

    let error = worker
        .submit(RefreshUrgency::Hint, None)
        .expect_err("stopped worker rejects submission");
    assert_eq!(error.code(), WorkerErrorCode::Closed);
    assert_eq!(clock.0.load(Ordering::Acquire), 0);
}

#[test]
fn worker_port_panic_faults_clears_and_joins_without_payload_result() {
    let clock = Arc::new(WorkerPanicClock::default());
    let mut worker =
        RefreshWorker::spawn(clock, |_| RefreshOutcome::Completed).expect("spawn worker");
    assert!(matches!(
        worker
            .submit(RefreshUrgency::Interactive, None)
            .expect("submit active task"),
        RefreshAdmission::Started(_)
    ));

    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.phase() == WorkerPhase::Faulted)
    });
    let snapshot = worker.snapshot().expect("faulted snapshot");
    assert_eq!(snapshot.active_request_id(), None);
    assert_eq!(snapshot.pending_count(), 0);
    assert_eq!(worker.try_completion().expect("completion poll"), None);
    let error = worker
        .submit(RefreshUrgency::Hint, None)
        .expect_err("faulted worker rejects submission");
    assert_eq!(error.code(), WorkerErrorCode::Faulted);
    assert_eq!(
        worker.shutdown().expect("join faulted worker"),
        WorkerPhase::Faulted
    );
}

struct CompletionRecorder {
    calls: AtomicU64,
    latest: Mutex<Option<WorkerCompletion>>,
}

impl CompletionRecorder {
    fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
            latest: Mutex::new(None),
        }
    }
}

impl WorkerCompletionNotifier for CompletionRecorder {
    fn completion_ready(&self, completion: WorkerCompletion) {
        *self.latest.lock().expect("latest notification") = Some(completion);
        self.calls.fetch_add(1, Ordering::AcqRel);
    }
}

#[test]
fn notified_worker_publishes_receipt_before_one_completion_hint() {
    let clock = Arc::new(TestClock::default());
    let notifier = Arc::new(CompletionRecorder::new());
    let mut worker =
        RefreshWorker::spawn_notified(clock, notifier.clone(), |_| RefreshOutcome::Completed)
            .expect("spawn notified worker");
    let request_id = match worker
        .submit(RefreshUrgency::Hint, None)
        .expect("submit notified work")
    {
        RefreshAdmission::Started(permit) => permit.id(),
        admission => panic!("unexpected admission: {admission:?}"),
    };

    wait_until(|| notifier.calls.load(Ordering::Acquire) == 1);
    let notified = notifier
        .latest
        .lock()
        .expect("latest notification")
        .expect("notification payload");
    assert_eq!(notified.request_id(), request_id);
    assert_eq!(notified.outcome(), RefreshOutcome::Completed);
    let receipt = wait_completion(&worker);
    assert_eq!(receipt, notified);
    assert_eq!(notifier.calls.load(Ordering::Acquire), 1);
    assert_eq!(worker.shutdown().expect("shutdown"), WorkerPhase::Stopped);
}

struct PanickingNotifier;

impl WorkerCompletionNotifier for PanickingNotifier {
    fn completion_ready(&self, _completion: WorkerCompletion) {
        panic!("private notifier payload must be redacted");
    }
}

#[test]
fn notifier_panic_does_not_fault_worker_or_lose_completion_receipt() {
    let clock = Arc::new(TestClock::default());
    let mut worker = RefreshWorker::spawn_notified(clock, Arc::new(PanickingNotifier), |_| {
        RefreshOutcome::Completed
    })
    .expect("spawn notified worker");
    assert!(matches!(
        worker
            .submit(RefreshUrgency::Hint, None)
            .expect("submit notified work"),
        RefreshAdmission::Started(_)
    ));

    let completion = wait_completion(&worker);
    assert_eq!(completion.outcome(), RefreshOutcome::Completed);
    assert_eq!(
        worker.snapshot().expect("worker snapshot").phase(),
        WorkerPhase::Running
    );
    assert_eq!(worker.shutdown().expect("shutdown"), WorkerPhase::Stopped);
}
