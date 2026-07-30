//! `AGENTS.md` binds the product to one invariant above the others: unavailable, partial
//! and legitimate zero are three different facts and must stay distinguishable everywhere
//! they are shown -- text, chart, accessible label. The table cells are guarded elsewhere.
//! These tests guard the chart, where a day with no evidence and a day that truly cost
//! nothing both have a token ratio of 0.0 and would otherwise draw the same bar.

use std::cell::Cell;
use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokenmaster_desktop::{
    DesktopIntent, DesktopIntentAdmission, DesktopIntentSink, DesktopReliableStateProjection,
    DesktopShell, HistoryDayRow, MainWindow,
};
use tokenmaster_product::ProductReducer;

struct SilentSink {
    submissions: Cell<u32>,
}

impl DesktopIntentSink for SilentSink {
    fn submit(&self, _intent: DesktopIntent) -> DesktopIntentAdmission {
        self.submissions.set(self.submissions.get() + 1);
        DesktopIntentAdmission::Started
    }
}

fn day(date: &str, availability: &str, total: &str, ratio: f32) -> HistoryDayRow {
    let cell = |value: &str| SharedString::from(value);
    HistoryDayRow {
        date_label: cell(date),
        event_label: cell("1"),
        input_availability: cell(availability),
        input_label: cell(total),
        cached_availability: cell(availability),
        cached_label: cell(total),
        output_availability: cell(availability),
        output_label: cell(total),
        reasoning_availability: cell(availability),
        reasoning_label: cell(total),
        total_availability: cell(availability),
        total_label: cell(total),
        cost_availability: cell(availability),
        cost_label: cell(total),
        token_ratio: ratio,
        cost_ratio: ratio,
    }
}

fn history_window() -> DesktopShell {
    i_slint_backend_testing::init_no_event_loop();
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        Rc::new(SilentSink {
            submissions: Cell::new(0),
        }),
    )
    .expect("desktop shell");
    let window = shell.window();
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_400, 1_600));
    window.invoke_select_route("history".into());
    window.set_history_day_rows(ModelRc::new(VecModel::from(vec![
        // No evidence for the day at all.
        day("Mon 01", "unavailable", "—", 0.0),
        // Evidence exists and the day genuinely cost nothing.
        day("Tue 02", "available", "0", 0.0),
        // Evidence exists and the day is the tallest in the window.
        day("Wed 03", "available", "1200", 1.0),
    ])));
    shell
}

fn bar(window: &MainWindow, label: &str) -> ElementHandle {
    let wanted = label.to_owned();
    let found = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Text)
        .match_predicate(move |element| element.accessible_label().as_deref() == Some(&wanted))
        .find_all();
    assert_eq!(found.len(), 1, "expected exactly one bar labelled {label}");
    found.into_iter().next().expect("bar")
}

#[test]
fn every_column_carries_its_own_accessible_label() {
    let shell = history_window();
    let window = shell.window();

    // The card keeps its own summary label; the columns add to it rather than replace it.
    assert_eq!(
        ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Image)
            .match_predicate(
                |element| element.accessible_label().as_deref() == Some("Thirty day token trend")
            )
            .find_all()
            .len(),
        1,
        "the trend card must stay one labelled image"
    );

    for label in ["Mon 01, —", "Tue 02, 0", "Wed 03, 1200"] {
        bar(window, label);
    }
}

#[test]
fn an_unavailable_day_and_a_legitimate_zero_do_not_draw_the_same_bar() {
    let shell = history_window();
    let window = shell.window();

    let unavailable = bar(window, "Mon 01, —").size().height;
    let legitimate_zero = bar(window, "Tue 02, 0").size().height;
    let tallest = bar(window, "Wed 03, 1200").size().height;

    assert_ne!(
        unavailable, legitimate_zero,
        "a day with no evidence must not render as a day that cost nothing"
    );
    assert!(
        legitimate_zero < unavailable,
        "the unavailable marker must stand above the zero baseline, \
         got zero={legitimate_zero} unavailable={unavailable}"
    );
    assert!(
        tallest > unavailable,
        "a real value must outgrow the unavailable marker, \
         got tallest={tallest} unavailable={unavailable}"
    );
}
