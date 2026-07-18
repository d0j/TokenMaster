use tokenmaster_desktop::{
    DesktopBackupHealth, DesktopBackupPolicy, DesktopOperationKind, DesktopOperationPhase,
    DesktopOperationSnapshot, DesktopRecoveryReceipt, DesktopReliableStateHealth,
    DesktopReliableStateInput, DesktopReliableStateProjection, DesktopReliableStateSummary,
    DesktopRestorePointInput, DesktopRestoreSelection, MAX_DESKTOP_RESTORE_POINTS,
};

fn restore_point(ordinal: u8) -> DesktopRestorePointInput {
    DesktopRestorePointInput::new(
        DesktopRestoreSelection::new(42, ordinal).expect("selection"),
        Some(1_721_234_567_890 + i64::from(ordinal)),
        1_048_576 + u64::from(ordinal),
        DesktopBackupHealth::Verified,
        "manual",
        Some(12),
        "normal",
    )
}

#[test]
fn reliable_state_projection_is_exact_bounded_and_path_free() {
    let policy = DesktopBackupPolicy::new(true, 300, 21_600, 512 * 1_048_576);
    let operation = DesktopOperationSnapshot::new(
        DesktopOperationKind::Verify,
        DesktopOperationPhase::Running,
        true,
        None,
    );
    let recovery = DesktopRecoveryReceipt::reconstructed_from_authoritative_source();
    let summary = DesktopReliableStateSummary::new(
        DesktopReliableStateHealth::Degraded,
        true,
        "fallback_corrupt_slot",
        policy,
        Some(1_721_234_567_890),
        Some(1_721_234_560_000),
        Some(21),
        Some(3),
        Some(42_000_000),
        Some("integrity"),
        Some(recovery),
        Some(operation),
        None,
    );
    let input = DesktopReliableStateInput::new(7, summary, (0_u8..20).map(restore_point).collect());
    let projection = DesktopReliableStateProjection::from_input(input);

    assert_eq!(projection.generation(), 7);
    assert_eq!(projection.health(), DesktopReliableStateHealth::Degraded);
    assert!(projection.safe_mode());
    assert_eq!(projection.settings_health_code(), "fallback_corrupt_slot");
    assert_eq!(projection.policy(), policy);
    assert_eq!(
        projection.latest_success_at_utc_ms(),
        Some(1_721_234_567_890)
    );
    assert_eq!(
        projection.latest_attempt_at_utc_ms(),
        Some(1_721_234_560_000)
    );
    assert_eq!(projection.successful_count(), Some(21));
    assert_eq!(projection.failure_count(), Some(3));
    assert_eq!(projection.published_bytes(), Some(42_000_000));
    assert_eq!(projection.latest_failure_code(), Some("integrity"));
    assert_eq!(projection.recovery_receipt(), Some(recovery));
    assert!(
        projection
            .recovery_receipt()
            .expect("reconstruction receipt")
            .non_reconstructible_domains_lost()
    );
    assert_eq!(projection.operation(), Some(operation));
    assert_eq!(
        projection.restore_points().len(),
        MAX_DESKTOP_RESTORE_POINTS
    );
    assert_eq!(MAX_DESKTOP_RESTORE_POINTS, 15);
    assert_eq!(
        projection.restore_selection(0),
        DesktopRestoreSelection::new(42, 0)
    );
    assert_eq!(
        projection.restore_selection(MAX_DESKTOP_RESTORE_POINTS - 1),
        DesktopRestoreSelection::new(42, 14)
    );
    assert_eq!(
        projection.restore_selection(MAX_DESKTOP_RESTORE_POINTS),
        None
    );
    assert!(!format!("{projection:?}").contains("\\"));
    assert!(!format!("{projection:?}").contains(":"));

    let completed = DesktopOperationSnapshot::new(
        DesktopOperationKind::Verify,
        DesktopOperationPhase::Succeeded,
        false,
        None,
    );
    let completed_projection = projection.clone().with_operation(Some(completed));
    assert_eq!(completed_projection.operation(), Some(completed));
    assert_eq!(
        completed_projection.restore_points(),
        projection.restore_points()
    );
}

#[test]
fn unavailable_projection_has_no_fabricated_times_counts_or_restore_points() {
    let projection = DesktopReliableStateProjection::unavailable();

    assert_eq!(projection.generation(), 0);
    assert_eq!(projection.health(), DesktopReliableStateHealth::Unavailable);
    assert!(!projection.safe_mode());
    assert_eq!(projection.latest_success_at_utc_ms(), None);
    assert_eq!(projection.latest_attempt_at_utc_ms(), None);
    assert_eq!(projection.successful_count(), None);
    assert_eq!(projection.failure_count(), None);
    assert_eq!(projection.published_bytes(), None);
    assert_eq!(projection.latest_failure_code(), None);
    assert_eq!(projection.recovery_receipt(), None);
    assert_eq!(projection.operation(), None);
    assert!(projection.restore_points().is_empty());
}
