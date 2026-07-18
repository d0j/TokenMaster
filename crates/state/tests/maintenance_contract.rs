use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use tokenmaster_state::{
    BackupMaintenanceRuntime, BackupPolicy, MaintenanceAdmission, MaintenanceClock,
    MaintenanceCoordinator, MaintenanceExecution, MaintenanceOutcome, MaintenancePurpose,
    MaintenanceRejection, MaintenanceSchedule, MaintenanceSourceIdentity, MaintenanceSourceState,
    MaintenanceTick, MaintenanceUrgency, MaintenanceWorker, MaintenanceWorkerPhase, SettingsValue,
    StateErrorCode,
};

fn healthy_coordinator() -> MaintenanceCoordinator {
    MaintenanceCoordinator::new(MaintenanceSourceState::Healthy, true)
}

fn default_policy() -> BackupPolicy {
    SettingsValue::safe_defaults().portable().backup().clone()
}

#[derive(Debug)]
struct ManualClock(AtomicU64);

impl ManualClock {
    fn new(millis: u64) -> Self {
        Self(AtomicU64::new(millis))
    }

    fn set(&self, millis: u64) {
        self.0.store(millis, Ordering::Release);
    }
}

impl MaintenanceClock for ManualClock {
    fn now(&self) -> MaintenanceTick {
        MaintenanceTick::from_millis(self.0.load(Ordering::Acquire))
    }
}

#[test]
fn ten_thousand_hints_keep_one_active_and_one_merged_follow_up() {
    let mut coordinator = healthy_coordinator();
    let active = match coordinator.submit(MaintenancePurpose::Periodic) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("first request must start: {other:?}"),
    };
    for _ in 0..10_000 {
        assert!(matches!(
            coordinator.submit(MaintenancePurpose::Periodic),
            MaintenanceAdmission::Coalesced { .. }
        ));
    }
    let snapshot = coordinator.snapshot();
    assert_eq!(snapshot.active_count(), 1);
    assert_eq!(snapshot.pending_count(), 1);
    assert_eq!(
        snapshot.pending_purpose(),
        Some(MaintenancePurpose::Periodic)
    );

    active.begin_publication().expect("publication boundary");
    let transition = coordinator
        .finish(active.id(), MaintenanceExecution::Published { bytes: 4096 })
        .expect("finish active");
    assert_eq!(
        transition.completion().outcome(),
        MaintenanceOutcome::Published
    );
    assert_eq!(
        transition.follow_up().map(|permit| permit.purpose()),
        Some(MaintenancePurpose::Periodic)
    );
}

#[test]
fn urgency_is_mandatory_then_manual_then_source_retry_then_periodic() {
    assert!(MaintenanceUrgency::Mandatory > MaintenanceUrgency::Manual);
    assert!(MaintenanceUrgency::Manual > MaintenanceUrgency::SourceRetry);
    assert!(MaintenanceUrgency::SourceRetry > MaintenanceUrgency::Periodic);
    let mut coordinator = healthy_coordinator();
    let active = match coordinator.submit(MaintenancePurpose::Periodic) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("first request must start: {other:?}"),
    };
    for purpose in [
        MaintenancePurpose::Periodic,
        MaintenancePurpose::Manual,
        MaintenancePurpose::PreMigration,
    ] {
        assert!(matches!(
            coordinator.submit(purpose),
            MaintenanceAdmission::Coalesced { .. }
        ));
    }
    assert_eq!(
        coordinator.snapshot().pending_purpose(),
        Some(MaintenancePurpose::PreMigration)
    );
    assert_eq!(
        coordinator.submit(MaintenancePurpose::PreRestore),
        MaintenanceAdmission::Rejected(MaintenanceRejection::Busy)
    );
    active.begin_publication().expect("publication boundary");
    let transition = coordinator
        .finish(active.id(), MaintenanceExecution::Published { bytes: 1 })
        .expect("finish active");
    assert_eq!(
        transition.follow_up().map(|permit| permit.purpose()),
        Some(MaintenancePurpose::PreMigration)
    );
}

#[test]
fn source_retry_is_internal_urgency_not_a_caller_submit_purpose() {
    let source = include_str!("../src/maintenance/coordinator.rs");
    let purpose = source
        .split("pub enum MaintenancePurpose")
        .nth(1)
        .and_then(|tail| tail.split("impl MaintenancePurpose").next())
        .expect("maintenance purpose declaration");
    assert!(
        !purpose.contains("SourceRetry"),
        "source retry must preserve the root purpose and exist only as urgency"
    );
}

#[test]
fn schedule_requires_first_publication_quiet_window_and_six_hour_minimum() {
    let policy = default_policy();
    let mut schedule = MaintenanceSchedule::new(
        &policy,
        MaintenanceTick::from_millis(0),
        MaintenanceSourceState::HealthyUnpublished,
    );
    schedule.record_durable_change(MaintenanceTick::from_millis(1_000));
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(21_600_000)),
        None,
        "automatic backup is closed before first healthy publication"
    );

    schedule.mark_healthy_publication(MaintenanceTick::from_millis(21_600_000));
    schedule.record_durable_change(MaintenanceTick::from_millis(21_601_000));
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(21_901_000)),
        None,
        "quiet alone cannot violate the six-hour minimum"
    );
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(43_200_000)),
        Some(MaintenancePurpose::Periodic)
    );
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(43_200_001)),
        None,
        "one due interval emits at most one request"
    );
}

#[test]
fn resume_and_clock_rollback_each_coalesce_one_catch_up() {
    let policy = default_policy();
    let mut schedule = MaintenanceSchedule::new(
        &policy,
        MaintenanceTick::from_millis(0),
        MaintenanceSourceState::HealthyUnpublished,
    );
    schedule.mark_healthy_publication(MaintenanceTick::from_millis(0));
    schedule.pause(MaintenanceTick::from_millis(1_000));
    schedule.resume(MaintenanceTick::from_millis(21_700_000));
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(21_700_000)),
        Some(MaintenancePurpose::Periodic)
    );
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(21_700_000)),
        None
    );

    assert_eq!(schedule.poll(MaintenanceTick::from_millis(100)), None);
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(100)),
        Some(MaintenancePurpose::Periodic)
    );
    assert_eq!(schedule.poll(MaintenanceTick::from_millis(100)), None);
}

#[test]
fn suspect_source_and_unpublished_source_reject_automatic_work() {
    for source in [
        MaintenanceSourceState::HealthyUnpublished,
        MaintenanceSourceState::Suspect,
    ] {
        let mut coordinator = MaintenanceCoordinator::new(source, true);
        assert_eq!(
            coordinator.submit(MaintenancePurpose::Periodic),
            MaintenanceAdmission::Rejected(MaintenanceRejection::SourceIneligible)
        );
    }
}

#[test]
fn same_source_identity_gets_one_retry_then_escalates_suspect() {
    let mut coordinator = healthy_coordinator();
    let identity = MaintenanceSourceIdentity::new([7_u8; 32]);
    let first = match coordinator.submit(MaintenancePurpose::Manual) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("manual must start: {other:?}"),
    };
    let retry = coordinator
        .finish(first.id(), MaintenanceExecution::SourceFailed { identity })
        .expect("first source failure");
    assert_eq!(
        retry.completion().outcome(),
        MaintenanceOutcome::RetryScheduled
    );
    let retry = retry.follow_up().expect("one automatic retry").clone();
    assert_eq!(retry.purpose(), MaintenancePurpose::Manual);
    assert_eq!(
        retry.urgency(),
        tokenmaster_state::MaintenanceUrgency::SourceRetry
    );
    assert_eq!(retry.root_request_id(), first.root_request_id());

    let suspect = coordinator
        .finish(retry.id(), MaintenanceExecution::SourceFailed { identity })
        .expect("second source failure");
    assert_eq!(
        suspect.completion().outcome(),
        MaintenanceOutcome::SourceSuspect
    );
    assert_eq!(
        suspect.completion().source_state(),
        MaintenanceSourceState::Suspect
    );
    assert!(suspect.follow_up().is_none());
}

#[test]
fn mandatory_retry_preserves_guard_purpose_and_root_until_final_success() {
    let mut coordinator = healthy_coordinator();
    let identity = MaintenanceSourceIdentity::new([9_u8; 32]);
    let first = match coordinator.submit(MaintenancePurpose::PreMigration) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("mandatory guard must start: {other:?}"),
    };
    let root = first.root_request_id();
    let retry = coordinator
        .finish(first.id(), MaintenanceExecution::SourceFailed { identity })
        .expect("schedule guard retry");
    assert!(!retry.completion().allows_mutation());
    let retry = retry.follow_up().expect("guard retry").clone();
    assert_eq!(retry.root_request_id(), root);
    assert_eq!(retry.purpose(), MaintenancePurpose::PreMigration);
    assert_eq!(
        retry.urgency(),
        tokenmaster_state::MaintenanceUrgency::SourceRetry
    );
    assert_eq!(
        coordinator.submit(MaintenancePurpose::PreRestore),
        MaintenanceAdmission::Rejected(MaintenanceRejection::Busy)
    );

    retry.begin_publication().expect("publication boundary");
    let final_completion = coordinator
        .finish(retry.id(), MaintenanceExecution::Published { bytes: 1024 })
        .expect("finish guard retry")
        .completion();
    assert_eq!(final_completion.root_request_id(), root);
    assert_eq!(final_completion.purpose(), MaintenancePurpose::PreMigration);
    assert!(final_completion.allows_mutation());
}

#[test]
fn cancellation_is_cooperative_until_publication_and_impossible_after_it() {
    let mut before = healthy_coordinator();
    let before = match before.submit(MaintenancePurpose::Manual) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("manual must start: {other:?}"),
    };
    let before_control = before.backup_control().expect("linked backup control");
    assert!(!before_control.is_cancelled());
    assert!(before.cancel());
    assert!(before.is_cancelled());
    assert!(before_control.is_cancelled());
    assert!(before.begin_publication().is_err());

    let mut after_coordinator = healthy_coordinator();
    let after = match after_coordinator.submit(MaintenancePurpose::Manual) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("manual must start: {other:?}"),
    };
    let after_control = after.backup_control().expect("linked backup control");
    after.begin_publication().expect("begin final publication");
    assert!(!after.cancel());
    assert!(!after.is_cancelled());
    assert!(!after_control.is_cancelled());
    assert_eq!(
        after_coordinator
            .finish(after.id(), MaintenanceExecution::Published { bytes: 1 })
            .expect("finish non-cancellable publication")
            .completion()
            .outcome(),
        MaintenanceOutcome::Published
    );

    let mut impossible_coordinator = healthy_coordinator();
    let impossible = match impossible_coordinator.submit(MaintenancePurpose::Manual) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("manual must start: {other:?}"),
    };
    impossible
        .begin_publication()
        .expect("begin final publication");
    let completion = impossible_coordinator
        .finish(impossible.id(), MaintenanceExecution::Cancelled)
        .expect("impossible late cancellation becomes a fixed failure")
        .completion();
    assert_eq!(completion.outcome(), MaintenanceOutcome::Failed);
    assert_eq!(
        completion.failure_code(),
        Some(StateErrorCode::InternalInvariant)
    );
}

#[test]
fn periodic_disablement_keeps_every_mandatory_guard_active() {
    let mut coordinator = MaintenanceCoordinator::new(MaintenanceSourceState::Healthy, false);
    assert_eq!(
        coordinator.submit(MaintenancePurpose::Periodic),
        MaintenanceAdmission::Rejected(MaintenanceRejection::PeriodicDisabled)
    );
    for purpose in [
        MaintenancePurpose::PreMigration,
        MaintenancePurpose::PreRestore,
        MaintenancePurpose::PreDestructiveMaintenance,
    ] {
        let permit = match coordinator.submit(purpose) {
            MaintenanceAdmission::Started(permit) => permit,
            other => panic!("mandatory request must start: {other:?}"),
        };
        permit.begin_publication().expect("publication boundary");
        let completion = coordinator
            .finish(permit.id(), MaintenanceExecution::Published { bytes: 1 })
            .expect("mandatory completion")
            .completion();
        assert!(completion.allows_mutation());
    }
}

#[test]
fn disabling_periodic_drops_an_already_merged_periodic_follow_up() {
    let executions = Arc::new(AtomicUsize::new(0));
    let observed_executions = Arc::clone(&executions);
    let (started_sender, started_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let (purpose_sender, purpose_receiver) = sync_channel(2);
    let mut worker =
        MaintenanceWorker::spawn(MaintenanceSourceState::Healthy, true, move |permit| {
            purpose_sender
                .send(permit.purpose())
                .expect("record maintenance purpose");
            if observed_executions.fetch_add(1, Ordering::AcqRel) == 0 {
                started_sender.send(()).expect("first execution started");
                release_receiver.recv().expect("release first execution");
            }
            MaintenanceExecution::Failed(StateErrorCode::Unavailable)
        })
        .expect("spawn worker");
    assert!(matches!(
        worker.submit(MaintenancePurpose::Manual),
        MaintenanceAdmission::Started(_)
    ));
    started_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("manual execution starts");
    assert!(matches!(
        worker.submit(MaintenancePurpose::Periodic),
        MaintenanceAdmission::Coalesced { .. }
    ));
    worker
        .set_periodic_enabled(false)
        .expect("disable periodic work");
    release_sender.send(()).expect("release manual execution");
    assert_eq!(
        purpose_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("manual purpose"),
        MaintenancePurpose::Manual
    );
    assert!(
        purpose_receiver
            .recv_timeout(Duration::from_millis(250))
            .is_err(),
        "disabled pending periodic work must not execute"
    );
    assert_eq!(executions.load(Ordering::Acquire), 1);
    worker.shutdown().expect("shutdown worker");
}

#[test]
fn mandatory_failure_blocks_mutation_while_explicit_empty_and_quarantine_paths_bypass() {
    let mut healthy = healthy_coordinator();
    let permit = match healthy.submit(MaintenancePurpose::PreMigration) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("mandatory request must start: {other:?}"),
    };
    let completion = healthy
        .finish(
            permit.id(),
            MaintenanceExecution::Failed(StateErrorCode::Unavailable),
        )
        .expect("mandatory failure")
        .completion();
    assert!(!completion.allows_mutation());

    for source in [
        MaintenanceSourceState::EmptyInstallation,
        MaintenanceSourceState::CorruptQuarantined,
    ] {
        let mut coordinator = MaintenanceCoordinator::new(source, false);
        let admission = coordinator.submit(MaintenancePurpose::PreRestore);
        assert!(admission.allows_guarded_mutation());
    }
}

#[test]
fn published_execution_requires_the_non_cancellable_boundary() {
    let mut coordinator = healthy_coordinator();
    let permit = match coordinator.submit(MaintenancePurpose::PreMigration) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("mandatory request must start: {other:?}"),
    };
    let completion = coordinator
        .finish(permit.id(), MaintenanceExecution::Published { bytes: 1 })
        .expect("state machine returns a fixed failure")
        .completion();
    assert_eq!(completion.outcome(), MaintenanceOutcome::Failed);
    assert_eq!(
        completion.failure_code(),
        Some(StateErrorCode::InternalInvariant)
    );
    assert!(!completion.allows_mutation());
}

#[test]
fn worker_keeps_only_latest_health_and_joins_on_shutdown() {
    let executions = Arc::new(AtomicUsize::new(0));
    let worker_executions = Arc::clone(&executions);
    let (started_sender, started_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let mut worker =
        MaintenanceWorker::spawn(MaintenanceSourceState::Healthy, true, move |permit| {
            let execution = worker_executions.fetch_add(1, Ordering::AcqRel);
            if execution == 0 {
                started_sender.send(()).expect("signal first execution");
                release_receiver.recv().expect("release first execution");
            }
            permit.begin_publication().expect("publication boundary");
            MaintenanceExecution::Published { bytes: 64 }
        })
        .expect("spawn maintenance worker");
    let _ = worker.submit(MaintenancePurpose::Periodic);
    started_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("first execution started");
    for _ in 1..10_000 {
        let _ = worker.submit(MaintenancePurpose::Periodic);
    }
    release_sender.send(()).expect("release worker");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while worker.snapshot().successful_count() == 0 && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(worker.snapshot().successful_count() > 0);
    assert!(executions.load(Ordering::Acquire) <= 2);
    assert_eq!(
        worker.pause().expect("pause"),
        MaintenanceWorkerPhase::Paused
    );
    assert_eq!(
        worker.resume().expect("resume"),
        MaintenanceWorkerPhase::Running
    );
    assert_eq!(
        worker.shutdown().expect("shutdown"),
        MaintenanceWorkerPhase::Stopped
    );
}

#[test]
fn runtime_owns_one_worker_and_scheduler_and_catches_up_after_resume() {
    let clock = Arc::new(ManualClock::new(0));
    let runtime_clock: Arc<dyn MaintenanceClock> = clock.clone();
    let (purpose_sender, purpose_receiver) = sync_channel(4);
    let mut runtime = BackupMaintenanceRuntime::spawn(
        runtime_clock,
        default_policy(),
        MaintenanceSourceState::HealthyUnpublished,
        move |permit| {
            purpose_sender
                .send(permit.purpose())
                .expect("publish observed purpose");
            permit.begin_publication().expect("publication boundary");
            MaintenanceExecution::Published { bytes: 128 }
        },
    )
    .expect("spawn backup maintenance runtime");

    assert!(matches!(
        runtime.submit(MaintenancePurpose::Manual),
        MaintenanceAdmission::Started(_)
    ));
    assert_eq!(
        purpose_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("manual execution"),
        MaintenancePurpose::Manual
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while runtime.snapshot().worker().successful_count() == 0
        && std::time::Instant::now() < deadline
    {
        std::thread::yield_now();
    }
    assert_eq!(runtime.snapshot().worker().successful_count(), 1);

    clock.set(21_700_000);
    runtime.pause().expect("pause runtime");
    runtime.resume().expect("resume runtime");
    assert_eq!(
        purpose_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("resume catch-up"),
        MaintenancePurpose::Periodic
    );
    assert!(runtime.snapshot().scheduler().submitted_count() >= 1);
    runtime.shutdown().expect("shutdown runtime");
}

#[test]
fn healthy_restart_seeds_periodic_schedule_from_prior_publication_truth() {
    let clock = Arc::new(ManualClock::new(0));
    let runtime_clock: Arc<dyn MaintenanceClock> = clock.clone();
    let (purpose_sender, purpose_receiver) = sync_channel(1);
    let mut runtime = BackupMaintenanceRuntime::spawn(
        runtime_clock,
        default_policy(),
        MaintenanceSourceState::Healthy,
        move |permit| {
            purpose_sender
                .send(permit.purpose())
                .expect("publish observed purpose");
            permit.begin_publication().expect("publication boundary");
            MaintenanceExecution::Published { bytes: 128 }
        },
    )
    .expect("spawn healthy maintenance runtime");

    clock.set(21_700_000);
    runtime.pause().expect("pause healthy runtime");
    runtime.resume().expect("resume healthy runtime");
    assert_eq!(
        purpose_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("healthy restart catch-up"),
        MaintenancePurpose::Periodic
    );
    runtime.shutdown().expect("shutdown runtime");
}

#[test]
fn runtime_waits_for_one_terminal_root_completion_without_polling() {
    let clock: Arc<dyn MaintenanceClock> = Arc::new(ManualClock::new(0));
    let mut runtime = BackupMaintenanceRuntime::spawn(
        clock,
        default_policy(),
        MaintenanceSourceState::HealthyUnpublished,
        |permit| {
            permit.begin_publication().expect("publication boundary");
            MaintenanceExecution::Published { bytes: 512 }
        },
    )
    .expect("spawn maintenance runtime");
    let root = match runtime.submit(MaintenancePurpose::Manual) {
        MaintenanceAdmission::Started(permit) => permit.root_request_id(),
        other => panic!("manual request must start: {other:?}"),
    };

    let completion = runtime
        .wait_for_completion(root, Duration::from_secs(5))
        .expect("bounded completion wait")
        .expect("terminal completion");

    assert_eq!(completion.root_request_id(), root);
    assert_eq!(completion.outcome(), MaintenanceOutcome::Published);
    assert_eq!(completion.published_bytes(), 512);
    runtime.shutdown().expect("shutdown runtime");
}

#[test]
fn atomic_submit_and_wait_reserves_the_exact_root_until_terminal_receipt() {
    let clock: Arc<dyn MaintenanceClock> = Arc::new(ManualClock::new(0));
    let (started_sender, started_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let mut runtime = BackupMaintenanceRuntime::spawn(
        clock,
        default_policy(),
        MaintenanceSourceState::HealthyUnpublished,
        move |permit| {
            started_sender.send(()).expect("started signal");
            release_receiver.recv().expect("release signal");
            permit.begin_publication().expect("publication boundary");
            MaintenanceExecution::Published { bytes: 1024 }
        },
    )
    .expect("spawn maintenance runtime");

    std::thread::scope(|scope| {
        let waiter = scope.spawn(|| {
            runtime
                .submit_and_wait(MaintenancePurpose::PostMigration, Duration::from_secs(5))
                .expect("atomic terminal wait")
        });
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("operation started");
        assert_eq!(
            runtime.submit(MaintenancePurpose::Manual),
            MaintenanceAdmission::Rejected(MaintenanceRejection::Busy)
        );
        release_sender.send(()).expect("release operation");
        let completion = waiter.join().expect("waiter thread");
        assert_eq!(completion.purpose(), MaintenancePurpose::PostMigration);
        assert_eq!(completion.outcome(), MaintenanceOutcome::Published);
        assert_eq!(completion.published_bytes(), 1024);
    });
    runtime.shutdown().expect("shutdown runtime");
}

#[test]
fn maintenance_source_has_two_fixed_threads_no_ui_async_or_trigger_queue() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let maintenance_root = root.join("src").join("maintenance");
    let mut source = String::new();
    for name in ["mod.rs", "coordinator.rs", "scheduler.rs", "worker.rs"] {
        source.push_str(
            &std::fs::read_to_string(maintenance_root.join(name)).expect("read maintenance source"),
        );
    }
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("state manifest");
    for forbidden in [
        "slint",
        "tokio",
        "async_std",
        "VecDeque",
        "crossbeam",
        "unbounded",
    ] {
        assert!(
            !source.contains(forbidden) && !manifest.contains(forbidden),
            "forbidden maintenance dependency or queue: {forbidden}"
        );
    }
    assert_eq!(
        source.matches("Builder::new()").count(),
        2,
        "one worker thread and one scheduler thread only"
    );
    assert_eq!(
        source.matches("recv_timeout(").count(),
        1,
        "one shared scheduler timer, never one timer per backup"
    );
}
