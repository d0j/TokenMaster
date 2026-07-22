use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;

use tokenmaster_desktop::{
    DesktopIntent, DesktopIntentAdmission, DesktopIntentSink, DesktopLocale,
    DesktopPresentationSelection, DesktopReliableStateProjection, DesktopShell,
};
use tokenmaster_product::ProductReducer;

const TRANSLATION_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/translations");

const SHELL_MSGIDS: [&str; 33] = [
    "TokenMaster",
    "Local usage intelligence",
    "Go to route",
    "Open route palette",
    "TokenMaster settings",
    "Density",
    "Comfortable",
    "Compact",
    "Ultra compact",
    "Presentation density",
    "Skin",
    "Refined",
    "Graphite",
    "Ember",
    "Presentation skin",
    "Scheme",
    "System",
    "Light",
    "Dark",
    "Presentation color scheme",
    "Layout",
    "Control center",
    "Workbench",
    "Presentation layout",
    "Saved",
    "Saving…",
    "Not saved — choose the current presentation again to retry",
    "Presentation persistence {0}",
    "Language",
    "English",
    "Russian",
    "Pseudo",
    "Presentation language",
];

const COMPONENT_MSGIDS: [&str; 29] = [
    "Review",
    "Review restore point ",
    "Route palette",
    "Filter routes",
    "No matching routes",
    "Up/Down to select · Enter to open · Escape to dismiss",
    "Expiry notifications. ",
    ". Open Notifications for current inventory and reminder policy.",
    "Expiry reminder",
    "Dismiss",
    "Dismiss expiry notifications",
    "These reminders were applied in TokenMaster. Open Notifications for the complete current inventory.",
    "Qty ",
    "Reliable state operation ",
    "Retry",
    "Retry reliable state operation",
    "Cancel",
    "Cancel reliable state operation",
    "Recovery mode status",
    "Safe mode is active. Usage data remains offline while recovery controls stay available.",
    "Usage was rebuilt from local Codex sources. Previous quota, reset-credit, reminder, and Git history is unavailable.",
    "Usage was rebuilt from local Codex sources.",
    "Data recovery completed from a verified backup.",
    "Data recovery requires attention.",
    "State: ",
    "No blocking reasons",
    "Reasons: ",
    "Product generation ",
    ": ",
];

const SETTINGS_REMINDER_BACKUP_MSGIDS: [&str; 36] = [
    "Settings",
    "Settings health: {0}",
    "Expiry reminders",
    "Sync state: {0}",
    "Reminder synchronization state {0}",
    "Enable expiry reminders",
    "Recommended lead times",
    "7 days",
    "24 hours",
    "12 hours",
    "6 hours",
    "1 hour",
    "Reminder lead time {0}",
    "Custom lead times",
    "Custom lead",
    "Enable custom reminder lead row {0}",
    "Custom reminder lead value row {0}",
    "seconds",
    "minutes",
    "hours",
    "days",
    "Custom reminder lead unit row {0}",
    "Save reminder profile",
    "Reset to recommended",
    "Reset reminder profile to recommended",
    "Reminder editor feedback {0}",
    "Automatic backup policy",
    "Enable periodic backups",
    "Quiet seconds",
    "Backup quiet period in seconds",
    "Interval seconds",
    "Backup interval in seconds",
    "Budget MiB",
    "Backup retention budget in mebibytes",
    "Save backup policy",
    "Save automatic backup policy",
];

const SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS: [&str; 41] = [
    "Start with Windows",
    "Disabled. TokenMaster will not start when you sign in.",
    "Enabled and verified for this executable.",
    "The registration points to a previous TokenMaster location. Repair or remove it explicitly.",
    "A conflicting startup value is present. TokenMaster will not overwrite it.",
    "Windows denied access to the current-user startup value.",
    "Current-user startup is unavailable on this system.",
    "Current-user startup status {0}",
    "Enable at sign-in",
    "Enable TokenMaster at Windows sign-in",
    "Disable at sign-in",
    "Disable TokenMaster at Windows sign-in",
    "Repair registration",
    "Repair TokenMaster startup registration",
    "Remove old registration",
    "Remove old TokenMaster startup registration",
    "Portable configuration",
    "Export configuration",
    "Export TokenMaster configuration",
    "Import configuration",
    "Import TokenMaster configuration",
    "Import review · {0} · {1} · {2}",
    "Apply",
    "Apply reviewed configuration import",
    "Discard",
    "Discard reviewed configuration import",
    "Dashboard board",
    "Reset board",
    "Reset dashboard board",
    "Order, visibility, and collapse are saved with the current presentation.",
    "Dashboard board section {0}",
    "Up",
    "Move {0} up",
    "Down",
    "Move {0} down",
    "Visible",
    "Show {0} on dashboard",
    "Collapsed",
    "Collapse {0} on dashboard",
    "Reduced motion enabled",
    "Reduced motion ready",
];

const PROJECTS_COMPACT_MSGIDS: [&str; 34] = [
    "Projects",
    "Recent usage · {0} · {1}",
    "Projects. Recent usage {0} {1} {2}. Today code {3} {4} {5}. {6}. {7}.",
    "Today code · {0} · {1}",
    "Recent tokens",
    "Recent cost · {0}",
    "Recent events",
    "Recent usage by project with separately labelled today code output",
    "Usage by project",
    "Waiting for project evidence",
    "No project usage in this range",
    "Project evidence unavailable",
    "Project",
    "Relative",
    "Total",
    "Cost",
    "Events",
    "Commits",
    "+Added",
    "-Removed",
    "Net",
    "Cost / 100 added product-code lines",
    "Recent usage {0} input {1} cached {2} output {3} reasoning {4} total {5} cost {6} {7} events {8}. Today code {9} {10} commits {11} added {12} removed {13} net {14} efficiency {15} {16} {17}",
    "Recent · In {0} · Cache {1} · Out {2} · Reason {3} · {4} events",
    "Today code · {0} · {1} · {2} commits · {3} / {4} · net {5}",
    "Recent mix · In {0} · Cache {1} · Out {2} · Reason {3} · {4} | Today code · {5} · {6} · {7} {8}",
    "Compact quota window {0} {1} {2} {3}",
    "Usage ratio unavailable",
    "Compact quota",
    "Current quota",
    "Return to Dashboard",
    "Waiting for quota evidence",
    "Quota evidence unavailable",
    "TokenMaster",
];

const COMPONENT_RAW_LITERAL_ALLOWLIST: [&str; 11] = [
    "", " ", " · ", ", ", "ready", "degraded", "waiting", "●", "▲", "…", "×",
];

struct RecordingIntentSink {
    selection: Cell<Option<DesktopPresentationSelection>>,
}

impl DesktopIntentSink for RecordingIntentSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        let DesktopIntent::UpdatePresentation(selection) = intent else {
            return DesktopIntentAdmission::Rejected;
        };
        self.selection.set(Some(selection));
        DesktopIntentAdmission::Started
    }
}

#[test]
fn hot_locale_shell_requires_compile_time_bundles() {
    let build = include_str!("../build.rs");

    assert!(
        build.contains("with_bundled_translations(\"translations\")"),
        "desktop Slint compilation must embed the closed locale catalog directory"
    );
    for locale in ["ru", "pseudo"] {
        assert!(
            Path::new(TRANSLATION_ROOT)
                .join(locale)
                .join("LC_MESSAGES")
                .join("tokenmaster-desktop.po")
                .is_file(),
            "missing bundled {locale} catalog"
        );
    }
}

#[test]
fn shell_component_and_settings_reminder_backup_catalogs_are_complete_and_preserve_placeholders() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .collect::<BTreeSet<_>>();
    for locale in ["ru", "pseudo"] {
        let catalog = std::fs::read_to_string(
            Path::new(TRANSLATION_ROOT)
                .join(locale)
                .join("LC_MESSAGES")
                .join("tokenmaster-desktop.po"),
        )
        .expect("bundled catalog");
        let entries = po_entries(&catalog);

        assert_eq!(
            entries.keys().copied().collect::<BTreeSet<_>>(),
            expected,
            "{locale} must translate exactly the G2a1 shell, Task 2a2 component, Task 2b1 Settings reminder/backup, and Task 2b2 Settings startup/config/board/footer key sets"
        );
        assert_eq!(
            po_entry_count(&catalog),
            expected.len(),
            "{locale} must not contain duplicate msgids"
        );
        for msgid in expected.iter().copied() {
            let msgstr = entries.get(msgid).expect("catalog completeness");
            assert!(
                !msgstr.is_empty(),
                "{locale} must not have empty translations"
            );
            assert_eq!(
                placeholders(msgstr),
                placeholders(msgid),
                "{locale} must preserve placeholders for {msgid:?}"
            );
        }
        assert!(
            !catalog.contains("%1"),
            "{locale} must not retain unsupported Slint %1 placeholders"
        );
    }
}

#[test]
fn projects_and_compact_widget_catalogs_are_complete_before_view_conversion() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .collect::<BTreeSet<_>>();
    for locale in ["ru", "pseudo"] {
        let catalog = std::fs::read_to_string(
            Path::new(TRANSLATION_ROOT)
                .join(locale)
                .join("LC_MESSAGES")
                .join("tokenmaster-desktop.po"),
        )
        .expect("bundled catalog");
        let entries = po_entries(&catalog);
        assert_eq!(
            entries.keys().copied().collect::<BTreeSet<_>>(),
            expected,
            "{locale} must translate exactly the closed Task 2b3 Projects and compact-widget key set"
        );
    }
}

#[test]
fn projects_and_compact_widget_use_the_closed_translation_key_set() {
    let projects = include_str!("../ui/views/projects-view.slint");
    let compact = include_str!("../ui/views/compact-widget-view.slint");

    for msgid in PROJECTS_COMPACT_MSGIDS {
        assert!(
            projects.contains(&format!("@tr(\"{msgid}\""))
                || compact.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b3 @tr for {msgid:?}"
        );
    }

    for raw in [
        "text: \"Projects\"",
        "text: \"Current quota\"",
        "text: \"Return to Dashboard\"",
        "accessible-label: \"Compact quota\"",
        "text: \"Usage by project\"",
    ] {
        assert!(
            !projects.contains(raw) && !compact.contains(raw),
            "unwrapped Task 2b3 literal {raw:?}"
        );
    }
}

#[test]
fn settings_startup_config_board_and_footer_use_only_the_closed_translation_key_set() {
    let settings = include_str!("../ui/views/settings-view.slint");

    for msgid in SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS {
        assert!(
            settings.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b2 Settings @tr for {msgid:?}"
        );
    }

    for raw in [
        "text: \"Start with Windows\"",
        "text: \"Portable configuration\"",
        "text: \"Dashboard board\"",
        "text: \"Reduced motion enabled\"",
        "text: \"Reduced motion ready\"",
    ] {
        assert!(
            !settings.contains(raw),
            "unwrapped Task 2b2 literal {raw:?}"
        );
    }
    assert!(
        !settings.contains("%1"),
        "Task 2b2 Settings source must use Slint format placeholders, not literal %1"
    );
}

#[test]
fn settings_reminder_backup_uses_only_the_closed_translation_key_set() {
    let settings = include_str!("../ui/views/settings-view.slint");

    for msgid in SETTINGS_REMINDER_BACKUP_MSGIDS {
        assert!(
            settings.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b1 Settings @tr for {msgid:?}"
        );
    }

    for raw in [
        "text: \"Settings\"",
        "text: \"Expiry reminders\"",
        "text: \"Automatic backup policy\"",
        "model: [\"seconds\", \"minutes\", \"hours\", \"days\"]",
        "text: \"Save reminder profile\"",
        "text: \"Save backup policy\"",
    ] {
        assert!(
            !settings.contains(raw),
            "unwrapped Task 2b1 literal {raw:?}"
        );
    }
    assert!(
        !settings.contains("%1"),
        "Task 2b1 Settings source must use Slint format placeholders, not literal %1"
    );
}

#[test]
fn shared_components_use_only_the_closed_translation_key_set() {
    let components = [
        (
            "backup-row.slint",
            include_str!("../ui/components/backup-row.slint"),
        ),
        (
            "command-palette.slint",
            include_str!("../ui/components/command-palette.slint"),
        ),
        (
            "in-app-notification-panel.slint",
            include_str!("../ui/components/in-app-notification-panel.slint"),
        ),
        (
            "metric-value.slint",
            include_str!("../ui/components/metric-value.slint"),
        ),
        (
            "operation-progress.slint",
            include_str!("../ui/components/operation-progress.slint"),
        ),
        (
            "quota-row.slint",
            include_str!("../ui/components/quota-row.slint"),
        ),
        (
            "recovery-banner.slint",
            include_str!("../ui/components/recovery-banner.slint"),
        ),
        (
            "route-nav-item.slint",
            include_str!("../ui/components/route-nav-item.slint"),
        ),
        (
            "route-state.slint",
            include_str!("../ui/components/route-state.slint"),
        ),
        (
            "section-state.slint",
            include_str!("../ui/components/section-state.slint"),
        ),
    ];

    for msgid in COMPONENT_MSGIDS {
        assert!(
            components
                .iter()
                .any(|(_, component)| component.contains(&format!("@tr(\"{msgid}\""))),
            "missing component @tr for {msgid:?}"
        );
    }

    for (path, component) in components {
        for (line_index, line) in component.lines().enumerate() {
            let property = line.trim_start();
            if !(property.starts_with("text:")
                || property.starts_with("accessible-label:")
                || property.starts_with("placeholder-text:"))
            {
                continue;
            }

            for literal in raw_quoted_literals(property) {
                assert!(
                    COMPONENT_RAW_LITERAL_ALLOWLIST.contains(&literal),
                    "{path}:{} has unwrapped linguistic literal {literal:?}; use @tr",
                    line_index + 1
                );
            }
        }
    }
}

#[test]
fn locale_selector_wires_the_complete_presentation_update_and_hot_bundle_apply() {
    i_slint_backend_testing::init_no_event_loop();
    let sink = Rc::new(RecordingIntentSink {
        selection: Cell::new(None),
    });
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();

    assert_eq!(window.get_presentation_locale_key(), "en");
    window.invoke_select_presentation_locale(1);

    assert_eq!(window.get_presentation_locale_key(), "ru");
    assert_eq!(
        sink.selection
            .get()
            .map(DesktopPresentationSelection::locale),
        Some(DesktopLocale::Russian)
    );
    window.invoke_select_presentation_locale(2);
    assert_eq!(window.get_presentation_locale_key(), "pseudo");
}

#[test]
fn shell_and_presentation_strip_use_only_the_closed_translation_key_set() {
    let main = include_str!("../ui/main.slint");
    let settings = include_str!("../ui/views/settings-view.slint");
    let ui = include_str!("../src/ui.rs");

    for msgid in SHELL_MSGIDS {
        assert!(
            main.contains(&format!("@tr(\"{msgid}\""))
                || settings.contains(&format!("@tr(\"{msgid}\"")),
            "missing @tr for {msgid:?}"
        );
    }
    for raw in [
        "text: \"TokenMaster\"",
        "text: \"Local usage intelligence\"",
        "text: \"Go to route\"",
        "accessible-label: \"Open route palette\"",
        "accessible-label: \"TokenMaster settings\"",
        "model: [\"Comfortable\", \"Compact\", \"Ultra compact\"]",
        "model: [\"Refined\", \"Graphite\", \"Ember\"]",
        "model: [\"System\", \"Light\", \"Dark\"]",
        "model: [\"Refined\", \"Control center\", \"Workbench\"]",
    ] {
        assert!(
            !main.contains(raw) && !settings.contains(raw),
            "unwrapped G2a1 literal {raw:?}"
        );
    }
    assert!(main.contains("in-out property <int> presentation-locale-id: 0;"));
    assert!(main.contains("callback select-presentation-locale(int);"));
    assert!(settings.contains("in property <int> presentation-locale-id;"));
    assert!(settings.contains("callback select-presentation-locale(int);"));
    assert!(ui.contains("fn wire_presentation_locale("));
    assert!(ui.contains("select_presentation_locale_if_admitted"));
    let select = ui
        .find("slint::select_bundled_translation(style.locale().stable_key())")
        .expect("bundle admission");
    let mutation = ui
        .find("window.set_presentation_palette")
        .expect("window presentation mutation");
    assert!(
        select < mutation,
        "bundle admission must precede window mutation"
    );
}

fn po_entries(catalog: &str) -> std::collections::BTreeMap<&str, &str> {
    let mut entries = std::collections::BTreeMap::new();
    let mut msgid = None;
    for line in catalog.lines() {
        if let Some(value) = line.strip_prefix("msgid ") {
            msgid = unquote(value);
        } else if let Some(value) = line.strip_prefix("msgstr ")
            && let (Some(msgid), Some(msgstr)) = (msgid.take(), unquote(value))
            && !msgid.is_empty()
        {
            entries.insert(msgid, msgstr);
        }
    }
    entries
}

fn po_entry_count(catalog: &str) -> usize {
    catalog
        .lines()
        .filter_map(|line| line.strip_prefix("msgid "))
        .filter_map(unquote)
        .filter(|msgid| !msgid.is_empty())
        .count()
}

fn raw_quoted_literals(line: &str) -> Vec<&str> {
    let mut literals = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = line[cursor..].find('"') {
        let start = cursor + relative_start;
        let value_start = start + 1;
        let Some(relative_end) = line[value_start..].find('"') else {
            break;
        };
        let end = value_start + relative_end;
        if !line[..start].ends_with("@tr(") {
            literals.push(&line[value_start..end]);
        }
        cursor = end + 1;
    }
    literals
}

fn unquote(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn placeholders(value: &str) -> Vec<&str> {
    let mut placeholders = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = value[cursor..].find('{') {
        let start = cursor + relative_start;
        let Some(relative_end) = value[start..].find('}') else {
            break;
        };
        let end = start + relative_end + 1;
        placeholders.push(&value[start..end]);
        cursor = end;
    }
    placeholders
}
