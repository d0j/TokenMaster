use crate::DesktopSkin;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopDensity {
    Comfortable,
    Compact,
    UltraCompact,
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
}

impl DesktopPresentationSelection {
    #[must_use]
    pub const fn new(
        density: DesktopDensity,
        skin: DesktopSkin,
        color_scheme: DesktopColorScheme,
        layout: DesktopLayout,
    ) -> Self {
        Self {
            density,
            skin,
            color_scheme,
            layout,
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

    const fn with_density(self, density: DesktopDensity) -> Self {
        Self::new(density, self.skin, self.color_scheme, self.layout)
    }

    const fn with_skin(self, skin: DesktopSkin) -> Self {
        Self::new(self.density, skin, self.color_scheme, self.layout)
    }

    const fn with_color_scheme(self, color_scheme: DesktopColorScheme) -> Self {
        Self::new(self.density, self.skin, color_scheme, self.layout)
    }

    const fn with_layout(self, layout: DesktopLayout) -> Self {
        Self::new(self.density, self.skin, self.color_scheme, layout)
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
    use crate::{DesktopColorScheme, DesktopLayout, DesktopSkin, DesktopSystemColorScheme};

    #[test]
    fn revision_exhaustion_preserves_every_complete_style_field() {
        let selection = DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
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
            )),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(style, prior);
    }
}
