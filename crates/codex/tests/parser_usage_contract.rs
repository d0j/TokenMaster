use tokenmaster_accounting::{Canonicalizer, EVENT_FINGERPRINT_VERSION};
use tokenmaster_codex::{
    LONG_CONTEXT_THRESHOLD, MAX_LINE_BYTES, ParseContext, ParseOutcome, ParserDiagnosticCode,
    ParserDiagnostics, ParserState, parse_line,
};
use tokenmaster_domain::{
    LongContextState, ObservationDraft, TokenCount, UsageProfileId, UsageSessionId, UsageSourceId,
};

fn context() -> ParseContext {
    let profile = UsageProfileId::new("profile_fixture").expect("valid profile");
    let source = UsageSourceId::new("source_fixture").expect("valid source");
    let session = UsageSessionId::new("session_fixture").expect("valid session");
    ParseContext::new(profile, source, Some(session.clone()), session)
}

fn emitted(
    context: &ParseContext,
    state: &mut ParserState,
    diagnostics: &mut ParserDiagnostics,
    offset: u64,
    line: &[u8],
) -> ObservationDraft {
    match parse_line(context, state, diagnostics, offset, line) {
        ParseOutcome::Emitted(event) => event,
        _ => panic!("event expected"),
    }
}

#[test]
fn top_level_usage_preserves_availability_and_caps_cached() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let line = br#"{"timestamp":"2026-07-10T08:00:00.123Z","model":"gpt-5.6-sol","usage":{"input_tokens":10,"cached_input_tokens":15,"output_tokens":2,"total_tokens":12}}"#;

    let event = emitted(&context, &mut state, &mut diagnostics, 17, line);

    assert_eq!(event.usage().input(), TokenCount::Available(10));
    assert_eq!(event.usage().cached(), TokenCount::Available(10));
    assert_eq!(event.usage().output(), TokenCount::Available(2));
    assert_eq!(event.usage().reasoning(), TokenCount::Unavailable);
    assert_eq!(event.usage().total(), TokenCount::Available(12));
    assert_eq!(event.source_offset(), 17);
    assert_eq!(event.model().as_str(), "gpt-5.6-sol");
    assert_eq!(
        event.raw_model().map(|value| value.as_str()),
        Some("gpt-5.6-sol")
    );
    assert!(!event.fallback_model());
    assert_eq!(event.long_context(), LongContextState::No);
    assert_eq!(event.session_id().as_str(), "session_fixture");
}

#[test]
fn total_only_usage_is_retained_without_fabricated_components() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let line = br#"{"timestamp":"2026-07-10T08:00:00Z","usage":{"total_tokens":99}}"#;

    let event = emitted(&context, &mut state, &mut diagnostics, 0, line);

    assert_eq!(event.usage().input(), TokenCount::Unavailable);
    assert_eq!(event.usage().cached(), TokenCount::Unavailable);
    assert_eq!(event.usage().output(), TokenCount::Unavailable);
    assert_eq!(event.usage().reasoning(), TokenCount::Unavailable);
    assert_eq!(event.usage().total(), TokenCount::Available(99));
    assert_eq!(event.long_context(), LongContextState::Unavailable);
}

#[test]
fn invalid_aliases_fall_through_without_inventing_zero() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let line = br#"{"timestamp":1700000000,"usage":{"input_tokens":null,"prompt_tokens":"7","cached_input_tokens":3,"output_tokens":-1,"completion_tokens":"2","reasoning_tokens":true,"total_tokens":"invalid"}}"#;

    let event = emitted(&context, &mut state, &mut diagnostics, 0, line);

    assert_eq!(event.usage().input(), TokenCount::Available(7));
    assert_eq!(event.usage().cached(), TokenCount::Available(3));
    assert_eq!(event.usage().output(), TokenCount::Available(2));
    assert_eq!(event.usage().reasoning(), TokenCount::Unavailable);
    assert_eq!(event.usage().total(), TokenCount::Unavailable);
    assert_eq!(diagnostics.count(ParserDiagnosticCode::InvalidToken), 4);
}

#[test]
fn non_scalar_and_overflowing_token_values_are_unavailable_not_malformed() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let line = br#"{"timestamp":1700000000,"usage":{"input_tokens":[],"prompt_tokens":18446744073709551616,"cached_input_tokens":{},"output_tokens":false,"reasoning_output_tokens":1.5,"reasoning_tokens":"18446744073709551616","total_tokens":5}}"#;

    let event = emitted(&context, &mut state, &mut diagnostics, 0, line);

    assert_eq!(event.usage().input(), TokenCount::Unavailable);
    assert_eq!(event.usage().cached(), TokenCount::Unavailable);
    assert_eq!(event.usage().output(), TokenCount::Unavailable);
    assert_eq!(event.usage().reasoning(), TokenCount::Unavailable);
    assert_eq!(event.usage().total(), TokenCount::Available(5));
    assert_eq!(diagnostics.count(ParserDiagnosticCode::InvalidToken), 6);
    assert_eq!(diagnostics.count(ParserDiagnosticCode::MalformedJson), 0);
}

#[test]
fn observed_zero_remains_available_and_all_zero_usage_skips() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let positive = br#"{"timestamp":1700000000,"usage":{"input_tokens":0,"output_tokens":1,"reasoning_tokens":0}}"#;

    let event = emitted(&context, &mut state, &mut diagnostics, 0, positive);
    assert_eq!(event.usage().input(), TokenCount::Available(0));
    assert_eq!(event.usage().cached(), TokenCount::Unavailable);
    assert_eq!(event.usage().reasoning(), TokenCount::Available(0));
    assert_eq!(event.usage().total(), TokenCount::Available(1));

    let zero = br#"{"timestamp":1700000001,"usage":{"input_tokens":0,"output_tokens":0,"reasoning_tokens":0,"total_tokens":0}}"#;
    assert!(matches!(
        parse_line(&context, &mut state, &mut diagnostics, 1, zero),
        ParseOutcome::Skipped
    ));
    assert_eq!(diagnostics.count(ParserDiagnosticCode::ZeroUsage), 1);
}

#[test]
fn escaped_usage_key_is_not_a_relevance_false_negative() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let line = br#"{"timestamp":1700000000,"\u0075sage":{"total_tokens":1}}"#;

    let event = emitted(&context, &mut state, &mut diagnostics, 0, line);

    assert_eq!(event.usage().total(), TokenCount::Available(1));
}

#[test]
fn exact_line_limit_is_accepted_and_plus_one_is_rejected_before_decode() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let prefix = br#"{"padding":""#;
    let suffix = br#"","timestamp":1700000000,"usage":{"total_tokens":1}}"#;
    let padding_len = MAX_LINE_BYTES - prefix.len() - suffix.len();
    let mut exact = Vec::with_capacity(MAX_LINE_BYTES);
    exact.extend_from_slice(prefix);
    exact.resize(exact.len() + padding_len, b'a');
    exact.extend_from_slice(suffix);
    assert_eq!(exact.len(), MAX_LINE_BYTES);

    assert!(matches!(
        parse_line(&context, &mut state, &mut diagnostics, 0, &exact),
        ParseOutcome::Emitted(_)
    ));

    exact.push(b' ');
    assert!(matches!(
        parse_line(&context, &mut state, &mut diagnostics, 1, &exact),
        ParseOutcome::Rejected(ParserDiagnosticCode::LineTooLarge)
    ));
    assert_eq!(diagnostics.count(ParserDiagnosticCode::LineTooLarge), 1);
}

#[test]
fn rfc3339_offsets_and_integral_numeric_timestamps_normalize_to_utc() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let offset = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":"2026-07-10T10:00:00.123+02:00","usage":{"total_tokens":1}}"#,
    );
    let utc = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        1,
        br#"{"timestamp":"2026-07-10T08:00:00.123Z","usage":{"total_tokens":1}}"#,
    );
    assert_eq!(offset.timestamp(), utc.timestamp());

    let seconds = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        2,
        br#"{"timestamp":1700000000,"usage":{"total_tokens":1}}"#,
    );
    assert_eq!(seconds.timestamp().unix_seconds(), 1_700_000_000);
    assert_eq!(seconds.timestamp().subsec_nanos(), 0);

    let milliseconds = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        3,
        br#"{"timestamp":1700000000123,"usage":{"total_tokens":1}}"#,
    );
    assert_eq!(milliseconds.timestamp().unix_seconds(), 1_700_000_000);
    assert_eq!(milliseconds.timestamp().subsec_nanos(), 123_000_000);

    assert!(matches!(
        parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            4,
            br#"{"timestamp":1700000000.5,"usage":{"total_tokens":1}}"#,
        ),
        ParseOutcome::Rejected(ParserDiagnosticCode::InvalidTimestamp)
    ));
}

#[test]
fn model_normalization_is_bounded_honest_and_long_context_is_explicit() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let absent = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"usage":{"input_tokens":272001,"output_tokens":0,"reasoning_tokens":0}}"#,
    );
    assert_eq!(absent.model().as_str(), "unknown");
    assert!(absent.raw_model().is_none());
    assert!(absent.fallback_model());
    assert_eq!(absent.long_context(), LongContextState::Yes);
    assert_eq!(diagnostics.count(ParserDiagnosticCode::ModelFallback), 1);
    let repeated_absent = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        1,
        br#"{"timestamp":1700000001,"usage":{"total_tokens":1}}"#,
    );
    assert!(repeated_absent.fallback_model());
    assert_eq!(diagnostics.count(ParserDiagnosticCode::ModelFallback), 2);

    let mut fresh_state = ParserState::new();
    let unsafe_model = emitted(
        &context,
        &mut fresh_state,
        &mut diagnostics,
        2,
        br#"{"timestamp":1700000001,"model":"future model","usage":{"total_tokens":1}}"#,
    );
    assert!(unsafe_model.model().as_str().starts_with("unknown_"));
    assert_eq!(unsafe_model.model().as_str().len(), 40);
    assert_eq!(
        unsafe_model.raw_model().map(|value| value.as_str()),
        Some("future model")
    );
    assert!(!unsafe_model.fallback_model());

    let oversized = format!(
        "{{\"timestamp\":1700000002,\"model\":\"{}\",\"usage\":{{\"total_tokens\":1}}}}",
        "m".repeat(513)
    );
    let mut fresh_state = ParserState::new();
    let fallback = emitted(
        &context,
        &mut fresh_state,
        &mut diagnostics,
        3,
        oversized.as_bytes(),
    );
    assert_eq!(fallback.model().as_str(), "unknown");
    assert!(fallback.raw_model().is_none());
    assert!(fallback.fallback_model());
    assert_ne!(diagnostics.count(ParserDiagnosticCode::InvalidModel), 0);
}

#[test]
fn invalid_model_alias_falls_through_to_the_next_valid_alias() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let event = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"model":null,"model_name":"fallback-model","usage":{"total_tokens":1}}"#,
    );

    assert_eq!(event.model().as_str(), "fallback-model");
    assert!(!event.fallback_model());
    assert_eq!(diagnostics.count(ParserDiagnosticCode::InvalidModel), 1);
}

#[test]
fn emitted_drafts_canonicalize_only_through_core_accounting() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let event = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        10,
        br#"{"timestamp":"2026-07-10T08:00:00.123Z","model":"gpt-5.6-sol","usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12}}"#,
    );

    let canonical = Canonicalizer::new()
        .canonicalize(&event)
        .expect("Codex draft canonicalizes through core accounting");
    assert_eq!(canonical.fingerprint_version(), EVENT_FINGERPRINT_VERSION);
}

#[test]
fn malformed_and_duplicate_recognized_fields_do_not_mutate_model_state() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let first = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"model":"stable-model","usage":{"total_tokens":1}}"#,
    );
    assert_eq!(first.model().as_str(), "stable-model");

    assert!(matches!(
        parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            1,
            br#"{"model":"evil","usage":{"input_tokens":"#,
        ),
        ParseOutcome::Rejected(ParserDiagnosticCode::MalformedJson)
    ));
    assert!(matches!(
        parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            2,
            br#"{"timestamp":1700000001,"usage":{"total_tokens":1},"usage":{"total_tokens":2}}"#,
        ),
        ParseOutcome::Rejected(ParserDiagnosticCode::MalformedJson)
    ));

    let after = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        3,
        br#"{"timestamp":1700000002,"unknown":{"prompt":"private"},"usage":{"total_tokens":1}}"#,
    );
    assert_eq!(after.model().as_str(), "stable-model");
    assert!(!after.fallback_model());
    assert_eq!(diagnostics.count(ParserDiagnosticCode::MalformedJson), 2);
}

#[test]
fn irrelevant_unescaped_input_skips_without_json_decode() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();

    assert!(matches!(
        parse_line(&context, &mut state, &mut diagnostics, 0, b"not-json",),
        ParseOutcome::Skipped
    ));
    assert_eq!(diagnostics.count(ParserDiagnosticCode::MalformedJson), 0);
    assert_eq!(diagnostics.count(ParserDiagnosticCode::Irrelevant), 1);
    assert_eq!(LONG_CONTEXT_THRESHOLD, 272_000);
}

#[test]
fn event_msg_prefers_last_usage_and_still_advances_cumulative_baseline() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let first = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        10,
        br#"{"timestamp":"2026-07-10T08:00:00Z","type":"event_msg","payload":{"type":"token_count","model":"payload-model","info":{"model":"info-model","last_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12},"total_token_usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":20,"reasoning_tokens":5,"total_tokens":125}}}}"#,
    );
    assert_eq!(first.usage().input(), TokenCount::Available(10));
    assert_eq!(first.usage().output(), TokenCount::Available(2));
    assert_eq!(first.model().as_str(), "payload-model");

    let delta = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        11,
        br#"{"timestamp":"2026-07-10T08:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":110,"cached_input_tokens":45,"output_tokens":22,"reasoning_tokens":6,"total_tokens":138}}}}"#,
    );
    assert_eq!(delta.usage().input(), TokenCount::Available(10));
    assert_eq!(delta.usage().cached(), TokenCount::Available(5));
    assert_eq!(delta.usage().output(), TokenCount::Available(2));
    assert_eq!(delta.usage().reasoning(), TokenCount::Available(1));
    assert_eq!(delta.usage().total(), TokenCount::Available(13));
}

#[test]
fn cumulative_usage_uses_component_deltas_and_reset_semantics() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let first = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":20,"reasoning_tokens":5,"total_tokens":125}}}}"#,
    );
    assert_eq!(first.usage().input(), TokenCount::Available(100));

    let second = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        1,
        br#"{"timestamp":1700000001,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":180,"cached_input_tokens":70,"output_tokens":35,"reasoning_tokens":8,"total_tokens":223}}}}"#,
    );
    assert_eq!(second.usage().input(), TokenCount::Available(80));
    assert_eq!(second.usage().cached(), TokenCount::Available(30));
    assert_eq!(second.usage().output(), TokenCount::Available(15));
    assert_eq!(second.usage().reasoning(), TokenCount::Available(3));
    assert_eq!(second.usage().total(), TokenCount::Available(98));

    let reset = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        2,
        br#"{"timestamp":1700000002,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"reasoning_tokens":1,"total_tokens":13}}}}"#,
    );
    assert_eq!(reset.usage().input(), TokenCount::Available(10));
    assert_eq!(reset.usage().cached(), TokenCount::Available(5));
    assert_eq!(reset.usage().output(), TokenCount::Available(2));
    assert_eq!(reset.usage().reasoning(), TokenCount::Available(1));
    assert_eq!(reset.usage().total(), TokenCount::Available(13));
}

#[test]
fn cumulative_deltas_preserve_component_availability() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let _first = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}}}}"#,
    );

    let delta = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        1,
        br#"{"timestamp":1700000001,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":110,"cached_input_tokens":5,"reasoning_tokens":2,"total_tokens":132}}}}"#,
    );
    assert_eq!(delta.usage().input(), TokenCount::Available(10));
    assert_eq!(delta.usage().cached(), TokenCount::Available(5));
    assert_eq!(delta.usage().output(), TokenCount::Unavailable);
    assert_eq!(delta.usage().reasoning(), TokenCount::Available(2));
    assert_eq!(delta.usage().total(), TokenCount::Available(12));
}

#[test]
fn rejected_cumulative_lines_cannot_advance_the_baseline() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let _first = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20,"reasoning_tokens":5,"total_tokens":125}}}}"#,
    );
    assert!(matches!(
        parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            1,
            br#"{"timestamp":1700000001,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":200"#,
        ),
        ParseOutcome::Rejected(ParserDiagnosticCode::MalformedJson)
    ));
    assert!(matches!(
        parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            2,
            br#"{"timestamp":1700000000.5,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":200,"output_tokens":40,"reasoning_tokens":10,"total_tokens":250}}}}"#,
        ),
        ParseOutcome::Rejected(ParserDiagnosticCode::InvalidTimestamp)
    ));

    let delta = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        3,
        br#"{"timestamp":1700000002,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":110,"output_tokens":22,"reasoning_tokens":6,"total_tokens":138}}}}"#,
    );
    assert_eq!(delta.usage().input(), TokenCount::Available(10));
    assert_eq!(delta.usage().output(), TokenCount::Available(2));
    assert_eq!(delta.usage().reasoning(), TokenCount::Available(1));
    assert_eq!(delta.usage().total(), TokenCount::Available(13));
}

#[test]
fn usage_without_cumulative_totals_cannot_clear_the_baseline() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let _first = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20,"reasoning_tokens":5,"total_tokens":125}}}}"#,
    );
    let _nested = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        1,
        br#"{"timestamp":1700000001,"usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}"#,
    );
    let _last_only = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        2,
        br#"{"timestamp":1700000002,"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":3,"output_tokens":1,"total_tokens":4}}}}"#,
    );

    let delta = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        3,
        br#"{"timestamp":1700000003,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":110,"output_tokens":22,"reasoning_tokens":6,"total_tokens":138}}}}"#,
    );
    assert_eq!(delta.usage().input(), TokenCount::Available(10));
    assert_eq!(delta.usage().output(), TokenCount::Available(2));
    assert_eq!(delta.usage().reasoning(), TokenCount::Available(1));
    assert_eq!(delta.usage().total(), TokenCount::Available(13));
}

#[test]
fn nested_usage_precedence_and_aliases_match_the_oracle() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let data = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000999,"model":"top-model","usage":{"total_tokens":4},"data":{"createdAt":"2026-07-10T10:00:00.123+02:00","model_name":"data-model","usage":{"total_tokens":1}},"result":{"timestamp":1700000001,"model":"result-model","usage":{"total_tokens":2}},"response":{"timestamp":1700000002,"model":"response-model","usage":{"total_tokens":3}}}"#,
    );
    assert_eq!(data.usage().total(), TokenCount::Available(1));
    assert_eq!(data.model().as_str(), "data-model");
    assert_eq!(data.timestamp().subsec_nanos(), 123_000_000);

    let result = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        1,
        br#"{"result":{"created_at":1700000100,"model":"result-only","usage":{"total_tokens":2}}}"#,
    );
    assert_eq!(result.usage().total(), TokenCount::Available(2));
    assert_eq!(result.model().as_str(), "result-only");
    assert_eq!(result.timestamp().unix_seconds(), 1_700_000_100);

    let response = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        2,
        br#"{"response":{"timestamp":1700000200,"model_name":"response-only","usage":{"total_tokens":3}}}"#,
    );
    assert_eq!(response.usage().total(), TokenCount::Available(3));
    assert_eq!(response.model().as_str(), "response-only");
}

#[test]
fn available_zero_and_unavailable_null_remain_distinct_after_canonicalization() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let observed_zero = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        0,
        br#"{"timestamp":1700000000,"model":"gpt-5.6-sol","usage":{"input_tokens":0,"output_tokens":2,"total_tokens":2}}"#,
    );
    let mut unavailable_state = ParserState::new();
    let unavailable = emitted(
        &context,
        &mut unavailable_state,
        &mut diagnostics,
        1,
        br#"{"timestamp":1700000000,"model":"gpt-5.6-sol","usage":{"output_tokens":2,"total_tokens":2}}"#,
    );

    let canonicalizer = Canonicalizer::new();
    let observed_zero = canonicalizer
        .canonicalize(&observed_zero)
        .expect("observed-zero draft canonicalizes");
    let unavailable = canonicalizer
        .canonicalize(&unavailable)
        .expect("unavailable draft canonicalizes");
    assert_ne!(observed_zero.fingerprint(), unavailable.fingerprint());
    assert_ne!(observed_zero.id(), unavailable.id());
}
