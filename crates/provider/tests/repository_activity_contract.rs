use std::path::PathBuf;

use tempfile::TempDir;
use tokenmaster_domain::{
    ProjectAlias, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId, UtcTimestamp,
};
use tokenmaster_provider::{
    RepositoryActivityHint, RepositoryActivityHintParts, RepositoryCandidatePath,
};

fn hint(
    candidate: RepositoryCandidatePath,
    project: Option<ProjectAlias>,
) -> RepositoryActivityHint {
    RepositoryActivityHint::new(RepositoryActivityHintParts {
        provider_id: UsageProviderId::new("codex").expect("provider"),
        profile_id: UsageProfileId::new("profile-a").expect("profile"),
        source_id: UsageSourceId::new("source-a").expect("source"),
        session_id: UsageSessionId::new("session-a").expect("session"),
        observed_at: UtcTimestamp::new(1_752_139_200, 123).expect("timestamp"),
        project,
        candidate,
    })
}

#[test]
fn repository_activity_is_exact_bounded_and_debug_private() {
    let directory = TempDir::new().expect("temporary directory");
    let private = directory.path().join("PRIVATE_REPOSITORY_MARKER");
    std::fs::create_dir(&private).expect("repository candidate");
    let candidate =
        RepositoryCandidatePath::new(private.clone()).expect("valid local directory candidate");
    let activity = hint(
        candidate.clone(),
        Some(ProjectAlias::new("project-a").expect("project")),
    );

    assert_eq!(activity.provider_id().as_str(), "codex");
    assert_eq!(activity.profile_id().as_str(), "profile-a");
    assert_eq!(activity.source_id().as_str(), "source-a");
    assert_eq!(activity.session_id().as_str(), "session-a");
    assert_eq!(activity.observed_at().unix_seconds(), 1_752_139_200);
    assert_eq!(activity.observed_at().subsec_nanos(), 123);
    assert_eq!(
        activity.project().map(ProjectAlias::as_str),
        Some("project-a")
    );
    assert_eq!(activity.candidate().as_path(), candidate.as_path());
    assert!(candidate.byte_len() <= tokenmaster_provider::MAX_PATH_BYTES);

    for debug in [format!("{candidate:?}"), format!("{activity:?}")] {
        assert!(!debug.contains("PRIVATE_REPOSITORY_MARKER"));
        assert!(!debug.contains(private.to_string_lossy().as_ref()));
        assert!(debug.contains("[redacted]"));
    }
}

#[test]
fn repository_candidate_rejects_untrusted_path_shapes() {
    for invalid in [
        PathBuf::from("relative"),
        PathBuf::from("."),
        PathBuf::from("parent").join("..").join("escape"),
    ] {
        assert!(
            RepositoryCandidatePath::new(invalid).is_err(),
            "untrusted path shape must fail"
        );
    }

    #[cfg(windows)]
    for invalid in [
        PathBuf::from(r"\\server\share\repo"),
        PathBuf::from(r"\\?\UNC\server\share\repo"),
        PathBuf::from(r"\\.\PhysicalDrive0"),
        PathBuf::from(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1"),
    ] {
        assert!(
            RepositoryCandidatePath::new(invalid).is_err(),
            "network and device namespaces must fail"
        );
    }
}

#[test]
fn repository_candidate_rejects_linked_ancestors() {
    let directory = TempDir::new().expect("temporary directory");
    let target = directory.path().join("target");
    let linked = directory.path().join("linked");
    std::fs::create_dir(&target).expect("target");

    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_dir(&target, &linked).is_err() {
            return;
        }
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &linked).expect("directory symlink");

    assert!(RepositoryCandidatePath::new(linked).is_err());
}
