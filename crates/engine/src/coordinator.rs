use core::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{EngineError, EngineErrorCode, MonotonicTime, RefreshDeadline, RefreshRequestId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RefreshUrgency {
    Hint,
    Periodic,
    Interactive,
    Recovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshOutcome {
    Completed,
    Busy,
    Cancelled,
    DeadlineExceeded,
    Failed,
}

#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl fmt::Debug for CancellationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl PartialEq for CancellationToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

impl Eq for CancellationToken {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshPermit {
    id: RefreshRequestId,
    urgency: RefreshUrgency,
    deadline: Option<RefreshDeadline>,
    cancellation: CancellationToken,
}

impl RefreshPermit {
    fn new(
        id: RefreshRequestId,
        urgency: RefreshUrgency,
        deadline: Option<RefreshDeadline>,
    ) -> Self {
        Self {
            id,
            urgency,
            deadline,
            cancellation: CancellationToken::new(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> RefreshRequestId {
        self.id
    }

    #[must_use]
    pub const fn urgency(&self) -> RefreshUrgency {
        self.urgency
    }

    #[must_use]
    pub const fn deadline(&self) -> Option<RefreshDeadline> {
        self.deadline
    }

    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    #[must_use]
    pub fn deadline_exceeded(&self, now: MonotonicTime) -> bool {
        self.deadline
            .is_some_and(|deadline| deadline.is_exceeded_at(now))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefreshAdmission {
    Started(RefreshPermit),
    Coalesced {
        request_id: RefreshRequestId,
        active_request_id: RefreshRequestId,
    },
    DeadlineExceeded {
        request_id: RefreshRequestId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefreshResult {
    request_id: RefreshRequestId,
    outcome: RefreshOutcome,
}

impl RefreshResult {
    #[must_use]
    pub const fn request_id(self) -> RefreshRequestId {
        self.request_id
    }

    #[must_use]
    pub const fn outcome(self) -> RefreshOutcome {
        self.outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinishTransition {
    completed: RefreshResult,
    follow_up: Option<RefreshPermit>,
    pending_deadline_exceeded: bool,
    pending_capacity_exceeded: bool,
}

impl FinishTransition {
    #[must_use]
    pub const fn completed(&self) -> RefreshResult {
        self.completed
    }

    #[must_use]
    pub const fn follow_up(&self) -> Option<&RefreshPermit> {
        self.follow_up.as_ref()
    }

    #[must_use]
    pub const fn pending_deadline_exceeded(&self) -> bool {
        self.pending_deadline_exceeded
    }

    #[must_use]
    pub const fn pending_capacity_exceeded(&self) -> bool {
        self.pending_capacity_exceeded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingRefresh {
    urgency: RefreshUrgency,
    deadline: Option<RefreshDeadline>,
}

impl PendingRefresh {
    fn merge(&mut self, urgency: RefreshUrgency, deadline: Option<RefreshDeadline>) {
        self.urgency = self.urgency.max(urgency);
        self.deadline = match (self.deadline, deadline) {
            (None, _) | (_, None) => None,
            (Some(current), Some(incoming)) => Some(current.max(incoming)),
        };
    }

    fn deadline_exceeded(self, now: MonotonicTime) -> bool {
        self.deadline
            .is_some_and(|deadline| deadline.is_exceeded_at(now))
    }
}

struct ActiveRefresh {
    permit: RefreshPermit,
    pending: Option<PendingRefresh>,
}

pub struct RefreshCoordinator {
    next_request_id: Option<u64>,
    active: Option<ActiveRefresh>,
}

impl Default for RefreshCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl RefreshCoordinator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_request_id: Some(1),
            active: None,
        }
    }

    fn allocate_request_id(&mut self) -> Result<RefreshRequestId, EngineError> {
        let value = self
            .next_request_id
            .ok_or_else(|| EngineError::new(EngineErrorCode::CapacityExceeded))?;
        let id = RefreshRequestId::new(value)?;
        self.next_request_id = value.checked_add(1);
        Ok(id)
    }

    pub fn submit(
        &mut self,
        urgency: RefreshUrgency,
        deadline: Option<RefreshDeadline>,
        now: MonotonicTime,
    ) -> Result<RefreshAdmission, EngineError> {
        let request_id = self.allocate_request_id()?;
        if deadline.is_some_and(|deadline| deadline.is_exceeded_at(now)) {
            return Ok(RefreshAdmission::DeadlineExceeded { request_id });
        }

        if let Some(active) = &mut self.active {
            match &mut active.pending {
                Some(pending) => pending.merge(urgency, deadline),
                None => active.pending = Some(PendingRefresh { urgency, deadline }),
            }
            return Ok(RefreshAdmission::Coalesced {
                request_id,
                active_request_id: active.permit.id(),
            });
        }

        let permit = RefreshPermit::new(request_id, urgency, deadline);
        self.active = Some(ActiveRefresh {
            permit: permit.clone(),
            pending: None,
        });
        Ok(RefreshAdmission::Started(permit))
    }

    pub fn cancel(&mut self, request_id: RefreshRequestId) -> Result<(), EngineError> {
        let active = self
            .active
            .as_ref()
            .filter(|active| active.permit.id() == request_id)
            .ok_or_else(|| EngineError::new(EngineErrorCode::StaleRequest))?;
        active.permit.cancellation.cancel();
        Ok(())
    }

    pub fn finish(
        &mut self,
        request_id: RefreshRequestId,
        outcome: RefreshOutcome,
        now: MonotonicTime,
    ) -> Result<FinishTransition, EngineError> {
        let active = self
            .active
            .take()
            .ok_or_else(|| EngineError::new(EngineErrorCode::StaleRequest))?;
        if active.permit.id() != request_id {
            self.active = Some(active);
            return Err(EngineError::new(EngineErrorCode::StaleRequest));
        }

        let outcome = if active.permit.is_cancelled() {
            RefreshOutcome::Cancelled
        } else if outcome == RefreshOutcome::Completed && active.permit.deadline_exceeded(now) {
            RefreshOutcome::DeadlineExceeded
        } else {
            outcome
        };
        let completed = RefreshResult {
            request_id,
            outcome,
        };
        let mut transition = FinishTransition {
            completed,
            follow_up: None,
            pending_deadline_exceeded: false,
            pending_capacity_exceeded: false,
        };

        let Some(pending) = active.pending else {
            return Ok(transition);
        };
        if pending.deadline_exceeded(now) {
            transition.pending_deadline_exceeded = true;
            return Ok(transition);
        }
        let follow_up_id = match self.allocate_request_id() {
            Ok(id) => id,
            Err(error) if error.code() == EngineErrorCode::CapacityExceeded => {
                transition.pending_capacity_exceeded = true;
                return Ok(transition);
            }
            Err(error) => return Err(error),
        };
        let follow_up = RefreshPermit::new(follow_up_id, pending.urgency, pending.deadline);
        self.active = Some(ActiveRefresh {
            permit: follow_up.clone(),
            pending: None,
        });
        transition.follow_up = Some(follow_up);
        Ok(transition)
    }

    #[must_use]
    pub fn active_request_id(&self) -> Option<RefreshRequestId> {
        self.active.as_ref().map(|active| active.permit.id())
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        usize::from(
            self.active
                .as_ref()
                .is_some_and(|active| active.pending.is_some()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_id_exhaustion_never_wraps_or_reopens_the_slot() {
        let mut coordinator = RefreshCoordinator {
            next_request_id: Some(u64::MAX),
            active: None,
        };
        let permit =
            match coordinator.submit(RefreshUrgency::Hint, None, MonotonicTime::from_millis(0)) {
                Ok(RefreshAdmission::Started(permit)) => permit,
                result => panic!("unexpected admission: {result:?}"),
            };
        assert_eq!(permit.id().get(), u64::MAX);
        if let Err(error) = coordinator.finish(
            permit.id(),
            RefreshOutcome::Completed,
            MonotonicTime::from_millis(1),
        ) {
            panic!("finish failed: {error}");
        }

        let error =
            match coordinator.submit(RefreshUrgency::Hint, None, MonotonicTime::from_millis(2)) {
                Ok(admission) => panic!("exhausted coordinator admitted: {admission:?}"),
                Err(error) => error,
            };
        assert_eq!(error.code(), EngineErrorCode::CapacityExceeded);
        assert_eq!(coordinator.active_request_id(), None);
    }

    #[test]
    fn follow_up_id_exhaustion_is_explicit_after_current_completion() {
        let mut coordinator = RefreshCoordinator {
            next_request_id: Some(u64::MAX - 1),
            active: None,
        };
        let active =
            match coordinator.submit(RefreshUrgency::Hint, None, MonotonicTime::from_millis(0)) {
                Ok(RefreshAdmission::Started(permit)) => permit,
                result => panic!("unexpected active admission: {result:?}"),
            };
        match coordinator.submit(
            RefreshUrgency::Recovery,
            None,
            MonotonicTime::from_millis(0),
        ) {
            Ok(RefreshAdmission::Coalesced { request_id, .. }) => {
                assert_eq!(request_id.get(), u64::MAX);
            }
            result => panic!("unexpected pending admission: {result:?}"),
        }

        let transition = match coordinator.finish(
            active.id(),
            RefreshOutcome::Completed,
            MonotonicTime::from_millis(1),
        ) {
            Ok(transition) => transition,
            Err(error) => panic!("finish failed: {error}"),
        };
        assert!(transition.follow_up().is_none());
        assert!(transition.pending_capacity_exceeded());
        assert_eq!(coordinator.active_request_id(), None);
    }
}
