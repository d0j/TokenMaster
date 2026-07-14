use tokenmaster_accounting::{
    CANONICALIZER_VERSION, CanonicalizationErrorCode, Canonicalizer, EVENT_FINGERPRINT_VERSION,
    REPLAY_SIGNATURE_VERSION, ReplayEvidence,
};
use tokenmaster_domain::{
    ActivityCounts, LongContextState, MetadataValue, ModelKey, ObservationDraft,
    ObservationDraftParts, ObservationVerification, ProjectAlias, TokenCount, TokenUsage,
    UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};

fn usage(input: u64, output: u64, total: u64) -> TokenUsage {
    TokenUsage::new(
        TokenCount::Available(input),
        TokenCount::Unavailable,
        TokenCount::Available(output),
        TokenCount::Unavailable,
        TokenCount::Available(total),
    )
}

fn cumulative() -> TokenUsage {
    TokenUsage::new(
        TokenCount::Available(100),
        TokenCount::Available(40),
        TokenCount::Available(20),
        TokenCount::Available(5),
        TokenCount::Available(125),
    )
}

fn draft(
    provider: &str,
    session: &str,
    ordinal: u64,
    source: &str,
    timestamp_seconds: i64,
    delta: TokenUsage,
    cumulative_usage: Option<TokenUsage>,
) -> ObservationDraft {
    ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new(provider).expect("valid provider"),
        profile_id: UsageProfileId::new("default").expect("valid profile"),
        session_id: UsageSessionId::new(session).expect("valid session"),
        parent_session_id: Some(UsageSessionId::new("parent").expect("valid parent")),
        session_ordinal: ordinal,
        lineage_conflict: false,
        source_id: UsageSourceId::new(source).expect("valid source"),
        source_offset: 17,
        source_verification: ObservationVerification::FullPrefix,
        timestamp: UtcTimestamp::new(timestamp_seconds, 123_000_000).expect("valid timestamp"),
        model: ModelKey::new("gpt-test").expect("valid model"),
        raw_model: Some(MetadataValue::new("gpt-test-preview").expect("valid raw model")),
        delta_usage: delta,
        cumulative_usage,
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: Some(MetadataValue::new("priority").expect("valid tier")),
        project: Some(ProjectAlias::new("tokenmaster").expect("valid project")),
        originator: Some(MetadataValue::new("codex_cli").expect("valid originator")),
        activity: ActivityCounts::default(),
    })
    .expect("valid draft")
}

#[test]
fn canonical_hash_vectors_are_versioned_and_deterministic() {
    let event = Canonicalizer::new()
        .canonicalize(&draft(
            "codex",
            "session-a",
            0,
            "source-a",
            1_720_598_400,
            usage(10, 2, 12),
            Some(cumulative()),
        ))
        .expect("valid draft canonicalizes");

    assert_eq!(event.canonicalizer_version(), CANONICALIZER_VERSION);
    assert_eq!(event.fingerprint_version(), EVENT_FINGERPRINT_VERSION);
    assert_eq!(event.replay_signature_version(), REPLAY_SIGNATURE_VERSION);
    assert_eq!(
        event.fingerprint().to_hex(),
        "de895eddeacdbe6f7df6e2613209111c59bb955ba6026ac973a4d01e7172be9f"
    );
    assert_eq!(
        event.lineage().signature().to_hex(),
        "88f132616bfd8a01078c731f35aa71694b046ce8499fdbaae6738ba26ebf0371"
    );
    assert_eq!(event.lineage().evidence(), ReplayEvidence::StrongCumulative);
    assert_eq!(event.id().as_str(), "event_de895eddeacdbe6f7df6");

    let debug = format!(
        "{:?} {:?}",
        event.fingerprint(),
        event.lineage().signature()
    );
    assert!(!debug.contains("de895e"));
    assert!(!debug.contains("88f132"));
    assert!(debug.contains("[redacted]"));
}

#[test]
fn fingerprint_is_logical_event_identity_while_replay_identity_is_structural() {
    let canonicalizer = Canonicalizer::new();
    let original = canonicalizer
        .canonicalize(&draft(
            "codex",
            "session-a",
            0,
            "source-a",
            1_720_598_400,
            usage(10, 2, 12),
            Some(cumulative()),
        ))
        .expect("original canonicalizes");
    let copied_source_and_time = canonicalizer
        .canonicalize(&draft(
            "codex",
            "session-a",
            0,
            "archive-copy",
            1_720_600_000,
            usage(10, 2, 12),
            Some(cumulative()),
        ))
        .expect("copy canonicalizes");
    assert_eq!(original.fingerprint(), copied_source_and_time.fingerprint());

    for changed in [
        draft(
            "other",
            "session-a",
            0,
            "source-a",
            1_720_598_400,
            usage(10, 2, 12),
            Some(cumulative()),
        ),
        draft(
            "codex",
            "session-b",
            0,
            "source-a",
            1_720_598_400,
            usage(10, 2, 12),
            Some(cumulative()),
        ),
        draft(
            "codex",
            "session-a",
            1,
            "source-a",
            1_720_598_400,
            usage(10, 2, 12),
            Some(cumulative()),
        ),
        draft(
            "codex",
            "session-a",
            0,
            "source-a",
            1_720_598_400,
            usage(11, 2, 13),
            Some(cumulative()),
        ),
    ] {
        let changed = canonicalizer
            .canonicalize(&changed)
            .expect("changed draft canonicalizes");
        assert_ne!(original.fingerprint(), changed.fingerprint());
        if changed.delta_usage() == original.delta_usage() {
            assert_eq!(
                original.lineage().signature(),
                changed.lineage().signature(),
                "replay identity excludes provider/session/ordinal"
            );
        }
    }
}

#[test]
fn weak_and_invalid_drafts_fail_closed_without_losing_explicit_state() {
    let canonicalizer = Canonicalizer::new();
    let weak = canonicalizer
        .canonicalize(&draft(
            "codex",
            "session-a",
            0,
            "source-a",
            1_720_598_400,
            usage(10, 2, 12),
            None,
        ))
        .expect("weak draft remains observable");
    assert_eq!(weak.lineage().evidence(), ReplayEvidence::WeakUsageOnly);

    let inconsistent = draft(
        "codex",
        "session-a",
        0,
        "source-a",
        1_720_598_400,
        usage(10, 2, 12),
        Some(usage(9, 2, 11)),
    );
    assert_eq!(
        canonicalizer
            .canonicalize(&inconsistent)
            .expect_err("cumulative below delta must fail")
            .code(),
        CanonicalizationErrorCode::InconsistentCumulative
    );

    let empty = draft(
        "codex",
        "session-a",
        0,
        "source-a",
        1_720_598_400,
        TokenUsage::new(
            TokenCount::Unavailable,
            TokenCount::Available(0),
            TokenCount::Unavailable,
            TokenCount::Unavailable,
            TokenCount::Available(0),
        ),
        None,
    );
    assert_eq!(
        canonicalizer
            .canonicalize(&empty)
            .expect_err("empty usage must fail")
            .code(),
        CanonicalizationErrorCode::EmptyUsage
    );

    let out_of_range = draft(
        "codex",
        "session-a",
        i64::MAX as u64 + 1,
        "source-a",
        1_720_598_400,
        usage(10, 2, 12),
        None,
    );
    assert_eq!(
        canonicalizer
            .canonicalize(&out_of_range)
            .expect_err("SQLite-incompatible ordinal must fail")
            .code(),
        CanonicalizationErrorCode::ValueOutOfRange
    );
}
