use std::time::{Duration, Instant};

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_accounting::{CanonicalUsageEvent, Canonicalizer};
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, ProjectAlias, TokenCount, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_store::{
    AppendBatch, AppendBatchParts, SourceKey, SourceKind, SourceRegistration,
    SourceRegistrationParts, StoredCheckpoint, StoredCheckpointParts, StoredSourceChunk,
    StoredVerification, UsageStore,
};

const SAMPLE_COUNT: usize = 20;
const APPEND_P95_BUDGET: Duration = Duration::from_millis(25);
const SMALL_CATCH_UP_P95_BUDGET: Duration = Duration::from_millis(50);
const CATCH_UP_P95_BUDGET: Duration = Duration::from_millis(250);
const MAX_AGGREGATE_OVERHEAD_RATIO: f64 = 1.5;

fn checkpoint(offset: u64) -> StoredCheckpoint {
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([1; 32]),
        logical_identity: [2; 32],
        committed_offset: offset,
        scan_offset: offset,
        observed_file_length: offset,
        modified_time_ns: Some(1),
        anchor_start: 0,
        anchor_len: u16::try_from(offset.min(1_024)).expect("bounded anchor"),
        anchor_sha256: [3; 32],
        resume: Box::default(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    })
    .expect("valid checkpoint")
}

fn registration() -> SourceRegistration {
    SourceRegistration::new(SourceRegistrationParts {
        source_key: SourceKey::from_bytes([1; 32]),
        provider_id: "codex".into(),
        profile_id: "default".into(),
        source_id: "performance-fixture".into(),
        source_kind: SourceKind::Active,
        logical_identity: [2; 32],
        physical_identity: Some([1; 32]),
        initial_checkpoint: checkpoint(0),
    })
    .expect("valid registration")
}

fn event(index: usize) -> CanonicalUsageEvent {
    let index = u64::try_from(index).expect("bounded event index");
    let draft = ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new("codex").expect("provider"),
        profile_id: UsageProfileId::new("default").expect("profile"),
        session_id: UsageSessionId::new(format!("session-{}", index / 16)).expect("session"),
        parent_session_id: None,
        session_ordinal: index,
        lineage_conflict: false,
        source_id: UsageSourceId::new("performance-fixture").expect("source"),
        source_offset: index * 100,
        source_verification: ObservationVerification::Incremental,
        timestamp: UtcTimestamp::new(
            1_720_598_400 + i64::try_from(index).expect("timestamp"),
            u32::try_from(index % 1_000_000_000).expect("nanos"),
        )
        .expect("timestamp"),
        model: ModelKey::new(if index.is_multiple_of(2) {
            "gpt-performance-a"
        } else {
            "gpt-performance-b"
        })
        .expect("model"),
        raw_model: None,
        delta_usage: TokenUsage::new(
            TokenCount::Available(index + 1),
            TokenCount::Unavailable,
            TokenCount::Available(2),
            TokenCount::Unavailable,
            TokenCount::Available(index + 3),
        ),
        cumulative_usage: None,
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        project: Some(ProjectAlias::new("tokenmaster").expect("project")),
        originator: None,
        activity: ActivityCounts::default(),
    })
    .expect("valid observation");
    Canonicalizer::new()
        .canonicalize(&draft)
        .expect("canonical event")
}

fn batch(event_count: usize) -> AppendBatch {
    let next_offset = u64::try_from(event_count).expect("event count") * 100;
    AppendBatch::new(AppendBatchParts {
        source_key: SourceKey::from_bytes([1; 32]),
        expected_generation: 0,
        expected_committed_offset: 0,
        expected_scan_offset: 0,
        events: (0..event_count)
            .map(event)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        previous_partial_chunk: None,
        chunk_updates: vec![
            StoredSourceChunk::new(
                0,
                u32::try_from(next_offset).expect("covered length"),
                [4; 32],
            )
            .expect("source chunk"),
        ]
        .into_boxed_slice(),
        next_checkpoint: checkpoint(next_offset),
        diagnostic_count_delta: 0,
    })
    .expect("append batch")
}

fn percentile_95(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    samples[(samples.len() * 95).div_ceil(100) - 1]
}

fn sqlite_resident_bytes(path: &std::path::Path) -> u64 {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let mut candidate = path.as_os_str().to_os_string();
            candidate.push(suffix);
            std::fs::metadata(candidate).map_or(0, |metadata| metadata.len())
        })
        .sum()
}

#[test]
#[ignore = "reference-machine aggregate append performance gate; run explicitly in release mode"]
fn aggregate_trigger_append_p95_stays_within_measured_budgets() {
    for event_count in [1_usize, 32, 256] {
        let append = batch(event_count);
        let mut baseline_p95: Option<Duration> = None;
        for aggregate_ready in [false, true] {
            let mut samples = Vec::with_capacity(SAMPLE_COUNT);
            let mut database_bytes = Vec::with_capacity(SAMPLE_COUNT);
            for sample in 0..SAMPLE_COUNT {
                let directory = TempDir::new().expect("temporary directory");
                let path = directory.path().join(format!(
                    "aggregate-{event_count}-{aggregate_ready}-{sample}.sqlite3"
                ));
                let mut store = UsageStore::open(&path).expect("usage store");
                store
                    .register_source(&registration())
                    .expect("register source");
                if !aggregate_ready {
                    drop(store);
                    let connection = Connection::open(&path).expect("aggregate state writer");
                    connection
                        .execute(
                            "UPDATE usage_aggregate_state
                             SET state = 'rebuild_required', rebuild_total_events = 0
                             WHERE singleton_id = 1",
                            [],
                        )
                        .expect("disable aggregate maintenance");
                    drop(connection);
                    store = UsageStore::open(&path).expect("reopen usage store");
                }
                let started = Instant::now();
                store
                    .apply_append_batch(&append)
                    .expect("apply measured append");
                samples.push(started.elapsed());
                drop(store);
                database_bytes.push(sqlite_resident_bytes(&path));
            }
            let p95 = percentile_95(&mut samples);
            database_bytes.sort_unstable();
            let median_bytes = database_bytes[database_bytes.len() / 2];
            eprintln!(
                "aggregate append events={event_count} ready={aggregate_ready} p95_ms={:.3} median_db_bytes={median_bytes}",
                p95.as_secs_f64() * 1_000.0
            );
            if aggregate_ready {
                let baseline = baseline_p95.expect("baseline measured first");
                let total_budget = if event_count == 1 {
                    APPEND_P95_BUDGET
                } else if event_count == 32 {
                    SMALL_CATCH_UP_P95_BUDGET
                } else {
                    CATCH_UP_P95_BUDGET
                };
                assert!(
                    p95 < total_budget,
                    "{event_count}-event append p95 {p95:?} exceeded {total_budget:?}"
                );
                assert!(
                    p95.as_secs_f64() <= baseline.as_secs_f64() * MAX_AGGREGATE_OVERHEAD_RATIO,
                    "{event_count}-event aggregate p95 {p95:?} regressed baseline {baseline:?}"
                );
            } else {
                baseline_p95 = Some(p95);
            }
        }
    }
}
