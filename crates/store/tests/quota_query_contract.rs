use std::{path::Path, time::Duration};

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample,
    QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaUnitId, QuotaUnits,
    QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey,
    QuotaWindowSemantics, UsageProviderId,
};
use tokenmaster_quota::{QuotaTransitionKind, quota_scope_id};
use tokenmaster_store::{
    MAX_QUOTA_CURRENT_WINDOWS, MAX_QUOTA_TRANSITION_PAGE_SIZE, QuotaCurrentQuery,
    QuotaOverviewQuery, QuotaTransitionPageQuery, StoreErrorCode, UsageReadStore, UsageStore,
};

fn window_key(account_id: &str, window_id: &str) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new(account_id).expect("account"),
            None,
        ),
        QuotaWindowId::new(window_id).expect("window"),
    )
}

fn definition(account_id: &str, window_id: &str, revision: u64) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key(account_id, window_id),
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
    key: &QuotaWindowKey,
    observation: u64,
    provider_epoch: &str,
    used_units: u64,
    capacity_units: u64,
) -> QuotaSample {
    let used_ratio =
        u32::try_from(used_units.checked_mul(1_000_000).expect("ratio multiply") / capacity_units)
            .expect("ratio");
    let observed_at_ms = i64::try_from(observation.checked_mul(10).expect("time")).expect("time");
    QuotaSample::new(QuotaSampleParts {
        key: key.clone(),
        observation_id: observation_id(observation),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 100,
        stale_after_ms: observed_at_ms + 200,
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

fn checkpoint(path: &Path) {
    let connection = Connection::open(path).expect("checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

fn quota_counts(path: &Path) -> (i64, i64, i64, i64) {
    let connection = Connection::open(path).expect("inspect archive");
    connection
        .query_row(
            "SELECT revision,
                    (SELECT count(*) FROM quota_sample),
                    (SELECT count(*) FROM quota_epoch_history),
                    (SELECT count(*) FROM quota_transition)
             FROM quota_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("quota counts")
}

#[test]
fn current_capture_is_owned_revision_exact_and_missing_safe_across_scopes() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-current-query.sqlite3");
    let weekly = definition("personal", "weekly", 1);
    let daily = definition("team", "daily", 1);
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        writer
            .apply_quota_observation(
                &weekly,
                &sample(weekly.key(), 1, "weekly-epoch", 100, 1_000),
            )
            .expect("weekly");
        writer
            .apply_quota_observation(&daily, &sample(daily.key(), 2, "daily-epoch", 200, 1_000))
            .expect("daily");
    }
    checkpoint(&path);

    let missing = window_key("personal", "monthly");
    let owned_capture = {
        let mut reader = UsageReadStore::open(&path).expect("reader");
        let empty = reader
            .capture_quota_windows(
                QuotaCurrentQuery::new(Box::default(), Duration::from_secs(2))
                    .expect("empty query"),
            )
            .expect("empty capture");
        assert_eq!(empty.quota_revision().get(), 2);
        assert!(empty.windows().is_empty());

        reader
            .capture_quota_windows(
                QuotaCurrentQuery::new(
                    vec![missing, daily.key().clone(), weekly.key().clone()].into_boxed_slice(),
                    Duration::from_secs(2),
                )
                .expect("current query"),
            )
            .expect("current capture")
    };

    assert_eq!(owned_capture.quota_revision().get(), 2);
    assert_eq!(owned_capture.windows().len(), 2);
    assert!(owned_capture.windows().iter().any(|window| {
        window.definition() == &weekly
            && window
                .sample()
                .used_ratio()
                .map(QuotaRatio::parts_per_million)
                == Some(100_000)
            && window.epoch().first_sample() == window.sample()
            && window.epoch().last_transition_sequence() == 0
            && window.last_transition().is_none()
    }));
    assert!(owned_capture.windows().iter().any(|window| {
        window.definition() == &daily
            && window
                .sample()
                .used_ratio()
                .map(QuotaRatio::parts_per_million)
                == Some(200_000)
    }));
    assert_eq!(quota_counts(&path), (2, 2, 0, 0));
}

#[test]
fn overview_discovers_every_current_window_in_opaque_order_without_changing_exact_empty() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-overview-query.sqlite3");
    let definitions = [
        definition("team", "weekly", 1),
        definition("personal", "daily", 1),
        definition("personal", "weekly", 1),
    ];
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for (index, definition) in definitions.iter().enumerate() {
            writer
                .apply_quota_observation(
                    definition,
                    &sample(
                        definition.key(),
                        u64::try_from(index + 1).expect("observation"),
                        &format!("epoch-{index}"),
                        100,
                        1_000,
                    ),
                )
                .expect("observation");
        }
    }
    checkpoint(&path);

    let mut reader = UsageReadStore::open(&path).expect("reader");
    let exact_empty = reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(Box::default(), Duration::from_secs(2)).expect("exact empty"),
        )
        .expect("exact empty capture");
    assert!(exact_empty.windows().is_empty());

    let overview = reader
        .capture_quota_overview(
            QuotaOverviewQuery::new(Duration::from_secs(2)).expect("overview query"),
        )
        .expect("overview capture");
    assert_eq!(overview.quota_revision().get(), 3);
    assert_eq!(overview.windows().len(), definitions.len());

    let mut expected = definitions
        .iter()
        .map(|definition| definition.key().clone())
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| {
        quota_scope_id(left.scope())
            .as_bytes()
            .cmp(quota_scope_id(right.scope()).as_bytes())
            .then_with(|| left.window_id().as_str().cmp(right.window_id().as_str()))
    });
    assert_eq!(
        overview
            .windows()
            .iter()
            .map(|window| window.definition().key())
            .collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );
    let debug = format!("{overview:?}");
    assert!(!debug.contains("personal"));
    assert!(!debug.contains("team"));
    assert!(!debug.contains("weekly"));
}

#[test]
fn overview_fails_closed_when_current_window_count_exceeds_the_bound() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-overview-capacity.sqlite3");
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for index in 0..=MAX_QUOTA_CURRENT_WINDOWS {
            let definition = definition("personal", &format!("window-{index:02}"), 1);
            writer
                .apply_quota_observation(
                    &definition,
                    &sample(
                        definition.key(),
                        u64::try_from(index + 1).expect("observation"),
                        &format!("epoch-{index}"),
                        100,
                        1_000,
                    ),
                )
                .expect("observation");
        }
    }
    checkpoint(&path);

    let mut reader = UsageReadStore::open(&path).expect("reader");
    let error = reader
        .capture_quota_overview(
            QuotaOverviewQuery::new(Duration::from_secs(2)).expect("overview query"),
        )
        .expect_err("33rd current window must fail closed");
    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_QUOTA_CURRENT_WINDOWS as u64));
}

#[test]
fn transition_history_is_revision_bound_descending_and_uses_256_plus_one() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-transition-query.sqlite3");
    let definition = definition("personal", "weekly", 1);
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for observation in 1_u64..=258 {
            writer
                .apply_quota_observation(
                    &definition,
                    &sample(
                        definition.key(),
                        observation,
                        &format!("epoch-{observation}"),
                        10,
                        1_000,
                    ),
                )
                .expect("reset sample");
        }
    }
    checkpoint(&path);

    let mut reader = UsageReadStore::open(&path).expect("reader");
    let current = reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(
                vec![definition.key().clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("current query"),
        )
        .expect("current");
    let revision = current.quota_revision();
    let current_window = &current.windows()[0];
    assert_eq!(current_window.epoch().last_transition_sequence(), 257);
    assert_eq!(
        current_window
            .last_transition()
            .map(|transition| transition.sequence()),
        Some(257)
    );

    let first = reader
        .capture_quota_transitions(
            QuotaTransitionPageQuery::new(
                definition.key().clone(),
                Some(revision),
                None,
                MAX_QUOTA_TRANSITION_PAGE_SIZE,
                Duration::from_secs(2),
            )
            .expect("first query"),
        )
        .expect("first page");
    assert_eq!(first.quota_revision(), revision);
    assert_eq!(first.transitions().len(), 256);
    assert!(first.has_more());
    assert_eq!(first.transitions()[0].sequence(), 257);
    assert_eq!(first.transitions()[255].sequence(), 2);
    assert!(
        first
            .transitions()
            .windows(2)
            .all(|pair| pair[0].sequence() > pair[1].sequence())
    );
    let cursor = first.next_cursor().cloned().expect("next cursor");
    let cursor_debug = format!("{cursor:?}");
    assert!(cursor_debug.contains("[redacted]"));
    assert!(!cursor_debug.contains("epoch-"));

    let second = reader
        .capture_quota_transitions(
            QuotaTransitionPageQuery::new(
                definition.key().clone(),
                Some(revision),
                Some(cursor.clone()),
                MAX_QUOTA_TRANSITION_PAGE_SIZE,
                Duration::from_secs(2),
            )
            .expect("second query"),
        )
        .expect("second page");
    assert_eq!(second.transitions().len(), 1);
    assert_eq!(second.transitions()[0].sequence(), 1);
    assert!(!second.has_more());
    assert!(second.next_cursor().is_none());

    let other_window = window_key("personal", "daily");
    let changed_filter = QuotaTransitionPageQuery::new(
        other_window,
        Some(revision),
        Some(cursor.clone()),
        16,
        Duration::from_secs(2),
    )
    .expect_err("changed filter must fail");
    assert_eq!(changed_filter.code(), StoreErrorCode::InvalidValue);

    {
        let mut writer = UsageStore::open(&path).expect("writer reopen");
        writer
            .apply_quota_observation(
                &definition,
                &sample(definition.key(), 259, "epoch-259", 10, 1_000),
            )
            .expect("new reset");
    }
    checkpoint(&path);
    let mut changed_reader = UsageReadStore::open(&path).expect("changed reader");
    let stale = changed_reader
        .capture_quota_transitions(
            QuotaTransitionPageQuery::new(
                definition.key().clone(),
                Some(revision),
                None,
                16,
                Duration::from_secs(2),
            )
            .expect("stale query"),
        )
        .expect_err("stale revision must fail");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);

    let new_revision = changed_reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(
                vec![definition.key().clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("new current query"),
        )
        .expect("new current")
        .quota_revision();
    let changed_revision = QuotaTransitionPageQuery::new(
        definition.key().clone(),
        Some(new_revision),
        Some(cursor),
        16,
        Duration::from_secs(2),
    )
    .expect_err("cursor revision mismatch must fail");
    assert_eq!(changed_revision.code(), StoreErrorCode::InvalidValue);
}

#[test]
fn current_and_transition_values_restore_allowance_reset_and_boundary_samples() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-transition-values.sqlite3");
    let definition = definition("personal", "weekly", 1);
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for sample in [
            sample(definition.key(), 1, "epoch-1", 100, 1_000),
            sample(definition.key(), 2, "epoch-1", 200, 1_000),
            sample(definition.key(), 3, "epoch-1", 200, 2_000),
            sample(definition.key(), 4, "epoch-2", 50, 2_000),
        ] {
            writer
                .apply_quota_observation(&definition, &sample)
                .expect("observation");
        }
    }
    checkpoint(&path);

    let mut reader = UsageReadStore::open(&path).expect("reader");
    let current = reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(
                vec![definition.key().clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("current query"),
        )
        .expect("current capture");
    let window = &current.windows()[0];
    assert_eq!(window.sample().observation_id(), observation_id(4));
    assert_eq!(
        window.epoch().first_sample().observation_id(),
        observation_id(4)
    );
    assert_eq!(
        window.epoch().maximum_used_ratio(),
        Some(QuotaRatio::new(25_000).expect("ratio"))
    );
    assert_eq!(window.epoch().last_transition_sequence(), 2);
    let last = window.last_transition().expect("last transition");
    assert_eq!(last.sequence(), 2);
    assert_eq!(last.kind(), QuotaTransitionKind::EarlyReset);
    assert_eq!(last.pre_sample().observation_id(), observation_id(3));
    assert_eq!(last.post_sample().observation_id(), observation_id(4));
    assert_eq!(
        last.maximum_used_ratio_before()
            .map(QuotaRatio::parts_per_million),
        Some(200_000)
    );
    assert!(last.allowance_change().is_none());

    let page = reader
        .capture_quota_transitions(
            QuotaTransitionPageQuery::new(
                definition.key().clone(),
                Some(current.quota_revision()),
                None,
                16,
                Duration::from_secs(2),
            )
            .expect("transition query"),
        )
        .expect("transitions");
    assert_eq!(page.transitions().len(), 2);
    assert_eq!(
        page.transitions()[0].kind(),
        QuotaTransitionKind::EarlyReset
    );
    let allowance = &page.transitions()[1];
    assert_eq!(allowance.kind(), QuotaTransitionKind::AllowanceChanged);
    let change = allowance.allowance_change().expect("allowance change");
    assert_eq!(change.old_units().capacity(), Some(1_000));
    assert_eq!(change.new_units().capacity(), Some(2_000));
}

#[test]
fn query_bounds_debug_and_read_only_capture_are_fail_closed() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota-query-bounds.sqlite3");
    let definition = definition("personal", "weekly", 1);
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        writer
            .apply_quota_observation(
                &definition,
                &sample(definition.key(), 1, "epoch-1", 100, 1_000),
            )
            .expect("sample");
    }
    checkpoint(&path);
    let before = quota_counts(&path);

    let too_many = (0..=MAX_QUOTA_CURRENT_WINDOWS)
        .map(|index| window_key("personal", &format!("window-{index}")))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let current_limit =
        QuotaCurrentQuery::new(too_many, Duration::from_secs(2)).expect_err("current window cap");
    assert_eq!(current_limit.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        current_limit.limit(),
        Some(MAX_QUOTA_CURRENT_WINDOWS as u64)
    );
    let duplicate = QuotaCurrentQuery::new(
        vec![definition.key().clone(), definition.key().clone()].into_boxed_slice(),
        Duration::from_secs(2),
    )
    .expect_err("duplicate filter");
    assert_eq!(duplicate.code(), StoreErrorCode::InvalidValue);
    let zero_page = QuotaTransitionPageQuery::new(
        definition.key().clone(),
        None,
        None,
        0,
        Duration::from_secs(2),
    )
    .expect_err("zero page");
    assert_eq!(zero_page.code(), StoreErrorCode::InvalidValue);
    let page_limit = QuotaTransitionPageQuery::new(
        definition.key().clone(),
        None,
        None,
        MAX_QUOTA_TRANSITION_PAGE_SIZE + 1,
        Duration::from_secs(2),
    )
    .expect_err("page cap");
    assert_eq!(page_limit.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        page_limit.limit(),
        Some(MAX_QUOTA_TRANSITION_PAGE_SIZE as u64)
    );
    for invalid_deadline in [Duration::ZERO, Duration::from_secs(3)] {
        let error = QuotaCurrentQuery::new(
            vec![definition.key().clone()].into_boxed_slice(),
            invalid_deadline,
        )
        .expect_err("invalid current deadline");
        assert_eq!(error.code(), StoreErrorCode::InvalidValue);
        let overview =
            QuotaOverviewQuery::new(invalid_deadline).expect_err("invalid overview deadline");
        assert_eq!(overview.code(), StoreErrorCode::InvalidValue);
    }

    let mut reader = UsageReadStore::open(&path).expect("reader");
    assert!(reader.runtime_policy().expect("policy").query_only());
    let capture = reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(
                vec![definition.key().clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("query"),
        )
        .expect("capture");
    let debug = format!("{capture:?}");
    assert!(debug.contains("QuotaObservationId([redacted])"));
    assert!(!debug.contains(&format!("{:?}", observation_id(1).as_bytes())));
    assert!(!debug.contains("personal"));
    assert!(!debug.contains("weekly"));
    drop(reader);
    assert_eq!(quota_counts(&path), before);

    let scope_id = quota_scope_id(definition.key().scope());
    assert!(!debug.contains(&format!("{:?}", scope_id.as_bytes())));

    let mut corrupted_reader = UsageReadStore::open(&path).expect("long-lived reader");
    let tamper = Connection::open(&path).expect("tamper connection");
    tamper
        .execute(
            "UPDATE quota_epoch_current SET last_transition_sequence = 1",
            [],
        )
        .expect("tamper epoch sequence");
    tamper
        .execute(
            "UPDATE quota_window_current SET last_transition_sequence = 1",
            [],
        )
        .expect("tamper window sequence");
    drop(tamper);
    checkpoint(&path);
    let corrupted = corrupted_reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(
                vec![definition.key().clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("corrupted query"),
        )
        .expect_err("missing transition must fail closed");
    assert_eq!(corrupted.code(), StoreErrorCode::InvalidStoredValue);
}

#[test]
fn current_capture_rejects_epoch_projection_drift_after_reader_open() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("quota-current-projection-drift.sqlite3");
    let definition = definition("personal", "weekly", 1);
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        writer
            .apply_quota_observation(
                &definition,
                &sample(definition.key(), 1, "epoch-1", 100, 1_000),
            )
            .expect("sample");
    }
    checkpoint(&path);

    let mut reader = UsageReadStore::open(&path).expect("long-lived reader");
    let tamper = Connection::open(&path).expect("tamper connection");
    tamper
        .execute(
            "UPDATE quota_epoch_current SET provider_epoch_id = 'forged-epoch'",
            [],
        )
        .expect("tamper epoch projection");
    drop(tamper);
    checkpoint(&path);

    let corrupted = reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(
                vec![definition.key().clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("query"),
        )
        .expect_err("epoch projection drift must fail closed");
    assert_eq!(corrupted.code(), StoreErrorCode::InvalidStoredValue);
}

#[test]
fn transition_capture_rejects_boundary_projection_drift_after_reader_open() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("quota-transition-projection-drift.sqlite3");
    let definition = definition("personal", "weekly", 1);
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for sample in [
            sample(definition.key(), 1, "epoch-1", 100, 1_000),
            sample(definition.key(), 2, "epoch-1", 200, 2_000),
        ] {
            writer
                .apply_quota_observation(&definition, &sample)
                .expect("observation");
        }
    }
    checkpoint(&path);

    let mut reader = UsageReadStore::open(&path).expect("long-lived reader");
    let revision = reader
        .capture_quota_windows(
            QuotaCurrentQuery::new(
                vec![definition.key().clone()].into_boxed_slice(),
                Duration::from_secs(2),
            )
            .expect("current query"),
        )
        .expect("current capture")
        .quota_revision();
    let tamper = Connection::open(&path).expect("tamper connection");
    tamper
        .execute_batch("DROP TRIGGER quota_transition_no_update;")
        .expect("disable immutable transition guard");
    tamper
        .execute(
            "UPDATE quota_transition
             SET old_resets_at_ms = old_resets_at_ms - 1,
                 allowance_old_used_units = allowance_old_used_units + 1",
            [],
        )
        .expect("tamper transition projection");
    drop(tamper);
    checkpoint(&path);

    let corrupted = reader
        .capture_quota_transitions(
            QuotaTransitionPageQuery::new(
                definition.key().clone(),
                Some(revision),
                None,
                16,
                Duration::from_secs(2),
            )
            .expect("transition query"),
        )
        .expect_err("transition projection drift must fail closed");
    assert_eq!(corrupted.code(), StoreErrorCode::InvalidStoredValue);
}
