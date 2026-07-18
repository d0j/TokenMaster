use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

use rusqlite::{Connection, params};
use tokenmaster_desktop::{
    DesktopAttempt, DesktopController, DesktopQueryPlan, DesktopQuerySource,
    DesktopRefreshAdmission, DesktopRefreshOutcome, DesktopRefreshUrgency,
    DesktopSessionDetailIntent, DesktopSnapshotEpoch,
};
use tokenmaster_product::{
    ProductGeneration, ProductSectionKind, ProductSessionDetailSelection,
    ProductSessionDetailSelectionGeneration,
};
use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewRequest, BenefitOverviewSnapshot, GitEnvelope,
    GitOutputRequest, GitOutputSnapshot, LatestActivityPage, LatestActivityRequest, PageSize,
    ProductDataStatusEnvelope, QueryClock, QueryEnvelope, QueryError, QueryService,
    QueryTimeSample, QuotaCurrentSnapshot, QuotaEnvelope, UsageAnalytics, UsageAnalyticsRequest,
    UsageSessionDetailResult, UsageSessionKey, UsageSessionPage, UsageSessionPageRequest,
};
use tokenmaster_store::UsageStore;

const SOURCE_KEY: [u8; 32] = [27; 32];

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

struct RecordingSource {
    inner: QueryService<FixedClock>,
    detail_calls: Arc<Mutex<Vec<UsageSessionKey>>>,
    status_calls: Arc<AtomicUsize>,
    detail_entered: Option<SyncSender<()>>,
    detail_release: Option<Receiver<()>>,
}

impl DesktopQuerySource for RecordingSource {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        self.status_calls.fetch_add(1, Ordering::AcqRel);
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
        self.detail_calls
            .lock()
            .expect("detail calls")
            .push(key.clone());
        if let Some(entered) = self.detail_entered.take() {
            entered.send(()).expect("detail entered signal");
            self.detail_release
                .take()
                .expect("detail release receiver")
                .recv()
                .expect("detail release");
        }
        self.inner.usage_session_detail(key)
    }
}

fn seed_two_sessions(path: &Path) {
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
             ) VALUES (?1, 'codex', 'default', 'private-controller-source', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [28_u8; 32].as_slice(),
                [29_u8; 32].as_slice()
            ],
        )
        .expect("source");
    transaction
        .execute(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completed_at_ms, completion_state,
               expected_scope_count
             ) VALUES (1, 1000, 2000, 'complete', 1)",
            [],
        )
        .expect("scan set");
    transaction
        .execute(
            "INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1000, 2000, 'complete')",
            [],
        )
        .expect("scan");
    transaction
        .execute(
            "INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 1, 1, 1, 1, 1)",
            [],
        )
        .expect("revision");
    transaction
        .execute(
            "UPDATE usage_archive_state
             SET archive_generation = 4, current_revision_id = 0,
                 latest_complete_scan_set_id = 1, incremental_state = 'complete'
             WHERE singleton_id = 1",
            [],
        )
        .expect("publication");
    for (index, session_id) in ["private-controller-a", "private-controller-b"]
        .into_iter()
        .enumerate()
    {
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
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', ?5,
                   'private-controller-source', ?6, 0, 'gpt-5.6', ?7, NULL, 1,
                   NULL, ?8, 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    [u8::try_from(index + 1).expect("bounded byte"); 32].as_slice(),
                    format!("event-{index}"),
                    SOURCE_KEY.as_slice(),
                    index,
                    session_id,
                    1_000 + index,
                    10 + index,
                    11 + index,
                ],
            )
            .expect("event");
    }
    transaction.commit().expect("commit fixture");
}

fn selection(generation: u64, ordinal: u8) -> ProductSessionDetailSelection {
    ProductSessionDetailSelection::new(
        ProductSessionDetailSelectionGeneration::new(generation)
            .expect("nonzero selection generation"),
        ordinal,
    )
}

fn intent(
    epoch: DesktopSnapshotEpoch,
    product_generation: ProductGeneration,
    selection: ProductSessionDetailSelection,
) -> DesktopSessionDetailIntent {
    DesktopSessionDetailIntent::new(epoch, product_generation, selection)
}

fn started_attempt(admission: DesktopRefreshAdmission) -> DesktopAttempt {
    match admission {
        DesktopRefreshAdmission::Started { attempt } => attempt,
        other => panic!("expected started admission, got {other:?}"),
    }
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

fn source(
    path: &Path,
    detail_calls: Arc<Mutex<Vec<UsageSessionKey>>>,
    status_calls: Arc<AtomicUsize>,
    detail_entered: Option<SyncSender<()>>,
    detail_release: Option<Receiver<()>>,
) -> RecordingSource {
    RecordingSource {
        inner: QueryService::open(path, FixedClock).expect("query service"),
        detail_calls,
        status_calls,
        detail_entered,
        detail_release,
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
    controller
        .bind_snapshot_epoch(epoch)
        .expect("bind snapshot epoch");
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
    assert_eq!(
        snapshot
            .sessions()
            .payload()
            .expect("sessions")
            .payload()
            .sessions()
            .len(),
        2
    );
    (controller, epoch, snapshot)
}

#[test]
fn exact_ordinal_resolves_only_inside_worker_and_stale_or_missing_rows_never_query() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("desktop-session-detail.sqlite3");
    seed_two_sessions(&path);
    let mut inspector = QueryService::open(&path, FixedClock).expect("inspector");
    let inspected = inspector
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(2).expect("page"), Vec::new())
                .expect("request"),
        )
        .expect("inspected page");
    let expected_second = inspected.payload().sessions()[1].key().clone();
    let detail_calls = Arc::new(Mutex::new(Vec::new()));
    let status_calls = Arc::new(AtomicUsize::new(0));
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        detail_calls.clone(),
        status_calls,
        None,
        None,
    ));

    controller
        .request_session_detail(intent(epoch, initial.generation(), selection(1, 1)))
        .expect("detail request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let detail = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("detail snapshot");
    assert_eq!(detail.session_detail_selection(), Some(selection(1, 1)));
    assert_eq!(detail.session_detail().kind(), ProductSectionKind::Ready);
    assert_eq!(
        detail_calls.lock().expect("detail calls").as_slice(),
        [expected_second]
    );

    let connection = Connection::open(&path).expect("missing detail connection");
    connection
        .execute("DELETE FROM usage_session_rollup", [])
        .expect("remove session rollups");
    drop(connection);
    controller
        .request_session_detail(intent(epoch, detail.generation(), selection(2, 0)))
        .expect("typed missing detail request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let missing_detail = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("missing detail snapshot");
    assert_eq!(
        missing_detail.session_detail().kind(),
        ProductSectionKind::Ready
    );
    assert!(
        missing_detail
            .session_detail()
            .payload()
            .expect("typed missing payload")
            .payload()
            .detail()
            .is_none()
    );
    assert_eq!(detail_calls.lock().expect("detail calls").len(), 2);

    controller
        .request_session_detail(intent(epoch, missing_detail.generation(), selection(3, 63)))
        .expect("missing row request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let missing_row = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("missing row snapshot");
    assert_eq!(
        missing_row.session_detail().kind(),
        ProductSectionKind::Unavailable
    );
    assert_eq!(detail_calls.lock().expect("detail calls").len(), 2);

    controller
        .request_session_detail(intent(epoch, initial.generation(), selection(4, 0)))
        .expect("stale product request admitted for worker validation");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    assert!(controller.take_snapshot().expect("mailbox").is_none());
    assert_eq!(detail_calls.lock().expect("detail calls").len(), 2);

    let wrong_epoch = DesktopSnapshotEpoch::new(2).expect("wrong epoch");
    let error = controller
        .request_session_detail(intent(
            wrong_epoch,
            missing_row.generation(),
            selection(5, 0),
        ))
        .expect_err("wrong backend epoch must fail closed");
    assert_eq!(error.stable_code(), "stale_selection");
    controller.shutdown().expect("controller stops");
}

#[test]
fn query_failure_clears_the_exact_selection_without_retaining_old_detail() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("desktop-session-detail-failure.sqlite3");
    seed_two_sessions(&path);
    let detail_calls = Arc::new(Mutex::new(Vec::new()));
    let status_calls = Arc::new(AtomicUsize::new(0));
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        detail_calls.clone(),
        status_calls,
        None,
        None,
    ));
    controller
        .request_session_detail(intent(epoch, initial.generation(), selection(1, 0)))
        .expect("first detail request");
    let _ = wait_for_completion(&controller);
    let ready = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("ready detail");
    assert_eq!(ready.session_detail().kind(), ProductSectionKind::Ready);

    let connection = Connection::open(&path).expect("dataset mutation connection");
    connection
        .execute(
            "UPDATE usage_event SET model = 'gpt-5.7' WHERE event_id = 'event-0'",
            [],
        )
        .expect("mutate session dataset");
    drop(connection);
    controller
        .request_session_detail(intent(epoch, ready.generation(), selection(2, 1)))
        .expect("stale key request");
    assert_eq!(
        wait_for_completion(&controller).outcome(),
        DesktopRefreshOutcome::Completed
    );
    let failed = controller
        .take_snapshot()
        .expect("mailbox")
        .expect("failed detail");
    assert_eq!(
        failed.session_detail().kind(),
        ProductSectionKind::Unavailable
    );
    assert!(!failed.session_detail().retains_payload());
    assert!(failed.session_detail().payload().is_none());
    assert_eq!(detail_calls.lock().expect("detail calls").len(), 2);
    controller.shutdown().expect("controller stops");
}

#[test]
fn rapid_selection_is_latest_wins_and_cancellation_publishes_nothing() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("desktop-session-race.sqlite3");
    seed_two_sessions(&path);
    let detail_calls = Arc::new(Mutex::new(Vec::new()));
    let status_calls = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        detail_calls.clone(),
        status_calls.clone(),
        Some(entered_sender),
        Some(release_receiver),
    ));

    let first_attempt = started_attempt(
        controller
            .request_session_detail(intent(epoch, initial.generation(), selection(1, 0)))
            .expect("first detail request"),
    );
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("first detail entered");
    assert!(matches!(
        controller
            .request_session_detail(intent(epoch, initial.generation(), selection(2, 1)))
            .expect("replacement detail request"),
        DesktopRefreshAdmission::Coalesced { .. }
    ));
    release_sender.send(()).expect("release first detail");

    let deadline = Instant::now() + Duration::from_secs(3);
    let latest = loop {
        if let Some(snapshot) = controller.take_snapshot().expect("mailbox")
            && snapshot.session_detail_selection() == Some(selection(2, 1))
        {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "latest detail timed out");
        thread::yield_now();
    };
    assert_eq!(latest.session_detail().kind(), ProductSectionKind::Ready);
    assert_eq!(detail_calls.lock().expect("detail calls").len(), 2);
    assert_ne!(first_attempt.get(), 0);

    let (cancel_entered_sender, cancel_entered_receiver) = sync_channel(1);
    let (cancel_release_sender, cancel_release_receiver) = sync_channel(1);
    controller.shutdown().expect("first controller stops");
    let (mut cancelled, epoch, initial) = initial_controller(source(
        &path,
        Arc::new(Mutex::new(Vec::new())),
        status_calls,
        Some(cancel_entered_sender),
        Some(cancel_release_receiver),
    ));
    let cancelled_attempt = started_attempt(
        cancelled
            .request_session_detail(intent(epoch, initial.generation(), selection(1, 0)))
            .expect("cancellable request"),
    );
    cancel_entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("cancellable detail entered");
    cancelled
        .cancel(cancelled_attempt)
        .expect("cancel active detail");
    cancel_release_sender
        .send(())
        .expect("release cancelled detail");
    assert_eq!(
        wait_for_completion(&cancelled).outcome(),
        DesktopRefreshOutcome::Cancelled
    );
    assert!(cancelled.take_snapshot().expect("mailbox").is_none());
    cancelled.shutdown().expect("cancelled controller stops");
}

#[test]
fn detail_and_refresh_share_one_worker_without_losing_the_follow_up_refresh() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let path = directory.path().join("desktop-session-refresh.sqlite3");
    seed_two_sessions(&path);
    let status_calls = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let (mut controller, epoch, initial) = initial_controller(source(
        &path,
        Arc::new(Mutex::new(Vec::new())),
        status_calls.clone(),
        Some(entered_sender),
        Some(release_receiver),
    ));
    controller
        .request_session_detail(intent(epoch, initial.generation(), selection(1, 0)))
        .expect("detail starts");
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("detail entered");
    assert!(matches!(
        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("refresh follows detail"),
        DesktopRefreshAdmission::Coalesced { .. }
    ));
    release_sender.send(()).expect("release detail");

    let deadline = Instant::now() + Duration::from_secs(3);
    while status_calls.load(Ordering::Acquire) < 2 {
        assert!(Instant::now() < deadline, "follow-up refresh timed out");
        thread::yield_now();
    }
    controller.shutdown().expect("controller stops");
}
