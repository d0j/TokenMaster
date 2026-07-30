use std::env;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokenmaster_codex::{CodexAppServerCommand, CodexQuotaTransport};

#[test]
#[ignore = "requires TOKENMASTER_CODEX_EXECUTABLE and an authenticated supported Codex app-server"]
fn authenticated_supported_codex_returns_bounded_live_quota() {
    let executable =
        env::var_os("TOKENMASTER_CODEX_EXECUTABLE").expect("configured Codex executable");
    let observed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_millis();
    let observed_at_ms = i64::try_from(observed_at_ms).expect("current time fits i64");
    let command =
        CodexAppServerCommand::new(PathBuf::from(executable)).expect("valid Codex executable");
    let transport =
        CodexQuotaTransport::new(command, Duration::from_secs(15)).expect("valid transport");

    let snapshot = transport
        .poll(observed_at_ms)
        .expect("supported authenticated Codex quota");

    assert!(!snapshot.observations().is_empty());
    assert!(snapshot.observations().len() <= 32);
    assert!(snapshot.observations().iter().all(|observation| {
        observation
            .definition()
            .nominal_duration_seconds()
            .is_none_or(|duration| duration > 0)
    }));
    println!(
        "Codex live quota observations={}",
        snapshot.observations().len()
    );
}
