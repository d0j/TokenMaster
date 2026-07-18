use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use tokenmaster_store::BackupControl;

use crate::{BackupPurpose, StateError, StateErrorCode};

const OPERATION_RUNNING: u8 = 0;
const OPERATION_CANCELLED: u8 = 1;
const OPERATION_PUBLISHING: u8 = 2;
const BACKUP_OPERATION_TIMEOUT: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MaintenanceUrgency {
    Periodic,
    SourceRetry,
    Manual,
    Mandatory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenancePurpose {
    Periodic,
    Manual,
    PreMigration,
    PostMigration,
    PreRestore,
    PreDestructiveMaintenance,
}

impl MaintenancePurpose {
    #[must_use]
    pub const fn urgency(self) -> MaintenanceUrgency {
        match self {
            Self::Periodic => MaintenanceUrgency::Periodic,
            Self::Manual => MaintenanceUrgency::Manual,
            Self::PreMigration
            | Self::PostMigration
            | Self::PreRestore
            | Self::PreDestructiveMaintenance => MaintenanceUrgency::Mandatory,
        }
    }

    #[must_use]
    pub const fn blocks_mutation(self) -> bool {
        matches!(
            self,
            Self::PreMigration | Self::PreRestore | Self::PreDestructiveMaintenance
        )
    }

    #[must_use]
    pub const fn backup_purpose(self) -> BackupPurpose {
        match self {
            Self::Periodic => BackupPurpose::Periodic,
            Self::Manual => BackupPurpose::Manual,
            Self::PreMigration => BackupPurpose::PreMigration,
            Self::PostMigration => BackupPurpose::PostMigration,
            Self::PreRestore => BackupPurpose::PreRestore,
            Self::PreDestructiveMaintenance => BackupPurpose::PreDestructiveMaintenance,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceSourceState {
    EmptyInstallation,
    HealthyUnpublished,
    Healthy,
    Suspect,
    CorruptQuarantined,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct MaintenanceSourceIdentity([u8; 32]);

impl MaintenanceSourceIdentity {
    #[must_use]
    pub const fn new(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl fmt::Debug for MaintenanceSourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MaintenanceSourceIdentity([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceRequestId(u64);

impl MaintenanceRequestId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone)]
pub struct MaintenancePermit {
    id: MaintenanceRequestId,
    root_request_id: MaintenanceRequestId,
    purpose: MaintenancePurpose,
    urgency: MaintenanceUrgency,
    operation_state: Arc<AtomicU8>,
    store_cancelled: Arc<AtomicBool>,
}

impl MaintenancePermit {
    fn new(id: MaintenanceRequestId, purpose: MaintenancePurpose) -> Self {
        Self {
            id,
            root_request_id: id,
            purpose,
            urgency: purpose.urgency(),
            operation_state: Arc::new(AtomicU8::new(OPERATION_RUNNING)),
            store_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    fn retry(id: MaintenanceRequestId, prior: &Self) -> Self {
        Self {
            id,
            root_request_id: prior.root_request_id,
            purpose: prior.purpose,
            urgency: MaintenanceUrgency::SourceRetry,
            operation_state: Arc::new(AtomicU8::new(OPERATION_RUNNING)),
            store_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    pub const fn id(&self) -> MaintenanceRequestId {
        self.id
    }

    #[must_use]
    pub const fn root_request_id(&self) -> MaintenanceRequestId {
        self.root_request_id
    }

    #[must_use]
    pub const fn purpose(&self) -> MaintenancePurpose {
        self.purpose
    }

    #[must_use]
    pub const fn urgency(&self) -> MaintenanceUrgency {
        self.urgency
    }

    /// Requests cooperative cancellation before final publication begins.
    #[must_use]
    pub fn cancel(&self) -> bool {
        let cancelled = self
            .operation_state
            .compare_exchange(
                OPERATION_RUNNING,
                OPERATION_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if cancelled {
            self.store_cancelled.store(true, Ordering::Release);
        }
        cancelled
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.operation_state.load(Ordering::Acquire) == OPERATION_CANCELLED
    }

    /// Enters the short final publish section, after which cancellation is rejected.
    pub fn begin_publication(&self) -> Result<(), StateError> {
        match self.operation_state.compare_exchange(
            OPERATION_RUNNING,
            OPERATION_PUBLISHING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(OPERATION_CANCELLED) => Err(StateError::unavailable()),
            Err(_) => Err(StateError::internal_invariant()),
        }
    }

    #[must_use]
    pub fn publication_started(&self) -> bool {
        self.operation_state.load(Ordering::Acquire) == OPERATION_PUBLISHING
    }

    /// Creates the exact store control linked to this permit's cancellation state.
    pub fn backup_control(&self) -> Result<BackupControl, StateError> {
        BackupControl::new(Arc::clone(&self.store_cancelled), BACKUP_OPERATION_TIMEOUT)
            .map_err(|_| StateError::internal_invariant())
    }
}

impl fmt::Debug for MaintenancePermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaintenancePermit")
            .field("id", &self.id)
            .field("root_request_id", &self.root_request_id)
            .field("purpose", &self.purpose)
            .field("urgency", &self.urgency)
            .field("cancelled", &self.is_cancelled())
            .field("publication_started", &self.publication_started())
            .finish()
    }
}

impl PartialEq for MaintenancePermit {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.root_request_id == other.root_request_id
            && self.purpose == other.purpose
            && self.urgency == other.urgency
            && Arc::ptr_eq(&self.operation_state, &other.operation_state)
            && Arc::ptr_eq(&self.store_cancelled, &other.store_cancelled)
    }
}

impl Eq for MaintenancePermit {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceRejection {
    Busy,
    PeriodicDisabled,
    SourceIneligible,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaintenanceAdmission {
    Started(MaintenancePermit),
    Coalesced {
        request_id: MaintenanceRequestId,
        active_request_id: MaintenanceRequestId,
    },
    BypassedEmptyInstallation,
    BypassedCorruptQuarantine,
    Rejected(MaintenanceRejection),
}

impl MaintenanceAdmission {
    #[must_use]
    pub const fn allows_guarded_mutation(&self) -> bool {
        matches!(
            self,
            Self::BypassedEmptyInstallation | Self::BypassedCorruptQuarantine
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceExecution {
    Published { bytes: u64 },
    SourceFailed { identity: MaintenanceSourceIdentity },
    Cancelled,
    Failed(StateErrorCode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceOutcome {
    Published,
    RetryScheduled,
    SourceSuspect,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceCompletion {
    request_id: MaintenanceRequestId,
    root_request_id: MaintenanceRequestId,
    purpose: MaintenancePurpose,
    outcome: MaintenanceOutcome,
    source_state: MaintenanceSourceState,
    failure_code: Option<StateErrorCode>,
    published_bytes: u64,
}

impl MaintenanceCompletion {
    #[must_use]
    pub const fn request_id(self) -> MaintenanceRequestId {
        self.request_id
    }

    #[must_use]
    pub const fn root_request_id(self) -> MaintenanceRequestId {
        self.root_request_id
    }

    #[must_use]
    pub const fn purpose(self) -> MaintenancePurpose {
        self.purpose
    }

    #[must_use]
    pub const fn outcome(self) -> MaintenanceOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn source_state(self) -> MaintenanceSourceState {
        self.source_state
    }

    #[must_use]
    pub const fn failure_code(self) -> Option<StateErrorCode> {
        self.failure_code
    }

    #[must_use]
    pub const fn published_bytes(self) -> u64 {
        self.published_bytes
    }

    #[must_use]
    pub const fn allows_mutation(self) -> bool {
        !self.purpose.blocks_mutation() || matches!(self.outcome, MaintenanceOutcome::Published)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaintenanceTransition {
    completion: MaintenanceCompletion,
    follow_up: Option<MaintenancePermit>,
}

impl MaintenanceTransition {
    #[must_use]
    pub const fn completion(&self) -> MaintenanceCompletion {
        self.completion
    }

    #[must_use]
    pub const fn follow_up(&self) -> Option<&MaintenancePermit> {
        self.follow_up.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceCoordinatorSnapshot {
    active_purpose: Option<MaintenancePurpose>,
    pending_purpose: Option<MaintenancePurpose>,
    source_state: MaintenanceSourceState,
    periodic_enabled: bool,
}

impl MaintenanceCoordinatorSnapshot {
    #[must_use]
    pub const fn active_count(self) -> usize {
        if self.active_purpose.is_some() { 1 } else { 0 }
    }

    #[must_use]
    pub const fn pending_count(self) -> usize {
        if self.pending_purpose.is_some() { 1 } else { 0 }
    }

    #[must_use]
    pub const fn active_purpose(self) -> Option<MaintenancePurpose> {
        self.active_purpose
    }

    #[must_use]
    pub const fn pending_purpose(self) -> Option<MaintenancePurpose> {
        self.pending_purpose
    }

    #[must_use]
    pub const fn source_state(self) -> MaintenanceSourceState {
        self.source_state
    }

    #[must_use]
    pub const fn periodic_enabled(self) -> bool {
        self.periodic_enabled
    }
}

#[derive(Clone, Copy)]
struct PendingRequest {
    id: MaintenanceRequestId,
    root_request_id: MaintenanceRequestId,
    purpose: MaintenancePurpose,
    urgency: MaintenanceUrgency,
}

struct ActiveRequest {
    permit: MaintenancePermit,
    pending: Option<PendingRequest>,
}

pub struct MaintenanceCoordinator {
    next_request_id: Option<u64>,
    active: Option<ActiveRequest>,
    source_state: MaintenanceSourceState,
    periodic_enabled: bool,
    last_source_failure: Option<(MaintenanceSourceIdentity, u8)>,
}

impl MaintenanceCoordinator {
    #[must_use]
    pub const fn new(source_state: MaintenanceSourceState, periodic_enabled: bool) -> Self {
        Self {
            next_request_id: Some(1),
            active: None,
            source_state,
            periodic_enabled,
            last_source_failure: None,
        }
    }

    pub fn submit(&mut self, purpose: MaintenancePurpose) -> MaintenanceAdmission {
        if purpose.blocks_mutation() {
            match self.source_state {
                MaintenanceSourceState::EmptyInstallation => {
                    return MaintenanceAdmission::BypassedEmptyInstallation;
                }
                MaintenanceSourceState::CorruptQuarantined => {
                    return MaintenanceAdmission::BypassedCorruptQuarantine;
                }
                MaintenanceSourceState::HealthyUnpublished | MaintenanceSourceState::Healthy => {}
                MaintenanceSourceState::Suspect => {
                    return MaintenanceAdmission::Rejected(MaintenanceRejection::SourceIneligible);
                }
            }
        } else if matches!(
            self.source_state,
            MaintenanceSourceState::EmptyInstallation
                | MaintenanceSourceState::Suspect
                | MaintenanceSourceState::CorruptQuarantined
        ) || (purpose == MaintenancePurpose::Periodic
            && self.source_state != MaintenanceSourceState::Healthy)
        {
            return MaintenanceAdmission::Rejected(MaintenanceRejection::SourceIneligible);
        }
        if purpose == MaintenancePurpose::Periodic && !self.periodic_enabled {
            return MaintenanceAdmission::Rejected(MaintenanceRejection::PeriodicDisabled);
        }
        let Some(request_id) = self.allocate_request_id() else {
            return MaintenanceAdmission::Rejected(MaintenanceRejection::Closed);
        };
        if let Some(active) = &mut self.active {
            if purpose.blocks_mutation()
                && (active.permit.purpose().blocks_mutation()
                    || active
                        .pending
                        .is_some_and(|pending| pending.purpose.blocks_mutation()))
            {
                return MaintenanceAdmission::Rejected(MaintenanceRejection::Busy);
            }
            match &mut active.pending {
                Some(pending) if purpose.urgency() > pending.urgency => {
                    *pending = PendingRequest {
                        id: request_id,
                        root_request_id: request_id,
                        purpose,
                        urgency: purpose.urgency(),
                    };
                }
                Some(_) => {}
                None => {
                    active.pending = Some(PendingRequest {
                        id: request_id,
                        root_request_id: request_id,
                        purpose,
                        urgency: purpose.urgency(),
                    });
                }
            }
            return MaintenanceAdmission::Coalesced {
                request_id,
                active_request_id: active.permit.id(),
            };
        }
        let permit = MaintenancePermit::new(request_id, purpose);
        self.active = Some(ActiveRequest {
            permit: permit.clone(),
            pending: None,
        });
        MaintenanceAdmission::Started(permit)
    }

    pub fn finish(
        &mut self,
        request_id: MaintenanceRequestId,
        execution: MaintenanceExecution,
    ) -> Result<MaintenanceTransition, StateError> {
        let active = self
            .active
            .take()
            .ok_or_else(StateError::internal_invariant)?;
        if active.permit.id() != request_id {
            self.active = Some(active);
            return Err(StateError::internal_invariant());
        }
        let execution = match execution {
            MaintenanceExecution::Published { .. } if !active.permit.publication_started() => {
                MaintenanceExecution::Failed(StateErrorCode::InternalInvariant)
            }
            MaintenanceExecution::Cancelled if active.permit.publication_started() => {
                MaintenanceExecution::Failed(StateErrorCode::InternalInvariant)
            }
            _ if active.permit.is_cancelled() => MaintenanceExecution::Cancelled,
            execution => execution,
        };
        let mut pending = active.pending;
        let (outcome, failure_code, published_bytes) = match execution {
            MaintenanceExecution::Published { bytes } => {
                self.source_state = MaintenanceSourceState::Healthy;
                self.last_source_failure = None;
                (MaintenanceOutcome::Published, None, bytes)
            }
            MaintenanceExecution::SourceFailed { identity } => {
                let failure_count = self
                    .last_source_failure
                    .filter(|(previous, _)| *previous == identity)
                    .map_or(1, |(_, count)| count.saturating_add(1));
                self.last_source_failure = Some((identity, failure_count));
                if failure_count >= 2 {
                    self.source_state = MaintenanceSourceState::Suspect;
                    pending = None;
                    (
                        MaintenanceOutcome::SourceSuspect,
                        Some(StateErrorCode::Integrity),
                        0,
                    )
                } else {
                    let retry = PendingRequest {
                        id: self
                            .allocate_request_id()
                            .ok_or_else(StateError::capacity_exceeded)?,
                        root_request_id: active.permit.root_request_id(),
                        purpose: active.permit.purpose(),
                        urgency: MaintenanceUrgency::SourceRetry,
                    };
                    match &mut pending {
                        Some(current)
                            if active.permit.purpose().blocks_mutation()
                                || retry.urgency > current.urgency =>
                        {
                            *current = retry;
                        }
                        Some(_) => {}
                        None => pending = Some(retry),
                    }
                    (
                        MaintenanceOutcome::RetryScheduled,
                        Some(StateErrorCode::Integrity),
                        0,
                    )
                }
            }
            MaintenanceExecution::Cancelled => (MaintenanceOutcome::Cancelled, None, 0),
            MaintenanceExecution::Failed(code) => (MaintenanceOutcome::Failed, Some(code), 0),
        };
        let completion = MaintenanceCompletion {
            request_id,
            root_request_id: active.permit.root_request_id(),
            purpose: active.permit.purpose(),
            outcome,
            source_state: self.source_state,
            failure_code,
            published_bytes,
        };
        let follow_up = pending.map(|pending| {
            if pending.root_request_id == pending.id {
                MaintenancePermit::new(pending.id, pending.purpose)
            } else {
                MaintenancePermit::retry(pending.id, &active.permit)
            }
        });
        if let Some(permit) = &follow_up {
            self.active = Some(ActiveRequest {
                permit: permit.clone(),
                pending: None,
            });
        }
        Ok(MaintenanceTransition {
            completion,
            follow_up,
        })
    }

    #[must_use]
    pub const fn snapshot(&self) -> MaintenanceCoordinatorSnapshot {
        MaintenanceCoordinatorSnapshot {
            active_purpose: match &self.active {
                Some(active) => Some(active.permit.purpose()),
                None => None,
            },
            pending_purpose: match &self.active {
                Some(active) => match active.pending {
                    Some(pending) => Some(pending.purpose),
                    None => None,
                },
                None => None,
            },
            source_state: self.source_state,
            periodic_enabled: self.periodic_enabled,
        }
    }

    pub(crate) fn cancel_active(&self) {
        if let Some(active) = &self.active {
            let _ = active.permit.cancel();
        }
    }

    pub(crate) fn set_periodic_enabled(&mut self, enabled: bool) {
        self.periodic_enabled = enabled;
        if !enabled
            && let Some(active) = &mut self.active
            && active.pending.is_some_and(|pending| {
                pending.purpose == MaintenancePurpose::Periodic
                    && pending.urgency == MaintenanceUrgency::Periodic
            })
        {
            active.pending = None;
        }
    }

    fn allocate_request_id(&mut self) -> Option<MaintenanceRequestId> {
        let value = self.next_request_id?;
        self.next_request_id = value.checked_add(1);
        Some(MaintenanceRequestId(value))
    }
}

impl fmt::Debug for MaintenanceCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaintenanceCoordinator")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}
