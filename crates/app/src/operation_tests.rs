use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use tempfile::tempdir;
use tokenmaster_desktop::{
    DesktopDensity, DesktopLayout, DesktopLocale, DesktopPresentationSelection, DesktopSkin,
};
use tokenmaster_platform::{
    ControlledFileDialog, FileDialogFileType, FileDialogResult, FileDialogSelector,
    ValidatedLocalDirectory,
};

use crate::command::{
    ApplicationCommand, ApplicationCommandAdmission, ApplicationCommandExecution,
    ApplicationCommandFailure, ApplicationCommandOutcome, ApplicationCommandRejection,
    ApplicationOperationPayload, ApplicationOperationRequest,
};
use crate::operation::{
    ApplicationOperationWorker, ApplicationOperationWorkerError,
    ApplicationOperationWorkerErrorCode, ApplicationOperationWorkerPhase,
};

const WAIT: Duration = Duration::from_secs(5);

fn receive<T>(receiver: &Receiver<T>) -> T {
    receiver.recv_timeout(WAIT).expect("worker signal")
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !predicate() {
        assert!(Instant::now() < deadline, "worker did not converge");
        thread::yield_now();
    }
}

#[test]
fn one_thread_executes_one_active_and_one_follow_up_without_retaining_history() {
    let caller = thread::current().id();
    let threads = Arc::new(Mutex::new(Vec::<ThreadId>::new()));
    let execution_threads = Arc::clone(&threads);
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let mut worker = ApplicationOperationWorker::spawn(move |permit| {
        execution_threads
            .lock()
            .expect("thread log")
            .push(thread::current().id());
        started_tx.send(permit.command()).expect("started signal");
        if permit.command() == ApplicationCommand::Backup {
            receive(&release_rx);
        }
        ApplicationCommandExecution::Succeeded
    })
    .expect("worker");

    let ApplicationCommandAdmission::Started(first) = worker.submit(ApplicationCommand::Backup)
    else {
        panic!("backup must start");
    };
    assert_eq!(receive(&started_rx), ApplicationCommand::Backup);
    let ApplicationCommandAdmission::Queued {
        request_id: queued,
        active_request_id,
    } = worker.submit(ApplicationCommand::Verify)
    else {
        panic!("verify must queue");
    };
    assert_eq!(active_request_id, first.id());
    for _ in 0..10_000 {
        assert_eq!(
            worker.submit(ApplicationCommand::Verify),
            ApplicationCommandAdmission::Coalesced {
                request_id: queued,
                active_request_id: first.id(),
            }
        );
    }
    assert_eq!(worker.snapshot().expect("snapshot").active_count(), 1);
    assert_eq!(worker.snapshot().expect("snapshot").pending_count(), 1);

    release_tx.send(()).expect("release");
    assert_eq!(receive(&started_rx), ApplicationCommand::Verify);
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        worker
            .snapshot()
            .expect("snapshot")
            .latest_completion()
            .expect("latest completion")
            .command(),
        ApplicationCommand::Verify
    );
    let completion = worker
        .try_completion()
        .expect("completion mailbox")
        .expect("latest completion");
    assert_eq!(completion.command(), ApplicationCommand::Verify);
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Succeeded);
    assert!(worker.try_completion().expect("empty mailbox").is_none());

    let execution_threads = threads.lock().expect("thread log");
    assert_eq!(execution_threads.len(), 2);
    assert_eq!(execution_threads[0], execution_threads[1]);
    assert_ne!(execution_threads[0], caller);
    drop(execution_threads);
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn bare_presentation_command_is_rejected_before_worker_or_coordinator_mutation() {
    let mut worker = ApplicationOperationWorker::spawn(|_| ApplicationCommandExecution::Succeeded)
        .expect("worker");

    assert_eq!(
        worker.submit(ApplicationCommand::UpdatePresentation),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::PayloadRequired)
    );
    let snapshot = worker.snapshot().expect("idle snapshot");
    assert_eq!(snapshot.active_count(), 0);
    assert_eq!(snapshot.pending_count(), 0);
    assert_eq!(snapshot.latest_completion(), None);

    assert!(matches!(
        worker
            .submitter()
            .submit_request(ApplicationOperationRequest::update_presentation(
                DesktopPresentationSelection::new(
                    DesktopDensity::Compact,
                    DesktopSkin::Graphite,
                    tokenmaster_desktop::DesktopColorScheme::System,
                    DesktopLayout::Refined,
                    DesktopLocale::English,
                ),
            )),
        ApplicationCommandAdmission::Started(_)
    ));
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn failed_command_is_the_only_retry_source_and_reexecutes_once() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let execution_attempts = Arc::clone(&attempts);
    let mut worker = ApplicationOperationWorker::spawn(move |_| {
        if execution_attempts.fetch_add(1, Ordering::AcqRel) == 0 {
            ApplicationCommandExecution::Failed(ApplicationCommandFailure::Unavailable)
        } else {
            ApplicationCommandExecution::Succeeded
        }
    })
    .expect("worker");
    assert_eq!(
        worker.retry_last(),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::NoRetryAvailable)
    );
    assert!(matches!(
        worker.submit(ApplicationCommand::Backup),
        ApplicationCommandAdmission::Started(_)
    ));
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        worker
            .try_completion()
            .expect("completion mailbox")
            .expect("failed completion")
            .failure(),
        Some(ApplicationCommandFailure::Unavailable)
    );
    assert!(matches!(
        worker.retry_last(),
        ApplicationCommandAdmission::Started(_)
    ));
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    assert_eq!(
        worker
            .try_completion()
            .expect("completion mailbox")
            .expect("retry completion")
            .outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn active_cancellation_is_exact_and_cooperative() {
    let (started_tx, started_rx) = channel();
    let mut worker = ApplicationOperationWorker::spawn(move |permit| {
        started_tx.send(permit.id()).expect("started signal");
        while !permit.is_cancelled() {
            thread::yield_now();
        }
        ApplicationCommandExecution::Cancelled
    })
    .expect("worker");
    let ApplicationCommandAdmission::Started(active) = worker.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild must start");
    };
    assert_eq!(receive(&started_rx), active.id());
    assert!(worker.cancel(active.id()));
    assert!(!worker.cancel(active.id()));
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    let completion = worker
        .try_completion()
        .expect("completion mailbox")
        .expect("cancel completion");
    assert_eq!(completion.request_id(), active.id());
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Cancelled);
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn cancellation_after_execution_before_finish_completes_without_faulting() {
    let (at_boundary_tx, at_boundary_rx) = channel();
    let (release_tx, release_rx) = channel();
    let mut worker = ApplicationOperationWorker::spawn_observed(
        |_| ApplicationCommandExecution::Succeeded,
        move |request_id| {
            at_boundary_tx.send(request_id).expect("boundary signal");
            receive(&release_rx);
        },
    )
    .expect("worker");
    let ApplicationCommandAdmission::Started(active) = worker.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild must start");
    };

    assert_eq!(receive(&at_boundary_rx), active.id());
    assert!(worker.cancel(active.id()));
    release_tx.send(()).expect("release boundary");
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    let completion = worker
        .try_completion()
        .expect("completion mailbox")
        .expect("cancel completion");
    assert_eq!(completion.request_id(), active.id());
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Cancelled);
    assert_eq!(
        worker.snapshot().expect("snapshot").phase(),
        ApplicationOperationWorkerPhase::Running
    );
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn cancellation_wins_command_outcome_while_concurrent_panic_faults_worker() {
    let (at_boundary_tx, at_boundary_rx) = channel();
    let (release_tx, release_rx) = channel();
    let mut worker = ApplicationOperationWorker::spawn_observed(
        |_| -> ApplicationCommandExecution { panic!("private concurrent panic") },
        move |request_id| {
            at_boundary_tx.send(request_id).expect("boundary signal");
            receive(&release_rx);
        },
    )
    .expect("worker");
    let ApplicationCommandAdmission::Started(active) = worker.submit(ApplicationCommand::Verify)
    else {
        panic!("verify must start");
    };

    assert_eq!(receive(&at_boundary_rx), active.id());
    assert!(worker.cancel(active.id()));
    release_tx.send(()).expect("release boundary");
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.phase() == ApplicationOperationWorkerPhase::Faulted)
    });
    let completion = worker
        .try_completion()
        .expect("completion mailbox")
        .expect("cancel completion");
    assert_eq!(completion.request_id(), active.id());
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Cancelled);
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Faulted
    );
}

#[test]
fn irreversible_boundary_rejects_late_cancellation() {
    let (irreversible_tx, irreversible_rx) = channel();
    let (release_tx, release_rx) = channel();
    let mut worker = ApplicationOperationWorker::spawn(move |permit| {
        permit.begin_irreversible().expect("irreversible boundary");
        irreversible_tx.send(permit.id()).expect("boundary signal");
        receive(&release_rx);
        ApplicationCommandExecution::Succeeded
    })
    .expect("worker");
    let ApplicationCommandAdmission::Started(active) = worker.submit(ApplicationCommand::Backup)
    else {
        panic!("backup must start");
    };
    assert_eq!(receive(&irreversible_rx), active.id());
    assert!(!worker.cancel(active.id()));
    release_tx.send(()).expect("release");
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        worker
            .try_completion()
            .expect("completion mailbox")
            .expect("completion")
            .outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn executor_panic_faults_closes_and_publishes_only_stable_internal_failure() {
    let mut worker = ApplicationOperationWorker::spawn(|_| -> ApplicationCommandExecution {
        panic!("private executor detail must be contained")
    })
    .expect("worker");
    let ApplicationCommandAdmission::Started(active) = worker.submit(ApplicationCommand::Verify)
    else {
        panic!("verify must start");
    };
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.phase() == ApplicationOperationWorkerPhase::Faulted)
    });
    let completion = worker
        .try_completion()
        .expect("completion mailbox")
        .expect("panic completion");
    assert_eq!(completion.request_id(), active.id());
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Failed);
    assert_eq!(
        completion.failure(),
        Some(ApplicationCommandFailure::Internal)
    );
    assert_eq!(
        worker.submit(ApplicationCommand::Backup),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed)
    );
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Faulted
    );
}

#[test]
fn drop_cancels_joins_and_drops_the_owned_executor() {
    struct DropSignal(Arc<AtomicBool>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let dropped = Arc::new(AtomicBool::new(false));
    let signal = DropSignal(Arc::clone(&dropped));
    let worker = ApplicationOperationWorker::spawn(move |_| {
        let _ = &signal;
        ApplicationCommandExecution::Succeeded
    })
    .expect("worker");
    drop(worker);
    assert!(dropped.load(Ordering::Acquire));
}

#[test]
fn worker_errors_expose_only_stable_path_private_codes() {
    for (error, code, stable) in [
        (
            ApplicationOperationWorkerError::unavailable(),
            ApplicationOperationWorkerErrorCode::Unavailable,
            "unavailable",
        ),
        (
            ApplicationOperationWorkerError::internal(),
            ApplicationOperationWorkerErrorCode::Internal,
            "internal",
        ),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), stable);
        assert_eq!(code.stable_code(), stable);
        assert!(!format!("{error:?}").contains('\\'));
    }
}

#[test]
fn cloned_submitter_moves_one_sealed_output_to_the_single_worker_without_disclosure() {
    let root = tempdir().expect("temporary root");
    let directory = ValidatedLocalDirectory::new(root.path()).expect("validated root");
    let dialog = ControlledFileDialog::selected(&directory, "settings.tmconfig").expect("dialog");
    let FileDialogResult::Selected(output) = dialog.select_output(FileDialogFileType::Config)
    else {
        panic!("output selection");
    };
    let request = ApplicationOperationRequest::export_config(output);
    assert_eq!(
        format!("{request:?}"),
        "ApplicationOperationRequest(ExportConfig, [redacted])"
    );
    assert!(!format!("{request:?}").contains(&root.path().display().to_string()));

    let (started_tx, started_rx) = channel();
    let mut worker = ApplicationOperationWorker::spawn_with_payload(move |permit, payload| {
        assert!(matches!(
            payload,
            ApplicationOperationPayload::ConfigOutput(_)
        ));
        started_tx.send(permit.command()).expect("started signal");
        ApplicationCommandExecution::Succeeded
    })
    .expect("worker");
    let submitter = worker.submitter();
    assert!(matches!(
        submitter.submit_request(request),
        ApplicationCommandAdmission::Started(_)
    ));
    assert_eq!(receive(&started_rx), ApplicationCommand::ExportConfig);
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        worker
            .try_completion()
            .expect("completion mailbox")
            .expect("completion")
            .outcome(),
        ApplicationCommandOutcome::Succeeded
    );
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn reminder_policy_follow_up_replaces_only_the_pending_payload_with_the_latest_save() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let executed = Arc::new(Mutex::new(Vec::new()));
    let execution_log = Arc::clone(&executed);
    let mut worker = ApplicationOperationWorker::spawn_with_payload(move |permit, payload| {
        let ApplicationOperationPayload::ReminderPolicy(update) = payload else {
            panic!("reminder payload");
        };
        let lead = update.lead_seconds()[0];
        execution_log.lock().expect("execution log").push(lead);
        started_tx
            .send((permit.id(), lead))
            .expect("started signal");
        if lead == 21_600 {
            receive(&release_rx);
        }
        ApplicationCommandExecution::Succeeded
    })
    .expect("worker");
    let submitter = worker.submitter();

    let ApplicationCommandAdmission::Started(first) =
        submitter.submit_request(ApplicationOperationRequest::update_reminder_policy(
            crate::command::ApplicationReminderPolicyUpdate::new(true, &[21_600])
                .expect("first policy"),
        ))
    else {
        panic!("first reminder save must start");
    };
    assert_eq!(receive(&started_rx), (first.id(), 21_600));

    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        active_request_id,
    } = submitter.submit_request(ApplicationOperationRequest::update_reminder_policy(
        crate::command::ApplicationReminderPolicyUpdate::new(true, &[10_800])
            .expect("middle policy"),
    ))
    else {
        panic!("middle reminder save must queue");
    };
    assert_eq!(active_request_id, first.id());
    assert_eq!(
        submitter.submit_request(ApplicationOperationRequest::update_reminder_policy(
            crate::command::ApplicationReminderPolicyUpdate::new(true, &[3_600])
                .expect("latest policy"),
        )),
        ApplicationCommandAdmission::Coalesced {
            request_id: pending,
            active_request_id: first.id(),
        }
    );
    assert_eq!(worker.snapshot().expect("snapshot").active_count(), 1);
    assert_eq!(worker.snapshot().expect("snapshot").pending_count(), 1);

    release_tx.send(()).expect("release first save");
    assert_eq!(receive(&started_rx), (pending, 3_600));
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        *executed.lock().expect("execution log"),
        vec![21_600, 3_600]
    );
    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn presentation_follow_up_replaces_only_the_pending_complete_payload() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let executed = Arc::new(Mutex::new(Vec::new()));
    let execution_log = Arc::clone(&executed);
    let mut worker = ApplicationOperationWorker::spawn_with_payload(move |permit, payload| {
        let ApplicationOperationPayload::Presentation(update) = payload else {
            panic!("presentation payload");
        };
        let selection = update.selection();
        execution_log.lock().expect("execution log").push(selection);
        started_tx
            .send((permit.id(), selection))
            .expect("started signal");
        if selection.density() == DesktopDensity::Compact {
            receive(&release_rx);
        }
        ApplicationCommandExecution::Succeeded
    })
    .expect("worker");
    let submitter = worker.submitter();

    let ApplicationCommandAdmission::Started(first) = submitter.submit_request(
        ApplicationOperationRequest::update_presentation(DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Refined,
            tokenmaster_desktop::DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        )),
    ) else {
        panic!("first density save must start");
    };
    assert_eq!(
        receive(&started_rx),
        (
            first.id(),
            DesktopPresentationSelection::new(
                DesktopDensity::Compact,
                DesktopSkin::Refined,
                tokenmaster_desktop::DesktopColorScheme::System,
                DesktopLayout::Refined,
                DesktopLocale::English,
            )
        )
    );

    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        active_request_id,
    } = submitter.submit_request(ApplicationOperationRequest::update_presentation(
        DesktopPresentationSelection::new(
            DesktopDensity::UltraCompact,
            DesktopSkin::Graphite,
            tokenmaster_desktop::DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        ),
    ))
    else {
        panic!("middle density save must queue");
    };
    assert_eq!(active_request_id, first.id());
    assert_eq!(
        submitter.submit_request(ApplicationOperationRequest::update_presentation(
            DesktopPresentationSelection::new(
                DesktopDensity::Comfortable,
                DesktopSkin::Ember,
                tokenmaster_desktop::DesktopColorScheme::System,
                DesktopLayout::Refined,
                DesktopLocale::English,
            ),
        )),
        ApplicationCommandAdmission::Coalesced {
            request_id: pending,
            active_request_id: first.id(),
        }
    );
    let snapshot = worker.snapshot().expect("snapshot");
    assert_eq!(snapshot.active_count(), 1);
    assert_eq!(snapshot.pending_count(), 1);

    release_tx.send(()).expect("release first save");
    assert_eq!(
        receive(&started_rx),
        (
            pending,
            DesktopPresentationSelection::new(
                DesktopDensity::Comfortable,
                DesktopSkin::Ember,
                tokenmaster_desktop::DesktopColorScheme::System,
                DesktopLayout::Refined,
                DesktopLocale::English,
            )
        )
    );
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        *executed.lock().expect("execution log"),
        vec![
            DesktopPresentationSelection::new(
                DesktopDensity::Compact,
                DesktopSkin::Refined,
                tokenmaster_desktop::DesktopColorScheme::System,
                DesktopLayout::Refined,
                DesktopLocale::English,
            ),
            DesktopPresentationSelection::new(
                DesktopDensity::Comfortable,
                DesktopSkin::Ember,
                tokenmaster_desktop::DesktopColorScheme::System,
                DesktopLayout::Refined,
                DesktopLocale::English,
            )
        ]
    );
    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}

#[test]
fn ten_thousand_presentation_updates_keep_one_latest_payload() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let executed = Arc::new(Mutex::new(Vec::new()));
    let execution_log = Arc::clone(&executed);
    let mut worker = ApplicationOperationWorker::spawn_with_payload(move |_permit, payload| {
        let ApplicationOperationPayload::Presentation(update) = payload else {
            panic!("presentation payload");
        };
        let selection = update.selection();
        execution_log.lock().expect("execution log").push(selection);
        started_tx.send(selection).expect("started signal");
        if selection.density() == DesktopDensity::Compact {
            receive(&release_rx);
        }
        ApplicationCommandExecution::Succeeded
    })
    .expect("worker");
    let submitter = worker.submitter();
    assert!(matches!(
        submitter.submit_request(ApplicationOperationRequest::update_presentation(
            DesktopPresentationSelection::new(
                DesktopDensity::Compact,
                DesktopSkin::Refined,
                tokenmaster_desktop::DesktopColorScheme::System,
                DesktopLayout::Refined,
                DesktopLocale::English,
            ),
        )),
        ApplicationCommandAdmission::Started(_)
    ));
    assert_eq!(
        receive(&started_rx),
        DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Refined,
            tokenmaster_desktop::DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        )
    );

    let mut final_selection = DesktopPresentationSelection::new(
        DesktopDensity::Comfortable,
        DesktopSkin::Refined,
        tokenmaster_desktop::DesktopColorScheme::System,
        DesktopLayout::Refined,
        DesktopLocale::English,
    );
    for index in 0..10_000 {
        let density = match index % 3 {
            0 => DesktopDensity::Comfortable,
            1 => DesktopDensity::Compact,
            _ => DesktopDensity::UltraCompact,
        };
        let skin = match (index / 3) % 3 {
            0 => DesktopSkin::Refined,
            1 => DesktopSkin::Graphite,
            _ => DesktopSkin::Ember,
        };
        let color_scheme = match (index / 9) % 3 {
            0 => tokenmaster_desktop::DesktopColorScheme::System,
            1 => tokenmaster_desktop::DesktopColorScheme::Light,
            _ => tokenmaster_desktop::DesktopColorScheme::Dark,
        };
        let layout = match (index / 27) % 3 {
            0 => DesktopLayout::Refined,
            1 => DesktopLayout::ControlCenter,
            _ => DesktopLayout::Workbench,
        };
        let locale = match (index / 81) % 3 {
            0 => DesktopLocale::English,
            1 => DesktopLocale::Russian,
            _ => DesktopLocale::Pseudo,
        };
        final_selection =
            DesktopPresentationSelection::new(density, skin, color_scheme, layout, locale);
        assert!(matches!(
            submitter.submit_request(ApplicationOperationRequest::update_presentation(
                final_selection,
            )),
            ApplicationCommandAdmission::Queued { .. }
                | ApplicationCommandAdmission::Coalesced { .. }
        ));
        let snapshot = worker.snapshot().expect("snapshot");
        assert_eq!(snapshot.active_count(), 1);
        assert_eq!(snapshot.pending_count(), 1);
    }

    release_tx.send(()).expect("release first save");
    assert_eq!(receive(&started_rx), final_selection);
    wait_until(|| {
        worker
            .snapshot()
            .is_ok_and(|snapshot| snapshot.active_count() == 0)
    });
    assert_eq!(
        *executed.lock().expect("execution log"),
        vec![
            DesktopPresentationSelection::new(
                DesktopDensity::Compact,
                DesktopSkin::Refined,
                tokenmaster_desktop::DesktopColorScheme::System,
                DesktopLayout::Refined,
                DesktopLocale::English,
            ),
            final_selection
        ]
    );
    assert_eq!(
        worker.shutdown().expect("worker shutdown"),
        ApplicationOperationWorkerPhase::Stopped
    );
}
