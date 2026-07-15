use tokenmaster_engine::{
    EngineErrorCode, MonotonicTime, RefreshAdmission, RefreshCoordinator, RefreshDeadline,
    RefreshOutcome, RefreshRequestId, RefreshUrgency,
};

fn time(milliseconds: u64) -> MonotonicTime {
    MonotonicTime::from_millis(milliseconds)
}

fn deadline(milliseconds: u64) -> RefreshDeadline {
    RefreshDeadline::from_millis(milliseconds)
}

fn started(admission: RefreshAdmission) -> tokenmaster_engine::RefreshPermit {
    match admission {
        RefreshAdmission::Started(permit) => permit,
        other => panic!("expected started admission, got {other:?}"),
    }
}

#[test]
fn idle_submission_starts_with_checked_monotonic_identity() {
    let mut coordinator = RefreshCoordinator::new();
    let permit = started(
        coordinator
            .submit(RefreshUrgency::Interactive, Some(deadline(50)), time(10))
            .expect("submit refresh"),
    );

    assert_eq!(permit.id(), RefreshRequestId::new(1).unwrap());
    assert_eq!(permit.urgency(), RefreshUrgency::Interactive);
    assert_eq!(permit.deadline(), Some(deadline(50)));
    assert!(!permit.is_cancelled());
    assert_eq!(coordinator.active_request_id(), Some(permit.id()));
    assert_eq!(coordinator.pending_count(), 0);
}

#[test]
fn expired_idle_submission_is_terminal_without_active_state() {
    let mut coordinator = RefreshCoordinator::new();
    let admission = coordinator
        .submit(RefreshUrgency::Hint, Some(deadline(10)), time(10))
        .expect("submit expired refresh");

    assert_eq!(
        admission,
        RefreshAdmission::DeadlineExceeded {
            request_id: RefreshRequestId::new(1).unwrap()
        }
    );
    assert_eq!(coordinator.active_request_id(), None);
    assert_eq!(coordinator.pending_count(), 0);
}

#[test]
fn ten_thousand_hints_collapse_to_one_highest_priority_follow_up() {
    let mut coordinator = RefreshCoordinator::new();
    let active = started(
        coordinator
            .submit(RefreshUrgency::Hint, None, time(1))
            .expect("start active refresh"),
    );

    for index in 0..10_000_u64 {
        let urgency = if index == 7_777 {
            RefreshUrgency::Recovery
        } else {
            RefreshUrgency::Periodic
        };
        let admission = coordinator
            .submit(urgency, None, time(2))
            .expect("coalesce refresh");
        assert!(matches!(
            admission,
            RefreshAdmission::Coalesced {
                active_request_id,
                ..
            } if active_request_id == active.id()
        ));
        assert_eq!(coordinator.pending_count(), 1);
    }

    let transition = coordinator
        .finish(active.id(), RefreshOutcome::Completed, time(3))
        .expect("finish active refresh");
    assert_eq!(transition.completed().outcome(), RefreshOutcome::Completed);
    let follow_up = transition.follow_up().expect("one follow-up");
    assert_eq!(follow_up.id(), RefreshRequestId::new(10_002).unwrap());
    assert_eq!(follow_up.urgency(), RefreshUrgency::Recovery);
    assert_eq!(follow_up.deadline(), None);
    assert_eq!(coordinator.pending_count(), 0);
    assert_eq!(coordinator.active_request_id(), Some(follow_up.id()));
}

#[test]
fn coalesced_deadlines_remain_live_while_any_request_is_live() {
    let mut coordinator = RefreshCoordinator::new();
    let active = started(
        coordinator
            .submit(RefreshUrgency::Hint, None, time(1))
            .unwrap(),
    );
    coordinator
        .submit(RefreshUrgency::Hint, Some(deadline(20)), time(2))
        .unwrap();
    coordinator
        .submit(RefreshUrgency::Interactive, Some(deadline(40)), time(2))
        .unwrap();

    let transition = coordinator
        .finish(active.id(), RefreshOutcome::Completed, time(25))
        .unwrap();
    let follow_up = transition.follow_up().expect("live merged deadline");
    assert_eq!(follow_up.urgency(), RefreshUrgency::Interactive);
    assert_eq!(follow_up.deadline(), Some(deadline(40)));
    assert!(!transition.pending_deadline_exceeded());
}

#[test]
fn expired_pending_work_does_not_start_a_follow_up() {
    let mut coordinator = RefreshCoordinator::new();
    let active = started(
        coordinator
            .submit(RefreshUrgency::Hint, None, time(1))
            .unwrap(),
    );
    coordinator
        .submit(RefreshUrgency::Periodic, Some(deadline(20)), time(2))
        .unwrap();

    let transition = coordinator
        .finish(active.id(), RefreshOutcome::Completed, time(20))
        .unwrap();
    assert!(transition.follow_up().is_none());
    assert!(transition.pending_deadline_exceeded());
    assert_eq!(coordinator.active_request_id(), None);
}

#[test]
fn active_deadline_dominates_nominal_success() {
    let mut coordinator = RefreshCoordinator::new();
    let permit = started(
        coordinator
            .submit(RefreshUrgency::Interactive, Some(deadline(20)), time(1))
            .unwrap(),
    );

    let transition = coordinator
        .finish(permit.id(), RefreshOutcome::Completed, time(20))
        .unwrap();
    assert_eq!(
        transition.completed().outcome(),
        RefreshOutcome::DeadlineExceeded
    );
    assert_eq!(coordinator.active_request_id(), None);
}

#[test]
fn cancellation_is_cooperative_and_dominates_nominal_success() {
    let mut coordinator = RefreshCoordinator::new();
    let permit = started(
        coordinator
            .submit(RefreshUrgency::Interactive, None, time(1))
            .unwrap(),
    );
    let token = permit.cancellation_token();
    coordinator
        .cancel(permit.id())
        .expect("cancel active refresh");

    assert!(token.is_cancelled());
    assert!(permit.is_cancelled());
    let transition = coordinator
        .finish(permit.id(), RefreshOutcome::Completed, time(2))
        .unwrap();
    assert_eq!(transition.completed().outcome(), RefreshOutcome::Cancelled);
}

#[test]
fn stale_completion_and_cancellation_cannot_mutate_a_newer_request() {
    let mut coordinator = RefreshCoordinator::new();
    let first = started(
        coordinator
            .submit(RefreshUrgency::Hint, None, time(1))
            .unwrap(),
    );
    coordinator
        .finish(first.id(), RefreshOutcome::Completed, time(2))
        .unwrap();
    let second = started(
        coordinator
            .submit(RefreshUrgency::Interactive, None, time(3))
            .unwrap(),
    );

    let finish_error = coordinator
        .finish(first.id(), RefreshOutcome::Failed, time(4))
        .expect_err("stale finish");
    assert_eq!(finish_error.code(), EngineErrorCode::StaleRequest);
    let cancel_error = coordinator.cancel(first.id()).expect_err("stale cancel");
    assert_eq!(cancel_error.code(), EngineErrorCode::StaleRequest);
    assert_eq!(coordinator.active_request_id(), Some(second.id()));
    assert!(!second.is_cancelled());
}

#[test]
fn busy_is_an_explicit_terminal_outcome_and_releases_the_slot() {
    let mut coordinator = RefreshCoordinator::new();
    let permit = started(
        coordinator
            .submit(RefreshUrgency::Periodic, None, time(1))
            .unwrap(),
    );
    let transition = coordinator
        .finish(permit.id(), RefreshOutcome::Busy, time(2))
        .unwrap();

    assert_eq!(transition.completed().outcome(), RefreshOutcome::Busy);
    assert_eq!(coordinator.active_request_id(), None);
}

#[test]
fn debug_and_errors_are_bounded_and_path_free() {
    let id = RefreshRequestId::new(7).unwrap();
    let id_debug = format!("{id:?}");
    assert_eq!(id_debug, "RefreshRequestId(7)");

    let invalid = RefreshRequestId::new(0).expect_err("zero request ID");
    assert_eq!(invalid.code(), EngineErrorCode::InvalidValue);
    assert_eq!(invalid.to_string(), "invalid_value");
    assert!(!format!("{invalid:?}").contains('\\'));
}
