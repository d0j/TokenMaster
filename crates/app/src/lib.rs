//! TokenMaster production application composition.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod application;
mod command;
mod data_root;
mod operation;
mod state;

pub use application::{ApplicationError, ApplicationErrorCode, run};

pub use data_root::{ApplicationEnvironment, DataMode, DataRoot, DataRootError, DataRootErrorCode};

#[cfg(test)]
#[path = "state_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod state_tests;

#[cfg(test)]
#[path = "command_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod command_tests;

#[cfg(test)]
#[path = "operation_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod operation_tests;
