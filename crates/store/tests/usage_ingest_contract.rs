use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_accounting::{CanonicalUsageEvent, Canonicalizer};
use tokenmaster_domain::{
    ActivityCounts, LongContextState, MetadataValue, ModelKey, ObservationDraft,
    ObservationDraftParts, ObservationVerification, ProjectAlias, TokenCount, TokenUsage,
    UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, SourceKey, SourceKind, SourceRegistration,
    SourceRegistrationParts, StoreErrorCode, StoredCheckpoint, StoredCheckpointParts,
    StoredSourceChunk, StoredVerification, UsageStore,
};

fn checkpoint(identity_seed: u8, committed_offset: u64) -> StoredCheckpoint {
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([identity_seed; 32]),
        logical_identity: [identity_seed.wrapping_add(1); 32],
        committed_offset,
        scan_offset: committed_offset,
        observed_file_length: committed_offset,
        modified_time_ns: Some(1_000 + i64::try_from(committed_offset).expect("fixture offset")),
        anchor_start: 0,
        anchor_len: u16::try_from(committed_offset.min(100)).expect("fixture anchor"),
        anchor_sha256: [identity_seed.wrapping_add(2); 32],
        resume: vec![identity_seed, 1].into_boxed_slice(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    })
    .expect("valid checkpoint")
}

fn discard_checkpoint(
    identity_seed: u8,
    committed_offset: u64,
    scan_offset: u64,
) -> StoredCheckpoint {
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([identity_seed; 32]),
        logical_identity: [identity_seed.wrapping_add(1); 32],
        committed_offset,
        scan_offset,
        observed_file_length: scan_offset,
        modified_time_ns: Some(2_000),
        anchor_start: 0,
        anchor_len: u16::try_from(committed_offset.min(100)).expect("fixture anchor"),
        anchor_sha256: [identity_seed.wrapping_add(2); 32],
        resume: vec![identity_seed, 1].into_boxed_slice(),
        discarding_oversized_line: true,
        incomplete_tail: true,
        verification: StoredVerification::Incremental,
    })
    .expect("valid discard checkpoint")
}

fn registration(source_key: u8) -> SourceRegistration {
    SourceRegistration::new(SourceRegistrationParts {
        source_key: SourceKey::from_bytes([source_key; 32]),
        provider_id: "codex".into(),
        profile_id: "default".into(),
        source_id: "fixture".into(),
        source_kind: SourceKind::Active,
        logical_identity: [source_key.wrapping_add(1); 32],
        physical_identity: Some([source_key; 32]),
        initial_checkpoint: checkpoint(source_key, 0),
    })
    .expect("valid source registration")
}

fn event(fingerprint: u8, source_offset: u64) -> CanonicalUsageEvent {
    event_for_provider("codex", fingerprint, source_offset)
}

fn event_for_provider(
    provider_id: &str,
    fingerprint: u8,
    source_offset: u64,
) -> CanonicalUsageEvent {
    let draft = ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new(provider_id).expect("provider"),
        profile_id: UsageProfileId::new("default").expect("profile"),
        session_id: UsageSessionId::new("session-fixture").expect("session"),
        parent_session_id: None,
        session_ordinal: u64::from(fingerprint),
        lineage_conflict: false,
        source_id: UsageSourceId::new("fixture").expect("source"),
        source_offset,
        source_verification: ObservationVerification::Incremental,
        timestamp: UtcTimestamp::new(
            1_720_598_400 + i64::from(fingerprint),
            u32::from(fingerprint),
        )
        .expect("timestamp"),
        model: ModelKey::new("gpt-test").expect("model"),
        raw_model: Some(MetadataValue::new("gpt-test").expect("raw model")),
        delta_usage: TokenUsage::new(
            TokenCount::Available(10),
            TokenCount::Unavailable,
            TokenCount::Available(2),
            TokenCount::Unavailable,
            TokenCount::Available(12),
        ),
        cumulative_usage: None,
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: Some(MetadataValue::new("priority").expect("tier")),
        project: Some(ProjectAlias::new("tokenmaster").expect("project")),
        originator: Some(MetadataValue::new("codex_cli").expect("originator")),
        activity: ActivityCounts::default(),
    })
    .expect("valid observation draft");
    Canonicalizer::new()
        .canonicalize(&draft)
        .expect("valid canonical event")
}

fn append_batch(
    source_key: u8,
    expected_offset: u64,
    next_offset: u64,
    events: Vec<CanonicalUsageEvent>,
    previous_partial_chunk: Option<StoredSourceChunk>,
) -> AppendBatch {
    AppendBatch::new(AppendBatchParts {
        source_key: SourceKey::from_bytes([source_key; 32]),
        expected_generation: 0,
        expected_committed_offset: expected_offset,
        expected_scan_offset: expected_offset,
        events: events.into_boxed_slice(),
        previous_partial_chunk,
        chunk_updates: vec![
            StoredSourceChunk::new(
                0,
                u32::try_from(next_offset).expect("fixture chunk"),
                [8; 32],
            )
            .expect("valid source chunk"),
        ]
        .into_boxed_slice(),
        next_checkpoint: checkpoint(source_key, next_offset),
        diagnostic_count_delta: 0,
    })
    .expect("valid append batch")
}

fn discard_batch(
    source_key: u8,
    expected_scan_offset: u64,
    next_scan_offset: u64,
    proof: StoredSourceChunk,
    sha256: [u8; 32],
) -> AppendBatch {
    AppendBatch::new(AppendBatchParts {
        source_key: SourceKey::from_bytes([source_key; 32]),
        expected_generation: 0,
        expected_committed_offset: 100,
        expected_scan_offset,
        events: Box::default(),
        previous_partial_chunk: Some(proof),
        chunk_updates: vec![
            StoredSourceChunk::new(
                0,
                u32::try_from(next_scan_offset).expect("fixture scan chunk"),
                sha256,
            )
            .expect("valid discard chunk"),
        ]
        .into_boxed_slice(),
        next_checkpoint: discard_checkpoint(source_key, 100, next_scan_offset),
        diagnostic_count_delta: 1,
    })
    .expect("valid discard batch")
}

#[test]
fn append_rejects_canonical_events_from_a_different_provider() {
    let mut store = UsageStore::in_memory().expect("usage store");
    store
        .register_source(&registration(6))
        .expect("register Codex source");
    let before = store.counts().expect("counts before rejected append");
    let batch = append_batch(6, 0, 100, vec![event_for_provider("other", 1, 10)], None);

    let error = store
        .apply_append_batch(&batch)
        .expect_err("provider mismatch must fail closed");
    assert_eq!(error.code(), StoreErrorCode::InvalidValue);
    assert_eq!(
        store.counts().expect("counts after rejected append"),
        before
    );
}

#[test]
fn append_cannot_clear_complete_scan_missing_authority() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("append-missing-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(4))
        .expect("register source");
    drop(store);

    let connection = Connection::open(&path).expect("mark source missing");
    connection
        .execute(
            "UPDATE usage_source SET missing = 1 WHERE file_key = ?1",
            [[4_u8; 32].as_slice()],
        )
        .expect("mark missing");
    drop(connection);

    let mut store = UsageStore::open(&path).expect("reopen missing source");
    store
        .apply_append_batch(&append_batch(4, 0, 100, vec![event(8, 10)], None))
        .expect("ordinary append remains valid");
    drop(store);

    let missing: i64 = Connection::open(&path)
        .expect("inspect missing state")
        .query_row(
            "SELECT missing FROM usage_source WHERE file_key = ?1",
            [[4_u8; 32].as_slice()],
            |row| row.get(0),
        )
        .expect("missing state");
    assert_eq!(missing, 1, "only a complete scan may restore presence");
}

#[test]
fn append_batch_is_atomic_replay_safe_and_stale_fail_closed() {
    let store_directory = TempDir::new().expect("temporary directory");
    let path = store_directory.path().join("atomic-append-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(7))
        .expect("register source");

    let batch = append_batch(7, 0, 100, vec![event(1, 10), event(2, 40)], None);
    store
        .apply_append_batch(&batch)
        .expect("append usage batch");
    let counts = store.counts().expect("counts after append");
    assert_eq!(counts.sources(), 1);
    assert_eq!(counts.generations(), 1);
    assert_eq!(counts.observations(), 2);
    assert_eq!(counts.canonical_events(), 2);
    assert_eq!(counts.chunks(), 1);
    assert_eq!(
        store
            .generation_snapshot(SourceKey::from_bytes([7; 32]))
            .expect("snapshot")
            .expect("current generation")
            .checkpoint()
            .committed_offset(),
        100
    );

    let replay = store
        .apply_append_batch(&batch)
        .expect_err("replayed stale checkpoint must fail closed");
    assert_eq!(replay.code(), StoreErrorCode::StaleCheckpoint);
    assert_eq!(store.counts().expect("counts after replay"), counts);

    let stale = append_batch(
        7,
        99,
        200,
        Vec::new(),
        Some(StoredSourceChunk::new(0, 100, [8; 32]).expect("prior chunk proof")),
    );
    let error = store
        .apply_append_batch(&stale)
        .expect_err("stale offset must fail closed");
    assert_eq!(error.code(), StoreErrorCode::StaleCheckpoint);
    assert_eq!(store.counts().expect("counts after stale batch"), counts);
    assert_eq!(
        store
            .generation_snapshot(SourceKey::from_bytes([7; 32]))
            .expect("snapshot after stale batch")
            .expect("current generation")
            .checkpoint()
            .committed_offset(),
        100
    );
}

#[test]
fn duplicate_fingerprint_keeps_two_observations_and_one_deterministic_canonical_event() {
    let store_directory = TempDir::new().expect("temporary directory");
    let path = store_directory
        .path()
        .join("canonical-selection-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    let fingerprint = *event(3, 10).fingerprint().as_bytes();
    store
        .register_source(&registration(9))
        .expect("register larger source key first");
    store
        .apply_append_batch(&append_batch(9, 0, 100, vec![event(3, 10)], None))
        .expect("append first observation");
    store
        .register_source(&registration(1))
        .expect("register smaller source key second");
    store
        .apply_append_batch(&append_batch(1, 0, 100, vec![event(3, 10)], None))
        .expect("append duplicate observation");

    let counts = store.counts().expect("deduplicated counts");
    assert_eq!(counts.observations(), 2);
    assert_eq!(counts.canonical_events(), 1);
    drop(store);

    let connection = Connection::open(path).expect("inspect deterministic selection");
    let selected_file_key: Vec<u8> = connection
        .query_row(
            "SELECT selected_file_key FROM usage_event WHERE fingerprint = ?1",
            [fingerprint.as_slice()],
            |row| row.get(0),
        )
        .expect("canonical source key");
    assert_eq!(selected_file_key, [1_u8; 32]);
}

#[test]
fn partial_chunk_proof_and_scan_offset_are_both_compare_and_swap_inputs() {
    let store_directory = TempDir::new().expect("temporary directory");
    let path = store_directory.path().join("partial-proof-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    store
        .register_source(&registration(5))
        .expect("register source");
    store
        .apply_append_batch(&append_batch(5, 0, 100, vec![event(4, 10)], None))
        .expect("seed partial source chunk");
    let counts = store.counts().expect("seeded counts");

    let wrong_proof = StoredSourceChunk::new(0, 100, [7; 32]).expect("wrong proof shape");
    let error = store
        .apply_append_batch(&discard_batch(5, 100, 200, wrong_proof, [9; 32]))
        .expect_err("wrong prior partial digest must fail closed");
    assert_eq!(error.code(), StoreErrorCode::StaleCheckpoint);
    assert_eq!(store.counts().expect("counts after wrong proof"), counts);

    let correct_proof = StoredSourceChunk::new(0, 100, [8; 32]).expect("correct proof");
    store
        .apply_append_batch(&discard_batch(5, 100, 200, correct_proof, [9; 32]))
        .expect("advance numeric discard checkpoint");
    let checkpoint = store
        .generation_snapshot(SourceKey::from_bytes([5; 32]))
        .expect("discard snapshot")
        .expect("current generation");
    assert_eq!(checkpoint.checkpoint().committed_offset(), 100);
    assert_eq!(checkpoint.checkpoint().scan_offset(), 200);

    let stale_error = store
        .apply_append_batch(&discard_batch(5, 100, 150, correct_proof, [6; 32]))
        .expect_err("stale numeric scan offset must not regress");
    assert_eq!(stale_error.code(), StoreErrorCode::StaleCheckpoint);
    assert_eq!(
        store
            .generation_snapshot(SourceKey::from_bytes([5; 32]))
            .expect("snapshot after stale numeric discard")
            .expect("current generation")
            .checkpoint()
            .scan_offset(),
        200
    );
}
