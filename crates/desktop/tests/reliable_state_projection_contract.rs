use std::cell::Cell;
use std::rc::Rc;

use tokenmaster_desktop::{
    DesktopBackupHealth, DesktopBackupPolicy, DesktopDensity, DesktopIntent,
    DesktopIntentAdmission, DesktopIntentSink, DesktopOperationKind, DesktopOperationPhase,
    DesktopOperationSnapshot, DesktopPresentationSelection, DesktopPresentationSettings,
    DesktopRecoveryReceipt, DesktopReliableStateHealth, DesktopReliableStateInput,
    DesktopReliableStateProjection, DesktopReliableStateSummary, DesktopReminderPolicy,
    DesktopReminderSyncState, DesktopRestorePointInput, DesktopRestoreSelection, DesktopShell,
    DesktopSkin, MAX_DESKTOP_RESTORE_POINTS,
};
use tokenmaster_product::ProductReducer;

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

#[test]
fn explicit_reminder_policy_round_trips_through_reliable_state_projection() {
    let reminder_policy = DesktopReminderPolicy::new(
        true,
        &[10_800, 604_800],
        DesktopReminderSyncState::Synchronized,
    )
    .expect("policy");
    let summary = DesktopReliableStateSummary::new_with_reminder_policy(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::disabled(),
        reminder_policy,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let projection = DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        8,
        summary,
        Vec::new(),
    ));

    assert_eq!(projection.reminder_policy(), reminder_policy);
}

#[test]
fn legacy_reliable_state_summary_uses_unavailable_reminder_fallback() {
    let summary = DesktopReliableStateSummary::new(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::disabled(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let projection = DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        8,
        summary,
        Vec::new(),
    ));

    assert_eq!(
        projection.reminder_policy(),
        DesktopReminderPolicy::unavailable()
    );
}

#[test]
fn presentation_settings_project_complete_quadruple_and_legacy_defaults_are_safe() {
    let summary = DesktopReliableStateSummary::new_with_settings(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::disabled(),
        DesktopReminderPolicy::unavailable(),
        DesktopPresentationSettings::new(
            DesktopDensity::UltraCompact,
            DesktopSkin::Graphite,
            tokenmaster_desktop::DesktopColorScheme::Light,
            tokenmaster_desktop::DesktopLayout::Workbench,
            tokenmaster_desktop::DesktopLocale::English,
        ),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let projection = DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        9,
        summary,
        Vec::new(),
    ));
    assert_eq!(
        projection.presentation().density(),
        DesktopDensity::UltraCompact
    );
    assert_eq!(projection.presentation().skin(), DesktopSkin::Graphite);
    assert_eq!(
        projection.presentation().color_scheme(),
        tokenmaster_desktop::DesktopColorScheme::Light
    );
    assert_eq!(
        projection.presentation().layout(),
        tokenmaster_desktop::DesktopLayout::Workbench
    );

    let legacy = DesktopReliableStateSummary::new_with_reminder_policy(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::disabled(),
        DesktopReminderPolicy::unavailable(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let legacy_projection = DesktopReliableStateProjection::from_input(
        DesktopReliableStateInput::new(10, legacy, Vec::new()),
    );
    assert_eq!(
        legacy_projection.presentation(),
        DesktopPresentationSettings::comfortable()
    );
}

struct RecordingPresentationIntentSink {
    submissions: Cell<u8>,
    selection: Cell<Option<DesktopPresentationSelection>>,
}

impl DesktopIntentSink for RecordingPresentationIntentSink {
    fn submit(&self, intent: DesktopIntent) -> DesktopIntentAdmission {
        let DesktopIntent::UpdatePresentation(selection) = intent else {
            panic!("presentation selection");
        };
        self.submissions.set(self.submissions.get() + 1);
        self.selection.set(Some(selection));
        DesktopIntentAdmission::Started
    }
}

#[test]
fn shell_hydrates_workbench_and_retains_it_when_density_submits_complete_selection() {
    i_slint_backend_testing::init_no_event_loop();
    let summary = DesktopReliableStateSummary::new_with_settings(
        DesktopReliableStateHealth::Healthy,
        false,
        "healthy",
        DesktopBackupPolicy::disabled(),
        DesktopReminderPolicy::unavailable(),
        DesktopPresentationSettings::new(
            DesktopDensity::UltraCompact,
            DesktopSkin::Graphite,
            tokenmaster_desktop::DesktopColorScheme::Dark,
            tokenmaster_desktop::DesktopLayout::Workbench,
            tokenmaster_desktop::DesktopLocale::English,
        ),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let projection = DesktopReliableStateProjection::from_input(DesktopReliableStateInput::new(
        11,
        summary,
        Vec::new(),
    ));
    let sink = Rc::new(RecordingPresentationIntentSink {
        submissions: Cell::new(0),
        selection: Cell::new(None),
    });
    let shell = DesktopShell::new_with_reliable_state(
        &ProductReducer::new().snapshot(),
        projection,
        sink.clone(),
    )
    .expect("shell");
    let window = shell.window();
    let address = window as *const _;
    assert_eq!(sink.submissions.get(), 0);
    assert_eq!(window.get_presentation_density_key(), "ultra_compact");
    assert_eq!(window.get_presentation_layout_key(), "workbench");

    window.invoke_select_presentation_density(1);

    assert_eq!(sink.submissions.get(), 1);
    assert_eq!(
        sink.selection.get(),
        Some(DesktopPresentationSelection::new(
            DesktopDensity::Compact,
            DesktopSkin::Graphite,
            tokenmaster_desktop::DesktopColorScheme::Dark,
            tokenmaster_desktop::DesktopLayout::Workbench,
            tokenmaster_desktop::DesktopLocale::English,
        ))
    );
    assert_eq!(address, shell.window() as *const _);
}

#[test]
fn reminder_policy_normalizes_descending_and_remains_copyable() {
    let policy = DesktopReminderPolicy::new(
        true,
        &[10_800, 60, 604_800, 31_536_000],
        DesktopReminderSyncState::Synchronized,
    )
    .expect("policy");
    let copied = policy;

    assert_eq!(copied.lead_seconds(), &[31_536_000, 604_800, 10_800, 60]);
    assert!(copied.enabled());
    assert_eq!(copied.sync_state(), DesktopReminderSyncState::Synchronized);
}

#[test]
fn reminder_policy_rejects_invalid_enabled_and_disabled_leads() {
    assert!(DesktopReminderPolicy::new(true, &[], DesktopReminderSyncState::Pending).is_none());
    assert!(DesktopReminderPolicy::new(false, &[60], DesktopReminderSyncState::Pending).is_none());
    assert!(
        DesktopReminderPolicy::new(true, &[60, 60], DesktopReminderSyncState::Pending).is_none()
    );
    assert!(DesktopReminderPolicy::new(true, &[59], DesktopReminderSyncState::Pending).is_none());
    assert!(
        DesktopReminderPolicy::new(true, &[31_536_001], DesktopReminderSyncState::Pending,)
            .is_none()
    );
    assert!(
        DesktopReminderPolicy::new(
            true,
            &[60, 61, 62, 63, 64, 65, 66, 67, 68],
            DesktopReminderSyncState::Pending,
        )
        .is_none()
    );
}

#[test]
fn unavailable_reminder_policy_is_disabled_and_bounded() {
    let policy = DesktopReminderPolicy::unavailable();

    assert!(!policy.enabled());
    assert!(policy.lead_seconds().is_empty());
    assert_eq!(policy.sync_state(), DesktopReminderSyncState::Unavailable);
}

#[test]
fn reminder_policy_intent_validates_before_retaining_and_redacts_debug() {
    assert!(DesktopIntent::update_reminder_policy(true, &[]).is_err());
    assert!(DesktopIntent::update_reminder_policy(false, &[60]).is_err());
    assert!(DesktopIntent::update_reminder_policy(true, &[60, 60]).is_err());
    assert!(DesktopIntent::update_reminder_policy(true, &[59]).is_err());
    assert!(DesktopIntent::update_reminder_policy(true, &[31_536_001]).is_err());
    assert!(
        DesktopIntent::update_reminder_policy(true, &[60, 61, 62, 63, 64, 65, 66, 67, 68]).is_err()
    );

    let intent = DesktopIntent::update_reminder_policy(true, &[10_800]).expect("intent");
    let DesktopIntent::UpdateReminderPolicy(update) = &intent else {
        panic!("reminder policy intent");
    };
    assert!(update.enabled());
    assert_eq!(update.lead_seconds(), &[10_800]);
    assert!(!format!("{intent:?}").contains("10800"));
}
