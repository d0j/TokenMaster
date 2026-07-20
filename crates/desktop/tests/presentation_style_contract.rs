use tokenmaster_desktop::{
    DesktopDensity, DesktopPresentationApplyOutcome, DesktopPresentationStyle,
};

#[test]
fn density_selection_is_checked_revisioned_and_constant_state() {
    let mut style = DesktopPresentationStyle::new();
    assert_eq!(style.density(), DesktopDensity::Comfortable);
    assert_eq!(style.revision().get(), 0);
    assert_eq!(
        style.select_density_index(1),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.density(), DesktopDensity::Compact);
    assert_eq!(style.revision().get(), 1);
    assert_eq!(
        style.select_density_index(1),
        DesktopPresentationApplyOutcome::Unchanged
    );
    let density_before_rejection = style.density();
    let revision_before_rejection = style.revision();
    assert_eq!(
        style.select_density_index(3),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(style.density(), density_before_rejection);
    assert_eq!(style.revision(), revision_before_rejection);
    for index in 0..10_000 {
        let expected = match index % 3 {
            0 => 0,
            1 => 1,
            _ => 2,
        };
        let _ = style.select_density_index(expected);
    }
    assert!(style.revision().get() <= 10_001);
}

#[test]
fn default_style_matches_the_initial_style() {
    assert_eq!(
        DesktopPresentationStyle::default(),
        DesktopPresentationStyle::new()
    );
}

#[test]
fn density_keys_and_slint_indices_are_fixed() {
    let expected = [
        (DesktopDensity::Comfortable, "comfortable", 0),
        (DesktopDensity::Compact, "compact", 1),
        (DesktopDensity::UltraCompact, "ultra_compact", 2),
    ];

    for (density, stable_key, slint_index) in expected {
        assert_eq!(density.stable_key(), stable_key);
        assert_eq!(density.slint_index(), slint_index);
    }
}
