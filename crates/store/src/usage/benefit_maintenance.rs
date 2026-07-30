use rusqlite::{Transaction, TransactionBehavior, params};
use tokenmaster_benefits::benefit_scope_id;
use tokenmaster_domain::BenefitScope;

use super::{
    BenefitInventoryRevision, BenefitMaintenanceResult, DEFAULT_BENEFIT_CHANGES_PER_SCOPE,
    DEFAULT_BENEFIT_DELIVERIES_PER_SCOPE, MAX_BENEFIT_MAINTENANCE_PAGE_SIZE, UsageStore,
};
use crate::{StoreError, StoreErrorCode};

impl UsageStore {
    pub fn maintain_benefit_history_page(
        &mut self,
        scope: &BenefitScope,
        page_size: u16,
    ) -> Result<BenefitMaintenanceResult, StoreError> {
        self.maintain_benefit_history_page_inner(scope, page_size, BenefitMaintenanceFault::None)
    }

    fn maintain_benefit_history_page_inner(
        &mut self,
        scope: &BenefitScope,
        page_size: u16,
        fault: BenefitMaintenanceFault,
    ) -> Result<BenefitMaintenanceResult, StoreError> {
        if page_size == 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if page_size > MAX_BENEFIT_MAINTENANCE_PAGE_SIZE {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                u64::from(MAX_BENEFIT_MAINTENANCE_PAGE_SIZE),
            ));
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scope_id = benefit_scope_id(scope);
        let scope_bytes = scope_id.as_bytes().as_slice();
        let before = benefit_scope_counts(&transaction, scope_bytes)?;
        let mut remaining_budget = u64::from(page_size);

        let change_budget = before
            .changes
            .saturating_sub(DEFAULT_BENEFIT_CHANGES_PER_SCOPE)
            .min(remaining_budget);
        let change_candidates =
            removable_change_candidates(&transaction, scope_bytes, change_budget)?;
        let examined_changes = change_candidates.len() as u64;
        for change_id in &change_candidates {
            let deleted = transaction.execute(
                "DELETE FROM benefit_change
                 WHERE scope_id = ?1 AND change_id = ?2",
                params![scope_bytes, change_id],
            )?;
            if deleted != 1 {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
        }
        let deleted_changes = examined_changes;
        remaining_budget = remaining_budget.saturating_sub(deleted_changes);
        maintenance_fault(fault, BenefitMaintenanceFault::AfterChangeDelete)?;

        let revision_candidates =
            orphan_revision_candidates(&transaction, scope_bytes, remaining_budget)?;
        for (lot_id, revision) in &revision_candidates {
            let deleted = transaction.execute(
                "DELETE FROM benefit_lot_revision
                 WHERE scope_id = ?1 AND lot_id = ?2 AND lot_revision = ?3",
                params![scope_bytes, lot_id, revision],
            )?;
            if deleted != 1 {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
        }
        let deleted_lot_revisions = revision_candidates.len() as u64;
        remaining_budget = remaining_budget.saturating_sub(deleted_lot_revisions);
        maintenance_fault(fault, BenefitMaintenanceFault::AfterRevisionDelete)?;

        let delivery_budget = before
            .deliveries
            .saturating_sub(DEFAULT_BENEFIT_DELIVERIES_PER_SCOPE)
            .min(remaining_budget);
        let delivery_candidates =
            removable_delivery_candidates(&transaction, scope_bytes, delivery_budget)?;
        let examined_deliveries = delivery_candidates.len() as u64;
        for delivery_id in &delivery_candidates {
            let deleted = transaction.execute(
                "DELETE FROM benefit_reminder_delivery
                 WHERE scope_id = ?1 AND delivery_id = ?2",
                params![scope_bytes, delivery_id],
            )?;
            if deleted != 1 {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
        }
        let deleted_deliveries = examined_deliveries;
        maintenance_fault(fault, BenefitMaintenanceFault::AfterDeliveryDelete)?;

        if deleted_changes != 0 || deleted_deliveries != 0 {
            update_benefit_state(&transaction, deleted_changes, deleted_deliveries, fault)?;
        }
        let remaining = benefit_scope_counts(&transaction, scope_bytes)?;
        transaction.commit()?;
        Ok(BenefitMaintenanceResult::new(
            examined_changes,
            deleted_changes,
            deleted_lot_revisions,
            examined_deliveries,
            deleted_deliveries,
            remaining.changes,
            remaining.deliveries,
        ))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum BenefitMaintenanceFault {
    None,
    AfterChangeDelete,
    AfterRevisionDelete,
    AfterDeliveryDelete,
    AfterState,
}

fn maintenance_fault(
    actual: BenefitMaintenanceFault,
    boundary: BenefitMaintenanceFault,
) -> Result<(), StoreError> {
    if actual == boundary {
        Err(StoreError::new(StoreErrorCode::Database))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct BenefitScopeCounts {
    changes: u64,
    deliveries: u64,
}

fn benefit_scope_counts(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
) -> Result<BenefitScopeCounts, StoreError> {
    let stored = transaction.query_row(
        "SELECT
           (SELECT count(*) FROM benefit_change WHERE scope_id = ?1),
           (SELECT count(*) FROM benefit_reminder_delivery WHERE scope_id = ?1)",
        [scope_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(BenefitScopeCounts {
        changes: stored_count(stored.0)?,
        deliveries: stored_count(stored.1)?,
    })
}

fn removable_change_candidates(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    limit: u64,
) -> Result<Vec<Vec<u8>>, StoreError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let limit =
        i64::try_from(limit).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut statement = transaction.prepare(
        "SELECT older.change_id
         FROM benefit_change AS older
         WHERE older.scope_id = ?1
           AND EXISTS (
             SELECT 1 FROM benefit_change AS newer
             WHERE newer.scope_id = older.scope_id
               AND newer.lot_id = older.lot_id
               AND newer.sequence > older.sequence
           )
         ORDER BY older.sequence, older.change_id
         LIMIT ?2",
    )?;
    Ok(statement
        .query_map(params![scope_id, limit], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn orphan_revision_candidates(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    limit: u64,
) -> Result<Vec<(Vec<u8>, i64)>, StoreError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let limit =
        i64::try_from(limit).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut statement = transaction.prepare(
        "SELECT revision.lot_id, revision.lot_revision
         FROM benefit_lot_revision AS revision
         WHERE revision.scope_id = ?1
           AND NOT EXISTS (
             SELECT 1 FROM benefit_lot_current AS current
             WHERE current.scope_id = revision.scope_id
               AND current.lot_id = revision.lot_id
               AND current.lot_revision = revision.lot_revision
           )
           AND NOT EXISTS (
             SELECT 1 FROM benefit_change AS change
             WHERE change.scope_id = revision.scope_id
               AND change.lot_id = revision.lot_id
               AND (change.before_revision = revision.lot_revision
                 OR change.after_revision = revision.lot_revision)
           )
           AND NOT EXISTS (
             SELECT 1 FROM benefit_reminder_due AS due
             WHERE due.scope_id = revision.scope_id
               AND due.lot_id = revision.lot_id
               AND due.lot_revision = revision.lot_revision
           )
           AND NOT EXISTS (
             SELECT 1 FROM benefit_reminder_delivery AS delivery
             WHERE delivery.scope_id = revision.scope_id
               AND delivery.lot_id = revision.lot_id
               AND delivery.lot_revision = revision.lot_revision
           )
         ORDER BY revision.lot_id, revision.lot_revision
         LIMIT ?2",
    )?;
    Ok(statement
        .query_map(params![scope_id, limit], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

fn removable_delivery_candidates(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    limit: u64,
) -> Result<Vec<Vec<u8>>, StoreError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let limit =
        i64::try_from(limit).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut statement = transaction.prepare(
        "SELECT delivery.delivery_id
         FROM benefit_reminder_delivery AS delivery
         WHERE delivery.scope_id = ?1
           AND EXISTS (
             SELECT 1 FROM benefit_reminder_ack AS acknowledgement
             WHERE acknowledgement.delivery_id = delivery.delivery_id
           )
           AND NOT EXISTS (
             SELECT 1 FROM benefit_lot_current AS current
             WHERE current.scope_id = delivery.scope_id
               AND current.lot_id = delivery.lot_id
               AND current.lot_revision = delivery.lot_revision
           )
         ORDER BY delivery.delivered_at_ms, delivery.delivery_id
         LIMIT ?2",
    )?;
    Ok(statement
        .query_map(params![scope_id, limit], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn update_benefit_state(
    transaction: &Transaction<'_>,
    deleted_changes: u64,
    deleted_deliveries: u64,
    fault: BenefitMaintenanceFault,
) -> Result<(), StoreError> {
    let stored = transaction.query_row(
        "SELECT revision, retained_change_count, retained_delivery_count
         FROM benefit_state WHERE singleton_id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    let revision = BenefitInventoryRevision::from_stored(stored.0)?.next()?;
    let changes = subtract_count(stored.1, deleted_changes)?;
    let deliveries = subtract_count(stored.2, deleted_deliveries)?;
    let updated = transaction.execute(
        "UPDATE benefit_state
         SET revision = ?1,
             retained_change_count = ?2,
             retained_delivery_count = ?3
         WHERE singleton_id = 1",
        params![revision.as_sql(), changes, deliveries],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    maintenance_fault(fault, BenefitMaintenanceFault::AfterState)
}

fn subtract_count(stored: i64, deleted: u64) -> Result<i64, StoreError> {
    let deleted =
        i64::try_from(deleted).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    stored
        .checked_sub(deleted)
        .filter(|value| *value >= 0)
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn stored_count(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

#[cfg(test)]
mod tests {
    use tokenmaster_domain::{
        BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
        BenefitInventoryCompleteness, BenefitInventoryObservation,
        BenefitInventoryObservationParts, BenefitKind, BenefitLabelKey, BenefitLotId,
        BenefitLotObservation, BenefitLotObservationParts, BenefitObservationId, BenefitScope,
        BenefitState, BenefitTarget, QuotaAccountId, UsageProviderId,
    };

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

    fn opaque_id(value: u64) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        bytes[24..].copy_from_slice(&value.to_be_bytes());
        bytes
    }

    fn scope() -> TestResult<BenefitScope> {
        Ok(BenefitScope::new(
            UsageProviderId::new("codex")?,
            QuotaAccountId::new("acct_private")?,
            None,
        ))
    }

    fn observation(revision: u64) -> TestResult<BenefitInventoryObservation> {
        let observed_at_ms = OBSERVED_AT_MS + i64::try_from(revision)?;
        Ok(BenefitInventoryObservation::new(
            BenefitInventoryObservationParts {
                scope: scope()?,
                observation_id: BenefitObservationId::from_bytes(opaque_id(revision)),
                observed_at_ms,
                fresh_until_ms: observed_at_ms + 1_000,
                stale_after_ms: observed_at_ms + 2_000,
                completeness: BenefitInventoryCompleteness::Complete,
                lots: vec![BenefitLotObservation::new(BenefitLotObservationParts {
                    lot_id: BenefitLotId::from_bytes(opaque_id(1)),
                    kind: BenefitKind::BankedRateLimitReset,
                    quantity: revision,
                    state: BenefitState::Available,
                    target: BenefitTarget::Provider,
                    granted_at_ms: None,
                    expiry: BenefitExpiry::unknown(),
                    source: BenefitEvidenceSource::ProviderOfficial,
                    confidence: BenefitConfidence::High,
                    detail_kind: BenefitDetailKind::ProviderDetail,
                    label_key: BenefitLabelKey::new("benefit.codex.banked_reset")?,
                })?],
            },
        )?)
    }

    fn seeded_store() -> TestResult<UsageStore> {
        let mut store = UsageStore::in_memory()?;
        for revision in 1_u64..=514 {
            store.apply_benefit_observation(&observation(revision)?)?;
        }
        let scope_id = benefit_scope_id(&scope()?);
        for delivery in 1_u64..=257 {
            store.connection.execute(
                "INSERT INTO benefit_reminder_delivery(
                   delivery_id, scope_id, lot_id, lot_revision, threshold_seconds,
                   channel, due_at_ms, expiry_at_ms, delivered_at_ms
                 ) VALUES (?1, ?2, ?3, 1, 3600, 'in_app', 1, 2, ?4)",
                params![
                    opaque_id(delivery).as_slice(),
                    scope_id.as_bytes().as_slice(),
                    opaque_id(1).as_slice(),
                    i64::try_from(delivery)?,
                ],
            )?;
            store.connection.execute(
                "INSERT INTO benefit_reminder_ack(delivery_id, acknowledged_at_ms)
                 VALUES (?1, ?2)",
                params![opaque_id(delivery).as_slice(), i64::try_from(delivery)?,],
            )?;
        }
        store.connection.execute(
            "UPDATE benefit_state
             SET retained_delivery_count = 257
             WHERE singleton_id = 1",
            [],
        )?;
        Ok(store)
    }

    fn snapshot(store: &UsageStore) -> TestResult<Vec<i64>> {
        Ok(store.connection.query_row(
            "SELECT
               (SELECT revision FROM benefit_state WHERE singleton_id = 1),
               (SELECT retained_change_count FROM benefit_state WHERE singleton_id = 1),
               (SELECT retained_delivery_count FROM benefit_state WHERE singleton_id = 1),
               (SELECT count(*) FROM benefit_lot_revision),
               (SELECT count(*) FROM benefit_lot_current),
               (SELECT count(*) FROM benefit_change),
               (SELECT count(*) FROM benefit_reminder_delivery),
               (SELECT revision FROM quota_state WHERE singleton_id = 1),
               (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1)",
            [],
            |row| (0..9).map(|index| row.get(index)).collect(),
        )?)
    }

    #[test]
    fn maintenance_faults_roll_back_changes_revisions_deliveries_and_state() -> TestResult {
        for fault in [
            BenefitMaintenanceFault::AfterChangeDelete,
            BenefitMaintenanceFault::AfterRevisionDelete,
            BenefitMaintenanceFault::AfterDeliveryDelete,
            BenefitMaintenanceFault::AfterState,
        ] {
            let mut store = seeded_store()?;
            let before = snapshot(&store)?;
            let error = match store.maintain_benefit_history_page_inner(
                &scope()?,
                MAX_BENEFIT_MAINTENANCE_PAGE_SIZE,
                fault,
            ) {
                Ok(_) => return Err("faulted benefit maintenance unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(snapshot(&store)?, before);
        }
        Ok(())
    }
}
