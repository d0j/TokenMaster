use std::{thread, time::Duration};

use tokenmaster_desktop::{
    DesktopBridgePhase, DesktopController, DesktopQueryPlan, DesktopRefreshUrgency, DesktopShell,
};
use tokenmaster_product::ProductReducer;
use tokenmaster_store::UsageStore;

#[test]
fn controller_snapshot_reaches_the_real_headless_slint_event_loop() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let directory = tempfile::TempDir::new().expect("temporary directory");
    let archive = directory.path().join("event-loop.sqlite3");
    drop(UsageStore::open(&archive).expect("create schema-v13 archive"));

    let reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    let shell = DesktopShell::new(&initial).expect("desktop shell");
    let mut controller = DesktopController::open(
        &archive,
        DesktopQueryPlan::overview().expect("overview plan"),
    )
    .expect("controller");
    let bridge = shell.snapshot_bridge(controller.snapshot_receiver());
    controller
        .attach_snapshot_notifier(bridge.notifier())
        .expect("bridge notifier");
    controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("refresh admitted");

    let observer = bridge.observer();
    let watcher = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let delivered = loop {
            if let Some(snapshot) = observer.snapshot() {
                if snapshot.delivered_count() == 1 {
                    break true;
                }
                if snapshot.phase() != DesktopBridgePhase::Running {
                    break false;
                }
            }
            if std::time::Instant::now() >= deadline {
                break false;
            }
            thread::sleep(Duration::from_millis(1));
        };
        let quit = slint::quit_event_loop();
        (delivered, quit)
    });

    slint::run_event_loop().expect("headless event loop");
    let (delivered, quit) = watcher.join().expect("watcher joins");
    quit.expect("event loop quit request");
    assert!(delivered, "snapshot was not delivered before timeout");
    assert_eq!(bridge.snapshot().delivered_count(), 1);
    assert_ne!(shell.window().get_product_generation(), "0");

    controller.shutdown().expect("controller stops");
    drop(bridge);
    drop(shell);
}
