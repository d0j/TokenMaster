use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use tokenmaster_domain::{BenefitKind, BenefitLabelKey, NotificationChannel, ReminderLeadTime};

use super::UsageStore;
use super::benefit_types::{
    BenefitReminderDelivery, BenefitReminderProcessResult, MAX_BENEFIT_DELIVERIES_PER_SCOPE,
    MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE,
};
use super::benefit_write::{channel_text, fixed_32, input_u64, invalid_stored, parse_kind};
use crate::{StoreError, StoreErrorCode};

#[derive(Clone)]
struct StoredDue {
    delivery_id: [u8; 32],
    scope_id: [u8; 32],
    lot_id: [u8; 32],
    lot_revision: u64,
    lead_time: ReminderLeadTime,
    channel: NotificationChannel,
    due_at_ms: i64,
    expiry_at_ms: i64,
    kind: BenefitKind,
    quantity: u64,
    label_key: Box<str>,
}

impl UsageStore {
    pub fn process_due_benefit_reminders(
        &mut self,
        delivered_at_ms: i64,
        channel: NotificationChannel,
        max_rows: usize,
    ) -> Result<BenefitReminderProcessResult, StoreError> {
        if delivered_at_ms <= 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if max_rows == 0 || max_rows > MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE as u64,
            ));
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let selected = load_due_page(&transaction, delivered_at_ms, channel, max_rows)?;
        let examined_count = output_u16(selected.len())?;
        let expired_count = output_u16(
            selected
                .iter()
                .filter(|due| due.expiry_at_ms <= delivered_at_ms)
                .count(),
        )?;
        let mut candidates = BTreeMap::<([u8; 32], [u8; 32], u64, u8), StoredDue>::new();
        for due in selected
            .iter()
            .filter(|due| due.expiry_at_ms > delivered_at_ms)
        {
            let key = (
                due.scope_id,
                due.lot_id,
                due.lot_revision,
                channel_code(due.channel),
            );
            match candidates.get(&key) {
                Some(current) if current.lead_time.seconds() <= due.lead_time.seconds() => {}
                _ => {
                    candidates.insert(key, due.clone());
                }
            }
        }

        let mut deliveries = Vec::with_capacity(candidates.len());
        let mut scope_delivery_counts = BTreeMap::<[u8; 32], u64>::new();
        for candidate in candidates.into_values() {
            if has_equal_or_more_urgent_receipt(&transaction, &candidate)? {
                continue;
            }
            let retained_for_scope = match scope_delivery_counts.get(&candidate.scope_id) {
                Some(count) => *count,
                None => {
                    let count = count_scope_deliveries(&transaction, &candidate.scope_id)?;
                    scope_delivery_counts.insert(candidate.scope_id, count);
                    count
                }
            };
            let next_count = retained_for_scope
                .checked_add(1)
                .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
            if next_count > MAX_BENEFIT_DELIVERIES_PER_SCOPE {
                return Err(StoreError::with_limit(
                    StoreErrorCode::CapacityExceeded,
                    MAX_BENEFIT_DELIVERIES_PER_SCOPE,
                ));
            }
            insert_receipt(&transaction, &candidate, delivered_at_ms)?;
            scope_delivery_counts.insert(candidate.scope_id, next_count);
            deliveries.push(BenefitReminderDelivery::new(
                candidate.kind,
                candidate.quantity,
                candidate.label_key,
                candidate.lead_time,
                candidate.channel,
                candidate.due_at_ms,
                candidate.expiry_at_ms,
                delivered_at_ms,
            ));
        }

        for due in &selected {
            let changed = transaction.execute(
                "DELETE FROM benefit_reminder_due WHERE delivery_id = ?1",
                [due.delivery_id.as_slice()],
            )?;
            if changed != 1 {
                return Err(invalid_stored());
            }
        }

        let (pending_due_count, retained_delivery_count) =
            update_global_counts(&transaction, selected.len(), deliveries.len())?;
        let nearest_due_at_ms = transaction.query_row(
            "SELECT min(due_at_ms) FROM benefit_reminder_due WHERE channel = ?1",
            [channel_text(channel)],
            |row| row.get::<_, Option<i64>>(0),
        )?;
        let suppressed_count = examined_count
            .checked_sub(expired_count)
            .and_then(|count| count.checked_sub(output_u16(deliveries.len()).ok()?))
            .ok_or_else(invalid_stored)?;
        let result = BenefitReminderProcessResult::new(
            examined_count,
            expired_count,
            suppressed_count,
            deliveries.into_boxed_slice(),
            pending_due_count,
            retained_delivery_count,
            nearest_due_at_ms,
        );
        transaction.commit()?;
        Ok(result)
    }
}

fn load_due_page(
    transaction: &Transaction<'_>,
    delivered_at_ms: i64,
    channel: NotificationChannel,
    max_rows: usize,
) -> Result<Vec<StoredDue>, StoreError> {
    let limit =
        i64::try_from(max_rows).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut statement = transaction.prepare(
        "SELECT due.delivery_id, due.scope_id, due.lot_id, due.lot_revision,
                due.threshold_seconds, due.channel, due.due_at_ms, due.expiry_at_ms,
                revision.kind, revision.quantity, revision.label_key
         FROM benefit_reminder_due AS due
         JOIN benefit_lot_revision AS revision
           ON revision.scope_id = due.scope_id
          AND revision.lot_id = due.lot_id
          AND revision.lot_revision = due.lot_revision
         WHERE due.channel = ?1 AND due.due_at_ms <= ?2
         ORDER BY due.expiry_at_ms, due.scope_id, due.lot_id, due.lot_revision,
                  due.channel, due.threshold_seconds
         LIMIT ?3",
    )?;
    let rows = statement.query_map(
        params![channel_text(channel), delivered_at_ms, limit],
        |row| {
            let stored_channel = parse_channel(&row.get::<_, String>(5)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let lead_time = ReminderLeadTime::new(
                u32::try_from(row.get::<_, i64>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let due_at_ms = row.get::<_, i64>(6)?;
            let expiry_at_ms = row.get::<_, i64>(7)?;
            if due_at_ms >= expiry_at_ms || expiry_at_ms <= 0 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let quantity =
                input_u64(row.get::<_, i64>(9)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
            if quantity == 0 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let label = BenefitLabelKey::new(row.get::<_, String>(10)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(StoredDue {
                delivery_id: fixed_32(row.get::<_, Vec<u8>>(0)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                scope_id: fixed_32(row.get::<_, Vec<u8>>(1)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                lot_id: fixed_32(row.get::<_, Vec<u8>>(2)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                lot_revision: input_u64(row.get::<_, i64>(3)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                lead_time,
                channel: stored_channel,
                due_at_ms,
                expiry_at_ms,
                kind: parse_kind(&row.get::<_, String>(8)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                quantity,
                label_key: Box::from(label.as_str()),
            })
        },
    )?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid_stored())
}

fn has_equal_or_more_urgent_receipt(
    transaction: &Transaction<'_>,
    candidate: &StoredDue,
) -> Result<bool, StoreError> {
    Ok(transaction
        .query_row(
            "SELECT 1
             FROM benefit_reminder_delivery
             WHERE scope_id = ?1
               AND lot_id = ?2
               AND lot_revision = ?3
               AND channel = ?4
               AND threshold_seconds <= ?5
             LIMIT 1",
            params![
                candidate.scope_id.as_slice(),
                candidate.lot_id.as_slice(),
                input_i64(candidate.lot_revision)?,
                channel_text(candidate.channel),
                i64::from(candidate.lead_time.seconds()),
            ],
            |_row| Ok(()),
        )
        .optional()?
        .is_some())
}

fn count_scope_deliveries(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
) -> Result<u64, StoreError> {
    let count = transaction.query_row(
        "SELECT count(*) FROM benefit_reminder_delivery WHERE scope_id = ?1",
        [scope_id.as_slice()],
        |row| row.get::<_, i64>(0),
    )?;
    input_u64(count)
}

fn insert_receipt(
    transaction: &Transaction<'_>,
    candidate: &StoredDue,
    delivered_at_ms: i64,
) -> Result<(), StoreError> {
    let changed = transaction.execute(
        "INSERT INTO benefit_reminder_delivery(
           delivery_id, scope_id, lot_id, lot_revision, threshold_seconds,
           channel, due_at_ms, expiry_at_ms, delivered_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            candidate.delivery_id.as_slice(),
            candidate.scope_id.as_slice(),
            candidate.lot_id.as_slice(),
            input_i64(candidate.lot_revision)?,
            i64::from(candidate.lead_time.seconds()),
            channel_text(candidate.channel),
            candidate.due_at_ms,
            candidate.expiry_at_ms,
            delivered_at_ms,
        ],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(invalid_stored())
    }
}

fn update_global_counts(
    transaction: &Transaction<'_>,
    deleted_due_count: usize,
    inserted_delivery_count: usize,
) -> Result<(u64, u64), StoreError> {
    let (pending, retained) = transaction
        .query_row(
            "SELECT pending_due_count, retained_delivery_count
             FROM benefit_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .ok_or_else(invalid_stored)?;
    let pending = input_u64(pending)?;
    let retained = input_u64(retained)?;
    let pending = pending
        .checked_sub(
            u64::try_from(deleted_due_count)
                .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?,
        )
        .ok_or_else(invalid_stored)?;
    let retained = retained
        .checked_add(
            u64::try_from(inserted_delivery_count)
                .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?,
        )
        .filter(|count| *count <= i64::MAX as u64)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let changed = transaction.execute(
        "UPDATE benefit_state
         SET pending_due_count = ?1, retained_delivery_count = ?2
         WHERE singleton_id = 1",
        params![input_i64(pending)?, input_i64(retained)?],
    )?;
    if changed != 1 {
        return Err(invalid_stored());
    }
    Ok((pending, retained))
}

fn parse_channel(value: &str) -> Result<NotificationChannel, StoreError> {
    match value {
        "in_app" => Ok(NotificationChannel::InApp),
        "os_scheduled" => Ok(NotificationChannel::OsScheduled),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

const fn channel_code(channel: NotificationChannel) -> u8 {
    match channel {
        NotificationChannel::InApp => 1,
        NotificationChannel::OsScheduled => 2,
    }
}

fn input_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn output_u16(value: usize) -> Result<u16, StoreError> {
    u16::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}
