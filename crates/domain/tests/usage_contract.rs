use tokenmaster_domain::{
    ActivityCounts, ActivityKind, LongContextState, MetadataValue, ModelKey, ObservationDraft,
    ObservationDraftParts, ObservationVerification, ProjectAlias, SessionRelationDraft,
    SessionRelationDraftParts, TokenCount, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};

#[test]
fn observation_draft_is_bounded_provider_neutral_and_private() {
    let session_id = UsageSessionId::new("session_child").expect("valid child session");
    let delta = TokenUsage::new(
        TokenCount::Available(10),
        TokenCount::Unavailable,
        TokenCount::Available(2),
        TokenCount::Unavailable,
        TokenCount::Available(12),
    );
    let cumulative = TokenUsage::new(
        TokenCount::Available(100),
        TokenCount::Available(40),
        TokenCount::Available(20),
        TokenCount::Available(5),
        TokenCount::Available(125),
    );
    let draft = ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new("codex").expect("valid provider"),
        profile_id: UsageProfileId::new("default").expect("valid profile"),
        session_id: session_id.clone(),
        parent_session_id: Some(
            UsageSessionId::new("session_parent").expect("valid parent session"),
        ),
        session_ordinal: 0,
        lineage_conflict: false,
        source_id: UsageSourceId::new("source_fixture").expect("valid source"),
        source_offset: 17,
        source_verification: ObservationVerification::FullPrefix,
        timestamp: UtcTimestamp::new(1_720_598_400, 123_000_000).expect("valid timestamp"),
        model: ModelKey::new("gpt-5.6-sol").expect("valid model"),
        raw_model: Some(MetadataValue::new("gpt-5.6-sol").expect("valid raw model")),
        delta_usage: delta,
        cumulative_usage: Some(cumulative),
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: Some(MetadataValue::new("priority").expect("valid tier")),
        project: Some(ProjectAlias::new("tokenmaster").expect("valid project")),
        originator: Some(MetadataValue::new("codex_cli").expect("valid originator")),
        activity: ActivityCounts::default(),
    })
    .expect("valid observation draft");

    assert_eq!(draft.provider_id().as_str(), "codex");
    assert_eq!(draft.profile_id().as_str(), "default");
    assert_eq!(draft.session_id(), &session_id);
    assert_eq!(
        draft.parent_session_id().map(UsageSessionId::as_str),
        Some("session_parent")
    );
    assert_eq!(draft.session_ordinal(), 0);
    assert!(!draft.lineage_conflict());
    assert_eq!(
        draft.source_verification(),
        ObservationVerification::FullPrefix
    );
    assert_eq!(draft.delta_usage(), &delta);
    assert_eq!(draft.cumulative_usage(), Some(&cumulative));

    let debug = format!("{draft:?}");
    assert!(debug.contains("provider_id"));
    assert!(debug.contains("session_ordinal"));
    assert!(!debug.contains("source_fixture"));
    assert!(!debug.contains("125"));

    assert!(UsageProviderId::new("provider/path").is_err());
    assert!(UsageProviderId::new("p".repeat(65)).is_err());
    assert_eq!(
        serde_json::to_string(&ObservationVerification::Incremental)
            .expect("verification serializes"),
        r#""incremental""#
    );

    let self_parent = ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new("codex").expect("valid provider"),
        profile_id: UsageProfileId::new("default").expect("valid profile"),
        session_id: session_id.clone(),
        parent_session_id: Some(session_id),
        session_ordinal: 0,
        lineage_conflict: false,
        source_id: UsageSourceId::new("source_fixture").expect("valid source"),
        source_offset: 17,
        source_verification: ObservationVerification::Incremental,
        timestamp: UtcTimestamp::new(1_720_598_400, 0).expect("valid timestamp"),
        model: ModelKey::new("gpt-5.6-sol").expect("valid model"),
        raw_model: None,
        delta_usage: delta,
        cumulative_usage: None,
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        project: None,
        originator: None,
        activity: ActivityCounts::default(),
    });
    assert!(self_parent.is_err());
}

#[test]
fn session_relation_draft_preserves_late_lineage_without_source_content() {
    let relation = SessionRelationDraft::new(SessionRelationDraftParts {
        provider_id: UsageProviderId::new("codex").expect("valid provider"),
        profile_id: UsageProfileId::new("default").expect("valid profile"),
        session_id: UsageSessionId::new("session_child").expect("valid session"),
        parent_session_id: UsageSessionId::new("session_parent").expect("valid parent"),
        declared_conflict: false,
        source_id: UsageSourceId::new("source_private").expect("valid source"),
        source_offset: 41,
    })
    .expect("valid session relation");

    assert_eq!(relation.provider_id().as_str(), "codex");
    assert_eq!(relation.profile_id().as_str(), "default");
    assert_eq!(relation.session_id().as_str(), "session_child");
    assert_eq!(relation.parent_session_id().as_str(), "session_parent");
    assert!(!relation.declared_conflict());
    assert_eq!(relation.source_offset(), 41);
    let debug = format!("{relation:?}");
    assert!(!debug.contains("source_private"));

    let session = UsageSessionId::new("same").expect("valid session");
    assert!(
        SessionRelationDraft::new(SessionRelationDraftParts {
            provider_id: UsageProviderId::new("codex").expect("valid provider"),
            profile_id: UsageProfileId::new("default").expect("valid profile"),
            session_id: session.clone(),
            parent_session_id: session,
            declared_conflict: false,
            source_id: UsageSourceId::new("source_private").expect("valid source"),
            source_offset: 41,
        })
        .is_err()
    );
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
