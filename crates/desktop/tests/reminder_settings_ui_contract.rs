use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use i_slint_backend_testing::{AccessibleRole, ElementQuery};
use slint::{ComponentHandle, Model, SharedString};
use tokenmaster_desktop::{
    DesktopBackupPolicy, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopReliableStateHealth, DesktopReliableStateInput, DesktopReliableStateProjection,
    DesktopReliableStateSummary, DesktopReminderPolicy, DesktopReminderSyncState, DesktopShell,
};
use tokenmaster_product::ProductReducer;

struct RecordingSink {
    intents: RefCell<Vec<DesktopIntent>>,
    admission: Cell<DesktopIntentAdmission>,
}

impl Default for RecordingSink {
    fn default() -> Self {
        Self {
            intents: RefCell::new(Vec::new()),
            admission: Cell::new(DesktopIntentAdmission::Started),
        }
    }
}

impl DesktopIntentSink for RecordingSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        self.intents.borrow_mut().push(intent);
        self.admission.get()
    }
}

fn reliable_state(leads: &[u32]) -> DesktopReliableStateProjection {
    let reminder = DesktopReminderPolicy::new(true, leads, DesktopReminderSyncState::Synchronized)
        .expect("reminder policy");
    DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        1,
        DesktopReliableStateSummary::new_with_reminder_policy(
            DesktopReliableStateHealth::Healthy,
            false,
            "healthy",
            DesktopBackupPolicy::disabled(),
            reminder,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        Vec::new(),
    ))
}

#[test]
fn reliable_reminder_policy_projects_into_the_bounded_editor() {
    i_slint_backend_testing::init_no_event_loop();
    let snapshot = ProductReducer::new().snapshot();
    let sink = Rc::new(RecordingSink::default());
    let shell = DesktopShell::new_with_reliable_state(
        &snapshot,
        reliable_state(&[604_800, 10_800, 90]),
        sink.clone(),
    )
    .expect("desktop shell");
    let window = shell.window();

    assert!(window.get_reminder_preset_seven_days());
    assert_eq!(window.get_reminder_sync_state(), "Synchronized");
    assert!(!window.get_reminder_dirty());
    let rows = window.get_reminder_custom_lead_rows();
    assert_eq!(rows.row_count(), 8);
    assert_eq!(rows.row_data(0).expect("first custom row").value, 3);
    assert_eq!(rows.row_data(0).expect("first custom row").unit_index, 2);
    assert_eq!(rows.row_data(1).expect("second custom row").value, 90);
    assert_eq!(rows.row_data(1).expect("second custom row").unit_index, 0);

    window.invoke_reminder_custom_lead_edited(2, true, 2, 1);
    window.invoke_save_reminder_policy();
    let intents = sink.intents.borrow();
    assert_eq!(intents.len(), 1);
    let DesktopIntent::UpdateReminderPolicy(update) = &intents[0] else {
        panic!("reminder intent")
    };
    assert!(update.enabled());
    assert_eq!(update.lead_seconds(), &[604_800, 10_800, 120, 90]);
    drop(intents);
    assert!(!window.get_reminder_dirty());
    assert_eq!(window.get_reminder_feedback(), "Reminder profile submitted");

    window.invoke_reminder_custom_lead_edited(3, true, 2, 1);
    window.invoke_reminder_custom_lead_edited(4, true, 2, 1);
    window.invoke_save_reminder_policy();
    assert_eq!(sink.intents.borrow().len(), 2);
    let intents = sink.intents.borrow();
    let DesktopIntent::UpdateReminderPolicy(update) = &intents[1] else {
        panic!("deduplicated reminder intent")
    };
    assert_eq!(update.lead_seconds(), &[604_800, 10_800, 120, 90]);
    drop(intents);

    window.invoke_reminder_custom_lead_edited(5, true, i32::MAX, 3);
    window.invoke_save_reminder_policy();
    assert_eq!(sink.intents.borrow().len(), 2);
    assert_eq!(
        window.get_reminder_feedback(),
        "Reminder profile is invalid"
    );
    window.invoke_reminder_custom_lead_edited(5, true, 1, 4);
    window.invoke_save_reminder_policy();
    assert_eq!(sink.intents.borrow().len(), 2);
    window.invoke_reset_reminder_recommended();
    for (index, value) in (2..=5).enumerate() {
        window.invoke_reminder_custom_lead_edited(index as i32, true, value, 0);
    }
    window.invoke_save_reminder_policy();
    assert_eq!(sink.intents.borrow().len(), 2);
    window.invoke_reminder_custom_lead_edited(5, false, 1, 0);
    window.invoke_reminder_enabled_edited(false);
    window.invoke_reminder_preset_edited(0, false);
    window.invoke_reminder_preset_edited(1, false);
    window.invoke_reminder_preset_edited(2, false);
    window.invoke_reminder_preset_edited(3, false);
    window.invoke_reminder_preset_edited(4, false);
    for index in 0..5 {
        window.invoke_reminder_custom_lead_edited(index, false, 1, 0);
    }
    window.invoke_save_reminder_policy();
    assert_eq!(sink.intents.borrow().len(), 3);
    {
        let intents = sink.intents.borrow();
        let DesktopIntent::UpdateReminderPolicy(update) = &intents[2] else {
            panic!("disabled reminder intent")
        };
        assert!(!update.enabled());
        assert!(update.lead_seconds().is_empty());
    }

    sink.admission.set(DesktopIntentAdmission::Rejected);
    window.invoke_reset_reminder_recommended();
    let draft = window
        .get_reminder_custom_lead_rows()
        .row_data(0)
        .expect("reset row");
    window.invoke_save_reminder_policy();
    assert!(window.get_reminder_dirty());
    assert_eq!(window.get_reminder_feedback(), "Reminder service is busy");
    assert_eq!(
        window
            .get_reminder_custom_lead_rows()
            .row_data(0)
            .expect("retained row"),
        draft
    );

    let replacement = reliable_state(&[3_600]);
    shell
        .apply_reliable_state(replacement)
        .expect("publish while dirty");
    assert!(window.get_reminder_preset_seven_days());
    sink.admission.set(DesktopIntentAdmission::Queued);
    window.invoke_save_reminder_policy();
    assert!(!window.get_reminder_dirty());
    shell
        .apply_reliable_state(reliable_state(&[3_600]))
        .expect("publish after acceptance");
    assert!(window.get_reminder_preset_one_hour());
    assert!(!window.get_reminder_preset_seven_days());

    window.invoke_select_route(SharedString::from("settings"));
    window.window().set_size(slint::PhysicalSize::new(700, 900));
    assert_eq!(window.get_settings_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 900));
    assert_eq!(window.get_settings_layout_mode(), "wide");
    window.show().expect("show settings");
    let labels = ElementQuery::from_root(window)
        .match_predicate(|element| element.accessible_label().is_some())
        .find_all()
        .into_iter()
        .filter_map(|element| element.accessible_label())
        .collect::<Vec<_>>();
    for label in [
        "Enable expiry reminders",
        "Save reminder profile",
        "Reset reminder profile to recommended",
    ] {
        assert!(
            labels.iter().any(|candidate| candidate.contains(label)),
            "missing label: {label}"
        );
    }
    assert!(
        !ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Button)
            .find_all()
            .is_empty()
    );
}
