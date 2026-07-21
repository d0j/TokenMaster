use crate::DesktopSkin;

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
pub struct DesktopPresentationSelection {
    density: DesktopDensity,
    skin: DesktopSkin,
}

impl DesktopPresentationSelection {
    #[must_use]
    pub const fn new(density: DesktopDensity, skin: DesktopSkin) -> Self {
        Self { density, skin }
    }

    #[must_use]
    pub const fn density(self) -> DesktopDensity {
        self.density
    }

    #[must_use]
    pub const fn skin(self) -> DesktopSkin {
        self.skin
    }

    const fn with_density(self, density: DesktopDensity) -> Self {
        Self::new(density, self.skin)
    }

    const fn with_skin(self, skin: DesktopSkin) -> Self {
        Self::new(self.density, skin)
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
}

impl Default for DesktopPresentationStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopPresentationStyle {
    #[must_use]
    pub const fn new() -> Self {
        Self::from_persisted(DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
        ))
    }

    #[must_use]
    pub const fn from_persisted(selection: DesktopPresentationSelection) -> Self {
        Self {
            selection,
            persisted_selection: selection,
            revision: DesktopPresentationRevision::initial(),
            persistence: DesktopPresentationPersistence::Saved,
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

    pub fn observe_persisted(
        &mut self,
        persisted_selection: DesktopPresentationSelection,
    ) -> DesktopPresentationApplyOutcome {
        let was_saved = matches!(self.persistence, DesktopPresentationPersistence::Saved);
        self.persisted_selection = persisted_selection;
        if self.selection == persisted_selection {
            self.persistence = DesktopPresentationPersistence::Saved;
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        if !was_saved {
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        self.apply_selection(persisted_selection, DesktopPresentationPersistence::Saved)
    }

    pub fn observe_persisted_unconfirmed(
        &mut self,
        persisted_selection: DesktopPresentationSelection,
    ) -> DesktopPresentationApplyOutcome {
        let was_saved = matches!(self.persistence, DesktopPresentationPersistence::Saved);
        self.persisted_selection = persisted_selection;
        if !was_saved || self.selection == persisted_selection {
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        self.apply_selection(persisted_selection, DesktopPresentationPersistence::Saved)
    }

    pub fn mark_not_saved(&mut self) {
        if matches!(self.persistence, DesktopPresentationPersistence::Saving)
            && self.selection != self.persisted_selection
        {
            self.persistence = DesktopPresentationPersistence::NotSaved;
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

    fn apply_selection(
        &mut self,
        selection: DesktopPresentationSelection,
        persistence: DesktopPresentationPersistence,
    ) -> DesktopPresentationApplyOutcome {
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.selection = selection;
        self.revision = revision;
        self.persistence = persistence;
        DesktopPresentationApplyOutcome::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopDensity, DesktopPresentationApplyOutcome, DesktopPresentationPersistence,
        DesktopPresentationRevision, DesktopPresentationSelection, DesktopPresentationStyle,
    };
    use crate::DesktopSkin;

    #[test]
    fn revision_exhaustion_preserves_every_complete_style_field() {
        let selection =
            DesktopPresentationSelection::new(DesktopDensity::Comfortable, DesktopSkin::Refined);
        let mut style = DesktopPresentationStyle {
            selection,
            persisted_selection: selection,
            revision: DesktopPresentationRevision(u64::MAX),
            persistence: DesktopPresentationPersistence::Saved,
        };
        let prior = style;

        assert_eq!(
            style.select_skin_index(1),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(style, prior);
    }
}
