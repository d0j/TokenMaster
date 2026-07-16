use tokenmaster_domain::{
    GitActivityAssociationId, GitLineMetrics, GitOutputCategory, GitOutputCategoryMetrics,
    GitOutputDay, GitOutputError, GitOutputPortfolio, GitOutputProjection,
    GitOutputProjectionParts, GitOutputQuality, GitOutputTotals, GitOutputUnavailableReason,
    GitOutputWarning, GitRepositoryId, MAX_GIT_OUTPUT_CATEGORIES, MAX_GIT_OUTPUT_DAYS,
    MAX_GIT_OUTPUT_REPOSITORIES, MAX_GIT_OUTPUT_WARNINGS,
};

fn repository(seed: u8) -> GitRepositoryId {
    GitRepositoryId::from_bytes([seed; 32])
}

fn association(seed: u8) -> GitActivityAssociationId {
    GitActivityAssociationId::from_bytes([seed; 32])
}

fn lines(added: u64, removed: u64) -> GitLineMetrics {
    GitLineMetrics::new(added, removed)
}

fn day(index: i32, commits: u64, added: u64, removed: u64) -> GitOutputDay {
    GitOutputDay::new(index, commits, 0, lines(added, removed)).expect("valid day")
}

fn category(category: GitOutputCategory, added: u64, removed: u64) -> GitOutputCategoryMetrics {
    GitOutputCategoryMetrics::new(category, lines(added, removed))
}

fn complete_projection(seed: u8) -> GitOutputProjection {
    GitOutputProjection::new(GitOutputProjectionParts {
        repository_id: repository(seed),
        association_id: association(seed),
        scan_revision: 7,
        observed_at_ms: 1_800_000_000_000,
        data_through_ms: Some(1_799_999_999_000),
        quality: GitOutputQuality::Complete,
        unavailable_reason: None,
        warnings: Vec::new(),
        totals: GitOutputTotals::new(2, 0, lines(30, 10), 0, 0, 0, 0).expect("valid totals"),
        days: vec![day(20_000, 1, 10, 4), day(20_001, 1, 20, 6)],
        categories: vec![
            category(GitOutputCategory::ProductCode, 20, 5),
            category(GitOutputCategory::Test, 10, 5),
        ],
    })
    .expect("valid projection")
}

#[test]
fn opaque_identities_and_stable_codes_are_path_private() {
    let repository_id = repository(3);
    let association_id = association(4);
    assert_eq!(repository_id.as_bytes(), &[3; 32]);
    assert_eq!(association_id.as_bytes(), &[4; 32]);
    assert_eq!(format!("{repository_id:?}"), "GitRepositoryId([redacted])");
    assert_eq!(
        format!("{association_id:?}"),
        "GitActivityAssociationId([redacted])"
    );

    assert_eq!(GitOutputCategory::ProductCode.stable_code(), "product_code");
    assert_eq!(
        GitOutputCategory::VendorGenerated.stable_code(),
        "vendor_generated"
    );
    assert_eq!(GitOutputQuality::Unavailable.stable_code(), "unavailable");
    assert_eq!(
        GitOutputUnavailableReason::AuthorIdentityMissing.stable_code(),
        "author_identity_missing"
    );
    assert_eq!(
        GitOutputWarning::SubmoduleLinesOmitted.stable_code(),
        "submodule_lines_omitted"
    );

    let debug = format!("{:?}", complete_projection(3));
    for forbidden in [
        r"C:\private\repo",
        "/home/private/repo",
        "author@example.com",
        "refs/heads/main",
        "deadbeef",
        "src/main.rs",
        "git log",
    ] {
        assert!(!debug.contains(forbidden), "leaked {forbidden}: {debug}");
    }
}

#[test]
fn line_totals_days_categories_and_net_are_exact() {
    let projection = complete_projection(1);
    assert_eq!(projection.scan_revision(), 7);
    assert_eq!(projection.totals().commits(), 2);
    assert_eq!(projection.totals().lines().added(), 30);
    assert_eq!(projection.totals().lines().removed(), 10);
    assert_eq!(projection.totals().lines().net_lines(), 20);
    assert_eq!(lines(4, 9).net_lines(), -5);
    assert_eq!(projection.days().len(), 2);
    assert_eq!(projection.categories().len(), 2);
    assert_eq!(
        projection.categories()[0].category(),
        GitOutputCategory::ProductCode
    );

    assert_eq!(
        GitLineMetrics::new(u64::MAX, 0).checked_add(lines(1, 0)),
        Err(GitOutputError::Overflow)
    );
    assert_eq!(
        GitOutputTotals::new(1, 2, lines(0, 0), 0, 0, 0, 0),
        Err(GitOutputError::IncoherentState)
    );
}

#[test]
fn quality_reason_warning_and_aggregate_coherence_fail_closed() {
    let mut parts = complete_projection(2).into_parts();
    parts.quality = GitOutputQuality::Partial;
    assert_eq!(
        GitOutputProjection::new(parts.clone()),
        Err(GitOutputError::IncoherentState)
    );

    parts.warnings = vec![GitOutputWarning::OversizedFieldsOmitted];
    parts.totals = GitOutputTotals::new(2, 0, lines(30, 10), 0, 0, 0, 1).expect("partial totals");
    assert!(GitOutputProjection::new(parts.clone()).is_ok());

    parts.totals = GitOutputTotals::new(2, 0, lines(30, 10), 1, 0, 0, 1).expect("binary totals");
    assert_eq!(
        GitOutputProjection::new(parts.clone()),
        Err(GitOutputError::IncoherentState)
    );
    parts
        .warnings
        .insert(0, GitOutputWarning::BinaryFilesOmitted);
    assert!(GitOutputProjection::new(parts.clone()).is_ok());

    parts.quality = GitOutputQuality::Complete;
    assert_eq!(
        GitOutputProjection::new(parts.clone()),
        Err(GitOutputError::IncoherentState)
    );

    parts.quality = GitOutputQuality::Unavailable;
    parts.unavailable_reason = Some(GitOutputUnavailableReason::GitNotFound);
    parts.data_through_ms = None;
    parts.warnings.clear();
    parts.totals = GitOutputTotals::new(0, 0, lines(0, 0), 0, 0, 0, 0).expect("zero totals");
    parts.days.clear();
    parts.categories.clear();
    assert!(GitOutputProjection::new(parts.clone()).is_ok());

    parts.unavailable_reason = None;
    assert_eq!(
        GitOutputProjection::new(parts),
        Err(GitOutputError::IncoherentState)
    );

    let mut mismatched = complete_projection(3).into_parts();
    mismatched.categories[0] = category(GitOutputCategory::ProductCode, 21, 5);
    assert_eq!(
        GitOutputProjection::new(mismatched),
        Err(GitOutputError::IncoherentState)
    );
}

#[test]
fn collections_are_bounded_ordered_and_duplicate_free() {
    let mut days = (0..=MAX_GIT_OUTPUT_DAYS)
        .map(|index| day(i32::try_from(index).expect("bounded index"), 0, 0, 0))
        .collect::<Vec<_>>();
    let mut parts = complete_projection(5).into_parts();
    parts.totals = GitOutputTotals::new(0, 0, lines(0, 0), 0, 0, 0, 0).expect("zero totals");
    parts.days = days.clone();
    parts.categories.clear();
    assert_eq!(
        GitOutputProjection::new(parts.clone()),
        Err(GitOutputError::CapacityExceeded {
            limit: MAX_GIT_OUTPUT_DAYS
        })
    );

    days.truncate(2);
    days.swap(0, 1);
    parts.days = days;
    assert_eq!(
        GitOutputProjection::new(parts),
        Err(GitOutputError::InvalidOrdering)
    );

    let categories = [
        GitOutputCategory::ProductCode,
        GitOutputCategory::Test,
        GitOutputCategory::DocsSpec,
        GitOutputCategory::ConfigBuild,
        GitOutputCategory::SchemaMigration,
        GitOutputCategory::VendorGenerated,
        GitOutputCategory::Asset,
        GitOutputCategory::Other,
    ];
    assert_eq!(categories.len(), MAX_GIT_OUTPUT_CATEGORIES);

    let mut projections = (0..=MAX_GIT_OUTPUT_REPOSITORIES)
        .map(|seed| complete_projection(u8::try_from(seed).expect("bounded seed")))
        .collect::<Vec<_>>();
    assert_eq!(
        GitOutputPortfolio::new(projections.clone()),
        Err(GitOutputError::CapacityExceeded {
            limit: MAX_GIT_OUTPUT_REPOSITORIES
        })
    );

    projections.truncate(2);
    projections.swap(0, 1);
    assert_eq!(
        GitOutputPortfolio::new(projections),
        Err(GitOutputError::InvalidOrdering)
    );

    let mut warning_parts = complete_projection(7).into_parts();
    warning_parts.quality = GitOutputQuality::Partial;
    warning_parts.warnings =
        vec![GitOutputWarning::AssociationIncomplete; MAX_GIT_OUTPUT_WARNINGS + 1];
    assert_eq!(
        GitOutputProjection::new(warning_parts),
        Err(GitOutputError::CapacityExceeded {
            limit: MAX_GIT_OUTPUT_WARNINGS
        })
    );
}

#[test]
fn serde_revalidates_nested_values_and_rejects_unknown_fields() {
    let projection = complete_projection(8);
    let encoded = serde_json::to_string(&projection).expect("serialize projection");
    let decoded: GitOutputProjection =
        serde_json::from_str(&encoded).expect("deserialize projection");
    assert_eq!(decoded, projection);

    let mut root_unknown = serde_json::to_value(&projection).expect("projection value");
    root_unknown
        .as_object_mut()
        .expect("projection object")
        .insert(
            "repository_path".into(),
            serde_json::json!(r"C:\private\repo"),
        );
    assert!(serde_json::from_value::<GitOutputProjection>(root_unknown).is_err());

    let mut nested_unknown = serde_json::to_value(&projection).expect("projection value");
    nested_unknown["totals"]["author_email"] = serde_json::json!("author@example.com");
    assert!(serde_json::from_value::<GitOutputProjection>(nested_unknown).is_err());

    let mut invalid_quality = serde_json::to_value(&projection).expect("projection value");
    invalid_quality["quality"] = serde_json::json!("partial");
    assert!(serde_json::from_value::<GitOutputProjection>(invalid_quality).is_err());

    let portfolio = GitOutputPortfolio::new(vec![complete_projection(1), complete_projection(2)])
        .expect("valid portfolio");
    let round_trip: GitOutputPortfolio =
        serde_json::from_str(&serde_json::to_string(&portfolio).expect("serialize portfolio"))
            .expect("deserialize portfolio");
    assert_eq!(round_trip, portfolio);
}
