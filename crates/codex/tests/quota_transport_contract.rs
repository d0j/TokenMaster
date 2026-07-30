use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_codex::{
    CodexAppServerCommand, CodexQuotaErrorCode, CodexQuotaTransport,
    SUPPORTED_CODEX_APP_SERVER_VERSION,
};

const OBSERVED_AT_MS: i64 = 1_700_000_000_000;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codex_app_server_fixture"))
}

struct FixtureTransport {
    _temp: TempDir,
    executable: PathBuf,
    receipt: PathBuf,
    transport: CodexQuotaTransport,
}

impl FixtureTransport {
    fn new(mode: &str, timeout: Duration) -> Self {
        let temp = TempDir::new().expect("fixture temp");
        let extension = if cfg!(windows) { ".exe" } else { "" };
        let executable = temp
            .path()
            .join(format!("codex_app_server_fixture__{mode}{extension}"));
        fs::copy(fixture_path(), &executable).expect("copy fixture executable");
        let receipt = executable.with_extension("receipt");
        let command = CodexAppServerCommand::new(executable.clone()).expect("fixture command");
        let transport = CodexQuotaTransport::new(command, timeout).expect("fixture transport");
        Self {
            _temp: temp,
            executable,
            receipt,
            transport,
        }
    }

    fn receipt(&self) -> String {
        fs::read_to_string(&self.receipt).expect("fixture receipt")
    }
}

#[test]
fn success_uses_exact_protocol_and_returns_normalized_snapshot() {
    assert_eq!(SUPPORTED_CODEX_APP_SERVER_VERSION, "0.144.1");
    let fixture = FixtureTransport::new("success", Duration::from_secs(5));

    let snapshot = fixture
        .transport
        .poll(OBSERVED_AT_MS)
        .expect("fixture poll");

    assert_eq!(snapshot.observations().len(), 1);
    assert_eq!(snapshot.benefit_observation(), None);
    assert_eq!(
        snapshot.observations()[0]
            .definition()
            .key()
            .window_id()
            .as_str(),
        "codex.primary.10080"
    );
    assert_eq!(
        fixture.receipt().lines().skip(1).collect::<Vec<_>>(),
        vec![
            "argv=app-server|--stdio",
            "request=initialize",
            "notification=initialized",
            "request=account/read",
            "request=account/rateLimits/read",
        ]
    );
    assert_fixture_process_exited(&fixture.receipt());
}

#[test]
fn stderr_is_discarded_and_private_paths_are_redacted() {
    let fixture = FixtureTransport::new("stderr", Duration::from_secs(5));
    let rendered = format!("{:?}", fixture.transport);

    assert!(!rendered.contains(fixture_path().to_string_lossy().as_ref()));
    assert!(!rendered.contains("TOKENMASTER_CODEX_FIXTURE"));
    fixture
        .transport
        .poll(OBSERVED_AT_MS)
        .expect("stderr must not contaminate protocol");
    assert_fixture_process_exited(&fixture.receipt());
}

#[test]
fn unsupported_version_and_rpc_failures_use_stable_redacted_codes() {
    for (mode, expected) in [
        (
            "unsupported_version",
            CodexQuotaErrorCode::UnsupportedVersion,
        ),
        ("rpc_error", CodexQuotaErrorCode::RpcError),
        ("early_exit", CodexQuotaErrorCode::ProcessExited),
    ] {
        let fixture = FixtureTransport::new(mode, Duration::from_secs(5));
        let error = fixture
            .transport
            .poll(OBSERVED_AT_MS)
            .expect_err("fixture must fail");
        assert_eq!(error.code(), expected, "mode {mode}");
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("private"));
        assert!(!rendered.contains("backend failure"));
        assert!(!rendered.contains("codex-home"));
        assert_fixture_process_exited(&fixture.receipt());
    }
}

#[test]
fn malformed_oversized_and_wrong_sequence_frames_fail_closed() {
    for (mode, expected) in [
        ("malformed", CodexQuotaErrorCode::ProtocolError),
        ("unknown_field", CodexQuotaErrorCode::ProtocolError),
        ("oversized", CodexQuotaErrorCode::CapacityExceeded),
        ("wrong_id", CodexQuotaErrorCode::ProtocolError),
        ("duplicate_id", CodexQuotaErrorCode::ProtocolError),
        ("out_of_order", CodexQuotaErrorCode::ProtocolError),
    ] {
        let fixture = FixtureTransport::new(mode, Duration::from_secs(5));
        let error = fixture
            .transport
            .poll(OBSERVED_AT_MS)
            .expect_err("fixture must fail");
        assert_eq!(error.code(), expected, "mode {mode}");
        assert_fixture_process_exited(&fixture.receipt());
    }
}

#[test]
fn timeout_terminates_the_task_owned_child() {
    let fixture = FixtureTransport::new("hang", Duration::from_millis(100));
    let started = Instant::now();

    let error = fixture
        .transport
        .poll(OBSERVED_AT_MS)
        .expect_err("hanging fixture must time out");

    assert_eq!(error.code(), CodexQuotaErrorCode::DeadlineExceeded);
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_timeout_fixture_process_exited(&fixture);
}

#[test]
fn command_and_timeout_validation_precede_process_creation() {
    let relative = CodexAppServerCommand::new(PathBuf::from("codex.exe"))
        .expect_err("relative executable must fail");
    assert_eq!(relative.code(), CodexQuotaErrorCode::InvalidCommand);

    let command = CodexAppServerCommand::new(fixture_path()).expect("fixture command");
    let zero = CodexQuotaTransport::new(command.clone(), Duration::ZERO)
        .expect_err("zero timeout must fail");
    assert_eq!(zero.code(), CodexQuotaErrorCode::InvalidTime);

    let excessive = CodexQuotaTransport::new(command, Duration::from_secs(31))
        .expect_err("excessive timeout must fail");
    assert_eq!(excessive.code(), CodexQuotaErrorCode::InvalidTime);

    let fixture = FixtureTransport::new("success", Duration::from_secs(5));
    let invalid_clock = fixture
        .transport
        .poll(i64::MAX)
        .expect_err("invalid clock must fail before process creation");
    assert_eq!(invalid_clock.code(), CodexQuotaErrorCode::InvalidTime);
    assert!(!fixture.receipt.exists());
}

fn assert_fixture_process_exited(receipt: &str) {
    let pid = receipt
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .and_then(|value| value.parse::<u32>().ok())
        .expect("fixture pid receipt");
    assert!(!process_exists(pid), "fixture process {pid} still exists");
}

fn assert_timeout_fixture_process_exited(fixture: &FixtureTransport) {
    match fs::read_to_string(&fixture.receipt) {
        Ok(receipt) => assert_fixture_process_exited(&receipt),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("fixture receipt: {error}"),
    }
    assert!(
        !executable_process_exists(&fixture.executable),
        "fixture executable still has a running process"
    );
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    let script = "if (Get-Process -Id $env:TM_FIXTURE_PID -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }";
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("TM_FIXTURE_PID", pid.to_string())
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

#[cfg(not(windows))]
fn process_exists(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
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
