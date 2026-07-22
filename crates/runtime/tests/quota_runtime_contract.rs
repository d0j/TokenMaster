#[cfg(windows)]
mod windows {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use tempfile::TempDir;
    use tokenmaster_engine::RefreshOutcome;
    use tokenmaster_runtime::{
        CodexQuotaRuntimeConfig, ProviderPollErrorCode, ProviderQuotaRefreshFailure,
        ProviderQuotaRefreshStage, ProviderQuotaRuntime, ProviderQuotaRuntimePhase,
    };

    fn harmless_non_codex_executable() -> PathBuf {
        PathBuf::from(std::env::var_os("SystemRoot").expect("Windows root"))
            .join("System32")
            .join("where.exe")
    }

    #[test]
    fn public_runtime_fails_closed_without_opening_store_or_exposing_paths() {
        let root = TempDir::new().expect("temporary root");
        let archive = root.path().join("usage.sqlite3");
        let executable = harmless_non_codex_executable();
        let config = CodexQuotaRuntimeConfig::new(archive.clone())
            .expect("quota config")
            .with_executable(executable.clone())
            .expect("fixed executable")
            .with_transport_timeout(Duration::from_secs(1))
            .expect("transport timeout");
        let mut runtime = ProviderQuotaRuntime::start(config).expect("quota runtime");

        let deadline = Instant::now() + Duration::from_secs(5);
        let completion = loop {
            if let Some(completion) = runtime.try_completion().expect("completion") {
                break completion;
            }
            assert!(Instant::now() < deadline, "quota refresh timed out");
            std::thread::yield_now();
        };
        assert!(matches!(
            completion.outcome(),
            RefreshOutcome::Failed | RefreshOutcome::DeadlineExceeded
        ));

        let snapshot = runtime.snapshot().expect("runtime snapshot");
        assert_eq!(snapshot.phase(), ProviderQuotaRuntimePhase::Running);
        let failure = snapshot.refresh().failure().expect("transport failure");
        assert_eq!(failure.stage(), ProviderQuotaRefreshStage::Transport);
        assert!(matches!(
            failure,
            ProviderQuotaRefreshFailure::Transport(
                ProviderPollErrorCode::ProcessExited
                    | ProviderPollErrorCode::ProtocolError
                    | ProviderPollErrorCode::DeadlineExceeded
            )
        ));
        assert!(
            !archive.exists(),
            "source failure must precede SQLite publication"
        );
        assert_eq!(snapshot.refresh().quota_failure_count(), 0);
        assert_eq!(snapshot.refresh().benefit_failure_count(), 0);
        let debug = format!("{runtime:?}");
        assert!(!debug.contains(executable.to_string_lossy().as_ref()));
        assert!(!debug.contains(archive.to_string_lossy().as_ref()));

        assert_eq!(
            runtime.shutdown().expect("shutdown"),
            ProviderQuotaRuntimePhase::Stopped
        );
    }
}
