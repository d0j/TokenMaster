use std::{fs, path::Path, time::Duration};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    EXPECTED_SQLITE_VERSION, EventCursor, JournalMode, MAX_USAGE_OVERVIEW_SEGMENTS, ScanScope,
    StoreErrorCode, USAGE_SCHEMA_VERSION, UsageActivityQuery, UsageAggregateBucketWidth,
    UsageAggregateSegment, UsageOverviewQuery, UsageQueryDatasetIdentity, UsageReadStore,
    UsageStore,
};

const SOURCE_KEY: [u8; 32] = [7; 32];

fn create_empty_archive(path: &Path) {
    drop(UsageStore::open(path).expect("create archive"));
    checkpoint(path);
}

fn checkpoint(path: &Path) {
    let connection = Connection::open(path).expect("open checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint archive");
}

fn seed_current_archive(path: &Path) {
    create_empty_archive(path);
    let mut connection = Connection::open(path).expect("open fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'fixture', 'active', ?2, ?3, 0)",
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
             ) VALUES (1, 1000, 2000, 'complete', 1)",
            [],
        )
        .expect("scan set");
    transaction
        .execute(
            "INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1000, 1900, 'complete')",
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
    for index in 0_u8..3 {
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
                   ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', 'session', 'fixture',
                   ?5, ?6, 'gpt-5.6', ?7, NULL, 1, NULL, ?8, 0, 'no',
                   0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    [index + 1; 32].as_slice(),
                    format!("event-{index}"),
                    SOURCE_KEY.as_slice(),
                    i64::from(index),
                    1_000_i64 + i64::from(index),
                    i64::from(index),
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

fn insert_current_event(path: &Path, index: u8, timestamp_seconds: i64) {
    let connection = Connection::open(path).expect("open current event fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
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
               ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', 'session', 'fixture',
               ?5, 0, 'gpt-5.6', 5, NULL, 2, NULL, 7, 0, 'no',
               0, 0, 0, 0, 0, 0, 0, 0
             )",
            params![
                [index; 32].as_slice(),
                format!("boundary-event-{index}"),
                SOURCE_KEY.as_slice(),
                i64::from(index),
                timestamp_seconds,
            ],
        )
        .expect("insert current event");
}

fn seed_legacy_archive(path: &Path) {
    let mut connection = Connection::open(path).expect("create v1 fixture");
    connection
        .execute_batch(include_str!("fixtures/usage_v1.sql"))
        .expect("exact v1 schema");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("legacy transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity
             ) VALUES (?1, 'codex', 'legacy', 'fixture', 'archived', ?2, ?3)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("legacy source");
    transaction
        .execute(
            "INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version, physical_identity,
               logical_identity, committed_offset, scan_offset, observed_file_length,
               modified_time_ns, anchor_start, anchor_len, anchor_sha256, resume_payload,
               discarding_oversized_line, incomplete_tail, verification_level
             ) VALUES (?1, 0, 'current', 1, ?2, ?3, 0, 0, 0, NULL, 0, 0, ?4, X'',
                       0, 0, 'full_prefix')",
            params![
                SOURCE_KEY.as_slice(),
                [3_u8; 32].as_slice(),
                [2_u8; 32].as_slice(),
                [4_u8; 32].as_slice()
            ],
        )
        .expect("legacy generation");
    transaction
        .execute(
            "INSERT INTO usage_observation(
               file_key, generation, source_offset, fingerprint, event_id, profile_id,
               session_id, source_id, timestamp_seconds, timestamp_nanos, model,
               input_tokens, cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (?1, 0, 0, ?2, 'event-legacy', 'legacy', 'session', 'fixture',
                       900, 0, 'gpt-5.6', 4, NULL, 1, NULL, 5, 0, 'no',
                       0, 0, 0, 0, 0, 0, 0, 0)",
            params![SOURCE_KEY.as_slice(), [9_u8; 32].as_slice()],
        )
        .expect("legacy observation");
    transaction
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens, cached_tokens,
               output_tokens, reasoning_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) SELECT fingerprint, event_id, file_key, generation, source_offset,
                      profile_id, session_id, source_id, timestamp_seconds, timestamp_nanos,
                      model, input_tokens, cached_tokens, output_tokens, reasoning_tokens,
                      total_tokens, fallback_model, long_context, activity_read,
                      activity_edit_write, activity_search, activity_git,
                      activity_build_test, activity_web, activity_subagents,
                      activity_terminal
               FROM usage_observation",
            [],
        )
        .expect("legacy event");
    transaction.commit().expect("legacy commit");
    drop(connection);
    drop(UsageStore::open(path).expect("migrate legacy archive"));
    checkpoint(path);
}

fn query(
    expected: Option<UsageQueryDatasetIdentity>,
    before: Option<tokenmaster_store::EventCursor>,
    page_size: usize,
) -> UsageActivityQuery {
    UsageActivityQuery::new(expected, before, page_size, Duration::from_secs(2))
        .expect("valid query")
}

fn overview_query(
    expected: Option<UsageQueryDatasetIdentity>,
    scopes: Box<[ScanScope]>,
) -> UsageOverviewQuery {
    UsageOverviewQuery::new(
        expected,
        vec![
            UsageAggregateSegment::new(UsageAggregateBucketWidth::Hour, 0, 3_600)
                .expect("valid overview segment"),
        ]
        .into_boxed_slice(),
        scopes,
        Duration::from_secs(2),
    )
    .expect("valid overview query")
}

#[test]
fn aggregate_overview_is_exact_scope_bounded_and_generation_bound() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("aggregate-overview.sqlite3");
    seed_current_archive(&path);
    let mut store = UsageReadStore::open(&path).expect("aggregate read store");

    let capture = store
        .capture_usage_overview(overview_query(None, Box::default()))
        .expect("aggregate overview");
    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::ReplayRevision {
            revision_id: 0,
            dataset_generation: 3,
        }
    );
    let metrics = capture.metrics();
    assert_eq!(metrics.event_count(), 3);
    assert_eq!(metrics.input().known_count(), 3);
    assert_eq!(metrics.input().known_sum(), 33);
    assert_eq!(metrics.cached().known_count(), 0);
    assert_eq!(metrics.cached().known_sum(), 0);
    assert_eq!(metrics.output().known_count(), 3);
    assert_eq!(metrics.output().known_sum(), 3);
    assert_eq!(metrics.reasoning().known_count(), 0);
    assert_eq!(metrics.total().known_count(), 3);
    assert_eq!(metrics.total().known_sum(), 36);
    assert_eq!(metrics.long_context_no_count(), 3);

    let included = ScanScope::new("codex", "default").expect("included scope");
    assert_eq!(
        store
            .capture_usage_overview(overview_query(None, vec![included].into_boxed_slice()))
            .expect("scoped overview")
            .metrics(),
        metrics
    );
    let excluded = ScanScope::new("codex", "other").expect("excluded scope");
    assert_eq!(
        store
            .capture_usage_overview(overview_query(None, vec![excluded].into_boxed_slice()))
            .expect("empty scoped overview")
            .metrics()
            .event_count(),
        0
    );

    let stale = overview_query(
        Some(UsageQueryDatasetIdentity::ReplayRevision {
            revision_id: 0,
            dataset_generation: 2,
        }),
        Box::default(),
    );
    assert_eq!(
        store
            .capture_usage_overview(stale)
            .expect_err("stale aggregate dataset")
            .code(),
        StoreErrorCode::StaleRevision
    );
}

#[test]
fn aggregate_overview_composes_adjacent_widths_without_gaps_or_double_counting() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("aggregate-segments.sqlite3");
    seed_current_archive(&path);
    insert_current_event(&path, 20, 3_600);
    insert_current_event(&path, 21, 7_200);

    let segments = vec![
        UsageAggregateSegment::new(UsageAggregateBucketWidth::Minute, 0, 3_600)
            .expect("minute prefix"),
        UsageAggregateSegment::new(UsageAggregateBucketWidth::Hour, 3_600, 7_200)
            .expect("hour middle"),
        UsageAggregateSegment::new(UsageAggregateBucketWidth::Minute, 7_200, 7_260)
            .expect("minute suffix"),
    ]
    .into_boxed_slice();
    let query = UsageOverviewQuery::new(None, segments, Box::default(), Duration::from_secs(2))
        .expect("valid composed overview");
    let mut store = UsageReadStore::open(&path).expect("aggregate read store");
    let capture = store
        .capture_usage_overview(query)
        .expect("composed overview");

    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::ReplayRevision {
            revision_id: 0,
            dataset_generation: 5,
        }
    );
    let metrics = capture.metrics();
    assert_eq!(metrics.event_count(), 5);
    assert_eq!(metrics.input().known_count(), 5);
    assert_eq!(metrics.input().known_sum(), 43);
    assert_eq!(metrics.output().known_sum(), 7);
    assert_eq!(metrics.total().known_sum(), 50);
    assert_eq!(metrics.long_context_no_count(), 5);
}

#[test]
fn aggregate_overview_rejects_unavailable_state_and_invalid_bounds() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("aggregate-unavailable.sqlite3");
    seed_current_archive(&path);
    let connection = Connection::open(&path).expect("aggregate state writer");
    connection
        .execute(
            "UPDATE usage_aggregate_state
             SET state = 'rebuild_required', rebuild_total_events = current_event_count
             WHERE singleton_id = 1",
            [],
        )
        .expect("require rebuild");
    drop(connection);
    let mut store = UsageReadStore::open(&path).expect("aggregate read store");
    assert_eq!(
        store
            .capture_usage_overview(overview_query(None, Box::default()))
            .expect_err("unavailable aggregates")
            .code(),
        StoreErrorCode::RebuildRequired
    );

    assert_eq!(
        UsageAggregateSegment::new(UsageAggregateBucketWidth::Hour, 1, 3_600,)
            .expect_err("misaligned range")
            .code(),
        StoreErrorCode::InvalidValue
    );
    let scopes = (0..33)
        .map(|index| ScanScope::new("codex", format!("scope-{index}")))
        .collect::<Result<Vec<_>, _>>()
        .expect("valid scopes")
        .into_boxed_slice();
    let error = UsageOverviewQuery::new(
        None,
        vec![
            UsageAggregateSegment::new(UsageAggregateBucketWidth::Hour, 0, 3_600)
                .expect("valid overview segment"),
        ]
        .into_boxed_slice(),
        scopes,
        Duration::from_secs(2),
    )
    .expect_err("scope overflow");
    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(32));

    let minute = |start, end| {
        UsageAggregateSegment::new(UsageAggregateBucketWidth::Minute, start, end)
            .expect("aligned minute segment")
    };
    for invalid_segments in [
        Vec::new(),
        vec![minute(0, 60), minute(120, 180)],
        vec![minute(0, 120), minute(60, 180)],
    ] {
        assert_eq!(
            UsageOverviewQuery::new(
                None,
                invalid_segments.into_boxed_slice(),
                Box::default(),
                Duration::from_secs(2),
            )
            .expect_err("invalid segment topology")
            .code(),
            StoreErrorCode::InvalidValue
        );
    }
    let too_many = (0..=MAX_USAGE_OVERVIEW_SEGMENTS)
        .map(|index| {
            let start = i64::try_from(index).expect("small index") * 60;
            minute(start, start + 60)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let error = UsageOverviewQuery::new(None, too_many, Box::default(), Duration::from_secs(2))
        .expect_err("segment overflow");
    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_USAGE_OVERVIEW_SEGMENTS as u64));
}

#[test]
fn read_store_is_query_only_bounded_and_does_not_modify_archive() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("usage.sqlite3");
    create_empty_archive(&path);
    let before = fs::read(&path).expect("archive bytes before");

    let mut store = UsageReadStore::open(&path).expect("read store");
    assert_eq!(
        store.sqlite_version().expect("SQLite version"),
        EXPECTED_SQLITE_VERSION
    );
    let policy = store.runtime_policy().expect("read policy");
    assert!(policy.query_only());
    assert!(policy.foreign_keys());
    assert!(!policy.trusted_schema());
    assert!(policy.defensive());
    assert!(policy.no_checkpoint_on_close());
    assert_eq!(policy.journal_mode(), JournalMode::Wal);
    assert_eq!(policy.busy_timeout_ms(), 250);
    assert_eq!(policy.cache_size_kib(), 4 * 1024);
    assert_eq!(policy.mmap_size_bytes(), 0);

    let capture = store
        .capture_activity_page(query(None, None, 256))
        .expect("empty capture");
    assert_eq!(capture.publication().generation(), 0);
    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::Empty
    );
    assert_eq!(capture.publication().data_through_ms(), None);
    assert!(capture.publication().scopes().is_empty());
    assert!(capture.events().is_empty());
    assert!(!capture.has_more());
    assert_eq!(format!("{store:?}"), "UsageReadStore([redacted])");
    drop(store);

    assert_eq!(fs::read(&path).expect("archive bytes after"), before);
    assert_eq!(
        UsageActivityQuery::new(None, None, 0, Duration::from_secs(2))
            .expect_err("zero page")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        UsageActivityQuery::new(None, None, 257, Duration::from_secs(2))
            .expect_err("oversized page")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        UsageActivityQuery::new(
            None,
            Some(EventCursor::new(0, 0, [0; 32]).expect("cursor")),
            1,
            Duration::from_secs(2),
        )
        .expect_err("continuation requires dataset identity")
        .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        UsageActivityQuery::new(
            Some(UsageQueryDatasetIdentity::ReplayRevision {
                revision_id: 0,
                dataset_generation: i64::MAX as u64 + 1,
            }),
            None,
            1,
            Duration::from_secs(2),
        )
        .expect_err("dataset generation exceeds SQLite range")
        .code(),
        StoreErrorCode::InvalidValue
    );
}

#[test]
fn read_store_rejects_missing_old_new_and_malformed_archives_without_migration() {
    let directory = TempDir::new().expect("temporary directory");
    let missing = directory.path().join("missing.sqlite3");
    assert_eq!(
        UsageReadStore::open(&missing)
            .expect_err("missing archive")
            .code(),
        StoreErrorCode::Database
    );
    assert!(!missing.exists());

    for (version, expected) in [
        (USAGE_SCHEMA_VERSION - 1, StoreErrorCode::SchemaMismatch),
        (USAGE_SCHEMA_VERSION + 1, StoreErrorCode::SchemaTooNew),
    ] {
        let path = directory.path().join(format!("version-{version}.sqlite3"));
        create_empty_archive(&path);
        let connection = Connection::open(&path).expect("open version fixture");
        connection
            .pragma_update(None, "user_version", version)
            .expect("set fixture version");
        drop(connection);
        checkpoint(&path);
        let before = fs::read(&path).expect("version bytes before");
        assert_eq!(
            UsageReadStore::open(&path)
                .expect_err("version must fail")
                .code(),
            expected
        );
        assert_eq!(fs::read(&path).expect("version bytes after"), before);
    }

    let malformed = directory.path().join("malformed.sqlite3");
    create_empty_archive(&malformed);
    let connection = Connection::open(&malformed).expect("open malformed fixture");
    connection
        .execute("DROP INDEX usage_event_time_desc", [])
        .expect("damage index");
    drop(connection);
    checkpoint(&malformed);
    let before = fs::read(&malformed).expect("malformed bytes before");
    assert_eq!(
        UsageReadStore::open(&malformed)
            .expect_err("malformed archive")
            .code(),
        StoreErrorCode::SchemaMismatch
    );
    assert_eq!(fs::read(&malformed).expect("malformed bytes after"), before);
}

#[test]
fn capture_is_exact_keyset_bounded_and_rejects_stale_dataset() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("usage.sqlite3");
    seed_current_archive(&path);
    let source_writer = Connection::open(&path).expect("source metadata writer");
    source_writer
        .execute(
            "UPDATE usage_source SET provider_id = 'changed-after-materialization'",
            [],
        )
        .expect("mutate non-authoritative source metadata");
    drop(source_writer);
    let mut store = UsageReadStore::open(&path).expect("read store");
    let identity = UsageQueryDatasetIdentity::ReplayRevision {
        revision_id: 0,
        dataset_generation: 3,
    };

    let first = store
        .capture_activity_page(query(Some(identity), None, 2))
        .expect("first page");
    assert_eq!(first.publication().generation(), 4);
    assert_eq!(first.publication().dataset_identity(), identity);
    assert_eq!(first.publication().data_through_ms(), Some(2_000));
    assert!(first.publication().accounting_versions_current());
    assert_eq!(first.publication().scopes().len(), 1);
    assert_eq!(first.events().len(), 2);
    assert_eq!(first.events()[0].event_id(), "event-2");
    assert_eq!(first.events()[0].provider_id(), "codex");
    assert_eq!(first.events()[0].profile_id(), "default");
    assert_eq!(first.events()[0].input_tokens(), Some(12));
    assert_eq!(first.events()[0].cached_tokens(), None);
    assert!(first.has_more());
    let cursor = first.next_cursor().expect("continuation cursor");

    let second = store
        .capture_activity_page(query(Some(identity), Some(cursor), 2))
        .expect("second page");
    assert_eq!(second.events().len(), 1);
    assert_eq!(second.events()[0].event_id(), "event-0");
    assert!(!second.has_more());
    assert!(second.next_cursor().is_none());

    let stale = store
        .capture_activity_page(query(
            Some(UsageQueryDatasetIdentity::ReplayRevision {
                revision_id: 1,
                dataset_generation: 3,
            }),
            Some(cursor),
            2,
        ))
        .expect_err("stale dataset");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);

    let writer = Connection::open(&path).expect("append identity writer");
    writer
        .execute_batch(
            "BEGIN IMMEDIATE;
             UPDATE usage_event SET timestamp_seconds = 3000 WHERE event_id = 'event-2';
             UPDATE usage_archive_state SET archive_generation = 5 WHERE singleton_id = 1;
             COMMIT;",
        )
        .expect("simulate current revision append publication");
    drop(writer);
    let stale_after_append = store
        .capture_activity_page(query(Some(identity), Some(cursor), 2))
        .expect_err("old cursor after current-revision mutation");
    assert_eq!(stale_after_append.code(), StoreErrorCode::StaleRevision);
    let appended_identity = UsageQueryDatasetIdentity::ReplayRevision {
        revision_id: 0,
        dataset_generation: 4,
    };
    assert_eq!(
        store
            .capture_activity_page(query(Some(appended_identity), None, 2))
            .expect("new dataset identity")
            .publication()
            .dataset_identity(),
        appended_identity
    );
}

#[test]
fn migrated_legacy_page_is_explicit_and_owned() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("legacy.sqlite3");
    seed_legacy_archive(&path);
    let mut store = UsageReadStore::open(&path).expect("legacy read store");
    let capture = store
        .capture_activity_page(query(
            Some(UsageQueryDatasetIdentity::LegacySnapshotV1),
            None,
            1,
        ))
        .expect("legacy capture");
    assert_eq!(
        capture.publication().dataset_identity(),
        UsageQueryDatasetIdentity::LegacySnapshotV1
    );
    assert_eq!(capture.publication().data_through_ms(), None);
    assert_eq!(capture.events().len(), 1);
    assert_eq!(capture.events()[0].event_id(), "event-legacy");
    assert_eq!(capture.events()[0].provider_id(), "codex");
    assert_eq!(capture.events()[0].profile_id(), "legacy");
    drop(store);
    assert_eq!(capture.events()[0].total_tokens(), Some(5));
}
