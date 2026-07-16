use tokenmaster_quota::QuotaTransitionId;

use crate::{StoreError, StoreErrorCode};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct QuotaRevision(u64);

impl QuotaRevision {
    pub(super) fn from_stored(value: i64) -> Result<Self, StoreError> {
        let value = u64::try_from(value)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        Ok(Self(value))
    }

    pub(super) fn next(self) -> Result<Self, StoreError> {
        let value = self
            .0
            .checked_add(1)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        Ok(Self(value))
    }

    pub(super) fn as_sql(self) -> i64 {
        self.0 as i64
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaApplyStatus {
    Started,
    Duplicate,
    Stale,
    Advanced,
    AllowanceChanged,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotaApplyResult {
    status: QuotaApplyStatus,
    quota_revision: QuotaRevision,
    transition_sequence: u64,
    transition_id: Option<QuotaTransitionId>,
}

impl QuotaApplyResult {
    pub(super) const fn new(
        status: QuotaApplyStatus,
        quota_revision: QuotaRevision,
        transition_sequence: u64,
        transition_id: Option<QuotaTransitionId>,
    ) -> Self {
        Self {
            status,
            quota_revision,
            transition_sequence,
            transition_id,
        }
    }

    #[must_use]
    pub const fn status(self) -> QuotaApplyStatus {
        self.status
    }

    #[must_use]
    pub const fn quota_revision(self) -> QuotaRevision {
        self.quota_revision
    }

    #[must_use]
    pub const fn transition_sequence(self) -> u64 {
        self.transition_sequence
    }

    #[must_use]
    pub const fn transition_id(self) -> Option<QuotaTransitionId> {
        self.transition_id
    }
}
