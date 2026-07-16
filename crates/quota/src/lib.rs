#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod detector;
mod identity;

pub use detector::{
    QuotaAllowanceChange, QuotaAllowanceChangeKind, QuotaDetectionTime, QuotaEpochState,
    QuotaEpochStateParts, QuotaError, QuotaErrorCode, QuotaEvaluation, QuotaTransition,
    QuotaTransitionKind, evaluate_sample,
};
pub use identity::{QuotaEpochId, QuotaScopeId, QuotaTransitionId, quota_epoch_id, quota_scope_id};
