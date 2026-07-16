use tokenmaster_domain::{AppState, LayoutId, LocaleId, RouteId, SessionSummary, ThemeId};

#[test]
fn ten_thousand_switches_preserve_route_and_selection() {
    let mut state = AppState::default();
    state.navigate(RouteId::Sessions);
    state.select_session(Some(731));

    for index in 0..10_000 {
        state.switch_layout(LayoutId::ALL[index % LayoutId::ALL.len()]);
        state.switch_theme(ThemeId::ALL[index % ThemeId::ALL.len()]);
        state.switch_locale(LocaleId::ALL[index % LocaleId::ALL.len()]);
    }

    assert_eq!(state.route(), RouteId::Sessions);
    assert_eq!(state.selected_session(), Some(731));
    assert_eq!(state.revision(), 29_999);
}

#[test]
fn assigning_current_skin_is_revision_neutral() {
    let mut state = AppState::default();
    state.switch_layout(LayoutId::Refined);
    state.switch_theme(ThemeId::Midnight);
    state.switch_locale(LocaleId::English);
    assert_eq!(state.revision(), 0);
}

#[test]
fn session_summary_exposes_metadata_only_fields() {
    let session = SessionSummary {
        id: 7,
        started_at_ms: 1_800_000_000,
        total_tokens: 42,
        model_key: "gpt-5".to_owned(),
    };

    assert_eq!(session.id, 7);
    assert_eq!(session.started_at_ms, 1_800_000_000);
    assert_eq!(session.total_tokens, 42);
    assert_eq!(session.model_key, "gpt-5");
}
