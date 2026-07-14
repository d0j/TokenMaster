use std::fs::File;

use super::{PhysicalFileIdentity, PhysicalIdentityError};

pub(super) fn platform_identity(
    _file: &File,
) -> Result<PhysicalFileIdentity, PhysicalIdentityError> {
    Err(PhysicalIdentityError::Unavailable)
}
