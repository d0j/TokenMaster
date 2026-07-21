#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopDensity {
    Comfortable,
    Compact,
    UltraCompact,
}

impl DesktopDensity {
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
            Self::UltraCompact => "ultra_compact",
        }
    }

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
pub struct DesktopPresentationRevision(u64);

impl DesktopPresentationRevision {
    pub const fn initial() -> Self {
        Self(0)
    }

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
    density: DesktopDensity,
    persisted_density: DesktopDensity,
    revision: DesktopPresentationRevision,
    persistence: DesktopPresentationPersistence,
}

impl Default for DesktopPresentationStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopPresentationStyle {
    pub const fn new() -> Self {
        Self::from_persisted(DesktopDensity::Comfortable)
    }

    pub const fn from_persisted(density: DesktopDensity) -> Self {
        Self {
            density,
            persisted_density: density,
            revision: DesktopPresentationRevision::initial(),
            persistence: DesktopPresentationPersistence::Saved,
        }
    }

    pub const fn density(self) -> DesktopDensity {
        self.density
    }

    pub const fn revision(self) -> DesktopPresentationRevision {
        self.revision
    }

    #[must_use]
    pub const fn persisted_density(self) -> DesktopDensity {
        self.persisted_density
    }

    #[must_use]
    pub const fn persistence(self) -> DesktopPresentationPersistence {
        self.persistence
    }

    pub fn select_density_index(&mut self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some((density, revision)) = self.checked_selection(index) else {
            return self.selection_failure(index);
        };
        self.density = density;
        self.revision = revision;
        self.persistence = DesktopPresentationPersistence::NotSaved;
        DesktopPresentationApplyOutcome::Applied
    }

    pub fn select_density_index_if_admitted(
        &mut self,
        index: i32,
        admit: impl FnOnce(DesktopDensity) -> bool,
    ) -> DesktopPresentationApplyOutcome {
        let Some((density, revision)) = self.checked_selection(index) else {
            return self.selection_failure(index);
        };
        if !admit(density) {
            return DesktopPresentationApplyOutcome::Rejected;
        }
        self.density = density;
        self.revision = revision;
        self.persistence = DesktopPresentationPersistence::Saving;
        DesktopPresentationApplyOutcome::Applied
    }

    pub fn observe_persisted(
        &mut self,
        persisted_density: DesktopDensity,
    ) -> DesktopPresentationApplyOutcome {
        let was_saved = matches!(self.persistence, DesktopPresentationPersistence::Saved);
        self.persisted_density = persisted_density;
        if self.density == persisted_density {
            self.persistence = DesktopPresentationPersistence::Saved;
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        if !was_saved {
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.density = persisted_density;
        self.revision = revision;
        DesktopPresentationApplyOutcome::Applied
    }

    pub fn observe_persisted_unconfirmed(
        &mut self,
        persisted_density: DesktopDensity,
    ) -> DesktopPresentationApplyOutcome {
        let was_saved = matches!(self.persistence, DesktopPresentationPersistence::Saved);
        self.persisted_density = persisted_density;
        if !was_saved || self.density == persisted_density {
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.density = persisted_density;
        self.revision = revision;
        DesktopPresentationApplyOutcome::Applied
    }

    pub fn mark_not_saved(&mut self) {
        if matches!(self.persistence, DesktopPresentationPersistence::Saving)
            && self.density != self.persisted_density
        {
            self.persistence = DesktopPresentationPersistence::NotSaved;
        }
    }

    pub fn apply_persisted_override(
        &mut self,
        persisted_density: DesktopDensity,
    ) -> DesktopPresentationApplyOutcome {
        if self.density == persisted_density {
            if self.persisted_density == persisted_density
                && matches!(self.persistence, DesktopPresentationPersistence::Saved)
            {
                return DesktopPresentationApplyOutcome::Unchanged;
            }
            self.persisted_density = persisted_density;
            self.persistence = DesktopPresentationPersistence::Saved;
            return DesktopPresentationApplyOutcome::Applied;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.density = persisted_density;
        self.persisted_density = persisted_density;
        self.revision = revision;
        self.persistence = DesktopPresentationPersistence::Saved;
        DesktopPresentationApplyOutcome::Applied
    }

    fn checked_selection(
        self,
        index: i32,
    ) -> Option<(DesktopDensity, DesktopPresentationRevision)> {
        let density = DesktopDensity::from_slint_index(index)?;
        if density == self.density {
            return None;
        }
        let revision = self.revision.checked_successor()?;
        Some((density, revision))
    }

    fn selection_failure(self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some(density) = DesktopDensity::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        if density == self.density {
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        DesktopPresentationApplyOutcome::RevisionExhausted
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopDensity, DesktopPresentationApplyOutcome, DesktopPresentationPersistence,
        DesktopPresentationRevision, DesktopPresentationStyle,
    };

    #[test]
    fn revision_exhaustion_retains_the_current_style() {
        let mut style = DesktopPresentationStyle {
            density: DesktopDensity::Comfortable,
            persisted_density: DesktopDensity::Comfortable,
            revision: DesktopPresentationRevision(u64::MAX),
            persistence: DesktopPresentationPersistence::Saved,
        };

        assert_eq!(
            style.select_density_index(1),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(style.density(), DesktopDensity::Comfortable);
        assert_eq!(style.revision(), DesktopPresentationRevision(u64::MAX));
    }

    #[test]
    fn exhausted_admission_does_not_call_the_closure() {
        let mut style = DesktopPresentationStyle {
            density: DesktopDensity::Comfortable,
            persisted_density: DesktopDensity::Comfortable,
            revision: DesktopPresentationRevision(u64::MAX),
            persistence: DesktopPresentationPersistence::Saved,
        };
        let mut calls = 0;

        assert_eq!(
            style.select_density_index_if_admitted(1, |_| {
                calls += 1;
                true
            }),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(calls, 0);
        assert_eq!(style.density(), DesktopDensity::Comfortable);
    }

    #[test]
    fn exhausted_override_preserves_the_complete_style() {
        let mut style = DesktopPresentationStyle {
            density: DesktopDensity::Compact,
            persisted_density: DesktopDensity::Comfortable,
            revision: DesktopPresentationRevision(u64::MAX),
            persistence: DesktopPresentationPersistence::NotSaved,
        };
        let prior = style;

        assert_eq!(
            style.apply_persisted_override(DesktopDensity::UltraCompact),
            DesktopPresentationApplyOutcome::RevisionExhausted
        );
        assert_eq!(style, prior);
    }
}
