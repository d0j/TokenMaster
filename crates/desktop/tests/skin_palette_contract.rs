use std::mem::size_of;

use tokenmaster_desktop::{DesktopColorTokens, DesktopRgb, DesktopSkin};

#[test]
fn built_in_skins_have_fixed_keys_and_slint_indices() {
    let expected = [
        (DesktopSkin::Refined, "refined", 0),
        (DesktopSkin::Graphite, "graphite", 1),
        (DesktopSkin::Ember, "ember", 2),
    ];

    for (skin, key, index) in expected {
        assert_eq!(skin.stable_key(), key);
        assert_eq!(skin.slint_index(), index);
        assert_eq!(DesktopSkin::from_slint_index(index), Some(skin));
    }
    for index in [-1, 3, i32::MIN, i32::MAX] {
        assert_eq!(DesktopSkin::from_slint_index(index), None);
    }
}

#[test]
fn built_in_skins_expose_the_exact_fifteen_role_rgb_palettes() {
    let expected = [
        (
            DesktopSkin::Refined,
            [
                (11, 15, 23),
                (17, 24, 39),
                (24, 34, 52),
                (14, 22, 36),
                (41, 53, 72),
                (244, 247, 251),
                (158, 171, 192),
                (124, 212, 253),
                (23, 48, 68),
                (167, 139, 250),
                (240, 171, 252),
                (112, 214, 165),
                (143, 163, 191),
                (242, 198, 109),
                (240, 139, 139),
            ],
        ),
        (
            DesktopSkin::Graphite,
            [
                (16, 18, 22),
                (24, 27, 32),
                (34, 38, 45),
                (20, 23, 28),
                (52, 58, 68),
                (245, 247, 250),
                (170, 178, 189),
                (120, 169, 255),
                (31, 45, 69),
                (165, 180, 252),
                (216, 180, 254),
                (115, 215, 173),
                (154, 167, 184),
                (234, 197, 116),
                (238, 141, 147),
            ],
        ),
        (
            DesktopSkin::Ember,
            [
                (20, 13, 10),
                (32, 21, 17),
                (46, 31, 25),
                (25, 15, 12),
                (75, 48, 38),
                (255, 247, 237),
                (205, 176, 157),
                (251, 146, 60),
                (71, 36, 23),
                (251, 191, 36),
                (244, 114, 182),
                (134, 212, 157),
                (189, 169, 158),
                (244, 200, 111),
                (245, 143, 134),
            ],
        ),
    ];

    for (skin, rgb) in expected {
        let palette = skin.color_tokens();
        assert_eq!(palette.role_count(), 15);
        assert_eq!(
            palette.rgb_roles(),
            rgb.map(|(red, green, blue)| DesktopRgb::new(red, green, blue))
        );
    }
}

#[test]
fn palette_values_are_fixed_copy_data_with_meaningful_contrast() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<DesktopRgb>();
    assert_copy::<DesktopColorTokens>();
    assert_copy::<DesktopSkin>();
    assert_eq!(size_of::<DesktopRgb>(), 3);
    assert_eq!(size_of::<DesktopColorTokens>(), 45);

    for skin in [
        DesktopSkin::Refined,
        DesktopSkin::Graphite,
        DesktopSkin::Ember,
    ] {
        let palette = skin.color_tokens();
        for foreground in [
            palette.text_primary(),
            palette.text_secondary(),
            palette.accent(),
            palette.ready(),
            palette.waiting(),
            palette.degraded(),
            palette.unavailable(),
        ] {
            assert!(contrast_ratio(foreground, palette.surface()) > 6.8);
        }
    }

    let source = include_str!("../src/skin.rs");
    for forbidden in ["String", "Vec", "Path", "serde", "slint::", "M0", "probe"] {
        assert!(
            !source.contains(forbidden),
            "skin DTO source contains {forbidden}"
        );
    }
}

fn contrast_ratio(left: DesktopRgb, right: DesktopRgb) -> f64 {
    let lighter = relative_luminance(left).max(relative_luminance(right));
    let darker = relative_luminance(left).min(relative_luminance(right));
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(rgb: DesktopRgb) -> f64 {
    fn channel(value: u8) -> f64 {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(rgb.red()) + 0.7152 * channel(rgb.green()) + 0.0722 * channel(rgb.blue())
}
