use core::fmt;

use jiff::{
    Span, Timestamp,
    civil::{Date, Weekday},
    tz::TimeZone,
};
use tokenmaster_store::{UsageAggregateBucketWidth, UsageAggregateRange, UsageAggregateSegment};

use crate::{QueryError, QueryErrorCode};

const MAX_TIME_ZONE_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CalendarDate {
    year: i16,
    month: u8,
    day: u8,
}

impl CalendarDate {
    pub fn new(year: i16, month: u8, day: u8) -> Result<Self, QueryError> {
        let value = Self { year, month, day };
        value.to_jiff()?;
        Ok(value)
    }

    #[must_use]
    pub const fn year(self) -> i16 {
        self.year
    }

    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }

    pub(crate) fn tomorrow(self) -> Result<Self, QueryError> {
        self.to_jiff()?
            .tomorrow()
            .map_err(|_error| QueryError::new(QueryErrorCode::Overflow))
            .and_then(Self::from_jiff)
    }

    pub(crate) fn days_until(self, end: Self) -> Result<i64, QueryError> {
        self.to_jiff()?
            .until(end.to_jiff()?)
            .map(|span| i64::from(span.get_days()))
            .map_err(|_error| QueryError::new(QueryErrorCode::Overflow))
    }

    fn from_jiff(value: Date) -> Result<Self, QueryError> {
        let month = u8::try_from(value.month())
            .map_err(|_error| QueryError::new(QueryErrorCode::Internal))?;
        let day = u8::try_from(value.day())
            .map_err(|_error| QueryError::new(QueryErrorCode::Internal))?;
        Ok(Self {
            year: value.year(),
            month,
            day,
        })
    }

    fn to_jiff(self) -> Result<Date, QueryError> {
        let month = i8::try_from(self.month)
            .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))?;
        let day = i8::try_from(self.day)
            .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))?;
        Date::new(self.year, month, day)
            .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum UsageTimeZoneValue {
    Iana(Box<str>),
    System,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageTimeZone(UsageTimeZoneValue);

impl UsageTimeZone {
    pub fn iana(value: &str) -> Result<Self, QueryError> {
        if value.is_empty()
            || value.len() > MAX_TIME_ZONE_ID_BYTES
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(QueryError::new(QueryErrorCode::InvalidTimeZone));
        }
        let zone = TimeZone::get(value)
            .map_err(|_error| QueryError::new(QueryErrorCode::InvalidTimeZone))?;
        let canonical = zone
            .iana_name()
            .ok_or_else(|| QueryError::new(QueryErrorCode::InvalidTimeZone))?;
        Ok(Self(UsageTimeZoneValue::Iana(canonical.into())))
    }

    #[must_use]
    pub const fn system() -> Self {
        Self(UsageTimeZoneValue::System)
    }

    #[must_use]
    pub const fn stable_code(&self) -> &'static str {
        match self.0 {
            UsageTimeZoneValue::Iana(_) => "iana",
            UsageTimeZoneValue::System => "system",
        }
    }

    #[must_use]
    pub fn iana_name(&self) -> Option<&str> {
        match &self.0 {
            UsageTimeZoneValue::Iana(name) => Some(name),
            UsageTimeZoneValue::System => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeekStart {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl WeekStart {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Monday => "monday",
            Self::Tuesday => "tuesday",
            Self::Wednesday => "wednesday",
            Self::Thursday => "thursday",
            Self::Friday => "friday",
            Self::Saturday => "saturday",
            Self::Sunday => "sunday",
        }
    }

    const fn to_jiff(self) -> Weekday {
        match self {
            Self::Monday => Weekday::Monday,
            Self::Tuesday => Weekday::Tuesday,
            Self::Wednesday => Weekday::Wednesday,
            Self::Thursday => Weekday::Thursday,
            Self::Friday => Weekday::Friday,
            Self::Saturday => Weekday::Saturday,
            Self::Sunday => Weekday::Sunday,
        }
    }
}

pub(crate) struct CalendarBoundaryResolver {
    canonical_id: Box<str>,
    zone: TimeZone,
}

impl fmt::Debug for CalendarBoundaryResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CalendarBoundaryResolver")
            .field("canonical_id", &self.canonical_id)
            .field("zone", &"[internal]")
            .finish()
    }
}

impl CalendarBoundaryResolver {
    pub(crate) fn new(requested: UsageTimeZone) -> Result<Self, QueryError> {
        Self::new_with_system(requested, || TimeZone::try_system().ok())
    }

    fn new_with_system<F>(requested: UsageTimeZone, resolve_system: F) -> Result<Self, QueryError>
    where
        F: FnOnce() -> Option<TimeZone>,
    {
        let (zone, unavailable_code) = match requested.0 {
            UsageTimeZoneValue::Iana(name) => (
                TimeZone::get(&name)
                    .map_err(|_error| QueryError::new(QueryErrorCode::InvalidTimeZone))?,
                QueryErrorCode::InvalidTimeZone,
            ),
            UsageTimeZoneValue::System => (
                resolve_system()
                    .ok_or_else(|| QueryError::new(QueryErrorCode::SystemTimeZoneUnavailable))?,
                QueryErrorCode::SystemTimeZoneUnavailable,
            ),
        };
        let canonical_id = zone
            .iana_name()
            .ok_or_else(|| QueryError::new(unavailable_code))?
            .into();
        Ok(Self { canonical_id, zone })
    }

    pub(crate) fn local_date_at_ms(&self, wall_time_ms: i64) -> Result<CalendarDate, QueryError> {
        let timestamp = Timestamp::from_millisecond(wall_time_ms)
            .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))?;
        CalendarDate::from_jiff(self.zone.to_datetime(timestamp).date())
    }

    pub(crate) fn canonical_id(&self) -> &str {
        &self.canonical_id
    }

    pub(crate) fn day(&self, date: CalendarDate) -> Result<CalendarBucket, QueryError> {
        let start = date.to_jiff()?;
        let end = start
            .tomorrow()
            .map_err(|_error| QueryError::new(QueryErrorCode::Overflow))?;
        self.bucket(start, end)
    }

    pub(crate) fn week_containing(
        &self,
        date: CalendarDate,
        week_start: WeekStart,
    ) -> Result<CalendarBucket, QueryError> {
        let start = date
            .to_jiff()?
            .tomorrow()
            .and_then(|tomorrow| tomorrow.nth_weekday(-1, week_start.to_jiff()))
            .map_err(|_error| QueryError::new(QueryErrorCode::Overflow))?;
        let end = start
            .checked_add(Span::new().days(7))
            .map_err(|_error| QueryError::new(QueryErrorCode::Overflow))?;
        self.bucket(start, end)
    }

    pub(crate) fn month(&self, date: CalendarDate) -> Result<CalendarBucket, QueryError> {
        let date = date.to_jiff()?;
        let start = date.first_of_month();
        let end = date
            .last_of_month()
            .tomorrow()
            .map_err(|_error| QueryError::new(QueryErrorCode::Overflow))?;
        self.bucket(start, end)
    }

    pub(crate) fn custom(
        &self,
        start: CalendarDate,
        end: CalendarDate,
    ) -> Result<CalendarBucket, QueryError> {
        let start = start.to_jiff()?;
        let end = end.to_jiff()?;
        if start >= end {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        self.bucket(start, end)
    }

    fn bucket(&self, start: Date, end: Date) -> Result<CalendarBucket, QueryError> {
        let start_timestamp = self
            .zone
            .to_timestamp(start.at(0, 0, 0, 0))
            .map_err(|_error| QueryError::new(QueryErrorCode::UnsupportedTimeBoundary))?;
        let end_timestamp = self
            .zone
            .to_timestamp(end.at(0, 0, 0, 0))
            .map_err(|_error| QueryError::new(QueryErrorCode::UnsupportedTimeBoundary))?;
        if start_timestamp.subsec_nanosecond() != 0 || end_timestamp.subsec_nanosecond() != 0 {
            return Err(QueryError::new(QueryErrorCode::UnsupportedTimeBoundary));
        }
        let start_seconds = start_timestamp.as_second();
        let end_seconds = end_timestamp.as_second();
        if start_seconds > end_seconds
            || start_seconds.rem_euclid(60) != 0
            || end_seconds.rem_euclid(60) != 0
        {
            return Err(QueryError::new(QueryErrorCode::UnsupportedTimeBoundary));
        }

        let segments = compose_segments(start_seconds, end_seconds)?;
        Ok(CalendarBucket {
            start_date: CalendarDate::from_jiff(start)?,
            end_date: CalendarDate::from_jiff(end)?,
            start_seconds,
            end_seconds,
            segments,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CalendarBucket {
    start_date: CalendarDate,
    end_date: CalendarDate,
    start_seconds: i64,
    end_seconds: i64,
    segments: Box<[UsageAggregateSegment]>,
}

impl CalendarBucket {
    pub(crate) const fn start_date(&self) -> CalendarDate {
        self.start_date
    }

    pub(crate) const fn end_date(&self) -> CalendarDate {
        self.end_date
    }

    pub(crate) const fn start_seconds(&self) -> i64 {
        self.start_seconds
    }

    pub(crate) const fn end_seconds(&self) -> i64 {
        self.end_seconds
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub(crate) fn into_store_range(self) -> Result<UsageAggregateRange, QueryError> {
        if self.is_empty() {
            UsageAggregateRange::empty(self.start_seconds)
        } else {
            UsageAggregateRange::new(self.segments)
        }
        .map_err(|_error| QueryError::new(QueryErrorCode::Internal))
    }

    pub(crate) fn into_store_series_point(
        self,
    ) -> Result<tokenmaster_store::UsageSeriesPoint, QueryError> {
        if self.is_empty() {
            tokenmaster_store::UsageSeriesPoint::empty(self.start_seconds)
        } else {
            tokenmaster_store::UsageSeriesPoint::new(self.segments)
        }
        .map_err(|_error| QueryError::new(QueryErrorCode::Internal))
    }
}

fn compose_segments(
    start_seconds: i64,
    end_seconds: i64,
) -> Result<Box<[UsageAggregateSegment]>, QueryError> {
    if start_seconds == end_seconds {
        return Ok(Box::default());
    }
    let start_remainder = start_seconds.rem_euclid(3_600);
    let middle_start = if start_remainder == 0 {
        start_seconds
    } else {
        start_seconds
            .checked_add(3_600 - start_remainder)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?
    };
    let middle_end = end_seconds
        .checked_sub(end_seconds.rem_euclid(3_600))
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;

    let mut segments = Vec::with_capacity(3);
    if middle_start >= middle_end {
        push_segment(
            &mut segments,
            UsageAggregateBucketWidth::Minute,
            start_seconds,
            end_seconds,
        )?;
    } else {
        if start_seconds < middle_start {
            push_segment(
                &mut segments,
                UsageAggregateBucketWidth::Minute,
                start_seconds,
                middle_start,
            )?;
        }
        push_segment(
            &mut segments,
            UsageAggregateBucketWidth::Hour,
            middle_start,
            middle_end,
        )?;
        if middle_end < end_seconds {
            push_segment(
                &mut segments,
                UsageAggregateBucketWidth::Minute,
                middle_end,
                end_seconds,
            )?;
        }
    }
    Ok(segments.into_boxed_slice())
}

fn push_segment(
    segments: &mut Vec<UsageAggregateSegment>,
    bucket_width: UsageAggregateBucketWidth,
    start_seconds: i64,
    end_seconds: i64,
) -> Result<(), QueryError> {
    let segment = UsageAggregateSegment::new(bucket_width, start_seconds, end_seconds)
        .map_err(|_error| QueryError::new(QueryErrorCode::Internal))?;
    segments.push(segment);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QueryErrorCode;

    fn require<T>(result: Result<T, QueryError>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("expected success, received {error}"),
        }
    }

    fn require_error<T>(result: Result<T, QueryError>) -> QueryError {
        match result {
            Ok(_value) => panic!("expected failure"),
            Err(error) => error,
        }
    }

    fn date(year: i16, month: u8, day: u8) -> CalendarDate {
        require(CalendarDate::new(year, month, day))
    }

    #[test]
    fn utc_day_week_month_and_custom_ranges_are_exact() {
        let resolver = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "UTC",
        ))));
        let day = require(resolver.day(date(2024, 2, 29)));
        assert_eq!(day.end_seconds() - day.start_seconds(), 86_400);
        assert_eq!(day.segment_count(), 1);

        let week = require(resolver.week_containing(date(2024, 3, 15), WeekStart::Monday));
        assert_eq!(week.end_seconds() - week.start_seconds(), 7 * 86_400);
        assert_eq!(week.start_date(), date(2024, 3, 11));

        let month = require(resolver.month(date(2024, 2, 15)));
        assert_eq!(month.start_date(), date(2024, 2, 1));
        assert_eq!(month.end_date(), date(2024, 3, 1));
        assert_eq!(month.end_seconds() - month.start_seconds(), 29 * 86_400);

        let custom = require(resolver.custom(date(2023, 12, 31), date(2024, 1, 2)));
        assert_eq!(custom.end_seconds() - custom.start_seconds(), 2 * 86_400);

        let december = require(resolver.month(date(2023, 12, 20)));
        assert_eq!(december.start_date(), date(2023, 12, 1));
        assert_eq!(december.end_date(), date(2024, 1, 1));
    }

    #[test]
    fn every_configurable_week_start_is_exact() {
        let resolver = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "UTC",
        ))));
        for (week_start, expected_start) in [
            (WeekStart::Monday, date(2024, 3, 11)),
            (WeekStart::Tuesday, date(2024, 3, 12)),
            (WeekStart::Wednesday, date(2024, 3, 13)),
            (WeekStart::Thursday, date(2024, 3, 7)),
            (WeekStart::Friday, date(2024, 3, 8)),
            (WeekStart::Saturday, date(2024, 3, 9)),
            (WeekStart::Sunday, date(2024, 3, 10)),
        ] {
            let week = require(resolver.week_containing(date(2024, 3, 13), week_start));
            assert_eq!(week.start_date(), expected_start);
            assert_eq!(week.end_seconds() - week.start_seconds(), 7 * 86_400);
        }
    }

    #[test]
    fn dst_and_fractional_offset_days_compose_exact_utc_segments() {
        let new_york = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "America/New_York",
        ))));
        let spring = require(new_york.day(date(2024, 3, 10)));
        assert_eq!(spring.end_seconds() - spring.start_seconds(), 23 * 3_600);
        assert_eq!(spring.segment_count(), 1);
        let fall = require(new_york.day(date(2024, 11, 3)));
        assert_eq!(fall.end_seconds() - fall.start_seconds(), 25 * 3_600);
        assert_eq!(fall.segment_count(), 1);

        let kathmandu = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "Asia/Kathmandu",
        ))));
        let quarter_hour = require(kathmandu.day(date(2024, 1, 1)));
        assert_eq!(
            quarter_hour.end_seconds() - quarter_hour.start_seconds(),
            24 * 3_600
        );
        assert_eq!(quarter_hour.segment_count(), 3);

        let lord_howe = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "Australia/Lord_Howe",
        ))));
        let half_hour_fall = require(lord_howe.day(date(2024, 4, 7)));
        assert_eq!(
            half_hour_fall.end_seconds() - half_hour_fall.start_seconds(),
            24 * 3_600 + 1_800
        );
        assert_eq!(half_hour_fall.segment_count(), 2);

        let jerusalem = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "Asia/Jerusalem",
        ))));
        let spring = require(jerusalem.day(date(2024, 3, 29)));
        assert_eq!(spring.end_seconds() - spring.start_seconds(), 23 * 3_600);
        let fall = require(jerusalem.day(date(2024, 10, 27)));
        assert_eq!(fall.end_seconds() - fall.start_seconds(), 25 * 3_600);
    }

    #[test]
    fn skipped_civil_date_is_an_explicit_zero_duration_bucket() {
        let apia = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "Pacific/Apia",
        ))));
        let skipped = require(apia.day(date(2011, 12, 30)));
        assert!(skipped.is_empty());
        assert_eq!(skipped.start_seconds(), skipped.end_seconds());
        assert_eq!(skipped.segment_count(), 0);
        require(skipped.into_store_range());
    }

    #[test]
    fn invalid_unknown_and_failed_system_zones_never_fall_back_to_utc() {
        for invalid in ["", " UTC", "UTC\n", "Etc/Unknown", "Missing/Zone"] {
            let error = require_error(UsageTimeZone::iana(invalid));
            assert_eq!(error.code(), QueryErrorCode::InvalidTimeZone);
        }
        let failed = require_error(CalendarBoundaryResolver::new_with_system(
            UsageTimeZone::system(),
            || None,
        ));
        assert_eq!(failed.code(), QueryErrorCode::SystemTimeZoneUnavailable);

        let resolved = require(CalendarBoundaryResolver::new_with_system(
            UsageTimeZone::system(),
            || TimeZone::get("Asia/Jerusalem").ok(),
        ));
        assert_eq!(resolved.canonical_id(), "Asia/Jerusalem");
        assert_eq!(require(resolved.local_date_at_ms(0)), date(1970, 1, 1));

        let canonical = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "america/new_york",
        ))));
        assert_eq!(canonical.canonical_id(), "America/New_York");
    }

    #[test]
    fn historical_non_minute_boundary_fails_instead_of_rounding() {
        let monrovia = require(CalendarBoundaryResolver::new(require(UsageTimeZone::iana(
            "Africa/Monrovia",
        ))));
        let error = require_error(monrovia.day(date(1972, 1, 7)));
        assert_eq!(error.code(), QueryErrorCode::UnsupportedTimeBoundary);
    }
}
