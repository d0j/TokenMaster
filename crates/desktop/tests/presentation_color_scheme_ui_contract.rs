use std::cell::Cell;
use std::rc::Rc;

use slint::{ComponentHandle, Model};
use tokenmaster_desktop::{
    DesktopColorScheme, DesktopDensity, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopPresentationSelection, DesktopReliableStateProjection, DesktopShell, DesktopSkin,
};
use tokenmaster_product::ProductReducer;

struct RecordingSink {
    submissions: Cell<u32>,
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
            tokenmaster_desktop::DesktopLayout::Refined,
            tokenmaster_desktop::DesktopLocale::English,
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
            .matches("model: [@tr(\"System\"), @tr(\"Light\"), @tr(\"Dark\")];")
            .count(),
        1
    );
    assert_eq!(
        settings
            .matches("accessible-label: @tr(\"Presentation color scheme\");")
            .count(),
        1
    );
}

#[test]
fn ten_thousand_compiled_ui_switches_cover_all_eighty_one_complete_combinations() {
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
    let address = window as *const _;
    let routes = window.get_route_rows().row_count();
    let quotas = window.get_dashboard_quota_rows().row_count();
    let size = window.window().size();
    window.invoke_select_presentation_color_scheme(2);

    for index in 0..10_000 {
        let density_index = index % 3;
        let skin_index = (index / 3) % 3;
        let scheme_index = (index / 9) % 3;
        let layout_index = (index / 27) % 3;
        window.invoke_select_presentation_density(density_index);
        window.invoke_select_presentation_skin(skin_index);
        window.invoke_select_presentation_color_scheme(scheme_index);
        window.invoke_select_presentation_layout(layout_index);

        assert_eq!(
            sink.selection.get(),
            Some(DesktopPresentationSelection::new(
                match density_index {
                    0 => DesktopDensity::Comfortable,
                    1 => DesktopDensity::Compact,
                    _ => DesktopDensity::UltraCompact,
                },
                match skin_index {
                    0 => DesktopSkin::Refined,
                    1 => DesktopSkin::Graphite,
                    _ => DesktopSkin::Ember,
                },
                match scheme_index {
                    0 => DesktopColorScheme::System,
                    1 => DesktopColorScheme::Light,
                    _ => DesktopColorScheme::Dark,
                },
                match layout_index {
                    0 => tokenmaster_desktop::DesktopLayout::Refined,
                    1 => tokenmaster_desktop::DesktopLayout::ControlCenter,
                    _ => tokenmaster_desktop::DesktopLayout::Workbench,
                },
                tokenmaster_desktop::DesktopLocale::English,
            )),
            "compiled UI submission {index} must retain one complete quadruple"
        );
    }

    assert_eq!(address, shell.window() as *const _);
    assert_eq!(window.get_route_rows().row_count(), routes);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), quotas);
    assert_eq!(window.window().size(), size);
    assert!(sink.submissions.get() >= 10_001);
    assert!(sink.submissions.get() <= 40_001);
}
