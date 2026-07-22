use std::{sync::Arc, time::Duration};

use tokenmaster_domain::{ModelKey, ProjectAlias, UsageProfileId, UsageProviderId};
use tokenmaster_pricing::{
    ContextClass, CostMode, CostResult, PriceBasisRow, PriceError, PriceErrorCode, PricingEngine,
    ServiceTier, TokenPriceBasis, UsdMicros, select_cost,
};
use tokenmaster_store::{
    MAX_USAGE_RHYTHM_OCCURRENCES, MAX_USAGE_RHYTHM_SEGMENTS, ScanScope, USAGE_RHYTHM_HOURS,
    USAGE_RHYTHM_WEEKDAYS, UsageAggregateActivity as StoreActivity,
    UsageAggregateMetrics as StoreMetrics, UsageAnalyticsCapture as StoreCapture,
    UsageAnalyticsQuery as StoreQuery, UsageBreakdown as StoreBreakdown,
    UsageBreakdownIdentity as StoreBreakdownIdentity, UsageBreakdownKind as StoreBreakdownKind,
    UsageBreakdownPriceBasisQuery as StoreBreakdownPriceQuery,
    UsagePriceBasisBatchCapture as StorePriceBatchCapture,
    UsagePriceBasisBatchQuery as StorePriceBatchQuery,
    UsagePriceBasisTargetCapture as StorePriceTarget, UsagePriceLongContext as StorePriceContext,
    UsagePriceTier as StorePriceTier, UsageQueryDatasetIdentity as StoreDatasetIdentity,
    UsageReportedCostState as StoreReportedState, UsageRhythmQuery as StoreRhythmQuery,
    UsageRhythmSegment as StoreRhythmSegment, UsageTokenAggregate as StoreTokenAggregate,
};

use crate::{
    CalendarDate, QueryError, QueryErrorCode, QueryScope, UsageTimeZone, WeekStart,
    calendar::{CalendarBoundaryResolver, CalendarBucket},
};

pub const MAX_QUERY_SERIES_POINTS: usize = 400;
const MAX_QUERY_BREAKDOWNS: usize = 4;
const MAX_RHYTHM_DAYS: u16 = 30;

#[derive(Clone, Debug, Eq, PartialEq)]
enum UsageRangeValue {
    Today,
    RecentDays(u16),
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

    pub fn recent_days(day_count: u16) -> Result<Self, QueryError> {
        if day_count == 0 {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        if usize::from(day_count) > MAX_QUERY_SERIES_POINTS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        Ok(Self(UsageRangeValue::RecentDays(day_count)))
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
            UsageRangeValue::RecentDays(_) => "recent_days",
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageRhythmSelection {
    None,
    HourAndWeekday,
}

impl UsageRhythmSelection {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::HourAndWeekday => "hour_and_weekday",
        }
    }
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
    rhythm: UsageRhythmSelection,
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
            rhythm: UsageRhythmSelection::None,
        })
    }

    pub fn with_rhythm(mut self, rhythm: UsageRhythmSelection) -> Result<Self, QueryError> {
        if rhythm == UsageRhythmSelection::HourAndWeekday
            && !matches!(
                self.range.0,
                UsageRangeValue::RecentDays(1..=MAX_RHYTHM_DAYS)
            )
        {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        self.rhythm = rhythm;
        Ok(self)
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

    #[must_use]
    pub const fn rhythm(&self) -> UsageRhythmSelection {
        self.rhythm
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
    cost: CostResult,
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

    #[must_use]
    pub const fn cost(&self) -> &CostResult {
        &self.cost
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
    cost: CostResult,
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

    #[must_use]
    pub const fn cost(&self) -> &CostResult {
        &self.cost
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageBreakdown {
    kind: UsageBreakdownKind,
    items: Arc<[UsageBreakdownItem]>,
    truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageWeekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl UsageWeekday {
    const ALL: [Self; USAGE_RHYTHM_WEEKDAYS] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Monday => "monday",
            Self::Tuesday => "tuesday",
            Self::Wednesday => "wednesday",
            Self::Thursday => "thursday",
            Self::Friday => "friday",
            Self::Saturday => "saturday",
            Self::Sunday => "sunday",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRhythmHour {
    hour: u8,
    metrics: UsageMetrics,
    elapsed_minutes: u64,
    occurrence_count: u16,
}

impl UsageRhythmHour {
    #[must_use]
    pub const fn hour(&self) -> u8 {
        self.hour
    }
    #[must_use]
    pub const fn metrics(&self) -> &UsageMetrics {
        &self.metrics
    }
    #[must_use]
    pub const fn elapsed_minutes(&self) -> u64 {
        self.elapsed_minutes
    }
    #[must_use]
    pub const fn occurrence_count(&self) -> u16 {
        self.occurrence_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRhythmWeekday {
    weekday: UsageWeekday,
    metrics: UsageMetrics,
    elapsed_minutes: u64,
    occurrence_count: u16,
}

impl UsageRhythmWeekday {
    #[must_use]
    pub const fn weekday(&self) -> UsageWeekday {
        self.weekday
    }
    #[must_use]
    pub const fn metrics(&self) -> &UsageMetrics {
        &self.metrics
    }
    #[must_use]
    pub const fn elapsed_minutes(&self) -> u64 {
        self.elapsed_minutes
    }
    #[must_use]
    pub const fn occurrence_count(&self) -> u16 {
        self.occurrence_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRhythm {
    hours: Arc<[UsageRhythmHour]>,
    weekdays: Arc<[UsageRhythmWeekday]>,
}

impl UsageRhythm {
    #[must_use]
    pub const fn hours(&self) -> &Arc<[UsageRhythmHour]> {
        &self.hours
    }
    #[must_use]
    pub const fn weekdays(&self) -> &Arc<[UsageRhythmWeekday]> {
        &self.weekdays
    }
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
    overview_cost: CostResult,
    series: Arc<[UsageSeriesPoint]>,
    breakdowns: Arc<[UsageBreakdown]>,
    rhythm: Option<UsageRhythm>,
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
    pub const fn overview_cost(&self) -> &CostResult {
        &self.overview_cost
    }
    #[must_use]
    pub const fn series(&self) -> &Arc<[UsageSeriesPoint]> {
        &self.series
    }
    #[must_use]
    pub const fn breakdowns(&self) -> &Arc<[UsageBreakdown]> {
        &self.breakdowns
    }

    #[must_use]
    pub const fn rhythm(&self) -> Option<&UsageRhythm> {
        self.rhythm.as_ref()
    }
}

pub(crate) struct UsageAnalyticsPlan {
    time_zone_id: Arc<str>,
    overview: CalendarBucket,
    series: Box<[CalendarBucket]>,
    rhythm: Option<UsageRhythmPlan>,
}

pub(crate) struct UsageRhythmPlan {
    store_query: StoreRhythmQuery,
    hour_elapsed_minutes: [u64; USAGE_RHYTHM_HOURS],
    hour_occurrence_count: [u16; USAGE_RHYTHM_HOURS],
    weekday_elapsed_minutes: [u64; USAGE_RHYTHM_WEEKDAYS],
    weekday_occurrence_count: [u16; USAGE_RHYTHM_WEEKDAYS],
}

impl UsageAnalyticsPlan {
    pub(crate) const fn overview(&self) -> &CalendarBucket {
        &self.overview
    }
}

pub(crate) fn build_plan(
    request: &UsageAnalyticsRequest,
    generated_at_ms: i64,
) -> Result<UsageAnalyticsPlan, QueryError> {
    let resolver = CalendarBoundaryResolver::new(request.time_zone.clone())?;
    let overview = match request.range.0 {
        UsageRangeValue::Today => resolver.day(resolver.local_date_at_ms(generated_at_ms)?)?,
        UsageRangeValue::RecentDays(day_count) => {
            let today = resolver.local_date_at_ms(generated_at_ms)?;
            let start = today.days_before(day_count - 1)?;
            resolver.custom(start, today.tomorrow()?)?
        }
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
    let rhythm = if request.rhythm == UsageRhythmSelection::HourAndWeekday {
        Some(build_rhythm_plan(&resolver, &overview)?)
    } else {
        None
    };
    Ok(UsageAnalyticsPlan {
        time_zone_id: Arc::from(resolver.canonical_id()),
        overview,
        series: series.into_boxed_slice(),
        rhythm,
    })
}

fn build_rhythm_plan(
    resolver: &CalendarBoundaryResolver,
    overview: &CalendarBucket,
) -> Result<UsageRhythmPlan, QueryError> {
    let mut store_segments = Vec::new();
    let mut hour_elapsed_minutes = [0u64; USAGE_RHYTHM_HOURS];
    let mut hour_occurrence_count = [0u16; USAGE_RHYTHM_HOURS];
    let mut weekday_elapsed_minutes = [0u64; USAGE_RHYTHM_WEEKDAYS];
    let mut weekday_occurrence_count = [0u16; USAGE_RHYTHM_WEEKDAYS];
    let mut occurrence_count = 0usize;
    let mut occurrence_start = overview.start_seconds();
    let mut current_key = resolver.rhythm_minute_key(occurrence_start)?;
    let mut cursor = occurrence_start
        .checked_add(60)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    while cursor < overview.end_seconds() {
        let key = resolver.rhythm_minute_key(cursor)?;
        if key != current_key {
            append_rhythm_occurrence(
                resolver,
                occurrence_start,
                cursor,
                current_key,
                &mut store_segments,
                &mut hour_elapsed_minutes,
                &mut hour_occurrence_count,
                &mut weekday_elapsed_minutes,
                &mut weekday_occurrence_count,
                &mut occurrence_count,
            )?;
            occurrence_start = cursor;
            current_key = key;
        }
        cursor = cursor
            .checked_add(60)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    }
    append_rhythm_occurrence(
        resolver,
        occurrence_start,
        overview.end_seconds(),
        current_key,
        &mut store_segments,
        &mut hour_elapsed_minutes,
        &mut hour_occurrence_count,
        &mut weekday_elapsed_minutes,
        &mut weekday_occurrence_count,
        &mut occurrence_count,
    )?;
    let store_query = StoreRhythmQuery::new(store_segments.into_boxed_slice())
        .map_err(crate::service::map_store_error)?;
    Ok(UsageRhythmPlan {
        store_query,
        hour_elapsed_minutes,
        hour_occurrence_count,
        weekday_elapsed_minutes,
        weekday_occurrence_count,
    })
}

#[allow(clippy::too_many_arguments)]
fn append_rhythm_occurrence(
    resolver: &CalendarBoundaryResolver,
    start_seconds: i64,
    end_seconds: i64,
    key: crate::calendar::RhythmMinuteKey,
    store_segments: &mut Vec<StoreRhythmSegment>,
    hour_elapsed_minutes: &mut [u64; USAGE_RHYTHM_HOURS],
    hour_occurrence_count: &mut [u16; USAGE_RHYTHM_HOURS],
    weekday_elapsed_minutes: &mut [u64; USAGE_RHYTHM_WEEKDAYS],
    weekday_occurrence_count: &mut [u16; USAGE_RHYTHM_WEEKDAYS],
    occurrence_count: &mut usize,
) -> Result<(), QueryError> {
    *occurrence_count = occurrence_count
        .checked_add(1)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    if *occurrence_count > MAX_USAGE_RHYTHM_OCCURRENCES {
        return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
    }
    let hour = usize::from(key.hour);
    let weekday = usize::from(key.weekday_index);
    let elapsed = u64::try_from((end_seconds - start_seconds) / 60)
        .map_err(|_| QueryError::new(QueryErrorCode::Overflow))?;
    hour_elapsed_minutes[hour] = hour_elapsed_minutes[hour]
        .checked_add(elapsed)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    weekday_elapsed_minutes[weekday] = weekday_elapsed_minutes[weekday]
        .checked_add(elapsed)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    hour_occurrence_count[hour] = hour_occurrence_count[hour]
        .checked_add(1)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    weekday_occurrence_count[weekday] = weekday_occurrence_count[weekday]
        .checked_add(1)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    for segment in resolver.rhythm_segments(start_seconds, end_seconds)? {
        if store_segments.len() == MAX_USAGE_RHYTHM_SEGMENTS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        store_segments.push(
            StoreRhythmSegment::new(key.hour, key.weekday_index, segment)
                .map_err(crate::service::map_store_error)?,
        );
    }
    Ok(())
}

pub(crate) fn build_store_query(
    request: &UsageAnalyticsRequest,
    generated_at_ms: i64,
    deadline: Duration,
) -> Result<(UsageAnalyticsPlan, StoreQuery), QueryError> {
    let plan = build_plan(request, generated_at_ms)?;
    let query = build_store_query_from_plan(&plan, request, deadline)?;
    Ok((plan, query))
}

pub(crate) fn build_store_query_from_plan(
    plan: &UsageAnalyticsPlan,
    request: &UsageAnalyticsRequest,
    deadline: Duration,
) -> Result<StoreQuery, QueryError> {
    let overview = &plan.overview;
    let series = &plan.series;
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
    let mut query = StoreQuery::new(
        None,
        store_overview,
        store_series,
        store_breakdowns,
        store_scopes,
        deadline,
    )
    .map_err(crate::service::map_store_error)?;
    if let Some(rhythm) = &plan.rhythm {
        query = query
            .with_rhythm(rhythm.store_query.clone())
            .map_err(crate::service::map_store_error)?;
    }
    Ok(query)
}

pub(crate) fn build_store_price_query(
    plan: &UsageAnalyticsPlan,
    expected_dataset: StoreDatasetIdentity,
    scopes: &[QueryScope],
    deadline: Duration,
) -> Result<StorePriceBatchQuery, QueryError> {
    let mut ranges = Vec::with_capacity(plan.series.len() + 1);
    ranges.push(plan.overview.clone().into_store_range()?);
    for bucket in &plan.series {
        ranges.push(bucket.clone().into_store_range()?);
    }
    let scopes = scopes
        .iter()
        .map(|scope| ScanScope::new(scope.provider_id().as_str(), scope.profile_id().as_str()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))?;
    StorePriceBatchQuery::new(
        Some(expected_dataset),
        ranges.into_boxed_slice(),
        scopes.into_boxed_slice(),
        deadline,
    )
    .map_err(crate::service::map_store_error)
}

pub(crate) fn build_store_breakdown_price_queries(
    plan: &UsageAnalyticsPlan,
    capture: &StoreCapture,
    scopes: &[QueryScope],
    deadline: Duration,
) -> Result<Vec<Option<StoreBreakdownPriceQuery>>, QueryError> {
    let expected_dataset = capture.publication().dataset_identity();
    let range = plan.overview.clone().into_store_range()?;
    let scopes = scopes
        .iter()
        .map(|scope| ScanScope::new(scope.provider_id().as_str(), scope.profile_id().as_str()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))?
        .into_boxed_slice();
    capture
        .breakdowns()
        .iter()
        .map(|breakdown| {
            if breakdown.items().is_empty() {
                return Ok(None);
            }
            let targets = breakdown
                .items()
                .iter()
                .map(|item| item.identity().clone())
                .collect::<Vec<_>>()
                .into_boxed_slice();
            StoreBreakdownPriceQuery::new(
                expected_dataset,
                range.clone(),
                scopes.clone(),
                breakdown.kind(),
                targets,
                deadline,
            )
            .map(Some)
            .map_err(crate::service::map_store_error)
        })
        .collect()
}

pub(crate) fn map_capture(
    plan: UsageAnalyticsPlan,
    capture: &StoreCapture,
    price_capture: &StorePriceBatchCapture,
    breakdown_price_captures: &[Option<StorePriceBatchCapture>],
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<UsageAnalytics, QueryError> {
    if plan.series.len() != capture.series().len()
        || plan.rhythm.is_some() != capture.rhythm().is_some()
        || price_capture.publication().dataset_identity()
            != capture.publication().dataset_identity()
        || price_capture.targets().len() != plan.series.len() + 1
        || breakdown_price_captures.len() != capture.breakdowns().len()
        || breakdown_price_captures.iter().flatten().any(|prices| {
            prices.publication().dataset_identity() != capture.publication().dataset_identity()
        })
    {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    let overview = UsageMetrics::from_store(capture.overview())?;
    let overview_cost = map_cost(
        &price_capture.targets()[0],
        overview.event_count(),
        pricing,
        cost_mode,
    )?;
    let mut series = Vec::with_capacity(plan.series.len());
    for ((bucket, captured), price) in plan
        .series
        .iter()
        .zip(capture.series())
        .zip(&price_capture.targets()[1..])
    {
        if bucket.start_seconds() != captured.start_seconds()
            || bucket.end_seconds() != captured.end_seconds()
        {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
        let metrics = UsageMetrics::from_store(captured.metrics())?;
        let cost = map_cost(price, metrics.event_count(), pricing, cost_mode)?;
        series.push(UsageSeriesPoint {
            start_date: bucket.start_date(),
            end_date: bucket.end_date(),
            start_seconds: bucket.start_seconds(),
            end_seconds: bucket.end_seconds(),
            metrics,
            cost,
        });
    }
    let breakdowns = capture
        .breakdowns()
        .iter()
        .zip(breakdown_price_captures)
        .map(|(breakdown, prices)| map_breakdown(breakdown, prices.as_ref(), pricing, cost_mode))
        .collect::<Result<Vec<_>, _>>()?;
    let rhythm = match (&plan.rhythm, capture.rhythm()) {
        (Some(plan), Some(capture)) => Some(map_rhythm(plan, capture)?),
        (None, None) => None,
        _ => return Err(QueryError::new(QueryErrorCode::CorruptArchive)),
    };
    Ok(UsageAnalytics {
        range: ResolvedUsageRange {
            time_zone_id: plan.time_zone_id,
            start_date: plan.overview.start_date(),
            end_date: plan.overview.end_date(),
            start_seconds: plan.overview.start_seconds(),
            end_seconds: plan.overview.end_seconds(),
        },
        overview,
        overview_cost,
        series: Arc::from(series),
        breakdowns: Arc::from(breakdowns),
        rhythm,
    })
}

fn map_rhythm(
    plan: &UsageRhythmPlan,
    capture: &tokenmaster_store::UsageRhythmCapture,
) -> Result<UsageRhythm, QueryError> {
    if capture.hours().len() != USAGE_RHYTHM_HOURS
        || capture.weekdays().len() != USAGE_RHYTHM_WEEKDAYS
    {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    let hours = capture
        .hours()
        .iter()
        .enumerate()
        .map(|(index, metrics)| {
            Ok(UsageRhythmHour {
                hour: u8::try_from(index).map_err(|_| QueryError::new(QueryErrorCode::Internal))?,
                metrics: UsageMetrics::from_store(metrics)?,
                elapsed_minutes: plan.hour_elapsed_minutes[index],
                occurrence_count: plan.hour_occurrence_count[index],
            })
        })
        .collect::<Result<Vec<_>, QueryError>>()?;
    let weekdays = capture
        .weekdays()
        .iter()
        .enumerate()
        .map(|(index, metrics)| {
            Ok(UsageRhythmWeekday {
                weekday: UsageWeekday::ALL[index],
                metrics: UsageMetrics::from_store(metrics)?,
                elapsed_minutes: plan.weekday_elapsed_minutes[index],
                occurrence_count: plan.weekday_occurrence_count[index],
            })
        })
        .collect::<Result<Vec<_>, QueryError>>()?;
    Ok(UsageRhythm {
        hours: Arc::from(hours),
        weekdays: Arc::from(weekdays),
    })
}

pub(crate) fn map_cost(
    capture: &StorePriceTarget,
    expected_event_count: u64,
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<CostResult, QueryError> {
    map_cost_parts(
        capture.rows(),
        capture.included(),
        capture.omitted(),
        expected_event_count,
        pricing,
        cost_mode,
    )
}

pub(crate) fn map_single_cost(
    capture: &tokenmaster_store::UsagePriceBasisCapture,
    expected_event_count: u64,
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<CostResult, QueryError> {
    map_cost_parts(
        capture.rows(),
        capture.included(),
        capture.omitted(),
        expected_event_count,
        pricing,
        cost_mode,
    )
}

fn map_cost_parts(
    rows: &[tokenmaster_store::UsagePriceBasisRow],
    included: tokenmaster_store::UsagePriceBasisMetrics,
    omitted: tokenmaster_store::UsagePriceBasisMetrics,
    expected_event_count: u64,
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<CostResult, QueryError> {
    let observed_event_count = included
        .event_count()
        .checked_add(omitted.event_count())
        .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
    if observed_event_count != expected_event_count {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    let rows = rows.iter().map(map_price_row).collect::<Vec<_>>();
    select_cost(pricing, cost_mode, &rows, omitted.event_count()).map_err(map_price_error)
}

fn map_price_row(row: &tokenmaster_store::UsagePriceBasisRow) -> PriceBasisRow<'_> {
    let metrics = row.metrics();
    let tier = match row.key().tier() {
        StorePriceTier::StandardReported => ServiceTier::StandardReported,
        StorePriceTier::StandardAssumed => ServiceTier::StandardAssumed,
        StorePriceTier::Priority => ServiceTier::Priority,
        StorePriceTier::Unknown => ServiceTier::Unknown,
    };
    let context = match row.key().long_context() {
        StorePriceContext::Yes => ContextClass::Long,
        StorePriceContext::No => ContextClass::Short,
        StorePriceContext::Unavailable => ContextClass::Unavailable,
    };
    let reported_cost = match row.key().reported_cost_state() {
        StoreReportedState::Present => Some(UsdMicros::new(metrics.reported_cost_usd_micros())),
        StoreReportedState::Missing => None,
    };
    PriceBasisRow {
        model: row.key().model(),
        tier,
        context,
        event_count: metrics.event_count(),
        calculable_event_count: metrics.calculable_event_count(),
        basis: TokenPriceBasis::new(
            metrics.uncached_input_tokens(),
            metrics.cached_input_tokens(),
            metrics.billable_output_tokens(),
        ),
        reported_event_count: metrics.reported_cost_count(),
        reported_cost,
    }
}

fn map_price_error(error: PriceError) -> QueryError {
    let code = match error.code() {
        PriceErrorCode::ArithmeticOverflow => QueryErrorCode::Overflow,
        PriceErrorCode::InvalidPriceRow | PriceErrorCode::TooManyPriceRows => {
            QueryErrorCode::CorruptArchive
        }
        PriceErrorCode::ModelUnpriced
        | PriceErrorCode::TierUnknown
        | PriceErrorCode::ContextUnavailable
        | PriceErrorCode::TierContextUnsupported
        | PriceErrorCode::TokenBasisUnavailable
        | PriceErrorCode::InconsistentTokenBasis
        | PriceErrorCode::InvalidRate => QueryErrorCode::Internal,
    };
    QueryError::new(code)
}

pub(crate) fn map_breakdown(
    value: &StoreBreakdown,
    prices: Option<&StorePriceBatchCapture>,
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<UsageBreakdown, QueryError> {
    let kind = UsageBreakdownKind::from_store(value.kind());
    if value.items().is_empty() {
        if prices.is_some() {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
    } else {
        let prices = prices.ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
        if prices.targets().len() != value.items().len() {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
    }
    let items = value
        .items()
        .iter()
        .enumerate()
        .map(|(index, item)| {
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
            let metrics = UsageMetrics::from_store(item.metrics())?;
            let cost = map_cost(
                &prices
                    .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?
                    .targets()[index],
                metrics.event_count(),
                pricing,
                cost_mode,
            )?;
            Ok(UsageBreakdownItem {
                identity,
                metrics,
                cost,
            })
        })
        .collect::<Result<Vec<_>, QueryError>>()?;
    Ok(UsageBreakdown {
        kind,
        items: Arc::from(items),
        truncated: value.truncated(),
    })
}
