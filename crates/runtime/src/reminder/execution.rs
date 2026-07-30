use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokenmaster_engine::{
    Clock, OperationControl, PortErrorCode, RefreshOutcome, RefreshPermit, WriterLease,
};
use tokenmaster_store::{
    BenefitReminderDelivery, BenefitReminderProcessResult, MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
    StoreErrorCode, UsageStore,
};

use super::health::{
    BenefitReminderFailure, BenefitReminderRefreshSnapshot, BenefitReminderRetryMode,
};
use super::runtime::ReminderScheduleControl;
use crate::RuntimeWriterLease;

pub(super) trait BenefitReminderWallClock: Send + Sync + 'static {
    fn now_millis(&self) -> Result<i64, ()>;
}

pub(super) struct SystemBenefitReminderWallClock;

impl BenefitReminderWallClock for SystemBenefitReminderWallClock {
    fn now_millis(&self) -> Result<i64, ()> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ())?
            .as_millis();
        i64::try_from(millis).map_err(|_| ())
    }
}

enum NotificationSlotState {
    Empty,
    Ready(Box<[BenefitReminderDelivery]>),
    Leased(Box<[BenefitReminderDelivery]>),
    Acknowledging(Box<[BenefitReminderDelivery]>),
}

pub(super) struct NotificationSlot {
    state: Mutex<NotificationSlotState>,
}

impl NotificationSlot {
    pub(super) const fn new() -> Self {
        Self {
            state: Mutex::new(NotificationSlotState::Empty),
        }
    }

    pub(super) fn publish(
        &self,
        deliveries: &[BenefitReminderDelivery],
    ) -> Result<bool, BenefitReminderFailure> {
        if deliveries.is_empty() {
            return Ok(false);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| BenefitReminderFailure::Control)?;
        if !matches!(*state, NotificationSlotState::Empty) {
            return Err(BenefitReminderFailure::Control);
        }
        *state = NotificationSlotState::Ready(deliveries.to_vec().into_boxed_slice());
        Ok(true)
    }

    pub(super) fn take_for_presentation(
        &self,
    ) -> Result<Option<Box<[BenefitReminderDelivery]>>, BenefitReminderFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BenefitReminderFailure::Control)?;
        let current = std::mem::replace(&mut *state, NotificationSlotState::Empty);
        match current {
            NotificationSlotState::Ready(batch) => {
                let presented = batch.clone();
                *state = NotificationSlotState::Leased(batch);
                Ok(Some(presented))
            }
            other => {
                *state = other;
                Ok(None)
            }
        }
    }

    pub(super) fn begin_acknowledgement(
        &self,
    ) -> Result<Option<Box<[BenefitReminderDelivery]>>, BenefitReminderFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BenefitReminderFailure::Control)?;
        let current = std::mem::replace(&mut *state, NotificationSlotState::Empty);
        match current {
            NotificationSlotState::Leased(batch) => {
                let acknowledgement = batch.clone();
                *state = NotificationSlotState::Acknowledging(batch);
                Ok(Some(acknowledgement))
            }
            other => {
                *state = other;
                Ok(None)
            }
        }
    }

    pub(super) fn finish_acknowledgement(
        &self,
        committed: bool,
    ) -> Result<(), BenefitReminderFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BenefitReminderFailure::Control)?;
        let current = std::mem::replace(&mut *state, NotificationSlotState::Empty);
        match current {
            NotificationSlotState::Acknowledging(batch) if !committed => {
                *state = NotificationSlotState::Leased(batch);
                Ok(())
            }
            NotificationSlotState::Acknowledging(_) => Ok(()),
            other => {
                *state = other;
                Err(BenefitReminderFailure::Control)
            }
        }
    }

    pub(super) fn release_presentation(&self) -> Result<bool, BenefitReminderFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BenefitReminderFailure::Control)?;
        let current = std::mem::replace(&mut *state, NotificationSlotState::Empty);
        match current {
            NotificationSlotState::Leased(batch) => {
                *state = NotificationSlotState::Ready(batch);
                Ok(true)
            }
            other => {
                *state = other;
                Ok(false)
            }
        }
    }
}

pub(super) struct BenefitReminderAcknowledger {
    wall_clock: Arc<dyn BenefitReminderWallClock>,
    archive_path: PathBuf,
    lease: RuntimeWriterLease,
}

impl BenefitReminderAcknowledger {
    pub(super) fn new(
        wall_clock: Arc<dyn BenefitReminderWallClock>,
        archive_path: &Path,
    ) -> Result<Self, crate::RuntimeError> {
        Ok(Self {
            wall_clock,
            archive_path: archive_path.to_path_buf(),
            lease: RuntimeWriterLease::new(archive_path)?,
        })
    }

    pub(super) fn acknowledge(
        &mut self,
        deliveries: &[BenefitReminderDelivery],
    ) -> Result<(), crate::RuntimeError> {
        let observed_at_ms = self
            .wall_clock
            .now_millis()
            .map_err(|_| crate::RuntimeError::new(crate::RuntimeErrorCode::Internal))?;
        let acknowledged_at_ms = deliveries
            .iter()
            .map(BenefitReminderDelivery::delivered_at_ms)
            .fold(observed_at_ms, i64::max);
        let guard = self.lease.try_acquire().map_err(|error| {
            crate::RuntimeError::new(match error.code() {
                PortErrorCode::Busy => crate::RuntimeErrorCode::Busy,
                PortErrorCode::Unavailable => crate::RuntimeErrorCode::StoreUnavailable,
                _ => crate::RuntimeErrorCode::Internal,
            })
        })?;
        let mut store = UsageStore::open(&self.archive_path).map_err(|error| {
            crate::RuntimeError::new(match error.code() {
                StoreErrorCode::Database
                | StoreErrorCode::VersionMismatch
                | StoreErrorCode::SchemaTooNew
                | StoreErrorCode::SchemaMismatch
                | StoreErrorCode::PolicyMismatch
                | StoreErrorCode::RebuildRequired => crate::RuntimeErrorCode::StoreUnavailable,
                _ => crate::RuntimeErrorCode::Internal,
            })
        })?;
        let result = store
            .acknowledge_benefit_reminders(deliveries, acknowledged_at_ms)
            .map_err(|error| {
                crate::RuntimeError::new(match error.code() {
                    StoreErrorCode::Database
                    | StoreErrorCode::VersionMismatch
                    | StoreErrorCode::SchemaTooNew
                    | StoreErrorCode::SchemaMismatch
                    | StoreErrorCode::PolicyMismatch
                    | StoreErrorCode::RebuildRequired => crate::RuntimeErrorCode::StoreUnavailable,
                    _ => crate::RuntimeErrorCode::Internal,
                })
            })?;
        drop(store);
        drop(guard);
        let completed = usize::from(result.acknowledged_count())
            .checked_add(usize::from(result.already_acknowledged_count()))
            .ok_or_else(|| crate::RuntimeError::new(crate::RuntimeErrorCode::Internal))?;
        if completed != deliveries.len() {
            return Err(crate::RuntimeError::new(crate::RuntimeErrorCode::Internal));
        }
        Ok(())
    }
}

pub(super) struct BenefitReminderExecution {
    monotonic_clock: Arc<dyn Clock>,
    wall_clock: Arc<dyn BenefitReminderWallClock>,
    archive_path: PathBuf,
    lease: RuntimeWriterLease,
    latest: Arc<Mutex<BenefitReminderRefreshSnapshot>>,
    notifications: Arc<NotificationSlot>,
    schedule: Arc<ReminderScheduleControl>,
}

impl BenefitReminderExecution {
    pub(super) fn new(
        monotonic_clock: Arc<dyn Clock>,
        wall_clock: Arc<dyn BenefitReminderWallClock>,
        archive_path: &Path,
        latest: Arc<Mutex<BenefitReminderRefreshSnapshot>>,
        notifications: Arc<NotificationSlot>,
        schedule: Arc<ReminderScheduleControl>,
    ) -> Result<Self, crate::RuntimeError> {
        Ok(Self {
            monotonic_clock,
            wall_clock,
            archive_path: archive_path.to_path_buf(),
            lease: RuntimeWriterLease::new(archive_path)?,
            latest,
            notifications,
            schedule,
        })
    }

    pub(super) fn run(&mut self, permit: &RefreshPermit) -> RefreshOutcome {
        let started_at = self.monotonic_clock.now().as_millis();
        let control = OperationControl::new(permit, self.monotonic_clock.as_ref());
        if let Err(error) = control.check() {
            return self.finish(
                started_at,
                AttemptResult::control(map_control_failure(error.code())),
            );
        }
        let observed_at_ms = match self.wall_clock.now_millis() {
            Ok(now) if now > 0 => now,
            _ => {
                return self.finish(
                    started_at,
                    AttemptResult::failed(BenefitReminderFailure::Clock, None),
                );
            }
        };
        if let Err(error) = control.check() {
            return self.finish(
                started_at,
                AttemptResult::control_at(map_control_failure(error.code()), observed_at_ms),
            );
        }
        let guard = match self.lease.try_acquire() {
            Ok(guard) => guard,
            Err(error) => {
                return self.finish(
                    started_at,
                    AttemptResult::failed(map_lease_failure(error.code()), Some(observed_at_ms)),
                );
            }
        };
        let mut store = match UsageStore::open(&self.archive_path) {
            Ok(store) => store,
            Err(error) => {
                drop(guard);
                return self.finish(
                    started_at,
                    AttemptResult::failed(map_store_failure(error.code()), Some(observed_at_ms)),
                );
            }
        };
        if let Err(error) = control.check() {
            drop(store);
            drop(guard);
            return self.finish(
                started_at,
                AttemptResult::control_at(map_control_failure(error.code()), observed_at_ms),
            );
        }
        let processed = match store.process_due_in_app_benefit_reminders(
            observed_at_ms,
            MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
        ) {
            Ok(processed) => processed,
            Err(error) => {
                drop(store);
                drop(guard);
                return self.finish(
                    started_at,
                    AttemptResult::failed(map_store_failure(error.code()), Some(observed_at_ms)),
                );
            }
        };
        drop(store);
        drop(guard);
        let notification_pending = match self.notifications.publish(processed.deliveries()) {
            Ok(pending) => pending,
            Err(failure) => {
                return self.finish(
                    started_at,
                    AttemptResult::failed(failure, Some(observed_at_ms)),
                );
            }
        };
        self.finish(
            started_at,
            AttemptResult::completed(observed_at_ms, processed, notification_pending),
        )
    }

    fn finish(&self, started_at: u64, result: AttemptResult) -> RefreshOutcome {
        let elapsed_millis = self
            .monotonic_clock
            .now()
            .as_millis()
            .saturating_sub(started_at);
        let outcome = result.outcome;
        let next_due = result.next_due_update;
        let notification_pending = result.notification_pending;
        let retry_mode = result.retry_mode;
        let observed_at_ms = result.observed_at_ms;
        let mut updated = false;
        if let Ok(mut latest) = self.latest.lock()
            && let Some(attempt_sequence) = latest.attempt_sequence.checked_add(1)
        {
            let last_success_observed_at_ms = if outcome == RefreshOutcome::Completed {
                observed_at_ms
            } else {
                latest.last_success_observed_at_ms
            };
            let nearest_due_at_ms = match next_due {
                Some(nearest_due_at_ms) => nearest_due_at_ms,
                None => latest.nearest_due_at_ms,
            };
            *latest = BenefitReminderRefreshSnapshot {
                attempt_sequence,
                outcome: Some(outcome),
                failure: result.failure,
                retry_mode,
                examined_count: result.examined_count,
                expired_count: result.expired_count,
                suppressed_count: result.suppressed_count,
                delivery_count: result.delivery_count,
                pending_due_count: result.pending_due_count,
                retained_delivery_count: result.retained_delivery_count,
                nearest_due_at_ms,
                observed_at_ms,
                elapsed_millis,
                last_success_observed_at_ms,
            };
            updated = true;
        }
        self.schedule
            .complete_attempt(next_due, retry_mode, notification_pending, observed_at_ms);
        if updated {
            outcome
        } else {
            RefreshOutcome::Failed
        }
    }
}

struct AttemptResult {
    outcome: RefreshOutcome,
    failure: Option<BenefitReminderFailure>,
    retry_mode: BenefitReminderRetryMode,
    examined_count: u16,
    expired_count: u16,
    suppressed_count: u16,
    delivery_count: u16,
    pending_due_count: u64,
    retained_delivery_count: u64,
    next_due_update: Option<Option<i64>>,
    observed_at_ms: Option<i64>,
    notification_pending: bool,
}

impl AttemptResult {
    fn completed(
        observed_at_ms: i64,
        processed: BenefitReminderProcessResult,
        notification_pending: bool,
    ) -> Self {
        Self {
            outcome: RefreshOutcome::Completed,
            failure: None,
            retry_mode: BenefitReminderRetryMode::Normal,
            examined_count: processed.examined_count(),
            expired_count: processed.expired_count(),
            suppressed_count: processed.suppressed_count(),
            delivery_count: u16::try_from(processed.delivery_count()).unwrap_or(u16::MAX),
            pending_due_count: processed.pending_due_count(),
            retained_delivery_count: processed.retained_delivery_count(),
            next_due_update: Some(processed.nearest_due_at_ms()),
            observed_at_ms: Some(observed_at_ms),
            notification_pending,
        }
    }

    fn failed(failure: BenefitReminderFailure, observed_at_ms: Option<i64>) -> Self {
        let retry_mode = if matches!(
            failure,
            BenefitReminderFailure::Busy | BenefitReminderFailure::StoreUnavailable
        ) {
            BenefitReminderRetryMode::Accelerated
        } else {
            BenefitReminderRetryMode::Normal
        };
        Self {
            outcome: match failure {
                BenefitReminderFailure::Busy => RefreshOutcome::Busy,
                BenefitReminderFailure::Cancelled => RefreshOutcome::Cancelled,
                BenefitReminderFailure::DeadlineExceeded => RefreshOutcome::DeadlineExceeded,
                _ => RefreshOutcome::Failed,
            },
            failure: Some(failure),
            retry_mode,
            examined_count: 0,
            expired_count: 0,
            suppressed_count: 0,
            delivery_count: 0,
            pending_due_count: 0,
            retained_delivery_count: 0,
            next_due_update: None,
            observed_at_ms,
            notification_pending: false,
        }
    }

    fn control(failure: BenefitReminderFailure) -> Self {
        Self::failed(failure, None)
    }

    fn control_at(failure: BenefitReminderFailure, observed_at_ms: i64) -> Self {
        Self::failed(failure, Some(observed_at_ms))
    }
}

const fn map_control_failure(code: PortErrorCode) -> BenefitReminderFailure {
    match code {
        PortErrorCode::Cancelled => BenefitReminderFailure::Cancelled,
        PortErrorCode::DeadlineExceeded => BenefitReminderFailure::DeadlineExceeded,
        _ => BenefitReminderFailure::Control,
    }
}

const fn map_lease_failure(code: PortErrorCode) -> BenefitReminderFailure {
    match code {
        PortErrorCode::Busy => BenefitReminderFailure::Busy,
        PortErrorCode::Cancelled => BenefitReminderFailure::Cancelled,
        PortErrorCode::DeadlineExceeded => BenefitReminderFailure::DeadlineExceeded,
        PortErrorCode::CapacityExceeded => BenefitReminderFailure::CapacityExceeded,
        PortErrorCode::InvalidData | PortErrorCode::StaleState => {
            BenefitReminderFailure::InvalidData
        }
        PortErrorCode::Unavailable | PortErrorCode::RebuildRequired | PortErrorCode::Failed => {
            BenefitReminderFailure::StoreUnavailable
        }
    }
}

const fn map_store_failure(code: StoreErrorCode) -> BenefitReminderFailure {
    match code {
        StoreErrorCode::CapacityExceeded => BenefitReminderFailure::CapacityExceeded,
        StoreErrorCode::DeadlineExceeded => BenefitReminderFailure::DeadlineExceeded,
        StoreErrorCode::Cancelled => BenefitReminderFailure::Cancelled,
        StoreErrorCode::Busy => BenefitReminderFailure::Busy,
        StoreErrorCode::Database
        | StoreErrorCode::BackupIo
        | StoreErrorCode::StaleBackupCandidate
        | StoreErrorCode::VersionMismatch
        | StoreErrorCode::SchemaTooNew
        | StoreErrorCode::SchemaMismatch
        | StoreErrorCode::PolicyMismatch
        | StoreErrorCode::RebuildRequired => BenefitReminderFailure::StoreUnavailable,
        StoreErrorCode::InvalidValue
        | StoreErrorCode::InvalidStoredValue
        | StoreErrorCode::AccountingVersionMismatch
        | StoreErrorCode::IncompleteManifest
        | StoreErrorCode::UnsealedRevision
        | StoreErrorCode::ArchiveModeMismatch
        | StoreErrorCode::StaleCheckpoint
        | StoreErrorCode::StaleRevision
        | StoreErrorCode::StaleScan
        | StoreErrorCode::PendingScan
        | StoreErrorCode::PendingContinuation
        | StoreErrorCode::ScanInProgress
        | StoreErrorCode::BackupHeaderCorrupt
        | StoreErrorCode::BackupPageCorrupt
        | StoreErrorCode::BackupIndexCorrupt
        | StoreErrorCode::BackupForeignKeyCorrupt
        | StoreErrorCode::BackupCountCorrupt
        | StoreErrorCode::BackupGenerationCorrupt
        | StoreErrorCode::BackupSemanticCorrupt => BenefitReminderFailure::InvalidData,
    }
}
