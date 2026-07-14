use tokenmaster_accounting::{
    CanonicalUsageEvent, Canonicalizer, MAX_REPLAY_DEPTH, MAX_REPLAY_FANOUT, ParentOrdinal,
    ReplayClassificationInput, ReplayClassifier, ReplayDisposition, ReplayTraversalFacts,
    SessionReplayState,
};
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ModelKey, ObservationDraft, ObservationDraftParts,
    ObservationVerification, TokenCount, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};

fn usage(input: u64, output: u64) -> TokenUsage {
    TokenUsage::new(
        TokenCount::Available(input),
        TokenCount::Unavailable,
        TokenCount::Available(output),
        TokenCount::Unavailable,
        TokenCount::Available(input + output),
    )
}

fn event(
    session: &str,
    parent: Option<&str>,
    ordinal: u64,
    delta: TokenUsage,
    cumulative: Option<TokenUsage>,
    declared_conflict: bool,
) -> CanonicalUsageEvent {
    event_in_scope(
        "codex",
        "default",
        session,
        parent,
        ordinal,
        delta,
        cumulative,
        declared_conflict,
    )
}

#[allow(clippy::too_many_arguments)]
fn event_in_scope(
    provider: &str,
    profile: &str,
    session: &str,
    parent: Option<&str>,
    ordinal: u64,
    delta: TokenUsage,
    cumulative: Option<TokenUsage>,
    declared_conflict: bool,
) -> CanonicalUsageEvent {
    let draft = ObservationDraft::new(ObservationDraftParts {
        provider_id: UsageProviderId::new(provider).expect("provider"),
        profile_id: UsageProfileId::new(profile).expect("profile"),
        session_id: UsageSessionId::new(session).expect("session"),
        parent_session_id: parent.map(|value| UsageSessionId::new(value).expect("parent")),
        session_ordinal: ordinal,
        lineage_conflict: declared_conflict,
        source_id: UsageSourceId::new("fixture").expect("source"),
        source_offset: ordinal,
        source_verification: ObservationVerification::FullPrefix,
        timestamp: UtcTimestamp::new(1_720_598_400 + ordinal as i64, 0).expect("timestamp"),
        model: ModelKey::new("gpt-test").expect("model"),
        raw_model: None,
        delta_usage: delta,
        cumulative_usage: cumulative,
        fallback_model: false,
        long_context: LongContextState::No,
        service_tier: None,
        project: None,
        originator: None,
        activity: ActivityCounts::default(),
    })
    .expect("valid draft");
    Canonicalizer::new()
        .canonicalize(&draft)
        .expect("canonical event")
}

fn clear(depth: usize, direct_children: usize) -> ReplayTraversalFacts {
    ReplayTraversalFacts::new(depth, direct_children, false, false)
}

fn classify(
    prior_state: SessionReplayState,
    child: &CanonicalUsageEvent,
    parent: ParentOrdinal<'_>,
    traversal: ReplayTraversalFacts,
) -> (ReplayDisposition, SessionReplayState) {
    let result = ReplayClassifier::new().classify(ReplayClassificationInput::new(
        prior_state,
        child,
        parent,
        traversal,
    ));
    (result.disposition(), result.next_state())
}

#[test]
fn roots_matches_and_divergence_follow_the_fail_closed_table() {
    let root = event("root", None, 0, usage(10, 2), Some(usage(100, 20)), false);
    assert_eq!(
        classify(
            SessionReplayState::Root,
            &root,
            ParentOrdinal::NotApplicable,
            clear(0, 0),
        ),
        (ReplayDisposition::Eligible, SessionReplayState::Root)
    );

    let parent = event("parent", None, 0, usage(10, 2), Some(usage(100, 20)), false);
    let replay = event(
        "child",
        Some("parent"),
        0,
        usage(10, 2),
        Some(usage(100, 20)),
        false,
    );
    assert_eq!(
        classify(
            SessionReplayState::Matching,
            &replay,
            ParentOrdinal::Present(&parent),
            clear(1, 1),
        ),
        (ReplayDisposition::Replay, SessionReplayState::Matching)
    );

    let mismatch = event(
        "child",
        Some("parent"),
        0,
        usage(11, 2),
        Some(usage(101, 20)),
        false,
    );
    assert_eq!(
        classify(
            SessionReplayState::Matching,
            &mismatch,
            ParentOrdinal::Present(&parent),
            clear(1, 1),
        ),
        (ReplayDisposition::Eligible, SessionReplayState::Diverged)
    );

    let later_weak = event("child", Some("parent"), 1, usage(9, 1), None, false);
    assert_eq!(
        classify(
            SessionReplayState::Diverged,
            &later_weak,
            ParentOrdinal::MissingOpen,
            clear(1, 1),
        ),
        (ReplayDisposition::Eligible, SessionReplayState::Diverged),
        "proven divergence is irreversible for the fixed relation"
    );
}

#[test]
fn weak_evidence_stays_pending_but_later_strong_mismatch_can_diverge() {
    let weak_parent = event("parent", None, 0, usage(10, 2), None, false);
    let weak_child = event("child", Some("parent"), 0, usage(10, 2), None, false);
    let weak = classify(
        SessionReplayState::Matching,
        &weak_child,
        ParentOrdinal::Present(&weak_parent),
        clear(1, 1),
    );
    assert_eq!(
        weak,
        (ReplayDisposition::Pending, SessionReplayState::Matching),
        "weak equality cannot suppress usage or block later proof"
    );

    let strong_parent = event("parent", None, 1, usage(10, 2), Some(usage(110, 22)), false);
    let strong_child = event(
        "child",
        Some("parent"),
        1,
        usage(12, 2),
        Some(usage(112, 22)),
        false,
    );
    assert_eq!(
        classify(
            weak.1,
            &strong_child,
            ParentOrdinal::Present(&strong_parent),
            clear(1, 1),
        ),
        (ReplayDisposition::Eligible, SessionReplayState::Diverged)
    );
}

#[test]
fn missing_parent_state_is_explicit_and_completed_tail_proves_divergence() {
    let child = event(
        "child",
        Some("parent"),
        3,
        usage(10, 2),
        Some(usage(100, 20)),
        false,
    );
    let pending = classify(
        SessionReplayState::Matching,
        &child,
        ParentOrdinal::MissingOpen,
        clear(1, 1),
    );
    assert_eq!(
        pending,
        (ReplayDisposition::Pending, SessionReplayState::Pending)
    );
    assert_eq!(
        classify(
            pending.1,
            &child,
            ParentOrdinal::MissingComplete,
            clear(1, 1),
        ),
        (ReplayDisposition::Pending, SessionReplayState::Pending),
        "a pending one-pass cursor must be replayed from matching when evidence changes"
    );
    assert_eq!(
        classify(
            SessionReplayState::Matching,
            &child,
            ParentOrdinal::MissingComplete,
            clear(1, 1),
        ),
        (ReplayDisposition::Eligible, SessionReplayState::Diverged)
    );
}

#[test]
fn conflicts_corrupt_combinations_and_cycles_fail_closed() {
    let valid_parent = event("parent", None, 0, usage(10, 2), Some(usage(100, 20)), false);
    let wrong_ordinal = event("parent", None, 1, usage(10, 2), Some(usage(100, 20)), false);
    let wrong_session = event(
        "other-parent",
        None,
        0,
        usage(10, 2),
        Some(usage(100, 20)),
        false,
    );
    let wrong_provider = event_in_scope(
        "other",
        "default",
        "parent",
        None,
        0,
        usage(10, 2),
        Some(usage(100, 20)),
        false,
    );
    let wrong_profile = event_in_scope(
        "codex",
        "other",
        "parent",
        None,
        0,
        usage(10, 2),
        Some(usage(100, 20)),
        false,
    );
    let child = event(
        "child",
        Some("parent"),
        0,
        usage(10, 2),
        Some(usage(100, 20)),
        false,
    );
    let declared = event(
        "child",
        Some("other-parent"),
        0,
        usage(10, 2),
        Some(usage(100, 20)),
        true,
    );

    for result in [
        classify(
            SessionReplayState::Matching,
            &declared,
            ParentOrdinal::MissingOpen,
            clear(1, 1),
        ),
        classify(
            SessionReplayState::Matching,
            &child,
            ParentOrdinal::Present(&valid_parent),
            ReplayTraversalFacts::new(1, 1, true, false),
        ),
        classify(
            SessionReplayState::Matching,
            &child,
            ParentOrdinal::MissingOpen,
            ReplayTraversalFacts::new(1, 1, false, true),
        ),
        classify(
            SessionReplayState::Root,
            &child,
            ParentOrdinal::NotApplicable,
            clear(0, 0),
        ),
        classify(
            SessionReplayState::Matching,
            &child,
            ParentOrdinal::Present(&wrong_ordinal),
            clear(1, 1),
        ),
        classify(
            SessionReplayState::Matching,
            &child,
            ParentOrdinal::Present(&wrong_session),
            clear(1, 1),
        ),
        classify(
            SessionReplayState::Matching,
            &child,
            ParentOrdinal::Present(&wrong_provider),
            clear(1, 1),
        ),
        classify(
            SessionReplayState::Matching,
            &child,
            ParentOrdinal::Present(&wrong_profile),
            clear(1, 1),
        ),
    ] {
        assert_eq!(
            result,
            (ReplayDisposition::Conflict, SessionReplayState::Conflict)
        );
    }
}

#[test]
fn exhausted_depth_or_fanout_is_pending_not_conflict() {
    let child = event(
        "child",
        Some("parent"),
        0,
        usage(10, 2),
        Some(usage(100, 20)),
        false,
    );
    for traversal in [
        clear(MAX_REPLAY_DEPTH + 1, 1),
        clear(1, MAX_REPLAY_FANOUT + 1),
    ] {
        assert_eq!(
            classify(
                SessionReplayState::Matching,
                &child,
                ParentOrdinal::MissingOpen,
                traversal,
            ),
            (ReplayDisposition::Pending, SessionReplayState::Pending)
        );
    }
}
