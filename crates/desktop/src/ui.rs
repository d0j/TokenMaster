use std::{cell::RefCell, rc::Rc};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokenmaster_product::ProductSnapshot;

use crate::{
    MainWindow, RouteRow,
    presentation::{DesktopApplyOutcome, DesktopProjection, DesktopRouteKey, DesktopState},
};

pub struct DesktopShell {
    window: MainWindow,
    state: Rc<RefCell<DesktopState>>,
}

impl DesktopShell {
    pub fn new(snapshot: &ProductSnapshot) -> Result<Self, slint::PlatformError> {
        let window = MainWindow::new()?;
        let state = Rc::new(RefCell::new(DesktopState::new(
            snapshot,
            DesktopRouteKey::Dashboard,
        )));
        apply_projection(&window, state.borrow().projection());
        wire_route_selection(&window, Rc::clone(&state));
        Ok(Self { window, state })
    }

    #[must_use]
    pub const fn window(&self) -> &MainWindow {
        &self.window
    }

    pub fn apply_snapshot(&self, snapshot: &ProductSnapshot) -> DesktopApplyOutcome {
        let outcome = self.state.borrow_mut().apply_snapshot(snapshot);
        if outcome == DesktopApplyOutcome::Accepted {
            apply_projection(&self.window, self.state.borrow().projection());
        }
        outcome
    }
}

fn wire_route_selection(window: &MainWindow, state: Rc<RefCell<DesktopState>>) {
    let weak = window.as_weak();
    window.on_select_route(move |key| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        if state.select_stable_key(key.as_str()).is_ok() {
            apply_projection(&window, state.projection());
        }
    });
}

fn apply_projection(window: &MainWindow, projection: &DesktopProjection) {
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
