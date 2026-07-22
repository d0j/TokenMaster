use std::cell::Cell;
use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{Model, SharedString};
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

#[test]
fn board_editor_exposes_six_accessible_rows_and_canonical_visible_slots() {
    let (shell, _) = shell();
    let window = shell.window();
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
