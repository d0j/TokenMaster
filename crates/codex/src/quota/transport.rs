use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokenmaster_provider::MAX_PATH_BYTES;

use super::normalize::normalize_wire;
use super::wire::{AccountResponseWire, RateLimitsResponseWire};
use super::{CodexQuotaError, CodexQuotaErrorCode, CodexQuotaSnapshot};

pub const SUPPORTED_CODEX_APP_SERVER_VERSION: &str = "0.144.1";
pub const MAX_CODEX_APP_SERVER_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_CODEX_APP_SERVER_STDOUT_BYTES: usize = 1024 * 1024;
pub const MAX_CODEX_APP_SERVER_FRAMES: usize = 64;
pub const MAX_CODEX_APP_SERVER_TIMEOUT: Duration = Duration::from_secs(30);

const INITIALIZE_REQUEST_ID: u64 = 0;
const ACCOUNT_REQUEST_ID: u64 = 1;
const QUOTA_REQUEST_ID: u64 = 2;
const MAX_PROTOCOL_TEXT_BYTES: usize = 512;
const MAX_RPC_ERROR_TEXT_BYTES: usize = 1024;

#[derive(Clone, Eq, PartialEq)]
pub struct CodexAppServerCommand {
    executable: PathBuf,
}

impl CodexAppServerCommand {
    pub fn new(executable: PathBuf) -> Result<Self, CodexQuotaError> {
        if crate::path_policy::validate_local_root_namespace(&executable).is_err() {
            return Err(invalid_command());
        }
        let metadata = fs::symlink_metadata(&executable).map_err(|_| invalid_command())?;
        if !metadata.is_file() || crate::path_policy::is_reparse_point(&metadata) {
            return Err(invalid_command());
        }
        let executable = fs::canonicalize(executable).map_err(|_| invalid_command())?;
        if crate::path_policy::validate_local_root_namespace(&executable).is_err() {
            return Err(invalid_command());
        }
        #[cfg(windows)]
        if !executable
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return Err(invalid_command());
        }
        Ok(Self { executable })
    }

    fn spawn(&self) -> Result<Child, CodexQuotaError> {
        let mut command = Command::new(&self.executable);
        command
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hide_child_window(&mut command);
        command
            .spawn()
            .map_err(|_| CodexQuotaError::new(CodexQuotaErrorCode::SpawnFailed))
    }
}

impl fmt::Debug for CodexAppServerCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexAppServerCommand")
            .field("executable", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CodexQuotaTransport {
    command: CodexAppServerCommand,
    timeout: Duration,
}

impl CodexQuotaTransport {
    pub fn new(command: CodexAppServerCommand, timeout: Duration) -> Result<Self, CodexQuotaError> {
        if timeout.is_zero() || timeout > MAX_CODEX_APP_SERVER_TIMEOUT {
            return Err(CodexQuotaError::new(CodexQuotaErrorCode::InvalidTime));
        }
        Ok(Self { command, timeout })
    }

    pub fn poll(&self, poll_started_at_ms: i64) -> Result<CodexQuotaSnapshot, CodexQuotaError> {
        if poll_started_at_ms <= 0
            || poll_started_at_ms
                .checked_add(super::CODEX_QUOTA_STALE_MILLIS)
                .is_none()
        {
            return Err(CodexQuotaError::new(CodexQuotaErrorCode::InvalidTime));
        }
        let mut child = self.command.spawn()?;
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                stop_and_reap(&mut child)?;
                return Err(CodexQuotaError::new(CodexQuotaErrorCode::Unavailable));
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                drop(stdin);
                stop_and_reap(&mut child)?;
                return Err(CodexQuotaError::new(CodexQuotaErrorCode::Unavailable));
            }
        };
        let (sender, receiver) = sync_channel(1);
        let worker = match thread::Builder::new()
            .name(String::from("tokenmaster-codex-quota-io"))
            .spawn(move || {
                let result = run_protocol(stdin, stdout);
                let _ = sender.send(result);
            }) {
            Ok(worker) => worker,
            Err(_) => {
                stop_and_reap(&mut child)?;
                return Err(CodexQuotaError::new(CodexQuotaErrorCode::Unavailable));
            }
        };

        let wire_result = match receiver.recv_timeout(self.timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => {
                stop_reap_and_join(&mut child, worker)?;
                return Err(CodexQuotaError::new(CodexQuotaErrorCode::DeadlineExceeded));
            }
            Err(RecvTimeoutError::Disconnected) => {
                stop_reap_and_join(&mut child, worker)?;
                return Err(CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError));
            }
        };
        stop_reap_and_join(&mut child, worker)?;
        let (account, quota) = wire_result?;
        normalize_wire(account, quota, poll_started_at_ms)
    }
}

impl fmt::Debug for CodexQuotaTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexQuotaTransport")
            .field("command", &self.command)
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InitializeResponseWire {
    codex_home: String,
    platform_family: String,
    platform_os: String,
    user_agent: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcResponseWire<T> {
    id: u64,
    result: Option<T>,
    error: Option<RpcErrorWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcErrorWire {
    code: i64,
    message: String,
    data: Option<Value>,
}

struct ProtocolBounds {
    frame_count: usize,
    stdout_bytes: usize,
}

impl ProtocolBounds {
    const fn new() -> Self {
        Self {
            frame_count: 0,
            stdout_bytes: 0,
        }
    }
}

fn run_protocol(
    mut stdin: ChildStdin,
    stdout: ChildStdout,
) -> Result<(AccountResponseWire, RateLimitsResponseWire), CodexQuotaError> {
    let mut reader = BufReader::new(stdout);
    let mut bounds = ProtocolBounds::new();
    write_frame(
        &mut stdin,
        &json!({
            "method": "initialize",
            "id": INITIALIZE_REQUEST_ID,
            "params": {
                "clientInfo": {
                    "name": "tokenmaster",
                    "title": "TokenMaster",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "optOutNotificationMethods": [
                        "account/rateLimits/updated",
                        "remoteControl/status/changed"
                    ]
                }
            }
        }),
    )?;
    let initialize: InitializeResponseWire =
        read_response(&mut reader, &mut bounds, INITIALIZE_REQUEST_ID)?;
    validate_initialize(initialize)?;

    write_frame(&mut stdin, &json!({"method": "initialized", "params": {}}))?;
    write_frame(
        &mut stdin,
        &json!({
            "method": "account/read",
            "id": ACCOUNT_REQUEST_ID,
            "params": {"refreshToken": false}
        }),
    )?;
    let account = read_response(&mut reader, &mut bounds, ACCOUNT_REQUEST_ID)?;

    write_frame(
        &mut stdin,
        &json!({
            "method": "account/rateLimits/read",
            "id": QUOTA_REQUEST_ID,
            "params": null
        }),
    )?;
    let quota = read_response(&mut reader, &mut bounds, QUOTA_REQUEST_ID)?;
    Ok((account, quota))
}

fn write_frame(writer: &mut impl Write, value: &Value) -> Result<(), CodexQuotaError> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|_| CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError))?;
    writer
        .write_all(b"\n")
        .and_then(|()| writer.flush())
        .map_err(|_| CodexQuotaError::new(CodexQuotaErrorCode::ProcessExited))
}

fn read_response<T: DeserializeOwned>(
    reader: &mut impl BufRead,
    bounds: &mut ProtocolBounds,
    expected_id: u64,
) -> Result<T, CodexQuotaError> {
    let frame = read_frame(reader, bounds)?;
    let response: RpcResponseWire<T> = serde_json::from_slice(&frame)
        .map_err(|_| CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError))?;
    if response.id != expected_id {
        return Err(CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError));
    }
    match (response.result, response.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => {
            validate_rpc_error(&error)?;
            Err(CodexQuotaError::new(CodexQuotaErrorCode::RpcError))
        }
        (Some(_), Some(_)) | (None, None) => {
            Err(CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError))
        }
    }
}

fn read_frame(
    reader: &mut impl BufRead,
    bounds: &mut ProtocolBounds,
) -> Result<Vec<u8>, CodexQuotaError> {
    if bounds.frame_count == MAX_CODEX_APP_SERVER_FRAMES {
        return Err(CodexQuotaError::with_limit(
            CodexQuotaErrorCode::CapacityExceeded,
            MAX_CODEX_APP_SERVER_FRAMES,
        ));
    }
    let mut frame = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| CodexQuotaError::new(CodexQuotaErrorCode::ProcessExited))?;
        if available.is_empty() {
            return Err(CodexQuotaError::new(if frame.is_empty() {
                CodexQuotaErrorCode::ProcessExited
            } else {
                CodexQuotaErrorCode::ProtocolError
            }));
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |position| position + 1);
        let content = newline.map_or(available, |position| &available[..position]);
        if frame.len().saturating_add(content.len()) > MAX_CODEX_APP_SERVER_FRAME_BYTES {
            return Err(CodexQuotaError::with_limit(
                CodexQuotaErrorCode::CapacityExceeded,
                MAX_CODEX_APP_SERVER_FRAME_BYTES,
            ));
        }
        let next_total = bounds.stdout_bytes.checked_add(consumed).ok_or_else(|| {
            CodexQuotaError::with_limit(
                CodexQuotaErrorCode::CapacityExceeded,
                MAX_CODEX_APP_SERVER_STDOUT_BYTES,
            )
        })?;
        if next_total > MAX_CODEX_APP_SERVER_STDOUT_BYTES {
            return Err(CodexQuotaError::with_limit(
                CodexQuotaErrorCode::CapacityExceeded,
                MAX_CODEX_APP_SERVER_STDOUT_BYTES,
            ));
        }
        frame.extend_from_slice(content);
        reader.consume(consumed);
        bounds.stdout_bytes = next_total;
        if newline.is_some() {
            if frame.last() == Some(&b'\r') {
                frame.pop();
            }
            if frame.is_empty() {
                return Err(CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError));
            }
            bounds.frame_count += 1;
            return Ok(frame);
        }
    }
}

fn validate_initialize(initialize: InitializeResponseWire) -> Result<(), CodexQuotaError> {
    validate_protocol_text(&initialize.codex_home, MAX_PATH_BYTES)?;
    validate_protocol_text(&initialize.platform_family, MAX_PROTOCOL_TEXT_BYTES)?;
    validate_protocol_text(&initialize.platform_os, MAX_PROTOCOL_TEXT_BYTES)?;
    validate_protocol_text(&initialize.user_agent, MAX_PROTOCOL_TEXT_BYTES)?;
    let version = extract_version(&initialize.user_agent)
        .ok_or_else(|| CodexQuotaError::new(CodexQuotaErrorCode::UnsupportedVersion))?;
    if version != SUPPORTED_CODEX_APP_SERVER_VERSION {
        return Err(CodexQuotaError::new(
            CodexQuotaErrorCode::UnsupportedVersion,
        ));
    }
    Ok(())
}

fn extract_version(user_agent: &str) -> Option<&str> {
    let (_, suffix) = user_agent.split_once('/')?;
    let length = suffix
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .count();
    if length == 0 {
        None
    } else {
        suffix.get(..length)
    }
}

fn validate_rpc_error(error: &RpcErrorWire) -> Result<(), CodexQuotaError> {
    let _ = (error.code, &error.data);
    validate_protocol_text(&error.message, MAX_RPC_ERROR_TEXT_BYTES)
}

fn validate_protocol_text(value: &str, max_bytes: usize) -> Result<(), CodexQuotaError> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.chars().any(|character| character.is_control())
    {
        return Err(CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError));
    }
    Ok(())
}

fn join_worker(worker: JoinHandle<()>) -> Result<(), CodexQuotaError> {
    worker
        .join()
        .map_err(|_| CodexQuotaError::new(CodexQuotaErrorCode::ProtocolError))
}

fn stop_reap_and_join(child: &mut Child, worker: JoinHandle<()>) -> Result<(), CodexQuotaError> {
    let process_result = stop_and_reap(child);
    let worker_result = join_worker(worker);
    process_result.and(worker_result)
}

fn stop_and_reap(child: &mut Child) -> Result<(), CodexQuotaError> {
    match child.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) | Err(_) => {
            let _ = child.kill();
            child
                .wait()
                .map(|_| ())
                .map_err(|_| CodexQuotaError::new(CodexQuotaErrorCode::ProcessCleanupFailed))
        }
    }
}

#[cfg(windows)]
fn hide_child_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_child_window(_command: &mut Command) {}

const fn invalid_command() -> CodexQuotaError {
    CodexQuotaError::new(CodexQuotaErrorCode::InvalidCommand)
}
