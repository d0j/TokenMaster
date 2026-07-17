//! Bounded immutable read values for TokenMaster frontends.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod activity;
mod analytics;
mod benefit;
mod calendar;
mod clock;
mod error;
mod git_output;
mod identity;
mod publication;
mod quota;
mod quota_identity;
mod service;
mod session;
mod status;

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
pub use benefit::{
    BENEFIT_OVERVIEW_QUERY_SCHEMA_VERSION, BENEFIT_QUERY_SCHEMA_VERSION, BenefitChangeCursor,
    BenefitChangeKind, BenefitChangePage, BenefitChangePageRequest, BenefitChangeValue,
    BenefitCurrentRequest, BenefitCurrentSnapshot, BenefitEnvelope, BenefitInventoryValue,
    BenefitLotValue, BenefitOverviewEnvelope, BenefitOverviewLotValue, BenefitOverviewQueryHeader,
    BenefitOverviewQueryHeaderParts, BenefitOverviewRequest, BenefitOverviewScopeValue,
    BenefitOverviewSnapshot, BenefitQueryHeader, BenefitQueryHeaderParts, BenefitReminderCoverage,
    BenefitReminderProfileSource, BenefitReminderProfileValue, BenefitRevision, BenefitScopeFilter,
    BenefitWarningCode,
};
pub use calendar::{CalendarDate, UsageTimeZone, WeekStart};
pub use clock::{QueryClock, QueryTimeSample, SystemQueryClock};
pub use error::{QueryError, QueryErrorCode};
pub use git_output::{
    GIT_QUERY_SCHEMA_VERSION, GitEfficiency, GitEfficiencyUnavailableReason, GitEfficiencyValue,
    GitEnvelope, GitOutputRange, GitOutputRepository, GitOutputRequest, GitOutputSnapshot,
    GitPublicationRevision, GitQueryHeader, GitRangeMetrics,
};
pub use identity::{
    DatasetGeneration, DatasetIdentity, MAX_QUERY_SCOPES, MAX_QUERY_WARNINGS,
    PublicationGeneration, QUERY_SCHEMA_VERSION, QueryEnvelope, QueryFreshness, QueryHeader,
    QueryHeaderParts, QueryQuality, QueryScope, QueryWarningCode, ReplayRevision,
    SnapshotGeneration,
};
pub use publication::{PublishOutcome, QuerySnapshotSlot};
pub use quota::{
    QuotaAllowanceChangeKind, QuotaAllowanceChangeValue, QuotaConfidence, QuotaCurrentRequest,
    QuotaCurrentSnapshot, QuotaDefinitionValue, QuotaDetectionTime, QuotaEpochValue,
    QuotaEvidenceSource, QuotaPresentation, QuotaRatioValue, QuotaResetEvidence,
    QuotaResetThresholdsValue, QuotaSampleQuality, QuotaSampleValue, QuotaTransitionCursor,
    QuotaTransitionKind, QuotaTransitionPage, QuotaTransitionPageRequest, QuotaTransitionValue,
    QuotaUnitsValue, QuotaWindowResult, QuotaWindowSemantics, QuotaWindowValue,
};
pub use quota_identity::{
    QUOTA_QUERY_SCHEMA_VERSION, QuotaEnvelope, QuotaQueryHeader, QuotaQueryHeaderParts,
    QuotaRevision, QuotaWarningCode, QuotaWindowFilter,
};
pub use service::{
    LatestActivityRequest, QUERY_FRESH_MAX_AGE_MS, QUERY_STALE_MIN_AGE_MS, QueryService,
};
pub use session::{
    UsageSessionCursor, UsageSessionDetail, UsageSessionDetailResult, UsageSessionKey,
    UsageSessionPage, UsageSessionPageRequest, UsageSessionSummary,
};
pub use status::{
    PRODUCT_DATA_STATUS_SCHEMA_VERSION, ProductAggregateProgress, ProductAggregateState,
    ProductAggregateStatus, ProductBenefitDataStatus, ProductComponentState,
    ProductDataStatusEnvelope, ProductDataStatusSnapshot, ProductDataWarningCode,
    ProductGitDataStatus, ProductQuotaDataStatus, ProductUsageDataStatus,
};
pub use tokenmaster_pricing::{
    CostAvailability, CostComposition, CostCounters, CostMode, CostResult, MissingCost,
    MissingReasonCode, OverrideRevision, OverrideSnapshot, PricingEngine,
};
