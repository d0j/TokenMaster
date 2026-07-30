use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineErrorCode {
    InvalidValue,
    CapacityExceeded,
    StaleRequest,
}

impl fmt::Display for EngineErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidValue => "invalid_value",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::StaleRequest => "stale_request",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{code}")]
pub struct EngineError {
    code: EngineErrorCode,
}

impl EngineError {
    pub(crate) const fn new(code: EngineErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> EngineErrorCode {
        self.code
    }
}
