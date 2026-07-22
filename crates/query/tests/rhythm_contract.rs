use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_domain::{UsageProfileId, UsageProviderId};
use tokenmaster_query::{
    QueryClock, QueryError, QueryScope, QueryService, QueryTimeSample, UsageAnalyticsRequest,
    UsageRange, UsageRhythmSelection, UsageSeriesSelection, UsageTimeZone, UsageWeekday, WeekStart,
};
use tokenmaster_store::UsageStore;

const SOURCE_KEY: [u8; 32] = [9; 32];

#[derive(Clone, Copy)]
struct FixedClock(i64);

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(self.0, 1))
    }
}

fn rhythm_request(day_count: u16, zone: &str) -> UsageAnalyticsRequest {
    UsageAnalyticsRequest::new(
        UsageRange::recent_days(day_count).expect("bounded recent range"),
        UsageTimeZone::iana(zone).expect("fixture timezone"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        Vec::new(),
    )
    .expect("analytics request")
    .with_rhythm(UsageRhythmSelection::HourAndWeekday)
    .expect("bounded rhythm")
}

fn timestamp_ms(value: &str) -> i64 {
    value
        .parse::<jiff::Timestamp>()
        .expect("fixture timestamp")
        .as_millisecond()
}

fn seed_current_event(path: &Path, timestamp_seconds: i64) {
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
                SOURCE_KEY.as_slice(),
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
             ) VALUES (1, 1000, 1710158400000, 'complete', 1);
             INSERT INTO usage_scan(
               scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
               completed_at_ms, completion_state
             ) VALUES (1, 1, 'codex', 'default', 1000, 1710158400000, 'complete');
             INSERT INTO usage_replay_revision(
               revision_id, status, canonicalizer_version, fingerprint_version,
               replay_signature_version, expected_source_count, evidence_epoch,
               sealed, promoted, scan_set_id
             ) VALUES (0, 'current', 1, 2, 1, 1, 1, 1, 1, 1);
             UPDATE usage_archive_state
             SET archive_generation = 4, current_revision_id = 0,
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
               timestamp_seconds, timestamp_nanos, model, input_tokens, cached_tokens,
               output_tokens, reasoning_tokens, total_tokens, fallback_model, long_context,
               activity_read, activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents, activity_terminal
             ) VALUES (
               ?1, 'event-rhythm', ?2, 0, 0, 0, 0, 0, 'codex', 'default',
               'session', 'private-source', ?3, 0, 'gpt-5.6', 10, 2, 3, 1, 16,
               0, 'no', 1, 0, 0, 0, 0, 0, 0, 0
             )",
            params![
                [1_u8; 32].as_slice(),
                SOURCE_KEY.as_slice(),
                timestamp_seconds
            ],
        )
        .expect("event");
    transaction.commit().expect("commit fixture");
}

#[test]
fn empty_utc_rhythm_has_canonical_bounded_rows_and_exposure() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("rhythm.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service =
        QueryService::open(&path, FixedClock(1_710_158_400_000)).expect("query service");

    let envelope = service
        .usage_analytics(rhythm_request(1, "UTC"))
        .expect("rhythm analytics");
    let rhythm = envelope.payload().rhythm().expect("requested rhythm");

    assert_eq!(rhythm.hours().len(), 24);
    assert_eq!(rhythm.weekdays().len(), 7);
    assert_eq!(rhythm.hours()[0].hour(), 0);
    assert_eq!(rhythm.hours()[23].hour(), 23);
    assert_eq!(rhythm.weekdays()[0].weekday(), UsageWeekday::Monday);
    assert_eq!(rhythm.weekdays()[6].weekday(), UsageWeekday::Sunday);
    assert!(
        rhythm
            .hours()
            .iter()
            .all(|bucket| bucket.elapsed_minutes() == 60)
    );
    assert!(
        rhythm
            .hours()
            .iter()
            .all(|bucket| bucket.occurrence_count() == 1)
    );
    assert_eq!(
        rhythm
            .weekdays()
            .iter()
            .map(|bucket| bucket.elapsed_minutes())
            .sum::<u64>(),
        24 * 60
    );
}

#[test]
fn new_york_spring_gap_and_fall_fold_have_exact_exposure() {
    let directory = TempDir::new().expect("temporary directory");
    let spring_path = directory.path().join("rhythm-spring.sqlite3");
    drop(UsageStore::open(&spring_path).expect("spring archive"));
    let mut spring =
        QueryService::open(&spring_path, FixedClock(1_710_158_400_000)).expect("spring service");
    let spring_rhythm = spring
        .usage_analytics(rhythm_request(3, "America/New_York"))
        .expect("spring rhythm");
    let spring_hour_two = &spring_rhythm.payload().rhythm().expect("rhythm").hours()[2];
    assert_eq!(spring_hour_two.elapsed_minutes(), 120);
    assert_eq!(spring_hour_two.occurrence_count(), 2);

    let fall_path = directory.path().join("rhythm-fall.sqlite3");
    drop(UsageStore::open(&fall_path).expect("fall archive"));
    let mut fall =
        QueryService::open(&fall_path, FixedClock(1_730_764_800_000)).expect("fall service");
    let fall_rhythm = fall
        .usage_analytics(rhythm_request(3, "America/New_York"))
        .expect("fall rhythm");
    let fall_hour_one = &fall_rhythm.payload().rhythm().expect("rhythm").hours()[1];
    assert_eq!(fall_hour_one.elapsed_minutes(), 240);
    assert_eq!(fall_hour_one.occurrence_count(), 4);
}

#[test]
fn fractional_offset_day_still_exposes_each_local_hour_once() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("rhythm-kathmandu.sqlite3");
    drop(UsageStore::open(&path).expect("create archive"));
    let mut service =
        QueryService::open(&path, FixedClock(1_710_158_400_000)).expect("query service");

    let envelope = service
        .usage_analytics(rhythm_request(1, "Asia/Kathmandu"))
        .expect("rhythm analytics");
    let rhythm = envelope.payload().rhythm().expect("requested rhythm");

    assert!(
        rhythm
            .hours()
            .iter()
            .all(|bucket| bucket.elapsed_minutes() == 60)
    );
    assert!(
        rhythm
            .hours()
            .iter()
            .all(|bucket| bucket.occurrence_count() == 1)
    );
}

#[test]
fn materialized_event_is_folded_from_rollups_into_both_dimensions() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("rhythm-event.sqlite3");
    seed_current_event(&path, 1_710_054_000);
    let mut service =
        QueryService::open(&path, FixedClock(1_710_158_400_000)).expect("query service");

    let envelope = service
        .usage_analytics(rhythm_request(3, "America/New_York"))
        .expect("rhythm analytics");
    let analytics = envelope.payload();
    let rhythm = analytics.rhythm().expect("requested rhythm");

    assert_eq!(analytics.overview().event_count(), 1);
    assert_eq!(rhythm.hours()[3].metrics().event_count(), 1);
    assert_eq!(rhythm.weekdays()[6].metrics().event_count(), 1);
    assert_eq!(
        rhythm
            .hours()
            .iter()
            .map(|bucket| bucket.metrics().event_count())
            .sum::<u64>(),
        1
    );
    assert_eq!(
        rhythm
            .weekdays()
            .iter()
            .map(|bucket| bucket.metrics().event_count())
            .sum::<u64>(),
        1
    );
}

#[test]
fn rhythm_honors_the_existing_provider_profile_scope() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("rhythm-scope.sqlite3");
    seed_current_event(&path, 1_710_054_000);
    let mut service =
        QueryService::open(&path, FixedClock(1_710_158_400_000)).expect("query service");
    let request = UsageAnalyticsRequest::new(
        UsageRange::recent_days(3).expect("recent range"),
        UsageTimeZone::iana("America/New_York").expect("fixture timezone"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        vec![QueryScope::new(
            UsageProviderId::new("codex").expect("provider"),
            UsageProfileId::new("other").expect("profile"),
        )],
        Vec::new(),
    )
    .expect("analytics request")
    .with_rhythm(UsageRhythmSelection::HourAndWeekday)
    .expect("rhythm request");

    let envelope = service.usage_analytics(request).expect("scoped rhythm");
    let analytics = envelope.payload();
    assert_eq!(analytics.overview().event_count(), 0);
    let rhythm = analytics.rhythm().expect("rhythm");
    assert_eq!(
        rhythm
            .hours()
            .iter()
            .map(|bucket| bucket.metrics().event_count())
            .sum::<u64>(),
        0
    );
    assert_eq!(
        rhythm
            .weekdays()
            .iter()
            .map(|bucket| bucket.metrics().event_count())
            .sum::<u64>(),
        0
    );
}

#[test]
fn lord_howe_half_hour_fold_and_apia_skipped_date_are_explicit() {
    let directory = TempDir::new().expect("temporary directory");
    let lord_howe_path = directory.path().join("rhythm-lord-howe.sqlite3");
    drop(UsageStore::open(&lord_howe_path).expect("Lord Howe archive"));
    let mut lord_howe = QueryService::open(
        &lord_howe_path,
        FixedClock(timestamp_ms("2024-04-08T12:00:00Z")),
    )
    .expect("Lord Howe service");
    let lord_howe_envelope = lord_howe
        .usage_analytics(rhythm_request(2, "Australia/Lord_Howe"))
        .expect("Lord Howe rhythm");
    let hour_one = &lord_howe_envelope
        .payload()
        .rhythm()
        .expect("rhythm")
        .hours()[1];
    assert_eq!(hour_one.elapsed_minutes(), 150);
    assert_eq!(hour_one.occurrence_count(), 3);

    let apia_path = directory.path().join("rhythm-apia.sqlite3");
    drop(UsageStore::open(&apia_path).expect("Apia archive"));
    let mut apia = QueryService::open(&apia_path, FixedClock(timestamp_ms("2011-12-31T12:00:00Z")))
        .expect("Apia service");
    let apia_envelope = apia
        .usage_analytics(rhythm_request(3, "Pacific/Apia"))
        .expect("Apia rhythm");
    let apia_rhythm = apia_envelope.payload().rhythm().expect("rhythm");
    assert_eq!(
        apia_rhythm
            .hours()
            .iter()
            .map(|bucket| bucket.elapsed_minutes())
            .sum::<u64>(),
        48 * 60
    );
    assert_eq!(
        apia_rhythm
            .hours()
            .iter()
            .map(|bucket| u64::from(bucket.occurrence_count()))
            .sum::<u64>(),
        48
    );
}

#[test]
fn rhythm_rejects_a_range_larger_than_thirty_civil_days() {
    let request = UsageAnalyticsRequest::new(
        UsageRange::recent_days(31).expect("valid analytics range"),
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Monday,
        UsageSeriesSelection::Daily,
        Vec::new(),
        Vec::new(),
    )
    .expect("analytics request");

    assert!(
        request
            .with_rhythm(UsageRhythmSelection::HourAndWeekday)
            .is_err()
    );
}
