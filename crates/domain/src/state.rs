use crate::{LayoutId, LocaleId, RouteId, ThemeId};

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct AppState {
    layout: LayoutId,
    theme: ThemeId,
    locale: LocaleId,
    route: RouteId,
    selected_session: Option<i64>,
    revision: u64,
}

impl AppState {
    pub fn switch_layout(&mut self, layout: LayoutId) {
        if self.layout != layout {
            self.layout = layout;
            self.revision += 1;
        }
    }

    pub fn switch_theme(&mut self, theme: ThemeId) {
        if self.theme != theme {
            self.theme = theme;
            self.revision += 1;
        }
    }

    pub fn switch_locale(&mut self, locale: LocaleId) {
        if self.locale != locale {
            self.locale = locale;
            self.revision += 1;
        }
    }

    pub fn navigate(&mut self, route: RouteId) {
        if self.route != route {
            self.route = route;
            self.revision += 1;
        }
    }

    pub fn select_session(&mut self, session: Option<i64>) {
        if self.selected_session != session {
            self.selected_session = session;
            self.revision += 1;
        }
    }

    pub fn route(&self) -> RouteId {
        self.route
    }

    pub fn selected_session(&self) -> Option<i64> {
        self.selected_session
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}
