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
        DesktopPresentationSettings::new(density, skin),
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
        ))
    );
    window.invoke_select_presentation_skin(2);
    assert_eq!(
        sink.last.get(),
        Some(DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Ember,
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
    let expected = skin.color_tokens().rgb_roles();
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
    if count(tokens, "export struct UiPalette") != 1 || count(tokens, "export struct") != 1 {
        return Err(String::from("exactly one UiPalette struct"));
    }
    if count(tokens, "export global UiTokens") != 1 || count(tokens, "export global") != 1 {
        return Err(String::from("exactly one UiTokens global"));
    }
    if count(tokens, "in-out property <UiPalette> palette") != 1 || count(tokens, "palette:") != 1 {
        return Err(String::from(
            "exactly one complete palette input and assignment",
        ));
    }
    for role in [
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
    ] {
        if count(
            tokens,
            &format!("out property <color> {role}: palette.{role};"),
        ) != 1
        {
            return Err(format!("{role} must be one palette alias"));
        }
    }
    for forbidden in [
        "Refined",
        "Graphite",
        "Ember",
        "skin-id",
        "palette-id",
        "palette ==",
        "palette !=",
        "palette ?",
        "palette :",
        "if palette",
        "match palette",
        "UiPaletteFamily",
    ] {
        if tokens.contains(forbidden) {
            return Err(format!("Slint must not own {forbidden}"));
        }
    }
    Ok(())
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}
