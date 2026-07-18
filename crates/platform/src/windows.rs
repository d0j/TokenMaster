use std::ffi::OsString;
use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use windows::Win32::Foundation::{
    ERROR_CANCELLED, ERROR_UNABLE_TO_MOVE_REPLACEMENT_2, GENERIC_READ, GENERIC_WRITE, HANDLE, HWND,
};
use windows::Win32::Storage::FileSystem::{
    DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
    FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO, FileAttributeTagInfo,
    FileDispositionInfo, FileIdInfo, FileStandardInfo, GetFileInformationByHandleEx,
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACE_FILE_FLAGS,
    ReplaceFileW, SetFileInformationByHandle,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FILEOPENDIALOGOPTIONS, FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
    FOS_NOCHANGEDIR, FOS_NODEREFERENCELINKS, FOS_NOREADONLYRETURN, FOS_NOTESTFILECREATE,
    FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FOS_STRICTFILETYPES, FileOpenDialog, FileSaveDialog,
    IFileDialog, IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, PCWSTR, PWSTR};

use super::{DurableFileError, PhysicalFileIdentity, PhysicalIdentityError, from_digest};
use crate::file_dialog::{FileDialogFileType, NativeFileDialogAction};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeFileDialogError;

pub(super) fn select_native_file(
    action: NativeFileDialogAction,
    file_type: FileDialogFileType,
) -> Result<Option<PathBuf>, NativeFileDialogError> {
    let _apartment = ComApartment::initialize()?;
    // SAFETY: this thread-affine selector is called only from the active UI thread;
    // the returned borrowed handle is used only during the modal Show call.
    let owner = unsafe { GetActiveWindow() };
    if owner.0.is_null() {
        return Err(NativeFileDialogError);
    }
    match action {
        NativeFileDialogAction::Open => show_open_dialog(file_type, owner),
        NativeFileDialogAction::Save => show_save_dialog(file_type, owner),
    }
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self, NativeFileDialogError> {
        // SAFETY: the reserved pointer is absent and this balanced guard remains on
        // the calling thread until every dialog COM interface has been dropped.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result.is_ok() {
            Ok(Self)
        } else {
            Err(NativeFileDialogError)
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: construction succeeded on this same thread and every successful
        // CoInitializeEx, including S_FALSE, requires one balanced uninitialize.
        unsafe { CoUninitialize() };
    }
}

fn show_open_dialog(
    file_type: FileDialogFileType,
    owner: HWND,
) -> Result<Option<PathBuf>, NativeFileDialogError> {
    // SAFETY: the documented in-process FileOpenDialog class is requested without
    // aggregation after COM apartment initialization.
    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| NativeFileDialogError)?;
    configure_dialog(&dialog, file_type, NativeFileDialogAction::Open)?;
    show_and_extract(&dialog, owner)
}

fn show_save_dialog(
    file_type: FileDialogFileType,
    owner: HWND,
) -> Result<Option<PathBuf>, NativeFileDialogError> {
    // SAFETY: the documented in-process FileSaveDialog class is requested without
    // aggregation after COM apartment initialization.
    let dialog: IFileSaveDialog =
        unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| NativeFileDialogError)?;
    configure_dialog(&dialog, file_type, NativeFileDialogAction::Save)?;
    let default_name = wide_literal(file_type.default_file_name());
    // SAFETY: the NUL-terminated UTF-16 buffer remains live for the call.
    unsafe { dialog.SetFileName(PCWSTR(default_name.as_ptr())) }
        .map_err(|_| NativeFileDialogError)?;
    show_and_extract(&dialog, owner)
}

fn configure_dialog(
    dialog: &IFileDialog,
    file_type: FileDialogFileType,
    action: NativeFileDialogAction,
) -> Result<(), NativeFileDialogError> {
    let filter_name = wide_literal(file_type.filter_name());
    let filter_pattern = wide_literal(file_type.filter_pattern());
    let filters = [COMDLG_FILTERSPEC {
        pszName: PCWSTR(filter_name.as_ptr()),
        pszSpec: PCWSTR(filter_pattern.as_ptr()),
    }];
    let extension = wide_literal(file_type.extension());
    // SAFETY: both filter strings and the filter array remain live for each call;
    // the dialog copies their values before returning.
    unsafe { dialog.SetFileTypes(&filters) }.map_err(|_| NativeFileDialogError)?;
    // SAFETY: one filter was installed, so the one-based first index is valid.
    unsafe { dialog.SetFileTypeIndex(1) }.map_err(|_| NativeFileDialogError)?;
    // SAFETY: the NUL-terminated UTF-16 extension remains live for the call.
    unsafe { dialog.SetDefaultExtension(PCWSTR(extension.as_ptr())) }
        .map_err(|_| NativeFileDialogError)?;

    // SAFETY: the initialized dialog returns its current bit flags by value.
    let current = unsafe { dialog.GetOptions() }.map_err(|_| NativeFileDialogError)?;
    let common = FOS_FORCEFILESYSTEM.0
        | FOS_PATHMUSTEXIST.0
        | FOS_NOCHANGEDIR.0
        | FOS_NODEREFERENCELINKS.0
        | FOS_DONTADDTORECENT.0;
    let action_flags = match action {
        NativeFileDialogAction::Open => FOS_FILEMUSTEXIST.0,
        NativeFileDialogAction::Save => {
            FOS_OVERWRITEPROMPT.0
                | FOS_STRICTFILETYPES.0
                | FOS_NOREADONLYRETURN.0
                | FOS_NOTESTFILECREATE.0
        }
    };
    // SAFETY: only documented FILEOPENDIALOGOPTIONS bits are combined.
    unsafe { dialog.SetOptions(FILEOPENDIALOGOPTIONS(current.0 | common | action_flags)) }
        .map_err(|_| NativeFileDialogError)
}

fn show_and_extract(
    dialog: &IFileDialog,
    owner: HWND,
) -> Result<Option<PathBuf>, NativeFileDialogError> {
    // SAFETY: the dialog is fully configured, and Show runs its documented modal loop.
    if let Err(error) = unsafe { dialog.Show(Some(owner)) } {
        return if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
            Ok(None)
        } else {
            Err(NativeFileDialogError)
        };
    }
    // SAFETY: Show succeeded, so exactly one result is available; FORCEFILESYSTEM
    // requires it to provide a filesystem display name.
    let item = unsafe { dialog.GetResult() }.map_err(|_| NativeFileDialogError)?;
    let raw =
        unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.map_err(|_| NativeFileDialogError)?;
    OwnedTaskWide(raw).into_path().map(Some)
}

struct OwnedTaskWide(PWSTR);

impl OwnedTaskWide {
    fn into_path(self) -> Result<PathBuf, NativeFileDialogError> {
        if self.0.0.is_null() {
            return Err(NativeFileDialogError);
        }
        // SAFETY: a successful IShellItem::GetDisplayName returns one NUL-terminated
        // CoTaskMem-allocated UTF-16 string retained by this guard.
        let units = unsafe { self.0.as_wide() };
        Ok(PathBuf::from(OsString::from_wide(units)))
    }
}

impl Drop for OwnedTaskWide {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            // SAFETY: this pointer came from GetDisplayName and is freed exactly once.
            unsafe { CoTaskMemFree(Some(self.0.0.cast::<c_void>())) };
        }
    }
}

fn wide_literal(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(core::iter::once(0)).collect()
}

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

pub(super) fn create_stage_file(path: &Path) -> std::io::Result<File> {
    let access = GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .access_mode(access)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .create_new(true)
        .open(path)
}

pub(super) fn open_regular_no_follow(path: &Path) -> Result<File, DurableFileError> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|_| DurableFileError::Unavailable)?;
    validate_handle_kind(&file, false)?;
    Ok(file)
}

pub(super) fn open_regular_for_delete_no_follow(path: &Path) -> Result<File, DurableFileError> {
    let file = std::fs::OpenOptions::new()
        .access_mode(GENERIC_READ.0 | DELETE.0)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|_| DurableFileError::Unavailable)?;
    validate_handle_kind(&file, false)?;
    Ok(file)
}

pub(super) fn directory_identity(path: &Path) -> Result<PhysicalFileIdentity, DurableFileError> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .open(path)
        .map_err(|_| DurableFileError::Unavailable)?;
    validate_handle_kind(&file, true)?;
    platform_identity(&file).map_err(|_| DurableFileError::Unavailable)
}

pub(super) fn delete_stage_by_handle(file: &File) -> Result<(), DurableFileError> {
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    let size = u32::try_from(size_of::<FILE_DISPOSITION_INFO>())
        .map_err(|_| DurableFileError::Unavailable)?;
    // SAFETY: the stage handle was opened with DELETE access, remains valid for the
    // call, and `disposition` is a correctly sized immutable input structure.
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            std::ptr::addr_of!(disposition).cast(),
            size,
        )
    }
    .map_err(|_| DurableFileError::Unavailable)
}

fn validate_handle_kind(file: &File, expect_directory: bool) -> Result<(), DurableFileError> {
    let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
    let size = u32::try_from(size_of::<FILE_ATTRIBUTE_TAG_INFO>())
        .map_err(|_| DurableFileError::Unavailable)?;
    // SAFETY: `file` remains open and `info` is a correctly sized writable buffer.
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileAttributeTagInfo,
            std::ptr::addr_of_mut!(info).cast(),
            size,
        )
    }
    .map_err(|_| DurableFileError::Unavailable)?;
    if info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(DurableFileError::UnsupportedLocation);
    }
    let is_directory = info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
    if is_directory != expect_directory {
        return Err(DurableFileError::UnexpectedType);
    }
    Ok(())
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

pub(super) fn available_space(path: &Path) -> Result<u64, DurableFileError> {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::core::PCWSTR;

    let path = wide_path(path)?;
    let mut available = 0_u64;
    // SAFETY: `path` is a retained NUL-terminated UTF-16 string and the output
    // pointer refers to a live `u64` for the duration of the call.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(path.as_ptr()), Some(&mut available), None, None) }
        .map_err(|_| DurableFileError::Unavailable)?;
    Ok(available)
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
