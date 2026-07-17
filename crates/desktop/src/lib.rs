//! Bounded production desktop adapter for TokenMaster.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod presentation;
pub mod shell;

mod bridge;
mod controller;

#[allow(clippy::unwrap_used, clippy::expect_used)]
mod generated_ui {
    slint::include_modules!();
}

pub use generated_ui::*;

mod ui;

pub use presentation::DesktopApplyOutcome;
pub use ui::{DesktopShell, DesktopUiError, DesktopUiErrorCode};

pub use bridge::{
    DesktopBridgeFailureCode, DesktopBridgeGeneration, DesktopBridgeObserver, DesktopBridgePhase,
    DesktopBridgeSnapshot, DesktopSnapshotBridge,
};
pub use controller::{
    DesktopAttempt, DesktopController, DesktopControllerError, DesktopControllerErrorCode,
    DesktopQueryPlan, DesktopQuerySource, DesktopRefreshAdmission, DesktopRefreshCompletion,
    DesktopRefreshOutcome, DesktopRefreshReceipt, DesktopRefreshUrgency, DesktopRuntimeObservation,
    DesktopRuntimeObservationOutcome, DesktopSnapshotNotifier, DesktopSnapshotReceiver,
};
