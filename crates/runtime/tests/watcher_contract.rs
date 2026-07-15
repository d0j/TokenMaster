use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, mpsc::channel};
use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_engine::RefreshUrgency;
use tokenmaster_runtime::{
    BoundedFilesystemWatcher, MAX_WATCH_ROOTS, RefreshScheduler, SystemClock, WatcherErrorCode,
};

static WATCHER_TEST_LOCK: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    WATCHER_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn real_watcher_reduces_create_append_and_rename_to_one_pathless_hint() {
    let _serial = serial();
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(SystemClock::shared(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("startup"),
        RefreshUrgency::Recovery
    );

    let root = TempDir::new().expect("watch root");
    let mut watcher = BoundedFilesystemWatcher::new(scheduler.hints()).expect("watcher");
    let snapshot = watcher
        .replace_roots(&[root.path().to_path_buf()])
        .expect("watch root generation");
    assert_eq!(snapshot.generation(), 1);
    assert_eq!(snapshot.root_count(), 1);
    assert_eq!(
        format!("{snapshot:?}"),
        "WatcherSnapshot { generation: 1, root_count: 1 }"
    );
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("root-change reconciliation"),
        RefreshUrgency::Recovery
    );

    let first = root.path().join("first.jsonl");
    let second = root.path().join("second.jsonl");
    std::fs::write(&first, b"one\n").expect("create watched file");
    OpenOptions::new()
        .append(true)
        .open(&first)
        .expect("open watched file")
        .write_all(b"two\n")
        .expect("append watched file");
    std::fs::rename(&first, &second).expect("rename watched file");

    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("watch hint"),
        RefreshUrgency::Hint
    );

    watcher.shutdown();
    assert_eq!(watcher.snapshot().root_count(), 0);
    scheduler.shutdown().expect("scheduler shutdown");
}

#[test]
fn root_capacity_and_invalid_roots_fail_before_backend_state_changes() {
    let _serial = serial();
    let (sender, _receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(SystemClock::shared(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    let mut watcher = BoundedFilesystemWatcher::new(scheduler.hints()).expect("watcher");

    let roots = vec![PathBuf::from(r"C:\bounded"); MAX_WATCH_ROOTS + 1];
    let capacity = watcher
        .replace_roots(&roots)
        .expect_err("root capacity must fail closed");
    assert_eq!(capacity.code(), WatcherErrorCode::CapacityExceeded);
    assert_eq!(watcher.snapshot().generation(), 0);

    let invalid = watcher
        .replace_roots(&[PathBuf::from("relative")])
        .expect_err("relative root must fail closed");
    assert_eq!(invalid.code(), WatcherErrorCode::InvalidRoot);
    assert!(!format!("{invalid:?}").contains("relative"));
    assert_eq!(watcher.snapshot().generation(), 0);

    let duplicate_root = TempDir::new().expect("duplicate root");
    let duplicate = duplicate_root.path().to_path_buf();
    let duplicate_error = watcher
        .replace_roots(&[duplicate.clone(), duplicate])
        .expect_err("duplicate roots must fail closed");
    assert_eq!(duplicate_error.code(), WatcherErrorCode::InvalidRoot);
    assert_eq!(watcher.snapshot().generation(), 0);

    watcher.shutdown();
    scheduler.shutdown().expect("scheduler shutdown");
}

#[test]
fn replacing_roots_publishes_only_the_latest_bounded_generation() {
    let _serial = serial();
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(SystemClock::shared(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("startup"),
        RefreshUrgency::Recovery
    );
    let first = TempDir::new().expect("first root");
    let second = TempDir::new().expect("second root");
    let mut watcher = BoundedFilesystemWatcher::new(scheduler.hints()).expect("watcher");

    watcher
        .replace_roots(&[first.path().to_path_buf()])
        .expect("first generation");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("first generation recovery"),
        RefreshUrgency::Recovery
    );
    let latest = watcher
        .replace_roots(&[second.path().to_path_buf()])
        .expect("second generation");
    assert_eq!(latest.generation(), 2);
    assert_eq!(latest.root_count(), 1);
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("second generation recovery"),
        RefreshUrgency::Recovery
    );

    std::fs::write(first.path().join("stale.jsonl"), b"stale\n").expect("stale event");
    assert_eq!(
        receiver.recv_timeout(Duration::from_millis(400)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    );
    std::fs::write(second.path().join("current.jsonl"), b"current\n").expect("current event");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("current generation hint"),
        RefreshUrgency::Hint
    );

    watcher.shutdown();
    scheduler.shutdown().expect("scheduler shutdown");
}

#[test]
fn missing_roots_use_no_backend_watch_and_force_degraded_reconciliation() {
    let _serial = serial();
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(SystemClock::shared(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("startup"),
        RefreshUrgency::Recovery
    );
    let root = TempDir::new().expect("root parent");
    let missing = root.path().join("not-created");
    let mut watcher = BoundedFilesystemWatcher::new(scheduler.hints()).expect("watcher");

    let snapshot = watcher
        .replace_roots(&[missing])
        .expect("missing root generation");
    assert_eq!(snapshot.generation(), 1);
    assert_eq!(snapshot.root_count(), 0);
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("missing root recovery"),
        RefreshUrgency::Recovery
    );

    watcher.shutdown();
    scheduler.shutdown().expect("scheduler shutdown");
}

#[cfg(windows)]
#[test]
fn repeated_generations_return_process_threads_and_handles_to_baseline() {
    let _serial = serial();
    let baseline = current_resource_counts();
    let (sender, receiver) = channel();
    let mut scheduler = RefreshScheduler::spawn(SystemClock::shared(), move |urgency| {
        sender.send(urgency).map_err(|_| ())
    })
    .expect("scheduler");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("startup"),
        RefreshUrgency::Recovery
    );
    let first = TempDir::new().expect("first root");
    let second = TempDir::new().expect("second root");
    let mut watcher = BoundedFilesystemWatcher::new(scheduler.hints()).expect("watcher");

    for generation in 0..32 {
        let root = if generation % 2 == 0 {
            first.path()
        } else {
            second.path()
        };
        watcher
            .replace_roots(&[root.to_path_buf()])
            .expect("bounded generation");
    }
    watcher.shutdown();
    scheduler.shutdown().expect("scheduler shutdown");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let current = current_resource_counts();
        if current.0 <= baseline.0 && current.1 <= baseline.1 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "watcher resources did not return to baseline: handles {}->{}, threads {}->{}",
            baseline.0,
            current.0,
            baseline.1,
            current.1
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn current_resource_counts() -> (u32, u32) {
    use std::mem::size_of;

    use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetProcessHandleCount,
    };
    use windows::core::HRESULT;

    let mut handles = 0_u32;
    // SAFETY: the count points to writable storage and the process pseudo-handle is valid.
    unsafe { GetProcessHandleCount(GetCurrentProcess(), &raw mut handles) }
        .expect("process handles");
    // SAFETY: fixed flags and no borrowed pointers; the returned owned handle is closed below.
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.expect("thread snapshot");
    // SAFETY: this call takes no pointers and returns this process's numeric identifier.
    let process_id = unsafe { GetCurrentProcessId() };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).expect("thread entry size"),
        ..Default::default()
    };
    let mut threads = 0_u32;
    // SAFETY: the entry is correctly sized and writable; the snapshot remains open.
    let first = unsafe { Thread32First(snapshot, &raw mut entry) };
    if first.is_ok() {
        loop {
            if entry.th32OwnerProcessID == process_id {
                threads = threads.checked_add(1).expect("thread count");
            }
            // SAFETY: same live snapshot and writable entry as the first call.
            match unsafe { Thread32Next(snapshot, &raw mut entry) } {
                Ok(()) => {}
                Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) => break,
                Err(error) => panic!("thread enumeration failed: {error}"),
            }
        }
    } else if first
        .as_ref()
        .is_err_and(|error| error.code() != HRESULT::from_win32(ERROR_NO_MORE_FILES.0))
    {
        panic!("thread enumeration failed");
    }
    // SAFETY: the snapshot is the owned live handle and is closed exactly once.
    unsafe { CloseHandle(snapshot) }.expect("close thread snapshot");
    (handles, threads)
}
