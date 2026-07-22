use std::cell::Cell;
use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{ComponentHandle, Model, SharedString};
use tokenmaster_desktop::{
    DesktopIntent, DesktopIntentAdmission, DesktopIntentSink, DesktopPresentationSelection,
    DesktopReliableStateProjection, DesktopShell,
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

fn shell() -> (DesktopShell, Rc<RecordingSink>) {
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
    (shell, sink)
}

fn visible_slot_keys(window: &tokenmaster_desktop::MainWindow) -> Vec<String> {
    let slots = window.get_dashboard_board_visible_slots();
    (0..slots.row_count())
        .map(|index| {
            slots
                .row_data(index)
                .expect("visible board slot")
                .key
                .to_string()
        })
        .collect()
}

fn card_geometry(window: &tokenmaster_desktop::MainWindow, label: &str) -> (f32, f32) {
    let label = label.to_owned();
    let element = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Groupbox)
        .match_predicate(move |element| {
            element.accessible_label().as_deref() == Some(label.as_str())
        })
        .find_all()
        .into_iter()
        .next()
        .expect("visible Dashboard card");
    let position = element.absolute_position();
    (position.x, position.y)
}

#[test]
fn board_editor_exposes_six_accessible_rows_and_canonical_visible_slots() {
    let (shell, _) = shell();
    let window = shell.window();
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_400, 3_000));
    window.invoke_select_route(SharedString::from("settings"));

    assert_eq!(window.get_dashboard_board_editor_rows().row_count(), 6);
    assert_eq!(window.get_dashboard_board_visible_slots().row_count(), 6);
    assert_eq!(
        ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Groupbox)
            .match_predicate(|element| {
                element.accessible_label().as_deref() == Some("Dashboard board")
            })
            .find_all()
            .len(),
        1
    );
    assert_eq!(
        ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Groupbox)
            .match_predicate(|element| {
                element.accessible_label().as_deref() == Some("Dashboard board section Plan Usage")
            })
            .find_all()
            .len(),
        1,
        "the compiled Settings board row must retain its dynamic label through the localized format key"
    );
}

#[test]
fn board_callbacks_submit_complete_preferences_and_compact_visible_slots() {
    let (shell, sink) = shell();
    let window = shell.window();

    window.invoke_move_dashboard_board_row(0, 1);
    let moved = sink.selection.get().expect("board move selection");
    assert_eq!(moved.board().rows()[0].key().stable_key(), "code_output");
    assert_eq!(window.get_dashboard_board_visible_slots().row_count(), 6);

    window.invoke_set_dashboard_board_row_visible(0, false);
    assert_eq!(sink.submissions.get(), 2);
    assert_eq!(window.get_dashboard_board_visible_slots().row_count(), 5);
    assert!(
        !sink
            .selection
            .get()
            .expect("hidden selection")
            .board()
            .rows()[0]
            .visible()
    );

    window.invoke_set_dashboard_board_row_collapsed(1, true);
    assert!(
        sink.selection
            .get()
            .expect("collapsed selection")
            .board()
            .rows()[1]
            .collapsed()
    );
    window.invoke_select_route(SharedString::from("dashboard"));
    assert_eq!(
        ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Groupbox)
            .match_predicate(|element| {
                element.accessible_label().as_deref() == Some("Plan Usage collapsed")
            })
            .find_all()
            .len(),
        1
    );
}

#[test]
fn board_callbacks_reject_invalid_and_last_visible_edits_then_reset_only_board() {
    let (shell, sink) = shell();
    let window = shell.window();

    window.invoke_move_dashboard_board_row(-1, 1);
    window.invoke_move_dashboard_board_row(0, 2);
    assert_eq!(sink.submissions.get(), 0);

    for index in 0..5 {
        window.invoke_set_dashboard_board_row_visible(index, false);
    }
    let before_last_visible = sink.submissions.get();
    window.invoke_set_dashboard_board_row_visible(5, false);
    assert_eq!(sink.submissions.get(), before_last_visible);
    assert_eq!(window.get_dashboard_board_visible_slots().row_count(), 1);

    window.invoke_reset_dashboard_board();
    let reset = sink.selection.get().expect("reset selection");
    assert_eq!(reset.board().rows()[0].key().stable_key(), "plan_usage");
    assert!(
        reset
            .board()
            .rows()
            .iter()
            .all(|row| row.visible() && !row.collapsed())
    );
    assert_eq!(window.get_dashboard_board_visible_slots().row_count(), 6);
}

#[test]
fn ten_thousand_board_edits_reuse_the_window_and_bounded_models() {
    let (shell, sink) = shell();
    let window = shell.window();
    let address = window as *const _;
    let editor_rows = window.get_dashboard_board_editor_rows().row_count();
    let source_sections = window.get_dashboard_section_rows().row_count();

    for index in 0..10_000 {
        window.invoke_set_dashboard_board_row_collapsed(0, index % 2 == 0);
    }

    assert_eq!(address, shell.window() as *const _);
    assert_eq!(
        window.get_dashboard_board_editor_rows().row_count(),
        editor_rows
    );
    assert_eq!(
        window.get_dashboard_section_rows().row_count(),
        source_sections
    );
    assert_eq!(window.get_dashboard_board_visible_slots().row_count(), 6);
    assert_eq!(sink.submissions.get(), 10_000);
}

#[test]
fn workbench_uses_compatibility_order_only_for_canonical_board_and_compacts_custom_hidden_rows() {
    let (shell, sink) = shell();
    let window = shell.window();
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_400, 900));
    window.invoke_select_presentation_layout(2);

    assert_eq!(
        visible_slot_keys(window),
        [
            "plan_usage",
            "code_output",
            "sessions",
            "trend",
            "models",
            "activity",
        ]
    );
    let code = card_geometry(window, "Code Output");
    let sessions = card_geometry(window, "Sessions");
    let trend = card_geometry(window, "Usage and Cost Trend");
    let models = card_geometry(window, "Model Usage");
    assert_eq!(code.1, sessions.1);
    assert!(sessions.0 > code.0);
    assert_eq!(trend.1, models.1);
    assert!(models.0 > trend.0);

    window.invoke_move_dashboard_board_row(0, 1);
    let custom = sink.selection.get().expect("custom board selection");
    let stored_custom_keys = custom
        .board()
        .rows()
        .iter()
        .map(|row| row.key().stable_key().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(visible_slot_keys(window), stored_custom_keys);
    assert!(
        card_geometry(window, "Code Output").1 < card_geometry(window, "Plan Usage").1,
        "custom ordinal slot zero is the first Workbench card"
    );

    window.invoke_set_dashboard_board_row_visible(0, false);
    assert_eq!(
        visible_slot_keys(window),
        ["plan_usage", "trend", "sessions", "activity", "models"]
    );
    assert_eq!(window.get_dashboard_board_visible_slots().row_count(), 5);
}
