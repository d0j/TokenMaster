use std::collections::HashSet;

use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};
use slint::ComponentHandle;
use tokenmaster_desktop::DesktopShell;
use tokenmaster_product::ProductReducer;

#[test]
fn startup_dashboard_software_frame_is_visible_and_non_uniform() {
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
        mock_time: false,
        threading: true,
        renderer_name: Some("software".into()),
    })))
    .expect("install software-rendered testing backend");

    let snapshot = ProductReducer::new().snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let window = shell.window();
    window.set_dashboard_startup_import_in_progress(true);
    window.set_dashboard_initial_import_stage(2);
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_288, 720));
    window.show().expect("show headless startup window");

    let frame = window
        .window()
        .take_snapshot()
        .expect("paint startup dashboard");
    let pixels = frame.as_bytes();
    let distinct = pixels
        .chunks_exact(4)
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect::<HashSet<_>>();

    assert!(
        distinct.len() >= 8,
        "startup frame must contain visible UI colors, got {}",
        distinct.len()
    );
    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0),
        "startup frame must not be fully black"
    );
}
