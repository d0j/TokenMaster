#![allow(dead_code)]

use std::path::Path;

use rusqlite::{Connection, params};
use tokenmaster_domain::{
    GitActivityAssociationId, GitOutputQuality, GitOutputWarning, GitRepositoryId, ProjectAlias,
};
use tokenmaster_git::{
    GitAuthorFingerprint, GitCommitAccumulator, GitCommitFingerprint, GitMailmapFingerprint,
    GitObjectFormat, GitPathStat, GitRefFingerprint, GitScanAccumulator, GitScanSummary,
    derive_project_fingerprint,
};
use tokenmaster_query::{
    CalendarDate, GitOutputRequest, QueryClock, QueryError, QueryService, QueryTimeSample,
    UsageRange, WeekStart,
};
use tokenmaster_store::{
    GitCacheIdentity, GitProjectKey, GitProjectionInput, GitProjectionInputParts, UsageStore,
};

pub const DAY_INDEX: i32 = 20_650;
pub const DAY_START_SECONDS: i64 = 1_784_160_000;
pub const WALL_TIME_MS: i64 = 1_784_203_200_000;

#[derive(Clone, Copy)]
pub struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(WALL_TIME_MS, 10))
    }
}

pub fn date() -> CalendarDate {
    CalendarDate::new(2026, 7, 16).expect("fixture date")
}

pub fn request(max_repositories: usize) -> GitOutputRequest {
    GitOutputRequest::new(
        UsageRange::day(date()),
        WeekStart::Monday,
        Vec::new(),
        max_repositories,
    )
    .expect("Git output request")
}

pub fn service(path: &Path) -> QueryService<FixedClock> {
    QueryService::open(path, FixedClock).expect("query service")
}

pub fn seed_current_usage(path: &Path, project: &str, reported_cost_usd_micros: u64) {
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
             ) VALUES (?1, 'codex', 'default', 'private-source', 'active', ?2, ?3, 0)",
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
               ?1, 'event-1', ?2, 0, 1, 0, 0, 0, 'codex', 'default',
               'private-session', 'private-source', ?3, 0, 'gpt-5.6',
               100, 20, 30, 10, 140, ?4, 0, 'standard', 'no', ?5,
               0, 0, 0, 0, 0, 0, 0, 0
             )",
            params![
                [1_u8; 32].as_slice(),
                [7_u8; 32].as_slice(),
                DAY_START_SECONDS + 3_600,
                i64::try_from(reported_cost_usd_micros).expect("bounded cost"),
                project
            ],
        )
        .expect("usage event");
    transaction.commit().expect("fixture commit");
}

pub fn seed_repository(
    path: &Path,
    repository_seed: u8,
    association_seed: u8,
    project: &str,
    summary: GitScanSummary,
) {
    seed_repository_at(
        path,
        repository_seed,
        association_seed,
        project,
        summary,
        WALL_TIME_MS - 1_000 + i64::from(repository_seed),
    );
}

pub fn seed_repository_at(
    path: &Path,
    repository_seed: u8,
    association_seed: u8,
    project: &str,
    summary: GitScanSummary,
    observed_at_ms: i64,
) {
    let mut store = UsageStore::open(path).expect("open archive");
    let salt = store.git_identity_salt().expect("installation salt");
    let alias = ProjectAlias::new(project).expect("project alias");
    let project_key = GitProjectKey::from_bytes(
        *derive_project_fingerprint(&salt, &alias)
            .expect("project fingerprint")
            .as_bytes(),
    );
    let daily_history_truncated = summary.daily_history_truncated();
    let input = GitProjectionInput::new(GitProjectionInputParts {
        repository_id: GitRepositoryId::from_bytes([repository_seed; 32]),
        association_id: GitActivityAssociationId::from_bytes([association_seed; 32]),
        project_key: Some(project_key),
        activity_at_ms: observed_at_ms - 1_000,
        observed_at_ms,
        data_through_ms: Some(observed_at_ms - 1_000),
        quality: if daily_history_truncated {
            GitOutputQuality::Partial
        } else {
            GitOutputQuality::Complete
        },
        unavailable_reason: None,
        warnings: if daily_history_truncated {
            vec![GitOutputWarning::DailyHistoryTruncated]
        } else {
            Vec::new()
        },
        summary: Some(summary),
        cache: Some(
            GitCacheIdentity::new(
                GitObjectFormat::Sha1,
                GitRefFingerprint::from_bytes([repository_seed; 32]),
                GitMailmapFingerprint::from_bytes([2; 32]),
                GitAuthorFingerprint::from_bytes([3; 32]),
                1,
                false,
            )
            .expect("cache"),
        ),
    })
    .expect("projection input");
    store.publish_git_rebuild(&input).expect("publish Git");
}

pub fn summary(day_index: i32, added: u64, removed: u64) -> GitScanSummary {
    let mut commit =
        GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([9; 32]), day_index, 1)
            .expect("commit");
    commit
        .record(GitPathStat::text(b"src/lib.rs", added, removed).expect("path"))
        .expect("record");
    let mut scan = GitScanAccumulator::new();
    scan.push(commit.finish().expect("finish")).expect("push");
    scan.finish().expect("summary")
}

pub fn summary_range(start_day_index: i32, day_count: usize) -> GitScanSummary {
    let mut scan = GitScanAccumulator::new();
    for offset in 0..day_count {
        let mut fingerprint = [0_u8; 32];
        fingerprint[..8]
            .copy_from_slice(&u64::try_from(offset).expect("bounded offset").to_be_bytes());
        let day_index = start_day_index
            .checked_add(i32::try_from(offset).expect("bounded offset"))
            .expect("bounded day");
        let mut commit =
            GitCommitAccumulator::new(GitCommitFingerprint::from_bytes(fingerprint), day_index, 1)
                .expect("commit");
        commit
            .record(GitPathStat::text(b"src/lib.rs", 1, 0).expect("path"))
            .expect("record");
        scan.push(commit.finish().expect("finish")).expect("push");
    }
    scan.finish().expect("summary")
}
