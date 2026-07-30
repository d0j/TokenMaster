use crate::DesktopSkin;

pub const DESKTOP_BOARD_SECTION_COUNT: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DesktopBoardSectionKey {
    PlanUsage,
    CodeOutput,
    Trend,
    Sessions,
    Activity,
    Models,
}

impl DesktopBoardSectionKey {
    pub const ALL: [Self; DESKTOP_BOARD_SECTION_COUNT] = [
        Self::PlanUsage,
        Self::CodeOutput,
        Self::Trend,
        Self::Sessions,
        Self::Activity,
        Self::Models,
    ];

    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::PlanUsage => "plan_usage",
            Self::CodeOutput => "code_output",
            Self::Trend => "trend",
            Self::Sessions => "sessions",
            Self::Activity => "activity",
            Self::Models => "models",
        }
    }

    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::PlanUsage => "dashboard.plan_usage",
            Self::CodeOutput => "dashboard.code_output",
            Self::Trend => "dashboard.trend",
            Self::Sessions => "dashboard.sessions",
            Self::Activity => "dashboard.activity",
            Self::Models => "dashboard.models",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopBoardSectionPreference {
    key: DesktopBoardSectionKey,
    visible: bool,
    collapsed: bool,
}

impl DesktopBoardSectionPreference {
    #[must_use]
    pub const fn new(key: DesktopBoardSectionKey, visible: bool, collapsed: bool) -> Self {
        Self {
            key,
            visible,
            collapsed,
        }
    }

    #[must_use]
    pub const fn key(self) -> DesktopBoardSectionKey {
        self.key
    }

    #[must_use]
    pub const fn visible(self) -> bool {
        self.visible
    }

    #[must_use]
    pub const fn collapsed(self) -> bool {
        self.collapsed
    }

    const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    const fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopBoardPreferences {
    rows: [DesktopBoardSectionPreference; DESKTOP_BOARD_SECTION_COUNT],
}

impl DesktopBoardPreferences {
    #[must_use]
    pub fn new(rows: [DesktopBoardSectionPreference; DESKTOP_BOARD_SECTION_COUNT]) -> Option<Self> {
        let mut seen = [false; DESKTOP_BOARD_SECTION_COUNT];
        let mut visible = false;
        for row in rows {
            let index = row.key().index();
            if seen[index] {
                return None;
            }
            seen[index] = true;
            visible |= row.visible();
        }
        if !seen.into_iter().all(|present| present) || !visible {
            return None;
        }
        Some(Self { rows })
    }

    #[must_use]
    pub const fn canonical() -> Self {
        Self {
            rows: [
                DesktopBoardSectionPreference::new(DesktopBoardSectionKey::PlanUsage, true, false),
                DesktopBoardSectionPreference::new(DesktopBoardSectionKey::CodeOutput, true, false),
                DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Trend, true, false),
                DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Sessions, true, false),
                DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Activity, true, false),
                DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Models, true, false),
            ],
        }
    }

    #[must_use]
    pub const fn rows(&self) -> &[DesktopBoardSectionPreference; DESKTOP_BOARD_SECTION_COUNT] {
        &self.rows
    }

    fn move_adjacent(self, index: usize, delta: i32) -> Option<Self> {
        if !matches!(delta, -1 | 1) {
            return None;
        }
        let target = if delta < 0 {
            index.checked_sub(1)?
        } else {
            index.checked_add(1)?
        };
        if index >= DESKTOP_BOARD_SECTION_COUNT || target >= DESKTOP_BOARD_SECTION_COUNT {
            return None;
        }
        let mut rows = self.rows;
        rows.swap(index, target);
        Some(Self { rows })
    }

    fn with_visibility(self, index: usize, visible: bool) -> Option<Self> {
        let mut rows = self.rows;
        let current = *rows.get(index)?;
        if !visible && current.visible() && rows.iter().filter(|row| row.visible()).count() == 1 {
            return None;
        }
        rows[index] = current.with_visible(visible);
        Some(Self { rows })
    }

    fn with_collapsed(self, index: usize, collapsed: bool) -> Option<Self> {
        let mut rows = self.rows;
        let row = rows.get_mut(index)?;
        *row = row.with_collapsed(collapsed);
        Some(Self { rows })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopDensity {
    Comfortable,
    Compact,
    UltraCompact,
}

impl DesktopDensity {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
            Self::UltraCompact => "ultra_compact",
        }
    }

    #[must_use]
    pub const fn slint_index(self) -> i32 {
        match self {
            Self::Comfortable => 0,
            Self::Compact => 1,
            Self::UltraCompact => 2,
        }
    }

    const fn from_slint_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Comfortable),
            1 => Some(Self::Compact),
            2 => Some(Self::UltraCompact),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopLocale {
    English,
    Russian,
    Pseudo,
}

impl DesktopLocale {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
            Self::Pseudo => "pseudo",
        }
    }

    #[must_use]
    pub const fn slint_index(self) -> i32 {
        match self {
            Self::English => 0,
            Self::Russian => 1,
            Self::Pseudo => 2,
        }
    }

    #[must_use]
    pub const fn from_slint_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::English),
            1 => Some(Self::Russian),
            2 => Some(Self::Pseudo),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopLayout {
    Refined,
    ControlCenter,
    Workbench,
}

impl DesktopLayout {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Refined => "refined",
            Self::ControlCenter => "control_center",
            Self::Workbench => "workbench",
        }
    }

    #[must_use]
    pub const fn slint_index(self) -> i32 {
        match self {
            Self::Refined => 0,
            Self::ControlCenter => 1,
            Self::Workbench => 2,
        }
    }

    const fn from_slint_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Refined),
            1 => Some(Self::ControlCenter),
            2 => Some(Self::Workbench),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopEffectiveColorScheme {
    Light,
    Dark,
}

impl DesktopEffectiveColorScheme {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub const fn slint_index(self) -> i32 {
        match self {
            Self::Light => 1,
            Self::Dark => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSystemColorScheme {
    Unknown,
    Light,
    Dark,
}

impl DesktopSystemColorScheme {
    #[must_use]
    pub const fn from_slint_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Unknown),
            1 => Some(Self::Light),
            2 => Some(Self::Dark),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopColorScheme {
    System,
    Light,
    Dark,
}

impl DesktopColorScheme {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub const fn slint_index(self) -> i32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    #[must_use]
    pub const fn from_slint_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::System),
            1 => Some(Self::Light),
            2 => Some(Self::Dark),
            _ => None,
        }
    }

    #[must_use]
    pub const fn resolve(
        self,
        system_color_scheme: DesktopSystemColorScheme,
    ) -> DesktopEffectiveColorScheme {
        match (self, system_color_scheme) {
            (Self::Light, _) | (Self::System, DesktopSystemColorScheme::Light) => {
                DesktopEffectiveColorScheme::Light
            }
            (Self::Dark, _)
            | (Self::System, DesktopSystemColorScheme::Dark | DesktopSystemColorScheme::Unknown) => {
                DesktopEffectiveColorScheme::Dark
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopPresentationSelection {
    density: DesktopDensity,
    skin: DesktopSkin,
    color_scheme: DesktopColorScheme,
    layout: DesktopLayout,
    locale: DesktopLocale,
    board: DesktopBoardPreferences,
}

impl DesktopPresentationSelection {
    #[must_use]
    pub const fn new(
        density: DesktopDensity,
        skin: DesktopSkin,
        color_scheme: DesktopColorScheme,
        layout: DesktopLayout,
        locale: DesktopLocale,
    ) -> Self {
        Self {
            density,
            skin,
            color_scheme,
            layout,
            locale,
            board: DesktopBoardPreferences::canonical(),
        }
    }

    #[must_use]
    pub const fn density(self) -> DesktopDensity {
        self.density
    }

    #[must_use]
    pub const fn skin(self) -> DesktopSkin {
        self.skin
    }

    #[must_use]
    pub const fn color_scheme(self) -> DesktopColorScheme {
        self.color_scheme
    }

    #[must_use]
    pub const fn layout(self) -> DesktopLayout {
        self.layout
    }

    #[must_use]
    pub const fn locale(self) -> DesktopLocale {
        self.locale
    }

    #[must_use]
    pub const fn board(self) -> DesktopBoardPreferences {
        self.board
    }

    #[must_use]
    pub const fn with_board(mut self, board: DesktopBoardPreferences) -> Self {
        self.board = board;
        self
    }

    const fn with_density(self, density: DesktopDensity) -> Self {
        Self::new(
            density,
            self.skin,
            self.color_scheme,
            self.layout,
            self.locale,
        )
        .with_board(self.board)
    }

    const fn with_skin(self, skin: DesktopSkin) -> Self {
        Self::new(
            self.density,
            skin,
            self.color_scheme,
            self.layout,
            self.locale,
        )
        .with_board(self.board)
    }

    const fn with_color_scheme(self, color_scheme: DesktopColorScheme) -> Self {
        Self::new(
            self.density,
            self.skin,
            color_scheme,
            self.layout,
            self.locale,
        )
        .with_board(self.board)
    }

    const fn with_layout(self, layout: DesktopLayout) -> Self {
        Self::new(
            self.density,
            self.skin,
            self.color_scheme,
            layout,
            self.locale,
        )
        .with_board(self.board)
    }

    const fn with_locale(self, locale: DesktopLocale) -> Self {
        Self::new(
            self.density,
            self.skin,
            self.color_scheme,
            self.layout,
            locale,
        )
        .with_board(self.board)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopPresentationRevision(u64);

impl DesktopPresentationRevision {
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    const fn checked_successor(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopPresentationApplyOutcome {
    Applied,
    Unchanged,
    Rejected,
    RevisionExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopPresentationPersistence {
    Saved,
    Saving,
    NotSaved,
}

impl DesktopPresentationPersistence {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Saving => "saving",
            Self::NotSaved => "not_saved",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopPresentationStyle {
    selection: DesktopPresentationSelection,
    persisted_selection: DesktopPresentationSelection,
    revision: DesktopPresentationRevision,
    persistence: DesktopPresentationPersistence,
    system_color_scheme: DesktopSystemColorScheme,
}

impl DesktopPresentationStyle {
    #[must_use]
    pub const fn from_persisted(selection: DesktopPresentationSelection) -> Self {
        Self {
            selection,
            persisted_selection: selection,
            revision: DesktopPresentationRevision::initial(),
            persistence: DesktopPresentationPersistence::Saved,
            system_color_scheme: DesktopSystemColorScheme::Unknown,
        }
    }

    #[must_use]
    pub const fn selection(self) -> DesktopPresentationSelection {
        self.selection
    }

    #[must_use]
    pub const fn persisted_selection(self) -> DesktopPresentationSelection {
        self.persisted_selection
    }

    #[must_use]
    pub const fn density(self) -> DesktopDensity {
        self.selection.density()
    }

    #[must_use]
    pub const fn skin(self) -> DesktopSkin {
        self.selection.skin()
    }

    #[must_use]
    pub const fn color_scheme(self) -> DesktopColorScheme {
        self.selection.color_scheme()
    }

    #[must_use]
    pub const fn layout(self) -> DesktopLayout {
        self.selection.layout()
    }

    #[must_use]
    pub const fn locale(self) -> DesktopLocale {
        self.selection.locale()
    }

    #[must_use]
    pub const fn effective_color_scheme(self) -> DesktopEffectiveColorScheme {
        self.color_scheme().resolve(self.system_color_scheme)
    }

    #[must_use]
    pub const fn revision(self) -> DesktopPresentationRevision {
        self.revision
    }

    #[must_use]
    pub const fn persistence(self) -> DesktopPresentationPersistence {
        self.persistence
    }

    pub fn select_density_index(&mut self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some(density) = DesktopDensity::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_density(density), false, |_| true)
    }

    pub fn select_skin_index(&mut self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some(skin) = DesktopSkin::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_skin(skin), false, |_| true)
    }

    pub fn select_color_scheme_index(&mut self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some(color_scheme) = DesktopColorScheme::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(
            self.selection.with_color_scheme(color_scheme),
            false,
            |_| true,
        )
    }

    pub fn select_layout_index(&mut self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some(layout) = DesktopLayout::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_layout(layout), false, |_| true)
    }

    pub fn select_locale_index(&mut self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some(locale) = DesktopLocale::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_locale(locale), false, |_| true)
    }

    pub fn select_density_index_if_admitted(
        &mut self,
        index: i32,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(density) = DesktopDensity::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_density(density), true, admit)
    }

    pub fn select_skin_index_if_admitted(
        &mut self,
        index: i32,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(skin) = DesktopSkin::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_skin(skin), true, admit)
    }

    pub fn select_color_scheme_index_if_admitted(
        &mut self,
        index: i32,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(color_scheme) = DesktopColorScheme::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_color_scheme(color_scheme), true, admit)
    }

    pub fn select_layout_index_if_admitted(
        &mut self,
        index: i32,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(layout) = DesktopLayout::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_layout(layout), true, admit)
    }

    pub fn select_locale_index_if_admitted(
        &mut self,
        index: i32,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(locale) = DesktopLocale::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_locale(locale), true, admit)
    }

    pub fn move_board_section_if_admitted(
        &mut self,
        index: usize,
        delta: i32,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(board) = self.selection.board().move_adjacent(index, delta) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_board(board), true, admit)
    }

    pub fn set_board_section_visible_if_admitted(
        &mut self,
        index: usize,
        visible: bool,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(board) = self.selection.board().with_visibility(index, visible) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_board(board), true, admit)
    }

    pub fn set_board_section_collapsed_if_admitted(
        &mut self,
        index: usize,
        collapsed: bool,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some(board) = self.selection.board().with_collapsed(index, collapsed) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        self.select(self.selection.with_board(board), true, admit)
    }

    pub fn reset_board_if_admitted(
        &mut self,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        self.select(
            self.selection
                .with_board(DesktopBoardPreferences::canonical()),
            true,
            admit,
        )
    }

    pub fn observe_system_color_scheme(
        &mut self,
        system_color_scheme: DesktopSystemColorScheme,
    ) -> DesktopPresentationApplyOutcome {
        let previous = self.effective_color_scheme();
        self.system_color_scheme = system_color_scheme;
        if self.effective_color_scheme() == previous {
            DesktopPresentationApplyOutcome::Unchanged
        } else {
            DesktopPresentationApplyOutcome::Applied
        }
    }

    pub fn observe_persisted(
        &mut self,
        persisted_selection: DesktopPresentationSelection,
    ) -> DesktopPresentationApplyOutcome {
        let was_saved = matches!(self.persistence, DesktopPresentationPersistence::Saved);
        if self.selection == persisted_selection {
            self.persisted_selection = persisted_selection;
            self.persistence = DesktopPresentationPersistence::Saved;
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        if !was_saved {
            self.persisted_selection = persisted_selection;
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.selection = persisted_selection;
        self.persisted_selection = persisted_selection;
        self.revision = revision;
        self.persistence = DesktopPresentationPersistence::Saved;
        DesktopPresentationApplyOutcome::Applied
    }

    pub fn observe_persisted_unconfirmed(
        &mut self,
        persisted_selection: DesktopPresentationSelection,
    ) -> DesktopPresentationApplyOutcome {
        let was_saved = matches!(self.persistence, DesktopPresentationPersistence::Saved);
        if !was_saved || self.selection == persisted_selection {
            self.persisted_selection = persisted_selection;
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.selection = persisted_selection;
        self.persisted_selection = persisted_selection;
        self.revision = revision;
        self.persistence = DesktopPresentationPersistence::Saved;
        DesktopPresentationApplyOutcome::Applied
    }

    pub fn mark_not_saved(&mut self) {
        if matches!(self.persistence, DesktopPresentationPersistence::Saving) {
            self.persistence = if self.selection == self.persisted_selection {
                DesktopPresentationPersistence::Saved
            } else {
                DesktopPresentationPersistence::NotSaved
            };
        }
    }

    pub fn apply_persisted_override(
        &mut self,
        persisted_selection: DesktopPresentationSelection,
    ) -> DesktopPresentationApplyOutcome {
        if self.selection == persisted_selection {
            if self.persisted_selection == persisted_selection
                && matches!(self.persistence, DesktopPresentationPersistence::Saved)
            {
                return DesktopPresentationApplyOutcome::Unchanged;
            }
            self.persisted_selection = persisted_selection;
            self.persistence = DesktopPresentationPersistence::Saved;
            return DesktopPresentationApplyOutcome::Applied;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.selection = persisted_selection;
        self.persisted_selection = persisted_selection;
        self.revision = revision;
        self.persistence = DesktopPresentationPersistence::Saved;
        DesktopPresentationApplyOutcome::Applied
    }

    fn select(
        &mut self,
        selection: DesktopPresentationSelection,
        admitted: bool,
        admit: impl FnOnce(DesktopPresentationSelection) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        if selection == self.selection {
            if !admitted || !matches!(self.persistence, DesktopPresentationPersistence::NotSaved) {
                return DesktopPresentationApplyOutcome::Unchanged;
            }
            if !admit(selection) {
                return DesktopPresentationApplyOutcome::Rejected;
            }
            self.persistence = DesktopPresentationPersistence::Saving;
            return DesktopPresentationApplyOutcome::Applied;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        if admitted && !admit(selection) {
            return DesktopPresentationApplyOutcome::Rejected;
        }
        self.selection = selection;
        self.revision = revision;
        self.persistence = if admitted {
            DesktopPresentationPersistence::Saving
        } else {
            DesktopPresentationPersistence::NotSaved
        };
        DesktopPresentationApplyOutcome::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopDensity, DesktopPresentationApplyOutcome, DesktopPresentationPersistence,
        DesktopPresentationRevision, DesktopPresentationSelection, DesktopPresentationStyle,
    };
    use crate::{
        DesktopColorScheme, DesktopLayout, DesktopLocale, DesktopSkin, DesktopSystemColorScheme,
    };

    #[test]
    fn locale_index_admission_is_closed_and_preserves_the_complete_selection() {
        let selection = DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        );
        let mut style = DesktopPresentationStyle::from_persisted(selection);

        assert_eq!(DesktopLocale::English.stable_key(), "en");
        assert_eq!(DesktopLocale::Russian.stable_key(), "ru");
        assert_eq!(DesktopLocale::Pseudo.stable_key(), "pseudo");
        assert_eq!(
            style.select_locale_index(1),
            DesktopPresentationApplyOutcome::Applied
        );
        assert_eq!(style.locale(), DesktopLocale::Russian);
        assert_eq!(style.selection().board(), selection.board());
        let before_rejected = style;
        assert_eq!(
            style.select_locale_index(-1),
            DesktopPresentationApplyOutcome::Rejected
        );
        assert_eq!(
            style.select_locale_index(3),
            DesktopPresentationApplyOutcome::Rejected
        );
        assert_eq!(style, before_rejected);
    }

    #[test]
    fn board_edits_preserve_axes_and_reject_hiding_the_last_visible_section() {
        let selection = DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        );
        let mut style = DesktopPresentationStyle::from_persisted(selection);

        assert_eq!(
            style.move_board_section_if_admitted(0, 1, |_| true),
            DesktopPresentationApplyOutcome::Applied
        );
        assert_eq!(
            style.selection().board().rows()[0].key(),
            crate::DesktopBoardSectionKey::CodeOutput
        );
        assert_eq!(style.density(), DesktopDensity::Comfortable);
        assert_eq!(style.skin(), DesktopSkin::Refined);
        assert_eq!(style.color_scheme(), DesktopColorScheme::System);
        assert_eq!(style.layout(), DesktopLayout::Refined);
        assert_eq!(
            style.move_board_section_if_admitted(0, 2, |_| true),
            DesktopPresentationApplyOutcome::Rejected
        );
        assert_eq!(
            style.move_board_section_if_admitted(0, -1, |_| true),
            DesktopPresentationApplyOutcome::Rejected
        );
        assert_eq!(
            style.set_board_section_collapsed_if_admitted(
                crate::DESKTOP_BOARD_SECTION_COUNT,
                true,
                |_| true,
            ),
            DesktopPresentationApplyOutcome::Rejected
        );
        let before_rejected_admission = style;
        assert_eq!(
            style.set_board_section_collapsed_if_admitted(0, true, |_| false),
            DesktopPresentationApplyOutcome::Rejected
        );
        assert_eq!(style, before_rejected_admission);

        for index in 0..crate::DESKTOP_BOARD_SECTION_COUNT - 1 {
            assert_eq!(
                style.set_board_section_visible_if_admitted(index, false, |_| true),
                DesktopPresentationApplyOutcome::Applied
            );
        }
        assert_eq!(
            style.set_board_section_visible_if_admitted(
                crate::DESKTOP_BOARD_SECTION_COUNT - 1,
                false,
                |_| true,
            ),
            DesktopPresentationApplyOutcome::Rejected
        );
        assert_eq!(
            style.set_board_section_collapsed_if_admitted(
                crate::DESKTOP_BOARD_SECTION_COUNT - 1,
                true,
                |_| true,
            ),
            DesktopPresentationApplyOutcome::Applied
        );
        assert_eq!(
            style.reset_board_if_admitted(|_| true),
            DesktopPresentationApplyOutcome::Applied
        );
        assert_eq!(
            style.selection().board(),
            crate::DesktopBoardPreferences::canonical()
        );
    }

    #[test]
    fn revision_exhaustion_preserves_every_complete_style_field() {
        let selection = DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        );
        let mut style = DesktopPresentationStyle {
            selection,
            persisted_selection: selection,
            revision: DesktopPresentationRevision(u64::MAX),
            persistence: DesktopPresentationPersistence::Saved,
            system_color_scheme: DesktopSystemColorScheme::Unknown,
        };
        let prior = style;

        assert_eq!(
            style.select_skin_index(1),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(style, prior);
    }

    #[test]
    fn exhausted_persisted_observation_preserves_every_complete_style_field() {
        let selection = DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        );
        let mut style = DesktopPresentationStyle {
            selection,
            persisted_selection: selection,
            revision: DesktopPresentationRevision(u64::MAX),
            persistence: DesktopPresentationPersistence::Saved,
            system_color_scheme: DesktopSystemColorScheme::Unknown,
        };
        let prior = style;

        assert_eq!(
            style.observe_persisted(DesktopPresentationSelection::new(
                DesktopDensity::Compact,
                DesktopSkin::Graphite,
                DesktopColorScheme::Light,
                DesktopLayout::ControlCenter,
                DesktopLocale::English,
            )),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(style, prior);
    }

    #[test]
    fn exhausted_unconfirmed_observation_preserves_every_complete_style_field() {
        let selection = DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
            DesktopLocale::English,
        );
        let mut style = DesktopPresentationStyle {
            selection,
            persisted_selection: selection,
            revision: DesktopPresentationRevision(u64::MAX),
            persistence: DesktopPresentationPersistence::Saved,
            system_color_scheme: DesktopSystemColorScheme::Unknown,
        };
        let prior = style;

        assert_eq!(
            style.observe_persisted_unconfirmed(DesktopPresentationSelection::new(
                DesktopDensity::Compact,
                DesktopSkin::Graphite,
                DesktopColorScheme::Dark,
                DesktopLayout::Workbench,
                DesktopLocale::English,
            )),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(style, prior);
    }
}
