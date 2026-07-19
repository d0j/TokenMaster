mod support;

use std::sync::Arc;

use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopBenefitExpiry, DesktopDashboardSectionState, DesktopFreshness,
    DesktopNotificationsProjection, DesktopQuality, DesktopRouteKey, DesktopState,
    MAX_NOTIFICATION_LEADS, MAX_NOTIFICATION_LOTS, MAX_NOTIFICATION_SCOPES,
};
use tokenmaster_domain::{
    BenefitExpiry, BenefitInventoryCompleteness, BenefitKind, BenefitLocalDate,
    BenefitLocalDateTime, BenefitLocalTime, BenefitState, BenefitTimeZoneId,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer};
use tokenmaster_query::{BenefitOverviewRequest, QueryErrorCode, QueryService};

use support::dashboard_fixture::{
    BENEFIT_EXPIRY_AT_MS, FixedClock, WALL_TIME_MS, notification_benefit_lot,
    publish_notification_benefit_scope, seed,
};

fn attempt(value: u64) -> ProductAttemptGeneration {
    ProductAttemptGeneration::new(value).expect("nonzero attempt")
}

fn publish_overview(path: &std::path::Path) -> ProductReducer {
    let mut service = QueryService::open(path, FixedClock).expect("query service");
    let overview = service
        .benefit_overview(BenefitOverviewRequest::new())
        .expect("benefit overview");
    let mut reducer = ProductReducer::new();
    reducer
        .publish_benefit(attempt(1), overview)
        .expect("publish benefit overview");
    reducer
}

#[test]
fn initial_notifications_are_bounded_waiting_truth() {
    let reducer = ProductReducer::new();
    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Notifications);
    let notifications = state.projection().notifications();

    assert_eq!(MAX_NOTIFICATION_SCOPES, 32);
    assert_eq!(MAX_NOTIFICATION_LOTS, 256);
    assert_eq!(MAX_NOTIFICATION_LEADS, 8);
    assert_eq!(notifications.state(), DesktopDashboardSectionState::Waiting);
    assert_eq!(notifications.scopes().len(), 0);
    assert_eq!(notifications.lots().len(), 0);
    assert_eq!(notifications.freshness(), None);
    assert_eq!(notifications.quality(), None);
    assert!(!notifications.scopes_truncated());
    assert!(!notifications.lots_truncated());
}

#[test]
fn unavailable_notifications_have_no_fabricated_inventory() {
    let mut reducer = ProductReducer::new();
    reducer
        .fail_benefit(attempt(1), QueryErrorCode::Unavailable)
        .expect("benefit failure");
    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Notifications);
    let notifications = state.projection().notifications();

    assert_eq!(
        notifications.state(),
        DesktopDashboardSectionState::Unavailable
    );
    assert!(notifications.scopes().is_empty());
    assert!(notifications.lots().is_empty());
    assert!(
        notifications
            .reason_codes()
            .iter()
            .any(|reason| reason == "unavailable")
    );
}

#[test]
fn production_overview_maps_effective_profile_and_safe_separate_lots() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("notifications-projection.sqlite3");
    seed(&path);
    let reducer = publish_overview(&path);
    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Notifications);
    let notifications: &DesktopNotificationsProjection = state.projection().notifications();

    assert_eq!(
        notifications.state(),
        DesktopDashboardSectionState::Degraded
    );
    assert_eq!(notifications.freshness(), Some(DesktopFreshness::Fresh));
    assert_eq!(notifications.quality(), Some(DesktopQuality::Authoritative));
    assert_eq!(notifications.scopes().len(), 1);
    assert_eq!(notifications.lots().len(), 4);

    let scope = &notifications.scopes()[0];
    assert_eq!(scope.ordinal(), 1);
    assert_eq!(scope.current_lot_count(), 4);
    assert_eq!(scope.profile_source(), "inherited");
    assert_eq!(scope.reminder_coverage(), "in_app_only");
    assert_eq!(
        scope.lead_seconds(),
        &[604_800, 86_400, 43_200, 21_600, 3_600]
    );
    assert_eq!(scope.completeness(), "complete");
    assert_eq!(scope.nearest_expiry_at_ms(), Some(BENEFIT_EXPIRY_AT_MS));
    assert_eq!(scope.freshness(), DesktopFreshness::Fresh);
    assert_eq!(scope.quality(), DesktopQuality::Authoritative);
    assert!(
        scope
            .warning_codes()
            .iter()
            .any(|reason| reason == "unknown_expiry")
    );

    assert_eq!(notifications.lots()[0].scope_ordinal(), 1);
    assert_eq!(notifications.lots()[0].kind(), "banked_rate_limit_reset");
    assert_eq!(notifications.lots()[0].quantity(), 7);
    assert_eq!(notifications.lots()[0].state(), "expired");
    assert_eq!(notifications.lots()[1].kind(), "banked_rate_limit_reset");
    assert_eq!(notifications.lots()[1].quantity(), 2);
    assert_eq!(notifications.lots()[1].state(), "available");
    assert_eq!(
        notifications.lots()[1].expiry(),
        &DesktopBenefitExpiry::ExactUtc {
            at_ms: BENEFIT_EXPIRY_AT_MS
        }
    );
    assert_eq!(notifications.lots()[2].kind(), "usage_credit");
    assert_eq!(notifications.lots()[2].quantity(), 4);
    assert_eq!(
        notifications.lots()[2].expiry(),
        &DesktopBenefitExpiry::Unknown
    );
    assert_eq!(notifications.lots()[3].kind(), "temporary_usage");
    assert_eq!(notifications.lots()[3].state(), "activation_pending");
    assert_eq!(
        notifications.lots()[0].evidence_source(),
        "provider_official"
    );
    assert_eq!(notifications.lots()[0].confidence(), "high");
    assert_eq!(notifications.lots()[0].detail_kind(), "provider_detail");

    let debug = format!("{notifications:?}");
    for forbidden in [
        path.to_string_lossy().as_ref(),
        "dashboard-private-account",
        "notification-private-account",
        "delivery_id",
        "lot_id",
        "scope_id",
        "activate",
    ] {
        assert!(
            !debug.contains(forbidden),
            "notifications exposed {forbidden}"
        );
    }
}

#[test]
fn expiry_precision_and_eight_lead_override_are_lossless() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("notifications-expiry-precision.sqlite3");
    let date = BenefitLocalDate::new(2026, 7, 31).expect("local date");
    let zone = BenefitTimeZoneId::new("Asia/Jerusalem").expect("time zone");
    let local = BenefitLocalDateTime::new(
        date,
        BenefitLocalTime::new(23, 45, 12, 345).expect("local time"),
    );
    publish_notification_benefit_scope(
        &path,
        41,
        BenefitInventoryCompleteness::Complete,
        vec![
            notification_benefit_lot(
                41,
                BenefitKind::BankedRateLimitReset,
                1,
                BenefitState::Available,
                BenefitExpiry::exact_utc(BENEFIT_EXPIRY_AT_MS).expect("exact expiry"),
            ),
            notification_benefit_lot(
                42,
                BenefitKind::UsageCredit,
                2,
                BenefitState::Available,
                BenefitExpiry::bounded_utc(BENEFIT_EXPIRY_AT_MS + 1, BENEFIT_EXPIRY_AT_MS + 2)
                    .expect("bounded expiry"),
            ),
            notification_benefit_lot(
                43,
                BenefitKind::TemporaryUsage,
                3,
                BenefitState::Available,
                BenefitExpiry::provider_local(local, zone.clone()),
            ),
            notification_benefit_lot(
                44,
                BenefitKind::Unknown,
                1,
                BenefitState::Ambiguous,
                BenefitExpiry::provider_date(date, Some(zone)),
            ),
            notification_benefit_lot(
                45,
                BenefitKind::Unknown,
                4,
                BenefitState::Revoked,
                BenefitExpiry::unknown(),
            ),
        ],
        Some(&[
            31_536_000, 604_800, 86_400, 43_200, 21_600, 10_800, 3_600, 60,
        ]),
    );
    let reducer = publish_overview(&path);
    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Notifications);
    let notifications = state.projection().notifications();

    assert_eq!(notifications.scopes()[0].profile_source(), "override");
    assert_eq!(
        notifications.scopes()[0].lead_seconds().len(),
        MAX_NOTIFICATION_LEADS
    );
    assert_eq!(notifications.lots().len(), 5);
    assert!(matches!(
        notifications.lots()[0].expiry(),
        DesktopBenefitExpiry::ExactUtc { .. }
    ));
    assert!(matches!(
        notifications.lots()[1].expiry(),
        DesktopBenefitExpiry::BoundedUtc { .. }
    ));
    assert!(matches!(
        notifications.lots()[2].expiry(),
        DesktopBenefitExpiry::ProviderLocal {
            year: 2026,
            month: 7,
            day: 31,
            hour: 23,
            minute: 45,
            second: 12,
            millisecond: 345,
            ..
        }
    ));
    assert!(matches!(
        notifications.lots()[3].expiry(),
        DesktopBenefitExpiry::ProviderDate {
            year: 2026,
            month: 7,
            day: 31,
            ..
        }
    ));
    assert_eq!(notifications.lots()[3].quantity(), 1);
    assert_eq!(notifications.lots()[3].state(), "ambiguous");
    assert_eq!(
        notifications.lots()[4].expiry(),
        &DesktopBenefitExpiry::Unknown
    );
    assert_eq!(notifications.lots()[4].state(), "revoked");
}

#[test]
fn retained_failure_keeps_current_inventory_and_current_reason() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("notifications-retained.sqlite3");
    seed(&path);
    let mut reducer = publish_overview(&path);
    reducer
        .fail_benefit(attempt(2), QueryErrorCode::DeadlineExceeded)
        .expect("retain overview");

    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Notifications);
    let notifications = state.projection().notifications();
    assert_eq!(
        notifications.state(),
        DesktopDashboardSectionState::Degraded
    );
    assert_eq!(notifications.scopes().len(), 1);
    assert_eq!(notifications.lots().len(), 4);
    assert!(
        notifications
            .reason_codes()
            .iter()
            .any(|reason| reason == "deadline_exceeded")
    );
}

#[test]
fn exact_scope_lot_and_lead_caps_remain_bounded() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("notifications-cap.sqlite3");
    for scope in 0..MAX_NOTIFICATION_SCOPES {
        let scope_seed = u8::try_from(scope + 1).expect("scope seed");
        let lots = (0..8)
            .map(|lot| {
                let id = u8::try_from(scope * 8 + lot).expect("lot id");
                notification_benefit_lot(
                    id,
                    BenefitKind::BankedRateLimitReset,
                    u64::try_from(lot + 1).expect("quantity"),
                    BenefitState::Available,
                    BenefitExpiry::exact_utc(
                        WALL_TIME_MS
                            + i64::try_from(scope * 8 + lot + 1).expect("expiry offset") * 1_000,
                    )
                    .expect("expiry"),
                )
            })
            .collect();
        publish_notification_benefit_scope(
            &path,
            scope_seed,
            BenefitInventoryCompleteness::Complete,
            lots,
            Some(&[
                31_536_000, 604_800, 86_400, 43_200, 21_600, 10_800, 3_600, 60,
            ]),
        );
    }
    let reducer = publish_overview(&path);
    let state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Notifications);
    let notifications = state.projection().notifications();

    assert_eq!(notifications.scopes().len(), MAX_NOTIFICATION_SCOPES);
    assert_eq!(notifications.lots().len(), MAX_NOTIFICATION_LOTS);
    assert!(
        notifications
            .scopes()
            .iter()
            .all(|scope| scope.lead_seconds().len() == MAX_NOTIFICATION_LEADS)
    );
    assert!(!notifications.scopes_truncated());
    assert!(!notifications.lots_truncated());
}

#[test]
fn ten_thousand_replacements_release_old_notification_arrays() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("notifications-replacement.sqlite3");
    seed(&path);
    let mut reducer = publish_overview(&path);
    let mut state = DesktopState::new(&reducer.snapshot(), DesktopRouteKey::Notifications);
    assert!(!state.projection().notifications().scopes().is_empty());
    assert!(!state.projection().notifications().lots().is_empty());
    let old_scopes = Arc::clone(state.projection().notifications().scopes());
    let old_lots = Arc::clone(state.projection().notifications().lots());
    let old_scopes_weak = Arc::downgrade(&old_scopes);
    let old_lots_weak = Arc::downgrade(&old_lots);
    drop(old_scopes);
    drop(old_lots);

    for generation in 2..=10_001 {
        reducer
            .fail_benefit(attempt(generation), QueryErrorCode::Unavailable)
            .expect("new generation");
        state.apply_snapshot(&reducer.snapshot());
    }

    assert!(old_scopes_weak.upgrade().is_none());
    assert!(old_lots_weak.upgrade().is_none());
}
