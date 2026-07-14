use std::{cell::RefCell, rc::Rc};

use slint::{CloseRequestResponse, ComponentHandle};

use crate::{
    MainWindow, TokenMasterTray,
    lifecycle::{Action, Lifecycle},
};

pub(crate) fn wire_close_to_tray(window: &MainWindow, lifecycle: Rc<RefCell<Lifecycle>>) {
    window.window().on_close_requested(move || {
        lifecycle.borrow_mut().apply(Action::CloseRequested);
        CloseRequestResponse::HideWindow
    });
}

pub(crate) fn wire_tray(
    tray: &TokenMasterTray,
    window: &MainWindow,
    lifecycle: Rc<RefCell<Lifecycle>>,
) {
    let weak = window.as_weak();
    let state = Rc::clone(&lifecycle);
    tray.on_show_requested(move || {
        state.borrow_mut().apply(Action::Show);
        if let Some(window) = weak.upgrade()
            && let Err(error) = window.show()
        {
            eprintln!("TokenMaster show failed: {error}");
        }
    });

    let weak = window.as_weak();
    let state = Rc::clone(&lifecycle);
    tray.on_hide_requested(move || {
        state.borrow_mut().apply(Action::Hide);
        if let Some(window) = weak.upgrade()
            && let Err(error) = window.hide()
        {
            eprintln!("TokenMaster hide failed: {error}");
        }
    });

    tray.on_quit_requested(move || {
        lifecycle.borrow_mut().apply(Action::Quit);
        if let Err(error) = slint::quit_event_loop() {
            eprintln!("TokenMaster quit failed: {error}");
        }
    });
}
