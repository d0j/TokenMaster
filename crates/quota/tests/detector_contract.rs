use std::fmt::Write;

use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
    QuotaResetThresholds, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId,
    QuotaWindowKey, QuotaWindowSemantics, QuotaWorkspaceId, UsageProviderId,
};
use tokenmaster_quota::{
    QuotaAllowanceChangeKind, QuotaDetectionTime, QuotaEpochState, QuotaErrorCode, QuotaEvaluation,
    QuotaTransitionKind, evaluate_sample,
};

#[derive(Clone)]
struct SampleSpec {
    id: u8,
    observed_at_ms: i64,
    epoch: Option<&'static str>,
    used_ppm: Option<u32>,
    remaining_ppm: Option<u32>,
    used_units: Option<u64>,
    remaining_units: Option<u64>,
    capacity: Option<u64>,
    unit_id: &'static str,
    resets_at_ms: Option<i64>,
    quality: QuotaSampleQuality,
    source: QuotaEvidenceSource,
    confidence: QuotaConfidence,
    reset_evidence: QuotaResetEvidence,
    reset_occurred_at_ms: Option<i64>,
}

impl SampleSpec {
    fn new(id: u8, observed_at_ms: i64) -> Self {
        Self {
            id,
            observed_at_ms,
            epoch: Some("epoch-1"),
            used_ppm: Some(700_000),
            remaining_ppm: Some(300_000),
            used_units: None,
            remaining_units: None,
            capacity: None,
            unit_id: "provider_units",
            resets_at_ms: Some(10_000),
            quality: QuotaSampleQuality::Authoritative,
            source: QuotaEvidenceSource::ProviderLocal,
            confidence: QuotaConfidence::High,
            reset_evidence: QuotaResetEvidence::None,
            reset_occurred_at_ms: None,
        }
    }
}

fn window_key(account: &str, workspace: Option<&str>) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new(account).expect("account"),
            workspace.map(|value| QuotaWorkspaceId::new(value).expect("workspace")),
        ),
        QuotaWindowId::new("weekly").expect("window"),
    )
}

fn window_definition(
    key: QuotaWindowKey,
    semantics: QuotaWindowSemantics,
    thresholds: bool,
) -> QuotaWindowDefinition {
    window_definition_at_revision(key, semantics, thresholds, 1)
}

fn window_definition_at_revision(
    key: QuotaWindowKey,
    semantics: QuotaWindowSemantics,
    thresholds: bool,
    revision: u64,
) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key,
        revision,
        label_key: "quota.weekly".to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics,
        nominal_duration_seconds: Some(603_900),
        reset_thresholds: thresholds.then(|| {
            QuotaResetThresholds::new(
                Some(QuotaRatio::new(50_000).expect("used floor")),
                Some(QuotaRatio::new(950_000).expect("remaining ceiling")),
                Some(QuotaRatio::new(500_000).expect("minimum drop")),
            )
            .expect("thresholds")
        }),
    })
    .expect("definition")
}

fn sample(key: &QuotaWindowKey, spec: SampleSpec) -> QuotaSample {
    let units =
        (spec.used_units.is_some() || spec.remaining_units.is_some() || spec.capacity.is_some())
            .then(|| {
                QuotaUnits::new(
                    QuotaUnitId::new(spec.unit_id).expect("unit"),
                    spec.used_units,
                    spec.remaining_units,
                    spec.capacity,
                )
                .expect("units")
            });
    QuotaSample::new(QuotaSampleParts {
        key: key.clone(),
        observation_id: QuotaObservationId::from_bytes([spec.id; 32]),
        observed_at_ms: spec.observed_at_ms,
        fresh_until_ms: spec.observed_at_ms + 100,
        stale_after_ms: spec.observed_at_ms + 200,
        provider_epoch_id: spec
            .epoch
            .map(|value| QuotaProviderEpochId::new(value).expect("epoch")),
        used_ratio: spec
            .used_ppm
            .map(|value| QuotaRatio::new(value).expect("used ratio")),
        remaining_ratio: spec
            .remaining_ppm
            .map(|value| QuotaRatio::new(value).expect("remaining ratio")),
        units,
        advertised_resets_at_ms: spec.resets_at_ms,
        quality: spec.quality,
        source: spec.source,
        confidence: spec.confidence,
        reset_evidence: spec.reset_evidence,
        reset_occurred_at_ms: spec.reset_occurred_at_ms,
    })
    .expect("sample")
}

fn started(definition: &QuotaWindowDefinition, first: &QuotaSample) -> QuotaEpochState {
    match evaluate_sample(definition, None, None, first, 1).expect("started") {
        QuotaEvaluation::Started { state } => state,
        other => panic!("expected started, got {other:?}"),
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("write to string");
    }
    output
}

#[test]
fn epoch_starts_advances_and_retains_only_comparable_maximum_use() {
    let key = window_key("personal", Some("default"));
    let definition = window_definition(key.clone(), QuotaWindowSemantics::Fixed, false);
    let mut first_spec = SampleSpec::new(1, 1_000);
    first_spec.used_units = Some(70);
    first_spec.remaining_units = Some(30);
    first_spec.capacity = Some(100);
    let first = sample(&key, first_spec);
    let state = started(&definition, &first);

    assert_eq!(state.last_transition_sequence(), 0);
    assert_eq!(
        state
            .maximum_used_ratio()
            .expect("maximum ratio")
            .parts_per_million(),
        700_000
    );
    assert_eq!(
        state.maximum_used_units().and_then(QuotaUnits::used),
        Some(70)
    );

    let mut second_spec = SampleSpec::new(2, 2_000);
    second_spec.used_ppm = Some(900_000);
    second_spec.remaining_ppm = Some(100_000);
    second_spec.used_units = Some(90);
    second_spec.remaining_units = Some(10);
    second_spec.capacity = Some(100);
    let second = sample(&key, second_spec);
    let state = match evaluate_sample(&definition, Some(&state), Some(&first), &second, 1)
        .expect("advance")
    {
        QuotaEvaluation::Advanced { state } => state,
        other => panic!("expected advance, got {other:?}"),
    };
    assert_eq!(
        state
            .maximum_used_ratio()
            .expect("maximum ratio")
            .parts_per_million(),
        900_000
    );
    assert_eq!(
        state.maximum_used_units().and_then(QuotaUnits::used),
        Some(90)
    );

    let mut drop_spec = SampleSpec::new(3, 3_000);
    drop_spec.used_ppm = Some(100_000);
    drop_spec.remaining_ppm = Some(900_000);
    drop_spec.epoch = Some("epoch-1");
    let drop_only = sample(&key, drop_spec);
    assert!(matches!(
        evaluate_sample(&definition, Some(&state), Some(&second), &drop_only, 1)
            .expect("drop-only advance"),
        QuotaEvaluation::Advanced { .. }
    ));
}

#[test]
fn definition_revision_advances_without_rekeying_the_open_epoch_and_cannot_regress() {
    let key = window_key("personal", Some("default"));
    let revision_one =
        window_definition_at_revision(key.clone(), QuotaWindowSemantics::Fixed, false, 1);
    let first = sample(&key, SampleSpec::new(1, 1_000));
    let state = started(&revision_one, &first);
    let original_epoch_id = state.epoch_id();

    let revision_two =
        window_definition_at_revision(key.clone(), QuotaWindowSemantics::Fixed, false, 2);
    let second = sample(&key, SampleSpec::new(2, 2_000));
    let state = match evaluate_sample(&revision_two, Some(&state), Some(&first), &second, 1)
        .expect("definition revision advance")
    {
        QuotaEvaluation::Advanced { state } => state,
        other => panic!("expected advance, got {other:?}"),
    };

    assert_eq!(state.epoch_id(), original_epoch_id);
    assert_eq!(state.epoch_definition_revision(), 1);
    assert_eq!(state.definition_revision(), 2);
    let restored = QuotaEpochState::restore(state.to_parts()).expect("restored advanced state");
    assert_eq!(restored, state);

    let third = sample(&key, SampleSpec::new(3, 3_000));
    assert_eq!(
        evaluate_sample(&revision_one, Some(&restored), Some(&second), &third, 1)
            .expect_err("definition revision regression")
            .code(),
        QuotaErrorCode::DefinitionRevisionRegressed
    );
}

#[test]
fn duplicate_stale_and_mismatched_state_fail_closed() {
    let key = window_key("personal", Some("default"));
    let definition = window_definition(key.clone(), QuotaWindowSemantics::Fixed, false);
    let first = sample(&key, SampleSpec::new(1, 1_000));
    let state = started(&definition, &first);

    assert!(matches!(
        evaluate_sample(&definition, Some(&state), Some(&first), &first, 1).expect("duplicate"),
        QuotaEvaluation::Duplicate
    ));

    let mut conflicting_spec = SampleSpec::new(1, 1_000);
    conflicting_spec.used_ppm = Some(800_000);
    let conflicting = sample(&key, conflicting_spec);
    assert_eq!(
        evaluate_sample(&definition, Some(&state), Some(&first), &conflicting, 1)
            .expect_err("conflicting duplicate")
            .code(),
        QuotaErrorCode::DuplicateConflict
    );

    for observed_at_ms in [999, 1_000] {
        let stale = sample(&key, SampleSpec::new(2, observed_at_ms));
        assert!(matches!(
            evaluate_sample(&definition, Some(&state), Some(&first), &stale, 1).expect("stale"),
            QuotaEvaluation::Stale
        ));
    }

    assert_eq!(
        evaluate_sample(&definition, Some(&state), None, &first, 1)
            .expect_err("missing previous")
            .code(),
        QuotaErrorCode::MissingPrevious
    );

    let other_key = window_key("other", Some("default"));
    let other_sample = sample(&other_key, SampleSpec::new(2, 2_000));
    assert_eq!(
        evaluate_sample(&definition, Some(&state), Some(&first), &other_sample, 1)
            .expect_err("sample mismatch")
            .code(),
        QuotaErrorCode::SampleWindowMismatch
    );

    let other_definition = window_definition(other_key.clone(), QuotaWindowSemantics::Fixed, false);
    let other_first = sample(&other_key, SampleSpec::new(9, 1_000));
    let other_state = started(&other_definition, &other_first);
    assert_eq!(
        evaluate_sample(
            &definition,
            Some(&other_state),
            Some(&other_first),
            &first,
            1
        )
        .expect_err("state mismatch")
        .code(),
        QuotaErrorCode::StateWindowMismatch
    );
}

#[test]
fn explicit_epoch_local_and_manual_resets_preserve_kind_time_and_identity() {
    let key = window_key("personal", Some("default"));
    let definition = window_definition(key.clone(), QuotaWindowSemantics::Fixed, false);
    let mut first_spec = SampleSpec::new(1, 1_000);
    first_spec.resets_at_ms = Some(5_000);
    let first = sample(&key, first_spec);
    let state = started(&definition, &first);

    let mut provider_spec = SampleSpec::new(2, 6_000);
    provider_spec.epoch = Some("epoch-2");
    provider_spec.resets_at_ms = Some(11_000);
    provider_spec.used_ppm = Some(20_000);
    provider_spec.remaining_ppm = Some(980_000);
    let provider_reset = sample(&key, provider_spec);
    let (provider_state, provider_transition) =
        match evaluate_sample(&definition, Some(&state), Some(&first), &provider_reset, 1)
            .expect("provider reset")
        {
            QuotaEvaluation::Reset { state, transition } => (state, transition),
            other => panic!("expected reset, got {other:?}"),
        };
    assert_eq!(
        provider_transition.kind(),
        QuotaTransitionKind::ScheduledReset
    );
    assert_eq!(
        provider_transition.detection_time(),
        QuotaDetectionTime::Interval {
            after_ms: 1_000,
            at_or_before_ms: 6_000,
        }
    );
    assert_eq!(
        lower_hex(provider_transition.id().as_bytes()),
        "695cb1c94bd1feaac1fda12ee571974e2036b6ce8c660c5e790c50e94d8bccfb"
    );
    assert_eq!(
        format!("{:?}", provider_transition.id()),
        "QuotaTransitionId([redacted])"
    );
    assert_ne!(
        provider_transition.previous_epoch_id(),
        provider_state.epoch_id()
    );

    let mut local_spec = SampleSpec::new(3, 4_000);
    local_spec.reset_evidence = QuotaResetEvidence::ExplicitLocal;
    local_spec.source = QuotaEvidenceSource::LocalResetEvent;
    local_spec.used_ppm = Some(10_000);
    local_spec.remaining_ppm = Some(990_000);
    let local_reset = sample(&key, local_spec);
    let local_transition =
        match evaluate_sample(&definition, Some(&state), Some(&first), &local_reset, 1)
            .expect("local reset")
        {
            QuotaEvaluation::Reset { transition, .. } => transition,
            other => panic!("expected reset, got {other:?}"),
        };
    assert_eq!(local_transition.kind(), QuotaTransitionKind::EarlyReset);
    assert_eq!(
        local_transition.source(),
        QuotaEvidenceSource::LocalResetEvent
    );

    let mut manual_spec = SampleSpec::new(4, 4_000);
    manual_spec.reset_evidence = QuotaResetEvidence::ManualOrBanked;
    manual_spec.source = QuotaEvidenceSource::Manual;
    manual_spec.reset_occurred_at_ms = Some(3_500);
    let manual_reset = sample(&key, manual_spec);
    let manual_transition =
        match evaluate_sample(&definition, Some(&state), Some(&first), &manual_reset, 1)
            .expect("manual reset")
        {
            QuotaEvaluation::Reset { transition, .. } => transition,
            other => panic!("expected reset, got {other:?}"),
        };
    assert_eq!(
        manual_transition.kind(),
        QuotaTransitionKind::ManualOrBankedReset
    );
    assert_eq!(
        manual_transition.detection_time(),
        QuotaDetectionTime::Exact(3_500)
    );
}

#[test]
fn provider_thresholds_detect_scheduled_early_and_lower_confidence_unknown_resets() {
    let key = window_key("personal", Some("default"));
    let definition = window_definition(key.clone(), QuotaWindowSemantics::Fixed, true);
    let mut first_spec = SampleSpec::new(1, 1_000);
    first_spec.used_ppm = Some(900_000);
    first_spec.remaining_ppm = Some(100_000);
    first_spec.resets_at_ms = Some(5_000);
    let first = sample(&key, first_spec);
    let state = started(&definition, &first);

    let mut scheduled_spec = SampleSpec::new(2, 6_000);
    scheduled_spec.used_ppm = Some(40_000);
    scheduled_spec.remaining_ppm = Some(960_000);
    scheduled_spec.resets_at_ms = Some(11_000);
    let scheduled = sample(&key, scheduled_spec);
    let scheduled_transition =
        match evaluate_sample(&definition, Some(&state), Some(&first), &scheduled, 1)
            .expect("scheduled threshold reset")
        {
            QuotaEvaluation::Reset { transition, .. } => transition,
            other => panic!("expected reset, got {other:?}"),
        };
    assert_eq!(
        scheduled_transition.kind(),
        QuotaTransitionKind::ScheduledReset
    );
    assert_eq!(scheduled_transition.confidence(), QuotaConfidence::High);

    let mut early_spec = SampleSpec::new(3, 4_000);
    early_spec.used_ppm = Some(40_000);
    early_spec.remaining_ppm = Some(960_000);
    early_spec.resets_at_ms = Some(11_000);
    let early = sample(&key, early_spec);
    let early_transition = match evaluate_sample(&definition, Some(&state), Some(&first), &early, 1)
        .expect("early threshold reset")
    {
        QuotaEvaluation::Reset { transition, .. } => transition,
        other => panic!("expected reset, got {other:?}"),
    };
    assert_eq!(early_transition.kind(), QuotaTransitionKind::EarlyReset);

    let mut unknown_spec = SampleSpec::new(4, 4_000);
    unknown_spec.used_ppm = Some(40_000);
    unknown_spec.remaining_ppm = Some(960_000);
    unknown_spec.resets_at_ms = None;
    let unknown = sample(&key, unknown_spec);
    let unknown_transition =
        match evaluate_sample(&definition, Some(&state), Some(&first), &unknown, 1)
            .expect("unknown threshold reset")
        {
            QuotaEvaluation::Reset { transition, .. } => transition,
            other => panic!("expected reset, got {other:?}"),
        };
    assert_eq!(unknown_transition.kind(), QuotaTransitionKind::UnknownReset);
    assert_eq!(unknown_transition.confidence(), QuotaConfidence::Low);
}

#[test]
fn rolling_recovery_and_untrusted_inference_never_create_automatic_resets() {
    let key = window_key("personal", Some("default"));
    let rolling = window_definition(key.clone(), QuotaWindowSemantics::Rolling, false);
    let mut first_spec = SampleSpec::new(1, 1_000);
    first_spec.used_ppm = Some(900_000);
    first_spec.remaining_ppm = Some(100_000);
    let first = sample(&key, first_spec);
    let state = started(&rolling, &first);
    let mut recovered_spec = SampleSpec::new(2, 2_000);
    recovered_spec.used_ppm = Some(20_000);
    recovered_spec.remaining_ppm = Some(980_000);
    recovered_spec.resets_at_ms = Some(20_000);
    let recovered = sample(&key, recovered_spec);
    assert!(matches!(
        evaluate_sample(&rolling, Some(&state), Some(&first), &recovered, 1)
            .expect("rolling recovery"),
        QuotaEvaluation::Advanced { .. }
    ));

    let fixed = window_definition(key.clone(), QuotaWindowSemantics::Fixed, true);
    let fixed_state = started(&fixed, &first);
    for (quality, confidence) in [
        (QuotaSampleQuality::Conflict, QuotaConfidence::High),
        (QuotaSampleQuality::Authoritative, QuotaConfidence::Unknown),
    ] {
        let mut untrusted_spec = SampleSpec::new(3, 2_000);
        untrusted_spec.used_ppm = Some(20_000);
        untrusted_spec.remaining_ppm = Some(980_000);
        untrusted_spec.resets_at_ms = Some(20_000);
        untrusted_spec.quality = quality;
        untrusted_spec.confidence = confidence;
        let untrusted = sample(&key, untrusted_spec);
        assert!(matches!(
            evaluate_sample(&fixed, Some(&fixed_state), Some(&first), &untrusted, 1)
                .expect("untrusted recovery"),
            QuotaEvaluation::Advanced { .. }
        ));
    }
}

#[test]
fn allowance_changes_are_orthogonal_and_may_accompany_a_reset() {
    let key = window_key("personal", Some("default"));
    let definition = window_definition(key.clone(), QuotaWindowSemantics::Fixed, false);
    let mut first_spec = SampleSpec::new(1, 1_000);
    first_spec.used_units = Some(90);
    first_spec.remaining_units = Some(10);
    first_spec.capacity = Some(100);
    let first = sample(&key, first_spec);
    let state = started(&definition, &first);

    let mut increased_spec = SampleSpec::new(2, 2_000);
    increased_spec.used_units = Some(95);
    increased_spec.remaining_units = Some(55);
    increased_spec.capacity = Some(150);
    let increased = sample(&key, increased_spec);
    let (increased_state, transition) =
        match evaluate_sample(&definition, Some(&state), Some(&first), &increased, 1)
            .expect("allowance increase")
        {
            QuotaEvaluation::AllowanceChanged { state, transition } => (state, transition),
            other => panic!("expected allowance change, got {other:?}"),
        };
    assert_eq!(
        transition.allowance_change().expect("allowance").kind(),
        QuotaAllowanceChangeKind::Increased
    );
    assert_eq!(
        transition.previous_epoch_id(),
        transition.current_epoch_id()
    );
    assert_eq!(increased_state.last_transition_sequence(), 1);

    let mut reset_spec = SampleSpec::new(3, 3_000);
    reset_spec.reset_evidence = QuotaResetEvidence::ManualOrBanked;
    reset_spec.source = QuotaEvidenceSource::Manual;
    reset_spec.used_units = Some(10);
    reset_spec.remaining_units = Some(190);
    reset_spec.capacity = Some(200);
    let reset = sample(&key, reset_spec);
    let reset_transition = match evaluate_sample(
        &definition,
        Some(&increased_state),
        Some(&increased),
        &reset,
        2,
    )
    .expect("reset plus allowance")
    {
        QuotaEvaluation::Reset { transition, .. } => transition,
        other => panic!("expected reset, got {other:?}"),
    };
    assert_eq!(
        reset_transition
            .allowance_change()
            .expect("allowance")
            .kind(),
        QuotaAllowanceChangeKind::Increased
    );
}

#[test]
fn repeated_resets_have_monotonic_sequences_and_distinct_epoch_transition_ids() {
    let key = window_key("personal", Some("default"));
    let definition = window_definition(key.clone(), QuotaWindowSemantics::Fixed, false);
    let mut first_spec = SampleSpec::new(1, 1_000);
    first_spec.resets_at_ms = Some(10_000);
    let first = sample(&key, first_spec);
    let state = started(&definition, &first);

    let mut reset_one_spec = SampleSpec::new(2, 4_000);
    reset_one_spec.reset_evidence = QuotaResetEvidence::ExplicitLocal;
    reset_one_spec.source = QuotaEvidenceSource::LocalResetEvent;
    reset_one_spec.resets_at_ms = Some(20_000);
    let reset_one = sample(&key, reset_one_spec);
    let (state, transition_one) =
        match evaluate_sample(&definition, Some(&state), Some(&first), &reset_one, 1)
            .expect("first reset")
        {
            QuotaEvaluation::Reset { state, transition } => (state, transition),
            other => panic!("expected reset, got {other:?}"),
        };

    let mut reset_two_spec = SampleSpec::new(3, 5_000);
    reset_two_spec.reset_evidence = QuotaResetEvidence::ManualOrBanked;
    reset_two_spec.source = QuotaEvidenceSource::Manual;
    reset_two_spec.resets_at_ms = Some(30_000);
    let reset_two = sample(&key, reset_two_spec);
    let (state, transition_two) =
        match evaluate_sample(&definition, Some(&state), Some(&reset_one), &reset_two, 2)
            .expect("second reset")
        {
            QuotaEvaluation::Reset { state, transition } => (state, transition),
            other => panic!("expected reset, got {other:?}"),
        };

    assert_eq!(transition_one.sequence(), 1);
    assert_eq!(transition_two.sequence(), 2);
    assert_ne!(transition_one.id(), transition_two.id());
    assert_ne!(
        transition_one.current_epoch_id(),
        transition_two.current_epoch_id()
    );
    assert_eq!(state.last_transition_sequence(), 2);
}

#[test]
fn transition_sequence_is_exact_checked_and_never_wraps() {
    let key = window_key("personal", Some("default"));
    let definition = window_definition(key.clone(), QuotaWindowSemantics::Fixed, false);
    let first = sample(&key, SampleSpec::new(1, 1_000));
    let state = started(&definition, &first);
    let mut reset_spec = SampleSpec::new(2, 2_000);
    reset_spec.reset_evidence = QuotaResetEvidence::ExplicitProvider;
    let reset = sample(&key, reset_spec);

    assert_eq!(
        evaluate_sample(&definition, Some(&state), Some(&first), &reset, 2)
            .expect_err("wrong next sequence")
            .code(),
        QuotaErrorCode::InvalidTransitionSequence
    );

    let mut saturated_parts = state.to_parts();
    saturated_parts.last_transition_sequence = u64::MAX;
    let saturated = QuotaEpochState::restore(saturated_parts).expect("restored state");
    assert_eq!(
        evaluate_sample(
            &definition,
            Some(&saturated),
            Some(&first),
            &reset,
            u64::MAX
        )
        .expect_err("sequence overflow")
        .code(),
        QuotaErrorCode::TransitionSequenceOverflow
    );
}
