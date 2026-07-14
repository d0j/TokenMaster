use tokenmaster_provider::{
    DiagnosticCode, DiscoveryDiagnostics, DiscoveryRequest, DiscoveryRoot, DiscoverySnapshot,
    ProfileAvailability, ProfileDescriptor, ProfileId, ProviderCapability, ProviderDescriptor,
    ProviderErrorCode, ProviderId, RootOrigin, SourceDescriptor, SourceId, SourceKind,
};

#[test]
fn provider_descriptor_composes_sorted_capabilities() {
    let descriptor = ProviderDescriptor::new(
        ProviderId::new("codex").expect("fixture provider id must be valid"),
        "Codex",
        [ProviderCapability::Quota, ProviderCapability::History],
    )
    .expect("fixture descriptor must be valid");

    assert_eq!(descriptor.id().as_str(), "codex");
    assert_eq!(descriptor.display_name(), "Codex");
    assert_eq!(
        descriptor
            .capabilities()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![ProviderCapability::History, ProviderCapability::Quota]
    );
    assert!(descriptor.supports(ProviderCapability::History));
    assert!(!descriptor.supports(ProviderCapability::Activity));
}

#[test]
fn public_identifiers_reject_empty_oversized_and_unsafe_text() {
    assert_eq!(
        ProviderId::new("")
            .expect_err("empty provider id must fail")
            .code(),
        ProviderErrorCode::InvalidId
    );
    assert_eq!(
        ProviderId::new("codex/path")
            .expect_err("path separator must fail")
            .code(),
        ProviderErrorCode::InvalidId
    );
    assert_eq!(
        ProfileId::new("x".repeat(129))
            .expect_err("oversized profile id must fail")
            .code(),
        ProviderErrorCode::InvalidId
    );
    assert_eq!(
        SourceId::new("source id")
            .expect_err("spaces must fail")
            .code(),
        ProviderErrorCode::InvalidId
    );
}

#[test]
fn public_identifiers_preserve_valid_bounded_values() {
    let provider = ProviderId::new("codex.local-1").expect("fixture must be valid");
    let profile = ProfileId::new(format!("profile_{}", "a".repeat(120)))
        .expect("128-byte profile id must be valid");
    let source = SourceId::new("source_deadbeef").expect("fixture must be valid");

    assert_eq!(provider.as_str(), "codex.local-1");
    assert_eq!(profile.as_str().len(), 128);
    assert_eq!(source.as_str(), "source_deadbeef");
}

#[test]
fn descriptor_rejects_empty_and_oversized_display_names() {
    let provider = ProviderId::new("codex").expect("fixture must be valid");

    assert_eq!(
        ProviderDescriptor::new(provider.clone(), "", [])
            .expect_err("empty name must fail")
            .code(),
        ProviderErrorCode::InvalidDisplayName
    );
    assert_eq!(
        ProviderDescriptor::new(provider, "x".repeat(129), [])
            .expect_err("oversized name must fail")
            .code(),
        ProviderErrorCode::InvalidDisplayName
    );
}

#[test]
fn discovery_request_enforces_root_and_path_bounds() {
    let roots = (0..65)
        .map(|index| {
            DiscoveryRoot::new(
                format!(r"C:\profiles\{index}"),
                RootOrigin::Configured,
                None,
                true,
            )
            .expect("fixture root must be valid")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        DiscoveryRequest::new(roots)
            .expect_err("65 roots must fail")
            .code(),
        ProviderErrorCode::TooManyRoots
    );
    assert_eq!(
        DiscoveryRoot::new("relative", RootOrigin::Configured, None, true)
            .expect_err("relative path must fail")
            .code(),
        ProviderErrorCode::InvalidPath
    );
    assert_eq!(
        DiscoveryRoot::new("C:\0invalid", RootOrigin::Configured, None, true)
            .expect_err("NUL path must fail")
            .code(),
        ProviderErrorCode::InvalidPath
    );
    assert_eq!(
        DiscoveryRoot::new(
            format!(r"C:\{}", "x".repeat(4096)),
            RootOrigin::Configured,
            None,
            true,
        )
        .expect_err("oversized path must fail")
        .code(),
        ProviderErrorCode::InvalidPath
    );
}

#[test]
fn configured_label_is_truncated_on_a_utf8_boundary() {
    let root = DiscoveryRoot::new(
        r"C:\profiles\primary",
        RootOrigin::Configured,
        Some("é".repeat(65)),
        false,
    )
    .expect("fixture root must be valid");

    let label = root.label().expect("configured label must remain present");
    assert_eq!(label.len(), 128);
    assert_eq!(label.chars().count(), 64);
    assert!(!root.enabled());
    assert_eq!(root.origin(), RootOrigin::Configured);
}

#[test]
fn diagnostics_serialize_only_stable_codes_and_counts() {
    let mut diagnostics = DiscoveryDiagnostics::default();
    diagnostics.record(DiagnosticCode::EmptyRoot);
    diagnostics.record(DiagnosticCode::InvalidRoot);
    diagnostics.record(DiagnosticCode::InvalidRoot);
    diagnostics.record(DiagnosticCode::UnsupportedRootNamespace);

    let json = serde_json::to_string(&diagnostics).expect("diagnostics must serialize");

    assert_eq!(
        json,
        r#"{"empty_root":1,"invalid_root":2,"disabled_root":0,"symlink_root":0,"unsupported_root_namespace":1,"invalid_source":0}"#
    );
    assert_eq!(
        diagnostics.count(DiagnosticCode::UnsupportedRootNamespace),
        1
    );
    assert!(!json.contains("C:\\"));
}

#[test]
fn path_bearing_descriptors_redact_debug_output() {
    let profile = ProfileDescriptor::new(
        ProfileId::new("profile_deadbeef").expect("fixture id must be valid"),
        RootOrigin::Configured,
        ProfileAvailability::Available,
        Some("Primary".to_owned()),
        r"C:\private\alice\.codex",
    )
    .expect("fixture profile must be valid");
    let source = SourceDescriptor::new(
        SourceId::new("source_deadbeef").expect("fixture id must be valid"),
        profile.id().clone(),
        SourceKind::Active,
        r"C:\private\alice\.codex\sessions",
    )
    .expect("fixture source must be valid");

    assert_eq!(
        profile.path(),
        std::path::Path::new(r"C:\private\alice\.codex")
    );
    assert_eq!(source.profile_id(), profile.id());
    assert!(!format!("{profile:?}").contains("alice"));
    assert!(!format!("{source:?}").contains("alice"));
}

#[test]
fn discovery_snapshot_rejects_capacity_overflow() {
    let profiles = (0..65)
        .map(|index| {
            ProfileDescriptor::new(
                ProfileId::new(format!("profile_{index:02}")).expect("fixture id must be valid"),
                RootOrigin::Configured,
                ProfileAvailability::Missing,
                None,
                format!(r"C:\profiles\{index}"),
            )
            .expect("fixture profile must be valid")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        DiscoverySnapshot::new(profiles, Vec::new(), DiscoveryDiagnostics::default())
            .expect_err("65 profiles must fail")
            .code(),
        ProviderErrorCode::CapacityExceeded
    );
}
