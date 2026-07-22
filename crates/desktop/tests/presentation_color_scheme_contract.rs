use tokenmaster_desktop::{
    DesktopColorScheme, DesktopDensity, DesktopEffectiveColorScheme,
    DesktopPresentationApplyOutcome, DesktopPresentationPersistence, DesktopPresentationSelection,
    DesktopPresentationStyle, DesktopSkin, DesktopSystemColorScheme,
};

const SYSTEM_REFINED: DesktopPresentationSelection = DesktopPresentationSelection::new(
    DesktopDensity::Comfortable,
    DesktopSkin::Refined,
    DesktopColorScheme::System,
);

#[test]
fn requested_scheme_keys_indices_and_resolution_are_total() {
    for (scheme, key, index) in [
        (DesktopColorScheme::System, "system", 0),
        (DesktopColorScheme::Light, "light", 1),
        (DesktopColorScheme::Dark, "dark", 2),
    ] {
        assert_eq!(scheme.stable_key(), key);
        assert_eq!(scheme.slint_index(), index);
        assert_eq!(DesktopColorScheme::from_slint_index(index), Some(scheme));
    }
    for index in [-1, 3, i32::MIN, i32::MAX] {
        assert_eq!(DesktopColorScheme::from_slint_index(index), None);
    }

    for (requested, observed, effective) in [
        (
            DesktopColorScheme::System,
            DesktopSystemColorScheme::Unknown,
            DesktopEffectiveColorScheme::Dark,
        ),
        (
            DesktopColorScheme::System,
            DesktopSystemColorScheme::Light,
            DesktopEffectiveColorScheme::Light,
        ),
        (
            DesktopColorScheme::System,
            DesktopSystemColorScheme::Dark,
            DesktopEffectiveColorScheme::Dark,
        ),
        (
            DesktopColorScheme::Light,
            DesktopSystemColorScheme::Dark,
            DesktopEffectiveColorScheme::Light,
        ),
        (
            DesktopColorScheme::Dark,
            DesktopSystemColorScheme::Light,
            DesktopEffectiveColorScheme::Dark,
        ),
    ] {
        assert_eq!(requested.resolve(observed), effective);
    }
}

#[test]
fn selector_admits_one_complete_triple_before_apply() {
    let mut style = DesktopPresentationStyle::from_persisted(SYSTEM_REFINED);
    let mut admitted = None;
    assert_eq!(
        style.select_color_scheme_index_if_admitted(1, |selection| {
            admitted = Some(selection);
            true
        }),
        DesktopPresentationApplyOutcome::Applied
    );
    let light = DesktopPresentationSelection::new(
        DesktopDensity::Comfortable,
        DesktopSkin::Refined,
        DesktopColorScheme::Light,
    );
    assert_eq!(admitted, Some(light));
    assert_eq!(style.selection(), light);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);

    let before = style;
    assert_eq!(
        style.select_color_scheme_index_if_admitted(3, |_| true),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(style, before);
}

#[test]
fn system_observation_changes_only_effective_scheme_without_revision_or_persistence_churn() {
    let mut style = DesktopPresentationStyle::from_persisted(SYSTEM_REFINED);
    let revision = style.revision();
    assert_eq!(
        style.effective_color_scheme(),
        DesktopEffectiveColorScheme::Dark
    );
    assert_eq!(
        style.observe_system_color_scheme(DesktopSystemColorScheme::Light),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.effective_color_scheme(),
        DesktopEffectiveColorScheme::Light
    );
    assert_eq!(style.selection(), SYSTEM_REFINED);
    assert_eq!(style.persisted_selection(), SYSTEM_REFINED);
    assert_eq!(style.revision(), revision);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
    assert_eq!(
        style.observe_system_color_scheme(DesktopSystemColorScheme::Light),
        DesktopPresentationApplyOutcome::Unchanged
    );

    let explicit_dark = DesktopPresentationSelection::new(
        DesktopDensity::Comfortable,
        DesktopSkin::Refined,
        DesktopColorScheme::Dark,
    );
    let mut style = DesktopPresentationStyle::from_persisted(explicit_dark);
    assert_eq!(
        style.observe_system_color_scheme(DesktopSystemColorScheme::Light),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(
        style.effective_color_scheme(),
        DesktopEffectiveColorScheme::Dark
    );
    assert_eq!(
        style.select_color_scheme_index(0),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.effective_color_scheme(),
        DesktopEffectiveColorScheme::Light,
        "returning to System must use the latest observation received while explicit"
    );
}
