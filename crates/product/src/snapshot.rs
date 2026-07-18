use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewSnapshot, GitEnvelope, GitOutputSnapshot,
    LatestActivityPage, ProductDataStatusEnvelope, QueryEnvelope, QuotaCurrentSnapshot,
    QuotaEnvelope, UsageAnalytics, UsageSessionDetailResult, UsageSessionPage,
};

use crate::{
    ProductReducerError, ProductRoute, ProductRouteStatus, ProductRuntimeStatus, ProductSection,
    route::{derive_routes, initial_routes},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductGeneration(u64);

impl ProductGeneration {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn checked_next(self) -> Result<Self, ProductReducerError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ProductReducerError::GenerationOverflow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductSnapshot {
    pub(crate) generation: ProductGeneration,
    pub(crate) data_status: ProductSection<ProductDataStatusEnvelope>,
    pub(crate) analytics: ProductSection<QueryEnvelope<UsageAnalytics>>,
    pub(crate) history: ProductSection<QueryEnvelope<UsageAnalytics>>,
    pub(crate) quota: ProductSection<QuotaEnvelope<QuotaCurrentSnapshot>>,
    pub(crate) benefit: ProductSection<BenefitOverviewEnvelope<BenefitOverviewSnapshot>>,
    pub(crate) git: ProductSection<GitEnvelope<GitOutputSnapshot>>,
    pub(crate) activity: ProductSection<QueryEnvelope<LatestActivityPage>>,
    pub(crate) sessions: ProductSection<QueryEnvelope<UsageSessionPage>>,
    pub(crate) session_detail: ProductSection<QueryEnvelope<UsageSessionDetailResult>>,
    pub(crate) runtime: ProductRuntimeStatus,
    pub(crate) routes: [ProductRouteStatus; 11],
}

impl ProductSnapshot {
    pub(crate) const fn initial() -> Self {
        Self {
            generation: ProductGeneration::INITIAL,
            data_status: ProductSection::waiting(),
            analytics: ProductSection::waiting(),
            history: ProductSection::waiting(),
            quota: ProductSection::waiting(),
            benefit: ProductSection::waiting(),
            git: ProductSection::waiting(),
            activity: ProductSection::waiting(),
            sessions: ProductSection::waiting(),
            session_detail: ProductSection::waiting(),
            runtime: ProductRuntimeStatus::waiting(),
            routes: initial_routes(),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> ProductGeneration {
        self.generation
    }

    #[must_use]
    pub const fn data_status(&self) -> &ProductSection<ProductDataStatusEnvelope> {
        &self.data_status
    }

    #[must_use]
    pub const fn analytics(&self) -> &ProductSection<QueryEnvelope<UsageAnalytics>> {
        &self.analytics
    }

    #[must_use]
    pub const fn history(&self) -> &ProductSection<QueryEnvelope<UsageAnalytics>> {
        &self.history
    }

    #[must_use]
    pub const fn quota(&self) -> &ProductSection<QuotaEnvelope<QuotaCurrentSnapshot>> {
        &self.quota
    }

    #[must_use]
    pub const fn benefit(
        &self,
    ) -> &ProductSection<BenefitOverviewEnvelope<BenefitOverviewSnapshot>> {
        &self.benefit
    }

    #[must_use]
    pub const fn git(&self) -> &ProductSection<GitEnvelope<GitOutputSnapshot>> {
        &self.git
    }

    #[must_use]
    pub const fn activity(&self) -> &ProductSection<QueryEnvelope<LatestActivityPage>> {
        &self.activity
    }

    #[must_use]
    pub const fn sessions(&self) -> &ProductSection<QueryEnvelope<UsageSessionPage>> {
        &self.sessions
    }

    #[must_use]
    pub const fn session_detail(&self) -> &ProductSection<QueryEnvelope<UsageSessionDetailResult>> {
        &self.session_detail
    }

    #[must_use]
    pub const fn runtime(&self) -> &ProductRuntimeStatus {
        &self.runtime
    }

    #[must_use]
    pub const fn route(&self, route: ProductRoute) -> ProductRouteStatus {
        self.routes[route.index()]
    }

    pub(crate) fn refresh_routes(&mut self) {
        self.routes = derive_routes(self);
    }
}
