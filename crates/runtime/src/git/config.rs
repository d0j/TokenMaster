use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokenmaster_git::{GitExecutable, GitExecutableSearchPath, MAX_GIT_PROCESS_TIMEOUT};

use crate::{RuntimeError, RuntimeErrorCode, RuntimeWriterLease};

const DEFAULT_GIT_SCAN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Eq, PartialEq)]
enum GitExecutableSelection {
    Automatic,
    Explicit(GitExecutable),
}

#[derive(Clone, Eq, PartialEq)]
pub struct GitRuntimeConfig {
    archive_path: PathBuf,
    executable: GitExecutableSelection,
    scan_timeout: Duration,
}

impl GitRuntimeConfig {
    pub fn new(archive_path: PathBuf) -> Result<Self, RuntimeError> {
        let _ = RuntimeWriterLease::new(&archive_path)?;
        Ok(Self {
            archive_path,
            executable: GitExecutableSelection::Automatic,
            scan_timeout: DEFAULT_GIT_SCAN_TIMEOUT,
        })
    }

    pub fn with_executable(mut self, path: PathBuf) -> Result<Self, RuntimeError> {
        self.executable = GitExecutableSelection::Explicit(
            GitExecutable::new(path)
                .map_err(|_| RuntimeError::new(RuntimeErrorCode::InvalidConfiguration))?,
        );
        Ok(self)
    }

    pub fn with_scan_timeout(mut self, timeout: Duration) -> Result<Self, RuntimeError> {
        if timeout.is_zero() || timeout > MAX_GIT_PROCESS_TIMEOUT {
            return Err(RuntimeError::new(RuntimeErrorCode::InvalidConfiguration));
        }
        self.scan_timeout = timeout;
        Ok(self)
    }

    pub(super) fn archive_path(&self) -> &Path {
        &self.archive_path
    }

    pub(super) const fn scan_timeout(&self) -> Duration {
        self.scan_timeout
    }

    pub(super) fn resolve_executable(
        &self,
    ) -> Result<GitExecutable, tokenmaster_git::GitBackendError> {
        match &self.executable {
            GitExecutableSelection::Automatic => {
                GitExecutableSearchPath::from_environment()?.resolve()
            }
            GitExecutableSelection::Explicit(executable) => Ok(executable.clone()),
        }
    }
}

impl fmt::Debug for GitRuntimeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitRuntimeConfig")
            .field("archive_path", &"[redacted]")
            .field(
                "executable",
                &match self.executable {
                    GitExecutableSelection::Automatic => "automatic",
                    GitExecutableSelection::Explicit(_) => "explicit",
                },
            )
            .field("scan_timeout", &self.scan_timeout)
            .finish()
    }
}
