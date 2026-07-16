#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod benefit;
mod git_output;
mod identity;
mod quota;
mod session;
mod state;
mod usage;

pub use benefit::{
    BenefitConfidence, BenefitDetailKind, BenefitError, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLocalDate, BenefitLocalDateTime, BenefitLocalTime,
    BenefitLotId, BenefitLotObservation, BenefitLotObservationParts, BenefitObservationId,
    BenefitScope, BenefitState, BenefitTarget, BenefitTimeZoneId, MAX_BENEFIT_LOTS_PER_OBSERVATION,
    MAX_REMINDER_LEAD_SECONDS, MAX_REMINDER_THRESHOLDS, MIN_REMINDER_LEAD_SECONDS,
    NotificationChannel, RECOMMENDED_REMINDER_LEAD_SECONDS, ReminderLeadTime, ReminderProfile,
    ReminderProfileParts, ReminderProfileRevision,
};
pub use git_output::{
    GitActivityAssociationId, GitLineMetrics, GitOutputCategory, GitOutputCategoryMetrics,
    GitOutputDay, GitOutputError, GitOutputPortfolio, GitOutputProjection,
    GitOutputProjectionParts, GitOutputQuality, GitOutputTotals, GitOutputUnavailableReason,
    GitOutputWarning, GitRepositoryId, MAX_GIT_OUTPUT_CATEGORIES, MAX_GIT_OUTPUT_DAYS,
    MAX_GIT_OUTPUT_REPOSITORIES, MAX_GIT_OUTPUT_WARNINGS,
};
pub use identity::{LayoutId, LocaleId, RouteId, ThemeId};
pub use quota::{
    QuotaAccountId, QuotaConfidence, QuotaError, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
    QuotaResetThresholds, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId,
    QuotaWindowKey, QuotaWindowSemantics, QuotaWorkspaceId,
};
pub use session::SessionSummary;
pub use state::AppState;
pub use usage::{
    ActivityCounts, ActivityKind, LongContextState, MAX_METADATA_BYTES, MAX_MODEL_KEY_BYTES,
    MAX_PROVIDER_ID_BYTES, MAX_SESSION_ID_BYTES, MAX_USAGE_ID_BYTES, MetadataValue, ModelKey,
    ObservationDraft, ObservationDraftParts, ObservationVerification, ProjectAlias,
    ReportedCostUsdMicros, SessionRelationDraft, SessionRelationDraftParts, TokenCount, TokenUsage,
    UsageError, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};
