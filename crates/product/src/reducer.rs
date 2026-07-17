use std::{fmt, sync::Arc};

use tokenmaster_query::{
    BenefitCurrentSnapshot, BenefitEnvelope, GitEnvelope, GitOutputSnapshot, LatestActivityPage,
    ProductDataStatusEnvelope, QueryEnvelope, QueryErrorCode, QuotaCurrentSnapshot, QuotaEnvelope,
    SnapshotGeneration, UsageAnalytics, UsageSessionDetailResult, UsageSessionPage,
};

use crate::{ProductSection, ProductSnapshot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductPublishOutcome {
    Accepted,
    Coalesced,
    RejectedOlder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductReducerError {
    GenerationOverflow,
}

impl fmt::Display for ProductReducerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("product_generation_overflow")
    }
}

impl std::error::Error for ProductReducerError {}

pub struct ProductReducer {
    current: Arc<ProductSnapshot>,
}

macro_rules! section_methods {
    ($publish:ident, $fail:ident, $field:ident, $value:ty, $generation:expr) => {
        pub fn $publish(
            &mut self,
            value: $value,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            let generation = ($generation)(&value);
            let current = self.current.$field.attempt_generation();
            self.apply(
                current,
                generation,
                ProductSection::ready(generation, value),
                |next, section| {
                    next.$field = section;
                },
            )
        }

        pub fn $fail(
            &mut self,
            generation: SnapshotGeneration,
            code: QueryErrorCode,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            let current = self.current.$field.attempt_generation();
            self.apply(
                current,
                generation,
                ProductSection::unavailable(generation, code),
                |next, section| {
                    next.$field = section;
                },
            )
        }
    };
}

impl ProductReducer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Arc::new(ProductSnapshot::initial()),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> Arc<ProductSnapshot> {
        Arc::clone(&self.current)
    }

    section_methods!(
        publish_data_status,
        fail_data_status,
        data_status,
        ProductDataStatusEnvelope,
        |value: &ProductDataStatusEnvelope| value.snapshot_generation()
    );
    section_methods!(
        publish_analytics,
        fail_analytics,
        analytics,
        QueryEnvelope<UsageAnalytics>,
        |value: &QueryEnvelope<UsageAnalytics>| value.header().snapshot_generation()
    );
    section_methods!(
        publish_quota,
        fail_quota,
        quota,
        QuotaEnvelope<QuotaCurrentSnapshot>,
        |value: &QuotaEnvelope<QuotaCurrentSnapshot>| value.header().snapshot_generation()
    );
    section_methods!(
        publish_benefit,
        fail_benefit,
        benefit,
        BenefitEnvelope<BenefitCurrentSnapshot>,
        |value: &BenefitEnvelope<BenefitCurrentSnapshot>| value.header().snapshot_generation()
    );
    section_methods!(
        publish_git,
        fail_git,
        git,
        GitEnvelope<GitOutputSnapshot>,
        |value: &GitEnvelope<GitOutputSnapshot>| value.header().snapshot_generation()
    );
    section_methods!(
        publish_activity,
        fail_activity,
        activity,
        QueryEnvelope<LatestActivityPage>,
        |value: &QueryEnvelope<LatestActivityPage>| value.header().snapshot_generation()
    );
    section_methods!(
        publish_sessions,
        fail_sessions,
        sessions,
        QueryEnvelope<UsageSessionPage>,
        |value: &QueryEnvelope<UsageSessionPage>| value.header().snapshot_generation()
    );
    section_methods!(
        publish_session_detail,
        fail_session_detail,
        session_detail,
        QueryEnvelope<UsageSessionDetailResult>,
        |value: &QueryEnvelope<UsageSessionDetailResult>| value.header().snapshot_generation()
    );

    fn apply<T>(
        &mut self,
        current: Option<SnapshotGeneration>,
        candidate: SnapshotGeneration,
        section: ProductSection<T>,
        replace: impl FnOnce(&mut ProductSnapshot, ProductSection<T>),
    ) -> Result<ProductPublishOutcome, ProductReducerError> {
        let outcome = classify(current, candidate);
        if outcome != ProductPublishOutcome::Accepted {
            return Ok(outcome);
        }
        let mut next = (*self.current).clone();
        next.generation = next.generation.checked_next()?;
        replace(&mut next, section);
        self.current = Arc::new(next);
        Ok(ProductPublishOutcome::Accepted)
    }
}

impl Default for ProductReducer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProductReducer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductReducer")
            .field("generation", &self.current.generation())
            .finish_non_exhaustive()
    }
}

fn classify(
    current: Option<SnapshotGeneration>,
    candidate: SnapshotGeneration,
) -> ProductPublishOutcome {
    match current {
        Some(current) if candidate < current => ProductPublishOutcome::RejectedOlder,
        Some(current) if candidate == current => ProductPublishOutcome::Coalesced,
        _ => ProductPublishOutcome::Accepted,
    }
}
