use slint::{Model, SharedString};
use tokenmaster_m0::{MainWindow, seed_probe_models, wire_skin_callbacks};

#[test]
fn compiled_skins_and_locales_switch_without_window_recreation() {
    let window = MainWindow::new().expect("window");
    wire_skin_callbacks(&window);
    seed_probe_models(&window);

    window.invoke_switch_layout(2);
    window.invoke_switch_theme(1);
    window.invoke_switch_locale(SharedString::from("ru"));

    assert_eq!(window.get_layout_id(), 2);
    assert_eq!(window.get_theme_id(), 1);
    assert_eq!(window.get_locale_id(), "ru");
    assert_eq!(window.get_quota_targets().row_count(), 3);
    assert_eq!(window.get_chart_points().row_count(), 120);
    assert_eq!(window.get_session_rows().row_count(), 256);

    window.invoke_switch_locale(SharedString::from("en"));
    assert_eq!(window.get_locale_id(), "en");
}
