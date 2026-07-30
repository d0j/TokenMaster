use super::command::{
    ApplicationBackupSelection, ApplicationCommand, ApplicationCommandAdmission,
    ApplicationCommandCoordinator, ApplicationCommandExecution, ApplicationCommandFailure,
    ApplicationCommandOutcome, ApplicationCommandRejection, ApplicationOperationPayload,
    ApplicationOperationRequest, ApplicationReminderPolicyUpdate,
};
use tokenmaster_desktop::{
    DesktopBoardPreferences, DesktopBoardSectionKey, DesktopBoardSectionPreference,
    DesktopColorScheme, DesktopDensity, DesktopIntent, DesktopLayout, DesktopLocale,
    DesktopPresentationSelection, DesktopSkin,
};

#[test]
fn presentation_payload_is_complete_redacted_and_maps_all_two_hundred_forty_three_combinations() {
    let densities = [
        (
            DesktopDensity::Comfortable,
            tokenmaster_state::PresentationDensity::Comfortable,
        ),
        (
            DesktopDensity::Compact,
            tokenmaster_state::PresentationDensity::Compact,
        ),
        (
            DesktopDensity::UltraCompact,
            tokenmaster_state::PresentationDensity::UltraCompact,
        ),
    ];
    let skins = [
        (
            DesktopSkin::Refined,
            tokenmaster_state::PresentationSkin::Refined,
        ),
        (
            DesktopSkin::Graphite,
            tokenmaster_state::PresentationSkin::Graphite,
        ),
        (
            DesktopSkin::Ember,
            tokenmaster_state::PresentationSkin::Ember,
        ),
    ];
    let schemes = [
        (
            DesktopColorScheme::System,
            tokenmaster_state::PresentationColorScheme::System,
        ),
        (
            DesktopColorScheme::Light,
            tokenmaster_state::PresentationColorScheme::Light,
        ),
        (
            DesktopColorScheme::Dark,
            tokenmaster_state::PresentationColorScheme::Dark,
        ),
    ];
    let layouts = [
        (
            DesktopLayout::Refined,
            tokenmaster_state::PresentationLayout::Refined,
        ),
        (
            DesktopLayout::ControlCenter,
            tokenmaster_state::PresentationLayout::ControlCenter,
        ),
        (
            DesktopLayout::Workbench,
            tokenmaster_state::PresentationLayout::Workbench,
        ),
    ];
    let locales = [
        (
            DesktopLocale::English,
            tokenmaster_state::PresentationLocale::English,
        ),
        (
            DesktopLocale::Russian,
            tokenmaster_state::PresentationLocale::Russian,
        ),
        (
            DesktopLocale::Pseudo,
            tokenmaster_state::PresentationLocale::Pseudo,
        ),
    ];
    for (desktop_density, state_density) in densities {
        for (desktop_skin, state_skin) in skins {
            for (desktop_scheme, state_scheme) in schemes {
                for (desktop_layout, state_layout) in layouts {
                    for (desktop_locale, state_locale) in locales {
                        let selection = DesktopPresentationSelection::new(
                            desktop_density,
                            desktop_skin,
                            desktop_scheme,
                            desktop_layout,
                            desktop_locale,
                        );
                        let (_, payload) =
                            ApplicationOperationRequest::update_presentation(selection)
                                .into_parts();
                        let ApplicationOperationPayload::Presentation(update) = payload else {
                            panic!("complete presentation payload")
                        };
                        assert_eq!(update.selection(), selection);
                        let state = update.into_state_presentation();
                        assert_eq!(state.density(), state_density);
                        assert_eq!(state.skin(), state_skin);
                        assert_eq!(state.color_scheme(), state_scheme);
                        assert_eq!(state.layout(), state_layout);
                        assert_eq!(state.locale(), state_locale);
                        assert_eq!(
                            format!("{update:?}"),
                            "ApplicationPresentationUpdate([redacted])"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn presentation_payload_preserves_each_custom_board_row() {
    let board = DesktopBoardPreferences::new([
        DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Models, true, true),
        DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Activity, false, true),
        DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Sessions, true, false),
        DesktopBoardSectionPreference::new(DesktopBoardSectionKey::Trend, false, false),
        DesktopBoardSectionPreference::new(DesktopBoardSectionKey::CodeOutput, true, true),
        DesktopBoardSectionPreference::new(DesktopBoardSectionKey::PlanUsage, true, false),
    ])
    .expect("complete board preferences");
    let selection = DesktopPresentationSelection::new(
        DesktopDensity::Compact,
        DesktopSkin::Ember,
        DesktopColorScheme::Light,
        DesktopLayout::Workbench,
        DesktopLocale::English,
    )
    .with_board(board);
    let (_, payload) = ApplicationOperationRequest::update_presentation(selection).into_parts();
    let ApplicationOperationPayload::Presentation(update) = payload else {
        panic!("complete presentation payload")
    };

    let board = update.into_state_presentation().board();
    assert_eq!(
        board.rows()[0].key(),
        tokenmaster_state::BoardSectionKey::Models
    );
    assert!(board.rows()[0].visible());
    assert!(board.rows()[0].collapsed());
    assert_eq!(
        board.rows()[1].key(),
        tokenmaster_state::BoardSectionKey::Activity
    );
    assert!(!board.rows()[1].visible());
    assert!(board.rows()[1].collapsed());
    assert_eq!(
        board.rows()[4].key(),
        tokenmaster_state::BoardSectionKey::CodeOutput
    );
    assert!(board.rows()[4].visible());
    assert!(board.rows()[4].collapsed());
}

#[test]
fn plain_requests_reject_every_payload_required_command() {
    for command in [
        ApplicationCommand::ExportConfig,
        ApplicationCommand::ImportConfig,
        ApplicationCommand::BackupCompact,
        ApplicationCommand::BackupEncrypted,
        ApplicationCommand::UpdateBackupPolicy,
        ApplicationCommand::UpdateReminderPolicy,
        ApplicationCommand::UpdatePresentation,
    ] {
        assert!(
            ApplicationOperationRequest::plain(command).is_none(),
            "payload-required command must not have an empty request: {command:?}"
        );
    }
    assert!(ApplicationOperationRequest::plain(ApplicationCommand::Backup).is_some());
}

#[test]
fn reminder_policy_payload_is_bounded_validated_and_redacted() {
    assert!(ApplicationReminderPolicyUpdate::new(true, &[]).is_none());
    assert!(ApplicationReminderPolicyUpdate::new(false, &[60]).is_none());
    assert!(ApplicationReminderPolicyUpdate::new(true, &[60, 60]).is_none());
    assert!(ApplicationReminderPolicyUpdate::new(true, &[59]).is_none());
    assert!(ApplicationReminderPolicyUpdate::new(true, &[31_536_001]).is_none());
    assert!(
        ApplicationReminderPolicyUpdate::new(true, &[60, 61, 62, 63, 64, 65, 66, 67, 68]).is_none()
    );

    let DesktopIntent::UpdateReminderPolicy(desktop) =
        DesktopIntent::update_reminder_policy(true, &[21_600, 3_600]).expect("desktop intent")
    else {
        panic!("reminder intent");
    };
    let update =
        ApplicationReminderPolicyUpdate::from_desktop(desktop).expect("bounded app payload");
    assert!(update.enabled());
    assert_eq!(update.lead_seconds(), &[21_600, 3_600]);
    assert_eq!(
        format!("{update:?}"),
        "ApplicationReminderPolicyUpdate([redacted])"
    );
}

#[test]
fn ten_thousand_identical_hints_retain_one_active_command() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::Backup)
    else {
        panic!("first command must start");
    };

    for _ in 0..10_000 {
        assert_eq!(
            coordinator.submit(ApplicationCommand::Backup),
            ApplicationCommandAdmission::Coalesced {
                request_id: active.id(),
                active_request_id: active.id(),
            }
        );
    }

    let snapshot = coordinator.snapshot();
    assert_eq!(snapshot.active_count(), 1);
    assert_eq!(snapshot.pending_count(), 0);
    assert_eq!(snapshot.active_command(), Some(ApplicationCommand::Backup));
    assert_eq!(active.id().get(), 1);
    let completion = coordinator
        .finish(active.id(), ApplicationCommandExecution::Succeeded)
        .expect("successful completion")
        .completion();
    assert_eq!(completion.command(), ApplicationCommand::Backup);
    assert_eq!(completion.outcome(), ApplicationCommandOutcome::Succeeded);
}

#[test]
fn one_follow_up_is_bounded_and_a_third_distinct_command_is_busy() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::ExportConfig)
    else {
        panic!("first command must start");
    };
    let queued = coordinator.submit(ApplicationCommand::ImportConfig);
    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        active_request_id,
    } = queued
    else {
        panic!("one follow-up must queue");
    };
    assert_eq!(active_request_id, active.id());

    for _ in 0..10_000 {
        assert_eq!(
            coordinator.submit(ApplicationCommand::ImportConfig),
            ApplicationCommandAdmission::Coalesced {
                request_id: pending,
                active_request_id: active.id(),
            }
        );
    }
    assert_eq!(
        coordinator.submit(ApplicationCommand::Verify),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Busy)
    );
    assert_eq!(coordinator.snapshot().active_count(), 1);
    assert_eq!(coordinator.snapshot().pending_count(), 1);
}

#[test]
fn cancellation_is_exact_and_stops_at_the_irreversible_boundary() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(cancelled) =
        coordinator.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild must start");
    };
    let cancellation_flag = cancelled.cancellation_flag();
    assert!(!cancellation_flag.load(std::sync::atomic::Ordering::Acquire));
    assert!(coordinator.cancel(cancelled.id()));
    assert!(cancelled.is_cancelled());
    assert!(cancellation_flag.load(std::sync::atomic::Ordering::Acquire));
    assert!(cancelled.begin_irreversible().is_err());
    coordinator
        .finish(cancelled.id(), ApplicationCommandExecution::Cancelled)
        .expect("cancelled completion");

    let ApplicationCommandAdmission::Started(committing) =
        coordinator.submit(ApplicationCommand::ImportConfig)
    else {
        panic!("import must start");
    };
    committing
        .begin_irreversible()
        .expect("irreversible boundary");
    assert!(!coordinator.cancel(committing.id()));
    assert!(!committing.is_cancelled());
}

#[test]
fn pending_restore_is_typed_and_can_be_cancelled_before_start() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::Backup)
    else {
        panic!("backup must start");
    };
    let selection = ApplicationBackupSelection::new(7, 3).expect("bounded selection");
    assert_eq!(selection.catalog_generation(), 7);
    assert_eq!(selection.ordinal(), 3);
    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        ..
    } = coordinator.submit(ApplicationCommand::RestoreData(selection))
    else {
        panic!("restore must queue");
    };
    assert!(coordinator.cancel(pending));
    assert_eq!(coordinator.snapshot().pending_count(), 0);
    assert!(!coordinator.cancel(pending));
    assert!(!active.is_cancelled());
    let full_restore = ApplicationCommand::RestoreDataAndPortableSettings(selection);
    assert!(matches!(
        coordinator.submit(full_restore),
        ApplicationCommandAdmission::Queued { .. }
    ));
    assert_eq!(coordinator.snapshot().pending_command(), Some(full_restore));
}

#[test]
fn completion_promotes_one_follow_up_and_retry_is_explicit() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(first) =
        coordinator.submit(ApplicationCommand::Backup)
    else {
        panic!("backup must start");
    };
    let ApplicationCommandAdmission::Queued {
        request_id: pending,
        ..
    } = coordinator.submit(ApplicationCommand::Rebuild)
    else {
        panic!("rebuild must queue");
    };

    let transition = coordinator
        .finish(
            first.id(),
            ApplicationCommandExecution::Failed(ApplicationCommandFailure::Unavailable),
        )
        .expect("failed transition");
    assert_eq!(transition.completion().request_id(), first.id());
    assert_eq!(
        transition.completion().failure(),
        Some(ApplicationCommandFailure::Unavailable)
    );
    let all_failures = [
        ApplicationCommandFailure::Unavailable,
        ApplicationCommandFailure::InvalidSelection,
        ApplicationCommandFailure::Integrity,
        ApplicationCommandFailure::CapacityExceeded,
        ApplicationCommandFailure::Internal,
    ];
    assert_eq!(all_failures.len(), 5);
    assert_eq!(transition.follow_up().expect("follow-up").id(), pending);
    assert_eq!(
        coordinator.retry_last(),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Busy)
    );
    assert_eq!(coordinator.snapshot().active_request_id(), Some(pending));
    assert_eq!(coordinator.snapshot().pending_count(), 0);
}

#[test]
fn close_rejects_new_work_without_discarding_the_active_receipt() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::Verify)
    else {
        panic!("verify must start");
    };
    coordinator.close();
    assert!(coordinator.snapshot().is_closed());
    assert_eq!(
        coordinator.submit(ApplicationCommand::Backup),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed)
    );
    assert_eq!(
        coordinator.snapshot().active_request_id(),
        Some(active.id())
    );
}

#[test]
fn restart_pause_discards_only_the_follow_up_and_can_resume_admission() {
    let mut coordinator = ApplicationCommandCoordinator::new();
    let ApplicationCommandAdmission::Started(active) =
        coordinator.submit(ApplicationCommand::ExportConfig)
    else {
        panic!("config export must start");
    };
    assert!(matches!(
        coordinator.submit(ApplicationCommand::Backup),
        ApplicationCommandAdmission::Queued { .. }
    ));

    coordinator.pause_admission();
    assert!(coordinator.snapshot().admission_paused());
    assert_eq!(
        coordinator.snapshot().active_request_id(),
        Some(active.id())
    );
    assert_eq!(coordinator.snapshot().pending_count(), 0);
    assert_eq!(
        coordinator.submit(ApplicationCommand::Verify),
        ApplicationCommandAdmission::Rejected(ApplicationCommandRejection::Closed)
    );

    coordinator.resume_admission();
    assert!(!coordinator.snapshot().admission_paused());
    assert!(matches!(
        coordinator.submit(ApplicationCommand::Verify),
        ApplicationCommandAdmission::Queued { .. }
    ));
}
