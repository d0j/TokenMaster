use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::WriterLease;
use tokenmaster_product::{
    ProductAttemptGeneration, ProductPublishOutcome, ProductReducer, ProductRoute,
    ProductRouteReason, ProductRouteState, ProductRuntimeFailureCode, ProductRuntimeGeneration,
    ProductRuntimeLifecycle, ProductSectionKind,
};
use tokenmaster_query::{
    QueryClock, QueryError, QueryService, QueryTimeSample, UsageAnalyticsRequest, UsageRange,
    UsageSeriesSelection, UsageTimeZone, WeekStart,
};
use tokenmaster_runtime::{
    BenefitReminderFailure, BenefitReminderRuntime, BenefitReminderRuntimeConfig, GitRuntime,
    GitRuntimeConfig, LiveRuntime, RuntimeErrorCode, RuntimeWriterLease,
};
use tokenmaster_store::UsageStore;

#[derive(Clone, Copy)]
struct FixedClock;

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(1_800_000_000_000, 1))
    }
}

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("non-zero product attempt")
}

fn runtime_generation(value: u64) -> ProductRuntimeGeneration {
    ProductRuntimeGeneration::new(value).expect("non-zero runtime generation")
}

fn publish_ready_usage(reducer: &mut ProductReducer, path: &std::path::Path) {
    drop(UsageStore::open(path).expect("create archive"));
    let mut service = QueryService::open(path, FixedClock).expect("query service");
    reducer
        .publish_data_status(
            attempt(1),
            service.product_data_status().expect("product status"),
        )
        .expect("publish status");
    let analytics = service
        .usage_analytics(
            UsageAnalyticsRequest::new(
                UsageRange::today(),
                UsageTimeZone::iana("UTC").expect("UTC"),
                WeekStart::Monday,
                UsageSeriesSelection::None,
                Vec::new(),
                Vec::new(),
            )
            .expect("analytics request"),
        )
        .expect("analytics");
    reducer
        .publish_analytics(attempt(1), analytics)
        .expect("publish analytics");
}

#[test]
fn runtime_observation_failures_are_ordered_and_fault_only_owned_routes() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("runtime-isolation.sqlite3");
    let mut reducer = ProductReducer::new();
    publish_ready_usage(&mut reducer, &path);
    assert_eq!(
        reducer.snapshot().route(ProductRoute::History).state(),
        ProductRouteState::Ready
    );

    assert_eq!(
        reducer
            .fail_usage_runtime(runtime_generation(2), RuntimeErrorCode::Faulted)
            .expect("usage runtime failure"),
        ProductPublishOutcome::Accepted
    );
    let usage_failed = reducer.snapshot();
    assert_eq!(
        usage_failed.runtime().usage().kind(),
        ProductSectionKind::Unavailable
    );
    assert_eq!(
        usage_failed
            .runtime()
            .usage()
            .observation_error()
            .expect("observation error")
            .stable_code(),
        "faulted"
    );
    assert_eq!(
        usage_failed.route(ProductRoute::History).state(),
        ProductRouteState::Degraded
    );
    assert!(
        usage_failed
            .route(ProductRoute::History)
            .reasons()
            .contains(ProductRouteReason::UsageUnavailable)
    );
    assert_eq!(
        usage_failed.route(ProductRoute::Settings).state(),
        ProductRouteState::Ready
    );
    assert_eq!(
        usage_failed.runtime().quota().kind(),
        ProductSectionKind::Waiting
    );

    assert_eq!(
        reducer
            .fail_usage_runtime(runtime_generation(1), RuntimeErrorCode::Internal)
            .expect("older failure"),
        ProductPublishOutcome::RejectedOlder
    );
    assert_eq!(
        reducer
            .fail_quota_runtime(runtime_generation(1), RuntimeErrorCode::StoreUnavailable)
            .expect("quota failure"),
        ProductPublishOutcome::Accepted
    );
    let independently_failed = reducer.snapshot();
    assert_eq!(
        independently_failed.runtime().usage().generation(),
        Some(runtime_generation(2))
    );
    assert_eq!(
        independently_failed.runtime().quota().generation(),
        Some(runtime_generation(1))
    );
    assert_eq!(
        independently_failed.runtime().reminder().kind(),
        ProductSectionKind::Waiting
    );
    assert_eq!(
        independently_failed.runtime().git().kind(),
        ProductSectionKind::Waiting
    );
}

#[test]
fn live_and_git_pause_resume_snapshots_are_copied_without_runtime_ownership() {
    let directory = TempDir::new().expect("temporary directory");
    let source = directory.path().join("source");
    std::fs::create_dir(&source).expect("source root");
    let configured = [ConfiguredCodexRoot::new(&source, None, true)];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request");
    let live_path = directory.path().join("live.sqlite3");
    let mut live = LiveRuntime::start(&live_path, request).expect("live runtime");
    live.pause().expect("pause live runtime");

    let git_path = directory.path().join("git.sqlite3");
    let mut git = GitRuntime::start(GitRuntimeConfig::new(git_path).expect("Git config"))
        .expect("Git runtime");
    git.pause().expect("pause Git runtime");

    let mut reducer = ProductReducer::new();
    reducer
        .publish_usage_runtime(
            runtime_generation(1),
            live.snapshot().expect("paused live snapshot"),
        )
        .expect("publish live health");
    reducer
        .publish_git_runtime(
            runtime_generation(1),
            git.snapshot().expect("paused Git snapshot"),
        )
        .expect("publish Git health");
    let paused = reducer.snapshot();
    assert_eq!(
        paused
            .runtime()
            .usage()
            .health()
            .expect("usage health")
            .lifecycle(),
        ProductRuntimeLifecycle::Paused
    );
    assert_eq!(
        paused
            .runtime()
            .git()
            .health()
            .expect("Git health")
            .lifecycle(),
        ProductRuntimeLifecycle::Paused
    );
    assert!(!std::mem::needs_drop::<
        tokenmaster_product::ProductUsageRuntimeHealth,
    >());
    assert!(!std::mem::needs_drop::<
        tokenmaster_product::ProductGitRuntimeHealth,
    >());
    assert!(!std::mem::needs_drop::<
        tokenmaster_product::ProductRuntimeStatus,
    >());

    live.resume().expect("resume live runtime");
    reducer
        .publish_usage_runtime(
            runtime_generation(2),
            live.snapshot().expect("resumed live snapshot"),
        )
        .expect("publish resumed health");
    assert_eq!(
        reducer
            .snapshot()
            .runtime()
            .usage()
            .health()
            .expect("resumed health")
            .lifecycle(),
        ProductRuntimeLifecycle::Running
    );

    live.shutdown().expect("shutdown live runtime");
    git.shutdown().expect("shutdown Git runtime");
    assert_eq!(
        paused
            .runtime()
            .usage()
            .health()
            .expect("retained copied health")
            .lifecycle(),
        ProductRuntimeLifecycle::Paused
    );
}

#[test]
fn reminder_failure_maps_to_owned_code_and_pending_metadata_is_bounded() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("reminder.sqlite3");
    let mut competing = RuntimeWriterLease::new(&path).expect("competing lease");
    let guard = competing.try_acquire().expect("hold lease");
    let mut reminder = BenefitReminderRuntime::start(
        BenefitReminderRuntimeConfig::new(path).expect("reminder config"),
    )
    .expect("reminder runtime");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if reminder.try_completion().expect("completion").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "reminder completion timed out");
        std::thread::yield_now();
    }
    let source = reminder.snapshot().expect("reminder snapshot");
    assert_eq!(
        source.refresh().failure(),
        Some(BenefitReminderFailure::Busy)
    );

    let mut reducer = ProductReducer::new();
    reducer
        .publish_reminder_runtime(runtime_generation(1), source)
        .expect("publish reminder health");
    let product = reducer.snapshot();
    let health = product
        .runtime()
        .reminder()
        .health()
        .expect("reminder health");
    assert_eq!(
        health.failure(),
        Some(ProductRuntimeFailureCode::ReminderBusy)
    );
    assert_eq!(health.pending_due_count(), 0);
    assert_eq!(health.retained_delivery_count(), 0);

    drop(guard);
    reminder.shutdown().expect("shutdown reminder");
}

#[cfg(windows)]
#[test]
fn quota_transport_failure_is_copied_without_executable_or_archive_identity() {
    use tokenmaster_runtime::{CodexQuotaRuntime, CodexQuotaRuntimeConfig};

    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("quota.sqlite3");
    let executable =
        std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("Windows root"))
            .join("System32")
            .join("where.exe");
    let config = CodexQuotaRuntimeConfig::new(path.clone())
        .expect("quota config")
        .with_executable(executable.clone())
        .expect("fixed executable")
        .with_transport_timeout(Duration::from_secs(1))
        .expect("transport timeout");
    let mut quota = CodexQuotaRuntime::start(config).expect("quota runtime");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if quota.try_completion().expect("completion").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "quota completion timed out");
        std::thread::yield_now();
    }

    let mut reducer = ProductReducer::new();
    reducer
        .publish_quota_runtime(
            runtime_generation(1),
            quota.snapshot().expect("quota snapshot"),
        )
        .expect("publish quota health");
    let product = reducer.snapshot();
    let health = product.runtime().quota().health().expect("quota health");
    assert_eq!(
        health.quota_failure(),
        Some(ProductRuntimeFailureCode::QuotaTransport)
    );
    assert_eq!(
        health.benefit_failure(),
        Some(ProductRuntimeFailureCode::QuotaTransport)
    );
    let debug = format!("{product:?}");
    assert!(!debug.contains(path.to_string_lossy().as_ref()));
    assert!(!debug.contains(executable.to_string_lossy().as_ref()));

    quota.shutdown().expect("shutdown quota");
}
