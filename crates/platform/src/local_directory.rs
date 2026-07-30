use std::fmt;
use std::path::{Component, Path, PathBuf};

#[cfg(windows)]
use std::path::Prefix;

/// Canonical absolute directory on a supported local filesystem.
///
/// Construction rejects traversal components, network/device namespaces, mapped
/// remote drives on Windows, and any existing symlink or reparse-point ancestor.
#[derive(Clone, Eq, PartialEq)]
pub struct ValidatedLocalDirectory {
    path: PathBuf,
}

impl ValidatedLocalDirectory {
    pub fn new(path: &Path) -> Result<Self, LocalDirectoryError> {
        validate_shape(path)?;
        reject_unsupported_location(path)?;
        reject_linked_ancestors(path)?;
        let canonical =
            std::fs::canonicalize(path).map_err(|_| LocalDirectoryError::Unavailable)?;
        validate_shape(&canonical)?;
        reject_unsupported_location(&canonical)?;
        reject_linked_ancestors(&canonical)?;
        let metadata =
            std::fs::symlink_metadata(&canonical).map_err(|_| LocalDirectoryError::Unavailable)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(LocalDirectoryError::UnsupportedLocation);
        }
        Ok(Self { path: canonical })
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Debug for ValidatedLocalDirectory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ValidatedLocalDirectory([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LocalDirectoryError {
    #[error("local directory path is invalid")]
    InvalidPath,
    #[error("local directory location is unsupported")]
    UnsupportedLocation,
    #[error("local directory is unavailable")]
    Unavailable,
}

fn validate_shape(path: &Path) -> Result<(), LocalDirectoryError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || path_has_nul(path)
    {
        return Err(LocalDirectoryError::InvalidPath);
    }
    Ok(())
}

fn reject_linked_ancestors(path: &Path) -> Result<(), LocalDirectoryError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|_| LocalDirectoryError::Unavailable)?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(LocalDirectoryError::UnsupportedLocation);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn reject_unsupported_location(path: &Path) -> Result<(), LocalDirectoryError> {
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;
    use windows::core::PCWSTR;

    let drive = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(LocalDirectoryError::UnsupportedLocation),
        },
        _ => return Err(LocalDirectoryError::UnsupportedLocation),
    };
    let root = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
    // SAFETY: `root` is a valid NUL-terminated `X:\` UTF-16 string and the API
    // does not retain its pointer.
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    accept_windows_drive_type(drive_type)
}

#[cfg(windows)]
fn accept_windows_drive_type(drive_type: u32) -> Result<(), LocalDirectoryError> {
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_RAMDISK: u32 = 6;

    if matches!(drive_type, DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_RAMDISK) {
        Ok(())
    } else {
        Err(LocalDirectoryError::UnsupportedLocation)
    }
}

#[cfg(unix)]
fn reject_unsupported_location(path: &Path) -> Result<(), LocalDirectoryError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(LocalDirectoryError::UnsupportedLocation)
    }
}

#[cfg(not(any(unix, windows)))]
fn reject_unsupported_location(_path: &Path) -> Result<(), LocalDirectoryError> {
    Err(LocalDirectoryError::UnsupportedLocation)
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
fn path_has_nul(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().any(|unit| unit == 0)
}

#[cfg(not(windows))]
fn path_has_nul(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().contains(&0)
}

#[cfg(all(test, windows))]
mod tests {
    use super::{LocalDirectoryError, accept_windows_drive_type};

    #[test]
    fn windows_drive_type_policy_rejects_remote_media() {
        for local in [2, 3, 6] {
            assert_eq!(accept_windows_drive_type(local), Ok(()));
        }
        for unsupported in [0, 1, 4, 5, u32::MAX] {
            assert_eq!(
                accept_windows_drive_type(unsupported),
                Err(LocalDirectoryError::UnsupportedLocation)
            );
        }
    }
}
