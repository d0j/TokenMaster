#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateResult {
    pub name: String,
    pub status: GateStatus,
    pub actual: Option<f64>,
    pub target: f64,
    pub hard_limit: f64,
    pub unit: String,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryScenario {
    Empty,
    HundredThousand,
    MillionRows,
    TenThousandSwitches,
    TenThousandRoutes,
    SeventyTwoHourGrowth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatencyScenario {
    WarmVisible,
    WarmInteractive,
    ColdInteractive,
    InputToPaintP95,
    InputToPaintP99,
    ThemeSwitchP95,
    LayoutSwitchP95,
    IncrementalAppendP95,
    CachedMillionDashboardP95,
    ColdDashboardP95,
}

impl LatencyScenario {
    const fn limit(self) -> (&'static str, f64, bool) {
        match self {
            Self::WarmVisible => ("warm_visible", 300.0, true),
            Self::WarmInteractive => ("warm_interactive", 500.0, true),
            Self::ColdInteractive => ("cold_interactive", 900.0, true),
            Self::InputToPaintP95 => ("input_to_paint_p95", 50.0, false),
            Self::InputToPaintP99 => ("input_to_paint_p99", 100.0, false),
            Self::ThemeSwitchP95 => ("theme_switch_p95", 16.7, true),
            Self::LayoutSwitchP95 => ("layout_switch_p95", 50.0, true),
            Self::IncrementalAppendP95 => ("incremental_append_p95", 25.0, false),
            Self::CachedMillionDashboardP95 => ("cached_1m_dashboard_p95", 250.0, false),
            Self::ColdDashboardP95 => ("cold_dashboard_p95", 1_000.0, false),
        }
    }
}

impl MemoryScenario {
    const fn limits(self) -> (&'static str, f64, f64) {
        match self {
            Self::Empty => ("memory_empty", 40.0, 64.0),
            Self::HundredThousand => ("memory_100k", 64.0, 96.0),
            Self::MillionRows => ("memory_1m", 80.0, 112.0),
            Self::TenThousandSwitches => ("retained_10k_switches", 2.0, 4.0),
            Self::TenThousandRoutes => ("retained_10k_routes", 2.0, 4.0),
            Self::SeventyTwoHourGrowth => ("growth_72h", 8.0, 16.0),
        }
    }
}

pub fn evaluate_memory(scenario: MemoryScenario, actual_mib: f64) -> GateResult {
    let (name, target, hard_limit) = scenario.limits();
    evaluate(name, actual_mib, target, hard_limit, "MiB", true)
}

pub fn evaluate_cpu(actual_percent: f64) -> GateResult {
    evaluate("idle_cpu", actual_percent, 0.2, 0.5, "percent", false)
}

pub fn evaluate_latency(scenario: LatencyScenario, actual_ms: f64) -> GateResult {
    let (name, limit, inclusive) = scenario.limit();
    evaluate(name, actual_ms, limit, limit, "ms", inclusive)
}

fn evaluate(
    name: &str,
    actual: f64,
    target: f64,
    hard_limit: f64,
    unit: &str,
    target_is_inclusive: bool,
) -> GateResult {
    if !actual.is_finite() {
        return GateResult {
            name: name.to_owned(),
            status: GateStatus::Fail,
            actual: None,
            target,
            hard_limit,
            unit: unit.to_owned(),
            reason: "sample is missing or non-finite".to_owned(),
        };
    }

    let passes_target = actual < target || (target_is_inclusive && actual == target);
    let (status, reason) = if passes_target {
        (GateStatus::Pass, "within target")
    } else if actual < hard_limit {
        (GateStatus::Warn, "above target but below hard limit")
    } else {
        (GateStatus::Fail, "at or above hard limit")
    };
    GateResult {
        name: name.to_owned(),
        status,
        actual: Some(actual),
        target,
        hard_limit,
        unit: unit.to_owned(),
        reason: reason.to_owned(),
    }
}
