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
pub struct DesktopPresentationStyle {
    density: DesktopDensity,
    revision: DesktopPresentationRevision,
}

impl Default for DesktopPresentationStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopPresentationStyle {
    pub const fn new() -> Self {
        Self {
            density: DesktopDensity::Comfortable,
            revision: DesktopPresentationRevision::initial(),
        }
    }

    pub const fn density(self) -> DesktopDensity {
        self.density
    }

    pub const fn revision(self) -> DesktopPresentationRevision {
        self.revision
    }

    pub fn select_density_index(&mut self, index: i32) -> DesktopPresentationApplyOutcome {
        let Some(density) = DesktopDensity::from_slint_index(index) else {
            return DesktopPresentationApplyOutcome::Rejected;
        };
        if density == self.density {
            return DesktopPresentationApplyOutcome::Unchanged;
        }
        let Some(revision) = self.revision.checked_successor() else {
            return DesktopPresentationApplyOutcome::RevisionExhausted;
        };
        self.density = density;
        self.revision = revision;
        DesktopPresentationApplyOutcome::Applied
    }
}
