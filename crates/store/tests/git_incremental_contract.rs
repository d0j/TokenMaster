#[allow(dead_code)]
mod support {
    include!("support/git_projection.rs");
}

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{
    GitActivityAssociationId, GitOutputQuality, GitOutputWarning, GitRepositoryId,
};
use tokenmaster_git::GitRefFingerprint;
use tokenmaster_store::{
    GitIncrementalAuthority, GitOutputQuery, GitRefreshInput, GitRefreshInputParts, StoreErrorCode,
    UsageReadStore, UsageStore,
};

use support::{cache, input, summary, summary_range};

fn raw_connection(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("open raw archive");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
}

#[test]
fn proven_append_merges_delta_into_a_new_generation_and_stale_cas_writes_nothing() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-incremental.sqlite3");
    let mut store = UsageStore::open(&path).expect("open store");
    store
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("initial rebuild");

    let authority = GitIncrementalAuthority::new(
        GitRepositoryId::from_bytes([1; 32]),
        1,
        GitRefFingerprint::from_bytes([3; 32]),
    )
    .expect("append authority");
    let publication = store
        .publish_git_append(
            authority,
            &input(1, 2, 4, 1_800_000_001_000, summary(20_000, 3, 1)),
        )
        .expect("append delta");
    assert_eq!(publication.publication_revision(), 2);
    assert_eq!(publication.scan_revision(), 2);
    assert_eq!(publication.aggregate_generation(), 2);

    let connection = raw_connection(&path);
    assert_eq!(
        connection
            .query_row(
                "SELECT commits, added_lines, removed_lines, active_generation
                 FROM git_repository",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .expect("merged repository"),
        (2, 10, 3, 2)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT commits, added_lines, removed_lines
                 FROM git_day_aggregate",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .expect("merged day"),
        (2, 10, 3)
    );

    let stale = store
        .publish_git_append(
            authority,
            &input(1, 2, 5, 1_800_000_002_000, summary(20_001, 100, 0)),
        )
        .expect_err("stale authority");
    assert_eq!(stale.code(), StoreErrorCode::StaleRevision);
    assert_eq!(
        connection
            .query_row(
                "SELECT publication_revision, added_lines
                 FROM git_installation_state
                 JOIN git_repository ON 1 = 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("unchanged after stale append"),
        (2, 10)
    );
}

#[test]
fn unchanged_refresh_mutates_no_aggregate_and_invalidation_preserves_stale_projection() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-refresh.sqlite3");
    let mut store = UsageStore::open(&path).expect("open store");
    store
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("initial rebuild");
    let authority = GitIncrementalAuthority::new(
        GitRepositoryId::from_bytes([1; 32]),
        1,
        GitRefFingerprint::from_bytes([3; 32]),
    )
    .expect("refresh authority");
    let refresh = GitRefreshInput::new(GitRefreshInputParts {
        authority,
        association_id: GitActivityAssociationId::from_bytes([2; 32]),
        project_key: None,
        activity_at_ms: 1_799_999_999_000,
        observed_at_ms: 1_800_000_001_000,
        cache: cache(3),
    })
    .expect("refresh input");
    let publication = store
        .refresh_git_unchanged(&refresh)
        .expect("unchanged refresh");
    assert_eq!(publication.aggregate_generation(), 1);
    assert_eq!(publication.scan_revision(), 2);

    let connection = raw_connection(&path);
    assert_eq!(
        connection
            .query_row(
                "SELECT
                   (SELECT count(*) FROM git_day_aggregate),
                   (SELECT count(*) FROM git_day_category_aggregate),
                   (SELECT active_generation FROM git_repository),
                   (SELECT observed_at_ms FROM git_repository),
                   (SELECT project_key IS NULL FROM git_activity_association),
                   (SELECT first_activity_at_ms FROM git_activity_association),
                   (SELECT last_activity_at_ms FROM git_activity_association)",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .expect("refresh state"),
        (
            1,
            8,
            1,
            1_800_000_001_000,
            1,
            1_799_999_999_000,
            1_799_999_999_999
        )
    );

    let invalidate = GitIncrementalAuthority::new(
        GitRepositoryId::from_bytes([1; 32]),
        2,
        GitRefFingerprint::from_bytes([3; 32]),
    )
    .expect("invalidation authority");
    let invalidated = store
        .mark_git_rebuild_required(invalidate, 1_800_000_002_000)
        .expect("mark rebuild");
    assert_eq!(invalidated.publication_revision(), 3);
    assert_eq!(invalidated.aggregate_generation(), 1);
    assert_eq!(
        connection
            .query_row(
                "SELECT publication_state, added_lines,
                        (SELECT count(*) FROM git_day_aggregate)
                 FROM git_repository",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .expect("stale projection"),
        ("rebuild_required".to_owned(), 7, 1)
    );
}

#[test]
fn append_keeps_only_the_latest_four_hundred_daily_points() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-daily-bound.sqlite3");
    let mut store = UsageStore::open(&path).expect("open store");
    store
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary_range(0, 400)))
        .expect("bounded rebuild");
    let authority = GitIncrementalAuthority::new(
        GitRepositoryId::from_bytes([1; 32]),
        1,
        GitRefFingerprint::from_bytes([3; 32]),
    )
    .expect("append authority");
    store
        .publish_git_append(
            authority,
            &input(1, 2, 4, 1_800_000_001_000, summary(400, 1, 0)),
        )
        .expect("append newest day");

    let connection = raw_connection(&path);
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*), min(day_index), max(day_index)
                 FROM git_day_aggregate",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .expect("bounded days"),
        (400, 1, 400)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM git_day_category_aggregate",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("bounded day categories"),
        3_200
    );

    drop(connection);
    let mut reader = UsageReadStore::open(&path).expect("open bounded reader");
    let capture = reader
        .capture_git_output(
            GitOutputQuery::new(0, 399, 32, Duration::from_secs(2)).expect("bounded query"),
        )
        .expect("bounded capture");
    let repository = &capture.repositories()[0];
    assert_eq!(repository.quality(), GitOutputQuality::Partial);
    assert!(repository.daily_history_truncated());
    assert_eq!(repository.retained_from_day_index(), Some(1));
    assert!(!repository.range_complete());
    assert!(
        repository
            .warnings()
            .contains(&GitOutputWarning::DailyHistoryTruncated)
    );
}

#[test]
fn append_refresh_and_invalidation_faults_restore_the_exact_prior_publication() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-incremental-faults.sqlite3");
    let mut store = UsageStore::open(&path).expect("open store");
    store
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("initial rebuild");
    let connection = raw_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER test_git_repository_update_abort
             BEFORE UPDATE ON git_repository
             BEGIN
               SELECT RAISE(ABORT, 'test repository update fault');
             END;",
        )
        .expect("install repository fault");
    let authority = GitIncrementalAuthority::new(
        GitRepositoryId::from_bytes([1; 32]),
        1,
        GitRefFingerprint::from_bytes([3; 32]),
    )
    .expect("append authority");
    assert_eq!(
        store
            .publish_git_append(
                authority,
                &input(1, 2, 4, 1_800_000_001_000, summary(20_001, 1, 0)),
            )
            .expect_err("faulted append")
            .code(),
        StoreErrorCode::Database
    );
    connection
        .execute_batch(
            "DROP TRIGGER test_git_repository_update_abort;
             CREATE TRIGGER test_git_state_update_abort
             BEFORE UPDATE ON git_installation_state
             BEGIN
               SELECT RAISE(ABORT, 'test publication fault');
             END;",
        )
        .expect("install publication fault");
    let refresh = GitRefreshInput::new(GitRefreshInputParts {
        authority,
        association_id: GitActivityAssociationId::from_bytes([2; 32]),
        project_key: None,
        activity_at_ms: 1_800_000_000_500,
        observed_at_ms: 1_800_000_001_000,
        cache: cache(3),
    })
    .expect("refresh input");
    assert_eq!(
        store
            .refresh_git_unchanged(&refresh)
            .expect_err("faulted refresh")
            .code(),
        StoreErrorCode::Database
    );
    assert_eq!(
        store
            .mark_git_rebuild_required(authority, 1_800_000_001_000)
            .expect_err("faulted invalidation")
            .code(),
        StoreErrorCode::Database
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT
                   (SELECT publication_revision FROM git_installation_state),
                   (SELECT active_generation FROM git_repository),
                   (SELECT scan_revision FROM git_repository),
                   (SELECT observed_at_ms FROM git_repository),
                   (SELECT publication_state FROM git_repository),
                   (SELECT count(*) FROM git_day_aggregate),
                   (SELECT count(*) FROM git_day_category_aggregate)",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .expect("prior publication"),
        (1, 1, 1, 1_800_000_000_000, "ready".to_owned(), 1, 8)
    );
}
