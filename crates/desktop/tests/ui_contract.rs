use slint::{Model, SharedString};
use tokenmaster_desktop::{DesktopApplyOutcome, DesktopShell};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::QueryErrorCode;

#[test]
fn compiled_shell_renders_exact_route_model_and_switches_in_place() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();

    assert_eq!(window.get_route_rows().row_count(), 11);
    assert_eq!(
        window.get_product_generation(),
        snapshot.generation().get().to_string()
    );
    assert_eq!(window.get_active_route_key(), "dashboard");
    assert_eq!(window.get_active_route_state(), "unavailable");
    assert_eq!(window.get_active_route_reasons(), "data_status_unavailable");

    window.invoke_select_route(SharedString::from("settings"));
    assert_eq!(window.get_active_route_key(), "settings");
    assert_eq!(window.get_active_route_state(), "ready");
    assert_eq!(
        window
            .get_route_rows()
            .iter()
            .filter(|row| row.selected)
            .count(),
        1
    );

    window.invoke_select_route(SharedString::from("not-a-route"));
    assert_eq!(window.get_active_route_key(), "settings");

    let attempt = ProductAttemptGeneration::new(1).expect("attempt");
    let mut reducer = reducer;
    reducer
        .fail_data_status(attempt, QueryErrorCode::Unavailable)
        .expect("new product generation");
    let newer = reducer.snapshot();
    assert_eq!(
        shell
            .apply_snapshot(&newer)
            .expect("shared presentation state remains available"),
        DesktopApplyOutcome::Accepted
    );
    assert_eq!(
        window.get_product_generation(),
        newer.generation().get().to_string()
    );
    assert_eq!(
        shell
            .apply_snapshot(&newer)
            .expect("shared presentation state remains available"),
        DesktopApplyOutcome::IgnoredNotNewer
    );
    assert_eq!(window.get_active_route_key(), "settings");
}
