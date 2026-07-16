#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::io::{self, BufRead, Write};
#[cfg(windows)]
use std::mem::size_of;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process;
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
use serde_json::Value;
#[cfg(windows)]
use tempfile::TempDir;
#[cfg(windows)]
use tokenmaster_codex::CodexQuotaErrorCode;
#[cfg(windows)]
use tokenmaster_engine::{RefreshOutcome, WriterLease};
#[cfg(windows)]
use tokenmaster_runtime::{
    CodexQuotaRefreshFailure, CodexQuotaRuntime, CodexQuotaRuntimeConfig, RuntimeWriterLease,
};

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
    success: PathBuf,
    rpc_error: PathBuf,
    timeout: PathBuf,
    success_archive: PathBuf,
    rpc_archive: PathBuf,
    timeout_archive: PathBuf,
    busy_archive: PathBuf,
    lifecycle_archive: PathBuf,
}

#[cfg(windows)]
impl FixtureSet {
    fn new() -> Self {
        let temp = TempDir::new().expect("runtime fixture temp");
        let success = copy_fixture(&temp, "success");
        let rpc_error = copy_fixture(&temp, "rpc_error");
        let timeout = copy_fixture(&temp, "hang");
        Self {
            success_archive: temp.path().join("success.sqlite3"),
            rpc_archive: temp.path().join("rpc.sqlite3"),
            timeout_archive: temp.path().join("timeout.sqlite3"),
            busy_archive: temp.path().join("busy.sqlite3"),
            lifecycle_archive: temp.path().join("lifecycle.sqlite3"),
            _temp: temp,
            success,
            rpc_error,
            timeout,
        }
    }

    fn exercise_round(&self) {
        let mut success =
            start_runtime(&self.success_archive, &self.success, Duration::from_secs(2));
        assert_eq!(
            wait_completion(&success).outcome(),
            RefreshOutcome::Completed
        );
        let success_snapshot = success.snapshot().expect("success snapshot").refresh();
        assert_eq!(success_snapshot.outcome(), Some(RefreshOutcome::Completed));
        assert_eq!(success_snapshot.observation_count(), 1);
        assert_eq!(success_snapshot.processed_count(), 1);
        success.shutdown().expect("success shutdown");

        let mut rpc = start_runtime(&self.rpc_archive, &self.rpc_error, Duration::from_secs(2));
        assert_eq!(wait_completion(&rpc).outcome(), RefreshOutcome::Failed);
        assert_eq!(
            rpc.snapshot().expect("RPC snapshot").refresh().failure(),
            Some(CodexQuotaRefreshFailure::Transport(
                CodexQuotaErrorCode::RpcError
            ))
        );
        rpc.shutdown().expect("RPC shutdown");

        let mut timeout = start_runtime(
            &self.timeout_archive,
            &self.timeout,
            Duration::from_millis(25),
        );
        assert_eq!(
            wait_completion(&timeout).outcome(),
            RefreshOutcome::DeadlineExceeded
        );
        assert_eq!(
            timeout
                .snapshot()
                .expect("timeout snapshot")
                .refresh()
                .failure(),
            Some(CodexQuotaRefreshFailure::Transport(
                CodexQuotaErrorCode::DeadlineExceeded
            ))
        );
        timeout.shutdown().expect("timeout shutdown");

        let mut competing =
            RuntimeWriterLease::new(&self.busy_archive).expect("competing writer lease");
        let guard = competing
            .try_acquire()
            .expect("hold competing writer lease");
        let mut busy = start_runtime(&self.busy_archive, &self.success, Duration::from_secs(2));
        assert_eq!(wait_completion(&busy).outcome(), RefreshOutcome::Busy);
        busy.shutdown().expect("busy shutdown");
        drop(guard);

        let mut lifecycle = start_runtime(
            &self.lifecycle_archive,
            &self.success,
            Duration::from_secs(2),
        );
        assert_eq!(
            wait_completion(&lifecycle).outcome(),
            RefreshOutcome::Completed
        );
        lifecycle.pause().expect("lifecycle pause");
        lifecycle.resume().expect("lifecycle resume");
        assert_eq!(
            wait_completion(&lifecycle).outcome(),
            RefreshOutcome::Completed
        );
        lifecycle.shutdown().expect("lifecycle shutdown");
    }
}

#[cfg(windows)]
fn copy_fixture(temp: &TempDir, mode: &str) -> PathBuf {
    let executable = temp
        .path()
        .join(format!("codex_quota_runtime_fixture__{mode}.exe"));
    fs::copy(
        std::env::current_exe().expect("resource harness"),
        &executable,
    )
    .expect("copy resource fixture");
    executable
}

#[cfg(windows)]
fn start_runtime(archive: &Path, executable: &Path, timeout: Duration) -> CodexQuotaRuntime {
    let config = CodexQuotaRuntimeConfig::new(archive.to_path_buf())
        .expect("runtime config")
        .with_executable(executable.to_path_buf())
        .expect("runtime fixture executable")
        .with_transport_timeout(timeout)
        .expect("runtime fixture timeout");
    CodexQuotaRuntime::start(config).expect("start quota runtime")
}

#[cfg(windows)]
fn wait_completion(runtime: &CodexQuotaRuntime) -> tokenmaster_engine::WorkerCompletion {
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
fn run_fixture() -> Result<(), ()> {
    let executable = std::env::current_exe().map_err(|_| ())?;
    let file_stem = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or(())?;
    let mode = file_stem
        .rsplit_once("__")
        .map(|(_, mode)| mode)
        .ok_or(())?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args != ["app-server", "--stdio"] {
        return Err(());
    }

    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let initialize = read_message(&mut input)?;
    if initialize.get("method").and_then(Value::as_str) != Some("initialize") {
        return Err(());
    }
    if mode == "hang" {
        thread::sleep(Duration::from_secs(30));
        return Ok(());
    }
    writeln!(
        output,
        "{{\"id\":0,\"result\":{{\"codexHome\":\"C:\\\\private\\\\runtime-fixture\",\
         \"platformFamily\":\"windows\",\"platformOs\":\"windows\",\
         \"userAgent\":\"Codex Runtime Fixture/0.144.1 (windows)\"}}}}"
    )
    .map_err(|_| ())?;
    output.flush().map_err(|_| ())?;

    if read_message(&mut input)?
        .get("method")
        .and_then(Value::as_str)
        != Some("initialized")
    {
        return Err(());
    }
    if read_message(&mut input)?
        .get("method")
        .and_then(Value::as_str)
        != Some("account/read")
    {
        return Err(());
    }
    if mode == "rpc_error" {
        writeln!(
            output,
            "{{\"id\":1,\"error\":{{\"code\":-32000,\
             \"message\":\"private runtime fixture failure\"}}}}"
        )
        .map_err(|_| ())?;
        output.flush().map_err(|_| ())?;
        return Ok(());
    }
    if mode != "success" {
        return Err(());
    }
    writeln!(
        output,
        "{{\"id\":1,\"result\":{{\"requiresOpenaiAuth\":true,\
         \"account\":{{\"type\":\"chatgpt\",\"email\":\"private-runtime@example.com\",\
         \"planType\":\"pro\"}}}}}}"
    )
    .map_err(|_| ())?;
    output.flush().map_err(|_| ())?;

    if read_message(&mut input)?
        .get("method")
        .and_then(Value::as_str)
        != Some("account/rateLimits/read")
    {
        return Err(());
    }
    writeln!(
        output,
        "{{\"id\":2,\"result\":{{\"rateLimitResetCredits\":null,\
         \"rateLimits\":{{\"limitId\":\"codex\",\"limitName\":null,\
         \"planType\":\"pro\",\"primary\":{{\"usedPercent\":42,\
         \"resetsAt\":1700100000,\"windowDurationMins\":10080}},\
         \"secondary\":null}},\"rateLimitsByLimitId\":null}}}}"
    )
    .map_err(|_| ())?;
    output.flush().map_err(|_| ())
}

#[cfg(windows)]
fn read_message(input: &mut impl BufRead) -> Result<Value, ()> {
    let mut line = String::new();
    if input.read_line(&mut line).map_err(|_| ())? == 0 {
        return Err(());
    }
    serde_json::from_str(&line).map_err(|_| ())
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
        "runtime handles grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.threads <= baseline.threads,
        "runtime threads grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.user_objects <= baseline.user_objects && sample.gdi_objects <= baseline.gdi_objects,
        "runtime GUI objects grew: baseline={baseline:?}, sample={sample:?}"
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
            if executable.starts_with("codex_quota_runtime_fixture__") {
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
        .unwrap_or_else(|| panic!("runtime did not establish a stable plateau: {warmup:?}"));

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
            "runtime private memory retained growth: plateau={plateau:?}, \
             private_floor={private_floor}, private_limit={private_limit}, \
             measured_floor={floor}, samples={private_samples:?}"
        );
    }
    let remaining = fixture_processes();
    assert!(
        remaining.is_empty(),
        "task-owned runtime fixture processes remain: {remaining:?}"
    );
    println!(
        "quota_runtime_resource_contract: pass warmup_rounds={} measured_rounds={} \
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
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["app-server", "--stdio"] {
        if run_fixture().is_err() {
            process::exit(91);
        }
    } else {
        run_windows_contract();
    }
}

#[cfg(not(windows))]
fn main() {
    println!("quota_runtime_resource_contract: skipped (Windows-only gate)");
}
