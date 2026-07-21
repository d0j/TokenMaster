use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

use rusqlite::{Connection, params};
use tokenmaster_desktop::{
    DesktopController, DesktopQueryPlan, DesktopQuerySource, DesktopRefreshAdmission,
    DesktopRefreshOutcome, DesktopRefreshUrgency, DesktopSessionNavigationGeneration,
    DesktopSessionPageDirection, DesktopSessionPageIntent, DesktopSnapshotEpoch,
    DesktopTerminalNavigationNotifier,
};
use tokenmaster_product::{
    ProductGeneration, ProductSectionKind, ProductSessionDetailSelection,
    ProductSessionDetailSelectionGeneration,
};
use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewRequest, BenefitOverviewSnapshot, GitEnvelope,
    GitOutputRequest, GitOutputSnapshot, LatestActivityPage, LatestActivityRequest, PageSize,
    ProductDataStatusEnvelope, QueryClock, QueryEnvelope, QueryError, QueryErrorCode, QueryService,
    QueryTimeSample, QuotaCurrentSnapshot, QuotaEnvelope, UsageAnalytics, UsageAnalyticsRequest,
    UsageSessionDetailResult, UsageSessionKey, UsageSessionPage, UsageSessionPageRequest,
};
use tokenmaster_store::UsageStore;

const SOURCE_KEY: [u8; 32] = [71; 32];

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

struct PageGate {
    block: AtomicBool,
    entered: Mutex<Option<SyncSender<()>>>,
    release: Mutex<Option<Receiver<()>>>,
}

struct RecordingTerminalNavigationNotifier {
    sender: SyncSender<DesktopSessionPageIntent>,
}

impl DesktopTerminalNavigationNotifier for RecordingTerminalNavigationNotifier {
    fn navigation_terminal(&self, intent: DesktopSessionPageIntent) {
        let _ = self.sender.try_send(intent);
    }
}

impl PageGate {
    fn open() -> Self {
        Self {
            block: AtomicBool::new(false),
            entered: Mutex::new(None),
            release: Mutex::new(None),
        }
    }

    fn block_next(&self, entered: SyncSender<()>, release: Receiver<()>) {
        *self.entered.lock().expect("entered gate") = Some(entered);
        *self.release.lock().expect("release gate") = Some(release);
        self.block.store(true, Ordering::Release);
    }
}

struct RecordingSource {
    inner: QueryService<FixedClock>,
    page_continuations: Arc<Mutex<Vec<bool>>>,
    page_calls: Arc<AtomicUsize>,
    fail_pages: Arc<AtomicBool>,
    gate: Arc<PageGate>,
}

impl DesktopQuerySource for RecordingSource {
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
        self.page_calls.fetch_add(1, Ordering::AcqRel);
        self.page_continuations
            .lock()
            .expect("page calls")
            .push(request.is_continuation());
        if self.gate.block.swap(false, Ordering::AcqRel) {
            self.gate
                .entered
                .lock()
                .expect("entered gate")
                .take()
                .expect("page entered sender")
                .send(())
                .expect("page entered");
            self.gate
                .release
                .lock()
                .expect("release gate")
                .take()
                .expect("page release receiver")
                .recv()
                .expect("page release");
        }
        if self.fail_pages.load(Ordering::Acquire) {
            return Err(PageSize::new(0).expect_err("zero page size is invalid"));
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

fn seed_sessions(path: &Path, count: usize) {
    drop(UsageStore::open(path).expect("create archive"));
    let mut connection = Connection::open(path).expect("fixture connection");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'private-pagination-source', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [72_u8; 32].as_slice(),
                [73_u8; 32].as_slice()
            ],
        )
        .expect("source");
    transaction.execute("INSERT INTO usage_scan_set(scan_set_id, started_at_ms, completed_at_ms, completion_state, expected_scope_count) VALUES (1, 1000, 2000, 'complete', 1)", []).expect("scan set");
    transaction.execute("INSERT INTO usage_scan(scan_id, scan_set_id, provider_id, profile_id, started_at_ms, completed_at_ms, completion_state) VALUES (1, 1, 'codex', 'default', 1000, 2000, 'complete')", []).expect("scan");
    transaction.execute("INSERT INTO usage_replay_revision(revision_id, status, canonicalizer_version, fingerprint_version, replay_signature_version, expected_source_count, evidence_epoch, sealed, promoted, scan_set_id) VALUES (0, 'current', 1, 2, 1, 1, 1, 1, 1, 1)", []).expect("revision");
    transaction.execute("UPDATE usage_archive_state SET archive_generation = 4, current_revision_id = 0, latest_complete_scan_set_id = 1, incremental_state = 'complete' WHERE singleton_id = 1", []).expect("publication");
    for index in 0..count {
        let index = i64::try_from(index).expect("bounded fixture index");
        transaction
            .execute(
                "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', ?5,
               'private-pagination-source', ?6, 0, 'gpt-5.6', ?7, NULL, 1,
               NULL, ?8, 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0)",
                params![
                    [u8::try_from(index + 1).expect("bounded byte"); 32].as_slice(),
                    format!("event-{index}"),
                    SOURCE_KEY.as_slice(),
                    index,
                    format!("private-pagination-{index}"),
                    1_000 + index,
                    10 + index,
                    11 + index,
                ],
            )
            .expect("event");
    }
    transaction.commit().expect("commit fixture");
}

fn source(
    path: &Path,
    page_continuations: Arc<Mutex<Vec<bool>>>,
    page_calls: Arc<AtomicUsize>,
    fail_pages: Arc<AtomicBool>,
    gate: Arc<PageGate>,
) -> RecordingSource {
    RecordingSource {
        inner: QueryService::open(path, FixedClock).expect("query service"),
        page_continuations,
        page_calls,
        fail_pages,
        gate,
    }
}

fn initial_controller(
    source: RecordingSource,
) -> (
    DesktopController,
    DesktopSnapshotEpoch,
    Arc<tokenmaster_product::ProductSnapshot>,
) {
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut controller =
        DesktopController::spawn(source, DesktopQueryPlan::overview().expect("overview plan"))
            .expect("controller");
    controller.bind_snapshot_epoch(epoch).expect("bind epoch");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let snapshot = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("initial snapshot");
    assert_eq!(snapshot.sessions().kind(), ProductSectionKind::Ready);
    (controller, epoch, snapshot)
}

fn intent(
    epoch: DesktopSnapshotEpoch,
    product_generation: ProductGeneration,
    generation: u64,
    direction: DesktopSessionPageDirection,
) -> DesktopSessionPageIntent {
    DesktopSessionPageIntent::new(
        epoch,
        product_generation,
        DesktopSessionNavigationGeneration::new(generation).expect("navigation generation"),
        direction,
    )
}

fn selection(generation: u64, ordinal: u8) -> ProductSessionDetailSelection {
    ProductSessionDetailSelection::new(
        ProductSessionDetailSelectionGeneration::new(generation).expect("selection generation"),
        ordinal,
    )
}

fn wait_for_completion(
    controller: &DesktopController,
) -> tokenmaster_desktop::DesktopRefreshCompletion {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(completion) = controller.try_completion().expect("worker healthy") {
            return completion;
        }
        assert!(Instant::now() < deadline, "completion timed out");
        thread::yield_now();
    }
}

fn wait_for_snapshot(controller: &DesktopController) -> Arc<tokenmaster_product::ProductSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(snapshot) = controller.take_snapshot().expect("mailbox") {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "snapshot timed out");
        thread::yield_now();
    }
}

#[test]
fn newest_and_next_pages_are_worker_resolved_and_page_success_clears_detail() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("desktop-session-pagination.sqlite3");
    seed_sessions(&path, 65);
    let page_continuations = Arc::new(Mutex::new(Vec::new()));
    let page_calls = Arc::new(AtomicUsize::new(0));
    let fail_pages = Arc::new(AtomicBool::new(false));
    let gate = Arc::new(PageGate::open());
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        page_continuations.clone(),
        page_calls,
        fail_pages,
        gate,
    ));
    controller
        .request_session_detail(tokenmaster_desktop::DesktopSessionDetailIntent::new(
            epoch,
            initial.generation(),
            selection(1, 0),
        ))
        .expect("detail request");
    let _ = wait_for_completion(&controller);
    let detailed = wait_for_snapshot(&controller);
    assert_eq!(detailed.session_detail().kind(), ProductSectionKind::Ready);

    controller
        .request_session_page(intent(
            epoch,
            detailed.generation(),
            1,
            DesktopSessionPageDirection::Next,
        ))
        .expect("next page request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let next = wait_for_snapshot(&controller);
    assert_eq!(
        next.sessions()
            .payload()
            .expect("sessions")
            .payload()
            .sessions()
            .len(),
        1
    );
    assert_eq!(next.session_detail().kind(), ProductSectionKind::Waiting);

    controller
        .request_session_page(intent(
            epoch,
            next.generation(),
            2,
            DesktopSessionPageDirection::Newest,
        ))
        .expect("newest page request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let newest = wait_for_snapshot(&controller);
    assert_eq!(
        newest
            .sessions()
            .payload()
            .expect("sessions")
            .payload()
            .sessions()
            .len(),
        64
    );
    assert_eq!(
        page_continuations.lock().expect("page calls").as_slice(),
        [false, true, false]
    );
    let error = controller
        .request_session_page(intent(
            epoch,
            newest.generation(),
            1,
            DesktopSessionPageDirection::Newest,
        ))
        .expect_err("completed navigation generation remains stale");
    assert_eq!(error.stable_code(), "stale_navigation");
    controller.shutdown().expect("controller stops");
}

#[test]
fn stale_published_product_generation_is_rejected_before_navigation_submission() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-pagination-stale.sqlite3");
    seed_sessions(&path, 65);
    let page_continuations = Arc::new(Mutex::new(Vec::new()));
    let page_calls = Arc::new(AtomicUsize::new(0));
    let fail_pages = Arc::new(AtomicBool::new(false));
    let gate = Arc::new(PageGate::open());
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        page_continuations,
        page_calls.clone(),
        fail_pages,
        gate,
    ));

    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("current product refresh");
    let _ = wait_for_completion(&controller);
    let current = wait_for_snapshot(&controller);
    assert_ne!(current.generation(), initial.generation());
    assert_eq!(
        controller
            .published_product_generation()
            .expect("published generation"),
        Some(current.generation())
    );

    let initial_calls = page_calls.load(Ordering::Acquire);
    let error = controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Newest,
        ))
        .expect_err("stale published product generation rejects synchronously");
    assert_eq!(error.stable_code(), "stale_navigation");
    assert_eq!(page_calls.load(Ordering::Acquire), initial_calls);
    assert!(controller.take_snapshot().expect("mailbox").is_none());
    assert_eq!(
        controller
            .published_product_generation()
            .expect("published generation after rejection"),
        Some(current.generation())
    );

    assert!(matches!(
        controller
            .request_session_page(intent(
                epoch,
                current.generation(),
                1,
                DesktopSessionPageDirection::Newest,
            ))
            .expect("current generation starts navigation"),
        DesktopRefreshAdmission::Started { .. }
    ));
    assert!(matches!(
        controller
            .request_session_page(intent(
                epoch,
                current.generation(),
                2,
                DesktopSessionPageDirection::Next,
            ))
            .expect("next current generation coalesces"),
        DesktopRefreshAdmission::Coalesced { .. }
    ));
    controller.shutdown().expect("controller stops");
}

#[test]
fn missing_continuation_stale_intents_and_page_failure_fail_closed_without_extra_query() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-pagination-failure.sqlite3");
    seed_sessions(&path, 1);
    let page_continuations = Arc::new(Mutex::new(Vec::new()));
    let page_calls = Arc::new(AtomicUsize::new(0));
    let fail_pages = Arc::new(AtomicBool::new(false));
    let gate = Arc::new(PageGate::open());
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        page_continuations,
        page_calls.clone(),
        fail_pages.clone(),
        gate,
    ));
    let initial_calls = page_calls.load(Ordering::Acquire);
    controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Next,
        ))
        .expect("missing continuation request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let missing = wait_for_snapshot(&controller);
    assert_eq!(missing.sessions().kind(), ProductSectionKind::Unavailable);
    assert_eq!(
        missing
            .sessions()
            .failure()
            .expect("missing cursor failure")
            .code(),
        QueryErrorCode::InvalidValue
    );
    assert_eq!(page_calls.load(Ordering::Acquire), initial_calls);

    let stale_epoch = DesktopSnapshotEpoch::new(2).expect("stale epoch");
    let error = controller
        .request_session_page(intent(
            stale_epoch,
            missing.generation(),
            2,
            DesktopSessionPageDirection::Newest,
        ))
        .expect_err("stale epoch rejects");
    assert_eq!(error.stable_code(), "stale_navigation");
    let error = controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            3,
            DesktopSessionPageDirection::Newest,
        ))
        .expect_err("stale product rejects before worker submission");
    assert_eq!(error.stable_code(), "stale_navigation");
    assert_eq!(page_calls.load(Ordering::Acquire), initial_calls);
    assert!(controller.take_snapshot().expect("mailbox").is_none());
    controller
        .request_session_page(intent(
            epoch,
            missing.generation(),
            3,
            DesktopSessionPageDirection::Newest,
        ))
        .expect("current product request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let current = wait_for_snapshot(&controller);

    fail_pages.store(true, Ordering::Release);
    controller
        .request_session_page(intent(
            epoch,
            current.generation(),
            4,
            DesktopSessionPageDirection::Newest,
        ))
        .expect("failure request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let failed = wait_for_snapshot(&controller);
    assert_eq!(failed.sessions().kind(), ProductSectionKind::Unavailable);
    assert_eq!(
        failed.sessions().failure().expect("query failure").code(),
        QueryErrorCode::InvalidValue
    );
    controller.shutdown().expect("controller stops");
}

#[test]
fn refresh_supersedes_navigation_and_cancellation_publishes_nothing() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-pagination-race.sqlite3");
    seed_sessions(&path, 65);
    let page_continuations = Arc::new(Mutex::new(Vec::new()));
    let page_calls = Arc::new(AtomicUsize::new(0));
    let fail_pages = Arc::new(AtomicBool::new(false));
    let gate = Arc::new(PageGate::open());
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        page_continuations,
        page_calls,
        fail_pages,
        gate.clone(),
    ));
    gate.block_next(entered_sender, release_receiver);
    controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Next,
        ))
        .expect("page starts");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("page entered");
    assert!(matches!(
        controller
            .request_session_page(intent(
                epoch,
                initial.generation(),
                2,
                DesktopSessionPageDirection::Newest,
            ))
            .expect("replacement page"),
        DesktopRefreshAdmission::Coalesced { .. }
    ));
    let error = controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Newest,
        ))
        .expect_err("stale navigation generation rejects");
    assert_eq!(error.stable_code(), "stale_navigation");
    assert!(matches!(
        controller
            .refresh(DesktopRefreshUrgency::Interactive)
            .expect("refresh follows page"),
        DesktopRefreshAdmission::Coalesced { .. }
    ));
    let error = controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Newest,
        ))
        .expect_err("refresh invalidation preserves navigation high-water");
    assert_eq!(error.stable_code(), "stale_navigation");
    release_sender.send(()).expect("release page");
    let snapshot = wait_for_snapshot(&controller);
    assert_eq!(
        snapshot
            .sessions()
            .payload()
            .expect("sessions")
            .payload()
            .sessions()
            .len(),
        64
    );

    let (cancel_entered_sender, cancel_entered_receiver) = sync_channel(1);
    let (cancel_release_sender, cancel_release_receiver) = sync_channel(1);
    controller.shutdown().expect("first controller stops");
    let gate = Arc::new(PageGate::open());
    let (mut cancelled, epoch, initial) = initial_controller(source(
        &path,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicBool::new(false)),
        gate.clone(),
    ));
    gate.block_next(cancel_entered_sender, cancel_release_receiver);
    let admission = cancelled
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Next,
        ))
        .expect("cancellable page");
    let attempt = match admission {
        DesktopRefreshAdmission::Started { attempt } => attempt,
        other => panic!("expected started page, got {other:?}"),
    };
    cancel_entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("page entered");
    cancelled.cancel(attempt).expect("cancel active page");
    cancel_release_sender
        .send(())
        .expect("release cancelled page");
    assert_eq!(
        wait_for_completion(&cancelled).outcome(),
        DesktopRefreshOutcome::Cancelled
    );
    assert!(cancelled.take_snapshot().expect("mailbox").is_none());
    let detail_admission =
        cancelled.request_session_detail(tokenmaster_desktop::DesktopSessionDetailIntent::new(
            epoch,
            initial.generation(),
            selection(1, 0),
        ));
    cancelled.shutdown().expect("cancelled controller stops");
    assert!(
        detail_admission.is_ok(),
        "completed cancellation releases navigation detail gate"
    );
}

#[test]
fn terminal_page_completion_rolls_back_only_the_matching_cancelled_navigation() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-terminal-cancel.sqlite3");
    seed_sessions(&path, 65);
    let gate = Arc::new(PageGate::open());
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicBool::new(false)),
        gate.clone(),
    ));
    let (terminal_sender, terminal_receiver) = sync_channel(1);
    controller
        .attach_terminal_navigation_notifier(Arc::new(RecordingTerminalNavigationNotifier {
            sender: terminal_sender,
        }))
        .expect("attach terminal notifier while idle");

    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    gate.block_next(entered_sender, release_receiver);
    let cancelled = intent(
        epoch,
        initial.generation(),
        1,
        DesktopSessionPageDirection::Next,
    );
    let admission = controller
        .request_session_page(cancelled)
        .expect("navigation starts");
    let attempt = match admission {
        DesktopRefreshAdmission::Started { attempt } => attempt,
        other => panic!("expected started page, got {other:?}"),
    };
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("page entered");
    controller.cancel(attempt).expect("cancel active page");
    release_sender.send(()).expect("release page");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Cancelled
    );
    assert_eq!(
        terminal_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("terminal rollback"),
        cancelled
    );
    assert!(controller.take_snapshot().expect("mailbox").is_none());
    assert!(
        terminal_receiver
            .recv_timeout(Duration::from_millis(50))
            .is_err(),
        "one terminal completion emits one rollback"
    );

    controller.shutdown().expect("controller stops");
}

#[test]
fn refresh_supersession_immediately_rolls_back_the_exact_navigation_without_a_snapshot() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-terminal-supersession.sqlite3");
    seed_sessions(&path, 65);
    let gate = Arc::new(PageGate::open());
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicBool::new(false)),
        gate.clone(),
    ));
    let (terminal_sender, terminal_receiver) = sync_channel(1);
    controller
        .attach_terminal_navigation_notifier(Arc::new(RecordingTerminalNavigationNotifier {
            sender: terminal_sender,
        }))
        .expect("attach terminal notifier while idle");

    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    gate.block_next(entered_sender, release_receiver);
    let navigation = intent(
        epoch,
        initial.generation(),
        1,
        DesktopSessionPageDirection::Next,
    );
    controller
        .request_session_page(navigation)
        .expect("navigation starts");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("page entered");
    assert!(matches!(
        controller
            .refresh(DesktopRefreshUrgency::Interactive)
            .expect("refresh supersedes navigation"),
        DesktopRefreshAdmission::Coalesced { .. }
    ));
    assert_eq!(
        terminal_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("immediate rollback"),
        navigation
    );
    release_sender.send(()).expect("release page");
    let _ = wait_for_completion(&controller);
    let refreshed = wait_for_snapshot(&controller);
    assert_eq!(refreshed.sessions().kind(), ProductSectionKind::Ready);
    assert!(
        terminal_receiver
            .recv_timeout(Duration::from_millis(50))
            .is_err(),
        "refresh publication must not duplicate the supersession rollback"
    );
    controller.shutdown().expect("controller stops");
}

#[test]
fn successful_and_query_error_page_snapshots_do_not_emit_terminal_rollback() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-terminal-snapshots.sqlite3");
    seed_sessions(&path, 65);
    let fail_pages = Arc::new(AtomicBool::new(false));
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(AtomicUsize::new(0)),
        fail_pages.clone(),
        Arc::new(PageGate::open()),
    ));
    let (terminal_sender, terminal_receiver) = sync_channel(1);
    controller
        .attach_terminal_navigation_notifier(Arc::new(RecordingTerminalNavigationNotifier {
            sender: terminal_sender,
        }))
        .expect("attach terminal notifier while idle");

    controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Next,
        ))
        .expect("successful page request");
    let _ = wait_for_completion(&controller);
    let successful = wait_for_snapshot(&controller);
    assert!(
        terminal_receiver
            .recv_timeout(Duration::from_millis(50))
            .is_err(),
        "published page success remains snapshot-authoritative"
    );

    fail_pages.store(true, Ordering::Release);
    controller
        .request_session_page(intent(
            epoch,
            successful.generation(),
            2,
            DesktopSessionPageDirection::Newest,
        ))
        .expect("query-error page request");
    let _ = wait_for_completion(&controller);
    let failed = wait_for_snapshot(&controller);
    assert_eq!(failed.sessions().kind(), ProductSectionKind::Unavailable);
    assert!(
        terminal_receiver
            .recv_timeout(Duration::from_millis(50))
            .is_err(),
        "published query error remains snapshot-authoritative"
    );
    controller.shutdown().expect("controller stops");
}

#[test]
fn ten_thousand_navigation_requests_retain_one_latest_intent_and_execute_only_it() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-pagination-stress.sqlite3");
    seed_sessions(&path, 65);
    let page_continuations = Arc::new(Mutex::new(Vec::new()));
    let page_calls = Arc::new(AtomicUsize::new(0));
    let fail_pages = Arc::new(AtomicBool::new(false));
    let gate = Arc::new(PageGate::open());
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        page_continuations.clone(),
        page_calls.clone(),
        fail_pages,
        gate.clone(),
    ));
    gate.block_next(entered_sender, release_receiver);
    controller
        .request_session_page(intent(
            epoch,
            initial.generation(),
            1,
            DesktopSessionPageDirection::Next,
        ))
        .expect("first page");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("first page entered");
    for generation in 2..=10_000 {
        assert!(matches!(
            controller
                .request_session_page(intent(
                    epoch,
                    initial.generation(),
                    generation,
                    DesktopSessionPageDirection::Newest,
                ))
                .expect("latest page request"),
            DesktopRefreshAdmission::Coalesced { .. }
        ));
    }
    release_sender.send(()).expect("release first page");
    let latest = wait_for_snapshot(&controller);
    assert_eq!(
        latest
            .sessions()
            .payload()
            .expect("sessions")
            .payload()
            .sessions()
            .len(),
        64
    );
    assert_eq!(page_calls.load(Ordering::Acquire), 3);
    assert_eq!(
        page_continuations.lock().expect("page calls").as_slice(),
        [false, true, false]
    );
    controller.shutdown().expect("controller stops");
}
