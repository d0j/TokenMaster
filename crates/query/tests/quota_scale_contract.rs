use std::{
    path::Path,
    time::{Duration, Instant},
};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence, QuotaSample,
    QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaWindowDefinition,
    QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey, QuotaWindowSemantics,
    UsageProviderId,
};
use tokenmaster_query::{
    LatestActivityRequest, PageSize, QueryClock, QueryError, QueryService, QueryTimeSample,
    QuotaCurrentRequest, QuotaTransitionKind, QuotaTransitionPageRequest,
};
use tokenmaster_store::{MAX_QUOTA_MAINTENANCE_PAGE_SIZE, QuotaApplyStatus, UsageStore};

const WINDOW_COUNT: usize = 32;
const TRANSITION_COUNT: u64 = 1_000;
const REDUNDANT_POLL_COUNT: usize = 10_000;
const OPERATION_BUDGET: Duration = Duration::from_secs(1);
const SOURCE_KEY: [u8; 32] = [7; 32];

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(2_000_000, 1))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResetFlavor {
    Scheduled,
    Early,
    Manual,
}

fn checkpoint(path: &Path) {
    Connection::open(path)
        .expect("checkpoint connection")
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

fn window_key(index: usize) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("scale-account").expect("account"),
            None,
        ),
        QuotaWindowId::new(format!("window-{index:02}")).expect("window"),
    )
}

fn definition(index: usize) -> QuotaWindowDefinition {
    QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key(index),
        revision: 1,
        label_key: format!("quota.window-{index:02}"),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: None,
        reset_thresholds: None,
    })
    .expect("definition")
}

fn observation_id(window: usize, observation: u64) -> QuotaObservationId {
    let value = (u64::try_from(window).expect("window") << 32) | observation;
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    QuotaObservationId::from_bytes(bytes)
}

fn next_flavor(sequence: u64) -> ResetFlavor {
    match sequence % 3 {
        1 => ResetFlavor::Scheduled,
        2 => ResetFlavor::Early,
        _ => ResetFlavor::Manual,
    }
}

fn advertised_for_next(observed_at_ms: i64, flavor: ResetFlavor) -> i64 {
    match flavor {
        ResetFlavor::Scheduled => observed_at_ms + 1_000,
        ResetFlavor::Early | ResetFlavor::Manual => observed_at_ms + 100_000,
    }
}

fn sample(
    index: usize,
    observation: u64,
    observed_at_ms: i64,
    advertised_resets_at_ms: i64,
    flavor: Option<ResetFlavor>,
) -> QuotaSample {
    let used_ppm = if observation.is_multiple_of(2) {
        10_000
    } else {
        900_000
    };
    QuotaSample::new(QuotaSampleParts {
        key: window_key(index),
        observation_id: observation_id(index, observation),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 10_000,
        stale_after_ms: observed_at_ms + 20_000,
        provider_epoch_id: Some(
            QuotaProviderEpochId::new(format!("epoch-{index:02}-{observation:04}"))
                .expect("provider epoch"),
        ),
        used_ratio: Some(QuotaRatio::new(used_ppm).expect("used ratio")),
        remaining_ratio: Some(QuotaRatio::new(1_000_000 - used_ppm).expect("remaining ratio")),
        units: None,
        advertised_resets_at_ms: Some(advertised_resets_at_ms),
        quality: QuotaSampleQuality::Authoritative,
        source: flavor.map_or(QuotaEvidenceSource::ProviderOfficial, |value| match value {
            ResetFlavor::Manual => QuotaEvidenceSource::Manual,
            ResetFlavor::Scheduled | ResetFlavor::Early => QuotaEvidenceSource::ProviderOfficial,
        }),
        confidence: QuotaConfidence::High,
        reset_evidence: if flavor == Some(ResetFlavor::Manual) {
            QuotaResetEvidence::ManualOrBanked
        } else {
            QuotaResetEvidence::None
        },
        reset_occurred_at_ms: (flavor == Some(ResetFlavor::Manual)).then_some(observed_at_ms - 1),
    })
    .expect("sample")
}

fn seed_current_usage(path: &Path) {
    drop(UsageStore::open(path).expect("create current archive"));
    let mut connection = Connection::open(path).expect("current fixture connection");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("current transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'quota-scale-private-source', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("current source");
    transaction
        .execute_batch(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completed_at_ms, completion_state,
               expected_scope_count
             ) VALUES (1, 1, 1000, 'complete', 1);
             INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1, 1000, 'complete');
             INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 1, 1, 1, 1, 1);
             UPDATE usage_archive_state
             SET archive_generation = 1, current_revision_id = 0,
                 latest_complete_scan_set_id = 1, incremental_state = 'complete'
             WHERE singleton_id = 1;",
        )
        .expect("current publication");
    transaction
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, project_alias, activity_read,
               activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents,
               activity_terminal
             ) VALUES (
               ?1, 'quota-scale-current-event', ?2, 0, 1, 0, 0, 0,
               'codex', 'default', 'private-session', 'quota-scale-private-source',
               1, 0, 'gpt-scale', 1, NULL, 1, NULL, 2, 0, 'no', 'tokenmaster',
               1, 0, 0, 0, 0, 0, 0, 0
             )",
            params![[9_u8; 32].as_slice(), SOURCE_KEY.as_slice()],
        )
        .expect("current event");
    transaction.commit().expect("current commit");
    drop(connection);
    checkpoint(path);
}

fn seed_legacy_usage(path: &Path) {
    let mut connection = Connection::open(path).expect("create v1 fixture");
    connection
        .execute_batch(include_str!("../../store/tests/fixtures/usage_v1.sql"))
        .expect("exact v1 schema");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("legacy transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity
             ) VALUES (?1, 'codex', 'legacy', 'quota-scale-legacy-source', 'archived', ?2, ?3)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("legacy source");
    transaction
        .execute(
            "INSERT INTO usage_generation(
               file_key, generation, status, parser_schema_version, physical_identity,
               logical_identity, committed_offset, scan_offset, observed_file_length,
               modified_time_ns, anchor_start, anchor_len, anchor_sha256, resume_payload,
               discarding_oversized_line, incomplete_tail, verification_level
             ) VALUES (?1, 0, 'current', 1, ?2, ?3, 0, 0, 0, NULL, 0, 0, ?4, X'',
                       0, 0, 'full_prefix')",
            params![
                SOURCE_KEY.as_slice(),
                [3_u8; 32].as_slice(),
                [2_u8; 32].as_slice(),
                [4_u8; 32].as_slice()
            ],
        )
        .expect("legacy generation");
    transaction
        .execute(
            "INSERT INTO usage_observation(
               file_key, generation, source_offset, fingerprint, event_id, profile_id,
               session_id, source_id, timestamp_seconds, timestamp_nanos, model,
               input_tokens, cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             ) VALUES (?1, 0, 0, ?2, 'quota-scale-legacy-event', 'legacy', 'private-session',
                       'quota-scale-legacy-source', 1, 0, 'gpt-scale', 1, NULL, 1, NULL, 2,
                       0, 'no', 0, 0, 0, 0, 0, 0, 0, 0)",
            params![SOURCE_KEY.as_slice(), [10_u8; 32].as_slice()],
        )
        .expect("legacy observation");
    transaction
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens, cached_tokens,
               output_tokens, reasoning_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) SELECT fingerprint, event_id, file_key, generation, source_offset,
                      profile_id, session_id, source_id, timestamp_seconds, timestamp_nanos,
                      model, input_tokens, cached_tokens, output_tokens, reasoning_tokens,
                      total_tokens, fallback_model, long_context, activity_read,
                      activity_edit_write, activity_search, activity_git,
                      activity_build_test, activity_web, activity_subagents,
                      activity_terminal
               FROM usage_observation",
            [],
        )
        .expect("legacy event");
    transaction.commit().expect("legacy commit");
    drop(connection);
    drop(UsageStore::open(path).expect("migrate legacy archive"));
    checkpoint(path);
}

fn verify_legacy_quota_coexistence(path: &Path) {
    seed_legacy_usage(path);
    let definition = definition(31);
    let first = sample(
        31,
        1,
        1_000,
        advertised_for_next(1_000, ResetFlavor::Scheduled),
        None,
    );
    UsageStore::open(path)
        .expect("legacy quota writer")
        .apply_quota_observation(&definition, &first)
        .expect("legacy quota observation");
    checkpoint(path);
    let mut service = QueryService::open(path, FixedClock).expect("legacy query service");
    let activity = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(1).expect("activity page"),
        ))
        .expect("legacy activity");
    assert_eq!(activity.payload().items().len(), 1);
    let quota = service
        .quota_windows(
            QuotaCurrentRequest::new(vec![definition.key().clone()]).expect("quota request"),
        )
        .expect("legacy quota");
    assert!(quota.payload().windows()[0].snapshot().is_some());
}

#[test]
#[ignore = "reference-machine quota release gate; run explicitly in release mode"]
fn quota_core_scale_restart_paging_and_usage_coexistence_meet_budgets() {
    let directory = TempDir::new().expect("temporary directory");
    let current_path = directory.path().join("quota-scale-current.sqlite3");
    seed_current_usage(&current_path);
    let definitions = (0..WINDOW_COUNT).map(definition).collect::<Vec<_>>();
    let mut writer = UsageStore::open(&current_path).expect("quota writer");
    let mut maximum_write = Duration::ZERO;

    for (index, definition) in definitions.iter().enumerate() {
        let started = Instant::now();
        let result = writer
            .apply_quota_observation(
                definition,
                &sample(
                    index,
                    1,
                    1_000,
                    advertised_for_next(1_000, next_flavor(1)),
                    None,
                ),
            )
            .expect("initial quota observation");
        maximum_write = maximum_write.max(started.elapsed());
        assert_eq!(result.status(), QuotaApplyStatus::Started);
    }

    let mut last_sample = None;
    let mut reset_counts = [0_u64; 3];
    for sequence in 1..=TRANSITION_COUNT {
        if sequence == 501 {
            drop(writer);
            writer = UsageStore::open(&current_path).expect("restart quota writer");
        }
        let flavor = next_flavor(sequence);
        match flavor {
            ResetFlavor::Scheduled => reset_counts[0] += 1,
            ResetFlavor::Early => reset_counts[1] += 1,
            ResetFlavor::Manual => reset_counts[2] += 1,
        }
        let observation = sequence + 1;
        let observed_at_ms = i64::try_from(observation * 1_000).expect("observation time");
        let current = sample(
            0,
            observation,
            observed_at_ms,
            advertised_for_next(observed_at_ms, next_flavor(sequence + 1)),
            Some(flavor),
        );
        let started = Instant::now();
        let result = writer
            .apply_quota_observation(&definitions[0], &current)
            .expect("reset observation");
        maximum_write = maximum_write.max(started.elapsed());
        assert_eq!(result.status(), QuotaApplyStatus::Reset);
        assert_eq!(result.transition_sequence(), sequence);
        last_sample = Some(current);
    }
    assert!(reset_counts.iter().all(|count| *count > 0));

    let last_sample = last_sample.expect("last sample");
    let mut maximum_redundant_poll = Duration::ZERO;
    for _ in 0..REDUNDANT_POLL_COUNT {
        let started = Instant::now();
        let result = writer
            .apply_quota_observation(&definitions[0], &last_sample)
            .expect("redundant poll");
        maximum_redundant_poll = maximum_redundant_poll.max(started.elapsed());
        assert_eq!(result.status(), QuotaApplyStatus::Duplicate);
    }
    let maintenance = writer
        .maintain_quota_history_page(definitions[0].key(), MAX_QUOTA_MAINTENANCE_PAGE_SIZE)
        .expect("bounded maintenance");
    assert_eq!(maintenance.examined_samples(), 0);
    assert_eq!(maintenance.deleted_samples(), 0);
    assert_eq!(maintenance.remaining_closed_epochs(), TRANSITION_COUNT);
    assert_eq!(maintenance.remaining_transitions(), TRANSITION_COUNT);
    drop(writer);
    checkpoint(&current_path);

    assert!(
        maximum_write < OPERATION_BUDGET,
        "quota write exceeded budget: {maximum_write:?}"
    );
    assert!(
        maximum_redundant_poll < OPERATION_BUDGET,
        "redundant quota poll exceeded budget: {maximum_redundant_poll:?}"
    );

    let mut service = QueryService::open(&current_path, FixedClock).expect("query service");
    let current_started = Instant::now();
    let current = service
        .quota_windows(
            QuotaCurrentRequest::new(
                definitions
                    .iter()
                    .map(|definition| definition.key().clone())
                    .collect(),
            )
            .expect("current request"),
        )
        .expect("current quota");
    let current_elapsed = current_started.elapsed();
    assert_eq!(current.payload().windows().len(), WINDOW_COUNT);
    assert!(
        current
            .payload()
            .windows()
            .iter()
            .all(|window| window.snapshot().is_some())
    );
    assert!(
        current_elapsed < OPERATION_BUDGET,
        "32-window current read exceeded budget: {current_elapsed:?}"
    );
    let overview_started = Instant::now();
    let overview = service.quota_overview().expect("quota overview");
    let overview_elapsed = overview_started.elapsed();
    assert_eq!(overview.payload().windows().len(), WINDOW_COUNT);
    assert_eq!(overview.header().filters().len(), WINDOW_COUNT);
    assert!(
        overview_elapsed < OPERATION_BUDGET,
        "32-window overview read exceeded budget: {overview_elapsed:?}"
    );

    let mut before = None;
    let mut expected_sequence = TRANSITION_COUNT;
    let mut transition_count = 0_u64;
    let mut maximum_history_read = Duration::ZERO;
    let mut seen_kinds = [false; 3];
    loop {
        let request = match before.take() {
            Some(cursor) => QuotaTransitionPageRequest::continuation(
                definitions[0].key().clone(),
                PageSize::new(256).expect("page size"),
                cursor,
            )
            .expect("continuation request"),
            None => QuotaTransitionPageRequest::first(
                definitions[0].key().clone(),
                PageSize::new(256).expect("page size"),
            )
            .expect("first request"),
        };
        let started = Instant::now();
        let page = service.quota_transitions(request).expect("transition page");
        maximum_history_read = maximum_history_read.max(started.elapsed());
        for transition in page.payload().transitions().iter() {
            assert_eq!(transition.sequence(), expected_sequence);
            expected_sequence -= 1;
            transition_count += 1;
            match transition.kind() {
                QuotaTransitionKind::ScheduledReset => seen_kinds[0] = true,
                QuotaTransitionKind::EarlyReset => seen_kinds[1] = true,
                QuotaTransitionKind::ManualOrBankedReset => seen_kinds[2] = true,
                QuotaTransitionKind::UnknownReset | QuotaTransitionKind::AllowanceChanged => {}
            }
        }
        if !page.payload().has_more() {
            break;
        }
        before = page.payload().next_cursor().cloned();
        assert!(before.is_some());
    }
    assert_eq!(transition_count, TRANSITION_COUNT);
    assert_eq!(expected_sequence, 0);
    assert!(seen_kinds.into_iter().all(|seen| seen));
    assert!(
        maximum_history_read < OPERATION_BUDGET,
        "256-row history read exceeded budget: {maximum_history_read:?}"
    );

    let activity = service
        .latest_activity(LatestActivityRequest::first(
            PageSize::new(1).expect("activity page"),
        ))
        .expect("current activity");
    assert_eq!(activity.payload().items().len(), 1);
    drop(service);
    let mut reopened = QueryService::open(&current_path, FixedClock).expect("reopened query");
    assert_eq!(
        reopened
            .quota_windows(
                QuotaCurrentRequest::new(vec![definitions[0].key().clone()])
                    .expect("reopen request"),
            )
            .expect("reopen quota")
            .payload()
            .windows()
            .len(),
        1
    );

    let legacy_path = directory.path().join("quota-scale-legacy.sqlite3");
    verify_legacy_quota_coexistence(&legacy_path);
    eprintln!(
        "P2-D quota scale windows={} transitions={} redundant_polls={} \
         max_write_ms={:.3} max_redundant_poll_ms={:.3} current_32_ms={:.3} \
         overview_32_ms={:.3} \
         max_history_256_ms={:.3} scheduled={} early={} manual={}",
        WINDOW_COUNT,
        TRANSITION_COUNT,
        REDUNDANT_POLL_COUNT,
        maximum_write.as_secs_f64() * 1_000.0,
        maximum_redundant_poll.as_secs_f64() * 1_000.0,
        current_elapsed.as_secs_f64() * 1_000.0,
        overview_elapsed.as_secs_f64() * 1_000.0,
        maximum_history_read.as_secs_f64() * 1_000.0,
        reset_counts[0],
        reset_counts[1],
        reset_counts[2],
    );
}
