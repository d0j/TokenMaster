use std::ffi::OsString;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokenmaster_codex::CodexAppServerCommand;
use tokenmaster_runtime::{
    CodexExecutableDiscoveryErrorCode, CodexExecutableSearchPath, CodexQuotaRuntimeConfig,
    MAX_CODEX_EXECUTABLE_SEARCH_DIRS, MAX_CODEX_EXECUTABLE_SEARCH_PATH_BYTES, RuntimeErrorCode,
};

fn native_name() -> &'static str {
    if cfg!(windows) { "codex.exe" } else { "codex" }
}

fn copy_native(directory: &Path) -> PathBuf {
    let candidate = directory.join(native_name());
    std::fs::copy(
        std::env::current_exe().expect("test executable"),
        &candidate,
    )
    .expect("copy native fixture");
    candidate
}

fn joined(paths: impl IntoIterator<Item = PathBuf>) -> OsString {
    std::env::join_paths(paths).expect("join search path")
}

#[test]
fn explicit_native_command_is_authoritative_and_path_private() {
    let root = TempDir::new().expect("temporary root");
    let archive = root.path().join("usage.sqlite3");
    let explicit = copy_native(root.path());
    let config = CodexQuotaRuntimeConfig::new(archive.clone())
        .expect("runtime config")
        .with_executable(explicit.clone())
        .expect("explicit executable");

    let debug = format!("{config:?}");
    assert!(!debug.contains(explicit.to_string_lossy().as_ref()));
    assert!(!debug.contains(archive.to_string_lossy().as_ref()));
    assert!(debug.contains("explicit"));

    let fallback_root = TempDir::new().expect("fallback root");
    let fallback = copy_native(fallback_root.path());
    let search = CodexExecutableSearchPath::new(joined([fallback.clone()])).expect("search path");
    assert_eq!(
        config.resolve_command(&search).expect("explicit command"),
        CodexAppServerCommand::new(explicit).expect("expected explicit command")
    );
    assert_ne!(
        config.resolve_command(&search).expect("repeat explicit"),
        CodexAppServerCommand::new(fallback).expect("fallback command")
    );
}

#[test]
fn invalid_explicit_configuration_never_falls_back() {
    let root = TempDir::new().expect("temporary root");
    let archive = root.path().join("usage.sqlite3");
    let fallback = copy_native(root.path());
    let invalid = root.path().join(if cfg!(windows) {
        "codex.cmd"
    } else {
        "missing-codex"
    });
    if cfg!(windows) {
        std::fs::write(&invalid, b"private shim").expect("write invalid shim");
    }

    let error = CodexQuotaRuntimeConfig::new(archive)
        .expect("runtime config")
        .with_executable(invalid.clone())
        .expect_err("invalid explicit executable");
    assert_eq!(error.code(), RuntimeErrorCode::InvalidConfiguration);
    assert!(!format!("{error:?}").contains(invalid.to_string_lossy().as_ref()));

    let search =
        CodexExecutableSearchPath::new(joined([root.path().to_path_buf()])).expect("search path");
    assert_eq!(
        search.resolve().expect("fallback fixture itself is valid"),
        CodexAppServerCommand::new(fallback).expect("fallback command")
    );
}

#[test]
fn automatic_search_uses_exact_native_name_and_directory_order() {
    let first_root = TempDir::new().expect("first root");
    let second_root = TempDir::new().expect("second root");
    let first = copy_native(first_root.path());
    let second = copy_native(second_root.path());
    if cfg!(windows) {
        std::fs::write(first_root.path().join("codex.cmd"), b"private shim")
            .expect("write cmd shim");
        std::fs::write(first_root.path().join("codex.ps1"), b"private shim")
            .expect("write PowerShell shim");
        std::fs::write(first_root.path().join("codex"), b"private shim")
            .expect("write extensionless shim");
    }
    let search = CodexExecutableSearchPath::new(joined([
        PathBuf::from("relative-entry"),
        first_root.path().to_path_buf(),
        second_root.path().to_path_buf(),
    ]))
    .expect("search path");

    let resolved = search.resolve().expect("resolved command");
    assert_eq!(
        resolved,
        CodexAppServerCommand::new(first.clone()).expect("first command")
    );
    assert_ne!(
        resolved,
        CodexAppServerCommand::new(second).expect("second command")
    );

    let debug = format!("{search:?}");
    assert!(!debug.contains(first.to_string_lossy().as_ref()));
    assert!(debug.contains("entry_count"));
}

#[test]
fn automatic_search_rejects_unbounded_or_missing_inputs() {
    let oversized =
        OsString::from("x".repeat(MAX_CODEX_EXECUTABLE_SEARCH_PATH_BYTES.saturating_add(1)));
    let error = CodexExecutableSearchPath::new(oversized).expect_err("oversized PATH");
    assert_eq!(
        error.code(),
        CodexExecutableDiscoveryErrorCode::CapacityExceeded
    );

    let roots = (0..=MAX_CODEX_EXECUTABLE_SEARCH_DIRS)
        .map(|index| PathBuf::from(format!("C:\\missing-{index}")))
        .collect::<Vec<_>>();
    let error = CodexExecutableSearchPath::new(joined(roots)).expect_err("excessive entries");
    assert_eq!(
        error.code(),
        CodexExecutableDiscoveryErrorCode::CapacityExceeded
    );

    let missing = CodexExecutableSearchPath::new(joined([PathBuf::from("relative-only")]))
        .expect("bounded missing search");
    let error = missing.resolve().expect_err("missing executable");
    assert_eq!(error.code(), CodexExecutableDiscoveryErrorCode::Unavailable);
    assert_eq!(
        format!("{error}"),
        "Codex executable discovery error: unavailable"
    );
}
