use std::{
    fmt,
    sync::{Arc, Mutex},
};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokenmaster_product::ProductSnapshot;

use crate::{
    DesktopSnapshotBridge, DesktopSnapshotReceiver, MainWindow, RouteRow,
    presentation::{DesktopApplyOutcome, DesktopProjection, DesktopRouteKey, DesktopState},
};

pub struct DesktopShell {
    window: MainWindow,
    state: SharedDesktopState,
}

pub(crate) type SharedDesktopState = Arc<Mutex<DesktopState>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopUiErrorCode {
    StateUnavailable,
}

impl DesktopUiErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::StateUnavailable => "state_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopUiError {
    code: DesktopUiErrorCode,
}

impl DesktopUiError {
    const fn state_unavailable() -> Self {
        Self {
            code: DesktopUiErrorCode::StateUnavailable,
        }
    }

    #[must_use]
    pub const fn code(self) -> DesktopUiErrorCode {
        self.code
    }

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        self.code.stable_code()
    }
}

impl fmt::Display for DesktopUiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

impl std::error::Error for DesktopUiError {}

impl DesktopShell {
    pub fn new(snapshot: &ProductSnapshot) -> Result<Self, slint::PlatformError> {
        let window = MainWindow::new()?;
        let initial_state = DesktopState::new(snapshot, DesktopRouteKey::Dashboard);
        apply_projection(&window, initial_state.projection());
        let state = Arc::new(Mutex::new(initial_state));
        wire_route_selection(&window, state.clone());
        Ok(Self { window, state })
    }

    #[must_use]
    pub const fn window(&self) -> &MainWindow {
        &self.window
    }

    pub fn apply_snapshot(
        &self,
        snapshot: &ProductSnapshot,
    ) -> Result<DesktopApplyOutcome, DesktopUiError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| DesktopUiError::state_unavailable())?;
        let outcome = state.apply_snapshot(snapshot);
        if outcome == DesktopApplyOutcome::Accepted {
            apply_projection(&self.window, state.projection());
        }
        Ok(outcome)
    }

    pub(crate) fn state_handle(&self) -> SharedDesktopState {
        self.state.clone()
    }

    #[must_use]
    pub fn snapshot_bridge(&self, receiver: DesktopSnapshotReceiver) -> DesktopSnapshotBridge {
        DesktopSnapshotBridge::new(self.window.as_weak(), self.state_handle(), receiver)
    }
}

fn wire_route_selection(window: &MainWindow, state: SharedDesktopState) {
    let weak = window.as_weak();
    window.on_select_route(move |key| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let Ok(mut state) = state.lock() else {
            return;
        };
        if state.select_stable_key(key.as_str()).is_ok() {
            apply_projection(&window, state.projection());
        }
    });
}

pub(crate) fn apply_projection(window: &MainWindow, projection: &DesktopProjection) {
    let rows = projection
        .routes()
        .iter()
        .map(|route| RouteRow {
            key: SharedString::from(route.key().stable_key()),
            label_key: SharedString::from(route.key().label_key()),
            label: SharedString::from(route.key().english_label()),
            state: SharedString::from(route.state().stable_code()),
            reasons: SharedString::from(join_reasons(route.reason_codes().iter())),
            selected: route.key() == projection.selected(),
        })
        .collect::<Vec<_>>();
    let active = projection.route(projection.selected());

    window.set_route_rows(ModelRc::new(VecModel::from(rows)));
    window.set_active_route_key(SharedString::from(projection.selected().stable_key()));
    window.set_active_route_label(SharedString::from(projection.selected().english_label()));
    window.set_active_route_state(SharedString::from(active.state().stable_code()));
    window.set_active_route_reasons(SharedString::from(join_reasons(
        active.reason_codes().iter(),
    )));
    window.set_product_generation(SharedString::from(
        projection.generation().get().to_string(),
    ));
}

fn join_reasons(reasons: impl Iterator<Item = &'static str>) -> String {
    reasons.collect::<Vec<_>>().join(", ")
}
