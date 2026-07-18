use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{LocalDirectoryError, PhysicalFileIdentity, ValidatedLocalDirectory};

#[cfg(unix)]
use crate::unix::{
    create_stage_file, directory_identity, move_file_write_through,
    open_regular_for_delete_no_follow, open_regular_no_follow,
    replace_file_redundant_write_through, replace_file_write_through,
};
#[cfg(not(any(unix, windows)))]
use crate::unsupported::{
    create_stage_file, directory_identity, move_file_write_through,
    open_regular_for_delete_no_follow, open_regular_no_follow,
    replace_file_redundant_write_through, replace_file_write_through,
};
#[cfg(windows)]
use crate::windows::{
    create_stage_file, delete_stage_by_handle, directory_identity, move_file_write_through,
    open_regular_for_delete_no_follow, open_regular_no_follow,
    replace_file_redundant_write_through, replace_file_write_through,
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
    parent_identity: Option<PhysicalFileIdentity>,
    stage_stem: String,
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
            parent_identity: None,
            stage_stem: child_name.to_owned(),
            path,
        })
    }

    pub(crate) fn selected_child(
        parent: &ValidatedLocalDirectory,
        child_name: &str,
    ) -> Result<Self, DurableFileError> {
        validate_selected_child_name(child_name)?;
        revalidate_parent(parent.as_path())?;
        let parent_identity = directory_identity(parent.as_path())?;
        Ok(Self {
            parent: parent.as_path().to_path_buf(),
            parent_identity: Some(parent_identity),
            stage_stem: selected_stage_stem(child_name),
            path: parent.as_path().join(child_name),
        })
    }

    /// Creates one collision-safe, create-new staging file beside this target.
    pub fn create_staged(&self, max_bytes: u64) -> Result<DurableStagedFile, DurableFileError> {
        if max_bytes > MAX_DURABLE_FILE_BYTES {
            return Err(DurableFileError::CapacityExceeded);
        }
        self.revalidate_parent()?;
        reject_unexpected_existing_type(&self.path, true)?;

        for attempt in 0..DURABLE_STAGE_ATTEMPTS {
            let stage_name = format!(".{}.tokenmaster-stage-{attempt:02}", self.stage_stem);
            let stage_path = self.parent.join(stage_name);
            match create_stage_file(&stage_path) {
                Ok(file) => {
                    let metadata = match file.metadata() {
                        Ok(metadata) => metadata,
                        Err(_) => {
                            discard_stage_handle(file, &stage_path);
                            return Err(DurableFileError::Unavailable);
                        }
                    };
                    if !metadata.is_file() {
                        discard_stage_handle(file, &stage_path);
                        return Err(DurableFileError::UnexpectedType);
                    }
                    if let Err(error) = ensure_single_link(&file) {
                        discard_stage_handle(file, &stage_path);
                        return Err(error);
                    }
                    let identity = match PhysicalFileIdentity::from_file(&file) {
                        Ok(identity) => identity,
                        Err(_) => {
                            discard_stage_handle(file, &stage_path);
                            return Err(DurableFileError::Unavailable);
                        }
                    };
                    let cleanup_file = match file.try_clone() {
                        Ok(cleanup_file) => cleanup_file,
                        Err(_) => {
                            discard_stage_handle(file, &stage_path);
                            return Err(DurableFileError::Unavailable);
                        }
                    };
                    if let Err(error) = self.revalidate_parent() {
                        discard_stage_handle(cleanup_file, &stage_path);
                        drop(file);
                        return Err(error);
                    }
                    return Ok(DurableStagedFile {
                        path: Some(stage_path),
                        file: Some(file),
                        cleanup_file: Some(cleanup_file),
                        identity,
                        parent: self.parent.clone(),
                        parent_identity: self.parent_identity,
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

    /// Opens one exact regular child for bounded, path-free streaming.
    pub fn open_reader(
        &self,
        max_bytes: u64,
    ) -> Result<Option<DurableFileReader>, DurableFileError> {
        if max_bytes > MAX_DURABLE_FILE_BYTES {
            return Err(DurableFileError::CapacityExceeded);
        }
        revalidate_parent(&self.parent)?;
        match existing_kind(&self.path)? {
            ExistingKind::Missing => return Ok(None),
            ExistingKind::Regular => {}
        }

        let file = File::open(&self.path).map_err(|_| DurableFileError::Unavailable)?;
        let metadata = file.metadata().map_err(|_| DurableFileError::Unavailable)?;
        if !metadata.is_file() {
            return Err(DurableFileError::UnexpectedType);
        }
        let expected_len = metadata.len();
        if expected_len > max_bytes {
            return Err(DurableFileError::CapacityExceeded);
        }
        Ok(Some(DurableFileReader {
            file,
            expected_len,
            consumed: 0,
            finished: false,
            read_failed: false,
        }))
    }

    pub(crate) fn exact_path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn open_selected_reader(
        &self,
        max_bytes: u64,
    ) -> Result<DurableFileReader, DurableFileError> {
        if max_bytes > MAX_DURABLE_FILE_BYTES {
            return Err(DurableFileError::CapacityExceeded);
        }
        self.revalidate_parent()?;
        if existing_kind(&self.path)? != ExistingKind::Regular {
            return Err(DurableFileError::TargetMissing);
        }
        let file = open_regular_no_follow(&self.path)?;
        self.revalidate_parent()?;
        let metadata = file.metadata().map_err(|_| DurableFileError::Unavailable)?;
        if !metadata.is_file() {
            return Err(DurableFileError::UnexpectedType);
        }
        ensure_single_link(&file)?;
        let expected_len = metadata.len();
        if expected_len > max_bytes {
            return Err(DurableFileError::CapacityExceeded);
        }
        Ok(DurableFileReader {
            file,
            expected_len,
            consumed: 0,
            finished: false,
            read_failed: false,
        })
    }

    pub(crate) fn selected_identity(
        &self,
    ) -> Result<Option<PhysicalFileIdentity>, DurableFileError> {
        self.selected_identity_at(&self.path)
    }

    fn selected_identity_at(
        &self,
        path: &Path,
    ) -> Result<Option<PhysicalFileIdentity>, DurableFileError> {
        self.revalidate_parent()?;
        match existing_kind(path)? {
            ExistingKind::Missing => Ok(None),
            ExistingKind::Regular => {
                let file = open_regular_no_follow(path)?;
                self.revalidate_parent()?;
                let metadata = file.metadata().map_err(|_| DurableFileError::Unavailable)?;
                if !metadata.is_file() || metadata.len() > MAX_DURABLE_FILE_BYTES {
                    return Err(DurableFileError::CapacityExceeded);
                }
                ensure_single_link(&file)?;
                PhysicalFileIdentity::from_file(&file)
                    .map(Some)
                    .map_err(|_| DurableFileError::Unavailable)
            }
        }
    }

    fn revalidate_parent(&self) -> Result<(), DurableFileError> {
        revalidate_bound_parent(&self.parent, self.parent_identity)
    }
}

impl fmt::Debug for DurableFileTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurableFileTarget([redacted])")
    }
}

/// Bounded path-free reader for one already-open exact durable file.
pub struct DurableFileReader {
    file: File,
    expected_len: u64,
    consumed: u64,
    finished: bool,
    read_failed: bool,
}

impl DurableFileReader {
    /// Reads one bounded chunk and verifies that the open file ends at its observed length.
    pub fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<usize, DurableFileError> {
        if self.read_failed {
            return Err(DurableFileError::InvalidState);
        }
        if buffer.len() > MAX_DURABLE_WRITE_CHUNK_BYTES {
            return Err(DurableFileError::CapacityExceeded);
        }
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.finished {
            return Ok(0);
        }

        let remaining = self
            .expected_len
            .checked_sub(self.consumed)
            .ok_or(DurableFileError::Integrity)?;
        if remaining == 0 {
            let mut trailing = [0_u8; 1];
            return match self.file.read(&mut trailing) {
                Ok(0) => {
                    self.finished = true;
                    Ok(0)
                }
                Ok(_) => {
                    self.read_failed = true;
                    Err(DurableFileError::Integrity)
                }
                Err(_) => {
                    self.read_failed = true;
                    Err(DurableFileError::Unavailable)
                }
            };
        }

        let request = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| DurableFileError::CapacityExceeded)?;
        match self.file.read(&mut buffer[..request]) {
            Ok(0) => {
                self.read_failed = true;
                Err(DurableFileError::Integrity)
            }
            Ok(count) => {
                self.consumed = self
                    .consumed
                    .checked_add(u64::try_from(count).map_err(|_| DurableFileError::Integrity)?)
                    .ok_or(DurableFileError::Integrity)?;
                Ok(count)
            }
            Err(_) => {
                self.read_failed = true;
                Err(DurableFileError::Unavailable)
            }
        }
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.expected_len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.expected_len == 0
    }
}

impl fmt::Debug for DurableFileReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurableFileReader([redacted])")
    }
}

/// Create-new staging file whose path and file handle never leave the platform package.
pub struct DurableStagedFile {
    path: Option<PathBuf>,
    file: Option<File>,
    cleanup_file: Option<File>,
    identity: PhysicalFileIdentity,
    parent: PathBuf,
    parent_identity: Option<PhysicalFileIdentity>,
    receipt: Option<DurableFileReceipt>,
    max_bytes: u64,
    written: u64,
    write_failed: bool,
    preserve_on_drop: bool,
}

impl DurableStagedFile {
    /// Returns the number of bytes accepted by this unpublished stage.
    #[must_use]
    pub const fn written_len(&self) -> u64 {
        self.written
    }

    /// Irreversibly invalidates this stage and removes its unpublished file when possible.
    pub fn discard(&mut self) -> Result<(), DurableFileError> {
        self.write_failed = true;
        self.receipt = None;
        self.file.take();
        if self.path.is_none() {
            return Ok(());
        }
        self.delete_owned_stage()
    }

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

    pub(crate) fn open_sealed_reader(&self) -> Result<DurableFileReader, DurableFileError> {
        if self.file.is_some() || self.write_failed {
            return Err(DurableFileError::InvalidState);
        }
        let receipt = self.receipt.ok_or(DurableFileError::InvalidState)?;
        let path = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
        verify_file(path, receipt.len, receipt.sha256)?;
        let file = File::open(path).map_err(|_| DurableFileError::Unavailable)?;
        let metadata = file.metadata().map_err(|_| DurableFileError::Unavailable)?;
        if !metadata.is_file() || metadata.len() != receipt.len {
            return Err(DurableFileError::Integrity);
        }
        Ok(DurableFileReader {
            file,
            expected_len: receipt.len,
            consumed: 0,
            finished: false,
            read_failed: false,
        })
    }

    pub(crate) const fn sealed_receipt(&self) -> Option<DurableFileReceipt> {
        self.receipt
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

    pub(crate) fn replace_selected(
        &mut self,
        target: &DurableFileTarget,
        expected: PhysicalFileIdentity,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        self.replace_selected_with(target, expected, &mut NoopPublicationHook)
    }

    fn replace_selected_with(
        &mut self,
        target: &DurableFileTarget,
        expected: PhysicalFileIdentity,
        hook: &mut impl PublicationHook,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        let receipt = self.receipt.ok_or(DurableFileError::InvalidState)?;
        let source = self
            .path
            .as_deref()
            .ok_or(DurableFileError::InvalidState)?
            .to_path_buf();
        if source == target.path {
            return Err(DurableFileError::InvalidState);
        }
        target.revalidate_parent()?;
        verify_file(&source, receipt.len, receipt.sha256)?;
        if target.selected_identity()? != Some(expected) {
            return Err(DurableFileError::TargetExists);
        }
        let displaced = reserve_displaced_path(target)?;
        hook.hit(PublicationBoundary::BeforeReplace)?;
        self.cleanup_file.take();
        if let Err(error) = replace_file_write_through(&target.path, &source, &displaced) {
            self.reacquire_cleanup_or_preserve();
            self.preserve_recovery_artifacts(error);
            return Err(error);
        }
        if hook.hit(PublicationBoundary::AfterSelectedReplace).is_err() {
            self.preserve_on_drop = true;
            return Err(DurableFileError::RecoveryRequired);
        }

        let displaced_identity = match target.selected_identity_at(&displaced) {
            Ok(Some(identity)) => identity,
            _ => {
                self.preserve_on_drop = true;
                return Err(DurableFileError::RecoveryRequired);
            }
        };
        if displaced_identity != expected {
            let published_identity = match target.selected_identity() {
                Ok(identity) => identity,
                Err(_) => {
                    self.preserve_on_drop = true;
                    return Err(DurableFileError::RecoveryRequired);
                }
            };
            if published_identity != Some(self.identity) {
                self.preserve_on_drop = true;
                return Err(DurableFileError::RecoveryRequired);
            }
            if replace_file_write_through(&target.path, &displaced, &source).is_err() {
                self.preserve_on_drop = true;
                return Err(DurableFileError::RecoveryRequired);
            }
            if hook
                .hit(PublicationBoundary::AfterSelectedRollback)
                .is_err()
            {
                self.preserve_on_drop = true;
                return Err(DurableFileError::RecoveryRequired);
            }
            let restored_identity = target.selected_identity();
            let candidate_identity = target.selected_identity_at(&source);
            if !matches!(restored_identity, Ok(Some(identity)) if identity == displaced_identity)
                || !matches!(candidate_identity, Ok(Some(identity)) if identity == self.identity)
            {
                self.preserve_on_drop = true;
                return Err(DurableFileError::RecoveryRequired);
            }
            self.reacquire_cleanup_or_preserve();
            if self.preserve_on_drop {
                return Err(DurableFileError::RecoveryRequired);
            }
            return Err(DurableFileError::TargetExists);
        }

        self.path = None;
        self.cleanup_file.take();
        hook.hit(PublicationBoundary::AfterReplace)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        sync_existing_file(&target.path).map_err(|_| DurableFileError::RecoveryRequired)?;
        let published = verify_file(&target.path, receipt.len, receipt.sha256)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        if !matches!(target.selected_identity(), Ok(Some(identity)) if identity == self.identity) {
            self.preserve_on_drop = true;
            return Err(DurableFileError::RecoveryRequired);
        }
        delete_selected_child(target, &displaced, expected)
            .map_err(|_| DurableFileError::RecoveryRequired)?;
        Ok(published)
    }

    fn publish_new_with(
        &mut self,
        target: &DurableFileTarget,
        hook: &mut impl PublicationHook,
    ) -> Result<DurableFileReceipt, DurableFileError> {
        let receipt = self.receipt.ok_or(DurableFileError::InvalidState)?;
        let source = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
        target.revalidate_parent()?;
        verify_file(source, receipt.len, receipt.sha256)?;
        match existing_kind(&target.path)? {
            ExistingKind::Missing => {}
            ExistingKind::Regular => return Err(DurableFileError::TargetExists),
        }
        hook.hit(PublicationBoundary::BeforeMove)?;
        self.cleanup_file.take();
        if let Err(error) = move_file_write_through(source, &target.path) {
            self.reacquire_cleanup_or_preserve();
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
        target.revalidate_parent()?;
        verify_file(source, receipt.len, receipt.sha256)?;
        if existing_kind(&target.path)? == ExistingKind::Missing {
            return Err(DurableFileError::TargetMissing);
        }
        hook.hit(PublicationBoundary::BeforeReplace)?;
        self.cleanup_file.take();
        if let Err(error) = replace_file_redundant_write_through(source, &target.path) {
            self.reacquire_cleanup_or_preserve();
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
        target.revalidate_parent()?;
        backup.revalidate_parent()?;
        verify_file(source, receipt.len, receipt.sha256)?;
        if existing_kind(&target.path)? == ExistingKind::Missing {
            return Err(DurableFileError::TargetMissing);
        }
        let replaced_receipt = inspect_file(&target.path)?;
        if existing_kind(&backup.path)? == ExistingKind::Regular {
            return Err(DurableFileError::TargetExists);
        }
        hook.hit(PublicationBoundary::BeforeReplace)?;
        self.cleanup_file.take();
        if let Err(error) = operation(&target.path, source, &backup.path) {
            self.reacquire_cleanup_or_preserve();
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

    fn reacquire_cleanup_or_preserve(&mut self) {
        let result = (|| {
            revalidate_bound_parent(&self.parent, self.parent_identity)?;
            let path = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
            let file = open_regular_for_delete_no_follow(path)?;
            revalidate_bound_parent(&self.parent, self.parent_identity)?;
            ensure_single_link(&file)?;
            let identity = PhysicalFileIdentity::from_file(&file)
                .map_err(|_| DurableFileError::Unavailable)?;
            if identity != self.identity {
                return Err(DurableFileError::Integrity);
            }
            self.cleanup_file = Some(file);
            Ok(())
        })();
        if result.is_err() {
            self.preserve_on_drop = true;
        }
    }

    fn delete_owned_stage(&mut self) -> Result<(), DurableFileError> {
        let cleanup = self
            .cleanup_file
            .as_ref()
            .ok_or(DurableFileError::InvalidState)?;
        let current =
            PhysicalFileIdentity::from_file(cleanup).map_err(|_| DurableFileError::Unavailable)?;
        if current != self.identity {
            return Err(DurableFileError::Integrity);
        }
        ensure_single_link(cleanup)?;

        #[cfg(windows)]
        delete_stage_by_handle(cleanup)?;

        #[cfg(unix)]
        {
            revalidate_bound_parent(&self.parent, self.parent_identity)?;
            let path = self.path.as_deref().ok_or(DurableFileError::InvalidState)?;
            let opened = open_regular_no_follow(path)?;
            let opened_identity = PhysicalFileIdentity::from_file(&opened)
                .map_err(|_| DurableFileError::Unavailable)?;
            if opened_identity != self.identity {
                return Err(DurableFileError::Integrity);
            }
            std::fs::remove_file(path).map_err(|_| DurableFileError::Unavailable)?;
        }

        #[cfg(not(any(unix, windows)))]
        return Err(DurableFileError::UnsupportedLocation);

        self.cleanup_file.take();
        self.path.take();
        Ok(())
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
        if self.path.is_some() {
            self.file.take();
            let _ = self.delete_owned_stage();
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
    pub(crate) const fn from_expected(len: u64, sha256: [u8; 32]) -> Self {
        Self { len, sha256 }
    }

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
    AfterSelectedReplace,
    AfterSelectedRollback,
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

fn validate_selected_child_name(child_name: &str) -> Result<(), DurableFileError> {
    const MAX_SELECTED_CHILD_BYTES: usize = 1024;
    const MAX_SELECTED_CHILD_UTF16_UNITS: usize = 255;

    let bytes = child_name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_SELECTED_CHILD_BYTES
        || child_name.encode_utf16().count() > MAX_SELECTED_CHILD_UTF16_UNITS
        || matches!(child_name, "." | "..")
        || child_name.ends_with(['.', ' '])
        || child_name.chars().any(|character| {
            character <= '\u{1f}'
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
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

fn selected_stage_stem(child_name: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut hasher = Sha256::new();
    hasher.update(b"tokenmaster-selected-stage-v1");
    hasher.update(child_name.as_bytes());
    let digest = hasher.finalize();
    let mut stem = String::with_capacity(25);
    stem.push_str("selected-");
    for byte in digest.iter().take(8) {
        stem.push(char::from(HEX[usize::from(byte >> 4)]));
        stem.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    stem
}

fn ensure_single_link(file: &File) -> Result<(), DurableFileError> {
    #[cfg(windows)]
    let link_count =
        crate::windows::platform_link_count(file).map_err(|_| DurableFileError::Unavailable)?;
    #[cfg(unix)]
    let link_count = {
        use std::os::unix::fs::MetadataExt;

        file.metadata()
            .map_err(|_| DurableFileError::Unavailable)?
            .nlink()
    };
    #[cfg(not(any(unix, windows)))]
    return Err(DurableFileError::UnsupportedLocation);

    if link_count == 1 {
        Ok(())
    } else {
        Err(DurableFileError::UnsupportedLocation)
    }
}

fn revalidate_parent(parent: &Path) -> Result<(), DurableFileError> {
    let validated = ValidatedLocalDirectory::new(parent).map_err(map_directory_error)?;
    if validated.as_path() == parent {
        Ok(())
    } else {
        Err(DurableFileError::UnsupportedLocation)
    }
}

fn revalidate_bound_parent(
    parent: &Path,
    expected: Option<PhysicalFileIdentity>,
) -> Result<(), DurableFileError> {
    revalidate_parent(parent)?;
    if let Some(expected) = expected
        && directory_identity(parent)? != expected
    {
        return Err(DurableFileError::UnsupportedLocation);
    }
    Ok(())
}

fn discard_stage_handle(file: File, _path: &Path) {
    #[cfg(windows)]
    {
        let _ = delete_stage_by_handle(&file);
    }
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(_path);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = _path;
    }
    drop(file);
}

fn reserve_displaced_path(target: &DurableFileTarget) -> Result<PathBuf, DurableFileError> {
    target.revalidate_parent()?;
    for attempt in 0..DURABLE_STAGE_ATTEMPTS {
        let name = format!(".{}.tokenmaster-displaced-{attempt:02}", target.stage_stem);
        let path = target.parent.join(name);
        if existing_kind(&path)? == ExistingKind::Missing {
            return Ok(path);
        }
    }
    Err(DurableFileError::CollisionLimit)
}

fn delete_selected_child(
    target: &DurableFileTarget,
    path: &Path,
    expected: PhysicalFileIdentity,
) -> Result<(), DurableFileError> {
    target.revalidate_parent()?;
    let file = open_regular_for_delete_no_follow(path)?;
    target.revalidate_parent()?;
    ensure_single_link(&file)?;
    let current =
        PhysicalFileIdentity::from_file(&file).map_err(|_| DurableFileError::Unavailable)?;
    if current != expected {
        return Err(DurableFileError::TargetExists);
    }

    #[cfg(windows)]
    delete_stage_by_handle(&file)?;

    #[cfg(unix)]
    std::fs::remove_file(path).map_err(|_| DurableFileError::Unavailable)?;

    #[cfg(not(any(unix, windows)))]
    return Err(DurableFileError::UnsupportedLocation);

    Ok(())
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

pub(crate) fn inspect_file(path: &Path) -> Result<DurableFileReceipt, DurableFileError> {
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

    struct ReplaceTargetAtBoundary {
        boundary: PublicationBoundary,
        target: std::path::PathBuf,
        replacement: &'static [u8],
    }

    #[cfg(windows)]
    struct MoveParentAtBoundary {
        boundary: PublicationBoundary,
        parent: std::path::PathBuf,
        moved: std::path::PathBuf,
    }

    #[cfg(windows)]
    struct RaceThenMoveParent {
        target: std::path::PathBuf,
        replacement: &'static [u8],
        parent: std::path::PathBuf,
        moved: std::path::PathBuf,
    }

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

    impl PublicationHook for ReplaceTargetAtBoundary {
        fn hit(&mut self, boundary: PublicationBoundary) -> Result<(), DurableFileError> {
            if boundary == self.boundary {
                std::fs::remove_file(&self.target).expect("remove selected identity");
                std::fs::write(&self.target, self.replacement).expect("race replacement");
            }
            Ok(())
        }
    }

    #[cfg(windows)]
    impl PublicationHook for MoveParentAtBoundary {
        fn hit(&mut self, boundary: PublicationBoundary) -> Result<(), DurableFileError> {
            if boundary == self.boundary {
                std::fs::rename(&self.parent, &self.moved).expect("move selected parent");
                std::fs::create_dir(&self.parent).expect("replacement parent");
            }
            Ok(())
        }
    }

    #[cfg(windows)]
    impl PublicationHook for RaceThenMoveParent {
        fn hit(&mut self, boundary: PublicationBoundary) -> Result<(), DurableFileError> {
            if boundary == PublicationBoundary::BeforeReplace {
                std::fs::remove_file(&self.target).expect("remove selected identity");
                std::fs::write(&self.target, self.replacement).expect("race replacement");
            } else if boundary == PublicationBoundary::AfterSelectedRollback {
                std::fs::rename(&self.parent, &self.moved).expect("move selected parent");
                std::fs::create_dir(&self.parent).expect("replacement parent");
            }
            Ok(())
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
    fn selected_replace_rolls_back_a_target_raced_after_identity_validation() {
        let root = TempDir::new().expect("root");
        let directory = ValidatedLocalDirectory::new(root.path()).expect("directory");
        let target =
            DurableFileTarget::selected_child(&directory, "export.tmconfig").expect("target");
        std::fs::write(&target.path, b"selected").expect("selected target");
        let expected = target
            .selected_identity()
            .expect("identity query")
            .expect("selected identity");
        let mut staged = staged(&target, b"new");
        let mut hook = ReplaceTargetAtBoundary {
            boundary: PublicationBoundary::BeforeReplace,
            target: target.path.clone(),
            replacement: b"concurrent",
        };

        assert_eq!(
            staged
                .replace_selected_with(&target, expected, &mut hook)
                .expect_err("selection race"),
            DurableFileError::TargetExists
        );
        assert_eq!(
            std::fs::read(&target.path).expect("concurrent retained"),
            b"concurrent"
        );
        assert!(staged.path.is_some(), "sealed candidate remains retryable");
    }

    #[test]
    fn selected_replace_preserves_displaced_bytes_until_new_bytes_are_verified() {
        let root = TempDir::new().expect("root");
        let directory = ValidatedLocalDirectory::new(root.path()).expect("directory");
        let target =
            DurableFileTarget::selected_child(&directory, "export.tmconfig").expect("target");
        std::fs::write(&target.path, b"selected").expect("selected target");
        let expected = target
            .selected_identity()
            .expect("identity query")
            .expect("selected identity");
        let mut staged = staged(&target, b"new");

        assert_eq!(
            staged
                .replace_selected_with(
                    &target,
                    expected,
                    &mut FailAt(PublicationBoundary::AfterReplace),
                )
                .expect_err("post-replace fault"),
            DurableFileError::RecoveryRequired
        );
        assert_eq!(std::fs::read(&target.path).expect("new target"), b"new");
        let displaced = std::fs::read_dir(root.path())
            .expect("directory")
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains("displaced"))
            .expect("displaced recovery artifact");
        assert_eq!(
            std::fs::read(displaced.path()).expect("old bytes retained"),
            b"selected"
        );
    }

    #[cfg(windows)]
    #[test]
    fn post_replace_identity_query_failure_requires_recovery_and_preserves_both_files() {
        let container = TempDir::new().expect("container");
        let selected = container.path().join("selected");
        let moved = container.path().join("moved");
        std::fs::create_dir(&selected).expect("selected directory");
        let directory = ValidatedLocalDirectory::new(&selected).expect("directory");
        let target =
            DurableFileTarget::selected_child(&directory, "export.tmconfig").expect("target");
        std::fs::write(&target.path, b"selected").expect("selected target");
        let expected = target
            .selected_identity()
            .expect("identity query")
            .expect("selected identity");
        let mut staged = staged(&target, b"new");
        let mut hook = MoveParentAtBoundary {
            boundary: PublicationBoundary::AfterSelectedReplace,
            parent: selected.clone(),
            moved: moved.clone(),
        };

        assert_eq!(
            staged
                .replace_selected_with(&target, expected, &mut hook)
                .expect_err("post-mutation identity ambiguity"),
            DurableFileError::RecoveryRequired
        );
        assert_eq!(
            std::fs::read(moved.join("export.tmconfig")).expect("new retained"),
            b"new"
        );
        let displaced = std::fs::read_dir(&moved)
            .expect("moved directory")
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains("displaced"))
            .expect("old recovery artifact");
        assert_eq!(
            std::fs::read(displaced.path()).expect("old retained"),
            b"selected"
        );
        assert_eq!(
            std::fs::read_dir(selected)
                .expect("replacement namespace")
                .count(),
            0
        );
    }

    #[cfg(windows)]
    #[test]
    fn post_rollback_identity_query_failure_requires_recovery_and_preserves_both_files() {
        let container = TempDir::new().expect("container");
        let selected = container.path().join("selected");
        let moved = container.path().join("moved");
        std::fs::create_dir(&selected).expect("selected directory");
        let directory = ValidatedLocalDirectory::new(&selected).expect("directory");
        let target =
            DurableFileTarget::selected_child(&directory, "export.tmconfig").expect("target");
        std::fs::write(&target.path, b"selected").expect("selected target");
        let expected = target
            .selected_identity()
            .expect("identity query")
            .expect("selected identity");
        let mut staged = staged(&target, b"new");
        let mut hook = RaceThenMoveParent {
            target: target.path.clone(),
            replacement: b"concurrent",
            parent: selected.clone(),
            moved: moved.clone(),
        };

        assert_eq!(
            staged
                .replace_selected_with(&target, expected, &mut hook)
                .expect_err("post-rollback identity ambiguity"),
            DurableFileError::RecoveryRequired
        );
        assert_eq!(
            std::fs::read(moved.join("export.tmconfig")).expect("concurrent retained"),
            b"concurrent"
        );
        let candidate = std::fs::read_dir(&moved)
            .expect("moved directory")
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains("stage"))
            .expect("candidate recovery artifact");
        assert_eq!(
            std::fs::read(candidate.path()).expect("candidate retained"),
            b"new"
        );
        assert_eq!(
            std::fs::read_dir(selected)
                .expect("replacement namespace")
                .count(),
            0
        );
    }

    #[cfg(windows)]
    #[test]
    fn sealed_stage_drop_deletes_its_handle_not_a_replacement_namespace_name() {
        let container = TempDir::new().expect("container");
        let selected = container.path().join("selected");
        let displaced = container.path().join("displaced");
        std::fs::create_dir(&selected).expect("selected directory");
        let directory = ValidatedLocalDirectory::new(&selected).expect("directory");
        let target =
            DurableFileTarget::selected_child(&directory, "export.tmconfig").expect("target");
        let staged = staged(&target, b"new");
        let stage_name = staged
            .path
            .as_deref()
            .and_then(Path::file_name)
            .expect("stage name")
            .to_owned();

        assert!(
            std::fs::rename(&selected, &displaced).is_err(),
            "the retained cleanup handle pins the selected namespace"
        );
        drop(staged);
        assert!(!selected.join(&stage_name).exists(), "owned stage deleted");
        std::fs::rename(&selected, &displaced).expect("move namespace after close");
        std::fs::create_dir(&selected).expect("replacement namespace");
        let replacement = selected.join(&stage_name);
        std::fs::write(&replacement, b"keep").expect("replacement stage name");
        assert_eq!(
            std::fs::read(replacement).expect("replacement retained"),
            b"keep"
        );
        assert!(!displaced.join(stage_name).exists(), "old namespace clean");
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
