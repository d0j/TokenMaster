#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod identity;
mod reconcile;
mod reminder;

pub use identity::{
    BenefitChangeId, BenefitScopeId, ReminderDeliveryId, benefit_scope_id, reminder_delivery_id,
};
pub use reconcile::{
    BenefitChange, BenefitChangeKind, BenefitCoreError, BenefitCurrentLot, BenefitInventoryState,
    BenefitReconciliation, BenefitReconciliationStatus, BenefitRevision, BenefitSequence,
    reconcile_inventory,
};
pub use reminder::{
    MAX_DUE_REMINDER_PAGE_SIZE, MAX_SCHEDULED_REMINDERS, ReminderDue, collapse_due_reminders,
    schedule_reminders,
};
