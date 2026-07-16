use rusqlite::{Transaction, TransactionBehavior, params};
use tokenmaster_domain::{QuotaSample, QuotaWindowKey};
use tokenmaster_quota::quota_scope_id;

use super::{
    DEFAULT_QUOTA_SAMPLES_PER_WINDOW, MAX_QUOTA_EPOCHS_PER_WINDOW, MAX_QUOTA_MAINTENANCE_PAGE_SIZE,
    MAX_QUOTA_SAMPLES_PER_WINDOW, MAX_QUOTA_TRANSITIONS_PER_WINDOW, QuotaMaintenanceResult,
    UsageStore,
};
use crate::{StoreError, StoreErrorCode};

impl UsageStore {
    pub fn maintain_quota_history_page(
        &mut self,
        window: &QuotaWindowKey,
        page_size: u16,
    ) -> Result<QuotaMaintenanceResult, StoreError> {
        self.maintain_quota_history_page_inner(window, page_size, QuotaMaintenanceFault::None)
    }

    fn maintain_quota_history_page_inner(
        &mut self,
        window: &QuotaWindowKey,
        page_size: u16,
        fault: QuotaMaintenanceFault,
    ) -> Result<QuotaMaintenanceResult, StoreError> {
        if page_size == 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if page_size > MAX_QUOTA_MAINTENANCE_PAGE_SIZE {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                u64::from(MAX_QUOTA_MAINTENANCE_PAGE_SIZE),
            ));
        }

        let scope_id = quota_scope_id(window.scope());
        let scope_bytes = scope_id.as_bytes().as_slice();
        let window_id = window.window_id().as_str();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = quota_window_counts(&transaction, scope_bytes, window_id)?;
        let delete_budget = before
            .samples
            .saturating_sub(DEFAULT_QUOTA_SAMPLES_PER_WINDOW)
            .min(u64::from(page_size));
        let candidates =
            redundant_sample_candidates(&transaction, scope_bytes, window_id, delete_budget)?;
        let examined_samples = candidates.len() as u64;
        let mut deleted_samples = 0_u64;
        for observation_id in candidates {
            if delete_unprotected_sample(&transaction, scope_bytes, window_id, &observation_id)? {
                deleted_samples = deleted_samples
                    .checked_add(1)
                    .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
            } else {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
        }
        quota_maintenance_fault(fault, QuotaMaintenanceFault::AfterDelete)?;

        if deleted_samples != 0 {
            let deleted = i64::try_from(deleted_samples)
                .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
            let updated = transaction.execute(
                "UPDATE quota_state
                 SET retained_sample_count = retained_sample_count - ?1
                 WHERE singleton_id = 1 AND retained_sample_count >= ?1",
                params![deleted],
            )?;
            if updated != 1 {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
        }
        quota_maintenance_fault(fault, QuotaMaintenanceFault::AfterState)?;

        let remaining = quota_window_counts(&transaction, scope_bytes, window_id)?;
        transaction.commit()?;
        Ok(QuotaMaintenanceResult::new(
            examined_samples,
            deleted_samples,
            remaining.samples,
            remaining.closed_epochs,
            remaining.transitions,
        ))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum QuotaMaintenanceFault {
    None,
    AfterDelete,
    AfterState,
}

fn quota_maintenance_fault(
    actual: QuotaMaintenanceFault,
    boundary: QuotaMaintenanceFault,
) -> Result<(), StoreError> {
    if actual == boundary {
        Err(StoreError::new(StoreErrorCode::Database))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) struct QuotaWindowCounts {
    pub(super) samples: u64,
    pub(super) closed_epochs: u64,
    pub(super) transitions: u64,
}

pub(super) fn quota_window_counts(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
) -> Result<QuotaWindowCounts, StoreError> {
    let stored = transaction.query_row(
        "SELECT
           (SELECT count(*) FROM quota_sample
             WHERE scope_id = ?1 AND window_id = ?2),
           (SELECT count(*) FROM quota_epoch_history
             WHERE scope_id = ?1 AND window_id = ?2),
           (SELECT count(*) FROM quota_transition
             WHERE scope_id = ?1 AND window_id = ?2)",
        params![scope_id, window_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    Ok(QuotaWindowCounts {
        samples: stored_count(stored.0)?,
        closed_epochs: stored_count(stored.1)?,
        transitions: stored_count(stored.2)?,
    })
}

pub(super) fn enforce_quota_window_hard_caps(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
) -> Result<(), StoreError> {
    let counts = quota_window_counts(transaction, scope_id, window_id)?;
    if counts.samples > MAX_QUOTA_SAMPLES_PER_WINDOW {
        return Err(StoreError::with_limit(
            StoreErrorCode::CapacityExceeded,
            MAX_QUOTA_SAMPLES_PER_WINDOW,
        ));
    }
    if counts.transitions > MAX_QUOTA_TRANSITIONS_PER_WINDOW {
        return Err(StoreError::with_limit(
            StoreErrorCode::CapacityExceeded,
            MAX_QUOTA_TRANSITIONS_PER_WINDOW,
        ));
    }
    if counts.closed_epochs > MAX_QUOTA_EPOCHS_PER_WINDOW {
        return Err(StoreError::with_limit(
            StoreErrorCode::CapacityExceeded,
            MAX_QUOTA_EPOCHS_PER_WINDOW,
        ));
    }
    Ok(())
}

pub(super) fn samples_are_redundant(previous: &QuotaSample, next: &QuotaSample) -> bool {
    previous.key() == next.key()
        && previous.provider_epoch_id() == next.provider_epoch_id()
        && previous.used_ratio() == next.used_ratio()
        && previous.remaining_ratio() == next.remaining_ratio()
        && previous.units() == next.units()
        && previous.advertised_resets_at_ms() == next.advertised_resets_at_ms()
        && previous.quality() == next.quality()
        && previous.source() == next.source()
        && previous.confidence() == next.confidence()
        && previous.reset_evidence() == next.reset_evidence()
        && previous.reset_occurred_at_ms() == next.reset_occurred_at_ms()
}

pub(super) fn delete_unprotected_sample(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    observation_id: &[u8],
) -> Result<bool, StoreError> {
    let deleted = transaction.execute(
        "DELETE FROM quota_sample
         WHERE observation_id = ?1
           AND scope_id = ?2
           AND window_id = ?3
           AND NOT EXISTS (
             SELECT 1 FROM quota_window_current
             WHERE scope_id = ?2 AND window_id = ?3
               AND sample_observation_id = ?1
           )
           AND NOT EXISTS (
             SELECT 1 FROM quota_epoch_current
             WHERE scope_id = ?2 AND window_id = ?3
               AND (first_observation_id = ?1
                 OR last_observation_id = ?1
                 OR maximum_used_ratio_observation_id = ?1
                 OR maximum_used_units_observation_id = ?1)
           )
           AND NOT EXISTS (
             SELECT 1 FROM quota_epoch_history
             WHERE scope_id = ?2 AND window_id = ?3
               AND (first_observation_id = ?1
                 OR last_observation_id = ?1
                 OR maximum_used_ratio_observation_id = ?1
                 OR maximum_used_units_observation_id = ?1)
           )
           AND NOT EXISTS (
             SELECT 1 FROM quota_transition
             WHERE scope_id = ?2 AND window_id = ?3
               AND (pre_observation_id = ?1
                 OR post_observation_id = ?1
                 OR maximum_used_ratio_observation_id = ?1
                 OR maximum_used_units_observation_id = ?1)
           )",
        params![observation_id, scope_id, window_id],
    )?;
    Ok(deleted == 1)
}

fn redundant_sample_candidates(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    limit: u64,
) -> Result<Vec<Vec<u8>>, StoreError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let limit =
        i64::try_from(limit).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut statement = transaction.prepare(
        "SELECT older.observation_id
         FROM quota_sample AS older
         WHERE older.scope_id = ?1
           AND older.window_id = ?2
           AND EXISTS (
             SELECT 1
             FROM quota_sample AS newer
             WHERE newer.scope_id = older.scope_id
               AND newer.window_id = older.window_id
               AND newer.definition_revision = older.definition_revision
               AND (newer.observed_at_ms > older.observed_at_ms
                 OR (newer.observed_at_ms = older.observed_at_ms
                   AND newer.observation_id > older.observation_id))
               AND newer.provider_epoch_id IS older.provider_epoch_id
               AND newer.used_ratio_ppm IS older.used_ratio_ppm
               AND newer.remaining_ratio_ppm IS older.remaining_ratio_ppm
               AND newer.unit_id IS older.unit_id
               AND newer.used_units IS older.used_units
               AND newer.remaining_units IS older.remaining_units
               AND newer.capacity_units IS older.capacity_units
               AND newer.advertised_resets_at_ms IS older.advertised_resets_at_ms
               AND newer.quality IS older.quality
               AND newer.source IS older.source
               AND newer.confidence IS older.confidence
               AND newer.reset_evidence IS older.reset_evidence
               AND newer.reset_occurred_at_ms IS older.reset_occurred_at_ms
           )
           AND NOT EXISTS (
             SELECT 1 FROM quota_window_current
             WHERE scope_id = older.scope_id AND window_id = older.window_id
               AND sample_observation_id = older.observation_id
           )
           AND NOT EXISTS (
             SELECT 1 FROM quota_epoch_current
             WHERE scope_id = older.scope_id AND window_id = older.window_id
               AND (first_observation_id = older.observation_id
                 OR last_observation_id = older.observation_id
                 OR maximum_used_ratio_observation_id = older.observation_id
                 OR maximum_used_units_observation_id = older.observation_id)
           )
           AND NOT EXISTS (
             SELECT 1 FROM quota_epoch_history
             WHERE scope_id = older.scope_id AND window_id = older.window_id
               AND (first_observation_id = older.observation_id
                 OR last_observation_id = older.observation_id
                 OR maximum_used_ratio_observation_id = older.observation_id
                 OR maximum_used_units_observation_id = older.observation_id)
           )
           AND NOT EXISTS (
             SELECT 1 FROM quota_transition
             WHERE scope_id = older.scope_id AND window_id = older.window_id
               AND (pre_observation_id = older.observation_id
                 OR post_observation_id = older.observation_id
                 OR maximum_used_ratio_observation_id = older.observation_id
                 OR maximum_used_units_observation_id = older.observation_id)
           )
         ORDER BY older.observed_at_ms, older.observation_id
         LIMIT ?3",
    )?;
    let rows = statement.query_map(params![scope_id, window_id, limit], |row| row.get(0))?;
    let candidates = rows.collect::<Result<Vec<Vec<u8>>, _>>()?;
    Ok(candidates)
}

fn stored_count(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

#[cfg(test)]
mod tests {
    use tokenmaster_domain::{
        QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
        QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
        QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaWindowDefinition,
        QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowSemantics, UsageProviderId,
    };

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn definition() -> TestResult<QuotaWindowDefinition> {
        Ok(QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
            key: QuotaWindowKey::new(
                QuotaScope::new(
                    UsageProviderId::new("codex")?,
                    QuotaAccountId::new("personal")?,
                    None,
                ),
                QuotaWindowId::new("weekly")?,
            ),
            revision: 1,
            label_key: "quota.weekly".to_owned(),
            presentation: QuotaPresentationDirection::Used,
            semantics: QuotaWindowSemantics::Fixed,
            nominal_duration_seconds: Some(604_800),
            reset_thresholds: None,
        })?)
    }

    fn sample(observation: u64, used_ratio: u32) -> TestResult<QuotaSample> {
        let mut bytes = [0_u8; 32];
        bytes[24..].copy_from_slice(&observation.to_be_bytes());
        Ok(QuotaSample::new(QuotaSampleParts {
            key: definition()?.key().clone(),
            observation_id: QuotaObservationId::from_bytes(bytes),
            observed_at_ms: i64::try_from(observation + 1)?,
            fresh_until_ms: i64::try_from(observation + 2)?,
            stale_after_ms: i64::try_from(observation + 3)?,
            provider_epoch_id: Some(QuotaProviderEpochId::new("epoch-1")?),
            used_ratio: Some(QuotaRatio::new(used_ratio)?),
            remaining_ratio: Some(QuotaRatio::new(1_000_000 - used_ratio)?),
            units: None,
            advertised_resets_at_ms: Some(10_000_000),
            quality: QuotaSampleQuality::Authoritative,
            source: QuotaEvidenceSource::ProviderLocal,
            confidence: QuotaConfidence::High,
            reset_evidence: QuotaResetEvidence::None,
            reset_occurred_at_ms: None,
        })?)
    }

    fn counts(store: &mut UsageStore) -> Result<(i64, i64), StoreError> {
        Ok(store.connection.query_row(
            "SELECT retained_sample_count, (SELECT count(*) FROM quota_sample)
             FROM quota_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    #[test]
    fn maintenance_faults_roll_back_sample_delete_and_state_count() -> TestResult {
        for fault in [
            QuotaMaintenanceFault::AfterDelete,
            QuotaMaintenanceFault::AfterState,
        ] {
            let definition = definition()?;
            let mut store = UsageStore::in_memory()?;
            for observation in 1_u64..=514 {
                let ratio = if observation.is_multiple_of(2) {
                    200_000
                } else {
                    100_000
                };
                store.apply_quota_observation(&definition, &sample(observation, ratio)?)?;
            }
            let before = counts(&mut store)?;
            let error = match store.maintain_quota_history_page_inner(definition.key(), 2, fault) {
                Ok(_) => return Err("injected maintenance fault unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(counts(&mut store)?, before);

            let retry = store.maintain_quota_history_page(definition.key(), 2)?;
            assert_eq!(retry.deleted_samples(), 2);
            assert_eq!(retry.remaining_samples(), 512);
        }
        Ok(())
    }
}
