#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopRgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl DesktopRgb {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    #[must_use]
    pub const fn red(self) -> u8 {
        self.red
    }

    #[must_use]
    pub const fn green(self) -> u8 {
        self.green
    }

    #[must_use]
    pub const fn blue(self) -> u8 {
        self.blue
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopColorTokens {
    background: DesktopRgb,
    surface: DesktopRgb,
    surface_raised: DesktopRgb,
    surface_subtle: DesktopRgb,
    border: DesktopRgb,
    text_primary: DesktopRgb,
    text_secondary: DesktopRgb,
    accent: DesktopRgb,
    accent_subtle: DesktopRgb,
    accent_secondary: DesktopRgb,
    accent_tertiary: DesktopRgb,
    ready: DesktopRgb,
    waiting: DesktopRgb,
    degraded: DesktopRgb,
    unavailable: DesktopRgb,
}

impl DesktopColorTokens {
    #[must_use]
    pub const fn background(self) -> DesktopRgb {
        self.background
    }
    #[must_use]
    pub const fn surface(self) -> DesktopRgb {
        self.surface
    }
    #[must_use]
    pub const fn surface_raised(self) -> DesktopRgb {
        self.surface_raised
    }
    #[must_use]
    pub const fn surface_subtle(self) -> DesktopRgb {
        self.surface_subtle
    }
    #[must_use]
    pub const fn border(self) -> DesktopRgb {
        self.border
    }
    #[must_use]
    pub const fn text_primary(self) -> DesktopRgb {
        self.text_primary
    }
    #[must_use]
    pub const fn text_secondary(self) -> DesktopRgb {
        self.text_secondary
    }
    #[must_use]
    pub const fn accent(self) -> DesktopRgb {
        self.accent
    }
    #[must_use]
    pub const fn accent_subtle(self) -> DesktopRgb {
        self.accent_subtle
    }
    #[must_use]
    pub const fn accent_secondary(self) -> DesktopRgb {
        self.accent_secondary
    }
    #[must_use]
    pub const fn accent_tertiary(self) -> DesktopRgb {
        self.accent_tertiary
    }
    #[must_use]
    pub const fn ready(self) -> DesktopRgb {
        self.ready
    }
    #[must_use]
    pub const fn waiting(self) -> DesktopRgb {
        self.waiting
    }
    #[must_use]
    pub const fn degraded(self) -> DesktopRgb {
        self.degraded
    }
    #[must_use]
    pub const fn unavailable(self) -> DesktopRgb {
        self.unavailable
    }

    #[must_use]
    pub const fn role_count(self) -> usize {
        15
    }

    #[must_use]
    pub const fn rgb_roles(self) -> [DesktopRgb; 15] {
        [
            self.background,
            self.surface,
            self.surface_raised,
            self.surface_subtle,
            self.border,
            self.text_primary,
            self.text_secondary,
            self.accent,
            self.accent_subtle,
            self.accent_secondary,
            self.accent_tertiary,
            self.ready,
            self.waiting,
            self.degraded,
            self.unavailable,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSkin {
    Refined,
    Graphite,
    Ember,
}

impl DesktopSkin {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Refined => "refined",
            Self::Graphite => "graphite",
            Self::Ember => "ember",
        }
    }

    #[must_use]
    pub const fn slint_index(self) -> i32 {
        match self {
            Self::Refined => 0,
            Self::Graphite => 1,
            Self::Ember => 2,
        }
    }

    #[must_use]
    pub const fn from_slint_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Refined),
            1 => Some(Self::Graphite),
            2 => Some(Self::Ember),
            _ => None,
        }
    }

    #[must_use]
    pub const fn color_tokens(self) -> DesktopColorTokens {
        match self {
            Self::Refined => refined_tokens(),
            Self::Graphite => graphite_tokens(),
            Self::Ember => ember_tokens(),
        }
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> DesktopRgb {
    DesktopRgb::new(red, green, blue)
}

const fn refined_tokens() -> DesktopColorTokens {
    DesktopColorTokens {
        background: rgb(11, 15, 23),
        surface: rgb(17, 24, 39),
        surface_raised: rgb(24, 34, 52),
        surface_subtle: rgb(14, 22, 36),
        border: rgb(41, 53, 72),
        text_primary: rgb(244, 247, 251),
        text_secondary: rgb(158, 171, 192),
        accent: rgb(124, 212, 253),
        accent_subtle: rgb(23, 48, 68),
        accent_secondary: rgb(167, 139, 250),
        accent_tertiary: rgb(240, 171, 252),
        ready: rgb(112, 214, 165),
        waiting: rgb(143, 163, 191),
        degraded: rgb(242, 198, 109),
        unavailable: rgb(240, 139, 139),
    }
}

const fn graphite_tokens() -> DesktopColorTokens {
    DesktopColorTokens {
        background: rgb(16, 18, 22),
        surface: rgb(24, 27, 32),
        surface_raised: rgb(34, 38, 45),
        surface_subtle: rgb(20, 23, 28),
        border: rgb(52, 58, 68),
        text_primary: rgb(245, 247, 250),
        text_secondary: rgb(170, 178, 189),
        accent: rgb(120, 169, 255),
        accent_subtle: rgb(31, 45, 69),
        accent_secondary: rgb(165, 180, 252),
        accent_tertiary: rgb(216, 180, 254),
        ready: rgb(115, 215, 173),
        waiting: rgb(154, 167, 184),
        degraded: rgb(234, 197, 116),
        unavailable: rgb(238, 141, 147),
    }
}

const fn ember_tokens() -> DesktopColorTokens {
    DesktopColorTokens {
        background: rgb(20, 13, 10),
        surface: rgb(32, 21, 17),
        surface_raised: rgb(46, 31, 25),
        surface_subtle: rgb(25, 15, 12),
        border: rgb(75, 48, 38),
        text_primary: rgb(255, 247, 237),
        text_secondary: rgb(205, 176, 157),
        accent: rgb(251, 146, 60),
        accent_subtle: rgb(71, 36, 23),
        accent_secondary: rgb(251, 191, 36),
        accent_tertiary: rgb(244, 114, 182),
        ready: rgb(134, 212, 157),
        waiting: rgb(189, 169, 158),
        degraded: rgb(244, 200, 111),
        unavailable: rgb(245, 143, 134),
    }
}
