use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::{ComponentHandle, Model, SharedString};
use tokenmaster_desktop::{
    DesktopBackupPolicy, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopReliableStateHealth, DesktopReliableStateInput, DesktopReliableStateProjection,
    DesktopReliableStateSummary, DesktopReminderPolicy, DesktopReminderSyncState, DesktopShell,
    MainWindow, ReminderCustomLeadRow,
};
use tokenmaster_product::ProductReducer;

struct RecordingSink {
    intents: RefCell<Vec<DesktopIntent>>,
    admission: Cell<DesktopIntentAdmission>,
}

#[derive(Debug, PartialEq)]
struct ReminderDraftSnapshot {
    enabled: bool,
    presets: [bool; 5],
    rows: Vec<ReminderCustomLeadRow>,
}

fn draft_snapshot(window: &MainWindow) -> ReminderDraftSnapshot {
    let rows = window.get_reminder_custom_lead_rows();
    ReminderDraftSnapshot {
        enabled: window.get_reminder_enabled(),
        presets: [
            window.get_reminder_preset_seven_days(),
            window.get_reminder_preset_twenty_four_hours(),
            window.get_reminder_preset_twelve_hours(),
            window.get_reminder_preset_six_hours(),
            window.get_reminder_preset_one_hour(),
        ],
        rows: (0..rows.row_count())
            .map(|index| rows.row_data(index).expect("draft row"))
            .collect(),
    }
}

fn assert_exact_accessible_role(window: &MainWindow, label: &str, role: AccessibleRole) {
    assert_eq!(
        ElementHandle::find_by_accessible_label(window, label)
            .filter(|element| element.accessible_role() == Some(role))
            .count(),
        1,
        "exact accessible role for {label}"
    );
}

fn backup_policy_intents(sink: &RecordingSink) -> Vec<(bool, u32, u32, u32)> {
    sink.intents
        .borrow()
        .iter()
        .filter_map(|intent| match intent {
            DesktopIntent::UpdateBackupPolicy {
                periodic_enabled,
                quiet_seconds,
                interval_seconds,
                retention_budget_mib,
            } => Some((
                *periodic_enabled,
                *quiet_seconds,
                *interval_seconds,
                *retention_budget_mib,
            )),
            _ => None,
        })
        .collect()
}

fn set_backup_value(window: &MainWindow, label: &str, value: u32) {
    ElementHandle::find_by_accessible_label(window, label)
        .find(|element| element.accessible_role() == Some(AccessibleRole::Spinbox))
        .expect("backup numeric control")
        .set_accessible_value(value.to_string());
}

fn backup_value(window: &MainWindow, label: &str) -> i32 {
    ElementHandle::find_by_accessible_label(window, label)
        .find(|element| element.accessible_role() == Some(AccessibleRole::Spinbox))
        .expect("backup numeric control")
        .accessible_value()
        .expect("backup accessible value")
        .parse()
        .expect("backup integer value")
}

fn save_backup_policy(window: &MainWindow, quiet: i32, interval: i32, budget: i32) {
    window.invoke_update_backup_policy(true, quiet, interval, budget);
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
    reliable_state_with_sync(leads, DesktopReminderSyncState::Synchronized)
}

fn reliable_state_with_sync(
    leads: &[u32],
    sync_state: DesktopReminderSyncState,
) -> DesktopReliableStateProjection {
    let reminder = DesktopReminderPolicy::new(true, leads, sync_state).expect("reminder policy");
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
    for (index, value) in [61, 62, 63, 64].into_iter().enumerate() {
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
    let reset_before = sink.intents.borrow().len();
    window.invoke_reset_reminder_recommended();
    assert_eq!(sink.intents.borrow().len(), reset_before);
    assert!(window.get_reminder_dirty());
    assert!(window.get_reminder_enabled());
    assert!(window.get_reminder_preset_seven_days());
    assert!(window.get_reminder_preset_twenty_four_hours());
    assert!(window.get_reminder_preset_twelve_hours());
    assert!(window.get_reminder_preset_six_hours());
    assert!(window.get_reminder_preset_one_hour());
    let draft_rows = (0..window.get_reminder_custom_lead_rows().row_count())
        .map(|index| {
            window
                .get_reminder_custom_lead_rows()
                .row_data(index)
                .expect("reset row")
        })
        .collect::<Vec<_>>();
    assert!(
        draft_rows
            .iter()
            .all(|row| !row.enabled && row.value == 1 && row.unit_index == 0)
    );
    window.invoke_reminder_preset_edited(0, false);
    window.invoke_reminder_custom_lead_edited(0, true, 17, 2);
    let draft = draft_snapshot(window);
    window.invoke_save_reminder_policy();
    assert!(window.get_reminder_dirty());
    assert_eq!(window.get_reminder_feedback(), "Reminder service is busy");
    assert_eq!(draft_snapshot(window), draft);

    shell
        .apply_reliable_state(reliable_state_with_sync(
            &[3_600],
            DesktopReminderSyncState::Pending,
        ))
        .expect("publish while dirty");
    assert_eq!(draft_snapshot(window), draft);
    assert_eq!(window.get_reminder_sync_state(), "Pending");
    shell
        .apply_reliable_state(DesktopReliableStateProjection::unavailable())
        .expect("unavailable projection while dirty");
    assert_eq!(draft_snapshot(window), draft);
    assert_eq!(window.get_reminder_sync_state(), "Unavailable");
    sink.admission.set(DesktopIntentAdmission::Queued);
    let queued_before = sink.intents.borrow().len();
    window.invoke_save_reminder_policy();
    assert_eq!(sink.intents.borrow().len(), queued_before + 1);
    assert!(!window.get_reminder_dirty());
    shell
        .apply_reliable_state(reliable_state(&[3_600]))
        .expect("publish after acceptance");
    assert!(window.get_reminder_preset_one_hour());
    assert!(!window.get_reminder_preset_seven_days());

    sink.admission.set(DesktopIntentAdmission::Coalesced);
    window.invoke_reset_reminder_recommended();
    let coalesced_before = sink.intents.borrow().len();
    window.invoke_save_reminder_policy();
    assert_eq!(sink.intents.borrow().len(), coalesced_before + 1);
    assert!(!window.get_reminder_dirty());

    window.invoke_select_route(SharedString::from("settings"));
    window.window().set_size(slint::PhysicalSize::new(700, 900));
    assert_eq!(window.get_settings_layout_mode(), "narrow");
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 1_000));
    assert_eq!(window.get_settings_layout_mode(), "wide");
    window.show().expect("show settings");
    let labels = ElementQuery::from_root(window)
        .match_predicate(|element| element.accessible_label().is_some())
        .find_all()
        .into_iter()
        .filter_map(|element| element.accessible_label())
        .collect::<Vec<_>>();
    let mut unique_labels = labels.clone();
    unique_labels.sort();
    unique_labels.dedup();
    for label in [
        "Enable expiry reminders",
        "Save reminder profile",
        "Reset reminder profile to recommended",
    ] {
        assert!(
            unique_labels
                .iter()
                .any(|candidate| candidate.contains(label)),
            "missing label: {label}"
        );
    }
    for (label, expected) in [
        ("Enable expiry reminders", 1),
        ("Reminder lead time", 5),
        ("Enable custom reminder lead row", 8),
        ("Custom reminder lead value row", 8),
        ("Custom reminder lead unit row", 8),
        ("Save reminder profile", 1),
        ("Reset reminder profile to recommended", 1),
        ("Reminder editor feedback", 1),
        ("Reminder synchronization state", 1),
    ] {
        assert_eq!(
            unique_labels
                .iter()
                .filter(|candidate| candidate.contains(label))
                .count(),
            expected,
            "accessible label count for {label}"
        );
    }
    for index in 1..=8 {
        for label in [
            format!("Enable custom reminder lead row {index}"),
            format!("Custom reminder lead value row {index}"),
            format!("Custom reminder lead unit row {index}"),
        ] {
            assert!(
                unique_labels.iter().any(|candidate| candidate == &label),
                "missing label: {label}"
            );
        }
    }
    assert_exact_accessible_role(window, "Enable expiry reminders", AccessibleRole::Checkbox);
    for label in [
        "Reminder lead time 7 days",
        "Reminder lead time 24 hours",
        "Reminder lead time 12 hours",
        "Reminder lead time 6 hours",
        "Reminder lead time 1 hour",
    ] {
        assert_exact_accessible_role(window, label, AccessibleRole::Checkbox);
    }
    for index in 1..=8 {
        assert_exact_accessible_role(
            window,
            &format!("Enable custom reminder lead row {index}"),
            AccessibleRole::Checkbox,
        );
        assert_exact_accessible_role(
            window,
            &format!("Custom reminder lead value row {index}"),
            AccessibleRole::Spinbox,
        );
        assert_exact_accessible_role(
            window,
            &format!("Custom reminder lead unit row {index}"),
            AccessibleRole::Combobox,
        );
    }
    assert_exact_accessible_role(window, "Save reminder profile", AccessibleRole::Button);
    assert_exact_accessible_role(
        window,
        "Reset reminder profile to recommended",
        AccessibleRole::Button,
    );
    assert_exact_accessible_role(
        window,
        &format!(
            "Reminder editor feedback {}",
            window.get_reminder_feedback()
        ),
        AccessibleRole::Text,
    );
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 1_000));
    set_backup_value(window, "Backup quiet period in seconds", 600);
    set_backup_value(window, "Backup interval in seconds", 43_200);
    set_backup_value(window, "Backup retention budget in mebibytes", 1_024);
    window
        .window()
        .set_size(slint::PhysicalSize::new(700, 1_600));
    assert_eq!(backup_value(window, "Backup quiet period in seconds"), 600);
    assert_eq!(backup_value(window, "Backup interval in seconds"), 43_200);
    assert_eq!(
        backup_value(window, "Backup retention budget in mebibytes"),
        1_024
    );
    save_backup_policy(window, 600, 43_200, 1_024);
    assert_eq!(
        backup_policy_intents(&sink),
        vec![(true, 600, 43_200, 1_024)]
    );
    set_backup_value(window, "Backup quiet period in seconds", 900);
    set_backup_value(window, "Backup interval in seconds", 86_400);
    set_backup_value(window, "Backup retention budget in mebibytes", 2_048);
    window
        .window()
        .set_size(slint::PhysicalSize::new(1120, 1_000));
    assert_eq!(backup_value(window, "Backup quiet period in seconds"), 900);
    assert_eq!(backup_value(window, "Backup interval in seconds"), 86_400);
    assert_eq!(
        backup_value(window, "Backup retention budget in mebibytes"),
        2_048
    );
    save_backup_policy(window, 900, 86_400, 2_048);
    assert_eq!(
        backup_policy_intents(&sink),
        vec![(true, 600, 43_200, 1_024), (true, 900, 86_400, 2_048)]
    );
    assert_exact_accessible_role(
        window,
        &format!(
            "Reminder synchronization state {}",
            window.get_reminder_sync_state()
        ),
        AccessibleRole::Text,
    );
    for label in unique_labels.iter().filter(|label| {
        label.contains("expiry reminders")
            || label.contains("Reminder lead time")
            || label.contains("custom reminder lead")
            || label.contains("Save reminder profile")
            || label.contains("Reset reminder profile")
            || label.contains("Reminder editor feedback")
            || label.contains("Reminder synchronization state")
    }) {
        assert!(
            ElementHandle::find_by_accessible_label(window, label)
                .any(|element| element.accessible_role().is_some()),
            "accessible role missing: {label}"
        );
    }
    assert!(
        !ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Button)
            .find_all()
            .is_empty()
    );
    for width in [700, 1120] {
        window
            .window()
            .set_size(slint::PhysicalSize::new(width, 900));
        let reminder_card =
            ElementHandle::find_by_element_id(window, "SettingsView::reminder-card")
                .next()
                .expect("reminder card");
        let card_position = reminder_card.absolute_position();
        let card_size = reminder_card.size();
        assert!(
            card_size.width > 0.0 && card_size.height > 0.0,
            "reminder card has bounds"
        );
        assert!(
            card_position.x >= 0.0 && card_position.x + card_size.width <= width as f32,
            "reminder card fits width {width}: x={} width={}",
            card_position.x,
            card_size.width
        );
        for label in unique_labels
            .iter()
            .filter(|label| label.contains("reminder") || label.contains("Reminder"))
        {
            let control = ElementHandle::find_by_accessible_label(window, label)
                .next()
                .expect("reminder control");
            let position = control.absolute_position();
            let size = control.size();
            assert!(
                size.width > 0.0 && size.height > 0.0,
                "positive bounds: {label}"
            );
            assert!(
                position.x >= card_position.x && position.y >= card_position.y,
                "inside card start: {label}"
            );
            assert!(
                position.x + size.width <= card_position.x + card_size.width,
                "inside card width: {label}"
            );
            assert!(
                position.y + size.height <= card_position.y + card_size.height,
                "inside card height: {label}"
            );
        }
        let reminder_bottom = window.get_settings_reminder_card_bottom();
        assert!(
            window.get_settings_backup_card_top() >= reminder_bottom,
            "backup card begins after reminder card at width {width}"
        );
        assert!(
            window.get_settings_config_card_top() >= reminder_bottom,
            "config card begins after reminder card at width {width}"
        );
    }
}
