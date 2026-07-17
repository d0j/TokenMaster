//! TokenMaster production application composition.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod data_root;

pub use data_root::{ApplicationEnvironment, DataMode, DataRoot, DataRootError, DataRootErrorCode};
