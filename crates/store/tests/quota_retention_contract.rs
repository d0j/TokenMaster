use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample,
    QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaUnitId, QuotaUnits,
    QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey,
    QuotaWindowSemantics, UsageProviderId,
};
use tokenmaster_store::{
    DEFAULT_QUOTA_EPOCHS_PER_WINDOW, DEFAULT_QUOTA_SAMPLES_PER_WINDOW,
    DEFAULT_QUOTA_TRANSITIONS_PER_WINDOW, MAX_QUOTA_EPOCHS_PER_WINDOW,
    MAX_QUOTA_MAINTENANCE_PAGE_SIZE, MAX_QUOTA_SAMPLES_PER_WINDOW,
    MAX_QUOTA_TRANSITIONS_PER_WINDOW, QuotaApplyStatus, StoreErrorCode, UsageStore,
};

fn window_key(window_id: &str) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("personal").expect("account"),
            None,
        ),
        QuotaWindowId::new(window_id).expect("window"),
    )
}

fn definition(window_id: &str, revision: u64) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key(window_id),
        revision,
        label_key: format!("quota.{window_id}"),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(604_800),
        reset_thresholds: None,
    })
    .expect("definition")
}

fn observation_id(value: u64) -> QuotaObservationId {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    QuotaObservationId::from_bytes(bytes)
}

fn sample(
    window_id: &str,
    observation: u64,
    provider_epoch: &str,
    used_units: u64,
    capacity_units: u64,
) -> QuotaSample {
    let used_ratio =
        u32::try_from(used_units.checked_mul(1_000_000).expect("ratio multiply") / capacity_units)
            .expect("ratio");
    QuotaSample::new(QuotaSampleParts {
        key: window_key(window_id),
        observation_id: observation_id(observation),
        observed_at_ms: i64::try_from(observation + 1).expect("observed time"),
        fresh_until_ms: i64::try_from(observation + 2).expect("fresh time"),
        stale_after_ms: i64::try_from(observation + 3).expect("stale time"),
        provider_epoch_id: Some(QuotaProviderEpochId::new(provider_epoch).expect("epoch")),
        used_ratio: Some(QuotaRatio::new(used_ratio).expect("used ratio")),
        remaining_ratio: Some(QuotaRatio::new(1_000_000 - used_ratio).expect("remaining ratio")),
        units: Some(
            QuotaUnits::new(
                QuotaUnitId::new("requests").expect("unit"),
                Some(used_units),
                Some(capacity_units - used_units),
                Some(capacity_units),
            )
            .expect("units"),
        ),
        advertised_resets_at_ms: Some(10_000_000),
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderLocal,
        confidence: QuotaConfidence::High,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    })
    .expect("sample")
}

fn quota_counts(path: &Path) -> (i64, i64, i64, i64, i64) {
    let connection = Connection::open(path).expect("inspect archive");
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

fn window_sample_count(path: &Path, window_id: &str) -> i64 {
    let connection = Connection::open(path).expect("inspect archive");
    connection
        .query_row(
            "SELECT count(*) FROM quota_sample WHERE window_id = ?1",
            params![window_id],
            |row| row.get(0),
        )
        .expect("window sample count")
}

fn sample_exists(path: &Path, observation: u64) -> bool {
    let connection = Connection::open(path).expect("inspect archive");
    connection
        .query_row(
            "SELECT count(*) = 1 FROM quota_sample WHERE observation_id = ?1",
            params![observation_id(observation).as_bytes().as_slice()],
            |row| row.get(0),
        )
        .expect("sample existence")
}

#[test]
fn ten_thousand_equivalent_polls_plateau_at_protected_first_and_latest() {
    let mut store = UsageStore::in_memory().expect("store");
    let definition = definition("weekly", 1);
    let mut last = None;
    for observation in 1..=10_000 {
        last = Some(
            store
                .apply_quota_observation(
                    &definition,
                    &sample("weekly", observation, "epoch-1", 100, 1_000),
                )
                .expect("redundant poll"),
        );
    }
    let last = last.expect("last result");
    assert_eq!(last.status(), QuotaApplyStatus::Advanced);
    assert_eq!(last.quota_revision().get(), 10_000);

    let result = store
        .maintain_quota_history_page(&window_key("weekly"), 1)
        .expect("maintenance");
    assert_eq!(result.examined_samples(), 0);
    assert_eq!(result.deleted_samples(), 0);
    assert_eq!(result.remaining_samples(), 2);
    assert_eq!(result.remaining_closed_epochs(), 0);
    assert_eq!(result.remaining_transitions(), 0);
}

#[test]
fn paged_maintenance_deletes_only_redundant_unprotected_backlog() {
    let mut store = UsageStore::in_memory().expect("store");
    let definition = definition("weekly", 1);
    for observation in 1_u64..=600 {
        let used = if observation.is_multiple_of(2) {
            200
        } else {
            100
        };
        store
            .apply_quota_observation(
                &definition,
                &sample("weekly", observation, "epoch-1", used, 1_000),
            )
            .expect("alternating sample");
    }

    let zero = store
        .maintain_quota_history_page(&window_key("weekly"), 0)
        .expect_err("zero page must fail");
    assert_eq!(zero.code(), StoreErrorCode::InvalidValue);
    let oversized = store
        .maintain_quota_history_page(&window_key("weekly"), MAX_QUOTA_MAINTENANCE_PAGE_SIZE + 1)
        .expect_err("oversized page must fail");
    assert_eq!(oversized.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        oversized.limit(),
        Some(u64::from(MAX_QUOTA_MAINTENANCE_PAGE_SIZE))
    );

    let maintained = store
        .maintain_quota_history_page(&window_key("weekly"), MAX_QUOTA_MAINTENANCE_PAGE_SIZE)
        .expect("maintenance");
    assert_eq!(maintained.examined_samples(), 88);
    assert_eq!(maintained.deleted_samples(), 88);
    assert_eq!(
        maintained.remaining_samples(),
        DEFAULT_QUOTA_SAMPLES_PER_WINDOW
    );
    assert_eq!(maintained.remaining_closed_epochs(), 0);
    assert_eq!(maintained.remaining_transitions(), 0);

    let stable = store
        .maintain_quota_history_page(&window_key("weekly"), 256)
        .expect("stable maintenance");
    assert_eq!(stable.examined_samples(), 0);
    assert_eq!(stable.deleted_samples(), 0);
    assert_eq!(stable.remaining_samples(), DEFAULT_QUOTA_SAMPLES_PER_WINDOW);
}

#[test]
fn maintenance_is_window_scoped_and_preserves_first_maximum_and_current() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-window-retention.sqlite3");
    let weekly = definition("weekly", 1);
    let daily = definition("daily", 1);
    {
        let mut store = UsageStore::open(&path).expect("store");
        for local in 1_u64..=600 {
            let used = if local.is_multiple_of(2) { 900 } else { 100 };
            store
                .apply_quota_observation(
                    &weekly,
                    &sample("weekly", local, "weekly-epoch", used, 1_000),
                )
                .expect("weekly sample");
            store
                .apply_quota_observation(
                    &daily,
                    &sample("daily", 1_000 + local, "daily-epoch", used, 1_000),
                )
                .expect("daily sample");
        }
        let maintained = store
            .maintain_quota_history_page(&window_key("weekly"), 256)
            .expect("weekly maintenance");
        assert_eq!(maintained.examined_samples(), 88);
        assert_eq!(maintained.deleted_samples(), 88);
        assert_eq!(maintained.remaining_samples(), 512);
    }

    assert_eq!(window_sample_count(&path, "weekly"), 512);
    assert_eq!(window_sample_count(&path, "daily"), 600);
    for protected in [1, 2, 600, 1_001, 1_002, 1_600] {
        assert!(sample_exists(&path, protected));
    }
}

#[test]
fn meaningful_samples_and_reset_evidence_may_exceed_soft_defaults_without_deletion() {
    let mut meaningful_store = UsageStore::in_memory().expect("meaningful store");
    let weekly = definition("weekly", 1);
    for observation in 1..=513 {
        meaningful_store
            .apply_quota_observation(
                &weekly,
                &sample("weekly", observation, "epoch-1", observation, 1_000),
            )
            .expect("meaningful sample");
    }
    let meaningful = meaningful_store
        .maintain_quota_history_page(&window_key("weekly"), 256)
        .expect("meaningful maintenance");
    assert_eq!(meaningful.examined_samples(), 0);
    assert_eq!(meaningful.deleted_samples(), 0);
    assert_eq!(meaningful.remaining_samples(), 513);
    assert_eq!(DEFAULT_QUOTA_SAMPLES_PER_WINDOW, 512);

    let mut reset_store = UsageStore::in_memory().expect("reset store");
    let reset_definition = definition("weekly", 1);
    for observation in 1..=258 {
        reset_store
            .apply_quota_observation(
                &reset_definition,
                &sample(
                    "weekly",
                    observation,
                    &format!("epoch-{observation}"),
                    10,
                    1_000,
                ),
            )
            .expect("reset sample");
    }
    let reset_history = reset_store
        .maintain_quota_history_page(&window_key("weekly"), 256)
        .expect("reset maintenance");
    assert_eq!(reset_history.examined_samples(), 0);
    assert_eq!(reset_history.deleted_samples(), 0);
    assert_eq!(reset_history.remaining_samples(), 258);
    assert_eq!(reset_history.remaining_closed_epochs(), 257);
    assert_eq!(reset_history.remaining_transitions(), 257);
    assert_eq!(DEFAULT_QUOTA_EPOCHS_PER_WINDOW, 256);
    assert_eq!(DEFAULT_QUOTA_TRANSITIONS_PER_WINDOW, 256);
}

#[test]
fn hard_sample_and_transition_caps_fail_the_applying_write_without_publication() {
    let directory = TempDir::new().expect("temporary directory");
    let sample_path = directory.path().join("quota-sample-hard-cap.sqlite3");
    let mut sample_store = UsageStore::open(&sample_path).expect("sample store");
    let sample_definition = definition("weekly", 1);
    for observation in 1..=MAX_QUOTA_SAMPLES_PER_WINDOW {
        sample_store
            .apply_quota_observation(
                &sample_definition,
                &sample("weekly", observation, "epoch-1", observation, 4_096),
            )
            .expect("sample below hard cap");
    }
    let sample_error = sample_store
        .apply_quota_observation(
            &sample_definition,
            &sample(
                "weekly",
                MAX_QUOTA_SAMPLES_PER_WINDOW + 1,
                "epoch-1",
                MAX_QUOTA_SAMPLES_PER_WINDOW + 1,
                4_096,
            ),
        )
        .expect_err("sample hard cap must fail");
    assert_eq!(sample_error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(sample_error.limit(), Some(MAX_QUOTA_SAMPLES_PER_WINDOW));
    let sample_state = sample_store
        .maintain_quota_history_page(&window_key("weekly"), 1)
        .expect("sample state");
    assert_eq!(
        sample_state.remaining_samples(),
        MAX_QUOTA_SAMPLES_PER_WINDOW
    );
    drop(sample_store);

    let tamper = Connection::open(&sample_path).expect("hard-cap tamper");
    let source_id = observation_id(MAX_QUOTA_SAMPLES_PER_WINDOW);
    let extra_id = observation_id(MAX_QUOTA_SAMPLES_PER_WINDOW + 2);
    tamper
        .execute(
            "INSERT INTO quota_sample(
               observation_id, scope_id, window_id, definition_revision,
               observed_at_ms, fresh_until_ms, stale_after_ms, provider_epoch_id,
               used_ratio_ppm, remaining_ratio_ppm, unit_id, used_units,
               remaining_units, capacity_units, advertised_resets_at_ms,
               quality, source, confidence, reset_evidence, reset_occurred_at_ms
             )
             SELECT ?1, scope_id, window_id, definition_revision,
                    9000001, 9000002, 9000003, provider_epoch_id,
                    used_ratio_ppm, remaining_ratio_ppm, unit_id, used_units,
                    remaining_units, capacity_units, advertised_resets_at_ms,
                    quality, source, confidence, reset_evidence, reset_occurred_at_ms
             FROM quota_sample WHERE observation_id = ?2",
            params![
                extra_id.as_bytes().as_slice(),
                source_id.as_bytes().as_slice()
            ],
        )
        .expect("insert over-cap sample");
    tamper
        .execute(
            "UPDATE quota_state
             SET retained_sample_count = retained_sample_count + 1
             WHERE singleton_id = 1",
            [],
        )
        .expect("publish tampered count");
    drop(tamper);
    let reopen_error =
        UsageStore::open(&sample_path).expect_err("over-cap archive must fail closed");
    assert_eq!(reopen_error.code(), StoreErrorCode::InvalidStoredValue);

    let mut transition_store = UsageStore::in_memory().expect("transition store");
    let transition_definition = definition("weekly", 1);
    for observation in 1..=MAX_QUOTA_TRANSITIONS_PER_WINDOW + 1 {
        transition_store
            .apply_quota_observation(
                &transition_definition,
                &sample(
                    "weekly",
                    observation,
                    &format!("epoch-{observation}"),
                    10,
                    4_096,
                ),
            )
            .expect("transition at or below hard cap");
    }
    let transition_error = transition_store
        .apply_quota_observation(
            &transition_definition,
            &sample(
                "weekly",
                MAX_QUOTA_TRANSITIONS_PER_WINDOW + 2,
                "epoch-overflow",
                10,
                4_096,
            ),
        )
        .expect_err("transition hard cap must fail");
    assert_eq!(transition_error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        transition_error.limit(),
        Some(MAX_QUOTA_TRANSITIONS_PER_WINDOW)
    );
    let transition_state = transition_store
        .maintain_quota_history_page(&window_key("weekly"), 1)
        .expect("transition state");
    assert_eq!(transition_state.examined_samples(), 0);
    assert_eq!(transition_state.deleted_samples(), 0);
    assert_eq!(
        transition_state.remaining_samples(),
        MAX_QUOTA_TRANSITIONS_PER_WINDOW + 1
    );
    assert_eq!(
        transition_state.remaining_closed_epochs(),
        MAX_QUOTA_EPOCHS_PER_WINDOW
    );
    assert_eq!(
        transition_state.remaining_transitions(),
        MAX_QUOTA_TRANSITIONS_PER_WINDOW
    );
}

#[test]
fn reopen_between_start_advance_allowance_reset_and_maintenance_is_exact() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-restart-retention.sqlite3");
    let definition = definition("weekly", 1);
    {
        let mut store = UsageStore::open(&path).expect("start store");
        store
            .apply_quota_observation(&definition, &sample("weekly", 1, "epoch-1", 100, 1_000))
            .expect("start");
    }
    {
        let mut store = UsageStore::open(&path).expect("advance store");
        store
            .apply_quota_observation(&definition, &sample("weekly", 2, "epoch-1", 200, 1_000))
            .expect("advance");
    }
    {
        let mut store = UsageStore::open(&path).expect("allowance store");
        let allowance = store
            .apply_quota_observation(&definition, &sample("weekly", 3, "epoch-1", 200, 2_000))
            .expect("allowance");
        assert_eq!(allowance.status(), QuotaApplyStatus::AllowanceChanged);
        assert_eq!(allowance.transition_sequence(), 1);
    }
    {
        let mut store = UsageStore::open(&path).expect("reset store");
        let reset = store
            .apply_quota_observation(&definition, &sample("weekly", 4, "epoch-2", 50, 2_000))
            .expect("reset");
        assert_eq!(reset.status(), QuotaApplyStatus::Reset);
        assert_eq!(reset.transition_sequence(), 2);
    }
    let mut reopened = UsageStore::open(&path).expect("maintenance reopen");
    let maintenance = reopened
        .maintain_quota_history_page(&window_key("weekly"), 256)
        .expect("maintenance");
    assert_eq!(maintenance.remaining_samples(), 4);
    assert_eq!(maintenance.remaining_closed_epochs(), 1);
    assert_eq!(maintenance.remaining_transitions(), 2);
    assert_eq!(quota_counts(&path), (4, 4, 1, 2, 1));
}

#[test]
fn revision_and_sequence_overflow_roll_back_the_complete_attempt() {
    let directory = TempDir::new().expect("temporary directory");
    let revision_path = directory.path().join("quota-revision-overflow.sqlite3");
    let definition = definition("weekly", 1);
    {
        let mut store = UsageStore::open(&revision_path).expect("revision store");
        store
            .apply_quota_observation(&definition, &sample("weekly", 1, "epoch-1", 100, 1_000))
            .expect("start");
    }
    let revision_connection = Connection::open(&revision_path).expect("revision tamper");
    revision_connection
        .execute(
            "UPDATE quota_state SET revision = ?1 WHERE singleton_id = 1",
            params![i64::MAX],
        )
        .expect("set revision max");
    drop(revision_connection);
    let mut revision_store = UsageStore::open(&revision_path).expect("revision reopen");
    let revision_error = revision_store
        .apply_quota_observation(&definition, &sample("weekly", 2, "epoch-1", 200, 1_000))
        .expect_err("revision overflow");
    assert_eq!(revision_error.code(), StoreErrorCode::CapacityExceeded);
    drop(revision_store);
    assert_eq!(quota_counts(&revision_path), (i64::MAX, 1, 0, 0, 1));

    let sequence_path = directory.path().join("quota-sequence-overflow.sqlite3");
    {
        let mut store = UsageStore::open(&sequence_path).expect("sequence store");
        store
            .apply_quota_observation(&definition, &sample("weekly", 10, "epoch-1", 100, 1_000))
            .expect("start");
    }
    let sequence_connection = Connection::open(&sequence_path).expect("sequence tamper");
    sequence_connection
        .execute(
            "UPDATE quota_epoch_current SET last_transition_sequence = ?1",
            params![i64::MAX],
        )
        .expect("epoch sequence max");
    sequence_connection
        .execute(
            "UPDATE quota_window_current SET last_transition_sequence = ?1",
            params![i64::MAX],
        )
        .expect("window sequence max");
    drop(sequence_connection);
    let mut sequence_store = UsageStore::open(&sequence_path).expect("sequence reopen");
    let sequence_error = sequence_store
        .apply_quota_observation(&definition, &sample("weekly", 11, "epoch-2", 10, 1_000))
        .expect_err("sequence overflow");
    assert_eq!(sequence_error.code(), StoreErrorCode::CapacityExceeded);
    drop(sequence_store);
    assert_eq!(quota_counts(&sequence_path), (1, 1, 0, 0, 1));
}
