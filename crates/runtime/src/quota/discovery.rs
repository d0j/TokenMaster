use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use tokenmaster_codex::{CodexAppServerCommand, CodexQuotaError};

pub const MAX_CODEX_EXECUTABLE_SEARCH_DIRS: usize = 128;
pub const MAX_CODEX_EXECUTABLE_SEARCH_PATH_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexExecutableDiscoveryErrorCode {
    Unavailable,
    CapacityExceeded,
}

impl fmt::Display for CodexExecutableDiscoveryErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "unavailable",
            Self::CapacityExceeded => "capacity_exceeded",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodexExecutableDiscoveryError {
    code: CodexExecutableDiscoveryErrorCode,
}

impl CodexExecutableDiscoveryError {
    const fn new(code: CodexExecutableDiscoveryErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> CodexExecutableDiscoveryErrorCode {
        self.code
    }
}

impl fmt::Display for CodexExecutableDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Codex executable discovery error: {}", self.code)
    }
}

impl std::error::Error for CodexExecutableDiscoveryError {}

#[derive(Clone, Eq, PartialEq)]
pub struct CodexExecutableSearchPath {
    raw: OsString,
    entry_count: usize,
}

impl CodexExecutableSearchPath {
    pub fn new(raw: OsString) -> Result<Self, CodexExecutableDiscoveryError> {
        if raw.len() > MAX_CODEX_EXECUTABLE_SEARCH_PATH_BYTES {
            return Err(CodexExecutableDiscoveryError::new(
                CodexExecutableDiscoveryErrorCode::CapacityExceeded,
            ));
        }
        let entry_count = env::split_paths(&raw).count();
        if entry_count > MAX_CODEX_EXECUTABLE_SEARCH_DIRS {
            return Err(CodexExecutableDiscoveryError::new(
                CodexExecutableDiscoveryErrorCode::CapacityExceeded,
            ));
        }
        Ok(Self { raw, entry_count })
    }

    pub fn from_environment() -> Result<Self, CodexExecutableDiscoveryError> {
        let raw = env::var_os("PATH").ok_or_else(|| {
            CodexExecutableDiscoveryError::new(CodexExecutableDiscoveryErrorCode::Unavailable)
        })?;
        Self::new(raw)
    }

    pub fn resolve(&self) -> Result<CodexAppServerCommand, CodexExecutableDiscoveryError> {
        env::split_paths(&self.raw)
            .filter(|directory| directory.is_absolute())
            .find_map(resolve_directory)
            .ok_or_else(|| {
                CodexExecutableDiscoveryError::new(CodexExecutableDiscoveryErrorCode::Unavailable)
            })
    }
}

impl fmt::Debug for CodexExecutableSearchPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexExecutableSearchPath")
            .field("entry_count", &self.entry_count)
            .finish()
    }
}

fn resolve_directory(directory: PathBuf) -> Option<CodexAppServerCommand> {
    let candidate = directory.join(native_executable_name());
    CodexAppServerCommand::new(candidate).ok()
}

const fn native_executable_name() -> &'static str {
    if cfg!(windows) { "codex.exe" } else { "codex" }
}

pub(crate) fn invalid_explicit_command(_error: CodexQuotaError) -> crate::RuntimeError {
    crate::RuntimeError::new(crate::RuntimeErrorCode::InvalidConfiguration)
}
