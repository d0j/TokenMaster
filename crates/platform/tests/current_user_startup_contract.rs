#![allow(clippy::expect_used)]

use tokenmaster_platform::{
    CurrentUserStartup, CurrentUserStartupAction, CurrentUserStartupError, CurrentUserStartupStatus,
};

#[test]
fn status_and_error_codes_are_exact_path_free_and_bounded() {
    assert_eq!(
        [
            CurrentUserStartupStatus::Disabled,
            CurrentUserStartupStatus::EnabledVerified,
            CurrentUserStartupStatus::StaleRelocation,
            CurrentUserStartupStatus::Conflict,
            CurrentUserStartupStatus::AccessDenied,
            CurrentUserStartupStatus::Unavailable,
        ]
        .map(CurrentUserStartupStatus::stable_code),
        [
            "disabled",
            "enabled_verified",
            "stale_relocation",
            "conflict",
            "access_denied",
            "unavailable",
        ]
    );
    assert_eq!(
        [
            CurrentUserStartupError::AccessDenied,
            CurrentUserStartupError::Unavailable,
            CurrentUserStartupError::StaleRequiresRepair,
            CurrentUserStartupError::Conflict,
            CurrentUserStartupError::InvalidState,
            CurrentUserStartupError::ReadbackFailed,
        ]
        .map(CurrentUserStartupError::stable_code),
        [
            "startup_access_denied",
            "startup_unavailable",
            "startup_stale_requires_repair",
            "startup_conflict",
            "startup_invalid_state",
            "startup_readback_failed",
        ]
    );
    for error in [
        CurrentUserStartupError::AccessDenied,
        CurrentUserStartupError::Unavailable,
        CurrentUserStartupError::StaleRequiresRepair,
        CurrentUserStartupError::Conflict,
        CurrentUserStartupError::InvalidState,
        CurrentUserStartupError::ReadbackFailed,
    ] {
        let text = format!("{error:?} {error}");
        assert!(!text.contains('\\'));
        assert!(!text.contains(':'));
        assert!(!text.to_ascii_lowercase().contains("users"));
    }
}

#[test]
fn inspection_is_path_free_and_actions_are_fixed() {
    assert_eq!(
        [
            CurrentUserStartupAction::Enable,
            CurrentUserStartupAction::RepairStale,
            CurrentUserStartupAction::Disable,
        ]
        .len(),
        3
    );
    let snapshot = CurrentUserStartup::inspect();
    assert!(matches!(
        snapshot.status(),
        CurrentUserStartupStatus::Disabled
            | CurrentUserStartupStatus::EnabledVerified
            | CurrentUserStartupStatus::StaleRelocation
            | CurrentUserStartupStatus::Conflict
            | CurrentUserStartupStatus::AccessDenied
            | CurrentUserStartupStatus::Unavailable
    ));
    let text = format!("{snapshot:?}");
    assert_eq!(
        text,
        format!(
            "CurrentUserStartupSnapshot({})",
            snapshot.status().stable_code()
        )
    );
    assert!(!text.contains('\\'));
    assert!(!text.contains(':'));
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_is_visible_and_rejects_mutation() {
    assert_eq!(
        CurrentUserStartup::inspect().status(),
        CurrentUserStartupStatus::Unavailable
    );
    assert_eq!(
        CurrentUserStartup::apply(CurrentUserStartupAction::Enable)
            .expect_err("unsupported mutation"),
        CurrentUserStartupError::Unavailable
    );
}
