use core::fmt;

use serde::Serialize;

/// Stable, path-private failure categories for reliable-state operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StateErrorCode {
    InvalidInput,
    UnsupportedVersion,
    CapacityExceeded,
    Integrity,
    Unavailable,
    Busy,
    DiskCapacity,
    RecoveryRequired,
    InternalInvariant,
}

impl StateErrorCode {
    /// Returns the stable machine-readable error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::UnsupportedVersion => "unsupported_version",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::Integrity => "integrity",
            Self::Unavailable => "unavailable",
            Self::Busy => "busy",
            Self::DiskCapacity => "disk_capacity",
            Self::RecoveryRequired => "recovery_required",
            Self::InternalInvariant => "internal_invariant",
        }
    }
}

impl fmt::Display for StateErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Fixed reliable-state error containing no source text, path, or private payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("state error: {code}")]
pub struct StateError {
    code: StateErrorCode,
}

impl StateError {
    /// Constructs a path-private error from one stable category.
    #[must_use]
    pub const fn from_code(code: StateErrorCode) -> Self {
        Self { code }
    }

    /// Returns the stable machine-readable category.
    #[must_use]
    pub const fn code(self) -> StateErrorCode {
        self.code
    }

    pub(crate) const fn capacity_exceeded() -> Self {
        Self::from_code(StateErrorCode::CapacityExceeded)
    }
}
