use core::cell::Cell;
use std::io::{self, Read, Write};

use tokenmaster_platform::{
    ArchiveRecoveryError, BackupDirectoryError, DurableFileError, MAX_DURABLE_WRITE_CHUNK_BYTES,
};
pub(crate) use tokenmaster_platform::{
    BackupStagedFile, DurableFileReader, DurableStagedFile, RecoveryStagedArchive,
};

use crate::StateError;

pub(crate) type DurableCapabilityError = DurableFileError;

pub(crate) struct DurableReaderAdapter<'a> {
    source: &'a mut DurableFileReader,
    failure: Option<DurableFileError>,
}

impl<'a> DurableReaderAdapter<'a> {
    pub(crate) fn new(source: &'a mut DurableFileReader) -> Self {
        Self {
            source,
            failure: None,
        }
    }

    pub(crate) const fn failure(&self) -> Option<DurableFileError> {
        self.failure
    }
}

impl Read for DurableReaderAdapter<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.source.read_chunk(buffer) {
            Ok(count) => Ok(count),
            Err(error) => {
                self.failure = Some(error);
                Err(io_error(error))
            }
        }
    }
}

pub(crate) struct DurableReaderFailure {
    failure: Cell<Option<DurableFileError>>,
}

impl DurableReaderFailure {
    pub(crate) const fn new() -> Self {
        Self {
            failure: Cell::new(None),
        }
    }

    pub(crate) const fn get(&self) -> Option<DurableFileError> {
        self.failure.get()
    }
}

pub(crate) struct TrackedDurableReaderAdapter<'a, 'failure> {
    source: &'a mut DurableFileReader,
    failure: &'failure DurableReaderFailure,
}

impl<'a, 'failure> TrackedDurableReaderAdapter<'a, 'failure> {
    pub(crate) fn new(
        source: &'a mut DurableFileReader,
        failure: &'failure DurableReaderFailure,
    ) -> Self {
        Self { source, failure }
    }
}

impl Read for TrackedDurableReaderAdapter<'_, '_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.source.read_chunk(buffer) {
            Ok(count) => Ok(count),
            Err(error) => {
                self.failure.failure.set(Some(error));
                Err(io_error(error))
            }
        }
    }
}

pub(crate) struct DurableWriterAdapter<'a> {
    destination: &'a mut DurableStagedFile,
    failure: Option<DurableFileError>,
}

impl<'a> DurableWriterAdapter<'a> {
    pub(crate) fn new(destination: &'a mut DurableStagedFile) -> Self {
        Self {
            destination,
            failure: None,
        }
    }

    pub(crate) const fn failure(&self) -> Option<DurableFileError> {
        self.failure
    }
}

impl Write for DurableWriterAdapter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(MAX_DURABLE_WRITE_CHUNK_BYTES);
        match self.destination.write_chunk(&bytes[..count]) {
            Ok(()) => Ok(count),
            Err(error) => {
                self.failure = Some(error);
                Err(io_error(error))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct RecoveryWriterAdapter<'a> {
    destination: &'a mut RecoveryStagedArchive,
    failure: Option<ArchiveRecoveryError>,
}

impl<'a> RecoveryWriterAdapter<'a> {
    pub(crate) fn new(destination: &'a mut RecoveryStagedArchive) -> Self {
        Self {
            destination,
            failure: None,
        }
    }

    pub(crate) const fn failure(&self) -> Option<ArchiveRecoveryError> {
        self.failure
    }
}

impl Write for RecoveryWriterAdapter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(MAX_DURABLE_WRITE_CHUNK_BYTES);
        match self.destination.write_chunk(&bytes[..count]) {
            Ok(()) => Ok(count),
            Err(error) => {
                self.failure = Some(error);
                Err(recovery_io_error(error))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct BackupWriterAdapter<'a> {
    destination: &'a mut BackupStagedFile,
    failure: Option<BackupDirectoryError>,
}

impl<'a> BackupWriterAdapter<'a> {
    pub(crate) fn new(destination: &'a mut BackupStagedFile) -> Self {
        Self {
            destination,
            failure: None,
        }
    }

    pub(crate) const fn failure(&self) -> Option<BackupDirectoryError> {
        self.failure
    }
}

impl Write for BackupWriterAdapter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(MAX_DURABLE_WRITE_CHUNK_BYTES);
        match self.destination.write_chunk(&bytes[..count]) {
            Ok(()) => Ok(count),
            Err(error) => {
                self.failure = Some(error);
                Err(backup_io_error(error))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) const fn map_durable_error(error: DurableFileError) -> StateError {
    match error {
        DurableFileError::CapacityExceeded | DurableFileError::CollisionLimit => {
            StateError::capacity_exceeded()
        }
        DurableFileError::Integrity => StateError::integrity(),
        DurableFileError::RecoveryRequired => StateError::recovery_required(),
        DurableFileError::InvalidName | DurableFileError::InvalidState => {
            StateError::internal_invariant()
        }
        DurableFileError::UnsupportedLocation
        | DurableFileError::TargetExists
        | DurableFileError::TargetMissing
        | DurableFileError::UnexpectedType
        | DurableFileError::Unavailable => StateError::unavailable(),
    }
}

pub(crate) const fn map_backup_directory_error(error: BackupDirectoryError) -> StateError {
    match error {
        BackupDirectoryError::UnexpectedEntry
        | BackupDirectoryError::UnexpectedType
        | BackupDirectoryError::LinkedEntry
        | BackupDirectoryError::AmbiguousIdentity => StateError::integrity(),
        BackupDirectoryError::CapacityExceeded => StateError::capacity_exceeded(),
        BackupDirectoryError::RecoveryRequired => StateError::recovery_required(),
        BackupDirectoryError::InvalidState => StateError::internal_invariant(),
        BackupDirectoryError::UnsupportedLocation
        | BackupDirectoryError::StaleEntry
        | BackupDirectoryError::Unavailable => StateError::unavailable(),
    }
}

pub(crate) const fn map_archive_recovery_error(error: ArchiveRecoveryError) -> StateError {
    match error {
        ArchiveRecoveryError::CapacityExceeded | ArchiveRecoveryError::CollisionLimit => {
            StateError::capacity_exceeded()
        }
        ArchiveRecoveryError::DiskCapacity => {
            StateError::from_code(crate::StateErrorCode::DiskCapacity)
        }
        ArchiveRecoveryError::ArtifactMismatch | ArchiveRecoveryError::UnexpectedArtifact => {
            StateError::integrity()
        }
        ArchiveRecoveryError::RecoveryRequired => StateError::recovery_required(),
        ArchiveRecoveryError::InvalidState => StateError::internal_invariant(),
        ArchiveRecoveryError::UnsupportedLocation
        | ArchiveRecoveryError::WrongLease
        | ArchiveRecoveryError::Unavailable => StateError::unavailable(),
    }
}

pub(crate) fn resolve_codec_error<T>(
    result: Result<T, StateError>,
    failures: &[Option<DurableFileError>],
) -> Result<T, StateError> {
    match result {
        Ok(receipt) => Ok(receipt),
        Err(codec_error) => Err(failures
            .iter()
            .flatten()
            .next()
            .copied()
            .map(map_durable_error)
            .unwrap_or(codec_error)),
    }
}

pub(crate) fn resolve_backup_codec_error<T>(
    result: Result<T, StateError>,
    database_failure: Option<DurableFileError>,
    destination_failure: Option<BackupDirectoryError>,
) -> Result<T, StateError> {
    match result {
        Ok(receipt) => Ok(receipt),
        Err(codec_error) => Err(database_failure
            .map(map_durable_error)
            .or_else(|| destination_failure.map(map_backup_directory_error))
            .unwrap_or(codec_error)),
    }
}

fn io_error(error: DurableFileError) -> io::Error {
    let kind = match error {
        DurableFileError::CapacityExceeded | DurableFileError::CollisionLimit => {
            io::ErrorKind::OutOfMemory
        }
        DurableFileError::Integrity
        | DurableFileError::InvalidName
        | DurableFileError::InvalidState
        | DurableFileError::UnsupportedLocation
        | DurableFileError::UnexpectedType => io::ErrorKind::InvalidData,
        DurableFileError::TargetExists
        | DurableFileError::TargetMissing
        | DurableFileError::Unavailable
        | DurableFileError::RecoveryRequired => io::ErrorKind::Other,
    };
    kind.into()
}

fn backup_io_error(error: BackupDirectoryError) -> io::Error {
    let kind = match error {
        BackupDirectoryError::CapacityExceeded => io::ErrorKind::OutOfMemory,
        BackupDirectoryError::UnexpectedEntry
        | BackupDirectoryError::UnexpectedType
        | BackupDirectoryError::LinkedEntry
        | BackupDirectoryError::AmbiguousIdentity
        | BackupDirectoryError::UnsupportedLocation
        | BackupDirectoryError::StaleEntry
        | BackupDirectoryError::InvalidState => io::ErrorKind::InvalidData,
        BackupDirectoryError::Unavailable | BackupDirectoryError::RecoveryRequired => {
            io::ErrorKind::Other
        }
    };
    kind.into()
}

fn recovery_io_error(error: ArchiveRecoveryError) -> io::Error {
    let kind = match error {
        ArchiveRecoveryError::CapacityExceeded | ArchiveRecoveryError::CollisionLimit => {
            io::ErrorKind::OutOfMemory
        }
        ArchiveRecoveryError::ArtifactMismatch
        | ArchiveRecoveryError::UnexpectedArtifact
        | ArchiveRecoveryError::InvalidState
        | ArchiveRecoveryError::UnsupportedLocation
        | ArchiveRecoveryError::WrongLease => io::ErrorKind::InvalidData,
        ArchiveRecoveryError::DiskCapacity
        | ArchiveRecoveryError::Unavailable
        | ArchiveRecoveryError::RecoveryRequired => io::ErrorKind::Other,
    };
    kind.into()
}

#[cfg(test)]
mod tests {
    use super::{
        BackupDirectoryError, DurableFileError, map_backup_directory_error,
        resolve_backup_codec_error,
    };
    use crate::{StateError, StateErrorCode};

    #[test]
    fn mixed_backup_failure_precedence_matches_durable_package_writes() {
        let error = match resolve_backup_codec_error::<()>(
            Err(StateError::unavailable()),
            Some(DurableFileError::Integrity),
            Some(BackupDirectoryError::CapacityExceeded),
        ) {
            Ok(()) => panic!("mixed failure must fail"),
            Err(error) => error,
        };
        assert_eq!(error.code(), StateErrorCode::Integrity);
        assert_eq!(
            map_backup_directory_error(BackupDirectoryError::InvalidState).code(),
            StateErrorCode::InternalInvariant
        );
    }
}
