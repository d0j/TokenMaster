use tempfile::TempDir;
use tokenmaster_query::{
    CalendarDate, QueryClock, QueryError, QueryService, QueryTimeSample, UsageAnalyticsRequest,
    UsageRange, UsageSeriesSelection, UsageTimeZone, WeekStart,
};
use tokenmaster_store::UsageStore;

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_710_158_400_000, 1))
    }
}

fn date(year: i16, month: u8, day: u8) -> CalendarDate {
    CalendarDate::new(year, month, day).expect("valid fixture date")
}

fn recent_request(day_count: u16, zone: &str) -> UsageAnalyticsRequest {
    UsageAnalyticsRequest::new(
        UsageRange::recent_days(day_count).expect("bounded recent range"),
        UsageTimeZone::iana(zone).expect("fixture time zone"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        Vec::new(),
    )
    .expect("recent analytics request")
}

#[test]
fn recent_days_resolves_exact_local_dates_and_dst_buckets() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("recent-history-dst.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");

    let envelope = service
        .usage_analytics(recent_request(3, "America/New_York"))
        .expect("recent history");
    let history = envelope.payload();

    assert_eq!(history.range().start_date(), date(2024, 3, 9));
    assert_eq!(history.range().end_date(), date(2024, 3, 12));
    assert_eq!(history.series().len(), 3);
    assert_eq!(history.series()[0].start_date(), date(2024, 3, 9));
    assert_eq!(history.series()[1].start_date(), date(2024, 3, 10));
    assert_eq!(history.series()[2].start_date(), date(2024, 3, 11));
    assert_eq!(
        history.series()[1].end_seconds() - history.series()[1].start_seconds(),
        23 * 60 * 60
    );
}

#[test]
fn recent_days_allows_the_exact_four_hundred_point_limit() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("recent-history-limit.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service = QueryService::open(&path, FixedClock).expect("query service");

    let envelope = service
        .usage_analytics(recent_request(400, "UTC"))
        .expect("maximum recent history");
    let history = envelope.payload();

    assert_eq!(history.range().start_date(), date(2023, 2, 6));
    assert_eq!(history.range().end_date(), date(2024, 3, 12));
    assert_eq!(history.series().len(), 400);
}
