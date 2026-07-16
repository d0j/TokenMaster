use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_codex::{
    CodexAppServerCommand, CodexQuotaErrorCode, CodexQuotaTransport,
    MAX_CODEX_APP_SERVER_FRAME_BYTES,
};

const OBSERVED_AT_MS: i64 = 1_700_000_000_000;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codex_app_server_fixture"))
}

fn fixture_transport(mode: &str) -> (TempDir, CodexQuotaTransport) {
    let temp = TempDir::new().expect("fixture temp");
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let executable = temp
        .path()
        .join(format!("codex_app_server_fixture__{mode}{extension}"));
    fs::copy(fixture_path(), &executable).expect("copy fixture executable");
    let command = CodexAppServerCommand::new(executable).expect("fixture command");
    let transport =
        CodexQuotaTransport::new(command, Duration::from_secs(5)).expect("fixture transport");
    (temp, transport)
}

#[test]
fn hostile_envelope_and_notification_matrix_fails_closed() {
    for mode in [
        "empty_initialize",
        "unknown_initialize",
        "jsonrpc_initialize",
        "blank",
        "notification",
        "both_result_error",
        "missing_result",
        "negative_id",
        "rpc_error_unknown",
        "oversized_rpc_error",
    ] {
        let (_temp, transport) = fixture_transport(mode);
        let error = transport
            .poll(OBSERVED_AT_MS)
            .expect_err("hostile fixture must fail");
        assert_eq!(
            error.code(),
            CodexQuotaErrorCode::ProtocolError,
            "mode {mode}"
        );
        assert_eq!(error.limit(), None, "mode {mode}");
    }
}

#[test]
fn frame_bound_is_smaller_than_total_stdout_bound_and_reported_without_payload() {
    let (_temp, transport) = fixture_transport("oversized");
    let error = transport
        .poll(OBSERVED_AT_MS)
        .expect_err("oversized fixture must fail");

    assert_eq!(error.code(), CodexQuotaErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_CODEX_APP_SERVER_FRAME_BYTES));
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("xxxxxxxx"));
    assert!(!rendered.contains("private"));
}
