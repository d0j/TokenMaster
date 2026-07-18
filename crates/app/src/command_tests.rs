use super::command::{
    ApplicationBackupSelection, ApplicationCommand, ApplicationCommandAdmission,
    ApplicationCommandCoordinator, ApplicationCommandExecution, ApplicationCommandFailure,
    ApplicationCommandOutcome, ApplicationCommandRejection,
};

#[test]
fn ten_thousand_identical_hints_retain_one_active_command() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::Backup)
    else {
        panic!("first command must start");
    };

    for _ in 0..10_000 {
        assert_eq!(
            coordinator.submit(ApplicationCommand::Backup),
            ApplicationCommandAdmission::Coalesced {
                request_id: active.id(),
                active_request_id: active.id(),
            }
        );
    }

    let snapshot = coordinator.snapshot();
    assert_eq!(snapshot.active_count(), 1);
    assert_eq!(snapshot.pending_count(), 0);
    assert_eq!(snapshot.active_command(), Some(ApplicationCommand::Backup));
    assert_eq!(active.id().get(), 1);
    let completion = coordinator
        .finish(active.id(), ApplicationCommandExecution::Succeeded)
        .expect("successful completion")
        .completion();
    assert_eq!(completion.command(), ApplicationCommand::Backup);
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Succeeded);
}

#[test]
fn one_follow_up_is_bounded_and_a_third_distinct_command_is_busy() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::ExportConfig)
    else {
        panic!("first command must start");
    };
    let queued = coordinator.submit(ApplicationCommand::ImportConfig);
    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        active_request_id,
    } = queued
    else {
        panic!("one follow-up must queue");
    };
    assert_eq!(active_request_id, active.id());

    for _ in 0..10_000 {
        assert_eq!(
            coordinator.submit(ApplicationCommand::ImportConfig),
            ApplicationCommandAdmission::Coalesced {
                request_id: pending,
                active_request_id: active.id(),
            }
        );
    }
    assert_eq!(
        coordinator.submit(ApplicationCommand::Verify),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Busy)
    );
    assert_eq!(coordinator.snapshot().active_count(), 1);
    assert_eq!(coordinator.snapshot().pending_count(), 1);
}

#[test]
fn cancellation_is_exact_and_stops_at_the_irreversible_boundary() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(cancelled) =
        coordinator.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild must start");
    };
    assert!(coordinator.cancel(cancelled.id()));
    assert!(cancelled.is_cancelled());
    assert!(cancelled.begin_irreversible().is_err());
    coordinator
        .finish(cancelled.id(), ApplicationCommandExecution::Cancelled)
        .expect("cancelled completion");

    let ApplicationCommandAdmission::Started(committing) =
        coordinator.submit(ApplicationCommand::ImportConfig)
    else {
        panic!("import must start");
    };
    committing
        .begin_irreversible()
        .expect("irreversible boundary");
    assert!(!coordinator.cancel(committing.id()));
    assert!(!committing.is_cancelled());
}

#[test]
fn pending_restore_is_typed_and_can_be_cancelled_before_start() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::Backup)
    else {
        panic!("backup must start");
    };
    let selection = ApplicationBackupSelection::new(7, 3).expect("bounded selection");
    assert_eq!(selection.catalog_generation(), 7);
    assert_eq!(selection.ordinal(), 3);
    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        ..
    } = coordinator.submit(ApplicationCommand::RestoreData(selection))
    else {
        panic!("restore must queue");
    };
    assert!(coordinator.cancel(pending));
    assert_eq!(coordinator.snapshot().pending_count(), 0);
    assert!(!coordinator.cancel(pending));
    assert!(!active.is_cancelled());
    let full_restore = ApplicationCommand::RestoreDataAndPortableSettings(selection);
    assert!(matches!(
        coordinator.submit(full_restore),
        ApplicationCommandAdmission::Queued { .. }
    ));
    assert_eq!(coordinator.snapshot().pending_command(), Some(full_restore));
}

#[test]
fn completion_promotes_one_follow_up_and_retry_is_explicit() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(first) =
        coordinator.submit(ApplicationCommand::Backup)
    else {
        panic!("backup must start");
    };
    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        ..
    } = coordinator.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild must queue");
    };

    let transition = coordinator
        .finish(
            first.id(),
            ApplicationCommandExecution::Failed(ApplicationCommandFailure::Unavailable),
        )
        .expect("failed transition");
    assert_eq!(transition.completion().request_id(), first.id());
    assert_eq!(
        transition.completion().failure(),
        Some(ApplicationCommandFailure::Unavailable)
    );
    let all_failures = [
        ApplicationCommandFailure::Unavailable,
        ApplicationCommandFailure::InvalidSelection,
        ApplicationCommandFailure::Integrity,
        ApplicationCommandFailure::CapacityExceeded,
        ApplicationCommandFailure::Internal,
    ];
    assert_eq!(all_failures.len(), 5);
    assert_eq!(transition.follow_up().expect("follow-up").id(), pending);
    assert_eq!(
        coordinator.retry_last(),
        ApplicationCommandAdmission::Queued {
            request_id: coordinator
                .snapshot()
                .pending_request_id()
                .expect("retry pending"),
            active_request_id: pending,
        }
    );
}

#[test]
fn close_rejects_new_work_without_discarding_the_active_receipt() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::Verify)
    else {
        panic!("verify must start");
    };
    coordinator.close();
    assert!(coordinator.snapshot().is_closed());
    assert_eq!(
        coordinator.submit(ApplicationCommand::Backup),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed)
    );
    assert_eq!(
        coordinator.snapshot().active_request_id(),
        Some(active.id())
    );
}

#[test]
fn restart_pause_discards_only_the_follow_up_and_can_resume_admission() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild must start");
    };
    assert!(matches!(
        coordinator.submit(ApplicationCommand::Backup),
        ApplicationCommandAdmission::Queued { .. }
    ));

    coordinator.pause_admission();
    assert!(coordinator.snapshot().admission_paused());
    assert_eq!(
        coordinator.snapshot().active_request_id(),
        Some(active.id())
    );
    assert_eq!(coordinator.snapshot().pending_count(), 0);
    assert_eq!(
        coordinator.submit(ApplicationCommand::Verify),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed)
    );

    coordinator.resume_admission();
    assert!(!coordinator.snapshot().admission_paused());
    assert!(matches!(
        coordinator.submit(ApplicationCommand::Verify),
        ApplicationCommandAdmission::Queued { .. }
    ));
}
