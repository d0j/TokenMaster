use tokenmaster_accounting::{CanonicalUsageEvent, Canonicalizer};
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, TokenCount, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, ArchiveGeneration, ArchivePublicationQuality,
    CurrentReplayAppendBatch, CurrentReplayAppendBatchParts, CurrentScanPublication,
    CurrentScanPublicationParts, ReplayAppendBatch, ReplayAppendBatchParts, ScanCounters,
    ScanOutcome, ScanScope, ScanSetManifest, SourceKey, SourceKind, SourceRegistration,
    SourceRegistrationParts, StoreErrorCode, StoredCheckpoint, StoredCheckpointParts,
    StoredSourceChunk, StoredVerification, UsageStore,
};

const SEED: u8 = 23;

fn checkpoint(offset: u64, verification: StoredVerification) -> StoredCheckpoint {
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([SEED; 32]),
        logical_identity: [SEED + 1; 32],
        committed_offset: offset,
        scan_offset: offset,
        observed_file_length: offset,
        modified_time_ns: Some(offset as i64),
        anchor_start: 0,
        anchor_len: u16::try_from(offset).expect("fixture anchor"),
        anchor_sha256: [SEED + 2; 32],
        resume: Box::default(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification,
    })
    .expect("checkpoint")
}

fn event(session: &str, source_offset: u64) -> CanonicalUsageEvent {
    let usage = TokenUsage::new(
        TokenCount::Available(10),
        TokenCount::Unavailable,
        TokenCount::Available(2),
        TokenCount::Unavailable,
        TokenCount::Available(12),
    );
    let draft = ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new("codex").unwrap(),
        profile_id: UsageProfileId::new("default").unwrap(),
        session_id: UsageSessionId::new(session).unwrap(),
        parent_session_id: None,
        session_ordinal: 0,
        lineage_conflict: false,
        source_id: UsageSourceId::new("fixture-23").unwrap(),
        source_offset,
        source_verification: ObservationVerification::FullPrefix,
        timestamp: UtcTimestamp::new(1_720_598_400 + source_offset as i64, 0).unwrap(),
        model: ModelKey::new("gpt-test").unwrap(),
        raw_model: None,
        delta_usage: usage,
        cumulative_usage: Some(usage),
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        project: None,
        originator: None,
        activity: ActivityCounts::default(),
    })
    .unwrap();
    Canonicalizer::new().canonicalize(&draft).unwrap()
}

fn append(
    expected_offset: u64,
    next_offset: u64,
    events: Vec<CanonicalUsageEvent>,
    prior_chunk: Option<StoredSourceChunk>,
    next_chunk_hash: [u8; 32],
    verification: StoredVerification,
) -> AppendBatch {
    AppendBatch::new(AppendBatchParts {
        source_key: SourceKey::from_bytes([SEED; 32]),
        expected_generation: 1,
        expected_committed_offset: expected_offset,
        expected_scan_offset: expected_offset,
        events: events.into_boxed_slice(),
        previous_partial_chunk: prior_chunk,
        chunk_updates: vec![
            StoredSourceChunk::new(0, u32::try_from(next_offset).unwrap(), next_chunk_hash)
                .unwrap(),
        ]
        .into_boxed_slice(),
        next_checkpoint: checkpoint(next_offset, verification),
        diagnostic_count_delta: 0,
    })
    .unwrap()
}

fn promoted_store() -> (UsageStore, tokenmaster_store::ReplayRevisionSnapshot) {
    let mut store = UsageStore::in_memory().unwrap();
    store
        .register_source(
            &SourceRegistration::new(SourceRegistrationParts {
                source_key: SourceKey::from_bytes([SEED; 32]),
                provider_id: "codex".into(),
                profile_id: "default".into(),
                source_id: "fixture-23".into(),
                source_kind: SourceKind::Active,
                logical_identity: [SEED + 1; 32],
                physical_identity: Some([SEED; 32]),
                initial_checkpoint: checkpoint(0, StoredVerification::Incremental),
            })
            .unwrap(),
        )
        .unwrap();
    let scan_set = store
        .begin_scan_set(
            &ScanSetManifest::new(
                vec![ScanScope::new("codex", "default").unwrap()].into_boxed_slice(),
            )
            .unwrap(),
            1_000,
        )
        .unwrap();
    let scan = store.scan_page(scan_set.id(), None, 1).unwrap()[0].id();
    store
        .observe_scan_source(scan, SourceKey::from_bytes([SEED; 32]))
        .unwrap();
    store
        .finish_scan(scan, ScanOutcome::Complete, 1_010, ScanCounters::default())
        .unwrap();
    store.finish_scan_set(scan_set.id(), 1_020).unwrap();
    let revision = store
        .begin_replay_revision_for_scan_set(scan_set.id())
        .unwrap();
    let initial_append = append(
        0,
        100,
        vec![event("first", 10)],
        None,
        [SEED + 3; 32],
        StoredVerification::FullPrefix,
    );
    let epoch = store
        .apply_replay_append_batch(
            &ReplayAppendBatch::new(ReplayAppendBatchParts {
                revision_id: revision.id(),
                expected_epoch: revision.epoch(),
                append_batch: initial_append,
                relations: Box::default(),
            })
            .unwrap(),
        )
        .unwrap();
    let sealed = store.seal_replay_revision(revision.id(), epoch).unwrap();
    let promoted = store
        .promote_replay_revision(revision.id(), sealed.epoch())
        .unwrap();
    (store, promoted)
}

#[test]
fn current_tail_append_is_atomic_and_advances_both_cas_tokens() {
    let (mut store, revision) = promoted_store();
    let publication = store.archive_publication().unwrap();
    assert_eq!(publication.generation(), ArchiveGeneration::new(1).unwrap());
    assert_eq!(publication.quality(), ArchivePublicationQuality::Complete);
    let batch = CurrentReplayAppendBatch::new(CurrentReplayAppendBatchParts {
        revision_id: revision.id(),
        expected_epoch: revision.epoch(),
        expected_archive_generation: publication.generation(),
        append_batch: append(
            100,
            200,
            vec![event("second", 110)],
            Some(StoredSourceChunk::new(0, 100, [SEED + 3; 32]).unwrap()),
            [SEED + 4; 32],
            StoredVerification::Incremental,
        ),
        relations: Box::default(),
    })
    .unwrap();
    let committed = store.apply_current_replay_append_batch(&batch).unwrap();
    assert_eq!(committed.epoch().get(), revision.epoch().get() + 1);
    assert_eq!(committed.archive_generation().get(), 2);
    assert_eq!(committed.quality(), ArchivePublicationQuality::Complete);
    assert!(!committed.remaining_work());
    assert_eq!(store.event_page_before(None, 256).unwrap().len(), 2);

    let stale = store
        .apply_current_replay_append_batch(&batch)
        .expect_err("exact CAS must reject a replayed batch");
    assert!(matches!(
        stale.code(),
        StoreErrorCode::StaleRevision | StoreErrorCode::StaleCheckpoint
    ));
}

#[test]
fn canonical_only_append_is_disabled_after_replay_promotion() {
    let (mut store, _) = promoted_store();
    let legacy_append = append(
        100,
        200,
        vec![event("legacy-bypass", 110)],
        Some(StoredSourceChunk::new(0, 100, [SEED + 3; 32]).unwrap()),
        [SEED + 4; 32],
        StoredVerification::Incremental,
    );
    let error = store
        .apply_append_batch(&legacy_append)
        .expect_err("canonical-only append must fail closed");
    assert_eq!(error.code(), StoreErrorCode::ArchiveModeMismatch);
    assert_eq!(store.event_page_before(None, 256).unwrap().len(), 1);
}

#[test]
fn rebuild_requirement_is_a_durable_generation_checked_publication_state() {
    let (mut store, revision) = promoted_store();
    let before = store.archive_publication().unwrap();

    let generation = store
        .mark_current_rebuild_required(revision.id(), before.generation())
        .unwrap();

    assert_eq!(generation.get(), before.generation().get() + 1);
    let after = store.archive_publication().unwrap();
    assert_eq!(after.generation(), generation);
    assert_eq!(after.current_revision(), Some(revision.id()));
    assert_eq!(after.quality(), ArchivePublicationQuality::RecoveryPending);
    let stale = store
        .mark_current_rebuild_required(revision.id(), before.generation())
        .expect_err("stale generation must not overwrite recovery state");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
}

#[test]
fn current_pending_work_continues_in_bounded_cas_transactions() {
    let (mut store, revision) = promoted_store();
    let publication = store.archive_publication().unwrap();
    let pending_event = {
        let usage = TokenUsage::new(
            TokenCount::Available(10),
            TokenCount::Unavailable,
            TokenCount::Available(2),
            TokenCount::Unavailable,
            TokenCount::Available(12),
        );
        let draft = ObservationDraft::new(ObservationDraftParts {
            provider_id: UsageProviderId::new("codex").unwrap(),
            profile_id: UsageProfileId::new("default").unwrap(),
            session_id: UsageSessionId::new("orphan").unwrap(),
            parent_session_id: Some(UsageSessionId::new("missing-parent").unwrap()),
            session_ordinal: 0,
            lineage_conflict: false,
            source_id: UsageSourceId::new("fixture-23").unwrap(),
            source_offset: 110,
            source_verification: ObservationVerification::FullPrefix,
            timestamp: UtcTimestamp::new(1_720_598_510, 0).unwrap(),
            model: ModelKey::new("gpt-test").unwrap(),
            raw_model: None,
            delta_usage: usage,
            cumulative_usage: Some(usage),
            fallback_model: false,
            long_context: LongContextState::No,
            service_tier: None,
            project: None,
            originator: None,
            activity: ActivityCounts::default(),
        })
        .unwrap();
        Canonicalizer::new().canonicalize(&draft).unwrap()
    };
    let batch = CurrentReplayAppendBatch::new(CurrentReplayAppendBatchParts {
        revision_id: revision.id(),
        expected_epoch: revision.epoch(),
        expected_archive_generation: publication.generation(),
        append_batch: append(
            100,
            200,
            vec![pending_event],
            Some(StoredSourceChunk::new(0, 100, [SEED + 3; 32]).unwrap()),
            [SEED + 4; 32],
            StoredVerification::Incremental,
        ),
        relations: Box::default(),
    })
    .unwrap();
    let mut state = store.apply_current_replay_append_batch(&batch).unwrap();
    assert!(state.remaining_work());
    assert_eq!(state.quality(), ArchivePublicationQuality::Partial);

    for _ in 0..8 {
        if !state.remaining_work() {
            break;
        }
        state = store
            .continue_current_replay(revision.id(), state.epoch(), state.archive_generation())
            .unwrap();
        assert!(state.processed_count() <= 256);
    }
    assert!(!state.remaining_work());
    assert_eq!(state.quality(), ArchivePublicationQuality::Complete);
    assert_eq!(
        store.archive_publication().unwrap().quality(),
        ArchivePublicationQuality::Complete
    );
}

#[test]
fn exact_complete_scan_publishes_freshness_with_the_same_current_revision() {
    let (mut store, revision) = promoted_store();
    let before = store.archive_publication().unwrap();
    let scan_set = store
        .begin_scan_set(
            &ScanSetManifest::new(
                vec![ScanScope::new("codex", "default").unwrap()].into_boxed_slice(),
            )
            .unwrap(),
            2_000,
        )
        .unwrap();
    let scan = store.scan_page(scan_set.id(), None, 1).unwrap()[0].id();
    store
        .observe_scan_source(scan, SourceKey::from_bytes([SEED; 32]))
        .unwrap();
    store
        .finish_scan(scan, ScanOutcome::Complete, 2_010, ScanCounters::default())
        .unwrap();
    store.finish_scan_set(scan_set.id(), 2_020).unwrap();
    let published = store
        .publish_current_scan(
            &CurrentScanPublication::new(CurrentScanPublicationParts {
                revision_id: revision.id(),
                expected_epoch: revision.epoch(),
                expected_archive_generation: before.generation(),
                scan_set_id: scan_set.id(),
                discovered_sources: Box::default(),
            })
            .unwrap(),
        )
        .unwrap();
    assert_eq!(published.epoch().get(), revision.epoch().get() + 1);
    assert_eq!(
        published.archive_generation().get(),
        before.generation().get() + 1
    );
    let after = store.archive_publication().unwrap();
    assert_eq!(after.current_revision(), Some(revision.id()));
    assert_eq!(after.latest_complete_scan_set(), Some(scan_set.id()));
    assert_eq!(after.quality(), ArchivePublicationQuality::Complete);
}

#[test]
fn discovered_source_is_admitted_only_by_its_exact_complete_scan() {
    let (mut store, revision) = promoted_store();
    let before = store.archive_publication().unwrap();
    let discovered_seed = 41_u8;
    let discovered_key = SourceKey::from_bytes([discovered_seed; 32]);
    let discovered = SourceRegistration::new(SourceRegistrationParts {
        source_key: discovered_key,
        provider_id: "codex".into(),
        profile_id: "default".into(),
        source_id: "fixture-41".into(),
        source_kind: SourceKind::Active,
        logical_identity: [discovered_seed + 1; 32],
        physical_identity: Some([discovered_seed; 32]),
        initial_checkpoint: StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some([discovered_seed; 32]),
            logical_identity: [discovered_seed + 1; 32],
            committed_offset: 0,
            scan_offset: 0,
            observed_file_length: 100,
            modified_time_ns: Some(3_000),
            anchor_start: 0,
            anchor_len: 0,
            anchor_sha256: [discovered_seed + 2; 32],
            resume: Box::default(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::Incremental,
        })
        .unwrap(),
    })
    .unwrap();
    let scan_set = store
        .begin_scan_set(
            &ScanSetManifest::new(
                vec![ScanScope::new("codex", "default").unwrap()].into_boxed_slice(),
            )
            .unwrap(),
            3_000,
        )
        .unwrap();
    let scan = store.scan_page(scan_set.id(), None, 1).unwrap()[0].id();
    store
        .observe_scan_source(scan, SourceKey::from_bytes([SEED; 32]))
        .unwrap();
    store
        .register_scan_discovered_source(scan, &discovered)
        .unwrap();
    assert!(store.generation_snapshot(discovered_key).unwrap().is_none());
    store
        .finish_scan(scan, ScanOutcome::Complete, 3_010, ScanCounters::default())
        .unwrap();
    store.finish_scan_set(scan_set.id(), 3_020).unwrap();
    let published = store
        .publish_current_scan(
            &CurrentScanPublication::new(CurrentScanPublicationParts {
                revision_id: revision.id(),
                expected_epoch: revision.epoch(),
                expected_archive_generation: before.generation(),
                scan_set_id: scan_set.id(),
                discovered_sources: vec![discovered_key].into_boxed_slice(),
            })
            .unwrap(),
        )
        .unwrap();
    assert_eq!(published.processed_count(), 1);
    assert_eq!(published.quality(), ArchivePublicationQuality::Partial);
    let admitted = store
        .generation_snapshot(discovered_key)
        .unwrap()
        .expect("admitted source generation");
    assert_eq!(admitted.generation(), 0);
    assert_eq!(admitted.checkpoint().committed_offset(), 0);
    assert_eq!(admitted.checkpoint().observed_file_length(), 100);
    assert_eq!(
        store
            .archive_publication()
            .unwrap()
            .latest_complete_scan_set(),
        Some(scan_set.id())
    );
    assert_eq!(
        store.archive_publication().unwrap().quality(),
        ArchivePublicationQuality::Partial
    );
    let no_op = store
        .continue_current_replay(
            revision.id(),
            published.epoch(),
            published.archive_generation(),
        )
        .unwrap();
    assert_eq!(no_op.processed_count(), 0);
    assert_eq!(no_op.epoch(), published.epoch());
    assert_eq!(no_op.archive_generation(), published.archive_generation());
    assert_eq!(no_op.quality(), ArchivePublicationQuality::Partial);

    let existing_tail = CurrentReplayAppendBatch::new(CurrentReplayAppendBatchParts {
        revision_id: revision.id(),
        expected_epoch: no_op.epoch(),
        expected_archive_generation: no_op.archive_generation(),
        append_batch: append(
            100,
            200,
            vec![event("existing-tail", 110)],
            Some(StoredSourceChunk::new(0, 100, [SEED + 3; 32]).unwrap()),
            [SEED + 4; 32],
            StoredVerification::Incremental,
        ),
        relations: Box::default(),
    })
    .unwrap();
    let appended = store
        .apply_current_replay_append_batch(&existing_tail)
        .expect("a pending discovered source must not block an existing source tail");
    assert_eq!(appended.quality(), ArchivePublicationQuality::Partial);
    assert_eq!(store.event_page_before(None, 256).unwrap().len(), 2);
}

#[test]
fn changed_unadmitted_source_requests_rebuild_instead_of_leaking_database_conflict() {
    let (mut store, revision) = promoted_store();
    let discovered_seed = 51_u8;
    let discovered_key = SourceKey::from_bytes([discovered_seed; 32]);
    let registration = |physical_seed: u8, length: u64, modified_time_ns: i64| {
        SourceRegistration::new(SourceRegistrationParts {
            source_key: discovered_key,
            provider_id: "codex".into(),
            profile_id: "default".into(),
            source_id: "fixture-51".into(),
            source_kind: SourceKind::Active,
            logical_identity: [discovered_seed + 1; 32],
            physical_identity: Some([physical_seed; 32]),
            initial_checkpoint: StoredCheckpoint::new(StoredCheckpointParts {
                parser_schema_version: 1,
                physical_identity: Some([physical_seed; 32]),
                logical_identity: [discovered_seed + 1; 32],
                committed_offset: 0,
                scan_offset: 0,
                observed_file_length: length,
                modified_time_ns: Some(modified_time_ns),
                anchor_start: 0,
                anchor_len: 0,
                anchor_sha256: [discovered_seed + 2; 32],
                resume: Box::default(),
                discarding_oversized_line: false,
                incomplete_tail: false,
                verification: StoredVerification::Incremental,
            })
            .unwrap(),
        })
        .unwrap()
    };

    let first_scan_set = store
        .begin_scan_set(
            &ScanSetManifest::new(
                vec![ScanScope::new("codex", "default").unwrap()].into_boxed_slice(),
            )
            .unwrap(),
            4_000,
        )
        .unwrap();
    let first_scan = store.scan_page(first_scan_set.id(), None, 1).unwrap()[0].id();
    store
        .observe_scan_source(first_scan, SourceKey::from_bytes([SEED; 32]))
        .unwrap();
    store
        .register_scan_discovered_source(first_scan, &registration(51, 100, 4_000))
        .unwrap();
    store
        .finish_scan(
            first_scan,
            ScanOutcome::Partial,
            4_010,
            ScanCounters::default(),
        )
        .unwrap();
    store.finish_scan_set(first_scan_set.id(), 4_020).unwrap();

    let second_scan_set = store
        .begin_scan_set(
            &ScanSetManifest::new(
                vec![ScanScope::new("codex", "default").unwrap()].into_boxed_slice(),
            )
            .unwrap(),
            5_000,
        )
        .unwrap();
    let second_scan = store.scan_page(second_scan_set.id(), None, 1).unwrap()[0].id();
    store
        .observe_scan_source(second_scan, SourceKey::from_bytes([SEED; 32]))
        .unwrap();

    let error = store
        .register_scan_discovered_source(second_scan, &registration(52, 120, 5_000))
        .expect_err("changed provisional source must request full rebuild");

    assert_eq!(error.code(), StoreErrorCode::RebuildRequired);
    let publication = store.archive_publication().unwrap();
    assert_eq!(publication.current_revision(), Some(revision.id()));
    assert_eq!(publication.quality(), ArchivePublicationQuality::Complete);
    assert!(store.generation_snapshot(discovered_key).unwrap().is_none());
}
