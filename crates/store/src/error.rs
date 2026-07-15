use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreErrorCode {
    Database,
    VersionMismatch,
    SchemaTooNew,
    SchemaMismatch,
    PolicyMismatch,
    InvalidValue,
    CapacityExceeded,
    InvalidStoredValue,
    StaleCheckpoint,
    RebuildRequired,
    StaleRevision,
    AccountingVersionMismatch,
    IncompleteManifest,
    UnsealedRevision,
    PendingContinuation,
    ScanInProgress,
    StaleScan,
    PendingScan,
    ArchiveModeMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreError {
    code: StoreErrorCode,
    limit: Option<u64>,
}

impl StoreError {
    pub(crate) const fn new(code: StoreErrorCode) -> Self {
        Self { code, limit: None }
    }

    pub(crate) const fn with_limit(code: StoreErrorCode, limit: u64) -> Self {
        Self {
            code,
            limit: Some(limit),
        }
    }

    #[must_use]
    pub const fn code(&self) -> StoreErrorCode {
        self.code
    }

    #[must_use]
    pub const fn limit(&self) -> Option<u64> {
        self.limit
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            StoreErrorCode::Database => "database operation failed",
            StoreErrorCode::VersionMismatch => "SQLite version mismatched",
            StoreErrorCode::SchemaTooNew => "database schema is newer than supported",
            StoreErrorCode::SchemaMismatch => "database schema mismatched",
            StoreErrorCode::PolicyMismatch => "database runtime policy mismatched",
            StoreErrorCode::InvalidValue => "store input is invalid",
            StoreErrorCode::CapacityExceeded => "store capacity was exceeded",
            StoreErrorCode::InvalidStoredValue => "stored value is invalid",
            StoreErrorCode::StaleCheckpoint => "stored checkpoint changed",
            StoreErrorCode::RebuildRequired => "archive rebuild is required",
            StoreErrorCode::StaleRevision => "replay revision changed",
            StoreErrorCode::AccountingVersionMismatch => "accounting version mismatched",
            StoreErrorCode::IncompleteManifest => "replay source manifest is incomplete",
            StoreErrorCode::UnsealedRevision => "replay revision is not sealed",
            StoreErrorCode::PendingContinuation => "replay continuation is pending",
            StoreErrorCode::ScanInProgress => "a scan set is already running",
            StoreErrorCode::StaleScan => "scan state changed",
            StoreErrorCode::PendingScan => "scan set still has running scopes",
            StoreErrorCode::ArchiveModeMismatch => "archive mode mismatched",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::new(StoreErrorCode::Database)
    }
}
