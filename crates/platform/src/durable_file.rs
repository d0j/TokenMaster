use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{LocalDirectoryError, ValidatedLocalDirectory};

#[cfg(unix)]
use crate::unix::{
    move_file_write_through, replace_file_redundant_write_through, replace_file_write_through,
};
#[cfg(not(any(unix, windows)))]
use crate::unsupported::{
    move_file_write_through, replace_file_redundant_write_through, replace_file_write_through,
};
#[cfg(windows)]
use crate::windows::{
    move_file_write_through, replace_file_redundant_write_through, replace_file_write_through,
};

/// Maximum create-new candidates retained per exact durable target.
pub const DURABLE_STAGE_ATTEMPTS: usize = 32;
/// Absolute v1 ceiling for any one staged durable file (64 GiB plus package overhead).
pub const MAX_DURABLE_FILE_BYTES: u64 = 64 * 1024 * 1024 * 1024 + 2 * 1024 * 1024;
/// Maximum caller-owned chunk accepted by the streaming writer.
pub const MAX_DURABLE_WRITE_CHUNK_BYTES: usize = 256 * 1024;

const MAX_CHILD_NAME_BYTES: usize = 96;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

/// One exact child below a validated local directory.
#[derive(Clone, Eq, PartialEq)]
pub struct DurableFileTarget {
    parent: PathBuf,
    child_name: String,
    path: PathBuf,
}

impl DurableFileTarget {
    /// Creates a path-owning descriptor from one validated parent and one safe child name.
    pub fn exact_child(
        parent: &ValidatedLocalDirectory,
        child_name: &str,
    ) -> Result<Self, DurableFileError> {
        validate_child_name(child_name)?;
        revalidate_parent(parent.as_path())?;
        let path = parent.as_path().join(child_name);
        reject_unexpected_existing_type(&path, true)?;
        Ok(Self {
            parent: parent.as_path().to_path_buf(),
            child_name: child_name.to_owned(),
            path,
        })
    }

    /// Creates one collision-safe, create-new staging file beside this target.
    pub fn create_staged(&self, max_bytes: u64) -> Result<DurableStagedFile, DurableFileError> {
        if max_bytes > MAX_DURABLE_FILE_BYTES {
            return Err(DurableFileError::CapacityExceeded);
        }
        revalidate_parent(&self.parent)?;
        reject_unexpected_existing_type(&self.path, true)?;

        for attempt in 0..DURABLE_STAGE_ATTEMPTS {
            let stage_name = format!(".{}.tokenmaster-stage-{attempt:02}", self.child_name);
            let stage_path = self.parent.join(stage_name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&stage_path)
            {
                Ok(file) => {
                    let metadata = match file.metadata() {
                        Ok(metadata) => metadata,
                        Err(_) => {
                            drop(file);
                            let _ = std::fs::remove_file(&stage_path);
                            return Err(DurableFileError::Unavailable);
                        }
                    };
                    if !metadata.is_file() {
                        drop(file);
                        let _ = std::fs::remove_file(&stage_path);
                        return Err(DurableFileError::UnexpectedType);
                    }
                    return Ok(DurableStagedFile {
                        path: Some(stage_path),
                        file: Some(file),
                        receipt: None,
                        max_bytes,
                        written: 0,
                        write_failed: false,
                        preserve_on_drop: false,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(DurableFileError::Unavailable),
            }
        }
        Err(DurableFileError::CollisionLimit)
    }

    /// Reads one exact regular child into caller-bounded memory, or returns `None` when absent.
    pub fn read_bounded(&self, max_bytes: u64) -> Result<Option<Vec<u8>>, DurableFileError> {
        if max_bytes > MAX_DURABLE_FILE_BYTES {
            return Err(DurableFileError::CapacityExceeded);
        }
        revalidate_parent(&self.parent)?;
        match existing_kind(&self.path)? {
            ExistingKind::Missing => return Ok(None),
            ExistingKind::Regular => {}
        }

        let mut file = File::open(&self.path).map_err(|_| DurableFileError::Unavailable)?;
        let expected_len = file
            .metadata()
            .map_err(|_| DurableFileError::Unavailable)?
            .len();
        if expected_len > max_bytes {
            return Err(DurableFileError::CapacityExceeded);
        }

        let mut bytes = Vec::new();
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|_| DurableFileError::Unavailable)?;
            if count == 0 {
                break;
            }
            let count_u64 = u64::try_from(count).map_err(|_| DurableFileError::Integrity)?;
            let next_len = u64::try_from(bytes.len())
                .map_err(|_| DurableFileError::CapacityExceeded)?
                .checked_add(count_u64)
                .ok_or(DurableFileError::CapacityExceeded)?;
            if next_len > max_bytes {
                return Err(DurableFileError::CapacityExceeded);
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        if u64::try_from(bytes.len()).map_err(|_| DurableFileError::Integrity)? != expected_len {
            return Err(DurableFileError::Integrity);
        }
        Ok(Some(bytes))
    }
}

impl fmt::Debug for DurableFileTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurableFileTarget([redacted])")
    }
}

/// Create-new staging file whose path and file handle never leave the platform package.
pub struct DurableStagedFile {
    path: Option<PathBuf>,
    file: Option<File>,
    receipt: Option<DurableFileReceipt>,
    max_bytes: u64,
    written: u64,
    write_failed: bool,
    preserve_on_drop: bool,
}

impl DurableStagedFile {
    /// Appends one bounded caller-owned chunk without exposing raw I/O errors.
    pub fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), DurableFileError> {
        if self.write_failed {
            return Err(DurableFileError::InvalidState);
        }
        if bytes.len() > MAX_DURABLE_WRITE_CHUNK_BYTES {
            return Err(DurableFileError::CapacityExceeded);
        }
        let result = {
            let file = self.file.as_mut().ok_or(DurableFileError::InvalidState)?;
            write_bounded(file, &mut self.written, self.max_bytes, bytes)
        };
        if result == Err(DurableFileError::Unavailable) {
            self.write_failed = true;
        }
        result
    }

    /// Flushes, closes, reopens, and verifies the exact expected length and SHA-256.
    pub fn seal(
        &mut self,
        expected_len: u64,
        expected_sha256: [u8; 32],
    ) -> Result<DurableFileReceipt, DurableFileError> {
        if self.receipt.is_some() || self.write_failed {
            return Err(DurableFileError::InvalidState);
        }
        if expected_len != self.written || expected_len > self.max_bytes {
            return Err(DurableFileError::Integrity);
        }
        let file = self.file.take().ok_or(DurableFileError::InvalidState)?;
        file.sync_all().map_err(|_| DurableFileError::Unavailable)?;
        drop(file);
        let path = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
        let receipt = verify_file(path, expected_len, expected_sha256)?;
        self.receipt = Some(receipt);
        Ok(receipt)
    }

    /// Publishes a sealed source only when the exact target is absent.
    pub fn publish_new(
        &mut self,
        target: &DurableFileTarget,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        self.publish_new_with(target, &mut NoopPublicationHook)
    }

    /// Atomically replaces an existing target while retaining the old file at backup.
    pub fn replace_existing(
        &mut self,
        target: &DurableFileTarget,
        backup: &DurableFileTarget,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        self.replace_existing_with(target, backup, &mut NoopPublicationHook)
    }

    /// Replaces one inactive redundant slot without creating a third backup child.
    pub fn replace_existing_redundant(
        &mut self,
        target: &DurableFileTarget,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        self.replace_existing_redundant_with(target, &mut NoopPublicationHook)
    }

    fn publish_new_with(
        &mut self,
        target: &DurableFileTarget,
        hook: &mut impl PublicationHook,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        let receipt = self.receipt.ok_or(DurableFileError::InvalidState)?;
        let source = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
        revalidate_parent(&target.parent)?;
        verify_file(source, receipt.len, receipt.sha256)?;
        match existing_kind(&target.path)? {
            ExistingKind::Missing => {}
            ExistingKind::Regular => return Err(DurableFileError::TargetExists),
        }
        hook.hit(PublicationBoundary::BeforeMove)?;
        if let Err(error) = move_file_write_through(source, &target.path) {
            self.preserve_recovery_artifacts(error);
            return Err(error);
        }
        self.path = None;
        hook.hit(PublicationBoundary::AfterMove)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        sync_existing_file(&target.path).map_err(|_| DurableFileError::RecoveryRequired)?;
        verify_file(&target.path, receipt.len, receipt.sha256)
            .map_err(|_| DurableFileError::RecoveryRequired)
    }

    fn replace_existing_with(
        &mut self,
        target: &DurableFileTarget,
        backup: &DurableFileTarget,
        hook: &mut impl PublicationHook,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        self.replace_existing_with_operation(target, backup, hook, replace_file_write_through)
    }

    fn replace_existing_redundant_with(
        &mut self,
        target: &DurableFileTarget,
        hook: &mut impl PublicationHook,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        let receipt = self.receipt.ok_or(DurableFileError::InvalidState)?;
        let source = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
        if source == target.path {
            return Err(DurableFileError::InvalidState);
        }
        revalidate_parent(&target.parent)?;
        verify_file(source, receipt.len, receipt.sha256)?;
        if existing_kind(&target.path)? == ExistingKind::Missing {
            return Err(DurableFileError::TargetMissing);
        }
        hook.hit(PublicationBoundary::BeforeReplace)?;
        if let Err(error) = replace_file_redundant_write_through(source, &target.path) {
            self.preserve_recovery_artifacts(error);
            return Err(error);
        }
        self.path = None;
        hook.hit(PublicationBoundary::AfterReplace)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        sync_existing_file(&target.path).map_err(|_| DurableFileError::RecoveryRequired)?;
        verify_file(&target.path, receipt.len, receipt.sha256)
            .map_err(|_| DurableFileError::RecoveryRequired)
    }

    fn replace_existing_with_operation(
        &mut self,
        target: &DurableFileTarget,
        backup: &DurableFileTarget,
        hook: &mut impl PublicationHook,
        operation: impl FnOnce(&Path, &Path, &Path) -> Result<(), DurableFileError>,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        let receipt = self.receipt.ok_or(DurableFileError::InvalidState)?;
        let source = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
        if target.path == backup.path || source == target.path || source == backup.path {
            return Err(DurableFileError::InvalidState);
        }
        revalidate_parent(&target.parent)?;
        revalidate_parent(&backup.parent)?;
        verify_file(source, receipt.len, receipt.sha256)?;
        if existing_kind(&target.path)? == ExistingKind::Missing {
            return Err(DurableFileError::TargetMissing);
        }
        let replaced_receipt = inspect_file(&target.path)?;
        if existing_kind(&backup.path)? == ExistingKind::Regular {
            return Err(DurableFileError::TargetExists);
        }
        hook.hit(PublicationBoundary::BeforeReplace)?;
        if let Err(error) = operation(&target.path, source, &backup.path) {
            self.preserve_recovery_artifacts(error);
            return Err(error);
        }
        self.path = None;
        hook.hit(PublicationBoundary::AfterReplace)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        sync_existing_file(&target.path).map_err(|_| DurableFileError::RecoveryRequired)?;
        sync_existing_file(&backup.path).map_err(|_| DurableFileError::RecoveryRequired)?;
        let published = verify_file(&target.path, receipt.len, receipt.sha256)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        verify_file(&backup.path, replaced_receipt.len, replaced_receipt.sha256)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        Ok(published)
    }

    fn preserve_recovery_artifacts(&mut self, error: DurableFileError) {
        if error == DurableFileError::RecoveryRequired {
            self.preserve_on_drop = true;
        }
    }
}

impl fmt::Debug for DurableStagedFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurableStagedFile([redacted])")
    }
}

impl Drop for DurableStagedFile {
    fn drop(&mut self) {
        if self.preserve_on_drop {
            return;
        }
        if let Some(path) = self.path.take() {
            self.file.take();
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Path-free proof of the exact bytes verified for one durable file.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DurableFileReceipt {
    len: u64,
    sha256: [u8; 32],
}

impl fmt::Debug for DurableFileReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurableFileReceipt([redacted])")
    }
}

impl DurableFileReceipt {
    #[must_use]
    pub const fn len(self) -> u64 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

/// Stable path- and OS-message-private durable file failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DurableFileError {
    #[error("durable file child name is invalid")]
    InvalidName,
    #[error("durable file location is unsupported")]
    UnsupportedLocation,
    #[error("durable file staging collision limit was reached")]
    CollisionLimit,
    #[error("durable file target already exists")]
    TargetExists,
    #[error("durable file target is missing")]
    TargetMissing,
    #[error("durable file child has an unexpected type")]
    UnexpectedType,
    #[error("durable file operation state is invalid")]
    InvalidState,
    #[error("durable file integrity verification failed")]
    Integrity,
    #[error("durable file capacity limit was exceeded")]
    CapacityExceeded,
    #[error("durable file operation is unavailable")]
    Unavailable,
    #[error("durable file operation requires recovery")]
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExistingKind {
    Missing,
    Regular,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationBoundary {
    BeforeMove,
    AfterMove,
    BeforeReplace,
    AfterReplace,
}

trait PublicationHook {
    fn hit(&mut self, boundary: PublicationBoundary) -> Result<(), DurableFileError>;
}

struct NoopPublicationHook;

impl PublicationHook for NoopPublicationHook {
    fn hit(&mut self, _boundary: PublicationBoundary) -> Result<(), DurableFileError> {
        Ok(())
    }
}

fn validate_child_name(child_name: &str) -> Result<(), DurableFileError> {
    let bytes = child_name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_CHILD_NAME_BYTES
        || matches!(child_name, "." | "..")
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || child_name.ends_with('.')
    {
        return Err(DurableFileError::InvalidName);
    }

    let stem = child_name.split('.').next().unwrap_or_default();
    let upper = stem.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && matches!(upper.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        return Err(DurableFileError::InvalidName);
    }
    Ok(())
}

fn revalidate_parent(parent: &Path) -> Result<(), DurableFileError> {
    let validated = ValidatedLocalDirectory::new(parent).map_err(map_directory_error)?;
    if validated.as_path() == parent {
        Ok(())
    } else {
        Err(DurableFileError::UnsupportedLocation)
    }
}

fn existing_kind(path: &Path) -> Result<ExistingKind, DurableFileError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse_point(&metadata) => {
            Err(DurableFileError::UnsupportedLocation)
        }
        Ok(metadata) if metadata.is_file() => Ok(ExistingKind::Regular),
        Ok(_) => Err(DurableFileError::UnexpectedType),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(ExistingKind::Missing),
        Err(_) => Err(DurableFileError::Unavailable),
    }
}

fn reject_unexpected_existing_type(
    path: &Path,
    missing_is_allowed: bool,
) -> Result<(), DurableFileError> {
    match existing_kind(path)? {
        ExistingKind::Regular => Ok(()),
        ExistingKind::Missing if missing_is_allowed => Ok(()),
        ExistingKind::Missing => Err(DurableFileError::TargetMissing),
    }
}

fn verify_file(
    path: &Path,
    expected_len: u64,
    expected_sha256: [u8; 32],
) -> Result<DurableFileReceipt, DurableFileError> {
    let receipt = inspect_file(path)?;
    if receipt.len != expected_len || receipt.sha256 != expected_sha256 {
        return Err(DurableFileError::Integrity);
    }
    Ok(receipt)
}

fn inspect_file(path: &Path) -> Result<DurableFileReceipt, DurableFileError> {
    if existing_kind(path)? != ExistingKind::Regular {
        return Err(DurableFileError::TargetMissing);
    }
    let mut file = File::open(path).map_err(|_| DurableFileError::Unavailable)?;
    let metadata = file.metadata().map_err(|_| DurableFileError::Unavailable)?;
    if !metadata.is_file() || metadata.len() > MAX_DURABLE_FILE_BYTES {
        return Err(DurableFileError::Integrity);
    }
    let expected_len = metadata.len();

    let mut hasher = Sha256::new();
    let mut observed_len = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| DurableFileError::Unavailable)?;
        if read == 0 {
            break;
        }
        observed_len = observed_len
            .checked_add(u64::try_from(read).map_err(|_| DurableFileError::Integrity)?)
            .ok_or(DurableFileError::Integrity)?;
        if observed_len > expected_len {
            return Err(DurableFileError::Integrity);
        }
        hasher.update(&buffer[..read]);
    }
    let observed_sha256: [u8; 32] = hasher.finalize().into();
    if observed_len != expected_len {
        return Err(DurableFileError::Integrity);
    }
    Ok(DurableFileReceipt {
        len: observed_len,
        sha256: observed_sha256,
    })
}

fn write_bounded(
    writer: &mut impl Write,
    written: &mut u64,
    max_bytes: u64,
    mut bytes: &[u8],
) -> Result<(), DurableFileError> {
    let additional = u64::try_from(bytes.len()).map_err(|_| DurableFileError::CapacityExceeded)?;
    written
        .checked_add(additional)
        .filter(|total| *total <= max_bytes)
        .ok_or(DurableFileError::CapacityExceeded)?;

    while !bytes.is_empty() {
        let count = match writer.write(bytes) {
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(DurableFileError::Unavailable),
        };
        if count == 0 {
            return Err(DurableFileError::Unavailable);
        }
        let count_u64 = u64::try_from(count).map_err(|_| DurableFileError::CapacityExceeded)?;
        *written = written
            .checked_add(count_u64)
            .ok_or(DurableFileError::CapacityExceeded)?;
        bytes = &bytes[count..];
    }
    Ok(())
}

fn sync_existing_file(path: &Path) -> Result<(), DurableFileError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| DurableFileError::Unavailable)
}

const fn map_directory_error(error: LocalDirectoryError) -> DurableFileError {
    match error {
        LocalDirectoryError::InvalidPath => DurableFileError::UnsupportedLocation,
        LocalDirectoryError::UnsupportedLocation => DurableFileError::UnsupportedLocation,
        LocalDirectoryError::Unavailable => DurableFileError::Unavailable,
    }
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        DurableFileError, DurableFileTarget, PublicationBoundary, PublicationHook, write_bounded,
    };
    use crate::ValidatedLocalDirectory;
    use sha2::{Digest, Sha256};
    use std::io::{self, Write};
    use std::path::Path;
    use tempfile::TempDir;

    struct FailAt(PublicationBoundary);

    struct PartialThenFail {
        bytes: Vec<u8>,
        first: bool,
    }

    impl Write for PartialThenFail {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.first {
                self.first = false;
                let count = bytes.len().min(2);
                self.bytes.extend_from_slice(&bytes[..count]);
                Ok(count)
            } else {
                Err(io::Error::other("injected partial failure"))
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl PublicationHook for FailAt {
        fn hit(&mut self, boundary: PublicationBoundary) -> Result<(), DurableFileError> {
            if boundary == self.0 {
                Err(DurableFileError::Unavailable)
            } else {
                Ok(())
            }
        }
    }

    fn staged(target: &DurableFileTarget, bytes: &[u8]) -> super::DurableStagedFile {
        let mut staged = target.create_staged(1024).expect("stage");
        staged.write_chunk(bytes).expect("write");
        staged
            .seal(bytes.len() as u64, Sha256::digest(bytes).into())
            .expect("seal");
        staged
    }

    #[test]
    fn injected_move_boundaries_leave_only_prepared_or_published_state() {
        for boundary in [
            PublicationBoundary::BeforeMove,
            PublicationBoundary::AfterMove,
        ] {
            let root = TempDir::new().expect("root");
            let directory = ValidatedLocalDirectory::new(root.path()).expect("directory");
            let target = DurableFileTarget::exact_child(&directory, "move.slot").expect("target");
            let mut staged = staged(&target, b"new");
            assert_eq!(
                staged
                    .publish_new_with(&target, &mut FailAt(boundary))
                    .expect_err("injected failure"),
                if boundary == PublicationBoundary::AfterMove {
                    DurableFileError::RecoveryRequired
                } else {
                    DurableFileError::Unavailable
                }
            );
            assert_eq!(
                target.path.exists(),
                boundary == PublicationBoundary::AfterMove
            );
            assert_eq!(
                staged.path.is_some(),
                boundary == PublicationBoundary::BeforeMove
            );
        }
    }

    #[test]
    fn injected_replace_boundaries_leave_only_prepared_or_replaced_state() {
        for boundary in [
            PublicationBoundary::BeforeReplace,
            PublicationBoundary::AfterReplace,
        ] {
            let root = TempDir::new().expect("root");
            let directory = ValidatedLocalDirectory::new(root.path()).expect("directory");
            let target = DurableFileTarget::exact_child(&directory, "target.slot").expect("target");
            let backup = DurableFileTarget::exact_child(&directory, "backup.slot").expect("backup");
            std::fs::write(&target.path, b"old").expect("old target");
            let mut staged = staged(&target, b"new");
            assert_eq!(
                staged
                    .replace_existing_with(&target, &backup, &mut FailAt(boundary))
                    .expect_err("injected failure"),
                if boundary == PublicationBoundary::AfterReplace {
                    DurableFileError::RecoveryRequired
                } else {
                    DurableFileError::Unavailable
                }
            );
            if boundary == PublicationBoundary::BeforeReplace {
                assert_eq!(std::fs::read(&target.path).expect("old"), b"old");
                assert!(!backup.path.exists());
                assert!(staged.path.is_some());
            } else {
                assert_eq!(std::fs::read(&target.path).expect("new"), b"new");
                assert_eq!(std::fs::read(&backup.path).expect("backup"), b"old");
                assert!(staged.path.is_none());
            }
        }
    }

    #[test]
    fn injected_redundant_replace_boundaries_leave_only_prepared_or_replaced_state() {
        for boundary in [
            PublicationBoundary::BeforeReplace,
            PublicationBoundary::AfterReplace,
        ] {
            let root = TempDir::new().expect("root");
            let directory = ValidatedLocalDirectory::new(root.path()).expect("directory");
            let target = DurableFileTarget::exact_child(&directory, "target.slot").expect("target");
            std::fs::write(&target.path, b"old").expect("old target");
            let mut staged = staged(&target, b"new");
            assert_eq!(
                staged
                    .replace_existing_redundant_with(&target, &mut FailAt(boundary))
                    .expect_err("injected redundant failure"),
                if boundary == PublicationBoundary::AfterReplace {
                    DurableFileError::RecoveryRequired
                } else {
                    DurableFileError::Unavailable
                }
            );
            if boundary == PublicationBoundary::BeforeReplace {
                assert_eq!(std::fs::read(&target.path).expect("old"), b"old");
                assert!(staged.path.is_some());
            } else {
                assert_eq!(std::fs::read(&target.path).expect("new"), b"new");
                assert!(staged.path.is_none());
            }
        }
    }

    #[test]
    fn partial_write_failure_advances_the_exact_retained_byte_counter() {
        let mut writer = PartialThenFail {
            bytes: Vec::new(),
            first: true,
        };
        let mut written = 0_u64;
        assert_eq!(
            write_bounded(&mut writer, &mut written, 4, b"1234").expect_err("second write fails"),
            DurableFileError::Unavailable
        );
        assert_eq!(writer.bytes, b"12");
        assert_eq!(written, 2);
        assert_eq!(
            write_bounded(&mut Vec::new(), &mut written, 4, b"345")
                .expect_err("stale counter must not admit excess"),
            DurableFileError::CapacityExceeded
        );
    }

    #[test]
    fn recovery_required_preserves_source_and_displaced_backup_on_drop() {
        let root = TempDir::new().expect("root");
        let directory = ValidatedLocalDirectory::new(root.path()).expect("directory");
        let target = DurableFileTarget::exact_child(&directory, "target.slot").expect("target");
        let backup = DurableFileTarget::exact_child(&directory, "backup.slot").expect("backup");
        std::fs::write(&target.path, b"old").expect("old target");
        let mut staged = staged(&target, b"new");
        let source = staged.path.clone().expect("staged path");
        let simulated_partial_replace = |target: &Path, _source: &Path, backup: &Path| {
            std::fs::rename(target, backup).expect("displace old target");
            Err(DurableFileError::RecoveryRequired)
        };

        assert_eq!(
            staged
                .replace_existing_with_operation(
                    &target,
                    &backup,
                    &mut super::NoopPublicationHook,
                    simulated_partial_replace,
                )
                .expect_err("rollback failure"),
            DurableFileError::RecoveryRequired
        );
        drop(staged);
        assert!(!target.path.exists());
        assert_eq!(std::fs::read(&backup.path).expect("old backup"), b"old");
        assert_eq!(std::fs::read(source).expect("preserved source"), b"new");
    }

    #[test]
    fn successful_platform_replace_must_still_produce_the_exact_old_backup() {
        let root = TempDir::new().expect("root");
        let directory = ValidatedLocalDirectory::new(root.path()).expect("directory");
        let target = DurableFileTarget::exact_child(&directory, "target.slot").expect("target");
        let backup = DurableFileTarget::exact_child(&directory, "backup.slot").expect("backup");
        std::fs::write(&target.path, b"old").expect("old target");
        let mut staged = staged(&target, b"new");
        let corrupting_replace = |target: &Path, source: &Path, backup: &Path| {
            std::fs::write(backup, b"not-old").expect("wrong backup");
            std::fs::rename(source, target).expect("publish source");
            Ok(())
        };

        assert_eq!(
            staged
                .replace_existing_with_operation(
                    &target,
                    &backup,
                    &mut super::NoopPublicationHook,
                    corrupting_replace,
                )
                .expect_err("wrong backup digest"),
            DurableFileError::RecoveryRequired
        );
        assert_eq!(std::fs::read(&target.path).expect("new target"), b"new");
        assert_eq!(
            std::fs::read(&backup.path).expect("wrong backup retained"),
            b"not-old"
        );
    }
}
