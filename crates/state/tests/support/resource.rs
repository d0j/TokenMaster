#![cfg(windows)]

use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessResources {
    pub private_bytes: usize,
    pub handles: u32,
    pub threads: u32,
    pub user_objects: u32,
    pub gdi_objects: u32,
    pub child_processes: u32,
}

impl ProcessResources {
    fn include(&mut self, sample: Self) {
        self.private_bytes = self.private_bytes.max(sample.private_bytes);
        self.handles = self.handles.max(sample.handles);
        self.threads = self.threads.max(sample.threads);
        self.user_objects = self.user_objects.max(sample.user_objects);
        self.gdi_objects = self.gdi_objects.max(sample.gdi_objects);
        self.child_processes = self.child_processes.max(sample.child_processes);
    }
}

pub struct ResourceMonitor {
    baseline: ProcessResources,
    stop: Arc<AtomicBool>,
    sampler: Option<JoinHandle<ProcessResources>>,
}

impl ResourceMonitor {
    pub fn start() -> Self {
        let baseline = sample();
        let stop = Arc::new(AtomicBool::new(false));
        let sampler_stop = Arc::clone(&stop);
        let sampler = thread::Builder::new()
            .name("p3d0-resource-sampler".to_owned())
            .spawn(move || {
                let mut peak = ProcessResources::default();
                while !sampler_stop.load(Ordering::Acquire) {
                    peak.include(sample());
                    thread::sleep(Duration::from_millis(1));
                }
                peak.include(sample());
                peak
            })
            .expect("spawn P3-D.0 resource sampler");
        Self {
            baseline,
            stop,
            sampler: Some(sampler),
        }
    }

    pub fn finish(mut self) -> ResourceWindow {
        self.stop.store(true, Ordering::Release);
        let peak = self
            .sampler
            .take()
            .expect("resource sampler handle")
            .join()
            .expect("resource sampler joins");
        ResourceWindow {
            baseline: self.baseline,
            peak,
        }
    }
}

impl Drop for ResourceMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(sampler) = self.sampler.take() {
            let _ = sampler.join();
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResourceWindow {
    pub baseline: ProcessResources,
    pub peak: ProcessResources,
}

impl ResourceWindow {
    #[allow(
        dead_code,
        reason = "used by the performance target, not every including target"
    )]
    pub fn private_growth(self) -> usize {
        self.peak
            .private_bytes
            .saturating_sub(self.baseline.private_bytes)
    }
}

pub fn settle_to(
    baseline: ProcessResources,
    private_tolerance: usize,
    handle_tolerance: u32,
    timeout: Duration,
) -> ProcessResources {
    let deadline = Instant::now() + timeout;
    loop {
        let current = sample();
        if current.private_bytes <= baseline.private_bytes.saturating_add(private_tolerance)
            && current.handles <= baseline.handles.saturating_add(handle_tolerance)
            && current.threads <= baseline.threads
            && current.user_objects <= baseline.user_objects
            && current.gdi_objects <= baseline.gdi_objects
            && current.child_processes == 0
        {
            return current;
        }
        assert!(
            Instant::now() < deadline,
            "process resources did not return: baseline={baseline:?}, current={current:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

pub fn sample() -> ProcessResources {
    use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
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
    // SAFETY: no pointer arguments; returns the current process identifier.
    let process_id = unsafe { GetCurrentProcessId() };
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
    .expect("P3-D.0 process memory");
    // SAFETY: GUI resource queries only read counters from the live process handle.
    let user_objects = unsafe { GetGuiResources(process, GR_USEROBJECTS) };
    // SAFETY: same valid process handle and a fixed GDI counter selector.
    let gdi_objects = unsafe { GetGuiResources(process, GR_GDIOBJECTS) };

    // SAFETY: fixed snapshot flags and no borrowed pointers; the handle is closed below.
    let thread_snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.expect("P3-D.0 thread snapshot");
    let mut thread_entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).expect("thread entry size"),
        ..Default::default()
    };
    let mut threads = 0_u32;
    // SAFETY: the entry is correctly sized and writable while the snapshot remains open.
    if unsafe { Thread32First(thread_snapshot, &raw mut thread_entry) }.is_ok() {
        loop {
            if thread_entry.th32OwnerProcessID == process_id {
                threads = threads.checked_add(1).expect("thread count");
            }
            // SAFETY: same live snapshot and writable entry as the first call.
            match unsafe { Thread32Next(thread_snapshot, &raw mut thread_entry) } {
                Ok(()) => {}
                Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(error) => panic!("enumerate P3-D.0 threads: {error}"),
            }
        }
    }
    // SAFETY: `thread_snapshot` is owned and closed exactly once.
    unsafe { CloseHandle(thread_snapshot) }.expect("close P3-D.0 thread snapshot");

    // SAFETY: fixed snapshot flags and no borrowed pointers; the handle is closed below.
    let process_snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .expect("P3-D.0 process snapshot");
    let mut process_entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).expect("process entry size"),
        ..Default::default()
    };
    let mut child_processes = 0_u32;
    // SAFETY: the entry is correctly sized and writable while the snapshot remains open.
    if unsafe { Process32FirstW(process_snapshot, &raw mut process_entry) }.is_ok() {
        loop {
            if process_entry.th32ParentProcessID == process_id {
                child_processes = child_processes.checked_add(1).expect("child process count");
            }
            // SAFETY: same live snapshot and writable entry as the first call.
            match unsafe { Process32NextW(process_snapshot, &raw mut process_entry) } {
                Ok(()) => {}
                Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(error) => panic!("enumerate P3-D.0 child processes: {error}"),
            }
        }
    }
    // SAFETY: `process_snapshot` is owned and closed exactly once.
    unsafe { CloseHandle(process_snapshot) }.expect("close P3-D.0 process snapshot");

    let mut handles = 0_u32;
    // Count after ToolHelp initialization and after both owned snapshots close, so the
    // returned baseline cannot omit one-time process-level ToolHelp state.
    // SAFETY: `handles` is writable and `process` is the current-process pseudo handle.
    unsafe { GetProcessHandleCount(process, &raw mut handles) }
        .expect("P3-D.0 process handle count");

    ProcessResources {
        private_bytes: memory.PrivateUsage,
        handles,
        threads,
        user_objects,
        gdi_objects,
        child_processes,
    }
}
