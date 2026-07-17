mod support;

use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_query::{CalendarDate, GitOutputRequest, UsageRange, WeekStart};

use support::git_output::{DAY_INDEX, seed_current_usage, seed_repository, service, summary_range};

const REPOSITORIES: usize = 32;
const DAYS: usize = 400;

#[cfg(windows)]
fn handle_count() -> u32 {
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

    let mut handles = 0_u32;
    unsafe { GetProcessHandleCount(GetCurrentProcess(), &raw mut handles) }
        .expect("process handle count");
    handles
}

#[test]
fn maximum_repository_and_day_snapshot_is_bounded_and_releases_resources() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("maximum.sqlite3");
    seed_current_usage(&path, "tokenmaster", 10_000);
    let start_day = DAY_INDEX - i32::try_from(DAYS).expect("days") + 1;
    for repository in 1..=REPOSITORIES {
        seed_repository(
            &path,
            u8::try_from(repository).expect("repository"),
            u8::try_from(repository).expect("association"),
            "tokenmaster",
            summary_range(start_day, DAYS),
        );
    }
    let request = GitOutputRequest::new(
        UsageRange::custom(
            CalendarDate::new(2025, 6, 12).expect("start"),
            CalendarDate::new(2026, 7, 17).expect("end"),
        )
        .expect("400 days"),
        WeekStart::Monday,
        Vec::new(),
        REPOSITORIES,
    )
    .expect("request");
    let mut service = service(&path);
    let started = Instant::now();
    let snapshot = service.git_output(request.clone()).expect("maximum query");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "maximum query exceeded the hard service deadline"
    );
    assert_eq!(snapshot.payload().repositories().len(), REPOSITORIES);
    assert!(!snapshot.payload().has_more_repositories());
    assert!(
        snapshot
            .payload()
            .repositories()
            .iter()
            .all(|repository| repository.days().len() == DAYS)
    );
    assert!(
        snapshot
            .payload()
            .repositories()
            .iter()
            .all(|repository| repository.range_categories().len() == 8)
    );

    #[cfg(windows)]
    let baseline_handles = handle_count();
    for _ in 0..16 {
        let current = service.git_output(request.clone()).expect("repeat");
        assert_eq!(current.payload().repositories().len(), REPOSITORIES);
    }
    #[cfg(windows)]
    assert!(
        handle_count() <= baseline_handles.saturating_add(1),
        "repeated Git snapshots retained process handles"
    );
    drop(service);
    let connection = rusqlite::Connection::open(&path).expect("writer connection");
    connection
        .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .expect("no read transaction or handler escaped");
}
