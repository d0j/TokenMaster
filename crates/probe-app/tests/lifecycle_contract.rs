use tokenmaster_m0::lifecycle::{Action, Lifecycle, Visibility};
use tokenmaster_m0::shell::RendererChoice;

#[test]
fn close_hides_without_destroying_window() {
    let mut lifecycle = Lifecycle::default();
    assert_eq!(lifecycle.apply(Action::CloseRequested), Visibility::Hidden);
    assert!(!lifecycle.quit_requested());
    assert_eq!(lifecycle.window_generation(), 1);
    assert_eq!(lifecycle.apply(Action::Show), Visibility::Visible);
    assert_eq!(lifecycle.window_generation(), 1);
}

#[test]
fn only_explicit_quit_ends_the_loop() {
    let mut lifecycle = Lifecycle::default();
    lifecycle.apply(Action::Hide);
    assert!(!lifecycle.quit_requested());
    lifecycle.apply(Action::Quit);
    assert!(lifecycle.quit_requested());
}

#[test]
fn renderer_choices_map_to_explicit_winit_backends() {
    assert_eq!(RendererChoice::FemtoVg.backend_name(), "winit-femtovg");
    assert_eq!(RendererChoice::Software.backend_name(), "winit-software");
}

#[test]
fn renderer_override_is_bounded_and_fail_closed() {
    assert_eq!(
        RendererChoice::from_override(None).expect("default"),
        RendererChoice::Software
    );
    assert_eq!(
        RendererChoice::from_override(Some("software")).expect("software"),
        RendererChoice::Software
    );
    assert!(RendererChoice::from_override(Some("unknown")).is_err());
}
