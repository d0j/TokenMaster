use std::{fmt, sync::Arc};

use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewSnapshot, GitEnvelope, GitOutputSnapshot,
    LatestActivityPage, ProductDataStatusEnvelope, QueryEnvelope, QueryErrorCode,
    QuotaCurrentSnapshot, QuotaEnvelope, UsageAnalytics, UsageSessionDetailResult,
    UsageSessionPage,
};
use tokenmaster_runtime::{
    BenefitReminderRuntimeSnapshot, CodexQuotaRuntimeSnapshot, GitRuntimeSnapshot,
    LiveRuntimeSnapshot, RuntimeErrorCode,
};

use crate::{
    ProductAttemptGeneration, ProductGitRuntimeHealth, ProductQuotaRuntimeHealth,
    ProductReminderRuntimeHealth, ProductRuntimeGeneration, ProductRuntimeObservationError,
    ProductRuntimeSection, ProductSection, ProductSessionDetailSelection, ProductSnapshot,
    ProductUsageRuntimeHealth,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductPublishOutcome {
    Accepted,
    Coalesced,
    RejectedOlder,
    RejectedIncompatible,
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
    ($publish:ident, $fail:ident, $field:ident, $value:ty, $compatible:expr) => {
        pub fn $publish(
            &mut self,
            attempt: ProductAttemptGeneration,
            value: $value,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            let outcome = classify(self.current.$field.attempt_generation(), attempt);
            if outcome != ProductPublishOutcome::Accepted {
                return Ok(outcome);
            }
            if !($compatible)(&self.current, &value) {
                return Ok(ProductPublishOutcome::RejectedIncompatible);
            }
            self.replace_section(ProductSection::ready(attempt, value), |next, section| {
                next.$field = section;
            })?;
            Ok(ProductPublishOutcome::Accepted)
        }

        pub fn $fail(
            &mut self,
            attempt: ProductAttemptGeneration,
            code: QueryErrorCode,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            let outcome = classify(self.current.$field.attempt_generation(), attempt);
            if outcome != ProductPublishOutcome::Accepted {
                return Ok(outcome);
            }
            let section =
                ProductSection::unavailable_retaining(attempt, code, &self.current.$field);
            self.replace_section(section, |next, section| {
                next.$field = section;
            })?;
            Ok(ProductPublishOutcome::Accepted)
        }
    };
}

macro_rules! runtime_methods {
    (
        $publish:ident,
        $publish_health:ident,
        $fail:ident,
        $fail_observation:ident,
        $field:ident,
        $source:ty,
        $health:ty
    ) => {
        pub fn $publish(
            &mut self,
            generation: ProductRuntimeGeneration,
            source: $source,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            self.$publish_health(generation, source.into())
        }

        pub fn $publish_health(
            &mut self,
            generation: ProductRuntimeGeneration,
            health: $health,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            let outcome = classify(self.current.runtime.$field.generation(), generation);
            if outcome != ProductPublishOutcome::Accepted {
                return Ok(outcome);
            }
            let section = ProductRuntimeSection::<$health>::ready(generation, health);
            self.replace_runtime_section(section, |next, section| {
                next.runtime.$field = section;
            })?;
            Ok(ProductPublishOutcome::Accepted)
        }

        pub fn $fail(
            &mut self,
            generation: ProductRuntimeGeneration,
            code: RuntimeErrorCode,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            self.$fail_observation(generation, ProductRuntimeObservationError::from(code))
        }

        pub fn $fail_observation(
            &mut self,
            generation: ProductRuntimeGeneration,
            error: ProductRuntimeObservationError,
        ) -> Result<ProductPublishOutcome, ProductReducerError> {
            let outcome = classify(self.current.runtime.$field.generation(), generation);
            if outcome != ProductPublishOutcome::Accepted {
                return Ok(outcome);
            }
            let section = ProductRuntimeSection::<$health>::unavailable_retaining(
                generation,
                error,
                self.current.runtime.$field,
            );
            self.replace_runtime_section(section, |next, section| {
                next.runtime.$field = section;
            })?;
            Ok(ProductPublishOutcome::Accepted)
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

    pub fn publish_data_status(
        &mut self,
        attempt: ProductAttemptGeneration,
        value: ProductDataStatusEnvelope,
    ) -> Result<ProductPublishOutcome, ProductReducerError> {
        let outcome = classify(self.current.data_status.attempt_generation(), attempt);
        if outcome != ProductPublishOutcome::Accepted {
            return Ok(outcome);
        }

        let mut next = (*self.current).clone();
        next.generation = next.generation.checked_next()?;
        next.data_status = ProductSection::ready(attempt, value);
        invalidate_incompatible_sections(&mut next, attempt);
        next.refresh_routes();
        self.current = Arc::new(next);
        Ok(ProductPublishOutcome::Accepted)
    }

    pub fn fail_data_status(
        &mut self,
        attempt: ProductAttemptGeneration,
        code: QueryErrorCode,
    ) -> Result<ProductPublishOutcome, ProductReducerError> {
        let outcome = classify(self.current.data_status.attempt_generation(), attempt);
        if outcome != ProductPublishOutcome::Accepted {
            return Ok(outcome);
        }
        let section =
            ProductSection::unavailable_retaining(attempt, code, &self.current.data_status);
        self.replace_section(section, |next, section| {
            next.data_status = section;
        })?;
        Ok(ProductPublishOutcome::Accepted)
    }

    section_methods!(
        publish_analytics,
        fail_analytics,
        analytics,
        QueryEnvelope<UsageAnalytics>,
        usage_compatible
    );
    section_methods!(
        publish_history,
        fail_history,
        history,
        QueryEnvelope<UsageAnalytics>,
        usage_compatible
    );

    runtime_methods!(
        publish_usage_runtime,
        publish_usage_runtime_health,
        fail_usage_runtime,
        fail_usage_runtime_observation,
        usage,
        LiveRuntimeSnapshot,
        ProductUsageRuntimeHealth
    );
    runtime_methods!(
        publish_quota_runtime,
        publish_quota_runtime_health,
        fail_quota_runtime,
        fail_quota_runtime_observation,
        quota,
        CodexQuotaRuntimeSnapshot,
        ProductQuotaRuntimeHealth
    );
    runtime_methods!(
        publish_reminder_runtime,
        publish_reminder_runtime_health,
        fail_reminder_runtime,
        fail_reminder_runtime_observation,
        reminder,
        BenefitReminderRuntimeSnapshot,
        ProductReminderRuntimeHealth
    );
    runtime_methods!(
        publish_git_runtime,
        publish_git_runtime_health,
        fail_git_runtime,
        fail_git_runtime_observation,
        git,
        GitRuntimeSnapshot,
        ProductGitRuntimeHealth
    );
    section_methods!(
        publish_quota,
        fail_quota,
        quota,
        QuotaEnvelope<QuotaCurrentSnapshot>,
        quota_compatible
    );
    section_methods!(
        publish_benefit,
        fail_benefit,
        benefit,
        BenefitOverviewEnvelope<BenefitOverviewSnapshot>,
        benefit_overview_compatible
    );
    section_methods!(
        publish_git,
        fail_git,
        git,
        GitEnvelope<GitOutputSnapshot>,
        git_compatible
    );
    section_methods!(
        publish_activity,
        fail_activity,
        activity,
        QueryEnvelope<LatestActivityPage>,
        usage_compatible
    );
    section_methods!(
        publish_sessions,
        fail_sessions,
        sessions,
        QueryEnvelope<UsageSessionPage>,
        usage_compatible
    );
    pub fn publish_session_detail(
        &mut self,
        attempt: ProductAttemptGeneration,
        selection: ProductSessionDetailSelection,
        value: QueryEnvelope<UsageSessionDetailResult>,
    ) -> Result<ProductPublishOutcome, ProductReducerError> {
        let outcome = classify_session_detail(&self.current, attempt, selection);
        if outcome != ProductPublishOutcome::Accepted {
            return Ok(outcome);
        }
        if !usage_compatible(&self.current, &value) {
            return Ok(ProductPublishOutcome::RejectedIncompatible);
        }
        self.replace_session_detail(selection, ProductSection::ready(attempt, value))?;
        Ok(ProductPublishOutcome::Accepted)
    }

    pub fn fail_session_detail(
        &mut self,
        attempt: ProductAttemptGeneration,
        selection: ProductSessionDetailSelection,
        code: QueryErrorCode,
    ) -> Result<ProductPublishOutcome, ProductReducerError> {
        let outcome = classify_session_detail(&self.current, attempt, selection);
        if outcome != ProductPublishOutcome::Accepted {
            return Ok(outcome);
        }
        self.replace_session_detail(selection, ProductSection::unavailable(attempt, code))?;
        Ok(ProductPublishOutcome::Accepted)
    }

    fn replace_section<T>(
        &mut self,
        section: ProductSection<T>,
        replace: impl FnOnce(&mut ProductSnapshot, ProductSection<T>),
    ) -> Result<(), ProductReducerError> {
        let mut next = (*self.current).clone();
        next.generation = next.generation.checked_next()?;
        replace(&mut next, section);
        next.refresh_routes();
        self.current = Arc::new(next);
        Ok(())
    }

    fn replace_runtime_section<T: Copy>(
        &mut self,
        section: ProductRuntimeSection<T>,
        replace: impl FnOnce(&mut ProductSnapshot, ProductRuntimeSection<T>),
    ) -> Result<(), ProductReducerError> {
        let mut next = (*self.current).clone();
        next.generation = next.generation.checked_next()?;
        replace(&mut next, section);
        next.refresh_routes();
        self.current = Arc::new(next);
        Ok(())
    }

    fn replace_session_detail(
        &mut self,
        selection: ProductSessionDetailSelection,
        section: ProductSection<QueryEnvelope<UsageSessionDetailResult>>,
    ) -> Result<(), ProductReducerError> {
        let mut next = (*self.current).clone();
        next.generation = next.generation.checked_next()?;
        next.session_detail_selection = Some(selection);
        next.session_detail = section;
        next.refresh_routes();
        self.current = Arc::new(next);
        Ok(())
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

fn classify<T: Copy + Ord>(current: Option<T>, candidate: T) -> ProductPublishOutcome {
    match current {
        Some(current) if candidate < current => ProductPublishOutcome::RejectedOlder,
        Some(current) if candidate == current => ProductPublishOutcome::Coalesced,
        _ => ProductPublishOutcome::Accepted,
    }
}

fn classify_session_detail(
    snapshot: &ProductSnapshot,
    attempt: ProductAttemptGeneration,
    selection: ProductSessionDetailSelection,
) -> ProductPublishOutcome {
    match snapshot.session_detail_selection {
        Some(current) if selection.generation() < current.generation() => {
            ProductPublishOutcome::RejectedOlder
        }
        Some(current)
            if selection.generation() == current.generation()
                && selection.row_ordinal() != current.row_ordinal() =>
        {
            ProductPublishOutcome::RejectedIncompatible
        }
        Some(current) if selection.generation() == current.generation() => {
            classify(snapshot.session_detail.attempt_generation(), attempt)
        }
        Some(_) | None => ProductPublishOutcome::Accepted,
    }
}

fn usage_compatible<T>(snapshot: &ProductSnapshot, value: &QueryEnvelope<T>) -> bool {
    snapshot.data_status.payload().is_none_or(|status| {
        value.header().dataset_identity() == status.payload().usage().dataset_identity()
    })
}

fn quota_compatible<T>(snapshot: &ProductSnapshot, value: &QuotaEnvelope<T>) -> bool {
    snapshot
        .data_status
        .payload()
        .is_none_or(|status| value.header().quota_revision() == status.payload().quota().revision())
}

fn benefit_overview_compatible<T>(
    snapshot: &ProductSnapshot,
    value: &BenefitOverviewEnvelope<T>,
) -> bool {
    snapshot.data_status.payload().is_none_or(|status| {
        value.header().benefit_revision() == status.payload().benefit().revision()
    })
}

fn git_compatible<T>(snapshot: &ProductSnapshot, value: &GitEnvelope<T>) -> bool {
    snapshot.data_status.payload().is_none_or(|status| {
        value.header().publication_revision() == status.payload().git().revision()
    })
}

fn invalidate_incompatible_sections(
    snapshot: &mut ProductSnapshot,
    status_attempt: ProductAttemptGeneration,
) {
    let Some(status) = snapshot.data_status.payload() else {
        return;
    };
    let usage_identity = status.payload().usage().dataset_identity();
    let quota_revision = status.payload().quota().revision();
    let benefit_revision = status.payload().benefit().revision();
    let git_revision = status.payload().git().revision();

    invalidate_usage(&mut snapshot.analytics, status_attempt, usage_identity);
    invalidate_usage(&mut snapshot.history, status_attempt, usage_identity);
    invalidate_usage(&mut snapshot.activity, status_attempt, usage_identity);
    invalidate_usage(&mut snapshot.sessions, status_attempt, usage_identity);
    invalidate_usage(&mut snapshot.session_detail, status_attempt, usage_identity);
    invalidate_if(&mut snapshot.quota, status_attempt, |value| {
        value.header().quota_revision() != quota_revision
    });
    invalidate_if(&mut snapshot.benefit, status_attempt, |value| {
        value.header().benefit_revision() != benefit_revision
    });
    invalidate_if(&mut snapshot.git, status_attempt, |value| {
        value.header().publication_revision() != git_revision
    });
}

fn invalidate_usage<T>(
    section: &mut ProductSection<QueryEnvelope<T>>,
    status_attempt: ProductAttemptGeneration,
    expected: tokenmaster_query::DatasetIdentity,
) {
    invalidate_if(section, status_attempt, |value| {
        value.header().dataset_identity() != expected
    });
}

fn invalidate_if<T>(
    section: &mut ProductSection<T>,
    status_attempt: ProductAttemptGeneration,
    incompatible: impl FnOnce(&T) -> bool,
) {
    let Some(payload) = section.payload() else {
        return;
    };
    if !incompatible(payload) {
        return;
    }
    let attempt = section
        .attempt_generation()
        .map_or(status_attempt, |current| current.max(status_attempt));
    *section = ProductSection::unavailable(attempt, QueryErrorCode::StaleSnapshot);
}
