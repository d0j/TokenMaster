use std::path::Path;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample,
    QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaUnitId, QuotaUnits,
    QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey,
    QuotaWindowSemantics, UsageProviderId,
};
use tokenmaster_store::{QuotaApplyStatus, StoreErrorCode, UsageStore};

fn window_key(account: &str) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new(account).expect("account"),
            None,
        ),
        QuotaWindowId::new("weekly").expect("window"),
    )
}

fn definition(account: &str, revision: u64) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key(account),
        revision,
        label_key: "quota.weekly".to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(603_900),
        reset_thresholds: None,
    })
    .expect("definition")
}

#[allow(clippy::too_many_arguments)]
fn sample(
    account: &str,
    observation_byte: u8,
    observed_at_ms: i64,
    provider_epoch: &str,
    used_ratio_ppm: u32,
    unit_id: &str,
    used_units: u64,
    capacity_units: u64,
    reset_evidence: QuotaResetEvidence,
) -> QuotaSample {
    QuotaSample::new(QuotaSampleParts {
        key: window_key(account),
        observation_id: QuotaObservationId::from_bytes([observation_byte; 32]),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 100,
        stale_after_ms: observed_at_ms + 200,
        provider_epoch_id: Some(QuotaProviderEpochId::new(provider_epoch).expect("epoch")),
        used_ratio: Some(QuotaRatio::new(used_ratio_ppm).expect("used ratio")),
        remaining_ratio: Some(
            QuotaRatio::new(1_000_000 - used_ratio_ppm).expect("remaining ratio"),
        ),
        units: Some(
            QuotaUnits::new(
                QuotaUnitId::new(unit_id).expect("unit"),
                Some(used_units),
                Some(capacity_units - used_units),
                Some(capacity_units),
            )
            .expect("units"),
        ),
        advertised_resets_at_ms: Some(observed_at_ms + 10_000),
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderLocal,
        confidence: QuotaConfidence::High,
        reset_evidence,
        reset_occurred_at_ms: match reset_evidence {
            QuotaResetEvidence::None => None,
            _ => Some(observed_at_ms),
        },
    })
    .expect("sample")
}

fn quota_counts(path: &Path) -> (i64, i64, i64, i64, i64) {
    let connection = Connection::open(path).expect("inspect quota archive");
    connection
        .query_row(
            "SELECT state.revision,
                    (SELECT count(*) FROM quota_sample),
                    (SELECT count(*) FROM quota_epoch_history),
                    (SELECT count(*) FROM quota_transition),
                    (SELECT count(*) FROM quota_window_current)
             FROM quota_state AS state WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("quota counts")
}

#[test]
fn start_duplicate_stale_and_advance_publish_only_visible_changes() {
    let mut store = UsageStore::in_memory().expect("store");
    let weekly_definition = definition("personal", 1);
    let first = sample(
        "personal",
        1,
        100,
        "epoch-1",
        100_000,
        "requests",
        10,
        100,
        QuotaResetEvidence::None,
    );

    let started = store
        .apply_quota_observation(&weekly_definition, &first)
        .expect("start");
    assert_eq!(started.status(), QuotaApplyStatus::Started);
    assert_eq!(started.quota_revision().get(), 1);
    assert_eq!(started.transition_sequence(), 0);
    assert!(started.transition_id().is_none());

    let duplicate = store
        .apply_quota_observation(&weekly_definition, &first)
        .expect("duplicate");
    assert_eq!(duplicate.status(), QuotaApplyStatus::Duplicate);
    assert_eq!(duplicate.quota_revision().get(), 1);
    assert_eq!(duplicate.transition_sequence(), 0);

    let future_definition = definition("personal", 2);
    let duplicate_with_future_definition = store
        .apply_quota_observation(&future_definition, &first)
        .expect("duplicate with an unpublished future definition");
    assert_eq!(
        duplicate_with_future_definition.status(),
        QuotaApplyStatus::Duplicate
    );
    assert_eq!(duplicate_with_future_definition.quota_revision().get(), 1);

    let stale = sample(
        "personal",
        2,
        99,
        "epoch-1",
        90_000,
        "requests",
        9,
        100,
        QuotaResetEvidence::None,
    );
    let stale_result = store
        .apply_quota_observation(&weekly_definition, &stale)
        .expect("stale");
    assert_eq!(stale_result.status(), QuotaApplyStatus::Stale);
    assert_eq!(stale_result.quota_revision().get(), 1);

    let advanced = sample(
        "personal",
        3,
        200,
        "epoch-1",
        200_000,
        "requests",
        20,
        100,
        QuotaResetEvidence::None,
    );
    let advanced_result = store
        .apply_quota_observation(&weekly_definition, &advanced)
        .expect("advance");
    assert_eq!(advanced_result.status(), QuotaApplyStatus::Advanced);
    assert_eq!(advanced_result.quota_revision().get(), 2);
    assert_eq!(advanced_result.transition_sequence(), 0);
}

#[test]
fn allowance_change_and_reset_plus_allowance_are_persisted_exactly() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-allowance.sqlite3");
    let definition = definition("personal", 1);
    let mut store = UsageStore::open(&path).expect("store");
    let first = sample(
        "personal",
        10,
        1_000,
        "epoch-1",
        100_000,
        "requests",
        10,
        100,
        QuotaResetEvidence::None,
    );
    store
        .apply_quota_observation(&definition, &first)
        .expect("start");

    let allowance = sample(
        "personal",
        11,
        2_000,
        "epoch-1",
        100_000,
        "requests",
        20,
        200,
        QuotaResetEvidence::None,
    );
    let allowance_result = store
        .apply_quota_observation(&definition, &allowance)
        .expect("allowance");
    assert_eq!(
        allowance_result.status(),
        QuotaApplyStatus::AllowanceChanged
    );
    assert_eq!(allowance_result.quota_revision().get(), 2);
    assert_eq!(allowance_result.transition_sequence(), 1);
    assert!(allowance_result.transition_id().is_some());

    let reset = sample(
        "personal",
        12,
        3_000,
        "epoch-2",
        50_000,
        "credits",
        5,
        300,
        QuotaResetEvidence::None,
    );
    let reset_result = store
        .apply_quota_observation(&definition, &reset)
        .expect("reset plus allowance");
    assert_eq!(reset_result.status(), QuotaApplyStatus::Reset);
    assert_eq!(reset_result.quota_revision().get(), 3);
    assert_eq!(reset_result.transition_sequence(), 2);
    drop(store);

    assert_eq!(quota_counts(&path), (3, 3, 1, 2, 1));
    let connection = Connection::open(&path).expect("inspect transitions");
    let transitions = connection
        .prepare(
            "SELECT sequence, kind, allowance_change_kind
             FROM quota_transition ORDER BY sequence",
        )
        .expect("prepare transitions")
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .expect("transition rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect transitions");
    assert_eq!(
        transitions,
        vec![
            (
                1,
                "allowance_changed".to_owned(),
                Some("increased".to_owned())
            ),
            (2, "early_reset".to_owned(), Some("unit_changed".to_owned())),
        ]
    );
}

#[test]
fn repeated_resets_are_sequenced_and_retry_is_a_noop() {
    let mut store = UsageStore::in_memory().expect("store");
    let definition = definition("personal", 1);
    let first = sample(
        "personal",
        20,
        10_000,
        "epoch-1",
        900_000,
        "requests",
        90,
        100,
        QuotaResetEvidence::None,
    );
    store
        .apply_quota_observation(&definition, &first)
        .expect("start");
    let second = sample(
        "personal",
        21,
        20_000,
        "epoch-2",
        100_000,
        "requests",
        10,
        100,
        QuotaResetEvidence::None,
    );
    let first_reset = store
        .apply_quota_observation(&definition, &second)
        .expect("first reset");
    let third = sample(
        "personal",
        22,
        30_000,
        "epoch-3",
        50_000,
        "requests",
        5,
        100,
        QuotaResetEvidence::ManualOrBanked,
    );
    let second_reset = store
        .apply_quota_observation(&definition, &third)
        .expect("second reset");
    assert_eq!(first_reset.transition_sequence(), 1);
    assert_eq!(second_reset.transition_sequence(), 2);
    assert_ne!(first_reset.transition_id(), second_reset.transition_id());

    let retry = store
        .apply_quota_observation(&definition, &third)
        .expect("retry");
    assert_eq!(retry.status(), QuotaApplyStatus::Duplicate);
    assert_eq!(retry.quota_revision().get(), 3);
    assert_eq!(retry.transition_sequence(), 2);
    assert!(retry.transition_id().is_none());
}

#[test]
fn account_scopes_are_isolated_even_with_the_same_window_id() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-scopes.sqlite3");
    let mut store = UsageStore::open(&path).expect("store");
    for (account, observation) in [("personal", 30_u8), ("work", 31_u8)] {
        store
            .apply_quota_observation(
                &definition(account, 1),
                &sample(
                    account,
                    observation,
                    1_000,
                    "epoch-1",
                    100_000,
                    "requests",
                    10,
                    100,
                    QuotaResetEvidence::None,
                ),
            )
            .expect("scope start");
    }
    drop(store);

    assert_eq!(quota_counts(&path), (2, 2, 0, 0, 2));
}

#[test]
fn reopen_restores_epoch_definition_and_sequence_continuity() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-reopen.sqlite3");
    {
        let mut store = UsageStore::open(&path).expect("store");
        store
            .apply_quota_observation(
                &definition("personal", 1),
                &sample(
                    "personal",
                    40,
                    1_000,
                    "epoch-1",
                    100_000,
                    "requests",
                    10,
                    100,
                    QuotaResetEvidence::None,
                ),
            )
            .expect("start");
        let advanced = store
            .apply_quota_observation(
                &definition("personal", 2),
                &sample(
                    "personal",
                    41,
                    2_000,
                    "epoch-1",
                    200_000,
                    "requests",
                    20,
                    100,
                    QuotaResetEvidence::None,
                ),
            )
            .expect("definition advance");
        assert_eq!(advanced.status(), QuotaApplyStatus::Advanced);
    }

    let mut reopened = UsageStore::open(&path).expect("reopen");
    let reset = reopened
        .apply_quota_observation(
            &definition("personal", 2),
            &sample(
                "personal",
                42,
                3_000,
                "epoch-2",
                50_000,
                "requests",
                5,
                100,
                QuotaResetEvidence::None,
            ),
        )
        .expect("reset after reopen");
    assert_eq!(reset.status(), QuotaApplyStatus::Reset);
    assert_eq!(reset.quota_revision().get(), 3);
    assert_eq!(reset.transition_sequence(), 1);

    let regressed = reopened
        .apply_quota_observation(
            &definition("personal", 1),
            &sample(
                "personal",
                43,
                4_000,
                "epoch-2",
                60_000,
                "requests",
                6,
                100,
                QuotaResetEvidence::None,
            ),
        )
        .expect_err("definition regression must fail");
    assert_eq!(regressed.code(), StoreErrorCode::StaleRevision);
}

#[test]
fn reused_observation_identity_with_different_content_fails_without_publication() {
    let mut store = UsageStore::in_memory().expect("store");
    let definition = definition("personal", 1);
    let first = sample(
        "personal",
        50,
        1_000,
        "epoch-1",
        100_000,
        "requests",
        10,
        100,
        QuotaResetEvidence::None,
    );
    store
        .apply_quota_observation(&definition, &first)
        .expect("start");
    let conflicting = sample(
        "personal",
        50,
        1_001,
        "epoch-1",
        200_000,
        "requests",
        20,
        100,
        QuotaResetEvidence::None,
    );
    let error = store
        .apply_quota_observation(&definition, &conflicting)
        .expect_err("duplicate conflict");
    assert_eq!(error.code(), StoreErrorCode::InvalidValue);

    let retry = store
        .apply_quota_observation(&definition, &first)
        .expect("original remains current");
    assert_eq!(retry.status(), QuotaApplyStatus::Duplicate);
    assert_eq!(retry.quota_revision().get(), 1);
}

#[test]
fn observation_identity_is_global_and_definition_identity_is_immutable() {
    let mut store = UsageStore::in_memory().expect("store");
    let personal_definition = definition("personal", 1);
    let first = sample(
        "personal",
        60,
        1_000,
        "epoch-1",
        100_000,
        "requests",
        10,
        100,
        QuotaResetEvidence::None,
    );
    store
        .apply_quota_observation(&personal_definition, &first)
        .expect("start");

    let cross_scope_reuse = sample(
        "work",
        60,
        1_000,
        "epoch-1",
        100_000,
        "requests",
        10,
        100,
        QuotaResetEvidence::None,
    );
    let observation_error = store
        .apply_quota_observation(&definition("work", 1), &cross_scope_reuse)
        .expect_err("observation identity must be global");
    assert_eq!(observation_error.code(), StoreErrorCode::InvalidValue);

    let conflicting_definition = QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key("personal"),
        revision: 1,
        label_key: "quota.weekly.changed".to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(603_900),
        reset_thresholds: None,
    })
    .expect("conflicting definition");
    let definition_error = store
        .apply_quota_observation(
            &conflicting_definition,
            &sample(
                "personal",
                61,
                2_000,
                "epoch-1",
                200_000,
                "requests",
                20,
                100,
                QuotaResetEvidence::None,
            ),
        )
        .expect_err("definition identity must be immutable");
    assert_eq!(definition_error.code(), StoreErrorCode::InvalidValue);

    let retry = store
        .apply_quota_observation(&personal_definition, &first)
        .expect("original state remains current");
    assert_eq!(retry.status(), QuotaApplyStatus::Duplicate);
    assert_eq!(retry.quota_revision().get(), 1);
}

#[test]
fn missing_current_projection_fails_closed_without_repair() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-corrupt-current.sqlite3");
    let definition = definition("personal", 1);
    let mut store = UsageStore::open(&path).expect("store");
    store
        .apply_quota_observation(
            &definition,
            &sample(
                "personal",
                70,
                1_000,
                "epoch-1",
                100_000,
                "requests",
                10,
                100,
                QuotaResetEvidence::None,
            ),
        )
        .expect("start");
    let external = Connection::open(&path).expect("external connection");
    external
        .execute("DELETE FROM quota_window_current", [])
        .expect("remove current projection");
    drop(external);

    let error = store
        .apply_quota_observation(
            &definition,
            &sample(
                "personal",
                71,
                2_000,
                "epoch-1",
                200_000,
                "requests",
                20,
                100,
                QuotaResetEvidence::None,
            ),
        )
        .expect_err("missing current projection must fail closed");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
    drop(store);
    assert_eq!(quota_counts(&path), (1, 1, 0, 0, 0));

    let reopen_error = UsageStore::open(&path).expect_err("corruption must fail on reopen");
    assert_eq!(reopen_error.code(), StoreErrorCode::InvalidStoredValue);
}
