use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_domain::TokenCount;
use tokenmaster_query::{
    DatasetGeneration, DatasetIdentity, LatestActivityRequest, PageSize, QUERY_FRESH_MAX_AGE_MS,
    QUERY_STALE_MIN_AGE_MS, QueryClock, QueryError, QueryErrorCode, QueryFreshness, QueryQuality,
    QueryService, QueryTimeSample, QueryWarningCode,
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
