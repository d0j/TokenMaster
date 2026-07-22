use tokenmaster_desktop::{
    DesktopColorScheme, DesktopDensity, DesktopLayout, DesktopPresentationApplyOutcome,
    DesktopPresentationPersistence, DesktopPresentationSelection, DesktopPresentationStyle,
    DesktopSkin,
};

const REFINED_COMFORTABLE: DesktopPresentationSelection = DesktopPresentationSelection::new(
    DesktopDensity::Comfortable,
    DesktopSkin::Refined,
    DesktopColorScheme::System,
    DesktopLayout::Refined,
);
const GRAPHITE_COMPACT: DesktopPresentationSelection = DesktopPresentationSelection::new(
    DesktopDensity::Compact,
    DesktopSkin::Graphite,
    DesktopColorScheme::Light,
    DesktopLayout::Refined,
);
const EMBER_ULTRA_COMPACT: DesktopPresentationSelection = DesktopPresentationSelection::new(
    DesktopDensity::UltraCompact,
    DesktopSkin::Ember,
    DesktopColorScheme::Dark,
    DesktopLayout::Refined,
);

#[test]
fn selection_is_complete_checked_and_revisioned_across_all_axes() {
    let mut style = DesktopPresentationStyle::from_persisted(REFINED_COMFORTABLE);
    assert_eq!(style.selection(), REFINED_COMFORTABLE);
    assert_eq!(style.persisted_selection(), REFINED_COMFORTABLE);
    assert_eq!(style.revision().get(), 0);

    assert_eq!(
        style.select_density_index(1),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.selection(),
        DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
        )
    );
    assert_eq!(style.revision().get(), 1);
    assert_eq!(
        style.select_skin_index(1),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.selection(),
        DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Graphite,
            DesktopColorScheme::System,
            DesktopLayout::Refined,
        )
    );
    assert_eq!(style.revision().get(), 2);
    assert_eq!(
        style.select_color_scheme_index(1),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.selection(), GRAPHITE_COMPACT);
    assert_eq!(style.revision().get(), 3);
    assert_eq!(
        style.select_layout_index(2),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.layout(), DesktopLayout::Workbench);
    assert_eq!(style.revision().get(), 4);
    assert_eq!(
        style.select_skin_index(1),
        DesktopPresentationApplyOutcome::Unchanged
    );

    let before_rejection = style;
    assert_eq!(
        style.select_density_index(-1),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(
        style.select_skin_index(3),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(
        style.select_layout_index(-1),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(style, before_rejection);
}

#[test]
fn density_and_skin_keys_and_slint_indices_are_fixed() {
    for (density, stable_key, slint_index) in [
        (DesktopDensity::Comfortable, "comfortable", 0),
        (DesktopDensity::Compact, "compact", 1),
        (DesktopDensity::UltraCompact, "ultra_compact", 2),
    ] {
        assert_eq!(density.stable_key(), stable_key);
        assert_eq!(density.slint_index(), slint_index);
    }
    for (layout, stable_key, slint_index) in [
        (DesktopLayout::Refined, "refined", 0),
        (DesktopLayout::ControlCenter, "control_center", 1),
        (DesktopLayout::Workbench, "workbench", 2),
    ] {
        assert_eq!(layout.stable_key(), stable_key);
        assert_eq!(layout.slint_index(), slint_index);
    }
}

#[test]
fn presentation_persistence_codes_are_fixed() {
    for (persistence, stable_code) in [
        (DesktopPresentationPersistence::Saved, "saved"),
        (DesktopPresentationPersistence::Saving, "saving"),
        (DesktopPresentationPersistence::NotSaved, "not_saved"),
    ] {
        assert_eq!(persistence.stable_code(), stable_code);
    }
}

#[test]
fn admission_receives_one_complete_selection_and_same_not_saved_value_retries() {
    let mut style = DesktopPresentationStyle::from_persisted(REFINED_COMFORTABLE);
    let mut admitted = None;
    assert_eq!(
        style.select_skin_index_if_admitted(1, |selection| {
            admitted = Some(selection);
            true
        }),
        DesktopPresentationApplyOutcome::Applied
    );
    let graphite_comfortable = DesktopPresentationSelection::new(
        DesktopDensity::Comfortable,
        DesktopSkin::Graphite,
        DesktopColorScheme::System,
        DesktopLayout::Refined,
    );
    assert_eq!(admitted, Some(graphite_comfortable));
    assert_eq!(style.selection(), graphite_comfortable);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);

    style.mark_not_saved();
    let revision = style.revision();
    let mut retries = 0;
    assert_eq!(
        style.select_skin_index_if_admitted(1, |selection| {
            retries += 1;
            assert_eq!(selection, graphite_comfortable);
            true
        }),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(retries, 1);
    assert_eq!(style.revision(), revision);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);
}

#[test]
fn stale_terminal_observations_cannot_overwrite_newer_mixed_axis_selection() {
    let mut style = DesktopPresentationStyle::from_persisted(REFINED_COMFORTABLE);
    assert_eq!(
        style.select_density_index_if_admitted(2, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.select_skin_index_if_admitted(2, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.select_color_scheme_index_if_admitted(2, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.selection(), EMBER_ULTRA_COMPACT);

    assert_eq!(
        style.observe_persisted(GRAPHITE_COMPACT),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.selection(), EMBER_ULTRA_COMPACT);
    style.mark_not_saved();
    assert_eq!(
        style.persistence(),
        DesktopPresentationPersistence::NotSaved
    );
    assert_eq!(
        style.observe_persisted_unconfirmed(REFINED_COMFORTABLE),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.selection(), EMBER_ULTRA_COMPACT);
    assert_eq!(
        style.persistence(),
        DesktopPresentationPersistence::NotSaved
    );
}

#[test]
fn config_import_and_portable_restore_override_both_axes_atomically() {
    let mut style = DesktopPresentationStyle::from_persisted(REFINED_COMFORTABLE);
    style.select_skin_index_if_admitted(1, |_| true);
    let revision_before_override = style.revision();

    assert_eq!(
        style.apply_persisted_override(EMBER_ULTRA_COMPACT),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.selection(), EMBER_ULTRA_COMPACT);
    assert_eq!(style.persisted_selection(), EMBER_ULTRA_COMPACT);
    assert_eq!(style.revision().get(), revision_before_override.get() + 1);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
}

#[test]
fn unrelated_projection_and_data_only_restore_leave_complete_selection_unchanged() {
    let mut style = DesktopPresentationStyle::from_persisted(REFINED_COMFORTABLE);
    style.select_skin_index_if_admitted(1, |_| true);
    let before = style;

    assert_eq!(
        style.observe_persisted_unconfirmed(EMBER_ULTRA_COMPACT),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.selection(), before.selection());
    assert_eq!(style.persisted_selection(), EMBER_ULTRA_COMPACT);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);
}

#[test]
fn terminal_failure_or_cancel_resolves_a_to_b_to_a_to_saved_without_permanent_saving() {
    for terminal in ["failed", "cancelled"] {
        let mut style = DesktopPresentationStyle::from_persisted(REFINED_COMFORTABLE);
        assert_eq!(
            style.select_skin_index_if_admitted(1, |_| true),
            DesktopPresentationApplyOutcome::Applied
        );
        assert_eq!(
            style.select_skin_index_if_admitted(0, |_| true),
            DesktopPresentationApplyOutcome::Applied
        );
        assert_eq!(style.selection(), REFINED_COMFORTABLE, "{terminal}");
        assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);

        assert_eq!(
            style.observe_persisted_unconfirmed(REFINED_COMFORTABLE),
            DesktopPresentationApplyOutcome::Unchanged
        );
        style.mark_not_saved();
        assert_eq!(
            style.persistence(),
            DesktopPresentationPersistence::Saved,
            "{terminal} terminal must not leave a matched complete selection saving"
        );
    }
}

#[test]
fn presentation_style_source_has_no_default_or_zero_argument_constructor() {
    let source = include_str!("../src/presentation_style.rs");

    assert!(!source.contains("impl Default for DesktopPresentationStyle"));
    assert!(!source.contains("pub const fn new() -> Self"));
}
