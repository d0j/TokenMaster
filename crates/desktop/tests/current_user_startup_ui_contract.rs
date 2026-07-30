use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::ComponentHandle;
use tokenmaster_desktop::{
    DesktopCurrentUserStartupStatus, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopReliableStateProjection, DesktopShell,
};
use tokenmaster_product::ProductReducer;

#[derive(Default)]
struct RecordingSink {
    intents: RefCell<Vec<DesktopIntent>>,
}

impl DesktopIntentSink for RecordingSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopIntentAdmission::Started
    }
}

#[test]
fn startup_status_uses_explicit_actions_and_three_typed_intents() {
    i_slint_backend_testing::init_no_event_loop();
    let snapshot = ProductReducer::new().snapshot();
    let sink = Rc::new(RecordingSink::default());
    let shell = DesktopShell::new_with_reliable_state(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        sink.clone(),
    )
    .expect("desktop shell");
    let window = shell.window();
    let presenter = shell.current_user_startup_presenter();

    presenter
        .present(DesktopCurrentUserStartupStatus::Disabled)
        .expect("disabled state");
    assert_eq!(window.get_current_user_startup_status(), "disabled");
    assert!(window.get_current_user_startup_can_enable());
    assert!(!window.get_current_user_startup_can_disable());
    assert!(!window.get_current_user_startup_can_repair());

    window.invoke_enable_current_user_startup();
    presenter
        .present(DesktopCurrentUserStartupStatus::EnabledVerified)
        .expect("enabled state");
    assert_eq!(window.get_current_user_startup_status(), "enabled_verified");
    assert!(!window.get_current_user_startup_can_enable());
    assert!(window.get_current_user_startup_can_disable());

    window.invoke_disable_current_user_startup();
    presenter
        .present(DesktopCurrentUserStartupStatus::StaleRelocation)
        .expect("stale state");
    assert_eq!(window.get_current_user_startup_status(), "stale_relocation");
    assert!(window.get_current_user_startup_can_disable());
    assert!(window.get_current_user_startup_can_repair());
    window.invoke_repair_current_user_startup();

    assert!(matches!(
        sink.intents.borrow().as_slice(),
        [
            DesktopIntent::EnableCurrentUserStartup,
            DesktopIntent::DisableCurrentUserStartup,
            DesktopIntent::RepairCurrentUserStartup,
        ]
    ));
}

#[test]
fn degraded_states_are_visible_accessible_and_non_destructive() {
    i_slint_backend_testing::init_no_event_loop();
    let snapshot = ProductReducer::new().snapshot();
    let sink = Rc::new(RecordingSink::default());
    let shell = DesktopShell::new_with_reliable_state(
        &snapshot,
        DesktopReliableStateProjection::unavailable(),
        sink.clone(),
    )
    .expect("desktop shell");
    let window = shell.window();
    let presenter = shell.current_user_startup_presenter();

    for state in [
        DesktopCurrentUserStartupStatus::Conflict,
        DesktopCurrentUserStartupStatus::AccessDenied,
        DesktopCurrentUserStartupStatus::Unavailable,
    ] {
        presenter.present(state).expect("degraded state");
        assert!(!window.get_current_user_startup_can_enable());
        assert!(!window.get_current_user_startup_can_disable());
        assert!(!window.get_current_user_startup_can_repair());
    }
    assert!(sink.intents.borrow().is_empty());

    window.invoke_select_route("settings".into());
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_200, 2_000));
    window.show().expect("show settings");
    for (state, labels) in [
        (
            DesktopCurrentUserStartupStatus::Disabled,
            ["Enable TokenMaster at Windows sign-in", ""],
        ),
        (
            DesktopCurrentUserStartupStatus::EnabledVerified,
            ["Disable TokenMaster at Windows sign-in", ""],
        ),
        (
            DesktopCurrentUserStartupStatus::StaleRelocation,
            [
                "Repair TokenMaster startup registration",
                "Remove old TokenMaster startup registration",
            ],
        ),
    ] {
        window.hide().expect("hide before action state");
        presenter.present(state).expect("action state");
        window.show().expect("show action state");
        for label in labels.into_iter().filter(|label| !label.is_empty()) {
            let mut actions = ElementHandle::find_by_accessible_label(window, label)
                .filter(|element| element.accessible_role() == Some(AccessibleRole::Button));
            let action = actions
                .next()
                .unwrap_or_else(|| panic!("missing accessible action for {label}"));
            assert!(
                actions.next().is_none(),
                "duplicate accessible action for {label}"
            );
            action.invoke_accessible_default_action();
        }
    }

    assert!(matches!(
        sink.intents.borrow().as_slice(),
        [
            DesktopIntent::EnableCurrentUserStartup,
            DesktopIntent::DisableCurrentUserStartup,
            DesktopIntent::RepairCurrentUserStartup,
            DesktopIntent::DisableCurrentUserStartup,
        ]
    ));
}
