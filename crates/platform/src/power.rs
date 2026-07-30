use std::fmt;

/// Pathless lifecycle notification produced by the operating-system power adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PowerLifecycleEvent {
    Suspend = 1,
    Resume = 2,
}

/// Fixed operational view of the capacity-one suspend/resume signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerMonitorSnapshot {
    pending: Option<PowerLifecycleEvent>,
    accepted_count: u64,
    coalesced_count: u64,
    unknown_count: u64,
    overflowed: bool,
}

impl PowerMonitorSnapshot {
    #[must_use]
    pub const fn pending(self) -> Option<PowerLifecycleEvent> {
        self.pending
    }

    #[must_use]
    pub const fn accepted_count(self) -> u64 {
        self.accepted_count
    }

    #[must_use]
    pub const fn coalesced_count(self) -> u64 {
        self.coalesced_count
    }

    #[must_use]
    pub const fn unknown_count(self) -> u64 {
        self.unknown_count
    }

    #[must_use]
    pub const fn overflowed(self) -> bool {
        self.overflowed
    }
}

/// Stable failures for process-wide suspend/resume registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PowerMonitorError {
    #[error("power notifications are unavailable on this platform")]
    Unavailable,
    #[error("power notification monitor is already registered")]
    AlreadyRegistered,
    #[error("power notification registration failed")]
    RegistrationFailed,
    #[error("power notification unregistration failed")]
    UnregistrationFailed,
}

/// Process-wide OS registration backed by one capacity-one atomic signal.
pub struct SuspendResumeMonitor {
    registration: Option<imp::Registration>,
}

impl SuspendResumeMonitor {
    /// Registers the single process power callback.
    pub fn subscribe() -> Result<Self, PowerMonitorError> {
        Ok(Self {
            registration: Some(imp::Registration::subscribe()?),
        })
    }

    /// Removes and returns the latest pending event, if any.
    #[must_use]
    pub fn take_pending(&self) -> Option<PowerLifecycleEvent> {
        if self.registration.is_some() {
            imp::take_pending()
        } else {
            None
        }
    }

    /// Returns fixed counters without exposing an OS handle or callback context.
    #[must_use]
    pub fn snapshot(&self) -> PowerMonitorSnapshot {
        if self.registration.is_some() {
            imp::snapshot()
        } else {
            PowerMonitorSnapshot {
                pending: None,
                accepted_count: 0,
                coalesced_count: 0,
                unknown_count: 0,
                overflowed: false,
            }
        }
    }

    /// Unregisters synchronously. `Drop` also performs the same best-effort cleanup.
    pub fn shutdown(&mut self) -> Result<(), PowerMonitorError> {
        self.close()
    }

    fn close(&mut self) -> Result<(), PowerMonitorError> {
        let Some(mut registration) = self.registration.take() else {
            return Ok(());
        };
        if let Err(error) = registration.close() {
            self.registration = Some(registration);
            return Err(error);
        }
        Ok(())
    }
}

impl fmt::Debug for SuspendResumeMonitor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SuspendResumeMonitor")
            .field("active", &self.registration.is_some())
            .field("signal", &self.snapshot())
            .finish()
    }
}

impl Drop for SuspendResumeMonitor {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

    use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
    use windows::Win32::System::Power::{
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY, RegisterSuspendResumeNotification,
        UnregisterSuspendResumeNotification,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMECRITICAL,
        PBT_APMRESUMESTANDBY, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    };

    use super::{PowerLifecycleEvent, PowerMonitorError, PowerMonitorSnapshot};

    const NONE: u8 = 0;

    static REGISTERED: AtomicBool = AtomicBool::new(false);
    static SIGNAL: PowerSignal = PowerSignal::new();

    struct PowerSignal {
        pending: AtomicU8,
        accepted_count: AtomicU64,
        coalesced_count: AtomicU64,
        unknown_count: AtomicU64,
        overflowed: AtomicBool,
    }

    impl PowerSignal {
        const fn new() -> Self {
            Self {
                pending: AtomicU8::new(NONE),
                accepted_count: AtomicU64::new(0),
                coalesced_count: AtomicU64::new(0),
                unknown_count: AtomicU64::new(0),
                overflowed: AtomicBool::new(false),
            }
        }

        fn reset(&self) {
            self.pending.store(NONE, Ordering::Release);
            self.accepted_count.store(0, Ordering::Release);
            self.coalesced_count.store(0, Ordering::Release);
            self.unknown_count.store(0, Ordering::Release);
            self.overflowed.store(false, Ordering::Release);
        }

        fn record_notification(&self, notification: u32) {
            let event = match notification {
                PBT_APMSUSPEND => PowerLifecycleEvent::Suspend,
                PBT_APMRESUMEAUTOMATIC
                | PBT_APMRESUMECRITICAL
                | PBT_APMRESUMESTANDBY
                | PBT_APMRESUMESUSPEND => PowerLifecycleEvent::Resume,
                _ => {
                    self.increment(&self.unknown_count);
                    return;
                }
            };
            let previous = self.pending.swap(event as u8, Ordering::AcqRel);
            if previous == event as u8 {
                self.increment(&self.coalesced_count);
            } else {
                self.increment(&self.accepted_count);
            }
        }

        fn take_pending(&self) -> Option<PowerLifecycleEvent> {
            decode(self.pending.swap(NONE, Ordering::AcqRel))
        }

        fn snapshot(&self) -> PowerMonitorSnapshot {
            PowerMonitorSnapshot {
                pending: decode(self.pending.load(Ordering::Acquire)),
                accepted_count: self.accepted_count.load(Ordering::Acquire),
                coalesced_count: self.coalesced_count.load(Ordering::Acquire),
                unknown_count: self.unknown_count.load(Ordering::Acquire),
                overflowed: self.overflowed.load(Ordering::Acquire),
            }
        }

        fn increment(&self, counter: &AtomicU64) {
            if counter
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                    value.checked_add(1)
                })
                .is_err()
            {
                self.overflowed.store(true, Ordering::Release);
            }
        }
    }

    fn decode(value: u8) -> Option<PowerLifecycleEvent> {
        match value {
            value if value == PowerLifecycleEvent::Suspend as u8 => {
                Some(PowerLifecycleEvent::Suspend)
            }
            value if value == PowerLifecycleEvent::Resume as u8 => {
                Some(PowerLifecycleEvent::Resume)
            }
            _ => None,
        }
    }

    pub(super) struct Registration {
        handle: Option<HPOWERNOTIFY>,
    }

    impl Registration {
        pub(super) fn subscribe() -> Result<Self, PowerMonitorError> {
            if REGISTERED
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(PowerMonitorError::AlreadyRegistered);
            }
            SIGNAL.reset();
            let parameters = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
                Callback: Some(power_callback),
                Context: ptr::null_mut(),
            };
            let recipient = HANDLE(ptr::from_ref(&parameters).cast_mut().cast::<c_void>());
            let handle =
                unsafe { RegisterSuspendResumeNotification(recipient, DEVICE_NOTIFY_CALLBACK) };
            match handle {
                Ok(handle) => Ok(Self {
                    handle: Some(handle),
                }),
                Err(_) => {
                    REGISTERED.store(false, Ordering::Release);
                    Err(PowerMonitorError::RegistrationFailed)
                }
            }
        }

        pub(super) fn close(&mut self) -> Result<(), PowerMonitorError> {
            let Some(handle) = self.handle else {
                return Ok(());
            };
            if unsafe { UnregisterSuspendResumeNotification(handle) }.is_err() {
                return Err(PowerMonitorError::UnregistrationFailed);
            }
            self.handle = None;
            SIGNAL.reset();
            REGISTERED.store(false, Ordering::Release);
            Ok(())
        }
    }

    pub(super) fn take_pending() -> Option<PowerLifecycleEvent> {
        SIGNAL.take_pending()
    }

    pub(super) fn snapshot() -> PowerMonitorSnapshot {
        SIGNAL.snapshot()
    }

    unsafe extern "system" fn power_callback(
        _context: *const c_void,
        notification: u32,
        _setting: *const c_void,
    ) -> u32 {
        SIGNAL.record_notification(notification);
        ERROR_SUCCESS.0
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn signal_is_capacity_one_last_wins_and_checked() {
            let signal = PowerSignal::new();
            signal.record_notification(PBT_APMSUSPEND);
            signal.record_notification(PBT_APMSUSPEND);
            signal.record_notification(PBT_APMRESUMEAUTOMATIC);
            signal.record_notification(u32::MAX);

            assert_eq!(signal.take_pending(), Some(PowerLifecycleEvent::Resume));
            assert_eq!(signal.take_pending(), None);
            let snapshot = signal.snapshot();
            assert_eq!(snapshot.accepted_count(), 2);
            assert_eq!(snapshot.coalesced_count(), 1);
            assert_eq!(snapshot.unknown_count(), 1);
            assert!(!snapshot.overflowed());

            signal.unknown_count.store(u64::MAX, Ordering::Release);
            signal.record_notification(u32::MAX);
            assert!(signal.snapshot().overflowed());
        }

        #[test]
        fn every_resume_form_is_last_wins_without_suspend_precondition() {
            for notification in [
                PBT_APMRESUMEAUTOMATIC,
                PBT_APMRESUMECRITICAL,
                PBT_APMRESUMESTANDBY,
                PBT_APMRESUMESUSPEND,
            ] {
                let signal = PowerSignal::new();
                signal.record_notification(notification);
                signal.record_notification(notification);
                assert_eq!(signal.take_pending(), Some(PowerLifecycleEvent::Resume));
                let snapshot = signal.snapshot();
                assert_eq!(snapshot.accepted_count(), 1);
                assert_eq!(snapshot.coalesced_count(), 1);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{PowerLifecycleEvent, PowerMonitorError, PowerMonitorSnapshot};

    pub(super) struct Registration;

    impl Registration {
        pub(super) fn subscribe() -> Result<Self, PowerMonitorError> {
            Err(PowerMonitorError::Unavailable)
        }

        pub(super) fn close(&mut self) -> Result<(), PowerMonitorError> {
            Ok(())
        }
    }

    pub(super) fn take_pending() -> Option<PowerLifecycleEvent> {
        None
    }

    pub(super) fn snapshot() -> PowerMonitorSnapshot {
        PowerMonitorSnapshot {
            pending: None,
            accepted_count: 0,
            coalesced_count: 0,
            unknown_count: 0,
            overflowed: false,
        }
    }
}
