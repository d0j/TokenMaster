mod coordinator;
mod scheduler;
mod worker;

use core::fmt;
use std::sync::{Arc, Mutex};

use crate::{BackupPolicy, StateError};

pub use coordinator::{
    MaintenanceAdmission, MaintenanceCompletion, MaintenanceCoordinator,
    MaintenanceCoordinatorSnapshot, MaintenanceExecution, MaintenanceOutcome, MaintenancePermit,
    MaintenancePurpose, MaintenanceRejection, MaintenanceRequestId, MaintenanceSourceIdentity,
    MaintenanceSourceState, MaintenanceTransition, MaintenanceUrgency,
};
pub use scheduler::{
    MaintenanceClock, MaintenanceSchedule, MaintenanceScheduleSnapshot, MaintenanceSchedulerPhase,
    MaintenanceSchedulerSnapshot, MaintenanceTick, SystemMaintenanceClock,
};
pub use worker::{MaintenanceWorker, MaintenanceWorkerPhase, MaintenanceWorkerSnapshot};

use scheduler::{MaintenanceScheduler, SharedMaintenanceSchedule};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupMaintenanceRuntimeSnapshot {
    worker: MaintenanceWorkerSnapshot,
    scheduler: MaintenanceSchedulerSnapshot,
}

impl BackupMaintenanceRuntimeSnapshot {
    #[must_use]
    pub const fn worker(self) -> MaintenanceWorkerSnapshot {
        self.worker
    }

    #[must_use]
    pub const fn scheduler(self) -> MaintenanceSchedulerSnapshot {
        self.scheduler
    }
}

/// Owns exactly one backup worker and one automatic-backup scheduler.
pub struct BackupMaintenanceRuntime {
    worker: MaintenanceWorker,
    scheduler: MaintenanceScheduler,
}

impl BackupMaintenanceRuntime {
    pub fn spawn<F>(
        clock: Arc<dyn MaintenanceClock>,
        policy: BackupPolicy,
        source_state: MaintenanceSourceState,
        mut execute: F,
    ) -> Result<Self, StateError>
    where
        F: FnMut(&MaintenancePermit) -> MaintenanceExecution + Send + 'static,
    {
        let schedule: SharedMaintenanceSchedule = Arc::new(Mutex::new(MaintenanceSchedule::new(
            &policy,
            clock.now(),
            source_state,
        )));
        let worker_schedule = Arc::clone(&schedule);
        let worker_clock = Arc::clone(&clock);
        let worker =
            MaintenanceWorker::spawn(source_state, policy.periodic_enabled(), move |permit| {
                let execution = execute(permit);
                if matches!(execution, MaintenanceExecution::Published { .. })
                    && !permit.publication_started()
                {
                    return MaintenanceExecution::Failed(crate::StateErrorCode::InternalInvariant);
                }
                if matches!(execution, MaintenanceExecution::Published { .. }) {
                    let Ok(mut schedule) = worker_schedule.lock() else {
                        return MaintenanceExecution::Failed(
                            crate::StateErrorCode::InternalInvariant,
                        );
                    };
                    schedule.mark_healthy_publication(worker_clock.now());
                }
                execution
            })?;
        let scheduler = MaintenanceScheduler::spawn(clock, schedule, worker.submitter())?;
        Ok(Self { worker, scheduler })
    }

    pub fn submit(&self, purpose: MaintenancePurpose) -> MaintenanceAdmission {
        self.worker.submit(purpose)
    }

    pub fn record_durable_change(&self) -> Result<(), StateError> {
        self.scheduler.record_durable_change()
    }

    #[must_use]
    pub fn guard_completion(
        &self,
        root_request_id: MaintenanceRequestId,
    ) -> Option<MaintenanceCompletion> {
        self.worker
            .snapshot()
            .latest_guard_completion()
            .filter(|completion| completion.root_request_id() == root_request_id)
    }

    pub fn update_policy(&self, policy: &BackupPolicy) -> Result<(), StateError> {
        self.worker
            .set_periodic_enabled(policy.periodic_enabled())?;
        self.scheduler.update_policy(policy)
    }

    #[must_use]
    pub fn snapshot(&self) -> BackupMaintenanceRuntimeSnapshot {
        BackupMaintenanceRuntimeSnapshot {
            worker: self.worker.snapshot(),
            scheduler: self.scheduler.snapshot(),
        }
    }

    pub fn pause(&self) -> Result<(), StateError> {
        self.scheduler.pause()?;
        if let Err(error) = self.worker.pause() {
            let _ = self.scheduler.resume();
            return Err(error);
        }
        Ok(())
    }

    pub fn resume(&self) -> Result<(), StateError> {
        self.worker.resume()?;
        if let Err(error) = self.scheduler.resume() {
            let _ = self.worker.pause();
            return Err(error);
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), StateError> {
        let scheduler = self.scheduler.shutdown();
        let worker = self.worker.shutdown();
        scheduler?;
        worker?;
        Ok(())
    }
}

impl fmt::Debug for BackupMaintenanceRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupMaintenanceRuntime")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl Drop for BackupMaintenanceRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
