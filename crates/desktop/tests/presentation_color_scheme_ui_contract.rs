use std::cell::Cell;
use std::rc::Rc;

use tokenmaster_desktop::{
    DesktopColorScheme, DesktopDensity, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopPresentationSelection, DesktopReliableStateProjection, DesktopShell, DesktopSkin,
};
use tokenmaster_product::ProductReducer;

struct RecordingSink {
    submissions: Cell<u8>,
    selection: Cell<Option<DesktopPresentationSelection>>,
}

impl DesktopIntentSink for RecordingSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        let DesktopIntent::UpdatePresentation(selection) = intent else {
            return DesktopIntentAdmission::Rejected;
        };
        self.submissions.set(self.submissions.get() + 1);
        self.selection.set(Some(selection));
        DesktopIntentAdmission::Started
    }
}

#[test]
fn system_observation_repaints_without_persistence_and_selector_submits_complete_triple() {
    i_slint_backend_testing::init_no_event_loop();
    let sink = Rc::new(RecordingSink {
        submissions: Cell::new(0),
        selection: Cell::new(None),
    });
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        sink.clone(),
    )
    .expect("desktop shell");
    let window = shell.window();
    assert_eq!(window.get_presentation_color_scheme_key(), "system");
    assert_eq!(window.get_presentation_effective_color_scheme_key(), "dark");
    let revision = window.get_presentation_revision();

    window.invoke_system_color_scheme_observed(1);

    assert_eq!(window.get_presentation_color_scheme_key(), "system");
    assert_eq!(
        window.get_presentation_effective_color_scheme_key(),
        "light"
    );
    assert_eq!(window.get_presentation_revision(), revision);
    assert_eq!(window.get_presentation_persistence_state(), "saved");
    assert_eq!(sink.submissions.get(), 0);

    window.invoke_select_presentation_color_scheme(2);

    assert_eq!(sink.submissions.get(), 1);
    assert_eq!(
        sink.selection.get(),
        Some(DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::Dark,
        ))
    );
    assert_eq!(window.get_presentation_color_scheme_key(), "dark");
    assert_eq!(window.get_presentation_effective_color_scheme_key(), "dark");
}

#[test]
fn compiled_ui_owns_one_three_entry_selector_and_reactive_slint_observation() {
    let main = include_str!("../ui/main.slint");
    let settings = include_str!("../ui/views/settings-view.slint");

    assert!(main.contains("Palette.color-scheme"));
    assert!(main.contains("changed observed-system-color-scheme-id"));
    assert_eq!(
        main.matches("callback system-color-scheme-observed(int);")
            .count(),
        1
    );
    assert_eq!(
        main.matches("callback select-presentation-color-scheme(int);")
            .count(),
        1
    );
    assert_eq!(
        settings
            .matches("model: [\"System\", \"Light\", \"Dark\"];")
            .count(),
        1
    );
    assert_eq!(
        settings
            .matches("accessible-label: \"Presentation color scheme\";")
            .count(),
        1
    );
}
