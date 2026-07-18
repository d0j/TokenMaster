use std::fmt;
use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{
    DurableFileError, DurableFileReader, DurableFileTarget, DurableStagedFile,
    MAX_DURABLE_FILE_BYTES, PhysicalFileIdentity, ValidatedLocalDirectory,
};

#[cfg(unix)]
use crate::unix::move_file_write_through;
#[cfg(not(any(unix, windows)))]
use crate::unsupported::move_file_write_through;
#[cfg(windows)]
use crate::windows::move_file_write_through;

pub const MAX_BACKUP_DIRECTORY_FILES: usize = 32;

const BACKUP_DIRECTORY_NAME: &str = "backups";
const BACKUP_FILE_PREFIX: &str = "point-";
const BACKUP_FILE_SUFFIX: &str = ".tmbackup";

/// Sealed access to the one fixed TokenMaster backup directory.
pub struct BackupDirectory {
    directory: ValidatedLocalDirectory,
    scope: [u8; 32],
}

impl BackupDirectory {
    /// Opens or creates the exact `backups` child below a validated reliable-state root.
    pub fn open_or_create(parent: &ValidatedLocalDirectory) -> Result<Self, BackupDirectoryError> {
        let current_parent = ValidatedLocalDirectory::new(parent.as_path())
            .map_err(|_| BackupDirectoryError::UnsupportedLocation)?;
        if current_parent != *parent {
            return Err(BackupDirectoryError::UnsupportedLocation);
        }

        let path = parent.as_path().join(BACKUP_DIRECTORY_NAME);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => validate_directory_metadata(&metadata)?,
            Err(error) if error.kind() == ErrorKind::NotFound => match std::fs::create_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(_) => return Err(BackupDirectoryError::Unavailable),
            },
            Err(_) => return Err(BackupDirectoryError::Unavailable),
        }

        let directory = ValidatedLocalDirectory::new(&path).map_err(map_directory_open_error)?;
        let scope = hash_path(directory.as_path());
        Ok(Self { directory, scope })
    }

    pub fn authorize_parent(
        &self,
        parent: &ValidatedLocalDirectory,
    ) -> Result<(), BackupDirectoryError> {
        self.revalidate_directory()?;
        let current_parent = ValidatedLocalDirectory::new(parent.as_path())
            .map_err(|_| BackupDirectoryError::UnsupportedLocation)?;
        if current_parent == *parent
            && self.directory.as_path() == parent.as_path().join(BACKUP_DIRECTORY_NAME)
        {
            Ok(())
        } else {
            Err(BackupDirectoryError::UnsupportedLocation)
        }
    }

    /// Enumerates the complete exact namespace into at most 32 opaque entries.
    pub fn scan(&self) -> Result<BackupDirectorySnapshot, BackupDirectoryError> {
        self.revalidate_directory()?;
        let children = std::fs::read_dir(self.directory.as_path())
            .map_err(|_| BackupDirectoryError::Unavailable)?;
        let mut entries = Vec::with_capacity(MAX_BACKUP_DIRECTORY_FILES);

        for child in children {
            let child = child.map_err(|_| BackupDirectoryError::Unavailable)?;
            let name = child
                .file_name()
                .into_string()
                .map_err(|_| BackupDirectoryError::UnexpectedEntry)?;
            let slot = match parse_slot_name(&name) {
                Some(slot) => slot,
                None if is_controlled_recovery_name(&name) => {
                    return Err(BackupDirectoryError::RecoveryRequired);
                }
                None => return Err(BackupDirectoryError::UnexpectedEntry),
            };
            if entries.len() >= MAX_BACKUP_DIRECTORY_FILES {
                return Err(BackupDirectoryError::CapacityExceeded);
            }
            let entry = self
                .inspect_slot(slot)?
                .ok_or(BackupDirectoryError::Unavailable)?;
            if entries.iter().any(|existing: &BackupDirectoryEntry| {
                existing.ordinal == entry.ordinal || existing.identity == entry.identity
            }) {
                return Err(BackupDirectoryError::AmbiguousIdentity);
            }
            entries.push(entry);
        }

        entries.sort_unstable_by_key(|entry| entry.ordinal);
        let generation = BackupDirectoryGeneration(hash_entries(self.scope, &entries));
        Ok(BackupDirectorySnapshot {
            generation,
            entries,
        })
    }

    /// Creates an unpublished stage for the first free exact package slot.
    pub fn create_staged(&self, max_bytes: u64) -> Result<BackupStagedFile, BackupDirectoryError> {
        if max_bytes > MAX_DURABLE_FILE_BYTES {
            return Err(BackupDirectoryError::CapacityExceeded);
        }
        let snapshot = self.scan()?;
        let slot = (0..MAX_BACKUP_DIRECTORY_FILES)
            .map(|value| u8::try_from(value).map_err(|_| BackupDirectoryError::CapacityExceeded))
            .find_map(|candidate| match candidate {
                Ok(candidate)
                    if !snapshot
                        .entries
                        .iter()
                        .any(|entry| entry.ordinal == candidate) =>
                {
                    Some(Ok(candidate))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .transpose()?
            .ok_or(BackupDirectoryError::CapacityExceeded)?;
        let target = self.target(slot)?;
        let file = target.create_staged(max_bytes).map_err(map_durable_error)?;
        Ok(BackupStagedFile {
            scope: self.scope,
            slot,
            target,
            file: Some(file),
        })
    }

    /// Publishes one sealed stage into its reserved exact slot.
    pub fn publish(
        &self,
        staged: &mut BackupStagedFile,
    ) -> Result<BackupDirectoryEntry, BackupDirectoryError> {
        self.revalidate_directory()?;
        if staged.scope != self.scope {
            return Err(BackupDirectoryError::InvalidState);
        }
        let file = staged
            .file
            .as_mut()
            .ok_or(BackupDirectoryError::InvalidState)?;
        let receipt = file
            .publish_new(&staged.target)
            .map_err(map_durable_error)?;
        staged.file.take();
        let entry = match self.inspect_slot(staged.slot) {
            Ok(Some(entry)) => entry,
            Ok(None) | Err(_) => return Err(BackupDirectoryError::RecoveryRequired),
        };
        if entry.len != receipt.len() {
            return Err(BackupDirectoryError::RecoveryRequired);
        }
        Ok(entry)
    }

    /// Opens only the unchanged exact file bound to an opaque scanned entry.
    pub fn open_reader(
        &self,
        entry: &BackupDirectoryEntry,
        max_bytes: u64,
    ) -> Result<DurableFileReader, BackupDirectoryError> {
        self.require_current(entry)?;
        self.target(entry.ordinal)?
            .open_reader(max_bytes)
            .map_err(map_durable_error)?
            .ok_or(BackupDirectoryError::StaleEntry)
    }

    /// Deletes only the unchanged exact file bound to an opaque scanned entry.
    pub fn delete(&self, entry: &BackupDirectoryEntry) -> Result<(), BackupDirectoryError> {
        self.delete_with(entry, &mut NoopDeletionHook)
    }

    fn delete_with(
        &self,
        entry: &BackupDirectoryEntry,
        hook: &mut dyn DeletionHook,
    ) -> Result<(), BackupDirectoryError> {
        self.require_current(entry)?;
        let source = self.slot_path(entry.ordinal);
        let tombstone = self.tombstone_path(entry.ordinal);
        if self.inspect_path(&tombstone, entry.ordinal)?.is_some() {
            return Err(BackupDirectoryError::RecoveryRequired);
        }
        hook.hit(DeletionBoundary::BeforeMove)?;
        if move_file_write_through(&source, &tombstone).is_err() {
            return match (
                self.inspect_slot(entry.ordinal),
                self.inspect_path(&tombstone, entry.ordinal),
            ) {
                (Ok(Some(current)), Ok(None)) if current == *entry => {
                    Err(BackupDirectoryError::Unavailable)
                }
                _ => Err(BackupDirectoryError::RecoveryRequired),
            };
        }
        if hook.hit(DeletionBoundary::AfterMove).is_err() {
            return Err(BackupDirectoryError::RecoveryRequired);
        }
        if self.inspect_slot(entry.ordinal)?.is_some()
            || self.inspect_path(&tombstone, entry.ordinal)? != Some(entry.clone())
        {
            return Err(BackupDirectoryError::RecoveryRequired);
        }
        match std::fs::remove_file(&tombstone) {
            Ok(()) => {}
            Err(_) => {
                return Err(BackupDirectoryError::RecoveryRequired);
            }
        }
        if hook.hit(DeletionBoundary::AfterRemove).is_err()
            || self.inspect_slot(entry.ordinal)?.is_some()
            || self.inspect_path(&tombstone, entry.ordinal)?.is_some()
        {
            return Err(BackupDirectoryError::RecoveryRequired);
        }
        Ok(())
    }

    fn require_current(&self, entry: &BackupDirectoryEntry) -> Result<(), BackupDirectoryError> {
        self.revalidate_directory()?;
        if entry.scope != self.scope {
            return Err(BackupDirectoryError::StaleEntry);
        }
        match self.inspect_slot(entry.ordinal)? {
            Some(current) if current == *entry => Ok(()),
            _ => Err(BackupDirectoryError::StaleEntry),
        }
    }

    fn inspect_slot(&self, slot: u8) -> Result<Option<BackupDirectoryEntry>, BackupDirectoryError> {
        if usize::from(slot) >= MAX_BACKUP_DIRECTORY_FILES {
            return Err(BackupDirectoryError::StaleEntry);
        }
        self.inspect_path(&self.slot_path(slot), slot)
    }

    fn inspect_path(
        &self,
        path: &Path,
        slot: u8,
    ) -> Result<Option<BackupDirectoryEntry>, BackupDirectoryError> {
        let link_metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(BackupDirectoryError::Unavailable),
        };
        if link_metadata.file_type().is_symlink() || is_reparse_point(&link_metadata) {
            return Err(BackupDirectoryError::LinkedEntry);
        }
        if !link_metadata.is_file() {
            return Err(BackupDirectoryError::UnexpectedType);
        }

        let file = File::open(path).map_err(|_| BackupDirectoryError::Unavailable)?;
        let metadata = file
            .metadata()
            .map_err(|_| BackupDirectoryError::Unavailable)?;
        if !metadata.is_file() {
            return Err(BackupDirectoryError::UnexpectedType);
        }
        if has_multiple_links(&file, &metadata)? {
            return Err(BackupDirectoryError::LinkedEntry);
        }
        if metadata.len() > MAX_DURABLE_FILE_BYTES {
            return Err(BackupDirectoryError::CapacityExceeded);
        }
        let identity = PhysicalFileIdentity::from_file(&file)
            .map_err(|_| BackupDirectoryError::Unavailable)?;
        Ok(Some(BackupDirectoryEntry {
            scope: self.scope,
            ordinal: slot,
            len: metadata.len(),
            identity,
        }))
    }

    fn target(&self, slot: u8) -> Result<DurableFileTarget, BackupDirectoryError> {
        DurableFileTarget::exact_child(&self.directory, &slot_name(slot)).map_err(map_durable_error)
    }

    fn slot_path(&self, slot: u8) -> PathBuf {
        self.directory.as_path().join(slot_name(slot))
    }

    fn tombstone_path(&self, slot: u8) -> PathBuf {
        self.directory
            .as_path()
            .join(format!(".{}.tokenmaster-delete", slot_name(slot)))
    }

    fn revalidate_directory(&self) -> Result<(), BackupDirectoryError> {
        let current = ValidatedLocalDirectory::new(self.directory.as_path())
            .map_err(map_directory_open_error)?;
        if current == self.directory && hash_path(current.as_path()) == self.scope {
            Ok(())
        } else {
            Err(BackupDirectoryError::UnsupportedLocation)
        }
    }
}

impl fmt::Debug for BackupDirectory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupDirectory([redacted])")
    }
}

/// One bounded snapshot of the exact backup namespace.
pub struct BackupDirectorySnapshot {
    generation: BackupDirectoryGeneration,
    entries: Vec<BackupDirectoryEntry>,
}

impl BackupDirectorySnapshot {
    #[must_use]
    pub const fn generation(&self) -> BackupDirectoryGeneration {
        self.generation
    }

    #[must_use]
    pub fn entries(&self) -> &[BackupDirectoryEntry] {
        &self.entries
    }
}

impl fmt::Debug for BackupDirectorySnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupDirectorySnapshot")
            .field("generation", &self.generation)
            .field("entry_count", &self.entries.len())
            .finish()
    }
}

/// Opaque identity of one exact directory scan.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BackupDirectoryGeneration([u8; 32]);

impl fmt::Debug for BackupDirectoryGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupDirectoryGeneration([redacted])")
    }
}

/// Opaque path-free token for one unchanged exact package child.
#[derive(Clone, Eq, PartialEq)]
pub struct BackupDirectoryEntry {
    scope: [u8; 32],
    ordinal: u8,
    len: u64,
    identity: PhysicalFileIdentity,
}

impl BackupDirectoryEntry {
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Debug for BackupDirectoryEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupDirectoryEntry")
            .field("ordinal", &self.ordinal)
            .field("len", &self.len)
            .finish()
    }
}

/// One unpublished file reserved for an exact backup slot.
pub struct BackupStagedFile {
    scope: [u8; 32],
    slot: u8,
    target: DurableFileTarget,
    file: Option<DurableStagedFile>,
}

impl BackupStagedFile {
    #[must_use]
    pub fn written_len(&self) -> u64 {
        self.file.as_ref().map_or(0, DurableStagedFile::written_len)
    }

    pub fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), BackupDirectoryError> {
        self.file
            .as_mut()
            .ok_or(BackupDirectoryError::InvalidState)?
            .write_chunk(bytes)
            .map_err(map_durable_error)
    }

    pub fn seal(
        &mut self,
        expected_len: u64,
        expected_sha256: [u8; 32],
    ) -> Result<(), BackupDirectoryError> {
        self.file
            .as_mut()
            .ok_or(BackupDirectoryError::InvalidState)?
            .seal(expected_len, expected_sha256)
            .map(|_| ())
            .map_err(map_durable_error)
    }

    /// Opens a path-free reader only after the exact unpublished stage is sealed.
    pub fn open_reader(&self) -> Result<DurableFileReader, BackupDirectoryError> {
        self.file
            .as_ref()
            .ok_or(BackupDirectoryError::InvalidState)?
            .open_sealed_reader()
            .map_err(map_durable_error)
    }

    pub fn discard(&mut self) -> Result<(), BackupDirectoryError> {
        self.file
            .as_mut()
            .ok_or(BackupDirectoryError::InvalidState)?
            .discard()
            .map_err(map_durable_error)
    }
}

impl fmt::Debug for BackupStagedFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupStagedFile([redacted])")
    }
}

/// Stable path- and OS-message-private backup directory failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BackupDirectoryError {
    #[error("backup directory location is unsupported")]
    UnsupportedLocation,
    #[error("backup directory contains an unexpected entry")]
    UnexpectedEntry,
    #[error("backup directory entry has an unexpected type")]
    UnexpectedType,
    #[error("backup directory entry is linked")]
    LinkedEntry,
    #[error("backup directory contains an ambiguous file identity")]
    AmbiguousIdentity,
    #[error("backup directory entry token is stale")]
    StaleEntry,
    #[error("backup directory operation state is invalid")]
    InvalidState,
    #[error("backup directory capacity limit was exceeded")]
    CapacityExceeded,
    #[error("backup directory operation is unavailable")]
    Unavailable,
    #[error("backup directory operation requires recovery")]
    RecoveryRequired,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DeletionBoundary {
    BeforeMove,
    AfterMove,
    AfterRemove,
}

trait DeletionHook {
    fn hit(&mut self, boundary: DeletionBoundary) -> Result<(), BackupDirectoryError>;
}

struct NoopDeletionHook;

impl DeletionHook for NoopDeletionHook {
    fn hit(&mut self, _boundary: DeletionBoundary) -> Result<(), BackupDirectoryError> {
        Ok(())
    }
}

fn validate_directory_metadata(metadata: &std::fs::Metadata) -> Result<(), BackupDirectoryError> {
    if metadata.file_type().is_symlink() || is_reparse_point(metadata) {
        return Err(BackupDirectoryError::UnsupportedLocation);
    }
    if !metadata.is_dir() {
        return Err(BackupDirectoryError::UnexpectedType);
    }
    Ok(())
}

fn slot_name(slot: u8) -> String {
    format!("{BACKUP_FILE_PREFIX}{slot:02}{BACKUP_FILE_SUFFIX}")
}

fn parse_slot_name(name: &str) -> Option<u8> {
    if name.len() != BACKUP_FILE_PREFIX.len() + 2 + BACKUP_FILE_SUFFIX.len()
        || !name.starts_with(BACKUP_FILE_PREFIX)
        || !name.ends_with(BACKUP_FILE_SUFFIX)
    {
        return None;
    }
    let digits = &name[BACKUP_FILE_PREFIX.len()..BACKUP_FILE_PREFIX.len() + 2];
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let slot = digits.parse::<u8>().ok()?;
    (usize::from(slot) < MAX_BACKUP_DIRECTORY_FILES).then_some(slot)
}

fn is_controlled_recovery_name(name: &str) -> bool {
    let Some(inner) = name.strip_prefix('.') else {
        return false;
    };
    if let Some(point) = inner.strip_suffix(".tokenmaster-delete") {
        return parse_slot_name(point).is_some();
    }
    let Some((point, attempt)) = inner.split_once(".tokenmaster-stage-") else {
        return false;
    };
    parse_slot_name(point).is_some()
        && attempt.len() == 2
        && attempt.bytes().all(|byte| byte.is_ascii_digit())
        && attempt
            .parse::<usize>()
            .is_ok_and(|value| value < crate::DURABLE_STAGE_ATTEMPTS)
}

fn hash_entries(scope: [u8; 32], entries: &[BackupDirectoryEntry]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"tm-backup-directory-generation-v1");
    hasher.update(scope);
    for entry in entries {
        hasher.update([entry.ordinal]);
        hasher.update(entry.len.to_le_bytes());
        hasher.update(entry.identity.as_bytes());
    }
    hasher.finalize().into()
}

#[cfg(windows)]
fn hash_path(path: &Path) -> [u8; 32] {
    use std::os::windows::ffi::OsStrExt;

    let mut hasher = Sha256::new();
    hasher.update(b"tm-backup-directory-scope-v1-windows");
    for unit in path.as_os_str().encode_wide() {
        hasher.update(unit.to_le_bytes());
    }
    hasher.finalize().into()
}

#[cfg(unix)]
fn hash_path(path: &Path) -> [u8; 32] {
    use std::os::unix::ffi::OsStrExt;

    let mut hasher = Sha256::new();
    hasher.update(b"tm-backup-directory-scope-v1-unix");
    hasher.update(path.as_os_str().as_bytes());
    hasher.finalize().into()
}

#[cfg(not(any(unix, windows)))]
fn hash_path(_path: &Path) -> [u8; 32] {
    [0_u8; 32]
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

#[cfg(windows)]
fn has_multiple_links(
    file: &File,
    _metadata: &std::fs::Metadata,
) -> Result<bool, BackupDirectoryError> {
    crate::windows::platform_link_count(file)
        .map(|count| count > 1)
        .map_err(|_| BackupDirectoryError::Unavailable)
}

#[cfg(unix)]
fn has_multiple_links(
    _file: &File,
    metadata: &std::fs::Metadata,
) -> Result<bool, BackupDirectoryError> {
    use std::os::unix::fs::MetadataExt;

    Ok(metadata.nlink() > 1)
}

#[cfg(not(any(unix, windows)))]
fn has_multiple_links(
    _file: &File,
    _metadata: &std::fs::Metadata,
) -> Result<bool, BackupDirectoryError> {
    Ok(true)
}

fn map_directory_open_error(error: crate::LocalDirectoryError) -> BackupDirectoryError {
    match error {
        crate::LocalDirectoryError::InvalidPath
        | crate::LocalDirectoryError::UnsupportedLocation => {
            BackupDirectoryError::UnsupportedLocation
        }
        crate::LocalDirectoryError::Unavailable => BackupDirectoryError::Unavailable,
    }
}

const fn map_durable_error(error: DurableFileError) -> BackupDirectoryError {
    match error {
        DurableFileError::UnsupportedLocation | DurableFileError::InvalidName => {
            BackupDirectoryError::UnsupportedLocation
        }
        DurableFileError::UnexpectedType => BackupDirectoryError::UnexpectedType,
        DurableFileError::CapacityExceeded | DurableFileError::CollisionLimit => {
            BackupDirectoryError::CapacityExceeded
        }
        DurableFileError::TargetMissing | DurableFileError::TargetExists => {
            BackupDirectoryError::StaleEntry
        }
        DurableFileError::InvalidState => BackupDirectoryError::InvalidState,
        DurableFileError::RecoveryRequired => BackupDirectoryError::RecoveryRequired,
        DurableFileError::Integrity | DurableFileError::Unavailable => {
            BackupDirectoryError::Unavailable
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::{BackupDirectory, BackupDirectoryError, DeletionBoundary, DeletionHook};
    use crate::{MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory};

    struct FailAt(DeletionBoundary);

    impl DeletionHook for FailAt {
        fn hit(&mut self, boundary: DeletionBoundary) -> Result<(), BackupDirectoryError> {
            if self.0 == boundary {
                Err(BackupDirectoryError::Unavailable)
            } else {
                Ok(())
            }
        }
    }

    fn directory() -> (TempDir, BackupDirectory) {
        let root = TempDir::new().expect("root");
        let validated = ValidatedLocalDirectory::new(root.path()).expect("validated root");
        let directory = BackupDirectory::open_or_create(&validated).expect("backup directory");
        (root, directory)
    }

    fn publish(directory: &BackupDirectory, bytes: &[u8]) -> super::BackupDirectoryEntry {
        let mut stage = directory
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .expect("stage");
        stage.write_chunk(bytes).expect("write");
        stage
            .seal(bytes.len() as u64, Sha256::digest(bytes).into())
            .expect("seal");
        directory.publish(&mut stage).expect("publish")
    }

    #[test]
    fn every_deletion_boundary_leaves_a_deterministic_next_scan() {
        let (_root, before_directory) = directory();
        let before = publish(&before_directory, b"before");
        assert_eq!(
            before_directory
                .delete_with(&before, &mut FailAt(DeletionBoundary::BeforeMove))
                .expect_err("before move injection"),
            BackupDirectoryError::Unavailable
        );
        assert_eq!(
            before_directory.scan().expect("before scan").entries(),
            &[before]
        );

        let (_root, moved_directory) = directory();
        let moved = publish(&moved_directory, b"moved");
        assert_eq!(
            moved_directory
                .delete_with(&moved, &mut FailAt(DeletionBoundary::AfterMove))
                .expect_err("after move injection"),
            BackupDirectoryError::RecoveryRequired
        );
        assert_eq!(
            moved_directory
                .scan()
                .expect_err("tombstone requires recovery"),
            BackupDirectoryError::RecoveryRequired
        );

        let (_root, removed_directory) = directory();
        let removed = publish(&removed_directory, b"removed");
        assert_eq!(
            removed_directory
                .delete_with(&removed, &mut FailAt(DeletionBoundary::AfterRemove))
                .expect_err("after remove injection"),
            BackupDirectoryError::RecoveryRequired
        );
        assert!(
            removed_directory
                .scan()
                .expect("removed scan")
                .entries()
                .is_empty()
        );
    }
}
