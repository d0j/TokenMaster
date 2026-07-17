//! Constant-state immutable product projections for TokenMaster frontends.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod reducer;
mod route;
mod section;
mod snapshot;

pub use reducer::{ProductPublishOutcome, ProductReducer, ProductReducerError};
pub use route::{
    ProductRoute, ProductRouteReason, ProductRouteReasonIter, ProductRouteReasons,
    ProductRouteState, ProductRouteStatus,
};
pub use section::{
    ProductAttemptGeneration, ProductSection, ProductSectionFailure, ProductSectionKind,
};
pub use snapshot::{ProductGeneration, ProductSnapshot};
