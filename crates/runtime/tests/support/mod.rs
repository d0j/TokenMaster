use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

pub fn git_fixture_path() -> PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support/git_runtime_fixture.rs");
            let directory = std::env::current_exe()
                .expect("current test executable")
                .parent()
                .expect("test executable directory")
                .to_path_buf();
            let output = directory.join(format!(
                "tokenmaster-runtime-git-fixture-{}{}",
                std::process::id(),
                std::env::consts::EXE_SUFFIX
            ));
            let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
            let status = Command::new(rustc)
                .args(["--crate-name", "tokenmaster_runtime_git_fixture"])
                .arg("--edition=2024")
                .arg(&source)
                .arg("-o")
                .arg(&output)
                .status()
                .expect("compile Git runtime fixture");
            assert!(status.success(), "Git runtime fixture compilation failed");
            output
        })
        .clone()
}
