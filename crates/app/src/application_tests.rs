use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_product::ProductSectionKind;

use super::*;

#[test]
fn early_notification_sets_one_pending_bit_without_allocating_generation() {
    let bundle: SharedBundle = Arc::new(Mutex::new(None));
    let notifier = ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle));

    notifier.publish().expect("lossy early notification");

    assert!(notifier.pending.load(Ordering::Acquire));
    assert_eq!(notifier.next_generation.load(Ordering::Acquire), 1);
}

#[test]
fn runtime_generation_overflow_is_checked_and_path_free() {
    let bundle: SharedBundle = Arc::new(Mutex::new(None));
    let notifier = ApplicationRuntimeNotifier::new(Arc::downgrade(&bundle));
    notifier.next_generation.store(u64::MAX, Ordering::Release);

    let error = notifier
        .next_generation()
        .expect_err("generation must not wrap");
    assert_eq!(error.code(), ApplicationErrorCode::GenerationOverflow);
    assert_eq!(error.to_string(), "generation_overflow");
}

#[test]
fn real_bundle_joins_live_health_and_independent_optional_failures_then_shuts_down() {
    let temporary = TempDir::new().expect("temporary directory");
    let codex_root = temporary.path().join("codex");
    std::fs::create_dir(&codex_root).expect("Codex root");
    let configured = [ConfiguredCodexRoot::new(&codex_root, None, true)];
    let discovery = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request");
    let archive = temporary.path().join("application.sqlite3");
    let live = LiveRuntime::start(&archive, discovery).expect("live runtime");
    let controller =
        DesktopController::open(&archive, DesktopQueryPlan::overview().expect("query plan"))
            .expect("desktop controller");
    let mut bundle = ApplicationBundle {
        live,
        quota: OptionalRuntime {
            owner: None,
            failure: Some(RuntimeErrorCode::ProviderUnavailable),
        },
        reminder: OptionalRuntime {
            owner: None,
            failure: Some(RuntimeErrorCode::StoreUnavailable),
        },
        controller,
    };

    bundle
        .publish_runtime(ProductRuntimeGeneration::new(1).expect("generation"))
        .expect("publish runtime health");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if bundle
            .controller
            .try_completion()
            .expect("controller completion")
            .is_some()
        {
            break;
        }
        assert!(Instant::now() < deadline, "controller completion timed out");
        std::thread::yield_now();
    }
    let snapshot = bundle
        .controller
        .take_snapshot()
        .expect("snapshot mailbox")
        .expect("product snapshot");
    assert_eq!(snapshot.runtime().usage().kind(), ProductSectionKind::Ready);
    assert_eq!(snapshot.runtime().git().kind(), ProductSectionKind::Ready);
    assert_eq!(
        snapshot.runtime().quota().observation_error(),
        Some(ProductRuntimeObservationError::ProviderUnavailable)
    );
    assert_eq!(
        snapshot.runtime().reminder().observation_error(),
        Some(ProductRuntimeObservationError::StoreUnavailable)
    );

    bundle.shutdown().expect("bundle shutdown");
}
