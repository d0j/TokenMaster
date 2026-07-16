use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_accounting::{
    CANONICALIZER_VERSION, CanonicalUsageEvent, Canonicalizer, EVENT_FINGERPRINT_VERSION,
    REPLAY_SIGNATURE_VERSION,
};
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, SessionRelationDraft, SessionRelationDraftParts, TokenCount,
    TokenUsage, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, ArchiveMode, GenerationStatus, MAX_APPEND_RELATIONS,
    ReplayAppendBatch, ReplayAppendBatchParts, ReplayManifest, ReplayRelation,
    ReplayRevisionStatus, ScanCounters, ScanOutcome, ScanScope, ScanSetId, ScanSetManifest,
    SourceKey, SourceKind, SourceRegistration, SourceRegistrationParts, StoreErrorCode,
    StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk, StoredVerification, UsageStore,
};

fn registration(seed: u8) -> SourceRegistration {
    registration_in_scope(seed, "codex", "default")
}

fn registration_in_scope(seed: u8, provider_id: &str, profile_id: &str) -> SourceRegistration {
    SourceRegistration::new(SourceRegistrationParts {
        source_key: SourceKey::from_bytes([seed; 32]),
        provider_id: provider_id.into(),
        profile_id: profile_id.into(),
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

fn finish_codex_scan(
    store: &mut UsageStore,
    observed: &[SourceKey],
    outcome: ScanOutcome,
    started_at_ms: i64,
) -> ScanSetId {
    let manifest = ScanSetManifest::new(
        vec![ScanScope::new("codex", "default").expect("scan scope")].into_boxed_slice(),
    )
    .expect("scan manifest");
    let scan_set = store
        .begin_scan_set(&manifest, started_at_ms)
        .expect("begin scan set");
    let scan = store.scan_page(scan_set.id(), None, 1).expect("scan page")[0].id();
    for source_key in observed {
        store
            .observe_scan_source(scan, *source_key)
            .expect("observe scan source");
    }
    store
        .finish_scan(scan, outcome, started_at_ms + 10, ScanCounters::default())
        .expect("finish child scan");
    let finished = store
        .finish_scan_set(scan_set.id(), started_at_ms + 20)
        .expect("finish scan set");
    assert_eq!(finished.outcome(), Some(outcome));
    scan_set.id()
}

fn source_key_for_index(index: u32) -> SourceKey {
    let mut bytes = [0_u8; 32];
    bytes[..4].copy_from_slice(&index.to_be_bytes());
    SourceKey::from_bytes(bytes)
}

fn digest_for_index(index: u32, tag: u8) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..4].copy_from_slice(&index.to_be_bytes());
    bytes[4] = tag;
    bytes
}

fn registration_for_index(index: u32) -> SourceRegistration {
    SourceRegistration::new(SourceRegistrationParts {
        source_key: source_key_for_index(index),
        provider_id: "codex".into(),
        profile_id: "large-fixture".into(),
        source_id: format!("fixture-{index}").into_boxed_str(),
        source_kind: SourceKind::Active,
        logical_identity: digest_for_index(index, 1),
        physical_identity: Some(digest_for_index(index, 2)),
        initial_checkpoint: StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some(digest_for_index(index, 2)),
            logical_identity: digest_for_index(index, 1),
            committed_offset: 0,
            scan_offset: 0,
            observed_file_length: 0,
            modified_time_ns: None,
            anchor_start: 0,
            anchor_len: 0,
            anchor_sha256: digest_for_index(index, 3),
            resume: Box::default(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::Incremental,
        })
        .expect("valid large-fixture checkpoint"),
    })
    .expect("valid large-fixture registration")
}

fn empty_replay_append_for_index(
    index: u32,
    revision_id: tokenmaster_store::ReplayRevisionId,
    epoch: tokenmaster_store::ReplayEpoch,
) -> ReplayAppendBatch {
    let append = AppendBatch::new(AppendBatchParts {
        source_key: source_key_for_index(index),
        expected_generation: 1,
        expected_committed_offset: 0,
        expected_scan_offset: 0,
        events: Box::default(),
        previous_partial_chunk: None,
        chunk_updates: Box::default(),
        next_checkpoint: StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some(digest_for_index(index, 2)),
            logical_identity: digest_for_index(index, 1),
            committed_offset: 0,
            scan_offset: 0,
            observed_file_length: 0,
            modified_time_ns: None,
            anchor_start: 0,
            anchor_len: 0,
            anchor_sha256: digest_for_index(index, 3),
            resume: Box::default(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::FullPrefix,
        })
        .expect("valid empty replay checkpoint"),
        diagnostic_count_delta: 0,
    })
    .expect("valid empty replay append");
    ReplayAppendBatch::new(ReplayAppendBatchParts {
        revision_id,
        expected_epoch: epoch,
        append_batch: append,
        relations: Box::default(),
    })
    .expect("valid empty replay append")
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
        reported_cost: None,
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
    replay_append_to(seed, revision, epoch, events, 100)
}

fn replay_append_to(
    seed: u8,
    revision: tokenmaster_store::ReplayRevisionId,
    epoch: tokenmaster_store::ReplayEpoch,
    events: Vec<CanonicalUsageEvent>,
    next_offset: u64,
) -> ReplayAppendBatch {
    replay_append_generation(seed, revision, epoch, 1, events, next_offset)
}

fn replay_append_generation(
    seed: u8,
    revision: tokenmaster_store::ReplayRevisionId,
    epoch: tokenmaster_store::ReplayEpoch,
    generation: u64,
    events: Vec<CanonicalUsageEvent>,
    next_offset: u64,
) -> ReplayAppendBatch {
    replay_append_generation_with_relations(
        seed,
        revision,
        epoch,
        generation,
        events,
        next_offset,
        Box::default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn replay_append_generation_with_relations(
    seed: u8,
    revision: tokenmaster_store::ReplayRevisionId,
    epoch: tokenmaster_store::ReplayEpoch,
    generation: u64,
    events: Vec<CanonicalUsageEvent>,
    next_offset: u64,
    relations: Box<[SessionRelationDraft]>,
) -> ReplayAppendBatch {
    let append = AppendBatch::new(AppendBatchParts {
        source_key: SourceKey::from_bytes([seed; 32]),
        expected_generation: generation,
        expected_committed_offset: 0,
        expected_scan_offset: 0,
        events: events.into_boxed_slice(),
        previous_partial_chunk: None,
        chunk_updates: vec![
            StoredSourceChunk::new(
                0,
                u32::try_from(next_offset).expect("fixture chunk length"),
                [seed.wrapping_add(3); 32],
            )
            .expect("source chunk"),
        ]
        .into_boxed_slice(),
        next_checkpoint: checkpoint(seed, next_offset, StoredVerification::FullPrefix),
        diagnostic_count_delta: 0,
    })
    .expect("append batch");
    ReplayAppendBatch::new(ReplayAppendBatchParts {
        revision_id: revision,
        expected_epoch: epoch,
        append_batch: append,
        relations,
    })
    .expect("valid replay append")
}

fn replay_relation(
    seed: u8,
    revision: tokenmaster_store::ReplayRevisionId,
    epoch: tokenmaster_store::ReplayEpoch,
    session: &str,
    parent: &str,
    source_offset: u64,
    declared_conflict: bool,
) -> ReplayRelation {
    let draft = replay_relation_draft(seed, session, parent, source_offset, declared_conflict);
    ReplayRelation::new(revision, epoch, SourceKey::from_bytes([seed; 32]), &draft)
        .expect("replay relation")
}

fn replay_relation_draft(
    seed: u8,
    session: &str,
    parent: &str,
    source_offset: u64,
    declared_conflict: bool,
) -> SessionRelationDraft {
    SessionRelationDraft::new(SessionRelationDraftParts {
        provider_id: UsageProviderId::new("codex").expect("provider"),
        profile_id: UsageProfileId::new("default").expect("profile"),
        session_id: UsageSessionId::new(session).expect("session"),
        parent_session_id: UsageSessionId::new(parent).expect("parent"),
        declared_conflict,
        source_id: UsageSourceId::new(format!("fixture-{seed}")).expect("source"),
        source_offset,
    })
    .expect("session relation")
}

fn seal_ready_store(
    path: &std::path::Path,
    seed: u8,
) -> (
    UsageStore,
    tokenmaster_store::ReplayRevisionSnapshot,
    tokenmaster_store::ReplayEpoch,
) {
    let mut store = UsageStore::open(path).expect("usage store");
    store
        .register_source(&registration(seed))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([seed; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let epoch = store
        .apply_replay_append_batch(&replay_append(
            seed,
            revision.id(),
            revision.epoch(),
            vec![replay_event(
                seed,
                "seal-root",
                None,
                0,
                10,
                Some(100),
                false,
            )],
        ))
        .expect("append seal fixture");
    (store, revision, epoch)
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
fn replay_append_relations_share_one_transaction_and_one_epoch_increment() {
    let seed = 5_u8;
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(seed))
        .expect("register source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([seed; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay");
    let batch = replay_append_generation_with_relations(
        seed,
        revision.id(),
        revision.epoch(),
        1,
        vec![
            replay_event(seed, "child-a", None, 0, 10, Some(100), false),
            replay_event(seed, "child-b", None, 0, 20, Some(100), false),
        ],
        100,
        vec![
            replay_relation_draft(seed, "child-a", "missing-a", 10, false),
            replay_relation_draft(seed, "child-b", "missing-b", 20, false),
        ]
        .into_boxed_slice(),
    );

    let epoch = store
        .apply_replay_append_batch(&batch)
        .expect("atomic event and relation batch");
    assert_eq!(epoch.get(), revision.epoch().get() + 1);
    let seal_error = store
        .seal_replay_revision(revision.id(), epoch)
        .expect_err("relation work must block seal");
    assert_eq!(seal_error.code(), StoreErrorCode::PendingContinuation);
}

#[test]
fn replay_append_relation_count_is_hard_bounded() {
    let seed = 4_u8;
    let append = AppendBatch::new(AppendBatchParts {
        source_key: SourceKey::from_bytes([seed; 32]),
        expected_generation: 1,
        expected_committed_offset: 0,
        expected_scan_offset: 0,
        events: Box::default(),
        previous_partial_chunk: None,
        chunk_updates: vec![StoredSourceChunk::new(0, 100, [7; 32]).expect("chunk")]
            .into_boxed_slice(),
        next_checkpoint: checkpoint(seed, 100, StoredVerification::FullPrefix),
        diagnostic_count_delta: 0,
    })
    .expect("append batch");
    let relation = replay_relation_draft(seed, "child", "parent", 10, false);

    let error = ReplayAppendBatch::new(ReplayAppendBatchParts {
        revision_id: tokenmaster_store::ReplayRevisionId::new(1).expect("revision"),
        expected_epoch: tokenmaster_store::ReplayEpoch::new(1).expect("epoch"),
        append_batch: append,
        relations: vec![relation; MAX_APPEND_RELATIONS + 1].into_boxed_slice(),
    })
    .expect_err("oversized relation batch");

    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_APPEND_RELATIONS as u64));
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
    assert_eq!(revision.scan_set_id(), None);
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
fn scan_bound_begin_rejects_partial_and_stages_exact_present_membership() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("scan-bound-begin-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    for seed in [1_u8, 2_u8] {
        store
            .register_source(&registration(seed))
            .expect("register source");
    }

    let partial = finish_codex_scan(&mut store, &[], ScanOutcome::Partial, 10_000);
    let partial_error = store
        .begin_replay_revision_for_scan_set(partial)
        .expect_err("partial scan set cannot authorize replay");
    assert_eq!(partial_error.code(), StoreErrorCode::IncompleteManifest);

    let present = SourceKey::from_bytes([1_u8; 32]);
    let complete = finish_codex_scan(&mut store, &[present], ScanOutcome::Complete, 11_000);
    store
        .register_source(&registration(3))
        .expect("register source after complete scan");
    let revision = store
        .begin_replay_revision_for_scan_set(complete)
        .expect("begin scan-bound replay");
    assert_eq!(revision.scan_set_id(), Some(complete));
    assert_eq!(revision.expected_source_count(), 1);
    drop(store);

    let connection = Connection::open(&path).expect("inspect exact membership");
    let keys = connection
        .prepare("SELECT file_key FROM usage_replay_source ORDER BY file_key")
        .expect("prepare replay membership")
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .expect("query replay membership")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect replay membership");
    assert_eq!(keys, vec![present.as_bytes().to_vec()]);
}

#[test]
fn scan_bound_begin_composes_same_profile_across_multiple_providers() {
    let mut store = UsageStore::in_memory().expect("usage store");
    let codex = SourceKey::from_bytes([6_u8; 32]);
    let hermes = SourceKey::from_bytes([7_u8; 32]);
    store
        .register_source(&registration_in_scope(6, "codex", "default"))
        .expect("register Codex source");
    store
        .register_source(&registration_in_scope(7, "hermes", "default"))
        .expect("register Hermes source");
    let manifest = ScanSetManifest::new(
        vec![
            ScanScope::new("hermes", "default").expect("Hermes scope"),
            ScanScope::new("codex", "default").expect("Codex scope"),
        ]
        .into_boxed_slice(),
    )
    .expect("multi-provider scan manifest");
    let scan_set = store.begin_scan_set(&manifest, 15_000).expect("begin scan");
    for scan in store
        .scan_page(scan_set.id(), None, usize::MAX)
        .expect("scan scopes")
    {
        let source = match scan.scope().provider_id() {
            "codex" => codex,
            "hermes" => hermes,
            _ => panic!("unexpected provider in bounded fixture"),
        };
        store
            .observe_scan_source(scan.id(), source)
            .expect("observe exact provider source");
        store
            .finish_scan(
                scan.id(),
                ScanOutcome::Complete,
                15_010,
                ScanCounters::default(),
            )
            .expect("finish provider scan");
    }
    store
        .finish_scan_set(scan_set.id(), 15_020)
        .expect("finish multi-provider set");

    let revision = store
        .begin_replay_revision_for_scan_set(scan_set.id())
        .expect("begin multi-provider replay");
    assert_eq!(revision.scan_set_id(), Some(scan_set.id()));
    assert_eq!(revision.expected_source_count(), 2);
}

#[test]
fn scan_bound_begin_rejects_parent_completed_before_its_child() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("scan-bound-time-tamper-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(8))
        .expect("register source");
    let source = SourceKey::from_bytes([8_u8; 32]);
    let scan_set = finish_codex_scan(&mut store, &[source], ScanOutcome::Complete, 16_000);
    drop(store);
    Connection::open(&path)
        .expect("open time tamper")
        .execute(
            "UPDATE usage_scan_set SET completed_at_ms = 16005 WHERE scan_set_id = ?1",
            [scan_set.get() as i64],
        )
        .expect("tamper parent completion time");

    let error = UsageStore::open(&path)
        .expect("reopen time-tampered archive")
        .begin_replay_revision_for_scan_set(scan_set)
        .expect_err("parent cannot complete before its child");
    assert_eq!(error.code(), StoreErrorCode::IncompleteManifest);
}

#[test]
fn scan_bound_seal_rejects_membership_changed_by_a_later_complete_scan() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("scan-bound-stale-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(4))
        .expect("register source");
    let source = SourceKey::from_bytes([4_u8; 32]);
    let authority = finish_codex_scan(&mut store, &[source], ScanOutcome::Complete, 20_000);
    let revision = store
        .begin_replay_revision_for_scan_set(authority)
        .expect("begin scan-bound replay");
    let epoch = store
        .apply_replay_append_batch(&replay_append(4, revision.id(), revision.epoch(), vec![]))
        .expect("complete staged source");

    finish_codex_scan(&mut store, &[], ScanOutcome::Complete, 21_000);
    let error = store
        .seal_replay_revision(revision.id(), epoch)
        .expect_err("changed membership must invalidate old scan authority");
    assert_eq!(error.code(), StoreErrorCode::IncompleteManifest);
}

#[test]
fn zero_source_scan_bound_revision_promotes_retained_truth_without_generation_loss() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("scan-bound-zero-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(5))
        .expect("register source");
    let first = store
        .begin_replay_revision_all_sources()
        .expect("begin initial replay");
    let first_epoch = store
        .apply_replay_append_batch(&replay_append(
            5,
            first.id(),
            first.epoch(),
            vec![replay_event(
                5,
                "retained-root",
                None,
                0,
                10,
                Some(100),
                false,
            )],
        ))
        .expect("append initial replay");
    let first_sealed = store
        .seal_replay_revision(first.id(), first_epoch)
        .expect("seal initial replay");
    let first_current = store
        .promote_replay_revision(first.id(), first_sealed.epoch())
        .expect("promote initial replay");

    let empty_authority = finish_codex_scan(&mut store, &[], ScanOutcome::Complete, 30_000);
    let empty = store
        .begin_replay_revision_for_scan_set(empty_authority)
        .expect("begin zero-source replay");
    assert_eq!(empty.scan_set_id(), Some(empty_authority));
    assert_eq!(empty.expected_source_count(), 0);
    drop(store);
    let mut store = UsageStore::open(&path).expect("reopen zero-source staging");
    let sealed = store
        .seal_replay_revision(empty.id(), empty.epoch())
        .expect("seal zero-source replay");
    let promoted = store
        .promote_replay_revision(empty.id(), sealed.epoch())
        .expect("promote retention-only replay");
    assert_eq!(promoted.scan_set_id(), Some(empty_authority));
    drop(store);

    let state: (i64, i64, i64, i64, i64, i64) = Connection::open(&path)
        .expect("inspect retained state")
        .query_row(
            "SELECT
               (SELECT current_generation FROM usage_source WHERE file_key = ?1),
               (SELECT missing FROM usage_source WHERE file_key = ?1),
               (SELECT count(*) FROM usage_generation WHERE file_key = ?1),
               (SELECT projection_revision_id FROM usage_event),
               (SELECT origin_revision_id FROM usage_event),
               (SELECT retained FROM usage_event)",
            [SourceKey::from_bytes([5_u8; 32]).as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("retained state");
    assert_eq!(state.0, 1, "missing source keeps its current generation");
    assert_eq!(state.1, 1);
    assert_eq!(
        state.2, 1,
        "zero-source replay creates no staging generation"
    );
    assert_eq!(state.3, promoted.id().get() as i64);
    assert_eq!(state.4, first_current.id().get() as i64);
    assert_eq!(state.5, 1);
}

#[test]
fn all_source_begin_stages_three_hundred_sources_without_a_manifest_vector() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("all-source-begin-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    for index in 0..300 {
        store
            .register_source(&registration_for_index(index))
            .expect("register large fixture source");
    }
    let page_before = store
        .event_page_before(None, 256)
        .expect("canonical page before all-source begin");
    let counts_before = store.counts().expect("counts before all-source begin");

    let revision = store
        .begin_replay_revision_all_sources()
        .expect("begin all-source revision");
    assert_eq!(revision.expected_source_count(), 300_u64);
    assert_eq!(revision.status(), ReplayRevisionStatus::Staging);
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("canonical page after all-source begin"),
        page_before
    );
    let counts_after = store.counts().expect("counts after all-source begin");
    assert_eq!(counts_after.sources(), counts_before.sources());
    assert_eq!(
        counts_after.generations(),
        counts_before.generations() + 300
    );
    assert_eq!(counts_after.observations(), counts_before.observations());
    assert_eq!(
        counts_after.canonical_events(),
        counts_before.canonical_events()
    );
    assert_eq!(counts_after.chunks(), counts_before.chunks());
    assert_eq!(counts_after.scans(), counts_before.scans());

    let error = store
        .begin_replay_revision_all_sources()
        .expect_err("second all-source staging begin must fail");
    assert_eq!(error.code(), StoreErrorCode::ArchiveModeMismatch);
    drop(store);

    let connection = Connection::open(&path).expect("inspect all-source staging");
    let state: (i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_source),
               (SELECT count(*) FROM usage_source WHERE current_generation = 0),
               (SELECT count(*) FROM usage_generation WHERE status = 'current'),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
               (SELECT count(*) FROM usage_replay_source)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("all-source staging state");
    assert_eq!(state, (300, 300, 300, 300, 300));
    drop(connection);
    let reopened = UsageStore::open(&path).expect("reopen all-source staging archive");
    assert!(
        reopened
            .archive_state()
            .expect("reopened archive state")
            .rebuild_staging()
    );
}

#[test]
fn replay_generation_snapshot_is_exact_staging_revision_state() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store.register_source(&registration(1)).expect("source 1");
    store.register_source(&registration(2)).expect("source 2");
    let revision = store
        .begin_replay_revision_all_sources()
        .expect("begin replay revision");

    let snapshot = store
        .replay_generation_snapshot(revision.id(), SourceKey::from_bytes([1; 32]))
        .expect("exact staging snapshot");
    assert_eq!(snapshot.source_key(), SourceKey::from_bytes([1; 32]));
    assert_eq!(snapshot.generation(), 1);
    assert_eq!(snapshot.status(), GenerationStatus::Staging);
    assert_eq!(snapshot.checkpoint().committed_offset(), 0);
    let debug = format!("{snapshot:?}");
    assert!(!debug.contains("private"));
    assert!(!debug.contains("[1, 1"));

    let wrong_source = store
        .replay_generation_snapshot(revision.id(), SourceKey::from_bytes([9; 32]))
        .expect_err("unowned source must not fall back to current");
    assert_eq!(wrong_source.code(), StoreErrorCode::StaleRevision);
    let wrong_revision = store
        .replay_generation_snapshot(
            tokenmaster_store::ReplayRevisionId::new(999).expect("bounded revision"),
            SourceKey::from_bytes([1; 32]),
        )
        .expect_err("wrong revision must not expose staging state");
    assert_eq!(wrong_revision.code(), StoreErrorCode::StaleRevision);

    store
        .discard_replay_revision(revision.id(), revision.epoch())
        .expect("discard exact staging revision");
    let discarded = store
        .replay_generation_snapshot(revision.id(), SourceKey::from_bytes([1; 32]))
        .expect_err("discarded revision must be stale");
    assert_eq!(discarded.code(), StoreErrorCode::StaleRevision);

    let mut promoted_store = UsageStore::in_memory().expect("promoted usage store");
    promoted_store
        .register_source(&registration(4))
        .expect("promoted source");
    let promoted_revision = promoted_store
        .begin_replay_revision_all_sources()
        .expect("begin promoted revision");
    let promoted_epoch = promoted_store
        .apply_replay_append_batch(&replay_append(
            4,
            promoted_revision.id(),
            promoted_revision.epoch(),
            vec![replay_event(4, "promoted", None, 0, 10, Some(100), false)],
        ))
        .expect("complete promoted source");
    let sealed = promoted_store
        .seal_replay_revision(promoted_revision.id(), promoted_epoch)
        .expect("seal promoted revision");
    let sealed_error = promoted_store
        .replay_generation_snapshot(promoted_revision.id(), SourceKey::from_bytes([4; 32]))
        .expect_err("sealed staging state must not be resumed");
    assert_eq!(sealed_error.code(), StoreErrorCode::StaleRevision);
    promoted_store
        .promote_replay_revision(promoted_revision.id(), sealed.epoch())
        .expect("promote revision");
    let promoted_error = promoted_store
        .replay_generation_snapshot(promoted_revision.id(), SourceKey::from_bytes([4; 32]))
        .expect_err("promoted revision must not expose staging state");
    assert_eq!(promoted_error.code(), StoreErrorCode::StaleRevision);
}

#[test]
fn prepare_replay_source_rebinds_only_untouched_staging_by_epoch() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store.register_source(&registration(5)).expect("source");
    let current_before = store
        .generation_snapshot(SourceKey::from_bytes([5; 32]))
        .expect("current snapshot")
        .expect("current generation");
    let revision = store
        .begin_replay_revision_all_sources()
        .expect("begin replay revision");
    let prepared_checkpoint = StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 2,
        physical_identity: Some([99; 32]),
        logical_identity: [6; 32],
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: 0,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [0; 32],
        resume: br#"{"provider":"empty-v2"}"#.to_vec().into_boxed_slice(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    })
    .expect("prepared zero checkpoint");

    let prepared_epoch = store
        .prepare_replay_source(
            revision.id(),
            revision.epoch(),
            SourceKey::from_bytes([5; 32]),
            &prepared_checkpoint,
        )
        .expect("prepare untouched staging source");
    assert_eq!(prepared_epoch.get(), revision.epoch().get() + 1);
    let staging = store
        .replay_generation_snapshot(revision.id(), SourceKey::from_bytes([5; 32]))
        .expect("prepared staging snapshot");
    assert_eq!(staging.checkpoint().parser_schema_version(), 2);
    assert_eq!(staging.checkpoint().physical_identity(), Some(&[99; 32]));
    assert_eq!(staging.checkpoint().resume(), br#"{"provider":"empty-v2"}"#);
    assert_eq!(
        store
            .generation_snapshot(SourceKey::from_bytes([5; 32]))
            .expect("current snapshot after prepare")
            .expect("current generation after prepare"),
        current_before
    );

    let stale = store
        .prepare_replay_source(
            revision.id(),
            revision.epoch(),
            SourceKey::from_bytes([5; 32]),
            &prepared_checkpoint,
        )
        .expect_err("stale preparation epoch must fail");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);

    let wrong_logical = StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 2,
        physical_identity: Some([99; 32]),
        logical_identity: [88; 32],
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: 0,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [0; 32],
        resume: Box::default(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    })
    .expect("bounded wrong-logical checkpoint");
    let mismatch = store
        .prepare_replay_source(
            revision.id(),
            prepared_epoch,
            SourceKey::from_bytes([5; 32]),
            &wrong_logical,
        )
        .expect_err("logical identity cannot be rebound");
    assert_eq!(mismatch.code(), StoreErrorCode::StaleCheckpoint);
    let after_mismatch = store
        .replay_generation_snapshot(revision.id(), SourceKey::from_bytes([5; 32]))
        .expect("staging after rejected preparation");
    assert_eq!(after_mismatch, staging);
}

#[test]
fn prepare_replay_source_rejects_completed_or_non_incremental_state_without_writes() {
    let mut touched = UsageStore::in_memory().expect("touched store");
    touched.register_source(&registration(6)).expect("source");
    let revision = touched
        .begin_replay_revision_all_sources()
        .expect("begin touched revision");
    let empty = StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([6; 32]),
        logical_identity: [7; 32],
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: 0,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [0; 32],
        resume: Box::default(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    })
    .expect("empty provider checkpoint");
    let prepared = touched
        .prepare_replay_source(
            revision.id(),
            revision.epoch(),
            SourceKey::from_bytes([6; 32]),
            &empty,
        )
        .expect("prepare source");
    let appended = touched
        .apply_replay_append_batch(&replay_append(
            6,
            revision.id(),
            prepared,
            vec![replay_event(6, "touched", None, 0, 10, Some(100), false)],
        ))
        .expect("touch staging generation");
    let before_retry = touched
        .replay_generation_snapshot(revision.id(), SourceKey::from_bytes([6; 32]))
        .expect("touched staging snapshot");
    let retry = touched
        .prepare_replay_source(
            revision.id(),
            appended,
            SourceKey::from_bytes([6; 32]),
            &empty,
        )
        .expect_err("touched staging source cannot be rebound");
    assert_eq!(retry.code(), StoreErrorCode::StaleCheckpoint);
    assert_eq!(
        touched
            .replay_generation_snapshot(revision.id(), SourceKey::from_bytes([6; 32]))
            .expect("snapshot after rejected retry"),
        before_retry
    );

    let mut invalid = UsageStore::in_memory().expect("invalid store");
    invalid.register_source(&registration(8)).expect("source");
    let invalid_revision = invalid
        .begin_replay_revision_all_sources()
        .expect("begin invalid revision");
    let full_prefix = StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([8; 32]),
        logical_identity: [9; 32],
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: 0,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [0; 32],
        resume: Box::default(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::FullPrefix,
    })
    .expect("bounded full-prefix checkpoint");
    let before_invalid = invalid
        .replay_generation_snapshot(invalid_revision.id(), SourceKey::from_bytes([8; 32]))
        .expect("staging before invalid preparation");
    let error = invalid
        .prepare_replay_source(
            invalid_revision.id(),
            invalid_revision.epoch(),
            SourceKey::from_bytes([8; 32]),
            &full_prefix,
        )
        .expect_err("full-prefix preparation must fail");
    assert_eq!(error.code(), StoreErrorCode::InvalidValue);
    assert_eq!(
        invalid
            .replay_generation_snapshot(invalid_revision.id(), SourceKey::from_bytes([8; 32]),)
            .expect("staging after invalid preparation"),
        before_invalid
    );
}

#[test]
fn source_chunk_reads_one_exact_proof_and_validates_bounds() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store.register_source(&registration(3)).expect("source");
    let revision = store
        .begin_replay_revision_all_sources()
        .expect("begin replay revision");
    store
        .apply_replay_append_batch(&replay_append(
            3,
            revision.id(),
            revision.epoch(),
            vec![replay_event(3, "chunk", None, 0, 10, Some(100), false)],
        ))
        .expect("append chunk fixture");

    let chunk = store
        .source_chunk(SourceKey::from_bytes([3; 32]), 1, 0)
        .expect("chunk lookup")
        .expect("exact chunk");
    assert_eq!(chunk.index(), 0);
    assert_eq!(chunk.covered_len(), 100);
    assert_eq!(chunk.sha256(), &[6; 32]);
    assert_eq!(
        store
            .source_chunk(SourceKey::from_bytes([3; 32]), 1, 1)
            .expect("absent chunk lookup"),
        None
    );
    assert_eq!(
        store
            .source_chunk(SourceKey::from_bytes([3; 32]), 0, 0)
            .expect("wrong generation lookup"),
        None
    );
    let overflow = store
        .source_chunk(SourceKey::from_bytes([3; 32]), i64::MAX as u64 + 1, 0)
        .expect_err("SQLite generation overflow must fail before query");
    assert_eq!(overflow.code(), StoreErrorCode::InvalidValue);
}

#[test]
fn source_chunk_fails_closed_on_invalid_stored_shape() {
    for (name, tamper) in [
        (
            "zero-length",
            "UPDATE usage_source_chunk SET covered_len = 0 WHERE generation = 1",
        ),
        (
            "short-digest",
            "UPDATE usage_source_chunk SET sha256 = zeroblob(31) WHERE generation = 1",
        ),
    ] {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join(format!("chunk-{name}.sqlite3"));
        let (ready, _, _) = seal_ready_store(&path, 7);
        drop(ready);
        let connection = Connection::open(&path).expect("open corruption fixture");
        connection
            .pragma_update(None, "ignore_check_constraints", "ON")
            .expect("allow deliberate corruption fixture");
        connection
            .execute_batch(tamper)
            .expect("tamper exact chunk row");
        drop(connection);
        let reopened = UsageStore::open(&path).expect("reopen corruption fixture");

        let error = reopened
            .source_chunk(SourceKey::from_bytes([7; 32]), 1, 0)
            .expect_err("invalid stored chunk must fail closed");
        assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue, "{name}");
    }
}

#[test]
fn all_source_begin_is_atomic_on_empty_missing_current_and_generation_overflow() {
    let mut empty = UsageStore::in_memory().expect("empty usage store");
    let empty_error = empty
        .begin_replay_revision_all_sources()
        .expect_err("empty all-source begin must fail");
    assert_eq!(empty_error.code(), StoreErrorCode::InvalidValue);
    assert!(
        !empty
            .archive_state()
            .expect("empty archive state")
            .rebuild_staging()
    );

    let directory = TempDir::new().expect("temporary directory");
    let missing_path = directory.path().join("missing-current-private.sqlite3");
    let mut missing_store = UsageStore::open(&missing_path).expect("missing-current store");
    missing_store
        .register_source(&registration_for_index(1))
        .expect("register missing-current source");
    drop(missing_store);
    let missing_connection = Connection::open(&missing_path).expect("damage current pointer");
    missing_connection
        .execute(
            "UPDATE usage_source SET current_generation = NULL WHERE file_key = ?1",
            [source_key_for_index(1).as_bytes().as_slice()],
        )
        .expect("clear current pointer");
    drop(missing_connection);
    let mut missing_store = UsageStore::open(&missing_path).expect("reopen missing-current store");
    let missing_error = missing_store
        .begin_replay_revision_all_sources()
        .expect_err("missing current generation must fail");
    assert_eq!(missing_error.code(), StoreErrorCode::InvalidStoredValue);
    drop(missing_store);
    let missing_connection = Connection::open(&missing_path).expect("inspect missing failure");
    let missing_staging: (i64, i64) = missing_connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("missing failure state");
    assert_eq!(missing_staging, (0, 0));

    let overflow_path = directory.path().join("generation-overflow-private.sqlite3");
    let mut overflow_store = UsageStore::open(&overflow_path).expect("overflow store");
    overflow_store
        .register_source(&registration_for_index(2))
        .expect("register overflow source");
    drop(overflow_store);
    let mut overflow_connection = Connection::open(&overflow_path).expect("open overflow fixture");
    overflow_connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable overflow fixture foreign keys");
    let transaction = overflow_connection
        .transaction()
        .expect("overflow fixture transaction");
    transaction
        .execute(
            "UPDATE usage_source SET current_generation = NULL WHERE file_key = ?1",
            [source_key_for_index(2).as_bytes().as_slice()],
        )
        .expect("clear overflow source pointer");
    transaction
        .execute(
            "UPDATE usage_generation SET generation = ?2 WHERE file_key = ?1",
            params![source_key_for_index(2).as_bytes().as_slice(), i64::MAX],
        )
        .expect("set maximum generation");
    transaction
        .execute(
            "UPDATE usage_source SET current_generation = ?2 WHERE file_key = ?1",
            params![source_key_for_index(2).as_bytes().as_slice(), i64::MAX],
        )
        .expect("select maximum generation");
    transaction.commit().expect("commit overflow fixture");
    drop(overflow_connection);
    let mut overflow_store = UsageStore::open(&overflow_path).expect("reopen overflow store");
    let overflow_error = overflow_store
        .begin_replay_revision_all_sources()
        .expect_err("generation overflow must fail");
    assert_eq!(overflow_error.code(), StoreErrorCode::InvalidStoredValue);
    drop(overflow_store);
    let overflow_connection = Connection::open(&overflow_path).expect("inspect overflow failure");
    let overflow_state: (i64, i64, i64) = overflow_connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
               (SELECT count(*) FROM usage_generation WHERE generation = ?1 AND status = 'current')",
            [i64::MAX],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("overflow failure state");
    assert_eq!(overflow_state, (0, 0, 1));
}

#[test]
fn three_hundred_sources_complete_seal_promote_and_reopen_in_pages() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("large-replay-lifecycle-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("large lifecycle store");
    for index in 0..300 {
        store
            .register_source(&registration_for_index(index))
            .expect("register large lifecycle source");
    }
    let revision = store
        .begin_replay_revision_all_sources()
        .expect("begin large lifecycle revision");
    let mut epoch = revision.epoch();
    for index in 0..300 {
        epoch = store
            .apply_replay_append_batch(&empty_replay_append_for_index(index, revision.id(), epoch))
            .expect("complete large lifecycle source");
    }
    let sealed = store
        .seal_replay_revision(revision.id(), epoch)
        .expect("seal large lifecycle revision");
    let promoted = store
        .promote_replay_revision(revision.id(), sealed.epoch())
        .expect("promote large lifecycle revision");
    assert_eq!(promoted.status(), ReplayRevisionStatus::Current);
    assert_eq!(promoted.expected_source_count(), 300);
    assert!(promoted.sealed());
    assert!(promoted.promoted());
    assert!(
        store
            .event_page_before(None, 256)
            .expect("empty promoted canonical page")
            .is_empty()
    );
    drop(store);

    let connection = Connection::open(&path).expect("inspect promoted large lifecycle");
    let promoted_state: (i64, i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision
                WHERE status = 'current' AND sealed = 1 AND promoted = 1),
               (SELECT expected_source_count FROM usage_replay_revision
                WHERE status = 'current'),
               (SELECT count(*) FROM usage_replay_source),
               (SELECT count(*) FROM usage_source
                WHERE current_generation = 1 AND verification_level = 'full_prefix'),
               (SELECT count(*) FROM usage_generation
                WHERE generation = 1 AND status = 'current'
                  AND verification_level = 'full_prefix'),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
               (SELECT count(*) FROM pragma_foreign_key_check)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .expect("promoted large lifecycle state");
    assert_eq!(promoted_state, (1, 300, 300, 300, 300, 0, 0));
    drop(connection);

    let reopened = UsageStore::open(&path).expect("reopen promoted large lifecycle");
    let archive = reopened.archive_state().expect("reopened archive state");
    assert_eq!(archive.mode(), ArchiveMode::ReplayVerified);
    assert_eq!(archive.active_revision(), Some(revision.id()));
    assert!(!archive.rebuild_staging());
}

#[test]
fn source_registered_after_all_source_begin_blocks_seal_without_mutation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("late-source-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("late-source store");
    for index in 0..300 {
        store
            .register_source(&registration_for_index(index))
            .expect("register initial late-source fixture");
    }
    let revision = store
        .begin_replay_revision_all_sources()
        .expect("begin late-source revision");
    let mut epoch = revision.epoch();
    for index in 0..300 {
        epoch = store
            .apply_replay_append_batch(&empty_replay_append_for_index(index, revision.id(), epoch))
            .expect("complete initial late-source fixture");
    }
    store
        .register_source(&registration_for_index(300))
        .expect("register source after all-source begin");
    drop(store);

    let connection = Connection::open(&path).expect("inspect state before blocked seal");
    let before: (i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT evidence_epoch FROM usage_replay_revision),
               (SELECT sealed FROM usage_replay_revision),
               (SELECT count(*) FROM usage_replay_source),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
               (SELECT count(*) FROM usage_source WHERE current_generation = 0),
               (SELECT count(*) FROM usage_event)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("state before blocked seal");
    drop(connection);
    let mut store = UsageStore::open(&path).expect("reopen before blocked seal");
    let error = store
        .seal_replay_revision(revision.id(), epoch)
        .expect_err("late source must block seal");
    assert_eq!(error.code(), StoreErrorCode::IncompleteManifest);
    drop(store);

    let connection = Connection::open(&path).expect("inspect state after blocked seal");
    let after: (i64, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT evidence_epoch FROM usage_replay_revision),
               (SELECT sealed FROM usage_replay_revision),
               (SELECT count(*) FROM usage_replay_source),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
               (SELECT count(*) FROM usage_source WHERE current_generation = 0),
               (SELECT count(*) FROM usage_event)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("state after blocked seal");
    assert_eq!(after, before);
    drop(connection);

    let mut store = UsageStore::open(&path).expect("reopen for exact discard");
    store
        .discard_replay_revision(revision.id(), epoch)
        .expect("discard blocked late-source revision");
    drop(store);
    let connection = Connection::open(&path).expect("inspect exact discard");
    let discarded: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision),
               (SELECT count(*) FROM usage_generation WHERE status = 'staging'),
               (SELECT count(*) FROM usage_source WHERE current_generation = 0),
               (SELECT count(*) FROM pragma_foreign_key_check)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("discarded late-source state");
    assert_eq!(discarded, (0, 0, 301, 0));
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
fn late_relation_invalidates_root_selection_and_reclassifies_after_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("late-relation-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(4))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([4; 32])].into_boxed_slice())
        .expect("manifest");
    let revision = store
        .begin_replay_revision(&manifest)
        .expect("begin replay revision");
    let epoch = store
        .apply_replay_append_batch(&replay_append(
            4,
            revision.id(),
            revision.epoch(),
            vec![
                replay_event(4, "parent", None, 0, 10, Some(100), false),
                replay_event(4, "child", None, 0, 20, Some(100), false),
            ],
        ))
        .expect("append roots");
    assert_eq!(
        store
            .replay_quality(revision.id())
            .expect("root quality")
            .eligible(),
        2
    );

    let relation_epoch = store
        .apply_replay_relation(&replay_relation(
            4,
            revision.id(),
            epoch,
            "child",
            "parent",
            90,
            false,
        ))
        .expect("apply late relation");
    drop(store);

    let connection = Connection::open(&path).expect("inspect invalidation boundary");
    let invalidated: (i64, String, String, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_selection WHERE revision_id = 0),
               parent_session_id,
               state,
               (SELECT count(*) FROM usage_replay_work
                WHERE revision_id = 0 AND session_id = 'child')
             FROM usage_replay_session
             WHERE revision_id = 0 AND session_id = 'child'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("late relation state");
    assert_eq!(
        invalidated,
        (1, "parent".to_owned(), "matching".to_owned(), 1)
    );
    drop(connection);

    let mut reopened = UsageStore::open(&path).expect("reopen usage store");
    let classified = reopened
        .continue_replay(revision.id(), relation_epoch)
        .expect("continue session classification");
    assert_eq!(classified.processed_count(), 1);
    assert!(classified.remaining_work());
    let drained = reopened
        .continue_replay(revision.id(), classified.epoch())
        .expect("drain child scan");
    assert_eq!(drained.processed_count(), 0);
    assert!(!drained.remaining_work());
    let quality = reopened
        .replay_quality(revision.id())
        .expect("reclassified quality");
    assert_eq!(quality.eligible(), 1);
    assert_eq!(quality.replay(), 1);
    assert_eq!(quality.pending(), 0);
    assert_eq!(quality.conflict(), 0);
}

#[test]
fn stale_and_disagreeing_late_relations_are_atomic_and_conflict_is_permanent() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("late-relation-conflict.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(7))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([7; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let append_epoch = store
        .apply_replay_append_batch(&replay_append(
            7,
            revision.id(),
            revision.epoch(),
            vec![replay_event(7, "child", None, 0, 10, Some(100), false)],
        ))
        .expect("append child root");
    let first_epoch = store
        .apply_replay_relation(&replay_relation(
            7,
            revision.id(),
            append_epoch,
            "child",
            "parent-a",
            90,
            false,
        ))
        .expect("first parent relation");

    let stale = store
        .apply_replay_relation(&replay_relation(
            7,
            revision.id(),
            append_epoch,
            "child",
            "parent-b",
            80,
            false,
        ))
        .expect_err("stale relation must not write");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
    let conflict_epoch = store
        .apply_replay_relation(&replay_relation(
            7,
            revision.id(),
            first_epoch,
            "child",
            "parent-b",
            80,
            false,
        ))
        .expect("disagreeing relation");
    let permanent_epoch = store
        .apply_replay_relation(&replay_relation(
            7,
            revision.id(),
            conflict_epoch,
            "child",
            "parent-a",
            70,
            false,
        ))
        .expect("earlier relation cannot clear conflict");
    let classified = store
        .continue_replay(revision.id(), permanent_epoch)
        .expect("classify permanent conflict");
    assert_eq!(classified.processed_count(), 1);
    let drained = store
        .continue_replay(revision.id(), classified.epoch())
        .expect("drain conflict child scan");
    assert!(!drained.remaining_work());
    assert_eq!(
        store
            .replay_quality(revision.id())
            .expect("conflict quality")
            .conflict(),
        1
    );
    drop(store);

    let connection = Connection::open(&path).expect("inspect permanent relation conflict");
    let state: (String, i64, i64, i64) = connection
        .query_row(
            "SELECT parent_session_id, relation_conflict,
                    (SELECT count(*) FROM usage_replay_selection WHERE revision_id = 0),
                    (SELECT evidence_epoch FROM usage_replay_revision WHERE revision_id = 0)
             FROM usage_replay_session
             WHERE revision_id = 0 AND session_id = 'child'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("permanent conflict state");
    assert_eq!(state, ("parent-a".to_owned(), 1, 0, 6));
}

#[test]
fn stale_persisted_work_epoch_rejects_continuation_without_writes() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("stale-work-epoch.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(6))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([6; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let append_epoch = store
        .apply_replay_append_batch(&replay_append(
            6,
            revision.id(),
            revision.epoch(),
            vec![replay_event(6, "child", None, 0, 10, Some(100), false)],
        ))
        .expect("append child root");
    let relation_epoch = store
        .apply_replay_relation(&replay_relation(
            6,
            revision.id(),
            append_epoch,
            "child",
            "parent",
            90,
            false,
        ))
        .expect("late relation");
    drop(store);

    let connection = Connection::open(&path).expect("tamper work epoch");
    connection
        .execute(
            "UPDATE usage_replay_work SET expected_evidence_epoch = 1
             WHERE revision_id = 0 AND session_id = 'child'",
            [],
        )
        .expect("make work stale");
    drop(connection);

    let mut reopened = UsageStore::open(&path).expect("reopen usage store");
    let error = reopened
        .continue_replay(revision.id(), relation_epoch)
        .expect_err("stale durable work must fail closed");
    assert_eq!(error.code(), StoreErrorCode::StaleRevision);
    drop(reopened);

    let connection = Connection::open(&path).expect("inspect stale-work rollback");
    let state: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT evidence_epoch FROM usage_replay_revision WHERE revision_id = 0),
               disposition,
               (SELECT count(*) FROM usage_replay_selection WHERE revision_id = 0),
               (SELECT expected_evidence_epoch FROM usage_replay_work
                WHERE revision_id = 0 AND session_id = 'child')
             FROM usage_replay_observation
             WHERE revision_id = 0 AND session_id = 'child'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("stale-work state");
    assert_eq!(state, (2, "eligible".to_owned(), 0, 1));
}

#[test]
fn nested_descendants_reclassify_in_session_and_ordinal_order() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("nested-continuation.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(5))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([5; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let append_epoch = store
        .apply_replay_append_batch(&replay_append_to(
            5,
            revision.id(),
            revision.epoch(),
            vec![
                replay_event(5, "parent", None, 0, 1, Some(100), false),
                replay_event(5, "parent", None, 1, 2, Some(110), false),
                replay_event(5, "child", None, 0, 3, Some(100), false),
                replay_event(5, "child", None, 1, 4, Some(110), false),
                replay_event(5, "grandchild", Some("child"), 0, 5, Some(100), false),
            ],
            1_000,
        ))
        .expect("append nested sessions");
    let relation_epoch = store
        .apply_replay_relation(&replay_relation(
            5,
            revision.id(),
            append_epoch,
            "child",
            "parent",
            900,
            false,
        ))
        .expect("late child relation");
    let child_zero = store
        .continue_replay(revision.id(), relation_epoch)
        .expect("classify child ordinal zero");
    assert_eq!(child_zero.processed_count(), 1);
    let child_one = store
        .continue_replay(revision.id(), child_zero.epoch())
        .expect("classify child ordinal one");
    assert_eq!(child_one.processed_count(), 1);
    let child_scan = store
        .continue_replay(revision.id(), child_one.epoch())
        .expect("scan direct grandchild");
    assert_eq!(child_scan.processed_count(), 1);
    let grandchild = store
        .continue_replay(revision.id(), child_scan.epoch())
        .expect("reclassify grandchild");
    assert_eq!(grandchild.processed_count(), 1);
    let drained = store
        .continue_replay(revision.id(), grandchild.epoch())
        .expect("drain nested continuation");
    assert_eq!(drained.processed_count(), 0);
    assert!(!drained.remaining_work());
    drop(store);

    let connection = Connection::open(&path).expect("inspect nested ordering");
    let epochs: Vec<(String, i64, i64)> = connection
        .prepare(
            "SELECT session_id, session_ordinal, evidence_epoch
             FROM usage_replay_observation
             WHERE revision_id = 0 AND session_id IN ('child','grandchild')
             ORDER BY evidence_epoch",
        )
        .expect("prepare evidence ordering")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("query evidence ordering")
        .collect::<Result<_, _>>()
        .expect("collect evidence ordering");
    assert_eq!(
        epochs,
        vec![
            ("child".to_owned(), 0, 3),
            ("child".to_owned(), 1, 4),
            ("grandchild".to_owned(), 0, 6),
        ]
    );
}

#[test]
fn confirmed_relation_cycle_fails_closed_without_continuation_loop() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(3))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([3; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let append_epoch = store
        .apply_replay_append_batch(&replay_append(
            3,
            revision.id(),
            revision.epoch(),
            vec![
                replay_event(3, "cycle-a", None, 0, 10, Some(100), false),
                replay_event(3, "cycle-b", None, 0, 20, Some(100), false),
            ],
        ))
        .expect("append cycle roots");
    let first_epoch = store
        .apply_replay_relation(&replay_relation(
            3,
            revision.id(),
            append_epoch,
            "cycle-a",
            "cycle-b",
            80,
            false,
        ))
        .expect("first cycle edge");
    let mut epoch = store
        .apply_replay_relation(&replay_relation(
            3,
            revision.id(),
            first_epoch,
            "cycle-b",
            "cycle-a",
            90,
            false,
        ))
        .expect("closing cycle edge");
    let mut steps = 0_u8;
    loop {
        let result = store
            .continue_replay(revision.id(), epoch)
            .expect("bounded cycle continuation");
        steps = steps.saturating_add(1);
        epoch = result.epoch();
        if !result.remaining_work() {
            break;
        }
        assert!(steps < 8, "cycle continuation must converge");
    }
    assert_eq!(steps, 4);
    let quality = store.replay_quality(revision.id()).expect("cycle quality");
    assert_eq!(quality.conflict(), 2);
    assert_eq!(quality.eligible(), 0);
}

#[test]
fn first_relation_identity_is_deterministic_across_arrival_order() {
    fn run(order: [u8; 2]) -> (String, Vec<u8>, i64, i64) {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("relation-order.sqlite3");
        let mut store = UsageStore::open(&path).expect("usage store");
        for seed in [9_u8, 1_u8] {
            store
                .register_source(&registration(seed))
                .expect("registered source");
        }
        let revision = store
            .begin_replay_revision(
                &ReplayManifest::new(
                    vec![
                        SourceKey::from_bytes([9; 32]),
                        SourceKey::from_bytes([1; 32]),
                    ]
                    .into_boxed_slice(),
                )
                .expect("manifest"),
            )
            .expect("begin replay revision");
        let first_epoch = store
            .apply_replay_append_batch(&replay_append(
                9,
                revision.id(),
                revision.epoch(),
                vec![replay_event(9, "child", None, 0, 10, Some(100), false)],
            ))
            .expect("append child root");
        let mut epoch = store
            .apply_replay_append_batch(&replay_append(
                1,
                revision.id(),
                first_epoch,
                vec![replay_event(1, "marker", None, 0, 10, Some(100), false)],
            ))
            .expect("advance second source");
        for seed in order {
            let (parent, offset) = if seed == 1 {
                ("parent-one", 80)
            } else {
                ("parent-nine", 90)
            };
            epoch = store
                .apply_replay_relation(&replay_relation(
                    seed,
                    revision.id(),
                    epoch,
                    "child",
                    parent,
                    offset,
                    false,
                ))
                .expect("apply ordered relation");
        }
        drop(store);
        let connection = Connection::open(&path).expect("inspect relation order");
        connection
            .query_row(
                "SELECT parent_session_id, first_relation_file_key,
                        first_relation_source_offset, relation_conflict
                 FROM usage_replay_session
                 WHERE revision_id = 0 AND session_id = 'child'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("deterministic relation state")
    }

    let forward = run([9, 1]);
    let reverse = run([1, 9]);
    assert_eq!(forward, reverse);
    assert_eq!(forward, ("parent-one".to_owned(), vec![1_u8; 32], 80, 1));
}

#[test]
fn fanout_continuation_is_keyset_bounded_and_resumes_after_reopen() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("fanout-continuation.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    for seed in [9_u8, 1_u8, 2_u8] {
        store
            .register_source(&registration(seed))
            .expect("registered source");
    }
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(
                vec![
                    SourceKey::from_bytes([9; 32]),
                    SourceKey::from_bytes([1; 32]),
                    SourceKey::from_bytes([2; 32]),
                ]
                .into_boxed_slice(),
            )
            .expect("manifest"),
        )
        .expect("begin replay revision");
    let parent_epoch = store
        .apply_replay_append_batch(&replay_append_to(
            9,
            revision.id(),
            revision.epoch(),
            vec![
                replay_event(9, "grandparent", None, 0, 1, Some(100), false),
                replay_event(9, "parent", None, 0, 2, Some(100), false),
            ],
            1_000,
        ))
        .expect("append parent roots");
    let children = (0_u64..256)
        .map(|index| {
            replay_event(
                1,
                &format!("child-{index:03}"),
                Some("parent"),
                0,
                index,
                Some(100),
                false,
            )
        })
        .collect();
    let first_children_epoch = store
        .apply_replay_append_batch(&replay_append_to(
            1,
            revision.id(),
            parent_epoch,
            children,
            1_000,
        ))
        .expect("append first child page");
    let all_children_epoch = store
        .apply_replay_append_batch(&replay_append_to(
            2,
            revision.id(),
            first_children_epoch,
            vec![replay_event(
                2,
                "child-256",
                Some("parent"),
                0,
                1,
                Some(100),
                false,
            )],
            1_000,
        ))
        .expect("append final child");
    let relation_epoch = store
        .apply_replay_relation(&replay_relation(
            9,
            revision.id(),
            all_children_epoch,
            "parent",
            "grandparent",
            900,
            false,
        ))
        .expect("late parent relation");
    let parent_reclassified = store
        .continue_replay(revision.id(), relation_epoch)
        .expect("reclassify parent");
    assert_eq!(parent_reclassified.processed_count(), 1);
    let first_page = store
        .continue_replay(revision.id(), parent_reclassified.epoch())
        .expect("scan first bounded child page");
    assert_eq!(first_page.processed_count(), 256);
    assert!(first_page.remaining_work());
    drop(store);

    let connection = Connection::open(&path).expect("inspect durable fanout cursor");
    let cursor: (String, String, i64) = connection
        .query_row(
            "SELECT reason, child_session_cursor, expected_evidence_epoch
             FROM usage_replay_work
             WHERE revision_id = 0 AND work_kind = 'scan_children'
               AND session_id = 'parent'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("fanout cursor");
    assert_eq!(
        cursor,
        (
            "fanout_bound".to_owned(),
            "child-255".to_owned(),
            i64::try_from(first_page.epoch().get()).expect("fixture epoch"),
        )
    );
    drop(connection);

    let mut reopened = UsageStore::open(&path).expect("reopen fanout archive");
    let second_page = reopened
        .continue_replay(revision.id(), first_page.epoch())
        .expect("resume child keyset cursor");
    assert_eq!(second_page.processed_count(), 1);
    assert!(second_page.remaining_work());
    drop(reopened);

    let connection = Connection::open(&path).expect("inspect resumed child work");
    let work: (i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_work
                WHERE revision_id = 0 AND work_kind = 'scan_children'
                  AND session_id = 'parent'),
               (SELECT count(*) FROM usage_replay_work
                WHERE revision_id = 0 AND work_kind = 'classify_session'
                  AND session_id LIKE 'child-%'),
               (SELECT count(DISTINCT session_id) FROM usage_replay_work
                WHERE revision_id = 0 AND work_kind = 'classify_session'
                  AND session_id LIKE 'child-%')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("bounded fanout work");
    assert_eq!(work, (0, 257, 257));
}

#[test]
fn depth_exhaustion_stays_pending_and_durable_without_epoch_spin() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("depth-bound.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(8))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([8; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let mut events = vec![replay_event(8, "depth-00", None, 0, 0, Some(100), false)];
    for depth in 1_u64..=33 {
        events.push(replay_event(
            8,
            &format!("depth-{depth:02}"),
            Some(&format!("depth-{:02}", depth - 1)),
            0,
            depth,
            Some(100),
            false,
        ));
    }
    let epoch = store
        .apply_replay_append_batch(&replay_append_to(
            8,
            revision.id(),
            revision.epoch(),
            events,
            1_000,
        ))
        .expect("append deep ancestry");
    let continuation = store
        .continue_replay(revision.id(), epoch)
        .expect("blocked depth continuation is observable");
    assert_eq!(continuation.processed_count(), 0);
    assert!(continuation.remaining_work());
    assert_eq!(continuation.epoch(), epoch);
    drop(store);

    let reopened = UsageStore::open(&path).expect("reopen depth-bound archive");
    let quality = reopened
        .replay_quality(revision.id())
        .expect("depth-bound quality");
    assert_eq!(quality.pending(), 1);
    drop(reopened);
    let connection = Connection::open(&path).expect("inspect depth-bound work");
    let work: (String, i64) = connection
        .query_row(
            "SELECT reason, expected_evidence_epoch FROM usage_replay_work
             WHERE revision_id = 0 AND work_kind = 'classify_session'
               AND session_id = 'depth-33'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("durable depth work");
    assert_eq!(work, ("depth_bound".to_owned(), 1));
}

#[test]
fn seal_and_promotion_publish_only_selected_eligible_events_atomically() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("seal-promote.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(4))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([4; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let parent = replay_event(4, "parent", None, 0, 10, Some(100), false);
    let parent_event_id = parent.id().as_str().to_owned();
    let append_epoch = store
        .apply_replay_append_batch(&replay_append(
            4,
            revision.id(),
            revision.epoch(),
            vec![
                parent,
                replay_event(4, "child", Some("parent"), 0, 20, Some(100), false),
                replay_event(
                    4,
                    "conflict-child",
                    Some("other-parent"),
                    0,
                    30,
                    Some(100),
                    true,
                ),
            ],
        ))
        .expect("append replay states");
    let quality = store
        .replay_quality(revision.id())
        .expect("staging quality");
    assert_eq!(quality.eligible(), 1);
    assert_eq!(quality.replay(), 1);
    assert_eq!(quality.conflict(), 1);

    let sealed = store
        .seal_replay_revision(revision.id(), append_epoch)
        .expect("seal replay revision");
    assert!(sealed.sealed());
    assert!(!sealed.promoted());
    assert_eq!(sealed.status(), ReplayRevisionStatus::Staging);
    let promoted = store
        .promote_replay_revision(revision.id(), sealed.epoch())
        .expect("promote replay revision");
    assert!(promoted.sealed());
    assert!(promoted.promoted());
    assert_eq!(promoted.status(), ReplayRevisionStatus::Current);
    assert_eq!(
        store.archive_state().expect("archive state").mode(),
        ArchiveMode::ReplayVerified
    );
    let page = store.event_page_before(None, 256).expect("promoted page");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].event_id(), parent_event_id);
    assert_eq!(
        store
            .generation_snapshot(SourceKey::from_bytes([4; 32]))
            .expect("promoted generation")
            .expect("current generation")
            .generation(),
        1
    );
    drop(store);

    let reopened = UsageStore::open(&path).expect("reopen promoted archive");
    let state = reopened.archive_state().expect("reopened archive state");
    assert_eq!(state.mode(), ArchiveMode::ReplayVerified);
    assert_eq!(state.active_revision(), Some(revision.id()));
    assert!(!state.rebuild_staging());
    assert_eq!(
        reopened
            .event_page_before(None, 256)
            .expect("reopened promoted page")
            .len(),
        1
    );
}

#[test]
fn complete_manifest_turns_missing_parent_into_divergence_before_seal() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(2))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([2; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let append_epoch = store
        .apply_replay_append_batch(&replay_append(
            2,
            revision.id(),
            revision.epoch(),
            vec![replay_event(
                2,
                "missing-child",
                Some("absent-parent"),
                0,
                10,
                Some(100),
                false,
            )],
        ))
        .expect("append missing parent");
    let blocked = store
        .seal_replay_revision(revision.id(), append_epoch)
        .expect_err("unfinished missing-parent work blocks seal");
    assert_eq!(blocked.code(), StoreErrorCode::PendingContinuation);

    let completed = store
        .continue_replay(revision.id(), append_epoch)
        .expect("classify missing complete");
    assert_eq!(completed.processed_count(), 1);
    let drained = store
        .continue_replay(revision.id(), completed.epoch())
        .expect("drain child scan");
    assert!(!drained.remaining_work());
    let quality = store
        .replay_quality(revision.id())
        .expect("completed quality");
    assert_eq!(quality.eligible(), 1);
    assert_eq!(quality.pending(), 0);
    let sealed = store
        .seal_replay_revision(revision.id(), drained.epoch())
        .expect("seal completed manifest");
    assert!(sealed.sealed());
}

#[test]
fn sealed_weak_pending_revision_cannot_be_promoted() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(3))
        .expect("registered source");
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([3; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let epoch = store
        .apply_replay_append_batch(&replay_append(
            3,
            revision.id(),
            revision.epoch(),
            vec![
                replay_event(3, "parent", None, 0, 10, Some(100), false),
                replay_event(3, "weak-child", Some("parent"), 0, 20, None, false),
            ],
        ))
        .expect("append weak pending evidence");
    let sealed = store
        .seal_replay_revision(revision.id(), epoch)
        .expect("seal reporting-quality revision");
    let error = store
        .promote_replay_revision(revision.id(), sealed.epoch())
        .expect_err("pending revision cannot become canonical");
    assert_eq!(error.code(), StoreErrorCode::PendingContinuation);
    assert_eq!(
        store.archive_state().expect("staging archive state").mode(),
        ArchiveMode::Empty
    );
    assert!(
        store
            .event_page_before(None, 256)
            .expect("current page")
            .is_empty()
    );
}

#[test]
fn discard_replay_revision_removes_only_staging_and_unblocks_rebuild() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(4))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([4; 32])].into_boxed_slice())
        .expect("manifest");

    let first = store
        .begin_replay_revision(&manifest)
        .expect("begin current replay");
    let current_event = replay_event(4, "current-root", None, 0, 10, Some(100), false);
    let first_epoch = store
        .apply_replay_append_batch(&replay_append(
            4,
            first.id(),
            first.epoch(),
            vec![current_event.clone()],
        ))
        .expect("append current replay");
    let first_sealed = store
        .seal_replay_revision(first.id(), first_epoch)
        .expect("seal current replay");
    store
        .promote_replay_revision(first.id(), first_sealed.epoch())
        .expect("promote current replay");
    let current_page = store
        .event_page_before(None, 256)
        .expect("current canonical page");

    let staging = store
        .begin_replay_revision(&manifest)
        .expect("begin replacement replay");
    let staging_epoch = store
        .apply_replay_append_batch(&replay_append_generation(
            4,
            staging.id(),
            staging.epoch(),
            2,
            vec![
                current_event,
                replay_event(4, "weak-child", Some("current-root"), 0, 20, None, false),
            ],
            100,
        ))
        .expect("append pending replacement");
    let staging_sealed = store
        .seal_replay_revision(staging.id(), staging_epoch)
        .expect("seal reporting-quality replacement");

    let stale = store
        .discard_replay_revision(staging.id(), staging_epoch)
        .expect_err("stale discard must fail closed");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("page after stale discard"),
        current_page
    );

    store
        .discard_replay_revision(staging.id(), staging_sealed.epoch())
        .expect("discard staging revision");
    let recovered = store.archive_state().expect("recovered archive state");
    assert_eq!(recovered.mode(), ArchiveMode::ReplayVerified);
    assert_eq!(recovered.active_revision(), Some(first.id()));
    assert!(!recovered.rebuild_staging());
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("page after discard"),
        current_page
    );
    assert!(
        store.begin_replay_revision(&manifest).is_ok(),
        "discarded staging must not block a fresh rebuild"
    );
}

#[test]
fn seal_rejects_omitted_registered_source_and_missing_overlay() {
    let mut store = UsageStore::in_memory().expect("usage store");
    for seed in [1_u8, 2_u8] {
        store
            .register_source(&registration(seed))
            .expect("registered source");
    }
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([1; 32])].into_boxed_slice())
                .expect("partial manifest"),
        )
        .expect("begin partial replay");
    let epoch = store
        .apply_replay_append_batch(&replay_append(
            1,
            revision.id(),
            revision.epoch(),
            vec![replay_event(1, "root", None, 0, 10, Some(100), false)],
        ))
        .expect("append partial manifest");
    let omitted = store
        .seal_replay_revision(revision.id(), epoch)
        .expect_err("omitted registered source must block seal");
    assert_eq!(omitted.code(), StoreErrorCode::IncompleteManifest);

    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("missing-overlay.sqlite3");
    let (ready, revision, epoch) = seal_ready_store(&path, 4);
    drop(ready);
    let connection = Connection::open(&path).expect("tamper replay overlay");
    connection
        .execute(
            "DELETE FROM usage_replay_observation WHERE revision_id = 0",
            [],
        )
        .expect("delete replay overlay");
    drop(connection);
    let mut reopened = UsageStore::open(&path).expect("reopen missing-overlay archive");
    let missing = reopened
        .seal_replay_revision(revision.id(), epoch)
        .expect_err("missing overlay must fail closed");
    assert_eq!(missing.code(), StoreErrorCode::InvalidStoredValue);
}

#[test]
fn seal_rejects_each_incomplete_checkpoint_and_chunk_shape() {
    let cases = [
        (
            "missing-generation",
            "DELETE FROM usage_generation WHERE status = 'staging'",
        ),
        (
            "incremental-verification",
            "UPDATE usage_generation SET verification_level = 'incremental' WHERE status = 'staging'",
        ),
        (
            "incomplete-tail",
            "UPDATE usage_generation SET incomplete_tail = 1 WHERE status = 'staging'",
        ),
        (
            "length-mismatch",
            "UPDATE usage_generation SET observed_file_length = 101 WHERE status = 'staging'",
        ),
        (
            "oversized-discard",
            "UPDATE usage_generation SET committed_offset = 99, scan_offset = 100,
                    observed_file_length = 100, discarding_oversized_line = 1,
                    incomplete_tail = 1, anchor_len = 99 WHERE status = 'staging'",
        ),
        (
            "chunk-gap",
            "UPDATE usage_source_chunk SET covered_len = 99 WHERE generation = 1",
        ),
    ];
    for (name, tamper) in cases {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join(format!("{name}.sqlite3"));
        let (ready, revision, epoch) = seal_ready_store(&path, 5);
        drop(ready);
        let connection = Connection::open(&path).expect("tamper checkpoint fixture");
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .expect("disable foreign keys for corruption fixture");
        connection
            .execute_batch(tamper)
            .expect("apply checkpoint tamper");
        drop(connection);
        let mut reopened = match UsageStore::open(&path) {
            Ok(store) => store,
            Err(error) => {
                assert_eq!(name, "missing-generation");
                assert_eq!(error.code(), StoreErrorCode::SchemaMismatch);
                continue;
            }
        };
        let error = reopened
            .seal_replay_revision(revision.id(), epoch)
            .expect_err("invalid source proof must block seal");
        assert_eq!(error.code(), StoreErrorCode::IncompleteManifest, "{name}");
    }
}

#[test]
fn seal_and_promotion_fail_closed_on_version_epoch_and_foreign_key_tamper() {
    let directory = TempDir::new().expect("temporary directory");
    let stale_path = directory.path().join("stale-seal.sqlite3");
    let (mut stale_store, revision, epoch) = seal_ready_store(&stale_path, 6);
    let stale = stale_store
        .seal_replay_revision(revision.id(), revision.epoch())
        .expect_err("stale seal epoch");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
    let unsealed = stale_store
        .promote_replay_revision(revision.id(), epoch)
        .expect_err("unsealed promotion");
    assert_eq!(unsealed.code(), StoreErrorCode::UnsealedRevision);
    drop(stale_store);

    let version_path = directory.path().join("version-seal.sqlite3");
    let (version_store, version_revision, version_epoch) = seal_ready_store(&version_path, 7);
    drop(version_store);
    let connection = Connection::open(&version_path).expect("tamper version");
    connection
        .execute(
            "UPDATE usage_replay_revision
             SET fingerprint_version = fingerprint_version + 1 WHERE revision_id = 0",
            [],
        )
        .expect("tamper revision version");
    drop(connection);
    let mut reopened = UsageStore::open(&version_path).expect("reopen version fixture");
    let mismatch = reopened
        .seal_replay_revision(version_revision.id(), version_epoch)
        .expect_err("version mismatch");
    assert_eq!(mismatch.code(), StoreErrorCode::AccountingVersionMismatch);
    drop(reopened);

    let foreign_path = directory.path().join("foreign-seal.sqlite3");
    let (foreign_store, _foreign_revision, _foreign_epoch) = seal_ready_store(&foreign_path, 8);
    drop(foreign_store);
    let connection = Connection::open(&foreign_path).expect("tamper foreign key");
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .expect("disable foreign keys for corruption fixture");
    connection
        .execute(
            "UPDATE usage_replay_session SET revision_id = 999 WHERE revision_id = 0",
            [],
        )
        .expect("orphan replay session");
    drop(connection);
    let foreign = UsageStore::open(&foreign_path).expect_err("foreign key corruption must fail");
    assert_eq!(foreign.code(), StoreErrorCode::SchemaMismatch);
}

#[test]
fn promotion_retains_immutable_migrated_legacy_snapshot() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("legacy-promotion.sqlite3");
    create_v1_event_fixture(&path);
    drop(UsageStore::open(&path).expect("migrate v1 archive"));
    let connection = Connection::open(&path).expect("align migrated source fixture");
    connection
        .execute(
            "UPDATE usage_source SET source_id = 'fixture-0', logical_identity = ?1",
            [[1_u8; 32].as_slice()],
        )
        .expect("align migrated source");
    connection
        .execute(
            "UPDATE usage_generation SET logical_identity = ?1 WHERE generation = 0",
            [[1_u8; 32].as_slice()],
        )
        .expect("align migrated generation");
    drop(connection);

    let mut store = UsageStore::open(&path).expect("open migrated archive");
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("legacy page before replay")[0]
            .event_id(),
        "legacy-event"
    );
    let revision = store
        .begin_replay_revision(
            &ReplayManifest::new(vec![SourceKey::from_bytes([0; 32])].into_boxed_slice())
                .expect("manifest"),
        )
        .expect("begin replay revision");
    let event = replay_event(0, "new-root", None, 0, 10, Some(100), false);
    let new_event_id = event.id().as_str().to_owned();
    let epoch = store
        .apply_replay_append_batch(&replay_append(
            0,
            revision.id(),
            revision.epoch(),
            vec![event],
        ))
        .expect("append migrated replay");
    let sealed = store
        .seal_replay_revision(revision.id(), epoch)
        .expect("seal migrated replay");
    store
        .promote_replay_revision(revision.id(), sealed.epoch())
        .expect("promote migrated replay");
    assert_eq!(
        store.event_page_before(None, 256).expect("promoted page")[0].event_id(),
        new_event_id
    );
    drop(store);

    let connection = Connection::open(&path).expect("inspect retained legacy snapshot");
    let legacy: (i64, String, i64) = connection
        .query_row(
            "SELECT snapshot.event_count, event.event_id,
                    (SELECT count(*) FROM usage_legacy_event WHERE snapshot_id = 1)
             FROM usage_legacy_snapshot AS snapshot
             JOIN usage_legacy_event AS event ON event.snapshot_id = snapshot.snapshot_id
             WHERE snapshot.snapshot_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("legacy snapshot state");
    assert_eq!(legacy, (1, "legacy-event".to_owned(), 1));
}

#[test]
fn second_promotion_replaces_prior_revision_without_losing_covered_projection() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("second-promotion.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(9))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([9; 32])].into_boxed_slice())
        .expect("manifest");
    let first = store
        .begin_replay_revision(&manifest)
        .expect("begin first replay");
    let first_event = replay_event(9, "first-root", None, 0, 10, Some(100), false);
    let first_epoch = store
        .apply_replay_append_batch(&replay_append(
            9,
            first.id(),
            first.epoch(),
            vec![first_event],
        ))
        .expect("append first replay");
    let first_sealed = store
        .seal_replay_revision(first.id(), first_epoch)
        .expect("seal first replay");
    store
        .promote_replay_revision(first.id(), first_sealed.epoch())
        .expect("promote first replay");

    let second = store
        .begin_replay_revision(&manifest)
        .expect("begin second replay");
    assert_eq!(second.id().get(), 1);
    let second_epoch = store
        .apply_replay_append_batch(&replay_append_generation(
            9,
            second.id(),
            second.epoch(),
            2,
            vec![
                replay_event(9, "first-root", None, 0, 10, Some(100), false),
                replay_event(9, "second-root", None, 0, 20, Some(200), false),
            ],
            100,
        ))
        .expect("append complete replacement replay");
    let second_sealed = store
        .seal_replay_revision(second.id(), second_epoch)
        .expect("seal second replay");
    store
        .promote_replay_revision(second.id(), second_sealed.epoch())
        .expect("promote second replay");
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("second promoted page")
            .len(),
        2
    );
    let repeated = store
        .promote_replay_revision(second.id(), second_sealed.epoch())
        .expect_err("repeated promotion must fail");
    assert_eq!(repeated.code(), StoreErrorCode::ArchiveModeMismatch);
    drop(store);

    let connection = Connection::open(&path).expect("inspect replacement state");
    let state: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision),
               (SELECT revision_id FROM usage_replay_revision WHERE status = 'current'),
               (SELECT count(*) FROM usage_generation),
               (SELECT generation FROM usage_generation WHERE status = 'current')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("replacement state");
    assert_eq!(state, (1, 1, 1, 2));
}

#[test]
fn replacement_promotion_carries_missing_prior_projection_with_exact_provenance() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("carry-forward-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(6))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([6; 32])].into_boxed_slice())
        .expect("manifest");
    let first = store
        .begin_replay_revision(&manifest)
        .expect("begin first replay");
    let retained_event = replay_event(6, "retained-root", None, 0, 10, Some(100), false);
    let retained_event_id = retained_event.id().as_str().to_owned();
    let first_epoch = store
        .apply_replay_append_batch(&replay_append(
            6,
            first.id(),
            first.epoch(),
            vec![retained_event],
        ))
        .expect("append first replay");
    let first_sealed = store
        .seal_replay_revision(first.id(), first_epoch)
        .expect("seal first replay");
    store
        .promote_replay_revision(first.id(), first_sealed.epoch())
        .expect("promote first replay");
    let current_page = store
        .event_page_before(None, 256)
        .expect("current canonical page");

    let second = store
        .begin_replay_revision(&manifest)
        .expect("begin incomplete replacement replay");
    let replacement_event = replay_event(6, "replacement-root", None, 0, 20, Some(200), false);
    let replacement_event_id = replacement_event.id().as_str().to_owned();
    let second_epoch = store
        .apply_replay_append_batch(&replay_append_generation(
            6,
            second.id(),
            second.epoch(),
            2,
            vec![replacement_event],
            100,
        ))
        .expect("append incomplete replacement replay");
    let second_sealed = store
        .seal_replay_revision(second.id(), second_epoch)
        .expect("seal incomplete replacement replay");
    store
        .promote_replay_revision(second.id(), second_sealed.epoch())
        .expect("complete replacement carries prior projection");
    let state = store
        .archive_state()
        .expect("archive state after carry-forward");
    assert_eq!(state.mode(), ArchiveMode::ReplayVerified);
    assert_eq!(state.active_revision(), Some(second.id()));
    assert!(!state.rebuild_staging());
    let promoted_page = store
        .event_page_before(None, 256)
        .expect("canonical page after carry-forward");
    assert_eq!(promoted_page.len(), 2);
    assert!(
        promoted_page
            .iter()
            .any(|event| event.event_id() == retained_event_id)
    );
    assert!(
        promoted_page
            .iter()
            .any(|event| event.event_id() == replacement_event_id)
    );
    assert_eq!(current_page.len(), 1);
    drop(store);

    let connection = Connection::open(&path).expect("inspect carry-forward provenance");
    let mut statement = connection
        .prepare(
            "SELECT event_id, selected_generation, projection_revision_id,
                    origin_revision_id, retained
             FROM usage_event ORDER BY event_id",
        )
        .expect("prepare provenance read");
    let provenance = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .expect("query provenance")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect provenance");
    assert!(provenance.contains(&(retained_event_id, 1, 1, Some(0), 1)));
    assert!(provenance.contains(&(replacement_event_id, 2, 1, Some(1), 0)));
    let generations: Vec<i64> = connection
        .prepare("SELECT generation FROM usage_generation ORDER BY generation")
        .expect("prepare generation read")
        .query_map([], |row| row.get(0))
        .expect("query generations")
        .collect::<Result<_, _>>()
        .expect("collect generations");
    assert_eq!(generations, vec![2]);
    drop(statement);
    drop(connection);

    let reopened = UsageStore::open(&path).expect("reopen retained projection");
    assert_eq!(
        reopened
            .event_page_before(None, 256)
            .expect("reopened retained page"),
        promoted_page
    );
}

#[test]
fn promotion_applies_direct_replay_conflict_and_absent_retention_truth_table() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("retention-truth-table-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(16))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([16; 32])].into_boxed_slice())
        .expect("manifest");

    let parent = replay_event(16, "parent", None, 0, 10, Some(100), false);
    let replay_later = replay_event(16, "child", None, 0, 20, Some(100), false);
    let eligible = replay_event(16, "eligible", None, 0, 30, Some(300), false);
    let conflict_later = replay_event(16, "conflict", None, 0, 40, Some(400), false);
    let absent_later = replay_event(16, "absent", None, 0, 50, Some(500), false);
    let ids = [
        parent.id().as_str().to_owned(),
        replay_later.id().as_str().to_owned(),
        eligible.id().as_str().to_owned(),
        conflict_later.id().as_str().to_owned(),
        absent_later.id().as_str().to_owned(),
    ];
    let first = store
        .begin_replay_revision(&manifest)
        .expect("begin first revision");
    let first_epoch = store
        .apply_replay_append_batch(&replay_append(
            16,
            first.id(),
            first.epoch(),
            vec![parent, replay_later, eligible, conflict_later, absent_later],
        ))
        .expect("append first revision");
    let first_sealed = store
        .seal_replay_revision(first.id(), first_epoch)
        .expect("seal first revision");
    store
        .promote_replay_revision(first.id(), first_sealed.epoch())
        .expect("promote first revision");
    assert_eq!(
        store
            .event_page_before(None, 256)
            .expect("first canonical page")
            .len(),
        5
    );

    let second = store
        .begin_replay_revision(&manifest)
        .expect("begin second revision");
    let second_epoch = store
        .apply_replay_append_batch(&replay_append_generation(
            16,
            second.id(),
            second.epoch(),
            2,
            vec![
                replay_event(16, "parent", None, 0, 10, Some(100), false),
                replay_event(16, "child", Some("parent"), 0, 20, Some(100), false),
                replay_event(16, "eligible", None, 0, 30, Some(300), false),
                replay_event(16, "conflict", Some("other"), 0, 40, Some(400), true),
            ],
            100,
        ))
        .expect("append second revision");
    let quality = store.replay_quality(second.id()).expect("second quality");
    assert_eq!(quality.eligible(), 2);
    assert_eq!(quality.replay(), 1);
    assert_eq!(quality.pending(), 0);
    assert_eq!(quality.conflict(), 1);
    let second_sealed = store
        .seal_replay_revision(second.id(), second_epoch)
        .expect("seal second revision");
    store
        .promote_replay_revision(second.id(), second_sealed.epoch())
        .expect("promote retention truth table");
    drop(store);

    let connection = Connection::open(&path).expect("inspect retention truth table");
    let rows: Vec<(String, i64, i64, i64)> = connection
        .prepare(
            "SELECT event_id, selected_generation, origin_revision_id, retained
             FROM usage_event ORDER BY event_id",
        )
        .expect("prepare retained rows")
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query retained rows")
        .collect::<Result<_, _>>()
        .expect("collect retained rows");
    assert_eq!(rows.len(), 4);
    assert!(rows.contains(&(ids[0].clone(), 2, 1, 0)));
    assert!(!rows.iter().any(|row| row.0 == ids[1]));
    assert!(rows.contains(&(ids[2].clone(), 2, 1, 0)));
    assert!(rows.contains(&(ids[3].clone(), 1, 0, 1)));
    assert!(rows.contains(&(ids[4].clone(), 1, 0, 1)));
}

#[test]
fn promotion_rejects_prior_projection_owned_by_the_staging_revision() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("projection-owner-tamper-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(17))
        .expect("registered source");
    let manifest = ReplayManifest::new(vec![SourceKey::from_bytes([17; 32])].into_boxed_slice())
        .expect("manifest");
    let first = store
        .begin_replay_revision(&manifest)
        .expect("begin first revision");
    let first_epoch = store
        .apply_replay_append_batch(&replay_append(
            17,
            first.id(),
            first.epoch(),
            vec![replay_event(17, "prior", None, 0, 10, Some(100), false)],
        ))
        .expect("append first revision");
    let first_sealed = store
        .seal_replay_revision(first.id(), first_epoch)
        .expect("seal first revision");
    store
        .promote_replay_revision(first.id(), first_sealed.epoch())
        .expect("promote first revision");
    let second = store
        .begin_replay_revision(&manifest)
        .expect("begin second revision");
    let second_epoch = store
        .apply_replay_append_batch(&replay_append_generation(
            17,
            second.id(),
            second.epoch(),
            2,
            vec![replay_event(17, "next", None, 0, 20, Some(200), false)],
            100,
        ))
        .expect("append second revision");
    let second_sealed = store
        .seal_replay_revision(second.id(), second_epoch)
        .expect("seal second revision");
    drop(store);

    let connection = Connection::open(&path).expect("tamper projection owner");
    connection
        .execute(
            "UPDATE usage_event
             SET projection_revision_id = 1, origin_revision_id = 1, retained = 0",
            [],
        )
        .expect("bind prior projection to staging revision");
    drop(connection);

    let mut reopened = UsageStore::open(&path).expect("reopen tampered projection");
    let error = reopened
        .promote_replay_revision(second.id(), second_sealed.epoch())
        .expect_err("staging-owned prior projection must fail closed");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
    let text = format!("{error:?} {error}");
    assert!(!text.contains(path.to_string_lossy().as_ref()));
    let state = reopened
        .archive_state()
        .expect("archive state after rejection");
    assert_eq!(state.active_revision(), Some(first.id()));
    assert!(state.rebuild_staging());
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
