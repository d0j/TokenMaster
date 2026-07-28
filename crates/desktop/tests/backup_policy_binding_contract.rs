//! A Slint binding written as `checked: root.x` is severed the first time the user
//! touches the control, permanently and by design. Every later push from Rust is then
//! dropped in silence, so the backup card can display a policy that is not the one
//! running -- after a config import, for instance. These tests interact first and
//! assert the push afterwards, because a test that only pushes passes either way.

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::ComponentHandle;
use tokenmaster_desktop::{
    DesktopIntent, DesktopIntentAdmission, DesktopIntentSink, DesktopReliableStateProjection,
    DesktopShell, MainWindow,
};
use tokenmaster_product::ProductReducer;

struct RecordingSink {
    intents: RefCell<Vec<DesktopIntent>>,
}

impl DesktopIntentSink for RecordingSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopIntentAdmission::Started
    }
}

fn settings_shell() -> (DesktopShell, Rc<RecordingSink>) {
    i_slint_backend_testing::init_no_event_loop();
    let sink = Rc::new(RecordingSink {
        intents: RefCell::new(Vec::new()),
    });
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        sink.clone(),
    )
    .expect("desktop shell");
    let window = shell.window();
    // Tall enough that the backup card is not clipped: `ElementQuery` skips any item
    // whose rect falls entirely outside its clipping ancestors.
    window
        .window()
        .set_size(slint::PhysicalSize::new(1_400, 3_000));
    window.invoke_select_route("settings".into());
    (shell, sink)
}

fn control(window: &MainWindow, role: AccessibleRole, label: &str) -> ElementHandle {
    let wanted = label.to_owned();
    ElementQuery::from_root(window)
        .match_accessible_role(role)
        .match_predicate(move |element| element.accessible_label().as_deref() == Some(&wanted))
        .find_first()
        .unwrap_or_else(|| panic!("missing control {label}"))
}

#[test]
fn a_pushed_quiet_period_reaches_the_spinbox_after_the_user_edits_it() {
    let (shell, _sink) = settings_shell();
    let window = shell.window();

    window.set_backup_quiet_seconds(600);
    let spinbox = control(
        window,
        AccessibleRole::Spinbox,
        "Backup quiet period in seconds",
    );
    assert_eq!(
        spinbox.accessible_value().as_deref(),
        Some("600"),
        "the first push must reach an untouched control"
    );

    // This is the severing interaction, not an assertion about stepping.
    spinbox.invoke_accessible_increment_action();
    let stepped = control(
        window,
        AccessibleRole::Spinbox,
        "Backup quiet period in seconds",
    );
    assert_ne!(
        stepped.accessible_value().as_deref(),
        Some("600"),
        "the increment must actually have moved the control"
    );

    window.set_backup_quiet_seconds(1_200);
    let after = control(
        window,
        AccessibleRole::Spinbox,
        "Backup quiet period in seconds",
    );
    assert_eq!(
        after.accessible_value().as_deref(),
        Some("1200"),
        "a push after a user edit must still reach the control"
    );
}

#[test]
fn a_pushed_periodic_flag_reaches_the_checkbox_after_the_user_toggles_it() {
    let (shell, _sink) = settings_shell();
    let window = shell.window();

    window.set_backup_periodic_enabled(true);
    let checkbox = control(window, AccessibleRole::Checkbox, "Enable periodic backups");
    assert_eq!(
        checkbox.accessible_checked(),
        Some(true),
        "the first push must reach an untouched control"
    );

    checkbox.invoke_accessible_default_action();
    let toggled = control(window, AccessibleRole::Checkbox, "Enable periodic backups");
    assert_eq!(
        toggled.accessible_checked(),
        Some(false),
        "the toggle must actually have moved the control"
    );

    window.set_backup_periodic_enabled(true);
    let after = control(window, AccessibleRole::Checkbox, "Enable periodic backups");
    assert_eq!(
        after.accessible_checked(),
        Some(true),
        "a push after a user toggle must still reach the control"
    );
}
