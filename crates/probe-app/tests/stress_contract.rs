use std::path::Path;

use clap::Parser;
use tokenmaster_gates::GateStatus;
use tokenmaster_m0::{
    args::{Args, StressKind},
    stress::{require_pass, retained_mib, validated_report_path},
};

#[test]
fn stress_arguments_are_bounded() {
    let args = Args::try_parse_from([
        "tokenmaster-m0",
        "--stress",
        "switches",
        "--iterations",
        "10000",
        "--rows",
        "1000000",
        "--duration-seconds",
        "2",
        "--report",
        "reports/switches.json",
    ])
    .expect("valid stress arguments");
    assert_eq!(args.stress, Some(StressKind::Switches));
    assert_eq!(args.iterations, 10_000);
    assert_eq!(args.rows, 1_000_000);
    assert_eq!(args.duration_seconds, 2);

    assert!(Args::try_parse_from(["tokenmaster-m0", "--iterations", "0"]).is_err());
    assert!(Args::try_parse_from(["tokenmaster-m0", "--rows", "1000001"]).is_err());
}

#[test]
fn retained_memory_saturates_when_allocator_returns_pages() {
    assert_eq!(retained_mib(10 * 1024 * 1024, 12 * 1024 * 1024), 2.0);
    assert_eq!(retained_mib(12 * 1024 * 1024, 10 * 1024 * 1024), 0.0);
}

#[test]
fn stress_status_is_fail_closed() {
    assert!(require_pass(GateStatus::Pass).is_ok());
    assert!(require_pass(GateStatus::Warn).is_err());
    assert!(require_pass(GateStatus::Fail).is_err());
}

#[test]
fn report_path_cannot_escape_report_directory() {
    let directory = tempfile::tempdir().expect("temp directory");
    let root = directory.path();
    std::fs::create_dir(root.join("reports")).expect("reports directory");

    let accepted = validated_report_path(root, Path::new("reports/result.json"))
        .expect("accepted report path");
    assert!(accepted.starts_with(root.join("reports")));
    assert!(validated_report_path(root, Path::new("../escape.json")).is_err());
    assert!(validated_report_path(root, Path::new("other/result.json")).is_err());
}
