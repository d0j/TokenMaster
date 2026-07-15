use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

#[cfg(windows)]
use std::path::Prefix;

pub const WRITER_LEASE_SUFFIX: &str = ".tokenmaster-writer.lock";

/// Path-owning factory for non-blocking cross-process archive writer guards.
pub struct ExclusiveFileLease {
    sidecar: PathBuf,
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
        reject_unsupported_location(parent)?;
        reject_linked_ancestors(parent)?;
        let canonical_parent =
            std::fs::canonicalize(parent).map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        if !canonical_parent.is_dir() {
            return Err(ExclusiveFileLeaseError::InvalidPath);
        }
        reject_unsupported_location(&canonical_parent)?;
        reject_linked_ancestors(&canonical_parent)?;
        let mut sidecar_name = archive
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(ExclusiveFileLeaseError::InvalidPath)?
            .to_os_string();
        sidecar_name.push(WRITER_LEASE_SUFFIX);
        Ok(Self {
            sidecar: canonical_parent.join(sidecar_name),
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
        file.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => ExclusiveFileLeaseError::Contended,
            TryLockError::Error(_) => ExclusiveFileLeaseError::Unavailable,
        })?;
        Ok(ExclusiveFileLeaseGuard { file })
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

fn reject_linked_ancestors(path: &Path) -> Result<(), ExclusiveFileLeaseError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|_| ExclusiveFileLeaseError::Unavailable)?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(ExclusiveFileLeaseError::UnsupportedLocation);
        }
    }
    Ok(())
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

#[cfg(windows)]
fn reject_unsupported_location(path: &Path) -> Result<(), ExclusiveFileLeaseError> {
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;
    use windows::core::PCWSTR;

    let drive = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(ExclusiveFileLeaseError::UnsupportedLocation),
        },
        _ => return Err(ExclusiveFileLeaseError::UnsupportedLocation),
    };
    let root = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
    // SAFETY: `root` is a valid NUL-terminated `X:\` UTF-16 string for the duration
    // of the call. `GetDriveTypeW` does not retain the pointer.
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    accept_windows_drive_type(drive_type)
}

#[cfg(windows)]
fn accept_windows_drive_type(drive_type: u32) -> Result<(), ExclusiveFileLeaseError> {
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_RAMDISK: u32 = 6;

    if matches!(drive_type, DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_RAMDISK) {
        Ok(())
    } else {
        Err(ExclusiveFileLeaseError::UnsupportedLocation)
    }
}

#[cfg(unix)]
fn reject_unsupported_location(path: &Path) -> Result<(), ExclusiveFileLeaseError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(ExclusiveFileLeaseError::UnsupportedLocation)
    }
}

#[cfg(not(any(unix, windows)))]
fn reject_unsupported_location(_path: &Path) -> Result<(), ExclusiveFileLeaseError> {
    Err(ExclusiveFileLeaseError::UnsupportedLocation)
}

#[cfg(all(test, windows))]
mod tests {
    use super::{ExclusiveFileLeaseError, accept_windows_drive_type};

    #[test]
    fn windows_drive_type_classification_rejects_remote_and_unknown_media() {
        for local in [2, 3, 6] {
            assert_eq!(accept_windows_drive_type(local), Ok(()));
        }
        for unsupported in [0, 1, 4, 5, u32::MAX] {
            assert_eq!(
                accept_windows_drive_type(unsupported),
                Err(ExclusiveFileLeaseError::UnsupportedLocation)
            );
        }
    }
}
