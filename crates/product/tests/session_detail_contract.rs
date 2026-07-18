use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_product::{
    ProductAttemptGeneration, ProductPublishOutcome, ProductReducer, ProductSectionKind,
    ProductSessionDetailSelection, ProductSessionDetailSelectionGeneration,
};
use tokenmaster_query::{
    PageSize, QueryClock, QueryError, QueryErrorCode, QueryService, QueryTimeSample,
    UsageSessionPageRequest,
};
use tokenmaster_store::UsageStore;

const SOURCE_KEY: [u8; 32] = [17; 32];

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

fn selection(generation: u64, row_ordinal: u8) -> ProductSessionDetailSelection {
    ProductSessionDetailSelection::new(
        ProductSessionDetailSelectionGeneration::new(generation)
            .expect("nonzero selection generation"),
        row_ordinal,
    )
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
             ) VALUES (?1, 'codex', 'default', 'private-fixture-source', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [18_u8; 32].as_slice(),
                [19_u8; 32].as_slice()
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
    for (index, session_id) in ["private-session-a", "private-session-b"]
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
                   'private-fixture-source', ?6, 0, 'gpt-5.6', ?7, NULL, 1,
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

#[test]
fn detail_selection_is_latest_wins_and_never_retains_another_rows_payload() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("session-detail-product.sqlite3");
    seed_two_sessions(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let page = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(2).expect("page"), Vec::new())
                .expect("request"),
        )
        .expect("sessions");
    let first = service
        .usage_session_detail(page.payload().sessions()[0].key().clone())
        .expect("first detail");
    let second = service
        .usage_session_detail(page.payload().sessions()[1].key().clone())
        .expect("second detail");
    let first_selection = selection(1, 0);
    let second_selection = selection(2, 1);
    let failed_selection = selection(3, 0);
    let recovered_selection = selection(4, 1);
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");

    assert_eq!(
        reducer
            .publish_session_detail(attempt(2), first_selection, first.clone())
            .expect("first detail"),
        ProductPublishOutcome::Accepted
    );
    assert_eq!(
        reducer
            .publish_session_detail(attempt(2), selection(1, 1), second.clone())
            .expect("conflicting ordinal"),
        ProductPublishOutcome::RejectedIncompatible
    );
    assert_eq!(
        reducer.snapshot().session_detail_selection(),
        Some(first_selection)
    );
    assert_eq!(
        reducer
            .publish_session_detail(attempt(3), second_selection, second.clone())
            .expect("second detail"),
        ProductPublishOutcome::Accepted
    );
    assert_eq!(
        reducer
            .fail_session_detail(
                attempt(4),
                failed_selection,
                QueryErrorCode::DeadlineExceeded,
            )
            .expect("detail failure"),
        ProductPublishOutcome::Accepted
    );
    let failed = reducer.snapshot();
    assert_eq!(failed.session_detail_selection(), Some(failed_selection));
    assert_eq!(
        failed.session_detail().kind(),
        ProductSectionKind::Unavailable
    );
    assert!(!failed.session_detail().retains_payload());
    assert!(failed.session_detail().payload().is_none());

    assert_eq!(
        reducer
            .publish_session_detail(attempt(5), first_selection, first)
            .expect("older selection"),
        ProductPublishOutcome::RejectedOlder
    );
    assert_eq!(
        reducer
            .publish_session_detail(attempt(5), recovered_selection, second)
            .expect("newer selection recovery"),
        ProductPublishOutcome::Accepted
    );
    let recovered = reducer.snapshot();
    assert_eq!(
        recovered.session_detail_selection(),
        Some(recovered_selection)
    );
    assert_eq!(recovered.session_detail().kind(), ProductSectionKind::Ready);

    let connection = Connection::open(&path).expect("publication connection");
    connection
        .execute(
            "UPDATE usage_event SET model = 'gpt-5.7' WHERE event_id = 'event-0'",
            [],
        )
        .expect("mutate dataset identity");
    drop(connection);
    let changed_status = service.product_data_status().expect("changed status");
    reducer
        .publish_data_status(attempt(6), changed_status)
        .expect("publish changed status");
    let invalidated = reducer.snapshot();
    assert_eq!(
        invalidated.session_detail_selection(),
        Some(recovered_selection)
    );
    assert_eq!(
        invalidated.session_detail().kind(),
        ProductSectionKind::Unavailable
    );
    assert!(!invalidated.session_detail().retains_payload());
    assert_eq!(
        invalidated
            .session_detail()
            .failure()
            .expect("stale detail failure")
            .code(),
        QueryErrorCode::StaleSnapshot
    );
}

#[test]
fn missing_detail_is_ready_truth_for_the_exact_selection() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("missing-session-detail-product.sqlite3");
    seed_two_sessions(&path);
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let status = service.product_data_status().expect("status");
    let page = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(1).expect("page"), Vec::new())
                .expect("request"),
        )
        .expect("sessions");
    let key = page.payload().sessions()[0].key().clone();
    let connection = Connection::open(&path).expect("fixture connection");
    connection
        .execute("DELETE FROM usage_session_rollup", [])
        .expect("remove rollups");
    drop(connection);
    let missing = service
        .usage_session_detail(key)
        .expect("typed missing detail");
    assert!(missing.payload().detail().is_none());

    let exact_selection = selection(1, 0);
    let mut reducer = ProductReducer::new();
    reducer
        .publish_data_status(attempt(1), status)
        .expect("publish status");
    reducer
        .publish_session_detail(attempt(2), exact_selection, missing)
        .expect("publish missing truth");
    let snapshot = reducer.snapshot();
    assert_eq!(snapshot.session_detail_selection(), Some(exact_selection));
    assert_eq!(snapshot.session_detail().kind(), ProductSectionKind::Ready);
    assert!(
        snapshot
            .session_detail()
            .payload()
            .expect("ready missing payload")
            .payload()
            .detail()
            .is_none()
    );
}

#[test]
fn selection_generation_is_nonzero_and_contains_no_opaque_identity() {
    assert!(ProductSessionDetailSelectionGeneration::new(0).is_none());
    let exact = selection(9, 63);
    assert_eq!(exact.generation().get(), 9);
    assert_eq!(exact.row_ordinal(), 63);
    assert_eq!(
        format!("{exact:?}"),
        "ProductSessionDetailSelection { generation: ProductSessionDetailSelectionGeneration(9), row_ordinal: 63 }"
    );
}
