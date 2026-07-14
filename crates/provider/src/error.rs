use std::fmt;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCode {
    InvalidId,
    InvalidDisplayName,
    TooManyRoots,
    InvalidPath,
    Io,
    CapacityExceeded,
}

impl fmt::Display for ProviderErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::InvalidId => "invalid_id",
            Self::InvalidDisplayName => "invalid_display_name",
            Self::TooManyRoots => "too_many_roots",
            Self::InvalidPath => "invalid_path",
            Self::Io => "io",
            Self::CapacityExceeded => "capacity_exceeded",
        };
        formatter.write_str(code)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("provider error: {code}")]
pub struct ProviderError {
    code: ProviderErrorCode,
    limit: Option<usize>,
}

impl ProviderError {
    pub const fn invalid_path(limit: usize) -> Self {
        Self::with_limit(ProviderErrorCode::InvalidPath, limit)
    }

    pub const fn too_many_roots(limit: usize) -> Self {
        Self::with_limit(ProviderErrorCode::TooManyRoots, limit)
    }

    pub const fn io() -> Self {
        Self {
            code: ProviderErrorCode::Io,
            limit: None,
        }
    }

    pub const fn capacity_exceeded(limit: usize) -> Self {
        Self::with_limit(ProviderErrorCode::CapacityExceeded, limit)
    }

    pub(crate) const fn with_limit(code: ProviderErrorCode, limit: usize) -> Self {
        Self {
            code,
            limit: Some(limit),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ProviderErrorCode {
        self.code
    }

    #[must_use]
    pub const fn limit(&self) -> Option<usize> {
        self.limit
    }
}
