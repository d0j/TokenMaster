use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tokenmaster_engine::{
    Clock, RefreshUrgency, RefreshWorker, WorkerCompletion, WorkerCompletionNotifier, WorkerError,
    WorkerErrorCode, WorkerPhase, WriterLease,
};
use tokenmaster_git::GitRepositoryFrontier;
use tokenmaster_platform::PowerLifecycleEvent;
use tokenmaster_provider::RepositoryActivityHint;
use tokenmaster_store::UsageStore;

use super::execution::GitExecution;
use super::{GitRefreshSnapshot, GitRuntimeConfig, GitRuntimePhase, GitRuntimeSnapshot};
use crate::{
    RefreshHintSink, RefreshScheduler, RuntimeError, RuntimeErrorCode, SchedulerError,
    SchedulerErrorCode, SchedulerPhase, SystemClock,
};

pub const MAX_GIT_RUNTIME_REPOSITORIES: usize = 32;

#[derive(Clone)]
pub(super) struct GitFrontierRecord {
    pub(super) frontier: GitRepositoryFrontier,
    pub(super) scan_revision: u64,
}

pub(super) struct GitHintSlot {
    pub(super) hint: RepositoryActivityHint,
    pub(super) sequence: u64,
    pub(super) frontier: Option<GitFrontierRecord>,
}

pub(super) struct GitHintState {
    pub(super) accepting: bool,
    pub(super) sequence: u64,
    pub(super) slots: VecDeque<GitHintSlot>,
    pub(super) dropped: u64,
}

#[derive(Clone)]
pub struct GitRepositoryHintIngress {
    state: Arc<Mutex<GitHintState>>,
    schedule: Arc<Mutex<Option<RefreshHintSink>>>,
}

impl GitRepositoryHintIngress {
    pub fn submit(&self, hint: RepositoryActivityHint) -> Result<(), RuntimeError> {
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
            if !state.accepting {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
            state.sequence = state
                .sequence
                .checked_add(1)
                .ok_or_else(|| RuntimeError::new(RuntimeErrorCode::Internal))?;
            let sequence = state.sequence;
            let position = state
                .slots
                .iter()
                .position(|slot| slot.hint.candidate().as_path() == hint.candidate().as_path());
            let frontier = position
                .and_then(|index| state.slots.remove(index))
                .and_then(|slot| slot.frontier);
            if position.is_none() && state.slots.len() == MAX_GIT_RUNTIME_REPOSITORIES {
                state.slots.pop_front();
                state.dropped = state.dropped.saturating_add(1);
            }
            state.slots.push_back(GitHintSlot {
                hint,
                sequence,
                frontier,
            });
        }
        let schedule = self
            .schedule
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        if schedule
            .as_ref()
            .is_some_and(RefreshHintSink::filesystem_changed)
        {
            Ok(())
        } else {
            Err(RuntimeError::new(RuntimeErrorCode::Closed))
        }
    }
}

impl fmt::Debug for GitRepositoryHintIngress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitRepositoryHintIngress([redacted])")
    }
}

pub struct GitRuntime {
    phase: GitRuntimePhase,
    scheduler: RefreshScheduler,
    worker: Arc<RefreshWorker>,
    admission_open: Arc<Mutex<bool>>,
    hint_state: Arc<Mutex<GitHintState>>,
    ingress: GitRepositoryHintIngress,
    latest: Arc<Mutex<GitRefreshSnapshot>>,
    scan_timeout: Duration,
}

impl GitRuntime {
    pub fn start(config: GitRuntimeConfig) -> Result<Self, RuntimeError> {
        Self::start_with_notifier(config, None)
    }

    pub fn start_notified(
        config: GitRuntimeConfig,
        notifier: Arc<dyn WorkerCompletionNotifier>,
    ) -> Result<Self, RuntimeError> {
        Self::start_with_notifier(config, Some(notifier))
    }

    fn start_with_notifier(
        config: GitRuntimeConfig,
        notifier: Option<Arc<dyn WorkerCompletionNotifier>>,
    ) -> Result<Self, RuntimeError> {
        let mut startup_lease = crate::RuntimeWriterLease::new(config.archive_path())?;
        let startup_guard = startup_lease
            .try_acquire()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Busy))?;
        let store = UsageStore::open(config.archive_path())
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::StoreUnavailable))?;
        let salt = store
            .git_identity_salt()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::StoreUnavailable))?;
        drop(store);
        drop(startup_guard);

        let clock: Arc<dyn Clock> = SystemClock::shared();
        let latest = Arc::new(Mutex::new(GitRefreshSnapshot::not_run()));
        let hint_state = Arc::new(Mutex::new(GitHintState {
            accepting: false,
            sequence: 0,
            slots: VecDeque::with_capacity(MAX_GIT_RUNTIME_REPOSITORIES),
            dropped: 0,
        }));
        let mut execution = GitExecution::new(
            config.clone(),
            salt,
            startup_lease,
            Arc::clone(&clock),
            Arc::clone(&hint_state),
            Arc::clone(&latest),
        );
        let worker = Arc::new(
            match notifier {
                Some(notifier) => {
                    RefreshWorker::spawn_notified(Arc::clone(&clock), notifier, move |permit| {
                        execution.run(permit)
                    })
                }
                None => {
                    RefreshWorker::spawn(Arc::clone(&clock), move |permit| execution.run(permit))
                }
            }
            .map_err(runtime_worker_error)?,
        );
        let admission_open = Arc::new(Mutex::new(false));
        let schedule = Arc::new(Mutex::new(None::<RefreshHintSink>));
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
        *schedule
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = Some(scheduler.hints());
        *admission_open
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = true;
        hint_state
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?
            .accepting = true;
        scheduler.resume().map_err(runtime_scheduler_error)?;
        let ingress = GitRepositoryHintIngress {
            state: Arc::clone(&hint_state),
            schedule,
        };
        Ok(Self {
            phase: GitRuntimePhase::Running,
            scheduler,
            worker,
            admission_open,
            hint_state,
            ingress,
            latest,
            scan_timeout: config.scan_timeout(),
        })
    }

    #[must_use]
    pub fn ingress(&self) -> GitRepositoryHintIngress {
        self.ingress.clone()
    }

    pub fn submit_hint(&self, hint: RepositoryActivityHint) -> Result<(), RuntimeError> {
        self.ingress.submit(hint)
    }

    pub fn refresh_now(&self) -> Result<(), RuntimeError> {
        if self.phase != GitRuntimePhase::Running {
            return Err(RuntimeError::new(match self.phase {
                GitRuntimePhase::Faulted => RuntimeErrorCode::Faulted,
                GitRuntimePhase::Running => RuntimeErrorCode::Internal,
                GitRuntimePhase::Paused | GitRuntimePhase::Stopping | GitRuntimePhase::Stopped => {
                    RuntimeErrorCode::Closed
                }
            }));
        }
        if self
            .scheduler
            .hints()
            .force_reconcile(RefreshUrgency::Interactive)
        {
            Ok(())
        } else {
            Err(RuntimeError::new(RuntimeErrorCode::Closed))
        }
    }

    pub fn try_completion(&self) -> Result<Option<WorkerCompletion>, RuntimeError> {
        self.worker.try_completion().map_err(runtime_worker_error)
    }

    pub fn snapshot(&self) -> Result<GitRuntimeSnapshot, RuntimeError> {
        let scheduler = self.scheduler.snapshot();
        let worker = self.worker.snapshot().map_err(runtime_worker_error)?;
        let refresh = *self
            .latest
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        let hints = self
            .hint_state
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
        let phase = if scheduler.phase() == SchedulerPhase::Faulted
            || worker.phase() == WorkerPhase::Faulted
        {
            GitRuntimePhase::Faulted
        } else {
            self.phase
        };
        Ok(GitRuntimeSnapshot {
            phase,
            scheduler,
            worker,
            refresh,
            retained_hint_count: hints.slots.len(),
            dropped_hint_count: hints.dropped,
        })
    }

    pub fn pause(&mut self) -> Result<GitRuntimePhase, RuntimeError> {
        match self.phase {
            GitRuntimePhase::Paused => return Ok(self.phase),
            GitRuntimePhase::Running => {}
            GitRuntimePhase::Faulted => return Err(RuntimeError::new(RuntimeErrorCode::Faulted)),
            GitRuntimePhase::Stopping | GitRuntimePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        *self
            .admission_open
            .lock()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))? = false;
        if let Err(error) = self.scheduler.pause() {
            self.phase = GitRuntimePhase::Faulted;
            return Err(runtime_scheduler_error(error));
        }
        {
            let mut hints = self
                .hint_state
                .lock()
                .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
            hints.accepting = false;
            for index in 0..hints.slots.len() {
                hints.sequence = hints
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| RuntimeError::new(RuntimeErrorCode::Internal))?;
                let sequence = hints.sequence;
                hints.slots[index].sequence = sequence;
                hints.slots[index].frontier = None;
            }
        }
        if let Some(active) = self
            .worker
            .snapshot()
            .map_err(runtime_worker_error)?
            .active_request_id()
            && let Err(error) = self.worker.cancel(active)
            && error.code() != WorkerErrorCode::StaleRequest
        {
            self.phase = GitRuntimePhase::Faulted;
            return Err(runtime_worker_error(error));
        }
        let deadline = Instant::now()
            .checked_add(self.scan_timeout + Duration::from_secs(1))
            .ok_or_else(|| RuntimeError::new(RuntimeErrorCode::Internal))?;
        while self
            .worker
            .snapshot()
            .map_err(runtime_worker_error)?
            .active_request_id()
            .is_some()
        {
            if Instant::now() >= deadline {
                self.phase = GitRuntimePhase::Faulted;
                return Err(RuntimeError::new(RuntimeErrorCode::Internal));
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.phase = GitRuntimePhase::Paused;
        Ok(self.phase)
    }

    pub fn resume(&mut self) -> Result<GitRuntimePhase, RuntimeError> {
        match self.phase {
            GitRuntimePhase::Running => return Ok(self.phase),
            GitRuntimePhase::Paused => {}
            GitRuntimePhase::Faulted => return Err(RuntimeError::new(RuntimeErrorCode::Faulted)),
            GitRuntimePhase::Stopping | GitRuntimePhase::Stopped => {
                return Err(RuntimeError::new(RuntimeErrorCode::Closed));
            }
        }
        {
            let mut hints = self
                .hint_state
                .lock()
                .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
            hints.accepting = true;
        }
        {
            let mut admission = match self.admission_open.lock() {
                Ok(admission) => admission,
                Err(_) => {
                    if let Ok(mut hints) = self.hint_state.lock() {
                        hints.accepting = false;
                    }
                    self.phase = GitRuntimePhase::Faulted;
                    return Err(RuntimeError::new(RuntimeErrorCode::Internal));
                }
            };
            *admission = true;
            if let Err(error) = self.scheduler.resume() {
                *admission = false;
                if let Ok(mut hints) = self.hint_state.lock() {
                    hints.accepting = false;
                }
                self.phase = GitRuntimePhase::Faulted;
                return Err(runtime_scheduler_error(error));
            }
        }
        self.phase = GitRuntimePhase::Running;
        if !self
            .scheduler
            .hints()
            .force_reconcile(RefreshUrgency::Recovery)
        {
            if let Ok(mut admission) = self.admission_open.lock() {
                *admission = false;
            }
            if let Ok(mut hints) = self.hint_state.lock() {
                hints.accepting = false;
            }
            let _ = self.scheduler.pause();
            self.phase = GitRuntimePhase::Faulted;
            return Err(RuntimeError::new(RuntimeErrorCode::Closed));
        }
        Ok(self.phase)
    }

    pub fn force_recovery(&self) -> Result<(), RuntimeError> {
        if self.phase != GitRuntimePhase::Running {
            return Err(RuntimeError::new(match self.phase {
                GitRuntimePhase::Faulted => RuntimeErrorCode::Faulted,
                GitRuntimePhase::Running => RuntimeErrorCode::Internal,
                GitRuntimePhase::Paused | GitRuntimePhase::Stopping | GitRuntimePhase::Stopped => {
                    RuntimeErrorCode::Closed
                }
            }));
        }
        {
            let mut hints = self
                .hint_state
                .lock()
                .map_err(|_| RuntimeError::new(RuntimeErrorCode::Internal))?;
            for index in 0..hints.slots.len() {
                hints.sequence = hints
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| RuntimeError::new(RuntimeErrorCode::Internal))?;
                let sequence = hints.sequence;
                hints.slots[index].sequence = sequence;
                hints.slots[index].frontier = None;
            }
        }
        if self
            .scheduler
            .hints()
            .force_reconcile(RefreshUrgency::Recovery)
        {
            Ok(())
        } else {
            Err(RuntimeError::new(RuntimeErrorCode::Closed))
        }
    }

    pub fn apply_power_event(
        &mut self,
        event: PowerLifecycleEvent,
    ) -> Result<GitRuntimePhase, RuntimeError> {
        match event {
            PowerLifecycleEvent::Suspend => self.pause(),
            PowerLifecycleEvent::Resume if self.phase == GitRuntimePhase::Running => {
                self.force_recovery()?;
                Ok(self.phase)
            }
            PowerLifecycleEvent::Resume => self.resume(),
        }
    }

    pub fn shutdown(&mut self) -> Result<GitRuntimePhase, RuntimeError> {
        if self.phase == GitRuntimePhase::Stopped {
            return Ok(self.phase);
        }
        self.phase = GitRuntimePhase::Stopping;
        let mut failed = false;
        match self.admission_open.lock() {
            Ok(mut admission) => *admission = false,
            Err(poisoned) => {
                *poisoned.into_inner() = false;
                failed = true;
            }
        }
        match self.hint_state.lock() {
            Ok(mut hints) => {
                hints.accepting = false;
                hints.slots.clear();
            }
            Err(poisoned) => {
                let mut hints = poisoned.into_inner();
                hints.accepting = false;
                hints.slots.clear();
                failed = true;
            }
        }
        let scheduler_phase = self.scheduler.shutdown().unwrap_or_else(|_| {
            failed = true;
            SchedulerPhase::Faulted
        });
        let worker_phase = match Arc::get_mut(&mut self.worker) {
            Some(worker) => worker.shutdown().unwrap_or_else(|_| {
                failed = true;
                WorkerPhase::Faulted
            }),
            None => {
                failed = true;
                WorkerPhase::Faulted
            }
        };
        if failed
            || scheduler_phase == SchedulerPhase::Faulted
            || worker_phase == WorkerPhase::Faulted
        {
            self.phase = GitRuntimePhase::Faulted;
            Err(RuntimeError::new(RuntimeErrorCode::Internal))
        } else {
            self.phase = GitRuntimePhase::Stopped;
            Ok(self.phase)
        }
    }
}

impl fmt::Debug for GitRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitRuntime")
            .field("snapshot", &self.snapshot().ok())
            .finish()
    }
}

impl Drop for GitRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn runtime_worker_error(error: WorkerError) -> RuntimeError {
    RuntimeError::new(match error.code() {
        WorkerErrorCode::Closed | WorkerErrorCode::StaleRequest => RuntimeErrorCode::Closed,
        WorkerErrorCode::Faulted => RuntimeErrorCode::Faulted,
        WorkerErrorCode::CapacityExceeded => RuntimeErrorCode::Busy,
        WorkerErrorCode::Unavailable | WorkerErrorCode::Internal => RuntimeErrorCode::Internal,
    })
}

fn runtime_scheduler_error(error: SchedulerError) -> RuntimeError {
    RuntimeError::new(match error.code() {
        SchedulerErrorCode::Closed => RuntimeErrorCode::Closed,
        SchedulerErrorCode::Faulted => RuntimeErrorCode::Faulted,
        SchedulerErrorCode::CapacityExceeded => RuntimeErrorCode::Busy,
        SchedulerErrorCode::Unavailable | SchedulerErrorCode::Internal => {
            RuntimeErrorCode::Internal
        }
    })
}
