use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use crate::{GitBackendError, GitBackendErrorCode, GitExecutable};

pub const MAX_GIT_EXECUTABLE_SEARCH_DIRS: usize = 128;
pub const MAX_GIT_EXECUTABLE_SEARCH_PATH_BYTES: usize = 64 * 1024;

#[derive(Clone, Eq, PartialEq)]
pub struct GitExecutableSearchPath {
    raw: OsString,
    entry_count: usize,
}

impl GitExecutableSearchPath {
    pub fn new(raw: OsString) -> Result<Self, GitBackendError> {
        if raw.len() > MAX_GIT_EXECUTABLE_SEARCH_PATH_BYTES {
            return Err(GitBackendError::with_limit(
                GitBackendErrorCode::CapacityExceeded,
                MAX_GIT_EXECUTABLE_SEARCH_PATH_BYTES,
            ));
        }
        let entry_count = env::split_paths(&raw).count();
        if entry_count > MAX_GIT_EXECUTABLE_SEARCH_DIRS {
            return Err(GitBackendError::with_limit(
                GitBackendErrorCode::CapacityExceeded,
                MAX_GIT_EXECUTABLE_SEARCH_DIRS,
            ));
        }
        Ok(Self { raw, entry_count })
    }

    pub fn from_environment() -> Result<Self, GitBackendError> {
        let raw = env::var_os("PATH")
            .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::Unavailable))?;
        Self::new(raw)
    }

    pub fn resolve(&self) -> Result<GitExecutable, GitBackendError> {
        env::split_paths(&self.raw)
            .filter(|directory| directory.is_absolute())
            .find_map(resolve_directory)
            .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::Unavailable))
    }
}

impl fmt::Debug for GitExecutableSearchPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitExecutableSearchPath")
            .field("entry_count", &self.entry_count)
            .finish()
    }
}

fn resolve_directory(directory: PathBuf) -> Option<GitExecutable> {
    GitExecutable::new(directory.join(native_name())).ok()
}

const fn native_name() -> &'static str {
    if cfg!(windows) { "git.exe" } else { "git" }
}
