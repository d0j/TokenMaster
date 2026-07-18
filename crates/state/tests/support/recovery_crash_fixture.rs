use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const ROOT_ENV: &str = "TOKENMASTER_TEST_RECOVERY_ROOT";
pub const PHASE_ENV: &str = "TOKENMASTER_TEST_RECOVERY_PHASE";
const READY_FILE: &str = "recovery-phase-ready";

pub fn child_root_and_phase() -> Option<(std::path::PathBuf, String)> {
    let root = std::env::var_os(ROOT_ENV).map(std::path::PathBuf::from)?;
    let phase = std::env::var(PHASE_ENV).ok()?;
    Some((root, phase))
}

pub fn signal_and_wait(root: &Path) -> ! {
    std::fs::write(root.join(READY_FILE), b"durable").expect("phase ready marker");
    loop {
        thread::park_timeout(Duration::from_secs(30));
    }
}

pub fn kill_after_durable_phase(root: &Path, phase: &str) {
    let marker = root.join(READY_FILE);
    let _ = std::fs::remove_file(&marker);
    let mut child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "recovery_crash_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROOT_ENV, root)
        .env(PHASE_ENV, phase)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn exact recovery integration test executable");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() {
        assert!(
            Instant::now() < deadline,
            "child did not reach durable recovery phase {phase}"
        );
        if let Some(status) = child.try_wait().expect("poll recovery child") {
            panic!("recovery child exited before phase {phase}: {status}");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.kill().expect("force terminate recovery child");
    child.wait().expect("reap recovery child");
    std::fs::remove_file(marker).expect("remove ready marker");
}
