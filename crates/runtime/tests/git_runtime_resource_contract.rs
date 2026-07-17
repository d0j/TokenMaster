#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::mem::size_of;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use tempfile::TempDir;
#[cfg(windows)]
use tokenmaster_domain::{
    ProjectAlias, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};
#[cfg(windows)]
use tokenmaster_provider::{
    RepositoryActivityHint, RepositoryActivityHintParts, RepositoryCandidatePath,
};
#[cfg(windows)]
use tokenmaster_runtime::{GitRuntime, GitRuntimeConfig};

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
    repository: PathBuf,
    archive: PathBuf,
    executable: PathBuf,
    sequence: u64,
}

#[cfg(windows)]
impl FixtureSet {
    fn new() -> Self {
        let temp = TempDir::new().expect("Git runtime fixture temp");
        let executable = git_executable();
        let repository = temp.path().join("resource-repository");
        fs::create_dir(&repository).expect("resource repository");
        run_git(&executable, &repository, &["init", "-b", "main"]);
        run_git(
            &executable,
            &repository,
            &["config", "user.name", "Resource User"],
        );
        run_git(
            &executable,
            &repository,
            &["config", "user.email", "resource@example.com"],
        );
        fs::write(repository.join("main.rs"), "fn main() {}\n").expect("resource source");
        run_git(&executable, &repository, &["add", "-A"]);
        run_git(&executable, &repository, &["commit", "-m", "resource root"]);
        Self {
            archive: temp.path().join("git-resource.sqlite3"),
            _temp: temp,
            repository,
            executable,
            sequence: 0,
        }
    }

    fn exercise_round(&mut self) {
        self.sequence = self.sequence.checked_add(1).expect("fixture sequence");
        let config = GitRuntimeConfig::new(self.archive.clone())
            .expect("Git runtime config")
            .with_executable(self.executable.clone())
            .expect("Git executable")
            .with_scan_timeout(Duration::from_secs(10))
            .expect("Git timeout");
        let mut runtime = GitRuntime::start(config).expect("start Git runtime");
        runtime
            .submit_hint(hint(&self.repository, self.sequence))
            .expect("submit Git hint");
        runtime.refresh_now().expect("force Git refresh");
        wait_publication(&runtime);
        let snapshot = runtime.snapshot().expect("Git runtime snapshot");
        assert_eq!(snapshot.retained_hint_count(), 1);
        assert!(snapshot.refresh().published_count() >= 1);
        runtime.pause().expect("pause Git runtime");
        assert_eq!(
            runtime
                .snapshot()
                .expect("paused Git runtime")
                .retained_hint_count(),
            1
        );
        runtime.resume().expect("resume Git runtime");
        runtime.shutdown().expect("shutdown Git runtime");
    }
}

#[cfg(windows)]
fn git_executable() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join("git.exe"))
        .find(|candidate| candidate.is_file())
        .expect("native Git on PATH")
}

#[cfg(windows)]
fn run_git(executable: &Path, repository: &Path, args: &[&str]) {
    let status = Command::new(executable)
        .current_dir(repository)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .expect("run Git fixture command");
    assert!(status.success(), "Git fixture command failed: {args:?}");
}

#[cfg(windows)]
fn hint(repository: &Path, sequence: u64) -> RepositoryActivityHint {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_secs();
    RepositoryActivityHint::new(RepositoryActivityHintParts {
        provider_id: UsageProviderId::new("codex").expect("provider"),
        profile_id: UsageProfileId::new("default").expect("profile"),
        source_id: UsageSourceId::new("resource-source").expect("source"),
        session_id: UsageSessionId::new(format!("resource-session-{sequence}")).expect("session"),
        observed_at: UtcTimestamp::new(i64::try_from(seconds).expect("seconds"), 0)
            .expect("timestamp"),
        project: Some(ProjectAlias::new("resource-project").expect("project")),
        candidate: RepositoryCandidatePath::new(repository.to_path_buf()).expect("candidate"),
    })
}

#[cfg(windows)]
fn wait_publication(runtime: &GitRuntime) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if runtime
            .snapshot()
            .expect("Git runtime snapshot")
            .refresh()
            .published_count()
            >= 1
        {
            return;
        }
        assert!(Instant::now() < deadline, "Git runtime publication timeout");
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
        .expect("Git runtime process handle count");
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
    .expect("Git runtime process memory");
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
        "Git handles grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.threads <= baseline.threads,
        "Git threads grew: baseline={baseline:?}, sample={sample:?}"
    );
    assert!(
        sample.user_objects <= baseline.user_objects && sample.gdi_objects <= baseline.gdi_objects,
        "Git GUI objects grew: baseline={baseline:?}, sample={sample:?}"
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
        .unwrap_or_else(|| panic!("Git runtime did not plateau: {warmup:?}"));

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
            "Git retained private memory grew: plateau={plateau:?}, private_floor={private_floor}, \
             private_limit={private_limit}, measured_floor={floor}, samples={private_samples:?}"
        );
    }
    println!(
        "git_runtime_resource_contract: pass warmup_rounds={} measured_rounds={} \
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
    println!("git_runtime_resource_contract: skipped (Windows-only gate)");
}
