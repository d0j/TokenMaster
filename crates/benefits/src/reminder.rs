use std::collections::BTreeMap;

use tokenmaster_domain::{
    BenefitLotId, BenefitScope, BenefitState, NotificationChannel, ReminderLeadTime,
    ReminderProfile,
};

use crate::identity::{ReminderDeliveryId, notification_channel_code};
use crate::{BenefitCoreError, BenefitCurrentLot, BenefitRevision, reminder_delivery_id};

pub const MAX_SCHEDULED_REMINDERS: usize = tokenmaster_domain::MAX_BENEFIT_LOTS_PER_OBSERVATION
    * tokenmaster_domain::MAX_REMINDER_THRESHOLDS
    * 2;
pub const MAX_DUE_REMINDER_PAGE_SIZE: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReminderDue {
    delivery_id: ReminderDeliveryId,
    lot_id: BenefitLotId,
    lot_revision: BenefitRevision,
    lead_time: ReminderLeadTime,
    channel: NotificationChannel,
    due_at_ms: i64,
    expiry_at_ms: i64,
}

impl ReminderDue {
    #[must_use]
    pub const fn delivery_id(&self) -> ReminderDeliveryId {
        self.delivery_id
    }

    #[must_use]
    pub const fn lot_id(&self) -> BenefitLotId {
        self.lot_id
    }

    #[must_use]
    pub const fn lot_revision(&self) -> BenefitRevision {
        self.lot_revision
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
}

pub fn schedule_reminders(
    scope: &BenefitScope,
    lots: &[BenefitCurrentLot],
    profile: &ReminderProfile,
) -> Result<Vec<ReminderDue>, BenefitCoreError> {
    let capacity = lots
        .len()
        .checked_mul(profile.lead_times().len())
        .and_then(|value| value.checked_mul(profile.channels().len()))
        .ok_or(BenefitCoreError::CapacityExceeded)?;
    if capacity > MAX_SCHEDULED_REMINDERS {
        return Err(BenefitCoreError::CapacityExceeded);
    }
    let mut scheduled = Vec::with_capacity(capacity);
    for current in lots {
        let lot = current.lot();
        if lot.state() != BenefitState::Available {
            continue;
        }
        let Some(expiry_at_ms) = lot.expiry().conservative_utc_ms() else {
            continue;
        };
        for lead_time in profile.lead_times() {
            let lead_millis = i64::from(lead_time.seconds())
                .checked_mul(1_000)
                .ok_or(BenefitCoreError::InvalidTime)?;
            let due_at_ms = expiry_at_ms.checked_sub(lead_millis).unwrap_or(i64::MIN);
            for channel in profile.channels() {
                scheduled.push(ReminderDue {
                    delivery_id: reminder_delivery_id(
                        scope,
                        lot.lot_id(),
                        current.revision(),
                        *lead_time,
                        *channel,
                    ),
                    lot_id: lot.lot_id(),
                    lot_revision: current.revision(),
                    lead_time: *lead_time,
                    channel: *channel,
                    due_at_ms,
                    expiry_at_ms,
                });
            }
        }
    }
    scheduled.sort_unstable_by(|left, right| {
        left.due_at_ms
            .cmp(&right.due_at_ms)
            .then_with(|| left.expiry_at_ms.cmp(&right.expiry_at_ms))
            .then_with(|| left.lot_id.as_bytes().cmp(right.lot_id.as_bytes()))
            .then_with(|| {
                notification_channel_code(left.channel)
                    .cmp(&notification_channel_code(right.channel))
            })
    });
    Ok(scheduled)
}

pub fn collapse_due_reminders(
    scheduled: &[ReminderDue],
    now_ms: i64,
    max_rows: usize,
) -> Result<Vec<ReminderDue>, BenefitCoreError> {
    if now_ms <= 0 || max_rows == 0 || max_rows > MAX_DUE_REMINDER_PAGE_SIZE {
        return Err(if now_ms <= 0 {
            BenefitCoreError::InvalidTime
        } else {
            BenefitCoreError::CapacityExceeded
        });
    }
    let mut selected = BTreeMap::<([u8; 32], u8), ReminderDue>::new();
    for entry in scheduled {
        if entry.due_at_ms > now_ms || entry.expiry_at_ms <= now_ms {
            continue;
        }
        let key = (
            *entry.lot_id.as_bytes(),
            notification_channel_code(entry.channel),
        );
        match selected.get(&key) {
            Some(current) if current.lead_time.seconds() <= entry.lead_time.seconds() => {}
            _ => {
                selected.insert(key, entry.clone());
            }
        }
    }
    let mut due = selected.into_values().collect::<Vec<_>>();
    due.sort_unstable_by(|left, right| {
        left.expiry_at_ms
            .cmp(&right.expiry_at_ms)
            .then_with(|| left.lot_id.as_bytes().cmp(right.lot_id.as_bytes()))
            .then_with(|| {
                notification_channel_code(left.channel)
                    .cmp(&notification_channel_code(right.channel))
            })
    });
    due.truncate(max_rows);
    Ok(due)
}
