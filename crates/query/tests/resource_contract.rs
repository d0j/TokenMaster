#![cfg(windows)]

use std::{mem::size_of, path::Path, sync::Mutex};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_pricing::{AliasOverrideDraft, OverrideDraft, OverrideSnapshot};
use tokenmaster_query::{
    CalendarDate, CostMode, LatestActivityRequest, PageSize, PricingEngine, QueryClock, QueryError,
    QueryService, QueryTimeSample, UsageAnalyticsRequest, UsageBreakdownKind, UsageRange,
    UsageSeriesSelection, UsageSessionPageRequest, UsageTimeZone, WeekStart,
};
use tokenmaster_store::{AggregateRebuildStatus, UsageStore};

const SOURCE_KEY: [u8; 32] = [7; 32];
const PRIVATE_PLATEAU_WARMUP_ROUNDS: usize = 8;
static RESOURCE_TEST_LOCK: Mutex<()> = Mutex::new(());

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
        Ok(QueryTimeSample::new(1, 1))
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

fn verify_resource_plateau(
    label: &str,
    mut exercise_round: impl FnMut(),
    measured_rounds: usize,
    private_budget: usize,
) {
    let mut warmup_samples = [ResourceCounts::default(); PRIVATE_PLATEAU_WARMUP_ROUNDS];
    for sample_slot in &mut warmup_samples {
        exercise_round();
        let sample = resource_counts();
        *sample_slot = sample;
    }

    let mut plateau = warmup_samples[0];
    for sample in &warmup_samples[1..] {
        plateau.private_bytes = plateau.private_bytes.max(sample.private_bytes);
        plateau.handles = plateau.handles.max(sample.handles);
        plateau.threads = plateau.threads.max(sample.threads);
        plateau.user_objects = plateau.user_objects.max(sample.user_objects);
        plateau.gdi_objects = plateau.gdi_objects.max(sample.gdi_objects);
    }
    let private_limit = plateau.private_bytes.saturating_add(private_budget);
    for _ in 0..measured_rounds {
        exercise_round();
        let sample = resource_counts();
        assert_structural_plateau(label, plateau, sample);
        assert!(
            sample.private_bytes <= private_limit,
            "{label} private bytes grew after plateau: \
             plateau={plateau:?}, sample={sample:?}, budget={private_budget}"
        );
    }
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

#[test]
fn repeated_open_query_drop_returns_resources_to_a_stable_plateau() {
    let _serial = RESOURCE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        8,
        1_048_576,
    );
}

#[test]
fn repeated_aggregate_session_and_resumable_rebuild_cycles_stay_on_a_resource_plateau() {
    let _serial = RESOURCE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
}
