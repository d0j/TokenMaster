use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;

use sha2::{Digest, Sha256};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx};

use super::{PhysicalFileIdentity, PhysicalIdentityError, from_digest};

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
