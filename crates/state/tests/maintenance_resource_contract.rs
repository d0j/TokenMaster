#[cfg(windows)]
use std::mem::size_of;
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
use tokenmaster_state::{
    BackupMaintenanceRuntime, MaintenanceAdmission, MaintenanceExecution, MaintenanceOutcome,
    MaintenancePurpose, MaintenanceSourceState, SettingsValue, StateErrorCode,
    SystemMaintenanceClock,
};

#[cfg(windows)]
const WARMUP_ROUNDS: usize = 12;
#[cfg(windows)]
const MEASURED_ROUNDS: usize = 24;
#[cfg(windows)]
const PRIVATE_BYTES_BUDGET: usize = 8 * 1024 * 1024;

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
fn exercise_round(round: usize) {
    let clock = Arc::new(SystemMaintenanceClock::new());
    let policy = SettingsValue::safe_defaults().portable().backup().clone();
    let mode = round % 3;
    let mut runtime = BackupMaintenanceRuntime::spawn(
        clock,
        policy,
        MaintenanceSourceState::Healthy,
        move |permit| match mode {
            0 => {
                permit.begin_publication().expect("publication boundary");
                MaintenanceExecution::Published { bytes: 4096 }
            }
            1 => MaintenanceExecution::Failed(StateErrorCode::Unavailable),
            _ => MaintenanceExecution::Cancelled,
        },
    )
    .expect("spawn maintenance runtime");
    let admission = runtime.submit(MaintenancePurpose::Manual);
    assert!(matches!(admission, MaintenanceAdmission::Started(_)));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(completion) = runtime.snapshot().worker().latest_completion() {
            assert_eq!(
                completion.outcome(),
                match mode {
                    0 => MaintenanceOutcome::Published,
                    1 => MaintenanceOutcome::Failed,
                    _ => MaintenanceOutcome::Cancelled,
                }
            );
            break;
        }
        assert!(Instant::now() < deadline, "maintenance completion timeout");
        std::thread::yield_now();
    }
    runtime.shutdown().expect("shutdown maintenance runtime");
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

    // SAFETY: the current-process pseudo handle requires no ownership transfer.
    let process = unsafe { GetCurrentProcess() };
    let mut handles = 0_u32;
    // SAFETY: `handles` is writable and `process` is the current-process pseudo handle.
    unsafe { GetProcessHandleCount(process, &raw mut handles) }
        .expect("maintenance process handle count");
    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).expect("counter size"),
        ..Default::default()
    };
    // SAFETY: the counters buffer is correctly sized and writable for the live process.
    unsafe {
        K32GetProcessMemoryInfo(
            process,
            (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
            memory.cb,
        )
    }
    .expect("maintenance process memory");
    // SAFETY: GUI resource queries only read counters from the live process handle.
    let user_objects = unsafe { GetGuiResources(process, GR_USEROBJECTS) };
    // SAFETY: same valid process handle and a fixed GDI counter selector.
    let gdi_objects = unsafe { GetGuiResources(process, GR_GDIOBJECTS) };

    // SAFETY: fixed snapshot flags and no borrowed pointers; the handle is closed below.
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.expect("thread snapshot");
    // SAFETY: no pointer arguments; returns the current process identifier.
    let process_id = unsafe { GetCurrentProcessId() };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).expect("thread entry size"),
        ..Default::default()
    };
    let mut threads = 0_u32;
    // SAFETY: the entry is correctly sized and writable while the snapshot remains open.
    if unsafe { Thread32First(snapshot, &raw mut entry) }.is_ok() {
        loop {
            if entry.th32OwnerProcessID == process_id {
                threads = threads.checked_add(1).expect("thread count");
            }
            // SAFETY: same live snapshot and writable entry as the first call.
            match unsafe { Thread32Next(snapshot, &raw mut entry) } {
                Ok(()) => {}
                Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(error) => panic!("enumerate maintenance threads: {error}"),
            }
        }
    }
    // SAFETY: `snapshot` is the owned handle returned above and is closed exactly once.
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
#[test]
fn repeated_success_failure_and_cancel_cycles_return_process_resources() {
    let mut baseline = ResourceCounts::default();
    for round in 0..WARMUP_ROUNDS {
        exercise_round(round);
        let sample = resource_counts();
        baseline.private_bytes = baseline.private_bytes.max(sample.private_bytes);
        baseline.handles = baseline.handles.max(sample.handles);
        baseline.threads = baseline.threads.max(sample.threads);
        baseline.user_objects = baseline.user_objects.max(sample.user_objects);
        baseline.gdi_objects = baseline.gdi_objects.max(sample.gdi_objects);
    }
    let private_ceiling = baseline.private_bytes.saturating_add(PRIVATE_BYTES_BUDGET);
    let mut final_private_bytes = baseline.private_bytes;
    for round in 0..MEASURED_ROUNDS {
        exercise_round(round);
        let deadline = Instant::now() + Duration::from_secs(5);
        let sample = loop {
            let sample = resource_counts();
            if sample.handles <= baseline.handles.saturating_add(1)
                && sample.threads <= baseline.threads
                && sample.user_objects <= baseline.user_objects
                && sample.gdi_objects <= baseline.gdi_objects
            {
                break sample;
            }
            assert!(
                Instant::now() < deadline,
                "maintenance resources did not settle: baseline={baseline:?}, sample={sample:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        final_private_bytes = sample.private_bytes;
        assert!(
            sample.private_bytes <= private_ceiling,
            "maintenance private memory left the post-warm-up envelope: baseline={baseline:?}, sample={sample:?}"
        );
        assert!(
            sample.handles <= baseline.handles.saturating_add(1),
            "maintenance handles grew: baseline={baseline:?}, sample={sample:?}"
        );
        assert!(
            sample.threads <= baseline.threads,
            "maintenance threads grew: baseline={baseline:?}, sample={sample:?}"
        );
        assert!(
            sample.user_objects <= baseline.user_objects
                && sample.gdi_objects <= baseline.gdi_objects,
            "maintenance GUI objects grew: baseline={baseline:?}, sample={sample:?}"
        );
    }
    assert!(
        final_private_bytes <= private_ceiling,
        "maintenance final private memory did not return: baseline={baseline:?}, final={final_private_bytes}"
    );
}

#[cfg(not(windows))]
#[test]
fn maintenance_resource_contract_is_windows_only() {
    println!("maintenance_resource_contract: skipped (Windows-only gate)");
}
