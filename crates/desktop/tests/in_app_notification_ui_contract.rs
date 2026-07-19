use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use slint::Model;
use tokenmaster_desktop::{
    DesktopInAppNotification, DesktopInAppNotificationBatch, DesktopNotificationKind,
    DesktopNotificationPresentationReceipt, DesktopShell, MAX_DESKTOP_IN_APP_NOTIFICATIONS,
};
use tokenmaster_product::ProductReducer;

#[derive(Default)]
struct RecordingReceipt {
    presented: AtomicU64,
    failed: AtomicU64,
}

impl RecordingReceipt {
    fn presented_count(&self) -> u64 {
        self.presented.load(Ordering::Acquire)
    }

    fn failed_count(&self) -> u64 {
        self.failed.load(Ordering::Acquire)
    }
}

impl DesktopNotificationPresentationReceipt for RecordingReceipt {
    fn presented(&self) {
        self.presented.fetch_add(1, Ordering::AcqRel);
    }

    fn failed(&self) {
        self.failed.fetch_add(1, Ordering::AcqRel);
    }
}

fn notification(index: usize) -> DesktopInAppNotification {
    let offset = i64::try_from(index).expect("test index fits i64");
    DesktopInAppNotification::new(
        DesktopNotificationKind::BankedRateLimitReset,
        u64::try_from(index + 1).expect("test quantity fits u64"),
        "benefit.codex.banked_reset",
        86_400,
        1_800_000_000_000 + offset,
        1_800_086_400_000 + offset,
        1_800_000_000_500 + offset,
    )
    .expect("valid desktop notification")
}

fn one_notification_batch() -> DesktopInAppNotificationBatch {
    DesktopInAppNotificationBatch::new(vec![notification(0)]).expect("one notification")
}

#[test]
fn notification_values_fail_closed_at_exact_bounds() {
    assert_eq!(MAX_DESKTOP_IN_APP_NOTIFICATIONS, 256);
    assert_eq!(
        DesktopInAppNotificationBatch::new(Vec::new())
            .expect_err("empty batch must fail")
            .stable_code(),
        "invalid_batch"
    );
    let maximum = (0..MAX_DESKTOP_IN_APP_NOTIFICATIONS)
        .map(notification)
        .collect::<Vec<_>>();
    let maximum_batch = DesktopInAppNotificationBatch::new(maximum).expect("exact maximum");
    assert_eq!(maximum_batch.len(), MAX_DESKTOP_IN_APP_NOTIFICATIONS);
    assert!(!maximum_batch.is_empty());
    let over = (0..=MAX_DESKTOP_IN_APP_NOTIFICATIONS)
        .map(notification)
        .collect::<Vec<_>>();
    assert_eq!(
        DesktopInAppNotificationBatch::new(over)
            .expect_err("over-cap batch must fail")
            .stable_code(),
        "capacity_exceeded"
    );
    assert_eq!(
        DesktopInAppNotification::new(
            DesktopNotificationKind::UsageCredit,
            0,
            "benefit.codex.credit",
            3_600,
            10,
            20,
            15,
        )
        .expect_err("zero quantity must fail")
        .stable_code(),
        "invalid_value"
    );
    assert_eq!(
        DesktopInAppNotification::new(
            DesktopNotificationKind::UsageCredit,
            1,
            "private label",
            3_600,
            10,
            20,
            15,
        )
        .expect_err("unsafe label must fail")
        .stable_code(),
        "invalid_value"
    );
    assert_eq!(
        DesktopInAppNotification::new(
            DesktopNotificationKind::UsageCredit,
            1,
            "benefit.codex.credit",
            3_600,
            20,
            20,
            15,
        )
        .expect_err("due must precede expiry")
        .stable_code(),
        "invalid_value"
    );
}

#[test]
fn real_event_loop_applies_every_row_before_presentation_receipt_and_dismisses() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let shell = DesktopShell::new(&ProductReducer::new().snapshot()).expect("desktop shell");
    let stale_bridge = shell
        .bridge_factory()
        .in_app_notification_bridge()
        .expect("stale notification bridge");
    let stale_receipt = Arc::new(RecordingReceipt::default());
    stale_bridge
        .present(one_notification_batch(), stale_receipt.clone())
        .expect("stale presentation scheduled");
    drop(stale_bridge);

    let bridge = shell
        .bridge_factory()
        .in_app_notification_bridge()
        .expect("notification bridge");
    let receipt = Arc::new(RecordingReceipt::default());
    let rows = (0..MAX_DESKTOP_IN_APP_NOTIFICATIONS)
        .map(notification)
        .collect::<Vec<_>>();
    bridge
        .present(
            DesktopInAppNotificationBatch::new(rows).expect("maximum batch"),
            receipt.clone(),
        )
        .expect("presentation scheduled");

    assert_eq!(
        receipt.presented_count(),
        0,
        "scheduling is not presentation"
    );
    assert_eq!(receipt.failed_count(), 0);
    assert_eq!(
        bridge
            .present(
                one_notification_batch(),
                Arc::new(RecordingReceipt::default())
            )
            .expect_err("second in-flight presentation must fail")
            .stable_code(),
        "busy"
    );

    let observed = receipt.clone();
    let observed_stale = stale_receipt.clone();
    let watcher = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while (observed.presented_count() == 0 && observed.failed_count() == 0)
            || (observed_stale.presented_count() == 0 && observed_stale.failed_count() == 0)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "presentation timed out"
            );
            thread::sleep(Duration::from_millis(1));
        }
        slint::quit_event_loop().expect("quit event loop");
    });
    slint::run_event_loop().expect("headless event loop");
    watcher.join().expect("watcher joins");

    assert_eq!(receipt.presented_count(), 1);
    assert_eq!(receipt.failed_count(), 0);
    assert_eq!(stale_receipt.presented_count(), 0);
    assert_eq!(stale_receipt.failed_count(), 1);
    assert!(shell.window().get_in_app_notification_visible());
    assert_eq!(
        shell.window().get_in_app_notification_rows().row_count(),
        MAX_DESKTOP_IN_APP_NOTIFICATIONS
    );
    assert_eq!(
        shell.window().get_in_app_notification_count_label(),
        "256 expiry reminders"
    );
    let first = shell
        .window()
        .get_in_app_notification_rows()
        .row_data(0)
        .expect("first visible notification");
    assert!(first.accessible_label.contains("Banked rate-limit reset"));
    assert!(first.accessible_label.contains("Expires"));

    shell.window().invoke_dismiss_in_app_notifications();
    assert!(!shell.window().get_in_app_notification_visible());
    assert_eq!(shell.window().get_in_app_notification_rows().row_count(), 0);
}
