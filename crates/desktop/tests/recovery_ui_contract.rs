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
    // Retention and authority are checked across the whole tree by
    // `no_slint_source_can_name_a_path_or_reach_past_the_bridge`, and animation keywords by
    // `animates`. What stays here is the required list above: it asserts these five views'
    // own contract, which is a different question from a tree-wide rule.
}

/// Every Slint source, read from disk with line endings normalised.
///
/// From disk rather than a list of `include_str!` names, for the reason the localization
/// contract gives: a list has to be remembered, and forgetting is the failure being guarded
/// against. Two rules in this file were written against five names while the tree held
/// twenty-five, and both were blind to the file that mattered.
fn slint_sources() -> Vec<(String, String)> {
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
    sources
}

/// The retention half of the rule above, which stayed on five files when the animation half
/// moved to the whole tree.
///
/// Twenty views had no guard against an absolute path reaching the interface, and that is
/// where the deferred Live Sessions and project-alias work is heading -- both want to name the
/// repository a row belongs to. The invariant is the first one in `CLAUDE.md`: never persist
/// or expose absolute user paths.
#[test]
fn no_slint_source_can_name_a_path_or_reach_past_the_bridge() {
    for (source, text) in slint_sources() {
        for line in text.lines() {
            let code = code_of(line);
            for forbidden in ["path", "filename", "file-name", "std::fs", "rusqlite"] {
                assert!(
                    !code.contains(forbidden),
                    "forbidden UI authority or retention {forbidden:?} in {source}: {:?}",
                    line.trim()
                );
            }
        }
    }
}

/// The whole-tree animation scan reads first-party sources only, and the animation a refactor
/// would reach for lives in the Slint crate: `std-widgets`' `Spinner` is built on `SpinnerBase`,
/// whose source reads `animation-tick()`. Seventeen of the twenty-five views already import
/// from `std-widgets.slint`, so adding `Spinner` to one of them is a single word, and the
/// first-party scan would find zero animations rather than a second one.
///
/// The set is closed in the same way the msgid arrays and `Get-ProductPackageExpectedFile`
/// are: a new widget has to be added here deliberately, and one that animates cannot arrive
/// by accident.
#[test]
fn the_widgets_imported_from_the_slint_library_are_a_closed_set() {
    const ALLOWED: [&str; 8] = [
        "AboutSlint",
        "Button",
        "CheckBox",
        "ComboBox",
        "LineEdit",
        "Palette",
        "ScrollView",
        "SpinBox",
    ];

    let mut seen = std::collections::BTreeSet::new();
    for (path, text) in slint_sources() {
        for line in text.lines() {
            let Some(rest) = line.split_once("from \"std-widgets.slint\"") else {
                continue;
            };
            let names = rest
                .0
                .trim()
                .trim_start_matches("import")
                .trim()
                .trim_start_matches('{')
                .trim_end_matches('}');
            for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                assert!(
                    ALLOWED.contains(&name),
                    "{name:?} is imported from std-widgets in {path} and is not in the closed \
                     set; a widget from the Slint library can animate without any first-party \
                     source saying so"
                );
                seen.insert(name.to_string());
            }
        }
    }
    assert!(
        seen.len() >= 6,
        "expected the product's std-widgets imports, found {seen:?}"
    );
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
/// The code of one line: string literals emptied and any trailing comment dropped.
///
/// Both keyword rules in this file were fooled by text rather than by code. A `//` inside a
/// string literal -- a URL is enough -- truncated the line before it was scanned, so anything
/// after it was never read. And the word "path" inside a translated sentence that promises
/// paths are *not* retained read as a property carrying one, which is what the tree-wide
/// retention rule hit on its first run.
///
/// Emptying literals before matching answers both at once: nothing a user reads can satisfy a
/// rule about what the code does, and nothing a user reads can hide code from it either.
fn code_of(line: &str) -> String {
    let mut code = String::with_capacity(line.len());
    let mut characters = line.chars().peekable();
    let mut inside_literal = false;
    while let Some(character) = characters.next() {
        match character {
            '\\' if inside_literal => {
                characters.next();
            }
            '"' => {
                inside_literal = !inside_literal;
                code.push('"');
            }
            '/' if !inside_literal && characters.peek() == Some(&'/') => break,
            _ if inside_literal => {}
            _ => code.push(character),
        }
    }
    code
}

fn animates(line: &str) -> bool {
    let code = code_of(line);
    let code = code.as_str();
    if code.contains("animation-") {
        return true;
    }
    for (keyword, opener) in [("Timer", '{'), ("states", '['), ("transitions", '[')] {
        let mut rest = code;
        while let Some(at) = rest.find(keyword) {
            rest = &rest[at + keyword.len()..];
            let after = rest.trim_start();
            if after.is_empty() || after.starts_with(opener) {
                return true;
            }
        }
    }
    let mut rest = code;
    while let Some(at) = rest.find("animate") {
        rest = &rest[at + "animate".len()..];
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            return true;
        }
    }
    false
}

/// No view may decide a fact by looking at the text it just rendered.
///
/// The Dashboard header carried `availability: root.tokens == "—" ? "unavailable" : "known"`,
/// which collapsed five availability states into two -- a `Partial` count announced itself as
/// known -- and hung the whole distinction on one glyph, so changing the em dash in any Rust
/// formatter would have flipped the meaning of the product's most prominent card with every
/// test still green. The projection already computed the fact; the card threw it away and
/// guessed it back from typography.
///
/// The rule is the shape, not those three lines: an em dash is a rendering, and a rendering is
/// not evidence. Every value that needs its availability receives it, the way the trend card
/// one row below always did.
#[test]
fn no_slint_source_infers_a_fact_from_the_unavailable_glyph() {
    let mut comparisons = Vec::new();
    for (source, text) in slint_sources() {
        for line in text.lines() {
            // Deliberately on the raw line: `code_of` empties string literals, and the glyph
            // being compared against lives inside one.
            if line.contains("== \"—\"") || line.contains("!= \"—\"") {
                comparisons.push(format!("{source}: {}", line.trim()));
            }
        }
    }
    // Started as a closed debt of fourteen across four views and is now empty, so the rule is
    // what it was always meant to be: a plain assertion that no view infers a fact from the
    // text it rendered.
    assert!(
        comparisons.is_empty(),
        "a view compared against the unavailable glyph instead of receiving the fact: \
         {comparisons:#?}"
    );
}

/// The tray's labels must arrive as data, never as literals in its own source.
///
/// The localization contract asserts a closed msgid set and that each entry appears as `@tr`
/// in the Slint tree, so a user-visible string emitted from Rust escapes it **by
/// construction** rather than by oversight. That has now happened twice: eight Activity
/// labels, and six tray strings that shipped English for the life of the product. Fixing the
/// strings without forbidding the shape leaves the third occurrence free to arrive the same
/// way, so this asserts the shape: every menu label is an expression, not a quoted string.
#[test]
fn the_tray_names_no_user_visible_string_of_its_own() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native_tray.rs"),
    )
    .expect("read the tray source");

    let mut literals = Vec::new();
    for line in source.lines() {
        let code = code_of(line);
        let Some(argument) = code.split("append_menu_item(").nth(1) else {
            continue;
        };
        // `code_of` empties literals but keeps their quotes, so a label that was written as a
        // string still shows as `""` here and an expression does not.
        if argument.contains("\"\"") {
            literals.push(line.trim().to_string());
        }
    }
    assert!(
        literals.is_empty(),
        "tray menu labels must be passed in, not written here: {literals:#?}"
    );
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
        // The keyword may be the last thing on its line, with the property and the block on
        // the next. An empty remainder is not "no whitespace after the keyword".
        "        animate",
        "    states",
        "    transitions",
        // A `//` inside a string literal must not truncate the line before it is scanned.
        "property <string> u: \"https://x\"; animate width { duration: 100ms; }",
    ] {
        assert!(
            animates(animating),
            "not recognised as animating: {animating:?}"
        );
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
    let sources = slint_sources();

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
