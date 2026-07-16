#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod identity;
mod quota;
mod session;
mod state;
mod usage;

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
