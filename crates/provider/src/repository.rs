use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokenmaster_domain::{
    ProjectAlias, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_platform::ValidatedLocalDirectory;

use crate::{MAX_PATH_BYTES, ProviderError};

/// Sealed, canonical local-directory candidate for read-only repository discovery.
#[derive(Clone, Eq, PartialEq)]
pub struct RepositoryCandidatePath {
    directory: Arc<ValidatedLocalDirectory>,
    byte_len: usize,
}

impl RepositoryCandidatePath {
    pub fn new(path: PathBuf) -> Result<Self, ProviderError> {
        if path_byte_len(&path) > MAX_PATH_BYTES || path_has_nul(&path) {
            return Err(ProviderError::invalid_path(MAX_PATH_BYTES));
        }
        let directory = ValidatedLocalDirectory::new(&path)
            .map_err(|_| ProviderError::invalid_path(MAX_PATH_BYTES))?;
        let byte_len = path_byte_len(directory.as_path());
        if byte_len > MAX_PATH_BYTES {
            return Err(ProviderError::invalid_path(MAX_PATH_BYTES));
        }
        Ok(Self {
            directory: Arc::new(directory),
            byte_len,
        })
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        self.directory.as_path()
    }

    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }
}

impl fmt::Debug for RepositoryCandidatePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RepositoryCandidatePath([redacted])")
    }
}

pub struct RepositoryActivityHintParts {
    pub provider_id: UsageProviderId,
    pub profile_id: UsageProfileId,
    pub source_id: UsageSourceId,
    pub session_id: UsageSessionId,
    pub observed_at: UtcTimestamp,
    pub project: Option<ProjectAlias>,
    pub candidate: RepositoryCandidatePath,
}

/// Provider-neutral, transient association between usage activity and a repository.
///
/// This value intentionally has no serialization implementation. The raw path is
/// available only to the trusted runtime consumer that performs repository discovery.
#[derive(Clone, Eq, PartialEq)]
pub struct RepositoryActivityHint {
    provider_id: UsageProviderId,
    profile_id: UsageProfileId,
    source_id: UsageSourceId,
    session_id: UsageSessionId,
    observed_at: UtcTimestamp,
    project: Option<ProjectAlias>,
    candidate: RepositoryCandidatePath,
}

impl RepositoryActivityHint {
    #[must_use]
    pub fn new(parts: RepositoryActivityHintParts) -> Self {
        Self {
            provider_id: parts.provider_id,
            profile_id: parts.profile_id,
            source_id: parts.source_id,
            session_id: parts.session_id,
            observed_at: parts.observed_at,
            project: parts.project,
            candidate: parts.candidate,
        }
    }

    #[must_use]
    pub const fn provider_id(&self) -> &UsageProviderId {
        &self.provider_id
    }

    #[must_use]
    pub const fn profile_id(&self) -> &UsageProfileId {
        &self.profile_id
    }

    #[must_use]
    pub const fn source_id(&self) -> &UsageSourceId {
        &self.source_id
    }

    #[must_use]
    pub const fn session_id(&self) -> &UsageSessionId {
        &self.session_id
    }

    #[must_use]
    pub const fn observed_at(&self) -> &UtcTimestamp {
        &self.observed_at
    }

    #[must_use]
    pub const fn project(&self) -> Option<&ProjectAlias> {
        self.project.as_ref()
    }

    #[must_use]
    pub const fn candidate(&self) -> &RepositoryCandidatePath {
        &self.candidate
    }
}

impl fmt::Debug for RepositoryActivityHint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RepositoryActivityHint([redacted])")
    }
}

#[cfg(windows)]
fn path_byte_len(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().count().saturating_mul(2)
}

#[cfg(not(windows))]
fn path_byte_len(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().len()
}

#[cfg(windows)]
fn path_has_nul(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().any(|unit| unit == 0)
}

#[cfg(not(windows))]
fn path_has_nul(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().contains(&0)
}
