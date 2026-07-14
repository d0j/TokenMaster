use tokenmaster_gates::{
    GateStatus, LatencyScenario, MemoryScenario, evaluate_cpu, evaluate_latency, evaluate_memory,
};

#[test]
fn one_million_memory_threshold_is_fail_closed() {
    assert_eq!(
        evaluate_memory(MemoryScenario::MillionRows, 80.0).status,
        GateStatus::Pass
    );
    assert_eq!(
        evaluate_memory(MemoryScenario::MillionRows, 111.99).status,
        GateStatus::Warn
    );
    assert_eq!(
        evaluate_memory(MemoryScenario::MillionRows, 112.0).status,
        GateStatus::Fail
    );
}

#[test]
fn retained_switch_threshold_is_fail_closed() {
    assert_eq!(
        evaluate_memory(MemoryScenario::TenThousandSwitches, 2.0).status,
        GateStatus::Pass
    );
    assert_eq!(
        evaluate_memory(MemoryScenario::TenThousandSwitches, 3.99).status,
        GateStatus::Warn
    );
    assert_eq!(
        evaluate_memory(MemoryScenario::TenThousandSwitches, 4.0).status,
        GateStatus::Fail
    );
}

#[test]
fn non_finite_samples_fail_instead_of_comparing() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            evaluate_memory(MemoryScenario::Empty, value).status,
            GateStatus::Fail
        );
    }
}

#[test]
fn idle_cpu_threshold_is_strict() {
    assert_eq!(evaluate_cpu(0.19).status, GateStatus::Pass);
    assert_eq!(evaluate_cpu(0.2).status, GateStatus::Warn);
    assert_eq!(evaluate_cpu(0.5).status, GateStatus::Fail);
}

#[test]
fn latency_limits_preserve_strict_and_inclusive_requirements() {
    assert_eq!(
        evaluate_latency(LatencyScenario::ThemeSwitchP95, 16.7).status,
        GateStatus::Pass
    );
    assert_eq!(
        evaluate_latency(LatencyScenario::ThemeSwitchP95, 16.71).status,
        GateStatus::Fail
    );
    assert_eq!(
        evaluate_latency(LatencyScenario::InputToPaintP95, 49.99).status,
        GateStatus::Pass
    );
    assert_eq!(
        evaluate_latency(LatencyScenario::InputToPaintP95, 50.0).status,
        GateStatus::Fail
    );
}
