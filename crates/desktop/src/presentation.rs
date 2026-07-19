use std::{fmt, num::NonZeroU64};

use tokenmaster_product::{
    ProductGeneration, ProductRoute, ProductRouteState, ProductRouteStatus,
    ProductSessionDetailSelection, ProductSessionDetailSelectionGeneration, ProductSnapshot,
};

use crate::{
    DesktopDashboardProjection, DesktopHistoryProjection, DesktopModelsProjection,
    DesktopProjectsProjection, DesktopSessionDetailIntent, DesktopSessionsProjection,
};

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
    history: DesktopHistoryProjection,
    models: DesktopModelsProjection,
    projects: DesktopProjectsProjection,
    sessions: DesktopSessionsProjection,
}

impl DesktopProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot, selected: DesktopRouteKey) -> Self {
        Self::from_snapshot_with_selection(snapshot, selected, None)
    }

    fn from_snapshot_with_selection(
        snapshot: &ProductSnapshot,
        selected: DesktopRouteKey,
        active_session_detail: Option<DesktopSessionDetailIntent>,
    ) -> Self {
        Self {
            generation: snapshot.generation(),
            selected,
            routes: std::array::from_fn(|index| {
                DesktopRouteProjection::from_status(snapshot.route(ProductRoute::ALL[index]))
            }),
            dashboard: DesktopDashboardProjection::from_snapshot(snapshot),
            history: DesktopHistoryProjection::from_snapshot(snapshot),
            models: DesktopModelsProjection::from_snapshot(snapshot),
            projects: DesktopProjectsProjection::from_snapshot(snapshot),
            sessions: DesktopSessionsProjection::from_snapshot_with_selection(
                snapshot,
                active_session_detail,
            ),
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

    #[must_use]
    pub const fn history(&self) -> &DesktopHistoryProjection {
        &self.history
    }

    #[must_use]
    pub const fn models(&self) -> &DesktopModelsProjection {
        &self.models
    }

    #[must_use]
    pub const fn projects(&self) -> &DesktopProjectsProjection {
        &self.projects
    }

    #[must_use]
    pub const fn sessions(&self) -> &DesktopSessionsProjection {
        &self.sessions
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopSnapshotEpoch(NonZeroU64);

impl DesktopSnapshotEpoch {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopState {
    projection: DesktopProjection,
    snapshot_epoch: Option<DesktopSnapshotEpoch>,
    active_session_detail: Option<DesktopSessionDetailIntent>,
    next_session_selection_generation: u64,
}

impl DesktopState {
    #[must_use]
    pub fn new(snapshot: &ProductSnapshot, selected: DesktopRouteKey) -> Self {
        Self {
            projection: DesktopProjection::from_snapshot(snapshot, selected),
            snapshot_epoch: None,
            active_session_detail: None,
            next_session_selection_generation: 1,
        }
    }

    #[must_use]
    pub const fn projection(&self) -> &DesktopProjection {
        &self.projection
    }

    #[must_use]
    pub const fn snapshot_epoch(&self) -> Option<DesktopSnapshotEpoch> {
        self.snapshot_epoch
    }

    pub fn select_stable_key(&mut self, value: &str) -> Result<(), DesktopSelectionError> {
        self.projection.select_stable_key(value)
    }

    pub fn select_session_row(
        &mut self,
        row_ordinal: usize,
    ) -> Result<DesktopSessionDetailIntent, DesktopSessionSelectionError> {
        let epoch = self
            .snapshot_epoch
            .ok_or(DesktopSessionSelectionError::Unavailable)?;
        if row_ordinal >= self.projection.sessions().rows().len() {
            return Err(DesktopSessionSelectionError::OutOfRange);
        }
        let row_ordinal =
            u8::try_from(row_ordinal).map_err(|_| DesktopSessionSelectionError::OutOfRange)?;
        let generation =
            ProductSessionDetailSelectionGeneration::new(self.next_session_selection_generation)
                .ok_or(DesktopSessionSelectionError::CapacityExceeded)?;
        self.next_session_selection_generation = self
            .next_session_selection_generation
            .checked_add(1)
            .unwrap_or(0);
        let selection = ProductSessionDetailSelection::new(generation, row_ordinal);
        let intent =
            DesktopSessionDetailIntent::new(epoch, self.projection.generation(), selection);
        self.active_session_detail = Some(intent);
        self.projection.sessions.start_detail(row_ordinal);
        Ok(intent)
    }

    pub fn reject_session_detail(&mut self, intent: DesktopSessionDetailIntent) {
        if self.active_session_detail == Some(intent) {
            self.projection
                .sessions
                .reject_detail(intent.selection().row_ordinal());
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: &ProductSnapshot) -> DesktopApplyOutcome {
        if self.snapshot_epoch.is_some() || snapshot.generation() <= self.projection.generation() {
            return DesktopApplyOutcome::IgnoredNotNewer;
        }

        self.replace_projection(snapshot, false);
        DesktopApplyOutcome::Accepted
    }

    pub fn apply_snapshot_for_epoch(
        &mut self,
        epoch: DesktopSnapshotEpoch,
        snapshot: &ProductSnapshot,
    ) -> DesktopApplyOutcome {
        match self.snapshot_epoch {
            Some(current) if epoch < current => return DesktopApplyOutcome::IgnoredNotNewer,
            Some(current)
                if epoch == current && snapshot.generation() <= self.projection.generation() =>
            {
                return DesktopApplyOutcome::IgnoredNotNewer;
            }
            Some(_) | None => {}
        }

        let replace_backend = self.snapshot_epoch.is_some_and(|current| epoch > current);
        self.snapshot_epoch = Some(epoch);
        self.replace_projection(snapshot, replace_backend);
        DesktopApplyOutcome::Accepted
    }

    fn replace_projection(&mut self, snapshot: &ProductSnapshot, replace_backend: bool) {
        let active = if replace_backend {
            None
        } else {
            self.active_session_detail.filter(|active| {
                snapshot.session_detail_selection() == Some(active.selection())
                    || snapshot.generation() == active.product_generation()
            })
        };
        self.active_session_detail = active;
        self.projection = DesktopProjection::from_snapshot_with_selection(
            snapshot,
            self.projection.selected(),
            active,
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSessionSelectionError {
    Unavailable,
    OutOfRange,
    CapacityExceeded,
}

impl fmt::Display for DesktopSessionSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "session_selection_unavailable",
            Self::OutOfRange => "session_selection_out_of_range",
            Self::CapacityExceeded => "session_selection_capacity_exceeded",
        })
    }
}

impl std::error::Error for DesktopSessionSelectionError {}
