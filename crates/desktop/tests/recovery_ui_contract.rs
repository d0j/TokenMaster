use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, Model};
use tokenmaster_desktop::{
    DesktopBackupHealth, DesktopBackupPolicy, DesktopIntent, DesktopIntentAdmission,
    DesktopIntentSink, DesktopOperationKind, DesktopOperationPhase, DesktopOperationSnapshot,
    DesktopRecoveryReceipt, DesktopReliableStateHealth, DesktopReliableStateInput,
    DesktopReliableStateNotifier, DesktopReliableStateProjection, DesktopReliableStateSummary,
    DesktopRestorePointInput, DesktopRestoreSelection, DesktopShell,
};
use tokenmaster_product::ProductReducer;

#[derive(Default)]
struct RecordingSink {
    intents: RefCell<Vec<DesktopIntent>>,
    notifier: RefCell<Option<DesktopReliableStateNotifier>>,
}

impl DesktopIntentSink for RecordingSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        if matches!(intent, DesktopIntent::ConfirmRestore { .. })
            && let Some(notifier) = self.notifier.borrow().as_ref()
        {
            notifier
                .publish_operation(Some(DesktopOperationSnapshot::new(
                    DesktopOperationKind::Restore,
                    DesktopOperationPhase::Running,
                    true,
                    None,
                )))
                .expect("publish restore operation");
        }
        self.intents.borrow_mut().push(intent);
        DesktopIntentAdmission::Started
    }
}

fn reliable_state() -> DesktopReliableStateProjection {
    let summary = DesktopReliableStateSummary::new(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::new(true, 300, 21_600, 512 * 1_048_576),
        Some(1_721_234_567_890),
        Some(1_721_234_567_890),
        Some(4),
        Some(1),
        Some(8_388_608),
        Some("unavailable"),
        Some(DesktopRecoveryReceipt::reconstructed_from_authoritative_source()),
        None,
        None,
    );
    DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        9,
        summary,
        vec![DesktopRestorePointInput::new(
            DesktopRestoreSelection::new(9, 0).expect("selection"),
            Some(1_721_234_567_890),
            2_097_152,
            DesktopBackupHealth::Verified,
            "manual",
            Some(12),
            "compact",
        )],
    ))
}

#[test]
fn compiled_shell_renders_data_health_and_dispatches_typed_path_free_intents() {
    let snapshot = ProductReducer::new().snapshot();
    let sink = Rc::new(RecordingSink::default());
    let shell = DesktopShell::new_with_reliable_state(&snapshot, reliable_state(), sink.clone())
        .expect("desktop shell");
    *sink.notifier.borrow_mut() = Some(shell.reliable_state_notifier());
    let window = shell.window();

    window.invoke_select_route("data_health".into());
    assert!(window.get_data_health_visible());
    assert_eq!(window.get_reliable_state_generation(), "9");
    assert_eq!(window.get_reliable_state_health(), "healthy");
    assert_eq!(window.get_reliable_recovery_kind(), "authoritative_source");
    assert!(window.get_reliable_non_reconstructible_domains_lost());
    assert_eq!(window.get_restore_point_rows().row_count(), 1);
    assert_eq!(
        window
            .get_restore_point_rows()
            .row_data(0)
            .expect("restore point")
            .health,
        "verified"
    );

    window.invoke_export_config();
    window.invoke_import_config();
    window.invoke_confirm_config_import();
    window.invoke_cancel_config_import();
    window.invoke_backup_normal();
    window.invoke_backup_compact();
    window.invoke_backup_encrypted("abcdefghijkl".into(), "abcdefghijkl".into());
    window.invoke_verify_backups();
    window.invoke_preview_restore(0);
    assert!(window.get_restore_confirmation_visible());
    assert_eq!(window.get_restore_confirmation_row(), 0);
    assert!(window.get_restore_confirmation_detail().contains("old"));
    assert!(
        window
            .get_restore_confirmation_detail()
            .contains("verified")
    );
    let replacement = DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        10,
        DesktopReliableStateSummary::new(
            DesktopReliableStateHealth::Healthy,
            false,
            "healthy",
            DesktopBackupPolicy::disabled(),
            None,
            None,
            Some(1),
            Some(0),
            Some(512),
            None,
            None,
            None,
            None,
        ),
        vec![DesktopRestorePointInput::new(
            DesktopRestoreSelection::new(10, 7).expect("replacement selection"),
            None,
            512,
            DesktopBackupHealth::Verified,
            "manual",
            Some(13),
            "normal",
        )],
    ));
    shell
        .reliable_state_notifier()
        .publish(replacement)
        .expect("publish replacement projection");
    let weak = window.as_weak();
    let notifier = shell.reliable_state_notifier();
    let unavailable_labels = Arc::new(Mutex::new(None));
    let labels = Arc::clone(&unavailable_labels);
    slint::invoke_from_event_loop(move || {
        let window = weak.upgrade().expect("live desktop window");
        assert_eq!(window.get_reliable_state_generation(), "10");
        window.invoke_confirm_restore(0, false);
        window.invoke_preview_restore(0);
        window.invoke_confirm_restore(0, true);
        notifier
            .publish(DesktopReliableStateProjection::unavailable())
            .expect("publish unavailable projection");
        let weak = window.as_weak();
        slint::invoke_from_event_loop(move || {
            let window = weak.upgrade().expect("live unavailable window");
            labels.lock().expect("unavailable labels").replace((
                window.get_reliable_successful_count_label().to_string(),
                window.get_reliable_failure_count_label().to_string(),
                window.get_reliable_published_bytes_label().to_string(),
            ));
            slint::quit_event_loop().expect("quit desktop event loop");
        })
        .expect("schedule unavailable assertion");
    })
    .expect("schedule projection replacement assertions");
    slint::run_event_loop_until_quit().expect("desktop event loop");
    window.invoke_rebuild_data();
    window.invoke_retry_operation();
    window.invoke_cancel_operation();
    window.invoke_update_backup_policy(true, 300, 21_600, 768);

    let intents = sink.intents.borrow();
    assert_eq!(intents.len(), 16);
    assert!(matches!(intents[0], DesktopIntent::ExportConfig));
    assert!(matches!(intents[1], DesktopIntent::ImportConfig));
    assert!(matches!(intents[2], DesktopIntent::ConfirmConfigImport));
    assert!(matches!(intents[3], DesktopIntent::CancelConfigImport));
    assert!(matches!(intents[4], DesktopIntent::BackupNormal));
    assert!(matches!(intents[5], DesktopIntent::BackupCompact));
    assert!(matches!(intents[6], DesktopIntent::BackupEncrypted { .. }));
    assert!(matches!(intents[7], DesktopIntent::VerifyBackups));
    assert!(matches!(intents[8], DesktopIntent::PreviewRestore(_)));
    assert_eq!(
        intents[9],
        DesktopIntent::ConfirmRestore {
            selection: DesktopRestoreSelection::new(9, 0).expect("reviewed selection"),
            portable_settings: false,
        }
    );
    assert_eq!(
        intents[10],
        DesktopIntent::PreviewRestore(
            DesktopRestoreSelection::new(10, 7).expect("replacement selection")
        )
    );
    assert_eq!(
        intents[11],
        DesktopIntent::ConfirmRestore {
            selection: DesktopRestoreSelection::new(10, 7).expect("replacement selection"),
            portable_settings: true,
        }
    );
    assert!(matches!(intents[12], DesktopIntent::RebuildData));
    assert!(matches!(intents[13], DesktopIntent::RetryOperation));
    assert!(matches!(intents[14], DesktopIntent::CancelOperation));
    assert!(matches!(
        intents[15],
        DesktopIntent::UpdateBackupPolicy { .. }
    ));
    assert!(!format!("{:?}", intents[6]).contains("abcdefghijkl"));
    assert_eq!(
        unavailable_labels
            .lock()
            .expect("unavailable labels")
            .as_ref()
            .expect("unavailable labels recorded"),
        &(
            "Unavailable".into(),
            "Unavailable".into(),
            "Unavailable".into()
        )
    );
}

#[test]
fn encrypted_backup_admission_rejects_invalid_or_mismatched_secrets_without_retention() {
    assert!(DesktopIntent::encrypted_backup("short", "short").is_err());
    assert!(DesktopIntent::encrypted_backup("abcdefghijkl", "mnopqrstuvwx").is_err());
    let intent =
        DesktopIntent::encrypted_backup("😀😀😀😀😀😀😀😀😀😀😀😀", "😀😀😀😀😀😀😀😀😀😀😀😀")
            .expect("Unicode scalar count is valid");
    assert!(matches!(intent, DesktopIntent::BackupEncrypted { .. }));
    assert!(!format!("{intent:?}").contains('😀'));
}

#[test]
fn recovery_ui_source_keeps_authority_bounded_and_accessible() {
    let main = include_str!("../ui/main.slint");
    let data_health = include_str!("../ui/views/data-health-view.slint");
    let settings = include_str!("../ui/views/settings-view.slint");
    let progress = include_str!("../ui/components/operation-progress.slint");
    let banner = include_str!("../ui/components/recovery-banner.slint");
    let combined = [main, data_health, settings, progress, banner].join("\n");

    for required in [
        "callback export-config()",
        "callback import-config()",
        "callback confirm-config-import()",
        "callback cancel-config-import()",
        "callback backup-normal()",
        "callback backup-compact()",
        "callback backup-encrypted(string, string)",
        "callback verify-backups()",
        "callback preview-restore(int)",
        "callback confirm-restore(int, bool)",
        "callback dismiss-restore-confirmation()",
        "Confirm destructive restore",
        "Data only",
        "Data + settings",
        "callback rebuild-data()",
        "callback retry-operation()",
        "callback cancel-operation()",
        "callback update-backup-policy(bool, int, int, int)",
        "ReminderCustomLeadRow",
        "callback reminder-enabled-edited(bool)",
        "callback reminder-preset-edited(int, bool)",
        "callback reminder-custom-lead-edited(int, bool, int, int)",
        "callback save-reminder-policy()",
        "callback reset-reminder-recommended()",
        "reminder-custom-lead-rows",
        "Save reminder profile",
        "Reset to recommended",
        "accessible-label",
        "high-contrast",
        "reduced-motion",
        "data-health-layout-mode",
        "Previous quota, reset-credit, reminder, and Git history is unavailable.",
        "passphrase.text = \"\"",
        "confirmation.text = \"\"",
        "minimum: 300",
        "maximum: 3600",
        "minimum: 21600",
        "maximum: 604800",
        "minimum: 256",
        "maximum: 65536",
    ] {
        assert!(
            combined.contains(required),
            "missing UI contract: {required}"
        );
    }
    for forbidden in [
        "path",
        "filename",
        "file-name",
        "Timer {",
        "animate ",
        "animation-",
        "std::fs",
        "rusqlite",
    ] {
        assert!(
            !combined.contains(forbidden),
            "forbidden UI authority or retention: {forbidden}"
        );
    }
}

/// True when a line of Slint code declares an animation or a repainting timer.
///
/// Written as a predicate rather than a list of byte sequences because the first version of
/// this check was a list, and a list matches spacing as well as spelling: it forbade
/// `Timer {` and `animate ` with exactly one space, so `Timer{` or `animate` followed by a
/// tab would have walked past it. Every keyword here is matched independently of the
/// whitespace around its opening delimiter.
///
/// `animate` must be followed by whitespace and never by more letters, which is what keeps a
/// property named `animated` from reading as an animation. `animation` shares no prefix with
/// `animate`, so `animation-tick()` is caught only by its own arm.
fn animates(line: &str) -> bool {
    // Stripped here rather than by the caller so the predicate owns the whole question. The
    // explanation written beside the one allowed animation names it, and must not count as a
    // second one; equally, a new animation must not be able to hide behind a `//`.
    let code = line.split("//").next().unwrap_or_default();
    if code.contains("animation-") {
        return true;
    }
    for (keyword, opener) in [("Timer", '{'), ("states", '['), ("transitions", '[')] {
        let mut rest = code;
        while let Some(at) = rest.find(keyword) {
            rest = &rest[at + keyword.len()..];
            if rest.trim_start().starts_with(opener) {
                return true;
            }
        }
    }
    let mut rest = code;
    while let Some(at) = rest.find("animate") {
        rest = &rest[at + "animate".len()..];
        if rest.starts_with(char::is_whitespace) {
            return true;
        }
    }
    false
}

/// The tree scan below can only prove what the tree happens to contain, and today it contains
/// exactly one animation written one way. These are the spellings it does not contain -- the
/// ones a list of byte sequences would have missed -- checked against the predicate directly
/// so the guarantee does not depend on someone writing them into a view first.
#[test]
fn an_animation_is_recognised_however_its_whitespace_is_written() {
    for animating in [
        "Timer { interval: 1s; }",
        "Timer{ interval: 1s; }",
        "Timer   { interval: 1s; }",
        "animate width { duration: 200ms; }",
        "animate\twidth { duration: 200ms; }",
        "opacity: animation-tick() / 1s;",
        "states [ shown when root.visible: { opacity: 1.0; } ]",
        "states[ shown when root.visible: { opacity: 1.0; } ]",
        "transitions [ in-out shown: { animate opacity {} } ]",
    ] {
        assert!(animates(animating), "not recognised as animating: {animating:?}");
    }

    for still in [
        "in property <bool> animated;",
        "text: @tr(\"Timer\");",
        "width: 7px;",
        "// animate width { duration: 200ms; }",
        "property <int> states-count: 3;",
    ] {
        assert!(!animates(still), "wrongly read as animating: {still:?}");
    }
}

/// The forbidden list in the test above reads five Slint files by name, and the one file in
/// the tree that animates is not among them -- so the rule that exists to keep an idle
/// animation out of the product could never have seen the only animation in the product.
/// It entered once already and was found by measuring a core at 97.7%, not by this gate.
///
/// Enumerated from disk for the same reason the localization contract is: a list of files
/// has to be remembered, and forgetting is the failure being guarded against. The single
/// exception is pinned by content rather than by file name, so moving `ImportActivityDot`
/// somewhere else keeps it legal and adding a second animation anywhere does not.
#[test]
fn the_only_animation_in_the_slint_tree_is_the_guarded_import_dot() {
    fn collect(directory: &std::path::Path, sources: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(directory).expect("read ui directory") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                collect(&path, sources);
            } else if path.extension().is_some_and(|value| value == "slint") {
                let text = std::fs::read_to_string(&path).expect("read slint source");
                sources.push((path.display().to_string(), text.replace("\r\n", "\n")));
            }
        }
    }

    let mut sources = Vec::new();
    collect(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui"),
        &mut sources,
    );
    assert!(
        sources.len() > 20,
        "expected the whole Slint tree, found {} files",
        sources.len()
    );

    let mut animating = Vec::new();
    for (path, text) in &sources {
        for (number, line) in text.lines().enumerate() {
            if animates(line) {
                let code = line.split("//").next().unwrap_or_default();
                animating.push((path.clone(), number, code.trim().to_string()));
            }
        }
    }
    assert_eq!(
        animating.len(),
        1,
        "exactly one animation is allowed in the tree, found {animating:#?}"
    );

    let (path, number, line) = &animating[0];
    assert!(
        line.contains("animation-tick()"),
        "the allowed animation is the import dot's tick, found {line:?} in {path}"
    );

    // The guard is the whole reason this one is allowed: `animation-tick()` schedules a
    // repaint for as long as its binding is evaluated, and Slint 1.17 instantiates the
    // contents of an `if` eagerly, so an unguarded read burns a core whether or not the
    // window is even visible. The condition must sit on the binding that reads the tick.
    let (_, text) = sources
        .iter()
        .find(|(candidate, _)| candidate == path)
        .expect("the animating source");
    let guard = text
        .lines()
        .nth(number.saturating_sub(1))
        .unwrap_or_default();
    assert!(
        guard.contains("root.active") && guard.contains("!root.reduced-motion"),
        "the animation must stay behind the active and reduced-motion guard, found {guard:?}"
    );
}

#[test]
fn desktop_bridge_factory_is_send_sync_and_retains_no_strong_window() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<tokenmaster_desktop::DesktopBridgeFactory>();
    let source = include_str!("../src/ui.rs");
    assert!(source.contains("window: slint::Weak<MainWindow>"));
    assert!(!source.contains("struct DesktopBridgeFactory {\n    window: MainWindow"));
}
