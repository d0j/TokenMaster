mod support;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopDashboardSectionState, DesktopFreshness, DesktopQuality, DesktopRouteKey,
    DesktopSessionBreakdownKind, DesktopSessionDetailState, DesktopSessionPageDirection,
    DesktopSessionPageKind, DesktopSnapshotEpoch, DesktopState, MAX_SESSION_DETAIL_MODEL_ROWS,
    MAX_SESSION_DETAIL_PROJECT_ROWS, MAX_SESSION_ROWS,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{PageSize, QueryErrorCode, QueryService, UsageSessionPageRequest};

use support::dashboard_fixture::{
    DAY_START_SECONDS, FixedClock, add_distinct_usage_rows, add_session_breakdown_rows, seed,
};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

#[test]
fn initial_sessions_are_bounded_waiting_truth_without_fabricated_completion() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Sessions);
    let sessions = state.projection().sessions();

    assert_eq!(MAX_SESSION_ROWS, 64);
    assert_eq!(sessions.state(), DesktopDashboardSectionState::Waiting);
    assert_eq!(sessions.rows().len(), 0);
    assert_eq!(sessions.has_more(), None);
    assert_eq!(sessions.freshness(), None);
    assert_eq!(sessions.quality(), None);
}

#[test]
fn ready_sessions_map_one_newest_first_page_and_preserve_has_more() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("sessions-projection.sqlite3");
    seed(&path);
    add_distinct_usage_rows(&path, 64);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let sessions = service
        .usage_sessions(
            UsageSessionPageRequest::first(
                PageSize::new(MAX_SESSION_ROWS).expect("page size"),
                Vec::new(),
            )
            .expect("session request"),
        )
        .expect("sessions");
    assert_eq!(sessions.payload().sessions().len(), MAX_SESSION_ROWS);
    assert!(sessions.payload().has_more());

    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_sessions(attempt(1), sessions)
        .expect("publish sessions");
    let snapshot = reducer.snapshot();
    let state = DesktopState::new(&snapshot, DesktopRouteKey::Sessions);
    let sessions = state.projection().sessions();

    assert_eq!(sessions.state(), DesktopDashboardSectionState::Ready);
    assert_eq!(sessions.freshness(), Some(DesktopFreshness::Fresh));
    assert_eq!(sessions.quality(), Some(DesktopQuality::Authoritative));
    assert_eq!(sessions.rows().len(), MAX_SESSION_ROWS);
    assert_eq!(sessions.has_more(), Some(true));
    let newest = &sessions.rows()[0];
    assert_eq!(newest.last_timestamp_seconds(), DAY_START_SECONDS + 4_063);
    assert_eq!(newest.event_count(), 1);
    assert_eq!(newest.total_tokens().known_sum(), Some(2));
    assert_eq!(newest.cost().micros(), Some(1));
    assert!(
        sessions
            .rows()
            .windows(2)
            .all(|pair| { pair[0].last_timestamp_seconds() >= pair[1].last_timestamp_seconds() })
    );
}

#[test]
fn session_navigation_projects_page_kind_and_rejects_only_its_pending_handoff() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("sessions-navigation.sqlite3");
    seed(&path);
    add_distinct_usage_rows(&path, 2);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let newest = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(1).expect("page"), Vec::new())
                .expect("newest request"),
        )
        .expect("newest page");
    let continuation = service
        .usage_sessions(
            UsageSessionPageRequest::continuation(
                PageSize::new(1).expect("page"),
                newest
                    .payload()
                    .next_cursor()
                    .expect("continuation cursor")
                    .clone(),
                Vec::new(),
            )
            .expect("continuation request"),
        )
        .expect("continuation page");

    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_sessions(attempt(1), newest)
        .expect("publish newest");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Sessions);
    assert_eq!(
        state.apply_snapshot_for_epoch(epoch, &reducer.snapshot()),
        tokenmaster_desktop::DesktopApplyOutcome::Accepted
    );
    assert_eq!(
        state.projection().sessions().page_kind(),
        Some(DesktopSessionPageKind::Newest)
    );
    let debug = format!("{:?}", state.projection().sessions());
    assert!(!debug.contains("cursor"));
    assert!(!debug.contains("UsageSessionKey"));
    assert!(!state.projection().sessions().navigation_pending());

    state.select_session_row(0).expect("select visible row");
    assert_eq!(
        state.projection().sessions().detail().state(),
        DesktopSessionDetailState::Loading
    );
    let next = state
        .request_session_page(DesktopSessionPageDirection::Next)
        .expect("next intent");
    assert_eq!(next.direction(), DesktopSessionPageDirection::Next);
    assert!(state.projection().sessions().navigation_pending());
    assert_eq!(
        state.projection().sessions().detail().state(),
        DesktopSessionDetailState::Idle
    );
    state.reject_session_page(next);
    assert!(!state.projection().sessions().navigation_pending());

    reducer
        .publish_sessions(attempt(2), continuation)
        .expect("publish continuation");
    assert_eq!(
        state.apply_snapshot_for_epoch(epoch, &reducer.snapshot()),
        tokenmaster_desktop::DesktopApplyOutcome::Accepted
    );
    assert_eq!(
        state.projection().sessions().page_kind(),
        Some(DesktopSessionPageKind::Continuation)
    );
    assert!(!state.projection().sessions().navigation_pending());
    let newest = state
        .request_session_page(DesktopSessionPageDirection::Newest)
        .expect("newest intent from continuation");
    assert_eq!(newest.direction(), DesktopSessionPageDirection::Newest);
    let replacement_epoch = DesktopSnapshotEpoch::new(2).expect("replacement epoch");
    assert_eq!(
        state.apply_snapshot_for_epoch(replacement_epoch, &reducer.snapshot()),
        tokenmaster_desktop::DesktopApplyOutcome::Accepted
    );
    assert!(!state.projection().sessions().navigation_pending());
    let after_replacement = state
        .request_session_page(DesktopSessionPageDirection::Newest)
        .expect("newest intent after replacement");
    assert!(
        after_replacement.navigation_generation().get() > newest.navigation_generation().get(),
        "backend epoch replacement must not reset local navigation generation"
    );
}

#[test]
fn session_navigation_requires_epoch_and_retained_recoverable_page() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let mut state = DesktopState::new(&snapshot, DesktopRouteKey::Sessions);
    assert!(
        state
            .request_session_page(DesktopSessionPageDirection::Next)
            .is_err()
    );
    assert_eq!(
        state.projection().sessions().page_kind(),
        None,
        "unavailable sessions never invent a page kind"
    );
    assert!(!state.projection().sessions().navigation_pending());
}

#[test]
fn exact_detail_projects_idle_loading_ready_missing_and_unavailable_with_fixed_caps() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("session-detail-projection.sqlite3");
    seed(&path);
    add_session_breakdown_rows(&path, 40);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let sessions = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(1).expect("page"), Vec::new())
                .expect("request"),
        )
        .expect("sessions");
    let key = sessions.payload().sessions()[0].key().clone();
    let detail = service
        .usage_session_detail(key.clone())
        .expect("session detail");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_sessions(attempt(1), sessions)
        .expect("publish sessions");
    let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
    let mut state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Sessions);
    assert_eq!(
        state.apply_snapshot_for_epoch(epoch, &reducer.snapshot()),
        tokenmaster_desktop::DesktopApplyOutcome::Accepted
    );
    assert_eq!(
        state.projection().sessions().detail().state(),
        DesktopSessionDetailState::Idle
    );

    let first_intent = state
        .select_session_row(0)
        .expect("select exact visible row");
    let loading = state.projection().sessions().detail();
    assert_eq!(loading.state(), DesktopSessionDetailState::Loading);
    assert_eq!(loading.selected_ordinal(), Some(0));
    assert!(loading.summary().is_none());
    assert!(loading.breakdown_rows().is_empty());

    reducer
        .publish_session_detail(attempt(2), first_intent.selection(), detail)
        .expect("publish detail");
    assert_eq!(
        state.apply_snapshot_for_epoch(epoch, &reducer.snapshot()),
        tokenmaster_desktop::DesktopApplyOutcome::Accepted
    );
    let ready = state.projection().sessions().detail();
    assert_eq!(ready.state(), DesktopSessionDetailState::Ready);
    assert_eq!(ready.selected_ordinal(), Some(0));
    assert_eq!(ready.freshness(), Some(DesktopFreshness::Fresh));
    assert_eq!(ready.quality(), Some(DesktopQuality::Authoritative));
    assert_eq!(ready.summary().expect("summary").event_count(), 41);
    assert_eq!(
        ready.breakdown_rows().len(),
        MAX_SESSION_DETAIL_MODEL_ROWS + MAX_SESSION_DETAIL_PROJECT_ROWS
    );
    assert_eq!(
        ready
            .breakdown_rows()
            .iter()
            .filter(|row| row.kind() == DesktopSessionBreakdownKind::Model)
            .count(),
        MAX_SESSION_DETAIL_MODEL_ROWS
    );
    assert_eq!(
        ready
            .breakdown_rows()
            .iter()
            .filter(|row| row.kind() == DesktopSessionBreakdownKind::Project)
            .count(),
        MAX_SESSION_DETAIL_PROJECT_ROWS
    );
    assert!(ready.truncated());
    assert!(ready.breakdown_rows().iter().all(|row| {
        !row.label().contains('/')
            && !row.label().contains('\\')
            && !row.label().contains("private")
    }));

    let failed_intent = state.select_session_row(0).expect("reselect row");
    assert_eq!(
        state.projection().sessions().detail().state(),
        DesktopSessionDetailState::Loading
    );
    reducer
        .fail_session_detail(
            attempt(3),
            failed_intent.selection(),
            QueryErrorCode::DeadlineExceeded,
        )
        .expect("fail detail");
    state.apply_snapshot_for_epoch(epoch, &reducer.snapshot());
    let unavailable = state.projection().sessions().detail();
    assert_eq!(unavailable.state(), DesktopSessionDetailState::Unavailable);
    assert!(unavailable.summary().is_none());
    assert!(unavailable.breakdown_rows().is_empty());

    let connection = rusqlite::Connection::open(&path).expect("missing detail connection");
    connection
        .execute("DELETE FROM usage_session_rollup", [])
        .expect("remove session rollup");
    drop(connection);
    let missing = service
        .usage_session_detail(key)
        .expect("typed missing detail");
    let missing_intent = state.select_session_row(0).expect("select missing row");
    reducer
        .publish_session_detail(attempt(4), missing_intent.selection(), missing)
        .expect("publish missing detail");
    state.apply_snapshot_for_epoch(epoch, &reducer.snapshot());
    let missing = state.projection().sessions().detail();
    assert_eq!(missing.state(), DesktopSessionDetailState::Missing);
    assert!(missing.summary().is_none());
    assert!(missing.breakdown_rows().is_empty());

    let replacement_epoch = DesktopSnapshotEpoch::new(2).expect("replacement epoch");
    assert_eq!(
        state.apply_snapshot_for_epoch(replacement_epoch, &reducer.snapshot()),
        tokenmaster_desktop::DesktopApplyOutcome::Accepted
    );
    let replaced = state.projection().sessions().detail();
    assert_eq!(replaced.state(), DesktopSessionDetailState::Idle);
    assert_eq!(replaced.selected_ordinal(), None);
}
