mod support;

use rusqlite::Connection;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread,
    time::Duration,
};
use support::dashboard_fixture::{FixedClock, seed};
use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopController, DesktopHistoryRangeGeneration, DesktopHistoryRangeIntent,
    DesktopHistoryRangePreset, DesktopQueryPlan, DesktopQuerySource, DesktopRefreshUrgency,
    DesktopSessionDetailIntent, DesktopSessionNavigationGeneration, DesktopSessionPageDirection,
    DesktopSessionPageIntent, DesktopSnapshotEpoch, DesktopSnapshotNotifier,
    DesktopSnapshotReceiver, DesktopTerminalHistoryRangeNotifier,
    DesktopTerminalNavigationNotifier,
};
use tokenmaster_product::{
    ProductGeneration, ProductSessionDetailSelection, ProductSessionDetailSelectionGeneration,
};
use tokenmaster_query::QueryService;
use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewRequest, BenefitOverviewSnapshot, GitEnvelope,
    GitOutputRequest, GitOutputSnapshot, LatestActivityPage, LatestActivityRequest,
    ProductDataStatusEnvelope, QueryEnvelope, QueryError, QuotaCurrentSnapshot, QuotaEnvelope,
    UsageAnalytics, UsageAnalyticsRequest, UsageRange, UsageSessionDetailResult, UsageSessionKey,
    UsageSessionPage, UsageSessionPageRequest,
};

#[test]
fn history_range_presets_are_closed_and_default_to_thirty_days() {
    assert_eq!(DesktopHistoryRangePreset::Recent1Day.day_count(), 1);
    assert_eq!(DesktopHistoryRangePreset::Recent7Days.day_count(), 7);
    assert_eq!(DesktopHistoryRangePreset::Recent30Days.day_count(), 30);
    assert_eq!(
        DesktopHistoryRangePreset::Recent1Day.stable_code(),
        "recent_1_day"
    );
    assert_eq!(
        DesktopHistoryRangePreset::Recent7Days.stable_code(),
        "recent_7_days"
    );
    assert_eq!(
        DesktopHistoryRangePreset::Recent30Days.stable_code(),
        "recent_30_days"
    );
    assert_eq!(
        DesktopQueryPlan::default_history_range_preset(),
        DesktopHistoryRangePreset::Recent30Days
    );
}

#[test]
fn history_range_generation_is_checked_and_intent_is_path_free() {
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let generation = DesktopHistoryRangeGeneration::new(7).expect("nonzero generation");
    let intent = DesktopHistoryRangeIntent::new(
        epoch,
        ProductGeneration::INITIAL,
        generation,
        DesktopHistoryRangePreset::Recent7Days,
    );

    assert!(DesktopHistoryRangeGeneration::new(0).is_none());
    assert_eq!(intent.snapshot_epoch(), epoch);
    assert_eq!(intent.product_generation(), ProductGeneration::INITIAL);
    assert_eq!(intent.generation(), generation);
    assert_eq!(intent.preset(), DesktopHistoryRangePreset::Recent7Days);
    let debug = format!("{intent:?}");
    assert!(!debug.contains("path"));
    assert!(!debug.contains("scope"));
    assert!(!debug.contains("date"));
}

struct UnavailableSource;

impl DesktopQuerySource for UnavailableSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        Err(query_failure())
    }
    fn usage_analytics(
        &mut self,
        _: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        Err(query_failure())
    }
    fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        Err(query_failure())
    }
    fn benefit_overview(
        &mut self,
        _: BenefitOverviewRequest,
    ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
        Err(query_failure())
    }
    fn git_output(
        &mut self,
        _: GitOutputRequest,
    ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
        Err(query_failure())
    }
    fn latest_activity(
        &mut self,
        _: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
        Err(query_failure())
    }
    fn usage_sessions(
        &mut self,
        _: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
        Err(query_failure())
    }
    fn usage_session_detail(
        &mut self,
        _: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        Err(query_failure())
    }
}

struct WorkerReleaseGuard {
    sender: Option<SyncSender<()>>,
}

impl WorkerReleaseGuard {
    fn new(sender: SyncSender<()>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    fn release(&mut self) {
        self.sender
            .take()
            .expect("worker release sender")
            .send(())
            .expect("release worker");
    }
}

impl Drop for WorkerReleaseGuard {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(());
        }
    }
}

#[test]
fn worker_release_guard_releases_on_drop() {
    let (sender, receiver) = sync_channel(1);
    {
        let _release = WorkerReleaseGuard::new(sender);
    }
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("drop releases worker");
}

struct BlockingDetailSource {
    inner: QueryService<FixedClock>,
    entered: Option<SyncSender<()>>,
    release: Option<Receiver<()>>,
    range_armed: Arc<AtomicBool>,
    range_entered: Option<SyncSender<()>>,
    range_release: Option<Receiver<()>>,
    drift_armed: Arc<AtomicBool>,
    drift: Option<QueryService<FixedClock>>,
    range_failure: Arc<AtomicBool>,
}

impl DesktopQuerySource for BlockingDetailSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        self.inner.product_data_status()
    }

    fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        if request.range().stable_code() == "recent_days"
            && self.drift_armed.swap(false, Ordering::AcqRel)
        {
            return self
                .drift
                .as_mut()
                .expect("dataset drift query service")
                .usage_analytics(request);
        }
        if request.range().stable_code() == "recent_days"
            && self.range_failure.swap(false, Ordering::AcqRel)
        {
            return Err(query_failure());
        }
        if request.range().stable_code() == "recent_days"
            && self.range_armed.swap(false, Ordering::AcqRel)
        {
            self.range_entered
                .take()
                .expect("range entry sender")
                .send(())
                .expect("range entry signal");
            self.range_release
                .take()
                .expect("range release receiver")
                .recv()
                .expect("range release");
        }
        self.inner.usage_analytics(request)
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

    fn usage_session_detail(
        &mut self,
        key: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        if let Some(entered) = self.entered.take() {
            entered.send(()).expect("detail entry signal");
            self.release
                .take()
                .expect("detail release receiver")
                .recv()
                .expect("detail release");
        }
        self.inner.usage_session_detail(key)
    }
}

struct BlockingRefreshSource {
    inner: QueryService<FixedClock>,
    armed: Arc<AtomicBool>,
    entered: Option<SyncSender<()>>,
    release: Option<Receiver<()>>,
}

impl DesktopQuerySource for BlockingRefreshSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        if self.armed.swap(false, Ordering::AcqRel) {
            self.entered
                .take()
                .expect("refresh entry sender")
                .send(())
                .expect("refresh entry signal");
            self.release
                .take()
                .expect("refresh release receiver")
                .recv()
                .expect("refresh release");
        }
        self.inner.product_data_status()
    }

    fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        self.inner.usage_analytics(request)
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

    fn usage_session_detail(
        &mut self,
        key: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        self.inner.usage_session_detail(key)
    }
}

struct BlockingNavigationSource {
    inner: QueryService<FixedClock>,
    armed: Arc<AtomicBool>,
    entered: Option<SyncSender<()>>,
    release: Option<Receiver<()>>,
}

impl DesktopQuerySource for BlockingNavigationSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        self.inner.product_data_status()
    }

    fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        self.inner.usage_analytics(request)
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
        if self.armed.swap(false, Ordering::AcqRel) {
            let entered = self.entered.take().expect("navigation entry sender");
            entered.send(()).expect("navigation entry signal");
            self.release
                .take()
                .expect("navigation release receiver")
                .recv()
                .expect("navigation release");
        }
        self.inner.usage_sessions(request)
    }

    fn usage_session_detail(
        &mut self,
        key: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        self.inner.usage_session_detail(key)
    }
}

struct RecordingHistorySource {
    inner: QueryService<FixedClock>,
    requests: Arc<Mutex<Vec<UsageAnalyticsRequest>>>,
}

impl DesktopQuerySource for RecordingHistorySource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        self.inner.product_data_status()
    }

    fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        self.requests
            .lock()
            .expect("request lock")
            .push(request.clone());
        self.inner.usage_analytics(request)
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

    fn usage_session_detail(
        &mut self,
        key: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        self.inner.usage_session_detail(key)
    }
}

struct RecordingHistoryTerminalNotifier {
    sender: SyncSender<DesktopHistoryRangeIntent>,
}

impl DesktopTerminalHistoryRangeNotifier for RecordingHistoryTerminalNotifier {
    fn history_range_terminal(&self, intent: DesktopHistoryRangeIntent) {
        let _ = self.sender.try_send(intent);
    }
}

struct RecordingNavigationTerminalNotifier {
    sender: SyncSender<DesktopSessionPageIntent>,
}

impl DesktopTerminalNavigationNotifier for RecordingNavigationTerminalNotifier {
    fn navigation_terminal(&self, intent: DesktopSessionPageIntent) {
        let _ = self.sender.try_send(intent);
    }
}

struct SnapshotBeforeTerminalNotifier {
    receiver: DesktopSnapshotReceiver,
    sender: SyncSender<()>,
}

impl DesktopSnapshotNotifier for SnapshotBeforeTerminalNotifier {
    fn snapshot_ready(&self) {
        assert!(
            self.receiver.has_snapshot().expect("snapshot mailbox"),
            "snapshot publication precedes observation"
        );
        let _ = self.sender.try_send(());
    }
}

fn query_failure() -> QueryError {
    tokenmaster_query::PageSize::new(0).expect_err("zero page size is invalid")
}

fn ready_controller() -> (DesktopController, DesktopSnapshotEpoch, ProductGeneration) {
    let mut controller = DesktopController::spawn(
        UnavailableSource,
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while controller.try_completion().expect("completion").is_none() {
        assert!(std::time::Instant::now() < deadline, "refresh timed out");
        thread::yield_now();
    }
    let generation = controller
        .published_product_generation()
        .expect("published generation")
        .expect("initial publication");
    (controller, epoch, generation)
}

#[test]
fn history_range_admission_rejects_stale_same_and_non_newer_intents() {
    let (mut controller, epoch, generation) = ready_controller();
    let same = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent30Days,
    );
    assert_eq!(
        controller
            .request_history_range(same)
            .expect_err("same preset")
            .stable_code(),
        "stale_history_range"
    );
    let stale_epoch = DesktopHistoryRangeIntent::new(
        DesktopSnapshotEpoch::new(2).expect("epoch"),
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(stale_epoch)
            .expect_err("stale epoch")
            .stable_code(),
        "stale_history_range"
    );
    let stale_product = DesktopHistoryRangeIntent::new(
        epoch,
        ProductGeneration::INITIAL,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(stale_product)
            .expect_err("stale product generation")
            .stable_code(),
        "stale_history_range"
    );
    let accepted = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(accepted)
        .expect("first range admission");
    let non_newer = DesktopHistoryRangeIntent::new(
        epoch,
        generation,
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent7Days,
    );
    assert_eq!(
        controller
            .request_history_range(non_newer)
            .expect_err("non-newer generation")
            .stable_code(),
        "stale_history_range"
    );
    controller.shutdown().expect("shutdown");
}

#[test]
fn range_and_sessions_admissions_are_mutually_exclusive_without_displacement() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-range-blocks-page.sqlite3");
    seed(&path);
    let range_armed = Arc::new(AtomicBool::new(false));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingDetailSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            entered: None,
            release: None,
            range_armed: Arc::clone(&range_armed),
            range_entered: Some(entered_sender),
            range_release: Some(release_receiver),
            drift_armed: Arc::new(AtomicBool::new(false)),
            drift: None,
            range_failure: Arc::new(AtomicBool::new(false)),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    let mut range_release = WorkerReleaseGuard::new(release_sender);
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");

    range_armed.store(true, Ordering::Release);
    let range = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(range)
        .expect("range admission");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("range query entered");
    let page = DesktopSessionPageIntent::new(
        epoch,
        initial.generation(),
        DesktopSessionNavigationGeneration::new(1).expect("generation"),
        DesktopSessionPageDirection::Newest,
    );
    let page_result = controller.request_session_page(page);
    range_release.release();
    let _ = wait_for_completion(&controller);
    controller.shutdown().expect("shutdown");
    assert_eq!(
        page_result.expect_err("range blocks page").stable_code(),
        "busy"
    );
}

#[test]
fn active_detail_query_keeps_history_range_busy_until_terminal_completion() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-range-active-detail.sqlite3");
    seed(&path);
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingDetailSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            entered: Some(entered_sender),
            release: Some(release_receiver),
            range_armed: Arc::new(AtomicBool::new(false)),
            range_entered: None,
            range_release: None,
            drift_armed: Arc::new(AtomicBool::new(false)),
            drift: None,
            range_failure: Arc::new(AtomicBool::new(false)),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    let mut detail_release = WorkerReleaseGuard::new(release_sender);
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    let selection = ProductSessionDetailSelection::new(
        ProductSessionDetailSelectionGeneration::new(1).expect("selection generation"),
        0,
    );
    controller
        .request_session_detail(DesktopSessionDetailIntent::new(
            epoch,
            initial.generation(),
            selection,
        ))
        .expect("detail admission");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("detail query entered");
    let range = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(range)
            .expect_err("active detail blocks range")
            .stable_code(),
        "busy"
    );
    detail_release.release();
    let _ = wait_for_completion(&controller);
    let detail = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("detail snapshot");
    let recovered_range = DesktopHistoryRangeIntent::new(
        epoch,
        detail.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(recovered_range)
        .expect("detail completion clears active scalar");
    let _ = wait_for_completion(&controller);
    controller.shutdown().expect("shutdown");
}

#[test]
fn active_and_pending_navigation_reject_range_without_displacement() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("history-range-active-navigation.sqlite3");
    seed(&path);
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let armed = Arc::new(AtomicBool::new(false));
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingNavigationSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            armed: Arc::clone(&armed),
            entered: Some(entered_sender),
            release: Some(release_receiver),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    armed.store(true, Ordering::Release);
    controller
        .request_session_page(DesktopSessionPageIntent::new(
            epoch,
            initial.generation(),
            DesktopSessionNavigationGeneration::new(1).expect("navigation generation"),
            DesktopSessionPageDirection::Newest,
        ))
        .expect("active navigation admission");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("navigation query entered");
    let range = |generation| {
        DesktopHistoryRangeIntent::new(
            epoch,
            initial.generation(),
            DesktopHistoryRangeGeneration::new(generation).expect("range generation"),
            DesktopHistoryRangePreset::Recent1Day,
        )
    };
    assert_eq!(
        controller
            .request_history_range(range(1))
            .expect_err("active navigation blocks range")
            .stable_code(),
        "busy"
    );
    assert!(matches!(
        controller
            .request_session_page(DesktopSessionPageIntent::new(
                epoch,
                initial.generation(),
                DesktopSessionNavigationGeneration::new(2).expect("navigation generation"),
                DesktopSessionPageDirection::Next,
            ))
            .expect("pending navigation admission"),
        tokenmaster_desktop::DesktopRefreshAdmission::Coalesced { .. }
    ));
    assert_eq!(
        controller
            .request_history_range(range(2))
            .expect_err("pending navigation blocks range without displacement")
            .stable_code(),
        "busy"
    );
    release_sender.send(()).expect("release navigation");
    let terminal = wait_for_terminal_completion(&controller);
    assert_eq!(
        terminal.outcome(),
        tokenmaster_desktop::DesktopRefreshOutcome::Completed
    );
    assert!(!terminal.follow_up_started());
    controller.shutdown().expect("shutdown");
}

#[test]
fn ten_thousand_range_intents_keep_only_the_direct_latest_follow_up() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-range-direct-latest.sqlite3");
    seed(&path);
    let range_armed = Arc::new(AtomicBool::new(false));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingDetailSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            entered: None,
            release: None,
            range_armed: Arc::clone(&range_armed),
            range_entered: Some(entered_sender),
            range_release: Some(release_receiver),
            drift_armed: Arc::new(AtomicBool::new(false)),
            drift: None,
            range_failure: Arc::new(AtomicBool::new(false)),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");

    range_armed.store(true, Ordering::Release);
    for generation in 1..=10_000_u64 {
        let preset = if generation % 2 == 0 {
            DesktopHistoryRangePreset::Recent7Days
        } else {
            DesktopHistoryRangePreset::Recent1Day
        };
        let admission = controller
            .request_history_range(DesktopHistoryRangeIntent::new(
                epoch,
                initial.generation(),
                DesktopHistoryRangeGeneration::new(generation).expect("range generation"),
                preset,
            ))
            .expect("direct latest range admission");
        if generation == 1 {
            assert!(matches!(
                admission,
                tokenmaster_desktop::DesktopRefreshAdmission::Started { .. }
            ));
            entered_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("first range query entered");
            assert_eq!(
                controller
                    .request_session_detail(DesktopSessionDetailIntent::new(
                        epoch,
                        initial.generation(),
                        ProductSessionDetailSelection::new(
                            ProductSessionDetailSelectionGeneration::new(1)
                                .expect("selection generation"),
                            0,
                        ),
                    ))
                    .expect_err("active range blocks detail without displacement")
                    .stable_code(),
                "busy"
            );
        } else {
            assert!(matches!(
                admission,
                tokenmaster_desktop::DesktopRefreshAdmission::Coalesced { .. }
            ));
        }
    }
    release_sender.send(()).expect("release first range");
    let _ = wait_for_completion(&controller);
    let _ = wait_for_completion(&controller);
    let published = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("latest range snapshot");
    assert_eq!(
        controller
            .request_history_range(DesktopHistoryRangeIntent::new(
                epoch,
                published.generation(),
                DesktopHistoryRangeGeneration::new(10_001).expect("range generation"),
                DesktopHistoryRangePreset::Recent7Days,
            ))
            .expect_err("only the ten-thousandth direct intent survives")
            .stable_code(),
        "stale_history_range"
    );
    controller.shutdown().expect("shutdown");
}

#[test]
fn cancelled_range_notifies_exactly_once_without_a_snapshot() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-range-cancel.sqlite3");
    seed(&path);
    let range_armed = Arc::new(AtomicBool::new(false));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingDetailSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            entered: None,
            release: None,
            range_armed: Arc::clone(&range_armed),
            range_entered: Some(entered_sender),
            range_release: Some(release_receiver),
            drift_armed: Arc::new(AtomicBool::new(false)),
            drift: None,
            range_failure: Arc::new(AtomicBool::new(false)),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    let (terminal_sender, terminal_receiver) = sync_channel(1);
    let (navigation_sender, navigation_receiver) = sync_channel(1);
    controller
        .attach_terminal_navigation_notifier(Arc::new(RecordingNavigationTerminalNotifier {
            sender: navigation_sender,
        }))
        .expect("attach navigation terminal notifier");
    controller
        .attach_terminal_history_range_notifier(Arc::new(RecordingHistoryTerminalNotifier {
            sender: terminal_sender,
        }))
        .expect("attach history terminal notifier");
    range_armed.store(true, Ordering::Release);
    let intent = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    let attempt = match controller
        .request_history_range(intent)
        .expect("range admission")
    {
        tokenmaster_desktop::DesktopRefreshAdmission::Started { attempt } => attempt,
        other => panic!("expected started range, got {other:?}"),
    };
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("range query entered");
    controller.cancel(attempt).expect("cancel range");
    release_sender.send(()).expect("release range");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        tokenmaster_desktop::DesktopRefreshOutcome::Cancelled
    );
    assert!(controller.take_snapshot().expect("mailbox").is_none());
    assert_eq!(
        terminal_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("exact terminal rollback"),
        intent
    );
    assert!(
        terminal_receiver.try_recv().is_err(),
        "terminal rollback is idempotent"
    );
    assert!(
        navigation_receiver.try_recv().is_err(),
        "history terminal does not displace the independent navigation slot"
    );
    controller.shutdown().expect("shutdown");
}

#[test]
fn dataset_identity_drift_is_rejected_without_snapshot_or_preset_advance() {
    let directory = TempDir::new().expect("temporary directory");
    let primary_path = directory.path().join("history-range-primary.sqlite3");
    let drift_path = directory.path().join("history-range-drift.sqlite3");
    seed(&primary_path);
    seed(&drift_path);
    Connection::open(&drift_path)
        .expect("drift connection")
        .execute_batch(
            "UPDATE usage_replay_revision
              SET status = 'staging', sealed = 0, promoted = 0
              WHERE revision_id = 0;
              INSERT INTO usage_replay_revision(
                revision_id, status, canonicalizer_version, fingerprint_version,
                replay_signature_version, expected_source_count, evidence_epoch,
                sealed, promoted, scan_set_id
              ) VALUES (1, 'current', 1, 2, 1, 1, 1, 1, 1, 1);
              UPDATE usage_archive_state
              SET current_revision_id = 1
              WHERE singleton_id = 1;",
        )
        .expect("advance drift dataset identity");
    let drift_armed = Arc::new(AtomicBool::new(false));
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingDetailSource {
            inner: QueryService::open(&primary_path, FixedClock).expect("primary query service"),
            entered: None,
            release: None,
            range_armed: Arc::new(AtomicBool::new(false)),
            range_entered: None,
            range_release: None,
            drift_armed: Arc::clone(&drift_armed),
            drift: Some(QueryService::open(&drift_path, FixedClock).expect("drift query service")),
            range_failure: Arc::new(AtomicBool::new(false)),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    let (terminal_sender, terminal_receiver) = sync_channel(1);
    controller
        .attach_terminal_history_range_notifier(Arc::new(RecordingHistoryTerminalNotifier {
            sender: terminal_sender,
        }))
        .expect("attach history terminal notifier");
    drift_armed.store(true, Ordering::Release);
    let intent = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(intent)
        .expect("range admission");
    let _ = wait_for_completion(&controller);
    assert!(controller.take_snapshot().expect("mailbox").is_none());
    assert_eq!(
        terminal_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("dataset drift terminal rollback"),
        intent
    );
    let same_preset = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(2).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(same_preset)
        .expect("rejected drift does not advance the published preset");
    let _ = wait_for_completion(&controller);
    controller.shutdown().expect("shutdown");
}

#[test]
fn accepted_success_advances_preset_and_accepted_failure_retains_it() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("history-range-publication.sqlite3");
    seed(&path);
    let range_failure = Arc::new(AtomicBool::new(false));
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingDetailSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            entered: None,
            release: None,
            range_armed: Arc::new(AtomicBool::new(false)),
            range_entered: None,
            range_release: None,
            drift_armed: Arc::new(AtomicBool::new(false)),
            drift: None,
            range_failure: Arc::clone(&range_failure),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    let (snapshot_sender, snapshot_receiver) = sync_channel(2);
    controller
        .attach_snapshot_notifier(Arc::new(SnapshotBeforeTerminalNotifier {
            receiver: controller.snapshot_receiver(),
            sender: snapshot_sender,
        }))
        .expect("attach snapshot notifier");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    snapshot_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("initial snapshot observation");
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    let initial_history = Arc::clone(
        initial
            .history()
            .payload()
            .expect("default history payload"),
    );
    let success = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    let (terminal_sender, terminal_receiver) = sync_channel(1);
    controller
        .attach_terminal_history_range_notifier(Arc::new(RecordingHistoryTerminalNotifier {
            sender: terminal_sender,
        }))
        .expect("attach terminal notifier");
    controller
        .request_history_range(success)
        .expect("successful range admission");
    let _ = wait_for_completion(&controller);
    snapshot_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("successful range snapshot observation");
    assert!(
        terminal_receiver.try_recv().is_err(),
        "successful commit consumes current work before terminal reconciliation"
    );
    let published = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("successful range snapshot");
    let successful_history = Arc::clone(
        published
            .history()
            .payload()
            .expect("successful history payload"),
    );
    assert!(
        !Arc::ptr_eq(&initial_history, &successful_history),
        "accepted range replaces the default history payload"
    );
    assert_eq!(successful_history.payload().series().len(), 1);
    let same_preset = DesktopHistoryRangeIntent::new(
        epoch,
        published.generation(),
        DesktopHistoryRangeGeneration::new(2).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(same_preset)
            .expect_err("successful range advances published preset")
            .stable_code(),
        "stale_history_range"
    );
    range_failure.store(true, Ordering::Release);
    let failure = DesktopHistoryRangeIntent::new(
        epoch,
        published.generation(),
        DesktopHistoryRangeGeneration::new(2).expect("range generation"),
        DesktopHistoryRangePreset::Recent7Days,
    );
    controller
        .request_history_range(failure)
        .expect("failing range admission");
    let _ = wait_for_completion(&controller);
    let degraded = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("accepted failure publishes degraded snapshot");
    let retained_history = degraded
        .history()
        .payload()
        .expect("degraded history retains prior payload");
    assert!(degraded.history().retains_payload());
    assert!(
        Arc::ptr_eq(&successful_history, retained_history),
        "accepted failure retains the exact successful range payload"
    );
    let retained_preset = DesktopHistoryRangeIntent::new(
        epoch,
        degraded.generation(),
        DesktopHistoryRangeGeneration::new(3).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(retained_preset)
            .expect_err("failure retains last successful preset")
            .stable_code(),
        "stale_history_range"
    );
    controller.shutdown().expect("shutdown");
}

#[test]
fn ordinary_refresh_reuses_each_successful_history_preset_and_payload() {
    for (preset, day_count) in [
        (DesktopHistoryRangePreset::Recent1Day, 1_u16),
        (DesktopHistoryRangePreset::Recent7Days, 7_u16),
    ] {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory
            .path()
            .join("history-range-refresh-preset.sqlite3");
        seed(&path);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        let mut controller = DesktopController::spawn(
            RecordingHistorySource {
                inner: QueryService::open(&path, FixedClock).expect("query service"),
                requests: Arc::clone(&requests),
            },
            DesktopQueryPlan::overview().expect("plan"),
        )
        .expect("controller");
        controller.bind_snapshot_epoch(epoch).expect("bind epoch");
        controller
            .refresh(DesktopRefreshUrgency::Interactive)
            .expect("initial refresh");
        let _ = wait_for_completion(&controller);
        let initial = controller
            .take_snapshot()
            .expect("mailbox")
            .expect("initial snapshot");
        requests.lock().expect("request lock").clear();
        controller
            .request_history_range(DesktopHistoryRangeIntent::new(
                epoch,
                initial.generation(),
                DesktopHistoryRangeGeneration::new(1).expect("range generation"),
                preset,
            ))
            .expect("range admission");
        let _ = wait_for_completion(&controller);
        let selected = controller
            .take_snapshot()
            .expect("mailbox")
            .expect("selected range snapshot");
        let selected_history = Arc::clone(
            selected
                .history()
                .payload()
                .expect("selected history payload"),
        );
        assert_eq!(
            selected_history.payload().series().len(),
            usize::from(day_count)
        );
        requests.lock().expect("request lock").clear();

        controller
            .refresh(DesktopRefreshUrgency::Interactive)
            .expect("ordinary refresh");
        let _ = wait_for_completion(&controller);
        let refreshed = controller
            .take_snapshot()
            .expect("mailbox")
            .expect("refreshed snapshot");
        let refreshed_history = refreshed
            .history()
            .payload()
            .expect("refreshed history payload");
        assert_eq!(
            refreshed_history.payload().series().len(),
            usize::from(day_count)
        );
        assert_eq!(refreshed_history.payload(), selected_history.payload());
        let history_requests = requests
            .lock()
            .expect("request lock")
            .iter()
            .filter(|request| request.range().stable_code() == "recent_days")
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(history_requests.len(), 1);
        assert_eq!(
            history_requests[0].range(),
            &UsageRange::recent_days(day_count).expect("bounded history range")
        );
        controller.shutdown().expect("shutdown");
    }
}

#[test]
fn range_queued_behind_exact_refresh_rebinds_once_to_its_published_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("history-range-refresh-rebind.sqlite3");
    seed(&path);
    let armed = Arc::new(AtomicBool::new(false));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingRefreshSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            armed: Arc::clone(&armed),
            entered: Some(entered_sender),
            release: Some(release_receiver),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    let (snapshot_sender, snapshot_receiver) = sync_channel(3);
    controller
        .attach_snapshot_notifier(Arc::new(SnapshotBeforeTerminalNotifier {
            receiver: controller.snapshot_receiver(),
            sender: snapshot_sender,
        }))
        .expect("attach snapshot notifier");

    armed.store(true, Ordering::Release);
    controller
        .refresh(DesktopRefreshUrgency::Hint)
        .expect("blocking refresh");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("refresh query entered");
    let intent = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert!(matches!(
        controller
            .request_history_range(intent)
            .expect("range queues behind exact refresh"),
        tokenmaster_desktop::DesktopRefreshAdmission::Coalesced { .. }
    ));
    release_sender.send(()).expect("release refresh");
    let _ = wait_for_completion(&controller);
    let _ = wait_for_completion(&controller);
    snapshot_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("refresh snapshot observation");
    snapshot_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("rebound range snapshot observation");
    let rebound = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("rebound range snapshot");
    let same_preset = DesktopHistoryRangeIntent::new(
        epoch,
        rebound.generation(),
        DesktopHistoryRangeGeneration::new(2).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    assert_eq!(
        controller
            .request_history_range(same_preset)
            .expect_err("only a successful rebound advances the preset")
            .stable_code(),
        "stale_history_range"
    );
    controller.shutdown().expect("shutdown");
}

#[test]
fn refresh_supersedes_active_range_and_rolls_back_before_refresh_publication() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("history-range-refresh-supersession.sqlite3");
    seed(&path);
    let range_armed = Arc::new(AtomicBool::new(false));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller = DesktopController::spawn(
        BlockingDetailSource {
            inner: QueryService::open(&path, FixedClock).expect("query service"),
            entered: None,
            release: None,
            range_armed: Arc::clone(&range_armed),
            range_entered: Some(entered_sender),
            range_release: Some(release_receiver),
            drift_armed: Arc::new(AtomicBool::new(false)),
            drift: None,
            range_failure: Arc::new(AtomicBool::new(false)),
        },
        DesktopQueryPlan::overview().expect("plan"),
    )
    .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    let _ = wait_for_completion(&controller);
    let initial = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    let (terminal_sender, terminal_receiver) = sync_channel(1);
    controller
        .attach_terminal_history_range_notifier(Arc::new(RecordingHistoryTerminalNotifier {
            sender: terminal_sender,
        }))
        .expect("attach terminal notifier");
    range_armed.store(true, Ordering::Release);
    let intent = DesktopHistoryRangeIntent::new(
        epoch,
        initial.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    controller
        .request_history_range(intent)
        .expect("range admission");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("range query entered");
    assert!(matches!(
        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("refresh supersedes range"),
        tokenmaster_desktop::DesktopRefreshAdmission::Coalesced { .. }
    ));
    assert_eq!(
        terminal_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("superseded range rollback"),
        intent
    );
    assert!(controller.take_snapshot().expect("mailbox").is_none());
    release_sender.send(()).expect("release range");
    let _ = wait_for_completion(&controller);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while controller.take_snapshot().expect("mailbox").is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "refresh snapshot timed out"
        );
        thread::yield_now();
    }
    assert!(
        terminal_receiver.try_recv().is_err(),
        "supersession is idempotent"
    );
    controller.shutdown().expect("shutdown");
}

fn wait_for_completion(
    controller: &DesktopController,
) -> tokenmaster_desktop::DesktopRefreshCompletion {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(completion) = controller.try_completion().expect("worker healthy") {
            return completion;
        }
        assert!(std::time::Instant::now() < deadline, "completion timed out");
        thread::yield_now();
    }
}

fn wait_for_terminal_completion(
    controller: &DesktopController,
) -> tokenmaster_desktop::DesktopRefreshCompletion {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(completion) = controller.try_completion().expect("worker healthy")
            && !completion.follow_up_started()
        {
            return completion;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "terminal completion timed out"
        );
        thread::sleep(Duration::from_millis(1));
    }
}
