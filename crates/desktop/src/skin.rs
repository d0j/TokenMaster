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
    pub const fn color_tokens(
        self,
        color_scheme: crate::DesktopEffectiveColorScheme,
    ) -> DesktopColorTokens {
        match (self, color_scheme) {
            (Self::Refined, crate::DesktopEffectiveColorScheme::Light) => refined_light_tokens(),
            (Self::Graphite, crate::DesktopEffectiveColorScheme::Light) => graphite_light_tokens(),
            (Self::Ember, crate::DesktopEffectiveColorScheme::Light) => ember_light_tokens(),
            (Self::Refined, crate::DesktopEffectiveColorScheme::Dark) => refined_tokens(),
            (Self::Graphite, crate::DesktopEffectiveColorScheme::Dark) => graphite_tokens(),
            (Self::Ember, crate::DesktopEffectiveColorScheme::Dark) => ember_tokens(),
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

const fn refined_light_tokens() -> DesktopColorTokens {
    DesktopColorTokens {
        background: rgb(246, 248, 252),
        surface: rgb(255, 255, 255),
        surface_raised: rgb(241, 245, 249),
        surface_subtle: rgb(236, 242, 248),
        border: rgb(190, 201, 215),
        text_primary: rgb(17, 24, 39),
        text_secondary: rgb(75, 85, 99),
        accent: rgb(0, 80, 125),
        accent_subtle: rgb(219, 238, 248),
        accent_secondary: rgb(91, 33, 182),
        accent_tertiary: rgb(126, 23, 139),
        ready: rgb(0, 95, 55),
        waiting: rgb(65, 75, 90),
        degraded: rgb(120, 65, 0),
        unavailable: rgb(155, 25, 25),
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

const fn graphite_light_tokens() -> DesktopColorTokens {
    DesktopColorTokens {
        background: rgb(245, 246, 248),
        surface: rgb(255, 255, 255),
        surface_raised: rgb(238, 240, 243),
        surface_subtle: rgb(232, 235, 239),
        border: rgb(182, 188, 198),
        text_primary: rgb(22, 25, 30),
        text_secondary: rgb(72, 78, 88),
        accent: rgb(21, 78, 145),
        accent_subtle: rgb(218, 230, 246),
        accent_secondary: rgb(70, 52, 168),
        accent_tertiary: rgb(112, 35, 143),
        ready: rgb(0, 94, 59),
        waiting: rgb(63, 72, 84),
        degraded: rgb(115, 67, 0),
        unavailable: rgb(151, 29, 37),
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

const fn ember_light_tokens() -> DesktopColorTokens {
    DesktopColorTokens {
        background: rgb(255, 248, 242),
        surface: rgb(255, 255, 255),
        surface_raised: rgb(250, 239, 230),
        surface_subtle: rgb(247, 233, 222),
        border: rgb(211, 184, 165),
        text_primary: rgb(43, 25, 18),
        text_secondary: rgb(91, 65, 52),
        accent: rgb(139, 46, 0),
        accent_subtle: rgb(249, 222, 204),
        accent_secondary: rgb(126, 71, 0),
        accent_tertiary: rgb(139, 35, 91),
        ready: rgb(20, 96, 54),
        waiting: rgb(82, 67, 59),
        degraded: rgb(121, 66, 0),
        unavailable: rgb(158, 31, 24),
    }
}
