use std::{fmt, sync::Arc};

/// Admission returned by the application-owned activation bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentSessionActivationAdmission {
    Accepted,
    Rejected,
}

/// Fixed, payload-free activation endpoint used by native current-session integration.
pub trait CurrentSessionActivationSink: Send + Sync + 'static {
    fn request_activation(&self) -> CurrentSessionActivationAdmission;
}

/// Bounded registration health for the fixed global shortcut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GlobalHotkeyHealth {
    NotStarted = 0,
    Registered = 1,
    Conflict = 2,
    Unavailable = 3,
}

/// Bounded lifecycle health for the one native integration thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CurrentSessionThreadHealth {
    NotStarted = 0,
    Running = 1,
    Stopped = 2,
    Faulted = 3,
    Unavailable = 4,
}

/// Path-free operational snapshot of current-session integration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentSessionIntegrationSnapshot {
    hotkey: GlobalHotkeyHealth,
    thread: CurrentSessionThreadHealth,
    activation_count: u64,
    rejected_count: u64,
    panicked_count: u64,
    overflowed: bool,
}

impl CurrentSessionIntegrationSnapshot {
    #[must_use]
    pub const fn hotkey(self) -> GlobalHotkeyHealth {
        self.hotkey
    }

    #[must_use]
    pub const fn thread(self) -> CurrentSessionThreadHealth {
        self.thread
    }

    #[must_use]
    pub const fn activation_count(self) -> u64 {
        self.activation_count
    }

    #[must_use]
    pub const fn rejected_count(self) -> u64 {
        self.rejected_count
    }

    #[must_use]
    pub const fn panicked_count(self) -> u64 {
        self.panicked_count
    }

    #[must_use]
    pub const fn overflowed(self) -> bool {
        self.overflowed
    }
}

/// Stable, path-free failures for current-session ownership and teardown.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CurrentSessionError {
    #[error("current-session ownership is unavailable")]
    OwnershipUnavailable,
    #[error("current-session activation signal failed")]
    SignalFailed,
    #[error("current-session integration is already started")]
    AlreadyStarted,
    #[error("current-session integration shutdown failed")]
    ShutdownFailed,
}

/// Successful claim result. A secondary receipt contains no reusable capability.
pub enum CurrentSessionClaim {
    Primary(CurrentSessionPrimary),
    Secondary(CurrentSessionSecondaryReceipt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentSessionSecondaryReceipt;

/// Namespace for acquiring the exact process/session boundary.
pub struct CurrentSessionIntegration;

impl CurrentSessionIntegration {
    pub fn claim() -> Result<CurrentSessionClaim, CurrentSessionError> {
        imp::claim()
    }
}

/// Opaque primary reservation and optional joined native integration owner.
pub struct CurrentSessionPrimary {
    inner: imp::Primary,
}

impl CurrentSessionPrimary {
    fn from_inner(inner: imp::Primary) -> Self {
        Self { inner }
    }

    /// Starts the exact native integration thread once. OS registration failures are
    /// represented by `snapshot()` health and do not disable the visible application.
    pub fn start(
        &mut self,
        sink: Arc<dyn CurrentSessionActivationSink>,
    ) -> Result<(), CurrentSessionError> {
        self.inner.start(sink)
    }

    #[must_use]
    pub fn snapshot(&self) -> CurrentSessionIntegrationSnapshot {
        self.inner.snapshot()
    }

    /// Signals, joins, and unregisters the one native owner before returning.
    pub fn shutdown(&mut self) -> Result<(), CurrentSessionError> {
        self.inner.shutdown()
    }
}

impl fmt::Debug for CurrentSessionPrimary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CurrentSessionPrimary")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl Drop for CurrentSessionPrimary {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(windows)]
mod imp {
    use std::{
        ffi::c_void,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
        },
        thread::{self, JoinHandle},
    };

    use windows::{
        Win32::{
            Foundation::{
                CloseHandle, ERROR_ALREADY_EXISTS, ERROR_HOTKEY_ALREADY_REGISTERED, GetLastError,
                HANDLE, WAIT_FAILED, WAIT_OBJECT_0,
            },
            System::Threading::{
                CreateEventExW, CreateEventW, EVENT_MODIFY_STATE, INFINITE,
                SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
            },
            UI::{
                Input::KeyboardAndMouse::{
                    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey,
                    UnregisterHotKey,
                },
                WindowsAndMessaging::{
                    MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW,
                    QS_ALLINPUT, WM_HOTKEY,
                },
            },
        },
        core::{HRESULT, PCWSTR, w},
    };

    use super::{
        CurrentSessionActivationAdmission, CurrentSessionActivationSink, CurrentSessionClaim,
        CurrentSessionError, CurrentSessionIntegrationSnapshot, CurrentSessionPrimary,
        CurrentSessionSecondaryReceipt, CurrentSessionThreadHealth, GlobalHotkeyHealth,
    };

    const ACTIVATION_EVENT_NAME: PCWSTR = w!("Local\\TokenMaster.CurrentSession.Activation.v1");
    const HOTKEY_ID: i32 = 0x544D;

    fn hotkey_modifiers() -> HOT_KEY_MODIFIERS {
        MOD_CONTROL | MOD_ALT | MOD_NOREPEAT
    }

    const VIRTUAL_KEY_T: u32 = 0x54;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum HotkeyRegistrationMode {
        Fixed,
        #[cfg(test)]
        Disabled,
    }

    struct OwnedHandle(HANDLE);

    impl OwnedHandle {
        const fn raw(&self) -> HANDLE {
            self.0
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    struct SharedState {
        hotkey: AtomicU8,
        thread: AtomicU8,
        activation_count: AtomicU64,
        rejected_count: AtomicU64,
        panicked_count: AtomicU64,
        overflowed: AtomicBool,
    }

    impl SharedState {
        const fn new() -> Self {
            Self {
                hotkey: AtomicU8::new(GlobalHotkeyHealth::NotStarted as u8),
                thread: AtomicU8::new(CurrentSessionThreadHealth::NotStarted as u8),
                activation_count: AtomicU64::new(0),
                rejected_count: AtomicU64::new(0),
                panicked_count: AtomicU64::new(0),
                overflowed: AtomicBool::new(false),
            }
        }

        fn snapshot(&self) -> CurrentSessionIntegrationSnapshot {
            CurrentSessionIntegrationSnapshot {
                hotkey: decode_hotkey(self.hotkey.load(Ordering::Acquire)),
                thread: decode_thread(self.thread.load(Ordering::Acquire)),
                activation_count: self.activation_count.load(Ordering::Acquire),
                rejected_count: self.rejected_count.load(Ordering::Acquire),
                panicked_count: self.panicked_count.load(Ordering::Acquire),
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

        fn dispatch(&self, sink: &dyn CurrentSessionActivationSink) {
            match catch_unwind(AssertUnwindSafe(|| sink.request_activation())) {
                Ok(CurrentSessionActivationAdmission::Accepted) => {
                    self.increment(&self.activation_count);
                }
                Ok(CurrentSessionActivationAdmission::Rejected) => {
                    self.increment(&self.rejected_count);
                }
                Err(_) => {
                    self.increment(&self.panicked_count);
                }
            }
        }
    }

    pub(super) struct Primary {
        activation: OwnedHandle,
        shutdown: Option<OwnedHandle>,
        thread: Option<JoinHandle<Result<(), CurrentSessionError>>>,
        shared: Arc<SharedState>,
        started: bool,
    }

    pub(super) fn claim() -> Result<CurrentSessionClaim, CurrentSessionError> {
        let desired_access = (EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE).0;
        let handle = unsafe {
            CreateEventExW(
                None,
                ACTIVATION_EVENT_NAME,
                Default::default(),
                desired_access,
            )
        }
        .map_err(|_| CurrentSessionError::OwnershipUnavailable)?;
        let activation = OwnedHandle(handle);
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { SetEvent(activation.raw()) }.map_err(|_| CurrentSessionError::SignalFailed)?;
            return Ok(CurrentSessionClaim::Secondary(
                CurrentSessionSecondaryReceipt,
            ));
        }
        Ok(CurrentSessionClaim::Primary(
            CurrentSessionPrimary::from_inner(Primary {
                activation,
                shutdown: None,
                thread: None,
                shared: Arc::new(SharedState::new()),
                started: false,
            }),
        ))
    }

    impl Primary {
        pub(super) fn start(
            &mut self,
            sink: Arc<dyn CurrentSessionActivationSink>,
        ) -> Result<(), CurrentSessionError> {
            if self.started {
                return Err(CurrentSessionError::AlreadyStarted);
            }
            self.started = true;
            let shutdown = match unsafe { CreateEventW(None, true, false, None) } {
                Ok(handle) => OwnedHandle(handle),
                Err(_) => {
                    self.shared
                        .hotkey
                        .store(GlobalHotkeyHealth::Unavailable as u8, Ordering::Release);
                    self.shared.thread.store(
                        CurrentSessionThreadHealth::Unavailable as u8,
                        Ordering::Release,
                    );
                    return Ok(());
                }
            };
            let activation_raw = self.activation.raw().0 as isize;
            let shutdown_raw = shutdown.raw().0 as isize;
            let shared = Arc::clone(&self.shared);
            let thread = thread::Builder::new()
                .name("tokenmaster-session-integration".to_owned())
                .spawn(move || {
                    let activation = HANDLE(activation_raw as *mut c_void);
                    let shutdown = HANDLE(shutdown_raw as *mut c_void);
                    run_thread(
                        activation,
                        shutdown,
                        sink,
                        &shared,
                        HotkeyRegistrationMode::Fixed,
                    )
                });
            match thread {
                Ok(thread) => {
                    self.shutdown = Some(shutdown);
                    self.thread = Some(thread);
                }
                Err(_) => {
                    self.shared
                        .hotkey
                        .store(GlobalHotkeyHealth::Unavailable as u8, Ordering::Release);
                    self.shared.thread.store(
                        CurrentSessionThreadHealth::Unavailable as u8,
                        Ordering::Release,
                    );
                }
            }
            Ok(())
        }

        pub(super) fn snapshot(&self) -> CurrentSessionIntegrationSnapshot {
            self.shared.snapshot()
        }

        pub(super) fn shutdown(&mut self) -> Result<(), CurrentSessionError> {
            let Some(thread) = self.thread.take() else {
                self.shutdown.take();
                return Ok(());
            };
            let signal_result = self
                .shutdown
                .as_ref()
                .ok_or(CurrentSessionError::ShutdownFailed)
                .and_then(|shutdown| {
                    unsafe { SetEvent(shutdown.raw()) }
                        .map_err(|_| CurrentSessionError::ShutdownFailed)
                });
            if signal_result.is_err() {
                self.thread = Some(thread);
                return signal_result;
            }
            let result = thread
                .join()
                .map_err(|_| CurrentSessionError::ShutdownFailed)?;
            self.shutdown.take();
            result
        }
    }

    fn run_thread(
        activation: HANDLE,
        shutdown: HANDLE,
        sink: Arc<dyn CurrentSessionActivationSink>,
        shared: &SharedState,
        hotkey_mode: HotkeyRegistrationMode,
    ) -> Result<(), CurrentSessionError> {
        let mut message = MSG::default();
        let _ = unsafe { PeekMessageW(&raw mut message, None, 0, 0, Default::default()) };
        let hotkey = match hotkey_mode {
            HotkeyRegistrationMode::Fixed => {
                match unsafe { RegisterHotKey(None, HOTKEY_ID, hotkey_modifiers(), VIRTUAL_KEY_T) }
                {
                    Ok(()) => GlobalHotkeyHealth::Registered,
                    Err(error)
                        if error.code()
                            == HRESULT::from_win32(ERROR_HOTKEY_ALREADY_REGISTERED.0) =>
                    {
                        GlobalHotkeyHealth::Conflict
                    }
                    Err(_) => GlobalHotkeyHealth::Unavailable,
                }
            }
            #[cfg(test)]
            HotkeyRegistrationMode::Disabled => GlobalHotkeyHealth::NotStarted,
        };
        shared.hotkey.store(hotkey as u8, Ordering::Release);
        shared
            .thread
            .store(CurrentSessionThreadHealth::Running as u8, Ordering::Release);

        let handles = [shutdown, activation];
        let loop_result = loop {
            let result = unsafe {
                MsgWaitForMultipleObjectsEx(
                    Some(&handles),
                    INFINITE,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                )
            };
            if result == WAIT_OBJECT_0 {
                break Ok(());
            }
            if result.0 == WAIT_OBJECT_0.0 + 1 {
                shared.dispatch(sink.as_ref());
                continue;
            }
            if result.0 == WAIT_OBJECT_0.0 + 2 {
                while unsafe { PeekMessageW(&raw mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                    if message.message == WM_HOTKEY && message.wParam.0 == HOTKEY_ID as usize {
                        shared.dispatch(sink.as_ref());
                    }
                }
                continue;
            }
            if result == WAIT_FAILED {
                break Err(CurrentSessionError::ShutdownFailed);
            }
        };

        let unregister_result = if hotkey_mode == HotkeyRegistrationMode::Fixed
            && hotkey == GlobalHotkeyHealth::Registered
        {
            unsafe { UnregisterHotKey(None, HOTKEY_ID) }
                .map_err(|_| CurrentSessionError::ShutdownFailed)
        } else {
            Ok(())
        };
        let result = loop_result.and(unregister_result);
        shared.thread.store(
            if result.is_ok() {
                CurrentSessionThreadHealth::Stopped as u8
            } else {
                CurrentSessionThreadHealth::Faulted as u8
            },
            Ordering::Release,
        );
        result
    }

    const fn decode_hotkey(value: u8) -> GlobalHotkeyHealth {
        match value {
            1 => GlobalHotkeyHealth::Registered,
            2 => GlobalHotkeyHealth::Conflict,
            3 => GlobalHotkeyHealth::Unavailable,
            _ => GlobalHotkeyHealth::NotStarted,
        }
    }

    const fn decode_thread(value: u8) -> CurrentSessionThreadHealth {
        match value {
            1 => CurrentSessionThreadHealth::Running,
            2 => CurrentSessionThreadHealth::Stopped,
            3 => CurrentSessionThreadHealth::Faulted,
            4 => CurrentSessionThreadHealth::Unavailable,
            _ => CurrentSessionThreadHealth::NotStarted,
        }
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
    mod tests {
        use std::{
            ffi::c_void,
            mem::size_of,
            process::Command,
            sync::{
                Arc,
                atomic::{AtomicU64, Ordering},
            },
            thread,
        };

        use windows::Win32::{
            Foundation::{CloseHandle, ERROR_NO_MORE_FILES, HANDLE},
            System::{
                Diagnostics::ToolHelp::{
                    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                    Thread32Next,
                },
                Threading::{
                    GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetCurrentProcessId,
                    GetGuiResources, GetProcessHandleCount,
                },
            },
        };

        use super::*;

        #[derive(Clone, Copy, Debug)]
        struct ResourceCounts {
            handles: u32,
            threads: u32,
            user_objects: u32,
            gdi_objects: u32,
        }

        const RESOURCE_CONTRACT_CHILD: &str = "TOKENMASTER_CURRENT_SESSION_RESOURCE_CONTRACT_CHILD";

        fn resource_counts() -> ResourceCounts {
            let process = unsafe { GetCurrentProcess() };
            let mut handles = 0_u32;
            unsafe { GetProcessHandleCount(process, &raw mut handles) }
                .expect("current-session process handle count");
            let user_objects = unsafe { GetGuiResources(process, GR_USEROBJECTS) };
            let gdi_objects = unsafe { GetGuiResources(process, GR_GDIOBJECTS) };

            let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }
                .expect("current-session thread snapshot");
            let process_id = unsafe { GetCurrentProcessId() };
            let mut entry = THREADENTRY32 {
                dwSize: u32::try_from(size_of::<THREADENTRY32>()).expect("thread entry size"),
                ..Default::default()
            };
            let mut threads = 0_u32;
            if unsafe { Thread32First(snapshot, &raw mut entry) }.is_ok() {
                loop {
                    if entry.th32OwnerProcessID == process_id {
                        threads = threads.checked_add(1).expect("bounded thread count");
                    }
                    match unsafe { Thread32Next(snapshot, &raw mut entry) } {
                        Ok(()) => {}
                        Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                        Err(error) => panic!("enumerate current-session threads: {error}"),
                    }
                }
            }
            unsafe { CloseHandle(snapshot) }.expect("close current-session thread snapshot");
            ResourceCounts {
                handles,
                threads,
                user_objects,
                gdi_objects,
            }
        }

        fn exercise_joined_owner_cycle() {
            let activation = OwnedHandle(
                unsafe { CreateEventW(None, false, false, None) }
                    .expect("unnamed activation event"),
            );
            let shutdown = OwnedHandle(
                unsafe { CreateEventW(None, true, false, None) }.expect("unnamed shutdown event"),
            );
            let activation_raw = activation.raw().0 as isize;
            let shutdown_raw = shutdown.raw().0 as isize;
            let shared = Arc::new(SharedState::new());
            let thread_shared = Arc::clone(&shared);
            let sink: Arc<dyn CurrentSessionActivationSink> =
                Arc::new(CountingSink(AtomicU64::new(0)));
            let owner = thread::spawn(move || {
                let activation = HANDLE(activation_raw as *mut c_void);
                let shutdown = HANDLE(shutdown_raw as *mut c_void);
                run_thread(
                    activation,
                    shutdown,
                    sink,
                    thread_shared.as_ref(),
                    HotkeyRegistrationMode::Disabled,
                )
            });
            unsafe { SetEvent(shutdown.raw()) }.expect("signal current-session shutdown");
            owner
                .join()
                .expect("join current-session test owner")
                .expect("current-session test owner shutdown");
            assert_eq!(
                shared.snapshot().thread(),
                CurrentSessionThreadHealth::Stopped
            );
        }

        struct CountingSink(AtomicU64);

        impl CurrentSessionActivationSink for CountingSink {
            fn request_activation(&self) -> CurrentSessionActivationAdmission {
                self.0.fetch_add(1, Ordering::AcqRel);
                CurrentSessionActivationAdmission::Accepted
            }
        }

        struct PanickingSink;

        impl CurrentSessionActivationSink for PanickingSink {
            fn request_activation(&self) -> CurrentSessionActivationAdmission {
                panic!("bounded activation panic")
            }
        }

        #[test]
        fn dispatch_counters_are_checked_and_sink_panic_is_contained() {
            let state = SharedState::new();
            let sink = CountingSink(AtomicU64::new(0));
            state.dispatch(&sink);
            state.dispatch(&PanickingSink);
            let snapshot = state.snapshot();
            assert_eq!(snapshot.activation_count(), 1);
            assert_eq!(snapshot.panicked_count(), 1);
            assert_eq!(sink.0.load(Ordering::Acquire), 1);
            assert!(!snapshot.overflowed());

            state.activation_count.store(u64::MAX, Ordering::Release);
            state.dispatch(&sink);
            assert!(state.snapshot().overflowed());
        }

        #[test]
        fn fixed_hotkey_metadata_is_stable() {
            assert_eq!(HOTKEY_ID, 0x544D);
            assert_eq!(VIRTUAL_KEY_T, 0x54);
            assert_eq!(hotkey_modifiers(), MOD_CONTROL | MOD_ALT | MOD_NOREPEAT);
        }

        fn assert_repeated_owner_cycles_return_native_resources() {
            exercise_joined_owner_cycle();
            let before = resource_counts();
            for _ in 0..4_096 {
                exercise_joined_owner_cycle();
            }
            let after = resource_counts();
            // This exact child test process has no unrelated library tests changing its
            // process-wide sample. The gate rejects growth above eight handles across the
            // complete 4,096 cycles.
            assert!(
                after.handles <= before.handles.saturating_add(8),
                "current-session handles grew: before={before:?}, after={after:?}"
            );
            assert!(
                after.threads <= before.threads.saturating_add(1),
                "current-session threads grew: before={before:?}, after={after:?}"
            );
            assert!(
                after.user_objects <= before.user_objects.saturating_add(1)
                    && after.gdi_objects <= before.gdi_objects.saturating_add(1),
                "current-session GUI objects grew: before={before:?}, after={after:?}"
            );
        }

        #[test]
        fn repeated_owner_cycles_return_native_resources() {
            if std::env::var_os(RESOURCE_CONTRACT_CHILD).is_some() {
                assert_repeated_owner_cycles_return_native_resources();
                return;
            }

            let status = Command::new(std::env::current_exe().expect("current test executable"))
                .args([
                    "--exact",
                    "current_session::imp::tests::repeated_owner_cycles_return_native_resources",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(RESOURCE_CONTRACT_CHILD, "1")
                .status()
                .expect("run isolated current-session resource contract");
            assert!(
                status.success(),
                "isolated current-session resource contract failed: {status}"
            );
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::Arc;

    use super::{
        CurrentSessionActivationSink, CurrentSessionClaim, CurrentSessionError,
        CurrentSessionIntegrationSnapshot, CurrentSessionPrimary, CurrentSessionSecondaryReceipt,
        CurrentSessionThreadHealth, GlobalHotkeyHealth,
    };

    pub(super) struct Primary {
        started: bool,
        stopped: bool,
    }

    pub(super) fn claim() -> Result<CurrentSessionClaim, CurrentSessionError> {
        Ok(CurrentSessionClaim::Primary(
            CurrentSessionPrimary::from_inner(Primary {
                started: false,
                stopped: false,
            }),
        ))
    }

    impl Primary {
        pub(super) fn start(
            &mut self,
            _sink: Arc<dyn CurrentSessionActivationSink>,
        ) -> Result<(), CurrentSessionError> {
            if self.started {
                return Err(CurrentSessionError::AlreadyStarted);
            }
            self.started = true;
            Ok(())
        }

        pub(super) const fn snapshot(&self) -> CurrentSessionIntegrationSnapshot {
            CurrentSessionIntegrationSnapshot {
                hotkey: GlobalHotkeyHealth::Unavailable,
                thread: if self.stopped {
                    CurrentSessionThreadHealth::Stopped
                } else {
                    CurrentSessionThreadHealth::Unavailable
                },
                activation_count: 0,
                rejected_count: 0,
                panicked_count: 0,
                overflowed: false,
            }
        }

        pub(super) fn shutdown(&mut self) -> Result<(), CurrentSessionError> {
            self.stopped = true;
            Ok(())
        }
    }

    const _: Option<CurrentSessionSecondaryReceipt> = None;
}
