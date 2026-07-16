use crate::{StoreError, StoreErrorCode};

pub const DEFAULT_BENEFIT_CHANGES_PER_SCOPE: u64 = 512;
pub const MAX_BENEFIT_CHANGES_PER_SCOPE: u64 = 2_048;
pub const DEFAULT_BENEFIT_DELIVERIES_PER_SCOPE: u64 = 256;
pub const MAX_BENEFIT_DELIVERIES_PER_SCOPE: u64 = 1_024;
pub const MAX_BENEFIT_MAINTENANCE_PAGE_SIZE: u16 = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BenefitInventoryRevision(u64);

impl BenefitInventoryRevision {
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

    pub(super) const fn as_sql(self) -> i64 {
        self.0 as i64
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitApplyStatus {
    Duplicate,
    Stale,
    FreshnessOnly,
    Changed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenefitApplyResult {
    status: BenefitApplyStatus,
    benefit_revision: BenefitInventoryRevision,
    change_count: u16,
    pending_due_count: u16,
}

impl BenefitApplyResult {
    pub(super) const fn new(
        status: BenefitApplyStatus,
        benefit_revision: BenefitInventoryRevision,
        change_count: u16,
        pending_due_count: u16,
    ) -> Self {
        Self {
            status,
            benefit_revision,
            change_count,
            pending_due_count,
        }
    }

    #[must_use]
    pub const fn status(self) -> BenefitApplyStatus {
        self.status
    }

    #[must_use]
    pub const fn benefit_revision(self) -> BenefitInventoryRevision {
        self.benefit_revision
    }

    #[must_use]
    pub const fn change_count(self) -> u16 {
        self.change_count
    }

    #[must_use]
    pub const fn pending_due_count(self) -> u16 {
        self.pending_due_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenefitProfileApplyResult {
    benefit_revision: BenefitInventoryRevision,
    pending_due_count: u16,
}

impl BenefitProfileApplyResult {
    pub(super) const fn new(
        benefit_revision: BenefitInventoryRevision,
        pending_due_count: u16,
    ) -> Self {
        Self {
            benefit_revision,
            pending_due_count,
        }
    }

    #[must_use]
    pub const fn benefit_revision(self) -> BenefitInventoryRevision {
        self.benefit_revision
    }

    #[must_use]
    pub const fn pending_due_count(self) -> u16 {
        self.pending_due_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenefitMaintenanceResult {
    examined_changes: u64,
    deleted_changes: u64,
    deleted_lot_revisions: u64,
    examined_deliveries: u64,
    deleted_deliveries: u64,
    remaining_changes: u64,
    remaining_deliveries: u64,
}

impl BenefitMaintenanceResult {
    pub(super) const fn new(
        examined_changes: u64,
        deleted_changes: u64,
        deleted_lot_revisions: u64,
        examined_deliveries: u64,
        deleted_deliveries: u64,
        remaining_changes: u64,
        remaining_deliveries: u64,
    ) -> Self {
        Self {
            examined_changes,
            deleted_changes,
            deleted_lot_revisions,
            examined_deliveries,
            deleted_deliveries,
            remaining_changes,
            remaining_deliveries,
        }
    }

    #[must_use]
    pub const fn examined_changes(self) -> u64 {
        self.examined_changes
    }

    #[must_use]
    pub const fn deleted_changes(self) -> u64 {
        self.deleted_changes
    }

    #[must_use]
    pub const fn deleted_lot_revisions(self) -> u64 {
        self.deleted_lot_revisions
    }

    #[must_use]
    pub const fn examined_deliveries(self) -> u64 {
        self.examined_deliveries
    }

    #[must_use]
    pub const fn deleted_deliveries(self) -> u64 {
        self.deleted_deliveries
    }

    #[must_use]
    pub const fn remaining_changes(self) -> u64 {
        self.remaining_changes
    }

    #[must_use]
    pub const fn remaining_deliveries(self) -> u64 {
        self.remaining_deliveries
    }

    #[must_use]
    pub const fn total_deleted(self) -> u64 {
        self.deleted_changes
            .saturating_add(self.deleted_lot_revisions)
            .saturating_add(self.deleted_deliveries)
    }
}
