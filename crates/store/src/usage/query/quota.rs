use std::{
    fmt,
    time::{Duration, Instant},
};

use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
    QuotaResetThresholds, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId,
    QuotaWindowKey, QuotaWindowSemantics, QuotaWorkspaceId, UsageProviderId,
};
use tokenmaster_quota::{
    QuotaAllowanceChange, QuotaAllowanceChangeKind, QuotaDetectionTime, QuotaEpochId,
    QuotaEpochState, QuotaEpochStateParts, QuotaTransition, QuotaTransitionId, QuotaTransitionKind,
    QuotaTransitionParts, quota_epoch_id, quota_scope_id,
};

use super::{MAX_QUERY_DURATION, PROGRESS_OP_INTERVAL, UsageReadStore, map_sql};
use crate::usage::QuotaRevision;
use crate::{StoreError, StoreErrorCode};

pub const MAX_QUOTA_CURRENT_WINDOWS: usize = 32;
pub const MAX_QUOTA_TRANSITION_PAGE_SIZE: usize = 256;

const OVERVIEW_WINDOW_KEYS_SQL: &str = "SELECT
       current.scope_id, current.window_id, definition.provider_id,
       definition.account_id, definition.workspace_id
     FROM quota_window_current AS current
     JOIN quota_window_definition AS definition
       ON definition.scope_id = current.scope_id
      AND definition.window_id = current.window_id
      AND definition.revision = current.definition_revision
     ORDER BY current.scope_id, current.window_id
     LIMIT ?1";

const CURRENT_WINDOW_SQL: &str = "SELECT
       definition.revision, definition.provider_id, definition.account_id,
       definition.workspace_id, definition.label_key, definition.presentation,
       definition.semantics, definition.nominal_duration_seconds,
       definition.maximum_post_reset_used_ppm,
       definition.minimum_post_reset_remaining_ppm,
       definition.minimum_used_ratio_drop_ppm,
       sample.observation_id, sample.observed_at_ms, sample.fresh_until_ms,
       sample.stale_after_ms, sample.provider_epoch_id, sample.used_ratio_ppm,
       sample.remaining_ratio_ppm, sample.unit_id, sample.used_units,
       sample.remaining_units, sample.capacity_units, sample.advertised_resets_at_ms,
       sample.quality, sample.source, sample.confidence, sample.reset_evidence,
       sample.reset_occurred_at_ms,
       current.observed_at_ms, current.fresh_until_ms, current.stale_after_ms,
       current.quality, current.source, current.confidence,
       current.last_transition_sequence,
       epoch.epoch_definition_revision, epoch.definition_revision, epoch.epoch_id,
       epoch.first_observation_id, epoch.last_observation_id,
       epoch.first_observed_at_ms,
       epoch.last_observed_at_ms, epoch.maximum_used_ratio_ppm,
       epoch.maximum_used_ratio_observation_id, epoch.maximum_unit_id,
       epoch.maximum_used_units, epoch.maximum_remaining_units,
       epoch.maximum_capacity_units, epoch.maximum_used_units_observation_id,
       epoch.provider_epoch_id, epoch.advertised_resets_at_ms,
       epoch.last_transition_sequence,
       first_sample.observation_id, first_sample.observed_at_ms,
       first_sample.fresh_until_ms, first_sample.stale_after_ms,
       first_sample.provider_epoch_id, first_sample.used_ratio_ppm,
       first_sample.remaining_ratio_ppm, first_sample.unit_id,
       first_sample.used_units, first_sample.remaining_units,
       first_sample.capacity_units, first_sample.advertised_resets_at_ms,
       first_sample.quality, first_sample.source, first_sample.confidence,
       first_sample.reset_evidence, first_sample.reset_occurred_at_ms
     FROM quota_window_current AS current
     JOIN quota_window_definition AS definition
       ON definition.scope_id = current.scope_id
      AND definition.window_id = current.window_id
      AND definition.revision = current.definition_revision
     JOIN quota_sample AS sample
       ON sample.scope_id = current.scope_id
      AND sample.window_id = current.window_id
      AND sample.definition_revision = current.definition_revision
      AND sample.observation_id = current.sample_observation_id
     JOIN quota_epoch_current AS epoch
       ON epoch.scope_id = current.scope_id
      AND epoch.window_id = current.window_id
      AND epoch.definition_revision = current.definition_revision
      AND epoch.epoch_id = current.epoch_id
     JOIN quota_sample AS first_sample
       ON first_sample.scope_id = epoch.scope_id
      AND first_sample.window_id = epoch.window_id
      AND first_sample.definition_revision = epoch.epoch_definition_revision
      AND first_sample.observation_id = epoch.first_observation_id
     WHERE current.scope_id = ?1 AND current.window_id = ?2";

const TRANSITION_SELECT_SQL: &str = "SELECT
       transition.transition_id, transition.definition_revision,
       transition.sequence, transition.kind, transition.previous_epoch_id,
       transition.current_epoch_id, transition.pre_observation_id,
       transition.post_observation_id, transition.maximum_used_ratio_ppm,
       transition.maximum_used_ratio_observation_id, transition.maximum_unit_id,
       transition.maximum_used_units, transition.maximum_remaining_units,
       transition.maximum_capacity_units,
       transition.maximum_used_units_observation_id,
       transition.old_resets_at_ms, transition.new_resets_at_ms,
       transition.allowance_change_kind, transition.allowance_old_unit_id,
       transition.allowance_old_used_units, transition.allowance_old_remaining_units,
       transition.allowance_old_capacity_units, transition.allowance_new_unit_id,
       transition.allowance_new_used_units, transition.allowance_new_remaining_units,
       transition.allowance_new_capacity_units, transition.source,
       transition.confidence, transition.detection_time_kind,
       transition.exact_at_ms, transition.after_ms, transition.at_or_before_ms,
       pre.observation_id, pre.observed_at_ms, pre.fresh_until_ms,
       pre.stale_after_ms, pre.provider_epoch_id, pre.used_ratio_ppm,
       pre.remaining_ratio_ppm, pre.unit_id, pre.used_units,
       pre.remaining_units, pre.capacity_units, pre.advertised_resets_at_ms,
       pre.quality, pre.source, pre.confidence, pre.reset_evidence,
       pre.reset_occurred_at_ms,
       post.observation_id, post.observed_at_ms, post.fresh_until_ms,
       post.stale_after_ms, post.provider_epoch_id, post.used_ratio_ppm,
       post.remaining_ratio_ppm, post.unit_id, post.used_units,
       post.remaining_units, post.capacity_units, post.advertised_resets_at_ms,
       post.quality, post.source, post.confidence, post.reset_evidence,
       post.reset_occurred_at_ms
     FROM quota_transition AS transition
     JOIN quota_sample AS pre
       ON pre.scope_id = transition.scope_id
      AND pre.window_id = transition.window_id
      AND pre.observation_id = transition.pre_observation_id
     JOIN quota_sample AS post
       ON post.scope_id = transition.scope_id
      AND post.window_id = transition.window_id
      AND post.definition_revision = transition.definition_revision
      AND post.observation_id = transition.post_observation_id";

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaCurrentQuery {
    windows: Box<[QuotaWindowKey]>,
    deadline: Duration,
}

impl fmt::Debug for QuotaCurrentQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaCurrentQuery")
            .field("window_count", &self.windows.len())
            .field("filters", &"[redacted]")
            .field("deadline", &self.deadline)
            .finish()
    }
}

impl QuotaCurrentQuery {
    pub fn new(windows: Box<[QuotaWindowKey]>, deadline: Duration) -> Result<Self, StoreError> {
        if deadline.is_zero() || deadline > MAX_QUERY_DURATION {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if windows.len() > MAX_QUOTA_CURRENT_WINDOWS {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_QUOTA_CURRENT_WINDOWS as u64,
            ));
        }
        let mut windows = windows.into_vec();
        windows.sort_by(window_key_order);
        if windows.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            windows: windows.into_boxed_slice(),
            deadline,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotaOverviewQuery {
    deadline: Duration,
}

impl QuotaOverviewQuery {
    pub fn new(deadline: Duration) -> Result<Self, StoreError> {
        if deadline.is_zero() || deadline > MAX_QUERY_DURATION {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self { deadline })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaTransitionCursor {
    quota_revision: QuotaRevision,
    window: QuotaWindowKey,
    sequence: u64,
    transition_id: QuotaTransitionId,
}

impl fmt::Debug for QuotaTransitionCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaTransitionCursor")
            .field("quota_revision", &self.quota_revision)
            .field("sequence", &self.sequence)
            .field("filter", &"[redacted]")
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaTransitionPageQuery {
    window: QuotaWindowKey,
    expected_revision: Option<QuotaRevision>,
    before: Option<QuotaTransitionCursor>,
    page_size: usize,
    deadline: Duration,
}

impl fmt::Debug for QuotaTransitionPageQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaTransitionPageQuery")
            .field("filter", &"[redacted]")
            .field("expected_revision", &self.expected_revision)
            .field("before", &self.before)
            .field("page_size", &self.page_size)
            .field("deadline", &self.deadline)
            .finish()
    }
}

impl QuotaTransitionPageQuery {
    pub fn new(
        window: QuotaWindowKey,
        expected_revision: Option<QuotaRevision>,
        before: Option<QuotaTransitionCursor>,
        page_size: usize,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        if page_size == 0
            || deadline.is_zero()
            || deadline > MAX_QUERY_DURATION
            || before.as_ref().is_some_and(|cursor| {
                expected_revision != Some(cursor.quota_revision) || cursor.window != window
            })
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if page_size > MAX_QUOTA_TRANSITION_PAGE_SIZE {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_QUOTA_TRANSITION_PAGE_SIZE as u64,
            ));
        }
        Ok(Self {
            window,
            expected_revision,
            before,
            page_size,
            deadline,
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaCurrentEpoch {
    state: QuotaEpochState,
    first_sample: QuotaSample,
}

impl fmt::Debug for QuotaCurrentEpoch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaCurrentEpoch")
            .field(
                "epoch_definition_revision",
                &self.state.epoch_definition_revision(),
            )
            .field("definition_revision", &self.state.definition_revision())
            .field("first_observation_id", &self.first_sample.observation_id())
            .field("first_observed_at_ms", &self.state.first_observed_at_ms())
            .field("last_observed_at_ms", &self.state.last_observed_at_ms())
            .field("maximum_used_ratio", &self.state.maximum_used_ratio())
            .field("maximum_used_units", &self.state.maximum_used_units())
            .field(
                "last_transition_sequence",
                &self.state.last_transition_sequence(),
            )
            .field("filter", &"[redacted]")
            .finish()
    }
}

impl QuotaCurrentEpoch {
    #[must_use]
    pub const fn state(&self) -> &QuotaEpochState {
        &self.state
    }

    #[must_use]
    pub const fn first_sample(&self) -> &QuotaSample {
        &self.first_sample
    }

    #[must_use]
    pub const fn epoch_definition_revision(&self) -> u64 {
        self.state.epoch_definition_revision()
    }

    #[must_use]
    pub const fn definition_revision(&self) -> u64 {
        self.state.definition_revision()
    }

    #[must_use]
    pub const fn maximum_used_ratio(&self) -> Option<QuotaRatio> {
        self.state.maximum_used_ratio()
    }

    #[must_use]
    pub const fn maximum_used_units(&self) -> Option<&QuotaUnits> {
        self.state.maximum_used_units()
    }

    #[must_use]
    pub const fn provider_epoch_id(&self) -> Option<&QuotaProviderEpochId> {
        self.state.provider_epoch_id()
    }

    #[must_use]
    pub const fn advertised_resets_at_ms(&self) -> Option<i64> {
        self.state.advertised_resets_at_ms()
    }

    #[must_use]
    pub const fn last_transition_sequence(&self) -> u64 {
        self.state.last_transition_sequence()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaTransitionRecord {
    transition: QuotaTransition,
    pre_sample: QuotaSample,
    post_sample: QuotaSample,
}

impl fmt::Debug for QuotaTransitionRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaTransitionRecord")
            .field(
                "definition_revision",
                &self.transition.definition_revision(),
            )
            .field("sequence", &self.transition.sequence())
            .field("kind", &self.transition.kind())
            .field("pre_observation_id", &self.pre_sample.observation_id())
            .field("post_observation_id", &self.post_sample.observation_id())
            .field(
                "maximum_used_ratio_before",
                &self.transition.maximum_used_ratio_before(),
            )
            .field(
                "maximum_used_units_before",
                &self.transition.maximum_used_units_before(),
            )
            .field("source", &self.transition.source())
            .field("confidence", &self.transition.confidence())
            .field("detection_time", &self.transition.detection_time())
            .field("filter", &"[redacted]")
            .field("identity", &"[redacted]")
            .finish()
    }
}

impl QuotaTransitionRecord {
    #[must_use]
    pub const fn transition(&self) -> &QuotaTransition {
        &self.transition
    }

    #[must_use]
    pub const fn definition_revision(&self) -> u64 {
        self.transition.definition_revision()
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.transition.sequence()
    }

    #[must_use]
    pub const fn kind(&self) -> QuotaTransitionKind {
        self.transition.kind()
    }

    #[must_use]
    pub const fn pre_sample(&self) -> &QuotaSample {
        &self.pre_sample
    }

    #[must_use]
    pub const fn post_sample(&self) -> &QuotaSample {
        &self.post_sample
    }

    #[must_use]
    pub const fn maximum_used_ratio_before(&self) -> Option<QuotaRatio> {
        self.transition.maximum_used_ratio_before()
    }

    #[must_use]
    pub const fn maximum_used_units_before(&self) -> Option<&QuotaUnits> {
        self.transition.maximum_used_units_before()
    }

    #[must_use]
    pub const fn old_resets_at_ms(&self) -> Option<i64> {
        self.transition.old_resets_at_ms()
    }

    #[must_use]
    pub const fn new_resets_at_ms(&self) -> Option<i64> {
        self.transition.new_resets_at_ms()
    }

    #[must_use]
    pub const fn allowance_change(&self) -> Option<&QuotaAllowanceChange> {
        self.transition.allowance_change()
    }

    #[must_use]
    pub const fn source(&self) -> QuotaEvidenceSource {
        self.transition.source()
    }

    #[must_use]
    pub const fn confidence(&self) -> QuotaConfidence {
        self.transition.confidence()
    }

    #[must_use]
    pub const fn detection_time(&self) -> QuotaDetectionTime {
        self.transition.detection_time()
    }

    fn cursor(
        &self,
        quota_revision: QuotaRevision,
        window: &QuotaWindowKey,
    ) -> QuotaTransitionCursor {
        QuotaTransitionCursor {
            quota_revision,
            window: window.clone(),
            sequence: self.transition.sequence(),
            transition_id: self.transition.id(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaCurrentWindow {
    definition: QuotaWindowDefinition,
    sample: QuotaSample,
    epoch: QuotaCurrentEpoch,
    last_transition: Option<QuotaTransitionRecord>,
}

impl fmt::Debug for QuotaCurrentWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaCurrentWindow")
            .field("definition_revision", &self.definition.revision())
            .field("current_observation_id", &self.sample.observation_id())
            .field("epoch", &self.epoch)
            .field("last_transition", &self.last_transition)
            .field("filter", &"[redacted]")
            .finish()
    }
}

impl QuotaCurrentWindow {
    #[must_use]
    pub const fn definition(&self) -> &QuotaWindowDefinition {
        &self.definition
    }

    #[must_use]
    pub const fn sample(&self) -> &QuotaSample {
        &self.sample
    }

    #[must_use]
    pub const fn epoch(&self) -> &QuotaCurrentEpoch {
        &self.epoch
    }

    #[must_use]
    pub const fn last_transition(&self) -> Option<&QuotaTransitionRecord> {
        self.last_transition.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaCurrentCapture {
    quota_revision: QuotaRevision,
    windows: Box<[QuotaCurrentWindow]>,
}

impl QuotaCurrentCapture {
    #[must_use]
    pub const fn quota_revision(&self) -> QuotaRevision {
        self.quota_revision
    }

    #[must_use]
    pub const fn windows(&self) -> &[QuotaCurrentWindow] {
        &self.windows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaTransitionPageCapture {
    quota_revision: QuotaRevision,
    transitions: Box<[QuotaTransitionRecord]>,
    next_cursor: Option<QuotaTransitionCursor>,
    has_more: bool,
}

impl QuotaTransitionPageCapture {
    #[must_use]
    pub const fn quota_revision(&self) -> QuotaRevision {
        self.quota_revision
    }

    #[must_use]
    pub const fn transitions(&self) -> &[QuotaTransitionRecord] {
        &self.transitions
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&QuotaTransitionCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

impl UsageReadStore {
    pub fn capture_quota_windows(
        &mut self,
        query: QuotaCurrentQuery,
    ) -> Result<QuotaCurrentCapture, StoreError> {
        self.capture_quota_windows_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    pub fn capture_quota_overview(
        &mut self,
        query: QuotaOverviewQuery,
    ) -> Result<QuotaCurrentCapture, StoreError> {
        self.capture_quota_overview_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_quota_overview_with_options<F>(
        &mut self,
        query: QuotaOverviewQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_revision: F,
    ) -> Result<QuotaCurrentCapture, StoreError>
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
        let result =
            capture_quota_overview(&mut self.connection, after_revision).and_then(|capture| {
                if started.elapsed() >= deadline {
                    Err(StoreError::new(StoreErrorCode::DeadlineExceeded))
                } else {
                    Ok(capture)
                }
            });
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    fn capture_quota_windows_with_options<F>(
        &mut self,
        query: QuotaCurrentQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_revision: F,
    ) -> Result<QuotaCurrentCapture, StoreError>
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
        let result = capture_quota_windows(&mut self.connection, query, after_revision).and_then(
            |capture| {
                if started.elapsed() >= deadline {
                    Err(StoreError::new(StoreErrorCode::DeadlineExceeded))
                } else {
                    Ok(capture)
                }
            },
        );
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    pub fn capture_quota_transitions(
        &mut self,
        query: QuotaTransitionPageQuery,
    ) -> Result<QuotaTransitionPageCapture, StoreError> {
        self.capture_quota_transitions_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_quota_transitions_with_options<F>(
        &mut self,
        query: QuotaTransitionPageQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_revision: F,
    ) -> Result<QuotaTransitionPageCapture, StoreError>
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
        let result = capture_quota_transitions(&mut self.connection, query, after_revision)
            .and_then(|capture| {
                if started.elapsed() >= deadline {
                    Err(StoreError::new(StoreErrorCode::DeadlineExceeded))
                } else {
                    Ok(capture)
                }
            });
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

fn capture_quota_overview<F>(
    connection: &mut rusqlite::Connection,
    after_revision: F,
) -> Result<QuotaCurrentCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let quota_revision = load_quota_revision(&transaction)?;
    after_revision()?;
    let keys = load_overview_window_keys(&transaction)?;
    let mut windows = Vec::with_capacity(keys.len());
    for key in keys {
        let current = load_current_window(&transaction, &key)?
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        windows.push(current);
    }
    map_sql(transaction.commit())?;
    Ok(QuotaCurrentCapture {
        quota_revision,
        windows: windows.into_boxed_slice(),
    })
}

fn load_overview_window_keys(
    transaction: &Transaction<'_>,
) -> Result<Vec<QuotaWindowKey>, StoreError> {
    let lookahead = MAX_QUOTA_CURRENT_WINDOWS
        .checked_add(1)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let sql_limit =
        i64::try_from(lookahead).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let rows = map_quota_row((|| -> rusqlite::Result<Vec<QuotaWindowKey>> {
        let mut statement = transaction.prepare(OVERVIEW_WINDOW_KEYS_SQL)?;
        let mapped = statement.query_map([sql_limit], |row| {
            let stored_scope_id = stored_bytes(row.get(0)?)?;
            let window_id = stored_domain(QuotaWindowId::new(row.get::<_, String>(1)?))?;
            let provider_id = stored_domain(UsageProviderId::new(row.get::<_, String>(2)?))?;
            let account_id = stored_domain(QuotaAccountId::new(row.get::<_, String>(3)?))?;
            let workspace_id = row
                .get::<_, Option<String>>(4)?
                .map(QuotaWorkspaceId::new)
                .transpose()
                .map_err(stored_domain_error)?;
            let key = QuotaWindowKey::new(
                QuotaScope::new(provider_id, account_id, workspace_id),
                window_id,
            );
            if quota_scope_id(key.scope()).as_bytes() != &stored_scope_id {
                return Err(invalid_stored_sql());
            }
            Ok(key)
        })?;
        mapped.collect()
    })())?;
    if rows.len() > MAX_QUOTA_CURRENT_WINDOWS {
        return Err(StoreError::with_limit(
            StoreErrorCode::CapacityExceeded,
            MAX_QUOTA_CURRENT_WINDOWS as u64,
        ));
    }
    Ok(rows)
}

fn capture_quota_windows<F>(
    connection: &mut rusqlite::Connection,
    query: QuotaCurrentQuery,
    after_revision: F,
) -> Result<QuotaCurrentCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let quota_revision = load_quota_revision(&transaction)?;
    after_revision()?;
    let mut windows = Vec::with_capacity(query.windows.len());
    for window in query.windows.iter() {
        if let Some(current) = load_current_window(&transaction, window)? {
            windows.push(current);
        }
    }
    map_sql(transaction.commit())?;
    Ok(QuotaCurrentCapture {
        quota_revision,
        windows: windows.into_boxed_slice(),
    })
}

fn capture_quota_transitions<F>(
    connection: &mut rusqlite::Connection,
    query: QuotaTransitionPageQuery,
    after_revision: F,
) -> Result<QuotaTransitionPageCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let quota_revision = load_quota_revision(&transaction)?;
    after_revision()?;
    if query
        .expected_revision
        .is_some_and(|expected| expected != quota_revision)
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let lookahead = query
        .page_size
        .checked_add(1)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut transitions = load_transition_page(&transaction, &query, lookahead)?;
    let has_more = transitions.len() > query.page_size;
    if has_more {
        transitions.truncate(query.page_size);
    }
    let next_cursor = if has_more {
        transitions
            .last()
            .map(|transition| transition.cursor(quota_revision, &query.window))
    } else {
        None
    };
    map_sql(transaction.commit())?;
    Ok(QuotaTransitionPageCapture {
        quota_revision,
        transitions: transitions.into_boxed_slice(),
        next_cursor,
        has_more,
    })
}

fn load_quota_revision(transaction: &Transaction<'_>) -> Result<QuotaRevision, StoreError> {
    let stored = map_sql(transaction.query_row(
        "SELECT revision FROM quota_state WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    ))?;
    QuotaRevision::from_stored(stored)
}

fn load_current_window(
    transaction: &Transaction<'_>,
    key: &QuotaWindowKey,
) -> Result<Option<QuotaCurrentWindow>, StoreError> {
    let scope_id = quota_scope_id(key.scope());
    let stored = map_quota_row(
        transaction
            .query_row(
                CURRENT_WINDOW_SQL,
                params![scope_id.as_bytes().as_slice(), key.window_id().as_str()],
                |row| {
                    let definition = restore_definition(row, 0, key)?;
                    let sample = restore_sample(row, 11, key)?;
                    let first_sample = restore_sample(row, 52, key)?;
                    let epoch = restore_current_epoch(row, 35, key, &sample, &first_sample)?;
                    validate_current_projection(row, 28, &sample, &epoch)?;
                    Ok((definition, sample, epoch))
                },
            )
            .optional(),
    )?;
    let Some((definition, sample, epoch)) = stored else {
        return Ok(None);
    };
    let last_transition = if epoch.last_transition_sequence() == 0 {
        None
    } else {
        Some(
            load_transition_by_sequence(transaction, key, epoch.last_transition_sequence())?
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        )
    };
    Ok(Some(QuotaCurrentWindow {
        definition,
        sample,
        epoch,
        last_transition,
    }))
}

fn load_transition_by_sequence(
    transaction: &Transaction<'_>,
    key: &QuotaWindowKey,
    sequence: u64,
) -> Result<Option<QuotaTransitionRecord>, StoreError> {
    let scope_id = quota_scope_id(key.scope());
    let sequence =
        i64::try_from(sequence).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let sql = format!(
        "{TRANSITION_SELECT_SQL}
         WHERE transition.scope_id = ?1 AND transition.window_id = ?2
           AND transition.sequence = ?3"
    );
    map_quota_row(
        transaction
            .query_row(
                &sql,
                params![
                    scope_id.as_bytes().as_slice(),
                    key.window_id().as_str(),
                    sequence
                ],
                |row| restore_transition_record(row, key),
            )
            .optional(),
    )
}

fn load_transition_page(
    transaction: &Transaction<'_>,
    query: &QuotaTransitionPageQuery,
    limit: usize,
) -> Result<Vec<QuotaTransitionRecord>, StoreError> {
    let scope_id = quota_scope_id(query.window.scope());
    let limit =
        i64::try_from(limit).map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    let mut transitions = Vec::with_capacity(limit as usize);
    if let Some(cursor) = query.before.as_ref() {
        let sequence = i64::try_from(cursor.sequence)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?;
        let sql = transition_page_sql(true);
        let mut statement = map_sql(transaction.prepare(&sql))?;
        let rows = map_sql(statement.query_map(
            params![
                scope_id.as_bytes().as_slice(),
                query.window.window_id().as_str(),
                sequence,
                cursor.transition_id.as_bytes().as_slice(),
                limit
            ],
            |row| restore_transition_record(row, &query.window),
        ))?;
        for row in rows {
            transitions.push(map_quota_row(row)?);
        }
    } else {
        let sql = transition_page_sql(false);
        let mut statement = map_sql(transaction.prepare(&sql))?;
        let rows = map_sql(statement.query_map(
            params![
                scope_id.as_bytes().as_slice(),
                query.window.window_id().as_str(),
                limit
            ],
            |row| restore_transition_record(row, &query.window),
        ))?;
        for row in rows {
            transitions.push(map_quota_row(row)?);
        }
    }
    Ok(transitions)
}

fn transition_page_sql(cursor: bool) -> String {
    let predicate = if cursor {
        "AND (transition.sequence < ?3
          OR (transition.sequence = ?3 AND transition.transition_id < ?4))"
    } else {
        ""
    };
    let limit = if cursor { "?5" } else { "?3" };
    format!(
        "{TRANSITION_SELECT_SQL}
         WHERE transition.scope_id = ?1 AND transition.window_id = ?2
         {predicate}
         ORDER BY transition.sequence DESC, transition.transition_id DESC
         LIMIT {limit}"
    )
}

fn restore_definition(
    row: &Row<'_>,
    offset: usize,
    expected_key: &QuotaWindowKey,
) -> rusqlite::Result<QuotaWindowDefinition> {
    let revision = stored_u64(row.get(offset)?)?;
    let provider_id = stored_domain(UsageProviderId::new(row.get::<_, String>(offset + 1)?))?;
    let account_id = stored_domain(QuotaAccountId::new(row.get::<_, String>(offset + 2)?))?;
    let workspace_id = row
        .get::<_, Option<String>>(offset + 3)?
        .map(QuotaWorkspaceId::new)
        .transpose()
        .map_err(stored_domain_error)?;
    let key = QuotaWindowKey::new(
        QuotaScope::new(provider_id, account_id, workspace_id),
        stored_domain(QuotaWindowId::new(
            expected_key.window_id().as_str().to_owned(),
        ))?,
    );
    if &key != expected_key {
        return Err(invalid_stored_sql());
    }
    let thresholds = stored_thresholds(
        row.get(offset + 8)?,
        row.get(offset + 9)?,
        row.get(offset + 10)?,
    )?;
    stored_domain(QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key,
        revision,
        label_key: row.get(offset + 4)?,
        presentation: stored_presentation(&row.get::<_, String>(offset + 5)?)?,
        semantics: stored_semantics(&row.get::<_, String>(offset + 6)?)?,
        nominal_duration_seconds: stored_optional_u64(row.get(offset + 7)?)?,
        reset_thresholds: thresholds,
    }))
}

fn restore_sample(
    row: &Row<'_>,
    offset: usize,
    key: &QuotaWindowKey,
) -> rusqlite::Result<QuotaSample> {
    let observation_id = QuotaObservationId::from_bytes(stored_bytes(row.get(offset)?)?);
    let units = stored_units(
        row.get(offset + 7)?,
        row.get(offset + 8)?,
        row.get(offset + 9)?,
        row.get(offset + 10)?,
    )?;
    stored_domain(QuotaSample::new(QuotaSampleParts {
        key: key.clone(),
        observation_id,
        observed_at_ms: row.get(offset + 1)?,
        fresh_until_ms: row.get(offset + 2)?,
        stale_after_ms: row.get(offset + 3)?,
        provider_epoch_id: row
            .get::<_, Option<String>>(offset + 4)?
            .map(QuotaProviderEpochId::new)
            .transpose()
            .map_err(stored_domain_error)?,
        used_ratio: row
            .get::<_, Option<i64>>(offset + 5)?
            .map(stored_ratio)
            .transpose()?,
        remaining_ratio: row
            .get::<_, Option<i64>>(offset + 6)?
            .map(stored_ratio)
            .transpose()?,
        units,
        advertised_resets_at_ms: row.get(offset + 11)?,
        quality: stored_quality(&row.get::<_, String>(offset + 12)?)?,
        source: stored_source(&row.get::<_, String>(offset + 13)?)?,
        confidence: stored_confidence(&row.get::<_, String>(offset + 14)?)?,
        reset_evidence: stored_reset_evidence(&row.get::<_, String>(offset + 15)?)?,
        reset_occurred_at_ms: row.get(offset + 16)?,
    }))
}

fn restore_current_epoch(
    row: &Row<'_>,
    offset: usize,
    key: &QuotaWindowKey,
    current_sample: &QuotaSample,
    first_sample: &QuotaSample,
) -> rusqlite::Result<QuotaCurrentEpoch> {
    let epoch_definition_revision = stored_u64(row.get(offset)?)?;
    let definition_revision = stored_u64(row.get(offset + 1)?)?;
    let epoch_id = QuotaEpochId::from_bytes(stored_bytes(row.get(offset + 2)?)?);
    let first_observation_id = QuotaObservationId::from_bytes(stored_bytes(row.get(offset + 3)?)?);
    let last_observation_id = QuotaObservationId::from_bytes(stored_bytes(row.get(offset + 4)?)?);
    let maximum_used_ratio = row
        .get::<_, Option<i64>>(offset + 7)?
        .map(stored_ratio)
        .transpose()?;
    let maximum_used_ratio_observation_id = row
        .get::<_, Option<Vec<u8>>>(offset + 8)?
        .map(stored_bytes)
        .transpose()?
        .map(QuotaObservationId::from_bytes);
    let maximum_used_units = stored_units(
        row.get(offset + 9)?,
        row.get(offset + 10)?,
        row.get(offset + 11)?,
        row.get(offset + 12)?,
    )?;
    let maximum_used_units_observation_id = row
        .get::<_, Option<Vec<u8>>>(offset + 13)?
        .map(stored_bytes)
        .transpose()?
        .map(QuotaObservationId::from_bytes);
    let provider_epoch_id = row
        .get::<_, Option<String>>(offset + 14)?
        .map(QuotaProviderEpochId::new)
        .transpose()
        .map_err(stored_domain_error)?;
    let state = stored_quota(QuotaEpochState::restore(QuotaEpochStateParts {
        key: key.clone(),
        epoch_definition_revision,
        definition_revision,
        epoch_id,
        first_observation_id,
        last_observation_id,
        first_observed_at_ms: row.get(offset + 5)?,
        last_observed_at_ms: row.get(offset + 6)?,
        maximum_used_ratio,
        maximum_used_ratio_observation_id,
        maximum_used_units,
        maximum_used_units_observation_id,
        provider_epoch_id,
        advertised_resets_at_ms: row.get(offset + 15)?,
        last_transition_sequence: stored_u64(row.get(offset + 16)?)?,
    }))?;
    if first_sample.observation_id() != state.first_observation_id()
        || first_sample.observed_at_ms() != state.first_observed_at_ms()
        || current_sample.observation_id() != state.last_observation_id()
        || current_sample.observed_at_ms() != state.last_observed_at_ms()
        || current_sample.provider_epoch_id() != state.provider_epoch_id()
        || current_sample.advertised_resets_at_ms() != state.advertised_resets_at_ms()
        || epoch_id
            != quota_epoch_id(
                key,
                epoch_definition_revision,
                first_sample.observation_id(),
            )
    {
        return Err(invalid_stored_sql());
    }
    Ok(QuotaCurrentEpoch {
        state,
        first_sample: first_sample.clone(),
    })
}

fn validate_current_projection(
    row: &Row<'_>,
    offset: usize,
    sample: &QuotaSample,
    epoch: &QuotaCurrentEpoch,
) -> rusqlite::Result<()> {
    if row.get::<_, i64>(offset)? != sample.observed_at_ms()
        || row.get::<_, i64>(offset + 1)? != sample.fresh_until_ms()
        || row.get::<_, i64>(offset + 2)? != sample.stale_after_ms()
        || row.get::<_, String>(offset + 3)? != quality_sql(sample.quality())
        || row.get::<_, String>(offset + 4)? != source_sql(sample.source())
        || row.get::<_, String>(offset + 5)? != confidence_sql(sample.confidence())
        || stored_u64(row.get(offset + 6)?)? != epoch.last_transition_sequence()
    {
        return Err(invalid_stored_sql());
    }
    Ok(())
}

fn restore_transition_record(
    row: &Row<'_>,
    key: &QuotaWindowKey,
) -> rusqlite::Result<QuotaTransitionRecord> {
    let id = QuotaTransitionId::from_bytes(stored_bytes(row.get(0)?)?);
    let definition_revision = stored_u64(row.get(1)?)?;
    let sequence = stored_u64(row.get(2)?)?;
    let kind = stored_transition_kind(&row.get::<_, String>(3)?)?;
    let previous_epoch_id = QuotaEpochId::from_bytes(stored_bytes(row.get(4)?)?);
    let current_epoch_id = QuotaEpochId::from_bytes(stored_bytes(row.get(5)?)?);
    let pre_observation_id = QuotaObservationId::from_bytes(stored_bytes(row.get(6)?)?);
    let post_observation_id = QuotaObservationId::from_bytes(stored_bytes(row.get(7)?)?);
    let maximum_used_ratio_before = row
        .get::<_, Option<i64>>(8)?
        .map(stored_ratio)
        .transpose()?;
    let maximum_used_ratio_observation_id_before = row
        .get::<_, Option<Vec<u8>>>(9)?
        .map(stored_bytes)
        .transpose()?
        .map(QuotaObservationId::from_bytes);
    let maximum_used_units_before =
        stored_units(row.get(10)?, row.get(11)?, row.get(12)?, row.get(13)?)?;
    let maximum_used_units_observation_id_before = row
        .get::<_, Option<Vec<u8>>>(14)?
        .map(stored_bytes)
        .transpose()?
        .map(QuotaObservationId::from_bytes);
    let allowance_change = stored_allowance_change(
        row.get(17)?,
        row.get(18)?,
        row.get(19)?,
        row.get(20)?,
        row.get(21)?,
        row.get(22)?,
        row.get(23)?,
        row.get(24)?,
        row.get(25)?,
    )?;
    let detection_time = stored_detection_time(
        &row.get::<_, String>(28)?,
        row.get(29)?,
        row.get(30)?,
        row.get(31)?,
    )?;
    let transition = stored_quota(QuotaTransition::restore(QuotaTransitionParts {
        id,
        definition_revision,
        sequence,
        key: key.clone(),
        kind,
        previous_epoch_id,
        current_epoch_id,
        pre_observation_id,
        post_observation_id,
        maximum_used_ratio_before,
        maximum_used_ratio_observation_id_before,
        maximum_used_units_before,
        maximum_used_units_observation_id_before,
        old_resets_at_ms: row.get(15)?,
        new_resets_at_ms: row.get(16)?,
        allowance_change,
        source: stored_source(&row.get::<_, String>(26)?)?,
        confidence: stored_confidence(&row.get::<_, String>(27)?)?,
        detection_time,
    }))?;
    let pre_sample = restore_sample(row, 32, key)?;
    let post_sample = restore_sample(row, 49, key)?;
    if pre_sample.observation_id() != transition.pre_observation_id()
        || post_sample.observation_id() != transition.post_observation_id()
        || !valid_transition_projection(&transition, &pre_sample, &post_sample)
    {
        return Err(invalid_stored_sql());
    }
    Ok(QuotaTransitionRecord {
        transition,
        pre_sample,
        post_sample,
    })
}

fn valid_transition_projection(
    transition: &QuotaTransition,
    pre_sample: &QuotaSample,
    post_sample: &QuotaSample,
) -> bool {
    let valid_detection_time = match transition.detection_time() {
        QuotaDetectionTime::Exact(at_ms) => {
            at_ms > pre_sample.observed_at_ms() && at_ms <= post_sample.observed_at_ms()
        }
        QuotaDetectionTime::Interval {
            after_ms,
            at_or_before_ms,
        } => {
            after_ms == pre_sample.observed_at_ms()
                && at_or_before_ms == post_sample.observed_at_ms()
        }
    };
    let valid_allowance = transition.allowance_change().is_none_or(|change| {
        pre_sample.units() == Some(change.old_units())
            && post_sample.units() == Some(change.new_units())
    });
    let valid_current_epoch = transition.kind() == QuotaTransitionKind::AllowanceChanged
        || transition.current_epoch_id()
            == quota_epoch_id(
                transition.key(),
                transition.definition_revision(),
                post_sample.observation_id(),
            );
    post_sample.observed_at_ms() > pre_sample.observed_at_ms()
        && transition.source() == post_sample.source()
        && transition.old_resets_at_ms() == pre_sample.advertised_resets_at_ms()
        && transition.new_resets_at_ms() == post_sample.advertised_resets_at_ms()
        && valid_detection_time
        && valid_allowance
        && valid_current_epoch
}

#[allow(clippy::too_many_arguments)]
fn stored_allowance_change(
    kind: Option<String>,
    old_unit_id: Option<String>,
    old_used: Option<i64>,
    old_remaining: Option<i64>,
    old_capacity: Option<i64>,
    new_unit_id: Option<String>,
    new_used: Option<i64>,
    new_remaining: Option<i64>,
    new_capacity: Option<i64>,
) -> rusqlite::Result<Option<QuotaAllowanceChange>> {
    let old_units = stored_units(old_unit_id, old_used, old_remaining, old_capacity)?;
    let new_units = stored_units(new_unit_id, new_used, new_remaining, new_capacity)?;
    match (kind, old_units, new_units) {
        (None, None, None) => Ok(None),
        (Some(kind), Some(old_units), Some(new_units)) => {
            let kind = stored_allowance_kind(&kind)?;
            stored_quota(QuotaAllowanceChange::restore(kind, old_units, new_units)).map(Some)
        }
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_detection_time(
    kind: &str,
    exact_at_ms: Option<i64>,
    after_ms: Option<i64>,
    at_or_before_ms: Option<i64>,
) -> rusqlite::Result<QuotaDetectionTime> {
    match (kind, exact_at_ms, after_ms, at_or_before_ms) {
        ("exact", Some(value), None, None) if value > 0 => Ok(QuotaDetectionTime::Exact(value)),
        ("interval", None, Some(after_ms), Some(at_or_before_ms))
            if after_ms > 0 && after_ms < at_or_before_ms =>
        {
            Ok(QuotaDetectionTime::Interval {
                after_ms,
                at_or_before_ms,
            })
        }
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_thresholds(
    maximum_post_reset_used: Option<i64>,
    minimum_post_reset_remaining: Option<i64>,
    minimum_used_ratio_drop: Option<i64>,
) -> rusqlite::Result<Option<QuotaResetThresholds>> {
    let maximum_post_reset_used = maximum_post_reset_used.map(stored_ratio).transpose()?;
    let minimum_post_reset_remaining =
        minimum_post_reset_remaining.map(stored_ratio).transpose()?;
    let minimum_used_ratio_drop = minimum_used_ratio_drop.map(stored_ratio).transpose()?;
    if maximum_post_reset_used.is_none()
        && minimum_post_reset_remaining.is_none()
        && minimum_used_ratio_drop.is_none()
    {
        return Ok(None);
    }
    stored_domain(QuotaResetThresholds::new(
        maximum_post_reset_used,
        minimum_post_reset_remaining,
        minimum_used_ratio_drop,
    ))
    .map(Some)
}

fn stored_units(
    unit_id: Option<String>,
    used: Option<i64>,
    remaining: Option<i64>,
    capacity: Option<i64>,
) -> rusqlite::Result<Option<QuotaUnits>> {
    match (unit_id, used, remaining, capacity) {
        (None, None, None, None) => Ok(None),
        (Some(unit_id), used, remaining, capacity) => stored_domain(QuotaUnits::new(
            stored_domain(QuotaUnitId::new(unit_id))?,
            stored_optional_u64(used)?,
            stored_optional_u64(remaining)?,
            stored_optional_u64(capacity)?,
        ))
        .map(Some),
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_ratio(value: i64) -> rusqlite::Result<QuotaRatio> {
    let value = u32::try_from(value).map_err(|_| invalid_stored_sql())?;
    stored_domain(QuotaRatio::new(value))
}

fn stored_optional_u64(value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value.map(stored_u64).transpose()
}

fn stored_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| invalid_stored_sql())
}

fn stored_bytes(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| invalid_stored_sql())
}

fn stored_presentation(value: &str) -> rusqlite::Result<QuotaPresentationDirection> {
    match value {
        "used" => Ok(QuotaPresentationDirection::Used),
        "remaining" => Ok(QuotaPresentationDirection::Remaining),
        "pace" => Ok(QuotaPresentationDirection::Pace),
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_semantics(value: &str) -> rusqlite::Result<QuotaWindowSemantics> {
    match value {
        "fixed" => Ok(QuotaWindowSemantics::Fixed),
        "rolling" => Ok(QuotaWindowSemantics::Rolling),
        "credit" => Ok(QuotaWindowSemantics::Credit),
        "unknown" => Ok(QuotaWindowSemantics::Unknown),
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_quality(value: &str) -> rusqlite::Result<QuotaSampleQuality> {
    match value {
        "authoritative" => Ok(QuotaSampleQuality::Authoritative),
        "partial" => Ok(QuotaSampleQuality::Partial),
        "conflict" => Ok(QuotaSampleQuality::Conflict),
        "unknown" => Ok(QuotaSampleQuality::Unknown),
        _ => Err(invalid_stored_sql()),
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

fn stored_source(value: &str) -> rusqlite::Result<QuotaEvidenceSource> {
    match value {
        "provider_local" => Ok(QuotaEvidenceSource::ProviderLocal),
        "provider_official" => Ok(QuotaEvidenceSource::ProviderOfficial),
        "local_reset_event" => Ok(QuotaEvidenceSource::LocalResetEvent),
        "manual" => Ok(QuotaEvidenceSource::Manual),
        "unknown" => Ok(QuotaEvidenceSource::Unknown),
        _ => Err(invalid_stored_sql()),
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

fn stored_confidence(value: &str) -> rusqlite::Result<QuotaConfidence> {
    match value {
        "high" => Ok(QuotaConfidence::High),
        "medium" => Ok(QuotaConfidence::Medium),
        "low" => Ok(QuotaConfidence::Low),
        "unknown" => Ok(QuotaConfidence::Unknown),
        _ => Err(invalid_stored_sql()),
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

fn stored_reset_evidence(value: &str) -> rusqlite::Result<QuotaResetEvidence> {
    match value {
        "none" => Ok(QuotaResetEvidence::None),
        "explicit_provider" => Ok(QuotaResetEvidence::ExplicitProvider),
        "explicit_local" => Ok(QuotaResetEvidence::ExplicitLocal),
        "manual_or_banked" => Ok(QuotaResetEvidence::ManualOrBanked),
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_transition_kind(value: &str) -> rusqlite::Result<QuotaTransitionKind> {
    match value {
        "scheduled_reset" => Ok(QuotaTransitionKind::ScheduledReset),
        "early_reset" => Ok(QuotaTransitionKind::EarlyReset),
        "manual_or_banked_reset" => Ok(QuotaTransitionKind::ManualOrBankedReset),
        "unknown_reset" => Ok(QuotaTransitionKind::UnknownReset),
        "allowance_changed" => Ok(QuotaTransitionKind::AllowanceChanged),
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_allowance_kind(value: &str) -> rusqlite::Result<QuotaAllowanceChangeKind> {
    match value {
        "increased" => Ok(QuotaAllowanceChangeKind::Increased),
        "decreased" => Ok(QuotaAllowanceChangeKind::Decreased),
        "unit_changed" => Ok(QuotaAllowanceChangeKind::UnitChanged),
        _ => Err(invalid_stored_sql()),
    }
}

fn stored_domain<T, E>(result: Result<T, E>) -> rusqlite::Result<T> {
    result.map_err(|_| invalid_stored_sql())
}

fn stored_quota<T, E>(result: Result<T, E>) -> rusqlite::Result<T> {
    result.map_err(|_| invalid_stored_sql())
}

fn stored_domain_error<E>(_error: E) -> rusqlite::Error {
    invalid_stored_sql()
}

fn invalid_stored_sql() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn map_quota_row<T>(result: rusqlite::Result<T>) -> Result<T, StoreError> {
    match result {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::InvalidQuery) => {
            Err(StoreError::new(StoreErrorCode::InvalidStoredValue))
        }
        Err(error) => map_sql(Err(error)),
    }
}

fn window_key_order(left: &QuotaWindowKey, right: &QuotaWindowKey) -> std::cmp::Ordering {
    let left_scope = quota_scope_id(left.scope());
    let right_scope = quota_scope_id(right.scope());
    left_scope
        .as_bytes()
        .cmp(right_scope.as_bytes())
        .then_with(|| left.window_id().as_str().cmp(right.window_id().as_str()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::{Connection, params_from_iter, types::Value};
    use tempfile::TempDir;

    use super::*;
    use crate::usage::UsageStore;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn empty_archive() -> TestResult<(TempDir, PathBuf)> {
        let directory = TempDir::new()?;
        let path = directory.path().join("quota-query.sqlite3");
        drop(UsageStore::open(&path)?);
        Ok((directory, path))
    }

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

    fn explain(
        connection: &Connection,
        sql: &str,
        parameters: &[Value],
    ) -> TestResult<Vec<String>> {
        let mut statement = connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let rows = statement.query_map(params_from_iter(parameters.iter()), |row| row.get(3))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    #[test]
    fn quota_sql_is_quota_only_and_uses_exact_indexes_without_offset() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let store = UsageReadStore::open(&path)?;
        for sql in [
            OVERVIEW_WINDOW_KEYS_SQL.to_owned(),
            CURRENT_WINDOW_SQL.to_owned(),
            transition_page_sql(false),
            transition_page_sql(true),
        ] {
            let normalized = sql.to_ascii_lowercase();
            assert!(normalized.contains("quota_"));
            assert!(!normalized.contains("usage_"));
            assert!(!normalized.contains("price_"));
            assert!(!normalized.contains(" offset "));
        }

        let key = key()?;
        let scope_id = quota_scope_id(key.scope());
        let overview = explain(
            &store.connection,
            OVERVIEW_WINDOW_KEYS_SQL,
            &[Value::Integer(33)],
        )?;
        assert!(overview.iter().any(|detail| {
            detail.contains("SCAN current USING INDEX quota_window_current_scope")
        }));
        assert!(overview.iter().any(|detail| {
            detail.contains("SEARCH definition USING INDEX")
                && detail.contains("quota_window_definition")
        }));
        assert!(!overview.iter().any(|detail| detail.contains("TEMP B-TREE")));
        let current = explain(
            &store.connection,
            CURRENT_WINDOW_SQL,
            &[
                Value::Blob(scope_id.as_bytes().to_vec()),
                Value::Text(key.window_id().as_str().to_owned()),
            ],
        )?;
        assert!(current.iter().any(|detail| {
            detail.contains("quota_window_current")
                && (detail.contains("PRIMARY KEY") || detail.contains("sqlite_autoindex"))
        }));

        let first = explain(
            &store.connection,
            &transition_page_sql(false),
            &[
                Value::Blob(scope_id.as_bytes().to_vec()),
                Value::Text(key.window_id().as_str().to_owned()),
                Value::Integer(257),
            ],
        )?;
        assert!(first.iter().any(|detail| {
            detail.contains("quota_transition_window_sequence")
                || detail.contains("sqlite_autoindex_quota_transition")
        }));
        assert!(
            !first
                .iter()
                .any(|detail| detail.contains("SCAN transition"))
        );
        assert!(!first.iter().any(|detail| detail.contains("TEMP B-TREE")));

        let cursor = explain(
            &store.connection,
            &transition_page_sql(true),
            &[
                Value::Blob(scope_id.as_bytes().to_vec()),
                Value::Text(key.window_id().as_str().to_owned()),
                Value::Integer(10),
                Value::Blob([9_u8; 32].to_vec()),
                Value::Integer(257),
            ],
        )?;
        assert!(cursor.iter().any(|detail| {
            detail.contains("quota_transition_window_sequence")
                || detail.contains("sqlite_autoindex_quota_transition")
        }));
        assert!(
            !cursor
                .iter()
                .any(|detail| detail.contains("SCAN transition"))
        );
        assert!(!cursor.iter().any(|detail| detail.contains("TEMP B-TREE")));
        Ok(())
    }

    #[test]
    fn quota_progress_cancellation_is_cleared_for_the_next_query() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let query = QuotaCurrentQuery::new(Box::default(), Duration::from_secs(2))?;
        let interrupted = match store.capture_quota_windows_with_options(query, 1, true, || Ok(()))
        {
            Ok(_) => return Err("cancelled quota query unexpectedly succeeded".into()),
            Err(error) => error,
        };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_quota_windows(QuotaCurrentQuery::new(
            Box::default(),
            Duration::from_secs(2),
        )?)?;
        assert_eq!(next.quota_revision().get(), 0);
        Ok(())
    }

    #[test]
    fn quota_overview_cancellation_and_total_deadline_clear_progress() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let interrupted = match store.capture_quota_overview_with_options(
            QuotaOverviewQuery::new(Duration::from_secs(2))?,
            1,
            true,
            || Ok(()),
        ) {
            Ok(_) => return Err("cancelled quota overview unexpectedly succeeded".into()),
            Err(error) => error,
        };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next =
            store.capture_quota_overview(QuotaOverviewQuery::new(Duration::from_secs(2))?)?;
        assert_eq!(next.quota_revision().get(), 0);

        let late = match store.capture_quota_overview_with_options(
            QuotaOverviewQuery::new(Duration::from_millis(1))?,
            i32::MAX,
            false,
            || {
                std::thread::sleep(Duration::from_millis(5));
                Ok(())
            },
        ) {
            Ok(_) => return Err("late quota overview unexpectedly succeeded".into()),
            Err(error) => error,
        };
        assert_eq!(late.code(), StoreErrorCode::DeadlineExceeded);
        let after =
            store.capture_quota_overview(QuotaOverviewQuery::new(Duration::from_secs(2))?)?;
        assert_eq!(after.quota_revision().get(), 0);
        Ok(())
    }

    #[test]
    fn quota_total_deadline_rejects_a_completed_late_capture_and_clears_progress() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let query = QuotaCurrentQuery::new(Box::default(), Duration::from_millis(1))?;
        let interrupted =
            match store.capture_quota_windows_with_options(query, i32::MAX, false, || {
                std::thread::sleep(Duration::from_millis(5));
                Ok(())
            }) {
                Ok(_) => return Err("late quota capture unexpectedly succeeded".into()),
                Err(error) => error,
            };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_quota_windows(QuotaCurrentQuery::new(
            Box::default(),
            Duration::from_secs(2),
        )?)?;
        assert_eq!(next.quota_revision().get(), 0);
        Ok(())
    }

    #[test]
    fn quota_read_transaction_keeps_revision_exact_during_concurrent_change() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let writer_path = path.clone();
        let capture = store.capture_quota_windows_with_options(
            QuotaCurrentQuery::new(Box::default(), Duration::from_secs(2))?,
            PROGRESS_OP_INTERVAL,
            false,
            move || {
                let writer = Connection::open(&writer_path)?;
                writer.execute(
                    "UPDATE quota_state
                     SET revision = 1, last_published_at_ms = 1
                     WHERE singleton_id = 1",
                    [],
                )?;
                Ok(())
            },
        )?;
        assert_eq!(capture.quota_revision().get(), 0);
        let next = store.capture_quota_windows(QuotaCurrentQuery::new(
            Box::default(),
            Duration::from_secs(2),
        )?)?;
        assert_eq!(next.quota_revision().get(), 1);
        Ok(())
    }

    #[test]
    fn quota_overview_transaction_keeps_revision_exact_during_concurrent_change() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let writer_path = path.clone();
        let capture = store.capture_quota_overview_with_options(
            QuotaOverviewQuery::new(Duration::from_secs(2))?,
            PROGRESS_OP_INTERVAL,
            false,
            move || {
                let writer = Connection::open(&writer_path)?;
                writer.execute(
                    "UPDATE quota_state
                     SET revision = 1, last_published_at_ms = 1
                     WHERE singleton_id = 1",
                    [],
                )?;
                Ok(())
            },
        )?;
        assert_eq!(capture.quota_revision().get(), 0);
        let next =
            store.capture_quota_overview(QuotaOverviewQuery::new(Duration::from_secs(2))?)?;
        assert_eq!(next.quota_revision().get(), 1);
        Ok(())
    }
}
