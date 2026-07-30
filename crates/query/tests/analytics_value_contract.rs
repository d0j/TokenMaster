use tokenmaster_domain::{UsageProfileId, UsageProviderId};
use tokenmaster_query::{
    AggregateTokenValue, CalendarDate, MAX_QUERY_SERIES_POINTS, QueryErrorCode, QueryScope,
    UsageAnalyticsRequest, UsageBreakdownKind, UsageRange, UsageSeriesSelection, UsageTimeZone,
    WeekStart,
};

fn date(year: i16, month: u8, day: u8) -> CalendarDate {
    CalendarDate::new(year, month, day).expect("valid date")
}

fn scope(provider: &str, profile: &str) -> QueryScope {
    QueryScope::new(
        UsageProviderId::new(provider).expect("provider"),
        UsageProfileId::new(profile).expect("profile"),
    )
}

#[test]
fn dates_zones_ranges_and_daily_point_bounds_are_validated() {
    assert_eq!(MAX_QUERY_SERIES_POINTS, 400);
    assert_eq!(date(2024, 2, 29).year(), 2024);
    assert_eq!(date(2024, 2, 29).month(), 2);
    assert_eq!(date(2024, 2, 29).day(), 29);
    assert_eq!(
        CalendarDate::new(2023, 2, 29)
            .expect_err("invalid date")
            .code(),
        QueryErrorCode::InvalidValue
    );
    let utc = UsageTimeZone::iana("UTC").expect("UTC");
    assert_eq!(utc.stable_code(), "iana");
    assert_eq!(utc.iana_name(), Some("UTC"));
    assert_eq!(UsageTimeZone::system().stable_code(), "system");
    assert_eq!(UsageTimeZone::system().iana_name(), None);
    assert!(UsageTimeZone::iana("America/New_York").is_ok());
    assert_eq!(
        UsageTimeZone::iana("Missing/Zone")
            .expect_err("unknown zone")
            .code(),
        QueryErrorCode::InvalidTimeZone
    );

    let maximum = UsageRange::custom(date(2024, 1, 1), date(2025, 2, 4)).expect("400 daily points");
    assert_eq!(maximum.stable_code(), "custom");
    assert_eq!(
        UsageRange::custom(date(2024, 1, 1), date(2025, 2, 5))
            .expect_err("401 daily points")
            .code(),
        QueryErrorCode::CapacityExceeded
    );
    assert_eq!(
        UsageRange::custom(date(2024, 1, 1), date(2024, 1, 1))
            .expect_err("empty custom range")
            .code(),
        QueryErrorCode::InvalidValue
    );
}

#[test]
fn recent_days_is_bounded_and_has_a_stable_code() {
    assert_eq!(
        UsageRange::recent_days(30)
            .expect("30-day recent range")
            .stable_code(),
        "recent_days"
    );
    assert_eq!(
        UsageRange::recent_days(0)
            .expect_err("zero-day recent range")
            .code(),
        QueryErrorCode::InvalidValue
    );
    assert_eq!(
        UsageRange::recent_days(401)
            .expect_err("recent range over the series cap")
            .code(),
        QueryErrorCode::CapacityExceeded
    );
}

#[test]
fn analytics_requests_canonicalize_bounded_filters_and_breakdowns() {
    let request = UsageAnalyticsRequest::new(
        UsageRange::month(date(2024, 2, 15)),
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        vec![scope("z-provider", "default"), scope("a-provider", "work")],
        vec![UsageBreakdownKind::Profile, UsageBreakdownKind::Model],
    )
    .expect("request");
    assert_eq!(request.range().stable_code(), "month");
    assert_eq!(request.series(), UsageSeriesSelection::Daily);
    assert_eq!(request.series().stable_code(), "daily");
    assert_eq!(request.week_start(), WeekStart::Monday);
    assert_eq!(request.week_start().stable_code(), "monday");
    assert_eq!(request.scopes()[0].provider_id().as_str(), "a-provider");
    assert_eq!(
        request.breakdowns(),
        &[UsageBreakdownKind::Model, UsageBreakdownKind::Profile]
    );

    let duplicate_scope = UsageAnalyticsRequest::new(
        UsageRange::today(),
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Sunday,
        UsageSeriesSelection::None,
        vec![scope("codex", "default"), scope("codex", "default")],
        Vec::new(),
    )
    .expect_err("duplicate scope");
    assert_eq!(duplicate_scope.code(), QueryErrorCode::InvalidValue);

    let duplicate_breakdown = UsageAnalyticsRequest::new(
        UsageRange::day(date(2024, 1, 1)),
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        vec![UsageBreakdownKind::Model, UsageBreakdownKind::Model],
    )
    .expect_err("duplicate breakdown");
    assert_eq!(duplicate_breakdown.code(), QueryErrorCode::InvalidValue);
}

#[test]
fn token_availability_never_fabricates_missing_zeroes() {
    assert_eq!(
        AggregateTokenValue::Unavailable.stable_code(),
        "unavailable"
    );
    assert_eq!(AggregateTokenValue::Known(0).stable_code(), "known");
    assert_eq!(
        AggregateTokenValue::Partial {
            known_sum: 12,
            known_count: 2,
            event_count: 3,
        }
        .stable_code(),
        "partial"
    );
}
