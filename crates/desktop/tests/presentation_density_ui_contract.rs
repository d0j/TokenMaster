use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{ComponentHandle, Model, SharedString};
use tokenmaster_desktop::DesktopShell;
use tokenmaster_product::ProductReducer;

#[test]
fn density_hot_switch_keeps_the_same_window_route_and_models() {
    i_slint_backend_testing::init_no_event_loop();

    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
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
