use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
    QuotaResetThresholds, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey,
    QuotaWindowSemantics, UsageProviderId,
};
use tokenmaster_quota::{QuotaEvaluation, QuotaTransitionKind, evaluate_sample};

fn window_key(name: &str) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("adversarial-account").expect("account"),
            None,
        ),
        QuotaWindowId::new(name).expect("window"),
    )
}

fn definition(key: QuotaWindowKey, semantics: QuotaWindowSemantics) -> QuotaWindowDefinition {
    let reset_thresholds = (semantics == QuotaWindowSemantics::Fixed).then(|| {
        QuotaResetThresholds::new(
            Some(QuotaRatio::new(50_000).expect("used threshold")),
            Some(QuotaRatio::new(950_000).expect("remaining threshold")),
            Some(QuotaRatio::new(500_000).expect("drop threshold")),
        )
        .expect("thresholds")
    });
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key,
        revision: 1,
        label_key: "quota.adversarial".to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics,
        nominal_duration_seconds: None,
        reset_thresholds,
    })
    .expect("definition")
}

fn observation_id(value: u64) -> QuotaObservationId {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    QuotaObservationId::from_bytes(bytes)
}

#[allow(clippy::too_many_arguments)]
fn sample(
    key: &QuotaWindowKey,
    observation: u64,
    observed_at_ms: i64,
    provider_epoch: &str,
    used_ppm: u32,
    quality: QuotaSampleQuality,
    confidence: QuotaConfidence,
    reset_evidence: QuotaResetEvidence,
) -> QuotaSample {
    QuotaSample::new(QuotaSampleParts {
        key: key.clone(),
        observation_id: observation_id(observation),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 100,
        stale_after_ms: observed_at_ms + 200,
        provider_epoch_id: Some(QuotaProviderEpochId::new(provider_epoch).expect("provider epoch")),
        used_ratio: Some(QuotaRatio::new(used_ppm).expect("used ratio")),
        remaining_ratio: Some(QuotaRatio::new(1_000_000 - used_ppm).expect("remaining ratio")),
        units: None,
        advertised_resets_at_ms: Some(20_000),
        quality,
        source: QuotaEvidenceSource::ProviderOfficial,
        confidence,
        reset_evidence,
        reset_occurred_at_ms: None,
    })
    .expect("sample")
}

#[test]
fn adversarial_recovery_matrix_never_infers_without_trusted_fixed_window_authority() {
    let qualities = [
        QuotaSampleQuality::Authoritative,
        QuotaSampleQuality::Partial,
        QuotaSampleQuality::Conflict,
        QuotaSampleQuality::Unknown,
    ];
    let confidences = [
        QuotaConfidence::High,
        QuotaConfidence::Medium,
        QuotaConfidence::Low,
        QuotaConfidence::Unknown,
    ];
    let recoveries = [0, 1, 49_999, 50_000];
    let mut evaluated = 0_u64;

    for (definition_index, semantics) in [
        QuotaWindowSemantics::Rolling,
        QuotaWindowSemantics::Unknown,
        QuotaWindowSemantics::Fixed,
    ]
    .into_iter()
    .enumerate()
    {
        let key = window_key(&format!("matrix-{definition_index}"));
        let definition = definition(key.clone(), semantics);
        let first = sample(
            &key,
            1,
            1_000,
            "same-epoch",
            900_000,
            QuotaSampleQuality::Authoritative,
            QuotaConfidence::High,
            QuotaResetEvidence::None,
        );
        let state = match evaluate_sample(&definition, None, None, &first, 1).expect("start") {
            QuotaEvaluation::Started { state } => state,
            other => panic!("expected start, got {other:?}"),
        };

        for quality in qualities {
            for confidence in confidences {
                for used_ppm in recoveries {
                    if semantics == QuotaWindowSemantics::Fixed
                        && quality == QuotaSampleQuality::Authoritative
                        && matches!(confidence, QuotaConfidence::High | QuotaConfidence::Medium)
                    {
                        continue;
                    }
                    evaluated += 1;
                    let candidate = sample(
                        &key,
                        evaluated + 1,
                        2_000 + i64::try_from(evaluated).expect("time"),
                        "same-epoch",
                        used_ppm,
                        quality,
                        confidence,
                        QuotaResetEvidence::None,
                    );
                    assert!(
                        matches!(
                            evaluate_sample(
                                &definition,
                                Some(&state),
                                Some(&first),
                                &candidate,
                                1,
                            )
                            .expect("adversarial evaluation"),
                            QuotaEvaluation::Advanced { .. }
                        ),
                        "untrusted or non-fixed recovery inferred a reset: semantics={semantics:?}, \
                         quality={quality:?}, confidence={confidence:?}, used_ppm={used_ppm}"
                    );
                }
            }
        }
    }
    assert!(evaluated >= 160);
}

#[test]
fn explicit_manual_authority_wins_without_exposing_private_identity_in_debug() {
    let key = window_key("manual-private-window");
    let definition = definition(key.clone(), QuotaWindowSemantics::Rolling);
    let first = sample(
        &key,
        1,
        1_000,
        "private-epoch-before",
        900_000,
        QuotaSampleQuality::Authoritative,
        QuotaConfidence::High,
        QuotaResetEvidence::None,
    );
    let state = match evaluate_sample(&definition, None, None, &first, 1).expect("start") {
        QuotaEvaluation::Started { state } => state,
        other => panic!("expected start, got {other:?}"),
    };
    let manual = sample(
        &key,
        2,
        2_000,
        "private-epoch-after",
        10_000,
        QuotaSampleQuality::Conflict,
        QuotaConfidence::Unknown,
        QuotaResetEvidence::ManualOrBanked,
    );
    let transition = match evaluate_sample(&definition, Some(&state), Some(&first), &manual, 1)
        .expect("manual reset")
    {
        QuotaEvaluation::Reset { transition, .. } => transition,
        other => panic!("expected reset, got {other:?}"),
    };
    assert_eq!(transition.kind(), QuotaTransitionKind::ManualOrBankedReset);
    let debug = format!(
        "{:?}{:?}{:?}",
        transition.id(),
        transition.previous_epoch_id(),
        transition.current_epoch_id()
    );
    for private in [
        "adversarial-account",
        "manual-private-window",
        "private-epoch-before",
        "private-epoch-after",
    ] {
        assert!(!debug.contains(private), "Debug exposed {private}");
    }
}
