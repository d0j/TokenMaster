use serde_json::json;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
    QuotaResetThresholds, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId,
    QuotaWindowKey, QuotaWindowSemantics, QuotaWorkspaceId, UsageProviderId,
};

fn window_key() -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("personal").expect("account"),
            Some(QuotaWorkspaceId::new("default").expect("workspace")),
        ),
        QuotaWindowId::new("weekly").expect("window"),
    )
}

fn sample_parts() -> QuotaSampleParts {
    QuotaSampleParts {
        key: window_key(),
        observation_id: QuotaObservationId::from_bytes([7; 32]),
        observed_at_ms: 1_000,
        fresh_until_ms: 2_000,
        stale_after_ms: 5_000,
        provider_epoch_id: Some(QuotaProviderEpochId::new("epoch-17").expect("epoch")),
        used_ratio: Some(QuotaRatio::new(840_000).expect("used")),
        remaining_ratio: Some(QuotaRatio::new(160_000).expect("remaining")),
        units: None,
        advertised_resets_at_ms: Some(10_000),
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderLocal,
        confidence: QuotaConfidence::High,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    }
}

#[test]
fn ratios_are_exact_parts_per_million() {
    assert_eq!(
        QuotaRatio::new(840_000).expect("ratio").parts_per_million(),
        840_000
    );
    assert!(QuotaRatio::new(1_000_001).is_err());
}

#[test]
fn fixed_window_definition_accepts_provider_thresholds() {
    let definition = QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key(),
        revision: 1,
        label_key: "quota.weekly".to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(603_900),
        reset_thresholds: Some(
            QuotaResetThresholds::new(
                Some(QuotaRatio::new(50_000).expect("used floor")),
                Some(QuotaRatio::new(950_000).expect("remaining ceiling")),
                Some(QuotaRatio::new(500_000).expect("minimum drop")),
            )
            .expect("thresholds"),
        ),
    })
    .expect("definition");

    assert_eq!(definition.revision(), 1);
    assert_eq!(definition.label_key(), "quota.weekly");
    assert_eq!(definition.presentation(), QuotaPresentationDirection::Used);
    assert_eq!(definition.semantics(), QuotaWindowSemantics::Fixed);
    assert_eq!(definition.nominal_duration_seconds(), Some(603_900));
    assert!(definition.reset_thresholds().is_some());
}

#[test]
fn sample_preserves_ratio_only_truth_without_capacity() {
    let sample = QuotaSample::new(sample_parts()).expect("sample");

    assert!(sample.units().is_none());
    assert_eq!(
        sample.used_ratio().expect("used").parts_per_million(),
        840_000
    );
    assert_eq!(
        sample
            .remaining_ratio()
            .expect("remaining")
            .parts_per_million(),
        160_000
    );
    assert_eq!(sample.advertised_resets_at_ms(), Some(10_000));
    assert_eq!(sample.reset_occurred_at_ms(), None);
}

#[test]
fn absolute_units_are_optional_bounded_and_coherent() {
    let units = QuotaUnits::new(
        QuotaUnitId::new("provider_units").expect("unit"),
        Some(84),
        Some(16),
        Some(100),
    )
    .expect("units");

    assert_eq!(units.unit_id().as_str(), "provider_units");
    assert_eq!(units.used(), Some(84));
    assert_eq!(units.remaining(), Some(16));
    assert_eq!(units.capacity(), Some(100));
    assert!(
        QuotaUnits::new(
            QuotaUnitId::new("provider_units").expect("unit"),
            Some(101),
            None,
            Some(100),
        )
        .is_err()
    );
    assert!(
        QuotaUnits::new(
            QuotaUnitId::new("provider_units").expect("unit"),
            None,
            Some(101),
            Some(100),
        )
        .is_err()
    );
    assert!(
        QuotaUnits::new(
            QuotaUnitId::new("provider_units").expect("unit"),
            None,
            None,
            None,
        )
        .is_err()
    );
}

#[test]
fn quota_ids_are_bounded_ascii_values() {
    assert_eq!(
        QuotaAccountId::new("account_17").expect("account").as_str(),
        "account_17"
    );

    for value in [
        "",
        "with space",
        "provider/path",
        "provider\\path",
        "line\nfeed",
    ] {
        assert!(QuotaAccountId::new(value).is_err());
        assert!(QuotaWorkspaceId::new(value).is_err());
        assert!(QuotaWindowId::new(value).is_err());
        assert!(QuotaUnitId::new(value).is_err());
        assert!(QuotaProviderEpochId::new(value).is_err());
    }

    let oversized = "q".repeat(129);
    assert!(QuotaAccountId::new(&oversized).is_err());
    assert!(QuotaWorkspaceId::new(&oversized).is_err());
    assert!(QuotaWindowId::new(&oversized).is_err());
    assert!(QuotaUnitId::new(&oversized).is_err());
    assert!(QuotaProviderEpochId::new(&oversized).is_err());
}

#[test]
fn definition_rejects_ambiguous_or_unbounded_rules() {
    let thresholds = QuotaResetThresholds::new(
        Some(QuotaRatio::new(50_000).expect("used floor")),
        None,
        None,
    )
    .expect("thresholds");

    for semantics in [
        QuotaWindowSemantics::Rolling,
        QuotaWindowSemantics::Credit,
        QuotaWindowSemantics::Unknown,
    ] {
        assert!(
            QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
                key: window_key(),
                revision: 1,
                label_key: "quota.window".to_owned(),
                presentation: QuotaPresentationDirection::Remaining,
                semantics,
                nominal_duration_seconds: None,
                reset_thresholds: Some(thresholds.clone()),
            })
            .is_err()
        );
    }

    for (revision, label_key, duration) in [
        (0, "quota.window".to_owned(), Some(1)),
        (1, String::new(), Some(1)),
        (1, "quota window".to_owned(), Some(1)),
        (1, "q".repeat(129), Some(1)),
        (1, "quota.window".to_owned(), Some(0)),
    ] {
        assert!(
            QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
                key: window_key(),
                revision,
                label_key,
                presentation: QuotaPresentationDirection::Used,
                semantics: QuotaWindowSemantics::Fixed,
                nominal_duration_seconds: duration,
                reset_thresholds: None,
            })
            .is_err()
        );
    }

    assert!(QuotaResetThresholds::new(None, None, None).is_err());
}

#[test]
fn sample_rejects_empty_incoherent_or_invalid_time_evidence() {
    for (observed, fresh, stale) in [(0, 1, 2), (2, 1, 3), (1, 3, 2), (1, 1, 0)] {
        let mut parts = sample_parts();
        parts.observed_at_ms = observed;
        parts.fresh_until_ms = fresh;
        parts.stale_after_ms = stale;
        assert!(QuotaSample::new(parts).is_err());
    }

    let mut empty = sample_parts();
    empty.provider_epoch_id = None;
    empty.used_ratio = None;
    empty.remaining_ratio = None;
    empty.units = None;
    empty.advertised_resets_at_ms = None;
    empty.reset_evidence = QuotaResetEvidence::None;
    assert!(QuotaSample::new(empty).is_err());

    let mut unsupported_exact_time = sample_parts();
    unsupported_exact_time.reset_occurred_at_ms = Some(900);
    assert!(QuotaSample::new(unsupported_exact_time).is_err());

    for occurred_at in [0, 1_001] {
        let mut invalid = sample_parts();
        invalid.reset_evidence = QuotaResetEvidence::ExplicitProvider;
        invalid.reset_occurred_at_ms = Some(occurred_at);
        assert!(QuotaSample::new(invalid).is_err());
    }

    let mut valid = sample_parts();
    valid.reset_evidence = QuotaResetEvidence::ManualOrBanked;
    valid.reset_occurred_at_ms = Some(999);
    assert_eq!(
        QuotaSample::new(valid)
            .expect("explicit reset")
            .reset_occurred_at_ms(),
        Some(999)
    );
}

#[test]
fn observation_identity_debug_is_exactly_redacted() {
    let id = QuotaObservationId::from_bytes([0xAB; 32]);
    assert_eq!(format!("{id:?}"), "QuotaObservationId([redacted])");
    assert_eq!(id.as_bytes(), &[0xAB; 32]);
}

#[test]
fn serde_round_trips_validated_values_without_converting_absence_to_zero() {
    let sample = QuotaSample::new(QuotaSampleParts {
        provider_epoch_id: None,
        units: None,
        advertised_resets_at_ms: None,
        ..sample_parts()
    })
    .expect("sample");
    let value = serde_json::to_value(&sample).expect("sample serializes");

    assert_eq!(value["provider_epoch_id"], json!(null));
    assert_eq!(value["units"], json!(null));
    assert_eq!(value["advertised_resets_at_ms"], json!(null));
    assert_eq!(value["reset_occurred_at_ms"], json!(null));

    let round_trip: QuotaSample =
        serde_json::from_value(value.clone()).expect("sample deserializes");
    assert_eq!(round_trip, sample);

    let mut invalid_ratio = value.clone();
    invalid_ratio["used_ratio"] = json!(1_000_001);
    assert!(serde_json::from_value::<QuotaSample>(invalid_ratio).is_err());

    let mut invalid_time = value.clone();
    invalid_time["fresh_until_ms"] = json!(999);
    assert!(serde_json::from_value::<QuotaSample>(invalid_time).is_err());

    let mut invalid_account = value;
    invalid_account["key"]["scope"]["account_id"] = json!("private/path");
    assert!(serde_json::from_value::<QuotaSample>(invalid_account).is_err());
}

#[test]
fn public_enums_use_stable_snake_case_wire_values() {
    assert_eq!(
        serde_json::to_string(&QuotaWindowSemantics::Fixed).expect("semantics"),
        r#""fixed""#
    );
    assert_eq!(
        serde_json::to_string(&QuotaResetEvidence::ExplicitLocal).expect("evidence"),
        r#""explicit_local""#
    );
    assert_eq!(
        serde_json::to_string(&QuotaSampleQuality::Conflict).expect("quality"),
        r#""conflict""#
    );
}

#[test]
fn nested_quota_identity_wire_values_reject_unknown_fields() {
    let sample = QuotaSample::new(sample_parts()).expect("sample");
    let value = serde_json::to_value(sample).expect("sample serializes");

    let mut unknown_scope = value.clone();
    unknown_scope["key"]["scope"]["unexpected"] = json!("ignored");
    assert!(serde_json::from_value::<QuotaSample>(unknown_scope).is_err());

    let mut unknown_window = value;
    unknown_window["key"]["unexpected"] = json!("ignored");
    assert!(serde_json::from_value::<QuotaSample>(unknown_window).is_err());
}
