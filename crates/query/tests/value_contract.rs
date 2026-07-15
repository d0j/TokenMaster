use std::sync::Arc;

use tokenmaster_domain::{ModelKey, TokenCount, TokenUsage, UsageProfileId, UsageProviderId};
use tokenmaster_query::{
    ActivityCursor, ActivityItem, ActivityItemParts, DatasetGeneration, DatasetIdentity,
    LatestActivityPage, MAX_QUERY_PAGE_SIZE, MAX_QUERY_SCOPES, MAX_QUERY_WARNINGS, PageSize,
    PublicationGeneration, QUERY_SCHEMA_VERSION, QueryClock, QueryEnvelope, QueryError,
    QueryErrorCode, QueryFreshness, QueryHeader, QueryHeaderParts, QueryQuality, QueryScope,
    QueryTimeSample, QueryWarningCode, ReplayRevision, SnapshotGeneration, SystemQueryClock,
};

fn scope(index: usize) -> QueryScope {
    QueryScope::new(
        UsageProviderId::new(format!("provider-{index}")).expect("valid provider"),
        UsageProfileId::new(format!("profile-{index}")).expect("valid profile"),
    )
}

fn item(index: usize) -> ActivityItem {
    ActivityItem::new(ActivityItemParts {
        scope: scope(index),
        event_id: format!("event-{index}"),
        timestamp_seconds: index as i64,
        timestamp_nanos: 0,
        model: ModelKey::new("gpt-5.6").expect("valid model"),
        usage: TokenUsage::new(
            TokenCount::Available(index as u64),
            TokenCount::Unavailable,
            TokenCount::Available(1),
            TokenCount::Unavailable,
            TokenCount::Available(index as u64 + 1),
        ),
        fingerprint: [index as u8; 32],
    })
    .expect("valid item")
}

#[test]
fn identity_and_generation_are_checked_and_ordered() {
    assert_eq!(QUERY_SCHEMA_VERSION, 1);
    assert_eq!(
        SnapshotGeneration::new(0)
            .expect_err("zero is invalid")
            .code(),
        QueryErrorCode::InvalidValue
    );
    let first = SnapshotGeneration::new(1).expect("first generation");
    let second = first.checked_next().expect("next generation");
    assert!(second.is_newer_than(Some(first)));
    assert!(!first.is_newer_than(Some(second)));
    assert!(first.is_newer_than(None));
    assert_eq!(
        SnapshotGeneration::new(u64::MAX)
            .expect("maximum is representable")
            .checked_next()
            .expect_err("generation overflow")
            .code(),
        QueryErrorCode::Overflow
    );

    assert_eq!(
        PublicationGeneration::new(0).expect("empty archive").get(),
        0
    );
    assert_eq!(
        PublicationGeneration::new(i64::MAX as u64 + 1)
            .expect_err("SQLite cannot store generation")
            .code(),
        QueryErrorCode::InvalidValue
    );
    assert_eq!(DatasetIdentity::Empty.stable_code(), "empty");
    assert_eq!(
        DatasetIdentity::LegacySnapshotV1.stable_code(),
        "legacy_snapshot_v1"
    );
    assert_eq!(ReplayRevision::new(0).expect("first revision").get(), 0);
    assert_eq!(
        DatasetGeneration::new(0)
            .expect("first dataset generation")
            .get(),
        0
    );
    assert_eq!(
        ReplayRevision::new(i64::MAX as u64 + 1)
            .expect_err("SQLite cannot store revision")
            .code(),
        QueryErrorCode::InvalidValue
    );
    assert_eq!(
        DatasetGeneration::new(i64::MAX as u64 + 1)
            .expect_err("SQLite cannot store dataset generation")
            .code(),
        QueryErrorCode::InvalidValue
    );
    assert_eq!(
        DatasetIdentity::ReplayRevision {
            revision: ReplayRevision::new(7).expect("revision"),
            dataset_generation: DatasetGeneration::new(9).expect("dataset generation"),
        }
        .stable_code(),
        "replay_revision"
    );
}

#[test]
fn page_scope_and_warning_bounds_are_enforced() {
    assert_eq!(MAX_QUERY_PAGE_SIZE, 256);
    assert_eq!(PageSize::new(1).expect("minimum").get(), 1);
    assert_eq!(PageSize::new(256).expect("maximum").get(), 256);
    for invalid in [0, 257, usize::MAX] {
        assert_eq!(
            PageSize::new(invalid)
                .expect_err("invalid page size")
                .code(),
            QueryErrorCode::InvalidValue
        );
    }

    let scopes = (0..MAX_QUERY_SCOPES).map(scope).collect::<Vec<_>>();
    let warnings = vec![QueryWarningCode::RecoveryPending; MAX_QUERY_WARNINGS];
    let header = QueryHeader::new(QueryHeaderParts {
        snapshot_generation: SnapshotGeneration::new(1).expect("generation"),
        publication_generation: PublicationGeneration::new(0).expect("publication"),
        dataset_identity: DatasetIdentity::Empty,
        generated_at_ms: 10,
        data_through_ms: None,
        freshness: QueryFreshness::Unavailable,
        quality: QueryQuality::Authoritative,
        scopes,
        warnings,
    })
    .expect("bounded header");
    assert_eq!(header.schema_version(), QUERY_SCHEMA_VERSION);
    assert_eq!(header.scopes().len(), MAX_QUERY_SCOPES);
    assert_eq!(header.warnings().len(), MAX_QUERY_WARNINGS);

    let too_many_scopes = QueryHeader::new(QueryHeaderParts {
        scopes: (0..=MAX_QUERY_SCOPES).map(scope).collect(),
        ..header.clone().into_parts()
    })
    .expect_err("scope cap");
    assert_eq!(too_many_scopes.code(), QueryErrorCode::CapacityExceeded);

    let too_many_warnings = QueryHeader::new(QueryHeaderParts {
        warnings: vec![QueryWarningCode::RecoveryPending; MAX_QUERY_WARNINGS + 1],
        ..header.into_parts()
    })
    .expect_err("warning cap");
    assert_eq!(too_many_warnings.code(), QueryErrorCode::CapacityExceeded);
}

#[test]
fn envelope_is_owned_and_uses_snapshot_generation_for_consumer_ordering() {
    let header = QueryHeader::new(QueryHeaderParts {
        snapshot_generation: SnapshotGeneration::new(2).expect("generation"),
        publication_generation: PublicationGeneration::new(9).expect("publication"),
        dataset_identity: DatasetIdentity::ReplayRevision {
            revision: ReplayRevision::new(7).expect("revision"),
            dataset_generation: DatasetGeneration::new(9).expect("dataset generation"),
        },
        generated_at_ms: 1_000,
        data_through_ms: Some(900),
        freshness: QueryFreshness::Fresh,
        quality: QueryQuality::Authoritative,
        scopes: vec![scope(1)],
        warnings: Vec::new(),
    })
    .expect("header");
    let page = LatestActivityPage::new(vec![item(1)], None, false).expect("page");
    let envelope = QueryEnvelope::new(header, page);
    assert_eq!(envelope.schema_version(), QUERY_SCHEMA_VERSION);
    assert_eq!(envelope.header().publication_generation().get(), 9);
    assert_eq!(envelope.payload().items().len(), 1);
    assert!(envelope.is_newer_than(None));

    let same = envelope.clone();
    assert!(!same.is_newer_than(Some(&envelope)));
}

#[test]
fn activity_pages_are_owned_bounded_and_cursor_debug_is_redacted() {
    let items = (0..MAX_QUERY_PAGE_SIZE).map(item).collect::<Vec<_>>();
    let cursor = items.last().expect("last item").cursor();
    let page = LatestActivityPage::new(items, Some(cursor), true).expect("maximum page");
    let shared: Arc<[ActivityItem]> = page.items().clone();
    assert_eq!(shared.len(), MAX_QUERY_PAGE_SIZE);
    assert!(page.has_more());

    let over = (0..=MAX_QUERY_PAGE_SIZE).map(item).collect::<Vec<_>>();
    assert_eq!(
        LatestActivityPage::new(over, None, false)
            .expect_err("page cap")
            .code(),
        QueryErrorCode::CapacityExceeded
    );

    let debug = format!(
        "{:?}",
        ActivityCursor::new(5, 17, [0xAB; 32]).expect("cursor")
    );
    assert_eq!(
        debug,
        "ActivityCursor { timestamp_seconds: 5, timestamp_nanos: 17, fingerprint: [redacted] }"
    );
    assert!(!debug.contains("171"));
    assert_eq!(
        ActivityCursor::new(0, 1_000_000_000, [0; 32])
            .expect_err("nanosecond bound")
            .code(),
        QueryErrorCode::InvalidValue
    );
}

#[test]
fn stable_codes_do_not_include_inner_or_path_text() {
    assert_eq!(QueryFreshness::Fresh.stable_code(), "fresh");
    assert_eq!(QueryQuality::Conflict.stable_code(), "conflict");
    assert_eq!(
        QueryWarningCode::ClockDiscontinuity.stable_code(),
        "clock_discontinuity"
    );
    assert_eq!(
        QueryWarningCode::AccountingVersionStale.stable_code(),
        "accounting_version_stale"
    );

    for code in [
        QueryErrorCode::InvalidValue,
        QueryErrorCode::CapacityExceeded,
        QueryErrorCode::Unavailable,
        QueryErrorCode::VersionMismatch,
        QueryErrorCode::StaleSnapshot,
        QueryErrorCode::DeadlineExceeded,
        QueryErrorCode::CorruptArchive,
        QueryErrorCode::Overflow,
        QueryErrorCode::Internal,
    ] {
        let rendered = code.to_string();
        assert_eq!(rendered, code.stable_code());
        assert!(!rendered.contains(':'));
        assert!(!rendered.contains('\\'));
        assert!(!rendered.contains('/'));
    }
}

#[test]
fn facade_clock_is_injected_as_one_exact_sample() {
    struct FixedClock;

    impl QueryClock for FixedClock {
        fn sample(&self) -> Result<QueryTimeSample, QueryError> {
            Ok(QueryTimeSample::new(42, 7))
        }
    }

    let sample = FixedClock.sample().expect("fixed sample");
    assert_eq!(sample.wall_time_ms(), 42);
    assert_eq!(sample.monotonic_ms(), 7);
}

#[test]
fn system_clock_is_monotonic_and_debug_private() {
    let clock = SystemQueryClock::new();
    let first = clock.sample().expect("first sample");
    let second = clock.sample().expect("second sample");
    assert!(first.wall_time_ms() > 0);
    assert!(second.monotonic_ms() >= first.monotonic_ms());
    assert_eq!(format!("{clock:?}"), "SystemQueryClock([redacted])");
}
