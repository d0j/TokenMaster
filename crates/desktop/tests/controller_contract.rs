use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread,
    time::Duration,
};

use tokenmaster_desktop::{
    DesktopAttempt, DesktopController, DesktopQueryPlan, DesktopQuerySource,
    DesktopRefreshAdmission, DesktopRefreshOutcome, DesktopRefreshUrgency,
    DesktopRuntimeObservation, DesktopRuntimeObservationOutcome, DesktopSnapshotNotifier,
    DesktopSnapshotReceiver,
};
use tokenmaster_product::{
    ProductRuntimeGeneration, ProductRuntimeObservationError, ProductSectionKind,
};
use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewRequest, BenefitOverviewSnapshot, GitEnvelope,
    GitOutputRequest, GitOutputSnapshot, LatestActivityPage, LatestActivityRequest, PageSize,
    ProductDataStatusEnvelope, QueryEnvelope, QueryError, QueryService, QuotaCurrentSnapshot,
    QuotaEnvelope, SystemQueryClock, UsageAnalytics, UsageAnalyticsRequest, UsageSessionPage,
    UsageSessionPageRequest,
};
use tokenmaster_store::UsageStore;

macro_rules! delegate_unavailable_methods {
    () => {
        fn usage_analytics(
            &mut self,
            _request: UsageAnalyticsRequest,
        ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
            Err(query_failure())
        }

        fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
            Err(query_failure())
        }

        fn benefit_overview(
            &mut self,
            _request: BenefitOverviewRequest,
        ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
            Err(query_failure())
        }

        fn git_output(
            &mut self,
            _request: GitOutputRequest,
        ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
            Err(query_failure())
        }

        fn latest_activity(
            &mut self,
            _request: LatestActivityRequest,
        ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
            Err(query_failure())
        }

        fn usage_sessions(
            &mut self,
            _request: UsageSessionPageRequest,
        ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
            Err(query_failure())
        }
    };
}

struct UnavailableSource {
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl UnavailableSource {
    fn record(&self, value: &'static str) {
        self.calls.lock().expect("call log").push(value);
    }
}

impl DesktopQuerySource for UnavailableSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        self.record("status");
        Err(query_failure())
    }

    fn usage_analytics(
        &mut self,
        _request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        self.record("analytics");
        Err(query_failure())
    }

    fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        self.record("quota");
        Err(query_failure())
    }

    fn benefit_overview(
        &mut self,
        _request: BenefitOverviewRequest,
    ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
        self.record("benefit");
        Err(query_failure())
    }

    fn git_output(
        &mut self,
        _request: GitOutputRequest,
    ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
        self.record("git");
        Err(query_failure())
    }

    fn latest_activity(
        &mut self,
        request: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
        assert_eq!(
            request.page_size().get(),
            DesktopQueryPlan::MAX_DASHBOARD_ROWS
        );
        self.record("activity");
        Err(query_failure())
    }

    fn usage_sessions(
        &mut self,
        request: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
        assert_eq!(
            request.page_size().get(),
            DesktopQueryPlan::MAX_DASHBOARD_ROWS
        );
        self.record("sessions");
        Err(query_failure())
    }
}

#[test]
fn controller_contract_is_typed_bounded_and_deterministic() {
    let plan = DesktopQueryPlan::overview().expect("bounded overview plan");
    assert_eq!(DesktopQueryPlan::MAX_SERIES_POINTS, 240);
    assert_eq!(DesktopQueryPlan::MAX_DASHBOARD_ROWS, 12);
    assert_eq!(DesktopQueryPlan::MAX_REPOSITORIES, 32);

    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut controller = DesktopController::spawn(
        UnavailableSource {
            calls: calls.clone(),
        },
        plan,
    )
    .expect("controller starts");
    let admission = controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh admitted");
    let attempt = match admission {
        DesktopRefreshAdmission::Started { attempt } => attempt,
        other => panic!("unexpected first admission: {other:?}"),
    };

    let completion = wait_for_completion(&controller);
    assert_eq!(completion.attempt(), attempt);
    assert_eq!(completion.outcome(), DesktopRefreshOutcome::Completed);
    assert_eq!(
        *calls.lock().expect("call log"),
        [
            "status",
            "analytics",
            "quota",
            "benefit",
            "git",
            "activity",
            "sessions"
        ]
    );

    let snapshot = controller
        .take_snapshot()
        .expect("latest slot remains healthy")
        .expect("one completed snapshot");
    for section in [
        snapshot.data_status().kind(),
        snapshot.analytics().kind(),
        snapshot.quota().kind(),
        snapshot.benefit().kind(),
        snapshot.git().kind(),
        snapshot.activity().kind(),
        snapshot.sessions().kind(),
    ] {
        assert_eq!(section, ProductSectionKind::Unavailable);
    }
    assert_eq!(
        snapshot
            .data_status()
            .attempt_generation()
            .expect("attempt generation")
            .get(),
        attempt.get()
    );
    for generation in [
        snapshot.analytics().attempt_generation(),
        snapshot.quota().attempt_generation(),
        snapshot.benefit().attempt_generation(),
        snapshot.git().attempt_generation(),
        snapshot.activity().attempt_generation(),
        snapshot.sessions().attempt_generation(),
    ] {
        assert_eq!(generation.expect("attempt generation").get(), attempt.get());
    }
    assert!(
        controller
            .take_snapshot()
            .expect("latest slot remains healthy")
            .is_none()
    );

    controller.shutdown().expect("controller stops");
    let error = controller
        .refresh(DesktopRefreshUrgency::Hint)
        .expect_err("stopped controller rejects work");
    assert_eq!(error.stable_code(), "closed");
    let attach_error = controller
        .attach_snapshot_notifier(Arc::new(NoopNotifier))
        .expect_err("stopped controller rejects notifier attachment");
    assert_eq!(attach_error.stable_code(), "closed");
}

fn runtime_generation(value: u64) -> ProductRuntimeGeneration {
    ProductRuntimeGeneration::new(value).expect("nonzero runtime generation")
}

fn unavailable_runtime_observation(value: u64) -> DesktopRuntimeObservation {
    DesktopRuntimeObservation::new(
        runtime_generation(value),
        Err(ProductRuntimeObservationError::StoreUnavailable),
        Err(ProductRuntimeObservationError::ProviderUnavailable),
        Err(ProductRuntimeObservationError::Closed),
        Err(ProductRuntimeObservationError::Faulted),
    )
}

#[test]
fn runtime_observations_are_capacity_one_generation_ordered_and_joined_on_refresh() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut controller = DesktopController::spawn(
        UnavailableSource { calls },
        DesktopQueryPlan::overview().expect("overview plan"),
    )
    .expect("controller starts");

    assert_eq!(
        controller
            .observe_runtime(unavailable_runtime_observation(1))
            .expect("first runtime observation"),
        DesktopRuntimeObservationOutcome::Accepted
    );
    assert_eq!(
        controller
            .observe_runtime(unavailable_runtime_observation(1))
            .expect("equal runtime observation"),
        DesktopRuntimeObservationOutcome::IgnoredNotNewer
    );
    for generation in 2..=10_000 {
        assert_eq!(
            controller
                .observe_runtime(unavailable_runtime_observation(generation))
                .expect("newer runtime observation"),
            DesktopRuntimeObservationOutcome::Accepted
        );
    }
    assert_eq!(
        controller
            .observe_runtime(unavailable_runtime_observation(9_999))
            .expect("older runtime observation"),
        DesktopRuntimeObservationOutcome::IgnoredNotNewer
    );

    controller
        .refresh(DesktopRefreshUrgency::Hint)
        .expect("refresh starts");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let snapshot = controller
        .take_snapshot()
        .expect("snapshot mailbox")
        .expect("joined product snapshot");
    for kind in [
        snapshot.runtime().usage().kind(),
        snapshot.runtime().quota().kind(),
        snapshot.runtime().reminder().kind(),
        snapshot.runtime().git().kind(),
    ] {
        assert_eq!(kind, ProductSectionKind::Unavailable);
    }
    for generation in [
        snapshot.runtime().usage().generation(),
        snapshot.runtime().quota().generation(),
        snapshot.runtime().reminder().generation(),
        snapshot.runtime().git().generation(),
    ] {
        assert_eq!(generation, Some(runtime_generation(10_000)));
    }
    assert_eq!(
        snapshot.runtime().usage().observation_error(),
        Some(ProductRuntimeObservationError::StoreUnavailable)
    );
    assert_eq!(
        snapshot.runtime().quota().observation_error(),
        Some(ProductRuntimeObservationError::ProviderUnavailable)
    );
    assert_eq!(
        snapshot.runtime().reminder().observation_error(),
        Some(ProductRuntimeObservationError::Closed)
    );
    assert_eq!(
        snapshot.runtime().git().observation_error(),
        Some(ProductRuntimeObservationError::Faulted)
    );

    controller.shutdown().expect("controller stops");
}

#[test]
fn runtime_observation_racing_active_query_is_joined_by_one_follow_up() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let source = BlockingUnavailableSource {
        calls,
        entered: entered_sender,
        release: Some(release_receiver),
    };
    let mut controller =
        DesktopController::spawn(source, DesktopQueryPlan::overview().expect("overview plan"))
            .expect("controller starts");
    controller
        .observe_runtime(unavailable_runtime_observation(1))
        .expect("initial runtime observation");
    let first = started_attempt(
        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("first refresh"),
    );
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("query entered");

    controller
        .observe_runtime(unavailable_runtime_observation(2))
        .expect("racing runtime observation");
    let receipt = match controller
        .refresh(DesktopRefreshUrgency::Hint)
        .expect("coalesced runtime refresh")
    {
        DesktopRefreshAdmission::Coalesced {
            receipt,
            active_attempt,
        } => {
            assert_eq!(active_attempt, first);
            receipt
        }
        admission => panic!("unexpected admission: {admission:?}"),
    };
    release_sender.send(()).expect("release query");

    let follow_up = receipt.get().checked_add(1).expect("follow-up attempt");
    let snapshot = wait_for_snapshot_attempt(&controller, follow_up);
    assert_eq!(
        snapshot.runtime().usage().generation(),
        Some(runtime_generation(2))
    );
    assert_eq!(
        snapshot.runtime().quota().generation(),
        Some(runtime_generation(2))
    );
    controller.shutdown().expect("controller stops");
}

#[test]
fn one_idle_notifier_observes_the_published_mailbox_before_wakeup() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut controller = DesktopController::spawn(
        UnavailableSource { calls },
        DesktopQueryPlan::overview().expect("overview plan"),
    )
    .expect("controller starts");
    let receiver = controller.snapshot_receiver();
    let notifications = Arc::new(AtomicUsize::new(0));
    let populated = Arc::new(AtomicUsize::new(0));
    controller
        .attach_snapshot_notifier(Arc::new(InspectingNotifier {
            receiver: receiver.clone(),
            notifications: notifications.clone(),
            populated: populated.clone(),
        }))
        .expect("first idle notifier attaches");

    let second_error = controller
        .attach_snapshot_notifier(Arc::new(InspectingNotifier {
            receiver: receiver.clone(),
            notifications: Arc::new(AtomicUsize::new(0)),
            populated: Arc::new(AtomicUsize::new(0)),
        }))
        .expect_err("second notifier is rejected");
    assert_eq!(second_error.stable_code(), "notifier_already_attached");

    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh admitted");
    let completion = wait_for_completion(&controller);
    assert_eq!(completion.outcome(), DesktopRefreshOutcome::Completed);
    assert_eq!(notifications.load(Ordering::Acquire), 1);
    assert_eq!(populated.load(Ordering::Acquire), 1);
    assert!(receiver.has_snapshot().expect("mailbox remains available"));
    assert!(
        receiver
            .take_snapshot()
            .expect("mailbox remains available")
            .is_some()
    );
    controller.shutdown().expect("controller stops");
}

#[test]
fn attaching_to_an_idle_populated_mailbox_triggers_one_wakeup() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut controller = DesktopController::spawn(
        UnavailableSource { calls },
        DesktopQueryPlan::overview().expect("overview plan"),
    )
    .expect("controller starts");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh admitted");
    let completion = wait_for_completion(&controller);
    assert_eq!(completion.outcome(), DesktopRefreshOutcome::Completed);

    let receiver = controller.snapshot_receiver();
    let notifications = Arc::new(AtomicUsize::new(0));
    let populated = Arc::new(AtomicUsize::new(0));
    controller
        .attach_snapshot_notifier(Arc::new(InspectingNotifier {
            receiver: receiver.clone(),
            notifications: notifications.clone(),
            populated: populated.clone(),
        }))
        .expect("idle notifier attaches after publication");

    assert_eq!(notifications.load(Ordering::Acquire), 1);
    assert_eq!(populated.load(Ordering::Acquire), 1);
    assert!(receiver.has_snapshot().expect("mailbox remains available"));
    controller.shutdown().expect("controller stops");
}

#[test]
fn notifier_attachment_is_rejected_while_refresh_is_active() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let source = BlockingUnavailableSource {
        calls,
        entered: entered_sender,
        release: Some(release_receiver),
    };
    let mut controller =
        DesktopController::spawn(source, DesktopQueryPlan::overview().expect("overview plan"))
            .expect("controller starts");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh starts");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("query entered");

    let error = controller
        .attach_snapshot_notifier(Arc::new(NoopNotifier))
        .expect_err("active refresh rejects attachment");
    assert_eq!(error.stable_code(), "busy");
    release_sender.send(()).expect("release query");
    let _ = wait_for_completion(&controller);
    controller.shutdown().expect("controller stops");
}

#[test]
fn refresh_hints_coalesce_and_only_the_newest_snapshot_is_retained() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let source = BlockingUnavailableSource {
        calls: calls.clone(),
        entered: entered_sender,
        release: Some(release_receiver),
    };
    let mut controller =
        DesktopController::spawn(source, DesktopQueryPlan::overview().expect("overview plan"))
            .expect("controller starts");

    let first = started_attempt(
        controller
            .refresh(DesktopRefreshUrgency::Interactive)
            .expect("first admission"),
    );
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("first query entered");

    let mut newest_receipt = None;
    for _ in 0..1_000 {
        newest_receipt = Some(
            match controller
                .refresh(DesktopRefreshUrgency::Hint)
                .expect("hint coalesced")
            {
                DesktopRefreshAdmission::Coalesced {
                    receipt,
                    active_attempt,
                } => {
                    assert_eq!(active_attempt, first);
                    receipt
                }
                other => panic!("unexpected coalesced admission: {other:?}"),
            },
        );
    }
    release_sender.send(()).expect("release first query");

    let expected_follow_up = newest_receipt
        .expect("coalesced receipt")
        .get()
        .checked_add(1)
        .expect("follow-up id");
    let snapshot = wait_for_snapshot_attempt(&controller, expected_follow_up);
    assert_eq!(calls.load(Ordering::Acquire), 2);
    assert_eq!(
        snapshot
            .data_status()
            .attempt_generation()
            .expect("newest attempt")
            .get(),
        expected_follow_up
    );
    assert!(
        controller
            .take_snapshot()
            .expect("latest slot remains healthy")
            .is_none()
    );
    controller.shutdown().expect("controller stops");
}

#[test]
fn cancellation_discards_partial_attempt_publication() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let source = BlockingUnavailableSource {
        calls,
        entered: entered_sender,
        release: Some(release_receiver),
    };
    let mut controller =
        DesktopController::spawn(source, DesktopQueryPlan::overview().expect("overview plan"))
            .expect("controller starts");
    let attempt = started_attempt(
        controller
            .refresh(DesktopRefreshUrgency::Interactive)
            .expect("refresh admitted"),
    );
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("first query entered");
    controller.cancel(attempt).expect("attempt cancelled");
    release_sender.send(()).expect("release first query");

    let completion = wait_for_completion(&controller);
    assert_eq!(completion.attempt(), attempt);
    assert_eq!(completion.outcome(), DesktopRefreshOutcome::Cancelled);
    assert!(
        controller
            .take_snapshot()
            .expect("latest slot remains healthy")
            .is_none()
    );
    controller.shutdown().expect("controller stops");
}

#[test]
fn real_empty_archive_publishes_truth_and_keeps_one_section_failure_local() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("controller.sqlite3");
    drop(UsageStore::open(&path).expect("create schema-v13 archive"));
    let source = AnalyticsFaultSource {
        inner: QueryService::open(&path, SystemQueryClock::new()).expect("query service"),
    };
    let mut controller =
        DesktopController::spawn(source, DesktopQueryPlan::overview().expect("overview plan"))
            .expect("controller starts");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh admitted");
    let completion = wait_for_completion(&controller);
    assert_eq!(completion.outcome(), DesktopRefreshOutcome::Completed);
    let snapshot = controller
        .take_snapshot()
        .expect("latest slot remains healthy")
        .expect("snapshot published");

    assert_eq!(snapshot.data_status().kind(), ProductSectionKind::Ready);
    assert_eq!(snapshot.analytics().kind(), ProductSectionKind::Unavailable);
    assert_eq!(snapshot.quota().kind(), ProductSectionKind::Ready);
    assert_eq!(snapshot.benefit().kind(), ProductSectionKind::Ready);
    assert_eq!(snapshot.git().kind(), ProductSectionKind::Ready);
    assert_eq!(snapshot.activity().kind(), ProductSectionKind::Ready);
    assert_eq!(snapshot.sessions().kind(), ProductSectionKind::Ready);
    controller.shutdown().expect("controller stops");
}

#[test]
fn open_error_is_stable_and_does_not_disclose_the_archive_path() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let marker = "private-controller-marker-9d3a";
    let path: PathBuf = directory.path().join(format!("{marker}.sqlite3"));
    let error = match DesktopController::open(
        &path,
        DesktopQueryPlan::overview().expect("overview plan"),
    ) {
        Ok(mut controller) => {
            let _ = controller.shutdown();
            panic!("missing archive must fail")
        }
        Err(error) => error,
    };
    assert_eq!(error.stable_code(), "unavailable");
    assert_eq!(error.to_string(), "unavailable");
    assert!(!error.to_string().contains(marker));
    assert!(!error.to_string().contains(path.to_string_lossy().as_ref()));
    assert!(!path.exists());
}

struct BlockingUnavailableSource {
    calls: Arc<AtomicUsize>,
    entered: SyncSender<()>,
    release: Option<Receiver<()>>,
}

struct InspectingNotifier {
    receiver: DesktopSnapshotReceiver,
    notifications: Arc<AtomicUsize>,
    populated: Arc<AtomicUsize>,
}

impl DesktopSnapshotNotifier for InspectingNotifier {
    fn snapshot_ready(&self) {
        self.notifications.fetch_add(1, Ordering::AcqRel);
        if self.receiver.has_snapshot().unwrap_or(false) {
            self.populated.fetch_add(1, Ordering::AcqRel);
        }
    }
}

struct NoopNotifier;

impl DesktopSnapshotNotifier for NoopNotifier {
    fn snapshot_ready(&self) {}
}

impl DesktopQuerySource for BlockingUnavailableSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if call == 0 {
            self.entered.send(()).expect("signal first query");
            self.release
                .take()
                .expect("one release receiver")
                .recv()
                .expect("release first query");
        }
        Err(query_failure())
    }

    delegate_unavailable_methods!();
}

struct AnalyticsFaultSource {
    inner: QueryService<SystemQueryClock>,
}

impl DesktopQuerySource for AnalyticsFaultSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        self.inner.product_data_status()
    }

    fn usage_analytics(
        &mut self,
        _request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        Err(query_failure())
    }

    fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        self.inner.quota_overview()
    }

    fn benefit_overview(
        &mut self,
        request: BenefitOverviewRequest,
    ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
        self.inner.benefit_overview(request)
    }

    fn git_output(
        &mut self,
        request: GitOutputRequest,
    ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
        self.inner.git_output(request)
    }

    fn latest_activity(
        &mut self,
        request: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
        self.inner.latest_activity(request)
    }

    fn usage_sessions(
        &mut self,
        request: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
        self.inner.usage_sessions(request)
    }
}

fn started_attempt(admission: DesktopRefreshAdmission) -> DesktopAttempt {
    match admission {
        DesktopRefreshAdmission::Started { attempt } => attempt,
        other => panic!("unexpected first admission: {other:?}"),
    }
}

fn wait_for_snapshot_attempt(
    controller: &DesktopController,
    expected: u64,
) -> Arc<tokenmaster_product::ProductSnapshot> {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(snapshot) = controller
            .take_snapshot()
            .expect("latest slot remains healthy")
            && snapshot
                .data_status()
                .attempt_generation()
                .is_some_and(|attempt| attempt.get() == expected)
        {
            return snapshot;
        }
        assert!(std::time::Instant::now() < deadline, "snapshot timed out");
        thread::yield_now();
    }
}

fn wait_for_completion(
    controller: &DesktopController,
) -> tokenmaster_desktop::DesktopRefreshCompletion {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(completion) = controller
            .try_completion()
            .expect("completion channel remains healthy")
        {
            return completion;
        }
        assert!(std::time::Instant::now() < deadline, "controller timed out");
        thread::yield_now();
    }
}

fn query_failure() -> QueryError {
    PageSize::new(0).expect_err("zero page size is invalid")
}
