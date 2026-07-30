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

// How far apart the samples inside a warmup window may sit and still count as a plateau.
// Deliberately tight, and guarded by bimodal_warmup_with_repeated_low_outliers_is_not_a_plateau:
// widening this would let a warmup that is still bouncing be mistaken for a settled one.
#[cfg(windows)]
const PRIVATE_PLATEAU_TOLERANCE: usize = 2_097_152;

// How far above the settled floor a measured window may sit before the process is called
// leaky. This was the same 2 MiB as the plateau tolerance, and it could not hold, because
// **2 MiB is smaller than this program's own noise**. Measured on CI run 30529480389, on a
// tree whose three other runs were green: private bytes across the 32 samples of a correct
// run spanned 3,035,136 to 5,988,352, a swing of 2,953,216 bytes. A criterion whose
// tolerance is below the observed noise of a correct program fails at random -- that run
// missed by 319,488 bytes on the last of four windows while handles, threads, user objects
// and GDI objects sat frozen at 105, 4, 1 and 0 across every sample.
//
// The bound therefore sits between two measured quantities rather than at a number that
// reads strict. Over sixteen known-good runs at MEASURED_ROUNDS rounds the worst drift any
// window reached was 3,067,904; the page-per-open leak below puts its last window at
// 7,995,392. This is near the geometric middle: 1.7x clear of the noise and 1.5x under the
// leak. Both margins are stated because moving the number costs one of them.
//
// Measured over windows, not medians, deliberately: the same runs give a 2.6x separation on
// the worst window against a 1.4x separation on the median, because a leak lifts every late
// window while noise spikes one. The sibling contract in crates/state allows 16 MiB for this
// measurement; this stays three times tighter.
//
// This comment used to add that a leak "climbs past any bound as the round count grows",
// which is true and was being used to excuse a bound that could not catch one -- the round
// count is fixed, so nothing grows on its own. Raising a ceiling is not what makes a leak
// visible; see MEASURED_ROUNDS, which is where that work is actually done.
#[cfg(windows)]
const PRIVATE_RETURN_TOLERANCE: usize = 5_242_880;

// How many measured rounds the drift is watched over, and the reason it is not sixteen.
//
// A bound alone cannot separate a leak from this allocator, because at sixteen rounds the
// leak is the smaller of the two. Measured over twenty-four known-good runs, drift from the
// settled floor reached 2,465,792 bytes with a median of 712,704; a leak of one retained
// 4 KiB page per open, at ROUND_CAPTURES opens a round, adds 131,072 a round and so reaches
// only about 2 MiB in sixteen. Signal under noise: no threshold can do it, and the 2 MiB
// bound that used to be here only appeared to, by also rejecting 2 of those 24 correct runs.
//
// Noise is a property of the allocator and does not grow with rounds. A leak is per round
// and does. Sixty-four rounds put the same page-per-open leak near 8 MiB, comfortably over
// the ceiling, while correct runs stay where they were. That is why the round count is the
// load-bearing number here and the tolerance is not.
#[cfg(windows)]
const MEASURED_ROUNDS: usize = 64;

// Captures per round, as exercise_open_capture_drop performs them. Named so the leak the
// contract must reject can be stated in the units the defect would actually have.
#[cfg(windows)]
const ROUND_CAPTURES: usize = 32;

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
    for _ in 0..ROUND_CAPTURES {
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
    if retained_ceiling > retained_floor.saturating_add(PRIVATE_PLATEAU_TOLERANCE) {
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

/// The lowest private-bytes reading in each window of four measured samples.
///
/// A window is used rather than a single sample because a caching allocator hands pages
/// back on its own schedule, so the question worth asking is whether the process came down
/// at all within four rounds, not whether it happened to be down when it was looked at.
#[cfg(windows)]
fn private_return_minima(measured: &[ResourceCounts]) -> Vec<usize> {
    measured
        .chunks_exact(4)
        .map(|window| {
            window
                .iter()
                .map(|sample| sample.private_bytes)
                .min()
                .expect("private window")
        })
        .collect()
}

/// Whether every measured window came back to within the tolerance of the settled floor.
///
/// Pulled out of the live test so it can be checked against recorded readings. A criterion
/// that only ever runs against whatever the machine happens to be doing cannot be shown to
/// reject anything, and this one had been rejecting correct programs.
#[cfg(windows)]
fn private_bytes_returned(baseline_private_bytes: usize, minima: &[usize]) -> bool {
    let ceiling = baseline_private_bytes.saturating_add(PRIVATE_RETURN_TOLERANCE);
    minima.iter().all(|minimum| *minimum <= ceiling)
}

/// The window minima a steady leak of `per_round` bytes would produce over the measured
/// rounds, reduced exactly the way real samples are.
///
/// Built from `MEASURED_ROUNDS` on purpose: a leak this small is only visible because it
/// accumulates, so a test written against a fixed-length series would keep passing after
/// someone shortened the measurement and removed the only reason it works.
#[cfg(windows)]
fn leaking_minima(baseline: usize, per_round: usize) -> Vec<usize> {
    let samples = (1..=MEASURED_ROUNDS)
        .map(|round| ResourceCounts {
            private_bytes: baseline + round * per_round,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    private_return_minima(&samples)
}

/// Both directions, against readings rather than against a running process.
///
/// The passing case is the exact series from CI run 30529480389, which failed under the old
/// 2 MiB bound while three other runs of the identical tree were green. The rejecting cases
/// are what a leak looks like: a floor that climbs with the rounds instead of settling.
#[cfg(windows)]
fn private_return_criterion_accepts_noise_and_rejects_growth() {
    let baseline = 3_305_472;

    let observed = [5_120_000, 3_088_384, 5_120_000, 5_722_112];
    assert!(
        private_bytes_returned(baseline, &observed),
        "the recorded readings of a correct run must be accepted: {observed:?}"
    );

    let leaking = [4_000_000, 8_000_000, 12_000_000, 16_000_000];
    assert!(
        !private_bytes_returned(baseline, &leaking),
        "a floor that climbs every window is a leak and must be rejected: {leaking:?}"
    );

    let one_late_climb = [5_120_000, 5_120_000, 5_120_000, 9_000_000];
    assert!(
        !private_bytes_returned(baseline, &one_late_climb),
        "a single window that never comes down must still be rejected: {one_late_climb:?}"
    );

    // The defect this contract exists for, in the units it would actually arrive in: one
    // retained 4 KiB page for every QueryService::open. It is deliberately built from
    // MEASURED_ROUNDS rather than written out, so the round count is load-bearing -- drop
    // it back to sixteen and this assertion fails, because sixteen rounds of it reach only
    // about 2 MiB and hide under an allocator that swings 2.4 MiB on correct code.
    let page_per_open = leaking_minima(baseline, 4_096 * ROUND_CAPTURES);
    assert!(
        !private_bytes_returned(baseline, &page_per_open),
        "one retained page per open must be rejected over {MEASURED_ROUNDS} rounds: \
         {page_per_open:?}"
    );

    assert!(
        private_return_minima(&[]).is_empty(),
        "no samples yields no windows"
    );
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

    let mut measured = Vec::with_capacity(MEASURED_ROUNDS);
    for _ in 0..MEASURED_ROUNDS {
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
    let return_minima = private_return_minima(&measured);
    assert!(
        private_bytes_returned(baseline.private_bytes, &return_minima),
        "status private bytes did not return: baseline={baseline:?}, warmup={warmup:?}, \
         measured={measured:?}, minima={return_minima:?}"
    );
    println!(
        "product_resource_contract: pass rounds={} captures={} baseline={baseline:?} return_minima={return_minima:?}",
        measured.len(),
        (warmup.len() + measured.len()) * ROUND_CAPTURES
    );
}

#[cfg(windows)]
fn main() {
    ten_thousand_replacements_retain_one_fixed_product_snapshot();
    bimodal_warmup_with_repeated_low_outliers_is_not_a_plateau();
    private_return_criterion_accepts_noise_and_rejects_growth();
    repeated_status_open_capture_drop_returns_process_resources();
}

#[cfg(not(windows))]
fn main() {
    ten_thousand_replacements_retain_one_fixed_product_snapshot();
    println!("product_resource_contract: pass platform=non-windows");
}
