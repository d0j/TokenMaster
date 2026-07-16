#[allow(dead_code)]
mod support {
    include!("support/git_projection.rs");
}

use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_domain::{GitLineMetrics, GitOutputCategory, GitOutputQuality, GitOutputWarning};
use tokenmaster_store::{
    GitOutputQuery, GitProjectKey, StoreErrorCode, UsageReadStore, UsageStore,
};

use support::{input, summary, unavailable_input};

#[test]
fn immutable_capture_returns_exact_bounded_range_and_all_time_projection() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-query.sqlite3");
    let mut writer = UsageStore::open(&path).expect("open writer");
    writer
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("first repository");
    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(19_999, 20_001, 32, Duration::from_secs(2)).expect("query"),
        )
        .expect("Git capture");
    assert_eq!(capture.publication_revision(), 1);
    assert_eq!(capture.repositories().len(), 1);
    assert!(!capture.has_more_repositories());
    let repository = &capture.repositories()[0];
    assert_eq!(repository.scan_revision(), 1);
    assert_eq!(repository.quality(), GitOutputQuality::Complete);
    assert_eq!(
        repository.project_key(),
        Some(GitProjectKey::from_bytes([2; 32]))
    );
    assert!(!repository.rebuild_required());
    assert_eq!(repository.all_time_totals().commits(), 1);
    assert_eq!(
        repository.all_time_totals().lines(),
        GitLineMetrics::new(7, 2)
    );
    assert_eq!(repository.range_totals().commits(), 1);
    assert_eq!(repository.range_totals().merge_commits(), 0);
    assert_eq!(repository.range_totals().lines(), GitLineMetrics::new(7, 2));
    assert_eq!(repository.days().len(), 1);
    assert_eq!(repository.days()[0].day_index(), 20_000);
    assert_eq!(repository.range_categories().len(), 8);
    assert_eq!(
        repository
            .range_categories()
            .iter()
            .find(|item| item.category() == GitOutputCategory::ProductCode)
            .expect("product code")
            .lines(),
        GitLineMetrics::new(7, 2)
    );

    writer
        .publish_git_rebuild(&input(1, 2, 4, 1_800_000_001_000, summary(20_001, 99, 1)))
        .expect("new publication");
    assert_eq!(capture.publication_revision(), 1);
    assert_eq!(
        capture.repositories()[0].all_time_totals().lines(),
        GitLineMetrics::new(7, 2)
    );
}

#[test]
fn rebuild_required_is_visible_without_destroying_prior_projection() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-query-stale.sqlite3");
    let mut writer = UsageStore::open(&path).expect("open writer");
    let publication = writer
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("repository");
    writer
        .mark_git_rebuild_required(
            tokenmaster_store::GitIncrementalAuthority::new(
                tokenmaster_domain::GitRepositoryId::from_bytes([1; 32]),
                publication.scan_revision(),
                tokenmaster_git::GitRefFingerprint::from_bytes([3; 32]),
            )
            .expect("authority"),
            1_800_000_001_000,
        )
        .expect("mark stale");

    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(20_000, 20_000, 32, Duration::from_secs(2)).expect("query"),
        )
        .expect("capture");
    let repository = &capture.repositories()[0];
    assert!(repository.rebuild_required());
    assert_eq!(repository.quality(), GitOutputQuality::Partial);
    assert!(
        repository
            .warnings()
            .contains(&GitOutputWarning::IncrementalRebuildPending)
    );
    assert_eq!(repository.all_time_totals().commits(), 1);
}

#[test]
fn query_bounds_fail_before_sql() {
    assert_eq!(
        GitOutputQuery::new(0, 400, 32, Duration::from_secs(2))
            .expect_err("401 days")
            .code(),
        StoreErrorCode::CapacityExceeded
    );
    assert_eq!(
        GitOutputQuery::new(0, 1, 33, Duration::from_secs(2))
            .expect_err("33 repositories")
            .code(),
        StoreErrorCode::CapacityExceeded
    );
    assert_eq!(
        GitOutputQuery::new(1, 0, 1, Duration::from_secs(2))
            .expect_err("reversed range")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        GitOutputQuery::new(0, 1, 1, Duration::ZERO)
            .expect_err("zero deadline")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        GitOutputQuery::new(0, 1, 1, Duration::from_secs(3))
            .expect_err("overlong deadline")
            .code(),
        StoreErrorCode::InvalidValue
    );
}

#[test]
fn unavailable_repository_preserves_absence_instead_of_fabricating_zero_series() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-query-unavailable.sqlite3");
    let mut writer = UsageStore::open(&path).expect("open writer");
    writer
        .publish_git_rebuild(&unavailable_input(1, 2, 1_800_000_000_000))
        .expect("unavailable publication");
    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(20_000, 20_001, 32, Duration::from_secs(2)).expect("query"),
        )
        .expect("capture");
    let repository = &capture.repositories()[0];
    assert_eq!(repository.quality(), GitOutputQuality::Unavailable);
    assert!(repository.data_through_ms().is_none());
    assert!(repository.days().is_empty());
    assert!(repository.range_categories().is_empty());
    assert!(repository.all_time_categories().is_empty());
}

#[test]
fn installation_publication_time_never_regresses_across_repositories() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-query-monotonic-time.sqlite3");
    let mut writer = UsageStore::open(&path).expect("open writer");
    writer
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_002_000, summary(20_000, 1, 0)))
        .expect("newer repository");
    writer
        .publish_git_rebuild(&input(4, 5, 6, 1_800_000_001_000, summary(20_000, 1, 0)))
        .expect("older independent repository");

    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(20_000, 20_000, 1, Duration::from_secs(2)).expect("query"),
        )
        .expect("capture");
    assert_eq!(capture.publication_revision(), 2);
    assert_eq!(capture.published_at_ms(), Some(1_800_000_002_000));
    assert_eq!(capture.repositories().len(), 1);
    assert!(capture.has_more_repositories());
}

#[test]
fn total_query_deadline_rejects_a_completed_late_capture() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-query-deadline.sqlite3");
    UsageStore::open(&path).expect("open writer");
    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let error = reader
        .capture_git_output(
            GitOutputQuery::new(20_000, 20_000, 32, Duration::from_nanos(1)).expect("query"),
        )
        .expect_err("expired query");
    assert_eq!(error.code(), StoreErrorCode::DeadlineExceeded);
    let next = reader
        .capture_git_output(
            GitOutputQuery::new(20_000, 20_000, 32, Duration::from_secs(2)).expect("next query"),
        )
        .expect("progress handler cleared");
    assert!(next.repositories().is_empty());
}

#[test]
fn conflicting_project_associations_remain_explicitly_unavailable_for_join() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-query-association.sqlite3");
    let mut writer = UsageStore::open(&path).expect("open writer");
    writer
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 1, 0)))
        .expect("first association");
    writer
        .publish_git_rebuild(&input(1, 4, 5, 1_800_000_001_000, summary(20_000, 1, 0)))
        .expect("conflicting association");

    let mut reader = UsageReadStore::open(&path).expect("open reader");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(20_000, 20_000, 32, Duration::from_secs(2)).expect("query"),
        )
        .expect("capture");
    let repository = &capture.repositories()[0];
    assert_eq!(repository.quality(), GitOutputQuality::Partial);
    assert!(repository.project_key().is_none());
    assert!(
        repository
            .warnings()
            .contains(&GitOutputWarning::AssociationIncomplete)
    );
}
