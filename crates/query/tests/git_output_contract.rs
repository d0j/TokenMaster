mod support;

use tempfile::TempDir;
use tokenmaster_domain::{GitOutputCategory, GitOutputQuality};
use tokenmaster_query::{GitEfficiency, QueryFreshness};

use support::git_output::{
    DAY_INDEX, WALL_TIME_MS, request, seed_current_usage, seed_repository, seed_repository_at,
    service, summary,
};

#[test]
fn git_output_is_immutable_utc_bounded_and_private() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("git-private.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));

    let mut service = service(&path);
    let snapshot = service.git_output(request(32)).expect("Git output");
    assert_eq!(snapshot.header().snapshot_generation().get(), 1);
    assert_eq!(snapshot.header().publication_revision().get(), 1);
    assert!(snapshot.header().scopes().is_empty());
    assert_eq!(snapshot.payload().range().time_zone_id(), "UTC");
    assert_eq!(
        snapshot.payload().range().start_seconds(),
        support::git_output::DAY_START_SECONDS
    );
    assert_eq!(
        snapshot.payload().range().end_seconds(),
        support::git_output::DAY_START_SECONDS + 86_400
    );
    assert_eq!(snapshot.payload().range().start_day_index(), DAY_INDEX);
    assert_eq!(
        snapshot.payload().range().end_day_index_exclusive(),
        DAY_INDEX + 1
    );
    assert!(!snapshot.payload().has_more_repositories());
    assert_eq!(snapshot.payload().repositories().len(), 1);

    let repository = &snapshot.payload().repositories()[0];
    assert_eq!(repository.quality(), GitOutputQuality::Complete);
    assert_eq!(repository.freshness(), QueryFreshness::Fresh);
    assert_eq!(
        repository.project_alias().expect("matched alias").as_str(),
        "tokenmaster"
    );
    assert_eq!(repository.range_totals().commits(), 1);
    assert_eq!(repository.range_totals().lines().added(), 200);
    assert_eq!(repository.range_totals().lines().removed(), 20);
    assert_eq!(repository.days().len(), 1);
    assert_eq!(repository.days()[0].day_index(), DAY_INDEX);
    assert_eq!(repository.range_categories().len(), 8);
    assert_eq!(
        repository
            .range_categories()
            .iter()
            .find(|item| item.category() == GitOutputCategory::ProductCode)
            .expect("product code")
            .lines()
            .added(),
        200
    );
    let GitEfficiency::Available(efficiency) = repository.efficiency() else {
        panic!("efficiency should be available");
    };
    assert_eq!(efficiency.usage_cost().get(), 10_000);
    assert_eq!(efficiency.product_code_added_lines(), 200);
    assert_eq!(efficiency.cost_per_100_added_lines().get(), 5_000);

    drop(service);
    assert_eq!(snapshot.payload().repositories()[0].days().len(), 1);
    let debug = format!("{snapshot:?}");
    for private in [
        path.to_string_lossy().as_ref(),
        "private-source",
        "private-session",
        "src/lib.rs",
        "SELECT ",
        "refs/heads",
        "@example.com",
    ] {
        assert!(!debug.contains(private), "debug exposed {private}");
    }
}

#[test]
fn successful_queries_are_strictly_generation_ordered() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("generation.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    let mut service = service(&path);

    let first = service.git_output(request(32)).expect("first");
    let second = service.git_output(request(32)).expect("second");
    assert_eq!(first.header().snapshot_generation().get(), 1);
    assert_eq!(second.header().snapshot_generation().get(), 2);
    assert!(second.is_newer_than(Some(&first)));
}

#[test]
fn prior_snapshot_is_owned_across_publication_and_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("isolation.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    let mut first_service = service(&path);
    let first = first_service.git_output(request(32)).expect("first");

    seed_repository_at(
        &path,
        1,
        2,
        "tokenmaster",
        summary(DAY_INDEX, 300, 30),
        WALL_TIME_MS,
    );
    let second = first_service.git_output(request(32)).expect("second");
    assert_eq!(
        first.payload().repositories()[0]
            .range_totals()
            .lines()
            .added(),
        200
    );
    assert_eq!(
        second.payload().repositories()[0]
            .range_totals()
            .lines()
            .added(),
        300
    );
    assert_eq!(first.header().publication_revision().get(), 1);
    assert_eq!(second.header().publication_revision().get(), 2);
    drop(first_service);

    let restarted = service(&path).git_output(request(32)).expect("restart");
    assert_eq!(restarted.header().snapshot_generation().get(), 1);
    assert_eq!(restarted.header().publication_revision().get(), 2);
    assert_eq!(
        first.payload().repositories()[0]
            .range_totals()
            .lines()
            .added(),
        200
    );

    let connection = rusqlite::Connection::open(&path).expect("writer connection");
    connection
        .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .expect("query returned every transaction");
}

#[test]
fn repository_limit_uses_one_row_lookahead() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("lookahead.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 1, "tokenmaster", summary(DAY_INDEX, 1, 0));
    seed_repository(&path, 2, 2, "tokenmaster", summary(DAY_INDEX, 1, 0));

    let snapshot = service(&path).git_output(request(1)).expect("limited");
    assert_eq!(snapshot.payload().repositories().len(), 1);
    assert!(snapshot.payload().has_more_repositories());
}

#[test]
fn corrupt_git_projection_fails_closed_without_consuming_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("corrupt.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    let mut service = service(&path);
    let connection = rusqlite::Connection::open(&path).expect("fixture connection");
    connection
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
             DROP TRIGGER git_day_no_update;
             UPDATE git_day_aggregate
             SET merge_commits = commits + 1;",
        )
        .expect("inject corruption");

    let error = service
        .git_output(request(32))
        .expect_err("corruption must fail");
    assert_eq!(
        error.code(),
        tokenmaster_query::QueryErrorCode::CorruptArchive
    );
    connection
        .execute(
            "UPDATE git_day_aggregate
             SET merge_commits = 0",
            [],
        )
        .expect("repair fixture");
    let recovered = service.git_output(request(32)).expect("recovered query");
    assert_eq!(recovered.header().snapshot_generation().get(), 1);
}

#[test]
fn git_query_does_not_scan_raw_usage_events() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("aggregate-only.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    seed_repository(&path, 1, 2, "tokenmaster", summary(DAY_INDEX, 200, 20));
    let mut service = service(&path);
    let connection = rusqlite::Connection::open(&path).expect("fixture connection");
    connection
        .execute_batch("ALTER TABLE usage_event RENAME TO private_raw_usage_event;")
        .expect("make raw usage table unavailable");

    let snapshot = service
        .git_output(request(32))
        .expect("aggregate-only Git query");
    assert_eq!(
        snapshot.payload().repositories()[0]
            .range_totals()
            .commits(),
        1
    );
}
