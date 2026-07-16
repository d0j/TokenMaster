use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use tokenmaster_benefits::{
    BenefitChange, BenefitChangeKind, BenefitCoreError, BenefitCurrentLot, BenefitInventoryState,
    BenefitReconciliationStatus, BenefitRevision, BenefitSequence, benefit_scope_id,
    reconcile_inventory, schedule_reminders,
};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitKind, BenefitLabelKey,
    BenefitLocalDate, BenefitLocalDateTime, BenefitLocalTime, BenefitLotId, BenefitLotObservation,
    BenefitLotObservationParts, BenefitObservationId, BenefitScope, BenefitState, BenefitTarget,
    BenefitTimeZoneId, NotificationChannel, QuotaAccountId, QuotaWindowId, QuotaWorkspaceId,
    ReminderLeadTime, ReminderProfile, ReminderProfileParts, ReminderProfileRevision,
    UsageProviderId,
};

use super::UsageStore;
use super::benefit_types::{
    BenefitApplyResult, BenefitApplyStatus, BenefitInventoryRevision, BenefitProfileApplyResult,
    MAX_BENEFIT_CHANGES_PER_SCOPE,
};
use crate::{StoreError, StoreErrorCode};

impl UsageStore {
    pub fn apply_benefit_observation(
        &mut self,
        observation: &BenefitInventoryObservation,
    ) -> Result<BenefitApplyResult, StoreError> {
        self.apply_benefit_observation_inner(observation, BenefitWriteFault::None)
    }

    fn apply_benefit_observation_inner(
        &mut self,
        observation: &BenefitInventoryObservation,
        fault: BenefitWriteFault,
    ) -> Result<BenefitApplyResult, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let global = load_global_state(&transaction)?;
        let scope_id = benefit_scope_id(observation.scope());
        ensure_scope(&transaction, observation.scope(), scope_id.as_bytes())?;
        let current = load_inventory_state(
            &transaction,
            observation.scope(),
            scope_id.as_bytes(),
            Some(observation.lots()),
        )?;
        let reconciliation = reconcile_inventory(&current, observation).map_err(map_core_error)?;
        let status = map_reconciliation_status(reconciliation.status());
        if matches!(
            reconciliation.status(),
            BenefitReconciliationStatus::Duplicate | BenefitReconciliationStatus::Stale
        ) {
            let due_count =
                count_scope_rows(&transaction, "benefit_reminder_due", scope_id.as_bytes())?;
            transaction.commit()?;
            return Ok(BenefitApplyResult::new(
                status,
                global.revision,
                0,
                output_u16(due_count)?,
            ));
        }

        let prior_current_count = i64::try_from(current.lots().len())
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let prior_due_count =
            count_scope_rows(&transaction, "benefit_reminder_due", scope_id.as_bytes())?;
        let retained_changes =
            count_scope_rows(&transaction, "benefit_change", scope_id.as_bytes())?;
        let next_change_count = retained_changes
            .checked_add(input_i64(reconciliation.changes().len())?)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        if next_change_count > MAX_BENEFIT_CHANGES_PER_SCOPE as i64 {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_BENEFIT_CHANGES_PER_SCOPE,
            ));
        }

        for change in reconciliation.changes() {
            if let Some(after) = change.after() {
                insert_lot_revision(
                    &transaction,
                    scope_id.as_bytes(),
                    change.lot_revision(),
                    after,
                )?;
            }
            insert_change(
                &transaction,
                scope_id.as_bytes(),
                observation.observed_at_ms(),
                change,
            )?;
        }
        benefit_write_fault(fault, BenefitWriteFault::AfterHistory)?;

        transaction.execute(
            "DELETE FROM benefit_reminder_due WHERE scope_id = ?1",
            [scope_id.as_bytes().as_slice()],
        )?;
        transaction.execute(
            "DELETE FROM benefit_lot_current WHERE scope_id = ?1",
            [scope_id.as_bytes().as_slice()],
        )?;
        for current_lot in reconciliation.state().lots() {
            insert_current_lot(&transaction, scope_id.as_bytes(), current_lot)?;
        }
        benefit_write_fault(fault, BenefitWriteFault::AfterCurrent)?;
        update_scope(
            &transaction,
            scope_id.as_bytes(),
            reconciliation.state(),
            observation,
        )?;
        let profile = load_active_profile(&transaction, scope_id.as_bytes())?;
        let next_due_count = rebuild_due(
            &transaction,
            observation.scope(),
            scope_id.as_bytes(),
            reconciliation.state().lots(),
            &profile,
        )?;
        benefit_write_fault(fault, BenefitWriteFault::AfterDue)?;
        let next_global_revision = global.revision.next()?;
        let next_current_count = checked_replace_count(
            global.current_lot_count,
            prior_current_count,
            input_i64(reconciliation.state().lots().len())?,
        )?;
        let next_global_due_count =
            checked_replace_count(global.pending_due_count, prior_due_count, next_due_count)?;
        let next_retained_change_count = global
            .retained_change_count
            .checked_add(input_i64(reconciliation.changes().len())?)
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        update_global_state(
            &transaction,
            next_global_revision,
            next_current_count,
            next_retained_change_count,
            next_global_due_count,
            global.retained_delivery_count,
            observation.observed_at_ms(),
        )?;
        benefit_write_fault(fault, BenefitWriteFault::AfterRevision)?;
        transaction.commit()?;
        Ok(BenefitApplyResult::new(
            status,
            next_global_revision,
            output_u16(input_i64(reconciliation.changes().len())?)?,
            output_u16(next_due_count)?,
        ))
    }

    pub fn set_benefit_reminder_override(
        &mut self,
        scope: &BenefitScope,
        profile: Option<&ReminderProfile>,
    ) -> Result<BenefitProfileApplyResult, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let global = load_global_state(&transaction)?;
        if global.revision.get() == 0 || global.last_published_at_ms.is_none() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let scope_id = benefit_scope_id(scope);
        let state = load_inventory_state(&transaction, scope, scope_id.as_bytes(), None)?;
        let prior_due_count =
            count_scope_rows(&transaction, "benefit_reminder_due", scope_id.as_bytes())?;
        transaction.execute(
            "DELETE FROM benefit_reminder_due WHERE scope_id = ?1",
            [scope_id.as_bytes().as_slice()],
        )?;
        transaction.execute(
            "DELETE FROM benefit_reminder_profile
             WHERE profile_kind = 'scope' AND profile_scope_id = ?1",
            [scope_id.as_bytes().as_slice()],
        )?;
        if let Some(profile) = profile {
            insert_profile_override(&transaction, scope_id.as_bytes(), profile)?;
        }
        let active = load_active_profile(&transaction, scope_id.as_bytes())?;
        let next_due_count = rebuild_due(
            &transaction,
            scope,
            scope_id.as_bytes(),
            state.lots(),
            &active,
        )?;
        let next_global_revision = global.revision.next()?;
        let next_global_due_count =
            checked_replace_count(global.pending_due_count, prior_due_count, next_due_count)?;
        update_global_state(
            &transaction,
            next_global_revision,
            global.current_lot_count,
            global.retained_change_count,
            next_global_due_count,
            global.retained_delivery_count,
            global
                .last_published_at_ms
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        )?;
        transaction.commit()?;
        Ok(BenefitProfileApplyResult::new(
            next_global_revision,
            output_u16(next_due_count)?,
        ))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum BenefitWriteFault {
    None,
    AfterHistory,
    AfterCurrent,
    AfterDue,
    AfterRevision,
}

fn benefit_write_fault(
    actual: BenefitWriteFault,
    boundary: BenefitWriteFault,
) -> Result<(), StoreError> {
    if actual == boundary {
        Err(StoreError::new(StoreErrorCode::Database))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct GlobalBenefitState {
    revision: BenefitInventoryRevision,
    current_lot_count: i64,
    retained_change_count: i64,
    pending_due_count: i64,
    retained_delivery_count: i64,
    last_published_at_ms: Option<i64>,
}

fn load_global_state(transaction: &Transaction<'_>) -> Result<GlobalBenefitState, StoreError> {
    let stored = transaction
        .query_row(
            "SELECT revision, current_lot_count, retained_change_count,
                    pending_due_count, retained_delivery_count, last_published_at_ms
             FROM benefit_state WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if stored.1 < 0
        || stored.2 < 0
        || stored.3 < 0
        || stored.4 < 0
        || (stored.0 == 0) != stored.5.is_none()
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(GlobalBenefitState {
        revision: BenefitInventoryRevision::from_stored(stored.0)?,
        current_lot_count: stored.1,
        retained_change_count: stored.2,
        pending_due_count: stored.3,
        retained_delivery_count: stored.4,
        last_published_at_ms: stored.5,
    })
}

fn ensure_scope(
    transaction: &Transaction<'_>,
    scope: &BenefitScope,
    scope_id: &[u8; 32],
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT OR IGNORE INTO benefit_scope(
           scope_id, provider_id, account_id, workspace_id, inventory_revision,
           last_change_sequence, observation_id, observed_at_ms, fresh_until_ms,
           stale_after_ms, completeness, current_lot_count
         ) VALUES (?1, ?2, ?3, ?4, 0, 0, NULL, NULL, NULL, NULL, NULL, 0)",
        params![
            scope_id.as_slice(),
            scope.provider_id().as_str(),
            scope.account_id().as_str(),
            scope.workspace_id().map(QuotaWorkspaceId::as_str),
        ],
    )?;
    let exact = transaction.query_row(
        "SELECT provider_id = ?2
                AND account_id = ?3
                AND workspace_id IS ?4
         FROM benefit_scope WHERE scope_id = ?1",
        params![
            scope_id.as_slice(),
            scope.provider_id().as_str(),
            scope.account_id().as_str(),
            scope.workspace_id().map(QuotaWorkspaceId::as_str),
        ],
        |row| row.get::<_, bool>(0),
    )?;
    if !exact {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    Ok(())
}

fn load_inventory_state(
    transaction: &Transaction<'_>,
    expected_scope: &BenefitScope,
    scope_id: &[u8; 32],
    observed_lots: Option<&[BenefitLotObservation]>,
) -> Result<BenefitInventoryState, StoreError> {
    let scope = transaction
        .query_row(
            "SELECT provider_id, account_id, workspace_id, inventory_revision,
                    last_change_sequence, observation_id, observed_at_ms, current_lot_count
             FROM benefit_scope WHERE scope_id = ?1",
            [scope_id.as_slice()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let stored_scope = BenefitScope::new(
        UsageProviderId::new(scope.0).map_err(|_| invalid_stored())?,
        QuotaAccountId::new(scope.1).map_err(|_| invalid_stored())?,
        scope
            .2
            .map(QuotaWorkspaceId::new)
            .transpose()
            .map_err(|_| invalid_stored())?,
    );
    if &stored_scope != expected_scope || scope.7 < 0 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let observation_id = scope
        .5
        .map(|bytes| fixed_32(bytes).map(BenefitObservationId::from_bytes))
        .transpose()?;
    let mut statement = transaction.prepare(
        "SELECT revision.lot_id, revision.lot_revision, revision.kind,
                revision.quantity, revision.state, revision.target_kind,
                revision.target_window_id, revision.granted_at_ms,
                revision.expiry_kind, revision.expiry_exact_at_ms,
                revision.expiry_local_year, revision.expiry_local_month,
                revision.expiry_local_day, revision.expiry_local_hour,
                revision.expiry_local_minute, revision.expiry_local_second,
                revision.expiry_local_millisecond, revision.expiry_time_zone,
                revision.expiry_bounded_earliest_at_ms,
                revision.expiry_bounded_latest_at_ms, revision.source,
                revision.confidence, revision.detail_kind, revision.label_key
         FROM benefit_lot_current AS current
         JOIN benefit_lot_revision AS revision
           ON revision.scope_id = current.scope_id
          AND revision.lot_id = current.lot_id
          AND revision.lot_revision = current.lot_revision
         WHERE current.scope_id = ?1
         ORDER BY current.lot_id",
    )?;
    let mut lots = statement
        .query_map([scope_id.as_slice()], decode_current_lot)?
        .collect::<Result<Vec<_>, _>>()?;
    if i64::try_from(lots.len()).ok() != Some(scope.7) {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    if let Some(observed_lots) = observed_lots {
        hydrate_retired_observed_lots(transaction, scope_id, observed_lots, &mut lots)?;
    }
    BenefitInventoryState::from_parts(
        stored_scope,
        BenefitRevision::new(input_u64(scope.3)?).map_err(map_core_error)?,
        BenefitSequence::new(input_u64(scope.4)?).map_err(map_core_error)?,
        observation_id,
        scope.6,
        lots,
    )
    .map_err(map_core_error)
}

fn hydrate_retired_observed_lots(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
    observed_lots: &[BenefitLotObservation],
    current_lots: &mut Vec<BenefitCurrentLot>,
) -> Result<(), StoreError> {
    let mut statement = transaction.prepare(
        "SELECT revision.lot_id, terminal.lot_revision, revision.kind,
                revision.quantity, 'ambiguous', revision.target_kind,
                revision.target_window_id, revision.granted_at_ms,
                revision.expiry_kind, revision.expiry_exact_at_ms,
                revision.expiry_local_year, revision.expiry_local_month,
                revision.expiry_local_day, revision.expiry_local_hour,
                revision.expiry_local_minute, revision.expiry_local_second,
                revision.expiry_local_millisecond, revision.expiry_time_zone,
                revision.expiry_bounded_earliest_at_ms,
                revision.expiry_bounded_latest_at_ms, revision.source,
                revision.confidence, revision.detail_kind, revision.label_key
         FROM benefit_change AS terminal
         JOIN benefit_lot_revision AS revision
           ON revision.scope_id = terminal.scope_id
          AND revision.lot_id = terminal.lot_id
          AND revision.lot_revision = terminal.before_revision
         WHERE terminal.scope_id = ?1
           AND terminal.lot_id = ?2
           AND terminal.kind = 'retired_terminal'
           AND terminal.after_revision IS NULL
           AND terminal.sequence = (
             SELECT max(latest.sequence)
             FROM benefit_change AS latest
             WHERE latest.scope_id = terminal.scope_id
               AND latest.lot_id = terminal.lot_id
           )
           AND NOT EXISTS (
             SELECT 1 FROM benefit_lot_current AS current
             WHERE current.scope_id = terminal.scope_id
               AND current.lot_id = terminal.lot_id
           )
         LIMIT 1",
    )?;
    for observed in observed_lots {
        if current_lots
            .iter()
            .any(|current| current.lot().lot_id() == observed.lot_id())
        {
            continue;
        }
        let retired = statement
            .query_row(
                params![scope_id.as_slice(), observed.lot_id().as_bytes().as_slice()],
                decode_current_lot,
            )
            .optional()?;
        if let Some(retired) = retired {
            current_lots.push(retired);
        }
    }
    Ok(())
}

fn decode_current_lot(row: &Row<'_>) -> rusqlite::Result<BenefitCurrentLot> {
    decode_current_lot_inner(row).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(error))
    })
}

fn decode_current_lot_inner(row: &Row<'_>) -> Result<BenefitCurrentLot, StoreError> {
    let lot_id = BenefitLotId::from_bytes(fixed_32(row.get::<_, Vec<u8>>(0)?)?);
    let revision =
        BenefitRevision::new(input_u64(row.get::<_, i64>(1)?)?).map_err(map_core_error)?;
    let kind = parse_kind(&row.get::<_, String>(2)?)?;
    let quantity = input_u64(row.get::<_, i64>(3)?)?;
    let state = parse_state(&row.get::<_, String>(4)?)?;
    let target_kind = row.get::<_, String>(5)?;
    let target_window = row.get::<_, Option<String>>(6)?;
    let target = match (target_kind.as_str(), target_window) {
        ("provider", None) => BenefitTarget::Provider,
        ("quota_window", Some(window_id)) => {
            BenefitTarget::QuotaWindow(QuotaWindowId::new(window_id).map_err(|_| invalid_stored())?)
        }
        _ => return Err(invalid_stored()),
    };
    let expiry = decode_expiry(
        &row.get::<_, String>(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
        row.get(18)?,
        row.get(19)?,
    )?;
    let lot = BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id,
        kind,
        quantity,
        state,
        target,
        granted_at_ms: row.get(7)?,
        expiry,
        source: parse_source(&row.get::<_, String>(20)?)?,
        confidence: parse_confidence(&row.get::<_, String>(21)?)?,
        detail_kind: parse_detail(&row.get::<_, String>(22)?)?,
        label_key: BenefitLabelKey::new(row.get::<_, String>(23)?).map_err(|_| invalid_stored())?,
    })
    .map_err(|_| invalid_stored())?;
    BenefitCurrentLot::new(lot, revision).map_err(map_core_error)
}

#[allow(clippy::too_many_arguments)]
fn decode_expiry(
    kind: &str,
    exact: Option<i64>,
    year: Option<i64>,
    month: Option<i64>,
    day: Option<i64>,
    hour: Option<i64>,
    minute: Option<i64>,
    second: Option<i64>,
    millisecond: Option<i64>,
    time_zone: Option<String>,
    earliest: Option<i64>,
    latest: Option<i64>,
) -> Result<BenefitExpiry, StoreError> {
    match kind {
        "exact_utc" => BenefitExpiry::exact_utc(required(exact)?).map_err(|_| invalid_stored()),
        "provider_local" => {
            let date = BenefitLocalDate::new(
                input_i32(required(year)?)?,
                input_u8(required(month)?)?,
                input_u8(required(day)?)?,
            )
            .map_err(|_| invalid_stored())?;
            let time = BenefitLocalTime::new(
                input_u8(required(hour)?)?,
                input_u8(required(minute)?)?,
                input_u8(required(second)?)?,
                input_u16(required(millisecond)?)?,
            )
            .map_err(|_| invalid_stored())?;
            let time_zone =
                BenefitTimeZoneId::new(required(time_zone)?).map_err(|_| invalid_stored())?;
            Ok(BenefitExpiry::provider_local(
                BenefitLocalDateTime::new(date, time),
                time_zone,
            ))
        }
        "provider_date" => {
            let date = BenefitLocalDate::new(
                input_i32(required(year)?)?,
                input_u8(required(month)?)?,
                input_u8(required(day)?)?,
            )
            .map_err(|_| invalid_stored())?;
            let time_zone = time_zone
                .map(BenefitTimeZoneId::new)
                .transpose()
                .map_err(|_| invalid_stored())?;
            Ok(BenefitExpiry::provider_date(date, time_zone))
        }
        "bounded_utc" => BenefitExpiry::bounded_utc(required(earliest)?, required(latest)?)
            .map_err(|_| invalid_stored()),
        "unknown" => Ok(BenefitExpiry::unknown()),
        _ => Err(invalid_stored()),
    }
}

fn insert_lot_revision(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
    revision: BenefitRevision,
    lot: &BenefitLotObservation,
) -> Result<(), StoreError> {
    let target_window_id = match lot.target() {
        BenefitTarget::Provider => None,
        BenefitTarget::QuotaWindow(window_id) => Some(window_id.as_str()),
    };
    let expiry = ExpiryColumns::from_expiry(lot.expiry());
    transaction.execute(
        "INSERT INTO benefit_lot_revision(
           scope_id, lot_id, lot_revision, kind, quantity, state,
           target_kind, target_window_id, granted_at_ms, expiry_kind,
           expiry_exact_at_ms, expiry_local_year, expiry_local_month,
           expiry_local_day, expiry_local_hour, expiry_local_minute,
           expiry_local_second, expiry_local_millisecond, expiry_time_zone,
           expiry_bounded_earliest_at_ms, expiry_bounded_latest_at_ms,
           source, confidence, detail_kind, label_key
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
           ?21, ?22, ?23, ?24, ?25
         )",
        params![
            scope_id.as_slice(),
            lot.lot_id().as_bytes().as_slice(),
            input_i64(revision.get())?,
            kind_text(lot.kind()),
            input_i64(lot.quantity())?,
            state_text(lot.state()),
            match lot.target() {
                BenefitTarget::Provider => "provider",
                BenefitTarget::QuotaWindow(_) => "quota_window",
            },
            target_window_id,
            lot.granted_at_ms(),
            expiry.kind,
            expiry.exact,
            expiry.year,
            expiry.month,
            expiry.day,
            expiry.hour,
            expiry.minute,
            expiry.second,
            expiry.millisecond,
            expiry.time_zone,
            expiry.earliest,
            expiry.latest,
            source_text(lot.source()),
            confidence_text(lot.confidence()),
            detail_text(lot.detail_kind()),
            lot.label_key(),
        ],
    )?;
    Ok(())
}

struct ExpiryColumns<'a> {
    kind: &'static str,
    exact: Option<i64>,
    year: Option<i32>,
    month: Option<u8>,
    day: Option<u8>,
    hour: Option<u8>,
    minute: Option<u8>,
    second: Option<u8>,
    millisecond: Option<u16>,
    time_zone: Option<&'a str>,
    earliest: Option<i64>,
    latest: Option<i64>,
}

impl<'a> ExpiryColumns<'a> {
    fn from_expiry(expiry: &'a BenefitExpiry) -> Self {
        match expiry {
            BenefitExpiry::ExactUtc { at_ms } => Self {
                kind: "exact_utc",
                exact: Some(*at_ms),
                year: None,
                month: None,
                day: None,
                hour: None,
                minute: None,
                second: None,
                millisecond: None,
                time_zone: None,
                earliest: None,
                latest: None,
            },
            BenefitExpiry::ProviderLocal { local, time_zone } => Self {
                kind: "provider_local",
                exact: None,
                year: Some(local.date().year()),
                month: Some(local.date().month()),
                day: Some(local.date().day()),
                hour: Some(local.time().hour()),
                minute: Some(local.time().minute()),
                second: Some(local.time().second()),
                millisecond: Some(local.time().millisecond()),
                time_zone: Some(time_zone.as_str()),
                earliest: None,
                latest: None,
            },
            BenefitExpiry::ProviderDate { date, time_zone } => Self {
                kind: "provider_date",
                exact: None,
                year: Some(date.year()),
                month: Some(date.month()),
                day: Some(date.day()),
                hour: None,
                minute: None,
                second: None,
                millisecond: None,
                time_zone: time_zone.as_ref().map(BenefitTimeZoneId::as_str),
                earliest: None,
                latest: None,
            },
            BenefitExpiry::BoundedUtc {
                earliest_at_ms,
                latest_at_ms,
            } => Self {
                kind: "bounded_utc",
                exact: None,
                year: None,
                month: None,
                day: None,
                hour: None,
                minute: None,
                second: None,
                millisecond: None,
                time_zone: None,
                earliest: Some(*earliest_at_ms),
                latest: Some(*latest_at_ms),
            },
            BenefitExpiry::Unknown => Self {
                kind: "unknown",
                exact: None,
                year: None,
                month: None,
                day: None,
                hour: None,
                minute: None,
                second: None,
                millisecond: None,
                time_zone: None,
                earliest: None,
                latest: None,
            },
        }
    }
}

fn insert_change(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
    observed_at_ms: i64,
    change: &BenefitChange,
) -> Result<(), StoreError> {
    let lot_revision = change.lot_revision().get();
    let before_revision = if change.before().is_some() {
        let logical_previous = lot_revision
            .checked_sub(1)
            .filter(|revision| *revision > 0)
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?;
        Some(resolve_material_before_revision(
            transaction,
            scope_id,
            change.lot_id(),
            logical_previous,
        )?)
    } else {
        None
    };
    let after_revision = change.after().map(|_| lot_revision);
    transaction.execute(
        "INSERT INTO benefit_change(
           change_id, scope_id, sequence, lot_id, lot_revision, kind,
           before_revision, after_revision, observed_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            change.id().as_bytes().as_slice(),
            scope_id.as_slice(),
            input_i64(change.sequence().get())?,
            change.lot_id().as_bytes().as_slice(),
            input_i64(lot_revision)?,
            change_kind_text(change.kind()),
            before_revision.map(input_i64).transpose()?,
            after_revision.map(input_i64).transpose()?,
            observed_at_ms,
        ],
    )?;
    Ok(())
}

fn resolve_material_before_revision(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
    lot_id: BenefitLotId,
    logical_previous: u64,
) -> Result<u64, StoreError> {
    let logical_previous = input_i64(logical_previous)?;
    let stored = transaction
        .query_row(
            "SELECT max(lot_revision)
             FROM benefit_lot_revision
             WHERE scope_id = ?1
               AND lot_id = ?2
               AND lot_revision <= ?3",
            params![
                scope_id.as_slice(),
                lot_id.as_bytes().as_slice(),
                logical_previous
            ],
            |row| row.get::<_, Option<i64>>(0),
        )?
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    input_u64(stored)
}

fn insert_current_lot(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
    current: &BenefitCurrentLot,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO benefit_lot_current(
           scope_id, lot_id, lot_revision, kind, quantity, state,
           detail_kind, conservative_expiry_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            scope_id.as_slice(),
            current.lot().lot_id().as_bytes().as_slice(),
            input_i64(current.revision().get())?,
            kind_text(current.lot().kind()),
            input_i64(current.lot().quantity())?,
            state_text(current.lot().state()),
            detail_text(current.lot().detail_kind()),
            current.lot().expiry().conservative_utc_ms(),
        ],
    )?;
    Ok(())
}

fn update_scope(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
    state: &BenefitInventoryState,
    observation: &BenefitInventoryObservation,
) -> Result<(), StoreError> {
    let changed = transaction.execute(
        "UPDATE benefit_scope
         SET inventory_revision = ?2,
             last_change_sequence = ?3,
             observation_id = ?4,
             observed_at_ms = ?5,
             fresh_until_ms = ?6,
             stale_after_ms = ?7,
             completeness = ?8,
             current_lot_count = ?9
         WHERE scope_id = ?1",
        params![
            scope_id.as_slice(),
            input_i64(state.revision().get())?,
            input_i64(state.last_change_sequence().get())?,
            observation.observation_id().as_bytes().as_slice(),
            observation.observed_at_ms(),
            observation.fresh_until_ms(),
            observation.stale_after_ms(),
            completeness_text(observation.completeness()),
            input_i64(state.lots().len())?,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn insert_profile_override(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
    profile: &ReminderProfile,
) -> Result<(), StoreError> {
    let in_app = i64::from(profile.channels().contains(&NotificationChannel::InApp));
    let os_scheduled = i64::from(
        profile
            .channels()
            .contains(&NotificationChannel::OsScheduled),
    );
    transaction.execute(
        "INSERT INTO benefit_reminder_profile(
           profile_kind, profile_scope_id, revision,
           channel_in_app, channel_os_scheduled
         ) VALUES ('scope', ?1, ?2, ?3, ?4)",
        params![
            scope_id.as_slice(),
            input_i64(profile.revision().get())?,
            in_app,
            os_scheduled,
        ],
    )?;
    for lead_time in profile.lead_times() {
        transaction.execute(
            "INSERT INTO benefit_reminder_threshold(
               profile_kind, profile_scope_id, threshold_seconds
             ) VALUES ('scope', ?1, ?2)",
            params![scope_id.as_slice(), i64::from(lead_time.seconds())],
        )?;
    }
    Ok(())
}

fn load_active_profile(
    transaction: &Transaction<'_>,
    scope_id: &[u8; 32],
) -> Result<ReminderProfile, StoreError> {
    let scope_profile = transaction
        .query_row(
            "SELECT revision, channel_in_app, channel_os_scheduled
             FROM benefit_reminder_profile
             WHERE profile_kind = 'scope' AND profile_scope_id = ?1",
            [scope_id.as_slice()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let (profile_kind, profile_scope_id, revision, in_app, os_scheduled) = match scope_profile {
        Some((revision, in_app, os_scheduled)) => {
            ("scope", scope_id.as_slice(), revision, in_app, os_scheduled)
        }
        None => {
            let global = transaction
                .query_row(
                    "SELECT revision, channel_in_app, channel_os_scheduled
                         FROM benefit_reminder_profile
                         WHERE profile_kind = 'global' AND length(profile_scope_id) = 0",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
            ("global", &[][..], global.0, global.1, global.2)
        }
    };
    if !matches!(in_app, 0 | 1) || !matches!(os_scheduled, 0 | 1) {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let mut statement = transaction.prepare(
        "SELECT threshold_seconds
         FROM benefit_reminder_threshold
         WHERE profile_kind = ?1 AND profile_scope_id = ?2
         ORDER BY threshold_seconds DESC",
    )?;
    let lead_times = statement
        .query_map(params![profile_kind, profile_scope_id], |row| {
            row.get::<_, i64>(0)
        })?
        .map(|value| {
            ReminderLeadTime::new(
                u32::try_from(value?)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, i64::MAX))?,
            )
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut channels = Vec::with_capacity(2);
    if in_app == 1 {
        channels.push(NotificationChannel::InApp);
    }
    if os_scheduled == 1 {
        channels.push(NotificationChannel::OsScheduled);
    }
    ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(input_u64(revision)?)
            .map_err(|_| invalid_stored())?,
        lead_times,
        channels,
    })
    .map_err(|_| invalid_stored())
}

fn rebuild_due(
    transaction: &Transaction<'_>,
    scope: &BenefitScope,
    scope_id: &[u8; 32],
    lots: &[BenefitCurrentLot],
    profile: &ReminderProfile,
) -> Result<i64, StoreError> {
    let scheduled = schedule_reminders(scope, lots, profile).map_err(map_core_error)?;
    for due in scheduled {
        transaction.execute(
            "INSERT INTO benefit_reminder_due(
               delivery_id, scope_id, lot_id, lot_revision, threshold_seconds,
               channel, due_at_ms, expiry_at_ms, profile_revision
             )
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9
             WHERE NOT EXISTS(
               SELECT 1 FROM benefit_reminder_delivery WHERE delivery_id = ?1
             )",
            params![
                due.delivery_id().as_bytes().as_slice(),
                scope_id.as_slice(),
                due.lot_id().as_bytes().as_slice(),
                input_i64(due.lot_revision().get())?,
                i64::from(due.lead_time().seconds()),
                channel_text(due.channel()),
                due.due_at_ms(),
                due.expiry_at_ms(),
                input_i64(profile.revision().get())?,
            ],
        )?;
    }
    count_scope_rows(transaction, "benefit_reminder_due", scope_id)
}

#[allow(clippy::too_many_arguments)]
fn update_global_state(
    transaction: &Transaction<'_>,
    revision: BenefitInventoryRevision,
    current_lot_count: i64,
    retained_change_count: i64,
    pending_due_count: i64,
    retained_delivery_count: i64,
    last_published_at_ms: i64,
) -> Result<(), StoreError> {
    if current_lot_count < 0
        || retained_change_count < 0
        || pending_due_count < 0
        || retained_delivery_count < 0
        || last_published_at_ms <= 0
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let changed = transaction.execute(
        "UPDATE benefit_state
         SET revision = ?1,
             current_lot_count = ?2,
             retained_change_count = ?3,
             pending_due_count = ?4,
             retained_delivery_count = ?5,
             last_published_at_ms = ?6
         WHERE singleton_id = 1",
        params![
            revision.as_sql(),
            current_lot_count,
            retained_change_count,
            pending_due_count,
            retained_delivery_count,
            last_published_at_ms,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn count_scope_rows(
    transaction: &Transaction<'_>,
    table: &str,
    scope_id: &[u8; 32],
) -> Result<i64, StoreError> {
    let sql = match table {
        "benefit_change" => "SELECT count(*) FROM benefit_change WHERE scope_id = ?1",
        "benefit_reminder_due" => "SELECT count(*) FROM benefit_reminder_due WHERE scope_id = ?1",
        _ => return Err(StoreError::new(StoreErrorCode::InvalidValue)),
    };
    Ok(transaction.query_row(sql, [scope_id.as_slice()], |row| row.get(0))?)
}

fn checked_replace_count(total: i64, old: i64, new: i64) -> Result<i64, StoreError> {
    total
        .checked_sub(old)
        .and_then(|value| value.checked_add(new))
        .filter(|value| *value >= 0)
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn fixed_32(bytes: Vec<u8>) -> Result<[u8; 32], StoreError> {
    bytes
        .try_into()
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn required<T>(value: Option<T>) -> Result<T, StoreError> {
    value.ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn input_i64(value: impl TryInto<i64>) -> Result<i64, StoreError> {
    value
        .try_into()
        .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn input_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| invalid_stored())
}

fn input_i32(value: i64) -> Result<i32, StoreError> {
    i32::try_from(value).map_err(|_| invalid_stored())
}

fn input_u16(value: i64) -> Result<u16, StoreError> {
    u16::try_from(value).map_err(|_| invalid_stored())
}

fn input_u8(value: i64) -> Result<u8, StoreError> {
    u8::try_from(value).map_err(|_| invalid_stored())
}

fn output_u16(value: i64) -> Result<u16, StoreError> {
    u16::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn map_core_error(error: BenefitCoreError) -> StoreError {
    let code = match error {
        BenefitCoreError::CapacityExceeded
        | BenefitCoreError::InvalidRevision
        | BenefitCoreError::InvalidSequence => StoreErrorCode::CapacityExceeded,
        BenefitCoreError::ScopeMismatch
        | BenefitCoreError::ConflictingObservationIdentity
        | BenefitCoreError::InvalidTime => StoreErrorCode::InvalidValue,
    };
    StoreError::new(code)
}

const fn invalid_stored() -> StoreError {
    StoreError::new(StoreErrorCode::InvalidStoredValue)
}

const fn map_reconciliation_status(status: BenefitReconciliationStatus) -> BenefitApplyStatus {
    match status {
        BenefitReconciliationStatus::Duplicate => BenefitApplyStatus::Duplicate,
        BenefitReconciliationStatus::Stale => BenefitApplyStatus::Stale,
        BenefitReconciliationStatus::FreshnessOnly => BenefitApplyStatus::FreshnessOnly,
        BenefitReconciliationStatus::Changed => BenefitApplyStatus::Changed,
    }
}

const fn kind_text(value: BenefitKind) -> &'static str {
    match value {
        BenefitKind::BankedRateLimitReset => "banked_rate_limit_reset",
        BenefitKind::UsageCredit => "usage_credit",
        BenefitKind::TemporaryUsage => "temporary_usage",
        BenefitKind::Unknown => "unknown",
    }
}

fn parse_kind(value: &str) -> Result<BenefitKind, StoreError> {
    match value {
        "banked_rate_limit_reset" => Ok(BenefitKind::BankedRateLimitReset),
        "usage_credit" => Ok(BenefitKind::UsageCredit),
        "temporary_usage" => Ok(BenefitKind::TemporaryUsage),
        "unknown" => Ok(BenefitKind::Unknown),
        _ => Err(invalid_stored()),
    }
}

const fn state_text(value: BenefitState) -> &'static str {
    match value {
        BenefitState::Available => "available",
        BenefitState::ActivationPending => "activation_pending",
        BenefitState::Activated => "activated",
        BenefitState::Expired => "expired",
        BenefitState::Revoked => "revoked",
        BenefitState::Ambiguous => "ambiguous",
    }
}

fn parse_state(value: &str) -> Result<BenefitState, StoreError> {
    match value {
        "available" => Ok(BenefitState::Available),
        "activation_pending" => Ok(BenefitState::ActivationPending),
        "activated" => Ok(BenefitState::Activated),
        "expired" => Ok(BenefitState::Expired),
        "revoked" => Ok(BenefitState::Revoked),
        "ambiguous" => Ok(BenefitState::Ambiguous),
        _ => Err(invalid_stored()),
    }
}

const fn source_text(value: BenefitEvidenceSource) -> &'static str {
    match value {
        BenefitEvidenceSource::ProviderOfficial => "provider_official",
        BenefitEvidenceSource::ProviderLocal => "provider_local",
        BenefitEvidenceSource::Manual => "manual",
        BenefitEvidenceSource::Unknown => "unknown",
    }
}

fn parse_source(value: &str) -> Result<BenefitEvidenceSource, StoreError> {
    match value {
        "provider_official" => Ok(BenefitEvidenceSource::ProviderOfficial),
        "provider_local" => Ok(BenefitEvidenceSource::ProviderLocal),
        "manual" => Ok(BenefitEvidenceSource::Manual),
        "unknown" => Ok(BenefitEvidenceSource::Unknown),
        _ => Err(invalid_stored()),
    }
}

const fn confidence_text(value: BenefitConfidence) -> &'static str {
    match value {
        BenefitConfidence::High => "high",
        BenefitConfidence::Medium => "medium",
        BenefitConfidence::Low => "low",
        BenefitConfidence::Unknown => "unknown",
    }
}

fn parse_confidence(value: &str) -> Result<BenefitConfidence, StoreError> {
    match value {
        "high" => Ok(BenefitConfidence::High),
        "medium" => Ok(BenefitConfidence::Medium),
        "low" => Ok(BenefitConfidence::Low),
        "unknown" => Ok(BenefitConfidence::Unknown),
        _ => Err(invalid_stored()),
    }
}

const fn detail_text(value: BenefitDetailKind) -> &'static str {
    match value {
        BenefitDetailKind::ProviderDetail => "provider_detail",
        BenefitDetailKind::ProviderAggregate => "provider_aggregate",
        BenefitDetailKind::Manual => "manual",
    }
}

fn parse_detail(value: &str) -> Result<BenefitDetailKind, StoreError> {
    match value {
        "provider_detail" => Ok(BenefitDetailKind::ProviderDetail),
        "provider_aggregate" => Ok(BenefitDetailKind::ProviderAggregate),
        "manual" => Ok(BenefitDetailKind::Manual),
        _ => Err(invalid_stored()),
    }
}

const fn completeness_text(value: BenefitInventoryCompleteness) -> &'static str {
    match value {
        BenefitInventoryCompleteness::Complete => "complete",
        BenefitInventoryCompleteness::CompleteQuantityPartialDetails => {
            "complete_quantity_partial_details"
        }
        BenefitInventoryCompleteness::Partial => "partial",
    }
}

const fn change_kind_text(value: BenefitChangeKind) -> &'static str {
    match value {
        BenefitChangeKind::Awarded => "awarded",
        BenefitChangeKind::QuantityChanged => "quantity_changed",
        BenefitChangeKind::StateChanged => "state_changed",
        BenefitChangeKind::ExpiryChanged => "expiry_changed",
        BenefitChangeKind::Corrected => "corrected",
        BenefitChangeKind::DisappearedAmbiguous => "disappeared_ambiguous",
        BenefitChangeKind::Reappeared => "reappeared",
        BenefitChangeKind::RetiredTerminal => "retired_terminal",
    }
}

const fn channel_text(value: NotificationChannel) -> &'static str {
    match value {
        NotificationChannel::InApp => "in_app",
        NotificationChannel::OsScheduled => "os_scheduled",
    }
}

#[cfg(test)]
mod tests {
    use tokenmaster_domain::{
        BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
        BenefitInventoryCompleteness, BenefitInventoryObservationParts, BenefitKind,
        BenefitLabelKey, BenefitLotId, BenefitLotObservationParts, BenefitObservationId,
        BenefitScope, BenefitState, BenefitTarget, QuotaAccountId, UsageProviderId,
    };

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

    fn scope() -> TestResult<BenefitScope> {
        Ok(BenefitScope::new(
            UsageProviderId::new("codex")?,
            QuotaAccountId::new("acct_private")?,
            None,
        ))
    }

    fn observation(id: u8, quantity: u64, expiry: i64) -> TestResult<BenefitInventoryObservation> {
        Ok(BenefitInventoryObservation::new(
            BenefitInventoryObservationParts {
                scope: scope()?,
                observation_id: BenefitObservationId::from_bytes([id; 32]),
                observed_at_ms: OBSERVED_AT_MS + i64::from(id),
                fresh_until_ms: OBSERVED_AT_MS + i64::from(id) + 1_000,
                stale_after_ms: OBSERVED_AT_MS + i64::from(id) + 2_000,
                completeness: BenefitInventoryCompleteness::Complete,
                lots: vec![BenefitLotObservation::new(BenefitLotObservationParts {
                    lot_id: BenefitLotId::from_bytes([7; 32]),
                    kind: BenefitKind::BankedRateLimitReset,
                    quantity,
                    state: BenefitState::Available,
                    target: BenefitTarget::Provider,
                    granted_at_ms: None,
                    expiry: BenefitExpiry::exact_utc(expiry)?,
                    source: BenefitEvidenceSource::ProviderOfficial,
                    confidence: BenefitConfidence::High,
                    detail_kind: BenefitDetailKind::ProviderDetail,
                    label_key: BenefitLabelKey::new("benefit.codex.banked_reset")?,
                })?],
            },
        )?)
    }

    fn snapshot(store: &UsageStore) -> TestResult<Vec<i64>> {
        Ok(store.connection.query_row(
            "SELECT
               (SELECT revision FROM benefit_state WHERE singleton_id = 1),
               (SELECT retained_change_count FROM benefit_state WHERE singleton_id = 1),
               (SELECT pending_due_count FROM benefit_state WHERE singleton_id = 1),
               (SELECT count(*) FROM benefit_scope),
               (SELECT count(*) FROM benefit_lot_revision),
               (SELECT count(*) FROM benefit_lot_current),
               (SELECT count(*) FROM benefit_change),
               (SELECT count(*) FROM benefit_reminder_due),
               (SELECT revision FROM quota_state WHERE singleton_id = 1),
               (SELECT dataset_generation FROM usage_archive_state WHERE singleton_id = 1),
               (SELECT count(*) FROM usage_event)",
            [],
            |row| (0..11).map(|index| row.get(index)).collect(),
        )?)
    }

    #[test]
    fn every_benefit_write_fault_rolls_back_history_current_due_and_revision() -> TestResult {
        for fault in [
            BenefitWriteFault::AfterHistory,
            BenefitWriteFault::AfterCurrent,
            BenefitWriteFault::AfterDue,
            BenefitWriteFault::AfterRevision,
        ] {
            let mut store = UsageStore::in_memory()?;
            store.apply_benefit_observation(&observation(
                1,
                1,
                OBSERVED_AT_MS + 10 * 24 * 60 * 60 * 1_000,
            )?)?;
            let before = snapshot(&store)?;
            let error = match store.apply_benefit_observation_inner(
                &observation(2, 2, OBSERVED_AT_MS + 20 * 24 * 60 * 60 * 1_000)?,
                fault,
            ) {
                Ok(_) => return Err("faulted benefit write unexpectedly committed".into()),
                Err(error) => error,
            };
            assert_eq!(error.code(), StoreErrorCode::Database);
            assert_eq!(snapshot(&store)?, before);
        }
        Ok(())
    }
}
