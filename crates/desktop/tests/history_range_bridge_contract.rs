use tokenmaster_desktop::{
    DesktopController, DesktopHistoryRangeGeneration, DesktopHistoryRangeIntent,
    DesktopHistoryRangePreset, DesktopQueryPlan, DesktopRefreshUrgency, DesktopShell,
    DesktopSnapshotEpoch,
};
use tokenmaster_product::ProductReducer;
use tokenmaster_store::UsageStore;

fn initialize_testing_platform() {
    static INITIALIZED: std::sync::Once = std::sync::Once::new();
    INITIALIZED.call_once(i_slint_backend_testing::init_integration_test_with_system_time);
}

fn run_until_delivery(
    bridge: &tokenmaster_desktop::DesktopSnapshotBridge,
    expected_deliveries: u64,
) {
    let observer = bridge.observer();
    let watcher = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if observer
                .snapshot()
                .is_some_and(|snapshot| snapshot.delivered_count() >= expected_deliveries)
            {
                return slint::quit_event_loop();
            }
            if std::time::Instant::now() >= deadline {
                return slint::quit_event_loop();
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    slint::run_event_loop().expect("headless event loop");
    watcher
        .join()
        .expect("delivery watcher")
        .expect("event loop quit");
    assert_eq!(bridge.snapshot().delivered_count(), expected_deliveries);
}

#[test]
fn history_terminal_notifier_is_single_attach_and_weak_after_window_teardown() {
    initialize_testing_platform();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let archive = directory.path().join("history-terminal.sqlite3");
    drop(UsageStore::open(&archive).expect("create archive"));

    let snapshot = ProductReducer::new().snapshot();
    let shell = DesktopShell::new(&snapshot).expect("desktop shell");
    let mut controller = DesktopController::open(
        &archive,
        DesktopQueryPlan::overview().expect("overview plan"),
    )
    .expect("controller");
    let bridge = shell
        .snapshot_bridge(controller.snapshot_receiver())
        .expect("bridge");
    controller
        .bind_snapshot_epoch(bridge.epoch())
        .expect("bind epoch");
    let notifier = bridge.terminal_history_range_notifier();
    controller
        .attach_terminal_history_range_notifier(notifier.clone())
        .expect("first attachment");
    assert_eq!(
        controller
            .attach_terminal_history_range_notifier(bridge.terminal_history_range_notifier())
            .expect_err("second attachment rejects")
            .code()
            .stable_code(),
        "notifier_already_attached"
    );

    let intent = DesktopHistoryRangeIntent::new(
        DesktopSnapshotEpoch::new(1).expect("epoch"),
        snapshot.generation(),
        DesktopHistoryRangeGeneration::new(1).expect("range generation"),
        DesktopHistoryRangePreset::Recent1Day,
    );
    drop(shell);
    notifier.history_range_terminal(intent);
    let observer = bridge.observer();
    let watcher = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if observer.snapshot().is_some_and(|snapshot| {
                snapshot.phase() == tokenmaster_desktop::DesktopBridgePhase::Closed
            }) {
                return slint::quit_event_loop();
            }
            if std::time::Instant::now() >= deadline {
                return slint::quit_event_loop();
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    slint::run_event_loop().expect("headless event loop");
    watcher
        .join()
        .expect("window-close watcher")
        .expect("event loop quit");
    assert_eq!(
        bridge.snapshot().phase(),
        tokenmaster_desktop::DesktopBridgePhase::Closed
    );
    controller.shutdown().expect("controller stops");
    queued_snapshot_is_applied_before_the_exact_history_terminal_rollback();
}

fn queued_snapshot_is_applied_before_the_exact_history_terminal_rollback() {
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let archive = directory.path().join("history-terminal-order.sqlite3");
    drop(UsageStore::open(&archive).expect("create archive"));

    let initial = ProductReducer::new().snapshot();
    let shell = DesktopShell::new(&initial).expect("desktop shell");
    let mut controller = DesktopController::open(
        &archive,
        DesktopQueryPlan::overview().expect("overview plan"),
    )
    .expect("controller");
    let bridge = shell
        .snapshot_bridge(controller.snapshot_receiver())
        .expect("bridge");
    controller
        .bind_snapshot_epoch(bridge.epoch())
        .expect("bind epoch");
    controller
        .attach_snapshot_notifier(bridge.notifier())
        .expect("snapshot attachment");
    let terminal = bridge.terminal_history_range_notifier();
    controller
        .attach_terminal_history_range_notifier(terminal.clone())
        .expect("history terminal attachment");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("initial refresh");
    run_until_delivery(&bridge, 1);

    let intent = shell
        .request_history_range(DesktopHistoryRangePreset::Recent1Day)
        .expect("UI range request");
    controller
        .request_history_range(intent)
        .expect("controller range admission");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !controller
        .snapshot_receiver()
        .has_snapshot()
        .expect("snapshot mailbox")
    {
        assert!(
            std::time::Instant::now() < deadline,
            "range snapshot timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    terminal.history_range_terminal(intent);
    run_until_delivery(&bridge, 2);

    assert_eq!(
        shell.history_range_state().expect("history range state"),
        (DesktopHistoryRangePreset::Recent1Day, false)
    );

    controller.shutdown().expect("controller stops");
}
