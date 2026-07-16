use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::process::{Child, ChildStdout, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{TryRecvError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{
    GitBackendError, GitBackendErrorCode, GitExecutable, GitLogParseConfig, GitLogStreamParser,
    GitScanAccumulator, GitScanSummary,
};

pub const MAX_GIT_PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_GIT_STDERR_BYTES: usize = 64 * 1024;
pub const MAX_GIT_LOG_STDOUT_BYTES: usize = 64 * 1024 * 1024;

const POLL_INTERVAL: Duration = Duration::from_millis(2);

#[derive(Clone, Debug)]
pub struct GitCancellation {
    cancelled: Arc<AtomicBool>,
}

impl GitCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Default for GitCancellation {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct GitRunControl {
    timeout: Duration,
    cancellation: GitCancellation,
}

impl GitRunControl {
    pub fn new(timeout: Duration, cancellation: GitCancellation) -> Result<Self, GitBackendError> {
        if timeout.is_zero() || timeout > MAX_GIT_PROCESS_TIMEOUT {
            return Err(GitBackendError::new(GitBackendErrorCode::InvalidTime));
        }
        Ok(Self {
            timeout,
            cancellation,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl GitVersion {
    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }

    #[must_use]
    pub const fn patch(self) -> u16 {
        self.patch
    }
}

#[derive(Clone, Debug)]
pub struct GitProcess {
    executable: GitExecutable,
    control: GitRunControl,
}

impl GitProcess {
    #[must_use]
    pub const fn new(executable: GitExecutable, control: GitRunControl) -> Self {
        Self {
            executable,
            control,
        }
    }

    pub(crate) fn operation_deadline(&self) -> Result<Instant, GitBackendError> {
        Instant::now()
            .checked_add(self.control.timeout)
            .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::InvalidTime))
    }

    pub(crate) fn limited_to(&self, deadline: Instant) -> Result<Self, GitBackendError> {
        if self.control.cancellation.is_cancelled() {
            return Err(GitBackendError::new(GitBackendErrorCode::Cancelled));
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::DeadlineExceeded))?;
        Ok(Self {
            executable: self.executable.clone(),
            control: GitRunControl {
                timeout: remaining.min(self.control.timeout),
                cancellation: self.control.cancellation.clone(),
            },
        })
    }

    pub fn version(&self) -> Result<GitVersion, GitBackendError> {
        self.version_with_limits(128, MAX_GIT_STDERR_BYTES)
    }

    pub fn version_with_limits(
        &self,
        stdout_limit: usize,
        stderr_limit: usize,
    ) -> Result<GitVersion, GitBackendError> {
        let result = self.capture_status(&["--version"], None, stdout_limit, stderr_limit)?;
        if !result.status.success() {
            return Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed));
        }
        parse_version(&result.stdout)
    }

    pub(crate) fn capture_status(
        &self,
        args: &[&str],
        current_dir: Option<&Path>,
        stdout_limit: usize,
        stderr_limit: usize,
    ) -> Result<CapturedOutput, GitBackendError> {
        self.run(args, current_dir, stderr_limit, move |stdout| {
            read_bounded(
                stdout,
                stdout_limit,
                GitBackendErrorCode::StdoutLimitExceeded,
            )
        })
        .map(|output| CapturedOutput {
            status: output.status,
            stdout: output.stdout,
        })
    }

    pub(crate) fn scan_log(
        &self,
        args: &[&str],
        current_dir: &Path,
        config: GitLogParseConfig,
    ) -> Result<GitScanSummary, GitBackendError> {
        let result = self.run(
            args,
            Some(current_dir),
            MAX_GIT_STDERR_BYTES,
            move |mut stdout| {
                let mut parser = GitLogStreamParser::new(config);
                let mut sink = LimitedScanSink::new();
                let mut total = 0_usize;
                let mut buffer = [0_u8; 16 * 1024];
                loop {
                    let read = stdout
                        .read(&mut buffer)
                        .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProcessFailed))?;
                    if read == 0 {
                        break;
                    }
                    total = total.checked_add(read).ok_or_else(|| {
                        GitBackendError::with_limit(
                            GitBackendErrorCode::StdoutLimitExceeded,
                            MAX_GIT_LOG_STDOUT_BYTES,
                        )
                    })?;
                    if total > MAX_GIT_LOG_STDOUT_BYTES {
                        return Err(GitBackendError::with_limit(
                            GitBackendErrorCode::StdoutLimitExceeded,
                            MAX_GIT_LOG_STDOUT_BYTES,
                        ));
                    }
                    parser
                        .push(&buffer[..read], &mut sink)
                        .map_err(map_core_error)?;
                }
                parser.finish(&mut sink).map_err(map_core_error)?;
                sink.finish()
            },
        )?;
        if !result.status.success() {
            return Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed));
        }
        Ok(result.stdout)
    }

    fn run<T: Send + 'static>(
        &self,
        args: &[&str],
        current_dir: Option<&Path>,
        stderr_limit: usize,
        stdout_worker: impl FnOnce(ChildStdout) -> Result<T, GitBackendError> + Send + 'static,
    ) -> Result<RunOutput<T>, GitBackendError> {
        if self.control.cancellation.is_cancelled() {
            return Err(GitBackendError::new(GitBackendErrorCode::Cancelled));
        }
        let deadline = Instant::now()
            .checked_add(self.control.timeout)
            .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::InvalidTime))?;
        let mut command = self.executable.command();
        command.args(args);
        if let Some(current_dir) = current_dir {
            command.current_dir(current_dir);
        }
        let mut child = ReapingChild::new(
            command
                .spawn()
                .map_err(|_| GitBackendError::new(GitBackendErrorCode::SpawnFailed))?,
        );
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| cleanup_error(&mut child, GitBackendErrorCode::ProcessFailed))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| cleanup_error(&mut child, GitBackendErrorCode::ProcessFailed))?;

        let (stdout_sender, stdout_receiver) = sync_channel(1);
        let stdout_thread = spawn_worker("tokenmaster-git-stdout", move || {
            let result = stdout_worker(stdout);
            let _ = stdout_sender.send(result);
        })
        .map_err(|error| cleanup_error(&mut child, error.code()))?;

        let (stderr_sender, stderr_receiver) = sync_channel(1);
        let stderr_thread = match spawn_worker("tokenmaster-git-stderr", move || {
            let result = drain_bounded(stderr, stderr_limit);
            let _ = stderr_sender.send(result);
        }) {
            Ok(worker) => worker,
            Err(error) => {
                let cleanup = stop_and_reap(&mut child);
                let stdout_cleanup = join_worker(stdout_thread);
                return cleanup.and(stdout_cleanup).and(Err(error));
            }
        };

        let mut stdout_result = None;
        let mut stderr_result = None;
        let status = loop {
            if let Err(error) = receive_ready(&stdout_receiver, &mut stdout_result) {
                stop_join_all(&mut child, stdout_thread, stderr_thread)?;
                return Err(error);
            }
            if let Err(error) = receive_ready(&stderr_receiver, &mut stderr_result) {
                stop_join_all(&mut child, stdout_thread, stderr_thread)?;
                return Err(error);
            }
            if let Some(Err(error)) = stdout_result.as_ref() {
                stop_join_all(&mut child, stdout_thread, stderr_thread)?;
                return Err(*error);
            }
            if let Some(Err(error)) = stderr_result.as_ref() {
                stop_join_all(&mut child, stdout_thread, stderr_thread)?;
                return Err(*error);
            }
            if self.control.cancellation.is_cancelled() {
                stop_join_all(&mut child, stdout_thread, stderr_thread)?;
                return Err(GitBackendError::new(GitBackendErrorCode::Cancelled));
            }
            if Instant::now() >= deadline {
                stop_join_all(&mut child, stdout_thread, stderr_thread)?;
                return Err(GitBackendError::new(GitBackendErrorCode::DeadlineExceeded));
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(_) => {
                    stop_join_all(&mut child, stdout_thread, stderr_thread)?;
                    return Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed));
                }
            }
        };

        let stdout_result = stdout_result.unwrap_or_else(|| {
            stdout_receiver
                .recv()
                .unwrap_or_else(|_| Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed)))
        });
        let stderr_result = stderr_result.unwrap_or_else(|| {
            stderr_receiver
                .recv()
                .unwrap_or_else(|_| Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed)))
        });
        let stdout_cleanup = join_worker(stdout_thread);
        let stderr_cleanup = join_worker(stderr_thread);
        stdout_cleanup.and(stderr_cleanup)?;
        let stdout = stdout_result?;
        stderr_result?;
        Ok(RunOutput { status, stdout })
    }
}

pub(crate) struct CapturedOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
}

struct RunOutput<T> {
    status: ExitStatus,
    stdout: T,
}

struct ReapingChild {
    inner: Child,
}

impl ReapingChild {
    const fn new(inner: Child) -> Self {
        Self { inner }
    }
}

impl Deref for ReapingChild {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ReapingChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for ReapingChild {
    fn drop(&mut self) {
        let _ = stop_and_reap(&mut self.inner);
    }
}

struct LimitedScanSink {
    inner: GitScanAccumulator,
}

impl LimitedScanSink {
    const fn new() -> Self {
        Self {
            inner: GitScanAccumulator::new(),
        }
    }

    fn finish(self) -> Result<GitScanSummary, GitBackendError> {
        self.inner.finish().map_err(map_core_error)
    }
}

impl crate::GitCommitSink for LimitedScanSink {
    fn push_commit(
        &mut self,
        commit: crate::GitCommitAggregate,
    ) -> Result<(), crate::GitCoreError> {
        self.inner.push(commit)
    }
}

fn parse_version(value: &[u8]) -> Result<GitVersion, GitBackendError> {
    let value = std::str::from_utf8(value)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))?
        .trim();
    let version = value
        .strip_prefix("git version ")
        .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
    let numeric = version
        .split_whitespace()
        .next()
        .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
    let mut components = numeric.split('.');
    let major = parse_component(components.next())?;
    let minor = parse_component(components.next())?;
    let patch_text = components
        .next()
        .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
    let patch_len = patch_text.bytes().take_while(u8::is_ascii_digit).count();
    if patch_len == 0 {
        return Err(GitBackendError::new(GitBackendErrorCode::ProtocolError));
    }
    let patch = patch_text[..patch_len]
        .parse::<u16>()
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
    if major < 2 || (major == 2 && minor < 31) {
        return Err(GitBackendError::new(
            GitBackendErrorCode::UnsupportedVersion,
        ));
    }
    Ok(GitVersion {
        major,
        minor,
        patch,
    })
}

fn parse_component(value: Option<&str>) -> Result<u16, GitBackendError> {
    value
        .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::ProtocolError))?
        .parse::<u16>()
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))
}

fn read_bounded(
    mut reader: impl Read,
    limit: usize,
    code: GitBackendErrorCode,
) -> Result<Vec<u8>, GitBackendError> {
    let mut output = Vec::with_capacity(limit.min(4096));
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProcessFailed))?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > limit {
            return Err(GitBackendError::with_limit(code, limit));
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> Result<(), GitBackendError> {
    let mut total = 0_usize;
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProcessFailed))?;
        if read == 0 {
            return Ok(());
        }
        total = total.checked_add(read).ok_or_else(|| {
            GitBackendError::with_limit(GitBackendErrorCode::StderrLimitExceeded, limit)
        })?;
        if total > limit {
            return Err(GitBackendError::with_limit(
                GitBackendErrorCode::StderrLimitExceeded,
                limit,
            ));
        }
    }
}

fn spawn_worker(
    name: &str,
    worker: impl FnOnce() + Send + 'static,
) -> Result<JoinHandle<()>, GitBackendError> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(worker)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProcessFailed))
}

fn receive_ready<T>(
    receiver: &std::sync::mpsc::Receiver<Result<T, GitBackendError>>,
    slot: &mut Option<Result<T, GitBackendError>>,
) -> Result<(), GitBackendError> {
    if slot.is_some() {
        return Ok(());
    }
    match receiver.try_recv() {
        Ok(value) => *slot = Some(value),
        Err(TryRecvError::Empty) => {}
        Err(TryRecvError::Disconnected) => {
            return Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed));
        }
    }
    Ok(())
}

fn stop_join_all(
    child: &mut Child,
    stdout: JoinHandle<()>,
    stderr: JoinHandle<()>,
) -> Result<(), GitBackendError> {
    let process = stop_and_reap(child);
    let stdout = join_worker(stdout);
    let stderr = join_worker(stderr);
    process.and(stdout).and(stderr)
}

fn stop_and_reap(child: &mut Child) -> Result<(), GitBackendError> {
    match child.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) | Err(_) => {
            let _ = child.kill();
            child
                .wait()
                .map(|_| ())
                .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProcessCleanupFailed))
        }
    }
}

fn join_worker(worker: JoinHandle<()>) -> Result<(), GitBackendError> {
    worker
        .join()
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProcessFailed))
}

fn cleanup_error(child: &mut Child, code: GitBackendErrorCode) -> GitBackendError {
    stop_and_reap(child)
        .err()
        .unwrap_or_else(|| GitBackendError::new(code))
}

fn map_core_error(error: crate::GitCoreError) -> GitBackendError {
    match error {
        crate::GitCoreError::CapacityExceeded { limit } => {
            GitBackendError::with_limit(GitBackendErrorCode::CapacityExceeded, limit)
        }
        _ => GitBackendError::new(GitBackendErrorCode::ProtocolError),
    }
}
