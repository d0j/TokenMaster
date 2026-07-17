use std::fs::File;
use std::path::Path;

use super::{DurableFileError, PhysicalFileIdentity, PhysicalIdentityError};

pub(super) fn platform_identity(
    _file: &File,
) -> Result<PhysicalFileIdentity, PhysicalIdentityError> {
    Err(PhysicalIdentityError::Unavailable)
}

pub(super) fn move_file_write_through(
    _source: &Path,
    _target: &Path,
) -> Result<(), DurableFileError> {
    Err(DurableFileError::UnsupportedLocation)
}

pub(super) fn replace_file_write_through(
    _target: &Path,
    _source: &Path,
    _backup: &Path,
) -> Result<(), DurableFileError> {
    Err(DurableFileError::UnsupportedLocation)
}
