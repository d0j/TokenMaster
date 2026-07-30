use std::fmt;
use std::path::Path;

use tokenmaster_engine::{PortError, PortErrorCode, WriterLease, WriterLeaseGuard};
use tokenmaster_platform::{ExclusiveFileLease, ExclusiveFileLeaseError, ExclusiveFileLeaseGuard};

use crate::{RuntimeError, RuntimeErrorCode};

/// Runtime bridge from an archive path to the provider-neutral engine lease port.
pub struct RuntimeWriterLease {
    inner: ExclusiveFileLease,
}

impl RuntimeWriterLease {
    pub fn new(archive: &Path) -> Result<Self, RuntimeError> {
        let inner = ExclusiveFileLease::for_archive(archive).map_err(|error| {
            RuntimeError::new(match error {
                ExclusiveFileLeaseError::InvalidPath
                | ExclusiveFileLeaseError::UnsupportedLocation => {
                    RuntimeErrorCode::InvalidConfiguration
                }
                ExclusiveFileLeaseError::Unavailable
                | ExclusiveFileLeaseError::Contended
                | ExclusiveFileLeaseError::InvalidSidecar => RuntimeErrorCode::StoreUnavailable,
            })
        })?;
        Ok(Self { inner })
    }

    pub(crate) fn try_acquire_startup(&self) -> Result<ExclusiveFileLeaseGuard, PortError> {
        self.inner.try_acquire().map_err(lease_port_error)
    }

    pub(crate) fn authorize_startup_guard(
        &self,
        guard: &ExclusiveFileLeaseGuard,
    ) -> Result<(), PortError> {
        self.inner.authorize_guard(guard).map_err(lease_port_error)
    }
}

impl fmt::Debug for RuntimeWriterLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RuntimeWriterLease([redacted])")
    }
}

impl WriterLease for RuntimeWriterLease {
    fn try_acquire(&mut self) -> Result<Box<dyn WriterLeaseGuard>, PortError> {
        self.inner
            .try_acquire()
            .map(|guard| Box::new(RuntimeWriterLeaseGuard { inner: guard }) as Box<_>)
            .map_err(lease_port_error)
    }
}

struct RuntimeWriterLeaseGuard {
    inner: ExclusiveFileLeaseGuard,
}

impl WriterLeaseGuard for RuntimeWriterLeaseGuard {}

impl fmt::Debug for RuntimeWriterLeaseGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = &self.inner;
        formatter.write_str("RuntimeWriterLeaseGuard([redacted])")
    }
}

fn lease_port_error(error: ExclusiveFileLeaseError) -> PortError {
    let code = match error {
        ExclusiveFileLeaseError::Contended => PortErrorCode::Busy,
        ExclusiveFileLeaseError::Unavailable => PortErrorCode::Unavailable,
        ExclusiveFileLeaseError::InvalidPath
        | ExclusiveFileLeaseError::UnsupportedLocation
        | ExclusiveFileLeaseError::InvalidSidecar => PortErrorCode::InvalidData,
    };
    PortError::new(code)
}
