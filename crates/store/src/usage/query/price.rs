use std::{sync::OnceLock, time::Duration, time::Instant};

use rusqlite::{Connection, TransactionBehavior, params_from_iter, types::Value};

use super::{
    MAX_QUERY_DURATION, MAX_USAGE_OVERVIEW_SEGMENTS, MAX_USAGE_QUERY_SCOPES, PROGRESS_OP_INTERVAL,
    UsageAggregateRange, UsageQueryDatasetIdentity, UsageQueryPublication, UsageReadStore,
    UsageSessionKey, load_query_publication, load_raw_publication, load_ready_aggregate_generation,
    map_sql, valid_model,
};
use crate::usage::types::ScanScope;
use crate::{StoreError, StoreErrorCode};

pub const MAX_USAGE_PRICE_BASIS_KEYS: usize = 512;
pub const MAX_USAGE_PRICE_BASIS_TARGETS: usize = 401;

const PRICE_RANGE_SQL: &str = "(
       (?3 >= 1 AND bucket_width = ?4
        AND bucket_start_seconds >= ?5 AND bucket_start_seconds < ?6)
       OR (?3 >= 2 AND bucket_width = ?7
        AND bucket_start_seconds >= ?8 AND bucket_start_seconds < ?9)
       OR (?3 >= 3 AND bucket_width = ?10
        AND bucket_start_seconds >= ?11 AND bucket_start_seconds < ?12)
     )";

const PRICE_SCOPE_SQL: &str = "(?13 = 0 OR (provider_id, profile_id) IN (VALUES
       (?14, ?15), (?16, ?17), (?18, ?19), (?20, ?21),
       (?22, ?23), (?24, ?25), (?26, ?27), (?28, ?29),
       (?30, ?31), (?32, ?33), (?34, ?35), (?36, ?37),
       (?38, ?39), (?40, ?41), (?42, ?43), (?44, ?45),
       (?46, ?47), (?48, ?49), (?50, ?51), (?52, ?53),
       (?54, ?55), (?56, ?57), (?58, ?59), (?60, ?61),
       (?62, ?63), (?64, ?65), (?66, ?67), (?68, ?69),
       (?70, ?71), (?72, ?73), (?74, ?75), (?76, ?77)
     ))";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UsagePriceTier {
    StandardReported,
    StandardAssumed,
    Priority,
    Unknown,
}

impl UsagePriceTier {
    fn from_stored(value: &str) -> Result<Self, StoreError> {
        match value {
            "standard_reported" => Ok(Self::StandardReported),
            "standard_assumed" => Ok(Self::StandardAssumed),
            "priority" => Ok(Self::Priority),
            "unknown" => Ok(Self::Unknown),
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UsagePriceLongContext {
    Yes,
    No,
    Unavailable,
}

impl UsagePriceLongContext {
    fn from_stored(value: &str) -> Result<Self, StoreError> {
        match value {
            "yes" => Ok(Self::Yes),
            "no" => Ok(Self::No),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UsageReportedCostState {
    Present,
    Missing,
}

impl UsageReportedCostState {
    fn from_stored(value: &str) -> Result<Self, StoreError> {
        match value {
            "present" => Ok(Self::Present),
            "missing" => Ok(Self::Missing),
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UsagePriceBasisKey {
    model: Box<str>,
    tier: UsagePriceTier,
    long_context: UsagePriceLongContext,
    reported_cost_state: UsageReportedCostState,
}

impl UsagePriceBasisKey {
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub const fn tier(&self) -> UsagePriceTier {
        self.tier
    }

    #[must_use]
    pub const fn long_context(&self) -> UsagePriceLongContext {
        self.long_context
    }

    #[must_use]
    pub const fn reported_cost_state(&self) -> UsageReportedCostState {
        self.reported_cost_state
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsagePriceBasisMetrics {
    event_count: u64,
    calculable_event_count: u64,
    uncached_input_tokens: u64,
    cached_input_tokens: u64,
    billable_output_tokens: u64,
    reported_cost_count: u64,
    reported_cost_usd_micros: u64,
}

impl UsagePriceBasisMetrics {
    fn checked_add(&mut self, other: Self) -> Result<(), StoreError> {
        for (left, right) in [
            (&mut self.event_count, other.event_count),
            (
                &mut self.calculable_event_count,
                other.calculable_event_count,
            ),
            (&mut self.uncached_input_tokens, other.uncached_input_tokens),
            (&mut self.cached_input_tokens, other.cached_input_tokens),
            (
                &mut self.billable_output_tokens,
                other.billable_output_tokens,
            ),
            (&mut self.reported_cost_count, other.reported_cost_count),
            (
                &mut self.reported_cost_usd_micros,
                other.reported_cost_usd_micros,
            ),
        ] {
            *left = left
                .checked_add(right)
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        }
        Ok(())
    }

    fn checked_sub(self, included: Self) -> Result<Self, StoreError> {
        let subtract = |total: u64, value: u64| {
            total
                .checked_sub(value)
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))
        };
        Ok(Self {
            event_count: subtract(self.event_count, included.event_count)?,
            calculable_event_count: subtract(
                self.calculable_event_count,
                included.calculable_event_count,
            )?,
            uncached_input_tokens: subtract(
                self.uncached_input_tokens,
                included.uncached_input_tokens,
            )?,
            cached_input_tokens: subtract(self.cached_input_tokens, included.cached_input_tokens)?,
            billable_output_tokens: subtract(
                self.billable_output_tokens,
                included.billable_output_tokens,
            )?,
            reported_cost_count: subtract(self.reported_cost_count, included.reported_cost_count)?,
            reported_cost_usd_micros: subtract(
                self.reported_cost_usd_micros,
                included.reported_cost_usd_micros,
            )?,
        })
    }

    #[must_use]
    pub const fn event_count(self) -> u64 {
        self.event_count
    }

    #[must_use]
    pub const fn calculable_event_count(self) -> u64 {
        self.calculable_event_count
    }

    #[must_use]
    pub const fn uncached_input_tokens(self) -> u64 {
        self.uncached_input_tokens
    }

    #[must_use]
    pub const fn cached_input_tokens(self) -> u64 {
        self.cached_input_tokens
    }

    #[must_use]
    pub const fn billable_output_tokens(self) -> u64 {
        self.billable_output_tokens
    }

    #[must_use]
    pub const fn reported_cost_count(self) -> u64 {
        self.reported_cost_count
    }

    #[must_use]
    pub const fn reported_cost_usd_micros(self) -> u64 {
        self.reported_cost_usd_micros
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsagePriceBasisRow {
    key: UsagePriceBasisKey,
    metrics: UsagePriceBasisMetrics,
}

impl UsagePriceBasisRow {
    #[must_use]
    pub const fn key(&self) -> &UsagePriceBasisKey {
        &self.key
    }

    #[must_use]
    pub const fn metrics(&self) -> UsagePriceBasisMetrics {
        self.metrics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsagePriceBasisCapture {
    publication: UsageQueryPublication,
    rows: Box<[UsagePriceBasisRow]>,
    included: UsagePriceBasisMetrics,
    omitted: UsagePriceBasisMetrics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsagePriceBasisTargetCapture {
    rows: Box<[UsagePriceBasisRow]>,
    included: UsagePriceBasisMetrics,
    omitted: UsagePriceBasisMetrics,
}

impl UsagePriceBasisTargetCapture {
    #[must_use]
    pub const fn rows(&self) -> &[UsagePriceBasisRow] {
        &self.rows
    }

    #[must_use]
    pub const fn included(&self) -> UsagePriceBasisMetrics {
        self.included
    }

    #[must_use]
    pub const fn omitted(&self) -> UsagePriceBasisMetrics {
        self.omitted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsagePriceBasisBatchCapture {
    publication: UsageQueryPublication,
    targets: Box<[UsagePriceBasisTargetCapture]>,
}

impl UsagePriceBasisBatchCapture {
    #[must_use]
    pub const fn publication(&self) -> &UsageQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn targets(&self) -> &[UsagePriceBasisTargetCapture] {
        &self.targets
    }
}

impl UsagePriceBasisCapture {
    #[must_use]
    pub const fn publication(&self) -> &UsageQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn rows(&self) -> &[UsagePriceBasisRow] {
        &self.rows
    }

    #[must_use]
    pub const fn included(&self) -> UsagePriceBasisMetrics {
        self.included
    }

    #[must_use]
    pub const fn omitted(&self) -> UsagePriceBasisMetrics {
        self.omitted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsagePriceBasisQuery {
    expected_dataset: Option<UsageQueryDatasetIdentity>,
    range: UsageAggregateRange,
    scopes: Box<[ScanScope]>,
    deadline: Duration,
}

impl UsagePriceBasisQuery {
    pub fn new(
        expected_dataset: Option<UsageQueryDatasetIdentity>,
        range: UsageAggregateRange,
        scopes: Box<[ScanScope]>,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        validate_dataset_and_deadline(expected_dataset, deadline)?;
        let scopes = validate_scopes(scopes)?;
        Ok(Self {
            expected_dataset,
            range,
            scopes,
            deadline,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsagePriceBasisBatchQuery {
    expected_dataset: Option<UsageQueryDatasetIdentity>,
    ranges: Box<[UsageAggregateRange]>,
    scopes: Box<[ScanScope]>,
    deadline: Duration,
}

impl UsagePriceBasisBatchQuery {
    pub fn new(
        expected_dataset: Option<UsageQueryDatasetIdentity>,
        ranges: Box<[UsageAggregateRange]>,
        scopes: Box<[ScanScope]>,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        validate_dataset_and_deadline(expected_dataset, deadline)?;
        if ranges.is_empty() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if ranges.len() > MAX_USAGE_PRICE_BASIS_TARGETS {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_USAGE_PRICE_BASIS_TARGETS as u64,
            ));
        }
        let scopes = validate_scopes(scopes)?;
        Ok(Self {
            expected_dataset,
            ranges,
            scopes,
            deadline,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionPriceBasisQuery {
    expected_dataset: UsageQueryDatasetIdentity,
    session: UsageSessionKey,
    deadline: Duration,
}

impl UsageSessionPriceBasisQuery {
    pub fn new(
        expected_dataset: UsageQueryDatasetIdentity,
        session: UsageSessionKey,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        validate_dataset_and_deadline(Some(expected_dataset), deadline)?;
        if expected_dataset == UsageQueryDatasetIdentity::Empty
            || expected_dataset != session.dataset_identity_internal()
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            expected_dataset,
            session,
            deadline,
        })
    }
}

impl UsageReadStore {
    pub fn capture_usage_price_basis(
        &mut self,
        query: UsagePriceBasisQuery,
    ) -> Result<UsagePriceBasisCapture, StoreError> {
        self.capture_usage_price_basis_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_usage_price_basis_with_options<F>(
        &mut self,
        query: UsagePriceBasisQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<UsagePriceBasisCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_range(&mut self.connection, query, after_publication);
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    pub fn capture_usage_price_basis_batch(
        &mut self,
        query: UsagePriceBasisBatchQuery,
    ) -> Result<UsagePriceBasisBatchCapture, StoreError> {
        self.capture_usage_price_basis_batch_with_options(
            query,
            PROGRESS_OP_INTERVAL,
            false,
            || Ok(()),
        )
    }

    fn capture_usage_price_basis_batch_with_options<F>(
        &mut self,
        query: UsagePriceBasisBatchQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<UsagePriceBasisBatchCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_range_batch(&mut self.connection, query, after_publication);
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    pub fn capture_usage_session_price_basis(
        &mut self,
        query: UsageSessionPriceBasisQuery,
    ) -> Result<UsagePriceBasisCapture, StoreError> {
        self.capture_usage_session_price_basis_with_options(
            query,
            PROGRESS_OP_INTERVAL,
            false,
            || Ok(()),
        )
    }

    fn capture_usage_session_price_basis_with_options<F>(
        &mut self,
        query: UsageSessionPriceBasisQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<UsagePriceBasisCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_session(&mut self.connection, query, after_publication);
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

fn capture_range_batch<F>(
    connection: &mut Connection,
    query: UsagePriceBasisBatchQuery,
    after_publication: F,
) -> Result<UsagePriceBasisBatchCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let raw_publication = load_raw_publication(&transaction)?;
    after_publication()?;
    let dataset_identity = raw_publication.dataset_identity()?;
    if query
        .expected_dataset
        .is_some_and(|expected| expected != dataset_identity)
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let publication = load_query_publication(&transaction, &raw_publication, dataset_identity)?;
    let active_generation =
        load_ready_aggregate_generation(&transaction, raw_publication.dataset_generation)?;
    let mut targets = (0..query.ranges.len())
        .map(|_| RawPriceRows::default())
        .collect::<Vec<_>>();
    if let Some(kind) = dataset_kind(dataset_identity) {
        let (sql, parameters) =
            range_batch_sql_and_parameters(active_generation, kind, &query.ranges, &query.scopes)?;
        if let Some(sql) = sql {
            targets = load_batch_rows(&transaction, &sql, &parameters, query.ranges.len())?;
        }
    }
    map_sql(transaction.commit())?;
    let targets = targets
        .into_iter()
        .map(RawPriceRows::target_capture)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(UsagePriceBasisBatchCapture {
        publication,
        targets: targets.into_boxed_slice(),
    })
}

fn capture_range<F>(
    connection: &mut Connection,
    query: UsagePriceBasisQuery,
    after_publication: F,
) -> Result<UsagePriceBasisCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let raw_publication = load_raw_publication(&transaction)?;
    after_publication()?;
    let dataset_identity = raw_publication.dataset_identity()?;
    if query
        .expected_dataset
        .is_some_and(|expected| expected != dataset_identity)
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let publication = load_query_publication(&transaction, &raw_publication, dataset_identity)?;
    let active_generation =
        load_ready_aggregate_generation(&transaction, raw_publication.dataset_generation)?;
    let rows = match dataset_kind(dataset_identity) {
        Some(kind) if !query.range.is_empty() => load_rows(
            &transaction,
            range_price_sql(),
            &range_parameters(active_generation, kind, &query.range, &query.scopes)?,
        )?,
        _ => RawPriceRows::default(),
    };
    map_sql(transaction.commit())?;
    rows.capture(publication)
}

fn capture_session<F>(
    connection: &mut Connection,
    query: UsageSessionPriceBasisQuery,
    after_publication: F,
) -> Result<UsagePriceBasisCapture, StoreError>
where
    F: FnOnce() -> Result<(), StoreError>,
{
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let raw_publication = load_raw_publication(&transaction)?;
    after_publication()?;
    let dataset_identity = raw_publication.dataset_identity()?;
    if query.expected_dataset != dataset_identity {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    let publication = load_query_publication(&transaction, &raw_publication, dataset_identity)?;
    let active_generation =
        load_ready_aggregate_generation(&transaction, raw_publication.dataset_generation)?;
    let kind = dataset_kind(dataset_identity)
        .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
    let rows = load_rows(
        &transaction,
        session_price_sql(),
        &[
            Value::Integer(active_generation),
            Value::Text(kind.to_owned()),
            Value::Text(query.session.provider_id_internal().to_owned()),
            Value::Text(query.session.profile_id_internal().to_owned()),
            Value::Text(query.session.session_id_internal().to_owned()),
        ],
    )?;
    map_sql(transaction.commit())?;
    rows.capture(publication)
}

fn validate_dataset_and_deadline(
    expected_dataset: Option<UsageQueryDatasetIdentity>,
    deadline: Duration,
) -> Result<(), StoreError> {
    if deadline.is_zero()
        || deadline > MAX_QUERY_DURATION
        || matches!(
            expected_dataset,
            Some(UsageQueryDatasetIdentity::ReplayRevision {
                revision_id,
                dataset_generation,
            }) if revision_id > i64::MAX as u64 || dataset_generation > i64::MAX as u64
        )
    {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    Ok(())
}

fn validate_scopes(scopes: Box<[ScanScope]>) -> Result<Box<[ScanScope]>, StoreError> {
    if scopes.len() > MAX_USAGE_QUERY_SCOPES {
        return Err(StoreError::with_limit(
            StoreErrorCode::CapacityExceeded,
            MAX_USAGE_QUERY_SCOPES as u64,
        ));
    }
    let mut scopes = scopes.into_vec();
    scopes.sort_unstable();
    if scopes.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    Ok(scopes.into_boxed_slice())
}

const fn dataset_kind(identity: UsageQueryDatasetIdentity) -> Option<&'static str> {
    match identity {
        UsageQueryDatasetIdentity::Empty => None,
        UsageQueryDatasetIdentity::LegacySnapshotV1 => Some("legacy"),
        UsageQueryDatasetIdentity::ReplayRevision { .. } => Some("current"),
    }
}

fn range_parameters(
    active_generation: i64,
    dataset_kind: &'static str,
    range: &UsageAggregateRange,
    scopes: &[ScanScope],
) -> Result<Vec<Value>, StoreError> {
    let mut values = Vec::with_capacity(77);
    values.push(Value::Integer(active_generation));
    values.push(Value::Text(dataset_kind.to_owned()));
    values.push(Value::Integer(
        i64::try_from(range.segments().len())
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?,
    ));
    for index in 0..MAX_USAGE_OVERVIEW_SEGMENTS {
        if let Some(segment) = range.segments().get(index) {
            values.push(Value::Text(segment.bucket_width.as_sql().to_owned()));
            values.push(Value::Integer(segment.start_seconds));
            values.push(Value::Integer(segment.end_seconds));
        } else {
            values.extend([Value::Null, Value::Null, Value::Null]);
        }
    }
    values
        .push(Value::Integer(i64::try_from(scopes.len()).map_err(
            |_| StoreError::new(StoreErrorCode::CapacityExceeded),
        )?));
    for index in 0..MAX_USAGE_QUERY_SCOPES {
        if let Some(scope) = scopes.get(index) {
            values.push(Value::Text(scope.provider_id().to_owned()));
            values.push(Value::Text(scope.profile_id().to_owned()));
        } else {
            values.extend([Value::Null, Value::Null]);
        }
    }
    Ok(values)
}

#[derive(Default)]
struct RawPriceRows {
    rows: Vec<UsagePriceBasisRow>,
    included: UsagePriceBasisMetrics,
    total: UsagePriceBasisMetrics,
}

impl RawPriceRows {
    fn capture(
        self,
        publication: UsageQueryPublication,
    ) -> Result<UsagePriceBasisCapture, StoreError> {
        Ok(UsagePriceBasisCapture {
            publication,
            rows: self.rows.into_boxed_slice(),
            included: self.included,
            omitted: self.total.checked_sub(self.included)?,
        })
    }

    fn target_capture(self) -> Result<UsagePriceBasisTargetCapture, StoreError> {
        Ok(UsagePriceBasisTargetCapture {
            rows: self.rows.into_boxed_slice(),
            included: self.included,
            omitted: self.total.checked_sub(self.included)?,
        })
    }
}

fn load_rows(
    connection: &Connection,
    sql: &str,
    parameters: &[Value],
) -> Result<RawPriceRows, StoreError> {
    let mut statement = map_sql(connection.prepare_cached(sql))?;
    let mapped = map_sql(
        statement.query_map(params_from_iter(parameters.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                raw_metrics(row, 4)?,
                raw_metrics(row, 11)?,
            ))
        }),
    )?;
    let mut result = RawPriceRows {
        rows: Vec::with_capacity(MAX_USAGE_PRICE_BASIS_KEYS),
        ..RawPriceRows::default()
    };
    let mut total = None;
    for mapped_row in mapped {
        let (model, tier, context, reported, metrics, row_total) = map_sql(mapped_row)?;
        let key = validate_key(model, tier, context, reported)?;
        let metrics = metrics.validate(key.reported_cost_state)?;
        let row_total = row_total.validate_aggregate()?;
        if total.is_some_and(|current| current != row_total) {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        total = Some(row_total);
        result.included.checked_add(metrics)?;
        result.rows.push(UsagePriceBasisRow { key, metrics });
    }
    result.total = total.unwrap_or_default();
    Ok(result)
}

fn load_batch_rows(
    connection: &Connection,
    sql: &str,
    parameters: &[Value],
    target_count: usize,
) -> Result<Vec<RawPriceRows>, StoreError> {
    let mut statement = map_sql(connection.prepare(sql))?;
    let mapped = map_sql(
        statement.query_map(params_from_iter(parameters.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                raw_metrics(row, 5)?,
                raw_metrics(row, 12)?,
            ))
        }),
    )?;
    let mut targets = (0..target_count)
        .map(|_| RawPriceRows::default())
        .collect::<Vec<_>>();
    let mut detail_count = 0_usize;
    for mapped_row in mapped {
        let (target, model, tier, context, reported, metrics, total) = map_sql(mapped_row)?;
        let target = usize::try_from(target)
            .ok()
            .filter(|target| *target < target_count)
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        detail_count = detail_count
            .checked_add(1)
            .filter(|count| *count <= MAX_USAGE_PRICE_BASIS_KEYS)
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        let key = validate_key(model, tier, context, reported)?;
        let metrics = metrics.validate(key.reported_cost_state)?;
        let total = total.validate_aggregate()?;
        let target_rows = &mut targets[target];
        if target_rows.total.event_count != 0 && target_rows.total != total {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        target_rows.total = total;
        target_rows.included.checked_add(metrics)?;
        target_rows.rows.push(UsagePriceBasisRow { key, metrics });
    }
    Ok(targets)
}

fn validate_key(
    model: String,
    tier: String,
    context: String,
    reported: String,
) -> Result<UsagePriceBasisKey, StoreError> {
    if !valid_model(&model) {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(UsagePriceBasisKey {
        model: model.into_boxed_str(),
        tier: UsagePriceTier::from_stored(&tier)?,
        long_context: UsagePriceLongContext::from_stored(&context)?,
        reported_cost_state: UsageReportedCostState::from_stored(&reported)?,
    })
}

struct RawMetrics([i64; 7]);

impl RawMetrics {
    fn validate(
        self,
        reported: UsageReportedCostState,
    ) -> Result<UsagePriceBasisMetrics, StoreError> {
        let metrics = self.validate_aggregate()?;
        if match reported {
            UsageReportedCostState::Present => metrics.reported_cost_count != metrics.event_count,
            UsageReportedCostState::Missing => {
                metrics.reported_cost_count != 0 || metrics.reported_cost_usd_micros != 0
            }
        } {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        Ok(metrics)
    }

    fn validate_aggregate(self) -> Result<UsagePriceBasisMetrics, StoreError> {
        let metrics = self.validate_total()?;
        if metrics.calculable_event_count > metrics.event_count
            || metrics.reported_cost_count > metrics.event_count
            || (metrics.calculable_event_count == 0
                && (metrics.uncached_input_tokens != 0
                    || metrics.cached_input_tokens != 0
                    || metrics.billable_output_tokens != 0))
        {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        Ok(metrics)
    }

    fn validate_total(self) -> Result<UsagePriceBasisMetrics, StoreError> {
        let mut values = [0_u64; 7];
        for (target, source) in values.iter_mut().zip(self.0) {
            *target = u64::try_from(source)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        }
        Ok(UsagePriceBasisMetrics {
            event_count: values[0],
            calculable_event_count: values[1],
            uncached_input_tokens: values[2],
            cached_input_tokens: values[3],
            billable_output_tokens: values[4],
            reported_cost_count: values[5],
            reported_cost_usd_micros: values[6],
        })
    }
}

fn raw_metrics(row: &rusqlite::Row<'_>, start: usize) -> rusqlite::Result<RawMetrics> {
    Ok(RawMetrics([
        row.get(start)?,
        row.get(start + 1)?,
        row.get(start + 2)?,
        row.get(start + 3)?,
        row.get(start + 4)?,
        row.get(start + 5)?,
        row.get(start + 6)?,
    ]))
}

fn grouped_sql(table: &str, predicate: &str) -> String {
    format!(
        "WITH grouped AS (
           SELECT model, service_tier, long_context, reported_state,
                  sum(event_count) AS event_count,
                  sum(calculable_event_count) AS calculable_event_count,
                  sum(uncached_input_sum) AS uncached_input_sum,
                  sum(cached_input_sum) AS cached_input_sum,
                  sum(billable_output_sum) AS billable_output_sum,
                  sum(reported_cost_count) AS reported_cost_count,
                  sum(reported_cost_sum) AS reported_cost_sum
           FROM {table}
           WHERE {predicate}
           GROUP BY model, service_tier, long_context, reported_state
         ), totals AS (
           SELECT grouped.*,
                  sum(event_count) OVER () AS total_event_count,
                  sum(calculable_event_count) OVER () AS total_calculable_event_count,
                  sum(uncached_input_sum) OVER () AS total_uncached_input_sum,
                  sum(cached_input_sum) OVER () AS total_cached_input_sum,
                  sum(billable_output_sum) OVER () AS total_billable_output_sum,
                  sum(reported_cost_count) OVER () AS total_reported_cost_count,
                  sum(reported_cost_sum) OVER () AS total_reported_cost_sum
           FROM grouped
         )
         SELECT model, service_tier, long_context, reported_state,
                event_count, calculable_event_count, uncached_input_sum,
                cached_input_sum, billable_output_sum, reported_cost_count,
                reported_cost_sum, total_event_count, total_calculable_event_count,
                total_uncached_input_sum, total_cached_input_sum,
                total_billable_output_sum, total_reported_cost_count,
                total_reported_cost_sum
         FROM totals
         ORDER BY event_count DESC, calculable_event_count DESC,
                  reported_cost_count DESC, uncached_input_sum DESC,
                  cached_input_sum DESC, billable_output_sum DESC,
                  reported_cost_sum DESC, model ASC, service_tier ASC,
                  long_context ASC, reported_state ASC
         LIMIT {MAX_USAGE_PRICE_BASIS_KEYS}"
    )
}

fn range_price_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        grouped_sql(
            "usage_price_time_rollup",
            &format!(
                "aggregate_generation = ?1 AND dataset_kind = ?2
                 AND {PRICE_RANGE_SQL} AND {PRICE_SCOPE_SQL}"
            ),
        )
    })
}

fn session_price_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        grouped_sql(
            "usage_price_session_rollup",
            "aggregate_generation = ?1 AND dataset_kind = ?2
             AND provider_id = ?3 AND profile_id = ?4 AND session_id = ?5",
        )
    })
}

fn range_batch_sql_and_parameters(
    active_generation: i64,
    dataset_kind: &'static str,
    ranges: &[UsageAggregateRange],
    scopes: &[ScanScope],
) -> Result<(Option<String>, Vec<Value>), StoreError> {
    let segment_count = ranges
        .iter()
        .try_fold(0_usize, |count, range| {
            count.checked_add(range.segments().len())
        })
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
    if segment_count == 0 {
        return Ok((None, Vec::new()));
    }
    let mut parameters = Vec::with_capacity(2 + segment_count * 4 + scopes.len() * 2);
    parameters.push(Value::Integer(active_generation));
    parameters.push(Value::Text(dataset_kind.to_owned()));
    let mut segment_values = String::new();
    for (target, range) in ranges.iter().enumerate() {
        for segment in range.segments() {
            if !segment_values.is_empty() {
                segment_values.push_str(", ");
            }
            let first = parameters.len() + 1;
            segment_values.push_str(&format!(
                "(?{first}, ?{}, ?{}, ?{})",
                first + 1,
                first + 2,
                first + 3
            ));
            parameters
                .push(Value::Integer(i64::try_from(target).map_err(|_| {
                    StoreError::new(StoreErrorCode::CapacityExceeded)
                })?));
            parameters.push(Value::Text(segment.bucket_width.as_sql().to_owned()));
            parameters.push(Value::Integer(segment.start_seconds));
            parameters.push(Value::Integer(segment.end_seconds));
        }
    }
    let scope_predicate = if scopes.is_empty() {
        String::new()
    } else {
        let mut scope_values = String::new();
        for scope in scopes {
            if !scope_values.is_empty() {
                scope_values.push_str(", ");
            }
            let first = parameters.len() + 1;
            scope_values.push_str(&format!("(?{first}, ?{})", first + 1));
            parameters.push(Value::Text(scope.provider_id().to_owned()));
            parameters.push(Value::Text(scope.profile_id().to_owned()));
        }
        format!("AND (price.provider_id, price.profile_id) IN (VALUES {scope_values})")
    };
    let sql = format!(
        "WITH segments(target_id, bucket_width, start_seconds, end_seconds) AS (
           VALUES {segment_values}
         ), grouped AS (
           SELECT segments.target_id, price.model, price.service_tier,
                  price.long_context, price.reported_state,
                  sum(price.event_count) AS event_count,
                  sum(price.calculable_event_count) AS calculable_event_count,
                  sum(price.uncached_input_sum) AS uncached_input_sum,
                  sum(price.cached_input_sum) AS cached_input_sum,
                  sum(price.billable_output_sum) AS billable_output_sum,
                  sum(price.reported_cost_count) AS reported_cost_count,
                  sum(price.reported_cost_sum) AS reported_cost_sum
           FROM segments
           JOIN usage_price_time_rollup AS price
             ON price.bucket_width = segments.bucket_width
            AND price.bucket_start_seconds >= segments.start_seconds
            AND price.bucket_start_seconds < segments.end_seconds
           WHERE price.aggregate_generation = ?1 AND price.dataset_kind = ?2
             {scope_predicate}
           GROUP BY segments.target_id, price.model, price.service_tier,
                    price.long_context, price.reported_state
         ), ranked AS (
           SELECT grouped.*,
                  row_number() OVER (
                    PARTITION BY target_id
                    ORDER BY event_count DESC, calculable_event_count DESC,
                             reported_cost_count DESC, uncached_input_sum DESC,
                             cached_input_sum DESC, billable_output_sum DESC,
                             reported_cost_sum DESC, model ASC, service_tier ASC,
                             long_context ASC, reported_state ASC
                  ) AS key_rank,
                  sum(event_count) OVER (PARTITION BY target_id) AS total_event_count,
                  sum(calculable_event_count) OVER (PARTITION BY target_id)
                    AS total_calculable_event_count,
                  sum(uncached_input_sum) OVER (PARTITION BY target_id)
                    AS total_uncached_input_sum,
                  sum(cached_input_sum) OVER (PARTITION BY target_id)
                    AS total_cached_input_sum,
                  sum(billable_output_sum) OVER (PARTITION BY target_id)
                    AS total_billable_output_sum,
                  sum(reported_cost_count) OVER (PARTITION BY target_id)
                    AS total_reported_cost_count,
                  sum(reported_cost_sum) OVER (PARTITION BY target_id)
                    AS total_reported_cost_sum
           FROM grouped
         ), selected AS (
           SELECT * FROM ranked
           ORDER BY key_rank ASC, target_id ASC
           LIMIT {MAX_USAGE_PRICE_BASIS_KEYS}
         )
         SELECT target_id, model, service_tier, long_context, reported_state,
                event_count, calculable_event_count, uncached_input_sum,
                cached_input_sum, billable_output_sum, reported_cost_count,
                reported_cost_sum, total_event_count, total_calculable_event_count,
                total_uncached_input_sum, total_cached_input_sum,
                total_billable_output_sum, total_reported_cost_count,
                total_reported_cost_sum
         FROM selected ORDER BY target_id ASC, key_rank ASC"
    );
    Ok((Some(sql), parameters))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::UsageStore;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn empty_archive() -> TestResult<(TempDir, std::path::PathBuf)> {
        let directory = TempDir::new()?;
        let path = directory.path().join("price-read.sqlite3");
        drop(UsageStore::open(&path)?);
        Ok((directory, path))
    }

    fn range() -> Result<UsageAggregateRange, StoreError> {
        UsageAggregateRange::new(
            vec![super::super::UsageAggregateSegment::new(
                super::super::UsageAggregateBucketWidth::Minute,
                0,
                60,
            )?]
            .into_boxed_slice(),
        )
    }

    fn query(deadline: Duration) -> Result<UsagePriceBasisQuery, StoreError> {
        UsagePriceBasisQuery::new(None, range()?, Box::default(), deadline)
    }

    #[test]
    fn price_queries_use_only_fixed_price_rollups_and_indexed_predicates() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let store = UsageReadStore::open(&path)?;
        let parameters = range_parameters(0, "current", &range()?, &[])?;
        let mut statement = store
            .connection
            .prepare(&format!("EXPLAIN QUERY PLAN {}", range_price_sql()))?;
        let details = statement
            .query_map(params_from_iter(parameters.iter()), |row| {
                row.get::<_, String>(3)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("usage_price_time_rollup"))
        );

        let (batch_sql, batch_parameters) =
            range_batch_sql_and_parameters(0, "current", &[range()?, range()?], &[])?;
        let batch_sql = batch_sql.ok_or("non-empty batch SQL")?;
        let mut batch_statement = store
            .connection
            .prepare(&format!("EXPLAIN QUERY PLAN {batch_sql}"))?;
        let batch_details = batch_statement
            .query_map(params_from_iter(batch_parameters.iter()), |row| {
                row.get::<_, String>(3)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(
            batch_details
                .iter()
                .any(|detail| detail.contains("usage_price_time_rollup"))
        );
        let normalized_batch = batch_sql.to_ascii_lowercase();
        assert!(!normalized_batch.contains("usage_event"));
        assert!(normalized_batch.contains("limit 512"));

        for (sql, expected, forbidden) in [
            (range_price_sql(), "usage_price_time_rollup", "usage_event"),
            (
                session_price_sql(),
                "usage_price_session_rollup",
                "usage_legacy_event",
            ),
        ] {
            let normalized = sql.to_ascii_lowercase();
            assert!(normalized.contains(expected));
            assert!(!normalized.contains(forbidden));
            assert!(!normalized.contains(" offset "));
            assert!(normalized.contains("limit 512"));
        }
        Ok(())
    }

    #[test]
    fn price_query_cancellation_is_cleared_for_the_next_query() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let interrupted = match store.capture_usage_price_basis_with_options(
            query(Duration::from_secs(2))?,
            1,
            true,
            || Ok(()),
        ) {
            Err(error) => error,
            Ok(_) => return Err("forced price cancellation unexpectedly completed".into()),
        };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_usage_price_basis(query(Duration::from_secs(2))?)?;
        assert!(next.rows().is_empty());

        let batch_query = || {
            UsagePriceBasisBatchQuery::new(
                None,
                vec![range()?].into_boxed_slice(),
                Box::default(),
                Duration::from_secs(2),
            )
        };
        let interrupted = match store.capture_usage_price_basis_batch_with_options(
            batch_query()?,
            1,
            true,
            || Ok(()),
        ) {
            Err(error) => error,
            Ok(_) => return Err("forced batch price cancellation unexpectedly completed".into()),
        };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_usage_price_basis_batch(batch_query()?)?;
        assert_eq!(next.targets().len(), 1);
        assert!(next.targets()[0].rows().is_empty());
        Ok(())
    }

    #[test]
    fn price_snapshot_keeps_ready_generation_during_concurrent_change() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let writer_path = path.clone();
        let capture = store.capture_usage_price_basis_with_options(
            query(Duration::from_secs(2))?,
            PROGRESS_OP_INTERVAL,
            false,
            move || {
                let writer = Connection::open(&writer_path)?;
                writer.execute(
                    "UPDATE usage_aggregate_state SET state = 'rebuild_required'
                     WHERE singleton_id = 1",
                    [],
                )?;
                Ok(())
            },
        )?;
        assert!(capture.rows().is_empty());
        let error = match store.capture_usage_price_basis(query(Duration::from_secs(2))?) {
            Err(error) => error,
            Ok(_) => return Err("changed aggregate state unexpectedly remained ready".into()),
        };
        assert_eq!(error.code(), StoreErrorCode::RebuildRequired);
        Ok(())
    }
}
