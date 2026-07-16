#![cfg(windows)]

use std::{mem::size_of, time::Duration, time::Instant};

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    UsageProviderId,
};
use tokenmaster_query::{
    BenefitChangePageRequest, BenefitCurrentRequest, PageSize, QueryClock, QueryError,
    QueryService, QueryTimeSample,
};
use tokenmaster_store::{MAX_BENEFIT_CHANGES_PER_SCOPE, UsageStore};

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;
const CURRENT_LOTS: usize = 64;
const HISTORY_CHANGES: u64 = 2_048;
const OPERATION_BUDGET: Duration = Duration::from_secs(2);
const BENEFIT_STORE_QUERY_SOURCE: &str = include_str!("../../store/src/usage/query/benefit.rs");

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(OBSERVED_AT_MS + 10_000, 1))
    }
}

#[derive(Clone, Copy, Debug)]
struct ResourceCounts {
    private_bytes: usize,
    handles: u32,
    threads: u32,
    user_objects: u32,
    gdi_objects: u32,
}

fn scope(account: &str) -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new(account).expect("account"),
        None,
    )
}

fn lot(id: u64, quantity: u64) -> BenefitLotObservation {
    let mut opaque = [0_u8; 32];
    opaque[24..].copy_from_slice(&id.to_be_bytes());
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes(opaque),
        kind: match id % 4 {
            0 => BenefitKind::BankedRateLimitReset,
            1 => BenefitKind::UsageCredit,
            2 => BenefitKind::TemporaryUsage,
            _ => BenefitKind::Unknown,
        },
        quantity,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: None,
        expiry: BenefitExpiry::unknown(),
        source: BenefitEvidenceSource::ProviderLocal,
        confidence: BenefitConfidence::Medium,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.scale").expect("label"),
    })
    .expect("lot")
}

fn observation(
    scope: BenefitScope,
    id: u64,
    lots: Vec<BenefitLotObservation>,
) -> BenefitInventoryObservation {
    let mut opaque = [0_u8; 32];
    opaque[24..].copy_from_slice(&id.to_be_bytes());
    let observed_at_ms = OBSERVED_AT_MS + i64::try_from(id).expect("observation time");
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id: BenefitObservationId::from_bytes(opaque),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 20_000,
        stale_after_ms: observed_at_ms + 40_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots,
    })
    .expect("observation")
}

#[test]
fn maximum_inventory_and_history_are_bounded_paged_and_return_resources() {
    assert_eq!(MAX_BENEFIT_CHANGES_PER_SCOPE, HISTORY_CHANGES);
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-scale.sqlite3");
    let current_scope = scope("scale-current");
    let history_scope = scope("scale-history");
    let mut writer = UsageStore::open(&path).expect("writer");
    writer
        .apply_benefit_observation(&observation(
            current_scope.clone(),
            1,
            (0..CURRENT_LOTS)
                .map(|index| lot(index as u64, index as u64 + 1))
                .collect(),
        ))
        .expect("maximum current inventory");
    for revision in 1..=HISTORY_CHANGES {
        writer
            .apply_benefit_observation(&observation(
                history_scope.clone(),
                revision,
                vec![lot(10_000, revision)],
            ))
            .expect("history observation");
    }
    drop(writer);
    Connection::open(&path)
        .expect("checkpoint connection")
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");

    let baseline = resource_counts();
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let current_started = Instant::now();
    let current = service
        .benefit_inventory(BenefitCurrentRequest::new(current_scope.clone()))
        .expect("current inventory");
    let current_elapsed = current_started.elapsed();
    assert_eq!(
        current
            .payload()
            .inventory()
            .expect("inventory")
            .current_lots()
            .len(),
        CURRENT_LOTS
    );
    assert!(
        current_elapsed < OPERATION_BUDGET,
        "64-lot current read exceeded budget: {current_elapsed:?}"
    );

    let mut cursor = None;
    let mut expected_sequence = HISTORY_CHANGES;
    let mut total = 0_u64;
    let mut maximum_page = Duration::ZERO;
    loop {
        let request = match cursor.take() {
            Some(cursor) => BenefitChangePageRequest::continuation(
                history_scope.clone(),
                PageSize::new(256).expect("page size"),
                cursor,
            )
            .expect("continuation"),
            None => BenefitChangePageRequest::first(
                history_scope.clone(),
                PageSize::new(256).expect("page size"),
            )
            .expect("first page"),
        };
        let started = Instant::now();
        let page = service.benefit_changes(request).expect("history page");
        maximum_page = maximum_page.max(started.elapsed());
        for change in page.payload().changes().iter() {
            assert_eq!(change.sequence(), expected_sequence);
            expected_sequence -= 1;
            total += 1;
        }
        if !page.payload().has_more() {
            break;
        }
        cursor = page.payload().next_cursor().cloned();
        assert!(cursor.is_some());
    }
    assert_eq!(total, HISTORY_CHANGES);
    assert_eq!(expected_sequence, 0);
    assert!(
        maximum_page < OPERATION_BUDGET,
        "256-row benefit history read exceeded budget: {maximum_page:?}"
    );
    drop(service);

    for _ in 0..32 {
        let snapshot = QueryService::open(&path, FixedClock)
            .expect("cycle service")
            .benefit_inventory(BenefitCurrentRequest::new(current_scope.clone()))
            .expect("cycle snapshot");
        assert_eq!(
            snapshot
                .payload()
                .inventory()
                .expect("cycle inventory")
                .current_lots()
                .len(),
            CURRENT_LOTS
        );
    }
    let returned = resource_counts();
    assert!(
        returned.handles <= baseline.handles.saturating_add(2),
        "benefit query handles grew: baseline={baseline:?}, returned={returned:?}"
    );
    assert!(
        returned.threads <= baseline.threads,
        "benefit query threads grew: baseline={baseline:?}, returned={returned:?}"
    );
    assert!(
        returned.user_objects <= baseline.user_objects
            && returned.gdi_objects <= baseline.gdi_objects,
        "benefit query GUI objects grew: baseline={baseline:?}, returned={returned:?}"
    );
    assert!(
        returned.private_bytes <= baseline.private_bytes.saturating_add(8 * 1024 * 1024),
        "benefit query retained excessive private bytes: baseline={baseline:?}, returned={returned:?}"
    );
    eprintln!(
        "benefit scale current_lots={CURRENT_LOTS} changes={HISTORY_CHANGES} \
         current_ms={:.3} max_page_ms={:.3} baseline={baseline:?} returned={returned:?}",
        current_elapsed.as_secs_f64() * 1_000.0,
        maximum_page.as_secs_f64() * 1_000.0,
    );
}

#[test]
fn redundant_current_projection_corruption_is_rejected_by_a_live_reader() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-corruption.sqlite3");
    let requested_scope = scope("corrupt-current");
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        writer
            .apply_benefit_observation(&observation(requested_scope.clone(), 1, vec![lot(1, 1)]))
            .expect("observation");
    }
    let mut service = QueryService::open(&path, FixedClock).expect("live reader");
    Connection::open(&path)
        .expect("corrupt connection")
        .execute("UPDATE benefit_lot_current SET quantity = quantity + 1", [])
        .expect("corrupt redundant projection");
    let error = service
        .benefit_inventory(BenefitCurrentRequest::new(requested_scope))
        .expect_err("corruption must fail closed");
    assert_eq!(
        error.code(),
        tokenmaster_query::QueryErrorCode::CorruptArchive
    );
}

#[test]
fn benefit_query_source_has_no_usage_dataset_scan() {
    for forbidden in [
        "FROM usage_event",
        "JOIN usage_event",
        "FROM usage_time_rollup",
        "JOIN usage_time_rollup",
        "FROM usage_session_rollup",
        "JOIN usage_session_rollup",
    ] {
        assert!(
            !BENEFIT_STORE_QUERY_SOURCE.contains(forbidden),
            "benefit query introduced a usage-dataset scan: {forbidden}"
        );
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
    unsafe { GetProcessHandleCount(process, &raw mut handles) }.expect("process handle count");
    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).expect("counter size"),
        ..Default::default()
    };
    unsafe {
        K32GetProcessMemoryInfo(
            process,
            (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
            memory.cb,
        )
    }
    .expect("process memory");
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
