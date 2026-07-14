use tokenmaster_domain::{
    ActivityCounts, ActivityKind, CanonicalUsageEvent, CanonicalUsageEventParts, EventFingerprint,
    LongContextState, MetadataValue, ModelKey, ProjectAlias, TokenCount, TokenUsage,
    UsageProfileId, UsageSessionId, UsageSourceId, UtcTimestamp,
};

fn sample_parts() -> CanonicalUsageEventParts {
    CanonicalUsageEventParts {
        profile_id: UsageProfileId::new("profile_fixture").expect("valid profile"),
        session_id: UsageSessionId::new("session_fixture").expect("valid session"),
        source_id: UsageSourceId::new("source_fixture").expect("valid source"),
        source_offset: 17,
        timestamp: UtcTimestamp::new(1_720_598_400, 123_000_000).expect("valid timestamp"),
        model: ModelKey::new("gpt-5.6-sol").expect("valid model"),
        raw_model: Some(MetadataValue::new("gpt-5.6-sol").expect("valid raw model")),
        usage: TokenUsage::new(
            TokenCount::Available(10),
            TokenCount::Unavailable,
            TokenCount::Available(2),
            TokenCount::Unavailable,
            TokenCount::Available(12),
        ),
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: Some(MetadataValue::new("priority").expect("valid tier")),
        project: Some(ProjectAlias::new("tokenmaster").expect("valid project")),
        originator: Some(MetadataValue::new("codex_cli").expect("valid originator")),
        activity: ActivityCounts::default(),
    }
}

#[test]
fn unavailable_tokens_serialize_as_null_not_zero() {
    let usage = TokenUsage::new(
        TokenCount::Available(10),
        TokenCount::Unavailable,
        TokenCount::Available(2),
        TokenCount::Unavailable,
        TokenCount::Available(12),
    );

    let json = serde_json::to_string(&usage).expect("metadata-only usage serializes");
    assert_eq!(
        json,
        r#"{"input":10,"cached":null,"output":2,"reasoning":null,"total":12}"#
    );
    assert_eq!(usage.input(), TokenCount::Available(10));
    assert_eq!(usage.cached(), TokenCount::Unavailable);
    assert_eq!(usage.output(), TokenCount::Available(2));
    assert_eq!(usage.reasoning(), TokenCount::Unavailable);
    assert_eq!(usage.total(), TokenCount::Available(12));

    let round_trip: TokenUsage =
        serde_json::from_str(&json).expect("available/null tuple deserializes");
    assert_eq!(round_trip, usage);
}

#[test]
fn bounded_identity_rejects_controls_oversize_and_unsafe_alphabets() {
    assert!(UsageSessionId::new("session\nsecret").is_err());
    assert!(UsageSessionId::new("s".repeat(513)).is_err());
    assert!(UsageProfileId::new("profile/unsafe").is_err());
    assert!(UsageProfileId::new("p".repeat(129)).is_err());
    assert!(UsageSourceId::new("source unsafe").is_err());
    assert!(ModelKey::new("model|collision").is_err());
    assert!(ModelKey::new("m".repeat(65)).is_err());
    assert!(MetadataValue::new("project\u{7}").is_err());
    assert!(MetadataValue::new("m".repeat(513)).is_err());
}

#[test]
fn bounded_values_preserve_valid_utf8_and_trim_display_metadata() {
    let session = UsageSessionId::new("  сессия-α  ").expect("valid UTF-8 session");
    let metadata = MetadataValue::new("  проект-β  ").expect("valid UTF-8 metadata");
    let profile = UsageProfileId::new("profile_01").expect("safe profile alphabet");
    let source = UsageSourceId::new("source-01.jsonl").expect("safe source alphabet");
    let model = ModelKey::new("openai/gpt-5.6:sol").expect("safe model alphabet");

    assert_eq!(session.as_str(), "сессия-α");
    assert_eq!(metadata.as_str(), "проект-β");
    assert_eq!(profile.as_str(), "profile_01");
    assert_eq!(source.as_str(), "source-01.jsonl");
    assert_eq!(model.as_str(), "openai/gpt-5.6:sol");
}

#[test]
fn project_alias_cannot_retain_a_full_path() {
    assert!(ProjectAlias::new(r"C:\private\tokenmaster").is_err());
    assert!(ProjectAlias::new("/home/private/tokenmaster").is_err());
    assert_eq!(
        ProjectAlias::new("  токен-мастер  ")
            .expect("valid basename alias")
            .as_str(),
        "токен-мастер"
    );
}

#[test]
fn deserialization_cannot_bypass_value_validation() {
    assert!(serde_json::from_str::<UsageProfileId>(r#""profile/unsafe""#).is_err());
    assert!(serde_json::from_str::<ModelKey>(r#""model|collision""#).is_err());
    assert!(serde_json::from_str::<TokenCount>("-1").is_err());
    assert!(
        serde_json::from_str::<UtcTimestamp>(
            r#"{"unix_seconds":1700000000,"subsec_nanos":1000000000}"#,
        )
        .is_err()
    );
}

#[test]
fn activity_counts_are_fixed_order_and_saturating() {
    let kinds = [
        ActivityKind::Read,
        ActivityKind::EditWrite,
        ActivityKind::Search,
        ActivityKind::Git,
        ActivityKind::BuildTest,
        ActivityKind::Web,
        ActivityKind::Subagents,
        ActivityKind::Terminal,
    ];
    let mut counts = ActivityCounts::default();
    for (index, kind) in kinds.into_iter().enumerate() {
        counts.add(kind, index as u64 + 1);
    }
    counts.add(ActivityKind::Read, u64::MAX);
    counts.increment(ActivityKind::Read);

    assert_eq!(counts.get(ActivityKind::Read), u64::MAX);
    assert_eq!(counts.as_array(), &[u64::MAX, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn timestamp_and_long_context_preserve_explicit_state() {
    let timestamp = UtcTimestamp::new(1_700_000_000, 999_999_999).expect("valid instant");
    assert_eq!(timestamp.unix_seconds(), 1_700_000_000);
    assert_eq!(timestamp.subsec_nanos(), 999_999_999);
    assert!(UtcTimestamp::new(1_700_000_000, 1_000_000_000).is_err());
    assert_ne!(LongContextState::Unavailable, LongContextState::No);
    assert_ne!(LongContextState::Unavailable, LongContextState::Yes);
}

#[test]
fn event_id_is_derived_from_the_full_fingerprint() {
    let fingerprint = EventFingerprint::new([0xab; 32]);
    let event = CanonicalUsageEvent::new(sample_parts(), fingerprint);

    assert_eq!(
        event.fingerprint().to_hex(),
        "abababababababababababababababababababababababababababababababab"
    );
    assert_eq!(event.fingerprint().as_bytes(), &[0xab; 32]);
    assert_eq!(event.id().as_str(), "event_abababababababababab");
    assert_eq!(event.profile_id().as_str(), "profile_fixture");
    assert_eq!(event.session_id().as_str(), "session_fixture");
    assert_eq!(event.source_id().as_str(), "source_fixture");
    assert_eq!(event.source_offset(), 17);
    assert_eq!(event.timestamp().subsec_nanos(), 123_000_000);
    assert_eq!(event.model().as_str(), "gpt-5.6-sol");
    assert_eq!(
        event.raw_model().map(MetadataValue::as_str),
        Some("gpt-5.6-sol")
    );
    assert_eq!(event.usage().total(), TokenCount::Available(12));
    assert!(!event.fallback_model());
    assert_eq!(event.long_context(), LongContextState::No);
    assert_eq!(
        event.service_tier().map(MetadataValue::as_str),
        Some("priority")
    );
    assert_eq!(
        event.project().map(ProjectAlias::as_str),
        Some("tokenmaster")
    );
    assert_eq!(
        event.originator().map(MetadataValue::as_str),
        Some("codex_cli")
    );
    assert_eq!(event.activity().as_array(), &[0; 8]);
}

#[test]
fn canonical_event_serialization_contains_only_the_allowed_shape() {
    let event = CanonicalUsageEvent::new(sample_parts(), EventFingerprint::new([0x11; 32]));
    let value = serde_json::to_value(event).expect("canonical event serializes");
    let object = value.as_object().expect("canonical event is an object");

    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        ["fingerprint", "id", "parts"]
    );
    let parts = object["parts"]
        .as_object()
        .expect("event parts are an object");
    assert_eq!(parts["source_offset"], 17);
    assert_eq!(parts["usage"]["cached"], serde_json::Value::Null);
}
