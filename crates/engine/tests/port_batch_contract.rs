use tokenmaster_accounting::Canonicalizer;
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, SessionRelationDraft, SessionRelationDraftParts, TokenCount,
    TokenUsage, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_engine::{
    AdapterBatch, AdapterBatchParts, AdapterCheckpoint, AdapterCounters, AdapterDiagnostics,
    BatchState, CanonicalBatch, CanonicalBatchParts, ChunkProofBatch, EngineErrorCode,
    MAX_OBSERVATIONS_PER_BATCH, MAX_RELATIONS_PER_BATCH, ScopeIdentity, SourceIdentity,
};

fn source_identity(provider: &str, profile: &str, source: &str) -> SourceIdentity {
    SourceIdentity::new(
        ScopeIdentity::new(provider, profile).expect("scope"),
        source,
    )
    .expect("source")
}

fn usage() -> TokenUsage {
    TokenUsage::new(
        TokenCount::Available(2),
        TokenCount::Available(0),
        TokenCount::Available(3),
        TokenCount::Available(0),
        TokenCount::Available(5),
    )
}

fn observation(source: &SourceIdentity, ordinal: u64) -> ObservationDraft {
    ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new(source.scope().provider_id()).unwrap(),
        profile_id: UsageProfileId::new(source.scope().profile_id()).unwrap(),
        session_id: UsageSessionId::new("session-a").unwrap(),
        parent_session_id: None,
        session_ordinal: ordinal,
        lineage_conflict: false,
        source_id: UsageSourceId::new(source.source_id()).unwrap(),
        source_offset: ordinal + 1,
        source_verification: ObservationVerification::Incremental,
        timestamp: UtcTimestamp::new(1_720_000_000 + ordinal as i64, 0).unwrap(),
        model: ModelKey::new("gpt-5").unwrap(),
        raw_model: None,
        delta_usage: usage(),
        cumulative_usage: Some(usage()),
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        project: None,
        originator: None,
        activity: ActivityCounts::default(),
    })
    .expect("observation")
}

fn relation(source: &SourceIdentity) -> SessionRelationDraft {
    SessionRelationDraft::new(SessionRelationDraftParts {
        provider_id: UsageProviderId::new(source.scope().provider_id()).unwrap(),
        profile_id: UsageProfileId::new(source.scope().profile_id()).unwrap(),
        session_id: UsageSessionId::new("session-a").unwrap(),
        parent_session_id: UsageSessionId::new("session-parent").unwrap(),
        declared_conflict: false,
        source_id: UsageSourceId::new(source.source_id()).unwrap(),
        source_offset: 9,
    })
    .expect("relation")
}

fn checkpoint() -> AdapterCheckpoint {
    AdapterCheckpoint::new(vec![1, 2, 3].into_boxed_slice()).unwrap()
}

fn proofs() -> ChunkProofBatch {
    ChunkProofBatch::new(None, Box::default()).unwrap()
}

#[test]
fn adapter_batch_is_scope_exact_bounded_and_debug_private() {
    let source = source_identity("codex", "profile-a", "source-a");
    let batch = AdapterBatch::new(
        &source,
        AdapterBatchParts {
            observations: vec![observation(&source, 0)].into_boxed_slice(),
            relations: vec![relation(&source)].into_boxed_slice(),
            chunk_proofs: proofs(),
            next_checkpoint: checkpoint(),
            state: BatchState::SnapshotEnd,
            counters: AdapterCounters::new(1, 120, 1, 0).unwrap(),
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .expect("adapter batch");

    assert_eq!(batch.observations().len(), 1);
    assert_eq!(batch.relations().len(), 1);
    assert_eq!(batch.state(), BatchState::SnapshotEnd);
    assert_eq!(batch.next_checkpoint().as_bytes(), &[1, 2, 3]);
    let debug = format!("{batch:?}");
    assert!(debug.contains("observation_count: 1"));
    assert!(debug.contains("relation_count: 1"));
    assert!(!debug.contains("profile-a"));
    assert!(!debug.contains("source-a"));
    assert!(!debug.contains("session-a"));

    let foreign = source_identity("hermes", "profile-a", "source-a");
    let mismatch = AdapterBatch::new(
        &source,
        AdapterBatchParts {
            observations: vec![observation(&foreign, 0)].into_boxed_slice(),
            relations: Box::default(),
            chunk_proofs: proofs(),
            next_checkpoint: checkpoint(),
            state: BatchState::More,
            counters: AdapterCounters::new(1, 1, 1, 0).unwrap(),
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .expect_err("foreign observation");
    assert_eq!(mismatch.code(), EngineErrorCode::InvalidValue);
}

#[test]
fn adapter_batch_rejects_count_and_counter_incoherence() {
    let source = source_identity("codex", "profile-a", "source-a");
    let observations = (0..=MAX_OBSERVATIONS_PER_BATCH)
        .map(|ordinal| observation(&source, ordinal as u64))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let oversized = AdapterBatch::new(
        &source,
        AdapterBatchParts {
            observations,
            relations: Box::default(),
            chunk_proofs: proofs(),
            next_checkpoint: checkpoint(),
            state: BatchState::More,
            counters: AdapterCounters::default(),
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .expect_err("oversized observations");
    assert_eq!(oversized.code(), EngineErrorCode::CapacityExceeded);

    let relations = vec![relation(&source); MAX_RELATIONS_PER_BATCH + 1].into_boxed_slice();
    let oversized = AdapterBatch::new(
        &source,
        AdapterBatchParts {
            observations: Box::default(),
            relations,
            chunk_proofs: proofs(),
            next_checkpoint: checkpoint(),
            state: BatchState::More,
            counters: AdapterCounters::default(),
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .expect_err("oversized relations");
    assert_eq!(oversized.code(), EngineErrorCode::CapacityExceeded);

    let incoherent = AdapterBatch::new(
        &source,
        AdapterBatchParts {
            observations: vec![observation(&source, 0)].into_boxed_slice(),
            relations: Box::default(),
            chunk_proofs: proofs(),
            next_checkpoint: checkpoint(),
            state: BatchState::More,
            counters: AdapterCounters::new(1, 1, 0, 0).unwrap(),
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .expect_err("event counter mismatch");
    assert_eq!(incoherent.code(), EngineErrorCode::InvalidValue);
}

#[test]
fn canonical_batch_accepts_only_scope_exact_accounting_output() {
    let source = source_identity("codex", "profile-a", "source-a");
    let event = Canonicalizer::new()
        .canonicalize(&observation(&source, 0))
        .expect("canonical event");
    let batch = CanonicalBatch::new(
        &source,
        CanonicalBatchParts {
            events: vec![event].into_boxed_slice(),
            relations: vec![relation(&source)].into_boxed_slice(),
            chunk_proofs: proofs(),
            next_checkpoint: checkpoint(),
            state: BatchState::SnapshotEnd,
            counters: AdapterCounters::new(1, 120, 1, 0).unwrap(),
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .expect("canonical batch");
    assert_eq!(batch.events().len(), 1);
    assert_eq!(batch.relations().len(), 1);
    assert_eq!(batch.state(), BatchState::SnapshotEnd);
    let debug = format!("{batch:?}");
    assert!(debug.contains("event_count: 1"));
    assert!(!debug.contains("session-a"));

    let foreign = source_identity("codex", "profile-b", "source-a");
    let foreign_event = Canonicalizer::new()
        .canonicalize(&observation(&foreign, 0))
        .expect("foreign canonical event");
    let mismatch = CanonicalBatch::new(
        &source,
        CanonicalBatchParts {
            events: vec![foreign_event].into_boxed_slice(),
            relations: Box::default(),
            chunk_proofs: proofs(),
            next_checkpoint: checkpoint(),
            state: BatchState::More,
            counters: AdapterCounters::new(1, 1, 1, 0).unwrap(),
            diagnostics: AdapterDiagnostics::default(),
        },
    )
    .expect_err("foreign canonical event");
    assert_eq!(mismatch.code(), EngineErrorCode::InvalidValue);
}
