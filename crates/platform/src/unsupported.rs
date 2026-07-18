use std::fs::File;
use std::path::Path;

pub(super) fn available_space(_path: &Path) -> Result<u64, DurableFileError> {
    Err(DurableFileError::Unavailable)
}

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

pub(super) fn replace_file_redundant_write_through(
    _source: &Path,
    _target: &Path,
) -> Result<(), DurableFileError> {
    Err(DurableFileError::UnsupportedLocation)
}
