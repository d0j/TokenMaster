use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use tokenmaster_domain::{
    QuotaConfidence, QuotaEvidenceSource, QuotaObservationId, QuotaPresentationDirection,
    QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample, QuotaSampleParts,
    QuotaSampleQuality, QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowKey,
    QuotaWindowSemantics,
};
use tokenmaster_quota::{
    QuotaAllowanceChangeKind, QuotaDetectionTime, QuotaEpochState, QuotaEpochStateParts,
    QuotaErrorCode, QuotaEvaluation, QuotaTransition, QuotaTransitionKind, evaluate_sample,
    quota_epoch_id, quota_scope_id,
};

use super::quota_maintenance::{
    delete_unprotected_sample, enforce_quota_window_hard_caps, samples_are_redundant,
};
use super::{QuotaApplyResult, QuotaApplyStatus, QuotaRevision, UsageStore};
use crate::{StoreError, StoreErrorCode};

impl UsageStore {
    pub fn apply_quota_observation(
        &mut self,
        definition: &QuotaWindowDefinition,
        sample: &QuotaSample,
    ) -> Result<QuotaApplyResult, StoreError> {
        self.apply_quota_observation_inner(definition, sample, QuotaWriteFault::None)
    }

    fn apply_quota_observation_inner(
        &mut self,
        definition: &QuotaWindowDefinition,
        sample: &QuotaSample,
        fault: QuotaWriteFault,
    ) -> Result<QuotaApplyResult, StoreError> {
        if definition.key() != sample.key() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let definition_revision = input_i64(definition.revision())?;
        let scope_id = quota_scope_id(definition.key().scope());
        let scope_bytes = scope_id.as_bytes().as_slice();
        let window_id = definition.key().window_id().as_str();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = load_quota_state(&transaction)?;
        let definition_exists = validate_definition(
            &transaction,
            scope_bytes,
            window_id,
            definition_revision,
            definition,
        )?;
        let observation_exists =
            validate_observation_identity(&transaction, scope_bytes, window_id, sample)?;
        let current = load_current_epoch(&transaction, definition.key(), scope_bytes, window_id)?;
        let previous = match current.as_ref() {
            Some(current) => Some(load_previous_sample(
                &transaction,
                definition.key(),
                scope_bytes,
                window_id,
                current,
            )?),
            None => None,
        };
        validate_current_projection(
            &transaction,
            scope_bytes,
            window_id,
            current.as_ref(),
            previous.as_ref(),
        )?;
        let next_transition_sequence = current.as_ref().map_or(0, |current| {
            current.last_transition_sequence().saturating_add(1)
        });
        let evaluation = evaluate_sample(
            definition,
            current.as_ref(),
            previous.as_ref(),
            sample,
            next_transition_sequence,
        )
        .map_err(map_quota_error)?;

        let no_op = match evaluation {
            QuotaEvaluation::Duplicate => Some(QuotaApplyStatus::Duplicate),
            QuotaEvaluation::Stale => Some(QuotaApplyStatus::Stale),
            QuotaEvaluation::Started { .. }
            | QuotaEvaluation::Advanced { .. }
            | QuotaEvaluation::AllowanceChanged { .. }
            | QuotaEvaluation::Reset { .. } => None,
        };
        if let Some(status) = no_op {
            let transition_sequence = current
                .as_ref()
                .map_or(0, QuotaEpochState::last_transition_sequence);
            transaction.commit()?;
            return Ok(QuotaApplyResult::new(
                status,
                state.revision,
                transition_sequence,
                None,
            ));
        }
        if observation_exists {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }

        if !definition_exists {
            insert_definition(
                &transaction,
                scope_bytes,
                window_id,
                definition_revision,
                definition,
            )?;
        }
        insert_sample(
            &transaction,
            scope_bytes,
            window_id,
            definition_revision,
            sample,
        )?;
        quota_write_fault(fault, QuotaWriteFault::AfterSample)?;

        transaction.execute(
            "DELETE FROM quota_window_current WHERE scope_id = ?1 AND window_id = ?2",
            params![scope_bytes, window_id],
        )?;

        let (status, next_state, transition, closes_epoch) = match evaluation {
            QuotaEvaluation::Started { state } => (QuotaApplyStatus::Started, state, None, false),
            QuotaEvaluation::Advanced { state } => (QuotaApplyStatus::Advanced, state, None, false),
            QuotaEvaluation::AllowanceChanged { state, transition } => (
                QuotaApplyStatus::AllowanceChanged,
                state,
                Some(transition),
                false,
            ),
            QuotaEvaluation::Reset { state, transition } => {
                (QuotaApplyStatus::Reset, state, Some(transition), true)
            }
            QuotaEvaluation::Duplicate | QuotaEvaluation::Stale => {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
        };
        if closes_epoch {
            let prior = current
                .as_ref()
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
            let closing_sequence = transition
                .as_ref()
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?
                .sequence();
            insert_epoch_history(
                &transaction,
                scope_bytes,
                window_id,
                prior,
                closing_sequence,
            )?;
        }
        write_current_epoch(&transaction, scope_bytes, window_id, &next_state)?;
        quota_write_fault(fault, QuotaWriteFault::AfterEpoch)?;

        if let Some(transition) = transition.as_ref() {
            insert_transition(
                &transaction,
                scope_bytes,
                window_id,
                definition_revision,
                transition,
            )?;
        }
        quota_write_fault(fault, QuotaWriteFault::AfterTransition)?;

        insert_window_current(
            &transaction,
            scope_bytes,
            window_id,
            definition_revision,
            sample,
            &next_state,
        )?;
        quota_write_fault(fault, QuotaWriteFault::AfterCurrent)?;

        let prune_previous = status == QuotaApplyStatus::Advanced
            && current
                .as_ref()
                .is_some_and(|current| current.definition_revision() == definition.revision())
            && previous
                .as_ref()
                .is_some_and(|previous| samples_are_redundant(previous, sample));
        let deleted_previous = if prune_previous {
            let previous = previous
                .as_ref()
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
            delete_unprotected_sample(
                &transaction,
                scope_bytes,
                window_id,
                previous.observation_id().as_bytes().as_slice(),
            )?
        } else {
            false
        };
        enforce_quota_window_hard_caps(&transaction, scope_bytes, window_id)?;

        let next_revision = state.revision.next()?;
        publish_quota_state(
            &transaction,
            &state,
            next_revision,
            sample.observed_at_ms(),
            1 - i64::from(deleted_previous),
            closes_epoch,
            transition.is_some(),
        )?;
        quota_write_fault(fault, QuotaWriteFault::AfterRevision)?;
        transaction.commit()?;

        Ok(QuotaApplyResult::new(
            status,
            next_revision,
            next_state.last_transition_sequence(),
            transition.as_ref().map(QuotaTransition::id),
        ))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum QuotaWriteFault {
    None,
    AfterSample,
    AfterEpoch,
    AfterTransition,
    AfterCurrent,
    AfterRevision,
}

fn quota_write_fault(actual: QuotaWriteFault, boundary: QuotaWriteFault) -> Result<(), StoreError> {
    if actual == boundary {
        Err(StoreError::new(StoreErrorCode::Database))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct QuotaState {
    revision: QuotaRevision,
    retained_sample_count: i64,
    retained_epoch_count: i64,
    retained_transition_count: i64,
    last_published_at_ms: Option<i64>,
}

fn load_quota_state(transaction: &Transaction<'_>) -> Result<QuotaState, StoreError> {
    let stored = transaction
        .query_row(
            "SELECT revision, retained_sample_count, retained_epoch_count,
                    retained_transition_count, last_published_at_ms
             FROM quota_state WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if stored.1 < 0 || stored.2 < 0 || stored.3 < 0 || (stored.0 == 0) != stored.4.is_none() {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(QuotaState {
        revision: QuotaRevision::from_stored(stored.0)?,
        retained_sample_count: stored.1,
        retained_epoch_count: stored.2,
        retained_transition_count: stored.3,
        last_published_at_ms: stored.4,
    })
}

fn validate_definition(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    revision: i64,
    definition: &QuotaWindowDefinition,
) -> Result<bool, StoreError> {
    let latest = transaction.query_row(
        "SELECT max(revision) FROM quota_window_definition
         WHERE scope_id = ?1 AND window_id = ?2",
        params![scope_id, window_id],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    if latest.is_some_and(|latest| revision < latest) {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let exists = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM quota_window_definition
           WHERE scope_id = ?1 AND window_id = ?2 AND revision = ?3
         )",
        params![scope_id, window_id, revision],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Ok(false);
    }
    let thresholds = definition.reset_thresholds();
    let nominal_duration = input_optional_i64(definition.nominal_duration_seconds())?;
    let maximum_used = thresholds
        .and_then(|value| value.maximum_post_reset_used_ratio())
        .map(|value| i64::from(value.parts_per_million()));
    let minimum_remaining = thresholds
        .and_then(|value| value.minimum_post_reset_remaining_ratio())
        .map(|value| i64::from(value.parts_per_million()));
    let minimum_drop = thresholds
        .and_then(|value| value.minimum_used_ratio_drop())
        .map(|value| i64::from(value.parts_per_million()));
    let scope = definition.key().scope();
    let exact = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM quota_window_definition
           WHERE scope_id = ?1 AND window_id = ?2 AND revision = ?3
             AND provider_id = ?4 AND account_id = ?5 AND workspace_id IS ?6
             AND label_key = ?7 AND presentation = ?8 AND semantics = ?9
             AND nominal_duration_seconds IS ?10
             AND maximum_post_reset_used_ppm IS ?11
             AND minimum_post_reset_remaining_ppm IS ?12
             AND minimum_used_ratio_drop_ppm IS ?13
         )",
        params![
            scope_id,
            window_id,
            revision,
            scope.provider_id().as_str(),
            scope.account_id().as_str(),
            scope.workspace_id().map(|value| value.as_str()),
            definition.label_key(),
            presentation_sql(definition.presentation()),
            semantics_sql(definition.semantics()),
            nominal_duration,
            maximum_used,
            minimum_remaining,
            minimum_drop,
        ],
        |row| row.get::<_, bool>(0),
    )?;
    if !exact {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    Ok(true)
}

fn validate_observation_identity(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    sample: &QuotaSample,
) -> Result<bool, StoreError> {
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM quota_sample WHERE observation_id = ?1)",
        params![sample.observation_id().as_bytes().as_slice()],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Ok(false);
    }
    let units = sample.units();
    let exact = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM quota_sample
           WHERE observation_id = ?1 AND scope_id = ?2 AND window_id = ?3
             AND observed_at_ms = ?4 AND fresh_until_ms = ?5 AND stale_after_ms = ?6
             AND provider_epoch_id IS ?7 AND used_ratio_ppm IS ?8
             AND remaining_ratio_ppm IS ?9 AND unit_id IS ?10
             AND used_units IS ?11 AND remaining_units IS ?12 AND capacity_units IS ?13
             AND advertised_resets_at_ms IS ?14 AND quality = ?15 AND source = ?16
             AND confidence = ?17 AND reset_evidence = ?18
             AND reset_occurred_at_ms IS ?19
         )",
        params![
            sample.observation_id().as_bytes().as_slice(),
            scope_id,
            window_id,
            sample.observed_at_ms(),
            sample.fresh_until_ms(),
            sample.stale_after_ms(),
            sample.provider_epoch_id().map(|value| value.as_str()),
            sample
                .used_ratio()
                .map(|value| i64::from(value.parts_per_million())),
            sample
                .remaining_ratio()
                .map(|value| i64::from(value.parts_per_million())),
            units.map(|value| value.unit_id().as_str()),
            input_optional_i64(units.and_then(QuotaUnits::used))?,
            input_optional_i64(units.and_then(QuotaUnits::remaining))?,
            input_optional_i64(units.and_then(QuotaUnits::capacity))?,
            sample.advertised_resets_at_ms(),
            quality_sql(sample.quality()),
            source_sql(sample.source()),
            confidence_sql(sample.confidence()),
            reset_evidence_sql(sample.reset_evidence()),
            sample.reset_occurred_at_ms(),
        ],
        |row| row.get::<_, bool>(0),
    )?;
    if !exact {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    Ok(true)
}

struct StoredEpoch {
    epoch_id: Vec<u8>,
    epoch_definition_revision: i64,
    definition_revision: i64,
    first_observation_id: Vec<u8>,
    last_observation_id: Vec<u8>,
    first_observed_at_ms: i64,
    last_observed_at_ms: i64,
    maximum_used_ratio_ppm: Option<i64>,
    maximum_used_ratio_observation_id: Option<Vec<u8>>,
    maximum_unit_id: Option<String>,
    maximum_used_units: Option<i64>,
    maximum_remaining_units: Option<i64>,
    maximum_capacity_units: Option<i64>,
    maximum_used_units_observation_id: Option<Vec<u8>>,
    provider_epoch_id: Option<String>,
    advertised_resets_at_ms: Option<i64>,
    last_transition_sequence: i64,
}

fn load_current_epoch(
    transaction: &Transaction<'_>,
    key: &QuotaWindowKey,
    scope_id: &[u8],
    window_id: &str,
) -> Result<Option<QuotaEpochState>, StoreError> {
    let stored = transaction
        .query_row(
            "SELECT epoch_id, epoch_definition_revision, definition_revision,
                    first_observation_id, last_observation_id,
                    first_observed_at_ms, last_observed_at_ms,
                    maximum_used_ratio_ppm, maximum_used_ratio_observation_id,
                    maximum_unit_id, maximum_used_units, maximum_remaining_units,
                    maximum_capacity_units, maximum_used_units_observation_id,
                    provider_epoch_id, advertised_resets_at_ms, last_transition_sequence
             FROM quota_epoch_current WHERE scope_id = ?1 AND window_id = ?2",
            params![scope_id, window_id],
            |row| {
                Ok(StoredEpoch {
                    epoch_id: row.get(0)?,
                    epoch_definition_revision: row.get(1)?,
                    definition_revision: row.get(2)?,
                    first_observation_id: row.get(3)?,
                    last_observation_id: row.get(4)?,
                    first_observed_at_ms: row.get(5)?,
                    last_observed_at_ms: row.get(6)?,
                    maximum_used_ratio_ppm: row.get(7)?,
                    maximum_used_ratio_observation_id: row.get(8)?,
                    maximum_unit_id: row.get(9)?,
                    maximum_used_units: row.get(10)?,
                    maximum_remaining_units: row.get(11)?,
                    maximum_capacity_units: row.get(12)?,
                    maximum_used_units_observation_id: row.get(13)?,
                    provider_epoch_id: row.get(14)?,
                    advertised_resets_at_ms: row.get(15)?,
                    last_transition_sequence: row.get(16)?,
                })
            },
        )
        .optional()?;
    stored.map(|stored| restore_epoch(key, stored)).transpose()
}

fn restore_epoch(key: &QuotaWindowKey, stored: StoredEpoch) -> Result<QuotaEpochState, StoreError> {
    let epoch_definition_revision = stored_u64(stored.epoch_definition_revision)?;
    let definition_revision = stored_u64(stored.definition_revision)?;
    let first_observation_id =
        QuotaObservationId::from_bytes(stored_bytes(stored.first_observation_id)?);
    let last_observation_id =
        QuotaObservationId::from_bytes(stored_bytes(stored.last_observation_id)?);
    let epoch_id = quota_epoch_id(key, epoch_definition_revision, first_observation_id);
    if epoch_id.as_bytes() != &stored_bytes(stored.epoch_id)? {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let maximum_used_ratio = stored
        .maximum_used_ratio_ppm
        .map(stored_ratio)
        .transpose()?;
    let maximum_used_ratio_observation_id = stored
        .maximum_used_ratio_observation_id
        .map(stored_bytes)
        .transpose()?
        .map(QuotaObservationId::from_bytes);
    let maximum_used_units = stored_units(
        stored.maximum_unit_id,
        stored.maximum_used_units,
        stored.maximum_remaining_units,
        stored.maximum_capacity_units,
    )?;
    let maximum_used_units_observation_id = stored
        .maximum_used_units_observation_id
        .map(stored_bytes)
        .transpose()?
        .map(QuotaObservationId::from_bytes);
    let provider_epoch_id = stored
        .provider_epoch_id
        .map(QuotaProviderEpochId::new)
        .transpose()
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    QuotaEpochState::restore(QuotaEpochStateParts {
        key: key.clone(),
        epoch_definition_revision,
        definition_revision,
        epoch_id,
        first_observation_id,
        last_observation_id,
        first_observed_at_ms: stored.first_observed_at_ms,
        last_observed_at_ms: stored.last_observed_at_ms,
        maximum_used_ratio,
        maximum_used_ratio_observation_id,
        maximum_used_units,
        maximum_used_units_observation_id,
        provider_epoch_id,
        advertised_resets_at_ms: stored.advertised_resets_at_ms,
        last_transition_sequence: stored_u64(stored.last_transition_sequence)?,
    })
    .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

struct StoredSample {
    definition_revision: i64,
    observation_id: Vec<u8>,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    provider_epoch_id: Option<String>,
    used_ratio_ppm: Option<i64>,
    remaining_ratio_ppm: Option<i64>,
    unit_id: Option<String>,
    used_units: Option<i64>,
    remaining_units: Option<i64>,
    capacity_units: Option<i64>,
    advertised_resets_at_ms: Option<i64>,
    quality: String,
    source: String,
    confidence: String,
    reset_evidence: String,
    reset_occurred_at_ms: Option<i64>,
}

fn load_previous_sample(
    transaction: &Transaction<'_>,
    key: &QuotaWindowKey,
    scope_id: &[u8],
    window_id: &str,
    current: &QuotaEpochState,
) -> Result<QuotaSample, StoreError> {
    let stored = transaction
        .query_row(
            "SELECT definition_revision, observation_id, observed_at_ms,
                    fresh_until_ms, stale_after_ms, provider_epoch_id,
                    used_ratio_ppm, remaining_ratio_ppm, unit_id,
                    used_units, remaining_units, capacity_units,
                    advertised_resets_at_ms, quality, source, confidence,
                    reset_evidence, reset_occurred_at_ms
             FROM quota_sample
             WHERE scope_id = ?1 AND window_id = ?2 AND observation_id = ?3",
            params![
                scope_id,
                window_id,
                current.last_observation_id().as_bytes().as_slice()
            ],
            |row| {
                Ok(StoredSample {
                    definition_revision: row.get(0)?,
                    observation_id: row.get(1)?,
                    observed_at_ms: row.get(2)?,
                    fresh_until_ms: row.get(3)?,
                    stale_after_ms: row.get(4)?,
                    provider_epoch_id: row.get(5)?,
                    used_ratio_ppm: row.get(6)?,
                    remaining_ratio_ppm: row.get(7)?,
                    unit_id: row.get(8)?,
                    used_units: row.get(9)?,
                    remaining_units: row.get(10)?,
                    capacity_units: row.get(11)?,
                    advertised_resets_at_ms: row.get(12)?,
                    quality: row.get(13)?,
                    source: row.get(14)?,
                    confidence: row.get(15)?,
                    reset_evidence: row.get(16)?,
                    reset_occurred_at_ms: row.get(17)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if stored_u64(stored.definition_revision)? != current.definition_revision() {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    restore_sample(key, stored)
}

fn restore_sample(key: &QuotaWindowKey, stored: StoredSample) -> Result<QuotaSample, StoreError> {
    let provider_epoch_id = stored
        .provider_epoch_id
        .map(QuotaProviderEpochId::new)
        .transpose()
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let used_ratio = stored.used_ratio_ppm.map(stored_ratio).transpose()?;
    let remaining_ratio = stored.remaining_ratio_ppm.map(stored_ratio).transpose()?;
    let units = stored_units(
        stored.unit_id,
        stored.used_units,
        stored.remaining_units,
        stored.capacity_units,
    )?;
    QuotaSample::new(QuotaSampleParts {
        key: key.clone(),
        observation_id: QuotaObservationId::from_bytes(stored_bytes(stored.observation_id)?),
        observed_at_ms: stored.observed_at_ms,
        fresh_until_ms: stored.fresh_until_ms,
        stale_after_ms: stored.stale_after_ms,
        provider_epoch_id,
        used_ratio,
        remaining_ratio,
        units,
        advertised_resets_at_ms: stored.advertised_resets_at_ms,
        quality: stored_quality(&stored.quality)?,
        source: stored_source(&stored.source)?,
        confidence: stored_confidence(&stored.confidence)?,
        reset_evidence: stored_reset_evidence(&stored.reset_evidence)?,
        reset_occurred_at_ms: stored.reset_occurred_at_ms,
    })
    .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

struct StoredCurrentProjection {
    definition_revision: i64,
    sample_observation_id: Vec<u8>,
    epoch_id: Vec<u8>,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    quality: String,
    source: String,
    confidence: String,
    last_transition_sequence: i64,
}

fn validate_current_projection(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    current: Option<&QuotaEpochState>,
    previous: Option<&QuotaSample>,
) -> Result<(), StoreError> {
    let stored = transaction
        .query_row(
            "SELECT definition_revision, sample_observation_id, epoch_id,
                    observed_at_ms, fresh_until_ms, stale_after_ms,
                    quality, source, confidence, last_transition_sequence
             FROM quota_window_current WHERE scope_id = ?1 AND window_id = ?2",
            params![scope_id, window_id],
            |row| {
                Ok(StoredCurrentProjection {
                    definition_revision: row.get(0)?,
                    sample_observation_id: row.get(1)?,
                    epoch_id: row.get(2)?,
                    observed_at_ms: row.get(3)?,
                    fresh_until_ms: row.get(4)?,
                    stale_after_ms: row.get(5)?,
                    quality: row.get(6)?,
                    source: row.get(7)?,
                    confidence: row.get(8)?,
                    last_transition_sequence: row.get(9)?,
                })
            },
        )
        .optional()?;
    let (stored, current, previous) = match (stored, current, previous) {
        (None, None, None) => return Ok(()),
        (Some(stored), Some(current), Some(previous)) => (stored, current, previous),
        _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    };
    let sample_observation_id = stored_bytes(stored.sample_observation_id)?;
    let epoch_id = stored_bytes(stored.epoch_id)?;
    if stored_u64(stored.definition_revision)? != current.definition_revision()
        || sample_observation_id != *previous.observation_id().as_bytes()
        || epoch_id != *current.epoch_id().as_bytes()
        || stored.observed_at_ms != previous.observed_at_ms()
        || stored.fresh_until_ms != previous.fresh_until_ms()
        || stored.stale_after_ms != previous.stale_after_ms()
        || stored.quality != quality_sql(previous.quality())
        || stored.source != source_sql(previous.source())
        || stored.confidence != confidence_sql(previous.confidence())
        || stored_u64(stored.last_transition_sequence)? != current.last_transition_sequence()
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn insert_definition(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    revision: i64,
    definition: &QuotaWindowDefinition,
) -> Result<(), StoreError> {
    let thresholds = definition.reset_thresholds();
    let scope = definition.key().scope();
    transaction.execute(
        "INSERT INTO quota_window_definition(
           scope_id, window_id, revision, provider_id, account_id, workspace_id,
           label_key, presentation, semantics, nominal_duration_seconds,
           maximum_post_reset_used_ppm, minimum_post_reset_remaining_ppm,
           minimum_used_ratio_drop_ppm
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            scope_id,
            window_id,
            revision,
            scope.provider_id().as_str(),
            scope.account_id().as_str(),
            scope.workspace_id().map(|value| value.as_str()),
            definition.label_key(),
            presentation_sql(definition.presentation()),
            semantics_sql(definition.semantics()),
            input_optional_i64(definition.nominal_duration_seconds())?,
            thresholds
                .and_then(|value| value.maximum_post_reset_used_ratio())
                .map(|value| i64::from(value.parts_per_million())),
            thresholds
                .and_then(|value| value.minimum_post_reset_remaining_ratio())
                .map(|value| i64::from(value.parts_per_million())),
            thresholds
                .and_then(|value| value.minimum_used_ratio_drop())
                .map(|value| i64::from(value.parts_per_million())),
        ],
    )?;
    Ok(())
}

fn insert_sample(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    definition_revision: i64,
    sample: &QuotaSample,
) -> Result<(), StoreError> {
    let units = sample.units();
    transaction.execute(
        "INSERT INTO quota_sample(
           observation_id, scope_id, window_id, definition_revision,
           observed_at_ms, fresh_until_ms, stale_after_ms, provider_epoch_id,
           used_ratio_ppm, remaining_ratio_ppm, unit_id, used_units,
           remaining_units, capacity_units, advertised_resets_at_ms,
           quality, source, confidence, reset_evidence, reset_occurred_at_ms
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
         )",
        params![
            sample.observation_id().as_bytes().as_slice(),
            scope_id,
            window_id,
            definition_revision,
            sample.observed_at_ms(),
            sample.fresh_until_ms(),
            sample.stale_after_ms(),
            sample.provider_epoch_id().map(|value| value.as_str()),
            sample
                .used_ratio()
                .map(|value| i64::from(value.parts_per_million())),
            sample
                .remaining_ratio()
                .map(|value| i64::from(value.parts_per_million())),
            units.map(|value| value.unit_id().as_str()),
            input_optional_i64(units.and_then(QuotaUnits::used))?,
            input_optional_i64(units.and_then(QuotaUnits::remaining))?,
            input_optional_i64(units.and_then(QuotaUnits::capacity))?,
            sample.advertised_resets_at_ms(),
            quality_sql(sample.quality()),
            source_sql(sample.source()),
            confidence_sql(sample.confidence()),
            reset_evidence_sql(sample.reset_evidence()),
            sample.reset_occurred_at_ms(),
        ],
    )?;
    Ok(())
}

fn write_current_epoch(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    state: &QuotaEpochState,
) -> Result<(), StoreError> {
    let parts = state.to_parts();
    let maximum_units = parts.maximum_used_units.as_ref();
    let maximum_ratio_observation_id = parts.maximum_used_ratio_observation_id;
    let maximum_units_observation_id = parts.maximum_used_units_observation_id;
    transaction.execute(
        "INSERT INTO quota_epoch_current(
           scope_id, window_id, epoch_id, epoch_definition_revision, definition_revision,
           first_observation_id, last_observation_id, first_observed_at_ms,
           last_observed_at_ms, maximum_used_ratio_ppm,
           maximum_used_ratio_observation_id, maximum_unit_id, maximum_used_units,
           maximum_remaining_units, maximum_capacity_units,
           maximum_used_units_observation_id, provider_epoch_id,
           advertised_resets_at_ms, last_transition_sequence
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
         )
         ON CONFLICT(scope_id, window_id) DO UPDATE SET
           epoch_id = excluded.epoch_id,
           epoch_definition_revision = excluded.epoch_definition_revision,
           definition_revision = excluded.definition_revision,
           first_observation_id = excluded.first_observation_id,
           last_observation_id = excluded.last_observation_id,
           first_observed_at_ms = excluded.first_observed_at_ms,
           last_observed_at_ms = excluded.last_observed_at_ms,
           maximum_used_ratio_ppm = excluded.maximum_used_ratio_ppm,
           maximum_used_ratio_observation_id = excluded.maximum_used_ratio_observation_id,
           maximum_unit_id = excluded.maximum_unit_id,
           maximum_used_units = excluded.maximum_used_units,
           maximum_remaining_units = excluded.maximum_remaining_units,
           maximum_capacity_units = excluded.maximum_capacity_units,
           maximum_used_units_observation_id = excluded.maximum_used_units_observation_id,
           provider_epoch_id = excluded.provider_epoch_id,
           advertised_resets_at_ms = excluded.advertised_resets_at_ms,
           last_transition_sequence = excluded.last_transition_sequence",
        params![
            scope_id,
            window_id,
            parts.epoch_id.as_bytes().as_slice(),
            input_i64(parts.epoch_definition_revision)?,
            input_i64(parts.definition_revision)?,
            parts.first_observation_id.as_bytes().as_slice(),
            parts.last_observation_id.as_bytes().as_slice(),
            parts.first_observed_at_ms,
            parts.last_observed_at_ms,
            parts
                .maximum_used_ratio
                .map(|value| i64::from(value.parts_per_million())),
            maximum_ratio_observation_id
                .as_ref()
                .map(|value| value.as_bytes().as_slice()),
            maximum_units.map(|value| value.unit_id().as_str()),
            input_optional_i64(maximum_units.and_then(QuotaUnits::used))?,
            input_optional_i64(maximum_units.and_then(QuotaUnits::remaining))?,
            input_optional_i64(maximum_units.and_then(QuotaUnits::capacity))?,
            maximum_units_observation_id
                .as_ref()
                .map(|value| value.as_bytes().as_slice()),
            parts.provider_epoch_id.as_ref().map(|value| value.as_str()),
            parts.advertised_resets_at_ms,
            sequence_i64(parts.last_transition_sequence)?,
        ],
    )?;
    Ok(())
}

fn insert_epoch_history(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    state: &QuotaEpochState,
    closing_transition_sequence: u64,
) -> Result<(), StoreError> {
    let parts = state.to_parts();
    let maximum_units = parts.maximum_used_units.as_ref();
    let maximum_ratio_observation_id = parts.maximum_used_ratio_observation_id;
    let maximum_units_observation_id = parts.maximum_used_units_observation_id;
    transaction.execute(
        "INSERT INTO quota_epoch_history(
           epoch_id, scope_id, window_id, epoch_definition_revision, definition_revision,
           first_observation_id, last_observation_id, first_observed_at_ms,
           last_observed_at_ms, maximum_used_ratio_ppm,
           maximum_used_ratio_observation_id, maximum_unit_id, maximum_used_units,
           maximum_remaining_units, maximum_capacity_units,
           maximum_used_units_observation_id, final_provider_epoch_id,
           final_advertised_resets_at_ms, closing_transition_sequence
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
         )",
        params![
            parts.epoch_id.as_bytes().as_slice(),
            scope_id,
            window_id,
            input_i64(parts.epoch_definition_revision)?,
            input_i64(parts.definition_revision)?,
            parts.first_observation_id.as_bytes().as_slice(),
            parts.last_observation_id.as_bytes().as_slice(),
            parts.first_observed_at_ms,
            parts.last_observed_at_ms,
            parts
                .maximum_used_ratio
                .map(|value| i64::from(value.parts_per_million())),
            maximum_ratio_observation_id
                .as_ref()
                .map(|value| value.as_bytes().as_slice()),
            maximum_units.map(|value| value.unit_id().as_str()),
            input_optional_i64(maximum_units.and_then(QuotaUnits::used))?,
            input_optional_i64(maximum_units.and_then(QuotaUnits::remaining))?,
            input_optional_i64(maximum_units.and_then(QuotaUnits::capacity))?,
            maximum_units_observation_id
                .as_ref()
                .map(|value| value.as_bytes().as_slice()),
            parts.provider_epoch_id.as_ref().map(|value| value.as_str()),
            parts.advertised_resets_at_ms,
            sequence_i64(closing_transition_sequence)?,
        ],
    )?;
    Ok(())
}

fn insert_transition(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    definition_revision: i64,
    transition: &QuotaTransition,
) -> Result<(), StoreError> {
    let maximum_units = transition.maximum_used_units_before();
    let allowance = transition.allowance_change();
    let old_units = allowance.map(|value| value.old_units());
    let new_units = allowance.map(|value| value.new_units());
    let maximum_ratio_observation_id = transition.maximum_used_ratio_observation_id_before();
    let maximum_units_observation_id = transition.maximum_used_units_observation_id_before();
    let (detection_kind, exact_at_ms, after_ms, at_or_before_ms) = match transition.detection_time()
    {
        QuotaDetectionTime::Exact(at_ms) => ("exact", Some(at_ms), None, None),
        QuotaDetectionTime::Interval {
            after_ms,
            at_or_before_ms,
        } => ("interval", None, Some(after_ms), Some(at_or_before_ms)),
    };
    transaction.execute(
        "INSERT INTO quota_transition(
           transition_id, scope_id, window_id, definition_revision, sequence, kind,
           previous_epoch_id, current_epoch_id, pre_observation_id, post_observation_id,
           maximum_used_ratio_ppm, maximum_used_ratio_observation_id,
           maximum_unit_id, maximum_used_units, maximum_remaining_units,
           maximum_capacity_units, maximum_used_units_observation_id,
           old_resets_at_ms, new_resets_at_ms, allowance_change_kind,
           allowance_old_unit_id, allowance_old_used_units,
           allowance_old_remaining_units, allowance_old_capacity_units,
           allowance_new_unit_id, allowance_new_used_units,
           allowance_new_remaining_units, allowance_new_capacity_units,
           source, confidence, detection_time_kind, exact_at_ms, after_ms, at_or_before_ms
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
           ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30,
           ?31, ?32, ?33, ?34
         )",
        params![
            transition.id().as_bytes().as_slice(),
            scope_id,
            window_id,
            definition_revision,
            sequence_i64(transition.sequence())?,
            transition_kind_sql(transition.kind()),
            transition.previous_epoch_id().as_bytes().as_slice(),
            transition.current_epoch_id().as_bytes().as_slice(),
            transition.pre_observation_id().as_bytes().as_slice(),
            transition.post_observation_id().as_bytes().as_slice(),
            transition
                .maximum_used_ratio_before()
                .map(|value| i64::from(value.parts_per_million())),
            maximum_ratio_observation_id
                .as_ref()
                .map(|value| value.as_bytes().as_slice()),
            maximum_units.map(|value| value.unit_id().as_str()),
            input_optional_i64(maximum_units.and_then(QuotaUnits::used))?,
            input_optional_i64(maximum_units.and_then(QuotaUnits::remaining))?,
            input_optional_i64(maximum_units.and_then(QuotaUnits::capacity))?,
            maximum_units_observation_id
                .as_ref()
                .map(|value| value.as_bytes().as_slice()),
            transition.old_resets_at_ms(),
            transition.new_resets_at_ms(),
            allowance.map(|value| allowance_change_sql(value.kind())),
            old_units.map(|value| value.unit_id().as_str()),
            input_optional_i64(old_units.and_then(QuotaUnits::used))?,
            input_optional_i64(old_units.and_then(QuotaUnits::remaining))?,
            input_optional_i64(old_units.and_then(QuotaUnits::capacity))?,
            new_units.map(|value| value.unit_id().as_str()),
            input_optional_i64(new_units.and_then(QuotaUnits::used))?,
            input_optional_i64(new_units.and_then(QuotaUnits::remaining))?,
            input_optional_i64(new_units.and_then(QuotaUnits::capacity))?,
            source_sql(transition.source()),
            confidence_sql(transition.confidence()),
            detection_kind,
            exact_at_ms,
            after_ms,
            at_or_before_ms,
        ],
    )?;
    Ok(())
}

fn insert_window_current(
    transaction: &Transaction<'_>,
    scope_id: &[u8],
    window_id: &str,
    definition_revision: i64,
    sample: &QuotaSample,
    state: &QuotaEpochState,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO quota_window_current(
           scope_id, window_id, definition_revision, sample_observation_id,
           epoch_id, observed_at_ms, fresh_until_ms, stale_after_ms,
           quality, source, confidence, last_transition_sequence
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            scope_id,
            window_id,
            definition_revision,
            sample.observation_id().as_bytes().as_slice(),
            state.epoch_id().as_bytes().as_slice(),
            sample.observed_at_ms(),
            sample.fresh_until_ms(),
            sample.stale_after_ms(),
            quality_sql(sample.quality()),
            source_sql(sample.source()),
            confidence_sql(sample.confidence()),
            sequence_i64(state.last_transition_sequence())?,
        ],
    )?;
    Ok(())
}

fn publish_quota_state(
    transaction: &Transaction<'_>,
    state: &QuotaState,
    next_revision: QuotaRevision,
    observed_at_ms: i64,
    sample_count_delta: i64,
    epoch_closed: bool,
    transition_inserted: bool,
) -> Result<(), StoreError> {
    let retained_sample_count = checked_count(state.retained_sample_count, sample_count_delta)?;
    let retained_epoch_count = checked_count(state.retained_epoch_count, i64::from(epoch_closed))?;
    let retained_transition_count = checked_count(
        state.retained_transition_count,
        i64::from(transition_inserted),
    )?;
    let last_published_at_ms = state
        .last_published_at_ms
        .map_or(observed_at_ms, |current| current.max(observed_at_ms));
    let updated = transaction.execute(
        "UPDATE quota_state
         SET revision = ?1, retained_sample_count = ?2,
             retained_epoch_count = ?3, retained_transition_count = ?4,
             last_published_at_ms = ?5
         WHERE singleton_id = 1 AND revision = ?6",
        params![
            next_revision.as_sql(),
            retained_sample_count,
            retained_epoch_count,
            retained_transition_count,
            last_published_at_ms,
            state.revision.as_sql(),
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    Ok(())
}

fn checked_count(current: i64, increment: i64) -> Result<i64, StoreError> {
    current
        .checked_add(increment)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn input_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))
}

fn sequence_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn input_optional_i64(value: Option<u64>) -> Result<Option<i64>, StoreError> {
    value.map(input_i64).transpose()
}

fn stored_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn stored_bytes(value: Vec<u8>) -> Result<[u8; 32], StoreError> {
    value
        .try_into()
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn stored_ratio(value: i64) -> Result<QuotaRatio, StoreError> {
    let value =
        u32::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    QuotaRatio::new(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn stored_units(
    unit_id: Option<String>,
    used: Option<i64>,
    remaining: Option<i64>,
    capacity: Option<i64>,
) -> Result<Option<QuotaUnits>, StoreError> {
    let Some(unit_id) = unit_id else {
        if used.is_some() || remaining.is_some() || capacity.is_some() {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        return Ok(None);
    };
    let unit_id = QuotaUnitId::new(unit_id)
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    QuotaUnits::new(
        unit_id,
        used.map(stored_u64).transpose()?,
        remaining.map(stored_u64).transpose()?,
        capacity.map(stored_u64).transpose()?,
    )
    .map(Some)
    .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn map_quota_error(error: tokenmaster_quota::QuotaError) -> StoreError {
    let code = match error.code() {
        QuotaErrorCode::SampleWindowMismatch => StoreErrorCode::InvalidValue,
        QuotaErrorCode::DefinitionRevisionRegressed => StoreErrorCode::StaleRevision,
        QuotaErrorCode::DuplicateConflict => StoreErrorCode::InvalidValue,
        QuotaErrorCode::TransitionSequenceOverflow => StoreErrorCode::CapacityExceeded,
        QuotaErrorCode::UnexpectedPrevious
        | QuotaErrorCode::MissingPrevious
        | QuotaErrorCode::StateWindowMismatch
        | QuotaErrorCode::PreviousWindowMismatch
        | QuotaErrorCode::StatePreviousMismatch
        | QuotaErrorCode::InvalidTransitionSequence
        | QuotaErrorCode::InvalidEpochState
        | QuotaErrorCode::InvalidTransitionState => StoreErrorCode::InvalidStoredValue,
    };
    StoreError::new(code)
}

const fn presentation_sql(value: QuotaPresentationDirection) -> &'static str {
    match value {
        QuotaPresentationDirection::Used => "used",
        QuotaPresentationDirection::Remaining => "remaining",
        QuotaPresentationDirection::Pace => "pace",
    }
}

const fn semantics_sql(value: QuotaWindowSemantics) -> &'static str {
    match value {
        QuotaWindowSemantics::Fixed => "fixed",
        QuotaWindowSemantics::Rolling => "rolling",
        QuotaWindowSemantics::Credit => "credit",
        QuotaWindowSemantics::Unknown => "unknown",
    }
}

const fn quality_sql(value: QuotaSampleQuality) -> &'static str {
    match value {
        QuotaSampleQuality::Authoritative => "authoritative",
        QuotaSampleQuality::Partial => "partial",
        QuotaSampleQuality::Conflict => "conflict",
        QuotaSampleQuality::Unknown => "unknown",
    }
}

const fn source_sql(value: QuotaEvidenceSource) -> &'static str {
    match value {
        QuotaEvidenceSource::ProviderLocal => "provider_local",
        QuotaEvidenceSource::ProviderOfficial => "provider_official",
        QuotaEvidenceSource::LocalResetEvent => "local_reset_event",
        QuotaEvidenceSource::Manual => "manual",
        QuotaEvidenceSource::Unknown => "unknown",
    }
}

const fn confidence_sql(value: QuotaConfidence) -> &'static str {
    match value {
        QuotaConfidence::High => "high",
        QuotaConfidence::Medium => "medium",
        QuotaConfidence::Low => "low",
        QuotaConfidence::Unknown => "unknown",
    }
}

const fn reset_evidence_sql(value: QuotaResetEvidence) -> &'static str {
    match value {
        QuotaResetEvidence::None => "none",
        QuotaResetEvidence::ExplicitProvider => "explicit_provider",
        QuotaResetEvidence::ExplicitLocal => "explicit_local",
        QuotaResetEvidence::ManualOrBanked => "manual_or_banked",
    }
}

const fn transition_kind_sql(value: QuotaTransitionKind) -> &'static str {
    match value {
        QuotaTransitionKind::ScheduledReset => "scheduled_reset",
        QuotaTransitionKind::EarlyReset => "early_reset",
        QuotaTransitionKind::ManualOrBankedReset => "manual_or_banked_reset",
        QuotaTransitionKind::UnknownReset => "unknown_reset",
        QuotaTransitionKind::AllowanceChanged => "allowance_changed",
    }
}

const fn allowance_change_sql(value: QuotaAllowanceChangeKind) -> &'static str {
    match value {
        QuotaAllowanceChangeKind::Increased => "increased",
        QuotaAllowanceChangeKind::Decreased => "decreased",
        QuotaAllowanceChangeKind::UnitChanged => "unit_changed",
    }
}

fn stored_quality(value: &str) -> Result<QuotaSampleQuality, StoreError> {
    match value {
        "authoritative" => Ok(QuotaSampleQuality::Authoritative),
        "partial" => Ok(QuotaSampleQuality::Partial),
        "conflict" => Ok(QuotaSampleQuality::Conflict),
        "unknown" => Ok(QuotaSampleQuality::Unknown),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn stored_source(value: &str) -> Result<QuotaEvidenceSource, StoreError> {
    match value {
        "provider_local" => Ok(QuotaEvidenceSource::ProviderLocal),
        "provider_official" => Ok(QuotaEvidenceSource::ProviderOfficial),
        "local_reset_event" => Ok(QuotaEvidenceSource::LocalResetEvent),
        "manual" => Ok(QuotaEvidenceSource::Manual),
        "unknown" => Ok(QuotaEvidenceSource::Unknown),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn stored_confidence(value: &str) -> Result<QuotaConfidence, StoreError> {
    match value {
        "high" => Ok(QuotaConfidence::High),
        "medium" => Ok(QuotaConfidence::Medium),
        "low" => Ok(QuotaConfidence::Low),
        "unknown" => Ok(QuotaConfidence::Unknown),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn stored_reset_evidence(value: &str) -> Result<QuotaResetEvidence, StoreError> {
    match value {
        "none" => Ok(QuotaResetEvidence::None),
        "explicit_provider" => Ok(QuotaResetEvidence::ExplicitProvider),
        "explicit_local" => Ok(QuotaResetEvidence::ExplicitLocal),
        "manual_or_banked" => Ok(QuotaResetEvidence::ManualOrBanked),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

#[cfg(test)]
mod tests {
    use tokenmaster_domain::{
        QuotaAccountId, QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaScope,
        QuotaUnitId, QuotaUnits, QuotaWindowDefinitionParts, QuotaWindowId, UsageProviderId,
    };

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn key() -> TestResult<QuotaWindowKey> {
        Ok(QuotaWindowKey::new(
            QuotaScope::new(
                UsageProviderId::new("codex")?,
                QuotaAccountId::new("personal")?,
                None,
            ),
            QuotaWindowId::new("weekly")?,
        ))
    }

    fn definition(revision: u64) -> TestResult<QuotaWindowDefinition> {
        Ok(QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
            key: key()?,
            revision,
            label_key: "quota.weekly".to_owned(),
            presentation: QuotaPresentationDirection::Used,
            semantics: QuotaWindowSemantics::Fixed,
            nominal_duration_seconds: Some(604_800),
            reset_thresholds: None,
        })?)
    }

    fn sample(
        observation: u8,
        observed_at_ms: i64,
        provider_epoch: &str,
        used_units: u64,
    ) -> TestResult<QuotaSample> {
        let ratio = u32::try_from(used_units * 10_000)?;
        Ok(QuotaSample::new(QuotaSampleParts {
            key: key()?,
            observation_id: QuotaObservationId::from_bytes([observation; 32]),
            observed_at_ms,
            fresh_until_ms: observed_at_ms + 100,
            stale_after_ms: observed_at_ms + 200,
            provider_epoch_id: Some(QuotaProviderEpochId::new(provider_epoch)?),
            used_ratio: Some(QuotaRatio::new(ratio)?),
            remaining_ratio: Some(QuotaRatio::new(1_000_000 - ratio)?),
            units: Some(QuotaUnits::new(
                QuotaUnitId::new("requests")?,
                Some(used_units),
                Some(100 - used_units),
                Some(100),
            )?),
            advertised_resets_at_ms: Some(observed_at_ms + 10_000),
            quality: QuotaSampleQuality::Authoritative,
            source: QuotaEvidenceSource::ProviderLocal,
            confidence: QuotaConfidence::High,
            reset_evidence: QuotaResetEvidence::None,
            reset_occurred_at_ms: None,
        })?)
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Snapshot {
        state: (i64, i64, i64, i64, Option<i64>),
        counts: (i64, i64, i64, i64, i64, i64),
        current: (Vec<u8>, i64, Vec<u8>, i64),
        window: (i64, Vec<u8>, Vec<u8>, i64),
    }

    fn snapshot(store: &UsageStore) -> Result<Snapshot, StoreError> {
        let state = store.connection.query_row(
            "SELECT revision, retained_sample_count, retained_epoch_count,
                    retained_transition_count, last_published_at_ms
             FROM quota_state WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        let counts = store.connection.query_row(
            "SELECT
               (SELECT count(*) FROM quota_window_definition),
               (SELECT count(*) FROM quota_sample),
               (SELECT count(*) FROM quota_epoch_current),
               (SELECT count(*) FROM quota_epoch_history),
               (SELECT count(*) FROM quota_transition),
               (SELECT count(*) FROM quota_window_current)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;
        let current = store.connection.query_row(
            "SELECT epoch_id, definition_revision, last_observation_id,
                    last_transition_sequence
             FROM quota_epoch_current",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let window = store.connection.query_row(
            "SELECT definition_revision, sample_observation_id, epoch_id,
                    last_transition_sequence
             FROM quota_window_current",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        Ok(Snapshot {
            state,
            counts,
            current,
            window,
        })
    }

    #[test]
    fn every_quota_write_fault_rolls_back_definition_sample_epoch_transition_and_revision()
    -> TestResult {
        let mut store = UsageStore::in_memory()?;
        store.apply_quota_observation(&definition(1)?, &sample(1, 100, "epoch-1", 80)?)?;
        let before = snapshot(&store)?;
        let reset_definition = definition(2)?;
        let reset = sample(2, 200, "epoch-2", 10)?;

        for fault in [
            QuotaWriteFault::AfterSample,
            QuotaWriteFault::AfterEpoch,
            QuotaWriteFault::AfterTransition,
            QuotaWriteFault::AfterCurrent,
            QuotaWriteFault::AfterRevision,
        ] {
            let error = match store.apply_quota_observation_inner(&reset_definition, &reset, fault)
            {
                Ok(_) => return Err("injected quota fault unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(snapshot(&store)?, before);
        }

        let retry = store.apply_quota_observation(&reset_definition, &reset)?;
        assert_eq!(retry.status(), QuotaApplyStatus::Reset);
        assert_eq!(retry.quota_revision().get(), 2);
        assert_eq!(retry.transition_sequence(), 1);
        assert!(retry.transition_id().is_some());
        Ok(())
    }

    #[test]
    fn transition_sequence_beyond_sqlite_range_is_a_capacity_error_and_rolls_back() -> TestResult {
        let mut store = UsageStore::in_memory()?;
        let definition = definition(1)?;
        store.apply_quota_observation(&definition, &sample(3, 300, "epoch-1", 80)?)?;
        store.connection.execute(
            "UPDATE quota_epoch_current SET last_transition_sequence = ?1",
            params![i64::MAX],
        )?;
        store.connection.execute(
            "UPDATE quota_window_current SET last_transition_sequence = ?1",
            params![i64::MAX],
        )?;
        let before = snapshot(&store)?;

        let error = match store
            .apply_quota_observation(&definition, &sample(4, 400, "epoch-2", 10)?)
        {
            Ok(_) => return Err("overflowing transition sequence unexpectedly committed".into()),
            Err(error) => error,
        };
        assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
        assert_eq!(snapshot(&store)?, before);
        Ok(())
    }
}
