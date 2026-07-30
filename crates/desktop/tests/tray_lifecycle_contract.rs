use std::{cell::RefCell, rc::Rc};

use tokenmaster_desktop::{
    DesktopLifecycleIntent, DesktopLifecycleIntentAdmission, DesktopLifecycleIntentRouter,
    DesktopLifecycleIntentSink,
};

#[derive(Default)]
struct RecordingLifecycleSink {
    intents: RefCell<Vec<DesktopLifecycleIntent>>,
}

impl DesktopLifecycleIntentSink for RecordingLifecycleSink {
    fn submit(&self, intent: DesktopLifecycleIntent) -> DesktopLifecycleIntentAdmission {
        self.intents.borrow_mut().push(intent);
        DesktopLifecycleIntentAdmission::Accepted
    }
}

#[test]
fn lifecycle_router_is_typed_single_install_and_queue_free() {
    let router = DesktopLifecycleIntentRouter::new();
    assert_eq!(
        router.submit(DesktopLifecycleIntent::Show),
        DesktopLifecycleIntentAdmission::Rejected
    );

    let sink = Rc::new(RecordingLifecycleSink::default());
    router.install(sink.clone()).expect("first install");
    assert!(router.install(sink.clone()).is_err());

    for intent in DesktopLifecycleIntent::ALL {
        assert_eq!(
            router.submit(intent),
            DesktopLifecycleIntentAdmission::Accepted
        );
    }
    assert_eq!(
        sink.intents.borrow().as_slice(),
        &DesktopLifecycleIntent::ALL
    );
    assert_eq!(
        DesktopLifecycleIntent::ALL,
        [
            DesktopLifecycleIntent::Show,
            DesktopLifecycleIntent::Hide,
            DesktopLifecycleIntent::OpenCompact,
            DesktopLifecycleIntent::OpenDashboard,
            DesktopLifecycleIntent::Quit,
        ]
    );
}
