use tempfile::TempDir;
use tokenmaster_codex::{
    MAX_LINE_BYTES, MAX_TOOL_NAMES, PARSER_SCHEMA_VERSION, ParseContext, ParseOutcome,
    ParserDiagnosticCode, ParserDiagnostics, ParserResumeErrorCode, ParserResumeStateV1,
    ParserState, parse_line,
};
use tokenmaster_domain::{UsageProfileId, UsageSessionId, UsageSourceId};

const ADVERSARIAL_CASES: usize = 10_000;

struct DeterministicBytes(u64);

impl DeterministicBytes {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte = self.next_u64().to_le_bytes()[0];
        }
    }
}

fn context() -> ParseContext {
    let profile = UsageProfileId::new("profile_adversarial").expect("valid profile");
    let source = UsageSourceId::new("source_adversarial").expect("valid source");
    let filename = UsageSessionId::new("filename-session").expect("valid filename session");
    let hashed = UsageSessionId::new("hashed-session").expect("valid hashed session");
    ParseContext::new(profile, source, Some(filename), hashed)
}

fn assert_state_is_bounded(state: &ParserState) {
    assert!(state.tool_count_len() <= MAX_TOOL_NAMES);
    assert!(state.retained_text_bytes() <= ParserState::MAX_RETAINED_TEXT_BYTES);
    assert_eq!(state.pending_activity().as_array().len(), 8);
}

fn adversarial_case(index: usize, bytes: &mut DeterministicBytes) -> Vec<u8> {
    match index % 8 {
        0 => {
            let mut value = vec![0_u8; index % 257];
            bytes.fill(&mut value);
            value
        }
        1 => {
            let mut value = br#"{"usage":"#.to_vec();
            let start = value.len();
            value.resize(start + (index % 193), 0);
            bytes.fill(&mut value[start..]);
            value
        }
        2 => {
            let depth = 1 + (index % 192);
            let mut value = br#"{"usage":"#.to_vec();
            value.extend(std::iter::repeat_n(b'[', depth));
            value.push(b'0');
            value.extend(std::iter::repeat_n(b']', depth));
            value.push(b'}');
            value
        }
        3 => {
            let mut value = br#"{"usage":{"total_tokens":1},"unknown":""#.to_vec();
            let start = value.len();
            value.resize(start + (index % 97), 0);
            bytes.fill(&mut value[start..]);
            value.push(0xff);
            value.extend_from_slice(br#""}"#);
            value
        }
        4 => format!(
            "{{\"\\u0075sage\":{{\"total_tokens\":{}}},\"timestamp\":1700000000,\"marker\":\"usage\"}}",
            index % 3
        )
        .into_bytes(),
        5 => match index % 4 {
            0 => br#"{"timestamp":1700000000,"usage":{"total_tokens":18446744073709551616}}"#
                .to_vec(),
            1 => br#"{"timestamp":-9223372036854775809,"usage":{"total_tokens":1}}"#
                .to_vec(),
            2 => br#"{"timestamp":1e999,"usage":{"total_tokens":1}}"#.to_vec(),
            _ => br#"{"timestamp":1700000000,"usage":{"input_tokens":-1,"output_tokens":1.5,"total_tokens":1}}"#
                .to_vec(),
        },
        6 => format!(
            "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"tool_{index:05}\"}}}}"
        )
        .into_bytes(),
        _ => serde_json::to_vec(&serde_json::json!({
            "type": "turn_context",
            "payload": {
                "model": {"nested": index},
                "cwd": ["usage"],
                "unknown": {"token_count": index}
            }
        }))
        .expect("adversarial fixture serializes"),
    }
}

fn usage_line_with_size(size: usize) -> Vec<u8> {
    let prefix = br#"{"padding":""#;
    let suffix = br#"","timestamp":1700000000,"usage":{"total_tokens":1}}"#;
    assert!(size >= prefix.len() + suffix.len());
    let mut line = Vec::with_capacity(size);
    line.extend_from_slice(prefix);
    line.resize(size - suffix.len(), b'a');
    line.extend_from_slice(suffix);
    assert_eq!(line.len(), size);
    line
}

#[test]
fn deterministic_adversarial_corpus_never_panics_or_exceeds_bounds() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let mut bytes = DeterministicBytes::new(0x8f3a_9c71_d6e2_450b);

    for index in 0..ADVERSARIAL_CASES {
        let line = adversarial_case(index, &mut bytes);
        let _ = parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            u64::try_from(index).expect("bounded corpus offset"),
            &line,
        );
        assert_state_is_bounded(&state);
    }

    let mut boundary = usage_line_with_size(MAX_LINE_BYTES);
    let exact = parse_line(&context, &mut state, &mut diagnostics, 10_000, &boundary);
    assert!(!matches!(
        exact,
        ParseOutcome::Rejected(ParserDiagnosticCode::LineTooLarge)
    ));
    assert_state_is_bounded(&state);

    boundary.push(b' ');
    assert!(matches!(
        parse_line(&context, &mut state, &mut diagnostics, 10_001, &boundary,),
        ParseOutcome::Rejected(ParserDiagnosticCode::LineTooLarge)
    ));
    assert_state_is_bounded(&state);
    assert_eq!(diagnostics.lines(), 10_002);
    assert_eq!(diagnostics.count(ParserDiagnosticCode::LineTooLarge), 1);
}

#[test]
fn serialized_and_debug_surfaces_exclude_private_input() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let private_root = temp.path().join("CWD_PARENT_SECRET");
    let cwd = private_root.join("safe-project");
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let metadata = serde_json::to_vec(&serde_json::json!({
        "timestamp": "2026-07-10T08:00:00Z",
        "type": "session_meta",
        "payload": {
            "id": "safe-session",
            "requested_model": "gpt-5.6-sol",
            "cwd": cwd,
            "prompt": "PROMPT_SECRET",
            "reasoning": "REASONING_SECRET",
            "unknown": {"nested": "PAYLOAD_SECRET"}
        },
        "response": {"text": "RESPONSE_SECRET"},
        "unknown": {"nested": "ROOT_SECRET"}
    }))
    .expect("metadata fixture serializes");
    assert!(matches!(
        parse_line(&context, &mut state, &mut diagnostics, 0, &metadata),
        ParseOutcome::MetadataOnly
    ));

    let tool = serde_json::to_vec(&serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "function_call",
            "name": "exec_command",
            "arguments": "ARGUMENT_SECRET",
            "command": "COMMAND_SECRET",
            "output": "OUTPUT_SECRET"
        }
    }))
    .expect("tool fixture serializes");
    assert!(matches!(
        parse_line(&context, &mut state, &mut diagnostics, 1, &tool),
        ParseOutcome::ToolOnly
    ));

    let outcome = parse_line(
        &context,
        &mut state,
        &mut diagnostics,
        2,
        br#"{"timestamp":1700000000,"usage":{"total_tokens":1}}"#,
    );
    let event = match &outcome {
        ParseOutcome::Emitted(event) => event,
        other => panic!("event expected, got {other:?}"),
    };
    let snapshot = state.snapshot();
    let mut unsupported = serde_json::to_value(&snapshot).expect("resume serializes to value");
    let unsupported_version = if PARSER_SCHEMA_VERSION == u16::MAX {
        0
    } else {
        PARSER_SCHEMA_VERSION + 1
    };
    unsupported["version"] = serde_json::json!(unsupported_version);
    let unsupported: ParserResumeStateV1 =
        serde_json::from_value(unsupported).expect("unsupported resume shape remains valid");
    let resume_error =
        ParserState::from_resume(unsupported).expect_err("unsupported version must fail");
    assert_eq!(
        resume_error.code(),
        ParserResumeErrorCode::UnsupportedVersion
    );

    let rendered = [
        serde_json::to_string(event).expect("event serializes"),
        serde_json::to_string(&snapshot).expect("resume serializes"),
        serde_json::to_string(&diagnostics).expect("diagnostics serialize"),
        serde_json::to_string(&resume_error.code()).expect("error code serializes"),
        format!("{event:?}"),
        format!("{outcome:?}"),
        format!("{snapshot:?}"),
        format!("{state:?}"),
        format!("{diagnostics:?}"),
        format!("{resume_error:?} {resume_error}"),
    ]
    .join("\n");
    let temp_basename = temp
        .path()
        .file_name()
        .and_then(|value| value.to_str())
        .expect("temporary directory basename is UTF-8");
    for prohibited in [
        "CWD_PARENT_SECRET",
        "PROMPT_SECRET",
        "REASONING_SECRET",
        "PAYLOAD_SECRET",
        "RESPONSE_SECRET",
        "ROOT_SECRET",
        "ARGUMENT_SECRET",
        "COMMAND_SECRET",
        "OUTPUT_SECRET",
        temp_basename,
    ] {
        assert!(
            !rendered.contains(prohibited),
            "surface leaked {prohibited}"
        );
    }
    assert!(rendered.contains("safe-project"));
    assert_state_is_bounded(&state);
}

#[test]
fn ten_thousand_distinct_tools_plateau_at_fixed_retained_capacity() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    assert!(matches!(
        parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            0,
            br#"{"type":"turn_context","payload":{"model":"gpt-5.6-sol"}}"#,
        ),
        ParseOutcome::MetadataOnly
    ));
    let mut retained_at_capacity = None;

    for index in 0..10_000_usize {
        let tool = format!(
            "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"tool_{index:05}\"}}}}"
        );
        assert!(matches!(
            parse_line(
                &context,
                &mut state,
                &mut diagnostics,
                u64::try_from(index + 1).expect("bounded tool offset"),
                tool.as_bytes(),
            ),
            ParseOutcome::ToolOnly
        ));
        if index % 1_000 == 0 {
            assert!(matches!(
                parse_line(
                    &context,
                    &mut state,
                    &mut diagnostics,
                    u64::try_from(20_000 + index).expect("bounded transition offset"),
                    br#"{"timestamp":1700000000,"usage":{"total_tokens":1}}"#,
                ),
                ParseOutcome::Emitted(_)
            ));
        }
        if index + 1 == MAX_TOOL_NAMES {
            retained_at_capacity = Some(state.retained_text_bytes());
        }
        assert_state_is_bounded(&state);
    }

    assert_eq!(state.tool_count_len(), MAX_TOOL_NAMES);
    assert_eq!(state.other_tools(), 10_000 - MAX_TOOL_NAMES as u64);
    assert_eq!(
        diagnostics.count(ParserDiagnosticCode::ToolCapacity),
        10_000 - MAX_TOOL_NAMES as u64
    );
    assert_eq!(diagnostics.tool_events(), 10_000);
    assert_eq!(
        state.retained_text_bytes(),
        retained_at_capacity.expect("capacity checkpoint captured")
    );
}
