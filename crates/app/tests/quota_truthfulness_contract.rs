//! A quota poll that failed must reach the card that reports quota.
//!
//! This sits in `app` rather than in `desktop` because it is a seam between two crates
//! that only `app` depends on at once: the runtime produces the failure and the desktop
//! projection has to carry it. Observed on a live window before it existed -- the Plan
//! Usage card rendered a `Ready` pill with no reason codes beside "Quota evidence
//! unavailable", because the archive query for quota succeeds whether or not Codex was
//! ever reached, and the poll's own verdict reached no projection at all.

use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokenmaster_desktop::{DesktopDashboardSectionKey, DesktopRouteKey, DesktopState};
use tokenmaster_product::{ProductReducer, ProductRuntimeGeneration};
use tokenmaster_runtime::{CodexQuotaRuntimeConfig, ProviderQuotaRuntime};

fn runtime_generation(value: u64) -> ProductRuntimeGeneration {
    ProductRuntimeGeneration::new(value).expect("nonzero runtime generation")
}

/// A real executable that is not Codex, so the poll fails for a reason of its own rather
/// than because the path was invented.
fn not_codex() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("Windows root"))
        .join("System32")
        .join("where.exe")
}

#[test]
fn a_failed_quota_poll_reaches_the_card_that_reports_quota() {
    let directory = TempDir::new().expect("temporary directory");
    let archive = directory.path().join("quota.sqlite3");
    let config = CodexQuotaRuntimeConfig::new(archive)
        .expect("quota config")
        .with_executable(not_codex())
        .expect("fixed executable")
        .with_transport_timeout(Duration::from_secs(1))
        .expect("transport timeout");
    let mut quota = ProviderQuotaRuntime::start(config).expect("quota runtime");

    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if quota.try_completion().expect("completion").is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let mut reducer = ProductReducer::new();
    reducer
        .publish_quota_runtime(
            runtime_generation(1),
            quota.snapshot().expect("quota snapshot"),
        )
        .expect("publish quota runtime health");
    quota.shutdown().expect("quota shutdown");

    let snapshot = reducer.snapshot();
    // The failure has to be real, or this test would pass against a runtime that never ran.
    let failure = snapshot
        .runtime()
        .quota()
        .health()
        .expect("quota health")
        .quota_failure()
        .expect("the poll against a non-Codex executable must fail");

    let state = DesktopState::new(&snapshot, DesktopRouteKey::Dashboard);
    let dashboard = state.projection().dashboard();
    let plan = dashboard
        .sections()
        .iter()
        .find(|section| section.key() == DesktopDashboardSectionKey::PlanUsage)
        .expect("plan usage section");
    let reasons = plan.reason_codes().iter().collect::<Vec<_>>();

    assert!(
        reasons.contains(&failure.stable_code()),
        "the card must name the failure the poll reported; expected {}, received {reasons:?}",
        failure.stable_code()
    );
}
