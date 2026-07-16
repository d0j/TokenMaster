use std::{sync::Arc, time::Duration};

use tokenmaster_domain::{ModelKey, ProjectAlias, UsageProfileId, UsageProviderId};
use tokenmaster_store::{
    ScanScope, UsageAggregateActivity as StoreActivity, UsageAggregateMetrics as StoreMetrics,
    UsageAnalyticsCapture as StoreCapture, UsageAnalyticsQuery as StoreQuery,
    UsageBreakdown as StoreBreakdown, UsageBreakdownIdentity as StoreBreakdownIdentity,
    UsageBreakdownKind as StoreBreakdownKind, UsageTokenAggregate as StoreTokenAggregate,
};

use crate::{
    CalendarDate, QueryError, QueryErrorCode, QueryScope, UsageTimeZone, WeekStart,
    calendar::{CalendarBoundaryResolver, CalendarBucket},
};

pub const MAX_QUERY_SERIES_POINTS: usize = 400;
const MAX_QUERY_BREAKDOWNS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
enum UsageRangeValue {
    Today,
    Day(CalendarDate),
    Week(CalendarDate),
    Month(CalendarDate),
    Custom {
        start: CalendarDate,
        end: CalendarDate,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRange(UsageRangeValue);

impl UsageRange {
    #[must_use]
    pub const fn today() -> Self {
        Self(UsageRangeValue::Today)
    }

    #[must_use]
    pub const fn day(date: CalendarDate) -> Self {
        Self(UsageRangeValue::Day(date))
    }

    #[must_use]
    pub const fn week(date: CalendarDate) -> Self {
        Self(UsageRangeValue::Week(date))
    }

    #[must_use]
    pub const fn month(date: CalendarDate) -> Self {
        Self(UsageRangeValue::Month(date))
    }

    pub fn custom(start: CalendarDate, end: CalendarDate) -> Result<Self, QueryError> {
        let point_count = start.days_until(end)?;
        if point_count <= 0 {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        if point_count > MAX_QUERY_SERIES_POINTS as i64 {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        Ok(Self(UsageRangeValue::Custom { start, end }))
    }

    #[must_use]
    pub const fn stable_code(&self) -> &'static str {
        match self.0 {
            UsageRangeValue::Today => "today",
            UsageRangeValue::Day(_) => "day",
            UsageRangeValue::Week(_) => "week",
            UsageRangeValue::Month(_) => "month",
            UsageRangeValue::Custom { .. } => "custom",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageSeriesSelection {
    None,
    Daily,
}

impl UsageSeriesSelection {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Daily => "daily",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UsageBreakdownKind {
    Model,
    Project,
    Provider,
    Profile,
}

impl UsageBreakdownKind {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Project => "project",
            Self::Provider => "provider",
            Self::Profile => "profile",
        }
    }

    const fn to_store(self) -> StoreBreakdownKind {
        match self {
            Self::Model => StoreBreakdownKind::Model,
            Self::Project => StoreBreakdownKind::Project,
            Self::Provider => StoreBreakdownKind::Provider,
            Self::Profile => StoreBreakdownKind::Profile,
        }
    }

    const fn from_store(value: StoreBreakdownKind) -> Self {
        match value {
            StoreBreakdownKind::Model => Self::Model,
            StoreBreakdownKind::Project => Self::Project,
            StoreBreakdownKind::Provider => Self::Provider,
            StoreBreakdownKind::Profile => Self::Profile,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageAnalyticsRequest {
    range: UsageRange,
    time_zone: UsageTimeZone,
    week_start: WeekStart,
    series: UsageSeriesSelection,
    scopes: Box<[QueryScope]>,
    breakdowns: Box<[UsageBreakdownKind]>,
}

impl UsageAnalyticsRequest {
    pub fn new(
        range: UsageRange,
        time_zone: UsageTimeZone,
        week_start: WeekStart,
        series: UsageSeriesSelection,
        mut scopes: Vec<QueryScope>,
        mut breakdowns: Vec<UsageBreakdownKind>,
    ) -> Result<Self, QueryError> {
        if scopes.len() > crate::MAX_QUERY_SCOPES || breakdowns.len() > MAX_QUERY_BREAKDOWNS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        scopes.sort_by(|left, right| {
            left.provider_id()
                .as_str()
                .cmp(right.provider_id().as_str())
                .then_with(|| left.profile_id().as_str().cmp(right.profile_id().as_str()))
        });
        if scopes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        breakdowns.sort_unstable();
        if breakdowns.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self {
            range,
            time_zone,
            week_start,
            series,
            scopes: scopes.into_boxed_slice(),
            breakdowns: breakdowns.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn range(&self) -> &UsageRange {
        &self.range
    }

    #[must_use]
    pub const fn time_zone(&self) -> &UsageTimeZone {
        &self.time_zone
    }

    #[must_use]
    pub const fn week_start(&self) -> WeekStart {
        self.week_start
    }

    #[must_use]
    pub const fn series(&self) -> UsageSeriesSelection {
        self.series
    }

    #[must_use]
    pub const fn scopes(&self) -> &[QueryScope] {
        &self.scopes
    }

    #[must_use]
    pub const fn breakdowns(&self) -> &[UsageBreakdownKind] {
        &self.breakdowns
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregateTokenValue {
    Unavailable,
    Known(u64),
    Partial {
        known_sum: u64,
        known_count: u64,
        event_count: u64,
    },
}

impl AggregateTokenValue {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Known(_) => "known",
            Self::Partial { .. } => "partial",
        }
    }

    fn from_store(value: StoreTokenAggregate, event_count: u64) -> Result<Self, QueryError> {
        let known_count = value.known_count();
        let known_sum = value.known_sum();
        if known_count > event_count || (known_count == 0 && known_sum != 0) {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
        if known_count == 0 {
            Ok(Self::Unavailable)
        } else if known_count == event_count {
            Ok(Self::Known(known_sum))
        } else {
            Ok(Self::Partial {
                known_sum,
                known_count,
                event_count,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsageActivity {
    read: u64,
    edit_write: u64,
    search: u64,
    git: u64,
    build_test: u64,
    web: u64,
    subagents: u64,
    terminal: u64,
}

impl UsageActivity {
    #[must_use]
    pub const fn read(self) -> u64 {
        self.read
    }
    #[must_use]
    pub const fn edit_write(self) -> u64 {
        self.edit_write
    }
    #[must_use]
    pub const fn search(self) -> u64 {
        self.search
    }
    #[must_use]
    pub const fn git(self) -> u64 {
        self.git
    }
    #[must_use]
    pub const fn build_test(self) -> u64 {
        self.build_test
    }
    #[must_use]
    pub const fn web(self) -> u64 {
        self.web
    }
    #[must_use]
    pub const fn subagents(self) -> u64 {
        self.subagents
    }
    #[must_use]
    pub const fn terminal(self) -> u64 {
        self.terminal
    }

    const fn from_store(value: StoreActivity) -> Self {
        Self {
            read: value.read(),
            edit_write: value.edit_write(),
            search: value.search(),
            git: value.git(),
            build_test: value.build_test(),
            web: value.web(),
            subagents: value.subagents(),
            terminal: value.terminal(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageMetrics {
    event_count: u64,
    input: AggregateTokenValue,
    cached: AggregateTokenValue,
    output: AggregateTokenValue,
    reasoning: AggregateTokenValue,
    total: AggregateTokenValue,
    fallback_model_count: u64,
    long_context_yes_count: u64,
    long_context_no_count: u64,
    long_context_unavailable_count: u64,
    activity: UsageActivity,
}

impl UsageMetrics {
    pub(crate) fn from_store(value: &StoreMetrics) -> Result<Self, QueryError> {
        let event_count = value.event_count();
        let long_context_count = value
            .long_context_yes_count()
            .checked_add(value.long_context_no_count())
            .and_then(|count| count.checked_add(value.long_context_unavailable_count()))
            .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
        if value.fallback_model_count() > event_count || long_context_count != event_count {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
        Ok(Self {
            event_count,
            input: AggregateTokenValue::from_store(value.input(), event_count)?,
            cached: AggregateTokenValue::from_store(value.cached(), event_count)?,
            output: AggregateTokenValue::from_store(value.output(), event_count)?,
            reasoning: AggregateTokenValue::from_store(value.reasoning(), event_count)?,
            total: AggregateTokenValue::from_store(value.total(), event_count)?,
            fallback_model_count: value.fallback_model_count(),
            long_context_yes_count: value.long_context_yes_count(),
            long_context_no_count: value.long_context_no_count(),
            long_context_unavailable_count: value.long_context_unavailable_count(),
            activity: UsageActivity::from_store(value.activity()),
        })
    }

    #[must_use]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }
    #[must_use]
    pub const fn input(&self) -> AggregateTokenValue {
        self.input
    }
    #[must_use]
    pub const fn cached(&self) -> AggregateTokenValue {
        self.cached
    }
    #[must_use]
    pub const fn output(&self) -> AggregateTokenValue {
        self.output
    }
    #[must_use]
    pub const fn reasoning(&self) -> AggregateTokenValue {
        self.reasoning
    }
    #[must_use]
    pub const fn total(&self) -> AggregateTokenValue {
        self.total
    }
    #[must_use]
    pub const fn fallback_model_count(&self) -> u64 {
        self.fallback_model_count
    }
    #[must_use]
    pub const fn long_context_yes_count(&self) -> u64 {
        self.long_context_yes_count
    }
    #[must_use]
    pub const fn long_context_no_count(&self) -> u64 {
        self.long_context_no_count
    }
    #[must_use]
    pub const fn long_context_unavailable_count(&self) -> u64 {
        self.long_context_unavailable_count
    }
    #[must_use]
    pub const fn activity(&self) -> UsageActivity {
        self.activity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUsageRange {
    time_zone_id: Arc<str>,
    start_date: CalendarDate,
    end_date: CalendarDate,
    start_seconds: i64,
    end_seconds: i64,
}

impl ResolvedUsageRange {
    #[must_use]
    pub fn time_zone_id(&self) -> &str {
        &self.time_zone_id
    }
    #[must_use]
    pub const fn start_date(&self) -> CalendarDate {
        self.start_date
    }
    #[must_use]
    pub const fn end_date(&self) -> CalendarDate {
        self.end_date
    }
    #[must_use]
    pub const fn start_seconds(&self) -> i64 {
        self.start_seconds
    }
    #[must_use]
    pub const fn end_seconds(&self) -> i64 {
        self.end_seconds
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSeriesPoint {
    start_date: CalendarDate,
    end_date: CalendarDate,
    start_seconds: i64,
    end_seconds: i64,
    metrics: UsageMetrics,
}

impl UsageSeriesPoint {
    #[must_use]
    pub const fn start_date(&self) -> CalendarDate {
        self.start_date
    }
    #[must_use]
    pub const fn end_date(&self) -> CalendarDate {
        self.end_date
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
    pub const fn metrics(&self) -> &UsageMetrics {
        &self.metrics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UsageBreakdownIdentity {
    Model(ModelKey),
    Project(ProjectAlias),
    UnassociatedProject,
    Provider(UsageProviderId),
    Profile(QueryScope),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageBreakdownItem {
    identity: UsageBreakdownIdentity,
    metrics: UsageMetrics,
}

impl UsageBreakdownItem {
    #[must_use]
    pub const fn identity(&self) -> &UsageBreakdownIdentity {
        &self.identity
    }
    #[must_use]
    pub const fn metrics(&self) -> &UsageMetrics {
        &self.metrics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageBreakdown {
    kind: UsageBreakdownKind,
    items: Arc<[UsageBreakdownItem]>,
    truncated: bool,
}

impl UsageBreakdown {
    #[must_use]
    pub const fn kind(&self) -> UsageBreakdownKind {
        self.kind
    }
    #[must_use]
    pub const fn items(&self) -> &Arc<[UsageBreakdownItem]> {
        &self.items
    }
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageAnalytics {
    range: ResolvedUsageRange,
    overview: UsageMetrics,
    series: Arc<[UsageSeriesPoint]>,
    breakdowns: Arc<[UsageBreakdown]>,
}

impl UsageAnalytics {
    #[must_use]
    pub const fn range(&self) -> &ResolvedUsageRange {
        &self.range
    }
    #[must_use]
    pub const fn overview(&self) -> &UsageMetrics {
        &self.overview
    }
    #[must_use]
    pub const fn series(&self) -> &Arc<[UsageSeriesPoint]> {
        &self.series
    }
    #[must_use]
    pub const fn breakdowns(&self) -> &Arc<[UsageBreakdown]> {
        &self.breakdowns
    }
}

pub(crate) struct UsageAnalyticsPlan {
    time_zone_id: Arc<str>,
    overview: CalendarBucket,
    series: Box<[CalendarBucket]>,
}

pub(crate) fn build_store_query(
    request: &UsageAnalyticsRequest,
    generated_at_ms: i64,
    deadline: Duration,
) -> Result<(UsageAnalyticsPlan, StoreQuery), QueryError> {
    let resolver = CalendarBoundaryResolver::new(request.time_zone.clone())?;
    let overview = match request.range.0 {
        UsageRangeValue::Today => resolver.day(resolver.local_date_at_ms(generated_at_ms)?)?,
        UsageRangeValue::Day(date) => resolver.day(date)?,
        UsageRangeValue::Week(date) => resolver.week_containing(date, request.week_start)?,
        UsageRangeValue::Month(date) => resolver.month(date)?,
        UsageRangeValue::Custom { start, end } => resolver.custom(start, end)?,
    };
    let mut series = Vec::new();
    if request.series == UsageSeriesSelection::Daily {
        let mut date = overview.start_date();
        while date < overview.end_date() {
            if series.len() == MAX_QUERY_SERIES_POINTS {
                return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
            }
            series.push(resolver.day(date)?);
            date = date.tomorrow()?;
        }
    }
    let store_overview = overview.clone().into_store_range()?;
    let store_series = series
        .iter()
        .cloned()
        .map(CalendarBucket::into_store_series_point)
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let store_breakdowns = request
        .breakdowns
        .iter()
        .copied()
        .map(UsageBreakdownKind::to_store)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let store_scopes = request
        .scopes
        .iter()
        .map(|scope| ScanScope::new(scope.provider_id().as_str(), scope.profile_id().as_str()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))?
        .into_boxed_slice();
    let query = StoreQuery::new(
        None,
        store_overview,
        store_series,
        store_breakdowns,
        store_scopes,
        deadline,
    )
    .map_err(crate::service::map_store_error)?;
    Ok((
        UsageAnalyticsPlan {
            time_zone_id: Arc::from(resolver.canonical_id()),
            overview,
            series: series.into_boxed_slice(),
        },
        query,
    ))
}

pub(crate) fn map_capture(
    plan: UsageAnalyticsPlan,
    capture: &StoreCapture,
) -> Result<UsageAnalytics, QueryError> {
    if plan.series.len() != capture.series().len() {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    let mut series = Vec::with_capacity(plan.series.len());
    for (bucket, captured) in plan.series.iter().zip(capture.series()) {
        if bucket.start_seconds() != captured.start_seconds()
            || bucket.end_seconds() != captured.end_seconds()
        {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
        series.push(UsageSeriesPoint {
            start_date: bucket.start_date(),
            end_date: bucket.end_date(),
            start_seconds: bucket.start_seconds(),
            end_seconds: bucket.end_seconds(),
            metrics: UsageMetrics::from_store(captured.metrics())?,
        });
    }
    let breakdowns = capture
        .breakdowns()
        .iter()
        .map(map_breakdown)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(UsageAnalytics {
        range: ResolvedUsageRange {
            time_zone_id: plan.time_zone_id,
            start_date: plan.overview.start_date(),
            end_date: plan.overview.end_date(),
            start_seconds: plan.overview.start_seconds(),
            end_seconds: plan.overview.end_seconds(),
        },
        overview: UsageMetrics::from_store(capture.overview())?,
        series: Arc::from(series),
        breakdowns: Arc::from(breakdowns),
    })
}

pub(crate) fn map_breakdown(value: &StoreBreakdown) -> Result<UsageBreakdown, QueryError> {
    let kind = UsageBreakdownKind::from_store(value.kind());
    let items = value
        .items()
        .iter()
        .map(|item| {
            let identity = match item.identity() {
                StoreBreakdownIdentity::Model(value) => UsageBreakdownIdentity::Model(
                    ModelKey::new(value.to_string())
                        .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?,
                ),
                StoreBreakdownIdentity::Project(value) => UsageBreakdownIdentity::Project(
                    ProjectAlias::new(value.to_string())
                        .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?,
                ),
                StoreBreakdownIdentity::UnassociatedProject => {
                    UsageBreakdownIdentity::UnassociatedProject
                }
                StoreBreakdownIdentity::Provider(value) => UsageBreakdownIdentity::Provider(
                    UsageProviderId::new(value.to_string())
                        .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?,
                ),
                StoreBreakdownIdentity::Profile {
                    provider_id,
                    profile_id,
                } => UsageBreakdownIdentity::Profile(QueryScope::new(
                    UsageProviderId::new(provider_id.to_string())
                        .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?,
                    UsageProfileId::new(profile_id.to_string())
                        .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?,
                )),
            };
            Ok(UsageBreakdownItem {
                identity,
                metrics: UsageMetrics::from_store(item.metrics())?,
            })
        })
        .collect::<Result<Vec<_>, QueryError>>()?;
    Ok(UsageBreakdown {
        kind,
        items: Arc::from(items),
        truncated: value.truncated(),
    })
}
