use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_domain::{TokenCount, UsageProfileId, UsageProviderId};
use tokenmaster_query::{
    AggregateTokenValue, CalendarDate, CostAvailability, DatasetGeneration, DatasetIdentity,
    LatestActivityRequest, PageSize, QUERY_FRESH_MAX_AGE_MS, QUERY_STALE_MIN_AGE_MS, QueryClock,
    QueryError, QueryErrorCode, QueryFreshness, QueryQuality, QueryService, QueryTimeSample,
    QueryWarningCode, UsageAnalyticsRequest, UsageBreakdownIdentity, UsageBreakdownKind,
    UsageRange, UsageSeriesSelection, UsageSessionPageRequest, UsageTimeZone, WeekStart,
};
use tokenmaster_store::UsageStore;

const SOURCE_KEY: [u8; 32] = [7; 32];

#[derive(Clone, Copy)]
struct FixedClock(QueryTimeSample);

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(self.0)
    }
}

fn checkpoint(path: &Path) {
    let connection = Connection::open(path).expect("checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

fn seed_empty_archive(path: &Path) {
    drop(UsageStore::open(path).expect("create archive"));
    checkpoint(path);
}

fn seed_current_archive(path: &Path, completed_at_ms: i64, quality: &str, event_count: u8) {
    seed_empty_archive(path);
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
             ) VALUES (?1, 'codex', 'default', 'fixture-source-private', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("source");
    transaction
        .execute(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completed_at_ms, completion_state,
               expected_scope_count
             ) VALUES (1, 1000, ?1, 'complete', 1)",
            [completed_at_ms],
        )
        .expect("scan set");
    transaction
        .execute(
            "INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1000, ?1, 'complete')",
            [completed_at_ms],
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
                 latest_complete_scan_set_id = 1, incremental_state = ?1
             WHERE singleton_id = 1",
            [quality],
        )
        .expect("publication");
    for index in 0..event_count {
        transaction
            .execute(
                "INSERT INTO usage_event(
                   fingerprint, event_id, selected_file_key, selected_generation,
                   selected_source_offset, projection_revision_id, origin_revision_id,
                   retained, provider_id, profile_id, session_id, source_id, timestamp_seconds,
                   timestamp_nanos, model, input_tokens, cached_tokens, output_tokens,
                   reasoning_tokens, total_tokens, fallback_model, long_context,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', 'session', 'fixture-source-private',
                   ?5, 0, 'gpt-5.6', ?6, NULL, 1, NULL, ?7, 0, 'no',
                   0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    [index + 1; 32].as_slice(),
                    format!("event-{index}"),
                    SOURCE_KEY.as_slice(),
                    i64::from(index),
                    1_000_i64 + i64::from(index),
                    i64::from(index) + 10,
                    i64::from(index) + 11,
                ],
            )
            .expect("event");
    }
    transaction.commit().expect("commit fixture");
    drop(connection);
    checkpoint(path);
}

fn service(path: &Path, wall_time_ms: i64) -> QueryService<FixedClock> {
    QueryService::open(path, FixedClock(QueryTimeSample::new(wall_time_ms, 10)))
        .expect("query service")
}

#[test]
fn empty_archive_is_authoritative_owned_and_strictly_generation_ordered() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("empty.sqlite3");
    seed_empty_archive(&path);
    let mut service = service(&path, 42);
    let request = LatestActivityRequest::first(PageSize::new(16).expect("page"));

    let first = service.latest_activity(request).expect("first snapshot");
    assert_eq!(first.header().snapshot_generation().get(), 1);
    assert_eq!(first.header().publication_generation().get(), 0);
    assert_eq!(first.header().dataset_identity(), DatasetIdentity::Empty);
    assert_eq!(first.header().generated_at_ms(), 42);
    assert_eq!(first.header().data_through_ms(), None);
    assert_eq!(first.header().freshness(), QueryFreshness::Unavailable);
    assert_eq!(first.header().quality(), QueryQuality::Authoritative);
    assert!(first.header().scopes().is_empty());
    assert!(first.header().warnings().is_empty());
    assert!(first.payload().items().is_empty());

    let second = service.latest_activity(request).expect("second snapshot");
    assert_eq!(second.header().snapshot_generation().get(), 2);
    assert!(second.is_newer_than(Some(&first)));
    assert_eq!(format!("{service:?}"), "QueryService([redacted])");
    drop(service);
    assert!(first.payload().items().is_empty());
}

#[test]
fn missing_archive_fails_path_free_without_creating_it() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("missing-private-name.sqlite3");
    let error = QueryService::open(&path, FixedClock(QueryTimeSample::new(1, 1)))
        .expect_err("missing archive");
    assert_eq!(error.code(), QueryErrorCode::Unavailable);
    assert_eq!(error.to_string(), "unavailable");
    assert!(!error.to_string().contains("missing-private-name"));
    assert!(!path.exists());
}

#[test]
fn freshness_boundaries_and_recovery_quality_are_truthful() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("current.sqlite3");
    let data_through = 10_000;
    seed_current_archive(&path, data_through, "complete", 1);
    let request = LatestActivityRequest::first(PageSize::new(1).expect("page"));

    for (age, expected) in [
        (QUERY_FRESH_MAX_AGE_MS, QueryFreshness::Fresh),
        (QUERY_FRESH_MAX_AGE_MS + 1, QueryFreshness::Aging),
        (QUERY_STALE_MIN_AGE_MS, QueryFreshness::Aging),
        (QUERY_STALE_MIN_AGE_MS + 1, QueryFreshness::Stale),
    ] {
        assert_eq!(
            service(&path, data_through + age)
                .latest_activity(request)
                .expect("freshness snapshot")
                .header()
                .freshness(),
            expected
        );
    }

    let connection = Connection::open(&path).expect("quality connection");
    connection
        .execute(
            "UPDATE usage_archive_state SET incremental_state = 'recovery_pending'",
            [],
        )
        .expect("recovery state");
    drop(connection);
    let recovered = service(&path, data_through + 1)
        .latest_activity(request)
        .expect("recovery snapshot");
    assert_eq!(recovered.header().quality(), QueryQuality::Partial);
    assert_eq!(
        recovered.header().warnings().as_ref(),
        &[QueryWarningCode::RecoveryPending]
    );
}

#[test]
fn clock_rollback_is_unavailable_and_never_negative_age() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("rollback.sqlite3");
    seed_current_archive(&path, 10_000, "complete", 1);
    let snapshot = service(&path, 9_999)
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(1).expect("page"),
        ))
        .expect("rollback snapshot");
    assert_eq!(snapshot.header().freshness(), QueryFreshness::Unavailable);
    assert_eq!(
        snapshot.header().warnings().as_ref(),
        &[QueryWarningCode::ClockDiscontinuity]
    );
}

#[test]
fn stale_accounting_version_is_visible_but_never_authoritative() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("stale-accounting.sqlite3");
    seed_current_archive(&path, 10_000, "complete", 1);
    let connection = Connection::open(&path).expect("stale version connection");
    connection
        .execute(
            "UPDATE usage_replay_revision SET fingerprint_version = 1
             WHERE status = 'current'",
            [],
        )
        .expect("stale fingerprint version");
    drop(connection);
    let snapshot = service(&path, 10_001)
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(1).expect("page"),
        ))
        .expect("stale accounting snapshot");
    assert_eq!(snapshot.header().quality(), QueryQuality::Unknown);
    assert_eq!(
        snapshot.header().warnings().as_ref(),
        &[QueryWarningCode::AccountingVersionStale]
    );
}

#[test]
fn activity_mapping_paging_and_failed_generation_are_exact() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("activity.sqlite3");
    seed_current_archive(&path, 2_000, "complete", 3);
    let mut service = service(&path, 2_001);
    let page_size = PageSize::new(2).expect("page");
    let first = service
        .latest_activity(LatestActivityRequest::first(page_size))
        .expect("first page");
    assert_eq!(first.payload().items().len(), 2);
    assert_eq!(first.payload().items()[0].event_id(), "event-2");
    assert_eq!(
        first.payload().items()[0].scope().provider_id().as_str(),
        "codex"
    );
    assert_eq!(
        first.payload().items()[0].usage().input(),
        TokenCount::Available(12)
    );
    assert_eq!(
        first.payload().items()[0].usage().cached(),
        TokenCount::Unavailable
    );
    assert!(first.payload().has_more());
    let path_text = path.to_string_lossy();
    let debug = format!("{first:?}");
    for private in [
        path_text.as_ref(),
        "fixture-source-private",
        "private-prompt",
        "private-response",
        "private-command",
        "private-reasoning",
        "usage_time_rollup",
        "usage_session_rollup",
        "SELECT ",
        "select ",
    ] {
        assert!(
            !debug.contains(private),
            "snapshot Debug exposed private fixture: {private}"
        );
    }
    assert!(!debug.contains("[3, 3, 3, 3"));
    assert!(debug.contains("fingerprint: [redacted]"));
    let cursor = first.payload().next_cursor().expect("cursor");

    let stale = service
        .latest_activity(LatestActivityRequest::continuation(
            page_size,
            DatasetIdentity::ReplayRevision {
                revision: tokenmaster_query::ReplayRevision::new(1).expect("revision"),
                dataset_generation: DatasetGeneration::new(1).expect("generation"),
            },
            cursor,
        ))
        .expect_err("stale dataset");
    assert_eq!(stale.code(), QueryErrorCode::StaleSnapshot);

    let connection = Connection::open(&path).expect("no-change publication connection");
    connection
        .execute("UPDATE usage_archive_state SET archive_generation = 5", [])
        .expect("advance publication only");
    drop(connection);

    let second = service
        .latest_activity(LatestActivityRequest::continuation(
            page_size,
            first.header().dataset_identity(),
            cursor,
        ))
        .expect("second page");
    assert_eq!(second.header().snapshot_generation().get(), 2);
    assert_eq!(second.header().publication_generation().get(), 5);
    assert_eq!(
        second.header().dataset_identity(),
        first.header().dataset_identity()
    );
    assert_eq!(second.payload().items().len(), 1);
    assert_eq!(second.payload().items()[0].event_id(), "event-0");
    assert!(!second.payload().has_more());

    let connection = Connection::open(&path).expect("dataset mutation connection");
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             UPDATE usage_event SET timestamp_seconds = 3000 WHERE event_id = 'event-2';
             UPDATE usage_archive_state SET archive_generation = 6 WHERE singleton_id = 1;
             COMMIT;",
        )
        .expect("mutate current revision dataset");
    drop(connection);
    let stale_epoch = service
        .latest_activity(LatestActivityRequest::continuation(
            page_size,
            first.header().dataset_identity(),
            cursor,
        ))
        .expect_err("stale dataset generation");
    assert_eq!(stale_epoch.code(), QueryErrorCode::StaleSnapshot);
    let changed = service
        .latest_activity(LatestActivityRequest::first(page_size))
        .expect("changed dataset first page");
    assert_eq!(changed.header().snapshot_generation().get(), 3);
    assert_ne!(
        changed.header().dataset_identity(),
        first.header().dataset_identity()
    );
}

#[test]
fn analytics_mapping_is_calendar_exact_owned_and_never_fabricates_missing_tokens() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("analytics.sqlite3");
    seed_current_archive(&path, 2_000, "complete", 3);
    let connection = Connection::open(&path).expect("partial-token connection");
    connection
        .execute(
            "UPDATE usage_event SET input_tokens = NULL WHERE event_id = 'event-0'",
            [],
        )
        .expect("make one input value unavailable");
    drop(connection);
    let mut service = service(&path, 2_001);
    let request = UsageAnalyticsRequest::new(
        UsageRange::custom(
            CalendarDate::new(1970, 1, 1).expect("start"),
            CalendarDate::new(1970, 1, 2).expect("end"),
        )
        .expect("range"),
        UsageTimeZone::iana("utc").expect("UTC"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        vec![
            UsageBreakdownKind::Profile,
            UsageBreakdownKind::Provider,
            UsageBreakdownKind::Project,
            UsageBreakdownKind::Model,
        ],
    )
    .expect("analytics request");
    let snapshot = service.usage_analytics(request).expect("analytics");

    assert_eq!(snapshot.header().snapshot_generation().get(), 1);
    assert!(snapshot.header().scopes().is_empty());
    assert_eq!(snapshot.payload().range().time_zone_id(), "UTC");
    assert_eq!(snapshot.payload().range().start_seconds(), 0);
    assert_eq!(snapshot.payload().range().end_seconds(), 86_400);
    assert_eq!(snapshot.payload().overview().event_count(), 3);
    assert_eq!(
        snapshot.payload().overview_cost().availability(),
        CostAvailability::Unavailable
    );
    assert_eq!(
        snapshot.payload().overview_cost().counters().total_events,
        3
    );
    assert_eq!(
        snapshot.payload().overview().input(),
        AggregateTokenValue::Partial {
            known_sum: 23,
            known_count: 2,
            event_count: 3,
        }
    );
    assert_eq!(
        snapshot.payload().overview().cached(),
        AggregateTokenValue::Unavailable
    );
    assert_eq!(
        snapshot.payload().overview().reasoning(),
        AggregateTokenValue::Unavailable
    );
    assert_eq!(
        snapshot.payload().overview().total(),
        AggregateTokenValue::Known(36)
    );
    assert_eq!(snapshot.payload().series().len(), 1);
    assert_eq!(snapshot.payload().series()[0].metrics().event_count(), 3);
    assert_eq!(
        snapshot.payload().series()[0].cost().availability(),
        CostAvailability::Unavailable
    );
    assert_eq!(snapshot.payload().breakdowns().len(), 4);
    assert_eq!(
        snapshot.payload().breakdowns()[0].kind(),
        UsageBreakdownKind::Model
    );
    assert_eq!(snapshot.payload().breakdowns()[0].items().len(), 1);
    assert_eq!(
        snapshot.payload().breakdowns()[0].items()[0]
            .cost()
            .availability(),
        CostAvailability::Unavailable
    );
    assert!(matches!(
        snapshot.payload().breakdowns()[1].items()[0].identity(),
        UsageBreakdownIdentity::UnassociatedProject
    ));

    drop(service);
    assert_eq!(snapshot.payload().overview().event_count(), 3);
    let debug = format!("{snapshot:?}");
    for private in [
        path.to_string_lossy().as_ref(),
        "fixture-source-private",
        "private-prompt",
        "private-response",
        "private-command",
        "private-reasoning",
    ] {
        assert!(
            !debug.contains(private),
            "analytics Debug exposed {private}"
        );
    }
}

#[test]
fn unavailable_aggregate_rebuild_does_not_consume_snapshot_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("rebuild.sqlite3");
    seed_current_archive(&path, 2_000, "complete", 1);
    let connection = Connection::open(&path).expect("rebuild connection");
    connection
        .execute(
            "UPDATE usage_aggregate_state SET state = 'rebuild_required' WHERE singleton_id = 1",
            [],
        )
        .expect("require aggregate rebuild");
    drop(connection);

    let mut service = service(&path, 2_001);
    let error = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::today(),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::None,
                Vec::new(),
                Vec::new(),
            )
            .expect("request"),
        )
        .expect_err("rebuild is unavailable");
    assert_eq!(error.code(), QueryErrorCode::Unavailable);

    let activity = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(1).expect("page"),
        ))
        .expect("activity remains available");
    assert_eq!(activity.header().snapshot_generation().get(), 1);
}

#[test]
fn calculated_cost_is_exact_across_overview_breakdowns_and_sessions() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("priced.sqlite3");
    seed_current_archive(&path, 2_000, "complete", 3);
    let connection = Connection::open(&path).expect("priced fixture connection");
    connection
        .execute(
            "UPDATE usage_event SET cached_tokens = 0, reasoning_tokens = 0",
            [],
        )
        .expect("make token basis calculable");
    drop(connection);

    let mut service = service(&path, 2_001);
    let analytics = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::today(),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::Daily,
                Vec::new(),
                vec![
                    UsageBreakdownKind::Model,
                    UsageBreakdownKind::Project,
                    UsageBreakdownKind::Provider,
                    UsageBreakdownKind::Profile,
                ],
            )
            .expect("analytics request"),
        )
        .expect("priced analytics");
    assert_eq!(
        analytics.payload().overview_cost().availability(),
        CostAvailability::Complete
    );
    assert_eq!(
        analytics
            .payload()
            .overview_cost()
            .amount()
            .map(|amount| amount.get()),
        Some(255)
    );
    assert_eq!(
        analytics.payload().series()[0]
            .cost()
            .amount()
            .map(|amount| amount.get()),
        Some(255)
    );
    for breakdown in analytics.payload().breakdowns().iter() {
        assert_eq!(breakdown.items().len(), 1);
        assert_eq!(
            breakdown.items()[0].cost().availability(),
            CostAvailability::Complete
        );
        assert_eq!(
            breakdown.items()[0]
                .cost()
                .amount()
                .map(|amount| amount.get()),
            Some(255)
        );
    }

    let page = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(1).expect("page"), Vec::new())
                .expect("session request"),
        )
        .expect("priced sessions");
    assert_eq!(
        page.payload().sessions()[0]
            .cost()
            .amount()
            .map(|amount| amount.get()),
        Some(255)
    );
    let detail = service
        .usage_session_detail(page.payload().sessions()[0].key().clone())
        .expect("priced detail");
    let detail = detail.payload().detail().expect("detail");
    assert_eq!(
        detail.summary().cost().amount().map(|amount| amount.get()),
        Some(255)
    );
    assert!(detail.breakdowns().iter().all(|breakdown| {
        breakdown.items().len() == 1
            && breakdown.items()[0]
                .cost()
                .amount()
                .is_some_and(|amount| amount.get() == 255)
    }));
}

#[test]
fn session_page_detail_cursor_and_stale_dataset_are_opaque_and_exact() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("sessions.sqlite3");
    seed_current_archive(&path, 2_000, "complete", 3);
    let connection = Connection::open(&path).expect("session fixture connection");
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             UPDATE usage_event SET session_id = 'private-session-a' WHERE event_id = 'event-0';
             UPDATE usage_event SET session_id = 'private-session-b' WHERE event_id = 'event-1';
             UPDATE usage_event SET session_id = 'private-session-c' WHERE event_id = 'event-2';
             COMMIT;",
        )
        .expect("split sessions");
    drop(connection);

    let mut service = service(&path, 2_001);
    let first = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(2).expect("page"), Vec::new())
                .expect("request"),
        )
        .expect("first session page");
    assert_eq!(first.payload().sessions().len(), 2);
    assert!(first.payload().has_more());
    assert_eq!(
        first.payload().sessions()[0].last_timestamp_seconds(),
        1_002
    );
    assert_eq!(first.payload().sessions()[0].metrics().event_count(), 1);
    assert_eq!(
        first.payload().sessions()[0].cost().availability(),
        CostAvailability::Unavailable
    );
    assert_eq!(
        first.payload().sessions()[0].metrics().input(),
        AggregateTokenValue::Known(12)
    );
    let key = first.payload().sessions()[0].key().clone();
    let cursor = first
        .payload()
        .next_cursor()
        .expect("continuation cursor")
        .clone();
    assert_eq!(key.dataset_identity(), first.header().dataset_identity());
    assert_eq!(cursor.dataset_identity(), first.header().dataset_identity());
    let debug = format!("{first:?} {key:?} {cursor:?}");
    for private in [
        path.to_string_lossy().as_ref(),
        "private-session-a",
        "private-session-b",
        "private-session-c",
        "fixture-source-private",
        "usage_time_rollup",
        "usage_session_rollup",
        "SELECT ",
        "select ",
    ] {
        assert!(!debug.contains(private), "session Debug exposed {private}");
    }
    assert!(debug.contains("identity: \"[redacted]\""));

    let changed_filter = UsageSessionPageRequest::continuation(
        PageSize::new(2).expect("page"),
        cursor.clone(),
        vec![tokenmaster_query::QueryScope::new(
            UsageProviderId::new("codex").expect("provider"),
            UsageProfileId::new("default").expect("profile"),
        )],
    )
    .expect_err("cursor cannot cross a filter change");
    assert_eq!(changed_filter.code(), QueryErrorCode::InvalidValue);

    let detail = service.usage_session_detail(key).expect("session detail");
    let detail = detail.payload().detail().expect("existing detail");
    assert_eq!(detail.summary().metrics().event_count(), 1);
    assert_eq!(
        detail.summary().cost().availability(),
        CostAvailability::Unavailable
    );
    assert_eq!(detail.breakdowns().len(), 2);
    assert_eq!(detail.breakdowns()[0].kind(), UsageBreakdownKind::Model);
    assert_eq!(
        detail.breakdowns()[0].items()[0].cost().availability(),
        CostAvailability::Unavailable
    );
    assert_eq!(detail.breakdowns()[1].kind(), UsageBreakdownKind::Project);
    assert!(matches!(
        detail.breakdowns()[1].items()[0].identity(),
        UsageBreakdownIdentity::UnassociatedProject
    ));

    let connection = Connection::open(&path).expect("publication connection");
    connection
        .execute("UPDATE usage_archive_state SET archive_generation = 5", [])
        .expect("no-change publication");
    drop(connection);
    let second = service
        .usage_sessions(
            UsageSessionPageRequest::continuation(
                PageSize::new(2).expect("page"),
                cursor.clone(),
                Vec::new(),
            )
            .expect("continuation request"),
        )
        .expect("continuation page");
    assert_eq!(second.header().snapshot_generation().get(), 3);
    assert_eq!(second.header().publication_generation().get(), 5);
    assert_eq!(second.payload().sessions().len(), 1);
    assert!(!second.payload().has_more());

    let connection = Connection::open(&path).expect("dataset mutation connection");
    connection
        .execute(
            "UPDATE usage_event SET model = 'gpt-5.7' WHERE event_id = 'event-0'",
            [],
        )
        .expect("mutate session dataset");
    drop(connection);
    let stale = service
        .usage_sessions(
            UsageSessionPageRequest::continuation(
                PageSize::new(2).expect("page"),
                cursor,
                Vec::new(),
            )
            .expect("stale request"),
        )
        .expect_err("stale dataset");
    assert_eq!(stale.code(), QueryErrorCode::StaleSnapshot);

    let changed = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(2).expect("page"), Vec::new())
                .expect("request"),
        )
        .expect("changed first page");
    assert_eq!(changed.header().snapshot_generation().get(), 4);
    assert_ne!(
        changed.header().dataset_identity(),
        first.header().dataset_identity()
    );
}

#[test]
fn missing_session_detail_is_typed_none_for_the_same_dataset() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("missing-session.sqlite3");
    seed_current_archive(&path, 2_000, "complete", 1);
    let mut service = service(&path, 2_001);
    let page = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(1).expect("page"), Vec::new())
                .expect("request"),
        )
        .expect("session page");
    let key = page.payload().sessions()[0].key().clone();

    let connection = Connection::open(&path).expect("fixture connection");
    connection
        .execute(
            "DELETE FROM usage_session_rollup
             WHERE provider_id = 'codex' AND profile_id = 'default' AND session_id = 'session'",
            [],
        )
        .expect("remove exact rollup fixture");
    drop(connection);

    let detail = service
        .usage_session_detail(key)
        .expect("typed missing detail");
    assert!(detail.payload().detail().is_none());
    assert_eq!(detail.header().snapshot_generation().get(), 2);
}
