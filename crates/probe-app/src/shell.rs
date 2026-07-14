use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, anyhow};
use slint::{BackendSelector, ComponentHandle};

use crate::{
    MainWindow, TokenMasterTray,
    lifecycle::Lifecycle,
    seed_probe_models,
    tray::{wire_close_to_tray, wire_tray},
    wire_skin_callbacks,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererChoice {
    FemtoVg,
    Software,
}

impl RendererChoice {
    pub const fn backend_name(self) -> &'static str {
        match self {
            Self::FemtoVg => "winit-femtovg",
            Self::Software => "winit-software",
        }
    }

    pub fn from_override(value: Option<&str>) -> Result<Self, RendererOverrideError> {
        match value {
            None | Some("") | Some("software") => Ok(Self::Software),
            Some("femtovg") => Ok(Self::FemtoVg),
            Some(_) => Err(RendererOverrideError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendererOverrideError;

impl std::fmt::Display for RendererOverrideError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("renderer override must be 'femtovg' or 'software'")
    }
}

impl std::error::Error for RendererOverrideError {}

pub fn run_desktop(preferred: RendererChoice) -> anyhow::Result<RendererChoice> {
    let renderer = select_renderer_with_fallback(preferred)?;
    let window = MainWindow::new().context("create main window")?;
    wire_skin_callbacks(&window);
    seed_probe_models(&window);

    let lifecycle = Rc::new(RefCell::new(Lifecycle::default()));
    wire_close_to_tray(&window, Rc::clone(&lifecycle));

    let tray = TokenMasterTray::new().context("create tray icon")?;
    wire_tray(&tray, &window, lifecycle);

    window.show().context("show main window")?;
    tray.show().context("show tray icon")?;
    slint::run_event_loop().context("run Slint event loop")?;
    Ok(renderer)
}

fn select_renderer_with_fallback(preferred: RendererChoice) -> anyhow::Result<RendererChoice> {
    if preferred == RendererChoice::Software {
        select_renderer(RendererChoice::Software).context("select software renderer")?;
        return Ok(RendererChoice::Software);
    }

    match select_renderer(RendererChoice::FemtoVg) {
        Ok(()) => Ok(RendererChoice::FemtoVg),
        Err(femtovg_error) => select_renderer(RendererChoice::Software)
            .map(|()| RendererChoice::Software)
            .map_err(|software_error| {
                anyhow!(
                    "renderer selection failed: femtovg={femtovg_error}; software={software_error}"
                )
            }),
    }
}

pub(crate) fn select_renderer(renderer: RendererChoice) -> Result<(), slint::PlatformError> {
    BackendSelector::new()
        .backend_name(renderer.backend_name().to_owned())
        .select()
}
