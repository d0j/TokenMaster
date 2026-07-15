#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod coordinator;
mod error;
mod time;

pub use coordinator::{
    CancellationToken, FinishTransition, RefreshAdmission, RefreshCoordinator, RefreshOutcome,
    RefreshPermit, RefreshResult, RefreshUrgency,
};
pub use error::{EngineError, EngineErrorCode};
pub use time::{MonotonicTime, RefreshDeadline, RefreshRequestId};
