#![allow(dead_code)]

use std::path::Path;

use rusqlite::{Connection, params};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, GitActivityAssociationId,
    GitOutputQuality, GitRepositoryId, ProjectAlias, QuotaAccountId, QuotaConfidence,
    QuotaEvidenceSource, QuotaObservationId, QuotaPresentationDirection, QuotaProviderEpochId,
    QuotaRatio, QuotaResetEvidence, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowDefinitionParts, QuotaWindowId,
    QuotaWindowKey, QuotaWindowSemantics, UsageProviderId,
};
use tokenmaster_git::{
    GitAuthorFingerprint, GitCommitAccumulator, GitCommitFingerprint, GitMailmapFingerprint,
    GitObjectFormat, GitPathStat, GitRefFingerprint, GitScanAccumulator,
    derive_project_fingerprint,
};
use tokenmaster_query::{CalendarDate, QueryClock, QueryError, QueryTimeSample, UsageRange};
use tokenmaster_store::{
    GitCacheIdentity, GitProjectKey, GitProjectionInput, GitProjectionInputParts, UsageStore,
};

pub const DAY_INDEX: i32 = 20_650;
pub const DAY_START_SECONDS: i64 = 1_784_160_000;
pub const WALL_TIME_MS: i64 = 1_784_203_200_000;
pub const RESET_AT_MS: i64 = WALL_TIME_MS + 3_600_000;
pub const BENEFIT_EXPIRY_AT_MS: i64 = WALL_TIME_MS + 86_400_000;

#[derive(Clone, Copy)]
pub struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(WALL_TIME_MS, 10))
    }
}

pub fn day() -> CalendarDate {
    CalendarDate::new(2026, 7, 16).expect("fixture date")
}

pub fn range() -> UsageRange {
    UsageRange::day(day())
}

pub fn seed(path: &Path) {
    seed_usage(path);
    seed_git(path);
    seed_quota(path);
    seed_benefits(path);
}

pub fn add_distinct_usage_rows(path: &Path, additional: u8) {
    let mut connection = Connection::open(path).expect("usage scale connection");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("usage scale transaction");
    for index in 0..additional {
        let seed = index.checked_add(20).expect("bounded fixture seed");
        transaction
            .execute(
                "INSERT INTO usage_event(
                   fingerprint, event_id, selected_file_key, selected_generation,
                   selected_source_offset, projection_revision_id, origin_revision_id,
                   retained, provider_id, profile_id, session_id, source_id,
                   timestamp_seconds, timestamp_nanos, model, input_tokens,
                   cached_tokens, output_tokens, reasoning_tokens, total_tokens,
                   reported_cost_usd_micros, fallback_model, service_tier, long_context,
                   project_alias, activity_read, activity_edit_write, activity_search,
                   activity_git, activity_build_test, activity_web, activity_subagents,
                   activity_terminal
                 ) VALUES (
                   ?1, ?2, ?3, 0, ?4, 0, 0, 0, 'codex', 'default', ?5,
                   'dashboard-private-source', ?6, 0, ?7, 1, 0, 1, 0, 2, 1, 0,
                   'standard', 'no', 'tokenmaster', 0, 0, 0, 0, 0, 0, 0, 0
                 )",
                params![
                    [seed; 32].as_slice(),
                    format!("dashboard-private-event-{index:02}"),
                    [7_u8; 32].as_slice(),
                    i64::from(index) + 2,
                    format!("dashboard-private-session-{index:02}"),
                    DAY_START_SECONDS + 4_000 + i64::from(index),
                    format!("model-{index:02}"),
                ],
            )
            .expect("usage scale row");
    }
    transaction.commit().expect("commit usage scale fixture");
}

pub fn add_second_git_project(path: &Path) {
    let connection = Connection::open(path).expect("second project connection");
    connection
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               reported_cost_usd_micros, fallback_model, service_tier, long_context,
               project_alias, activity_read, activity_edit_write, activity_search,
               activity_git, activity_build_test, activity_web, activity_subagents,
               activity_terminal
             ) VALUES (
               ?1, 'dashboard-private-second-event', ?2, 0, 50, 0, 0, 0,
               'codex', 'default', 'dashboard-private-second-session',
               'dashboard-private-source', ?3, 0, 'gpt-5.6', 50, 10, 20, 5, 70,
               5000, 0, 'standard', 'no', 'second-project',
               0, 0, 0, 0, 0, 0, 0, 0
             )",
            params![
                [40_u8; 32].as_slice(),
                [7_u8; 32].as_slice(),
                DAY_START_SECONDS + 7_200,
            ],
        )
        .expect("second project usage");
    publish_git_projection(path, 10, 11, "second-project", 100, 10);
}

fn seed_usage(path: &Path) {
    drop(UsageStore::open(path).expect("create archive"));
    let mut connection = Connection::open(path).expect("fixture connection");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    let transaction = connection.transaction().expect("fixture transaction");
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'dashboard-private-source', 'active', ?2, ?3, 0)",
            params![
                [7_u8; 32].as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("source");
    transaction
        .execute_batch(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completed_at_ms, completion_state,
               expected_scope_count
             ) VALUES (1, 1784200000000, 1784203199000, 'complete', 1);
             INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (
               1, 1, 'codex', 'default', 1784200000000, 1784203199000, 'complete'
             );
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
        .expect("publication");
    transaction
        .execute(
            "INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               reported_cost_usd_micros, fallback_model, service_tier, long_context,
               project_alias, activity_read, activity_edit_write, activity_search,
               activity_git, activity_build_test, activity_web, activity_subagents,
               activity_terminal
             ) VALUES (
               ?1, 'dashboard-private-event', ?2, 0, 1, 0, 0, 0, 'codex', 'default',
               'dashboard-private-session', 'dashboard-private-source', ?3, 0, 'gpt-5.6',
               100, 20, 30, 10, 140, 10000, 0, 'standard', 'no', 'tokenmaster',
               1, 2, 3, 4, 5, 6, 7, 8
             )",
            params![
                [1_u8; 32].as_slice(),
                [7_u8; 32].as_slice(),
                DAY_START_SECONDS + 3_600,
            ],
        )
        .expect("usage event");
    transaction.commit().expect("commit usage fixture");
}

fn seed_git(path: &Path) {
    publish_git_projection(path, 4, 5, "tokenmaster", 200, 20);
}

fn publish_git_projection(
    path: &Path,
    repository_seed: u8,
    association_seed: u8,
    project: &str,
    added: u64,
    removed: u64,
) {
    let mut commit =
        GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([9; 32]), DAY_INDEX, 1)
            .expect("commit");
    commit
        .record(GitPathStat::text(b"src/lib.rs", added, removed).expect("path"))
        .expect("record path");
    let mut scan = GitScanAccumulator::new();
    scan.push(commit.finish().expect("finish commit"))
        .expect("push commit");
    let summary = scan.finish().expect("scan summary");

    let mut store = UsageStore::open(path).expect("open archive for Git");
    let salt = store.git_identity_salt().expect("installation salt");
    let alias = ProjectAlias::new(project).expect("project alias");
    let project_key = GitProjectKey::from_bytes(
        *derive_project_fingerprint(&salt, &alias)
            .expect("project fingerprint")
            .as_bytes(),
    );
    let input = GitProjectionInput::new(GitProjectionInputParts {
        repository_id: GitRepositoryId::from_bytes([repository_seed; 32]),
        association_id: GitActivityAssociationId::from_bytes([association_seed; 32]),
        project_key: Some(project_key),
        activity_at_ms: WALL_TIME_MS - 2_000,
        observed_at_ms: WALL_TIME_MS - 1_000,
        data_through_ms: Some(WALL_TIME_MS - 2_000),
        quality: GitOutputQuality::Complete,
        unavailable_reason: None,
        warnings: Vec::new(),
        summary: Some(summary),
        cache: Some(
            GitCacheIdentity::new(
                GitObjectFormat::Sha1,
                GitRefFingerprint::from_bytes([repository_seed; 32]),
                GitMailmapFingerprint::from_bytes([association_seed; 32]),
                GitAuthorFingerprint::from_bytes([repository_seed.saturating_add(1); 32]),
                1,
                false,
            )
            .expect("cache identity"),
        ),
    })
    .expect("Git projection input");
    store.publish_git_rebuild(&input).expect("publish Git");
}

fn quota_key(account: &str, window: &str) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new(account).expect("account"),
            None,
        ),
        QuotaWindowId::new(window).expect("window"),
    )
}

fn seed_quota(path: &Path) {
    publish_quota(
        path,
        7,
        "dashboard-private-account",
        "dynamic-weekly",
        "quota.dynamic_weekly",
        700_000,
    );
}

pub fn add_quota_windows(path: &Path, additional: u8) {
    for index in 0..additional {
        let seed = index.checked_add(50).expect("bounded quota fixture");
        let window = format!("dynamic-window-{index:02}");
        let label = format!("quota.dynamic_window_{index:02}");
        publish_quota(
            path,
            seed,
            &format!("dashboard-private-account-{index:02}"),
            &window,
            &label,
            100_000_u32.saturating_add(u32::from(index) * 10_000),
        );
    }
}

fn publish_quota(path: &Path, seed: u8, account: &str, window: &str, label: &str, used_ppm: u32) {
    let key = quota_key(account, window);
    let definition = QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: key.clone(),
        revision: 1,
        label_key: label.to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(7 * 24 * 60 * 60),
        reset_thresholds: None,
    })
    .expect("quota definition");
    let sample = QuotaSample::new(QuotaSampleParts {
        key,
        observation_id: QuotaObservationId::from_bytes([seed; 32]),
        observed_at_ms: WALL_TIME_MS - 1_000,
        fresh_until_ms: WALL_TIME_MS + 60_000,
        stale_after_ms: WALL_TIME_MS + 120_000,
        provider_epoch_id: Some(QuotaProviderEpochId::new("epoch-1").expect("epoch")),
        used_ratio: Some(QuotaRatio::new(used_ppm).expect("used ratio")),
        remaining_ratio: Some(
            QuotaRatio::new(
                1_000_000_u32
                    .checked_sub(used_ppm)
                    .expect("remaining ratio"),
            )
            .expect("remaining ratio"),
        ),
        units: Some(
            QuotaUnits::new(
                QuotaUnitId::new("tokens").expect("unit"),
                Some(u64::from(used_ppm) / 1_000),
                Some(u64::from(1_000_000_u32 - used_ppm) / 1_000),
                Some(1_000),
            )
            .expect("quota units"),
        ),
        advertised_resets_at_ms: Some(RESET_AT_MS),
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderOfficial,
        confidence: QuotaConfidence::High,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    })
    .expect("quota sample");
    UsageStore::open(path)
        .expect("open archive for quota")
        .apply_quota_observation(&definition, &sample)
        .expect("publish quota");
}

fn benefit_lot(
    id: u8,
    kind: BenefitKind,
    quantity: u64,
    state: BenefitState,
    expiry: BenefitExpiry,
) -> BenefitLotObservation {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([id; 32]),
        kind,
        quantity,
        state,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(WALL_TIME_MS - 1_000),
        expiry,
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.inventory").expect("label"),
    })
    .expect("benefit lot")
}

fn seed_benefits(path: &Path) {
    let observation = BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: BenefitScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("dashboard-private-account").expect("account"),
            None,
        ),
        observation_id: BenefitObservationId::from_bytes([8; 32]),
        observed_at_ms: WALL_TIME_MS - 1_000,
        fresh_until_ms: WALL_TIME_MS + 60_000,
        stale_after_ms: WALL_TIME_MS + 120_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots: vec![
            benefit_lot(
                1,
                BenefitKind::BankedRateLimitReset,
                2,
                BenefitState::Available,
                BenefitExpiry::exact_utc(BENEFIT_EXPIRY_AT_MS).expect("reset expiry"),
            ),
            benefit_lot(
                2,
                BenefitKind::UsageCredit,
                4,
                BenefitState::Available,
                BenefitExpiry::unknown(),
            ),
            benefit_lot(
                3,
                BenefitKind::TemporaryUsage,
                3,
                BenefitState::ActivationPending,
                BenefitExpiry::unknown(),
            ),
            benefit_lot(
                4,
                BenefitKind::BankedRateLimitReset,
                7,
                BenefitState::Expired,
                BenefitExpiry::exact_utc(WALL_TIME_MS - 1).expect("expired at"),
            ),
        ],
    })
    .expect("benefit observation");
    UsageStore::open(path)
        .expect("open archive for benefit")
        .apply_benefit_observation(&observation)
        .expect("publish benefit");
}
