use std::{mem::size_of, sync::Arc};

use tempfile::TempDir;
use tokenmaster_product::{
    ProductAttemptGeneration, ProductReducer, ProductRuntimeStatus, ProductSectionKind,
    ProductSnapshot,
};
use tokenmaster_query::{QueryClock, QueryError, QueryService, QueryTimeSample};
use tokenmaster_store::UsageStore;

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("non-zero attempt")
}

fn ten_thousand_replacements_retain_one_fixed_product_snapshot() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("constant-product.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");
    let source = service.product_data_status().expect("source status");
    drop(service);

    let mut reducer = ProductReducer::new();
    for generation in 1..=10_000 {
        reducer
            .publish_data_status(attempt(generation), source.clone())
            .expect("replace product status");
    }
    let current = reducer.snapshot();
    assert_eq!(current.generation().get(), 10_000);
    assert_eq!(
        current.data_status().attempt_generation(),
        Some(attempt(10_000))
    );
    assert_eq!(current.data_status().kind(), ProductSectionKind::Ready);
    let payload = current.data_status().payload().expect("current payload");
    assert_eq!(Arc::strong_count(payload), 1);
    assert_eq!(
        current.runtime().usage().kind(),
        ProductSectionKind::Waiting
    );
    assert_eq!(
        current.runtime().quota().kind(),
        ProductSectionKind::Waiting
    );
    assert_eq!(
        current.runtime().reminder().kind(),
        ProductSectionKind::Waiting
    );
    assert_eq!(current.runtime().git().kind(), ProductSectionKind::Waiting);
    assert!(size_of::<ProductSnapshot>() <= 2_048);
    assert!(size_of::<ProductRuntimeStatus>() <= 1_024);
    assert_eq!(
        size_of::<ProductReducer>(),
        size_of::<Arc<ProductSnapshot>>()
    );
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Default)]
struct ResourceCounts {
    private_bytes: usize,
    handles: u32,
    threads: u32,
    user_objects: u32,
    gdi_objects: u32,
}

#[cfg(windows)]
const PRIVATE_RETURN_TOLERANCE: usize = 2_097_152;

#[cfg(windows)]
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
    // SAFETY: the process pseudo-handle is valid and `handles` is writable.
    unsafe { GetProcessHandleCount(process, &raw mut handles) }.expect("process handle count");
    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).expect("counter size"),
        ..Default::default()
    };
    // SAFETY: the destination is live, writable, and correctly sized.
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

#[cfg(windows)]
fn exercise_open_capture_drop(path: &std::path::Path) {
    for _ in 0..32 {
        let mut service = QueryService::open(path, FixedClock).expect("open query service");
        let status = service.product_data_status().expect("capture status");
        assert_eq!(status.payload().usage().scope_count(), 0);
        drop(service);
    }
}

#[cfg(windows)]
fn retained_private_floor(samples: &[ResourceCounts]) -> usize {
    let mut values = samples
        .iter()
        .map(|sample| sample.private_bytes)
        .collect::<Vec<_>>();
    values.sort_unstable();
    values[1.min(values.len() - 1)]
}

#[cfg(windows)]
fn stable_warmup_baseline(samples: &[ResourceCounts]) -> Option<ResourceCounts> {
    const WINDOW: usize = 8;
    let candidate = samples.get(samples.len().checked_sub(WINDOW * 2)?..)?;
    let topology = candidate[0];
    if candidate.iter().any(|sample| {
        sample.handles != topology.handles
            || sample.threads != topology.threads
            || sample.user_objects != topology.user_objects
            || sample.gdi_objects != topology.gdi_objects
    }) {
        return None;
    }
    let (previous, current) = candidate.split_at(WINDOW);
    let previous_floor = retained_private_floor(previous);
    let current_floor = retained_private_floor(current);
    if current_floor > previous_floor.saturating_add(1_048_576) {
        return None;
    }
    let retained_floor = previous_floor.max(current_floor);
    let retained_ceiling = candidate
        .iter()
        .map(|sample| sample.private_bytes)
        .max()
        .expect("warmup candidate");
    if retained_ceiling > retained_floor.saturating_add(PRIVATE_RETURN_TOLERANCE) {
        return None;
    }
    Some(ResourceCounts {
        private_bytes: retained_floor,
        handles: topology.handles,
        threads: topology.threads,
        user_objects: topology.user_objects,
        gdi_objects: topology.gdi_objects,
    })
}

#[cfg(windows)]
fn bimodal_warmup_with_repeated_low_outliers_is_not_a_plateau() {
    let private_bytes = [
        3_670_016, 3_977_216, 5_095_424, 5_107_712, 5_107_712, 6_045_696, 4_132_864, 3_723_264,
        5_115_904, 6_053_888, 6_053_888, 3_526_656, 6_053_888, 3_547_136, 6_053_888, 6_062_080,
    ];
    let samples = private_bytes.map(|private_bytes| ResourceCounts {
        private_bytes,
        handles: 128,
        threads: 4,
        user_objects: 1,
        gdi_objects: 0,
    });

    assert!(
        stable_warmup_baseline(&samples).is_none(),
        "a bimodal warmup with a retained high plateau must continue warming"
    );
}

#[cfg(windows)]
fn repeated_status_open_capture_drop_returns_process_resources() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("status-resource.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));

    let mut warmup = Vec::with_capacity(64);
    let mut baseline = None;
    for _ in 0..64 {
        exercise_open_capture_drop(&path);
        warmup.push(resource_counts());
        baseline = stable_warmup_baseline(&warmup);
        if baseline.is_some() {
            break;
        }
    }
    let baseline = baseline.unwrap_or_else(|| {
        panic!("status resources did not reach a stable warmup plateau: {warmup:?}")
    });

    let mut measured = Vec::with_capacity(16);
    for _ in 0..16 {
        exercise_open_capture_drop(&path);
        let sample = resource_counts();
        assert!(
            sample.handles <= baseline.handles.saturating_add(1),
            "status handles grew: baseline={baseline:?}, sample={sample:?}"
        );
        assert!(
            sample.threads <= baseline.threads,
            "status threads grew: baseline={baseline:?}, sample={sample:?}"
        );
        assert!(
            sample.user_objects <= baseline.user_objects
                && sample.gdi_objects <= baseline.gdi_objects,
            "status GUI objects grew: baseline={baseline:?}, sample={sample:?}"
        );
        measured.push(sample);
    }
    let return_minima = measured
        .chunks_exact(4)
        .map(|window| {
            window
                .iter()
                .map(|sample| sample.private_bytes)
                .min()
                .expect("private window")
        })
        .collect::<Vec<_>>();
    assert!(
        return_minima.iter().all(|minimum| {
            *minimum
                <= baseline
                    .private_bytes
                    .saturating_add(PRIVATE_RETURN_TOLERANCE)
        }),
        "status private bytes did not return: baseline={baseline:?}, warmup={warmup:?}, \
         measured={measured:?}, minima={return_minima:?}"
    );
    println!(
        "product_resource_contract: pass rounds={} captures={} baseline={baseline:?} return_minima={return_minima:?}",
        measured.len(),
        (warmup.len() + measured.len()) * 32
    );
}

#[cfg(windows)]
fn main() {
    ten_thousand_replacements_retain_one_fixed_product_snapshot();
    bimodal_warmup_with_repeated_low_outliers_is_not_a_plateau();
    repeated_status_open_capture_drop_returns_process_resources();
}

#[cfg(not(windows))]
fn main() {
    ten_thousand_replacements_retain_one_fixed_product_snapshot();
    println!("product_resource_contract: pass platform=non-windows");
}
