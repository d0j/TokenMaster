use tokenmaster_quota::QuotaTransitionId;

use crate::{StoreError, StoreErrorCode};

pub const DEFAULT_QUOTA_SAMPLES_PER_WINDOW: u64 = 512;
pub const MAX_QUOTA_SAMPLES_PER_WINDOW: u64 = 2_048;
pub const DEFAULT_QUOTA_EPOCHS_PER_WINDOW: u64 = 256;
pub const MAX_QUOTA_EPOCHS_PER_WINDOW: u64 = 1_024;
pub const DEFAULT_QUOTA_TRANSITIONS_PER_WINDOW: u64 = 256;
pub const MAX_QUOTA_TRANSITIONS_PER_WINDOW: u64 = 1_024;
pub const MAX_QUOTA_MAINTENANCE_PAGE_SIZE: u16 = 256;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotaMaintenanceResult {
    examined_samples: u64,
    deleted_samples: u64,
    remaining_samples: u64,
    remaining_closed_epochs: u64,
    remaining_transitions: u64,
}

impl QuotaMaintenanceResult {
    pub(super) const fn new(
        examined_samples: u64,
        deleted_samples: u64,
        remaining_samples: u64,
        remaining_closed_epochs: u64,
        remaining_transitions: u64,
    ) -> Self {
        Self {
            examined_samples,
            deleted_samples,
            remaining_samples,
            remaining_closed_epochs,
            remaining_transitions,
        }
    }

    #[must_use]
    pub const fn examined_samples(self) -> u64 {
        self.examined_samples
    }

    #[must_use]
    pub const fn deleted_samples(self) -> u64 {
        self.deleted_samples
    }

    #[must_use]
    pub const fn remaining_samples(self) -> u64 {
        self.remaining_samples
    }

    #[must_use]
    pub const fn remaining_closed_epochs(self) -> u64 {
        self.remaining_closed_epochs
    }

    #[must_use]
    pub const fn remaining_transitions(self) -> u64 {
        self.remaining_transitions
    }
}
