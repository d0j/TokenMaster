use core::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::durable_file::inspect_file;
use crate::{
    DURABLE_STAGE_ATTEMPTS, DurableFileError, DurableFileReader, DurableFileReceipt,
    DurableFileTarget, DurableStagedFile, ExclusiveFileLeaseGuard, LocalDirectoryError,
    ValidatedLocalDirectory,
};

#[cfg(unix)]
use crate::unix::{available_space, move_file_write_through, replace_file_write_through};
#[cfg(not(any(unix, windows)))]
use crate::unsupported::{available_space, move_file_write_through, replace_file_write_through};
#[cfg(windows)]
use crate::windows::{available_space, move_file_write_through, replace_file_write_through};

const ACTIVE_MAIN: &str = "tokenmaster.sqlite3";
const ACTIVE_WAL: &str = "tokenmaster.sqlite3-wal";
const ACTIVE_SHM: &str = "tokenmaster.sqlite3-shm";
const RELIABLE_STATE: &str = "reliable-state";
const STAGING: &str = "staging";
const QUARANTINE: &str = "quarantine";
const FAILED_MAIN: &str = "failed-main.sqlite3";
const OPERATION_PREFIX: &str = "op-";
const CANDIDATE_PREFIX: &str = "restore-";
const CANDIDATE_SUFFIX: &str = ".sqlite3";
const RESERVATION_PREFIX: &str = "reserve-";
const RESERVATION_SUFFIX: &str = ".tokenmaster";
const OPERATION_ATTEMPTS: usize = 32;
pub const MAX_QUARANTINE_SETS: usize = 3;
pub const MAX_RECOVERY_STAGING_ARTIFACTS: usize = 3;

static OPERATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Fixed path-owning authority for one TokenMaster archive recovery namespace.
pub struct ArchiveRecoveryScope {
    active_main: PathBuf,
    active_wal: PathBuf,
    active_shm: PathBuf,
    reliable_state: ValidatedLocalDirectory,
    staging: ValidatedLocalDirectory,
    quarantine: ValidatedLocalDirectory,
    scope: [u8; 32],
}

impl ArchiveRecoveryScope {
    pub fn new(
        data_root: &ValidatedLocalDirectory,
        reliable_state: &ValidatedLocalDirectory,
    ) -> Result<Self, ArchiveRecoveryError> {
        let expected_reliable = data_root.as_path().join(RELIABLE_STATE);
        if expected_reliable != reliable_state.as_path() {
            return Err(ArchiveRecoveryError::UnsupportedLocation);
        }
        let staging = exact_directory(reliable_state.as_path(), STAGING)?;
        let quarantine = exact_directory(reliable_state.as_path(), QUARANTINE)?;
        let active_main = data_root.as_path().join(ACTIVE_MAIN);
        let active_wal = data_root.as_path().join(ACTIVE_WAL);
        let active_shm = data_root.as_path().join(ACTIVE_SHM);
        let scope = hash_scope(&active_main, staging.as_path(), quarantine.as_path());
        Ok(Self {
            active_main,
            active_wal,
            active_shm,
            reliable_state: reliable_state.clone(),
            staging,
            quarantine,
            scope,
        })
    }

    pub fn observe(
        &self,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<ArchiveSetObservation, ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        Ok(ArchiveSetObservation {
            main: inspect_optional(&self.active_main)?,
            wal: inspect_optional(&self.active_wal)?,
            shm: inspect_optional(&self.active_shm)?,
        })
    }

    pub fn authorize_guard(
        &self,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<(), ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()
    }

    pub fn authorize_data_root(
        &self,
        data_root: &ValidatedLocalDirectory,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<(), ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        if data_root.as_path().join(ACTIVE_MAIN) != self.active_main {
            return Err(ArchiveRecoveryError::UnsupportedLocation);
        }
        Ok(())
    }

    pub fn reliable_state_root(&self) -> Result<ValidatedLocalDirectory, ArchiveRecoveryError> {
        self.revalidate()?;
        Ok(self.reliable_state.clone())
    }

    pub fn staging_root(&self) -> Result<ValidatedLocalDirectory, ArchiveRecoveryError> {
        self.revalidate()?;
        Ok(self.staging.clone())
    }

    pub fn has_recovery_evidence(
        &self,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<bool, ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        Ok(self.has_any_staging_evidence()? || self.scan_quarantine()? != 0)
    }

    pub fn require_available_staging_bytes(
        &self,
        guard: &ExclusiveFileLeaseGuard,
        required: u64,
    ) -> Result<(), ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        if required == 0 {
            return Err(ArchiveRecoveryError::InvalidState);
        }
        if available_space(self.staging.as_path()).map_err(map_durable_error)? < required {
            return Err(ArchiveRecoveryError::DiskCapacity);
        }
        Ok(())
    }

    pub fn reserve_operation(
        &self,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<RecoveryOperation, ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        if self.scan_staging()? >= MAX_RECOVERY_STAGING_ARTIFACTS {
            return Err(ArchiveRecoveryError::CapacityExceeded);
        }
        if self.scan_quarantine()? >= MAX_QUARANTINE_SETS {
            return Err(ArchiveRecoveryError::CapacityExceeded);
        }
        for _ in 0..OPERATION_ATTEMPTS {
            let id = next_operation_id()?;
            let directory = self.quarantine.as_path().join(operation_name(id));
            let candidate = self.staging.as_path().join(candidate_name(id));
            let reservation = self.staging.as_path().join(reservation_name(id));
            if path_exists(&directory)? || path_exists(&candidate)? || path_exists(&reservation)? {
                continue;
            }
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&reservation)
            {
                Ok(file) => {
                    if file.sync_all().is_err() {
                        drop(file);
                        let _ = fs::remove_file(&reservation);
                        return Err(ArchiveRecoveryError::Unavailable);
                    }
                    drop(file);
                    return Ok(RecoveryOperation {
                        id,
                        scope: self.scope,
                        directory,
                        resumed: false,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(ArchiveRecoveryError::Unavailable),
            }
        }
        Err(ArchiveRecoveryError::CollisionLimit)
    }

    pub fn resume_operation(
        &self,
        persisted_id: [u8; 16],
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<RecoveryOperation, ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        self.scan_quarantine()?;
        let id = RecoveryOperationId(persisted_id);
        let path = self.quarantine.as_path().join(operation_name(id));
        if path_exists(&path)? {
            let directory = ValidatedLocalDirectory::new(&path).map_err(map_directory_error)?;
            validate_operation_directory(directory.as_path())?;
        }
        Ok(RecoveryOperation {
            id,
            scope: self.scope,
            directory: path,
            resumed: true,
        })
    }

    pub fn create_candidate(
        &self,
        operation: &RecoveryOperation,
        max_bytes: u64,
    ) -> Result<RecoveryStagedArchive, ArchiveRecoveryError> {
        self.require_operation(operation)?;
        self.revalidate()?;
        if self.scan_staging()? >= MAX_RECOVERY_STAGING_ARTIFACTS {
            return Err(ArchiveRecoveryError::CapacityExceeded);
        }
        let child = candidate_name(operation.id);
        let target =
            DurableFileTarget::exact_child(&self.staging, &child).map_err(map_durable_error)?;
        let stage = target.create_staged(max_bytes).map_err(map_durable_error)?;
        Ok(RecoveryStagedArchive {
            scope: self.scope,
            target,
            stage: Some(stage),
            receipt: None,
            published: false,
        })
    }

    pub fn resume_candidate(
        &self,
        operation: &RecoveryOperation,
        expected_len: u64,
        expected_sha256: [u8; 32],
    ) -> Result<RecoveryStagedArchive, ArchiveRecoveryError> {
        self.require_operation(operation)?;
        self.revalidate()?;
        let expected = expected_receipt(expected_len, expected_sha256)?;
        let target = DurableFileTarget::exact_child(&self.staging, &candidate_name(operation.id))
            .map_err(map_durable_error)?;
        let observed =
            inspect_optional(&self.staging.as_path().join(candidate_name(operation.id)))?;
        let published = match observed {
            Some(receipt) if receipt == expected => true,
            None => false,
            Some(_) => return Err(ArchiveRecoveryError::ArtifactMismatch),
        };
        Ok(RecoveryStagedArchive {
            scope: self.scope,
            target,
            stage: None,
            receipt: Some(expected),
            published,
        })
    }

    /// Removes only recognized unpublished recovery staging artifacts.
    ///
    /// The caller may invoke this only after proving that no recovery journal exists.
    pub fn discard_abandoned_staging(
        &self,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<usize, ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        let count = self.scan_staging()?;
        for entry in
            fs::read_dir(self.staging.as_path()).map_err(|_| ArchiveRecoveryError::Unavailable)?
        {
            let entry = entry.map_err(|_| ArchiveRecoveryError::Unavailable)?;
            validate_staging_entry(&entry)?;
            fs::remove_file(entry.path()).map_err(|_| ArchiveRecoveryError::RecoveryRequired)?;
        }
        if self.scan_staging()? != 0 {
            return Err(ArchiveRecoveryError::RecoveryRequired);
        }
        Ok(count)
    }

    pub fn open_candidate_reader(
        &self,
        candidate: &RecoveryStagedArchive,
    ) -> Result<DurableFileReader, ArchiveRecoveryError> {
        self.require_candidate(candidate)?;
        let expected = candidate
            .receipt
            .ok_or(ArchiveRecoveryError::InvalidState)?;
        let reader = candidate
            .target
            .open_reader(expected.len())
            .map_err(map_durable_error)?
            .ok_or(ArchiveRecoveryError::ArtifactMismatch)?;
        if reader.len() != expected.len() {
            return Err(ArchiveRecoveryError::ArtifactMismatch);
        }
        Ok(reader)
    }

    pub fn open_active_reader(
        &self,
        guard: &ExclusiveFileLeaseGuard,
        expected_len: u64,
        expected_sha256: [u8; 32],
    ) -> Result<DurableFileReader, ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.revalidate()?;
        let expected = expected_receipt(expected_len, expected_sha256)?;
        if inspect_optional(&self.active_main)? != Some(expected) {
            return Err(ArchiveRecoveryError::ArtifactMismatch);
        }
        DurableFileTarget::exact_child(
            &ValidatedLocalDirectory::new(
                self.active_main
                    .parent()
                    .ok_or(ArchiveRecoveryError::UnsupportedLocation)?,
            )
            .map_err(map_directory_error)?,
            ACTIVE_MAIN,
        )
        .map_err(map_durable_error)?
        .open_reader(expected.len())
        .map_err(map_durable_error)?
        .ok_or(ArchiveRecoveryError::ArtifactMismatch)
    }

    pub fn quarantine_sidecars(
        &self,
        operation: &RecoveryOperation,
        guard: &ExclusiveFileLeaseGuard,
        before: ArchiveSetExpectation,
    ) -> Result<(), ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.require_operation(operation)?;
        self.revalidate()?;
        self.ensure_operation_directory(operation)?;
        self.remove_reservation(operation)?;
        if inspect_optional(&self.active_main)? != before.main {
            return Err(ArchiveRecoveryError::ArtifactMismatch);
        }
        move_expected(
            &self.active_wal,
            &operation.directory.join(ACTIVE_WAL),
            before.wal,
        )?;
        move_expected(
            &self.active_shm,
            &operation.directory.join(ACTIVE_SHM),
            before.shm,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn promote_main(
        &self,
        operation: &RecoveryOperation,
        guard: &ExclusiveFileLeaseGuard,
        candidate: &mut RecoveryStagedArchive,
        expected_candidate_len: u64,
        expected_candidate_sha256: [u8; 32],
        before: ArchiveSetExpectation,
        mode: RecoveryMainMode,
    ) -> Result<DurableFileReceipt, ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.require_operation(operation)?;
        self.require_candidate(candidate)?;
        self.revalidate()?;
        self.require_existing_operation_directory(operation)?;
        let expected_candidate =
            expected_receipt(expected_candidate_len, expected_candidate_sha256)?;
        if candidate.receipt != Some(expected_candidate) {
            return Err(ArchiveRecoveryError::ArtifactMismatch);
        }
        self.require_quarantined_sidecars(operation, before)?;
        let candidate_path = self.staging.as_path().join(candidate_name(operation.id));
        let old_main = operation.directory.join(ACTIVE_MAIN);

        let result = match mode {
            RecoveryMainMode::ReplaceExisting => self.replace_existing_main(
                &candidate_path,
                &old_main,
                expected_candidate,
                before.main.ok_or(ArchiveRecoveryError::ArtifactMismatch)?,
            ),
            RecoveryMainMode::PromoteMissing => {
                if before.main.is_some() {
                    return Err(ArchiveRecoveryError::ArtifactMismatch);
                }
                self.promote_missing_main(&candidate_path, &old_main, expected_candidate)
            }
        };
        match result {
            Ok(receipt) => {
                candidate.receipt = None;
                candidate.published = false;
                Ok(receipt)
            }
            Err(ArchiveRecoveryError::Unavailable) => self.classify_unavailable_promotion(
                operation,
                candidate,
                expected_candidate,
                before,
                mode,
            ),
            Err(error) => Err(error),
        }
    }

    fn classify_unavailable_promotion(
        &self,
        operation: &RecoveryOperation,
        candidate: &mut RecoveryStagedArchive,
        expected_candidate: DurableFileReceipt,
        before: ArchiveSetExpectation,
        mode: RecoveryMainMode,
    ) -> Result<DurableFileReceipt, ArchiveRecoveryError> {
        let candidate_path = self.staging.as_path().join(candidate_name(operation.id));
        let old_main = operation.directory.join(ACTIVE_MAIN);
        let state = PromotionObservation {
            active: inspect_optional(&self.active_main)
                .map_err(|_| ArchiveRecoveryError::RecoveryRequired)?,
            staged: inspect_optional(&candidate_path)
                .map_err(|_| ArchiveRecoveryError::RecoveryRequired)?,
            quarantined: inspect_optional(&old_main)
                .map_err(|_| ArchiveRecoveryError::RecoveryRequired)?,
        };
        match classify_unavailable_promotion(state, mode, expected_candidate, before.main) {
            PromotionDisposition::Completed => {
                candidate.receipt = None;
                candidate.published = false;
                Ok(expected_candidate)
            }
            PromotionDisposition::NotStarted => {
                self.restore_sidecars(operation, before)
                    .map_err(|_| ArchiveRecoveryError::RecoveryRequired)?;
                Err(ArchiveRecoveryError::Unavailable)
            }
            PromotionDisposition::Ambiguous => Err(ArchiveRecoveryError::RecoveryRequired),
        }
    }

    pub fn rollback(
        &self,
        operation: &RecoveryOperation,
        guard: &ExclusiveFileLeaseGuard,
        before: ArchiveSetExpectation,
    ) -> Result<(), ArchiveRecoveryError> {
        self.require_guard(guard)?;
        self.require_operation(operation)?;
        self.revalidate()?;
        self.require_existing_operation_directory(operation)?;
        let old_main = operation.directory.join(ACTIVE_MAIN);
        let failed_main = operation.directory.join(FAILED_MAIN);
        match before.main {
            Some(expected_old) => {
                let active = inspect_optional(&self.active_main)?;
                let quarantined = inspect_optional(&old_main)?;
                let failed = inspect_optional(&failed_main)?;
                if active == Some(expected_old) && quarantined.is_none() && failed.is_some() {
                    // The atomic rollback already completed.
                } else if active.is_some() && quarantined == Some(expected_old) && failed.is_none()
                {
                    replace_file_write_through(&self.active_main, &old_main, &failed_main)
                        .map_err(map_durable_error)?;
                } else {
                    return Err(ArchiveRecoveryError::ArtifactMismatch);
                }
                if inspect_optional(&self.active_main)? != Some(expected_old) {
                    return Err(ArchiveRecoveryError::RecoveryRequired);
                }
            }
            None => {
                let active = inspect_optional(&self.active_main)?;
                let failed = inspect_optional(&failed_main)?;
                if active.is_none() && failed.is_some() {
                    // Missing-main rollback already completed.
                } else if active.is_some() && failed.is_none() {
                    move_file_write_through(&self.active_main, &failed_main)
                        .map_err(map_durable_error)?;
                } else {
                    return Err(ArchiveRecoveryError::ArtifactMismatch);
                }
                if inspect_optional(&self.active_main)?.is_some() {
                    return Err(ArchiveRecoveryError::RecoveryRequired);
                }
            }
        }
        self.restore_sidecars(operation, before)
    }

    fn replace_existing_main(
        &self,
        candidate: &Path,
        old_main: &Path,
        expected_candidate: DurableFileReceipt,
        expected_old: DurableFileReceipt,
    ) -> Result<DurableFileReceipt, ArchiveRecoveryError> {
        let active = inspect_optional(&self.active_main)?;
        let staged = inspect_optional(candidate)?;
        let quarantined = inspect_optional(old_main)?;
        if active == Some(expected_candidate)
            && staged.is_none()
            && quarantined == Some(expected_old)
        {
            return Ok(expected_candidate);
        }
        if active != Some(expected_old)
            || staged != Some(expected_candidate)
            || quarantined.is_some()
        {
            return Err(ArchiveRecoveryError::ArtifactMismatch);
        }
        replace_file_write_through(&self.active_main, candidate, old_main)
            .map_err(map_durable_error)?;
        if inspect_optional(&self.active_main)? != Some(expected_candidate)
            || inspect_optional(old_main)? != Some(expected_old)
        {
            return Err(ArchiveRecoveryError::RecoveryRequired);
        }
        Ok(expected_candidate)
    }

    fn promote_missing_main(
        &self,
        candidate: &Path,
        old_main: &Path,
        expected_candidate: DurableFileReceipt,
    ) -> Result<DurableFileReceipt, ArchiveRecoveryError> {
        let active = inspect_optional(&self.active_main)?;
        let staged = inspect_optional(candidate)?;
        if active == Some(expected_candidate) && staged.is_none() {
            return Ok(expected_candidate);
        }
        if active.is_some()
            || staged != Some(expected_candidate)
            || inspect_optional(old_main)?.is_some()
        {
            return Err(ArchiveRecoveryError::ArtifactMismatch);
        }
        move_file_write_through(candidate, &self.active_main).map_err(map_durable_error)?;
        if inspect_optional(&self.active_main)? != Some(expected_candidate) {
            return Err(ArchiveRecoveryError::RecoveryRequired);
        }
        Ok(expected_candidate)
    }

    fn restore_sidecars(
        &self,
        operation: &RecoveryOperation,
        before: ArchiveSetExpectation,
    ) -> Result<(), ArchiveRecoveryError> {
        restore_expected(
            &operation.directory.join(ACTIVE_WAL),
            &self.active_wal,
            before.wal,
        )?;
        restore_expected(
            &operation.directory.join(ACTIVE_SHM),
            &self.active_shm,
            before.shm,
        )
    }

    fn require_quarantined_sidecars(
        &self,
        operation: &RecoveryOperation,
        before: ArchiveSetExpectation,
    ) -> Result<(), ArchiveRecoveryError> {
        if inspect_optional(&self.active_wal)?.is_some()
            || inspect_optional(&self.active_shm)?.is_some()
            || inspect_optional(&operation.directory.join(ACTIVE_WAL))? != before.wal
            || inspect_optional(&operation.directory.join(ACTIVE_SHM))? != before.shm
        {
            return Err(ArchiveRecoveryError::ArtifactMismatch);
        }
        Ok(())
    }

    fn scan_quarantine(&self) -> Result<usize, ArchiveRecoveryError> {
        let mut count = 0_usize;
        for entry in fs::read_dir(self.quarantine.as_path())
            .map_err(|_| ArchiveRecoveryError::Unavailable)?
        {
            let entry = entry.map_err(|_| ArchiveRecoveryError::Unavailable)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| ArchiveRecoveryError::UnexpectedArtifact)?;
            if parse_operation_name(&name).is_none() {
                return Err(ArchiveRecoveryError::UnexpectedArtifact);
            }
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| ArchiveRecoveryError::Unavailable)?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata)
            {
                return Err(ArchiveRecoveryError::UnexpectedArtifact);
            }
            validate_operation_directory(&entry.path())?;
            count = count
                .checked_add(1)
                .ok_or(ArchiveRecoveryError::CapacityExceeded)?;
            if count > MAX_QUARANTINE_SETS {
                return Err(ArchiveRecoveryError::CapacityExceeded);
            }
        }
        Ok(count)
    }

    fn scan_staging(&self) -> Result<usize, ArchiveRecoveryError> {
        let mut count = 0_usize;
        for entry in
            fs::read_dir(self.staging.as_path()).map_err(|_| ArchiveRecoveryError::Unavailable)?
        {
            let entry = entry.map_err(|_| ArchiveRecoveryError::Unavailable)?;
            validate_staging_entry(&entry)?;
            count = count
                .checked_add(1)
                .ok_or(ArchiveRecoveryError::CapacityExceeded)?;
            if count > MAX_RECOVERY_STAGING_ARTIFACTS {
                return Err(ArchiveRecoveryError::CapacityExceeded);
            }
        }
        Ok(count)
    }

    fn has_any_staging_evidence(&self) -> Result<bool, ArchiveRecoveryError> {
        let mut count = 0_usize;
        for entry in
            fs::read_dir(self.staging.as_path()).map_err(|_| ArchiveRecoveryError::Unavailable)?
        {
            entry.map_err(|_| ArchiveRecoveryError::Unavailable)?;
            count = count
                .checked_add(1)
                .ok_or(ArchiveRecoveryError::CapacityExceeded)?;
            if count > MAX_RECOVERY_STAGING_ARTIFACTS {
                return Err(ArchiveRecoveryError::CapacityExceeded);
            }
        }
        Ok(count != 0)
    }

    fn require_guard(&self, guard: &ExclusiveFileLeaseGuard) -> Result<(), ArchiveRecoveryError> {
        match guard.authorizes_archive(&self.active_main) {
            Ok(true) => Ok(()),
            Ok(false) => Err(ArchiveRecoveryError::WrongLease),
            Err(_) => Err(ArchiveRecoveryError::Unavailable),
        }
    }

    fn require_operation(&self, operation: &RecoveryOperation) -> Result<(), ArchiveRecoveryError> {
        if operation.scope != self.scope {
            return Err(ArchiveRecoveryError::InvalidState);
        }
        let expected = self.quarantine.as_path().join(operation_name(operation.id));
        if operation.directory != expected {
            return Err(ArchiveRecoveryError::InvalidState);
        }
        if path_exists(&operation.directory)? {
            self.require_existing_operation_directory(operation)
        } else {
            Ok(())
        }
    }

    fn ensure_operation_directory(
        &self,
        operation: &RecoveryOperation,
    ) -> Result<(), ArchiveRecoveryError> {
        if path_exists(&operation.directory)? {
            return if operation.resumed {
                self.require_existing_operation_directory(operation)
            } else {
                Err(ArchiveRecoveryError::UnexpectedArtifact)
            };
        }
        if self.scan_quarantine()? >= MAX_QUARANTINE_SETS {
            return Err(ArchiveRecoveryError::CapacityExceeded);
        }
        match fs::create_dir(&operation.directory) {
            Ok(()) => self.require_existing_operation_directory(operation),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                Err(ArchiveRecoveryError::UnexpectedArtifact)
            }
            Err(_) => Err(ArchiveRecoveryError::Unavailable),
        }
    }

    fn require_existing_operation_directory(
        &self,
        operation: &RecoveryOperation,
    ) -> Result<(), ArchiveRecoveryError> {
        let directory =
            ValidatedLocalDirectory::new(&operation.directory).map_err(map_directory_error)?;
        validate_operation_directory(directory.as_path())
    }

    fn remove_reservation(
        &self,
        operation: &RecoveryOperation,
    ) -> Result<(), ArchiveRecoveryError> {
        let path = self.staging.as_path().join(reservation_name(operation.id));
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound && operation.resumed => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                Err(ArchiveRecoveryError::ArtifactMismatch)
            }
            Err(_) => Err(ArchiveRecoveryError::RecoveryRequired),
        }
    }

    fn require_candidate(
        &self,
        candidate: &RecoveryStagedArchive,
    ) -> Result<(), ArchiveRecoveryError> {
        if candidate.scope == self.scope {
            Ok(())
        } else {
            Err(ArchiveRecoveryError::InvalidState)
        }
    }

    fn revalidate(&self) -> Result<(), ArchiveRecoveryError> {
        let reliable_state = ValidatedLocalDirectory::new(self.reliable_state.as_path())
            .map_err(map_directory_error)?;
        let staging =
            ValidatedLocalDirectory::new(self.staging.as_path()).map_err(map_directory_error)?;
        let quarantine =
            ValidatedLocalDirectory::new(self.quarantine.as_path()).map_err(map_directory_error)?;
        if reliable_state == self.reliable_state
            && staging == self.staging
            && quarantine == self.quarantine
            && staging.as_path() == reliable_state.as_path().join(STAGING)
            && quarantine.as_path() == reliable_state.as_path().join(QUARANTINE)
        {
            Ok(())
        } else {
            Err(ArchiveRecoveryError::UnsupportedLocation)
        }
    }
}

impl fmt::Debug for ArchiveRecoveryScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ArchiveRecoveryScope([redacted])")
    }
}

/// One opaque, scope-bound quarantine reservation.
pub struct RecoveryOperation {
    id: RecoveryOperationId,
    scope: [u8; 32],
    directory: PathBuf,
    resumed: bool,
}

impl RecoveryOperation {
    #[must_use]
    pub const fn id(&self) -> RecoveryOperationId {
        self.id
    }
}

impl fmt::Debug for RecoveryOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryOperation([redacted])")
    }
}

/// Persistable path-free identity generated only by the recovery scope.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RecoveryOperationId([u8; 16]);

impl RecoveryOperationId {
    #[must_use]
    pub const fn to_persisted_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for RecoveryOperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryOperationId([redacted])")
    }
}

/// Sealed fixed-operation candidate whose path never leaves platform ownership.
pub struct RecoveryStagedArchive {
    scope: [u8; 32],
    target: DurableFileTarget,
    stage: Option<DurableStagedFile>,
    receipt: Option<DurableFileReceipt>,
    published: bool,
}

impl RecoveryStagedArchive {
    #[must_use]
    pub const fn written_len(&self) -> u64 {
        match (&self.stage, self.receipt) {
            (Some(stage), _) => stage.written_len(),
            (None, Some(receipt)) => receipt.len(),
            (None, None) => 0,
        }
    }

    pub fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), ArchiveRecoveryError> {
        if self.receipt.is_some() {
            return Err(ArchiveRecoveryError::InvalidState);
        }
        self.stage
            .as_mut()
            .ok_or(ArchiveRecoveryError::InvalidState)?
            .write_chunk(bytes)
            .map_err(map_durable_error)
    }

    pub fn seal(
        &mut self,
        expected_len: u64,
        expected_sha256: [u8; 32],
    ) -> Result<DurableFileReceipt, ArchiveRecoveryError> {
        if self.receipt.is_some() {
            return Err(ArchiveRecoveryError::InvalidState);
        }
        let stage = self
            .stage
            .as_mut()
            .ok_or(ArchiveRecoveryError::InvalidState)?;
        let sealed = stage
            .seal(expected_len, expected_sha256)
            .map_err(map_durable_error)?;
        if stage.sealed_receipt() != Some(sealed) {
            return Err(ArchiveRecoveryError::RecoveryRequired);
        }
        let published = stage.publish_new(&self.target).map_err(map_durable_error)?;
        if published != sealed {
            return Err(ArchiveRecoveryError::RecoveryRequired);
        }
        self.receipt = Some(published);
        self.published = true;
        Ok(published)
    }

    pub fn discard(&mut self) -> Result<(), ArchiveRecoveryError> {
        if let Some(expected) = self.receipt.take() {
            if inspect_optional(self.target.exact_path())? != Some(expected) {
                self.receipt = Some(expected);
                return Err(ArchiveRecoveryError::ArtifactMismatch);
            }
            fs::remove_file(self.target.exact_path())
                .map_err(|_| ArchiveRecoveryError::RecoveryRequired)?;
            if inspect_optional(self.target.exact_path())?.is_some() {
                return Err(ArchiveRecoveryError::RecoveryRequired);
            }
            self.published = false;
            Ok(())
        } else if let Some(stage) = self.stage.as_mut() {
            stage.discard().map_err(map_durable_error)
        } else {
            Err(ArchiveRecoveryError::InvalidState)
        }
    }

    #[must_use]
    pub const fn receipt(&self) -> Option<DurableFileReceipt> {
        self.receipt
    }

    /// Reports whether the sealed candidate still exists in the fixed staging slot.
    #[must_use]
    pub const fn is_staged(&self) -> bool {
        self.receipt.is_some() && self.published
    }
}

impl fmt::Debug for RecoveryStagedArchive {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryStagedArchive([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryMainMode {
    ReplaceExisting,
    PromoteMissing,
}

#[derive(Clone, Copy)]
struct PromotionObservation {
    active: Option<DurableFileReceipt>,
    staged: Option<DurableFileReceipt>,
    quarantined: Option<DurableFileReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PromotionDisposition {
    Completed,
    NotStarted,
    Ambiguous,
}

fn classify_unavailable_promotion(
    observed: PromotionObservation,
    mode: RecoveryMainMode,
    expected_candidate: DurableFileReceipt,
    expected_old: Option<DurableFileReceipt>,
) -> PromotionDisposition {
    match mode {
        RecoveryMainMode::ReplaceExisting => match expected_old {
            Some(old)
                if observed.active == Some(expected_candidate)
                    && observed.staged.is_none()
                    && observed.quarantined == Some(old) =>
            {
                PromotionDisposition::Completed
            }
            Some(old)
                if observed.active == Some(old)
                    && observed.staged == Some(expected_candidate)
                    && observed.quarantined.is_none() =>
            {
                PromotionDisposition::NotStarted
            }
            _ => PromotionDisposition::Ambiguous,
        },
        RecoveryMainMode::PromoteMissing => {
            if expected_old.is_some() {
                return PromotionDisposition::Ambiguous;
            }
            if observed.active == Some(expected_candidate)
                && observed.staged.is_none()
                && observed.quarantined.is_none()
            {
                PromotionDisposition::Completed
            } else if observed.active.is_none()
                && observed.staged == Some(expected_candidate)
                && observed.quarantined.is_none()
            {
                PromotionDisposition::NotStarted
            } else {
                PromotionDisposition::Ambiguous
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ArchiveFileObservation(Option<DurableFileReceipt>);

impl ArchiveFileObservation {
    #[must_use]
    pub const fn receipt(self) -> Option<DurableFileReceipt> {
        self.0
    }

    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.is_some()
    }
}

impl fmt::Debug for ArchiveFileObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(if self.0.is_some() {
            "ArchiveFileObservation::Present([redacted])"
        } else {
            "ArchiveFileObservation::Missing"
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ArchiveSetObservation {
    main: Option<DurableFileReceipt>,
    wal: Option<DurableFileReceipt>,
    shm: Option<DurableFileReceipt>,
}

impl ArchiveSetObservation {
    #[must_use]
    pub const fn main(self) -> Option<DurableFileReceipt> {
        self.main
    }

    #[must_use]
    pub const fn wal(self) -> Option<DurableFileReceipt> {
        self.wal
    }

    #[must_use]
    pub const fn shm(self) -> Option<DurableFileReceipt> {
        self.shm
    }

    #[must_use]
    pub const fn has_any_archive_artifact(self) -> bool {
        self.main.is_some() || self.wal.is_some() || self.shm.is_some()
    }

    #[must_use]
    pub const fn expectation(self) -> ArchiveSetExpectation {
        ArchiveSetExpectation {
            main: self.main,
            wal: self.wal,
            shm: self.shm,
        }
    }
}

impl fmt::Debug for ArchiveSetObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArchiveSetObservation")
            .field("main_present", &self.main.is_some())
            .field("wal_present", &self.wal.is_some())
            .field("shm_present", &self.shm.is_some())
            .finish()
    }
}

/// Persisted expected identities for the three fixed active archive children.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ArchiveSetExpectation {
    main: Option<DurableFileReceipt>,
    wal: Option<DurableFileReceipt>,
    shm: Option<DurableFileReceipt>,
}

impl ArchiveSetExpectation {
    pub fn from_persisted(
        main: Option<(u64, [u8; 32])>,
        wal: Option<(u64, [u8; 32])>,
        shm: Option<(u64, [u8; 32])>,
    ) -> Result<Self, ArchiveRecoveryError> {
        Ok(Self {
            main: expected_optional(main)?,
            wal: expected_optional_sidecar(wal)?,
            shm: expected_optional_sidecar(shm)?,
        })
    }

    #[must_use]
    pub const fn main(self) -> Option<DurableFileReceipt> {
        self.main
    }

    #[must_use]
    pub const fn wal(self) -> Option<DurableFileReceipt> {
        self.wal
    }

    #[must_use]
    pub const fn shm(self) -> Option<DurableFileReceipt> {
        self.shm
    }
}

impl fmt::Debug for ArchiveSetExpectation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArchiveSetExpectation")
            .field("main_present", &self.main.is_some())
            .field("wal_present", &self.wal.is_some())
            .field("shm_present", &self.shm.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ArchiveRecoveryError {
    #[error("archive recovery location is unsupported")]
    UnsupportedLocation,
    #[error("archive recovery requires the matching writer lease")]
    WrongLease,
    #[error("archive recovery contains an unexpected artifact")]
    UnexpectedArtifact,
    #[error("archive recovery artifact identity does not match")]
    ArtifactMismatch,
    #[error("archive recovery capacity limit was exceeded")]
    CapacityExceeded,
    #[error("archive recovery has insufficient disk capacity")]
    DiskCapacity,
    #[error("archive recovery operation ID collision limit was reached")]
    CollisionLimit,
    #[error("archive recovery operation state is invalid")]
    InvalidState,
    #[error("archive recovery operation is unavailable")]
    Unavailable,
    #[error("archive recovery operation requires safe-mode recovery")]
    RecoveryRequired,
}

fn exact_directory(
    parent: &Path,
    name: &str,
) -> Result<ValidatedLocalDirectory, ArchiveRecoveryError> {
    let path = parent.join(name);
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(_) => return Err(ArchiveRecoveryError::Unavailable),
    }
    ValidatedLocalDirectory::new(&path).map_err(map_directory_error)
}

fn validate_operation_directory(path: &Path) -> Result<(), ArchiveRecoveryError> {
    for entry in fs::read_dir(path).map_err(|_| ArchiveRecoveryError::Unavailable)? {
        let entry = entry.map_err(|_| ArchiveRecoveryError::Unavailable)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| ArchiveRecoveryError::UnexpectedArtifact)?;
        if !matches!(
            name.as_str(),
            ACTIVE_MAIN | ACTIVE_WAL | ACTIVE_SHM | FAILED_MAIN
        ) {
            return Err(ArchiveRecoveryError::UnexpectedArtifact);
        }
        inspect_optional(&entry.path())?.ok_or(ArchiveRecoveryError::UnexpectedArtifact)?;
    }
    Ok(())
}

fn validate_staging_entry(entry: &fs::DirEntry) -> Result<(), ArchiveRecoveryError> {
    let name = entry
        .file_name()
        .into_string()
        .map_err(|_| ArchiveRecoveryError::UnexpectedArtifact)?;
    let reservation = parse_reservation_name(&name).is_some();
    if !reservation
        && parse_candidate_name(&name).is_none()
        && parse_candidate_stage_name(&name).is_none()
    {
        return Err(ArchiveRecoveryError::UnexpectedArtifact);
    }
    let metadata =
        fs::symlink_metadata(entry.path()).map_err(|_| ArchiveRecoveryError::Unavailable)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(ArchiveRecoveryError::UnexpectedArtifact);
    }
    if reservation && metadata.len() != 0 {
        return Err(ArchiveRecoveryError::UnexpectedArtifact);
    }
    let file = File::open(entry.path()).map_err(|_| ArchiveRecoveryError::Unavailable)?;
    if has_multiple_links(&file, &metadata)? {
        return Err(ArchiveRecoveryError::UnexpectedArtifact);
    }
    Ok(())
}

fn move_expected(
    source: &Path,
    target: &Path,
    expected: Option<DurableFileReceipt>,
) -> Result<(), ArchiveRecoveryError> {
    let source_observed = inspect_optional(source)?;
    let target_observed = inspect_optional(target)?;
    match expected {
        None if source_observed.is_none() && target_observed.is_none() => Ok(()),
        Some(receipt) if source_observed == Some(receipt) && target_observed.is_none() => {
            move_file_write_through(source, target).map_err(map_durable_error)?;
            if inspect_optional(source)?.is_none() && inspect_optional(target)? == Some(receipt) {
                Ok(())
            } else {
                Err(ArchiveRecoveryError::RecoveryRequired)
            }
        }
        Some(receipt) if source_observed.is_none() && target_observed == Some(receipt) => Ok(()),
        _ => Err(ArchiveRecoveryError::ArtifactMismatch),
    }
}

fn restore_expected(
    source: &Path,
    target: &Path,
    expected: Option<DurableFileReceipt>,
) -> Result<(), ArchiveRecoveryError> {
    move_expected(source, target, expected)
}

fn inspect_optional(path: &Path) -> Result<Option<DurableFileReceipt>, ArchiveRecoveryError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ArchiveRecoveryError::Unavailable),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(ArchiveRecoveryError::UnexpectedArtifact);
    }
    let file = File::open(path).map_err(|_| ArchiveRecoveryError::Unavailable)?;
    if has_multiple_links(&file, &metadata)? {
        return Err(ArchiveRecoveryError::UnexpectedArtifact);
    }
    inspect_file(path).map(Some).map_err(map_durable_error)
}

fn path_exists(path: &Path) -> Result<bool, ArchiveRecoveryError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(ArchiveRecoveryError::Unavailable),
    }
}

fn next_operation_id() -> Result<RecoveryOperationId, ArchiveRecoveryError> {
    let counter = OPERATION_COUNTER
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        })
        .map_err(|_| ArchiveRecoveryError::CapacityExceeded)?;
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ArchiveRecoveryError::Unavailable)?;
    let mut hasher = Sha256::new();
    hasher.update(b"tm-recovery-operation-v1");
    hasher.update(elapsed.as_nanos().to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(counter.to_le_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    Ok(RecoveryOperationId(id))
}

fn operation_name(id: RecoveryOperationId) -> String {
    format!("{OPERATION_PREFIX}{}", encode_hex(&id.0))
}

fn candidate_name(id: RecoveryOperationId) -> String {
    format!("{CANDIDATE_PREFIX}{}{CANDIDATE_SUFFIX}", encode_hex(&id.0))
}

fn reservation_name(id: RecoveryOperationId) -> String {
    format!(
        "{RESERVATION_PREFIX}{}{RESERVATION_SUFFIX}",
        encode_hex(&id.0)
    )
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn parse_operation_name(name: &str) -> Option<RecoveryOperationId> {
    let hex = name.strip_prefix(OPERATION_PREFIX)?;
    parse_operation_hex(hex)
}

fn parse_candidate_name(name: &str) -> Option<RecoveryOperationId> {
    let hex = name
        .strip_prefix(CANDIDATE_PREFIX)?
        .strip_suffix(CANDIDATE_SUFFIX)?;
    parse_operation_hex(hex)
}

fn parse_reservation_name(name: &str) -> Option<RecoveryOperationId> {
    let hex = name
        .strip_prefix(RESERVATION_PREFIX)?
        .strip_suffix(RESERVATION_SUFFIX)?;
    parse_operation_hex(hex)
}

fn parse_candidate_stage_name(name: &str) -> Option<RecoveryOperationId> {
    let stage = name.strip_prefix('.')?;
    let (candidate, attempt) = stage.rsplit_once(".tokenmaster-stage-")?;
    let id = parse_candidate_name(candidate)?;
    let digits = attempt.as_bytes();
    if digits.len() != 2 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let value = usize::from(digits[0] - b'0') * 10 + usize::from(digits[1] - b'0');
    (value < DURABLE_STAGE_ATTEMPTS).then_some(id)
}

fn parse_operation_hex(hex: &str) -> Option<RecoveryOperationId> {
    if hex.len() != 32
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return None;
    }
    let mut id = [0_u8; 16];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        id[index] = (decode_hex(pair[0])? << 4) | decode_hex(pair[1])?;
    }
    Some(RecoveryOperationId(id))
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn expected_optional(
    value: Option<(u64, [u8; 32])>,
) -> Result<Option<DurableFileReceipt>, ArchiveRecoveryError> {
    value
        .map(|(len, sha256)| expected_receipt(len, sha256))
        .transpose()
}

fn expected_optional_sidecar(
    value: Option<(u64, [u8; 32])>,
) -> Result<Option<DurableFileReceipt>, ArchiveRecoveryError> {
    value
        .map(|(len, sha256)| {
            if sha256 == [0_u8; 32] {
                Err(ArchiveRecoveryError::ArtifactMismatch)
            } else {
                Ok(DurableFileReceipt::from_expected(len, sha256))
            }
        })
        .transpose()
}

fn expected_receipt(
    len: u64,
    sha256: [u8; 32],
) -> Result<DurableFileReceipt, ArchiveRecoveryError> {
    if len == 0 || sha256 == [0_u8; 32] {
        return Err(ArchiveRecoveryError::ArtifactMismatch);
    }
    Ok(DurableFileReceipt::from_expected(len, sha256))
}

#[cfg(windows)]
fn hash_scope(active: &Path, staging: &Path, quarantine: &Path) -> [u8; 32] {
    use std::os::windows::ffi::OsStrExt;

    let mut hasher = Sha256::new();
    hasher.update(b"tm-archive-recovery-scope-v1-windows");
    for path in [active, staging, quarantine] {
        for unit in path.as_os_str().encode_wide() {
            hasher.update(unit.to_le_bytes());
        }
        hasher.update([0xff]);
    }
    hasher.finalize().into()
}

#[cfg(unix)]
fn hash_scope(active: &Path, staging: &Path, quarantine: &Path) -> [u8; 32] {
    use std::os::unix::ffi::OsStrExt;

    let mut hasher = Sha256::new();
    hasher.update(b"tm-archive-recovery-scope-v1-unix");
    for path in [active, staging, quarantine] {
        hasher.update(path.as_os_str().as_bytes());
        hasher.update([0xff]);
    }
    hasher.finalize().into()
}

#[cfg(not(any(unix, windows)))]
fn hash_scope(_active: &Path, _staging: &Path, _quarantine: &Path) -> [u8; 32] {
    [0_u8; 32]
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn has_multiple_links(file: &File, _metadata: &fs::Metadata) -> Result<bool, ArchiveRecoveryError> {
    crate::windows::platform_link_count(file)
        .map(|count| count > 1)
        .map_err(|_| ArchiveRecoveryError::Unavailable)
}

#[cfg(unix)]
fn has_multiple_links(_file: &File, metadata: &fs::Metadata) -> Result<bool, ArchiveRecoveryError> {
    use std::os::unix::fs::MetadataExt;

    Ok(metadata.nlink() > 1)
}

#[cfg(not(any(unix, windows)))]
fn has_multiple_links(
    _file: &File,
    _metadata: &fs::Metadata,
) -> Result<bool, ArchiveRecoveryError> {
    Ok(true)
}

const fn map_directory_error(error: LocalDirectoryError) -> ArchiveRecoveryError {
    match error {
        LocalDirectoryError::InvalidPath | LocalDirectoryError::UnsupportedLocation => {
            ArchiveRecoveryError::UnsupportedLocation
        }
        LocalDirectoryError::Unavailable => ArchiveRecoveryError::Unavailable,
    }
}

const fn map_durable_error(error: DurableFileError) -> ArchiveRecoveryError {
    match error {
        DurableFileError::InvalidName | DurableFileError::UnsupportedLocation => {
            ArchiveRecoveryError::UnsupportedLocation
        }
        DurableFileError::CollisionLimit => ArchiveRecoveryError::CollisionLimit,
        DurableFileError::CapacityExceeded => ArchiveRecoveryError::CapacityExceeded,
        DurableFileError::InvalidState => ArchiveRecoveryError::InvalidState,
        DurableFileError::Integrity
        | DurableFileError::TargetExists
        | DurableFileError::TargetMissing
        | DurableFileError::UnexpectedType => ArchiveRecoveryError::ArtifactMismatch,
        DurableFileError::Unavailable => ArchiveRecoveryError::Unavailable,
        DurableFileError::RecoveryRequired => ArchiveRecoveryError::RecoveryRequired,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PromotionDisposition, PromotionObservation, RecoveryMainMode,
        classify_unavailable_promotion,
    };
    use crate::DurableFileReceipt;

    fn receipt(byte: u8) -> DurableFileReceipt {
        DurableFileReceipt::from_expected(1, [byte; 32])
    }

    #[test]
    fn unavailable_replace_is_classified_only_from_exact_namespace_facts() {
        let old = receipt(1);
        let candidate = receipt(2);
        assert_eq!(
            classify_unavailable_promotion(
                PromotionObservation {
                    active: Some(old),
                    staged: Some(candidate),
                    quarantined: None,
                },
                RecoveryMainMode::ReplaceExisting,
                candidate,
                Some(old),
            ),
            PromotionDisposition::NotStarted
        );
        assert_eq!(
            classify_unavailable_promotion(
                PromotionObservation {
                    active: Some(candidate),
                    staged: None,
                    quarantined: Some(old),
                },
                RecoveryMainMode::ReplaceExisting,
                candidate,
                Some(old),
            ),
            PromotionDisposition::Completed
        );
    }

    #[test]
    fn unavailable_missing_main_promotion_is_classified_only_from_exact_facts() {
        let candidate = receipt(2);
        assert_eq!(
            classify_unavailable_promotion(
                PromotionObservation {
                    active: None,
                    staged: Some(candidate),
                    quarantined: None,
                },
                RecoveryMainMode::PromoteMissing,
                candidate,
                None,
            ),
            PromotionDisposition::NotStarted
        );
        assert_eq!(
            classify_unavailable_promotion(
                PromotionObservation {
                    active: Some(candidate),
                    staged: None,
                    quarantined: None,
                },
                RecoveryMainMode::PromoteMissing,
                candidate,
                None,
            ),
            PromotionDisposition::Completed
        );
    }

    #[test]
    fn unavailable_promotion_never_guesses_through_ambiguous_state() {
        let old = receipt(1);
        let candidate = receipt(2);
        assert_eq!(
            classify_unavailable_promotion(
                PromotionObservation {
                    active: Some(candidate),
                    staged: Some(candidate),
                    quarantined: Some(old),
                },
                RecoveryMainMode::ReplaceExisting,
                candidate,
                Some(old),
            ),
            PromotionDisposition::Ambiguous
        );
    }
}
