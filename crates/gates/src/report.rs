use crate::{GateResult, GateStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportKind {
    DeveloperSmoke,
    M0Candidate,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct M0Report {
    pub schema_version: u32,
    pub kind: ReportKind,
    pub renderer: String,
    pub row_count: u64,
    pub samples: usize,
    pub gates: Vec<GateResult>,
    pub overall: GateStatus,
}

impl M0Report {
    pub fn new(
        kind: ReportKind,
        renderer: impl Into<String>,
        row_count: u64,
        samples: usize,
        gates: Vec<GateResult>,
    ) -> Self {
        let overall = if gates.iter().any(|gate| gate.status == GateStatus::Fail) {
            GateStatus::Fail
        } else if gates.iter().any(|gate| gate.status == GateStatus::Warn) {
            GateStatus::Warn
        } else {
            GateStatus::Pass
        };
        Self {
            schema_version: 1,
            kind,
            renderer: renderer.into(),
            row_count,
            samples,
            gates,
            overall,
        }
    }
}
