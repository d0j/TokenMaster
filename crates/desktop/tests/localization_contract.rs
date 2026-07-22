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
    "Presentation persistence %1",
    "Language",
    "English",
    "Russian",
    "Pseudo",
    "Presentation language",
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
fn shell_catalogs_are_complete_and_preserve_placeholders() {
    let expected = SHELL_MSGIDS.into_iter().collect::<BTreeSet<_>>();
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
            "{locale} must translate exactly the G2a1 shell key set"
        );
        for msgid in SHELL_MSGIDS {
            let msgstr = entries.get(msgid).expect("catalog completeness");
            assert_eq!(
                placeholders(msgstr),
                placeholders(msgid),
                "{locale} must preserve placeholders for {msgid:?}"
            );
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

fn unquote(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn placeholders(value: &str) -> Vec<&str> {
    value
        .split_whitespace()
        .filter(|part| part.starts_with('%'))
        .collect()
}
