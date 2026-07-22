use std::cell::Cell;
use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{ComponentHandle, Model};
use tokenmaster_desktop::{
    DesktopColorScheme, DesktopDensity, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopLayout, DesktopPresentationSelection, DesktopReliableStateProjection, DesktopShell,
    DesktopSkin,
};
use tokenmaster_product::ProductReducer;

struct RecordingSink {
    submissions: Cell<u32>,
    selection: Cell<Option<DesktopPresentationSelection>>,
}

fn card_x(window: &tokenmaster_desktop::MainWindow, label: &str) -> f32 {
    let label = label.to_owned();
    ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Groupbox)
        .match_predicate(move |element| {
            element.accessible_label().as_deref() == Some(label.as_str())
        })
        .find_all()
        .into_iter()
        .next()
        .expect("visible Dashboard card")
        .absolute_position()
        .x
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
fn layout_selector_submits_the_complete_selection_and_changes_wide_geometry() {
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
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_400, 900));

    assert_eq!(window.get_presentation_layout_key(), "refined");
    assert_eq!(window.get_dashboard_layout_preset(), "refined");
    assert!(card_x(window, "Usage and Cost Trend") > card_x(window, "Code Output"));
    assert_eq!(card_x(window, "Sessions"), card_x(window, "Code Output"));

    window.invoke_select_presentation_layout(1);
    assert_eq!(
        sink.selection.get(),
        Some(DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            DesktopColorScheme::System,
            DesktopLayout::ControlCenter,
        ))
    );
    assert_eq!(window.get_presentation_layout_key(), "control_center");
    assert_eq!(window.get_dashboard_layout_preset(), "control_center");
    assert_eq!(
        card_x(window, "Usage and Cost Trend"),
        card_x(window, "Plan Usage")
    );
    assert_eq!(card_x(window, "Sessions"), card_x(window, "Plan Usage"));

    window.invoke_select_presentation_layout(2);
    assert_eq!(window.get_dashboard_layout_preset(), "workbench");
    assert!(card_x(window, "Usage and Cost Trend") > card_x(window, "Code Output"));
    assert!(card_x(window, "Sessions") > card_x(window, "Usage and Cost Trend"));

    let selection = sink.selection.get();
    let revision = window.get_presentation_revision();
    window.invoke_select_presentation_layout(2);
    window.invoke_select_presentation_layout(9);
    assert_eq!(sink.selection.get(), selection);
    assert_eq!(window.get_presentation_revision(), revision);
}

#[test]
fn narrow_width_retains_selected_layout_but_uses_safe_single_column_geometry() {
    i_slint_backend_testing::init_no_event_loop();
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        Rc::new(RecordingSink {
            submissions: Cell::new(0),
            selection: Cell::new(None),
        }),
    )
    .expect("desktop shell");
    let window = shell.window();
    window.invoke_select_presentation_layout(2);
    window.window().set_size(slint::PhysicalSize::new(700, 900));

    assert_eq!(window.get_presentation_layout_key(), "workbench");
    assert_eq!(window.get_dashboard_layout_preset(), "workbench");
    assert_eq!(window.get_dashboard_layout_mode(), "narrow");
    assert_eq!(
        card_x(window, "Usage and Cost Trend"),
        card_x(window, "Plan Usage")
    );
    assert_eq!(card_x(window, "Sessions"), card_x(window, "Plan Usage"));
}

#[test]
fn ten_thousand_layout_switches_reuse_the_same_window_routes_models_and_size() {
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
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_400, 900));
    let address = window as *const _;
    let routes = window.get_route_rows().row_count();
    let quotas = window.get_dashboard_quota_rows().row_count();
    let size = window.window().size();

    for index in 0..10_000 {
        window.invoke_select_presentation_layout(index % 3);
    }

    assert_eq!(address, shell.window() as *const _);
    assert_eq!(window.get_route_rows().row_count(), routes);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), quotas);
    assert_eq!(window.window().size(), size);
    assert_eq!(window.get_presentation_layout_key(), "refined");
    assert_eq!(sink.submissions.get(), 9_999);
}

#[test]
fn source_owns_one_three_entry_selector_and_three_dashboard_layout_branches() {
    let main = include_str!("../ui/main.slint");
    let settings = include_str!("../ui/views/settings-view.slint");
    let dashboard = include_str!("../ui/views/dashboard-view.slint");

    assert_eq!(
        main.matches("callback select-presentation-layout(int);")
            .count(),
        1
    );
    assert_eq!(
        settings
            .matches("model: [\"Refined\", \"Control center\", \"Workbench\"];")
            .count(),
        1
    );
    assert_eq!(
        settings
            .matches("accessible-label: \"Presentation layout\";")
            .count(),
        1
    );
    for branch in [
        "root.layout-id == 0",
        "root.layout-id == 1",
        "root.layout-id == 2",
    ] {
        assert!(dashboard.contains(branch));
    }
}
