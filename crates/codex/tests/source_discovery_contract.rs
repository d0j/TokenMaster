use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;
use tokenmaster_codex::{
    CodexProvider, CodexRootInput, ConfiguredCodexRoot, build_discovery_request,
};
use tokenmaster_provider::{
    DiagnosticCode, DiscoveryProvider, ProfileAvailability, ProviderCapability, SourceKind,
};

fn request_for(path: &Path, enabled: bool) -> tokenmaster_provider::DiscoveryRequest {
    let configured = [ConfiguredCodexRoot::new(
        path,
        Some("Fixture".to_owned()),
        enabled,
    )];
    build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("fixture request must build")
}

fn create_junction(target: &Path, junction: &Path) {
    let status = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "New-Item -ItemType Junction -Path $env:TOKENMASTER_TEST_JUNCTION -Target $env:TOKENMASTER_TEST_TARGET | Out-Null",
        ])
        .env("TOKENMASTER_TEST_JUNCTION", junction)
        .env("TOKENMASTER_TEST_TARGET", target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Windows PowerShell must be available for the junction security gate");
    assert!(status.success(), "junction fixture must be created");
}

#[test]
fn provider_descriptor_declares_complete_codex_capabilities() {
    let provider = CodexProvider::new().expect("static descriptor must be valid");

    for capability in [
        ProviderCapability::History,
        ProviderCapability::Quota,
        ProviderCapability::Activity,
        ProviderCapability::Projects,
        ProviderCapability::Models,
        ProviderCapability::CodeOutput,
    ] {
        assert!(provider.descriptor().supports(capability));
    }
}

#[test]
fn missing_and_disabled_roots_remain_explicit() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let missing = temp.path().join("missing");
    let provider = CodexProvider::new().expect("static descriptor must be valid");

    let missing_snapshot = provider
        .discover(&request_for(&missing, true))
        .expect("missing root is profile state, not a global error");
    assert_eq!(missing_snapshot.profiles().len(), 1);
    assert_eq!(
        missing_snapshot.profiles()[0].availability(),
        ProfileAvailability::Missing
    );
    assert!(missing_snapshot.sources().is_empty());

    let disabled_snapshot = provider
        .discover(&request_for(&missing, false))
        .expect("disabled root must not be traversed");
    assert_eq!(
        disabled_snapshot.profiles()[0].availability(),
        ProfileAvailability::Rejected
    );
    assert_eq!(
        disabled_snapshot
            .diagnostics()
            .count(DiagnosticCode::DisabledRoot),
        1
    );
}

#[test]
fn active_archived_and_direct_sources_are_detected_in_stable_order() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let both = temp.path().join("both");
    let direct = temp.path().join("direct");
    fs::create_dir_all(both.join("sessions")).expect("active fixture must be created");
    fs::create_dir_all(both.join("archived_sessions")).expect("archive fixture must be created");
    fs::create_dir_all(&direct).expect("direct fixture must be created");
    let provider = CodexProvider::new().expect("static descriptor must be valid");

    let first = provider
        .discover(&request_for(&both, true))
        .expect("real sources must be discovered");
    let second = provider
        .discover(&request_for(&both, true))
        .expect("repeat discovery must pass");
    assert_eq!(first, second);
    assert_eq!(
        first
            .sources()
            .iter()
            .map(|source| source.kind())
            .collect::<Vec<_>>(),
        vec![SourceKind::Active, SourceKind::Archived]
    );
    assert_ne!(first.sources()[0].id(), first.sources()[1].id());

    let direct_snapshot = provider
        .discover(&request_for(&direct, true))
        .expect("direct root must be discovered");
    assert_eq!(direct_snapshot.sources().len(), 1);
    assert_eq!(direct_snapshot.sources()[0].kind(), SourceKind::Direct);
    assert_eq!(direct_snapshot.sources()[0].path(), direct);
}

#[test]
fn symlink_roots_and_sources_are_never_followed() {
    use std::os::windows::fs::symlink_dir;

    let temp = TempDir::new().expect("temporary directory must be created");
    let target = temp.path().join("target");
    let linked_root = temp.path().join("linked-root");
    let real_root = temp.path().join("real-root");
    fs::create_dir_all(target.join("sessions")).expect("target fixture must be created");
    fs::create_dir_all(&real_root).expect("real root fixture must be created");
    symlink_dir(&target, &linked_root).expect("directory symlink support is a security gate");
    symlink_dir(target.join("sessions"), real_root.join("sessions"))
        .expect("source symlink support is a security gate");
    let provider = CodexProvider::new().expect("static descriptor must be valid");

    let root_snapshot = provider
        .discover(&request_for(&linked_root, true))
        .expect("symlink root must be rejected as profile state");
    assert_eq!(
        root_snapshot.profiles()[0].availability(),
        ProfileAvailability::Rejected
    );
    assert!(root_snapshot.sources().is_empty());
    assert_eq!(
        root_snapshot
            .diagnostics()
            .count(DiagnosticCode::SymlinkRoot),
        1
    );

    let source_snapshot = provider
        .discover(&request_for(&real_root, true))
        .expect("symlink source must be isolated");
    assert_eq!(
        source_snapshot
            .diagnostics()
            .count(DiagnosticCode::InvalidSource),
        1
    );
    assert_eq!(source_snapshot.sources().len(), 1);
    assert_eq!(source_snapshot.sources()[0].kind(), SourceKind::Direct);
    assert_eq!(source_snapshot.sources()[0].path(), real_root);
}

#[test]
fn junction_roots_and_sources_are_never_followed() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let target_root = temp.path().join("target-root");
    let linked_root = temp.path().join("linked-root");
    let target_sessions = temp.path().join("target-sessions");
    let real_root = temp.path().join("real-root");
    fs::create_dir_all(target_root.join("sessions")).expect("target root must be created");
    fs::create_dir_all(&target_sessions).expect("target sessions must be created");
    fs::create_dir_all(&real_root).expect("real root must be created");
    create_junction(&target_root, &linked_root);
    create_junction(&target_sessions, &real_root.join("sessions"));
    let provider = CodexProvider::new().expect("static descriptor must be valid");

    let root_snapshot = provider
        .discover(&request_for(&linked_root, true))
        .expect("junction root must be rejected as profile state");
    assert_eq!(
        root_snapshot.profiles()[0].availability(),
        ProfileAvailability::Rejected
    );
    assert!(root_snapshot.sources().is_empty());
    assert_eq!(
        root_snapshot
            .diagnostics()
            .count(DiagnosticCode::SymlinkRoot),
        1
    );

    let source_snapshot = provider
        .discover(&request_for(&real_root, true))
        .expect("junction source must be isolated");
    assert_eq!(
        source_snapshot
            .diagnostics()
            .count(DiagnosticCode::InvalidSource),
        1
    );
    assert_eq!(source_snapshot.sources().len(), 1);
    assert_eq!(source_snapshot.sources()[0].kind(), SourceKind::Direct);
    assert_eq!(source_snapshot.sources()[0].path(), real_root);
}

#[test]
fn sixty_four_profiles_and_one_hundred_twenty_eight_sources_stay_bounded() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let mut configured = Vec::new();
    for index in 0..64 {
        let root = temp.path().join(format!("profile-{index}"));
        fs::create_dir_all(root.join("sessions")).expect("active fixture must be created");
        fs::create_dir_all(root.join("archived_sessions"))
            .expect("archive fixture must be created");
        configured.push(ConfiguredCodexRoot::new(root, None, true));
    }
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("bounded request must build");
    let provider = CodexProvider::new().expect("static descriptor must be valid");

    let snapshot = provider
        .discover(&request)
        .expect("bounded discovery must pass");

    assert_eq!(snapshot.profiles().len(), 64);
    assert_eq!(snapshot.sources().len(), 128);
}

#[test]
fn public_snapshot_debug_output_does_not_expose_absolute_paths() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let root = temp.path().join("private-profile");
    fs::create_dir_all(&root).expect("fixture root must be created");
    let provider = CodexProvider::new().expect("static descriptor must be valid");

    let snapshot = provider
        .discover(&request_for(&root, true))
        .expect("fixture discovery must pass");
    let debug = format!("{snapshot:?}");

    assert!(!debug.contains(temp.path().to_string_lossy().as_ref()));
    assert!(!debug.contains("private-profile"));
}
