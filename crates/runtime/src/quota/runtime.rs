use std::fmt;
use std::sync::{Arc, Mutex};

use tokenmaster_engine::{
    Clock, RefreshOutcome, RefreshPermit, RefreshUrgency, RefreshWorker, WorkerCompletion,
    WorkerCompletionNotifier, WorkerError, WorkerErrorCode, WorkerPhase,
};
use tokenmaster_platform::PowerLifecycleEvent;

use super::execution::{
    CodexQuotaExecution, RuntimeCodexQuotaSource, StoreQuotaPublisher, SystemCodexQuotaWallClock,
};
use super::{
    CodexQuotaRefreshSnapshot, CodexQuotaRetryMode, CodexQuotaRuntimeConfig,
    CodexQuotaRuntimePhase, CodexQuotaRuntimeSnapshot, CodexQuotaScheduleSnapshot,
};
use crate::{
    RefreshHintSink, RefreshScheduler, RuntimeError, RuntimeErrorCode, SchedulerError,
    SchedulerErrorCode, SchedulerPhase, SystemClock, WatcherHealth,
};

pub struct CodexQuotaRuntime {
    phase: CodexQuotaRuntimePhase,
    scheduler: RefreshScheduler,
    worker: Arc<RefreshWorker>,
    admission_open: Arc<Mutex<bool>>,
    latest: Arc<Mutex<CodexQuotaRefreshSnapshot>>,
}

impl CodexQuotaRuntime {
    pub fn start(config: CodexQuotaRuntimeConfig) -> Result<Self, RuntimeError> {
        Self::start_with_notifier(config, None)
    }

    pub fn start_notified(
        config: CodexQuotaRuntimeConfig,
        notifier: Arc<dyn WorkerCompletionNotifier>,
    ) -> Result<Self, RuntimeError> {
        Self::start_with_notifier(config, Some(notifier))
    }

    fn start_with_notifier(
        config: CodexQuotaRuntimeConfig,
        notifier: Option<Arc<dyn WorkerCompletionNotifier>>,
    ) -> Result<Self, RuntimeError> {
        let clock: Arc<dyn Clock> = SystemClock::shared();
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let source = RuntimeCodexQuotaSource::new(config.clone());
        let publisher = StoreQuotaPublisher::new(config.archive_path())?;
        let mut execution = CodexQuotaExecution::new(
            Arc::clone(&clock),
            SystemCodexQuotaWallClock,
            source,
            publisher,
            Arc::clone(&latest),
        );
        Self::start_with_runner_notified(clock, latest, notifier, move |permit| {
            execution.run(permit)
        })
    }

    #[cfg(test)]
    fn start_with_runner<F>(
        clock: Arc<dyn Clock>,
        latest: Arc<Mutex<CodexQuotaRefreshSnapshot>>,
        execute: F,
    ) -> Result<Self, RuntimeError>
    where
        F: FnMut(&RefreshPermit) -> RefreshOutcome + Send + 'static,
    {
        Self::start_with_runner_notified(clock, latest, None, execute)
    }

    fn start_with_runner_notified<F>(
        clock: Arc<dyn Clock>,
        latest: Arc<Mutex<CodexQuotaRefreshSnapshot>>,
        notifier: Option<Arc<dyn WorkerCompletionNotifier>>,
        mut execute: F,
    ) -> Result<Self, RuntimeError>
    where
        F: FnMut(&RefreshPermit) -> RefreshOutcome + Send + 'static,
    {
        let cadence_slot = Arc::new(Mutex::new(None::<RefreshHintSink>));
        let execution_latest = Arc::clone(&latest);
        let execution_cadence = Arc::clone(&cadence_slot);
        let worker_execution = move |permit: &RefreshPermit| {
            let outcome = execute(permit);
            let retry_mode = match execution_latest.lock() {
                Ok(latest) => latest.retry_mode(),
                Err(_) => return RefreshOutcome::Failed,
            };
            let health = match retry_mode {
                CodexQuotaRetryMode::Normal => WatcherHealth::Healthy,
                CodexQuotaRetryMode::Accelerated => WatcherHealth::Degraded,
            };
            let cadence = match execution_cadence.lock() {
                Ok(cadence) => cadence,
                Err(_) => return RefreshOutcome::Failed,
            };
            if let Some(hints) = cadence.as_ref() {
                let _ = hints.set_poll_health(health);
            }
            outcome
        };
        let worker = Arc::new(
            match notifier {
                Some(notifier) => {
                    RefreshWorker::spawn_notified(Arc::clone(&clock), notifier, worker_execution)
                }
                None => RefreshWorker::spawn(Arc::clone(&clock), worker_execution),
            }
            .map_err(runtime_worker_error)?,
        );

        let admission_open = Arc::new(Mutex::new(false));
        let scheduler_worker = Arc::clone(&worker);
        let scheduler_admission = Arc::clone(&admission_open);
        let scheduler = RefreshScheduler::spawn_paused(clock, move |urgency| {
            let admission = scheduler_admission.lock().map_err(|_| ())?;
            if !*admission {
                return Ok(());
            }
            scheduler_worker
                .submit(urgency, None)
                .map(|_admission| ())
                .map_err(|_| ())
        })
        .map_err(runtime_scheduler_error)?;
        *cadence_slot
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = Some(scheduler.hints());
        *admission_open
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = true;
        scheduler.resume().map_err(runtime_scheduler_error)?;

        Ok(Self {
            phase: CodexQuotaRuntimePhase::Running,
            scheduler,
            worker,
            admission_open,
            latest,
        })
    }

    pub fn refresh_now(&self) -> Result<(), RuntimeError> {
        self.refresh_with_urgency(RefreshUrgency::Interactive)
    }

    fn refresh_with_urgency(&self, urgency: RefreshUrgency) -> Result<(), RuntimeError> {
        if self.phase != CodexQuotaRuntimePhase::Running {
            return Err(RuntimeError::new(match self.phase {
                CodexQuotaRuntimePhase::Faulted => RuntimeErrorCode::Faulted,
                CodexQuotaRuntimePhase::Running => RuntimeErrorCode::Internal,
                CodexQuotaRuntimePhase::Paused
                | CodexQuotaRuntimePhase::Stopping
                | CodexQuotaRuntimePhase::Stopped => RuntimeErrorCode::Closed,
            }));
        }
        if self.scheduler.hints().force_reconcile(urgency) {
            Ok(())
        } else {
            Err(RuntimeError::new(RuntimeErrorCode::Closed))
        }
    }

    pub fn try_completion(&self) -> Result<Option<WorkerCompletion>, RuntimeError> {
        self.worker.try_completion().map_err(runtime_worker_error)
    }

    pub fn snapshot(&self) -> Result<CodexQuotaRuntimeSnapshot, RuntimeError> {
        let scheduler = self.scheduler.snapshot();
        let worker = self.worker.snapshot().map_err(runtime_worker_error)?;
        let refresh = *self
            .latest
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        let phase = if scheduler.phase() == SchedulerPhase::Faulted
            || worker.phase() == WorkerPhase::Faulted
        {
            CodexQuotaRuntimePhase::Faulted
        } else {
            self.phase
        };
        let retry_mode = match scheduler.watcher_health() {
            WatcherHealth::Healthy => CodexQuotaRetryMode::Normal,
            WatcherHealth::Degraded => CodexQuotaRetryMode::Accelerated,
        };
        Ok(CodexQuotaRuntimeSnapshot {
            phase,
            schedule: CodexQuotaScheduleSnapshot {
                phase: scheduler.phase(),
                retry_mode,
                refresh_pending: scheduler.dirty() || scheduler.force_reconcile(),
                accepted_refresh_count: scheduler.accepted_hint_count(),
                submitted_count: scheduler.submitted_count(),
            },
            worker,
            refresh,
        })
    }

    pub fn pause(&mut self) -> Result<CodexQuotaRuntimePhase, RuntimeError> {
        match self.phase {
            CodexQuotaRuntimePhase::Paused => return Ok(CodexQuotaRuntimePhase::Paused),
            CodexQuotaRuntimePhase::Running => {}
            CodexQuotaRuntimePhase::Faulted => {
                return Err(RuntimeError::new(RuntimeErrorCode::Faulted));
            }
            CodexQuotaRuntimePhase::Stopping | CodexQuotaRuntimePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        let mut admission = match self.admission_open.lock() {
            Ok(admission) => admission,
            Err(_) => {
                self.phase = CodexQuotaRuntimePhase::Faulted;
                return Err(RuntimeError::new(RuntimeErrorCode::Internal));
            }
        };
        *admission = false;
        if let Err(error) = self.scheduler.pause() {
            self.phase = CodexQuotaRuntimePhase::Faulted;
            return Err(runtime_scheduler_error(error));
        }
        let snapshot = match self.worker.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.phase = CodexQuotaRuntimePhase::Faulted;
                return Err(runtime_worker_error(error));
            }
        };
        if let Some(active) = snapshot.active_request_id()
            && let Err(error) = self.worker.cancel(active)
            && error.code() != WorkerErrorCode::StaleRequest
        {
            self.phase = CodexQuotaRuntimePhase::Faulted;
            return Err(runtime_worker_error(error));
        }
        self.phase = CodexQuotaRuntimePhase::Paused;
        Ok(self.phase)
    }

    pub fn resume(&mut self) -> Result<CodexQuotaRuntimePhase, RuntimeError> {
        match self.phase {
            CodexQuotaRuntimePhase::Running => return Ok(CodexQuotaRuntimePhase::Running),
            CodexQuotaRuntimePhase::Paused => {}
            CodexQuotaRuntimePhase::Faulted => {
                return Err(RuntimeError::new(RuntimeErrorCode::Faulted));
            }
            CodexQuotaRuntimePhase::Stopping | CodexQuotaRuntimePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        let mut admission = match self.admission_open.lock() {
            Ok(admission) => admission,
            Err(_) => {
                self.phase = CodexQuotaRuntimePhase::Faulted;
                return Err(RuntimeError::new(RuntimeErrorCode::Internal));
            }
        };
        *admission = true;
        if let Err(error) = self.scheduler.resume() {
            *admission = false;
            self.phase = CodexQuotaRuntimePhase::Faulted;
            return Err(runtime_scheduler_error(error));
        }
        self.phase = CodexQuotaRuntimePhase::Running;
        Ok(self.phase)
    }

    pub fn apply_power_event(
        &mut self,
        event: PowerLifecycleEvent,
    ) -> Result<CodexQuotaRuntimePhase, RuntimeError> {
        match event {
            PowerLifecycleEvent::Suspend => self.pause(),
            PowerLifecycleEvent::Resume if self.phase == CodexQuotaRuntimePhase::Running => {
                self.refresh_with_urgency(RefreshUrgency::Recovery)?;
                Ok(self.phase)
            }
            PowerLifecycleEvent::Resume => self.resume(),
        }
    }

    pub fn shutdown(&mut self) -> Result<CodexQuotaRuntimePhase, RuntimeError> {
        if self.phase == CodexQuotaRuntimePhase::Stopped {
            return Ok(self.phase);
        }
        self.phase = CodexQuotaRuntimePhase::Stopping;
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
                SchedulerPhase::Faulted
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
            || scheduler_phase == SchedulerPhase::Faulted
            || worker_phase == WorkerPhase::Faulted
        {
            self.phase = CodexQuotaRuntimePhase::Faulted;
            Err(RuntimeError::new(RuntimeErrorCode::Internal))
        } else {
            self.phase = CodexQuotaRuntimePhase::Stopped;
            Ok(self.phase)
        }
    }
}

impl fmt::Debug for CodexQuotaRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexQuotaRuntime")
            .field("snapshot", &self.snapshot().ok())
            .finish()
    }
}

impl Drop for CodexQuotaRuntime {
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

fn runtime_scheduler_error(error: SchedulerError) -> RuntimeError {
    let code = match error.code() {
        SchedulerErrorCode::Closed => RuntimeErrorCode::Closed,
        SchedulerErrorCode::Faulted => RuntimeErrorCode::Faulted,
        SchedulerErrorCode::CapacityExceeded => RuntimeErrorCode::Busy,
        SchedulerErrorCode::Unavailable => RuntimeErrorCode::ProviderUnavailable,
        SchedulerErrorCode::Internal => RuntimeErrorCode::Internal,
    };
    RuntimeError::new(code)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{Receiver, RecvTimeoutError, channel},
    };
    use std::time::Duration;

    use tempfile::TempDir;
    use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
    use tokenmaster_engine::{
        Clock, MonotonicTime, RefreshOutcome, RefreshPermit, RefreshUrgency, WorkerPhase,
    };
    use tokenmaster_platform::PowerLifecycleEvent;

    use super::CodexQuotaRuntime;
    use crate::RuntimeErrorCode;

    trait TestResultExt<T, E> {
        fn test_value(self, context: &str) -> T;
        fn test_error(self, context: &str) -> E;
    }

    impl<T, E> TestResultExt<T, E> for Result<T, E> {
        fn test_value(self, context: &str) -> T {
            match self {
                Ok(value) => value,
                Err(_) => panic!("{context}"),
            }
        }

        fn test_error(self, context: &str) -> E {
            match self {
                Ok(_) => panic!("{context}"),
                Err(error) => error,
            }
        }
    }
    use crate::quota::{CodexQuotaRefreshSnapshot, CodexQuotaRetryMode, CodexQuotaRuntimePhase};

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

    fn receive(receiver: &Receiver<RefreshUrgency>) -> RefreshUrgency {
        receiver
            .recv_timeout(Duration::from_secs(2))
            .test_value("quota refresh execution")
    }

    fn assert_no_refresh(receiver: &Receiver<RefreshUrgency>) {
        assert_eq!(
            receiver.recv_timeout(Duration::from_millis(40)),
            Err(RecvTimeoutError::Timeout)
        );
    }

    fn record(
        latest: &Arc<Mutex<CodexQuotaRefreshSnapshot>>,
        outcome: RefreshOutcome,
        retry_mode: CodexQuotaRetryMode,
    ) {
        let mut snapshot = latest.lock().test_value("latest");
        snapshot.attempt_sequence = snapshot.attempt_sequence.saturating_add(1);
        snapshot.outcome = Some(outcome);
        snapshot.failure = None;
        snapshot.retry_mode = retry_mode;
    }

    #[test]
    fn startup_runs_one_recovery_refresh_and_publishes_separate_health() {
        let clock = Arc::new(FakeClock::default());
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let execution_latest = Arc::clone(&latest);
        let (sender, receiver) = channel();
        let mut runtime =
            CodexQuotaRuntime::start_with_runner(clock, Arc::clone(&latest), move |permit| {
                sender.send(permit.urgency()).test_value("record urgency");
                record(
                    &execution_latest,
                    RefreshOutcome::Completed,
                    CodexQuotaRetryMode::Normal,
                );
                RefreshOutcome::Completed
            })
            .test_value("quota runtime");

        assert_eq!(receive(&receiver), RefreshUrgency::Recovery);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if runtime
                .snapshot()
                .test_value("runtime snapshot")
                .refresh()
                .outcome()
                .is_some()
            {
                break;
            }
            std::thread::yield_now();
        }
        let snapshot = runtime.snapshot().test_value("runtime snapshot");
        assert_eq!(snapshot.phase(), CodexQuotaRuntimePhase::Running);
        assert_eq!(
            snapshot.refresh().outcome(),
            Some(RefreshOutcome::Completed)
        );
        assert_eq!(snapshot.schedule().submitted_count(), 1);
        assert_eq!(
            snapshot.schedule().retry_mode(),
            CodexQuotaRetryMode::Normal
        );
        assert_no_refresh(&receiver);
        assert_eq!(
            runtime.shutdown().test_value("quota shutdown"),
            CodexQuotaRuntimePhase::Stopped
        );
    }

    #[test]
    fn manual_refresh_burst_retains_only_one_worker_follow_up() {
        let clock = Arc::new(FakeClock::default());
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let execution_latest = Arc::clone(&latest);
        let (started_sender, started_receiver) = channel();
        let (release_sender, release_receiver) = channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let execution_calls = Arc::clone(&calls);
        let mut runtime = CodexQuotaRuntime::start_with_runner(clock, latest, move |permit| {
            let call = execution_calls.fetch_add(1, Ordering::AcqRel);
            started_sender
                .send(permit.urgency())
                .test_value("record execution");
            if call == 0 {
                release_receiver
                    .recv()
                    .test_value("release first execution");
            }
            record(
                &execution_latest,
                RefreshOutcome::Completed,
                CodexQuotaRetryMode::Normal,
            );
            RefreshOutcome::Completed
        })
        .test_value("quota runtime");
        assert_eq!(receive(&started_receiver), RefreshUrgency::Recovery);

        for _ in 0..10_000 {
            runtime.refresh_now().test_value("coalesced manual refresh");
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            let snapshot = runtime.snapshot().test_value("snapshot");
            if snapshot.worker().pending_count() == 1 && !snapshot.schedule().refresh_pending() {
                break;
            }
            std::thread::yield_now();
        }
        let drained = runtime.snapshot().test_value("drained snapshot");
        assert_eq!(drained.worker().pending_count(), 1);
        assert!(!drained.schedule().refresh_pending());

        release_sender
            .send(())
            .test_value("release first execution");
        assert_eq!(receive(&started_receiver), RefreshUrgency::Interactive);
        assert_no_refresh(&started_receiver);
        assert_eq!(calls.load(Ordering::Acquire), 2);
        runtime.shutdown().test_value("quota shutdown");
    }

    #[test]
    fn transient_failure_uses_accelerated_period_then_success_returns_to_normal() {
        let clock = Arc::new(FakeClock::default());
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let execution_latest = Arc::clone(&latest);
        let execution_count = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&execution_count);
        let (sender, receiver) = channel();
        let mut runtime =
            CodexQuotaRuntime::start_with_runner(clock.clone(), latest, move |permit| {
                let call = calls.fetch_add(1, Ordering::AcqRel);
                sender.send(permit.urgency()).test_value("record execution");
                let (outcome, retry) = if call == 0 {
                    (RefreshOutcome::Failed, CodexQuotaRetryMode::Accelerated)
                } else {
                    (RefreshOutcome::Completed, CodexQuotaRetryMode::Normal)
                };
                record(&execution_latest, outcome, retry);
                outcome
            })
            .test_value("quota runtime");
        assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if runtime
                .snapshot()
                .test_value("snapshot")
                .schedule()
                .retry_mode()
                == CodexQuotaRetryMode::Accelerated
            {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(
            runtime
                .snapshot()
                .test_value("snapshot")
                .schedule()
                .retry_mode(),
            CodexQuotaRetryMode::Accelerated
        );

        clock.set(crate::DEGRADED_POLL_MILLIS - 1);
        assert!(runtime.scheduler.hints().wake());
        assert_no_refresh(&receiver);
        clock.set(crate::DEGRADED_POLL_MILLIS);
        assert!(runtime.scheduler.hints().wake());
        assert_eq!(receive(&receiver), RefreshUrgency::Periodic);

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if runtime
                .snapshot()
                .test_value("snapshot")
                .schedule()
                .retry_mode()
                == CodexQuotaRetryMode::Normal
            {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(
            runtime
                .snapshot()
                .test_value("snapshot")
                .schedule()
                .retry_mode(),
            CodexQuotaRetryMode::Normal
        );
        assert_eq!(execution_count.load(Ordering::Acquire), 2);
        runtime.shutdown().test_value("quota shutdown");
    }

    #[test]
    fn pause_resume_power_and_shutdown_are_bounded_and_idempotent() {
        let clock = Arc::new(FakeClock::default());
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let execution_latest = Arc::clone(&latest);
        let (sender, receiver) = channel();
        let mut runtime = CodexQuotaRuntime::start_with_runner(clock, latest, move |permit| {
            sender.send(permit.urgency()).test_value("record execution");
            record(
                &execution_latest,
                RefreshOutcome::Completed,
                CodexQuotaRetryMode::Normal,
            );
            RefreshOutcome::Completed
        })
        .test_value("quota runtime");
        assert_eq!(receive(&receiver), RefreshUrgency::Recovery);

        assert_eq!(
            runtime.pause().test_value("pause"),
            CodexQuotaRuntimePhase::Paused
        );
        assert_eq!(
            runtime.refresh_now().test_error("paused refresh").code(),
            RuntimeErrorCode::Closed
        );
        assert_eq!(
            runtime
                .apply_power_event(PowerLifecycleEvent::Suspend)
                .test_value("idempotent suspend"),
            CodexQuotaRuntimePhase::Paused
        );
        assert_eq!(
            runtime
                .apply_power_event(PowerLifecycleEvent::Resume)
                .test_value("resume"),
            CodexQuotaRuntimePhase::Running
        );
        assert_eq!(receive(&receiver), RefreshUrgency::Recovery);
        assert_eq!(
            runtime.shutdown().test_value("shutdown"),
            CodexQuotaRuntimePhase::Stopped
        );
        assert_eq!(
            runtime.shutdown().test_value("idempotent shutdown"),
            CodexQuotaRuntimePhase::Stopped
        );
    }

    #[test]
    fn runner_panic_faults_only_quota_and_preserves_live_usage_snapshot() {
        let source_root = TempDir::new().test_value("usage source root");
        let archive_root = TempDir::new().test_value("usage archive root");
        let configured = [ConfiguredCodexRoot::new(source_root.path(), None, true)];
        let request = build_discovery_request(CodexRootInput {
            user_profile: None,
            codex_home: None,
            configured: &configured,
        })
        .test_value("usage discovery request");
        let mut usage_runtime =
            crate::LiveRuntime::start(&archive_root.path().join("usage.sqlite3"), request)
                .test_value("usage runtime");
        let usage_deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if usage_runtime
                .try_completion()
                .test_value("usage completion")
                .is_some()
            {
                break;
            }
            assert!(
                std::time::Instant::now() < usage_deadline,
                "usage runtime refresh timed out"
            );
            std::thread::yield_now();
        }
        let usage_before = usage_runtime
            .snapshot()
            .test_value("usage snapshot before quota fault")
            .engine();

        let clock = Arc::new(FakeClock::default());
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let runtime = CodexQuotaRuntime::start_with_runner(clock, latest, |_permit| {
            panic!("private quota runner panic")
        })
        .test_value("quota runtime");

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if runtime.snapshot().test_value("snapshot").worker().phase() == WorkerPhase::Faulted {
                break;
            }
            std::thread::yield_now();
        }
        let snapshot = runtime.snapshot().test_value("snapshot");
        assert_eq!(snapshot.phase(), CodexQuotaRuntimePhase::Faulted);
        assert_eq!(snapshot.worker().phase(), WorkerPhase::Faulted);
        assert!(!format!("{runtime:?}").contains("private quota runner panic"));
        assert_eq!(
            usage_runtime
                .snapshot()
                .test_value("usage snapshot after quota fault")
                .engine(),
            usage_before
        );
        usage_runtime
            .shutdown()
            .test_value("usage runtime shutdown");
    }

    #[test]
    fn runner_observes_only_bounded_permit_metadata() {
        let clock = Arc::new(FakeClock::default());
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let (sender, receiver) = channel::<(RefreshUrgency, bool)>();
        let mut runtime =
            CodexQuotaRuntime::start_with_runner(clock, latest, move |permit: &RefreshPermit| {
                sender
                    .send((permit.urgency(), permit.deadline().is_some()))
                    .test_value("permit metadata");
                RefreshOutcome::Completed
            })
            .test_value("quota runtime");
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)),
            Ok((RefreshUrgency::Recovery, false))
        );
        runtime.shutdown().test_value("shutdown");
    }
}
