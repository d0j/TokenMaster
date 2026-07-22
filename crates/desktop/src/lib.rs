//! Bounded production desktop adapter for TokenMaster.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod activity;
pub mod dashboard;
pub mod history;
pub mod in_app_notification;
pub mod models;
mod native_tray;
pub mod notifications;
pub mod presentation;
pub mod presentation_style;
pub mod projects;
pub mod reliable_state;
pub mod sessions;
pub mod shell;
pub mod skin;

mod bridge;
mod controller;

#[allow(clippy::unwrap_used, clippy::expect_used)]
mod generated_ui {
    slint::include_modules!();
}

pub use generated_ui::*;

pub use native_tray::{DesktopWindowActivationError, activate_window};

mod ui;

pub use activity::*;
pub use dashboard::*;
pub use history::*;
pub use in_app_notification::*;
pub use models::*;
pub use notifications::*;
pub use presentation::{
    DesktopApplyOutcome, DesktopHistoryRangeSelectionError, DesktopRouteKey,
    DesktopSessionSelectionError, DesktopSnapshotEpoch, DesktopState,
};
pub use presentation_style::{
    DesktopColorScheme, DesktopDensity, DesktopEffectiveColorScheme,
    DesktopPresentationApplyOutcome, DesktopPresentationPersistence, DesktopPresentationRevision,
    DesktopPresentationSelection, DesktopPresentationStyle, DesktopSystemColorScheme,
};
pub use projects::*;
pub use reliable_state::*;
pub use sessions::*;
pub use shell::{
    DesktopCloseEffect, DesktopLifecycleIntent, DesktopLifecycleIntentAdmission,
    DesktopLifecycleIntentRouter, DesktopLifecycleIntentRouterError, DesktopLifecycleIntentSink,
    DesktopTrayAvailability, select_production_renderer,
};
pub use skin::{DesktopColorTokens, DesktopRgb, DesktopSkin};
pub use ui::{
    DesktopBridgeFactory, DesktopCurrentUserStartupPresenter, DesktopReliableStateNotifier,
    DesktopShell, DesktopUiError, DesktopUiErrorCode,
};

pub use bridge::{
    DesktopBridgeFailureCode, DesktopBridgeGeneration, DesktopBridgeObserver, DesktopBridgePhase,
    DesktopBridgeSnapshot, DesktopSnapshotBridge,
};
pub use controller::{
    DesktopAttempt, DesktopController, DesktopControllerError, DesktopControllerErrorCode,
    DesktopHistoryRangeGeneration, DesktopHistoryRangeIntent, DesktopHistoryRangePreset,
    DesktopQueryPlan, DesktopQuerySource, DesktopRefreshAdmission, DesktopRefreshCompletion,
    DesktopRefreshIngress, DesktopRefreshOutcome, DesktopRefreshReceipt, DesktopRefreshUrgency,
    DesktopRuntimeObservation, DesktopRuntimeObservationOutcome, DesktopSessionDetailIntent,
    DesktopSessionNavigationGeneration, DesktopSessionPageDirection, DesktopSessionPageIntent,
    DesktopSnapshotNotifier, DesktopSnapshotReceiver, DesktopTerminalHistoryRangeNotifier,
    DesktopTerminalNavigationNotifier,
};
