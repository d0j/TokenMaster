use std::fs::File;
use std::os::unix::fs::MetadataExt;

use sha2::{Digest, Sha256};

use super::{PhysicalFileIdentity, PhysicalIdentityError, from_digest};

pub(super) fn platform_identity(
    file: &File,
) -> Result<PhysicalFileIdentity, PhysicalIdentityError> {
    let metadata = file
        .metadata()
        .map_err(|_| PhysicalIdentityError::QueryFailed)?;
    let mut hasher = Sha256::new();
    hasher.update(b"tm-physical-file-v1");
    hasher.update(metadata.dev().to_le_bytes());
    hasher.update(metadata.ino().to_le_bytes());
    Ok(from_digest(hasher.finalize()))
}
