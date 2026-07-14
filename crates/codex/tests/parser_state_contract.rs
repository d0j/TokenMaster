use tokenmaster_codex::{
    MAX_TOOL_NAME_BYTES, MAX_TOOL_NAMES, PARSER_SCHEMA_VERSION, ParseContext, ParseOutcome,
    ParserDiagnosticCode, ParserDiagnostics, ParserResumeErrorCode, ParserResumeStateV1,
    ParserState, parse_line,
};
use tokenmaster_domain::{
    ActivityKind, CanonicalUsageEvent, TokenCount, UsageProfileId, UsageSessionId, UsageSourceId,
};

fn context() -> ParseContext {
    let profile = UsageProfileId::new("profile_fixture").expect("valid profile");
    let source = UsageSourceId::new("source_fixture").expect("valid source");
    let filename = UsageSessionId::new("filename-session").expect("valid filename session");
    let hashed = UsageSessionId::new("hashed-session").expect("valid hashed session");
    ParseContext::new(profile, source, Some(filename), hashed)
}

fn parse(
    context: &ParseContext,
    state: &mut ParserState,
    diagnostics: &mut ParserDiagnostics,
    offset: u64,
    line: &[u8],
) -> ParseOutcome {
    parse_line(context, state, diagnostics, offset, line)
}

fn emitted(
    context: &ParseContext,
    state: &mut ParserState,
    diagnostics: &mut ParserDiagnostics,
    offset: u64,
    line: &[u8],
) -> CanonicalUsageEvent {
    match parse(context, state, diagnostics, offset, line) {
        ParseOutcome::Emitted(event) => event,
        other => panic!("event expected, got {other:?}"),
    }
}

fn seed_metadata_and_tool(
    context: &ParseContext,
    state: &mut ParserState,
    diagnostics: &mut ParserDiagnostics,
) -> CanonicalUsageEvent {
    let metadata = br#"{"timestamp":"2026-07-10T08:00:00Z","type":"session_meta","payload":{"id":"real-session-id","session_id":"general-session-id","requested_model":"gpt-5.6-sol","service_tier":"priority","cwd":"C:\\PRIVATE_PARENT_MARKER\\customer-api","originator":"codex_win","source":"cli","git":{"branch":"feature/usage"},"model_context_window":1050000,"prompt":"PROMPT_SECRET","reasoning":"REASONING_SECRET"},"unknown":{"response":"RESPONSE_SECRET"}}"#;
    assert!(matches!(
        parse(context, state, diagnostics, 0, metadata),
        ParseOutcome::MetadataOnly
    ));

    let tool = br#"{"type":"response_item","payload":{"type":"function_call","name":"read_file","arguments":"ARGUMENT_SECRET","output":"OUTPUT_SECRET"}}"#;
    assert!(matches!(
        parse(context, state, diagnostics, 1, tool),
        ParseOutcome::ToolOnly
    ));

    emitted(
        context,
        state,
        diagnostics,
        2,
        br#"{"timestamp":"2026-07-10T08:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":20,"cached_input_tokens":5,"output_tokens":3,"reasoning_output_tokens":1,"total_tokens":24}}}}"#,
    )
}

#[test]
fn metadata_activity_and_privacy_follow_the_bounded_contract() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();

    let event = seed_metadata_and_tool(&context, &mut state, &mut diagnostics);

    assert_eq!(event.model().as_str(), "gpt-5.6-sol");
    assert_eq!(
        event.service_tier().map(|value| value.as_str()),
        Some("priority")
    );
    assert_eq!(
        event.project().map(|value| value.as_str()),
        Some("customer-api")
    );
    assert_eq!(
        event.originator().map(|value| value.as_str()),
        Some("codex_win")
    );
    assert_eq!(event.session_id().as_str(), "real-session-id");
    assert_eq!(event.activity().get(ActivityKind::Read), 1);
    assert_eq!(state.pending_activity().get(ActivityKind::Read), 0);
    assert_eq!(state.aggregate_activity().get(ActivityKind::Read), 1);
    assert_eq!(state.context_window(), Some(1_050_000));
    assert_eq!(state.source_alias(), Some("cli"));
    assert_eq!(state.git_branch(), Some("feature/usage"));

    let next = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        3,
        br#"{"timestamp":"2026-07-10T08:02:00Z","usage":{"total_tokens":1}}"#,
    );
    assert_eq!(next.activity().get(ActivityKind::Read), 0);
    assert_eq!(diagnostics.lines(), 4);
    assert_eq!(diagnostics.emitted_events(), 2);
    assert_eq!(diagnostics.metadata_lines(), 1);
    assert_eq!(diagnostics.tool_events(), 1);

    let event_json = serde_json::to_string(&event).expect("event serializes");
    let resume_json = serde_json::to_string(&state.snapshot()).expect("resume serializes");
    for marker in [
        "PRIVATE_PARENT_MARKER",
        "PROMPT_SECRET",
        "REASONING_SECRET",
        "RESPONSE_SECRET",
        "ARGUMENT_SECRET",
        "OUTPUT_SECRET",
    ] {
        assert!(!event_json.contains(marker), "event leaked {marker}");
        assert!(!resume_json.contains(marker), "resume leaked {marker}");
    }
    assert!(resume_json.contains("real-session-id"));
    assert!(resume_json.contains("feature/usage"));
    assert!(state.retained_text_bytes() <= ParserState::MAX_RETAINED_TEXT_BYTES);
}

#[test]
fn display_metadata_truncates_on_utf8_boundaries_and_paths_never_do() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let long_originator = "é".repeat(300);
    let line = format!(
        "{{\"type\":\"turn_context\",\"payload\":{{\"originator\":\"{long_originator}\",\"cwd\":\"relative\\\\PRIVATE_PATH\",\"model\":\"stable-model\"}}}}"
    );
    assert!(matches!(
        parse(&context, &mut state, &mut diagnostics, 0, line.as_bytes(),),
        ParseOutcome::MetadataOnly
    ));
    let event = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        1,
        br#"{"timestamp":1700000000,"usage":{"total_tokens":1}}"#,
    );
    let originator = event.originator().expect("truncated originator retained");
    assert_eq!(originator.as_str().len(), 512);
    assert!(originator.as_str().is_char_boundary(512));
    assert!(event.project().is_none());
    assert_eq!(
        diagnostics.count(ParserDiagnosticCode::MetadataTruncated),
        1
    );
    assert_eq!(diagnostics.count(ParserDiagnosticCode::InvalidPath), 1);
    assert!(
        !serde_json::to_string(&state.snapshot())
            .expect("resume serializes")
            .contains("PRIVATE_PATH")
    );
}

#[test]
fn tool_names_are_sorted_truncated_and_capacity_bounded() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();

    for index in 0..=MAX_TOOL_NAMES {
        let line = format!(
            "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"tool_{index:03}\"}}}}"
        );
        assert!(matches!(
            parse(
                &context,
                &mut state,
                &mut diagnostics,
                u64::try_from(index).expect("bounded index"),
                line.as_bytes(),
            ),
            ParseOutcome::ToolOnly
        ));
    }

    assert_eq!(state.tool_count_len(), MAX_TOOL_NAMES);
    assert_eq!(state.other_tools(), 1);
    assert_eq!(diagnostics.count(ParserDiagnosticCode::ToolCapacity), 1);
    assert_eq!(
        state.pending_activity().get(ActivityKind::Terminal),
        u64::try_from(MAX_TOOL_NAMES + 1).expect("bounded count")
    );
    assert_eq!(
        state.aggregate_activity().get(ActivityKind::Terminal),
        u64::try_from(MAX_TOOL_NAMES + 1).expect("bounded count")
    );
    assert!(
        state
            .tool_counts()
            .windows(2)
            .all(|pair| pair[0].name() < pair[1].name())
    );

    let mut truncated_state = ParserState::new();
    let long_name = "é".repeat(50);
    let line = format!(
        "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"{long_name}\"}}}}"
    );
    assert!(matches!(
        parse(
            &context,
            &mut truncated_state,
            &mut diagnostics,
            0,
            line.as_bytes(),
        ),
        ParseOutcome::ToolOnly
    ));
    assert_eq!(
        truncated_state.tool_counts()[0].name().len(),
        MAX_TOOL_NAME_BYTES
    );
    assert!(
        truncated_state.tool_counts()[0]
            .name()
            .is_char_boundary(MAX_TOOL_NAME_BYTES)
    );
    assert_ne!(
        diagnostics.count(ParserDiagnosticCode::MetadataTruncated),
        0
    );
}

#[test]
fn tool_activity_classification_uses_the_fixed_precedence_order() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let fixtures = [
        ("read_edit_file", ActivityKind::Read),
        ("edit_file", ActivityKind::EditWrite),
        ("search_code", ActivityKind::Search),
        ("git_status", ActivityKind::Git),
        ("build_project", ActivityKind::BuildTest),
        ("browser_open", ActivityKind::Web),
        ("spawn_agent", ActivityKind::Subagents),
        ("exec_command", ActivityKind::Terminal),
    ];
    for (offset, (name, _)) in fixtures.iter().enumerate() {
        let line = format!(
            "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"{name}\"}}}}"
        );
        assert!(matches!(
            parse(
                &context,
                &mut state,
                &mut diagnostics,
                u64::try_from(offset).expect("bounded offset"),
                line.as_bytes(),
            ),
            ParseOutcome::ToolOnly
        ));
    }
    for (_, kind) in fixtures {
        assert_eq!(state.aggregate_activity().get(kind), 1);
    }
}

#[test]
fn resume_is_deterministic_strict_and_revalidated() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let _event = seed_metadata_and_tool(&context, &mut state, &mut diagnostics);
    let _baseline = emitted(
        &context,
        &mut state,
        &mut diagnostics,
        3,
        br#"{"timestamp":1700000000,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20,"reasoning_tokens":5,"total_tokens":125}}}}"#,
    );
    assert!(matches!(
        parse(
            &context,
            &mut state,
            &mut diagnostics,
            4,
            br#"{"type":"response_item","payload":{"type":"function_call","name":"write_file"}}"#,
        ),
        ParseOutcome::ToolOnly
    ));

    let snapshot = state.snapshot();
    let first_json = serde_json::to_string(&snapshot).expect("resume serializes");
    let second_json = serde_json::to_string(&snapshot).expect("resume serializes twice");
    assert_eq!(first_json, second_json);
    assert!(first_json.contains(&format!("\"version\":{PARSER_SCHEMA_VERSION}")));

    let decoded: ParserResumeStateV1 =
        serde_json::from_str(&first_json).expect("resume deserializes");
    let restored = ParserState::from_resume(decoded).expect("valid resume restores");
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.tool_count_len(), 2);
    let mut restored = restored;
    let delta = emitted(
        &context,
        &mut restored,
        &mut diagnostics,
        5,
        br#"{"timestamp":1700000001,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":110,"output_tokens":22,"reasoning_tokens":6,"total_tokens":138}}}}"#,
    );
    assert_eq!(delta.usage().input(), TokenCount::Available(10));
    assert_eq!(delta.usage().total(), TokenCount::Available(13));
    assert_eq!(delta.activity().get(ActivityKind::EditWrite), 1);
    assert_eq!(restored.pending_activity().get(ActivityKind::EditWrite), 0);

    let mut unknown: serde_json::Value =
        serde_json::from_str(&first_json).expect("resume json value");
    unknown
        .as_object_mut()
        .expect("resume object")
        .insert("raw_tail".to_owned(), serde_json::json!("TAIL_SECRET"));
    assert!(serde_json::from_value::<ParserResumeStateV1>(unknown).is_err());

    let mut unsupported: serde_json::Value =
        serde_json::from_str(&first_json).expect("resume json value");
    unsupported["version"] = serde_json::json!(PARSER_SCHEMA_VERSION + 1);
    let unsupported: ParserResumeStateV1 =
        serde_json::from_value(unsupported).expect("shape remains valid");
    let error = ParserState::from_resume(unsupported).expect_err("version must fail");
    assert_eq!(error.code(), ParserResumeErrorCode::UnsupportedVersion);

    let mut unsorted: serde_json::Value =
        serde_json::from_str(&first_json).expect("resume json value");
    let tools = unsorted["tool_counts"]
        .as_array_mut()
        .expect("tool count array");
    tools.push(serde_json::json!({"name":"aaa","count":1}));
    let unsorted: ParserResumeStateV1 =
        serde_json::from_value(unsorted).expect("bounded shape deserializes");
    let error = ParserState::from_resume(unsorted).expect_err("order must fail");
    assert_eq!(error.code(), ParserResumeErrorCode::InvalidState);

    let mut invalid_cached: serde_json::Value =
        serde_json::from_str(&first_json).expect("resume json value");
    invalid_cached["previous_totals"]["cached"] = serde_json::json!(101);
    let invalid_cached: ParserResumeStateV1 =
        serde_json::from_value(invalid_cached).expect("bounded shape deserializes");
    let error = ParserState::from_resume(invalid_cached).expect_err("cached bound must fail");
    assert_eq!(error.code(), ParserResumeErrorCode::InvalidState);

    let mut impossible_other: serde_json::Value =
        serde_json::from_str(&first_json).expect("resume json value");
    impossible_other["tool_counts"] = serde_json::json!([]);
    impossible_other["other_tools"] = serde_json::json!(1);
    let impossible_other: ParserResumeStateV1 =
        serde_json::from_value(impossible_other).expect("bounded shape deserializes");
    let error =
        ParserState::from_resume(impossible_other).expect_err("other tools require full capacity");
    assert_eq!(error.code(), ParserResumeErrorCode::InvalidState);

    let mut impossible_activity: serde_json::Value =
        serde_json::from_str(&first_json).expect("resume json value");
    impossible_activity["pending_activity"][0] = serde_json::json!(2);
    impossible_activity["aggregate_activity"][0] = serde_json::json!(1);
    let impossible_activity: ParserResumeStateV1 =
        serde_json::from_value(impossible_activity).expect("bounded shape deserializes");
    let error = ParserState::from_resume(impossible_activity)
        .expect_err("pending activity cannot exceed aggregate activity");
    assert_eq!(error.code(), ParserResumeErrorCode::InvalidState);

    let oversized_tools = serde_json::json!({
        "version": PARSER_SCHEMA_VERSION,
        "current_model": null,
        "previous_totals": null,
        "service_tier": null,
        "session_id": null,
        "project": null,
        "originator": null,
        "source_alias": null,
        "git_branch": null,
        "context_window": null,
        "pending_activity": [0,0,0,0,0,0,0,0],
        "aggregate_activity": [0,0,0,0,0,0,0,0],
        "tool_counts": (0..=MAX_TOOL_NAMES)
            .map(|index| serde_json::json!({"name":format!("tool_{index:03}"),"count":1}))
            .collect::<Vec<_>>(),
        "other_tools": 0
    });
    assert!(serde_json::from_value::<ParserResumeStateV1>(oversized_tools).is_err());
}

#[test]
fn rejected_lines_and_invalid_metadata_cannot_corrupt_state() {
    let context = context();
    let mut state = ParserState::new();
    let mut diagnostics = ParserDiagnostics::new();
    let _event = seed_metadata_and_tool(&context, &mut state, &mut diagnostics);
    let before = state.snapshot();

    assert!(matches!(
        parse(
            &context,
            &mut state,
            &mut diagnostics,
            3,
            br#"{"type":"session_meta","payload":{"id":"evil-session""#,
        ),
        ParseOutcome::Rejected(ParserDiagnosticCode::MalformedJson)
    ));
    assert!(matches!(
        parse(
            &context,
            &mut state,
            &mut diagnostics,
            4,
            br#"{"timestamp":1700000000.5,"type":"event_msg","payload":{"type":"token_count","session_id":"evil-session","cwd":"C:\\EVIL_PATH\\evil","info":{"total_token_usage":{"input_tokens":30,"output_tokens":4,"total_tokens":34}}}}"#,
        ),
        ParseOutcome::Rejected(ParserDiagnosticCode::InvalidTimestamp)
    ));
    assert_eq!(state.snapshot(), before);

    let mut fallback_state = ParserState::new();
    assert!(matches!(
        parse(
            &context,
            &mut fallback_state,
            &mut diagnostics,
            0,
            br#"{"type":"session_meta","payload":{"id":"","session_id":"general-session","cwd":"relative\\PRIVATE_PATH"}}"#,
        ),
        ParseOutcome::MetadataOnly
    ));
    let event = emitted(
        &context,
        &mut fallback_state,
        &mut diagnostics,
        1,
        br#"{"timestamp":1700000001,"usage":{"total_tokens":1}}"#,
    );
    assert_eq!(event.session_id().as_str(), "general-session");
    assert!(event.project().is_none());
    assert_ne!(diagnostics.count(ParserDiagnosticCode::InvalidPath), 0);
}
