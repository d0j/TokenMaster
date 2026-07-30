#[path = "../../state/tests/automatic_recovery_contract.rs"]
mod automatic_recovery_contract;
#[path = "../../state/tests/maintenance_contract.rs"]
mod maintenance_contract;
#[path = "../../state/tests/recovery_journal_contract.rs"]
mod recovery_journal_contract;
#[path = "../../state/tests/restore_contract.rs"]
mod restore_contract;

const APPLICATION_SOURCE: &str = include_str!("../src/application.rs");
const APPLICATION_TESTS: &str = include_str!("../src/application_tests.rs");
const MAINTENANCE_TESTS: &str = include_str!("../../state/tests/maintenance_contract.rs");
const JOURNAL_TESTS: &str = include_str!("../../state/tests/recovery_journal_contract.rs");
const RESTORE_TESTS: &str = include_str!("../../state/tests/restore_contract.rs");
const AUTOMATIC_TESTS: &str = include_str!("../../state/tests/automatic_recovery_contract.rs");

fn assert_exact_anchor(source: &str, anchor: &str) {
    assert_eq!(
        source.match_indices(anchor).count(),
        1,
        "expected one executable recovery anchor: {anchor}"
    );
}

#[test]
fn application_recovery_and_migration_matrix_remains_executable() {
    assert_exact_anchor(APPLICATION_SOURCE, "#[path = \"application_tests.rs\"]");
    for anchor in [
        "fn application_bootstraps_live_and_safe_mode_then_marks_clean_after_joined_shutdown()",
        "fn assert_migrated_archive_retries_pending_post_backup_before_live_restart()",
        "fn assert_no_backup_rebuild_preserves_corrupt_truth_and_completes_authoritative_reconciliation()",
        "fn assert_reconstruction_reconciliation_survives_restart_and_retries_without_rebuild()",
        "fn assert_reconstruction_safe_mode_keeps_explicit_reconciliation_retry()",
    ] {
        assert_exact_anchor(APPLICATION_TESTS, anchor);
    }
}

#[test]
fn application_gate_is_bound_to_the_complete_state_recovery_matrix() {
    for (source, anchor) in [
        (
            JOURNAL_TESTS,
            "fn only_the_exact_six_state_sequence_is_accepted_and_same_step_is_idempotent()",
        ),
        (
            RESTORE_TESTS,
            "fn settings_publication_failure_rolls_database_back_to_the_exact_old_main()",
        ),
        (
            RESTORE_TESTS,
            "fn forced_termination_after_every_durable_phase_resumes_to_one_complete_generation()",
        ),
        (
            AUTOMATIC_TESTS,
            "fn a_missing_main_with_prior_backup_evidence_uses_recovery_not_empty_creation()",
        ),
        (
            RESTORE_TESTS,
            "fn automatic_restore_is_forced_to_data_only_and_leaves_settings_unchanged()",
        ),
        (
            MAINTENANCE_TESTS,
            "fn periodic_disablement_keeps_every_mandatory_guard_active()",
        ),
    ] {
        assert_exact_anchor(source, anchor);
    }
}
