use std::fmt;

use tokenmaster_product::{
    ProductGeneration, ProductRoute, ProductRouteState, ProductRouteStatus, ProductSnapshot,
};

use crate::DesktopDashboardProjection;

pub const DESKTOP_ROUTE_COUNT: usize = ProductRoute::ALL.len();
const MAX_ROUTE_REASONS: usize = 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DesktopRouteKey {
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

impl DesktopRouteKey {
    pub const ALL: [Self; DESKTOP_ROUTE_COUNT] = [
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

    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::History => "history",
            Self::Sessions => "sessions",
            Self::Models => "models",
            Self::Projects => "projects",
            Self::Activity => "activity",
            Self::DataHealth => "data_health",
            Self::Notifications => "notifications",
            Self::Settings => "settings",
            Self::HelpAbout => "help_about",
            Self::CompactWidget => "compact_widget",
        }
    }

    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Dashboard => "route.dashboard",
            Self::History => "route.history",
            Self::Sessions => "route.sessions",
            Self::Models => "route.models",
            Self::Projects => "route.projects",
            Self::Activity => "route.activity",
            Self::DataHealth => "route.data_health",
            Self::Notifications => "route.notifications",
            Self::Settings => "route.settings",
            Self::HelpAbout => "route.help_about",
            Self::CompactWidget => "route.compact_widget",
        }
    }

    #[must_use]
    pub const fn english_label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::History => "History",
            Self::Sessions => "Sessions",
            Self::Models => "Models",
            Self::Projects => "Projects",
            Self::Activity => "Activity",
            Self::DataHealth => "Data Health",
            Self::Notifications => "Notifications",
            Self::Settings => "Settings",
            Self::HelpAbout => "Help / About",
            Self::CompactWidget => "Compact Widget",
        }
    }

    #[must_use]
    pub const fn product_route(self) -> ProductRoute {
        ProductRoute::ALL[self as usize]
    }

    #[must_use]
    pub fn from_stable_key(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.stable_key() == value)
    }

    const fn from_product_route(route: ProductRoute) -> Self {
        Self::ALL[route as usize]
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopRouteState {
    Ready,
    Degraded,
    Unavailable,
}

impl DesktopRouteState {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }
}

impl From<ProductRouteState> for DesktopRouteState {
    fn from(value: ProductRouteState) -> Self {
        match value {
            ProductRouteState::Ready => Self::Ready,
            ProductRouteState::Degraded => Self::Degraded,
            ProductRouteState::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopReasonCodes {
    values: [Option<&'static str>; MAX_ROUTE_REASONS],
    len: u8,
}

impl DesktopReasonCodes {
    fn from_status(status: ProductRouteStatus) -> Self {
        let mut values = [None; MAX_ROUTE_REASONS];
        let mut len = 0_usize;
        for reason in status.reasons().iter() {
            values[len] = Some(reason.stable_code());
            len += 1;
        }
        Self {
            values,
            len: len as u8,
        }
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.len as usize
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.values[..self.len()].iter().filter_map(|value| *value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopRouteProjection {
    route: ProductRoute,
    key: DesktopRouteKey,
    state: DesktopRouteState,
    reason_codes: DesktopReasonCodes,
}

impl DesktopRouteProjection {
    fn from_status(status: ProductRouteStatus) -> Self {
        Self {
            route: status.route(),
            key: DesktopRouteKey::from_product_route(status.route()),
            state: DesktopRouteState::from(status.state()),
            reason_codes: DesktopReasonCodes::from_status(status),
        }
    }

    #[must_use]
    pub const fn route(self) -> ProductRoute {
        self.route
    }

    #[must_use]
    pub const fn key(self) -> DesktopRouteKey {
        self.key
    }

    #[must_use]
    pub const fn state(self) -> DesktopRouteState {
        self.state
    }

    #[must_use]
    pub const fn reason_codes(self) -> DesktopReasonCodes {
        self.reason_codes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProjection {
    generation: ProductGeneration,
    selected: DesktopRouteKey,
    routes: [DesktopRouteProjection; DESKTOP_ROUTE_COUNT],
    dashboard: DesktopDashboardProjection,
}

impl DesktopProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot, selected: DesktopRouteKey) -> Self {
        Self {
            generation: snapshot.generation(),
            selected,
            routes: std::array::from_fn(|index| {
                DesktopRouteProjection::from_status(snapshot.route(ProductRoute::ALL[index]))
            }),
            dashboard: DesktopDashboardProjection::from_snapshot(snapshot),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> ProductGeneration {
        self.generation
    }

    #[must_use]
    pub const fn selected(&self) -> DesktopRouteKey {
        self.selected
    }

    #[must_use]
    pub const fn routes(&self) -> &[DesktopRouteProjection; DESKTOP_ROUTE_COUNT] {
        &self.routes
    }

    #[must_use]
    pub const fn route(&self, key: DesktopRouteKey) -> DesktopRouteProjection {
        self.routes[key.index()]
    }

    #[must_use]
    pub const fn dashboard(&self) -> &DesktopDashboardProjection {
        &self.dashboard
    }

    pub fn select_stable_key(&mut self, value: &str) -> Result<(), DesktopSelectionError> {
        let selected = DesktopRouteKey::from_stable_key(value).ok_or(DesktopSelectionError)?;
        self.selected = selected;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopSelectionError;

impl fmt::Display for DesktopSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unknown desktop route")
    }
}

impl std::error::Error for DesktopSelectionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopApplyOutcome {
    Accepted,
    IgnoredNotNewer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopState {
    projection: DesktopProjection,
}

impl DesktopState {
    #[must_use]
    pub fn new(snapshot: &ProductSnapshot, selected: DesktopRouteKey) -> Self {
        Self {
            projection: DesktopProjection::from_snapshot(snapshot, selected),
        }
    }

    #[must_use]
    pub const fn projection(&self) -> &DesktopProjection {
        &self.projection
    }

    pub fn select_stable_key(&mut self, value: &str) -> Result<(), DesktopSelectionError> {
        self.projection.select_stable_key(value)
    }

    pub fn apply_snapshot(&mut self, snapshot: &ProductSnapshot) -> DesktopApplyOutcome {
        if snapshot.generation() <= self.projection.generation() {
            return DesktopApplyOutcome::IgnoredNotNewer;
        }

        let next = DesktopProjection::from_snapshot(snapshot, self.projection.selected());
        self.projection = next;
        DesktopApplyOutcome::Accepted
    }
}
