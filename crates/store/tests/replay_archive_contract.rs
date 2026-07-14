use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_accounting::{
    CANONICALIZER_VERSION, EVENT_FINGERPRINT_VERSION, REPLAY_SIGNATURE_VERSION,
};
use tokenmaster_store::{ArchiveMode, StoreErrorCode, UsageStore};

fn create_v1_event_fixture(path: &std::path::Path) {
    let connection = Connection::open(path).expect("create v1 archive fixture");
    connection
        .execute_batch(include_str!("fixtures/usage_v1.sql"))
        .expect("create exact v1 schema");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity
             ) VALUES (
               zeroblob(32), 'codex', 'default', 'fixture', 'active',
               zeroblob(32), zeroblob(32)
             );
             INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version,
               physical_identity, logical_identity, committed_offset, scan_offset,
               observed_file_length, anchor_start, anchor_len, anchor_sha256,
               resume_payload, discarding_oversized_line, incomplete_tail,
               verification_level
             ) VALUES (
               zeroblob(32), 0, 'current', 1, zeroblob(32), zeroblob(32),
               0, 0, 0, 0, 0, zeroblob(32), zeroblob(0), 0, 0, 'full_prefix'
             );
             UPDATE usage_source SET current_generation = 0;
             INSERT INTO usage_observation(
               file_key, generation, source_offset, fingerprint, event_id,
               profile_id, session_id, source_id, timestamp_seconds,
               timestamp_nanos, model, input_tokens, output_tokens, total_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (
               zeroblob(32), 0, 0, zeroblob(32), 'legacy-event', 'default',
               'legacy-session', 'fixture', 100, 0, 'gpt-test', 1, 2, 3,
               0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
             );
             INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               output_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents,
               activity_terminal
             ) VALUES (
               zeroblob(32), 'legacy-event', zeroblob(32), 0, 0, 'default',
               'legacy-session', 'fixture', 100, 0, 'gpt-test', 1, 2, 3,
               0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
             );",
        )
        .expect("seed v1 archive fixture");
}

fn insert_revision(path: &std::path::Path, revision_id: i64, status: &str) {
    let connection = Connection::open(path).expect("open revision fixture");
    let (sealed, promoted) = if status == "current" { (1, 1) } else { (0, 0) };
    connection
        .execute(
            "INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 7, ?6, ?7)",
            rusqlite::params![
                revision_id,
                status,
                i64::from(CANONICALIZER_VERSION),
                i64::from(EVENT_FINGERPRINT_VERSION),
                i64::from(REPLAY_SIGNATURE_VERSION),
                sealed,
                promoted,
            ],
        )
        .expect("insert replay revision fixture");
}

#[test]
fn fresh_migrated_current_and_staging_archive_states_are_explicit() {
    let fresh_directory = TempDir::new().expect("fresh temporary directory");
    let fresh_path = fresh_directory.path().join("fresh-state-private.sqlite3");
    drop(UsageStore::open(&fresh_path).expect("create fresh v2 archive"));
    let fresh = UsageStore::open(&fresh_path).expect("open fresh archive");
    let state = fresh.archive_state().expect("fresh archive state");
    assert_eq!(state.mode(), ArchiveMode::Empty);
    assert_eq!(state.active_revision(), None);
    assert!(!state.rebuild_staging());
    assert!(
        fresh
            .event_page_before(None, 256)
            .expect("empty page")
            .is_empty()
    );
    drop(fresh);

    insert_revision(&fresh_path, 5, "current");
    let verified = UsageStore::open(&fresh_path).expect("open verified archive");
    let state = verified.archive_state().expect("verified state");
    assert_eq!(state.mode(), ArchiveMode::ReplayVerified);
    assert_eq!(state.active_revision().expect("active revision").get(), 5);
    assert!(!state.rebuild_staging());
    let quality = verified
        .replay_quality(state.active_revision().expect("quality revision"))
        .expect("empty quality counts");
    assert_eq!(quality.eligible(), 0);
    assert_eq!(quality.replay(), 0);
    assert_eq!(quality.pending(), 0);
    assert_eq!(quality.conflict(), 0);
    drop(verified);

    insert_revision(&fresh_path, 6, "staging");
    let staging = UsageStore::open(&fresh_path).expect("open staging archive");
    let state = staging.archive_state().expect("staging state");
    assert_eq!(state.mode(), ArchiveMode::ReplayVerified);
    assert_eq!(state.active_revision().expect("current revision").get(), 5);
    assert!(state.rebuild_staging());
    drop(staging);

    let connection = Connection::open(&fresh_path).expect("make revision stale");
    connection
        .execute(
            "UPDATE usage_replay_revision
             SET fingerprint_version = fingerprint_version + 1
             WHERE revision_id = 5",
            [],
        )
        .expect("change stored accounting version");
    drop(connection);
    let stale = UsageStore::open(&fresh_path).expect("open stale archive");
    assert_eq!(
        stale.archive_state().expect("stale state").mode(),
        ArchiveMode::ReplayVersionStale
    );
}

#[test]
fn migrated_legacy_reads_ignore_mutated_live_projection() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("legacy-read-private.sqlite3");
    create_v1_event_fixture(&path);
    drop(UsageStore::open(&path).expect("migrate v1 archive"));
    let connection = Connection::open(&path).expect("mutate old live projection");
    connection
        .execute("UPDATE usage_event SET event_id = 'mutated-live'", [])
        .expect("mutate old live projection");
    drop(connection);

    let store = UsageStore::open(&path).expect("open migrated archive");
    let state = store.archive_state().expect("legacy state");
    assert_eq!(state.mode(), ArchiveMode::LegacyUnverified);
    assert_eq!(state.active_revision(), None);
    assert!(!state.rebuild_staging());
    let page = store.event_page_before(None, 256).expect("legacy page");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].event_id(), "legacy-event");
}

#[test]
fn replay_quality_rejects_an_unknown_revision() {
    let store = UsageStore::in_memory().expect("usage store");
    let unknown = tokenmaster_store::ReplayRevisionId::new(99).expect("revision id");
    let error = store
        .replay_quality(unknown)
        .expect_err("unknown revision must fail closed");
    assert_eq!(error.code(), StoreErrorCode::StaleRevision);
}

#[test]
fn replay_quality_counts_each_disposition_without_returning_rows() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quality-counts-private.sqlite3");
    drop(UsageStore::open(&path).expect("create v2 archive"));
    let mut connection = Connection::open(&path).expect("open quality fixture");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable fixture foreign keys");
    let transaction = connection
        .transaction()
        .expect("quality fixture transaction");
    transaction
        .execute_batch(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity
             ) VALUES (
               zeroblob(32), 'codex', 'default', 'fixture', 'active',
               zeroblob(32), zeroblob(32)
             );
             INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version,
               physical_identity, logical_identity, committed_offset, scan_offset,
               observed_file_length, anchor_start, anchor_len, anchor_sha256,
               resume_payload, discarding_oversized_line, incomplete_tail,
               verification_level
             ) VALUES (
               zeroblob(32), 0, 'current', 1, zeroblob(32), zeroblob(32),
               0, 0, 0, 0, 0, zeroblob(32), zeroblob(0), 0, 0, 'full_prefix'
             );
             UPDATE usage_source SET current_generation = 0;
             INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted
             ) VALUES (9, 'staging', 1, 2, 1, 1, 0, 0, 0);",
        )
        .expect("seed quality fixture headers");
    for (index, disposition) in ["eligible", "replay", "pending", "conflict"]
        .into_iter()
        .enumerate()
    {
        let value = u8::try_from(index + 1).expect("bounded fixture index");
        let digest = [value; 32];
        let offset = i64::try_from(index).expect("bounded fixture offset");
        let session_id = format!("session-{index}");
        transaction
            .execute(
                "INSERT INTO usage_observation(
                   file_key, generation, source_offset, fingerprint, event_id,
                   profile_id, session_id, source_id, timestamp_seconds,
                   timestamp_nanos, model, input_tokens, output_tokens, total_tokens,
                   fallback_model, long_context, activity_read, activity_edit_write,
                   activity_search, activity_git, activity_build_test, activity_web,
                   activity_subagents, activity_terminal
                 ) VALUES (
                   zeroblob(32), 0, ?1, ?2, ?3, 'default', ?4, 'fixture', ?1,
                   0, 'gpt-test', 1, 2, 3, 0, 'no', 0, 0, 0, 0, 0, 0, 0, 0
                 )",
                rusqlite::params![
                    offset,
                    digest.as_slice(),
                    format!("event-{index}"),
                    session_id
                ],
            )
            .expect("insert quality observation");
        transaction
            .execute(
                "INSERT INTO usage_replay_observation(
                   revision_id, file_key, generation, source_offset, fingerprint,
                   provider_id, profile_id, session_id, session_ordinal,
                   canonicalizer_version, fingerprint_version,
                   replay_signature_version, replay_signature, evidence,
                   disposition, declared_conflict, evidence_epoch
                 ) VALUES (
                   9, zeroblob(32), 0, ?1, ?2, 'codex', 'default', ?3, 0,
                   1, 2, 1, ?2, 'strong_cumulative', ?4, 0, 0
                 )",
                rusqlite::params![offset, digest.as_slice(), session_id, disposition],
            )
            .expect("insert quality replay overlay");
    }
    transaction.commit().expect("commit quality fixture");
    drop(connection);

    let store = UsageStore::open(&path).expect("reopen quality fixture");
    let revision = tokenmaster_store::ReplayRevisionId::new(9).expect("revision id");
    let quality = store.replay_quality(revision).expect("quality counts");
    assert_eq!(quality.eligible(), 1);
    assert_eq!(quality.replay(), 1);
    assert_eq!(quality.pending(), 1);
    assert_eq!(quality.conflict(), 1);
    assert_eq!(quality.total(), 4);
}
