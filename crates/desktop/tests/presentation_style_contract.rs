use tokenmaster_desktop::{
    DesktopDensity, DesktopPresentationApplyOutcome, DesktopPresentationPersistence,
    DesktopPresentationStyle,
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
        assert_eq!(
            style.select_density_index(expected),
            DesktopPresentationApplyOutcome::Applied
        );
    }
    assert_eq!(style.density(), DesktopDensity::Comfortable);
    assert_eq!(style.revision().get(), 10_001);
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

#[test]
fn persistence_reconciliation_never_overwrites_a_newer_unsaved_selection() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    assert_eq!(
        style.select_density_index_if_admitted(1, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);
    assert_eq!(
        style.select_density_index_if_admitted(2, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.observe_persisted(DesktopDensity::Compact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.density(), DesktopDensity::UltraCompact);
    style.mark_not_saved();
    assert_eq!(
        style.persistence(),
        DesktopPresentationPersistence::NotSaved
    );
    assert_eq!(
        style.observe_persisted(DesktopDensity::UltraCompact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
}

#[test]
fn explicit_import_override_is_atomic_and_checked() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Compact);
    style.select_density_index_if_admitted(2, |_| true);
    assert_eq!(
        style.apply_persisted_override(DesktopDensity::Comfortable),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.density(), DesktopDensity::Comfortable);
    assert_eq!(style.persisted_density(), DesktopDensity::Comfortable);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
}

#[test]
fn admission_runs_once_after_validation_and_rejection_preserves_state() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    let unchanged = style;
    let mut calls = 0;
    assert_eq!(
        style.select_density_index_if_admitted(3, |_| {
            calls += 1;
            true
        }),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(calls, 0);
    assert_eq!(style, unchanged);

    assert_eq!(
        style.select_density_index_if_admitted(0, |_| {
            calls += 1;
            true
        }),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(calls, 0);
    assert_eq!(style, unchanged);

    assert_eq!(
        style.select_density_index_if_admitted(1, |_| {
            calls += 1;
            false
        }),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(calls, 1);
    assert_eq!(style, unchanged);
}
