use anyhow::Context;
use slint::{BackendSelector, ComponentHandle};
use tokenmaster_product::ProductReducer;

use crate::DesktopShell;

pub const PRODUCTION_RENDERER: &str = "winit-software";

pub fn run() -> anyhow::Result<()> {
    BackendSelector::new()
        .backend_name(PRODUCTION_RENDERER.to_owned())
        .select()
        .context("select production software renderer")?;

    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let shell = DesktopShell::new(&snapshot).context("create TokenMaster desktop shell")?;
    shell.window().show().context("show TokenMaster window")?;
    slint::run_event_loop().context("run TokenMaster event loop")?;
    Ok(())
}
