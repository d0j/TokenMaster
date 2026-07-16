//! Bounded immutable read values for TokenMaster frontends.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod activity;
mod analytics;
mod calendar;
mod clock;
mod error;
mod identity;
mod publication;
mod service;
mod session;

pub use activity::{
    ActivityCursor, ActivityItem, ActivityItemParts, LatestActivityPage, MAX_QUERY_PAGE_SIZE,
    PageSize,
};
pub use analytics::{
    AggregateTokenValue, MAX_QUERY_SERIES_POINTS, ResolvedUsageRange, UsageActivity,
    UsageAnalytics, UsageAnalyticsRequest, UsageBreakdown, UsageBreakdownIdentity,
    UsageBreakdownItem, UsageBreakdownKind, UsageMetrics, UsageRange, UsageSeriesPoint,
    UsageSeriesSelection,
};
pub use calendar::{CalendarDate, UsageTimeZone, WeekStart};
pub use clock::{QueryClock, QueryTimeSample, SystemQueryClock};
pub use error::{QueryError, QueryErrorCode};
pub use identity::{
    DatasetGeneration, DatasetIdentity, MAX_QUERY_SCOPES, MAX_QUERY_WARNINGS,
    PublicationGeneration, QUERY_SCHEMA_VERSION, QueryEnvelope, QueryFreshness, QueryHeader,
    QueryHeaderParts, QueryQuality, QueryScope, QueryWarningCode, ReplayRevision,
    SnapshotGeneration,
};
pub use publication::{PublishOutcome, QuerySnapshotSlot};
pub use service::{
    LatestActivityRequest, QUERY_FRESH_MAX_AGE_MS, QUERY_STALE_MIN_AGE_MS, QueryService,
};
pub use session::{
    UsageSessionCursor, UsageSessionDetail, UsageSessionDetailResult, UsageSessionKey,
    UsageSessionPage, UsageSessionPageRequest, UsageSessionSummary,
};
pub use tokenmaster_pricing::{
    CostAvailability, CostComposition, CostCounters, CostMode, CostResult, MissingCost,
    MissingReasonCode, OverrideRevision, OverrideSnapshot, PricingEngine,
};
