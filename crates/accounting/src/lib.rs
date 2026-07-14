//! Canonical accounting identities have no public constructors.
//!
//! ```compile_fail
//! let _ = tokenmaster_accounting::EventFingerprint::new([0; 32]);
//! ```
//!
//! Canonical events can only be produced by [`Canonicalizer`].
//!
//! ```compile_fail
//! let _ = tokenmaster_accounting::CanonicalUsageEvent::new();
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod event;
mod hash;

use std::fmt;

use tokenmaster_domain::{ObservationDraft, TokenCount};

pub use event::{
    CanonicalUsageEvent, EventFingerprint, EventId, ReplayEvidence, ReplaySignature, UsageLineage,
};

pub const CANONICALIZER_VERSION: u16 = 1;
pub const EVENT_FINGERPRINT_VERSION: u16 = 2;
pub const REPLAY_SIGNATURE_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalizationErrorCode {
    EmptyUsage,
    InconsistentCumulative,
    ValueOutOfRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalizationError {
    code: CanonicalizationErrorCode,
}

impl CanonicalizationError {
    const fn new(code: CanonicalizationErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> CanonicalizationErrorCode {
        self.code
    }
}

impl fmt::Display for CanonicalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            CanonicalizationErrorCode::EmptyUsage => "usage observation has no positive counts",
            CanonicalizationErrorCode::InconsistentCumulative => {
                "cumulative usage is below the emitted delta"
            }
            CanonicalizationErrorCode::ValueOutOfRange => {
                "usage observation exceeds the archive numeric range"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CanonicalizationError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct Canonicalizer;

impl Canonicalizer {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn canonicalize(
        &self,
        draft: &ObservationDraft,
    ) -> Result<CanonicalUsageEvent, CanonicalizationError> {
        if draft.session_ordinal() > i64::MAX as u64 || draft.source_offset() > i64::MAX as u64 {
            return Err(CanonicalizationError::new(
                CanonicalizationErrorCode::ValueOutOfRange,
            ));
        }
        if !usage_is_positive(draft.delta_usage()) {
            return Err(CanonicalizationError::new(
                CanonicalizationErrorCode::EmptyUsage,
            ));
        }
        if draft
            .cumulative_usage()
            .is_some_and(|cumulative| !cumulative_covers_delta(draft.delta_usage(), cumulative))
        {
            return Err(CanonicalizationError::new(
                CanonicalizationErrorCode::InconsistentCumulative,
            ));
        }

        let fingerprint = hash::event_fingerprint(draft);
        let signature = hash::replay_signature(draft);
        let evidence = match draft
            .cumulative_usage()
            .map(tokenmaster_domain::TokenUsage::total)
        {
            Some(TokenCount::Available(_)) => ReplayEvidence::StrongCumulative,
            Some(TokenCount::Unavailable) | None => ReplayEvidence::WeakUsageOnly,
        };
        Ok(CanonicalUsageEvent::from_draft(
            draft.clone(),
            fingerprint,
            signature,
            evidence,
        ))
    }
}

fn usage_is_positive(usage: &tokenmaster_domain::TokenUsage) -> bool {
    usage_counts(usage)
        .into_iter()
        .any(|count| matches!(count, TokenCount::Available(value) if value > 0))
}

fn cumulative_covers_delta(
    delta: &tokenmaster_domain::TokenUsage,
    cumulative: &tokenmaster_domain::TokenUsage,
) -> bool {
    usage_counts(delta)
        .into_iter()
        .zip(usage_counts(cumulative))
        .all(|(delta, cumulative)| match (delta, cumulative) {
            (TokenCount::Available(delta), TokenCount::Available(cumulative)) => {
                cumulative >= delta
            }
            _ => true,
        })
}

fn usage_counts(usage: &tokenmaster_domain::TokenUsage) -> [TokenCount; 5] {
    [
        usage.input(),
        usage.cached(),
        usage.output(),
        usage.reasoning(),
        usage.total(),
    ]
}
