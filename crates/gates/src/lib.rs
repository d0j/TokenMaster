#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod budget;
mod report;

pub use budget::{
    GateResult, GateStatus, LatencyScenario, MemoryScenario, evaluate_cpu, evaluate_latency,
    evaluate_memory,
};
pub use report::{M0Report, ReportKind};
