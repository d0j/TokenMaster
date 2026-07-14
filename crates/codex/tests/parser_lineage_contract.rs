use tokenmaster_codex::{
    ParseContext, ParseOutcome, ParserDiagnosticCode, ParserDiagnostics, ParserState, parse_line,
};
use tokenmaster_domain::{
    ObservationDraft, SessionRelationDraft, TokenCount, UsageProfileId, UsageSessionId,
    UsageSourceId,
};

fn context() -> ParseContext {
    ParseContext::new(
        UsageProfileId::new("default").expect("valid profile"),
        UsageSourceId::new("source_fixture").expect("valid source"),
        Some(UsageSessionId::new("child").expect("valid filename session")),
        UsageSessionId::new("hashed").expect("valid hashed session"),
    )
}

fn relation(
    state: &mut ParserState,
    diagnostics: &mut ParserDiagnostics,
    line: &[u8],
) -> SessionRelationDraft {
    match parse_line(&context(), state, diagnostics, 11, line) {
        ParseOutcome::SessionRelation(relation) => relation,
        other => panic!("session relation expected, got {other:?}"),
    }
}

fn usage(
    state: &mut ParserState,
    diagnostics: &mut ParserDiagnostics,
    offset: u64,
    line: &[u8],
) -> ObservationDraft {
    match parse_line(&context(), state, diagnostics, offset, line) {
        ParseOutcome::Emitted(draft) => draft,
        other => panic!("usage draft expected, got {other:?}"),
    }
}

#[test]
fn ancestry_shapes_merge_without_precedence_and_conflicts_are_explicit() {
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let duplicate = relation(
        &mut state,
        &mut diagnostics,
        br#"{"type":"session_meta","forked_from_id":"parent-a","parent_thread_id":"parent-a","payload":{"id":"child","forked_from_id":"parent-a","parent_thread_id":"parent-a","source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent-a"}}}}}"#,
    );
    assert_eq!(duplicate.parent_session_id().as_str(), "parent-a");
    assert!(!duplicate.declared_conflict());

    let mut structured_state = ParserState::new();
    let structured = relation(
        &mut structured_state,
        &mut diagnostics,
        br#"{"type":"session_meta","payload":{"id":"child","source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent-structured"}}}}}"#,
    );
    assert_eq!(structured.parent_session_id().as_str(), "parent-structured");
    assert!(!structured.declared_conflict());

    let mut conflict_state = ParserState::new();
    let conflict = relation(
        &mut conflict_state,
        &mut diagnostics,
        br#"{"type":"session_meta","forked_from_id":"parent-a","payload":{"id":"child","parent_thread_id":"parent-b"}}"#,
    );
    assert_eq!(conflict.parent_session_id().as_str(), "parent-a");
    assert!(conflict.declared_conflict());

    let mut self_state = ParserState::new();
    let self_parent = relation(
        &mut self_state,
        &mut diagnostics,
        br#"{"type":"session_meta","payload":{"id":"child","parent_thread_id":"child"}}"#,
    );
    assert!(self_parent.declared_conflict());
}

#[test]
fn late_ancestry_is_emitted_separately_and_applies_to_later_ordinals() {
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let first = usage(
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"model":"gpt-test","usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12}}"#,
    );
    assert_eq!(first.provider_id().as_str(), "codex");
    assert_eq!(first.session_ordinal(), 0);
    assert!(first.parent_session_id().is_none());

    let late = relation(
        &mut state,
        &mut diagnostics,
        br#"{"type":"session_meta","payload":{"id":"child","forked_from_id":"parent"}}"#,
    );
    assert_eq!(late.session_id().as_str(), "child");
    assert_eq!(late.parent_session_id().as_str(), "parent");

    let second = usage(
        &mut state,
        &mut diagnostics,
        20,
        br#"{"timestamp":1700000001,"model":"gpt-test","usage":{"input_tokens":11,"output_tokens":3,"total_tokens":14}}"#,
    );
    assert_eq!(second.session_ordinal(), 1);
    assert_eq!(
        second.parent_session_id().map(UsageSessionId::as_str),
        Some("parent")
    );
}

#[test]
fn cumulative_snapshot_and_invalid_ancestry_remain_explicit_and_private() {
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let event = usage(
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12},"total_token_usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":20,"reasoning_tokens":5,"total_tokens":125}}}}"#,
    );
    let cumulative = event
        .cumulative_usage()
        .expect("provider cumulative snapshot retained");
    assert_eq!(cumulative.input(), TokenCount::Available(100));
    assert_eq!(cumulative.total(), TokenCount::Available(125));

    let invalid = br#"{"type":"session_meta","payload":{"id":"child","forked_from_id":{"private":"DO_NOT_LEAK"},"parent_thread_id":"bad\nparent"}}"#;
    let outcome = parse_line(&context(), &mut state, &mut diagnostics, 100, invalid);
    assert!(matches!(outcome, ParseOutcome::MetadataOnly));
    assert!(diagnostics.count(ParserDiagnosticCode::InvalidMetadata) >= 2);
    let debug = format!("{outcome:?} {diagnostics:?} {state:?}");
    assert!(!debug.contains("DO_NOT_LEAK"));
    assert!(!debug.contains("bad\nparent"));
}
