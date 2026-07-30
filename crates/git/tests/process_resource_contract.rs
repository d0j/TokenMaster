use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_git::{GitCancellation, GitExecutable, GitProcess, GitRunControl};

mod support;

fn native_name() -> &'static str {
    if cfg!(windows) { "git.exe" } else { "git" }
}

fn fixture_path() -> PathBuf {
    support::fixture_path()
}

#[test]
fn repeated_short_lived_processes_are_reaped_without_retained_children() {
    let temp = TempDir::new().expect("resource root");
    let directory = temp.path().join("success");
    fs::create_dir(&directory).expect("fixture directory");
    let executable_path = directory.join(native_name());
    fs::copy(fixture_path(), &executable_path).expect("copy fixture");
    let executable = GitExecutable::new(executable_path.clone()).expect("fixture executable");

    for _ in 0..64 {
        let process = GitProcess::new(
            executable.clone(),
            GitRunControl::new(Duration::from_secs(2), GitCancellation::new())
                .expect("run control"),
        );
        process.version().expect("version round");
    }

    let receipt = fs::read_to_string(directory.join("receipt.txt")).expect("fixture receipt");
    let pids = receipt
        .lines()
        .filter_map(|line| line.strip_prefix("pid="))
        .map(|value| value.parse::<u32>().expect("fixture pid"))
        .collect::<Vec<_>>();
    assert_eq!(pids.len(), 64);
    assert!(
        !any_process_exists(&pids, &executable_path),
        "a task-owned fixture process still exists"
    );
}

#[cfg(windows)]
fn any_process_exists(pids: &[u32], executable: &Path) -> bool {
    let script = "$ids = $env:TM_FIXTURE_PIDS -split ','; $live = Get-Process -Id $ids -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $env:TM_FIXTURE_PATH }; if ($live) { exit 0 } else { exit 1 }";
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env(
            "TM_FIXTURE_PIDS",
            pids.iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
        .env("TM_FIXTURE_PATH", executable)
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn any_process_exists(pids: &[u32], executable: &Path) -> bool {
    pids.iter().any(|pid| {
        std::fs::read_link(format!("/proc/{pid}/exe"))
            .ok()
            .and_then(|path| path.canonicalize().ok())
            .zip(executable.canonicalize().ok())
            .is_some_and(|(current, expected)| current == expected)
    })
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn any_process_exists(pids: &[u32], _executable: &Path) -> bool {
    pids.iter().any(|pid| {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    })
}
