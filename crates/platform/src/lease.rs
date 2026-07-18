use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::local_directory::{LocalDirectoryError, ValidatedLocalDirectory};
use crate::{PhysicalFileIdentity, PhysicalIdentityError};

pub const WRITER_LEASE_SUFFIX: &str = ".tokenmaster-writer.lock";

/// Path-owning factory for non-blocking cross-process archive writer guards.
pub struct ExclusiveFileLease {
    sidecar: PathBuf,
    archive_scope: [u8; 32],
}

impl ExclusiveFileLease {
    /// Resolves one stable sidecar identity beside an archive in a controlled local directory.
    pub fn for_archive(archive: &Path) -> Result<Self, ExclusiveFileLeaseError> {
        if !archive.is_absolute() {
            return Err(ExclusiveFileLeaseError::InvalidPath);
        }
        let parent = archive
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or(ExclusiveFileLeaseError::InvalidPath)?;
        let canonical_parent = ValidatedLocalDirectory::new(parent)
            .map_err(map_local_directory_error)?
            .as_path()
            .to_path_buf();
        let mut sidecar_name = archive
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(ExclusiveFileLeaseError::InvalidPath)?
            .to_os_string();
        sidecar_name.push(WRITER_LEASE_SUFFIX);
        let archive = canonical_parent.join(
            archive
                .file_name()
                .filter(|name| !name.is_empty())
                .ok_or(ExclusiveFileLeaseError::InvalidPath)?,
        );
        Ok(Self {
            sidecar: canonical_parent.join(sidecar_name),
            archive_scope: hash_archive_scope(&archive),
        })
    }

    /// Attempts one exclusive OS lock without waiting.
    pub fn try_acquire(&self) -> Result<ExclusiveFileLeaseGuard, ExclusiveFileLeaseError> {
        reject_existing_link(&self.sidecar)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.sidecar)
            .map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        let metadata = file
            .metadata()
            .map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        if !metadata.is_file() || metadata.len() != 0 {
            return Err(ExclusiveFileLeaseError::InvalidSidecar);
        }
        let sidecar_identity =
            PhysicalFileIdentity::from_file(&file).map_err(map_identity_error)?;
        file.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => ExclusiveFileLeaseError::Contended,
            TryLockError::Error(_) => ExclusiveFileLeaseError::Unavailable,
        })?;
        Ok(ExclusiveFileLeaseGuard {
            file,
            sidecar: self.sidecar.clone(),
            sidecar_identity,
            archive_scope: self.archive_scope,
        })
    }

    /// Revalidates that an already-held guard owns this exact archive lease.
    ///
    /// This permits a bootstrap owner to hand the same continuously held OS lock to
    /// a downstream runtime without releasing and reacquiring the sidecar.
    pub fn authorize_guard(
        &self,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<(), ExclusiveFileLeaseError> {
        if guard.archive_scope != self.archive_scope || guard.sidecar != self.sidecar {
            return Err(ExclusiveFileLeaseError::InvalidSidecar);
        }
        reject_existing_link(&self.sidecar)?;
        let current = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.sidecar)
            .map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        let metadata = current
            .metadata()
            .map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        if !metadata.is_file() || metadata.len() != 0 {
            return Err(ExclusiveFileLeaseError::InvalidSidecar);
        }
        let current_identity =
            PhysicalFileIdentity::from_file(&current).map_err(map_identity_error)?;
        if current_identity == guard.sidecar_identity {
            Ok(())
        } else {
            Err(ExclusiveFileLeaseError::InvalidSidecar)
        }
    }
}

impl fmt::Debug for ExclusiveFileLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExclusiveFileLease([redacted])")
    }
}

/// Owns exactly one locked sidecar file handle. Drop releases the OS lock.
pub struct ExclusiveFileLeaseGuard {
    file: File,
    sidecar: PathBuf,
    sidecar_identity: PhysicalFileIdentity,
    archive_scope: [u8; 32],
}

impl ExclusiveFileLeaseGuard {
    pub(crate) fn authorizes_archive(
        &self,
        archive: &Path,
    ) -> Result<bool, ExclusiveFileLeaseError> {
        if self.archive_scope != hash_archive_scope(archive) {
            return Ok(false);
        }
        reject_existing_link(&self.sidecar)?;
        let current = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.sidecar)
            .map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        let metadata = current
            .metadata()
            .map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        if !metadata.is_file() || metadata.len() != 0 {
            return Ok(false);
        }
        let current_identity =
            PhysicalFileIdentity::from_file(&current).map_err(map_identity_error)?;
        Ok(current_identity == self.sidecar_identity)
    }
}

impl fmt::Debug for ExclusiveFileLeaseGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = &self.file;
        formatter.write_str("ExclusiveFileLeaseGuard([redacted])")
    }
}

/// Stable path- and OS-message-private lease failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ExclusiveFileLeaseError {
    #[error("writer lease path is invalid")]
    InvalidPath,
    #[error("writer lease location is unsupported")]
    UnsupportedLocation,
    #[error("writer lease is unavailable")]
    Unavailable,
    #[error("writer lease is already held")]
    Contended,
    #[error("writer lease sidecar is invalid")]
    InvalidSidecar,
}

fn reject_existing_link(path: &Path) -> Result<(), ExclusiveFileLeaseError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse_point(&metadata) => {
            Err(ExclusiveFileLeaseError::UnsupportedLocation)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ExclusiveFileLeaseError::Unavailable),
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

const fn map_local_directory_error(error: LocalDirectoryError) -> ExclusiveFileLeaseError {
    match error {
        LocalDirectoryError::InvalidPath => ExclusiveFileLeaseError::InvalidPath,
        LocalDirectoryError::UnsupportedLocation => ExclusiveFileLeaseError::UnsupportedLocation,
        LocalDirectoryError::Unavailable => ExclusiveFileLeaseError::Unavailable,
    }
}

const fn map_identity_error(_error: PhysicalIdentityError) -> ExclusiveFileLeaseError {
    ExclusiveFileLeaseError::Unavailable
}

#[cfg(windows)]
fn hash_archive_scope(path: &Path) -> [u8; 32] {
    use std::os::windows::ffi::OsStrExt;

    let mut hasher = Sha256::new();
    hasher.update(b"tm-writer-lease-scope-v1-windows");
    for unit in path.as_os_str().encode_wide() {
        hasher.update(unit.to_le_bytes());
    }
    hasher.finalize().into()
}

#[cfg(unix)]
fn hash_archive_scope(path: &Path) -> [u8; 32] {
    use std::os::unix::ffi::OsStrExt;

    let mut hasher = Sha256::new();
    hasher.update(b"tm-writer-lease-scope-v1-unix");
    hasher.update(path.as_os_str().as_bytes());
    hasher.finalize().into()
}

#[cfg(not(any(unix, windows)))]
fn hash_archive_scope(_path: &Path) -> [u8; 32] {
    [0_u8; 32]
}
