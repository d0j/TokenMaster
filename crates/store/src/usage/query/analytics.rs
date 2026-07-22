use std::{sync::OnceLock, time::Duration, time::Instant};

use rusqlite::{Connection, TransactionBehavior, params_from_iter, types::Value};

use super::{
    MAX_QUERY_DURATION, MAX_USAGE_OVERVIEW_SEGMENTS, MAX_USAGE_QUERY_SCOPES, PROGRESS_OP_INTERVAL,
    UsageAggregateMetrics, UsageAggregateSegment, UsageQueryDatasetIdentity, UsageQueryPublication,
    UsageReadStore, load_aggregate_metrics, load_query_publication, load_raw_publication,
    load_ready_aggregate_generation, map_sql, raw_metrics_at, valid_ascii_id, valid_model,
};
use crate::usage::types::ScanScope;
use crate::{StoreError, StoreErrorCode};

pub const MAX_USAGE_SERIES_POINTS: usize = 400;
pub const MAX_USAGE_BREAKDOWNS: usize = 4;
pub const MAX_USAGE_BREAKDOWN_ITEMS: usize = 256;
pub const MAX_USAGE_RHYTHM_OCCURRENCES: usize = 768;
pub const MAX_USAGE_RHYTHM_SEGMENTS: usize = 2_304;
pub const USAGE_RHYTHM_HOURS: usize = 24;
pub const USAGE_RHYTHM_WEEKDAYS: usize = 7;

const BREAKDOWN_METRICS_SQL: &str = "coalesce(sum(event_count), 0),
       coalesce(sum(input_known_count), 0), coalesce(sum(input_known_sum), 0),
       coalesce(sum(cached_known_count), 0), coalesce(sum(cached_known_sum), 0),
       coalesce(sum(output_known_count), 0), coalesce(sum(output_known_sum), 0),
       coalesce(sum(reasoning_known_count), 0), coalesce(sum(reasoning_known_sum), 0),
       coalesce(sum(total_known_count), 0), coalesce(sum(total_known_sum), 0),
       coalesce(sum(fallback_model_count), 0),
       coalesce(sum(long_context_yes_count), 0),
       coalesce(sum(long_context_no_count), 0),
       coalesce(sum(long_context_unavailable_count), 0),
       coalesce(sum(activity_read), 0), coalesce(sum(activity_edit_write), 0),
       coalesce(sum(activity_search), 0), coalesce(sum(activity_git), 0),
       coalesce(sum(activity_build_test), 0), coalesce(sum(activity_web), 0),
       coalesce(sum(activity_subagents), 0), coalesce(sum(activity_terminal), 0)";

const BREAKDOWN_RANGE_SQL: &str = "(
       (?3 >= 1 AND bucket_width = ?4
        AND bucket_start_seconds >= ?5 AND bucket_start_seconds < ?6)
       OR (?3 >= 2 AND bucket_width = ?7
        AND bucket_start_seconds >= ?8 AND bucket_start_seconds < ?9)
       OR (?3 >= 3 AND bucket_width = ?10
        AND bucket_start_seconds >= ?11 AND bucket_start_seconds < ?12)
     )";

const BREAKDOWN_SCOPE_SQL: &str = "(?13 = 0 OR (provider_id, profile_id) IN (VALUES
       (?14, ?15), (?16, ?17), (?18, ?19), (?20, ?21),
       (?22, ?23), (?24, ?25), (?26, ?27), (?28, ?29),
       (?30, ?31), (?32, ?33), (?34, ?35), (?36, ?37),
       (?38, ?39), (?40, ?41), (?42, ?43), (?44, ?45),
       (?46, ?47), (?48, ?49), (?50, ?51), (?52, ?53),
       (?54, ?55), (?56, ?57), (?58, ?59), (?60, ?61),
       (?62, ?63), (?64, ?65), (?66, ?67), (?68, ?69),
       (?70, ?71), (?72, ?73), (?74, ?75), (?76, ?77)
     ))";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageAggregateRange {
    start_seconds: i64,
    end_seconds: i64,
    segments: Box<[UsageAggregateSegment]>,
}

impl UsageAggregateRange {
    pub fn new(segments: Box<[UsageAggregateSegment]>) -> Result<Self, StoreError> {
        if segments.len() > MAX_USAGE_OVERVIEW_SEGMENTS {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_USAGE_OVERVIEW_SEGMENTS as u64,
            ));
        }
        let Some(first) = segments.first() else {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        };
        let Some(last) = segments.last() else {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        };
        if segments
            .windows(2)
            .any(|pair| pair[0].end_seconds != pair[1].start_seconds)
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            start_seconds: first.start_seconds,
            end_seconds: last.end_seconds,
            segments,
        })
    }

    pub fn empty(boundary_seconds: i64) -> Result<Self, StoreError> {
        if boundary_seconds.rem_euclid(60) != 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            start_seconds: boundary_seconds,
            end_seconds: boundary_seconds,
            segments: Box::default(),
        })
    }

    #[must_use]
    pub const fn start_seconds(&self) -> i64 {
        self.start_seconds
    }

    #[must_use]
    pub const fn end_seconds(&self) -> i64 {
        self.end_seconds
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub(super) const fn segments(&self) -> &[UsageAggregateSegment] {
        &self.segments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSeriesPoint {
    range: UsageAggregateRange,
}

impl UsageSeriesPoint {
    pub fn new(segments: Box<[UsageAggregateSegment]>) -> Result<Self, StoreError> {
        Ok(Self {
            range: UsageAggregateRange::new(segments)?,
        })
    }

    pub fn empty(boundary_seconds: i64) -> Result<Self, StoreError> {
        Ok(Self {
            range: UsageAggregateRange::empty(boundary_seconds)?,
        })
    }

    #[must_use]
    pub const fn start_seconds(&self) -> i64 {
        self.range.start_seconds
    }

    #[must_use]
    pub const fn end_seconds(&self) -> i64 {
        self.range.end_seconds
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UsageBreakdownKind {
    Model,
    Project,
    Provider,
    Profile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageAnalyticsQuery {
    expected_dataset: Option<UsageQueryDatasetIdentity>,
    overview: UsageAggregateRange,
    series_points: Box<[UsageSeriesPoint]>,
    breakdowns: Box<[UsageBreakdownKind]>,
    scopes: Box<[ScanScope]>,
    rhythm: Option<UsageRhythmQuery>,
    deadline: Duration,
}

impl UsageAnalyticsQuery {
    pub fn new(
        expected_dataset: Option<UsageQueryDatasetIdentity>,
        overview: UsageAggregateRange,
        series_points: Box<[UsageSeriesPoint]>,
        breakdowns: Box<[UsageBreakdownKind]>,
        scopes: Box<[ScanScope]>,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
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
        if series_points.len() > MAX_USAGE_SERIES_POINTS {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_USAGE_SERIES_POINTS as u64,
            ));
        }
        if !series_points.is_empty()
            && (series_points[0].start_seconds() != overview.start_seconds
                || series_points[series_points.len() - 1].end_seconds() != overview.end_seconds
                || series_points
                    .windows(2)
                    .any(|pair| pair[0].end_seconds() != pair[1].start_seconds()))
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if breakdowns.len() > MAX_USAGE_BREAKDOWNS {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_USAGE_BREAKDOWNS as u64,
            ));
        }
        let mut breakdowns = breakdowns.into_vec();
        breakdowns.sort_unstable();
        if breakdowns.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
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
        Ok(Self {
            expected_dataset,
            overview,
            series_points,
            breakdowns: breakdowns.into_boxed_slice(),
            scopes: scopes.into_boxed_slice(),
            rhythm: None,
            deadline,
        })
    }

    pub fn with_rhythm(mut self, rhythm: UsageRhythmQuery) -> Result<Self, StoreError> {
        self.rhythm = Some(rhythm);
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsageRhythmSegment {
    hour_index: u8,
    weekday_index: u8,
    segment: UsageAggregateSegment,
}

impl UsageRhythmSegment {
    pub fn new(
        hour_index: u8,
        weekday_index: u8,
        segment: UsageAggregateSegment,
    ) -> Result<Self, StoreError> {
        if usize::from(hour_index) >= USAGE_RHYTHM_HOURS
            || usize::from(weekday_index) >= USAGE_RHYTHM_WEEKDAYS
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            hour_index,
            weekday_index,
            segment,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRhythmQuery {
    segments: Box<[UsageRhythmSegment]>,
}

impl UsageRhythmQuery {
    pub fn new(segments: Box<[UsageRhythmSegment]>) -> Result<Self, StoreError> {
        if segments.is_empty() || segments.len() > MAX_USAGE_RHYTHM_SEGMENTS {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_USAGE_RHYTHM_SEGMENTS as u64,
            ));
        }
        if segments
            .windows(2)
            .any(|pair| pair[0].segment.end_seconds != pair[1].segment.start_seconds)
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self { segments })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRhythmCapture {
    hours: Box<[UsageAggregateMetrics]>,
    weekdays: Box<[UsageAggregateMetrics]>,
}

impl UsageRhythmCapture {
    #[must_use]
    pub const fn hours(&self) -> &[UsageAggregateMetrics] {
        &self.hours
    }

    #[must_use]
    pub const fn weekdays(&self) -> &[UsageAggregateMetrics] {
        &self.weekdays
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSeriesPointCapture {
    start_seconds: i64,
    end_seconds: i64,
    metrics: UsageAggregateMetrics,
}

impl UsageSeriesPointCapture {
    #[must_use]
    pub const fn start_seconds(&self) -> i64 {
        self.start_seconds
    }

    #[must_use]
    pub const fn end_seconds(&self) -> i64 {
        self.end_seconds
    }

    #[must_use]
    pub const fn metrics(&self) -> &UsageAggregateMetrics {
        &self.metrics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UsageBreakdownIdentity {
    Model(Box<str>),
    Project(Box<str>),
    UnassociatedProject,
    Provider(Box<str>),
    Profile {
        provider_id: Box<str>,
        profile_id: Box<str>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageBreakdownItem {
    identity: UsageBreakdownIdentity,
    metrics: UsageAggregateMetrics,
}

impl UsageBreakdownItem {
    #[must_use]
    pub const fn identity(&self) -> &UsageBreakdownIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn metrics(&self) -> &UsageAggregateMetrics {
        &self.metrics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageBreakdown {
    kind: UsageBreakdownKind,
    items: Box<[UsageBreakdownItem]>,
    truncated: bool,
}

impl UsageBreakdown {
    #[must_use]
    pub const fn kind(&self) -> UsageBreakdownKind {
        self.kind
    }

    #[must_use]
    pub const fn items(&self) -> &[UsageBreakdownItem] {
        &self.items
    }

    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageAnalyticsCapture {
    publication: UsageQueryPublication,
    overview: UsageAggregateMetrics,
    series: Box<[UsageSeriesPointCapture]>,
    breakdowns: Box<[UsageBreakdown]>,
    rhythm: Option<UsageRhythmCapture>,
}

impl UsageAnalyticsCapture {
    #[must_use]
    pub const fn publication(&self) -> &UsageQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn overview(&self) -> &UsageAggregateMetrics {
        &self.overview
    }

    #[must_use]
    pub const fn series(&self) -> &[UsageSeriesPointCapture] {
        &self.series
    }

    #[must_use]
    pub const fn breakdowns(&self) -> &[UsageBreakdown] {
        &self.breakdowns
    }

    #[must_use]
    pub const fn rhythm(&self) -> Option<&UsageRhythmCapture> {
        self.rhythm.as_ref()
    }
}

impl UsageReadStore {
    pub fn capture_usage_analytics(
        &mut self,
        query: UsageAnalyticsQuery,
    ) -> Result<UsageAnalyticsCapture, StoreError> {
        self.capture_usage_analytics_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    pub(super) fn capture_usage_analytics_with_options<F>(
        &mut self,
        query: UsageAnalyticsQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<UsageAnalyticsCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_usage_analytics(&mut self.connection, query, after_publication);
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

fn capture_usage_analytics<F>(
    connection: &mut Connection,
    query: UsageAnalyticsQuery,
    after_publication: F,
) -> Result<UsageAnalyticsCapture, StoreError>
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
    let dataset_kind = match dataset_identity {
        UsageQueryDatasetIdentity::Empty => None,
        UsageQueryDatasetIdentity::ReplayRevision { .. } => Some("current"),
        UsageQueryDatasetIdentity::LegacySnapshotV1 => Some("legacy"),
    };

    let overview = load_range_metrics(
        &transaction,
        active_generation,
        dataset_kind,
        &query.overview,
        &query.scopes,
    )?;
    let mut series = Vec::with_capacity(query.series_points.len());
    for point in &query.series_points {
        series.push(UsageSeriesPointCapture {
            start_seconds: point.start_seconds(),
            end_seconds: point.end_seconds(),
            metrics: load_range_metrics(
                &transaction,
                active_generation,
                dataset_kind,
                &point.range,
                &query.scopes,
            )?,
        });
    }
    let mut breakdowns = Vec::with_capacity(query.breakdowns.len());
    for kind in query.breakdowns.iter().copied() {
        breakdowns.push(load_breakdown(
            &transaction,
            active_generation,
            dataset_kind,
            kind,
            &query.overview,
            &query.scopes,
        )?);
    }
    let rhythm = query
        .rhythm
        .as_ref()
        .map(|rhythm| {
            load_rhythm(
                &transaction,
                active_generation,
                dataset_kind,
                rhythm,
                &query.scopes,
                &overview,
            )
        })
        .transpose()?;
    map_sql(transaction.commit())?;
    Ok(UsageAnalyticsCapture {
        publication,
        overview,
        series: series.into_boxed_slice(),
        breakdowns: breakdowns.into_boxed_slice(),
        rhythm,
    })
}

fn load_rhythm(
    connection: &Connection,
    active_generation: i64,
    dataset_kind: Option<&'static str>,
    query: &UsageRhythmQuery,
    scopes: &[ScanScope],
    overview: &UsageAggregateMetrics,
) -> Result<UsageRhythmCapture, StoreError> {
    let Some(dataset_kind) = dataset_kind else {
        return Ok(UsageRhythmCapture {
            hours: vec![UsageAggregateMetrics::default(); USAGE_RHYTHM_HOURS].into_boxed_slice(),
            weekdays: vec![UsageAggregateMetrics::default(); USAGE_RHYTHM_WEEKDAYS]
                .into_boxed_slice(),
        });
    };
    let parameters = rhythm_parameters(active_generation, dataset_kind, query, scopes)?;
    let hours = load_rhythm_dimension(
        connection,
        query,
        &parameters,
        "hour_index",
        USAGE_RHYTHM_HOURS,
    )?;
    let weekdays = load_rhythm_dimension(
        connection,
        query,
        &parameters,
        "weekday_index",
        USAGE_RHYTHM_WEEKDAYS,
    )?;
    for buckets in [&hours, &weekdays] {
        let mut total = UsageAggregateMetrics::default();
        for bucket in buckets {
            total.checked_add(bucket)?;
        }
        if &total != overview {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
    }
    Ok(UsageRhythmCapture {
        hours: hours.into_boxed_slice(),
        weekdays: weekdays.into_boxed_slice(),
    })
}

fn load_rhythm_dimension(
    connection: &Connection,
    query: &UsageRhythmQuery,
    parameters: &[Value],
    dimension: &'static str,
    expected: usize,
) -> Result<Vec<UsageAggregateMetrics>, StoreError> {
    let sql = rhythm_sql(query.segments.len(), dimension);
    let mut statement = map_sql(connection.prepare(&sql))?;
    let rows = map_sql(
        statement.query_map(params_from_iter(parameters.iter()), |row| {
            Ok((row.get::<_, i64>(0)?, raw_metrics_at(row, 1)?))
        }),
    )?;
    let mut result = Vec::with_capacity(expected);
    for row in rows {
        let (index, raw) = map_sql(row)?;
        let expected_index = i64::try_from(result.len())
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        if index != expected_index || result.len() >= expected {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        result.push(raw.validate()?);
    }
    if result.len() != expected {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(result)
}

fn rhythm_parameters(
    active_generation: i64,
    dataset_kind: &'static str,
    query: &UsageRhythmQuery,
    scopes: &[ScanScope],
) -> Result<Vec<Value>, StoreError> {
    let mut parameters =
        Vec::with_capacity(3 + query.segments.len() * 5 + MAX_USAGE_QUERY_SCOPES * 2);
    parameters.push(Value::Integer(active_generation));
    parameters.push(Value::Text(dataset_kind.to_owned()));
    for tagged in &query.segments {
        parameters.push(Value::Integer(i64::from(tagged.hour_index)));
        parameters.push(Value::Integer(i64::from(tagged.weekday_index)));
        parameters.push(Value::Text(tagged.segment.bucket_width.as_sql().to_owned()));
        parameters.push(Value::Integer(tagged.segment.start_seconds));
        parameters.push(Value::Integer(tagged.segment.end_seconds));
    }
    parameters
        .push(Value::Integer(i64::try_from(scopes.len()).map_err(
            |_| StoreError::new(StoreErrorCode::CapacityExceeded),
        )?));
    for index in 0..MAX_USAGE_QUERY_SCOPES {
        if let Some(scope) = scopes.get(index) {
            parameters.push(Value::Text(scope.provider_id().to_owned()));
            parameters.push(Value::Text(scope.profile_id().to_owned()));
        } else {
            parameters.push(Value::Null);
            parameters.push(Value::Null);
        }
    }
    Ok(parameters)
}

fn rhythm_sql(segment_count: usize, dimension: &'static str) -> String {
    let mut next = 3usize;
    let values = (0..segment_count)
        .map(|_| {
            let row = format!(
                "(?{}, ?{}, ?{}, ?{}, ?{})",
                next,
                next + 1,
                next + 2,
                next + 3,
                next + 4
            );
            next += 5;
            row
        })
        .collect::<Vec<_>>()
        .join(", ");
    let scope_count = next;
    next += 1;
    let scope_values = (0..MAX_USAGE_QUERY_SCOPES)
        .map(|_| {
            let pair = format!("(?{}, ?{})", next, next + 1);
            next += 2;
            pair
        })
        .collect::<Vec<_>>()
        .join(", ");
    let expected = if dimension == "hour_index" {
        USAGE_RHYTHM_HOURS
    } else {
        USAGE_RHYTHM_WEEKDAYS
    };
    format!(
        "WITH RECURSIVE
         rhythm_segment(hour_index, weekday_index, bucket_width, start_seconds, end_seconds) AS (VALUES {values}),
         rhythm_key(value) AS (SELECT 0 UNION ALL SELECT value + 1 FROM rhythm_key WHERE value + 1 < {expected})
         SELECT rhythm_key.value, {BREAKDOWN_METRICS_SQL}
         FROM rhythm_key
         LEFT JOIN rhythm_segment ON rhythm_segment.{dimension} = rhythm_key.value
         LEFT JOIN usage_time_rollup AS item
           ON item.aggregate_generation = ?1 AND item.dataset_kind = ?2
          AND item.bucket_width = rhythm_segment.bucket_width
          AND item.bucket_start_seconds >= rhythm_segment.start_seconds
          AND item.bucket_start_seconds < rhythm_segment.end_seconds
          AND item.dimension_kind = 'all' AND item.dimension_value = ''
          AND (?{scope_count} = 0 OR (item.provider_id, item.profile_id) IN (VALUES {scope_values}))
         GROUP BY rhythm_key.value
         ORDER BY rhythm_key.value"
    )
}

fn load_range_metrics(
    connection: &Connection,
    active_generation: i64,
    dataset_kind: Option<&'static str>,
    range: &UsageAggregateRange,
    scopes: &[ScanScope],
) -> Result<UsageAggregateMetrics, StoreError> {
    let Some(dataset_kind) = dataset_kind else {
        return Ok(UsageAggregateMetrics::default());
    };
    if range.is_empty() {
        return Ok(UsageAggregateMetrics::default());
    }
    load_aggregate_metrics(
        connection,
        active_generation,
        dataset_kind,
        &range.segments,
        scopes,
    )
}

fn load_breakdown(
    connection: &Connection,
    active_generation: i64,
    dataset_kind: Option<&'static str>,
    kind: UsageBreakdownKind,
    range: &UsageAggregateRange,
    scopes: &[ScanScope],
) -> Result<UsageBreakdown, StoreError> {
    let Some(dataset_kind) = dataset_kind else {
        return Ok(empty_breakdown(kind));
    };
    if range.is_empty() {
        return Ok(empty_breakdown(kind));
    }
    let parameters = breakdown_parameters(active_generation, dataset_kind, range, scopes)?;
    let mut statement = map_sql(connection.prepare_cached(breakdown_sql(kind)))?;
    let rows = map_sql(
        statement.query_map(params_from_iter(parameters.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                raw_metrics_at(row, 2)?,
            ))
        }),
    )?;
    let mut items = Vec::with_capacity(MAX_USAGE_BREAKDOWN_ITEMS + 1);
    for row in rows {
        let (key1, key2, raw) = map_sql(row)?;
        items.push(UsageBreakdownItem {
            identity: validate_identity(kind, key1, key2)?,
            metrics: raw.validate()?,
        });
    }
    let truncated = items.len() > MAX_USAGE_BREAKDOWN_ITEMS;
    if truncated {
        items.truncate(MAX_USAGE_BREAKDOWN_ITEMS);
    }
    Ok(UsageBreakdown {
        kind,
        items: items.into_boxed_slice(),
        truncated,
    })
}

fn empty_breakdown(kind: UsageBreakdownKind) -> UsageBreakdown {
    UsageBreakdown {
        kind,
        items: Box::default(),
        truncated: false,
    }
}

pub(super) fn validate_identity(
    kind: UsageBreakdownKind,
    key1: String,
    key2: String,
) -> Result<UsageBreakdownIdentity, StoreError> {
    let invalid = || StoreError::new(StoreErrorCode::InvalidStoredValue);
    match kind {
        UsageBreakdownKind::Model if key2.is_empty() && valid_model(&key1) => {
            Ok(UsageBreakdownIdentity::Model(key1.into_boxed_str()))
        }
        UsageBreakdownKind::Project if key2.is_empty() && key1.is_empty() => {
            Ok(UsageBreakdownIdentity::UnassociatedProject)
        }
        UsageBreakdownKind::Project if key2.is_empty() && valid_project(&key1) => {
            Ok(UsageBreakdownIdentity::Project(key1.into_boxed_str()))
        }
        UsageBreakdownKind::Provider if key2.is_empty() && valid_ascii_id(&key1, 64) => {
            Ok(UsageBreakdownIdentity::Provider(key1.into_boxed_str()))
        }
        UsageBreakdownKind::Profile if valid_ascii_id(&key1, 64) && valid_ascii_id(&key2, 128) => {
            Ok(UsageBreakdownIdentity::Profile {
                provider_id: key1.into_boxed_str(),
                profile_id: key2.into_boxed_str(),
            })
        }
        _ => Err(invalid()),
    }
}

pub(super) fn usage_breakdown_item(
    kind: UsageBreakdownKind,
    key1: String,
    metrics: UsageAggregateMetrics,
) -> Result<UsageBreakdownItem, StoreError> {
    Ok(UsageBreakdownItem {
        identity: validate_identity(kind, key1, String::new())?,
        metrics,
    })
}

pub(super) fn usage_breakdown(
    kind: UsageBreakdownKind,
    items: Vec<UsageBreakdownItem>,
    truncated: bool,
) -> UsageBreakdown {
    UsageBreakdown {
        kind,
        items: items.into_boxed_slice(),
        truncated,
    }
}

fn valid_project(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
        && !value.bytes().any(|byte| matches!(byte, b'/' | b'\\'))
}

fn breakdown_parameters(
    active_generation: i64,
    dataset_kind: &'static str,
    range: &UsageAggregateRange,
    scopes: &[ScanScope],
) -> Result<Vec<Value>, StoreError> {
    let mut parameters = Vec::with_capacity(77);
    parameters.push(Value::Integer(active_generation));
    parameters.push(Value::Text(dataset_kind.to_owned()));
    parameters.push(Value::Integer(
        i64::try_from(range.segments.len())
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?,
    ));
    for index in 0..MAX_USAGE_OVERVIEW_SEGMENTS {
        if let Some(segment) = range.segments.get(index) {
            parameters.push(Value::Text(segment.bucket_width.as_sql().to_owned()));
            parameters.push(Value::Integer(segment.start_seconds));
            parameters.push(Value::Integer(segment.end_seconds));
        } else {
            parameters.extend([Value::Null, Value::Null, Value::Null]);
        }
    }
    parameters
        .push(Value::Integer(i64::try_from(scopes.len()).map_err(
            |_| StoreError::new(StoreErrorCode::CapacityExceeded),
        )?));
    for index in 0..MAX_USAGE_QUERY_SCOPES {
        if let Some(scope) = scopes.get(index) {
            parameters.push(Value::Text(scope.provider_id().to_owned()));
            parameters.push(Value::Text(scope.profile_id().to_owned()));
        } else {
            parameters.push(Value::Null);
            parameters.push(Value::Null);
        }
    }
    Ok(parameters)
}

fn breakdown_sql(kind: UsageBreakdownKind) -> &'static str {
    static MODEL: OnceLock<String> = OnceLock::new();
    static PROJECT: OnceLock<String> = OnceLock::new();
    static PROVIDER: OnceLock<String> = OnceLock::new();
    static PROFILE: OnceLock<String> = OnceLock::new();
    match kind {
        UsageBreakdownKind::Model => MODEL.get_or_init(|| {
            build_breakdown_sql(
                "dimension_value AS key1, '' AS key2",
                "dimension_kind = 'model'",
                "dimension_value",
            )
        }),
        UsageBreakdownKind::Project => PROJECT.get_or_init(|| {
            build_breakdown_sql(
                "dimension_value AS key1, '' AS key2",
                "dimension_kind = 'project'",
                "dimension_value",
            )
        }),
        UsageBreakdownKind::Provider => PROVIDER.get_or_init(|| {
            build_breakdown_sql(
                "provider_id AS key1, '' AS key2",
                "dimension_kind = 'all' AND dimension_value = ''",
                "provider_id",
            )
        }),
        UsageBreakdownKind::Profile => PROFILE.get_or_init(|| {
            build_breakdown_sql(
                "provider_id AS key1, profile_id AS key2",
                "dimension_kind = 'all' AND dimension_value = ''",
                "provider_id, profile_id",
            )
        }),
    }
}

fn build_breakdown_sql(key_select: &str, dimension: &str, group_by: &str) -> String {
    format!(
        "SELECT {key_select}, {BREAKDOWN_METRICS_SQL}
         FROM usage_time_rollup
         WHERE aggregate_generation = ?1 AND dataset_kind = ?2
           AND {dimension} AND {BREAKDOWN_RANGE_SQL} AND {BREAKDOWN_SCOPE_SQL}
         GROUP BY {group_by}
         ORDER BY coalesce(sum(total_known_sum), 0) DESC,
                  coalesce(sum(event_count), 0) DESC, key1 ASC, key2 ASC
         LIMIT 257"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    use rusqlite::Connection;
    use tempfile::TempDir;

    use crate::UsageStore;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn empty_archive() -> TestResult<(TempDir, std::path::PathBuf)> {
        let directory = TempDir::new()?;
        let path = directory.path().join("analytics.sqlite3");
        drop(UsageStore::open(&path)?);
        Ok((directory, path))
    }

    fn empty_query() -> Result<UsageAnalyticsQuery, StoreError> {
        UsageAnalyticsQuery::new(
            None,
            UsageAggregateRange::empty(0)?,
            Box::default(),
            Box::default(),
            Box::default(),
            Duration::from_secs(2),
        )
    }

    #[test]
    fn breakdown_queries_are_fixed_rollup_only_and_offset_free() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let store = UsageReadStore::open(&path)?;
        let range = UsageAggregateRange::new(
            vec![UsageAggregateSegment::new(
                super::super::UsageAggregateBucketWidth::Minute,
                0,
                60,
            )?]
            .into_boxed_slice(),
        )?;
        let parameters = breakdown_parameters(0, "current", &range, &[])?;
        for kind in [
            UsageBreakdownKind::Model,
            UsageBreakdownKind::Project,
            UsageBreakdownKind::Provider,
            UsageBreakdownKind::Profile,
        ] {
            let normalized = breakdown_sql(kind).to_ascii_lowercase();
            assert!(normalized.contains("usage_time_rollup"));
            assert!(!normalized.contains("usage_event"));
            assert!(!normalized.contains("usage_legacy_event"));
            assert!(!normalized.contains(" offset "));
            let explain = format!("EXPLAIN QUERY PLAN {}", breakdown_sql(kind));
            let mut statement = store.connection.prepare(&explain)?;
            let rows = statement.query_map(params_from_iter(parameters.iter()), |row| {
                row.get::<_, String>(3)
            })?;
            let mut details = Vec::new();
            for row in rows {
                details.push(row?);
            }
            assert!(details.join("\n").contains("usage_time_rollup"));
        }
        Ok(())
    }

    #[test]
    fn rhythm_queries_are_bounded_rollup_only_and_offset_free() {
        for dimension in ["hour_index", "weekday_index"] {
            let normalized = rhythm_sql(MAX_USAGE_RHYTHM_SEGMENTS, dimension).to_ascii_lowercase();
            assert!(normalized.contains("usage_time_rollup"));
            assert!(!normalized.contains("usage_event"));
            assert!(!normalized.contains("usage_legacy_event"));
            assert!(!normalized.contains(" offset "));
            let last = 3 + (MAX_USAGE_RHYTHM_SEGMENTS - 1) * 5;
            assert!(normalized.contains(&format!(
                "(?{last}, ?{}, ?{}, ?{}, ?{})",
                last + 1,
                last + 2,
                last + 3,
                last + 4
            )));
        }
    }

    #[test]
    fn analytics_snapshot_stays_exact_across_concurrent_state_change() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let writer_path = path.clone();
        let capture = store.capture_usage_analytics_with_options(
            empty_query()?,
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
        assert_eq!(capture.overview().event_count(), 0);
        let error = match store.capture_usage_analytics(empty_query()?) {
            Err(error) => error,
            Ok(_) => return Err("new analytics snapshot ignored rebuild state".into()),
        };
        assert_eq!(error.code(), StoreErrorCode::RebuildRequired);
        Ok(())
    }

    #[test]
    fn analytics_progress_cancellation_is_cleared_for_next_query() -> TestResult {
        let (_directory, path) = empty_archive()?;
        let mut store = UsageReadStore::open(&path)?;
        let interrupted =
            match store.capture_usage_analytics_with_options(empty_query()?, 1, true, || Ok(())) {
                Err(error) => error,
                Ok(_) => return Err("forced analytics cancellation unexpectedly completed".into()),
            };
        assert_eq!(interrupted.code(), StoreErrorCode::DeadlineExceeded);
        let next = store.capture_usage_analytics(empty_query()?)?;
        assert_eq!(next.overview().event_count(), 0);
        Ok(())
    }
}
