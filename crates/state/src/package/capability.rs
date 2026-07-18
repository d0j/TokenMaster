use std::io::{self, Read, Write};

use tokenmaster_platform::{DurableFileError, MAX_DURABLE_WRITE_CHUNK_BYTES};
pub(crate) use tokenmaster_platform::{DurableFileReader, DurableStagedFile};

use crate::StateError;

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
