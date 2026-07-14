#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod args;
pub mod lifecycle;
pub mod metrics;
pub mod presentation;
pub mod shell;
pub mod stress;

#[allow(clippy::unwrap_used, clippy::expect_used)]
mod generated_ui {
    slint::include_modules!();
}

pub use generated_ui::*;

mod tray;
mod ui_runtime;

pub use ui_runtime::{seed_probe_models, wire_skin_callbacks};
