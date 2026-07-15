use tokenmaster_engine::{
    AdapterCheckpoint, AdapterCounters, AdapterDiagnosticCode, AdapterDiagnostics, ChunkProof,
    ChunkProofBatch, CompletionQuality, DiscoveredSource, EngineErrorCode,
    MAX_ADAPTER_CHECKPOINT_BYTES, MAX_CHUNK_PROOFS_PER_BATCH, MAX_SCOPE_MANIFEST_ENTRIES,
    SOURCE_CHUNK_BYTES, ScopeIdentity, ScopeManifest, SourceIdentity, SourceKind,
};

fn scope(profile: &str) -> ScopeIdentity {
    ScopeIdentity::new("codex", profile).expect("valid scope")
}

#[test]
fn identities_are_bounded_provider_neutral_and_debug_private() {
    let scope = scope("profile-a");
    let source = SourceIdentity::new(scope.clone(), "source-1", [7; 32]).expect("valid source");
    let sibling = SourceIdentity::new(scope.clone(), "source-1", [8; 32]).expect("sibling source");
    let discovered = DiscoveredSource::new(source.clone(), SourceKind::Active);

    assert_eq!(scope.provider_id(), "codex");
    assert_eq!(scope.profile_id(), "profile-a");
    assert_eq!(source.scope(), &scope);
    assert_eq!(source.source_id(), "source-1");
    assert_eq!(source.logical_file_key(), &[7; 32]);
    assert_eq!(sibling.source_id(), source.source_id());
    assert_ne!(source, sibling);
    assert_eq!(discovered.identity(), &source);
    assert_eq!(discovered.kind(), SourceKind::Active);
    assert_eq!(discovered.logical_identity(), &[7; 32]);
    assert_eq!(format!("{scope:?}"), "ScopeIdentity([redacted])");
    assert_eq!(format!("{source:?}"), "SourceIdentity([redacted])");
    let discovered_debug = format!("{discovered:?}");
    assert!(!discovered_debug.contains("profile-a"));
    assert!(!discovered_debug.contains("source-1"));
    assert!(!discovered_debug.contains("7, 7"));

    let invalid =
        ScopeIdentity::new(r"C:\Users\private", "profile-a").expect_err("path-like provider ID");
    assert_eq!(invalid.code(), EngineErrorCode::InvalidValue);
    assert_eq!(invalid.to_string(), "invalid_value");
    assert!(!format!("{invalid:?}").contains("private"));

    let oversized =
        SourceIdentity::new(scope, "s".repeat(129), [0; 32]).expect_err("oversized source ID");
    assert_eq!(oversized.code(), EngineErrorCode::CapacityExceeded);
}

#[test]
fn scope_manifest_is_sorted_unique_and_hard_bounded() {
    let manifest =
        ScopeManifest::new(vec![scope("profile-b"), scope("profile-a")].into_boxed_slice())
            .expect("bounded manifest");
    assert_eq!(manifest.scope_count(), 2);
    assert_eq!(manifest.scopes()[0].profile_id(), "profile-a");
    assert_eq!(manifest.scopes()[1].profile_id(), "profile-b");
    assert_eq!(format!("{manifest:?}"), "ScopeManifest { scope_count: 2 }");

    let empty = ScopeManifest::new(Box::default()).expect_err("empty manifest");
    assert_eq!(empty.code(), EngineErrorCode::InvalidValue);
    let duplicate =
        ScopeManifest::new(vec![scope("profile-a"), scope("profile-a")].into_boxed_slice())
            .expect_err("duplicate scope");
    assert_eq!(duplicate.code(), EngineErrorCode::InvalidValue);

    let scopes = (0..=MAX_SCOPE_MANIFEST_ENTRIES)
        .map(|index| scope(&format!("profile-{index}")))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let oversized = ScopeManifest::new(scopes).expect_err("oversized manifest");
    assert_eq!(oversized.code(), EngineErrorCode::CapacityExceeded);
}

#[test]
fn opaque_checkpoint_is_exactly_bounded_and_never_debugs_bytes() {
    let checkpoint =
        AdapterCheckpoint::new(vec![0x5a; MAX_ADAPTER_CHECKPOINT_BYTES].into_boxed_slice())
            .expect("maximum checkpoint");
    assert_eq!(checkpoint.byte_len(), MAX_ADAPTER_CHECKPOINT_BYTES);
    assert_eq!(checkpoint.as_bytes()[0], 0x5a);
    assert_eq!(
        format!("{checkpoint:?}"),
        format!(
            "AdapterCheckpoint {{ byte_len: {}, bytes: [redacted] }}",
            MAX_ADAPTER_CHECKPOINT_BYTES
        )
    );

    let oversized =
        AdapterCheckpoint::new(vec![0; MAX_ADAPTER_CHECKPOINT_BYTES + 1].into_boxed_slice())
            .expect_err("oversized checkpoint");
    assert_eq!(oversized.code(), EngineErrorCode::CapacityExceeded);
}

#[test]
fn chunk_proofs_are_fixed_size_sorted_unique_and_bounded() {
    let previous = ChunkProof::new(4, 17, [4; 32]).expect("partial proof");
    let batch = ChunkProofBatch::new(
        Some(previous),
        vec![
            ChunkProof::new(6, 12, [6; 32]).unwrap(),
            ChunkProof::new(5, SOURCE_CHUNK_BYTES, [5; 32]).unwrap(),
        ]
        .into_boxed_slice(),
    )
    .expect("proof batch");
    assert_eq!(batch.previous_partial().map(ChunkProof::index), Some(4));
    assert_eq!(batch.updates()[0].index(), 5);
    assert_eq!(batch.updates()[1].index(), 6);
    assert_eq!(
        format!("{:?}", batch.updates()[0]),
        "ChunkProof { index: 5, covered_len: 1048576, sha256: [redacted] }"
    );

    let zero = ChunkProof::new(0, 0, [0; 32]).expect_err("zero coverage");
    assert_eq!(zero.code(), EngineErrorCode::InvalidValue);
    let oversized =
        ChunkProof::new(0, SOURCE_CHUNK_BYTES + 1, [0; 32]).expect_err("oversized coverage");
    assert_eq!(oversized.code(), EngineErrorCode::CapacityExceeded);
    let duplicate = ChunkProofBatch::new(
        None,
        vec![
            ChunkProof::new(3, 1, [1; 32]).unwrap(),
            ChunkProof::new(3, 2, [2; 32]).unwrap(),
        ]
        .into_boxed_slice(),
    )
    .expect_err("duplicate chunk index");
    assert_eq!(duplicate.code(), EngineErrorCode::InvalidValue);

    let too_many = (0..=MAX_CHUNK_PROOFS_PER_BATCH)
        .map(|index| ChunkProof::new(index as u64, 1, [0; 32]).unwrap())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let oversized = ChunkProofBatch::new(None, too_many).expect_err("too many proofs");
    assert_eq!(oversized.code(), EngineErrorCode::CapacityExceeded);
}

#[test]
fn counters_and_diagnostics_are_checked_fixed_state_values() {
    let counters = AdapterCounters::new(1, 2, 3, 4).expect("valid counters");
    assert_eq!(counters.files_read(), 1);
    assert_eq!(counters.bytes_read(), 2);
    assert_eq!(counters.events_observed(), 3);
    assert_eq!(counters.diagnostics(), 4);
    assert_eq!(
        counters
            .checked_add(AdapterCounters::new(5, 6, 7, 8).unwrap())
            .unwrap(),
        AdapterCounters::new(6, 8, 10, 12).unwrap()
    );
    let invalid = AdapterCounters::new(i64::MAX as u64 + 1, 0, 0, 0)
        .expect_err("SQLite-incompatible counter");
    assert_eq!(invalid.code(), EngineErrorCode::CapacityExceeded);
    let overflow = AdapterCounters::new(i64::MAX as u64, 0, 0, 0)
        .unwrap()
        .checked_add(AdapterCounters::new(1, 0, 0, 0).unwrap())
        .expect_err("counter overflow");
    assert_eq!(overflow.code(), EngineErrorCode::CapacityExceeded);

    let mut diagnostics = AdapterDiagnostics::default();
    diagnostics
        .record(AdapterDiagnosticCode::MalformedInput)
        .unwrap();
    diagnostics
        .record(AdapterDiagnosticCode::MalformedInput)
        .unwrap();
    diagnostics
        .record(AdapterDiagnosticCode::SourceChanged)
        .unwrap();
    assert_eq!(diagnostics.count(AdapterDiagnosticCode::MalformedInput), 2);
    assert_eq!(diagnostics.count(AdapterDiagnosticCode::SourceChanged), 1);
    assert_eq!(diagnostics.total().unwrap(), 3);
    assert_eq!(core::mem::size_of::<AdapterDiagnostics>(), 8 * 8);

    assert_ne!(CompletionQuality::Complete, CompletionQuality::Partial);
}
