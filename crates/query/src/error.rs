use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryErrorCode {
    InvalidValue,
    CapacityExceeded,
    Unavailable,
    VersionMismatch,
    StaleSnapshot,
    DeadlineExceeded,
    CorruptArchive,
    Overflow,
    Internal,
}

impl QueryErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidValue => "invalid_value",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::Unavailable => "unavailable",
            Self::VersionMismatch => "version_mismatch",
            Self::StaleSnapshot => "stale_snapshot",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::CorruptArchive => "corrupt_archive",
            Self::Overflow => "overflow",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for QueryErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{code}")]
pub struct QueryError {
    code: QueryErrorCode,
}

impl QueryError {
    pub(crate) const fn new(code: QueryErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> QueryErrorCode {
        self.code
    }
}
