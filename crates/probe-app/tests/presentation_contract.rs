use tokenmaster_m0::presentation::{bounded_chart, pseudo_localize, session_page};

#[test]
fn chart_and_session_models_are_bounded() {
    assert_eq!(bounded_chart((0..10_000).map(f64::from)).len(), 240);
    assert_eq!(session_page(10_000).len(), 256);
}

#[test]
fn bounded_chart_keeps_the_newest_points() {
    let points = bounded_chart((0..300).map(f64::from));
    assert_eq!(points.first(), Some(&60.0));
    assert_eq!(points.last(), Some(&299.0));
}

#[test]
fn pseudo_locale_expands_and_marks_text() {
    assert_eq!(pseudo_localize("Usage"), "［Ûšåğê ···］");
}

#[test]
fn pseudo_locale_expands_long_visible_labels_by_at_least_thirty_five_percent() {
    for label in ["Sessions", "Control center", "Language settings"] {
        let pseudo = pseudo_localize(label);
        let required_content = (label.chars().count() * 135).div_ceil(100);
        assert!(pseudo.starts_with('［'));
        assert!(pseudo.ends_with('］'));
        assert!(pseudo.chars().count() >= required_content + 2);
        assert!(pseudo.chars().count() <= label.chars().count() * 4 + 2);
    }
}
