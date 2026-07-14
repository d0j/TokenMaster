use std::path::Path;

use tokenmaster_codex::{
    CodexRootInput, ConfiguredCodexRoot, build_discovery_request, profile_id_for_root,
};
use tokenmaster_provider::{DiagnosticCode, ProviderErrorCode, RootOrigin};

#[test]
fn default_environment_and_configured_roots_merge_deterministically() {
    let configured = vec![ConfiguredCodexRoot::new(
        r"d:/CODEX-A/./",
        Some("Primary".to_owned()),
        true,
    )];
    let input = CodexRootInput {
        user_profile: Some(Path::new(r"C:\Users\Example")),
        codex_home: Some(r" D:\codex-a , E:\codex-b "),
        configured: &configured,
    };

    let first = build_discovery_request(input).expect("valid roots must build");
    let second = build_discovery_request(input).expect("repeat must build");

    assert_eq!(first, second);
    assert_eq!(first.roots().len(), 3);
    assert_eq!(
        first.roots()[0].path(),
        Path::new(r"C:\Users\Example\.codex")
    );
    assert_eq!(first.roots()[0].origin(), RootOrigin::Default);
    assert_eq!(first.roots()[1].path(), Path::new(r"D:\CODEX-A"));
    assert_eq!(first.roots()[1].origin(), RootOrigin::Configured);
    assert_eq!(first.roots()[1].label(), Some("Primary"));
    assert_eq!(first.roots()[2].path(), Path::new(r"E:\codex-b"));
    assert_eq!(first.roots()[2].origin(), RootOrigin::Environment);
}

#[test]
fn configured_disabled_duplicate_overrides_default_metadata() {
    let configured = vec![ConfiguredCodexRoot::new(
        r"c:/users/example/.CODEX",
        Some("Disabled default".to_owned()),
        false,
    )];
    let request = build_discovery_request(CodexRootInput {
        user_profile: Some(Path::new(r"C:\Users\Example")),
        codex_home: None,
        configured: &configured,
    })
    .expect("valid roots must build");

    assert_eq!(request.roots().len(), 1);
    assert_eq!(request.roots()[0].origin(), RootOrigin::Configured);
    assert_eq!(request.roots()[0].label(), Some("Disabled default"));
    assert!(!request.roots()[0].enabled());
}

#[test]
fn empty_and_invalid_entries_are_counted_before_traversal() {
    let configured = vec![ConfiguredCodexRoot::new("relative", None, true)];
    let request = build_discovery_request(CodexRootInput {
        user_profile: Some(Path::new(r"C:\Users\Example")),
        codex_home: Some(r" , relative , "),
        configured: &configured,
    })
    .expect("invalid entries must be isolated");

    assert_eq!(request.roots().len(), 1);
    assert_eq!(request.diagnostics().count(DiagnosticCode::EmptyRoot), 2);
    assert_eq!(request.diagnostics().count(DiagnosticCode::InvalidRoot), 2);
}

#[test]
fn more_than_sixty_four_unique_roots_fail_closed() {
    let configured = (0..64)
        .map(|index| ConfiguredCodexRoot::new(format!(r"D:\profiles\{index}"), None, true))
        .collect::<Vec<_>>();

    assert_eq!(
        build_discovery_request(CodexRootInput {
            user_profile: Some(Path::new(r"C:\Users\Example")),
            codex_home: None,
            configured: &configured,
        })
        .expect_err("65 unique roots must fail")
        .code(),
        ProviderErrorCode::TooManyRoots
    );
}

#[test]
fn more_than_sixty_four_configured_entries_fail_before_deduplication() {
    let configured = (0..65)
        .map(|_| ConfiguredCodexRoot::new(r"D:\profiles\duplicate", None, true))
        .collect::<Vec<_>>();

    assert_eq!(
        build_discovery_request(CodexRootInput {
            user_profile: None,
            codex_home: None,
            configured: &configured,
        })
        .expect_err("raw configured input must be bounded")
        .code(),
        ProviderErrorCode::TooManyRoots
    );
}

#[test]
fn profile_id_matches_pinned_sha256_fixture() {
    let profile_id = profile_id_for_root(Path::new(r"C:\Users\Example\.codex"))
        .expect("fixture path must be valid");

    assert_eq!(profile_id.as_str(), "profile_06055d531d2fa0d0");
    assert_eq!(
        profile_id,
        profile_id_for_root(Path::new(r"c:/users/example/.CODEX/./"))
            .expect("equivalent fixture must be valid")
    );
}

#[test]
fn profile_id_rejects_relative_paths_with_the_stable_path_code() {
    assert_eq!(
        profile_id_for_root(Path::new("relative"))
            .expect_err("relative path must fail")
            .code(),
        ProviderErrorCode::InvalidPath
    );
}
