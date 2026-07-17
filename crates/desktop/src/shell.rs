use anyhow::Context;
use slint::BackendSelector;

pub const PRODUCTION_RENDERER: &str = "winit-software";

pub fn select_production_renderer() -> anyhow::Result<()> {
    BackendSelector::new()
        .backend_name(PRODUCTION_RENDERER.to_owned())
        .select()
        .context("select production software renderer")?;
    Ok(())
}
