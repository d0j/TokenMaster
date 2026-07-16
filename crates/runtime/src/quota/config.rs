use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use tokenmaster_codex::{CodexAppServerCommand, MAX_CODEX_APP_SERVER_TIMEOUT};

use super::CodexExecutableSearchPath;
use super::discovery::invalid_explicit_command;
use crate::{RuntimeError, RuntimeErrorCode, RuntimeWriterLease};

const DEFAULT_CODEX_QUOTA_TRANSPORT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Eq, PartialEq)]
enum CodexExecutableSelection {
    Automatic,
    Explicit(CodexAppServerCommand),
}

#[derive(Clone, Eq, PartialEq)]
pub struct CodexQuotaRuntimeConfig {
    archive_path: PathBuf,
    executable: CodexExecutableSelection,
    transport_timeout: Duration,
}

impl CodexQuotaRuntimeConfig {
    pub fn new(archive_path: PathBuf) -> Result<Self, RuntimeError> {
        let _ = RuntimeWriterLease::new(&archive_path)?;
        Ok(Self {
            archive_path,
            executable: CodexExecutableSelection::Automatic,
            transport_timeout: DEFAULT_CODEX_QUOTA_TRANSPORT_TIMEOUT,
        })
    }

    pub fn with_executable(mut self, executable: PathBuf) -> Result<Self, RuntimeError> {
        self.executable = CodexExecutableSelection::Explicit(
            CodexAppServerCommand::new(executable).map_err(invalid_explicit_command)?,
        );
        Ok(self)
    }

    pub fn with_transport_timeout(mut self, timeout: Duration) -> Result<Self, RuntimeError> {
        if timeout.is_zero() || timeout > MAX_CODEX_APP_SERVER_TIMEOUT {
            return Err(RuntimeError::new(RuntimeErrorCode::InvalidConfiguration));
        }
        self.transport_timeout = timeout;
        Ok(self)
    }

    pub fn resolve_command(
        &self,
        automatic: &CodexExecutableSearchPath,
    ) -> Result<CodexAppServerCommand, super::CodexExecutableDiscoveryError> {
        match &self.executable {
            CodexExecutableSelection::Automatic => automatic.resolve(),
            CodexExecutableSelection::Explicit(command) => Ok(command.clone()),
        }
    }
}

impl fmt::Debug for CodexQuotaRuntimeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexQuotaRuntimeConfig")
            .field("archive_path", &"[redacted]")
            .field(
                "executable",
                &match self.executable {
                    CodexExecutableSelection::Automatic => "automatic",
                    CodexExecutableSelection::Explicit(_) => "explicit",
                },
            )
            .field("transport_timeout", &self.transport_timeout)
            .finish()
    }
}
