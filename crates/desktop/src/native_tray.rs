use std::{cell::Cell, rc::Rc};

use crate::{DesktopLifecycleIntentSink, DesktopTrayAvailability};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DesktopNativeTrayError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopWindowActivationError;

#[cfg(target_os = "windows")]
mod platform {
    // SAFETY BOUNDARY: this module is the sole production Win32 tray owner. Construction,
    // callbacks, and destruction all run on Slint/winit's UI thread. The boxed callback
    // state keeps a stable address; GWLP_USERDATA is cleared before any native handle or
    // the box is destroyed; and no Rust panic is permitted to cross window_proc.
    use std::{
        cell::Cell,
        ffi::c_void,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
        sync::atomic::{AtomicBool, AtomicU32, Ordering},
    };

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
            Graphics::Gdi::{
                BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection,
                DIB_RGB_COLORS, DeleteObject, GetDC, ReleaseDC,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::{
                Shell::{
                    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
                    Shell_NotifyIconW,
                },
                WindowsAndMessaging::{
                    AppendMenuW, BringWindowToTop, CreateIconIndirect, CreatePopupMenu,
                    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow,
                    GWLP_USERDATA, GetCursorPos, GetWindowLongPtrW, HICON, HMENU, ICONINFO,
                    MF_SEPARATOR, MF_STRING, PostMessageW, RegisterClassW, RegisterWindowMessageW,
                    SW_RESTORE, SetForegroundWindow, SetWindowLongPtrW, ShowWindow,
                    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
                    WM_APP, WM_CONTEXTMENU, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
                    WS_EX_TOOLWINDOW, WS_POPUP,
                },
            },
        },
        core::{HSTRING, PCWSTR, w},
    };

    use crate::{
        DesktopLifecycleIntent, DesktopLifecycleIntentSink, DesktopTrayAvailability,
        native_tray::{DesktopNativeTrayError, DesktopWindowActivationError},
    };

    const WM_TRAY_ICON: u32 = WM_APP + 0x41;
    const TRAY_ID: u32 = 1;
    const COMMAND_SHOW: u32 = 0x100;
    const COMMAND_DASHBOARD: u32 = 0x101;
    const COMMAND_COMPACT: u32 = 0x102;
    const COMMAND_HIDE: u32 = 0x103;
    const COMMAND_QUIT: u32 = 0x104;
    const CLASS_NAME: PCWSTR = w!("TokenMasterNativeTrayWindow");
    const TOOLTIP: &str = "TokenMaster - local usage intelligence";

    static OWNER_ACTIVE: AtomicBool = AtomicBool::new(false);
    static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);
    static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

    pub(super) struct NativeTrayOwner {
        inner: Box<Inner>,
    }

    struct Inner {
        hwnd: HWND,
        icon: HICON,
        menu: HMENU,
        sink: Rc<dyn DesktopLifecycleIntentSink>,
        availability: Rc<Cell<DesktopTrayAvailability>>,
        registered: Cell<bool>,
    }

    struct OwnerReservation {
        committed: bool,
    }

    impl OwnerReservation {
        fn acquire() -> Result<Self, DesktopNativeTrayError> {
            OWNER_ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .map_err(|_| DesktopNativeTrayError)?;
            Ok(Self { committed: false })
        }

        fn commit(mut self) {
            self.committed = true;
        }
    }

    impl Drop for OwnerReservation {
        fn drop(&mut self) {
            if !self.committed {
                OWNER_ACTIVE.store(false, Ordering::Release);
            }
        }
    }

    impl Inner {
        fn submit(&self, intent: DesktopLifecycleIntent) {
            self.submit_with_panic_handler(intent, || {
                let _ = slint::quit_event_loop();
            });
        }

        fn submit_with_panic_handler(
            &self,
            intent: DesktopLifecycleIntent,
            on_panic: impl FnOnce(),
        ) {
            if catch_unwind(AssertUnwindSafe(|| self.sink.submit(intent))).is_err() {
                self.availability.set(DesktopTrayAvailability::Unavailable);
                on_panic();
            }
        }

        fn set_available(&self, available: bool) {
            let next = if available {
                DesktopTrayAvailability::Available
            } else {
                DesktopTrayAvailability::Unavailable
            };
            self.registered.set(available);
            self.availability.set(next);
            if !available {
                self.submit(DesktopLifecycleIntent::Show);
            }
        }
    }

    impl NativeTrayOwner {
        pub(super) fn new(
            sink: Rc<dyn DesktopLifecycleIntentSink>,
            availability: Rc<Cell<DesktopTrayAvailability>>,
        ) -> Result<Self, DesktopNativeTrayError> {
            let reservation = OwnerReservation::acquire()?;
            register_window_class()?;
            if taskbar_created_message() == 0 {
                return Err(DesktopNativeTrayError);
            }
            let instance = unsafe { GetModuleHandleW(None) }.map_err(|_| DesktopNativeTrayError)?;
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW,
                    CLASS_NAME,
                    PCWSTR::null(),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
            }
            .map_err(|_| DesktopNativeTrayError)?;

            let icon = match create_tokenmaster_icon() {
                Ok(icon) => icon,
                Err(error) => {
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                    return Err(error);
                }
            };
            let menu = match create_menu() {
                Ok(menu) => menu,
                Err(error) => {
                    unsafe {
                        let _ = DestroyIcon(icon);
                        let _ = DestroyWindow(hwnd);
                    }
                    return Err(error);
                }
            };
            let inner = Box::new(Inner {
                hwnd,
                icon,
                menu,
                sink,
                availability,
                registered: Cell::new(false),
            });
            let callback_state = (&raw const *inner).cast::<c_void>();
            unsafe {
                SetWindowLongPtrW(inner.hwnd, GWLP_USERDATA, callback_state as isize);
            }
            let installed = unsafe { GetWindowLongPtrW(inner.hwnd, GWLP_USERDATA) };
            if installed != callback_state as isize {
                unsafe {
                    SetWindowLongPtrW(inner.hwnd, GWLP_USERDATA, 0);
                    let _ = DestroyMenu(inner.menu);
                    let _ = DestroyIcon(inner.icon);
                    let _ = DestroyWindow(inner.hwnd);
                }
                return Err(DesktopNativeTrayError);
            }

            let tip = HSTRING::from(TOOLTIP);
            let data = notify_icon_data(inner.hwnd, inner.icon, &tip);
            if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
                unsafe {
                    SetWindowLongPtrW(inner.hwnd, GWLP_USERDATA, 0);
                    let _ = DestroyMenu(inner.menu);
                    let _ = DestroyIcon(inner.icon);
                    let _ = DestroyWindow(inner.hwnd);
                }
                return Err(DesktopNativeTrayError);
            }
            inner.registered.set(true);
            inner.availability.set(DesktopTrayAvailability::Available);
            reservation.commit();
            Ok(Self { inner })
        }
    }

    impl Drop for NativeTrayOwner {
        fn drop(&mut self) {
            self.inner
                .availability
                .set(DesktopTrayAvailability::Unavailable);
            unsafe {
                SetWindowLongPtrW(self.inner.hwnd, GWLP_USERDATA, 0);
                if self.inner.registered.get() {
                    let data = tray_identity(self.inner.hwnd);
                    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                }
                let _ = DestroyMenu(self.inner.menu);
                let _ = DestroyWindow(self.inner.hwnd);
                let _ = DestroyIcon(self.inner.icon);
            }
            OWNER_ACTIVE.store(false, Ordering::Release);
        }
    }

    pub(super) fn activate_window(
        window: &slint::Window,
    ) -> Result<(), DesktopWindowActivationError> {
        let provider = window.window_handle();
        let borrowed = provider
            .window_handle()
            .map_err(|_| DesktopWindowActivationError)?;
        let RawWindowHandle::Win32(handle) = borrowed.as_raw() else {
            return Err(DesktopWindowActivationError);
        };
        let hwnd = HWND(handle.hwnd.get() as *mut c_void);
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            BringWindowToTop(hwnd).map_err(|_| DesktopWindowActivationError)?;
            if !SetForegroundWindow(hwnd).as_bool() {
                return Err(DesktopWindowActivationError);
            }
        }
        Ok(())
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_TRAY_ICON {
            let event = (lparam.0 as u32) & 0xffff;
            let inner = unsafe { inner_from_window(hwnd) };
            if let Some(inner) = inner {
                if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    show_menu(inner);
                } else if event == WM_LBUTTONUP {
                    inner.submit(DesktopLifecycleIntent::Show);
                }
            }
            return LRESULT(0);
        }

        if message == taskbar_created_message() && message != 0 {
            if let Some(inner) = unsafe { inner_from_window(hwnd) } {
                let tip = HSTRING::from(TOOLTIP);
                let data = notify_icon_data(hwnd, inner.icon, &tip);
                let restored = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
                inner.set_available(restored);
            }
            return LRESULT(0);
        }

        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    unsafe fn inner_from_window(hwnd: HWND) -> Option<&'static Inner> {
        let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Inner;
        unsafe { pointer.as_ref() }
    }

    fn show_menu(inner: &Inner) {
        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&raw mut cursor) }.is_err() {
            return;
        }
        let _ = unsafe { SetForegroundWindow(inner.hwnd) };
        let command = unsafe {
            TrackPopupMenu(
                inner.menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                cursor.x,
                cursor.y,
                None,
                inner.hwnd,
                None,
            )
        }
        .0 as u32;
        let _ = unsafe { PostMessageW(Some(inner.hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };
        if let Some(intent) = command_intent(command) {
            inner.submit(intent);
        }
    }

    const fn command_intent(command: u32) -> Option<DesktopLifecycleIntent> {
        match command {
            COMMAND_SHOW => Some(DesktopLifecycleIntent::Show),
            COMMAND_DASHBOARD => Some(DesktopLifecycleIntent::OpenDashboard),
            COMMAND_COMPACT => Some(DesktopLifecycleIntent::OpenCompact),
            COMMAND_HIDE => Some(DesktopLifecycleIntent::Hide),
            COMMAND_QUIT => Some(DesktopLifecycleIntent::Quit),
            _ => None,
        }
    }

    fn register_window_class() -> Result<(), DesktopNativeTrayError> {
        if CLASS_REGISTERED.load(Ordering::Acquire) {
            return Ok(());
        }
        let instance = unsafe { GetModuleHandleW(None) }.map_err(|_| DesktopNativeTrayError)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        let _ = unsafe { RegisterClassW(&raw const class) };
        CLASS_REGISTERED.store(true, Ordering::Release);
        Ok(())
    }

    fn taskbar_created_message() -> u32 {
        let current = TASKBAR_CREATED.load(Ordering::Acquire);
        if current != 0 {
            return current;
        }
        let message = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        if message != 0 {
            TASKBAR_CREATED.store(message, Ordering::Release);
        }
        message
    }

    fn create_menu() -> Result<HMENU, DesktopNativeTrayError> {
        let menu = unsafe { CreatePopupMenu() }.map_err(|_| DesktopNativeTrayError)?;
        let result = (|| {
            append_menu_item(menu, COMMAND_SHOW, "Show")?;
            append_menu_item(menu, COMMAND_DASHBOARD, "Open Dashboard")?;
            append_menu_item(menu, COMMAND_COMPACT, "Open Compact")?;
            append_menu_item(menu, COMMAND_HIDE, "Hide")?;
            unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) }
                .map_err(|_| DesktopNativeTrayError)?;
            append_menu_item(menu, COMMAND_QUIT, "Quit")
        })();
        if result.is_err() {
            unsafe {
                let _ = DestroyMenu(menu);
            }
        }
        result.map(|()| menu)
    }

    fn append_menu_item(
        menu: HMENU,
        command: u32,
        title: &str,
    ) -> Result<(), DesktopNativeTrayError> {
        let title = HSTRING::from(title);
        unsafe { AppendMenuW(menu, MF_STRING, command as usize, &title) }
            .map_err(|_| DesktopNativeTrayError)
    }

    fn notify_icon_data(hwnd: HWND, icon: HICON, tooltip: &[u16]) -> NOTIFYICONDATAW {
        let mut buffer = [0_u16; 128];
        let length = tooltip.len().min(buffer.len() - 1);
        buffer[..length].copy_from_slice(&tooltip[..length]);
        NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: icon,
            szTip: buffer,
            ..Default::default()
        }
    }

    fn tray_identity(hwnd: HWND) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            ..Default::default()
        }
    }

    fn create_tokenmaster_icon() -> Result<HICON, DesktopNativeTrayError> {
        const SIZE: i32 = 32;
        let pixels = tokenmaster_icon_pixels();
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE,
                biHeight: -SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = unsafe { GetDC(None) };
        let mut bits = std::ptr::null_mut();
        let color =
            unsafe { CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &raw mut bits, None, 0) };
        let _ = unsafe { ReleaseDC(None, dc) };
        let color = color.map_err(|_| DesktopNativeTrayError)?;
        if bits.is_null() {
            unsafe {
                let _ = DeleteObject(color.into());
            }
            return Err(DesktopNativeTrayError);
        }
        unsafe {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
        }
        let mask = unsafe { CreateBitmap(SIZE, SIZE, 1, 1, None) };
        if mask.is_invalid() {
            unsafe {
                let _ = DeleteObject(color.into());
            }
            return Err(DesktopNativeTrayError);
        }
        let info = ICONINFO {
            fIcon: true.into(),
            hbmMask: mask,
            hbmColor: color,
            ..Default::default()
        };
        let icon =
            unsafe { CreateIconIndirect(&raw const info) }.map_err(|_| DesktopNativeTrayError);
        unsafe {
            let _ = DeleteObject(mask.into());
            let _ = DeleteObject(color.into());
        }
        icon
    }

    fn tokenmaster_icon_pixels() -> [u8; 32 * 32 * 4] {
        const SIZE: usize = 32;
        let mut pixels = [0_u8; SIZE * SIZE * 4];
        for y in 0..SIZE {
            for x in 0..SIZE {
                let source_x = (x as f32 * 2.0) + 1.0;
                let source_y = (y as f32 * 2.0) + 1.0;
                let outer = inside_hex(source_x, source_y, 9.0, 55.0, 4.0, 60.0);
                let inner = inside_hex(source_x, source_y, 14.0, 50.0, 10.0, 54.0);
                let white_mark = (9..=22).contains(&x) && (11..=14).contains(&y)
                    || (14..=17).contains(&x) && (14..=25).contains(&y)
                    || (8..=11).contains(&x) && (17..=22).contains(&y)
                    || (20..=23).contains(&x) && (17..=22).contains(&y);
                let star = (x == 16 && (5..=9).contains(&y)) || (y == 7 && (14..=18).contains(&x));
                let rgba = if white_mark {
                    [255, 255, 255, 255]
                } else if star || (outer && !inner) {
                    [155, 255, 0, 255]
                } else if inner {
                    [6, 16, 11, 255]
                } else {
                    [0, 0, 0, 0]
                };
                let offset = (y * SIZE + x) * 4;
                pixels[offset] = rgba[2];
                pixels[offset + 1] = rgba[1];
                pixels[offset + 2] = rgba[0];
                pixels[offset + 3] = rgba[3];
            }
        }
        pixels
    }

    fn inside_hex(x: f32, y: f32, left: f32, right: f32, top: f32, bottom: f32) -> bool {
        if y < top || y > bottom {
            return false;
        }
        let center_y = (top + bottom) / 2.0;
        let half_height = (bottom - top) / 2.0;
        let center_x = (left + right) / 2.0;
        let half_width = (right - left) / 2.0;
        let taper = ((y - center_y).abs() / half_height).min(1.0);
        let allowed = half_width * (1.0 - (taper * 0.48));
        (x - center_x).abs() <= allowed
    }

    #[cfg(test)]
    mod tests {
        use std::{
            cell::{Cell, RefCell},
            rc::Rc,
        };

        use windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{HICON, HMENU},
        };

        use super::{
            COMMAND_COMPACT, COMMAND_DASHBOARD, COMMAND_HIDE, COMMAND_QUIT, COMMAND_SHOW, Inner,
            command_intent, tokenmaster_icon_pixels,
        };
        use crate::{
            DesktopLifecycleIntent, DesktopLifecycleIntentAdmission, DesktopLifecycleIntentSink,
            DesktopTrayAvailability,
        };

        struct PanickingSink;

        #[derive(Default)]
        struct RecordingSink {
            intents: RefCell<Vec<DesktopLifecycleIntent>>,
        }

        impl DesktopLifecycleIntentSink for PanickingSink {
            fn submit(&self, _intent: DesktopLifecycleIntent) -> DesktopLifecycleIntentAdmission {
                panic!("bounded test panic")
            }
        }

        impl DesktopLifecycleIntentSink for RecordingSink {
            fn submit(&self, intent: DesktopLifecycleIntent) -> DesktopLifecycleIntentAdmission {
                self.intents.borrow_mut().push(intent);
                DesktopLifecycleIntentAdmission::Accepted
            }
        }

        #[test]
        fn native_menu_ids_map_to_the_five_typed_intents() {
            assert_eq!(
                command_intent(COMMAND_SHOW),
                Some(DesktopLifecycleIntent::Show)
            );
            assert_eq!(
                command_intent(COMMAND_DASHBOARD),
                Some(DesktopLifecycleIntent::OpenDashboard)
            );
            assert_eq!(
                command_intent(COMMAND_COMPACT),
                Some(DesktopLifecycleIntent::OpenCompact)
            );
            assert_eq!(
                command_intent(COMMAND_HIDE),
                Some(DesktopLifecycleIntent::Hide)
            );
            assert_eq!(
                command_intent(COMMAND_QUIT),
                Some(DesktopLifecycleIntent::Quit)
            );
            assert_eq!(command_intent(0), None);
        }

        #[test]
        fn generated_icon_is_bounded_bgra_with_transparent_corners() {
            let pixels = tokenmaster_icon_pixels();
            assert_eq!(pixels.len(), 32 * 32 * 4);
            assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
            let center = (16 * 32 + 16) * 4;
            assert_eq!(&pixels[center..center + 4], &[255, 255, 255, 255]);
        }

        #[test]
        fn native_callback_panic_is_contained_and_fails_closed() {
            let availability = Rc::new(Cell::new(DesktopTrayAvailability::Available));
            let quit_requested = Cell::new(false);
            let inner = Inner {
                hwnd: HWND::default(),
                icon: HICON::default(),
                menu: HMENU::default(),
                sink: Rc::new(PanickingSink),
                availability: Rc::clone(&availability),
                registered: Cell::new(false),
            };

            inner.submit_with_panic_handler(DesktopLifecycleIntent::Show, || {
                quit_requested.set(true);
            });

            assert_eq!(availability.get(), DesktopTrayAvailability::Unavailable);
            assert!(quit_requested.get());
        }

        #[test]
        fn failed_explorer_readd_marks_unavailable_and_requests_visible_fallback() {
            let availability = Rc::new(Cell::new(DesktopTrayAvailability::Available));
            let sink = Rc::new(RecordingSink::default());
            let inner = Inner {
                hwnd: HWND::default(),
                icon: HICON::default(),
                menu: HMENU::default(),
                sink: sink.clone(),
                availability: Rc::clone(&availability),
                registered: Cell::new(true),
            };

            inner.set_available(false);

            assert_eq!(availability.get(), DesktopTrayAvailability::Unavailable);
            assert!(!inner.registered.get());
            assert_eq!(
                sink.intents.borrow().as_slice(),
                &[DesktopLifecycleIntent::Show]
            );

            inner.set_available(true);
            assert_eq!(availability.get(), DesktopTrayAvailability::Available);
            assert!(inner.registered.get());
            assert_eq!(sink.intents.borrow().len(), 1);
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use std::{cell::Cell, rc::Rc};

    use crate::{
        DesktopLifecycleIntentSink, DesktopTrayAvailability,
        native_tray::{DesktopNativeTrayError, DesktopWindowActivationError},
    };

    pub(super) struct NativeTrayOwner;

    impl NativeTrayOwner {
        pub(super) fn new(
            _sink: Rc<dyn DesktopLifecycleIntentSink>,
            _availability: Rc<Cell<DesktopTrayAvailability>>,
        ) -> Result<Self, DesktopNativeTrayError> {
            Err(DesktopNativeTrayError)
        }
    }

    pub(super) fn activate_window(
        _window: &slint::Window,
    ) -> Result<(), DesktopWindowActivationError> {
        Ok(())
    }
}

pub(crate) struct DesktopNativeTrayOwner {
    _platform: platform::NativeTrayOwner,
}

impl DesktopNativeTrayOwner {
    pub(crate) fn new(
        sink: Rc<dyn DesktopLifecycleIntentSink>,
        availability: Rc<Cell<DesktopTrayAvailability>>,
    ) -> Result<Self, DesktopNativeTrayError> {
        platform::NativeTrayOwner::new(sink, availability).map(|platform| Self {
            _platform: platform,
        })
    }
}

pub fn activate_window(window: &slint::Window) -> Result<(), DesktopWindowActivationError> {
    platform::activate_window(window)
}
