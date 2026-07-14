#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum LayoutId {
    #[default]
    Refined,
    ControlCenter,
    Workbench,
}

impl LayoutId {
    pub const ALL: [Self; 3] = [Self::Refined, Self::ControlCenter, Self::Workbench];
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum ThemeId {
    #[default]
    Midnight,
    Graphite,
    Light,
}

impl ThemeId {
    pub const ALL: [Self; 3] = [Self::Midnight, Self::Graphite, Self::Light];
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum LocaleId {
    #[default]
    English,
    Russian,
    Pseudo,
}

impl LocaleId {
    pub const ALL: [Self; 3] = [Self::English, Self::Russian, Self::Pseudo];
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum RouteId {
    #[default]
    Dashboard,
    Sessions,
    Settings,
}
