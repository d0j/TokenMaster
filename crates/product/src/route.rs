use tokenmaster_query::{ProductAggregateState, ProductComponentState};

use crate::{ProductSectionKind, ProductSnapshot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductRoute {
    Dashboard,
    History,
    Sessions,
    Models,
    Projects,
    Activity,
    DataHealth,
    Notifications,
    Settings,
    HelpAbout,
    CompactWidget,
}

impl ProductRoute {
    pub const ALL: [Self; 11] = [
        Self::Dashboard,
        Self::History,
        Self::Sessions,
        Self::Models,
        Self::Projects,
        Self::Activity,
        Self::DataHealth,
        Self::Notifications,
        Self::Settings,
        Self::HelpAbout,
        Self::CompactWidget,
    ];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductRouteState {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductRouteReason {
    DataStatusUnavailable,
    UsageUnavailable,
    AggregateRebuildRequired,
    AggregateRebuilding,
    AggregateFailed,
    QuotaUnavailable,
    BenefitUnavailable,
    GitUnavailable,
    ActivityUnavailable,
    SessionsUnavailable,
    SessionDetailUnavailable,
}

impl ProductRouteReason {
    const ALL: [Self; 11] = [
        Self::DataStatusUnavailable,
        Self::UsageUnavailable,
        Self::AggregateRebuildRequired,
        Self::AggregateRebuilding,
        Self::AggregateFailed,
        Self::QuotaUnavailable,
        Self::BenefitUnavailable,
        Self::GitUnavailable,
        Self::ActivityUnavailable,
        Self::SessionsUnavailable,
        Self::SessionDetailUnavailable,
    ];

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::DataStatusUnavailable => "data_status_unavailable",
            Self::UsageUnavailable => "usage_unavailable",
            Self::AggregateRebuildRequired => "aggregate_rebuild_required",
            Self::AggregateRebuilding => "aggregate_rebuilding",
            Self::AggregateFailed => "aggregate_failed",
            Self::QuotaUnavailable => "quota_unavailable",
            Self::BenefitUnavailable => "benefit_unavailable",
            Self::GitUnavailable => "git_unavailable",
            Self::ActivityUnavailable => "activity_unavailable",
            Self::SessionsUnavailable => "sessions_unavailable",
            Self::SessionDetailUnavailable => "session_detail_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductRouteReasons(u16);

impl ProductRouteReasons {
    const fn empty() -> Self {
        Self(0)
    }

    const fn with(mut self, reason: ProductRouteReason) -> Self {
        self.0 |= 1_u16 << reason as u8;
        self
    }

    #[must_use]
    pub const fn contains(self, reason: ProductRouteReason) -> bool {
        self.0 & (1_u16 << reason as u8) != 0
    }

    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn iter(self) -> ProductRouteReasonIter {
        ProductRouteReasonIter {
            reasons: self,
            index: 0,
        }
    }
}

pub struct ProductRouteReasonIter {
    reasons: ProductRouteReasons,
    index: usize,
}

impl Iterator for ProductRouteReasonIter {
    type Item = ProductRouteReason;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < ProductRouteReason::ALL.len() {
            let reason = ProductRouteReason::ALL[self.index];
            self.index += 1;
            if self.reasons.contains(reason) {
                return Some(reason);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductRouteStatus {
    route: ProductRoute,
    state: ProductRouteState,
    reasons: ProductRouteReasons,
}

impl ProductRouteStatus {
    const fn new(
        route: ProductRoute,
        state: ProductRouteState,
        reasons: ProductRouteReasons,
    ) -> Self {
        Self {
            route,
            state,
            reasons,
        }
    }

    #[must_use]
    pub const fn route(self) -> ProductRoute {
        self.route
    }

    #[must_use]
    pub const fn state(self) -> ProductRouteState {
        self.state
    }

    #[must_use]
    pub const fn reasons(self) -> ProductRouteReasons {
        self.reasons
    }
}

pub(crate) fn derive_routes(snapshot: &ProductSnapshot) -> [ProductRouteStatus; 11] {
    let Some(status) = snapshot.data_status.payload() else {
        return initial_routes();
    };

    let mut common = ProductRouteReasons::empty();
    if snapshot.data_status.kind() != ProductSectionKind::Ready {
        common = common.with(ProductRouteReason::DataStatusUnavailable);
    }
    let aggregate_reason = match status.payload().usage().aggregate().state() {
        ProductAggregateState::Ready => None,
        ProductAggregateState::RebuildRequired => {
            Some(ProductRouteReason::AggregateRebuildRequired)
        }
        ProductAggregateState::Rebuilding => Some(ProductRouteReason::AggregateRebuilding),
        ProductAggregateState::Failed => Some(ProductRouteReason::AggregateFailed),
    };
    let usage_ready = snapshot.analytics.kind() == ProductSectionKind::Ready;
    let activity_ready = snapshot.activity.kind() == ProductSectionKind::Ready;
    let sessions_ready = snapshot.sessions.kind() == ProductSectionKind::Ready;
    let quota_ready = status.payload().quota().state() == ProductComponentState::Published
        && snapshot.quota.kind() == ProductSectionKind::Ready;
    let benefit_ready = status.payload().benefit().state() == ProductComponentState::Published
        && snapshot.benefit.kind() == ProductSectionKind::Ready;
    let git_ready = status.payload().git().state() == ProductComponentState::Published
        && snapshot.git.kind() == ProductSectionKind::Ready;

    let mut dashboard = common;
    if !usage_ready {
        dashboard = dashboard.with(ProductRouteReason::UsageUnavailable);
    }
    if let Some(reason) = aggregate_reason {
        dashboard = dashboard.with(reason);
    }
    if !quota_ready {
        dashboard = dashboard.with(ProductRouteReason::QuotaUnavailable);
    }
    if !benefit_ready {
        dashboard = dashboard.with(ProductRouteReason::BenefitUnavailable);
    }
    if !git_ready {
        dashboard = dashboard.with(ProductRouteReason::GitUnavailable);
    }

    let mut usage = common;
    if !usage_ready {
        usage = usage.with(ProductRouteReason::UsageUnavailable);
    }
    if let Some(reason) = aggregate_reason {
        usage = usage.with(reason);
    }

    let mut sessions = common;
    if !sessions_ready {
        sessions = sessions.with(ProductRouteReason::SessionsUnavailable);
    }
    if let Some(reason) = aggregate_reason {
        sessions = sessions.with(reason);
    }

    let mut projects = usage;
    if !git_ready {
        projects = projects.with(ProductRouteReason::GitUnavailable);
    }

    let mut activity = common;
    if !activity_ready {
        activity = activity.with(ProductRouteReason::ActivityUnavailable);
    }

    let mut notifications = common;
    if !benefit_ready {
        notifications = notifications.with(ProductRouteReason::BenefitUnavailable);
    }

    let mut compact = common;
    if !quota_ready {
        compact = compact.with(ProductRouteReason::QuotaUnavailable);
    }

    [
        status_for(ProductRoute::Dashboard, dashboard),
        aggregate_status_for(ProductRoute::History, usage),
        aggregate_status_for(ProductRoute::Sessions, sessions),
        aggregate_status_for(ProductRoute::Models, usage),
        aggregate_status_for(ProductRoute::Projects, projects),
        status_for(ProductRoute::Activity, activity),
        status_for(ProductRoute::DataHealth, common),
        status_for(ProductRoute::Notifications, notifications),
        ProductRouteStatus::new(
            ProductRoute::Settings,
            ProductRouteState::Ready,
            ProductRouteReasons::empty(),
        ),
        ProductRouteStatus::new(
            ProductRoute::HelpAbout,
            ProductRouteState::Ready,
            ProductRouteReasons::empty(),
        ),
        status_for(ProductRoute::CompactWidget, compact),
    ]
}

pub(crate) const fn initial_routes() -> [ProductRouteStatus; 11] {
    let unavailable = ProductRouteReasons::empty().with(ProductRouteReason::DataStatusUnavailable);
    [
        ProductRouteStatus::new(
            ProductRoute::Dashboard,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::History,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::Sessions,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::Models,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::Projects,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::Activity,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::DataHealth,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::Notifications,
            ProductRouteState::Unavailable,
            unavailable,
        ),
        ProductRouteStatus::new(
            ProductRoute::Settings,
            ProductRouteState::Ready,
            ProductRouteReasons::empty(),
        ),
        ProductRouteStatus::new(
            ProductRoute::HelpAbout,
            ProductRouteState::Ready,
            ProductRouteReasons::empty(),
        ),
        ProductRouteStatus::new(
            ProductRoute::CompactWidget,
            ProductRouteState::Unavailable,
            unavailable,
        ),
    ]
}

const fn status_for(route: ProductRoute, reasons: ProductRouteReasons) -> ProductRouteStatus {
    let state = if reasons.is_empty() {
        ProductRouteState::Ready
    } else {
        ProductRouteState::Degraded
    };
    ProductRouteStatus::new(route, state, reasons)
}

const fn aggregate_status_for(
    route: ProductRoute,
    reasons: ProductRouteReasons,
) -> ProductRouteStatus {
    let aggregate_unavailable = reasons.contains(ProductRouteReason::AggregateRebuildRequired)
        || reasons.contains(ProductRouteReason::AggregateRebuilding)
        || reasons.contains(ProductRouteReason::AggregateFailed);
    let state = if aggregate_unavailable {
        ProductRouteState::Unavailable
    } else if reasons.is_empty() {
        ProductRouteState::Ready
    } else {
        ProductRouteState::Degraded
    };
    ProductRouteStatus::new(route, state, reasons)
}
