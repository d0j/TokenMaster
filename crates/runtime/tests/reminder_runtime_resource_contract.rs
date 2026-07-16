#[cfg(windows)]
use std::mem::size_of;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use tempfile::TempDir;
#[cfg(windows)]
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    UsageProviderId,
};
#[cfg(windows)]
use tokenmaster_engine::{RefreshOutcome, WriterLease};
#[cfg(windows)]
use tokenmaster_runtime::{
    BenefitReminderFailure, BenefitReminderRuntime, BenefitReminderRuntimeConfig,
    RuntimeWriterLease,
};
#[cfg(windows)]
use tokenmaster_store::UsageStore;

#[cfg(windows)]
const PLATEAU_WINDOW: usize = 8;
#[cfg(windows)]
const MAX_WARMUP_ROUNDS: usize = 32;
#[cfg(windows)]
const MEASURED_ROUNDS: usize = 48;
#[cfg(windows)]
const PRIVATE_BYTES_BUDGET: usize = 4 * 1024 * 1024;

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
struct FixtureSet {
    _temp: TempDir,
    archive: PathBuf,
    busy_archive: PathBuf,
    sequence: u64,
}

#[cfg(windows)]
impl FixtureSet {
    fn new() -> Self {
        let temp = TempDir::new().expect("reminder fixture temp");
        Self {
            archive: temp.path().join("reminder.sqlite3"),
            busy_archive: temp.path().join("busy.sqlite3"),
            _temp: temp,
            sequence: 0,
        }
    }

    fn exercise_round(&mut self) {
        self.sequence = self.sequence.checked_add(1).expect("fixture sequence");
        seed_due(&self.archive, self.sequence);
        let mut runtime = start_runtime(&self.archive);
        assert_eq!(
            wait_completion(&runtime).outcome(),
            RefreshOutcome::Completed
        );
        let refresh = runtime.snapshot().expect("snapshot").refresh();
        assert_eq!(refresh.outcome(), Some(RefreshOutcome::Completed));
        assert_eq!(refresh.delivery_count(), 1);
        assert_eq!(
            runtime
                .take_notifications()
                .expect("take notifications")
                .expect("notification batch")
                .len(),
            1
        );
        assert!(
            runtime
                .acknowledge_notifications()
                .expect("acknowledge notification")
        );
        assert_eq!(
            wait_completion(&runtime).outcome(),
            RefreshOutcome::Completed
        );
        assert!(runtime.take_notifications().expect("empty batch").is_none());
        runtime.pause().expect("pause");
        runtime.resume().expect("resume");
        assert_eq!(
            wait_completion(&runtime).outcome(),
            RefreshOutcome::Completed
        );
        runtime.shutdown().expect("shutdown");

        let mut competing =
            RuntimeWriterLease::new(&self.busy_archive).expect("competing writer lease");
        let guard = competing
            .try_acquire()
            .expect("hold competing writer lease");
        let mut busy = start_runtime(&self.busy_archive);
        assert_eq!(wait_completion(&busy).outcome(), RefreshOutcome::Busy);
        assert_eq!(
            busy.snapshot().expect("busy snapshot").refresh().failure(),
            Some(BenefitReminderFailure::Busy)
        );
        busy.shutdown().expect("busy shutdown");
        drop(guard);
    }
}

#[cfg(windows)]
fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_millis(),
    )
    .expect("wall clock")
}

#[cfg(windows)]
fn opaque_id(sequence: u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&sequence.to_be_bytes());
    bytes
}

#[cfg(windows)]
fn seed_due(path: &Path, sequence: u64) {
    let observed_at_ms = now_ms();
    let expiry_at_ms = observed_at_ms
        .checked_add(30 * 60 * 1_000)
        .and_then(|expiry| expiry.checked_add(i64::try_from(sequence).expect("sequence millis")))
        .expect("expiry");
    let scope = BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new("resource_private").expect("account"),
        None,
    );
    let lot = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([9; 32]),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: 1,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(observed_at_ms - 1),
        expiry: BenefitExpiry::exact_utc(expiry_at_ms).expect("expiry"),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot");
    let observation = BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id: BenefitObservationId::from_bytes(opaque_id(sequence)),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots: vec![lot],
    })
    .expect("observation");
    UsageStore::open(path)
        .expect("seed store")
        .apply_benefit_observation(&observation)
        .expect("seed due reminder");
}

#[cfg(windows)]
fn start_runtime(path: &Path) -> BenefitReminderRuntime {
    BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(path.to_path_buf()).expect("runtime config"),
    )
    .expect("start reminder runtime")
}

#[cfg(windows)]
fn wait_completion(runtime: &BenefitReminderRuntime) -> tokenmaster_engine::WorkerCompletion {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(completion) = runtime.try_completion().expect("runtime completion") {
            return completion;
        }
        assert!(Instant::now() < deadline, "runtime completion timeout");
        thread::yield_now();
    }
}

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
    unsafe { GetProcessHandleCount(process, &raw mut handles) }
        .expect("runtime process handle count");
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
    .expect("runtime process memory");
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
fn assert_structural_plateau(baseline: ResourceCounts, sample: ResourceCounts) {
    assert!(
        sample.handles <= baseline.handles.saturating_add(1),
        "reminder handles grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.threads <= baseline.threads,
        "reminder threads grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.user_objects <= baseline.user_objects && sample.gdi_objects <= baseline.gdi_objects,
        "reminder GUI objects grew: baseline={baseline:?}, sample={sample:?}"
    );
}

#[cfg(windows)]
fn stable_plateau(samples: &[ResourceCounts]) -> Option<(ResourceCounts, usize)> {
    let required = PLATEAU_WINDOW.checked_mul(2)?;
    if samples.len() < required {
        return None;
    }
    let candidate = &samples[samples.len() - required..];
    let topology = candidate[0];
    if candidate.iter().any(|sample| {
        sample.handles != topology.handles
            || sample.threads != topology.threads
            || sample.user_objects != topology.user_objects
            || sample.gdi_objects != topology.gdi_objects
    }) {
        return None;
    }
    let (previous, current) = candidate.split_at(PLATEAU_WINDOW);
    let previous_floor = retained_private_floor(previous)?;
    let current_floor = retained_private_floor(current)?;
    if current_floor > previous_floor.saturating_add(PRIVATE_BYTES_BUDGET) {
        return None;
    }
    let mut plateau = current[0];
    for sample in &current[1..] {
        plateau.private_bytes = plateau.private_bytes.max(sample.private_bytes);
        plateau.handles = plateau.handles.max(sample.handles);
        plateau.threads = plateau.threads.max(sample.threads);
        plateau.user_objects = plateau.user_objects.max(sample.user_objects);
        plateau.gdi_objects = plateau.gdi_objects.max(sample.gdi_objects);
    }
    Some((plateau, previous_floor.max(current_floor)))
}

#[cfg(windows)]
fn retained_private_floor(samples: &[ResourceCounts]) -> Option<usize> {
    let mut values = samples.iter().map(|sample| sample.private_bytes);
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

#[cfg(windows)]
fn run_windows_contract() {
    let mut fixtures = FixtureSet::new();
    let mut warmup = Vec::with_capacity(MAX_WARMUP_ROUNDS);
    for _ in 0..MAX_WARMUP_ROUNDS {
        fixtures.exercise_round();
        warmup.push(resource_counts());
        if stable_plateau(&warmup).is_some() {
            break;
        }
    }
    let (plateau, private_floor) = stable_plateau(&warmup)
        .unwrap_or_else(|| panic!("reminder runtime did not plateau: {warmup:?}"));

    let mut private_samples = Vec::with_capacity(MEASURED_ROUNDS);
    let mut highest = ResourceCounts::default();
    for _ in 0..MEASURED_ROUNDS {
        fixtures.exercise_round();
        let sample = resource_counts();
        assert_structural_plateau(plateau, sample);
        highest.private_bytes = highest.private_bytes.max(sample.private_bytes);
        highest.handles = highest.handles.max(sample.handles);
        highest.threads = highest.threads.max(sample.threads);
        highest.user_objects = highest.user_objects.max(sample.user_objects);
        highest.gdi_objects = highest.gdi_objects.max(sample.gdi_objects);
        private_samples.push(sample.private_bytes);
    }
    let private_limit = private_floor.saturating_add(PRIVATE_BYTES_BUDGET);
    for window in private_samples.chunks_exact(PLATEAU_WINDOW) {
        let floor = window.iter().copied().min().expect("private window");
        assert!(
            floor <= private_limit,
            "reminder retained private memory grew: plateau={plateau:?}, \
             private_floor={private_floor}, private_limit={private_limit}, \
             measured_floor={floor}, samples={private_samples:?}"
        );
    }
    println!(
        "reminder_runtime_resource_contract: pass warmup_rounds={} measured_rounds={} \
         private_floor={} private_high={} handles={} threads={} user={} gdi={}",
        warmup.len(),
        MEASURED_ROUNDS,
        private_floor,
        highest.private_bytes,
        highest.handles,
        highest.threads,
        highest.user_objects,
        highest.gdi_objects
    );
}

#[cfg(windows)]
fn main() {
    run_windows_contract();
}

#[cfg(not(windows))]
fn main() {
    println!("reminder_runtime_resource_contract: skipped (Windows-only gate)");
}
