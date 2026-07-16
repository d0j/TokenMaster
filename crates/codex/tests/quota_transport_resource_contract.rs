#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::mem::size_of;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use tempfile::TempDir;
#[cfg(windows)]
use tokenmaster_codex::{CodexAppServerCommand, CodexQuotaErrorCode, CodexQuotaTransport};

#[cfg(windows)]
const OBSERVED_AT_MS: i64 = 1_700_000_000_000;
#[cfg(windows)]
const PLATEAU_WINDOW: usize = 8;
#[cfg(windows)]
const MAX_WARMUP_ROUNDS: usize = 48;
#[cfg(windows)]
const MEASURED_ROUNDS: usize = 64;
#[cfg(windows)]
const PRIVATE_BYTES_BUDGET: usize = 2 * 1024 * 1024;

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
    success: CodexQuotaTransport,
    rpc_error: CodexQuotaTransport,
    timeout: CodexQuotaTransport,
}

#[cfg(windows)]
impl FixtureSet {
    fn new() -> Self {
        let temp = TempDir::new().expect("fixture temp");
        let success = copy_transport(&temp, "success", Duration::from_secs(5));
        let rpc_error = copy_transport(&temp, "rpc_error", Duration::from_secs(5));
        let timeout = copy_transport(&temp, "hang", Duration::from_millis(25));
        Self {
            _temp: temp,
            success,
            rpc_error,
            timeout,
        }
    }

    fn exercise_round(&self) {
        let snapshot = self.success.poll(OBSERVED_AT_MS).expect("success fixture");
        assert_eq!(snapshot.observations().len(), 1);

        let rpc_error = self
            .rpc_error
            .poll(OBSERVED_AT_MS)
            .expect_err("RPC error fixture");
        assert_eq!(rpc_error.code(), CodexQuotaErrorCode::RpcError);

        let timeout = self
            .timeout
            .poll(OBSERVED_AT_MS)
            .expect_err("timeout fixture");
        assert_eq!(timeout.code(), CodexQuotaErrorCode::DeadlineExceeded);
    }
}

#[cfg(windows)]
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codex_app_server_fixture"))
}

#[cfg(windows)]
fn copy_transport(temp: &TempDir, mode: &str, timeout: Duration) -> CodexQuotaTransport {
    let executable = temp
        .path()
        .join(format!("codex_app_server_fixture__{mode}.exe"));
    fs::copy(fixture_path(), &executable).expect("copy fixture executable");
    let command = CodexAppServerCommand::new(executable).expect("fixture command");
    CodexQuotaTransport::new(command, timeout).expect("fixture transport")
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
    // SAFETY: `handles` is writable and `process` is the valid process pseudo-handle.
    unsafe { GetProcessHandleCount(process, &raw mut handles) }
        .expect("transport process handle count");
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
    .expect("transport process memory");
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
        "transport handles grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.threads <= baseline.threads,
        "transport threads grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.user_objects <= baseline.user_objects && sample.gdi_objects <= baseline.gdi_objects,
        "transport GUI objects grew: baseline={baseline:?}, sample={sample:?}"
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
fn fixture_processes() -> Vec<(u32, String)> {
    use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32, Process32First, Process32Next, TH32CS_SNAPPROCESS,
    };

    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.expect("process snapshot");
    let mut entry = PROCESSENTRY32 {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32>()).expect("process entry size"),
        ..Default::default()
    };
    let mut matches = Vec::new();
    if unsafe { Process32First(snapshot, &raw mut entry) }.is_ok() {
        loop {
            let length = entry
                .szExeFile
                .iter()
                .position(|character| *character == 0)
                .unwrap_or(entry.szExeFile.len());
            let executable_bytes = entry.szExeFile[..length]
                .iter()
                .map(|character| *character as u8)
                .collect::<Vec<_>>();
            let executable = String::from_utf8_lossy(&executable_bytes);
            if executable.starts_with("codex_app_server_fixture__") {
                matches.push((entry.th32ProcessID, executable.into_owned()));
            }
            match unsafe { Process32Next(snapshot, &raw mut entry) } {
                Ok(()) => {}
                Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(error) => panic!("enumerate processes: {error}"),
            }
        }
    }
    unsafe { CloseHandle(snapshot) }.expect("close process snapshot");
    matches
}

#[cfg(windows)]
fn run_windows_contract() {
    let fixtures = FixtureSet::new();
    let mut warmup = Vec::with_capacity(MAX_WARMUP_ROUNDS);
    for _ in 0..MAX_WARMUP_ROUNDS {
        fixtures.exercise_round();
        warmup.push(resource_counts());
        if stable_plateau(&warmup).is_some() {
            break;
        }
    }
    let (plateau, private_floor) = stable_plateau(&warmup)
        .unwrap_or_else(|| panic!("transport did not establish a stable plateau: {warmup:?}"));

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
            "transport private memory retained growth: plateau={plateau:?}, \
             private_floor={private_floor}, private_limit={private_limit}, \
             measured_floor={floor}, samples={private_samples:?}"
        );
    }
    let remaining = fixture_processes();
    assert!(
        remaining.is_empty(),
        "task-owned fixture processes remain: {remaining:?}"
    );
    println!(
        "quota_transport_resource_contract: pass warmup_rounds={} measured_rounds={} \
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
    println!("quota_transport_resource_contract: skipped (Windows-only gate)");
}
