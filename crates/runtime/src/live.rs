use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use tokenmaster_engine::{
    Clock, OneShotExecutor, OperationControl, PortError, PortErrorCode, RefreshOutcome,
    RefreshPermit, RefreshUrgency, RefreshWorker, WorkerCompletion, WorkerError, WorkerErrorCode,
    WorkerPhase, WriterLease, WriterLeaseGuard,
};
use tokenmaster_provider::DiscoveryRequest;
use tokenmaster_store::{ArchiveMode, ArchivePublicationQuality, UsageStore};

use crate::lifecycle::{LivePhase, LiveRefreshKind, LiveRefreshSnapshot, LiveRuntimeSnapshot};
use crate::recovery::{StartupRecoveryReport, recover_startup};
use crate::{
    BoundedFilesystemWatcher, CodexAdapter, IncrementalRefreshOutcome, RefreshHintSink,
    RefreshScheduler, RuntimeError, RuntimeErrorCode, SchedulerError, SchedulerErrorCode,
    SchedulerPhase, StoreArchive, SystemClock, WatcherSnapshot, refresh_incremental,
};

pub struct LiveRuntime {
    phase: LivePhase,
    startup_recovery: StartupRecoveryReport,
    scheduler: RefreshScheduler,
    worker: Arc<RefreshWorker>,
    watcher_slot: Arc<Mutex<Option<BoundedFilesystemWatcher>>>,
    last_watcher_snapshot: WatcherSnapshot,
    admission_open: Arc<Mutex<bool>>,
    reset_watcher: Arc<AtomicBool>,
    latest_refresh: Arc<Mutex<LiveRefreshSnapshot>>,
}

impl LiveRuntime {
    pub fn start(archive_path: &Path, request: DiscoveryRequest) -> Result<Self, RuntimeError> {
        let clock: Arc<dyn Clock> = SystemClock::shared();
        let mut lease = crate::RuntimeWriterLease::new(archive_path)?;
        let startup_guard = lease.try_acquire().map_err(startup_port_error)?;
        let store = UsageStore::open(archive_path)
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::StoreUnavailable))?;
        let mut archive = StoreArchive::new(store);
        let startup_recovery = recover_startup(&mut archive).map_err(startup_port_error)?;
        drop(startup_guard);

        let watcher_slot = Arc::new(Mutex::new(None));
        let reset_watcher = Arc::new(AtomicBool::new(true));
        let execution_watcher = Arc::clone(&watcher_slot);
        let execution_reset = Arc::clone(&reset_watcher);
        let execution_clock = Arc::clone(&clock);
        let latest_refresh = Arc::new(Mutex::new(LiveRefreshSnapshot::not_run()));
        let execution_refresh = Arc::clone(&latest_refresh);
        let mut execution = LiveExecution {
            clock: Arc::clone(&clock),
            lease,
            adapter: CodexAdapter::new(request)?,
            archive,
            watcher_slot: execution_watcher,
            reset_watcher: execution_reset,
            last_watch_roots: Vec::new(),
            watch_set_complete: false,
            latest_refresh: execution_refresh,
        };
        let worker = Arc::new(
            RefreshWorker::spawn(execution_clock, move |permit| execution.run(permit))
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
        let watcher = BoundedFilesystemWatcher::new(scheduler.hints())
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        let last_watcher_snapshot = watcher.snapshot();
        *watcher_slot
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = Some(watcher);
        *admission_open
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = true;
        scheduler.resume().map_err(runtime_scheduler_error)?;

        Ok(Self {
            phase: LivePhase::Running,
            startup_recovery,
            scheduler,
            worker,
            watcher_slot,
            last_watcher_snapshot,
            admission_open,
            reset_watcher,
            latest_refresh,
        })
    }

    #[must_use]
    pub const fn startup_recovery(&self) -> StartupRecoveryReport {
        self.startup_recovery
    }

    #[must_use]
    pub fn hints(&self) -> RefreshHintSink {
        self.scheduler.hints()
    }

    pub fn refresh_now(&self, urgency: RefreshUrgency) -> Result<(), RuntimeError> {
        if self.phase != LivePhase::Running {
            return Err(RuntimeError::new(match self.phase {
                LivePhase::Faulted => RuntimeErrorCode::Faulted,
                LivePhase::Running => RuntimeErrorCode::Internal,
                LivePhase::Paused | LivePhase::Stopping | LivePhase::Stopped => {
                    RuntimeErrorCode::Closed
                }
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

    pub fn snapshot(&self) -> Result<LiveRuntimeSnapshot, RuntimeError> {
        let scheduler = self.scheduler.snapshot();
        let worker = self.worker.snapshot().map_err(runtime_worker_error)?;
        let watcher = self
            .watcher_slot
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?
            .as_ref()
            .map_or(
                self.last_watcher_snapshot,
                BoundedFilesystemWatcher::snapshot,
            );
        let phase = if scheduler.phase() == SchedulerPhase::Faulted
            || worker.phase() == WorkerPhase::Faulted
        {
            LivePhase::Faulted
        } else {
            self.phase
        };
        let refresh = *self
            .latest_refresh
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        Ok(LiveRuntimeSnapshot {
            phase,
            scheduler,
            worker,
            watcher,
            refresh,
        })
    }

    pub fn pause(&mut self) -> Result<LivePhase, RuntimeError> {
        match self.phase {
            LivePhase::Paused => return Ok(LivePhase::Paused),
            LivePhase::Running => {}
            LivePhase::Faulted => return Err(RuntimeError::new(RuntimeErrorCode::Faulted)),
            LivePhase::Stopping | LivePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        let mut admission = match self.admission_open.lock() {
            Ok(admission) => admission,
            Err(_) => {
                self.phase = LivePhase::Faulted;
                return Err(RuntimeError::new(RuntimeErrorCode::Internal));
            }
        };
        *admission = false;
        if let Err(error) = self.scheduler.pause() {
            self.phase = LivePhase::Faulted;
            return Err(runtime_scheduler_error(error));
        }
        let snapshot = match self.worker.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.phase = LivePhase::Faulted;
                return Err(runtime_worker_error(error));
            }
        };
        if let Some(active) = snapshot.active_request_id()
            && let Err(error) = self.worker.cancel(active)
            && error.code() != WorkerErrorCode::StaleRequest
        {
            self.phase = LivePhase::Faulted;
            return Err(runtime_worker_error(error));
        }
        self.phase = LivePhase::Paused;
        Ok(self.phase)
    }

    pub fn resume(&mut self) -> Result<LivePhase, RuntimeError> {
        match self.phase {
            LivePhase::Running => return Ok(LivePhase::Running),
            LivePhase::Paused => {}
            LivePhase::Faulted => return Err(RuntimeError::new(RuntimeErrorCode::Faulted)),
            LivePhase::Stopping | LivePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        let mut admission = match self.admission_open.lock() {
            Ok(admission) => admission,
            Err(_) => {
                self.phase = LivePhase::Faulted;
                return Err(RuntimeError::new(RuntimeErrorCode::Internal));
            }
        };
        self.reset_watcher.store(true, Ordering::Release);
        *admission = true;
        if let Err(error) = self.scheduler.resume() {
            *admission = false;
            self.phase = LivePhase::Faulted;
            return Err(runtime_scheduler_error(error));
        }
        self.phase = LivePhase::Running;
        Ok(self.phase)
    }

    pub fn shutdown(&mut self) -> Result<LivePhase, RuntimeError> {
        if self.phase == LivePhase::Stopped {
            return Ok(self.phase);
        }
        self.phase = LivePhase::Stopping;
        let mut failed = false;
        match self.admission_open.lock() {
            Ok(mut admission) => *admission = false,
            Err(poisoned) => {
                *poisoned.into_inner() = false;
                failed = true;
            }
        }
        let watcher_snapshot = match self.watcher_slot.lock() {
            Ok(mut slot) => stop_watcher_slot(&mut slot),
            Err(poisoned) => {
                let snapshot = stop_watcher_slot(&mut poisoned.into_inner());
                failed = true;
                snapshot
            }
        };
        if let Some(snapshot) = watcher_snapshot {
            self.last_watcher_snapshot = snapshot;
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
            self.phase = LivePhase::Faulted;
            Err(RuntimeError::new(RuntimeErrorCode::Internal))
        } else {
            self.phase = LivePhase::Stopped;
            Ok(self.phase)
        }
    }
}

impl fmt::Debug for LiveRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveRuntime")
            .field("snapshot", &self.snapshot().ok())
            .field("startup_recovery", &self.startup_recovery)
            .finish()
    }
}

impl Drop for LiveRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct LiveExecution {
    clock: Arc<dyn Clock>,
    lease: crate::RuntimeWriterLease,
    adapter: CodexAdapter,
    archive: StoreArchive,
    watcher_slot: Arc<Mutex<Option<BoundedFilesystemWatcher>>>,
    reset_watcher: Arc<AtomicBool>,
    last_watch_roots: Vec<PathBuf>,
    watch_set_complete: bool,
    latest_refresh: Arc<Mutex<LiveRefreshSnapshot>>,
}

impl LiveExecution {
    fn run(&mut self, permit: &RefreshPermit) -> RefreshOutcome {
        let refresh = self.refresh(permit);
        let outcome = refresh.outcome().unwrap_or(RefreshOutcome::Failed);
        if outcome == RefreshOutcome::Completed {
            self.sync_watcher(permit.urgency());
        }
        if let Ok(mut latest) = self.latest_refresh.lock() {
            *latest = refresh;
        }
        outcome
    }

    fn refresh(&mut self, permit: &RefreshPermit) -> LiveRefreshSnapshot {
        let control = OperationControl::new(permit, self.clock.as_ref());
        if let Err(error) = control.check() {
            return refresh_error(LiveRefreshKind::None, error);
        }
        let guard = match self.lease.try_acquire() {
            Ok(guard) => guard,
            Err(error) => return refresh_error(LiveRefreshKind::None, error),
        };
        let mode = match self.archive.store().archive_state() {
            Ok(state) => state.mode(),
            Err(_) => {
                return LiveRefreshSnapshot::result(
                    LiveRefreshKind::None,
                    RefreshOutcome::Failed,
                    Some(PortErrorCode::Unavailable),
                );
            }
        };
        let quality = match self.archive.store().archive_publication() {
            Ok(publication) => publication.quality(),
            Err(_) => {
                return LiveRefreshSnapshot::result(
                    LiveRefreshKind::None,
                    RefreshOutcome::Failed,
                    Some(PortErrorCode::Unavailable),
                );
            }
        };
        let incremental = mode == ArchiveMode::ReplayVerified
            && matches!(
                quality,
                ArchivePublicationQuality::Complete | ArchivePublicationQuality::Partial
            );
        if incremental {
            match refresh_incremental(&mut self.adapter, &mut self.archive, &control) {
                Ok(report) if report.outcome() == IncrementalRefreshOutcome::RebuildRequired => {
                    self.full_rebuild(permit, guard)
                }
                Ok(_report) => {
                    drop(guard);
                    LiveRefreshSnapshot::result(
                        LiveRefreshKind::Incremental,
                        RefreshOutcome::Completed,
                        None,
                    )
                }
                Err(error) => {
                    drop(guard);
                    refresh_error(LiveRefreshKind::Incremental, error)
                }
            }
        } else {
            self.full_rebuild(permit, guard)
        }
    }

    fn full_rebuild(
        &mut self,
        permit: &RefreshPermit,
        guard: Box<dyn WriterLeaseGuard>,
    ) -> LiveRefreshSnapshot {
        let mut lease = PreAcquiredLease { guard: Some(guard) };
        let result = OneShotExecutor::new().run(
            permit,
            self.clock.as_ref(),
            &mut lease,
            &mut self.adapter,
            &mut self.archive,
        );
        LiveRefreshSnapshot::result(
            LiveRefreshKind::FullRebuild,
            result.outcome(),
            result.error(),
        )
    }

    fn sync_watcher(&mut self, urgency: RefreshUrgency) {
        let Some(roots) = self.adapter.watch_roots() else {
            return;
        };
        let reset = self.reset_watcher.swap(false, Ordering::AcqRel);
        let roots_changed = roots != self.last_watch_roots;
        let periodic_retry = !self.watch_set_complete && urgency == RefreshUrgency::Periodic;
        if !reset && !roots_changed && !periodic_retry {
            return;
        }
        let Ok(mut slot) = self.watcher_slot.lock() else {
            return;
        };
        let Some(watcher) = slot.as_mut() else {
            return;
        };
        let root_count = match watcher.replace_roots(&roots) {
            Ok(snapshot) => snapshot.root_count(),
            Err(_) => 0,
        };
        self.watch_set_complete = root_count == roots.len();
        self.last_watch_roots = roots;
    }
}

struct PreAcquiredLease {
    guard: Option<Box<dyn WriterLeaseGuard>>,
}

fn stop_watcher_slot(slot: &mut Option<BoundedFilesystemWatcher>) -> Option<WatcherSnapshot> {
    slot.take().map(|mut watcher| {
        let generation = watcher.snapshot().generation();
        watcher.shutdown();
        WatcherSnapshot::stopped(generation)
    })
}

impl WriterLease for PreAcquiredLease {
    fn try_acquire(&mut self) -> Result<Box<dyn WriterLeaseGuard>, PortError> {
        self.guard
            .take()
            .ok_or_else(|| PortError::new(PortErrorCode::Busy))
    }
}

fn outcome_for_port_error(error: PortError) -> RefreshOutcome {
    match error.code() {
        PortErrorCode::Busy => RefreshOutcome::Busy,
        PortErrorCode::Cancelled => RefreshOutcome::Cancelled,
        PortErrorCode::DeadlineExceeded => RefreshOutcome::DeadlineExceeded,
        PortErrorCode::InvalidData
        | PortErrorCode::CapacityExceeded
        | PortErrorCode::StaleState
        | PortErrorCode::RebuildRequired
        | PortErrorCode::Unavailable
        | PortErrorCode::Failed => RefreshOutcome::Failed,
    }
}

fn refresh_error(kind: LiveRefreshKind, error: PortError) -> LiveRefreshSnapshot {
    LiveRefreshSnapshot::result(kind, outcome_for_port_error(error), Some(error.code()))
}

fn startup_port_error(error: PortError) -> RuntimeError {
    RuntimeError::new(match error.code() {
        PortErrorCode::Busy => RuntimeErrorCode::Busy,
        PortErrorCode::InvalidData
        | PortErrorCode::Cancelled
        | PortErrorCode::DeadlineExceeded
        | PortErrorCode::CapacityExceeded
        | PortErrorCode::StaleState
        | PortErrorCode::RebuildRequired
        | PortErrorCode::Unavailable
        | PortErrorCode::Failed => RuntimeErrorCode::StoreUnavailable,
    })
}

fn runtime_worker_error(error: WorkerError) -> RuntimeError {
    RuntimeError::new(match error.code() {
        WorkerErrorCode::Closed | WorkerErrorCode::StaleRequest => RuntimeErrorCode::Closed,
        WorkerErrorCode::Faulted => RuntimeErrorCode::Faulted,
        WorkerErrorCode::CapacityExceeded
        | WorkerErrorCode::Unavailable
        | WorkerErrorCode::Internal => RuntimeErrorCode::Internal,
    })
}

fn runtime_scheduler_error(error: SchedulerError) -> RuntimeError {
    RuntimeError::new(match error.code() {
        SchedulerErrorCode::Closed => RuntimeErrorCode::Closed,
        SchedulerErrorCode::Faulted => RuntimeErrorCode::Faulted,
        SchedulerErrorCode::CapacityExceeded
        | SchedulerErrorCode::Unavailable
        | SchedulerErrorCode::Internal => RuntimeErrorCode::Internal,
    })
}
