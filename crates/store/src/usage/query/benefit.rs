use std::{
    fmt,
    time::{Duration, Instant},
};

use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use tokenmaster_benefits::{
    BenefitChangeKind, BenefitCurrentLot, BenefitRevision, BenefitScopeId, benefit_scope_id,
};
use tokenmaster_domain::{
    BenefitInventoryCompleteness, BenefitLotId, BenefitLotObservation, BenefitObservationId,
    BenefitScope, NotificationChannel, ReminderLeadTime, ReminderProfile,
};

use super::{MAX_QUERY_DURATION, PROGRESS_OP_INTERVAL, UsageReadStore, map_sql};
use crate::usage::benefit_write::{
    decode_lot_revision_at, fixed_32, input_u64, invalid_stored, load_active_profile,
    map_core_error,
};
use crate::usage::{BenefitInventoryRevision, MAX_BENEFIT_CHANGES_PER_SCOPE};
use crate::{StoreError, StoreErrorCode};

pub const MAX_BENEFIT_CURRENT_LOTS: usize = tokenmaster_domain::MAX_BENEFIT_LOTS_PER_OBSERVATION;
pub const MAX_BENEFIT_CHANGE_PAGE_SIZE: usize = 256;

const LOT_REVISION_COLUMNS: &str = "
       revision.lot_id, revision.lot_revision, revision.kind,
       revision.quantity, revision.state, revision.target_kind,
       revision.target_window_id, revision.granted_at_ms,
       revision.expiry_kind, revision.expiry_exact_at_ms,
       revision.expiry_local_year, revision.expiry_local_month,
       revision.expiry_local_day, revision.expiry_local_hour,
       revision.expiry_local_minute, revision.expiry_local_second,
       revision.expiry_local_millisecond, revision.expiry_time_zone,
       revision.expiry_bounded_earliest_at_ms,
       revision.expiry_bounded_latest_at_ms, revision.source,
       revision.confidence, revision.detail_kind, revision.label_key";

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitCurrentQuery {
    scope: BenefitScope,
    deadline: Duration,
}

impl BenefitCurrentQuery {
    pub fn new(scope: BenefitScope, deadline: Duration) -> Result<Self, StoreError> {
        validate_deadline(deadline)?;
        Ok(Self { scope, deadline })
    }
}

impl fmt::Debug for BenefitCurrentQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitCurrentQuery")
            .field("scope", &"[redacted]")
            .field("deadline", &self.deadline)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitChangeCursor {
    benefit_revision: BenefitInventoryRevision,
    scope_id: BenefitScopeId,
    sequence: u64,
    change_id: [u8; 32],
}

impl fmt::Debug for BenefitChangeCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitChangeCursor")
            .field("benefit_revision", &self.benefit_revision)
            .field("sequence", &self.sequence)
            .field("scope", &"[redacted]")
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitChangePageQuery {
    scope: BenefitScope,
    expected_revision: Option<BenefitInventoryRevision>,
    before: Option<BenefitChangeCursor>,
    page_size: usize,
    deadline: Duration,
}

impl BenefitChangePageQuery {
    pub fn new(
        scope: BenefitScope,
        expected_revision: Option<BenefitInventoryRevision>,
        before: Option<BenefitChangeCursor>,
        page_size: usize,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        validate_deadline(deadline)?;
        let scope_id = benefit_scope_id(&scope);
        if page_size == 0
            || before.as_ref().is_some_and(|cursor| {
                expected_revision != Some(cursor.benefit_revision) || cursor.scope_id != scope_id
            })
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if page_size > MAX_BENEFIT_CHANGE_PAGE_SIZE {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_BENEFIT_CHANGE_PAGE_SIZE as u64,
            ));
        }
        Ok(Self {
            scope,
            expected_revision,
            before,
            page_size,
            deadline,
        })
    }
}

impl fmt::Debug for BenefitChangePageQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitChangePageQuery")
            .field("scope", &"[redacted]")
            .field("expected_revision", &self.expected_revision)
            .field("before", &self.before)
            .field("page_size", &self.page_size)
            .field("deadline", &self.deadline)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitScopeSnapshot {
    inventory_revision: BenefitRevision,
    last_change_sequence: u64,
    observation_id: BenefitObservationId,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    completeness: BenefitInventoryCompleteness,
}

impl BenefitScopeSnapshot {
    #[must_use]
    pub const fn inventory_revision(&self) -> BenefitRevision {
        self.inventory_revision
    }

    #[must_use]
    pub const fn last_change_sequence(&self) -> u64 {
        self.last_change_sequence
    }

    #[must_use]
    pub const fn observation_id(&self) -> BenefitObservationId {
        self.observation_id
    }

    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    #[must_use]
    pub const fn fresh_until_ms(&self) -> i64 {
        self.fresh_until_ms
    }

    #[must_use]
    pub const fn stale_after_ms(&self) -> i64 {
        self.stale_after_ms
    }

    #[must_use]
    pub const fn completeness(&self) -> BenefitInventoryCompleteness {
        self.completeness
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitReminderProfileSnapshot {
    profile: ReminderProfile,
    inherited: bool,
}

impl BenefitReminderProfileSnapshot {
    #[must_use]
    pub const fn profile(&self) -> &ReminderProfile {
        &self.profile
    }

    #[must_use]
    pub const fn inherited(&self) -> bool {
        self.inherited
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitDueSnapshot {
    lot_id: BenefitLotId,
    lot_revision: BenefitRevision,
    lead_time: ReminderLeadTime,
    channel: NotificationChannel,
    due_at_ms: i64,
    expiry_at_ms: i64,
}

impl BenefitDueSnapshot {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitCurrentCapture {
    benefit_revision: BenefitInventoryRevision,
    scope: Option<BenefitScopeSnapshot>,
    lots: Box<[BenefitCurrentLot]>,
    reminder_profile: BenefitReminderProfileSnapshot,
    nearest_due: Option<BenefitDueSnapshot>,
}

impl BenefitCurrentCapture {
    #[must_use]
    pub const fn benefit_revision(&self) -> BenefitInventoryRevision {
        self.benefit_revision
    }

    #[must_use]
    pub const fn scope(&self) -> Option<&BenefitScopeSnapshot> {
        self.scope.as_ref()
    }

    #[must_use]
    pub const fn lots(&self) -> &[BenefitCurrentLot] {
        &self.lots
    }

    #[must_use]
    pub const fn reminder_profile(&self) -> &BenefitReminderProfileSnapshot {
        &self.reminder_profile
    }

    #[must_use]
    pub const fn nearest_due(&self) -> Option<&BenefitDueSnapshot> {
        self.nearest_due.as_ref()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitChangeRecord {
    sequence: u64,
    lot_id: BenefitLotId,
    lot_revision: BenefitRevision,
    kind: BenefitChangeKind,
    before_revision: Option<BenefitRevision>,
    after_revision: Option<BenefitRevision>,
    before: Option<BenefitLotObservation>,
    after: Option<BenefitLotObservation>,
    observed_at_ms: i64,
    change_id: [u8; 32],
}

impl BenefitChangeRecord {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
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
    pub const fn kind(&self) -> BenefitChangeKind {
        self.kind
    }

    #[must_use]
    pub const fn before_revision(&self) -> Option<BenefitRevision> {
        self.before_revision
    }

    #[must_use]
    pub const fn after_revision(&self) -> Option<BenefitRevision> {
        self.after_revision
    }

    #[must_use]
    pub const fn before(&self) -> Option<&BenefitLotObservation> {
        self.before.as_ref()
    }

    #[must_use]
    pub const fn after(&self) -> Option<&BenefitLotObservation> {
        self.after.as_ref()
    }

    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    fn cursor(
        &self,
        benefit_revision: BenefitInventoryRevision,
        scope_id: BenefitScopeId,
    ) -> BenefitChangeCursor {
        BenefitChangeCursor {
            benefit_revision,
            scope_id,
            sequence: self.sequence,
            change_id: self.change_id,
        }
    }
}

impl fmt::Debug for BenefitChangeRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitChangeRecord")
            .field("sequence", &self.sequence)
            .field("lot_revision", &self.lot_revision)
            .field("kind", &self.kind)
            .field("has_before", &self.before.is_some())
            .field("has_after", &self.after.is_some())
            .field("observed_at_ms", &self.observed_at_ms)
            .field("lot_id", &"[redacted]")
            .field("change_id", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitChangePageCapture {
    benefit_revision: BenefitInventoryRevision,
    scope: Option<BenefitScopeSnapshot>,
    changes: Box<[BenefitChangeRecord]>,
    next_cursor: Option<BenefitChangeCursor>,
    has_more: bool,
}

impl BenefitChangePageCapture {
    #[must_use]
    pub const fn benefit_revision(&self) -> BenefitInventoryRevision {
        self.benefit_revision
    }

    #[must_use]
    pub const fn scope(&self) -> Option<&BenefitScopeSnapshot> {
        self.scope.as_ref()
    }

    #[must_use]
    pub const fn changes(&self) -> &[BenefitChangeRecord] {
        &self.changes
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&BenefitChangeCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

impl UsageReadStore {
    pub fn capture_benefit_current(
        &mut self,
        query: BenefitCurrentQuery,
    ) -> Result<BenefitCurrentCapture, StoreError> {
        self.capture_benefit_current_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_benefit_current_with_options<F>(
        &mut self,
        query: BenefitCurrentQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_revision: F,
    ) -> Result<BenefitCurrentCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        let progress_started = started;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || progress_started.elapsed() >= deadline),
        ))?;
        let result = capture_benefit_current(&mut self.connection, query, after_revision).and_then(
            |capture| {
                if started.elapsed() >= deadline {
                    Err(StoreError::new(StoreErrorCode::DeadlineExceeded))
                } else {
                    Ok(capture)
                }
            },
        );
        clear_progress_handler(&self.connection, result)
    }

    pub fn capture_benefit_changes(
        &mut self,
        query: BenefitChangePageQuery,
    ) -> Result<BenefitChangePageCapture, StoreError> {
        self.capture_benefit_changes_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_benefit_changes_with_options<F>(
        &mut self,
        query: BenefitChangePageQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_revision: F,
    ) -> Result<BenefitChangePageCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        let progress_started = started;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || progress_started.elapsed() >= deadline),
        ))?;
        let result = capture_benefit_changes(&mut self.connection, query, after_revision).and_then(
            |capture| {
                if started.elapsed() >= deadline {
                    Err(StoreError::new(StoreErrorCode::DeadlineExceeded))
                } else {
                    Ok(capture)
                }
            },
        );
        clear_progress_handler(&self.connection, result)
    }
}

fn clear_progress_handler<T>(
    connection: &rusqlite::Connection,
    result: Result<T, StoreError>,
) -> Result<T, StoreError> {
    let clear_result = map_sql(connection.progress_handler(0, None::<fn() -> bool>));
    match (result, clear_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn capture_benefit_current<F>(
    connection: &mut rusqlite::Connection,
    query: BenefitCurrentQuery,
    after_revision: F,
) -> Result<BenefitCurrentCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let benefit_revision = load_benefit_revision(&transaction)?;
    after_revision()?;
    let scope_id = benefit_scope_id(&query.scope);
    let scope = load_scope_snapshot(&transaction, &query.scope, scope_id)?;
    let lots = if let Some(scope) = &scope {
        load_current_lots(&transaction, scope_id, scope)?
    } else {
        Vec::new()
    };
    let reminder_profile = load_profile_snapshot(&transaction, scope_id)?;
    let nearest_due = if scope.is_some() {
        load_nearest_due(&transaction, scope_id)?
    } else {
        None
    };
    map_sql(transaction.commit())?;
    Ok(BenefitCurrentCapture {
        benefit_revision,
        scope,
        lots: lots.into_boxed_slice(),
        reminder_profile,
        nearest_due,
    })
}

fn capture_benefit_changes<F>(
    connection: &mut rusqlite::Connection,
    query: BenefitChangePageQuery,
    after_revision: F,
) -> Result<BenefitChangePageCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let benefit_revision = load_benefit_revision(&transaction)?;
    after_revision()?;
    if query
        .expected_revision
        .is_some_and(|expected| expected != benefit_revision)
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let scope_id = benefit_scope_id(&query.scope);
    let scope = load_scope_snapshot(&transaction, &query.scope, scope_id)?;
    let lookahead = query
        .page_size
        .checked_add(1)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut changes = if scope.is_some() {
        load_change_page(&transaction, scope_id, query.before.as_ref(), lookahead)?
    } else {
        Vec::new()
    };
    let has_more = changes.len() > query.page_size;
    if has_more {
        changes.truncate(query.page_size);
    }
    let next_cursor = if has_more {
        changes
            .last()
            .map(|change| change.cursor(benefit_revision, scope_id))
    } else {
        None
    };
    map_sql(transaction.commit())?;
    Ok(BenefitChangePageCapture {
        benefit_revision,
        scope,
        changes: changes.into_boxed_slice(),
        next_cursor,
        has_more,
    })
}

fn load_benefit_revision(
    transaction: &Transaction<'_>,
) -> Result<BenefitInventoryRevision, StoreError> {
    let stored = map_sql(transaction.query_row(
        "SELECT revision FROM benefit_state WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    ))?;
    BenefitInventoryRevision::from_stored(stored)
}

fn load_scope_snapshot(
    transaction: &Transaction<'_>,
    expected_scope: &BenefitScope,
    expected_scope_id: BenefitScopeId,
) -> Result<Option<BenefitScopeSnapshot>, StoreError> {
    let stored = map_sql(
        transaction
            .query_row(
                "SELECT provider_id, account_id, workspace_id, inventory_revision,
                        last_change_sequence, observation_id, observed_at_ms,
                        fresh_until_ms, stale_after_ms, completeness, current_lot_count
                 FROM benefit_scope WHERE scope_id = ?1",
                [expected_scope_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<Vec<u8>>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, i64>(10)?,
                    ))
                },
            )
            .optional(),
    )?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    if stored.0 != expected_scope.provider_id().as_str()
        || stored.1 != expected_scope.account_id().as_str()
        || stored.2.as_deref()
            != expected_scope
                .workspace_id()
                .map(tokenmaster_domain::QuotaWorkspaceId::as_str)
        || stored.10 < 0
        || stored.10 as usize > MAX_BENEFIT_CURRENT_LOTS
    {
        return Err(invalid_stored());
    }
    let observation_id = stored
        .5
        .map(|value| fixed_32(value).map(BenefitObservationId::from_bytes))
        .transpose()?;
    match (
        observation_id,
        stored.6,
        stored.7,
        stored.8,
        stored.9.as_deref(),
    ) {
        (Some(observation_id), Some(observed), Some(fresh), Some(stale), Some(completeness))
            if observed > 0 && observed <= fresh && fresh <= stale =>
        {
            Ok(Some(BenefitScopeSnapshot {
                inventory_revision: BenefitRevision::new(input_u64(stored.3)?)
                    .map_err(map_core_error)?,
                last_change_sequence: input_u64(stored.4)?,
                observation_id,
                observed_at_ms: observed,
                fresh_until_ms: fresh,
                stale_after_ms: stale,
                completeness: parse_completeness(completeness)?,
            }))
        }
        _ => Err(invalid_stored()),
    }
}

fn load_current_lots(
    transaction: &Transaction<'_>,
    scope_id: BenefitScopeId,
    scope: &BenefitScopeSnapshot,
) -> Result<Vec<BenefitCurrentLot>, StoreError> {
    let sql = format!(
        "SELECT {LOT_REVISION_COLUMNS},
                current.kind, current.quantity, current.state, current.detail_kind,
                current.conservative_expiry_at_ms
         FROM benefit_lot_current AS current
         JOIN benefit_lot_revision AS revision
           ON revision.scope_id = current.scope_id
          AND revision.lot_id = current.lot_id
          AND revision.lot_revision = current.lot_revision
         WHERE current.scope_id = ?1
         ORDER BY
           CASE WHEN current.conservative_expiry_at_ms IS NULL THEN 1 ELSE 0 END,
           current.conservative_expiry_at_ms,
           CASE current.kind
             WHEN 'banked_rate_limit_reset' THEN 1
             WHEN 'usage_credit' THEN 2
             WHEN 'temporary_usage' THEN 3
             WHEN 'unknown' THEN 4
           END,
           current.lot_id"
    );
    let mut statement = map_sql(transaction.prepare(&sql))?;
    let rows = map_benefit_rows(
        statement
            .query_map([scope_id.as_bytes().as_slice()], |row| {
                decode_current_projection(row)
            })?
            .collect::<Result<Vec<_>, _>>(),
    )?;
    if rows.len() > MAX_BENEFIT_CURRENT_LOTS
        || rows.len() != current_count(scope, transaction, scope_id)?
    {
        return Err(invalid_stored());
    }
    Ok(rows)
}

fn current_count(
    _scope: &BenefitScopeSnapshot,
    transaction: &Transaction<'_>,
    scope_id: BenefitScopeId,
) -> Result<usize, StoreError> {
    let stored = map_sql(transaction.query_row(
        "SELECT current_lot_count FROM benefit_scope WHERE scope_id = ?1",
        [scope_id.as_bytes().as_slice()],
        |row| row.get::<_, i64>(0),
    ))?;
    usize::try_from(stored).map_err(|_| invalid_stored())
}

fn decode_current_projection(row: &Row<'_>) -> rusqlite::Result<BenefitCurrentLot> {
    decode_current_projection_inner(row).map_err(|_error| rusqlite::Error::InvalidQuery)
}

fn decode_current_projection_inner(row: &Row<'_>) -> Result<BenefitCurrentLot, StoreError> {
    let (lot, revision) = decode_lot_revision_at(row, 0)?;
    let current_kind = row.get::<_, String>(24)?;
    let current_quantity = input_u64(row.get::<_, i64>(25)?)?;
    let current_state = row.get::<_, String>(26)?;
    let current_detail = row.get::<_, String>(27)?;
    let current_expiry = row.get::<_, Option<i64>>(28)?;
    if current_kind != kind_text(lot.kind())
        || current_quantity != lot.quantity()
        || current_state != state_text(lot.state())
        || current_detail != detail_text(lot.detail_kind())
        || current_expiry != lot.expiry().conservative_utc_ms()
    {
        return Err(invalid_stored());
    }
    BenefitCurrentLot::new(lot, revision).map_err(map_core_error)
}

fn load_profile_snapshot(
    transaction: &Transaction<'_>,
    scope_id: BenefitScopeId,
) -> Result<BenefitReminderProfileSnapshot, StoreError> {
    let inherited = map_sql(
        transaction
            .query_row(
                "SELECT 0 FROM benefit_reminder_profile
                 WHERE profile_kind = 'scope' AND profile_scope_id = ?1",
                [scope_id.as_bytes().as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .optional(),
    )?
    .is_none();
    Ok(BenefitReminderProfileSnapshot {
        profile: load_active_profile(transaction, scope_id.as_bytes())?,
        inherited,
    })
}

fn load_nearest_due(
    transaction: &Transaction<'_>,
    scope_id: BenefitScopeId,
) -> Result<Option<BenefitDueSnapshot>, StoreError> {
    let stored = map_sql(
        transaction
            .query_row(
                "SELECT lot_id, lot_revision, threshold_seconds, channel,
                        due_at_ms, expiry_at_ms
                 FROM benefit_reminder_due
                 WHERE scope_id = ?1
                 ORDER BY due_at_ms, expiry_at_ms, lot_id, channel
                 LIMIT 1",
                [scope_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional(),
    )?;
    stored
        .map(|stored| {
            let due_at_ms = stored.4;
            let expiry_at_ms = stored.5;
            if due_at_ms >= expiry_at_ms || expiry_at_ms <= 0 {
                return Err(invalid_stored());
            }
            Ok(BenefitDueSnapshot {
                lot_id: BenefitLotId::from_bytes(fixed_32(stored.0)?),
                lot_revision: BenefitRevision::new(input_u64(stored.1)?).map_err(map_core_error)?,
                lead_time: ReminderLeadTime::new(
                    u32::try_from(stored.2).map_err(|_| invalid_stored())?,
                )
                .map_err(|_| invalid_stored())?,
                channel: parse_channel(&stored.3)?,
                due_at_ms,
                expiry_at_ms,
            })
        })
        .transpose()
}

fn load_change_page(
    transaction: &Transaction<'_>,
    scope_id: BenefitScopeId,
    before: Option<&BenefitChangeCursor>,
    limit: usize,
) -> Result<Vec<BenefitChangeRecord>, StoreError> {
    if limit > MAX_BENEFIT_CHANGE_PAGE_SIZE + 1 || limit as u64 > MAX_BENEFIT_CHANGES_PER_SCOPE {
        return Err(StoreError::new(StoreErrorCode::CapacityExceeded));
    }
    let before_sequence = before
        .map(|cursor| i64::try_from(cursor.sequence))
        .transpose()
        .map_err(|_| invalid_stored())?;
    let before_change_id = before.map(|cursor| cursor.change_id.as_slice());
    let sql = change_page_sql();
    let mut statement = map_sql(transaction.prepare(&sql))?;
    let rows = map_benefit_rows(
        statement
            .query_map(
                params![
                    scope_id.as_bytes().as_slice(),
                    before_sequence,
                    before_change_id,
                    i64::try_from(limit)
                        .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?
                ],
                decode_change_record,
            )?
            .collect::<Result<Vec<_>, _>>(),
    )?;
    Ok(rows)
}

fn change_page_sql() -> String {
    let before = LOT_REVISION_COLUMNS.replace("revision.", "before_lot.");
    let after = LOT_REVISION_COLUMNS.replace("revision.", "after_lot.");
    format!(
        "SELECT change.change_id, change.sequence, change.lot_id,
                change.lot_revision, change.kind, change.before_revision,
                change.after_revision, change.observed_at_ms,
                {before}, {after}
         FROM benefit_change AS change
         LEFT JOIN benefit_lot_revision AS before_lot
           ON before_lot.scope_id = change.scope_id
          AND before_lot.lot_id = change.lot_id
          AND before_lot.lot_revision = change.before_revision
         LEFT JOIN benefit_lot_revision AS after_lot
           ON after_lot.scope_id = change.scope_id
          AND after_lot.lot_id = change.lot_id
          AND after_lot.lot_revision = change.after_revision
         WHERE change.scope_id = ?1
           AND (?2 IS NULL OR (change.sequence, change.change_id) < (?2, ?3))
         ORDER BY change.sequence DESC, change.change_id DESC
         LIMIT ?4"
    )
}

fn decode_change_record(row: &Row<'_>) -> rusqlite::Result<BenefitChangeRecord> {
    decode_change_record_inner(row).map_err(|_error| rusqlite::Error::InvalidQuery)
}

fn decode_change_record_inner(row: &Row<'_>) -> Result<BenefitChangeRecord, StoreError> {
    let change_id = fixed_32(row.get::<_, Vec<u8>>(0)?)?;
    let sequence = input_u64(row.get::<_, i64>(1)?)?;
    if sequence == 0 {
        return Err(invalid_stored());
    }
    let lot_id = BenefitLotId::from_bytes(fixed_32(row.get::<_, Vec<u8>>(2)?)?);
    let lot_revision =
        BenefitRevision::new(input_u64(row.get::<_, i64>(3)?)?).map_err(map_core_error)?;
    let kind = parse_change_kind(&row.get::<_, String>(4)?)?;
    let before_revision = row.get::<_, Option<i64>>(5)?.map(input_u64).transpose()?;
    let after_revision = row.get::<_, Option<i64>>(6)?.map(input_u64).transpose()?;
    let observed_at_ms = row.get::<_, i64>(7)?;
    if observed_at_ms <= 0 {
        return Err(invalid_stored());
    }
    let before = decode_optional_lot_revision(row, 8)?;
    let after = decode_optional_lot_revision(row, 32)?;
    if before_revision.is_some() != before.is_some()
        || after_revision.is_some() != after.is_some()
        || before.as_ref().is_some_and(|(lot, revision)| {
            lot.lot_id() != lot_id || revision.get() != before_revision.unwrap_or(0)
        })
        || after.as_ref().is_some_and(|(lot, revision)| {
            lot.lot_id() != lot_id || revision.get() != after_revision.unwrap_or(0)
        })
        || (after_revision.is_some() && lot_revision.get() != after_revision.unwrap_or(0))
        || (after_revision.is_none()
            && before_revision.and_then(|revision| revision.checked_add(1))
                != Some(lot_revision.get()))
    {
        return Err(invalid_stored());
    }
    Ok(BenefitChangeRecord {
        sequence,
        lot_id,
        lot_revision,
        kind,
        before_revision: before_revision
            .map(BenefitRevision::new)
            .transpose()
            .map_err(map_core_error)?,
        after_revision: after_revision
            .map(BenefitRevision::new)
            .transpose()
            .map_err(map_core_error)?,
        before: before.map(|value| value.0),
        after: after.map(|value| value.0),
        observed_at_ms,
        change_id,
    })
}

fn decode_optional_lot_revision(
    row: &Row<'_>,
    start: usize,
) -> Result<Option<(BenefitLotObservation, BenefitRevision)>, StoreError> {
    if row.get::<_, Option<Vec<u8>>>(start)?.is_none() {
        Ok(None)
    } else {
        decode_lot_revision_at(row, start).map(Some)
    }
}

fn validate_deadline(deadline: Duration) -> Result<(), StoreError> {
    if deadline.is_zero() || deadline > MAX_QUERY_DURATION {
        Err(StoreError::new(StoreErrorCode::InvalidValue))
    } else {
        Ok(())
    }
}

fn map_benefit_rows<T>(result: rusqlite::Result<T>) -> Result<T, StoreError> {
    match result {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::InvalidQuery) => Err(invalid_stored()),
        Err(error) => map_sql(Err(error)),
    }
}

fn parse_completeness(value: &str) -> Result<BenefitInventoryCompleteness, StoreError> {
    match value {
        "complete" => Ok(BenefitInventoryCompleteness::Complete),
        "complete_quantity_partial_details" => {
            Ok(BenefitInventoryCompleteness::CompleteQuantityPartialDetails)
        }
        "partial" => Ok(BenefitInventoryCompleteness::Partial),
        _ => Err(invalid_stored()),
    }
}

fn parse_channel(value: &str) -> Result<NotificationChannel, StoreError> {
    match value {
        "in_app" => Ok(NotificationChannel::InApp),
        "os_scheduled" => Ok(NotificationChannel::OsScheduled),
        _ => Err(invalid_stored()),
    }
}

fn parse_change_kind(value: &str) -> Result<BenefitChangeKind, StoreError> {
    match value {
        "awarded" => Ok(BenefitChangeKind::Awarded),
        "quantity_changed" => Ok(BenefitChangeKind::QuantityChanged),
        "state_changed" => Ok(BenefitChangeKind::StateChanged),
        "expiry_changed" => Ok(BenefitChangeKind::ExpiryChanged),
        "corrected" => Ok(BenefitChangeKind::Corrected),
        "disappeared_ambiguous" => Ok(BenefitChangeKind::DisappearedAmbiguous),
        "reappeared" => Ok(BenefitChangeKind::Reappeared),
        "retired_terminal" => Ok(BenefitChangeKind::RetiredTerminal),
        _ => Err(invalid_stored()),
    }
}

const fn kind_text(value: tokenmaster_domain::BenefitKind) -> &'static str {
    match value {
        tokenmaster_domain::BenefitKind::BankedRateLimitReset => "banked_rate_limit_reset",
        tokenmaster_domain::BenefitKind::UsageCredit => "usage_credit",
        tokenmaster_domain::BenefitKind::TemporaryUsage => "temporary_usage",
        tokenmaster_domain::BenefitKind::Unknown => "unknown",
    }
}

const fn state_text(value: tokenmaster_domain::BenefitState) -> &'static str {
    match value {
        tokenmaster_domain::BenefitState::Available => "available",
        tokenmaster_domain::BenefitState::ActivationPending => "activation_pending",
        tokenmaster_domain::BenefitState::Activated => "activated",
        tokenmaster_domain::BenefitState::Expired => "expired",
        tokenmaster_domain::BenefitState::Revoked => "revoked",
        tokenmaster_domain::BenefitState::Ambiguous => "ambiguous",
    }
}

const fn detail_text(value: tokenmaster_domain::BenefitDetailKind) -> &'static str {
    match value {
        tokenmaster_domain::BenefitDetailKind::ProviderDetail => "provider_detail",
        tokenmaster_domain::BenefitDetailKind::ProviderAggregate => "provider_aggregate",
        tokenmaster_domain::BenefitDetailKind::Manual => "manual",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{path::Path, thread};

    use tempfile::TempDir;
    use tokenmaster_domain::{
        BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
        BenefitInventoryCompleteness, BenefitInventoryObservation,
        BenefitInventoryObservationParts, BenefitKind, BenefitLabelKey, BenefitLotId,
        BenefitLotObservation, BenefitLotObservationParts, BenefitObservationId, BenefitScope,
        BenefitState, BenefitTarget, QuotaAccountId, UsageProviderId,
    };

    use super::*;
    use crate::usage::UsageStore;

    const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

    fn scope() -> BenefitScope {
        BenefitScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("query-private").expect("account"),
            None,
        )
    }

    fn observation(id: u8, quantity: u64) -> BenefitInventoryObservation {
        let lot = BenefitLotObservation::new(BenefitLotObservationParts {
            lot_id: BenefitLotId::from_bytes([1; 32]),
            kind: BenefitKind::UsageCredit,
            quantity,
            state: BenefitState::Available,
            target: BenefitTarget::Provider,
            granted_at_ms: None,
            expiry: BenefitExpiry::unknown(),
            source: BenefitEvidenceSource::ProviderLocal,
            confidence: BenefitConfidence::Medium,
            detail_kind: BenefitDetailKind::ProviderDetail,
            label_key: BenefitLabelKey::new("benefit.query").expect("label"),
        })
        .expect("lot");
        BenefitInventoryObservation::new(BenefitInventoryObservationParts {
            scope: scope(),
            observation_id: BenefitObservationId::from_bytes([id; 32]),
            observed_at_ms: OBSERVED_AT_MS + i64::from(id),
            fresh_until_ms: OBSERVED_AT_MS + i64::from(id) + 1_000,
            stale_after_ms: OBSERVED_AT_MS + i64::from(id) + 2_000,
            completeness: BenefitInventoryCompleteness::Complete,
            lots: vec![lot],
        })
        .expect("observation")
    }

    fn seed(path: &Path) {
        UsageStore::open(path)
            .expect("writer")
            .apply_benefit_observation(&observation(1, 1))
            .expect("seed observation");
    }

    #[test]
    fn benefit_read_transaction_keeps_revision_exact_during_concurrent_change() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("benefit-query-snapshot.sqlite3");
        seed(&path);
        let mut reader = UsageReadStore::open(&path).expect("reader");
        let writer_path = path.clone();
        let capture = reader
            .capture_benefit_current_with_options(
                BenefitCurrentQuery::new(scope(), Duration::from_secs(2)).expect("current query"),
                i32::MAX,
                false,
                move || {
                    thread::spawn(move || {
                        UsageStore::open(&writer_path)
                            .expect("concurrent writer")
                            .apply_benefit_observation(&observation(2, 2))
                            .expect("concurrent observation");
                    })
                    .join()
                    .expect("concurrent writer join");
                    Ok(())
                },
            )
            .expect("transaction-exact capture");
        assert_eq!(capture.benefit_revision().get(), 1);
        assert_eq!(capture.lots()[0].lot().quantity(), 1);

        let next = reader
            .capture_benefit_current(
                BenefitCurrentQuery::new(scope(), Duration::from_secs(2)).expect("next query"),
            )
            .expect("next capture");
        assert_eq!(next.benefit_revision().get(), 2);
        assert_eq!(next.lots()[0].lot().quantity(), 2);
    }

    #[test]
    fn benefit_progress_cancellation_is_cleared_for_the_next_query() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("benefit-query-cancel.sqlite3");
        seed(&path);
        let mut reader = UsageReadStore::open(&path).expect("reader");
        let error = reader
            .capture_benefit_current_with_options(
                BenefitCurrentQuery::new(scope(), Duration::from_secs(2)).expect("current query"),
                1,
                true,
                || Ok(()),
            )
            .expect_err("forced cancellation");
        assert_eq!(error.code(), StoreErrorCode::DeadlineExceeded);
        let next = reader
            .capture_benefit_current(
                BenefitCurrentQuery::new(scope(), Duration::from_secs(2)).expect("next query"),
            )
            .expect("next capture");
        assert_eq!(next.lots().len(), 1);
    }
}
