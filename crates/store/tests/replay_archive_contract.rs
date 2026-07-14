use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_accounting::{
    CANONICALIZER_VERSION, CanonicalUsageEvent, Canonicalizer, EVENT_FINGERPRINT_VERSION,
    REPLAY_SIGNATURE_VERSION,
};
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, TokenCount, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, ArchiveMode, GenerationStatus, ReplayAppendBatch,
    ReplayAppendBatchParts, ReplayManifest, ReplayRevisionStatus, SourceKey, SourceKind,
    SourceRegistration, SourceRegistrationParts, StoreErrorCode, StoredCheckpoint,
    StoredCheckpointParts, StoredSourceChunk, StoredVerification, UsageStore,
};

fn registration(seed: u8) -> SourceRegistration {
    SourceRegistration::new(SourceRegistrationParts {
        source_key: SourceKey::from_bytes([seed; 32]),
        provider_id: "codex".into(),
        profile_id: "default".into(),
        source_id: format!("fixture-{seed}").into_boxed_str(),
        source_kind: SourceKind::Active,
        logical_identity: [seed.wrapping_add(1); 32],
        physical_identity: Some([seed; 32]),
        initial_checkpoint: StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some([seed; 32]),
            logical_identity: [seed.wrapping_add(1); 32],
            committed_offset: 0,
            scan_offset: 0,
            observed_file_length: 0,
            modified_time_ns: None,
            anchor_start: 0,
            anchor_len: 0,
            anchor_sha256: [seed.wrapping_add(2); 32],
            resume: Box::default(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::Incremental,
        })
        .expect("initial checkpoint"),
    })
    .expect("source registration")
}

fn checkpoint(seed: u8, offset: u64, verification: StoredVerification) -> StoredCheckpoint {
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([seed; 32]),
        logical_identity: [seed.wrapping_add(1); 32],
        committed_offset: offset,
        scan_offset: offset,
        observed_file_length: offset,
        modified_time_ns: Some(i64::try_from(offset).expect("fixture offset")),
        anchor_start: 0,
        anchor_len: u16::try_from(offset.min(100)).expect("fixture anchor"),
        anchor_sha256: [seed.wrapping_add(2); 32],
        resume: Box::default(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification,
    })
    .expect("replay checkpoint")
}

#[allow(clippy::too_many_arguments)]
fn replay_event(
    seed: u8,
    session: &str,
    parent: Option<&str>,
    ordinal: u64,
    source_offset: u64,
    cumulative_input: Option<u64>,
    declared_conflict: bool,
) -> CanonicalUsageEvent {
    replay_event_at(
        seed,
        session,
        parent,
        ordinal,
        source_offset,
        cumulative_input,
        declared_conflict,
        1_720_598_400 + source_offset as i64,
    )
}

#[allow(clippy::too_many_arguments)]
fn replay_event_at(
    seed: u8,
    session: &str,
    parent: Option<&str>,
    ordinal: u64,
    source_offset: u64,
    cumulative_input: Option<u64>,
    declared_conflict: bool,
    timestamp_seconds: i64,
) -> CanonicalUsageEvent {
    let delta = TokenUsage::new(
        TokenCount::Available(10 + ordinal),
        TokenCount::Unavailable,
        TokenCount::Available(2),
        TokenCount::Unavailable,
        TokenCount::Available(12 + ordinal),
    );
    let cumulative = cumulative_input.map(|input| {
        TokenUsage::new(
            TokenCount::Available(input),
            TokenCount::Unavailable,
            TokenCount::Available(20 + ordinal),
            TokenCount::Unavailable,
            TokenCount::Available(input + 20 + ordinal),
        )
    });
    let draft = ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new("codex").expect("provider"),
        profile_id: UsageProfileId::new("default").expect("profile"),
        session_id: UsageSessionId::new(session).expect("session"),
        parent_session_id: parent.map(|value| UsageSessionId::new(value).expect("parent")),
        session_ordinal: ordinal,
        lineage_conflict: declared_conflict,
        source_id: UsageSourceId::new(format!("fixture-{seed}")).expect("source"),
        source_offset,
        source_verification: ObservationVerification::FullPrefix,
        timestamp: UtcTimestamp::new(timestamp_seconds, 0).expect("timestamp"),
        model: ModelKey::new("gpt-test").expect("model"),
        raw_model: None,
        delta_usage: delta,
        cumulative_usage: cumulative,
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        project: None,
        originator: None,
        activity: ActivityCounts::default(),
    })
    .expect("observation draft");
    Canonicalizer::new()
        .canonicalize(&draft)
        .expect("canonical event")
}

fn replay_append(
    seed: u8,
    revision: tokenmaster_store::ReplayRevisionId,
    epoch: tokenmaster_store::ReplayEpoch,
    events: Vec<CanonicalUsageEvent>,
) -> ReplayAppendBatch {
    let append = AppendBatch::new(AppendBatchParts {
        source_key: SourceKey::from_bytes([seed; 32]),
        expected_generation: 1,
        expected_committed_offset: 0,
        expected_scan_offset: 0,
        events: events.into_boxed_slice(),
        previous_partial_chunk: None,
        chunk_updates: vec![
            StoredSourceChunk::new(0, 100, [seed.wrapping_add(3); 32]).expect("source chunk"),
        ]
        .into_boxed_slice(),
        next_checkpoint: checkpoint(seed, 100, StoredVerification::FullPrefix),
        last_seen_scan_id: None,
        diagnostic_count_delta: 0,
    })
    .expect("append batch");
    ReplayAppendBatch::new(ReplayAppendBatchParts {
        revision_id: revision,
        expected_epoch: epoch,
        append_batch: append,
    })
}

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

#[test]
fn replay_manifest_bounds_and_begin_are_atomic_invisible_and_version_owned() {
    let empty_error = ReplayManifest::new(Box::default()).expect_err("empty manifest");
    assert_eq!(empty_error.code(), StoreErrorCode::InvalidValue);

    let oversized = (0..=256)
        .map(|value| {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(&(value as u64).to_be_bytes());
            SourceKey::from_bytes(bytes)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let oversized_error = ReplayManifest::new(oversized).expect_err("oversized manifest");
    assert_eq!(oversized_error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(oversized_error.limit(), Some(256));

    let duplicate_error = ReplayManifest::new(
        vec![
            SourceKey::from_bytes([1; 32]),
            SourceKey::from_bytes([1; 32]),
        ]
        .into_boxed_slice(),
    )
    .expect_err("duplicate manifest key");
    assert_eq!(duplicate_error.code(), StoreErrorCode::InvalidValue);

    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("replay-begin-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store.register_source(&registration(2)).expect("source 2");
    store.register_source(&registration(1)).expect("source 1");
    let before_page = store
        .event_page_before(None, 256)
        .expect("page before begin");
    let manifest = ReplayManifest::new(
        vec![
            SourceKey::from_bytes([2; 32]),
            SourceKey::from_bytes([1; 32]),
        ]
        .into_boxed_slice(),
    )
    .expect("bounded manifest");
    let manifest_debug = format!("{manifest:?}");
    assert!(manifest_debug.contains("source_count: 2"));
    assert!(!manifest_debug.contains("SourceKey"));
    assert!(!manifest_debug.contains("[1, 1"));

    let revision = store
        .begin_replay_revision(&manifest)
        .expect("begin replay revision");
    assert_eq!(revision.id().get(), 0);
    assert_eq!(revision.epoch().get(), 0);
    assert_eq!(revision.status(), ReplayRevisionStatus::Staging);
    assert_eq!(revision.expected_source_count(), 2);
    assert!(!revision.sealed());
    assert!(!revision.promoted());
    assert_eq!(revision.versions().canonicalizer(), CANONICALIZER_VERSION);
    assert_eq!(revision.versions().fingerprint(), EVENT_FINGERPRINT_VERSION);
    assert_eq!(
        revision.versions().replay_signature(),
        REPLAY_SIGNATURE_VERSION
    );
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("page after begin"),
        before_page
    );
    let state = store.archive_state().expect("rebuild archive state");
    assert_eq!(state.mode(), ArchiveMode::Empty);
    assert!(state.rebuild_staging());
    for seed in [1_u8, 2_u8] {
        let current = store
            .generation_snapshot(SourceKey::from_bytes([seed; 32]))
            .expect("current generation")
            .expect("registered current generation");
        assert_eq!(current.generation(), 0);
        assert_eq!(current.status(), GenerationStatus::Current);
    }
    let before_repeat = store.counts().expect("counts before repeat");
    let repeat_error = store
        .begin_replay_revision(&manifest)
        .expect_err("second staging revision must fail");
    assert_eq!(repeat_error.code(), StoreErrorCode::ArchiveModeMismatch);
    assert_eq!(store.counts().expect("counts after repeat"), before_repeat);
    drop(store);

    let connection = Connection::open(&path).expect("inspect staging generations");
    let staging: i64 = connection
        .query_row(
            "SELECT count(*) FROM usage_generation
             WHERE generation = 1 AND status = 'staging'",
            [],
            |row| row.get(0),
        )
        .expect("count staging generations");
    assert_eq!(staging, 2);
    let manifest_rows: i64 = connection
        .query_row("SELECT count(*) FROM usage_replay_source", [], |row| {
            row.get(0)
        })
        .expect("count replay manifest rows");
    assert_eq!(manifest_rows, 2);
}

#[test]
fn replay_begin_rejects_an_unregistered_source_without_partial_state() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(4))
        .expect("registered source");
    let manifest = ReplayManifest::new(
        vec![
            SourceKey::from_bytes([4; 32]),
            SourceKey::from_bytes([9; 32]),
        ]
        .into_boxed_slice(),
    )
    .expect("bounded manifest");
    let before = store.counts().expect("counts before rejected begin");
    let error = store
        .begin_replay_revision(&manifest)
        .expect_err("unregistered source must fail");
    assert_eq!(error.code(), StoreErrorCode::InvalidValue);
    assert_eq!(store.counts().expect("counts after rejected begin"), before);
    assert!(
        !store
            .archive_state()
            .expect("archive state")
            .rebuild_staging()
    );
}

#[test]
fn replay_append_derives_root_eligibility_and_keeps_staging_invisible() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(3))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([3; 32])].into_boxed_slice())
        .expect("manifest");
    let revision = store
        .begin_replay_revision(&manifest)
        .expect("begin replay revision");
    let page_before = store
        .event_page_before(None, 256)
        .expect("page before append");
    let batch = replay_append(
        3,
        revision.id(),
        revision.epoch(),
        vec![replay_event(3, "root", None, 0, 10, Some(100), false)],
    );

    let next_epoch = store
        .apply_replay_append_batch(&batch)
        .expect("apply replay append");
    assert_eq!(next_epoch.get(), 1);
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("page after append"),
        page_before
    );
    let quality = store.replay_quality(revision.id()).expect("replay quality");
    assert_eq!(quality.eligible(), 1);
    assert_eq!(quality.replay(), 0);
    assert_eq!(quality.pending(), 0);
    assert_eq!(quality.conflict(), 0);

    let counts_before_stale = store.counts().expect("counts before stale append");
    let stale = store
        .apply_replay_append_batch(&batch)
        .expect_err("stale epoch must fail");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
    assert_eq!(
        store.counts().expect("counts after stale append"),
        counts_before_stale
    );
}

#[test]
fn replay_append_persists_replay_divergence_pending_conflict_and_selection() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("replay-classification-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(5))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([5; 32])].into_boxed_slice())
        .expect("manifest");
    let revision = store
        .begin_replay_revision(&manifest)
        .expect("begin replay revision");
    let events = vec![
        replay_event(5, "parent", None, 0, 10, Some(100), false),
        replay_event(5, "parent", None, 1, 20, Some(110), false),
        replay_event(5, "parent", None, 2, 30, None, false),
        replay_event(5, "child", Some("parent"), 0, 40, Some(100), false),
        replay_event(5, "child", Some("parent"), 1, 50, Some(111), false),
        replay_event(5, "weak-child", Some("parent"), 2, 60, None, false),
        replay_event(
            5,
            "missing-child",
            Some("absent-parent"),
            0,
            70,
            Some(100),
            false,
        ),
        replay_event(
            5,
            "conflict-child",
            Some("other-parent"),
            0,
            80,
            Some(100),
            true,
        ),
    ];
    let batch = replay_append(5, revision.id(), revision.epoch(), events);
    let epoch = store
        .apply_replay_append_batch(&batch)
        .expect("apply classified replay batch");
    assert_eq!(epoch.get(), 1);
    let quality = store
        .replay_quality(revision.id())
        .expect("classification quality");
    assert_eq!(quality.eligible(), 4);
    assert_eq!(quality.replay(), 1);
    assert_eq!(quality.pending(), 2);
    assert_eq!(quality.conflict(), 1);
    assert_eq!(quality.total(), 8);
    assert!(
        store
            .event_page_before(None, 256)
            .expect("staging remains invisible")
            .is_empty()
    );
    drop(store);

    let connection = Connection::open(&path).expect("inspect replay archive");
    let selections: i64 = connection
        .query_row(
            "SELECT count(*) FROM usage_replay_selection WHERE revision_id = 0",
            [],
            |row| row.get(0),
        )
        .expect("selection count");
    assert_eq!(selections, 4);
    let missing_work: i64 = connection
        .query_row(
            "SELECT count(*) FROM usage_replay_work
             WHERE revision_id = 0 AND reason = 'missing_parent'",
            [],
            |row| row.get(0),
        )
        .expect("missing-parent work count");
    assert_eq!(missing_work, 1);
    let source_state: String = connection
        .query_row(
            "SELECT state FROM usage_replay_source WHERE revision_id = 0",
            [],
            |row| row.get(0),
        )
        .expect("replay source state");
    assert_eq!(source_state, "complete");
    let visible_verification: String = connection
        .query_row(
            "SELECT verification_level FROM usage_source WHERE file_key = ?1",
            [[5_u8; 32].as_slice()],
            |row| row.get(0),
        )
        .expect("visible source verification");
    assert_eq!(visible_verification, "incremental");
    let states: Vec<(String, String)> = connection
        .prepare(
            "SELECT session_id, state FROM usage_replay_session
             WHERE revision_id = 0 ORDER BY session_id",
        )
        .expect("prepare session states")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query session states")
        .collect::<Result<_, _>>()
        .expect("collect session states");
    assert!(states.contains(&("child".to_owned(), "diverged".to_owned())));
    assert!(states.contains(&("weak-child".to_owned(), "matching".to_owned())));
    assert!(states.contains(&("missing-child".to_owned(), "pending".to_owned())));
    assert!(states.contains(&("conflict-child".to_owned(), "conflict".to_owned())));
    drop(connection);

    let reopened = UsageStore::open(&path).expect("reopen replay archive");
    assert_eq!(
        reopened
            .replay_quality(revision.id())
            .expect("reopened quality"),
        quality
    );
}

#[test]
fn mismatched_duplicate_observation_rolls_back_the_complete_replay_append() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("duplicate-rollback-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(6))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([6; 32])].into_boxed_slice())
        .expect("manifest");
    let revision = store
        .begin_replay_revision(&manifest)
        .expect("begin replay revision");
    let first = replay_event_at(6, "root", None, 0, 10, Some(100), false, 1_000);
    let mismatched = replay_event_at(6, "root", None, 0, 10, Some(100), false, 2_000);
    assert_eq!(first.fingerprint(), mismatched.fingerprint());
    let batch = replay_append(6, revision.id(), revision.epoch(), vec![first, mismatched]);

    let error = store
        .apply_replay_append_batch(&batch)
        .expect_err("mismatched duplicate must fail");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
    let quality = store
        .replay_quality(revision.id())
        .expect("rolled-back quality");
    assert_eq!(quality.total(), 0);
    drop(store);

    let connection = Connection::open(&path).expect("inspect rolled-back replay append");
    let persisted: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_observation WHERE generation = 1),
               (SELECT count(*) FROM usage_replay_observation),
               (SELECT count(*) FROM usage_replay_selection),
               (SELECT evidence_epoch FROM usage_replay_revision WHERE revision_id = 0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("rolled-back replay state");
    assert_eq!(persisted, (0, 0, 0, 0));
    let committed_offset: i64 = connection
        .query_row(
            "SELECT committed_offset FROM usage_generation
             WHERE generation = 1 AND status = 'staging'",
            [],
            |row| row.get(0),
        )
        .expect("rolled-back staging checkpoint");
    assert_eq!(committed_offset, 0);
}

#[test]
fn replay_append_rejects_parent_facts_from_a_different_accounting_version() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("parent-version-mismatch-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    for seed in [9_u8, 1_u8] {
        store
            .register_source(&registration(seed))
            .expect("registered source");
    }
    let manifest = ReplayManifest::new(
        vec![
            SourceKey::from_bytes([9; 32]),
            SourceKey::from_bytes([1; 32]),
        ]
        .into_boxed_slice(),
    )
    .expect("manifest");
    let revision = store
        .begin_replay_revision(&manifest)
        .expect("begin replay revision");
    let epoch = store
        .apply_replay_append_batch(&replay_append(
            9,
            revision.id(),
            revision.epoch(),
            vec![replay_event(9, "parent", None, 0, 10, Some(100), false)],
        ))
        .expect("append parent");
    drop(store);

    let connection = Connection::open(&path).expect("tamper private replay version");
    connection
        .execute(
            "UPDATE usage_replay_observation
             SET fingerprint_version = fingerprint_version + 1
             WHERE revision_id = 0 AND session_id = 'parent'",
            [],
        )
        .expect("tamper persisted parent version");
    drop(connection);

    let mut reopened = UsageStore::open(&path).expect("reopen usage store");
    let error = reopened
        .apply_replay_append_batch(&replay_append(
            1,
            revision.id(),
            epoch,
            vec![replay_event(
                1,
                "child",
                Some("parent"),
                0,
                10,
                Some(100),
                false,
            )],
        ))
        .expect_err("mixed accounting versions must fail closed");
    assert_eq!(error.code(), StoreErrorCode::AccountingVersionMismatch);
    drop(reopened);

    let connection = Connection::open(&path).expect("inspect rolled-back child append");
    let state: (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT evidence_epoch FROM usage_replay_revision WHERE revision_id = 0),
               (SELECT committed_offset FROM usage_generation
                WHERE file_key = ?1 AND generation = 1 AND status = 'staging'),
               (SELECT count(*) FROM usage_replay_observation
                WHERE revision_id = 0 AND session_id = 'child')",
            [[1_u8; 32].as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("rolled-back child state");
    assert_eq!(state, (1, 0, 0));
}

#[test]
fn eligible_selection_uses_the_smallest_source_key_for_equal_fingerprints() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("selection-order-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    for seed in [9_u8, 1_u8] {
        store
            .register_source(&registration(seed))
            .expect("registered source");
    }
    let manifest = ReplayManifest::new(
        vec![
            SourceKey::from_bytes([9; 32]),
            SourceKey::from_bytes([1; 32]),
        ]
        .into_boxed_slice(),
    )
    .expect("manifest");
    let revision = store
        .begin_replay_revision(&manifest)
        .expect("begin replay revision");
    let first_event = replay_event(9, "root", None, 0, 10, Some(100), false);
    let fingerprint = *first_event.fingerprint().as_bytes();
    let epoch = store
        .apply_replay_append_batch(&replay_append(
            9,
            revision.id(),
            revision.epoch(),
            vec![first_event],
        ))
        .expect("append larger source key");
    let second_event = replay_event(1, "root", None, 0, 10, Some(100), false);
    assert_eq!(second_event.fingerprint().as_bytes(), &fingerprint);
    store
        .apply_replay_append_batch(&replay_append(1, revision.id(), epoch, vec![second_event]))
        .expect("append smaller source key");
    drop(store);

    let connection = Connection::open(&path).expect("inspect deterministic selection");
    let selected_key: Vec<u8> = connection
        .query_row(
            "SELECT file_key FROM usage_replay_selection
             WHERE revision_id = 0 AND fingerprint = ?1",
            [fingerprint.as_slice()],
            |row| row.get(0),
        )
        .expect("selected source key");
    assert_eq!(selected_key, [1_u8; 32]);
}
