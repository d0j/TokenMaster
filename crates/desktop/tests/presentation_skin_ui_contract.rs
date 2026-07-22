use std::{cell::Cell, rc::Rc};

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::{ComponentHandle, Model, SharedString};
use tokenmaster_desktop::{
    DesktopBackupPolicy, DesktopDensity, DesktopIntent, DesktopIntentAdmission, DesktopIntentSink,
    DesktopOperationKind, DesktopOperationPhase, DesktopOperationSnapshot,
    DesktopPresentationSelection, DesktopPresentationSettings, DesktopReliableStateHealth,
    DesktopReliableStateInput, DesktopReliableStateProjection, DesktopReliableStateSummary,
    DesktopReminderPolicy, DesktopShell, DesktopSkin,
};
use tokenmaster_product::ProductReducer;

struct RecordingIntentSink {
    admission: DesktopIntentAdmission,
    count: Cell<u64>,
    last: Cell<Option<DesktopPresentationSelection>>,
}

impl RecordingIntentSink {
    fn accepting() -> Self {
        Self {
            admission: DesktopIntentAdmission::Started,
            count: Cell::new(0),
            last: Cell::new(None),
        }
    }

    fn rejecting() -> Self {
        Self {
            admission: DesktopIntentAdmission::Rejected,
            count: Cell::new(0),
            last: Cell::new(None),
        }
    }
}

impl DesktopIntentSink for RecordingIntentSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        if let DesktopIntent::UpdatePresentation(selection) = intent {
            self.count.set(self.count.get() + 1);
            self.last.set(Some(selection));
        }
        self.admission
    }
}

fn reliable_state(
    density: DesktopDensity,
    skin: DesktopSkin,
    operation: Option<DesktopOperationSnapshot>,
) -> DesktopReliableStateProjection {
    let summary = DesktopReliableStateSummary::new_with_settings(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::disabled(),
        DesktopReminderPolicy::unavailable(),
        DesktopPresentationSettings::new(
            density,
            skin,
            tokenmaster_desktop::DesktopColorScheme::System,
        ),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        operation,
        None,
    );
    DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        1,
        summary,
        Vec::new(),
    ))
}

#[test]
fn skin_selector_applies_all_fifteen_exact_palette_roles_after_admission() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state(DesktopDensity::Comfortable, DesktopSkin::Refined, None),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    assert_eq!(window.get_presentation_skin_key(), "refined");
    assert_palette(window.get_presentation_palette(), DesktopSkin::Refined);

    window.invoke_select_presentation_skin(1);

    assert_eq!(sink.count.get(), 1);
    assert_eq!(
        sink.last.get(),
        Some(DesktopPresentationSelection::new(
            DesktopDensity::Comfortable,
            DesktopSkin::Graphite,
            tokenmaster_desktop::DesktopColorScheme::System
        ))
    );
    assert_eq!(window.get_presentation_skin_key(), "graphite");
    assert_palette(window.get_presentation_palette(), DesktopSkin::Graphite);

    window.invoke_select_presentation_skin(2);
    assert_eq!(window.get_presentation_skin_key(), "ember");
    assert_palette(window.get_presentation_palette(), DesktopSkin::Ember);
}

#[test]
fn invalid_or_rejected_skin_index_leaves_every_window_field_unchanged() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::rejecting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state(DesktopDensity::Compact, DesktopSkin::Graphite, None),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    let before = window_fields(window);

    window.invoke_select_presentation_skin(-1);
    window.invoke_select_presentation_skin(3);
    window.invoke_select_presentation_skin(2);

    assert_eq!(sink.count.get(), 1, "only valid skin submits");
    assert_eq!(window_fields(window), before);
}

#[test]
fn density_and_skin_selectors_submit_complete_pairs_and_keep_one_window_models_geometry() {
    i_slint_backend_testing::init_no_event_loop();

    let sink = Rc::new(RecordingIntentSink::accepting());
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state(DesktopDensity::Comfortable, DesktopSkin::Refined, None),
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    let address = window as *const _;
    let routes = window.get_route_rows().row_count();
    let quotas = window.get_dashboard_quota_rows().row_count();
    let size = window.window().size();

    window.invoke_select_presentation_density(1);
    assert_eq!(
        sink.last.get(),
        Some(DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Refined,
            tokenmaster_desktop::DesktopColorScheme::System
        ))
    );
    window.invoke_select_presentation_skin(2);
    assert_eq!(
        sink.last.get(),
        Some(DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Ember,
            tokenmaster_desktop::DesktopColorScheme::System
        ))
    );

    for index in 0..10_000 {
        let skin = if index % 2 == 0 {
            DesktopSkin::Graphite
        } else {
            DesktopSkin::Ember
        };
        window.invoke_select_presentation_skin(skin.slint_index());
        window.invoke_select_presentation_density(index % 3);
        assert_eq!(
            sink.last.get(),
            Some(DesktopPresentationSelection::new(
                match index % 3 {
                    0 => DesktopDensity::Comfortable,
                    1 => DesktopDensity::Compact,
                    _ => DesktopDensity::UltraCompact,
                },
                skin,
                tokenmaster_desktop::DesktopColorScheme::System,
            )),
            "density submission {index} must retain the immediately current skin"
        );
    }

    assert_eq!(window.get_presentation_density_key(), "comfortable");
    assert_eq!(window.get_presentation_skin_key(), "ember");
    assert_eq!(address, shell.window() as *const _);
    assert_eq!(window.get_route_rows().row_count(), routes);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), quotas);
    assert_eq!(window.window().size(), size);
    assert_eq!(sink.count.get(), 20_002);
}

#[test]
fn skin_selector_is_exactly_one_accessible_combobox_and_stays_in_bounds() {
    i_slint_backend_testing::init_no_event_loop();

    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state(DesktopDensity::Comfortable, DesktopSkin::Graphite, None),
        Rc::new(RecordingIntentSink::accepting()),
    )
    .expect("shell");
    let window = shell.window();
    window.invoke_select_route(SharedString::from("settings"));
    window.show().expect("show settings");
    let scale_factor = window.window().scale_factor();

    for (width, height, layout) in [
        (560, 480, "narrow"),
        (700, 900, "narrow"),
        (1120, 1000, "wide"),
    ] {
        window.window().set_size(
            slint::LogicalSize::new(width as f32, height as f32).to_physical(scale_factor),
        );
        let actual_size = window.window().size().to_logical(scale_factor);
        let settings = ElementHandle::find_by_accessible_label(window, "TokenMaster settings")
            .find(|element| element.accessible_role() == Some(AccessibleRole::Groupbox))
            .expect("settings root");
        let settings_position = settings.absolute_position();
        let settings_size = settings.size();
        let strip = ElementHandle::find_by_element_id(window, "SettingsView::presentation-strip")
            .next()
            .expect("presentation strip");
        let strip_position = strip.absolute_position();
        let strip_size = strip.size();
        assert!(
            strip_position.x >= 0.0
                && strip_position.y >= 0.0
                && strip_position.x + strip_size.width <= actual_size.width
                && strip_position.y + strip_size.height <= actual_size.height,
            "presentation strip fits actual {}x{}",
            actual_size.width,
            actual_size.height,
        );
        assert_eq!(window.get_settings_layout_mode(), layout);
        let selectors = ElementQuery::from_root(window)
            .match_accessible_role(AccessibleRole::Combobox)
            .match_predicate(|element| {
                element.accessible_label().as_deref() == Some("Presentation skin")
            })
            .find_all();
        assert_eq!(selectors.len(), 1, "one Skin selector at {width}x{height}");
        let selector = ElementHandle::find_by_accessible_label(window, "Presentation skin")
            .find(|element| element.accessible_role() == Some(AccessibleRole::Combobox))
            .expect("skin selector");
        let position = selector.absolute_position();
        let size = selector.size();
        assert!(
            size.width > 0.0 && size.height > 0.0,
            "skin selector has bounds"
        );
        assert!(
            position.x >= 0.0 && position.y >= 0.0,
            "skin selector starts in bounds"
        );
        assert!(
            position.x + size.width <= actual_size.width
                && position.y + size.height <= actual_size.height,
            "skin selector fits requested {width}x{height} and actual {}x{}; SettingsView x={} y={} width={} height={}; selector x={} y={} width={} height={}",
            actual_size.width,
            actual_size.height,
            settings_position.x,
            settings_position.y,
            settings_size.width,
            settings_size.height,
            position.x,
            position.y,
            size.width,
            size.height,
        );
        for (label, role) in [
            ("Presentation density", AccessibleRole::Combobox),
            ("Presentation skin", AccessibleRole::Combobox),
            ("Presentation persistence Saved", AccessibleRole::Text),
        ] {
            let element = ElementHandle::find_by_accessible_label(window, label)
                .find(|element| element.accessible_role() == Some(role))
                .expect("presentation strip control");
            let position = element.absolute_position();
            let size = element.size();
            assert!(
                position.x >= strip_position.x
                    && position.y >= strip_position.y
                    && position.x + size.width <= strip_position.x + strip_size.width
                    && position.y + size.height <= strip_position.y + strip_size.height,
                "{label} stays in the presentation strip at {width}x{height}"
            );
        }
    }
}

#[test]
fn slint_owns_exactly_one_complete_palette_without_family_logic() {
    let tokens = include_str!("../ui/tokens.slint");
    assert_palette_ownership(tokens).expect("one complete Rust-owned palette input");
}

#[test]
fn palette_ownership_guard_rejects_a_second_family_branch() {
    let tokens = include_str!("../ui/tokens.slint");
    let bypass = tokens.replacen(
        "export global UiTokens",
        "export global UiPaletteFamily { in-out property <UiPalette> palette; }\n\nexport global UiTokens",
        1,
    );
    assert!(
        assert_palette_ownership(&bypass).is_err(),
        "a second Slint palette family must fail closed"
    );
}

#[test]
fn palette_ownership_guard_rejects_a_second_palette_typed_property() {
    let tokens = include_str!("../ui/tokens.slint");
    let bypass = tokens.replacen(
        "    in-out property <int> density-id: 0;",
        "    in-out property <UiPalette> secondary: palette;\n    in-out property <int> density-id: 0;",
        1,
    );
    assert!(
        assert_palette_ownership(&bypass).is_err(),
        "a second UiPalette property must fail closed"
    );
}

#[test]
fn palette_ownership_guard_rejects_an_extra_brush_alias() {
    let tokens = include_str!("../ui/tokens.slint");
    let bypass = tokens.replacen(
        "    out property <length> space-xs:",
        "    out property <brush> shadow: root.palette.shadow;\n    out property <length> space-xs:",
        1,
    );
    assert!(
        assert_palette_ownership(&bypass).is_err(),
        "an extra palette brush alias must fail closed"
    );
}

#[test]
fn palette_ownership_guard_rejects_a_conditional_family_initializer() {
    let tokens = include_str!("../ui/tokens.slint");
    let bypass = tokens
        .replacen(
            "    in-out property <UiPalette> palette: {",
            "    in-out property <UiPalette> palette: family-id == 0 ? {",
            1,
        )
        .replacen(
            "    };\n    in-out property <int> density-id: 0;",
            "    } : {\n        background: #17110b,\n        surface: #271811,\n        surface-raised: #342218,\n        surface-subtle: #24160e,\n        border: #483529,\n        text-primary: #fbf7f4,\n        text-secondary: #c0ab9e,\n        accent: #fdd47c,\n        accent-subtle: #443017,\n        accent-secondary: #fa8ba7,\n        accent-tertiary: #fcaaf0,\n        ready: #a5d670,\n        waiting: #bfaa8f,\n        degraded: #f2c66d,\n        unavailable: #f08b8b,\n    };\n    in-out property <int> family-id: 0;\n    in-out property <int> density-id: 0;",
            1,
        );
    assert!(
        assert_palette_ownership(&bypass).is_err(),
        "a conditional palette family initializer must fail closed"
    );
}

#[test]
fn palette_ownership_guard_rejects_a_spaced_renamed_family_initializer() {
    let tokens = include_str!("../ui/tokens.slint");
    let bypass = conditional_palette_initializer(
        tokens,
        "variant-choice /* renamed family */ ==\n        0",
        "variant-choice",
    );
    let error = assert_palette_ownership(&bypass).expect_err("conditional palette initializer");
    assert!(
        error.contains("direct UiPalette struct initializer"),
        "the palette initializer guard, not a literal family name, must reject the mutation: {error}"
    );
}

#[test]
fn palette_ownership_guard_tolerates_whitespace_and_comments() {
    let tokens = include_str!("../ui/tokens.slint");
    let formatted = tokens
        .replacen(
            "export struct UiPalette {",
            "export /* palette */ struct\nUiPalette\n{",
            1,
        )
        .replacen(
            "in-out property <UiPalette> palette:",
            "in-out\nproperty <\nUiPalette\n> palette:",
            1,
        )
        .replacen(
            "out property <color> background: palette.background;",
            "out /* alias */ property < color > background: palette /* role */.background;",
            1,
        );
    assert_palette_ownership(&formatted).expect("lexical palette ownership guard");
}

#[test]
fn stale_terminal_cannot_replace_new_skin_but_import_and_portable_restore_replace_both_axes() {
    i_slint_backend_testing::init_no_event_loop();

    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        reliable_state(DesktopDensity::Comfortable, DesktopSkin::Refined, None),
        Rc::new(RecordingIntentSink::accepting()),
    )
    .expect("shell");
    let window = shell.window();
    window.invoke_select_presentation_skin(2);
    shell
        .apply_reliable_state(reliable_state(
            DesktopDensity::Comfortable,
            DesktopSkin::Refined,
            Some(DesktopOperationSnapshot::new(
                DesktopOperationKind::UpdatePresentation,
                DesktopOperationPhase::Succeeded,
                false,
                None,
            )),
        ))
        .expect("stale terminal");
    assert_eq!(window.get_presentation_skin_key(), "ember");
    assert_eq!(window.get_presentation_persistence_state(), "saving");

    for kind in [
        DesktopOperationKind::ApplyConfig,
        DesktopOperationKind::RestoreWithPortableSettings,
    ] {
        shell
            .apply_reliable_state(reliable_state(
                DesktopDensity::UltraCompact,
                DesktopSkin::Graphite,
                Some(DesktopOperationSnapshot::new(
                    kind,
                    DesktopOperationPhase::Succeeded,
                    false,
                    None,
                )),
            ))
            .expect("complete override");
        assert_eq!(window.get_presentation_density_key(), "ultra_compact");
        assert_eq!(window.get_presentation_skin_key(), "graphite");
        assert_eq!(window.get_presentation_persistence_state(), "saved");
    }
}

#[test]
fn selector_and_atomic_palette_source_contracts_are_stable() {
    let main = include_str!("../ui/main.slint");
    let settings = include_str!("../ui/views/settings-view.slint");
    let ui = include_str!("../src/ui.rs");

    assert!(main.contains("in-out property <int> presentation-skin-id"));
    assert!(main.contains("presentation-skin-key"));
    assert_eq!(count(main, "callback select-presentation-skin(int);"), 1);
    assert_eq!(
        count(
            main,
            "select-presentation-skin(index) => { root.select-presentation-skin(index); }"
        ),
        1
    );
    assert!(settings.contains("presentation-skin-id"));
    assert_eq!(
        count(settings, "callback select-presentation-skin(int);"),
        1
    );
    assert!(settings.contains("model: [\"Refined\", \"Graphite\", \"Ember\"]"));
    assert!(settings.contains("accessible-label: \"Presentation skin\""));
    assert!(settings.contains("Not saved — choose the current presentation again to retry"));
    assert_eq!(count(ui, "window.set_presentation_palette"), 1);
    let palette = ui
        .find("set_presentation_palette")
        .expect("palette assignment");
    let final_metadata = ui
        .find("set_presentation_persistence_state")
        .expect("persistence metadata");
    for metadata in [
        "set_presentation_skin_id",
        "set_presentation_density_id",
        "set_presentation_revision",
        "set_presentation_persistence_state",
    ] {
        assert!(
            palette < ui.find(metadata).expect("presentation metadata"),
            "palette must precede {metadata}"
        );
    }
    let assignment = &ui[palette..final_metadata];
    for forbidden in [
        "invoke_from_event_loop(",
        ".await",
        "thread::",
        "Timer::",
        "\n    window.on_",
    ] {
        assert!(
            !assignment.contains(forbidden),
            "palette-to-metadata sequence must not yield via {forbidden}"
        );
    }
}

fn window_fields(window: &tokenmaster_desktop::MainWindow) -> (String, String, String, String) {
    (
        window.get_presentation_density_key().to_string(),
        window.get_presentation_skin_key().to_string(),
        window.get_presentation_revision().to_string(),
        window.get_presentation_persistence_state().to_string(),
    )
}

fn assert_palette(palette: tokenmaster_desktop::UiPalette, skin: DesktopSkin) {
    let expected = skin
        .color_tokens(tokenmaster_desktop::DesktopEffectiveColorScheme::Dark)
        .rgb_roles();
    let actual = [
        palette.background,
        palette.surface,
        palette.surface_raised,
        palette.surface_subtle,
        palette.border,
        palette.text_primary,
        palette.text_secondary,
        palette.accent,
        palette.accent_subtle,
        palette.accent_secondary,
        palette.accent_tertiary,
        palette.ready,
        palette.waiting,
        palette.degraded,
        palette.unavailable,
    ];
    for (role, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
        assert_eq!(
            actual,
            slint::Color::from_rgb_u8(expected.red(), expected.green(), expected.blue()),
            "role {role} for {}",
            skin.stable_key()
        );
    }
}

fn assert_palette_ownership(tokens: &str) -> Result<(), String> {
    let tokens = compact_slint_source(tokens);
    if count(&tokens, "exportstructUiPalette{") != 1 || count(&tokens, "exportstruct") != 1 {
        return Err(String::from("exactly one UiPalette struct"));
    }
    if count(&tokens, "exportglobalUiTokens{") != 1 || count(&tokens, "exportglobal") != 1 {
        return Err(String::from("exactly one UiTokens global"));
    }
    if count(&tokens, "property<UiPalette>") != 1
        || count(&tokens, "in-outproperty<UiPalette>palette:") != 1
    {
        return Err(String::from(
            "exactly one complete palette input and assignment",
        ));
    }
    let roles = [
        "background",
        "surface",
        "surface-raised",
        "surface-subtle",
        "border",
        "text-primary",
        "text-secondary",
        "accent",
        "accent-subtle",
        "accent-secondary",
        "accent-tertiary",
        "ready",
        "waiting",
        "degraded",
        "unavailable",
    ];
    if count(&tokens, "property<color>") + count(&tokens, "property<brush>") != roles.len() {
        return Err(String::from("exactly fifteen palette brush aliases"));
    }
    for role in roles {
        if count(
            &tokens,
            &format!("outproperty<color>{role}:palette.{role};"),
        ) != 1
        {
            return Err(format!("{role} must be one palette alias"));
        }
    }
    let initializer = palette_initializer(&tokens)?;
    if initializer.contains('?')
        || initializer.contains("==")
        || initializer.contains("!=")
        || ["if", "match", "family", "skin", "theme", "palette"]
            .into_iter()
            .any(|identifier| contains_identifier_component(initializer, identifier))
    {
        return Err(String::from(
            "UiPalette initializer must not select a palette family",
        ));
    }
    for forbidden in ["Refined", "Graphite", "Ember", "UiPaletteFamily"] {
        if tokens.contains(forbidden) {
            return Err(format!("Slint must not own {forbidden}"));
        }
    }
    Ok(())
}

fn palette_initializer(tokens: &str) -> Result<&str, String> {
    let assignment = "in-outproperty<UiPalette>palette:";
    let initializer = tokens
        .find(assignment)
        .map(|index| &tokens[index + assignment.len()..])
        .ok_or_else(|| String::from("UiPalette palette assignment"))?;
    if !initializer.starts_with('{') {
        return Err(String::from(
            "UiPalette must use a direct UiPalette struct initializer",
        ));
    }
    let end = matching_brace(initializer)
        .ok_or_else(|| String::from("balanced UiPalette struct initializer"))?;
    if !initializer[end + 1..].starts_with(';') {
        return Err(String::from("terminated UiPalette struct initializer"));
    }
    Ok(&initializer[1..end])
}

fn matching_brace(source: &str) -> Option<usize> {
    let mut depth = 0_u32;
    for (index, character) in source.char_indices() {
        match character {
            '{' => depth = depth.checked_add(1)?,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn contains_identifier_component(source: &str, identifier: &str) -> bool {
    source
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|component| component == identifier)
}

fn conditional_palette_initializer(tokens: &str, condition: &str, selector: &str) -> String {
    tokens
        .replacen(
            "    in-out property <UiPalette> palette: {",
            &format!("    in-out property <UiPalette> palette: {condition} ? {{"),
            1,
        )
        .replacen(
            "    };\n    in-out property <int> density-id: 0;",
            &format!("    }} : {{\n        background: #17110b,\n        surface: #271811,\n        surface-raised: #342218,\n        surface-subtle: #24160e,\n        border: #483529,\n        text-primary: #fbf7f4,\n        text-secondary: #c0ab9e,\n        accent: #fdd47c,\n        accent-subtle: #443017,\n        accent-secondary: #fa8ba7,\n        accent-tertiary: #fcaaf0,\n        ready: #a5d670,\n        waiting: #bfaa8f,\n        degraded: #f2c66d,\n        unavailable: #f08b8b,\n    }};\n    in-out property <int> {selector}: 0;\n    in-out property <int> density-id: 0;"),
            1,
        )
}

fn compact_slint_source(source: &str) -> String {
    let mut compact = String::new();
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '"' || character == '\'' {
            let quote = character;
            let mut escaped = false;
            for quoted_character in chars.by_ref() {
                if escaped {
                    escaped = false;
                } else if quoted_character == '\\' {
                    escaped = true;
                } else if quoted_character == quote {
                    break;
                }
            }
        } else if character == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for comment_character in chars.by_ref() {
                if comment_character == '\n' {
                    break;
                }
            }
        } else if character == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for comment_character in chars.by_ref() {
                if previous == '*' && comment_character == '/' {
                    break;
                }
                previous = comment_character;
            }
        } else if !character.is_whitespace() {
            compact.push(character);
        }
    }
    compact
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}
