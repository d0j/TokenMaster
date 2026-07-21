#[test]
fn slint_uses_one_complete_palette_input_without_skin_selection_logic() {
    let tokens = include_str!("../ui/tokens.slint");

    assert_eq!(count(tokens, "export struct UiPalette"), 1);
    assert_eq!(count(tokens, "export struct"), 1);
    assert_eq!(count(tokens, "export global UiTokens"), 1);
    assert_eq!(count(tokens, "export global"), 1);
    assert_eq!(count(tokens, "in-out property <UiPalette> palette"), 1);
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
        assert_eq!(
            count(
                tokens,
                &format!("out property <color> {role}: palette.{role};")
            ),
            1,
            "{role} must be exactly one palette alias"
        );
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
    ] {
        assert!(
            !tokens.contains(forbidden),
            "Slint must not own {forbidden}"
        );
    }
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}
