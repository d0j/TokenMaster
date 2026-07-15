#![cfg(windows)]

use std::{mem::size_of, path::Path};

use tempfile::TempDir;
use tokenmaster_query::{
    LatestActivityRequest, PageSize, QueryClock, QueryError, QueryService, QueryTimeSample,
};
use tokenmaster_store::UsageStore;

#[derive(Clone, Copy, Debug)]
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

fn open_query_drop(path: &Path) {
    let mut service = QueryService::open(path, FixedClock).expect("open query service");
    let snapshot = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(16).expect("page size"),
        ))
        .expect("query empty archive");
    assert!(snapshot.payload().items().is_empty());
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

#[test]
fn repeated_open_query_drop_returns_resources_to_a_stable_plateau() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("resource.sqlite3");
    seed_empty_archive(&path);

    for _ in 0..32 {
        open_query_drop(&path);
    }
    let _measurement_warmup = resource_counts();
    let before = resource_counts();
    for _ in 0..256 {
        open_query_drop(&path);
    }
    let after = resource_counts();

    assert!(
        after.handles <= before.handles.saturating_add(1),
        "query handles grew: before={before:?}, after={after:?}"
    );
    assert!(
        after.threads <= before.threads,
        "query threads grew: before={before:?}, after={after:?}"
    );
    assert!(
        after.user_objects <= before.user_objects && after.gdi_objects <= before.gdi_objects,
        "query GUI objects grew: before={before:?}, after={after:?}"
    );
    assert!(
        after.private_bytes <= before.private_bytes.saturating_add(1_048_576),
        "query private bytes grew over 1 MiB: before={before:?}, after={after:?}"
    );
}
