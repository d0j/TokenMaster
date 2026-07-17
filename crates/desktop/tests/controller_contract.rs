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
};
use tokenmaster_product::ProductSectionKind;
use tokenmaster_query::{
    BenefitCurrentRequest, BenefitCurrentSnapshot, BenefitEnvelope, GitEnvelope, GitOutputRequest,
    GitOutputSnapshot, LatestActivityPage, LatestActivityRequest, PageSize,
    ProductDataStatusEnvelope, QueryEnvelope, QueryError, QueryService, QuotaCurrentRequest,
    QuotaCurrentSnapshot, QuotaEnvelope, SystemQueryClock, UsageAnalytics, UsageAnalyticsRequest,
    UsageSessionPage, UsageSessionPageRequest,
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

        fn quota_windows(
            &mut self,
            _request: QuotaCurrentRequest,
        ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
            Err(query_failure())
        }

        fn benefit_inventory(
            &mut self,
            _request: BenefitCurrentRequest,
        ) -> Result<BenefitEnvelope<BenefitCurrentSnapshot>, QueryError> {
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

    fn quota_windows(
        &mut self,
        _request: QuotaCurrentRequest,
    ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        self.record("quota");
        Err(query_failure())
    }

    fn benefit_inventory(
        &mut self,
        _request: BenefitCurrentRequest,
    ) -> Result<BenefitEnvelope<BenefitCurrentSnapshot>, QueryError> {
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
        _request: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
        self.record("activity");
        Err(query_failure())
    }

    fn usage_sessions(
        &mut self,
        _request: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
        self.record("sessions");
        Err(query_failure())
    }
}

#[test]
fn controller_contract_is_typed_bounded_and_deterministic() {
    let plan = DesktopQueryPlan::overview().expect("bounded overview plan");
    assert_eq!(DesktopQueryPlan::MAX_SERIES_POINTS, 240);
    assert_eq!(DesktopQueryPlan::MAX_PAGE_ROWS, 256);
    assert_eq!(DesktopQueryPlan::MAX_REPOSITORIES, 32);
    assert!(plan.benefit_request().is_none());

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
    assert_eq!(snapshot.benefit().kind(), ProductSectionKind::Unavailable);
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

    fn quota_windows(
        &mut self,
        request: QuotaCurrentRequest,
    ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        self.inner.quota_windows(request)
    }

    fn benefit_inventory(
        &mut self,
        request: BenefitCurrentRequest,
    ) -> Result<BenefitEnvelope<BenefitCurrentSnapshot>, QueryError> {
        self.inner.benefit_inventory(request)
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
