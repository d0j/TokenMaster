use std::path::Path;

use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_domain::{GitActivityAssociationId, GitOutputQuality, GitRepositoryId};
use tokenmaster_git::{
    GitAuthorFingerprint, GitCommitAccumulator, GitCommitFingerprint, GitMailmapFingerprint,
    GitObjectFormat, GitPathStat, GitRefFingerprint, GitScanAccumulator, GitScanSummary,
};
use tokenmaster_store::{
    GitCacheIdentity, GitProjectKey, GitProjectionInput, GitProjectionInputParts, StoreErrorCode,
    UsageStore,
};

fn summary(day: i32, added: u64, removed: u64) -> GitScanSummary {
    let mut commit = GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([9; 32]), day, 1)
        .expect("commit accumulator");
    commit
        .record(GitPathStat::text(b"src/main.rs", added, removed).expect("path stat"))
        .expect("record path");
    let mut scan = GitScanAccumulator::new();
    scan.push(commit.finish().expect("finish commit"))
        .expect("scan commit");
    scan.finish().expect("scan summary")
}

fn input(
    repository_seed: u8,
    association_seed: u8,
    heads_seed: u8,
    observed_at_ms: i64,
    summary: GitScanSummary,
) -> GitProjectionInput {
    GitProjectionInput::new(GitProjectionInputParts {
        repository_id: GitRepositoryId::from_bytes([repository_seed; 32]),
        association_id: GitActivityAssociationId::from_bytes([association_seed; 32]),
        project_key: Some(GitProjectKey::from_bytes([association_seed; 32])),
        activity_at_ms: observed_at_ms - 1,
        observed_at_ms,
        data_through_ms: Some(observed_at_ms - 1),
        quality: GitOutputQuality::Complete,
        unavailable_reason: None,
        warnings: Vec::new(),
        summary: Some(summary),
        cache: Some(
            GitCacheIdentity::new(
                GitObjectFormat::Sha1,
                GitRefFingerprint::from_bytes([heads_seed; 32]),
                GitMailmapFingerprint::from_bytes([2; 32]),
                GitAuthorFingerprint::from_bytes([3; 32]),
                1,
                false,
            )
            .expect("cache identity"),
        ),
    })
    .expect("projection input")
}

fn raw_connection(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("open raw archive");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
}

#[test]
fn authoritative_rebuild_switches_one_immutable_generation_atomically() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-projection.sqlite3");
    let mut store = UsageStore::open(&path).expect("open store");
    assert_eq!(
        format!(
            "{:?}",
            store.git_identity_salt().expect("installation salt")
        ),
        "GitIdentitySalt([redacted])"
    );

    let first = store
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("first rebuild");
    assert_eq!(first.publication_revision(), 1);
    assert_eq!(first.scan_revision(), 1);
    assert_eq!(first.aggregate_generation(), 1);

    let second = store
        .publish_git_rebuild(&input(1, 2, 4, 1_800_000_001_000, summary(20_001, 11, 1)))
        .expect("replacement rebuild");
    assert_eq!(second.publication_revision(), 2);
    assert_eq!(second.scan_revision(), 2);
    assert_eq!(second.aggregate_generation(), 2);

    let connection = raw_connection(&path);
    assert_eq!(
        connection
            .query_row(
                "SELECT publication_revision, repository_count, association_count
                 FROM git_installation_state WHERE singleton_id = 1",
                [],
                |row| Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?
                )),
            )
            .expect("publication state"),
        (2, 1, 1)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT active_generation, scan_revision, added_lines, removed_lines
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
            .expect("repository"),
        (2, 2, 11, 1)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT
                   (SELECT count(*) FROM git_day_aggregate),
                   (SELECT count(*) FROM git_day_category_aggregate),
                   (SELECT count(*) FROM git_category_aggregate)",
                [],
                |row| Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?
                )),
            )
            .expect("aggregate row counts"),
        (1, 8, 8)
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT min(aggregate_generation), max(aggregate_generation)
                 FROM git_day_category_aggregate",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("active immutable generation"),
        (2, 2)
    );
}

#[test]
fn failed_rebuild_preserves_prior_publication_and_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-projection-fault.sqlite3");
    let mut store = UsageStore::open(&path).expect("open store");
    store
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("first rebuild");
    let connection = raw_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER test_git_day_insert_abort
             BEFORE INSERT ON git_day_aggregate
             BEGIN
               SELECT RAISE(ABORT, 'test fault');
             END;",
        )
        .expect("install fault");

    let error = store
        .publish_git_rebuild(&input(1, 2, 4, 1_800_000_001_000, summary(20_001, 11, 1)))
        .expect_err("faulted rebuild");
    assert_eq!(error.code(), StoreErrorCode::Database);
    assert_eq!(
        connection
            .query_row(
                "SELECT publication_revision FROM git_installation_state
                 WHERE singleton_id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("publication revision"),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT active_generation, added_lines FROM git_repository",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("preserved repository"),
        (1, 7)
    );
}

#[test]
fn incomplete_active_day_category_projection_fails_on_reopen() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-projection-corrupt.sqlite3");
    let mut store = UsageStore::open(&path).expect("open store");
    store
        .publish_git_rebuild(&input(1, 2, 3, 1_800_000_000_000, summary(20_000, 7, 2)))
        .expect("rebuild");
    drop(store);
    let connection = raw_connection(&path);
    connection
        .execute(
            "DELETE FROM git_day_category_aggregate
             WHERE category = 'product_code'",
            [],
        )
        .expect("corrupt day category projection");
    drop(connection);

    let error = UsageStore::open(&path).expect_err("incomplete projection must fail closed");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
}
