use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{ComponentHandle, Model, SharedString};
use tokenmaster_desktop::{
    DesktopBackupPolicy, DesktopDensity, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopOperationKind, DesktopOperationPhase, DesktopOperationSnapshot,
    DesktopPresentationSettings, DesktopReliableStateHealth, DesktopReliableStateInput,
    DesktopReliableStateProjection, DesktopReliableStateSummary, DesktopReminderPolicy,
    DesktopShell,
};
use tokenmaster_product::ProductReducer;

struct RecordingIntentSink {
    admission: DesktopIntentAdmission,
    count: std::cell::Cell<u64>,
    last: std::cell::Cell<Option<DesktopDensity>>,
}

impl RecordingIntentSink {
    fn accepting() -> Self {
        Self {
            admission: DesktopIntentAdmission::Started,
            count: std::cell::Cell::new(0),
            last: std::cell::Cell::new(None),
        }
    }

    fn rejecting() -> Self {
        Self {
            admission: DesktopIntentAdmission::Rejected,
            count: std::cell::Cell::new(0),
            last: std::cell::Cell::new(None),
        }
    }

    fn last(&self) -> Option<DesktopDensity> {
        self.last.get()
    }

    fn count(&self) -> u64 {
        self.count.get()
    }
}

impl DesktopIntentSink for RecordingIntentSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        if let DesktopIntent::UpdatePresentationDensity(density) = intent {
            self.count.set(self.count.get() + 1);
            self.last.set(Some(density));
        }
        self.admission
    }
}

fn reliable_state_with_density_and_operation(
    density: DesktopDensity,
    operation: Option<DesktopOperationSnapshot>,
) -> DesktopReliableStateProjection {
    let summary = DesktopReliableStateSummary::new_with_settings(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::disabled(),
        DesktopReminderPolicy::unavailable(),
        DesktopPresentationSettings::new(density),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        operation,
        None,
    );
    DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        1,
        summary,
        Vec::new(),
    ))
}

fn reliable_state_with_density(density: DesktopDensity) -> DesktopReliableStateProjection {
    reliable_state_with_density_and_operation(density, None)
}

#[test]
fn persisted_density_hydrates_before_show_and_admitted_switch_is_immediate() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::UltraCompact),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    assert_eq!(window.get_presentation_density_key(), "ultra_compact");
    assert_eq!(window.get_presentation_persistence_state(), "saved");

    window.invoke_select_presentation_density(1);

    assert_eq!(window.get_presentation_density_key(), "compact");
    assert_eq!(window.get_presentation_persistence_state(), "saving");
    assert_eq!(sink.last(), Some(DesktopDensity::Compact));
}

#[test]
fn rejected_density_admission_retains_visible_density_and_revision() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::rejecting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::Comfortable),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    let density = window.get_presentation_density_key();
    let revision = window.get_presentation_revision();

    window.invoke_select_presentation_density(1);

    assert_eq!(window.get_presentation_density_key(), density);
    assert_eq!(window.get_presentation_revision(), revision);
    assert_eq!(window.get_presentation_persistence_state(), "saved");
    assert_eq!(sink.last(), Some(DesktopDensity::Compact));
}

#[test]
fn stale_persisted_density_does_not_overwrite_a_newer_saving_selection() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::UltraCompact),
        sink,
    )
    .expect("shell");
    let window = shell.window();
    window.invoke_select_presentation_density(1);
    shell
        .apply_reliable_state(reliable_state_with_density(DesktopDensity::UltraCompact))
        .expect("stale reliable state");
    assert_eq!(window.get_presentation_density_key(), "compact");
    assert_eq!(window.get_presentation_persistence_state(), "saving");

    shell
        .apply_reliable_state(reliable_state_with_density_and_operation(
            DesktopDensity::Compact,
            Some(DesktopOperationSnapshot::new(
                DesktopOperationKind::UpdatePresentation,
                DesktopOperationPhase::Succeeded,
                false,
                None,
            )),
        ))
        .expect("matching reliable state");
    assert_eq!(window.get_presentation_persistence_state(), "saved");
}

#[test]
fn failed_density_persistence_is_not_saved_but_import_and_portable_restore_override() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::Comfortable),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    window.invoke_select_presentation_density(1);
    shell
        .apply_reliable_state(reliable_state_with_density_and_operation(
            DesktopDensity::Comfortable,
            Some(DesktopOperationSnapshot::new(
                DesktopOperationKind::UpdatePresentation,
                DesktopOperationPhase::Failed,
                false,
                Some("unavailable"),
            )),
        ))
        .expect("failed presentation save");
    assert_eq!(window.get_presentation_density_key(), "compact");
    assert_eq!(window.get_presentation_persistence_state(), "not_saved");

    for kind in [
        DesktopOperationKind::ApplyConfig,
        DesktopOperationKind::RestoreWithPortableSettings,
    ] {
        shell
            .apply_reliable_state(reliable_state_with_density_and_operation(
                DesktopDensity::UltraCompact,
                Some(DesktopOperationSnapshot::new(
                    kind,
                    DesktopOperationPhase::Succeeded,
                    false,
                    None,
                )),
            ))
            .expect("portable settings override");
        assert_eq!(window.get_presentation_density_key(), "ultra_compact");
        assert_eq!(window.get_presentation_persistence_state(), "saved");
    }
}

#[test]
fn preview_cancel_and_data_only_restore_do_not_override_unsaved_density() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::Comfortable),
        sink,
    )
    .expect("shell");
    let window = shell.window();
    window.invoke_select_presentation_density(1);
    for (kind, phase) in [
        (
            DesktopOperationKind::ImportConfig,
            DesktopOperationPhase::Succeeded,
        ),
        (
            DesktopOperationKind::ImportConfig,
            DesktopOperationPhase::Cancelled,
        ),
        (
            DesktopOperationKind::Restore,
            DesktopOperationPhase::Succeeded,
        ),
    ] {
        shell
            .apply_reliable_state(reliable_state_with_density_and_operation(
                DesktopDensity::UltraCompact,
                Some(DesktopOperationSnapshot::new(kind, phase, false, None)),
            ))
            .expect("non-overriding reliable state");
        assert_eq!(window.get_presentation_density_key(), "compact");
        assert_eq!(window.get_presentation_persistence_state(), "saving");
    }
}

#[test]
fn ten_thousand_accepted_density_switches_reuse_the_same_window_routes_and_models() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::Comfortable),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    let component_address = window as *const _;
    let routes = window.get_route_rows().row_count();
    let quotas = window.get_dashboard_quota_rows().row_count();

    for index in 0..10_000 {
        window.invoke_select_presentation_density(index % 3);
    }

    assert_eq!(component_address, shell.window() as *const _);
    assert_eq!(window.get_route_rows().row_count(), routes);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), quotas);
    assert_eq!(sink.count(), 9_999);
    assert_eq!(sink.last(), Some(DesktopDensity::Comfortable));
}

#[test]
fn stale_running_projection_cannot_confirm_a_newer_a_to_b_to_a_selection() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state_with_density(DesktopDensity::Comfortable),
        sink,
    )
    .expect("shell");
    let window = shell.window();
    window.invoke_select_presentation_density(1);
    window.invoke_select_presentation_density(0);
    assert_eq!(window.get_presentation_density_key(), "comfortable");
    assert_eq!(window.get_presentation_persistence_state(), "saving");

    shell
        .apply_reliable_state(reliable_state_with_density_and_operation(
            DesktopDensity::Comfortable,
            Some(DesktopOperationSnapshot::new(
                DesktopOperationKind::UpdatePresentation,
                DesktopOperationPhase::Running,
                true,
                None,
            )),
        ))
        .expect("stale running A");
    assert_eq!(window.get_presentation_persistence_state(), "saving");

    shell
        .apply_reliable_state(reliable_state_with_density_and_operation(
            DesktopDensity::Compact,
            Some(DesktopOperationSnapshot::new(
                DesktopOperationKind::UpdatePresentation,
                DesktopOperationPhase::Succeeded,
                false,
                None,
            )),
        ))
        .expect("successful B");
    assert_eq!(window.get_presentation_density_key(), "comfortable");
    assert_eq!(window.get_presentation_persistence_state(), "saving");

    shell
        .apply_reliable_state(reliable_state_with_density_and_operation(
            DesktopDensity::Comfortable,
            Some(DesktopOperationSnapshot::new(
                DesktopOperationKind::UpdatePresentation,
                DesktopOperationPhase::Succeeded,
                false,
                None,
            )),
        ))
        .expect("successful A");
    assert_eq!(window.get_presentation_density_key(), "comfortable");
    assert_eq!(window.get_presentation_persistence_state(), "saved");
}

#[test]
fn density_hot_switch_keeps_the_same_window_route_and_models() {
    i_slint_backend_testing::init_no_event_loop();

    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new_with_reliable_state(
        &snapshot,
        reliable_state_with_density(DesktopDensity::Comfortable),
        Rc::new(RecordingIntentSink::accepting()),
    )
    .expect("desktop shell");
    let window = shell.window();
    let component_address = window as *const _;

    window.invoke_select_route(SharedString::from("settings"));
    let routes = window.get_route_rows().row_count();
    let quotas = window.get_dashboard_quota_rows().row_count();
    window.show().expect("show settings window");
    assert_eq!(
        ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Combobox)
            .match_predicate(
                |element| element.accessible_label().as_deref() == Some("Presentation density")
            )
            .find_all()
            .len(),
        1
    );
    assert_eq!(window.get_presentation_density_key(), "comfortable");
    assert_eq!(window.get_presentation_revision(), "0");
    assert_eq!(window.get_presentation_space(), 16.0);
    assert_eq!(window.get_presentation_radius(), 8.0);

    // Slint's testing accessibility increment action does not update ComboBox selection in 1.17.1,
    // so callback forwarding remains covered through MainWindow's compiled callback API.
    window.invoke_select_presentation_density(1);
    assert_eq!(window.get_presentation_density_key(), "compact");
    assert_eq!(window.get_presentation_revision(), "1");
    assert_eq!(window.get_presentation_space(), 12.0);
    assert_eq!(window.get_presentation_radius(), 6.0);
    assert_eq!(window.get_active_route_key(), "settings");
    assert_eq!(window.get_route_rows().row_count(), routes);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), quotas);
    assert_eq!(component_address, shell.window() as *const _);

    let density = window.get_presentation_density_key();
    let revision = window.get_presentation_revision();
    window.invoke_select_presentation_density(1);
    assert_eq!(window.get_presentation_density_key(), density);
    assert_eq!(window.get_presentation_revision(), revision);
    window.invoke_select_presentation_density(9);
    assert_eq!(window.get_presentation_density_key(), density);
    assert_eq!(window.get_presentation_revision(), revision);

    window.invoke_select_presentation_density(2);
    assert_eq!(window.get_presentation_density_key(), "ultra_compact");
    assert_eq!(window.get_presentation_space(), 8.0);
    assert_eq!(window.get_presentation_radius(), 4.0);

    for index in 0..10_000 {
        window.invoke_select_presentation_density(index % 3);
    }
    assert_eq!(window.get_active_route_key(), "settings");
    assert_eq!(window.get_route_rows().row_count(), routes);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), quotas);
    assert_eq!(component_address, shell.window() as *const _);
}

#[test]
fn density_token_source_has_the_complete_fixed_table() {
    let tokens = include_str!("../ui/tokens.slint");

    for token in [
        "space-xs: density-id == 2 ? 2px : (density-id == 1 ? 3px : 4px)",
        "space-sm: density-id == 2 ? 4px : (density-id == 1 ? 6px : 8px)",
        "space: density-id == 2 ? 8px : (density-id == 1 ? 12px : 16px)",
        "space-lg: density-id == 2 ? 12px : (density-id == 1 ? 18px : 24px)",
        "radius-sm: density-id == 2 ? 3px : (density-id == 1 ? 4px : 5px)",
        "radius: density-id == 2 ? 4px : (density-id == 1 ? 6px : 8px)",
        "radius-lg: density-id == 2 ? 6px : (density-id == 1 ? 9px : 12px)",
    ] {
        assert!(tokens.contains(token), "missing density token {token}");
    }
}
