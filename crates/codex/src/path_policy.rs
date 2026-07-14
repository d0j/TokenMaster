use std::path::Path;

#[cfg(windows)]
use std::path::Component;

use tokenmaster_provider::MAX_PATH_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PathPolicyCode {
    InvalidPath,
    UnsupportedRootNamespace,
}

pub(crate) fn validate_local_root_namespace(path: &Path) -> Result<(), PathPolicyCode> {
    if !path.is_absolute() || path_byte_len(path) > MAX_PATH_BYTES {
        return Err(PathPolicyCode::InvalidPath);
    }

    validate_prefix(path)
}

#[cfg(windows)]
fn validate_prefix(path: &Path) -> Result<(), PathPolicyCode> {
    use std::path::Prefix;

    match path.components().next() {
        Some(Component::Prefix(component)) => match component.kind() {
            Prefix::Disk(_) | Prefix::VerbatimDisk(_) => Ok(()),
            Prefix::Verbatim(_)
            | Prefix::VerbatimUNC(_, _)
            | Prefix::UNC(_, _)
            | Prefix::DeviceNS(_) => Err(PathPolicyCode::UnsupportedRootNamespace),
        },
        _ => Err(PathPolicyCode::InvalidPath),
    }
}

#[cfg(not(windows))]
fn validate_prefix(_path: &Path) -> Result<(), PathPolicyCode> {
    Ok(())
}

#[cfg(windows)]
pub(crate) fn path_byte_len(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().count().saturating_mul(2)
}

#[cfg(not(windows))]
pub(crate) fn path_byte_len(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().len()
}

#[cfg(windows)]
pub(crate) fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    has_reparse_attribute(metadata.file_attributes())
}

#[cfg(windows)]
const fn has_reparse_attribute(file_attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(crate) fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(all(test, windows))]
mod tests {
    use std::path::Path;

    use super::{
        PathPolicyCode, has_reparse_attribute, path_byte_len, validate_local_root_namespace,
    };

    #[test]
    fn local_disk_namespaces_are_supported() {
        for root in [r"C:\fixture", r"\\?\C:\fixture"] {
            assert_eq!(validate_local_root_namespace(Path::new(root)), Ok(()));
        }
    }

    #[test]
    fn relative_roots_are_invalid_before_namespace_classification() {
        assert_eq!(
            validate_local_root_namespace(Path::new("relative")),
            Err(PathPolicyCode::InvalidPath)
        );
    }

    #[test]
    fn remote_and_device_namespaces_are_rejected_without_io() {
        for root in [
            r"\\server\share\codex",
            r"\\?\UNC\server\share\codex",
            r"\\.\PhysicalDrive0",
            r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1",
        ] {
            assert_eq!(
                validate_local_root_namespace(Path::new(root)),
                Err(PathPolicyCode::UnsupportedRootNamespace)
            );
        }
    }

    #[test]
    fn path_length_uses_lossless_windows_bytes() {
        assert_eq!(path_byte_len(Path::new(r"C:\fixture")), 20);
    }

    #[test]
    fn every_windows_reparse_attribute_is_rejected_independent_of_tag() {
        assert!(!has_reparse_attribute(0));
        assert!(has_reparse_attribute(0x0000_0400));
        assert!(has_reparse_attribute(0x0000_0410));
    }
}
