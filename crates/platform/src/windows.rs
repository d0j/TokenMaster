use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use sha2::{Digest, Sha256};
use windows::Win32::Foundation::{ERROR_UNABLE_TO_MOVE_REPLACEMENT_2, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FILE_ID_INFO, FILE_STANDARD_INFO, FileIdInfo, FileStandardInfo, GetFileInformationByHandleEx,
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACE_FILE_FLAGS,
    ReplaceFileW,
};
use windows::core::{HRESULT, PCWSTR};

use super::{DurableFileError, PhysicalFileIdentity, PhysicalIdentityError, from_digest};

pub(super) fn platform_identity(
    file: &File,
) -> Result<PhysicalFileIdentity, PhysicalIdentityError> {
    let mut info = FILE_ID_INFO::default();
    let size =
        u32::try_from(size_of::<FILE_ID_INFO>()).map_err(|_| PhysicalIdentityError::QueryFailed)?;

    // SAFETY: `file` remains open for the call, `info` is a correctly sized writable
    // FILE_ID_INFO buffer, and `size` is the exact size of that buffer.
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileIdInfo,
            std::ptr::addr_of_mut!(info).cast(),
            size,
        )
    }
    .map_err(|_| PhysicalIdentityError::QueryFailed)?;

    let mut hasher = Sha256::new();
    hasher.update(b"tm-physical-file-v1");
    hasher.update(info.VolumeSerialNumber.to_le_bytes());
    hasher.update(info.FileId.Identifier);
    Ok(from_digest(hasher.finalize()))
}

pub(super) fn platform_link_count(file: &File) -> Result<u32, PhysicalIdentityError> {
    let mut info = FILE_STANDARD_INFO::default();
    let size = u32::try_from(size_of::<FILE_STANDARD_INFO>())
        .map_err(|_| PhysicalIdentityError::QueryFailed)?;

    // SAFETY: `file` remains open for the call, `info` is a correctly sized writable
    // FILE_STANDARD_INFO buffer, and `size` is the exact size of that buffer.
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileStandardInfo,
            std::ptr::addr_of_mut!(info).cast(),
            size,
        )
    }
    .map_err(|_| PhysicalIdentityError::QueryFailed)?;
    Ok(info.NumberOfLinks)
}

pub(super) fn move_file_write_through(
    source: &Path,
    target: &Path,
) -> Result<(), DurableFileError> {
    let source = wide_path(source)?;
    let target = wide_path(target)?;
    // SAFETY: both vectors are valid NUL-terminated UTF-16 paths retained for the call.
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|_| DurableFileError::Unavailable)
}

pub(super) fn replace_file_write_through(
    target: &Path,
    source: &Path,
    backup: &Path,
) -> Result<(), DurableFileError> {
    let target = wide_path(target)?;
    let source = wide_path(source)?;
    let backup = wide_path(backup)?;
    // SAFETY: all vectors are valid NUL-terminated UTF-16 paths retained for the call;
    // exclusion and reserved pointers are explicitly absent.
    let result = unsafe {
        ReplaceFileW(
            PCWSTR(target.as_ptr()),
            PCWSTR(source.as_ptr()),
            PCWSTR(backup.as_ptr()),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
    };
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.code() == HRESULT::from_win32(ERROR_UNABLE_TO_MOVE_REPLACEMENT_2.0) => {
            restore_displaced_target(target.as_slice(), backup.as_slice())
        }
        Err(_) => Err(DurableFileError::Unavailable),
    }
}

pub(super) fn replace_file_redundant_write_through(
    source: &Path,
    target: &Path,
) -> Result<(), DurableFileError> {
    let source = wide_path(source)?;
    let target = wide_path(target)?;
    // SAFETY: both vectors are valid NUL-terminated UTF-16 paths retained for the call.
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|_| DurableFileError::Unavailable)
}

fn restore_displaced_target(target: &[u16], backup: &[u16]) -> Result<(), DurableFileError> {
    // ReplaceFileW error 1177 leaves the replacement at its original path and moves
    // the replaced target to the requested backup name. Restore that old target
    // before returning the fixed failure category.
    let restored = unsafe {
        MoveFileExW(
            PCWSTR(backup.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_WRITE_THROUGH,
        )
    };
    match restored {
        Ok(()) => Err(DurableFileError::Unavailable),
        Err(_) => Err(DurableFileError::RecoveryRequired),
    }
}

fn wide_path(path: &Path) -> Result<Vec<u16>, DurableFileError> {
    use std::os::windows::ffi::OsStrExt;

    let mut units = Vec::with_capacity(path.as_os_str().len().saturating_add(1));
    units.extend(path.as_os_str().encode_wide());
    if units.contains(&0) {
        return Err(DurableFileError::UnsupportedLocation);
    }
    units.push(0);
    Ok(units)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{restore_displaced_target, wide_path};
    use crate::DurableFileError;
    use tempfile::TempDir;

    #[test]
    fn partial_replace_error_restores_displaced_old_target_write_through() {
        let root = TempDir::new().expect("root");
        let target = root.path().join("target.slot");
        let backup = root.path().join("backup.slot");
        std::fs::write(&backup, b"old").expect("displaced old target");
        let target_wide = wide_path(&target).expect("target path");
        let backup_wide = wide_path(&backup).expect("backup path");

        assert_eq!(
            restore_displaced_target(&target_wide, &backup_wide)
                .expect_err("original ReplaceFileW call still failed"),
            DurableFileError::Unavailable
        );
        assert_eq!(std::fs::read(target).expect("restored target"), b"old");
        assert!(!backup.exists());
    }
}
