//! Constant-state immutable product projections for TokenMaster frontends.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod reducer;
mod section;
mod snapshot;

pub use reducer::{ProductPublishOutcome, ProductReducer, ProductReducerError};
pub use section::{ProductSection, ProductSectionFailure, ProductSectionKind};
pub use snapshot::{ProductGeneration, ProductSnapshot};
