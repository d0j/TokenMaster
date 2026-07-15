use tokenmaster_engine::{PortError, PortErrorCode};
use tokenmaster_provider::{ProviderError, ProviderErrorCode};
use tokenmaster_store::{StoreError, StoreErrorCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeErrorCode {
    InvalidConfiguration,
    ProviderUnavailable,
    StoreUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeError {
    code: RuntimeErrorCode,
}

impl RuntimeError {
    pub(crate) const fn new(code: RuntimeErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> RuntimeErrorCode {
        self.code
    }
}

impl core::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self.code {
            RuntimeErrorCode::InvalidConfiguration => "invalid runtime configuration",
            RuntimeErrorCode::ProviderUnavailable => "runtime provider is unavailable",
            RuntimeErrorCode::StoreUnavailable => "runtime store is unavailable",
        })
    }
}

impl std::error::Error for RuntimeError {}

pub(crate) fn provider_port_error(error: &ProviderError) -> PortError {
    let code = match error.code() {
        ProviderErrorCode::CapacityExceeded | ProviderErrorCode::TooManyRoots => {
            PortErrorCode::CapacityExceeded
        }
        ProviderErrorCode::InvalidId
        | ProviderErrorCode::InvalidDisplayName
        | ProviderErrorCode::InvalidPath => PortErrorCode::InvalidData,
        ProviderErrorCode::Io => PortErrorCode::Unavailable,
    };
    PortError::new(code)
}

pub(crate) fn store_port_error(error: &StoreError) -> PortError {
    let code = match error.code() {
        StoreErrorCode::CapacityExceeded => PortErrorCode::CapacityExceeded,
        StoreErrorCode::StaleCheckpoint
        | StoreErrorCode::StaleRevision
        | StoreErrorCode::StaleScan
        | StoreErrorCode::PendingScan
        | StoreErrorCode::PendingContinuation => PortErrorCode::StaleState,
        StoreErrorCode::ScanInProgress => PortErrorCode::Busy,
        StoreErrorCode::Database
        | StoreErrorCode::VersionMismatch
        | StoreErrorCode::SchemaTooNew
        | StoreErrorCode::SchemaMismatch
        | StoreErrorCode::PolicyMismatch => PortErrorCode::Unavailable,
        StoreErrorCode::InvalidValue
        | StoreErrorCode::InvalidStoredValue
        | StoreErrorCode::AccountingVersionMismatch
        | StoreErrorCode::IncompleteManifest
        | StoreErrorCode::UnsealedRevision
        | StoreErrorCode::ArchiveModeMismatch => PortErrorCode::InvalidData,
    };
    PortError::new(code)
}
