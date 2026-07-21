#[test]
fn slint_uses_one_complete_palette_input_without_skin_selection_logic() {
    let tokens = include_str!("../ui/tokens.slint");

    assert!(tokens.contains("export struct UiPalette"));
    assert!(tokens.contains("in-out property <UiPalette> palette"));
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
        assert!(
            tokens.contains(&format!("{role}: palette.{role}")),
            "{role} must alias palette"
        );
    }
    for forbidden in ["Refined", "Graphite", "Ember", "skin-id", "palette-id"] {
        assert!(
            !tokens.contains(forbidden),
            "Slint must not own {forbidden}"
        );
    }
}
