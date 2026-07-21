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
fn presentation_persistence_codes_are_fixed() {
    let expected = [
        (DesktopPresentationPersistence::Saved, "saved"),
        (DesktopPresentationPersistence::Saving, "saving"),
        (DesktopPresentationPersistence::NotSaved, "not_saved"),
    ];

    for (persistence, stable_code) in expected {
        assert_eq!(persistence.stable_code(), stable_code);
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
    assert_eq!(
        style.select_density_index_if_admitted(2, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    let revision_before_override = style.revision();
    assert_eq!(
        style.apply_persisted_override(DesktopDensity::Comfortable),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.density(), DesktopDensity::Comfortable);
    assert_eq!(style.persisted_density(), DesktopDensity::Comfortable);
    assert_eq!(style.revision().get(), revision_before_override.get() + 1);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
}

#[test]
fn saved_observation_applies_a_new_persisted_density_once() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);

    assert_eq!(
        style.observe_persisted(DesktopDensity::Compact),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.density(), DesktopDensity::Compact);
    assert_eq!(style.persisted_density(), DesktopDensity::Compact);
    assert_eq!(style.revision().get(), 1);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saved);
}

#[test]
fn equal_observation_and_override_resolve_persistence_without_revising() {
    let mut saving = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    assert_eq!(
        saving.select_density_index_if_admitted(1, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    let saving_revision = saving.revision();
    assert_eq!(
        saving.observe_persisted(DesktopDensity::Compact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(saving.revision(), saving_revision);
    assert_eq!(saving.persistence(), DesktopPresentationPersistence::Saved);

    let mut not_saved = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    not_saved.select_density_index_if_admitted(1, |_| true);
    not_saved.mark_not_saved();
    let not_saved_revision = not_saved.revision();
    assert_eq!(
        not_saved.observe_persisted(DesktopDensity::Compact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(not_saved.revision(), not_saved_revision);
    assert_eq!(
        not_saved.persistence(),
        DesktopPresentationPersistence::Saved
    );

    let override_revision = not_saved.revision();
    assert_eq!(
        not_saved.apply_persisted_override(DesktopDensity::Compact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(not_saved.revision(), override_revision);
    assert_eq!(
        not_saved.persistence(),
        DesktopPresentationPersistence::Saved
    );
}

#[test]
fn unconfirmed_observation_never_confirms_a_local_a_to_b_to_a_sequence() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    assert_eq!(
        style.select_density_index_if_admitted(1, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(
        style.select_density_index_if_admitted(0, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);

    assert_eq!(
        style.observe_persisted_unconfirmed(DesktopDensity::Comfortable),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);
    assert_eq!(
        style.observe_persisted(DesktopDensity::Compact),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(style.density(), DesktopDensity::Comfortable);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);
    assert_eq!(
        style.observe_persisted(DesktopDensity::Comfortable),
        DesktopPresentationApplyOutcome::Unchanged
    );
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

#[test]
fn same_density_not_saved_retry_submits_once_without_revising() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    assert_eq!(
        style.select_density_index_if_admitted(1, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    style.mark_not_saved();
    let revision = style.revision();
    let before_retry = style;
    let mut calls = 0;

    assert_eq!(
        style.select_density_index_if_admitted(1, |_| {
            calls += 1;
            true
        }),
        DesktopPresentationApplyOutcome::Applied
    );
    assert_eq!(calls, 1);
    assert_eq!(style.density(), DesktopDensity::Compact);
    assert_eq!(style.persisted_density(), DesktopDensity::Comfortable);
    assert_eq!(style.revision(), revision);
    assert_eq!(style.persistence(), DesktopPresentationPersistence::Saving);

    let before_rejection = style;
    let calls_before_unchanged = calls;
    assert_eq!(
        style.select_density_index_if_admitted(1, |_| false),
        DesktopPresentationApplyOutcome::Unchanged
    );
    assert_eq!(calls, calls_before_unchanged);
    assert_eq!(style, before_rejection);

    assert_ne!(before_retry.persistence(), style.persistence());
}

#[test]
fn same_density_not_saved_rejection_preserves_all_style_fields() {
    let mut style = DesktopPresentationStyle::from_persisted(DesktopDensity::Comfortable);
    assert_eq!(
        style.select_density_index_if_admitted(1, |_| true),
        DesktopPresentationApplyOutcome::Applied
    );
    style.mark_not_saved();
    let before_rejection = style;
    let mut calls = 0;

    assert_eq!(
        style.select_density_index_if_admitted(1, |_| {
            calls += 1;
            false
        }),
        DesktopPresentationApplyOutcome::Rejected
    );
    assert_eq!(calls, 1);
    assert_eq!(style, before_rejection);
}
