#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod error;
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Task 3 record core is consumed by Task 4 typed stores"
    )
)]
mod record;
#[cfg(test)]
mod record_contract_tests;

pub use error::{StateError, StateErrorCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteLimit(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ItemLimit(usize);

/// Immutable byte and item limits for bounded reliable-state inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateLimits {
    bytes: ByteLimit,
    items: ItemLimit,
}

impl StateLimits {
    /// Creates an exact inclusive byte/item limit pair.
    #[must_use]
    pub const fn new(max_bytes: u64, max_items: usize) -> Self {
        Self {
            bytes: ByteLimit(max_bytes),
            items: ItemLimit(max_items),
        }
    }

    /// Returns the inclusive byte limit.
    #[must_use]
    pub const fn max_bytes(self) -> u64 {
        self.bytes.0
    }

    /// Returns the inclusive item limit.
    #[must_use]
    pub const fn max_items(self) -> usize {
        self.items.0
    }

    /// Adds byte counts without overflow and rejects values above the limit.
    pub fn checked_bytes(self, current: u64, additional: u64) -> Result<u64, StateError> {
        let total = current
            .checked_add(additional)
            .ok_or_else(StateError::capacity_exceeded)?;
        if total > self.bytes.0 {
            return Err(StateError::capacity_exceeded());
        }
        Ok(total)
    }

    /// Adds item counts without overflow and rejects values above the limit.
    pub fn checked_items(self, current: usize, additional: usize) -> Result<usize, StateError> {
        let total = current
            .checked_add(additional)
            .ok_or_else(StateError::capacity_exceeded)?;
        if total > self.items.0 {
            return Err(StateError::capacity_exceeded());
        }
        Ok(total)
    }
}
