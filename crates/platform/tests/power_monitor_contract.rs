#![cfg(windows)]

use tokenmaster_platform::{PowerMonitorError, PowerMonitorSnapshot, SuspendResumeMonitor};

#[derive(Clone, Copy, Debug)]
struct ResourceCounts {
    private_bytes: usize,
    handles: u32,
    threads: u32,
    user_objects: u32,
    gdi_objects: u32,
}

fn resource_counts() -> ResourceCounts {
    use std::mem::size_of;

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
    let mut count = 0_u32;
    // SAFETY: `count` is writable and `process` is the valid process pseudo-handle.
    unsafe { GetProcessHandleCount(process, &raw mut count) }.expect("query process handle count");
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
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.expect("create thread snapshot");
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
        handles: count,
        threads,
        user_objects,
        gdi_objects,
    }
}

#[test]
fn suspend_resume_registration_is_singleton_reusable_and_private() {
    assert!(std::mem::size_of::<SuspendResumeMonitor>() <= 32);
    assert!(std::mem::size_of::<PowerMonitorSnapshot>() <= 64);
    let mut monitor = SuspendResumeMonitor::subscribe().expect("first registration");
    let snapshot = monitor.snapshot();
    assert_eq!(snapshot.pending(), None);
    assert_eq!(snapshot.accepted_count(), 0);
    assert_eq!(snapshot.coalesced_count(), 0);
    assert_eq!(snapshot.unknown_count(), 0);
    assert!(!snapshot.overflowed());

    let second = SuspendResumeMonitor::subscribe().expect_err("singleton registration");
    assert_eq!(second, PowerMonitorError::AlreadyRegistered);

    let private_debug = format!("{monitor:?}");
    assert!(!private_debug.contains("HPOWERNOTIFY"));
    assert!(!private_debug.contains("HANDLE"));
    assert!(!private_debug.contains("0x"));

    monitor.shutdown().expect("first shutdown");
    assert_eq!(monitor.take_pending(), None);
    assert!(format!("{monitor:?}").contains("active: false"));
    let replacement = SuspendResumeMonitor::subscribe().expect("replacement registration");
    drop(replacement);
    let mut final_monitor = SuspendResumeMonitor::subscribe().expect("registration after drop");
    final_monitor.shutdown().expect("final shutdown");

    let _measurement_warmup = resource_counts();
    let before = resource_counts();
    for _ in 0..4_096 {
        let mut monitor = SuspendResumeMonitor::subscribe().expect("bounded registration");
        monitor.shutdown().expect("bounded unregistration");
    }
    let after = resource_counts();
    assert!(
        after.handles <= before.handles.saturating_add(1),
        "power monitor handles grew: before={before:?}, after={after:?}"
    );
    assert!(
        after.threads <= before.threads,
        "power monitor threads grew: before={before:?}, after={after:?}"
    );
    assert!(
        after.user_objects <= before.user_objects && after.gdi_objects <= before.gdi_objects,
        "power monitor GUI objects grew: before={before:?}, after={after:?}"
    );
    assert!(
        after.private_bytes <= before.private_bytes.saturating_add(1_048_576),
        "power monitor private bytes grew over 1 MiB: before={before:?}, after={after:?}"
    );
}
