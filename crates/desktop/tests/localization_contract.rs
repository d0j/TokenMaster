use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;
use std::sync::Mutex;

use slint::{ComponentHandle, Model};
use tokenmaster_desktop::{
    DesktopIntent, DesktopIntentAdmission, DesktopIntentSink, DesktopLocale,
    DesktopPresentationSelection, DesktopReliableStateProjection, DesktopShell, ProjectionStrings,
};
use tokenmaster_product::ProductReducer;

const TRANSLATION_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/translations");
static LOCALE_SWITCH_LOCK: Mutex<()> = Mutex::new(());

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

const SESSIONS_DASHBOARD_MSGIDS: [&str; 47] = [
    "Sessions {0} {1}",
    "Sessions",
    "All-time session summaries",
    "Next page",
    "Back to newest",
    "Loading sessions…",
    "Session summaries",
    "Waiting for session evidence",
    "Session evidence unavailable",
    "Last activity",
    "Tokens",
    "Duration",
    "Input",
    "Cached",
    "Output",
    "Reasoning",
    "From {0} to {1} {2} events {3} tokens {4}",
    "Selected session detail {0}",
    "Selected session",
    "Select a session summary to inspect its exact totals and breakdown.",
    "Loading exact session detail…",
    "This session is no longer present in the current archive snapshot.",
    "Session detail is unavailable. Select the row again to retry.",
    "Duration {0}",
    "Events {0}",
    "Input {0}",
    "Cached {0}",
    "Output {0}",
    "Reasoning {0}",
    "Total {0}",
    "Cost {0}",
    "Model and project breakdown",
    "In {0}",
    "Out {0}",
    "Today's usage",
    "Today",
    "Plan Usage",
    "Banked resets {0}",
    "Banked resets",
    "Credits {0} · Temporary {1} · Unavailable {2}",
    "Code Output",
    "Usage and Cost Trend",
    "Daily token trend",
    "Model Usage",
    "{0} collapsed",
    "{0} (collapsed)",
    "Section collapsed; its data remains available.",
];

const DATA_HEALTH_MSGIDS: [&str; 27] = [
    "Data health and recovery",
    "Data & Recovery",
    "Last success {0} · Last attempt {1} · {2} successful · {3} failed · {4}",
    "Backup",
    "Create normal backup",
    "Compact export",
    "Export maximum compact backup",
    "Verify",
    "Verify all backups",
    "Rebuild",
    "Rebuild local usage data",
    "Encryption passphrase",
    "Confirm passphrase",
    "Confirm encryption passphrase",
    "Encrypted export",
    "Export encrypted backup",
    "Use 12–128 Unicode characters. Values are cleared after admission.",
    "Restore points",
    "Confirm destructive restore",
    "Restore review · {0}",
    "Data only",
    "Confirm restore data only",
    "Data + settings",
    "Confirm restore data and portable settings",
    "Cancel restore review",
    "Generation {0}",
    " · Latest failure {0}",
];

const HELP_ABOUT_MSGIDS: [&str; 29] = [
    "About and licenses. TokenMaster is MIT licensed. WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies. The interface is made with Slint under the selected Royalty-free License 2.0 attribution route.",
    "About and licenses",
    "TokenMaster · MIT",
    "WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies.",
    "TokenMaster Help and About. Version {0}. Local-first usage intelligence with explicit data-source, privacy, health, automation, and license truth.",
    "TokenMaster version {0}. Fast local-first Codex usage, quota, expiry, and recovery visibility with bounded memory and explicit unavailable truth.",
    "TokenMaster",
    "Version {0} · local-first Windows desktop",
    "Fast Codex usage, quota, expiry, and recovery visibility with bounded memory and explicit unavailable truth.",
    "Start here",
    "Dashboard is the quota-first overview. History, Sessions, Models, Projects, and Activity explain usage. Notifications shows current expiry safety.",
    "Data Health owns backup, verification, restore, rebuild, and recovery truth. Settings owns backup policy and portable configuration.",
    "Every missing or stale fact stays explicit — never fabricated as zero.",
    "Data sources and truth",
    "Usage is derived from bounded local Codex history. Plan usage is separate provider evidence from the installed official machine interface when available.",
    "Local tokens never become a guessed provider allowance. Unsupported, stale, partial, or unavailable evidence remains labelled.",
    "No browser session reuse or private endpoint replay.",
    "Privacy by design",
    "No prompts, responses, reasoning, commands, source contents, credentials, or raw absolute paths are retained or exposed.",
    "Frontend rows carry bounded aggregate facts and stable reason codes, not provider, account, workspace, source, session, lot, or delivery identity.",
    "Local-first archive · no listener · no telemetry surface.",
    "Health and recovery",
    "Open Data Health for database health and every backup, verification, restore, rebuild, and recovery operation.",
    "Settings owns backup policy and portable configuration import/export. Stable failure codes omit paths and raw operating-system messages.",
    "Recovery is verified, bounded, and local.",
    "Automation status",
    "CLI and stdio MCP are not available in the current build. No local server or listener is active.",
    "P5 will add strict bounded read-only JSON and stdio MCP for Hermes and other clients. Browser/session automation and automatic benefit activation are not implied.",
    "Current automation authority: none.",
];

const ACTIVITY_MODELS_MSGIDS: [&str; 37] = [
    "Recent activity",
    "Recent activity {0} {1} {2} {3}",
    "Newest archive events · {0}",
    "Waiting for recent activity evidence",
    "Recent activity evidence unavailable",
    "No activity events in the archive",
    "No activity events in the available page",
    "Newest bounded activity events with UTC timestamps",
    "Latest events",
    "UTC time",
    "Model",
    "Relative",
    "Input",
    "Cached",
    "Output",
    "Reasoning",
    "Total",
    "{0} model {1} input {2} cached {3} output {4} reasoning {5} total {6}",
    "In {0} · Cache {1} · Out {2} · Reason {3}",
    "Usage by local hour",
    "Usage by weekday",
    "{0} {1} in {2} {3}",
    "Waiting for rhythm evidence",
    "Rhythm evidence unavailable",
    "Model usage {0} {1} {2} cost {3} {4}",
    "Models",
    "Total tokens",
    "Cost · {0}",
    "Events",
    "Model usage ranked by total tokens",
    "Usage by model",
    "Waiting for model evidence",
    "No model usage in this range",
    "Model evidence unavailable",
    "Cost · evidence",
    "{0} input {1} cached {2} output {3} reasoning {4} total {5} cost {6} {7} events {8}",
    "In {0} · Cache {1} · Out {2} · Reason {3} · {4} events",
];

const HISTORY_MSGIDS: [&str; 28] = [
    "Usage history ",
    "Usage History",
    "1 day",
    "7 days",
    "30 days",
    "History range 1 day",
    "History range 7 days",
    "History range 30 days",
    "Loading history…",
    "Selected range",
    "Total tokens",
    "Cost",
    "Events",
    "Input",
    "Cached",
    "Output",
    "Reasoning",
    "Daily token history",
    "Daily trend",
    "Waiting for history evidence",
    "History evidence unavailable",
    "Thirty day token trend",
    "Daily usage history",
    "Daily details",
    "Date",
    "Tokens",
    "Total",
    " tokens ",
];

const NOTIFICATIONS_MSGIDS: [&str; 22] = [
    "Notifications expiry reminders {0} {1} {2}",
    "Notifications",
    "Expiry reminders · effective in-app coverage",
    "Effective reminder profiles and truthful delivery coverage",
    "Reminder profiles",
    "Waiting for reminder profiles",
    "No reminder profiles",
    "Reminder profiles unavailable",
    "{0} {1} coverage {2} policy {3} leads {4} {5} {6} {7} {8} {9}",
    "Leads · {0}",
    "Current separate benefit lots with expiry precision and evidence",
    "Current benefits",
    "Waiting for benefit inventory",
    "No current benefits",
    "Benefit inventory unavailable",
    "Scope",
    "Benefit",
    "Quantity",
    "State",
    "Expiry",
    "Evidence",
    "{0} {1} {2} quantity {3} state {4} {5} {6} {7}",
];

const PROJECTION_MSGIDS: [&str; 83] = [
    "{0} · {1}",
    "{0} {1} capacity",
    "{0} {1} remaining",
    "{0} {1} used",
    "{0} / {1} {2}",
    "{0} / {1} {2} remaining",
    "{0} / 100 lines",
    "{0} categories · {1} fields",
    "{0} confidence",
    "{0} event",
    "{0} events",
    "{0} remaining",
    "{0} used",
    "Activity",
    "Aging",
    "Authoritative",
    "Code Output",
    "Compact Widget",
    "Complete",
    "Conflict",
    "Dashboard",
    "Data Health",
    "Derived",
    "Efficiency unavailable",
    "Estimated",
    "Evidence unavailable",
    "Expires {0}",
    "Expiry unavailable",
    "Fresh",
    "Help / About",
    "History",
    "In-app reminders",
    "Incomplete",
    "Model Usage",
    "Models",
    "No pending changes",
    "Notifications",
    "Partial",
    "Plan Usage",
    "Projects",
    "Reminder state unavailable",
    "Reminders disabled",
    "Reset time unavailable",
    "Resets {0}",
    "Schema {0}",
    "Schema unavailable",
    "Sessions",
    "Settings",
    "Stale",
    "Time unavailable",
    "Unavailable",
    "Unknown",
    "Usage and Cost Trend",
    "Range unavailable",
    "{0} – before {1}",
    "{0} model loaded",
    "{0} models loaded",
    "Completeness unavailable",
    "More models available",
    "Complete range",
    "{0} project loaded",
    "{0} projects loaded",
    "More projects available",
    "Repositories unavailable",
    "{0} repository loaded",
    "{0} repositories loaded",
    "Code completeness unavailable",
    "Incomplete code range",
    "Complete code range",
    "Complete code",
    "Incomplete code",
    "Code unavailable",
    "Git unavailable",
    "Not linked",
    "Unassociated project",
    "Known",
    "Zero",
    "calculated",
    "reported",
    "mixed",
    "{0} / 100 added product-code lines",
    "{0} repository",
    "{0} repositories",
];

const ACTIVITY_PROJECTION_MSGIDS: [&str; 14] = [
    "UTC timestamps",
    "Page status unavailable",
    "More activity available",
    "First page complete",
    "Time zone unavailable",
    "{0} event loaded",
    "{0} events loaded",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
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
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
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
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
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
fn sessions_and_dashboard_catalogs_are_complete_before_view_conversion() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 432, "projection catalog exact inventory");

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
            "{locale} must translate exactly the closed Task 2b4 Sessions and Dashboard key set"
        );
        assert_eq!(po_entry_count(&catalog), 432, "{locale} exact key count");
        for msgid in SESSIONS_DASHBOARD_MSGIDS {
            let msgstr = entries.get(msgid).expect("Task 2b4 catalog completeness");
            assert!(!msgstr.is_empty(), "{locale} must translate {msgid:?}");
            assert_eq!(
                placeholders(msgstr),
                placeholders(msgid),
                "{locale} must preserve placeholders for {msgid:?}"
            );
        }
    }
}

#[test]
fn data_health_catalog_and_source_use_the_closed_translation_key_set() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 432, "projection catalog exact inventory");

    let data_health = include_str!("../ui/views/data-health-view.slint");
    for msgid in DATA_HEALTH_MSGIDS {
        assert!(
            data_health.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b5a Data Health @tr for {msgid:?}"
        );
    }

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
            "{locale} must translate exactly the closed Task 2b5a Data Health key set"
        );
        assert_eq!(po_entry_count(&catalog), 432, "{locale} exact key count");
        for msgid in DATA_HEALTH_MSGIDS {
            let msgstr = entries.get(msgid).expect("Task 2b5a catalog completeness");
            assert!(!msgstr.is_empty(), "{locale} must translate {msgid:?}");
            assert_eq!(placeholders(msgstr), placeholders(msgid));
        }
    }
}

#[test]
fn help_about_catalog_and_source_use_the_closed_translation_key_set() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 432, "projection catalog exact inventory");

    let help_about = include_str!("../ui/views/help-about-view.slint");
    for msgid in HELP_ABOUT_MSGIDS {
        assert!(
            help_about.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b5b Help/About @tr for {msgid:?}"
        );
    }

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
            "{locale} must translate exactly the closed Task 2b5b Help/About key set"
        );
        assert_eq!(po_entry_count(&catalog), 432, "{locale} exact key count");
        for msgid in HELP_ABOUT_MSGIDS {
            let msgstr = entries.get(msgid).expect("Task 2b5b catalog completeness");
            assert!(!msgstr.is_empty(), "{locale} must translate {msgid:?}");
            assert_eq!(placeholders(msgstr), placeholders(msgid));
        }
    }
}

#[test]
fn activity_and_models_catalog_and_source_use_the_closed_translation_key_set() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 432, "projection catalog exact inventory");

    let activity = include_str!("../ui/views/activity-view.slint");
    let models = include_str!("../ui/views/models-view.slint");
    for msgid in ACTIVITY_MODELS_MSGIDS {
        assert!(
            activity.contains(&format!("@tr(\"{msgid}\""))
                || models.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b6 Activity/Models @tr for {msgid:?}"
        );
    }

    for locale in ["ru", "pseudo"] {
        let catalog = std::fs::read_to_string(
            Path::new(TRANSLATION_ROOT)
                .join(locale)
                .join("LC_MESSAGES")
                .join("tokenmaster-desktop.po"),
        )
        .expect("bundled catalog");
        let entries = po_entries(&catalog);
        assert_eq!(entries.keys().copied().collect::<BTreeSet<_>>(), expected);
        assert_eq!(po_entry_count(&catalog), 432, "{locale} exact key count");
        for msgid in ACTIVITY_MODELS_MSGIDS {
            let msgstr = entries.get(msgid).expect("Task 2b6 catalog completeness");
            assert!(!msgstr.is_empty(), "{locale} must translate {msgid:?}");
            assert_eq!(placeholders(msgstr), placeholders(msgid));
        }
    }

    let ru_catalog = std::fs::read_to_string(
        Path::new(TRANSLATION_ROOT)
            .join("ru")
            .join("LC_MESSAGES")
            .join("tokenmaster-desktop.po"),
    )
    .expect("bundled Russian catalog");
    assert_eq!(
        po_entries(&ru_catalog).get("In {0} · Cache {1} · Out {2} · Reason {3} · {4} events"),
        Some(&"Ввод {0} · Кэш {1} · Вывод {2} · Рассуждения {3} · {4} событий"),
        "Russian narrow Models copy must keep the event count before its noun"
    );
}

#[test]
fn history_catalog_and_source_use_the_closed_translation_key_set() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 432, "projection catalog exact inventory");

    let history = include_str!("../ui/views/history-view.slint");
    for msgid in HISTORY_MSGIDS {
        assert!(
            history.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b7a History @tr for {msgid:?}"
        );
    }

    for locale in ["ru", "pseudo"] {
        let catalog = std::fs::read_to_string(
            Path::new(TRANSLATION_ROOT)
                .join(locale)
                .join("LC_MESSAGES")
                .join("tokenmaster-desktop.po"),
        )
        .expect("bundled catalog");
        let entries = po_entries(&catalog);
        assert_eq!(entries.keys().copied().collect::<BTreeSet<_>>(), expected);
        assert_eq!(po_entry_count(&catalog), 432, "{locale} exact key count");
        for msgid in HISTORY_MSGIDS {
            let msgstr = entries.get(msgid).expect("Task 2b7a catalog completeness");
            assert!(!msgstr.is_empty(), "{locale} must translate {msgid:?}");
            assert_eq!(placeholders(msgstr), placeholders(msgid));
        }
    }
}

#[test]
fn notifications_catalog_and_source_use_the_closed_translation_key_set() {
    let expected = SHELL_MSGIDS
        .into_iter()
        .chain(COMPONENT_MSGIDS)
        .chain(SETTINGS_REMINDER_BACKUP_MSGIDS)
        .chain(SETTINGS_STARTUP_CONFIG_BOARD_FOOTER_MSGIDS)
        .chain(PROJECTS_COMPACT_MSGIDS)
        .chain(SESSIONS_DASHBOARD_MSGIDS)
        .chain(DATA_HEALTH_MSGIDS)
        .chain(HELP_ABOUT_MSGIDS)
        .chain(ACTIVITY_MODELS_MSGIDS)
        .chain(HISTORY_MSGIDS)
        .chain(NOTIFICATIONS_MSGIDS)
        .chain(PROJECTION_MSGIDS)
        .chain(ACTIVITY_PROJECTION_MSGIDS)
        .collect::<BTreeSet<_>>();

    let notifications = include_str!("../ui/views/notifications-view.slint");
    for msgid in NOTIFICATIONS_MSGIDS {
        assert!(
            notifications.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b7b Notifications @tr for {msgid:?}"
        );
    }
    for raw in [
        "text: \"Notifications\"",
        "text: \"Reminder profiles\"",
        "text: \"Current benefits\"",
        "accessible-label: \"Notifications expiry reminders ",
        "accessible-label: \"Effective reminder profiles and truthful delivery coverage\"",
        "accessible-label: \"Current separate benefit lots with expiry precision and evidence\"",
    ] {
        assert!(
            !notifications.contains(raw),
            "unwrapped Task 2b7b Notifications literal {raw:?}"
        );
    }

    for locale in ["ru", "pseudo"] {
        let catalog = std::fs::read_to_string(
            Path::new(TRANSLATION_ROOT)
                .join(locale)
                .join("LC_MESSAGES")
                .join("tokenmaster-desktop.po"),
        )
        .expect("bundled catalog");
        let entries = po_entries(&catalog);
        assert_eq!(entries.keys().copied().collect::<BTreeSet<_>>(), expected);
        for msgid in NOTIFICATIONS_MSGIDS {
            let msgstr = entries.get(msgid).expect("Task 2b7b catalog completeness");
            assert!(!msgstr.is_empty(), "{locale} must translate {msgid:?}");
            assert_eq!(placeholders(msgstr), placeholders(msgid));
        }
    }
}

#[test]
fn pseudo_help_about_preserves_product_dependency_and_license_names() {
    let catalog = std::fs::read_to_string(
        Path::new(TRANSLATION_ROOT)
            .join("pseudo")
            .join("LC_MESSAGES")
            .join("tokenmaster-desktop.po"),
    )
    .expect("bundled pseudo catalog");
    let entries = po_entries(&catalog);
    let protected = [
        ("TokenMaster", &["TokenMaster"][..]),
        ("TokenMaster · MIT", &["TokenMaster", "MIT"][..]),
        (
            "WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies.",
            &["WhereMyTokens", "ccusage", "MIT"][..],
        ),
        (
            "TokenMaster Help and About. Version {0}. Local-first usage intelligence with explicit data-source, privacy, health, automation, and license truth.",
            &["TokenMaster"][..],
        ),
        (
            "TokenMaster version {0}. Fast local-first Codex usage, quota, expiry, and recovery visibility with bounded memory and explicit unavailable truth.",
            &["TokenMaster"][..],
        ),
        (
            "About and licenses. TokenMaster is MIT licensed. WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies. The interface is made with Slint under the selected Royalty-free License 2.0 attribution route.",
            &[
                "TokenMaster",
                "MIT",
                "WhereMyTokens",
                "ccusage",
                "Slint",
                "Royalty-free License 2.0",
            ][..],
        ),
    ];

    for (msgid, names) in protected {
        let msgstr = entries.get(msgid).expect("protected Help/About entry");
        for name in names {
            assert!(
                msgstr.contains(name),
                "pseudo translation for {msgid:?} must preserve {name:?} verbatim"
            );
        }
    }
}

#[test]
fn sessions_and_dashboard_use_the_closed_translation_key_set() {
    let sessions = include_str!("../ui/views/sessions-view.slint");
    let dashboard = include_str!("../ui/views/dashboard-view.slint");

    for msgid in SESSIONS_DASHBOARD_MSGIDS {
        assert!(
            sessions.contains(&format!("@tr(\"{msgid}\""))
                || dashboard.contains(&format!("@tr(\"{msgid}\"")),
            "missing Task 2b4 @tr for {msgid:?}"
        );
    }
    assert!(
        sessions.contains("text: @tr(\"Next page\")"),
        "Task 2b4 must translate the visible Next page control"
    );
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
    let _locale_guard = LOCALE_SWITCH_LOCK.lock().expect("locale switch lock");
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
fn locale_switch_reprojects_closed_history_models_and_projects_projection_labels() {
    let _locale_guard = LOCALE_SWITCH_LOCK.lock().expect("locale switch lock");
    i_slint_backend_testing::init_no_event_loop();
    let sink = Rc::new(RecordingIntentSink {
        selection: Cell::new(None),
    });
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        sink,
    )
    .expect("shell");
    let window = shell.window();

    assert_eq!(window.get_models_range_label(), "Range unavailable");
    assert_eq!(window.get_projects_usage_range_label(), "Range unavailable");
    assert_eq!(window.get_models_loaded_label(), "Unavailable");
    assert_eq!(window.get_projects_loaded_label(), "Unavailable");
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_range_before("2026-07-01".into(), "2026-07-22".into()),
        "2026-07-01 – before 2026-07-22"
    );
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_loaded_label("2".into(), "model".into(), false),
        "2 models loaded"
    );
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_evidence_label("Fresh".into(), "Authoritative".into()),
        "Fresh · Authoritative"
    );
    let history_row_count = window.get_history_day_rows().row_count();
    let model_row_count = window.get_model_usage_rows().row_count();
    let project_row_count = window.get_project_usage_rows().row_count();

    window.invoke_select_presentation_locale(1);

    assert_eq!(window.get_models_range_label(), "Диапазон недоступен");
    assert_eq!(
        window.get_projects_usage_range_label(),
        "Диапазон недоступен"
    );
    assert_eq!(window.get_models_loaded_label(), "Недоступно");
    assert_eq!(window.get_projects_loaded_label(), "Недоступно");
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_range_before("2026-07-01".into(), "2026-07-22".into()),
        "2026-07-01 – до 2026-07-22"
    );
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_loaded_label("2".into(), "model".into(), false),
        "Моделей загружено: 2"
    );
    assert_eq!(
        window.global::<ProjectionStrings>().invoke_loaded_label(
            "21".into(),
            "project".into(),
            false
        ),
        "Проектов загружено: 21"
    );
    assert_eq!(
        window.global::<ProjectionStrings>().invoke_loaded_label(
            "22".into(),
            "repository".into(),
            false
        ),
        "Репозиториев загружено: 22"
    );
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_repository_label("21".into(), false),
        "Репозитории: 21"
    );
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_evidence_label("Свежие".into(), "Авторитетные".into()),
        "Свежие · Авторитетные"
    );
    assert_eq!(window.get_history_day_rows().row_count(), history_row_count);
    assert_eq!(window.get_model_usage_rows().row_count(), model_row_count);
    assert_eq!(
        window.get_project_usage_rows().row_count(),
        project_row_count
    );

    window.invoke_select_presentation_locale(2);
    assert_ne!(window.get_models_range_label(), "Range unavailable");
    assert_ne!(window.get_projects_usage_range_label(), "Range unavailable");
    assert_ne!(window.get_models_loaded_label(), "Unavailable");
    assert_ne!(window.get_projects_loaded_label(), "Unavailable");

    window.invoke_select_presentation_locale(0);
}

#[test]
fn locale_switch_localizes_closed_activity_projection_atoms() {
    let _locale_guard = LOCALE_SWITCH_LOCK.lock().expect("locale switch lock");
    i_slint_backend_testing::init_no_event_loop();
    let sink = Rc::new(RecordingIntentSink {
        selection: Cell::new(None),
    });
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        sink,
    )
    .expect("shell");
    let window = shell.window();
    let strings = window.global::<ProjectionStrings>();

    assert_eq!(strings.invoke_activity_context_label(), "UTC timestamps");
    assert_eq!(
        strings.invoke_activity_loaded_label("2".into(), false),
        "2 events loaded"
    );
    assert_eq!(
        strings.invoke_weekday_label("monday".into()),
        "Monday",
        "known weekday codes have display-only labels"
    );
    assert_eq!(
        strings.invoke_weekday_label("provider_weekday".into()),
        "provider_weekday",
        "unknown codes retain their existing fallback"
    );

    window.invoke_select_presentation_locale(1);
    assert_eq!(strings.invoke_activity_context_label(), "Метки времени UTC");
    assert_eq!(
        strings.invoke_activity_loaded_label("2".into(), false),
        "Загружено событий: 2"
    );
    assert_eq!(strings.invoke_weekday_label("monday".into()), "Понедельник");
    assert_eq!(
        strings.invoke_weekday_label("provider_weekday".into()),
        "provider_weekday"
    );

    window.invoke_select_presentation_locale(2);
    assert_ne!(strings.invoke_activity_context_label(), "UTC timestamps");
    assert_ne!(
        strings.invoke_activity_loaded_label("2".into(), false),
        "2 events loaded"
    );
}

#[test]
fn locale_switch_reprojects_shared_route_labels_without_mutating_route_payload() {
    let _locale_guard = LOCALE_SWITCH_LOCK.lock().expect("locale switch lock");
    i_slint_backend_testing::init_no_event_loop();
    let sink = Rc::new(RecordingIntentSink {
        selection: Cell::new(None),
    });
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        DesktopReliableStateProjection::unavailable(),
        sink,
    )
    .expect("shell");
    let window = shell.window();
    let english_routes = window.get_route_rows();
    let english_dashboard = english_routes.row_data(0).expect("dashboard route");
    let route_count = english_routes.row_count();
    let stable_key = english_dashboard.key.clone();
    let stable_state = english_dashboard.state.clone();

    assert_eq!(english_dashboard.label, "Dashboard");
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_quota_units_remaining_capacity("700".into(), "1,000".into(), "tokens".into()),
        "700 / 1,000 tokens remaining"
    );
    window.invoke_select_presentation_locale(1);

    let russian_routes = window.get_route_rows();
    let russian_dashboard = russian_routes.row_data(0).expect("dashboard route");
    assert_eq!(russian_dashboard.label, "Панель управления");
    assert_eq!(russian_routes.row_count(), route_count);
    assert_eq!(russian_dashboard.key, stable_key);
    assert_eq!(russian_dashboard.state, stable_state);
    assert_eq!(
        window
            .global::<ProjectionStrings>()
            .invoke_quota_units_remaining_capacity("700".into(), "1,000".into(), "tokens".into()),
        "Осталось 700 из 1,000 tokens"
    );
    window.invoke_open_command_palette();
    window.invoke_command_palette_query_edited("история".into());
    let localized_palette_rows = window.get_command_palette_rows();
    assert_eq!(localized_palette_rows.row_count(), 1);
    assert_eq!(
        localized_palette_rows
            .row_data(0)
            .expect("localized History route")
            .label,
        "История"
    );
    window.invoke_dismiss_command_palette();

    window.invoke_select_presentation_locale(2);
    let pseudo_dashboard = window
        .get_route_rows()
        .row_data(0)
        .expect("dashboard route");
    assert_ne!(pseudo_dashboard.label, "Dashboard");
    assert_eq!(pseudo_dashboard.key, stable_key);
    assert_eq!(pseudo_dashboard.state, stable_state);

    window.invoke_select_presentation_locale(0);
}

#[test]
fn shell_and_presentation_strip_use_only_the_closed_translation_key_set() {
    let main = include_str!("../ui/main.slint");
    let projection_strings = include_str!("../ui/projection-strings.slint");
    let settings = include_str!("../ui/views/settings-view.slint");
    let ui = include_str!("../src/ui.rs");

    for msgid in SHELL_MSGIDS {
        assert!(
            main.contains(&format!("@tr(\"{msgid}\""))
                || settings.contains(&format!("@tr(\"{msgid}\"")),
            "missing @tr for {msgid:?}"
        );
    }
    for msgid in PROJECTION_MSGIDS {
        assert!(
            projection_strings.contains(&format!("@tr(\"{msgid}\"")),
            "missing ProjectionStrings @tr for {msgid:?}"
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
