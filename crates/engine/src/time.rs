use crate::{EngineError, EngineErrorCode};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicTime(u64);

impl MonotonicTime {
    #[must_use]
    pub const fn from_millis(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RefreshDeadline(u64);

impl RefreshDeadline {
    #[must_use]
    pub const fn from_millis(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_exceeded_at(self, now: MonotonicTime) -> bool {
        now.0 >= self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RefreshRequestId(u64);

impl RefreshRequestId {
    pub const fn new(value: u64) -> Result<Self, EngineError> {
        if value == 0 {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
