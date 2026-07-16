use tokenmaster_domain::{BenefitKind, NotificationChannel, ReminderLeadTime};

use crate::{StoreError, StoreErrorCode};

pub const DEFAULT_BENEFIT_CHANGES_PER_SCOPE: u64 = 512;
pub const MAX_BENEFIT_CHANGES_PER_SCOPE: u64 = 2_048;
pub const DEFAULT_BENEFIT_DELIVERIES_PER_SCOPE: u64 = 256;
pub const MAX_BENEFIT_DELIVERIES_PER_SCOPE: u64 = 1_024;
pub const MAX_BENEFIT_MAINTENANCE_PAGE_SIZE: u16 = 256;
pub const MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE: usize =
    tokenmaster_benefits::MAX_DUE_REMINDER_PAGE_SIZE;

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

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitReminderDelivery {
    delivery_id: [u8; 32],
    kind: BenefitKind,
    quantity: u64,
    label_key: Box<str>,
    lead_time: ReminderLeadTime,
    channel: NotificationChannel,
    due_at_ms: i64,
    expiry_at_ms: i64,
    delivered_at_ms: i64,
}

impl BenefitReminderDelivery {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        delivery_id: [u8; 32],
        kind: BenefitKind,
        quantity: u64,
        label_key: Box<str>,
        lead_time: ReminderLeadTime,
        channel: NotificationChannel,
        due_at_ms: i64,
        expiry_at_ms: i64,
        delivered_at_ms: i64,
    ) -> Self {
        Self {
            delivery_id,
            kind,
            quantity,
            label_key,
            lead_time,
            channel,
            due_at_ms,
            expiry_at_ms,
            delivered_at_ms,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> BenefitKind {
        self.kind
    }

    #[must_use]
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }

    #[must_use]
    pub fn label_key(&self) -> &str {
        &self.label_key
    }

    #[must_use]
    pub const fn lead_time(&self) -> ReminderLeadTime {
        self.lead_time
    }

    #[must_use]
    pub const fn channel(&self) -> NotificationChannel {
        self.channel
    }

    #[must_use]
    pub const fn due_at_ms(&self) -> i64 {
        self.due_at_ms
    }

    #[must_use]
    pub const fn expiry_at_ms(&self) -> i64 {
        self.expiry_at_ms
    }

    #[must_use]
    pub const fn delivered_at_ms(&self) -> i64 {
        self.delivered_at_ms
    }

    pub(super) const fn delivery_id(&self) -> &[u8; 32] {
        &self.delivery_id
    }
}

impl core::fmt::Debug for BenefitReminderDelivery {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BenefitReminderDelivery")
            .field("kind", &self.kind)
            .field("quantity", &self.quantity)
            .field("label_key", &self.label_key)
            .field("lead_time", &self.lead_time)
            .field("channel", &self.channel)
            .field("due_at_ms", &self.due_at_ms)
            .field("expiry_at_ms", &self.expiry_at_ms)
            .field("delivered_at_ms", &self.delivered_at_ms)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenefitReminderAcknowledgeResult {
    acknowledged_count: u16,
    already_acknowledged_count: u16,
}

impl BenefitReminderAcknowledgeResult {
    pub(super) const fn new(acknowledged_count: u16, already_acknowledged_count: u16) -> Self {
        Self {
            acknowledged_count,
            already_acknowledged_count,
        }
    }

    #[must_use]
    pub const fn acknowledged_count(self) -> u16 {
        self.acknowledged_count
    }

    #[must_use]
    pub const fn already_acknowledged_count(self) -> u16 {
        self.already_acknowledged_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitReminderProcessResult {
    examined_count: u16,
    expired_count: u16,
    suppressed_count: u16,
    deliveries: Box<[BenefitReminderDelivery]>,
    pending_due_count: u64,
    retained_delivery_count: u64,
    nearest_due_at_ms: Option<i64>,
}

impl BenefitReminderProcessResult {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        examined_count: u16,
        expired_count: u16,
        suppressed_count: u16,
        deliveries: Box<[BenefitReminderDelivery]>,
        pending_due_count: u64,
        retained_delivery_count: u64,
        nearest_due_at_ms: Option<i64>,
    ) -> Self {
        Self {
            examined_count,
            expired_count,
            suppressed_count,
            deliveries,
            pending_due_count,
            retained_delivery_count,
            nearest_due_at_ms,
        }
    }

    #[must_use]
    pub const fn examined_count(&self) -> u16 {
        self.examined_count
    }

    #[must_use]
    pub const fn expired_count(&self) -> u16 {
        self.expired_count
    }

    #[must_use]
    pub const fn suppressed_count(&self) -> u16 {
        self.suppressed_count
    }

    #[must_use]
    pub fn delivery_count(&self) -> usize {
        self.deliveries.len()
    }

    #[must_use]
    pub const fn deliveries(&self) -> &[BenefitReminderDelivery] {
        &self.deliveries
    }

    #[must_use]
    pub const fn pending_due_count(&self) -> u64 {
        self.pending_due_count
    }

    #[must_use]
    pub const fn retained_delivery_count(&self) -> u64 {
        self.retained_delivery_count
    }

    #[must_use]
    pub const fn nearest_due_at_ms(&self) -> Option<i64> {
        self.nearest_due_at_ms
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
