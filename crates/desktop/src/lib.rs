//! Bounded production desktop adapter for TokenMaster.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod activity;
pub mod dashboard;
pub mod history;
pub mod in_app_notification;
pub mod models;
pub mod notifications;
pub mod presentation;
pub mod projects;
pub mod reliable_state;
pub mod sessions;
pub mod shell;

mod bridge;
mod controller;

#[allow(clippy::unwrap_used, clippy::expect_used)]
mod generated_ui {
    slint::include_modules!();
}

pub use generated_ui::*;

mod ui;

pub use activity::*;
pub use dashboard::*;
pub use history::*;
pub use in_app_notification::*;
pub use models::*;
pub use notifications::*;
pub use presentation::{
    DesktopApplyOutcome, DesktopRouteKey, DesktopSessionSelectionError, DesktopSnapshotEpoch,
    DesktopState,
};
pub use projects::*;
pub use reliable_state::*;
pub use sessions::*;
pub use shell::{
    DesktopLifecycleIntent, DesktopLifecycleIntentAdmission, DesktopLifecycleIntentRouter,
    DesktopLifecycleIntentRouterError, DesktopLifecycleIntentSink, select_production_renderer,
};
pub use ui::{
    DesktopBridgeFactory, DesktopReliableStateNotifier, DesktopShell, DesktopUiError,
    DesktopUiErrorCode,
};

pub use bridge::{
    DesktopBridgeFailureCode, DesktopBridgeGeneration, DesktopBridgeObserver, DesktopBridgePhase,
    DesktopBridgeSnapshot, DesktopSnapshotBridge,
};
pub use controller::{
    DesktopAttempt, DesktopController, DesktopControllerError, DesktopControllerErrorCode,
    DesktopQueryPlan, DesktopQuerySource, DesktopRefreshAdmission, DesktopRefreshCompletion,
    DesktopRefreshIngress, DesktopRefreshOutcome, DesktopRefreshReceipt, DesktopRefreshUrgency,
    DesktopRuntimeObservation, DesktopRuntimeObservationOutcome, DesktopSessionDetailIntent,
    DesktopSnapshotNotifier, DesktopSnapshotReceiver,
};
