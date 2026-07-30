use std::{path::Path, time::Duration};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_store::{
    AggregateRebuildStatus, MAX_USAGE_QUERY_SCOPES, MAX_USAGE_SESSION_PAGE_SIZE, ScanScope,
    StoreErrorCode, UsageBreakdownIdentity, UsageBreakdownKind, UsageQueryDatasetIdentity,
    UsageReadStore, UsageSessionDetailQuery, UsageSessionPageQuery, UsageStore,
};

const SOURCE_KEY: [u8; 32] = [7; 32];
const SECOND_SOURCE_KEY: [u8; 32] = [8; 32];

fn create_current_archive(path: &Path) {
    drop(UsageStore::open(path).expect("create archive"));
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
                [3_u8; 32].as_slice(),
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
    transaction.commit().expect("commit fixture");
    checkpoint(path);
}

#[allow(clippy::too_many_arguments)]
fn insert_session_event(
    path: &Path,
    index: u8,
    session_id: &str,
    timestamp_seconds: i64,
    timestamp_nanos: u32,
    model: &str,
    project_alias: Option<&str>,
    total_tokens: i64,
) {
    let connection = Connection::open(path).expect("open event fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, provider_id, profile_id, session_id, source_id, timestamp_seconds,
               timestamp_nanos, model, project_alias, input_tokens, cached_tokens,
               output_tokens, reasoning_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) VALUES (
               ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', ?5, 'fixture', ?6,
               ?7, ?8, ?9, 5, NULL, 2, NULL, ?10, 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
             )",
            params![
                [index; 32].as_slice(),
                format!("session-event-{index}"),
                SOURCE_KEY.as_slice(),
                i64::from(index),
                session_id,
                timestamp_seconds,
                timestamp_nanos,
                model,
                project_alias,
                total_tokens,
            ],
        )
        .expect("insert event");
    checkpoint(path);
}

fn insert_bulk_session_events(path: &Path, count: usize, one_session: bool) {
    let mut connection = Connection::open(path).expect("open bulk fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("bulk transaction");
    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO usage_event(
                   fingerprint, event_id, selected_file_key, selected_generation,
                   selected_source_offset, projection_revision_id, origin_revision_id,
                   retained, provider_id, profile_id, session_id, source_id, timestamp_seconds,
                   timestamp_nanos, model, project_alias, input_tokens, cached_tokens,
                   output_tokens, reasoning_tokens, total_tokens, fallback_model, long_context,
                   activity_read, activity_edit_write, activity_search, activity_git,
                   activity_build_test, activity_web, activity_subagents, activity_terminal
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', ?5, 'fixture', ?6,
                   0, ?7, NULL, 1, NULL, 0, NULL, 1, 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
                 )",
            )
            .expect("bulk statement");
        for index in 0..count {
            let mut fingerprint = [0_u8; 32];
            fingerprint[..8].copy_from_slice(
                &u64::try_from(index)
                    .expect("bounded fixture index")
                    .to_le_bytes(),
            );
            let session = if one_session {
                "private-bulk".to_owned()
            } else {
                format!("private-session-{index:03}")
            };
            statement
                .execute(params![
                    fingerprint.as_slice(),
                    format!("bulk-event-{index}"),
                    SOURCE_KEY.as_slice(),
                    i64::try_from(index).expect("bounded source offset"),
                    session,
                    1_000_i64 + i64::try_from(index).expect("bounded timestamp"),
                    format!("m{index:03}"),
                ])
                .expect("bulk event");
        }
    }
    transaction.commit().expect("commit bulk fixture");
    drop(connection);
    checkpoint(path);
}

fn add_second_scope(path: &Path) {
    let connection = Connection::open(path).expect("open second scope fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'hermes', 'work', 'fixture-2', 'active', ?2, ?3, 0)",
            params![
                SECOND_SOURCE_KEY.as_slice(),
                [12_u8; 32].as_slice(),
                [13_u8; 32].as_slice(),
            ],
        )
        .expect("second source");
    connection
        .execute(
            "INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (2, 1, 'hermes', 'work', 1000, 1900, 'complete')",
            [],
        )
        .expect("second scan scope");
    connection
        .execute_batch(
            "UPDATE usage_scan_set SET expected_scope_count = 2 WHERE scan_set_id = 1;
             UPDATE usage_replay_revision SET expected_source_count = 2 WHERE revision_id = 0;",
        )
        .expect("second scope counts");
}

fn insert_second_scope_event(path: &Path) {
    let connection = Connection::open(path).expect("open second-scope event fixture");
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
               ?1, 'second-scope-event', ?2, 0, 0, 0, 0, 0, 'hermes', 'work',
               'private-hermes', 'fixture-2', 500, 0, 'hermes-model', 1, NULL, 0,
               NULL, 1, 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
             )",
            params![[90_u8; 32].as_slice(), SECOND_SOURCE_KEY.as_slice()],
        )
        .expect("second-scope event");
    checkpoint(path);
}

fn create_legacy_archive(path: &Path) {
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
                [3_u8; 32].as_slice(),
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
                [4_u8; 32].as_slice(),
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
             ) VALUES (?1, 0, 0, ?2, 'event-legacy', 'legacy', 'private-legacy', 'fixture',
                       900, 0, 'gpt-legacy', 4, NULL, 1, NULL, 5, 0, 'no',
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
    let mut writer = UsageStore::open(path).expect("migrate legacy archive");
    let mut ready = false;
    for _ in 0..8 {
        if writer
            .rebuild_aggregates_page(256)
            .expect("legacy rebuild page")
            .status()
            == AggregateRebuildStatus::Ready
        {
            ready = true;
            break;
        }
    }
    assert!(ready, "legacy rebuild did not finish within bound");
    drop(writer);
    checkpoint(path);
}

fn checkpoint(path: &Path) {
    Connection::open(path)
        .expect("open checkpoint connection")
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint archive");
}

#[test]
fn session_page_request_enforces_all_bounds_before_sqlite_access() {
    let zero_page =
        UsageSessionPageQuery::new(None, None, Box::default(), 0, Duration::from_secs(2))
            .expect_err("zero-sized page must fail");
    assert_eq!(zero_page.code(), StoreErrorCode::InvalidValue);

    let too_large = UsageSessionPageQuery::new(
        None,
        None,
        Box::default(),
        MAX_USAGE_SESSION_PAGE_SIZE + 1,
        Duration::from_secs(2),
    )
    .expect_err("oversized page must fail");
    assert_eq!(too_large.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(too_large.limit(), Some(MAX_USAGE_SESSION_PAGE_SIZE as u64));

    let scopes = (0..=MAX_USAGE_QUERY_SCOPES)
        .map(|index| ScanScope::new("codex", format!("profile-{index}")))
        .collect::<Result<Vec<_>, _>>()
        .expect("valid distinct scopes")
        .into_boxed_slice();
    let too_many_scopes = UsageSessionPageQuery::new(None, None, scopes, 1, Duration::from_secs(2))
        .expect_err("too many scopes must fail");
    assert_eq!(too_many_scopes.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(too_many_scopes.limit(), Some(MAX_USAGE_QUERY_SCOPES as u64));

    let excessive_deadline =
        UsageSessionPageQuery::new(None, None, Box::default(), 1, Duration::from_secs(3))
            .expect_err("deadline above the store budget must fail");
    assert_eq!(excessive_deadline.code(), StoreErrorCode::InvalidValue);
}

#[test]
fn session_pages_are_exact_opaque_keyset_bounded_and_dataset_bound() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("session-page.sqlite3");
    create_current_archive(&path);
    insert_session_event(&path, 1, "private-alpha", 100, 0, "gpt-a", Some("one"), 7);
    insert_session_event(&path, 2, "private-alpha", 200, 0, "gpt-b", None, 11);
    insert_session_event(&path, 3, "private-beta", 300, 0, "gpt-a", Some("two"), 13);
    insert_session_event(&path, 4, "private-gamma", 300, 0, "gpt-a", Some("two"), 17);

    let mut store = UsageReadStore::open(&path).expect("read store");
    let first = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(None, None, Box::default(), 2, Duration::from_secs(2))
                .expect("first query"),
        )
        .expect("first page");
    assert_eq!(first.sessions().len(), 2);
    assert!(first.has_more());
    assert!(first.next_cursor().is_some());
    assert_eq!(first.sessions()[0].last_timestamp_seconds(), 300);
    assert_eq!(first.sessions()[0].metrics().total().known_sum(), 13);
    assert_eq!(first.sessions()[1].metrics().total().known_sum(), 17);
    assert_eq!(first.sessions()[0].provider_id(), "codex");
    assert_eq!(first.sessions()[0].profile_id(), "default");
    let debug = format!("{first:?}");
    assert!(!debug.contains("private-alpha"));
    assert!(!debug.contains("private-beta"));
    assert!(!debug.contains("private-gamma"));

    let expected = first.publication().dataset_identity();
    let second = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(
                Some(expected),
                first.next_cursor().cloned(),
                Box::default(),
                2,
                Duration::from_secs(2),
            )
            .expect("cursor query"),
        )
        .expect("second page");
    assert_eq!(second.sessions().len(), 1);
    assert!(!second.has_more());
    assert!(second.next_cursor().is_none());
    assert_eq!(second.sessions()[0].metrics().event_count(), 2);
    assert_eq!(second.sessions()[0].first_timestamp_seconds(), 100);
    assert_eq!(second.sessions()[0].last_timestamp_seconds(), 200);

    let unbound_cursor = UsageSessionPageQuery::new(
        None,
        Some(second.sessions()[0].cursor()),
        Box::default(),
        2,
        Duration::from_secs(2),
    )
    .expect_err("a continuation without exact dataset identity must fail");
    assert_eq!(unbound_cursor.code(), StoreErrorCode::InvalidValue);

    insert_session_event(&path, 5, "private-delta", 400, 0, "gpt-a", None, 19);
    let stale = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(
                Some(expected),
                first.next_cursor().cloned(),
                Box::default(),
                2,
                Duration::from_secs(2),
            )
            .expect("stale cursor request"),
        )
        .expect_err("dataset mutation must reject the cursor");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
}

#[test]
fn session_detail_is_exact_bounded_and_never_scans_raw_session_content() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("session-detail.sqlite3");
    create_current_archive(&path);
    insert_session_event(&path, 1, "private-detail", 100, 1, "gpt-a", Some("one"), 7);
    insert_session_event(&path, 2, "private-detail", 200, 2, "gpt-b", None, 11);

    let mut store = UsageReadStore::open(&path).expect("read store");
    let page = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(None, None, Box::default(), 1, Duration::from_secs(2))
                .expect("page query"),
        )
        .expect("session page");
    let expected = page.publication().dataset_identity();
    assert!(matches!(
        expected,
        UsageQueryDatasetIdentity::ReplayRevision { .. }
    ));
    let detail = store
        .capture_usage_session_detail(
            UsageSessionDetailQuery::new(
                expected,
                page.sessions()[0].key().clone(),
                Duration::from_secs(2),
            )
            .expect("detail query"),
        )
        .expect("detail capture");
    let detail = detail.detail().expect("stored session detail");
    assert_eq!(detail.summary().metrics().event_count(), 2);
    assert_eq!(detail.summary().metrics().total().known_sum(), 18);
    assert_eq!(detail.breakdowns().len(), 2);
    assert_eq!(detail.breakdowns()[0].kind(), UsageBreakdownKind::Model);
    assert_eq!(detail.breakdowns()[0].items().len(), 2);
    assert_eq!(detail.breakdowns()[1].kind(), UsageBreakdownKind::Project);
    assert_eq!(detail.breakdowns()[1].items().len(), 2);
    assert!(
        detail.breakdowns()[1]
            .items()
            .iter()
            .any(|item| item.identity() == &UsageBreakdownIdentity::UnassociatedProject)
    );
    assert!(!format!("{detail:?}").contains("private-detail"));

    let missing_path = directory.path().join("session-detail-missing.sqlite3");
    create_current_archive(&missing_path);
    insert_session_event(&missing_path, 1, "other-one", 100, 0, "gpt-a", None, 1);
    insert_session_event(&missing_path, 2, "other-two", 200, 0, "gpt-b", None, 1);
    let mut missing_store = UsageReadStore::open(&missing_path).expect("missing read store");
    let missing = missing_store
        .capture_usage_session_detail(
            UsageSessionDetailQuery::new(
                expected,
                page.sessions()[0].key().clone(),
                Duration::from_secs(2),
            )
            .expect("missing detail query"),
        )
        .expect("missing detail capture");
    assert!(missing.detail().is_none());
}

#[test]
fn session_page_and_detail_use_one_row_lookahead_at_the_256_item_limit() {
    let directory = TempDir::new().expect("temporary directory");
    let page_path = directory.path().join("session-limit.sqlite3");
    create_current_archive(&page_path);
    insert_bulk_session_events(&page_path, MAX_USAGE_SESSION_PAGE_SIZE + 1, false);
    let mut page_store = UsageReadStore::open(&page_path).expect("page read store");
    let first = page_store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(
                None,
                None,
                Box::default(),
                MAX_USAGE_SESSION_PAGE_SIZE,
                Duration::from_secs(2),
            )
            .expect("bounded first page"),
        )
        .expect("bounded first capture");
    assert_eq!(first.sessions().len(), MAX_USAGE_SESSION_PAGE_SIZE);
    assert!(first.has_more());
    let second = page_store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(
                Some(first.publication().dataset_identity()),
                first.next_cursor().cloned(),
                Box::default(),
                MAX_USAGE_SESSION_PAGE_SIZE,
                Duration::from_secs(2),
            )
            .expect("bounded continuation"),
        )
        .expect("bounded second capture");
    assert_eq!(second.sessions().len(), 1);
    assert!(!second.has_more());
    assert_eq!(second.sessions()[0].first_timestamp_seconds(), 1_000);

    let detail_path = directory.path().join("detail-limit.sqlite3");
    create_current_archive(&detail_path);
    insert_bulk_session_events(&detail_path, 257, true);
    let mut detail_store = UsageReadStore::open(&detail_path).expect("detail read store");
    let page = detail_store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(None, None, Box::default(), 1, Duration::from_secs(2))
                .expect("detail page query"),
        )
        .expect("detail page");
    let detail = detail_store
        .capture_usage_session_detail(
            UsageSessionDetailQuery::new(
                page.publication().dataset_identity(),
                page.sessions()[0].key().clone(),
                Duration::from_secs(2),
            )
            .expect("bounded detail query"),
        )
        .expect("bounded detail capture");
    let detail = detail.detail().expect("detail exists");
    let models = &detail.breakdowns()[0];
    assert_eq!(models.kind(), UsageBreakdownKind::Model);
    assert_eq!(models.items().len(), 256);
    assert!(models.truncated());
    assert_eq!(
        models.items()[0].identity(),
        &UsageBreakdownIdentity::Model("m000".into())
    );
    let projects = &detail.breakdowns()[1];
    assert_eq!(projects.items().len(), 1);
    assert!(!projects.truncated());
    assert_eq!(
        projects.items()[0].identity(),
        &UsageBreakdownIdentity::UnassociatedProject
    );
}

#[test]
fn session_pages_apply_exact_provider_profile_scopes() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("session-scope.sqlite3");
    create_current_archive(&path);
    insert_session_event(&path, 1, "private-codex", 100, 0, "gpt-a", None, 7);
    add_second_scope(&path);
    insert_second_scope_event(&path);
    let mut store = UsageReadStore::open(&path).expect("scope read store");

    let codex = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(
                None,
                None,
                vec![ScanScope::new("codex", "default").expect("codex scope")].into_boxed_slice(),
                16,
                Duration::from_secs(2),
            )
            .expect("codex query"),
        )
        .expect("codex page");
    assert_eq!(codex.sessions().len(), 1);
    assert_eq!(codex.sessions()[0].provider_id(), "codex");

    let hermes = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(
                Some(codex.publication().dataset_identity()),
                None,
                vec![ScanScope::new("hermes", "work").expect("hermes scope")].into_boxed_slice(),
                16,
                Duration::from_secs(2),
            )
            .expect("hermes query"),
        )
        .expect("hermes page");
    assert_eq!(hermes.sessions().len(), 1);
    assert_eq!(hermes.sessions()[0].provider_id(), "hermes");
    assert_eq!(hermes.sessions()[0].profile_id(), "work");
}

#[test]
fn rebuilt_legacy_session_page_and_detail_preserve_legacy_identity() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("legacy-session.sqlite3");
    create_legacy_archive(&path);
    let mut store = UsageReadStore::open(&path).expect("legacy read store");
    let page = store
        .capture_usage_session_page(
            UsageSessionPageQuery::new(
                Some(UsageQueryDatasetIdentity::LegacySnapshotV1),
                None,
                Box::default(),
                16,
                Duration::from_secs(2),
            )
            .expect("legacy page query"),
        )
        .expect("legacy page");
    assert_eq!(
        page.publication().dataset_identity(),
        UsageQueryDatasetIdentity::LegacySnapshotV1
    );
    assert_eq!(page.sessions().len(), 1);
    assert_eq!(page.sessions()[0].profile_id(), "legacy");
    assert_eq!(page.sessions()[0].metrics().total().known_sum(), 5);
    assert!(!format!("{page:?}").contains("private-legacy"));

    let detail = store
        .capture_usage_session_detail(
            UsageSessionDetailQuery::new(
                UsageQueryDatasetIdentity::LegacySnapshotV1,
                page.sessions()[0].key().clone(),
                Duration::from_secs(2),
            )
            .expect("legacy detail query"),
        )
        .expect("legacy detail");
    let detail = detail.detail().expect("legacy detail exists");
    assert_eq!(detail.summary().metrics().event_count(), 1);
    assert_eq!(
        detail.breakdowns()[0].items()[0].identity(),
        &UsageBreakdownIdentity::Model("gpt-legacy".into())
    );
    assert_eq!(
        detail.breakdowns()[1].items()[0].identity(),
        &UsageBreakdownIdentity::UnassociatedProject
    );
}
