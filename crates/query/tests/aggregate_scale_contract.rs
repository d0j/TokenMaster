use std::{path::Path, time::Duration, time::Instant};

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_domain::{UsageProfileId, UsageProviderId};
use tokenmaster_query::{
    CalendarDate, CostAvailability, DatasetGeneration, DatasetIdentity, PageSize, QueryClock,
    QueryError, QueryScope, QueryService, QueryTimeSample, ReplayRevision, UsageAnalyticsRequest,
    UsageBreakdownKind, UsageRange, UsageSeriesSelection, UsageSessionPageRequest, UsageTimeZone,
    WeekStart,
};
use tokenmaster_store::{AggregateRebuildStatus, MAX_AGGREGATE_REBUILD_PAGE_SIZE, UsageStore};

const EVENT_COUNT: i64 = 1_000_000;
const SOURCE_KEY: [u8; 32] = [7; 32];
const START_SECONDS: i64 = 1_704_067_200;
const SAMPLE_COUNT: usize = 20;
const FULL_SAMPLE_COUNT: usize = 5;
const COLD_OVERVIEW_BUDGET: Duration = Duration::from_secs(1);
const CACHED_OVERVIEW_P95_BUDGET: Duration = Duration::from_millis(250);
const FULL_ANALYTICS_P95_BUDGET: Duration = Duration::from_secs(1);
const SESSION_PAGE_P95_BUDGET: Duration = Duration::from_millis(100);
const SESSION_DETAIL_P95_BUDGET: Duration = Duration::from_millis(100);
const REBUILD_PAGE_P95_BUDGET: Duration = Duration::from_millis(500);
const MIN_REBUILD_EVENTS_PER_SECOND: f64 = 5_000.0;
const MAX_DATABASE_AMPLIFICATION: f64 = 3.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureKind {
    Current,
    Legacy,
}

impl FixtureKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Legacy => "legacy",
        }
    }
}

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_738_627_200_000, 1))
    }
}

fn checkpoint(connection: &Connection) {
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint");
}

fn sqlite_resident_bytes(path: &Path) -> u64 {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let mut candidate = path.as_os_str().to_os_string();
            candidate.push(suffix);
            std::fs::metadata(candidate).map_or(0, |metadata| metadata.len())
        })
        .sum()
}

fn percentile_95(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    samples[(samples.len() * 95).div_ceil(100) - 1]
}

fn seed_publication(transaction: &rusqlite::Transaction<'_>, kind: FixtureKind) {
    transaction
        .execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, physical_identity, missing
             ) VALUES (?1, 'codex', 'default', 'scale-private-source', 'active', ?2, ?3, 0)",
            params![
                SOURCE_KEY.as_slice(),
                [2_u8; 32].as_slice(),
                [3_u8; 32].as_slice()
            ],
        )
        .expect("source");
    if kind == FixtureKind::Current {
        transaction
            .execute_batch(
                "INSERT INTO usage_scan_set(
                   scan_set_id, started_at_ms, completed_at_ms, completion_state,
                   expected_scope_count
                 ) VALUES (1, 1, 1738627200000, 'complete', 1);
                 INSERT INTO usage_scan(
                   scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
                   completed_at_ms, completion_state
                 ) VALUES (1, 1, 'codex', 'default', 1, 1738627200000, 'complete');
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
            .expect("current publication");
    }
}

fn seed_current_events(transaction: &rusqlite::Transaction<'_>) {
    transaction
        .execute(
            "WITH RECURSIVE series(value) AS (
               VALUES(1)
               UNION ALL
               SELECT value + 1 FROM series WHERE value < ?1
             )
             INSERT INTO usage_event(
               fingerprint, event_id, selected_file_key, selected_generation,
               selected_source_offset, projection_revision_id, origin_revision_id,
               retained, provider_id, profile_id, session_id, source_id,
               timestamp_seconds, timestamp_nanos, model, input_tokens,
               cached_tokens, output_tokens, reasoning_tokens, total_tokens,
               fallback_model, long_context, project_alias, activity_read,
               activity_edit_write, activity_search, activity_git,
               activity_build_test, activity_web, activity_subagents,
               activity_terminal
             )
             SELECT unhex(printf('%064x', value)), 'event-' || value, ?2, 0, value,
                    0, 0, 0, 'codex', 'default',
                    'private-session-' || ((value - 1) / 16), 'scale-private-source',
                    ?3 + value - 1, 0, 'gpt-scale-' || (value % 8),
                    10, NULL, 2, NULL, 12, 0, 'no', 'project-' || (value % 8),
                    1, 0, 0, 0, 0, 0, 0, 0
             FROM series",
            params![EVENT_COUNT, SOURCE_KEY.as_slice(), START_SECONDS],
        )
        .expect("current events");
}

fn seed_legacy_events(transaction: &rusqlite::Transaction<'_>) {
    transaction
        .execute_batch(
            "DROP TRIGGER usage_legacy_event_no_insert;
             DROP TRIGGER usage_legacy_event_no_update;
             DROP TRIGGER usage_legacy_event_no_delete;",
        )
        .expect("open immutable fixture boundary");
    transaction
        .execute(
            "INSERT INTO usage_legacy_snapshot(
               snapshot_id, source_schema_version, quality_state, event_count
             ) VALUES (1, 1, 'legacy_unverified', ?1)",
            [EVENT_COUNT],
        )
        .expect("legacy snapshot");
    transaction
        .execute(
            "WITH RECURSIVE series(value) AS (
               VALUES(1)
               UNION ALL
               SELECT value + 1 FROM series WHERE value < ?1
             )
             INSERT INTO usage_legacy_event(
               snapshot_id, fingerprint, event_id, selected_file_key,
               selected_generation, selected_source_offset, profile_id,
               session_id, source_id, timestamp_seconds, timestamp_nanos,
               model, input_tokens, cached_tokens, output_tokens,
               reasoning_tokens, total_tokens, fallback_model, long_context,
               project_alias, activity_read, activity_edit_write,
               activity_search, activity_git, activity_build_test, activity_web,
               activity_subagents, activity_terminal
             )
             SELECT 1, unhex(printf('%064x', value)), 'event-' || value, ?2, 0, value,
                    'default', 'private-session-' || ((value - 1) / 16),
                    'scale-private-source', ?3 + value - 1, 0,
                    'gpt-scale-' || (value % 8), 10, NULL, 2, NULL, 12, 0, 'no',
                    'project-' || (value % 8), 1, 0, 0, 0, 0, 0, 0, 0
             FROM series",
            params![EVENT_COUNT, SOURCE_KEY.as_slice(), START_SECONDS],
        )
        .expect("legacy events");
    transaction
        .execute_batch(
            "CREATE TRIGGER usage_legacy_event_no_insert
             BEFORE INSERT ON usage_legacy_event
             BEGIN
               SELECT RAISE(ABORT, 'immutable legacy snapshot');
             END;
             CREATE TRIGGER usage_legacy_event_no_update
             BEFORE UPDATE ON usage_legacy_event
             BEGIN
               SELECT RAISE(ABORT, 'immutable legacy snapshot');
             END;
             CREATE TRIGGER usage_legacy_event_no_delete
             BEFORE DELETE ON usage_legacy_event
             BEGIN
               SELECT RAISE(ABORT, 'immutable legacy snapshot');
             END;
             UPDATE usage_aggregate_state
             SET state = 'rebuild_required', legacy_event_count = 1000000,
                 rebuild_total_events = 1000000
             WHERE singleton_id = 1;",
        )
        .expect("restore immutable fixture boundary");
}

fn seed_one_million_archive(path: &Path, kind: FixtureKind) -> (Duration, u64) {
    drop(UsageStore::open(path).expect("create archive"));
    let mut connection = Connection::open(path).expect("fixture connection");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    connection
        .pragma_update(None, "synchronous", "OFF")
        .expect("fixture synchronous mode");
    let started = Instant::now();
    let transaction = connection.transaction().expect("fixture transaction");
    seed_publication(&transaction, kind);
    if kind == FixtureKind::Current {
        transaction
            .execute(
                "UPDATE usage_aggregate_state
                 SET state = 'rebuild_required'
                 WHERE singleton_id = 1",
                [],
            )
            .expect("disable trigger materialization");
        seed_current_events(&transaction);
    } else {
        seed_legacy_events(&transaction);
    }
    transaction.commit().expect("commit fixture");
    checkpoint(&connection);
    let elapsed = started.elapsed();
    drop(connection);
    (elapsed, sqlite_resident_bytes(path))
}

fn rebuild_one_million(path: &Path) -> (Duration, f64, usize, Duration) {
    let started = Instant::now();
    let mut calls = 0_usize;
    let mut page_samples = Vec::new();
    let mut status = AggregateRebuildStatus::Rebuilding;
    while status != AggregateRebuildStatus::Ready {
        let mut store = UsageStore::open(path).expect("open resumable rebuild");
        for _ in 0..257 {
            let page_started = Instant::now();
            let progress = store
                .rebuild_aggregates_page(MAX_AGGREGATE_REBUILD_PAGE_SIZE)
                .expect("rebuild aggregate page");
            page_samples.push(page_started.elapsed());
            calls = calls.checked_add(1).expect("bounded rebuild calls");
            status = progress.status();
            if status == AggregateRebuildStatus::Ready {
                assert_eq!(progress.processed_events(), EVENT_COUNT as u64);
                break;
            }
        }
    }
    let elapsed = started.elapsed();
    let throughput = EVENT_COUNT as f64 / elapsed.as_secs_f64();
    let page_p95 = percentile_95(&mut page_samples);
    (elapsed, throughput, calls, page_p95)
}

fn date(year: i16, month: u8, day: u8) -> CalendarDate {
    CalendarDate::new(year, month, day).expect("valid fixture date")
}

fn analytics_request(
    series: UsageSeriesSelection,
    scopes: Vec<QueryScope>,
    breakdowns: Vec<UsageBreakdownKind>,
) -> UsageAnalyticsRequest {
    UsageAnalyticsRequest::new(
        UsageRange::custom(date(2024, 1, 1), date(2025, 2, 4)).expect("400-day range"),
        UsageTimeZone::iana("UTC").expect("UTC"),
        WeekStart::Monday,
        series,
        scopes,
        breakdowns,
    )
    .expect("analytics request")
}

fn maximum_scopes() -> Vec<QueryScope> {
    let mut scopes = vec![QueryScope::new(
        UsageProviderId::new("codex").expect("provider"),
        UsageProfileId::new("default").expect("profile"),
    )];
    for index in 1..32 {
        scopes.push(QueryScope::new(
            UsageProviderId::new(format!("provider-{index:02}")).expect("provider"),
            UsageProfileId::new(format!("profile-{index:02}")).expect("profile"),
        ));
    }
    scopes
}

#[test]
#[ignore = "reference-machine one-million-event aggregate gate; run explicitly in release mode"]
fn current_and_legacy_one_million_event_aggregates_meet_reference_budgets() {
    for kind in [FixtureKind::Current, FixtureKind::Legacy] {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory
            .path()
            .join(format!("aggregate-scale-{}.sqlite3", kind.as_str()));
        let (seed_elapsed, raw_bytes) = seed_one_million_archive(&path, kind);
        let (rebuild_elapsed, rebuild_events_per_second, rebuild_calls, rebuild_page_p95) =
            rebuild_one_million(&path);
        assert!(
            rebuild_events_per_second >= MIN_REBUILD_EVENTS_PER_SECOND,
            "{} rebuild throughput {:.0} events/s is below {:.0} events/s",
            kind.as_str(),
            rebuild_events_per_second,
            MIN_REBUILD_EVENTS_PER_SECOND
        );
        assert!(
            rebuild_page_p95 < REBUILD_PAGE_P95_BUDGET,
            "{} rebuild page p95 {rebuild_page_p95:?} exceeded {REBUILD_PAGE_P95_BUDGET:?}",
            kind.as_str()
        );

        let connection = Connection::open(&path).expect("checkpoint connection");
        checkpoint(&connection);
        let row_counts: (i64, i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                   CASE ?1 WHEN 'current' THEN (SELECT count(*) FROM usage_event)
                           ELSE (SELECT count(*) FROM usage_legacy_event WHERE snapshot_id = 1)
                   END,
                   (SELECT count(*) FROM usage_time_rollup),
                   (SELECT count(*) FROM usage_session_rollup),
                   (SELECT count(*) FROM usage_price_time_rollup),
                   (SELECT count(*) FROM usage_price_session_rollup)",
                [kind.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("fixture row counts");
        assert_eq!(row_counts.0, EVENT_COUNT);
        assert!(row_counts.3 > 0);
        assert!(row_counts.4 > 0);
        drop(connection);
        let final_bytes = sqlite_resident_bytes(&path);
        let database_amplification = final_bytes as f64 / raw_bytes as f64;
        assert!(
            database_amplification <= MAX_DATABASE_AMPLIFICATION,
            "{} database amplification {:.3} exceeded {:.3}",
            kind.as_str(),
            database_amplification,
            MAX_DATABASE_AMPLIFICATION
        );

        let overview_request =
            analytics_request(UsageSeriesSelection::None, Vec::new(), Vec::new());
        let cold_started = Instant::now();
        let mut service = QueryService::open(&path, FixedClock).expect("open cold query service");
        let cold = service
            .usage_analytics(overview_request.clone())
            .expect("cold overview");
        let cold_elapsed = cold_started.elapsed();
        assert_eq!(cold.payload().overview().event_count(), EVENT_COUNT as u64);
        assert_eq!(
            cold.payload().overview_cost().availability(),
            CostAvailability::Unavailable
        );
        assert_eq!(
            cold.payload().overview_cost().counters().total_events,
            EVENT_COUNT as u64
        );
        assert_eq!(
            cold.header().dataset_identity(),
            match kind {
                FixtureKind::Current => DatasetIdentity::ReplayRevision {
                    revision: ReplayRevision::new(0).expect("revision"),
                    dataset_generation: DatasetGeneration::new(EVENT_COUNT as u64)
                        .expect("dataset generation"),
                },
                FixtureKind::Legacy => DatasetIdentity::LegacySnapshotV1,
            }
        );
        assert!(
            cold_elapsed < COLD_OVERVIEW_BUDGET,
            "{} cold overview {cold_elapsed:?} exceeded {COLD_OVERVIEW_BUDGET:?}",
            kind.as_str()
        );

        let mut overview_samples = Vec::with_capacity(SAMPLE_COUNT);
        for _ in 0..SAMPLE_COUNT {
            let started = Instant::now();
            let snapshot = service
                .usage_analytics(overview_request.clone())
                .expect("cached overview");
            overview_samples.push(started.elapsed());
            assert_eq!(
                snapshot.payload().overview().event_count(),
                EVENT_COUNT as u64
            );
            assert_eq!(
                snapshot.payload().overview_cost().counters().total_events,
                EVENT_COUNT as u64
            );
        }
        let overview_p95 = percentile_95(&mut overview_samples);
        assert!(
            overview_p95 < CACHED_OVERVIEW_P95_BUDGET,
            "{} cached overview p95 {overview_p95:?} exceeded {CACHED_OVERVIEW_P95_BUDGET:?}",
            kind.as_str()
        );

        let full_request = analytics_request(
            UsageSeriesSelection::Daily,
            Vec::new(),
            vec![
                UsageBreakdownKind::Model,
                UsageBreakdownKind::Project,
                UsageBreakdownKind::Provider,
                UsageBreakdownKind::Profile,
            ],
        );
        let mut full_samples = Vec::with_capacity(FULL_SAMPLE_COUNT);
        for _ in 0..FULL_SAMPLE_COUNT {
            let started = Instant::now();
            let snapshot = service
                .usage_analytics(full_request.clone())
                .expect("full analytics");
            full_samples.push(started.elapsed());
            assert_eq!(snapshot.payload().series().len(), 400);
            assert_eq!(snapshot.payload().breakdowns().len(), 4);
            assert_eq!(
                snapshot.payload().overview_cost().counters().total_events,
                EVENT_COUNT as u64
            );
            assert!(snapshot.payload().breakdowns().iter().all(|breakdown| {
                breakdown
                    .items()
                    .iter()
                    .map(|item| item.cost().counters().total_events)
                    .sum::<u64>()
                    == EVENT_COUNT as u64
            }));
        }
        let full_p95 = percentile_95(&mut full_samples);
        assert!(
            full_p95 < FULL_ANALYTICS_P95_BUDGET,
            "{} full analytics p95 {full_p95:?} exceeded {FULL_ANALYTICS_P95_BUDGET:?}",
            kind.as_str()
        );

        let scoped_started = Instant::now();
        let scoped = service
            .usage_analytics(analytics_request(
                UsageSeriesSelection::Daily,
                maximum_scopes(),
                vec![
                    UsageBreakdownKind::Model,
                    UsageBreakdownKind::Project,
                    UsageBreakdownKind::Provider,
                    UsageBreakdownKind::Profile,
                ],
            ))
            .expect("maximum-scope analytics");
        let scoped_elapsed = scoped_started.elapsed();
        assert_eq!(
            scoped.payload().overview().event_count(),
            EVENT_COUNT as u64
        );
        assert!(
            scoped_elapsed < FULL_ANALYTICS_P95_BUDGET,
            "{} maximum-scope analytics {scoped_elapsed:?} exceeded {FULL_ANALYTICS_P95_BUDGET:?}",
            kind.as_str()
        );

        let first_request =
            UsageSessionPageRequest::first(PageSize::new(256).expect("page size"), Vec::new())
                .expect("first page request");
        let mut first_samples = Vec::with_capacity(SAMPLE_COUNT);
        let mut first_page = None;
        for _ in 0..SAMPLE_COUNT {
            let started = Instant::now();
            let page = service
                .usage_sessions(first_request.clone())
                .expect("first session page");
            first_samples.push(started.elapsed());
            assert_eq!(page.payload().sessions().len(), 256);
            assert!(page.payload().has_more());
            assert_eq!(
                page.payload().sessions()[0].cost().counters().total_events,
                16
            );
            first_page = Some(page);
        }
        let first_p95 = percentile_95(&mut first_samples);
        assert!(
            first_p95 < SESSION_PAGE_P95_BUDGET,
            "{} first session p95 {first_p95:?} exceeded {SESSION_PAGE_P95_BUDGET:?}",
            kind.as_str()
        );
        let cursor = first_page
            .as_ref()
            .expect("measured first page")
            .payload()
            .next_cursor()
            .expect("session cursor")
            .clone();
        let mut cursor_samples = Vec::with_capacity(SAMPLE_COUNT);
        for _ in 0..SAMPLE_COUNT {
            let request = UsageSessionPageRequest::continuation(
                PageSize::new(256).expect("page size"),
                cursor.clone(),
                Vec::new(),
            )
            .expect("cursor request");
            let started = Instant::now();
            let page = service
                .usage_sessions(request)
                .expect("cursor session page");
            cursor_samples.push(started.elapsed());
            assert_eq!(page.payload().sessions().len(), 256);
        }
        let cursor_p95 = percentile_95(&mut cursor_samples);
        assert!(
            cursor_p95 < SESSION_PAGE_P95_BUDGET,
            "{} cursor session p95 {cursor_p95:?} exceeded {SESSION_PAGE_P95_BUDGET:?}",
            kind.as_str()
        );

        let detail_key = first_page
            .as_ref()
            .expect("measured first page")
            .payload()
            .sessions()[0]
            .key()
            .clone();
        let mut detail_samples = Vec::with_capacity(SAMPLE_COUNT);
        for _ in 0..SAMPLE_COUNT {
            let started = Instant::now();
            let detail = service
                .usage_session_detail(detail_key.clone())
                .expect("session detail");
            detail_samples.push(started.elapsed());
            let detail = detail.payload().detail().expect("existing session");
            assert_eq!(detail.summary().cost().counters().total_events, 16);
            assert!(detail.breakdowns().iter().all(|breakdown| {
                breakdown
                    .items()
                    .iter()
                    .map(|item| item.cost().counters().total_events)
                    .sum::<u64>()
                    == 16
            }));
        }
        let detail_p95 = percentile_95(&mut detail_samples);
        assert!(
            detail_p95 < SESSION_DETAIL_P95_BUDGET,
            "{} session detail p95 {detail_p95:?} exceeded {SESSION_DETAIL_P95_BUDGET:?}",
            kind.as_str()
        );

        eprintln!(
            "P2-C scale dataset={} events={} seed_s={:.3} rebuild_s={:.3} rebuild_events_per_s={:.0} rebuild_calls={} rebuild_page_p95_ms={:.3} raw_bytes={} final_bytes={} amplification={:.3} time_rows={} session_rows={} price_time_rows={} price_session_rows={} cold_overview_ms={:.3} cached_overview_p95_ms={:.3} full_analytics_p95_ms={:.3} scoped_full_ms={:.3} session_first_p95_ms={:.3} session_cursor_p95_ms={:.3} session_detail_p95_ms={:.3}",
            kind.as_str(),
            EVENT_COUNT,
            seed_elapsed.as_secs_f64(),
            rebuild_elapsed.as_secs_f64(),
            rebuild_events_per_second,
            rebuild_calls,
            rebuild_page_p95.as_secs_f64() * 1_000.0,
            raw_bytes,
            final_bytes,
            database_amplification,
            row_counts.1,
            row_counts.2,
            row_counts.3,
            row_counts.4,
            cold_elapsed.as_secs_f64() * 1_000.0,
            overview_p95.as_secs_f64() * 1_000.0,
            full_p95.as_secs_f64() * 1_000.0,
            scoped_elapsed.as_secs_f64() * 1_000.0,
            first_p95.as_secs_f64() * 1_000.0,
            cursor_p95.as_secs_f64() * 1_000.0,
            detail_p95.as_secs_f64() * 1_000.0,
        );
    }
}
