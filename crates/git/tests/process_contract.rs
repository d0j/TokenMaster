use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_git::{
    GitBackendErrorCode, GitCancellation, GitExecutable, GitExecutableSearchPath, GitProcess,
    GitRunControl, MAX_GIT_EXECUTABLE_SEARCH_DIRS, MAX_GIT_EXECUTABLE_SEARCH_PATH_BYTES,
};

mod support;

fn native_name() -> &'static str {
    if cfg!(windows) { "git.exe" } else { "git" }
}

fn fixture_path() -> PathBuf {
    support::fixture_path()
}

struct Fixture {
    _temp: TempDir,
    directory: PathBuf,
    executable: PathBuf,
    receipt: PathBuf,
}

impl Fixture {
    fn new(mode: &str) -> Self {
        let temp = TempDir::new().expect("fixture root");
        let directory = temp.path().join(mode);
        fs::create_dir(&directory).expect("fixture mode directory");
        let executable = directory.join(native_name());
        fs::copy(fixture_path(), &executable).expect("copy fixture");
        let receipt = directory.join("receipt.txt");
        Self {
            _temp: temp,
            directory,
            executable,
            receipt,
        }
    }

    fn process(&self, timeout: Duration) -> GitProcess {
        let executable = GitExecutable::new(self.executable.clone()).expect("fixture executable");
        let control = GitRunControl::new(timeout, GitCancellation::new()).expect("run control");
        GitProcess::new(executable, control)
    }

    fn receipt(&self) -> String {
        fs::read_to_string(&self.receipt).expect("fixture receipt")
    }
}

fn joined(paths: impl IntoIterator<Item = PathBuf>) -> OsString {
    std::env::join_paths(paths).expect("join paths")
}

#[test]
fn exact_native_discovery_is_bounded_ordered_and_path_private() {
    let first = Fixture::new("success");
    let second = Fixture::new("success");
    let search = GitExecutableSearchPath::new(joined([
        PathBuf::from("relative"),
        first
            .executable
            .parent()
            .expect("first parent")
            .to_path_buf(),
        second
            .executable
            .parent()
            .expect("second parent")
            .to_path_buf(),
    ]))
    .expect("search path");
    assert_eq!(
        search.resolve().expect("resolved executable"),
        GitExecutable::new(first.executable.clone()).expect("first executable")
    );
    assert!(!format!("{search:?}").contains(first.executable.to_string_lossy().as_ref()));

    if cfg!(windows) {
        fs::write(
            first
                .executable
                .parent()
                .expect("first parent")
                .join("git.cmd"),
            b"shim",
        )
        .expect("write shim");
    }

    let oversized =
        OsString::from("x".repeat(MAX_GIT_EXECUTABLE_SEARCH_PATH_BYTES.saturating_add(1)));
    assert_eq!(
        GitExecutableSearchPath::new(oversized)
            .expect_err("oversized path")
            .code(),
        GitBackendErrorCode::CapacityExceeded
    );
    let too_many = (0..=MAX_GIT_EXECUTABLE_SEARCH_DIRS)
        .map(|index| PathBuf::from(format!("C:\\missing-{index}")))
        .collect::<Vec<_>>();
    assert_eq!(
        GitExecutableSearchPath::new(joined(too_many))
            .expect_err("too many entries")
            .code(),
        GitBackendErrorCode::CapacityExceeded
    );
}

#[test]
fn version_command_uses_fixed_environment_and_rejects_unsupported_git() {
    let fixture = Fixture::new("success");
    let version = fixture
        .process(Duration::from_secs(2))
        .version()
        .expect("supported version");
    assert_eq!(version.major(), 2);
    assert_eq!(version.minor(), 54);
    let receipt = fixture.receipt();
    assert!(receipt.contains("argv=--version"));
    assert!(receipt.contains("optional_locks:0"));
    assert!(receipt.contains("prompt:0"));
    assert!(receipt.contains("pager:cat"));
    assert!(receipt.contains("no_color:1"));
    assert!(receipt.contains("isolated=dir:;work_tree:;index:;config:;trace:;askpass:"));
    assert!(!receipt.contains(fixture.executable.to_string_lossy().as_ref()));

    let unsupported = Fixture::new("unsupported");
    let error = unsupported
        .process(Duration::from_secs(2))
        .version()
        .expect_err("unsupported version");
    assert_eq!(error.code(), GitBackendErrorCode::UnsupportedVersion);
    assert!(!format!("{error:?} {error}").contains("2.20.0"));
}

#[test]
fn timeout_cancellation_and_output_caps_kill_and_reap_children() {
    let fixture = Fixture::new("hang");
    let started = Instant::now();
    let error = fixture
        .process(Duration::from_millis(100))
        .version()
        .expect_err("hang must time out");
    assert_eq!(error.code(), GitBackendErrorCode::DeadlineExceeded);
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_fixture_processes_exited(&fixture);

    let fixture = Fixture::new("hang");
    let cancellation = GitCancellation::new();
    let cancel_from_thread = cancellation.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        cancel_from_thread.cancel();
    });
    let executable = GitExecutable::new(fixture.executable.clone()).expect("fixture executable");
    let process = GitProcess::new(
        executable,
        GitRunControl::new(Duration::from_secs(2), cancellation).expect("run control"),
    );
    let error = process.version().expect_err("cancelled process");
    worker.join().expect("cancel worker");
    assert_eq!(error.code(), GitBackendErrorCode::Cancelled);
    assert_fixture_processes_exited(&fixture);

    for (mode, expected) in [
        ("stdout_oversized", GitBackendErrorCode::StdoutLimitExceeded),
        ("stderr_oversized", GitBackendErrorCode::StderrLimitExceeded),
    ] {
        let fixture = Fixture::new(mode);
        let error = fixture
            .process(Duration::from_secs(2))
            .version_with_limits(128, 128)
            .expect_err("bounded output must fail");
        assert_eq!(error.code(), expected, "mode={mode}");
        assert_receipt_pids_exited(&fixture.receipt(), &fixture.executable);
    }
}

#[test]
fn timeout_before_fixture_initialization_still_reaps_child() {
    let fixture = Fixture::new("delayed_start");
    let error = fixture
        .process(Duration::from_millis(100))
        .version()
        .expect_err("delayed fixture start must time out");
    assert_eq!(error.code(), GitBackendErrorCode::DeadlineExceeded);
    assert!(!fixture.receipt.exists());
    assert_fixture_processes_exited(&fixture);
}

#[test]
fn executable_validation_rejects_relative_wrong_name_and_symlink() {
    assert_eq!(
        GitExecutable::new(PathBuf::from(native_name()))
            .expect_err("relative executable")
            .code(),
        GitBackendErrorCode::InvalidExecutable
    );
    let root = TempDir::new().expect("validation root");
    let wrong_name = root
        .path()
        .join(if cfg!(windows) { "git.cmd" } else { "other" });
    fs::copy(fixture_path(), &wrong_name).expect("wrong-name fixture");
    assert_eq!(
        GitExecutable::new(wrong_name)
            .expect_err("wrong executable name")
            .code(),
        GitBackendErrorCode::InvalidExecutable
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new("success");
        let link = root.path().join(native_name());
        symlink(&fixture.executable, &link).expect("fixture symlink");
        assert_eq!(
            GitExecutable::new(link).expect("canonical native symlink"),
            GitExecutable::new(fixture.executable).expect("canonical native target")
        );
    }
}

#[test]
fn repository_candidate_rejects_linked_ancestors() {
    let root = TempDir::new().expect("validation root");
    let target = root.path().join("target");
    let linked = root.path().join("linked");
    fs::create_dir(&target).expect("target");

    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_dir(&target, &linked).is_err() {
            return;
        }
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &linked).expect("directory symlink");

    assert_eq!(
        tokenmaster_git::GitRepositoryCandidate::new(linked)
            .expect_err("linked candidate")
            .code(),
        GitBackendErrorCode::RepositoryPathRejected
    );
}

#[test]
fn missing_author_is_explicit_and_repository_paths_never_enter_arguments_or_errors() {
    let fixture = Fixture::new("missing_author");
    fs::create_dir(fixture.directory.join(".git")).expect("fixture common directory");
    let candidate = tokenmaster_git::GitRepositoryCandidate::new(fixture.directory.clone())
        .expect("repository candidate");
    let error = fixture
        .process(Duration::from_secs(2))
        .scan(
            &candidate,
            tokenmaster_git::GitIdentitySalt::from_bytes([4; 32]),
        )
        .expect_err("missing author must fail closed");
    assert_eq!(error.code(), GitBackendErrorCode::AuthorIdentityMissing);
    let receipt = fixture.receipt();
    assert!(receipt.contains("argv=rev-parse|--path-format=absolute"));
    assert!(receipt.contains("argv=config|--no-includes|--local|--get|user.email"));
    assert!(receipt.contains("argv=config|--no-includes|--global|--get|user.email"));
    assert!(!receipt.contains(fixture.directory.to_string_lossy().as_ref()));
    assert!(
        !format!("{candidate:?} {error:?} {error}")
            .contains(fixture.directory.to_string_lossy().as_ref())
    );

    let fixture = Fixture::new("author_error");
    fs::create_dir(fixture.directory.join(".git")).expect("fixture common directory");
    let candidate = tokenmaster_git::GitRepositoryCandidate::new(fixture.directory.clone())
        .expect("repository candidate");
    let error = fixture
        .process(Duration::from_secs(2))
        .scan(
            &candidate,
            tokenmaster_git::GitIdentitySalt::from_bytes([4; 32]),
        )
        .expect_err("config execution error must not look absent");
    assert_eq!(error.code(), GitBackendErrorCode::ProcessFailed);
}

#[test]
fn ref_change_during_scan_is_rejected_before_a_snapshot_can_escape() {
    let fixture = Fixture::new("history_change");
    fs::create_dir(fixture.directory.join(".git")).expect("fixture common directory");
    let candidate = tokenmaster_git::GitRepositoryCandidate::new(fixture.directory.clone())
        .expect("repository candidate");
    let error = fixture
        .process(Duration::from_secs(2))
        .scan(
            &candidate,
            tokenmaster_git::GitIdentitySalt::from_bytes([2; 32]),
        )
        .expect_err("mixed history must fail closed");
    assert_eq!(error.code(), GitBackendErrorCode::HistoryChangedDuringScan);
    let receipt = fixture.receipt();
    assert!(receipt.contains(
        "argv=--no-pager|--no-replace-objects|-c|core.pager=cat|-c|color.ui=false|-c|core.attributesFile=|-c|mailmap.file=|-c|mailmap.blob=|-c|log.showSignature=false|log|--branches|--root|--diff-merges=off|--raw|--numstat|-z|--no-color|--no-ext-diff|--no-textconv|--use-mailmap|--find-renames=50%|--format=format:%x1e%H%x00%at%x00%ae%x00%aE%x00%P%x00"
    ));
    assert!(!receipt.contains(fixture.directory.to_string_lossy().as_ref()));
    assert!(!receipt.contains("--all"));
    assert!(!receipt.contains("--remotes"));
    assert!(!receipt.contains("--tags"));
}

#[test]
fn scan_deadline_is_global_instead_of_reset_for_each_child() {
    let fixture = Fixture::new("slow_scan");
    fs::create_dir(fixture.directory.join(".git")).expect("fixture common directory");
    let candidate = tokenmaster_git::GitRepositoryCandidate::new(fixture.directory.clone())
        .expect("repository candidate");
    let started = Instant::now();
    let error = fixture
        .process(Duration::from_millis(100))
        .scan(
            &candidate,
            tokenmaster_git::GitIdentitySalt::from_bytes([3; 32]),
        )
        .expect_err("whole scan must share one deadline");
    assert_eq!(error.code(), GitBackendErrorCode::DeadlineExceeded);
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_fixture_processes_exited(&fixture);
}

fn assert_fixture_processes_exited(fixture: &Fixture) {
    match fs::read_to_string(&fixture.receipt) {
        Ok(receipt) => assert_receipt_pids_exited(&receipt, &fixture.executable),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("fixture receipt: {error}"),
    }
    assert!(
        !executable_process_exists(&fixture.executable),
        "fixture executable still has a running process"
    );
}

fn assert_receipt_pids_exited(receipt: &str, executable: &Path) {
    for pid in receipt
        .lines()
        .filter_map(|line| line.strip_prefix("pid="))
        .map(|value| value.parse::<u32>().expect("fixture pid"))
    {
        assert!(
            !process_exists(pid, executable),
            "fixture process {pid} still exists"
        );
    }
}

#[cfg(windows)]
fn process_exists(pid: u32, executable: &Path) -> bool {
    let script = "$p = Get-Process -Id $env:TM_FIXTURE_PID -ErrorAction SilentlyContinue; if ($p -and $p.Path -eq $env:TM_FIXTURE_PATH) { exit 0 } else { exit 1 }";
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("TM_FIXTURE_PID", pid.to_string())
        .env("TM_FIXTURE_PATH", executable)
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(windows)]
fn executable_process_exists(executable: &Path) -> bool {
    let script = "$p = Get-Process -ErrorAction SilentlyContinue | Where-Object { try { $_.Path -eq $env:TM_FIXTURE_PATH } catch { $false } } | Select-Object -First 1; if ($p) { exit 0 } else { exit 1 }";
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("TM_FIXTURE_PATH", executable)
        .status()
        .expect("inspect fixture executable process")
        .success()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn process_exists(pid: u32, executable: &Path) -> bool {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .and_then(|path| path.canonicalize().ok())
        .zip(executable.canonicalize().ok())
        .is_some_and(|(current, expected)| current == expected)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn executable_process_exists(executable: &Path) -> bool {
    let expected = executable
        .canonicalize()
        .expect("canonical fixture executable");
    let entries = std::fs::read_dir("/proc").expect("inspect process directory");
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            && std::fs::read_link(entry.path().join("exe"))
                .ok()
                .and_then(|path| path.canonicalize().ok())
                .is_some_and(|path| path == expected)
    })
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn process_exists(pid: u32, _executable: &Path) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn executable_process_exists(executable: &Path) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-axo", "command="])
        .output()
        .expect("inspect fixture executable processes");
    let executable = executable.to_string_lossy();
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        line.trim_start()
            .strip_prefix(executable.as_ref())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

#[cfg(not(any(windows, unix)))]
fn executable_process_exists(_executable: &Path) -> bool {
    panic!("fixture process inspection is unsupported on this platform")
}
