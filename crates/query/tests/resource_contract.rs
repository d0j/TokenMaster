#![cfg(windows)]

use std::{mem::size_of, path::Path};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample,
    QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaWindowDefinition,
    QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey, QuotaWindowSemantics,
    UsageProviderId,
};
use tokenmaster_pricing::{AliasOverrideDraft, OverrideDraft, OverrideSnapshot};
use tokenmaster_query::{
    CalendarDate, CostMode, LatestActivityRequest, PageSize, PricingEngine, QueryClock, QueryError,
    QueryService, QueryTimeSample, QuotaCurrentRequest, QuotaTransitionPageRequest,
    UsageAnalyticsRequest, UsageBreakdownKind, UsageRange, UsageSeriesSelection,
    UsageSessionPageRequest, UsageTimeZone, WeekStart,
};
use tokenmaster_store::{AggregateRebuildStatus, UsageStore};

const SOURCE_KEY: [u8; 32] = [7; 32];
const PRIVATE_PLATEAU_WARMUP_ROUNDS: usize = 8;
const PRIVATE_PLATEAU_MAX_WARMUP_ROUNDS: usize = 64;

#[derive(Clone, Copy, Debug, Default)]
struct ResourceCounts {
    private_bytes: usize,
    handles: u32,
    threads: u32,
    user_objects: u32,
    gdi_objects: u32,
}

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_000_000, 1))
    }
}

fn seed_empty_archive(path: &Path) {
    drop(UsageStore::open(path).expect("create archive"));
    let connection = rusqlite::Connection::open(path).expect("checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

fn seed_bounded_current_archive(path: &Path, event_count: i64) {
    seed_empty_archive(path);
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
             ) VALUES (?1, 'codex', 'default', 'resource-private-source', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("source");
    transaction
        .execute_batch(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completed_at_ms, completion_state,
               expected_scope_count
             ) VALUES (1, 1, 1000, 'complete', 1);
             INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1, 1000, 'complete');
             INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 1, 1, 1, 1, 1);
             UPDATE usage_archive_state
             SET archive_generation = 1, current_revision_id = 0,
                 latest_complete_scan_set_id = 1, incremental_state = 'complete'
             WHERE singleton_id = 1;",
        )
        .expect("publication metadata");
    transaction
        .execute(
            "WITH RECURSIVE series(value) AS (
               VALUES(1)
               UNION ALL
               SELECT value + 1 FROM series WHERE value < ?1
             )
             INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, project_alias, activity_read,
               activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents,
               activity_terminal
             )
             SELECT unhex(printf('%064x', value)), 'event-' || value, ?2, 0, value,
                    0, 0, 0, 'codex', 'default', 'private-session-' || value,
                    'resource-private-source', value, 0,
                    CASE value % 2 WHEN 0 THEN 'gpt-a' ELSE 'gpt-b' END,
                    value, NULL, 1, NULL, value + 1, 0, 'no', 'tokenmaster',
                    1, 0, 0, 0, 0, 0, 0, 0
             FROM series",
            params![event_count, SOURCE_KEY.as_slice()],
        )
        .expect("bulk events");
    transaction.commit().expect("commit fixture");
    drop(connection);
    let connection = Connection::open(path).expect("checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

fn quota_key(index: u64) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("resource-account").expect("account"),
            None,
        ),
        QuotaWindowId::new(format!("resource-{index}")).expect("window"),
    )
}

fn quota_definition(index: u64) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: quota_key(index),
        revision: 1,
        label_key: format!("quota.resource-{index}"),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: None,
        reset_thresholds: None,
    })
    .expect("quota definition")
}

fn quota_observation_id(window: u64, observation: u64) -> QuotaObservationId {
    let mut bytes = [0_u8; 32];
    bytes[16..24].copy_from_slice(&window.to_be_bytes());
    bytes[24..].copy_from_slice(&observation.to_be_bytes());
    QuotaObservationId::from_bytes(bytes)
}

fn quota_sample(index: u64, observation: u64) -> QuotaSample {
    let observed_at_ms = i64::try_from(observation * 1_000).expect("observation time");
    QuotaSample::new(QuotaSampleParts {
        key: quota_key(index),
        observation_id: quota_observation_id(index, observation),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 10_000,
        stale_after_ms: observed_at_ms + 20_000,
        provider_epoch_id: Some(
            QuotaProviderEpochId::new(format!("resource-epoch-{index}-{observation}"))
                .expect("provider epoch"),
        ),
        used_ratio: Some(QuotaRatio::new(100_000).expect("used ratio")),
        remaining_ratio: Some(QuotaRatio::new(900_000).expect("remaining ratio")),
        units: None,
        advertised_resets_at_ms: Some(observed_at_ms + 100_000),
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderOfficial,
        confidence: QuotaConfidence::High,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    })
    .expect("quota sample")
}

fn seed_bounded_quota(path: &Path) {
    let mut store = UsageStore::open(path).expect("quota writer");
    for index in 0..4 {
        let definition = quota_definition(index);
        for observation in 1..=9 {
            store
                .apply_quota_observation(&definition, &quota_sample(index, observation))
                .expect("quota observation");
        }
    }
    drop(store);
    let connection = Connection::open(path).expect("quota checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("quota checkpoint");
}

fn open_query_drop(path: &Path) {
    let mut service = QueryService::open(path, FixedClock).expect("open query service");
    let snapshot = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(16).expect("page size"),
        ))
        .expect("query empty archive");
    assert!(snapshot.payload().items().is_empty());
}

fn exercise_bounded_snapshots(path: &Path) {
    let mut service = QueryService::open(path, FixedClock).expect("open query service");
    let override_snapshot = OverrideSnapshot::build(&[OverrideDraft::Alias(AliasOverrideDraft {
        alias: "resource-alias",
        target: "gpt-5.6-sol",
    })])
    .expect("bounded override snapshot");
    service.replace_pricing(
        PricingEngine::new(override_snapshot.clone()),
        CostMode::Calculated,
    );
    let analytics = UsageAnalyticsRequest::new(
        UsageRange::custom(
            CalendarDate::new(1970, 1, 1).expect("range start"),
            CalendarDate::new(1971, 2, 5).expect("range end"),
        )
        .expect("400-day range"),
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        vec![
            UsageBreakdownKind::Model,
            UsageBreakdownKind::Project,
            UsageBreakdownKind::Provider,
            UsageBreakdownKind::Profile,
        ],
    )
    .expect("analytics request");
    let snapshot = service
        .usage_analytics(analytics)
        .expect("bounded analytics snapshot");
    assert_eq!(snapshot.payload().series().len(), 400);
    assert_eq!(snapshot.payload().breakdowns().len(), 4);

    service.replace_pricing(PricingEngine::embedded(), CostMode::Reported);
    let page = service
        .usage_sessions(
            UsageSessionPageRequest::first(PageSize::new(16).expect("page"), Vec::new())
                .expect("session request"),
        )
        .expect("first session page");
    assert_eq!(page.payload().sessions().len(), 16);
    assert!(page.payload().has_more());
    let detail_key = page.payload().sessions()[0].key().clone();
    let continuation = UsageSessionPageRequest::continuation(
        PageSize::new(16).expect("page"),
        page.payload().next_cursor().expect("continuation").clone(),
        Vec::new(),
    )
    .expect("continuation request");
    service.replace_pricing(PricingEngine::new(override_snapshot), CostMode::Auto);
    let next = service
        .usage_sessions(continuation)
        .expect("continuation page");
    assert_eq!(next.payload().sessions().len(), 16);
    let detail = service
        .usage_session_detail(detail_key)
        .expect("session detail");
    assert!(detail.payload().detail().is_some());
}

fn exercise_bounded_quota_snapshots(path: &Path) {
    let keys = (0..4).map(quota_key).collect::<Vec<_>>();
    let mut service = QueryService::open(path, FixedClock).expect("quota query service");
    let current = service
        .quota_windows(QuotaCurrentRequest::new(keys.clone()).expect("quota current request"))
        .expect("quota current snapshot");
    assert_eq!(current.payload().windows().len(), 4);
    assert!(
        current
            .payload()
            .windows()
            .iter()
            .all(|window| window.snapshot().is_some())
    );
    for key in keys {
        let first = service
            .quota_transitions(
                QuotaTransitionPageRequest::first(
                    key.clone(),
                    PageSize::new(4).expect("quota page"),
                )
                .expect("quota first request"),
            )
            .expect("quota first page");
        assert_eq!(first.payload().transitions().len(), 4);
        assert!(first.payload().has_more());
        let second = service
            .quota_transitions(
                QuotaTransitionPageRequest::continuation(
                    key,
                    PageSize::new(4).expect("quota page"),
                    first
                        .payload()
                        .next_cursor()
                        .cloned()
                        .expect("quota cursor"),
                )
                .expect("quota continuation request"),
            )
            .expect("quota continuation page");
        assert_eq!(second.payload().transitions().len(), 4);
        assert!(!second.payload().has_more());
    }
}

fn resource_counts() -> ResourceCounts {
    use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows::Win32::System::Threading::{
        GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetCurrentProcessId, GetGuiResources,
        GetProcessHandleCount,
    };

    let process = unsafe { GetCurrentProcess() };
    let mut handles = 0_u32;
    // SAFETY: `handles` is writable and `process` is the valid process pseudo-handle.
    unsafe { GetProcessHandleCount(process, &raw mut handles) }
        .expect("query process handle count");
    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).expect("counter size"),
        ..Default::default()
    };
    // SAFETY: the destination is live, correctly sized writable storage.
    unsafe {
        K32GetProcessMemoryInfo(
            process,
            (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
            memory.cb,
        )
    }
    .expect("query process memory");
    let user_objects = unsafe { GetGuiResources(process, GR_USEROBJECTS) };
    let gdi_objects = unsafe { GetGuiResources(process, GR_GDIOBJECTS) };

    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.expect("thread snapshot");
    let process_id = unsafe { GetCurrentProcessId() };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).expect("thread entry size"),
        ..Default::default()
    };
    let mut threads = 0_u32;
    if unsafe { Thread32First(snapshot, &raw mut entry) }.is_ok() {
        loop {
            if entry.th32OwnerProcessID == process_id {
                threads = threads.checked_add(1).expect("thread count");
            }
            match unsafe { Thread32Next(snapshot, &raw mut entry) } {
                Ok(()) => {}
                Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(error) => panic!("enumerate threads: {error}"),
            }
        }
    }
    unsafe { CloseHandle(snapshot) }.expect("close thread snapshot");

    ResourceCounts {
        private_bytes: memory.PrivateUsage,
        handles,
        threads,
        user_objects,
        gdi_objects,
    }
}

fn assert_structural_plateau(label: &str, baseline: ResourceCounts, sample: ResourceCounts) {
    assert!(
        sample.handles <= baseline.handles.saturating_add(1),
        "{label} handles grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.threads <= baseline.threads,
        "{label} threads grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.user_objects <= baseline.user_objects && sample.gdi_objects <= baseline.gdi_objects,
        "{label} GUI objects grew: baseline={baseline:?}, sample={sample:?}"
    );
}

fn private_return_windows_accept_transient_allocator_spikes() {
    let baseline = [4_800_000, 4_700_000, 4_900_000, 4_750_000];
    let measured = [
        6_300_000, 4_760_000, 6_100_000, 4_780_000, 6_200_000, 4_790_000, 6_000_000, 4_770_000,
    ];

    assert!(private_return_windows_within_budget(
        &baseline, &measured, 4, 100_000
    ));
}

fn private_warmup_accepts_one_transient_allocator_trough() {
    let mut samples = Vec::new();
    samples.extend((0..8).map(|offset| ResourceCounts {
        private_bytes: if offset == 2 { 5_390_000 } else { 6_700_000 },
        handles: 112,
        threads: 1,
        user_objects: 1,
        gdi_objects: 0,
    }));
    samples.extend((0..8).map(|_| ResourceCounts {
        private_bytes: 6_750_000,
        handles: 112,
        threads: 1,
        user_objects: 1,
        gdi_objects: 0,
    }));

    let plateau =
        stable_warmup_plateau(&samples, 8, 1_048_576).expect("single trough is not a plateau");
    assert_eq!(plateau.private_floor, 6_750_000);
}

fn private_return_windows_reject_sustained_retained_growth() {
    let baseline = [4_800_000, 4_700_000, 4_900_000, 4_750_000];
    let measured = [
        4_760_000, 4_780_000, 4_790_000, 4_770_000, 4_900_001, 4_910_000, 4_920_000, 4_930_000,
    ];

    assert!(!private_return_windows_within_budget(
        &baseline, &measured, 4, 200_000
    ));
    assert!(!private_return_windows_within_budget(
        &baseline,
        &measured[..7],
        4,
        200_000
    ));
}

fn private_warmup_restarts_after_topology_and_allocator_phase_changes() {
    let mut samples = Vec::new();
    samples.extend((0..8).map(|offset| ResourceCounts {
        private_bytes: 4_700_000 + offset * 10_000,
        handles: 119,
        threads: 4,
        user_objects: 1,
        gdi_objects: 0,
    }));
    samples.extend((0..8).map(|offset| ResourceCounts {
        private_bytes: if offset < 2 {
            3_100_000
        } else {
            4_700_000 + offset * 20_000
        },
        handles: 119,
        threads: 1,
        user_objects: 1,
        gdi_objects: 0,
    }));
    assert!(stable_warmup_plateau(&samples, 8, 1_048_576).is_none());

    samples.extend((0..8).map(|offset| ResourceCounts {
        private_bytes: 5_600_000 + offset * 20_000,
        handles: 119,
        threads: 1,
        user_objects: 1,
        gdi_objects: 0,
    }));
    assert!(stable_warmup_plateau(&samples, 8, 1_048_576).is_none());

    samples.extend((0..8).map(|offset| ResourceCounts {
        private_bytes: 5_700_000 + offset * 20_000,
        handles: 119,
        threads: 1,
        user_objects: 1,
        gdi_objects: 0,
    }));
    let plateau = stable_warmup_plateau(&samples, 8, 1_048_576).expect("stable latest plateau");
    assert_eq!(plateau.samples.len(), 8);
    assert_eq!(plateau.private_floor, 5_720_000);
    assert_eq!(
        plateau
            .samples
            .iter()
            .map(|sample| sample.private_bytes)
            .min(),
        Some(5_700_000)
    );
    assert!(plateau.samples.iter().all(|sample| sample.threads == 1));
}

struct StableWarmupPlateau<'a> {
    samples: &'a [ResourceCounts],
    private_floor: usize,
}

fn retained_private_floor(values: impl IntoIterator<Item = usize>) -> Option<usize> {
    let mut values = values.into_iter();
    let mut lowest = values.next()?;
    let mut second_lowest = None;
    for value in values {
        if value < lowest {
            second_lowest = Some(lowest);
            lowest = value;
        } else if second_lowest.is_none_or(|current| value < current) {
            second_lowest = Some(value);
        }
    }
    Some(second_lowest.unwrap_or(lowest))
}

fn stable_warmup_plateau<'a>(
    samples: &'a [ResourceCounts],
    window_size: usize,
    private_budget: usize,
) -> Option<StableWarmupPlateau<'a>> {
    let required_samples = window_size.checked_mul(2)?;
    if window_size == 0 || samples.len() < required_samples {
        return None;
    }
    let candidate = &samples[samples.len() - required_samples..];
    let topology = candidate[0];
    if candidate.iter().any(|sample| {
        sample.handles != topology.handles
            || sample.threads != topology.threads
            || sample.user_objects != topology.user_objects
            || sample.gdi_objects != topology.gdi_objects
    }) {
        return None;
    }
    let (previous_window, current_window) = candidate.split_at(window_size);
    let previous_floor =
        retained_private_floor(previous_window.iter().map(|sample| sample.private_bytes))?;
    let current_floor =
        retained_private_floor(current_window.iter().map(|sample| sample.private_bytes))?;
    if current_floor > previous_floor.saturating_add(private_budget) {
        return None;
    }
    Some(StableWarmupPlateau {
        samples: current_window,
        private_floor: previous_floor.max(current_floor),
    })
}

fn private_return_windows_within_budget(
    baseline: &[usize],
    measured: &[usize],
    window_size: usize,
    private_budget: usize,
) -> bool {
    if baseline.is_empty()
        || measured.is_empty()
        || window_size == 0
        || !measured.len().is_multiple_of(window_size)
    {
        return false;
    }
    let Some(baseline_floor) = baseline.iter().copied().min() else {
        return false;
    };
    let private_limit = baseline_floor.saturating_add(private_budget);
    measured.chunks_exact(window_size).all(|window| {
        window
            .iter()
            .copied()
            .min()
            .is_some_and(|floor| floor <= private_limit)
    })
}

fn verify_resource_plateau(
    label: &str,
    mut exercise_round: impl FnMut(),
    measured_rounds: usize,
    private_budget: usize,
) {
    let mut warmup_samples = Vec::with_capacity(PRIVATE_PLATEAU_MAX_WARMUP_ROUNDS);
    for _ in 0..PRIVATE_PLATEAU_MAX_WARMUP_ROUNDS {
        exercise_round();
        let sample = resource_counts();
        warmup_samples.push(sample);
        if stable_warmup_plateau(
            &warmup_samples,
            PRIVATE_PLATEAU_WARMUP_ROUNDS,
            private_budget,
        )
        .is_some()
        {
            break;
        }
    }

    let stable_plateau = stable_warmup_plateau(
        &warmup_samples,
        PRIVATE_PLATEAU_WARMUP_ROUNDS,
        private_budget,
    )
    .unwrap_or_else(|| {
        panic!(
            "{label} did not establish a topology-stable retained plateau within \
             {PRIVATE_PLATEAU_MAX_WARMUP_ROUNDS} rounds: samples={warmup_samples:?}"
        )
    });
    let plateau_samples = stable_plateau.samples;
    let mut plateau = plateau_samples[0];
    for sample in &plateau_samples[1..] {
        plateau.private_bytes = plateau.private_bytes.max(sample.private_bytes);
        plateau.handles = plateau.handles.max(sample.handles);
        plateau.threads = plateau.threads.max(sample.threads);
        plateau.user_objects = plateau.user_objects.max(sample.user_objects);
        plateau.gdi_objects = plateau.gdi_objects.max(sample.gdi_objects);
    }
    let mut first_measured = None;
    let mut highest_measured = ResourceCounts::default();
    let mut last_measured = ResourceCounts::default();
    let mut private_samples = Vec::with_capacity(measured_rounds);
    for _ in 0..measured_rounds {
        exercise_round();
        let sample = resource_counts();
        private_samples.push(sample.private_bytes);
        first_measured.get_or_insert(sample);
        if sample.private_bytes > highest_measured.private_bytes {
            highest_measured = sample;
        }
        last_measured = sample;
        assert_structural_plateau(label, plateau, sample);
    }
    let warmup_private = [stable_plateau.private_floor];
    let return_window_minima = private_samples
        .chunks_exact(PRIVATE_PLATEAU_WARMUP_ROUNDS)
        .filter_map(|window| window.iter().copied().min())
        .collect::<Vec<_>>();
    assert!(
        private_return_windows_within_budget(
            &warmup_private,
            &private_samples,
            PRIVATE_PLATEAU_WARMUP_ROUNDS,
            private_budget,
        ),
        "{label} private bytes did not return to the retained plateau: plateau={plateau:?}, \
         first={first_measured:?}, highest={highest_measured:?}, last={last_measured:?}, \
         budget={private_budget}, return_window_minima={return_window_minima:?}, \
         private_samples={private_samples:?}"
    );
}

fn exercise_rebuild_cycle(path: &Path) {
    let connection = Connection::open(path).expect("rebuild fixture connection");
    connection
        .execute(
            "UPDATE usage_aggregate_state
             SET state = 'rebuild_required', rebuild_aggregate_generation = NULL,
                 rebuild_dataset_kind = NULL, rebuild_cursor_fingerprint = NULL,
                 rebuild_processed_events = 0,
                 rebuild_total_events = current_event_count + legacy_event_count
             WHERE singleton_id = 1",
            [],
        )
        .expect("require rebuild");
    drop(connection);

    let mut status = AggregateRebuildStatus::Rebuilding;
    for _ in 0..64 {
        let mut store = UsageStore::open(path).expect("resume rebuild");
        status = store
            .rebuild_aggregates_page(256)
            .expect("one cooperative rebuild page")
            .status();
        drop(store);
        if status == AggregateRebuildStatus::Ready {
            break;
        }
    }
    assert_eq!(status, AggregateRebuildStatus::Ready);
}

fn repeated_open_query_drop_returns_resources_to_a_stable_plateau() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("resource.sqlite3");
    seed_empty_archive(&path);

    verify_resource_plateau(
        "query open/drop",
        || {
            for _ in 0..128 {
                open_query_drop(&path);
            }
        },
        16,
        1_048_576,
    );
}

fn repeated_aggregate_session_and_resumable_rebuild_cycles_stay_on_a_resource_plateau() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("aggregate-resource.sqlite3");
    seed_bounded_current_archive(&path, 512);

    verify_resource_plateau(
        "aggregate query",
        || {
            for _ in 0..4 {
                exercise_bounded_snapshots(&path);
            }
        },
        8,
        2_097_152,
    );
    verify_resource_plateau(
        "resumable rebuild",
        || exercise_rebuild_cycle(&path),
        8,
        2_097_152,
    );
    seed_bounded_quota(&path);
    verify_resource_plateau(
        "quota current/history/reopen",
        || exercise_bounded_quota_snapshots(&path),
        8,
        2_097_152,
    );
}

fn main() {
    private_return_windows_accept_transient_allocator_spikes();
    private_warmup_accepts_one_transient_allocator_trough();
    private_return_windows_reject_sustained_retained_growth();
    private_warmup_restarts_after_topology_and_allocator_phase_changes();
    repeated_open_query_drop_returns_resources_to_a_stable_plateau();
    repeated_aggregate_session_and_resumable_rebuild_cycles_stay_on_a_resource_plateau();
    println!("resource_contract: pass");
}
