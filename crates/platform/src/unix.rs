use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{DurableFileError, PhysicalFileIdentity, PhysicalIdentityError, from_digest};

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

pub(super) fn move_file_write_through(
    source: &Path,
    target: &Path,
) -> Result<(), DurableFileError> {
    match std::fs::hard_link(source, target) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(DurableFileError::TargetExists);
        }
        Err(_) => return Err(DurableFileError::Unavailable),
    }

    if sync_file(target).is_err() || sync_parent_io(target).is_err() {
        return Err(DurableFileError::RecoveryRequired);
    }
    if std::fs::remove_file(source).is_err() || sync_parent_io(source).is_err() {
        return Err(DurableFileError::RecoveryRequired);
    }
    Ok(())
}

pub(super) fn replace_file_write_through(
    target: &Path,
    source: &Path,
    backup: &Path,
) -> Result<(), DurableFileError> {
    match std::fs::hard_link(target, backup) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(DurableFileError::TargetExists);
        }
        Err(_) => return Err(DurableFileError::Unavailable),
    }

    if sync_file(backup).is_err() || sync_parent_io(backup).is_err() {
        return Err(DurableFileError::RecoveryRequired);
    }

    if std::fs::rename(source, target).is_err() {
        if std::fs::remove_file(backup).is_ok() && sync_parent_io(backup).is_ok() {
            return Err(DurableFileError::Unavailable);
        }
        return Err(DurableFileError::RecoveryRequired);
    }

    if sync_file(target).is_err()
        || sync_parent_io(source).is_err()
        || sync_parent_io(target).is_err()
        || sync_parent_io(backup).is_err()
    {
        return Err(DurableFileError::RecoveryRequired);
    }
    Ok(())
}

pub(super) fn replace_file_redundant_write_through(
    source: &Path,
    target: &Path,
) -> Result<(), DurableFileError> {
    std::fs::rename(source, target).map_err(|_| DurableFileError::Unavailable)?;
    if sync_file(target).is_err()
        || sync_parent_io(source).is_err()
        || sync_parent_io(target).is_err()
    {
        return Err(DurableFileError::RecoveryRequired);
    }
    Ok(())
}

fn sync_file(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

fn sync_parent_io(path: &Path) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing parent directory")
    })?;
    File::open(parent)?.sync_all()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{move_file_write_through, replace_file_write_through};
    use crate::DurableFileError;
    use tempfile::TempDir;

    #[test]
    fn create_new_never_overwrites_a_concurrent_target() {
        let root = TempDir::new().expect("root");
        let source = root.path().join("source.slot");
        let target = root.path().join("target.slot");
        std::fs::write(&source, b"new").expect("source");
        std::fs::write(&target, b"concurrent").expect("target");

        assert_eq!(
            move_file_write_through(&source, &target).expect_err("target exists"),
            DurableFileError::TargetExists
        );
        assert_eq!(std::fs::read(source).expect("source retained"), b"new");
        assert_eq!(
            std::fs::read(target).expect("target retained"),
            b"concurrent"
        );
    }

    #[test]
    fn occupied_backup_preserves_all_replacement_inputs() {
        let root = TempDir::new().expect("root");
        let source = root.path().join("source.slot");
        let target = root.path().join("target.slot");
        let backup = root.path().join("backup.slot");
        std::fs::write(&source, b"new").expect("source");
        std::fs::write(&target, b"old").expect("target");
        std::fs::write(&backup, b"occupied").expect("backup");

        assert_eq!(
            replace_file_write_through(&target, &source, &backup).expect_err("backup exists"),
            DurableFileError::TargetExists
        );
        assert_eq!(std::fs::read(source).expect("source retained"), b"new");
        assert_eq!(std::fs::read(target).expect("target retained"), b"old");
        assert_eq!(std::fs::read(backup).expect("backup retained"), b"occupied");
    }
}
