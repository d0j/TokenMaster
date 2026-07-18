use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use sha2::Digest;
use tempfile::TempDir;
use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, DURABLE_STAGE_ATTEMPTS, ExclusiveFileLease,
    MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BACKUP_INTERVAL_DEFAULT_SECONDS, BACKUP_QUIET_DEFAULT_SECONDS, BACKUP_RETENTION_DEFAULT_BYTES,
    BACKUP_RETENTION_MIN_BYTES, BackupCatalog, BackupCompression, BackupMetadata, BackupPackage,
    BackupPolicy, BackupPurpose, CatalogSelection, DeviceRoute, DeviceSettings, PortableSettings,
    PortableSettingsCandidate, RecoveryArchiveFacts, RecoveryBoundary, RecoveryCoordinator,
    RecoveryJournalLoad, RecoveryJournalStore, RecoveryPhase, RecoverySettingsMode, ReminderPolicy,
    RestoreMode, SettingsStore, SettingsValue, StateErrorCode,
};
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, UsageStore, create_online_snapshot,
    verify_backup_candidate,
};

#[path = "support/recovery_crash_fixture.rs"]
mod recovery_crash_fixture;

#[test]
fn persisted_archive_facts_accept_empty_sidecars_but_reject_empty_main() {
    let digest = [7_u8; 32];
    let facts = RecoveryArchiveFacts::from_persisted(
        Some((16, digest)),
        Some((0, digest)),
        Some((0, digest)),
    )
    .expect("empty WAL and SHM are valid persisted facts");

    assert_eq!(facts.main().expect("main fact").len(), 16);
    assert_eq!(facts.wal().expect("WAL fact").len(), 0);
    assert_eq!(facts.shm().expect("SHM fact").len(), 0);
    let platform = facts.to_platform().expect("platform expectation");
    assert_eq!(platform.wal().expect("platform WAL").len(), 0);
    assert_eq!(platform.shm().expect("platform SHM").len(), 0);

    assert_eq!(
        RecoveryArchiveFacts::from_persisted(Some((0, digest)), None, None)
            .expect_err("empty main is not a valid SQLite archive set")
            .code(),
        StateErrorCode::InvalidInput
    );
}

#[test]
fn no_usable_backup_reconstructs_a_verified_fresh_archive_and_preserves_corrupt_truth() {
    let fixture = Fixture::new();
    corrupt_only_backup(&fixture);
    let mut catalog = BackupCatalog::rebuild(&fixture.backups, None).expect("cold catalog");
    let guard = fixture.guard();

    let mut boundaries = Vec::new();
    let receipt = fixture
        .coordinator()
        .reconstruct_definitively_corrupt_with_observer(
            &fixture.backups,
            &mut catalog,
            &guard,
            &fixture.control,
            |boundary| {
                boundaries.push(boundary);
                Ok(())
            },
        )
        .unwrap_or_else(|error| {
            panic!("authoritative reconstruction stage: {error:?}, boundaries={boundaries:?}")
        });

    assert!(receipt.reconstructed_from_authoritative_source());
    assert!(receipt.non_reconstructible_domains_lost());
    assert_eq!(
        receipt.settings_mode(),
        RecoverySettingsMode::ReconstructionDataOnly
    );
    let journal = match fixture.journal.load().expect("journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected complete reconstruction journal, got {other:?}"),
    };
    assert_eq!(journal.phase(), RecoveryPhase::Complete);
    assert!(journal.backup().is_none());
    assert_eq!(
        fs::read(
            fixture
                .reliable
                .as_path()
                .join("quarantine")
                .read_dir()
                .expect("quarantine directory")
                .next()
                .expect("quarantine set")
                .expect("quarantine entry")
                .path()
                .join("tokenmaster.sqlite3")
        )
        .expect("quarantined main"),
        b"definitively-corrupt-active"
    );
    drop(guard);
    drop(
        UsageStore::open(fixture.data.as_path().join("tokenmaster.sqlite3"))
            .expect("normal store reopens reconstructed archive"),
    );
}

#[test]
fn reconstruction_resumes_after_promotion_before_journal_advance() {
    let fixture = Fixture::new();
    corrupt_only_backup(&fixture);
    let mut catalog = BackupCatalog::rebuild(&fixture.backups, None).expect("cold catalog");
    let guard = fixture.guard();
    let error = fixture
        .coordinator()
        .reconstruct_definitively_corrupt_with_observer(
            &fixture.backups,
            &mut catalog,
            &guard,
            &fixture.control,
            |boundary| {
                if boundary == RecoveryBoundary::MainPromotedBeforeJournal {
                    return Err(tokenmaster_state::StateError::from_code(
                        StateErrorCode::Unavailable,
                    ));
                }
                Ok(())
            },
        )
        .expect_err("injected promotion crash boundary");
    assert_eq!(error.code(), StateErrorCode::Unavailable);
    let cold = BackupCatalog::rebuild(&fixture.backups, None).expect("resume catalog");

    let receipt = fixture
        .coordinator()
        .resume(&fixture.backups, &cold, &guard, &fixture.control)
        .expect("resume reconstruction")
        .expect("reconstruction receipt");

    assert!(receipt.reconstructed_from_authoritative_source());
    assert!(matches!(
        fixture.journal.load().expect("complete journal"),
        RecoveryJournalLoad::Pending(journal) if journal.phase() == RecoveryPhase::Complete
            && journal.backup().is_none()
    ));
}

fn corrupt_only_backup(fixture: &Fixture) {
    let backup_path = fs::read_dir(fixture.reliable.as_path().join("backups"))
        .expect("backup directory")
        .next()
        .expect("one backup")
        .expect("backup entry")
        .path();
    let mut bytes = fs::read(&backup_path).expect("backup bytes");
    let last = bytes.last_mut().expect("non-empty backup");
    *last ^= 0xff;
    fs::write(&backup_path, bytes).expect("corrupt only backup");
}

struct Fixture {
    _root: Option<TempDir>,
    data: ValidatedLocalDirectory,
    reliable: ValidatedLocalDirectory,
    scope: ArchiveRecoveryScope,
    backups: BackupDirectory,
    verification_staging: BackupStaging,
    settings: SettingsStore,
    journal: RecoveryJournalStore,
    catalog: BackupCatalog,
    selection: CatalogSelection,
    package_settings: PortableSettingsCandidate,
    control: BackupControl,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary TokenMaster root");
        let path = root.path().to_path_buf();
        Self::initialize(&path, Some(root))
    }

    fn initialize(root: &std::path::Path, owner: Option<TempDir>) -> Self {
        let reliable_path = root.join("reliable-state");
        fs::create_dir(&reliable_path).expect("reliable-state");
        let data = ValidatedLocalDirectory::new(root).expect("data root");
        let reliable = ValidatedLocalDirectory::new(&reliable_path).expect("reliable-state root");
        let scope = ArchiveRecoveryScope::new(&data, &reliable).expect("recovery scope");
        let staging_root = ValidatedLocalDirectory::new(&reliable_path.join("staging"))
            .expect("recovery staging root");
        let verification_staging = BackupStaging::new(&staging_root).expect("verification staging");
        let backups = BackupDirectory::open_or_create(&reliable).expect("backup directory");
        let settings = SettingsStore::new(&reliable).expect("settings store");
        let journal = RecoveryJournalStore::new(&reliable).expect("journal store");
        settings
            .save(&SettingsValue::new(
                SettingsValue::safe_defaults().portable().clone(),
                DeviceSettings::new(DeviceRoute::Settings),
            ))
            .expect("initial settings");

        let package_settings = changed_portable_settings();
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("backup control");
        let (catalog, selection) =
            publish_current_sqlite_backup(&backups, &package_settings, &control);
        fs::write(
            root.join("tokenmaster.sqlite3"),
            b"definitively-corrupt-active",
        )
        .expect("corrupt active fixture");

        Self {
            _root: owner,
            data,
            reliable,
            scope,
            backups,
            verification_staging,
            settings,
            journal,
            catalog,
            selection,
            package_settings,
            control,
        }
    }

    fn open_existing(root: &std::path::Path) -> Self {
        let reliable_path = root.join("reliable-state");
        let data = ValidatedLocalDirectory::new(root).expect("existing data root");
        let reliable =
            ValidatedLocalDirectory::new(&reliable_path).expect("existing reliable-state root");
        let scope = ArchiveRecoveryScope::new(&data, &reliable).expect("existing recovery scope");
        let staging_root = ValidatedLocalDirectory::new(&reliable_path.join("staging"))
            .expect("existing staging root");
        let verification_staging =
            BackupStaging::new(&staging_root).expect("existing verification staging");
        let backups = BackupDirectory::open_or_create(&reliable).expect("existing backups");
        let settings = SettingsStore::new(&reliable).expect("existing settings");
        let journal = RecoveryJournalStore::new(&reliable).expect("existing journal");
        let mut catalog = BackupCatalog::rebuild(&backups, None).expect("rebuilt catalog");
        let entry = backups.scan().expect("backup scan").entries()[0].clone();
        let mut reader = backups
            .open_reader(&entry, MAX_DURABLE_FILE_BYTES)
            .expect("backup reader");
        let package = BackupPackage::inspect(&mut reader).expect("reverified package");
        let selection = catalog.points()[0].selection();
        catalog
            .bind_verified(selection, &package)
            .expect("rebound package proof");
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("resume control");
        Self {
            _root: None,
            data,
            reliable,
            scope,
            backups,
            verification_staging,
            settings,
            journal,
            catalog,
            selection,
            package_settings: changed_portable_settings(),
            control,
        }
    }

    fn guard(&self) -> tokenmaster_platform::ExclusiveFileLeaseGuard {
        ExclusiveFileLease::for_archive(&self.data.as_path().join("tokenmaster.sqlite3"))
            .expect("archive lease")
            .try_acquire()
            .expect("held archive lease")
    }

    fn coordinator(&self) -> RecoveryCoordinator<'_> {
        RecoveryCoordinator::new(
            &self.scope,
            &self.verification_staging,
            &self.journal,
            &self.settings,
        )
        .expect("bound recovery capabilities")
    }
}

fn changed_portable_settings() -> PortableSettingsCandidate {
    let reminders = ReminderPolicy::new(true, &[86_400, 10_800]).expect("reminders");
    let backup = BackupPolicy::new(
        true,
        BACKUP_QUIET_DEFAULT_SECONDS,
        BACKUP_INTERVAL_DEFAULT_SECONDS,
        BACKUP_RETENTION_MIN_BYTES,
    )
    .expect("backup policy");
    PortableSettingsCandidate::new(PortableSettings::new(reminders, backup))
        .expect("portable settings")
}

fn publish_current_sqlite_backup(
    directory: &BackupDirectory,
    settings: &PortableSettingsCandidate,
    control: &BackupControl,
) -> (BackupCatalog, CatalogSelection) {
    let source_root = tempfile::tempdir().expect("source root");
    let source_staging_path = source_root.path().join("staging");
    fs::create_dir(&source_staging_path).expect("source staging");
    let archive = source_root.path().join("tokenmaster.sqlite3");
    drop(UsageStore::open(&archive).expect("current source archive"));
    let source_data = ValidatedLocalDirectory::new(source_root.path()).expect("source data root");
    let source_staging_root =
        ValidatedLocalDirectory::new(&source_staging_path).expect("source staging root");
    let source = BackupSource::new(&source_data).expect("backup source");
    let staging = BackupStaging::new(&source_staging_root).expect("source backup staging");
    let verified = verify_backup_candidate(
        create_online_snapshot(&source, &staging, control).expect("online snapshot"),
        control,
    )
    .expect("verified source snapshot");
    let database = verified.open_reader(control).expect("verified reader");
    let empty = BackupCatalog::rebuild(directory, None).expect("empty catalog");
    let mut stage = directory
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("package stage");
    BackupPackage::write_verified_candidate_to_backup_stage(
        settings,
        database,
        BackupCompression::Automatic,
        BackupMetadata::new(1_735_689_600_000, BackupPurpose::Manual).expect("backup metadata"),
        &mut stage,
    )
    .expect("backup package");
    let package = BackupPackage::verify_backup_stage(&stage).expect("verified package");
    directory.publish(&mut stage).expect("published package");
    let mut catalog = BackupCatalog::rebuild(directory, Some(&empty)).expect("published catalog");
    let selection = catalog.points()[0].selection();
    catalog
        .bind_verified(selection, &package)
        .expect("bound package proof");
    (catalog, selection)
}

#[test]
fn full_restore_promotes_verified_sqlite_preserves_device_state_and_resumes_once() {
    let fixture = Fixture::new();
    let guard = fixture.guard();
    let coordinator = fixture.coordinator();

    let receipt = coordinator
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataAndPortableSettings,
            &guard,
            &fixture.control,
        )
        .expect("complete full restore");
    assert_eq!(
        receipt.candidate().schema_version(),
        tokenmaster_store::USAGE_SCHEMA_VERSION as u32
    );
    let active = fs::read(fixture.data.as_path().join("tokenmaster.sqlite3")).expect("active DB");
    assert!(active.starts_with(b"SQLite format 3\0"));
    let loaded = fixture.settings.load().expect("restored settings");
    assert_eq!(
        fixture
            .settings
            .full_backup_candidate()
            .expect("restored portable settings")
            .digest(),
        fixture.package_settings.digest()
    );
    assert_eq!(loaded.value().device().last_route(), DeviceRoute::Settings);
    let committed_generation = loaded.generation();
    let journal = match fixture.journal.load().expect("journal load") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected journal, got {other:?}"),
    };
    assert_eq!(journal.phase(), RecoveryPhase::Complete);

    let resumed = coordinator
        .resume(&fixture.backups, &fixture.catalog, &guard, &fixture.control)
        .expect("resume complete")
        .expect("completed receipt");
    assert_eq!(resumed, receipt);
    assert_eq!(
        fixture.settings.load().expect("same settings").generation(),
        committed_generation
    );
    assert_eq!(
        format!("{coordinator:?}"),
        "RecoveryCoordinator([redacted])"
    );
}

#[test]
fn data_only_restore_leaves_all_settings_unchanged() {
    let fixture = Fixture::new();
    let before = fixture.settings.load().expect("settings before");
    let guard = fixture.guard();

    fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
        )
        .expect("data-only restore");
    let after = fixture.settings.load().expect("settings after");
    assert_eq!(after.generation(), before.generation());
    assert_eq!(after.value(), before.value());
}

#[test]
fn healthy_active_database_cannot_use_the_internal_corruption_authority() {
    let fixture = Fixture::new();
    let guard = fixture.guard();
    fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
        )
        .expect("first recovery");

    let error = fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
        )
        .expect_err("healthy active database is not definitively corrupt");
    assert_eq!(error.code(), StateErrorCode::InvalidInput);
    let journal = match fixture.journal.load().expect("journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected completed journal, got {other:?}"),
    };
    assert_eq!(journal.operation_generation(), 1);
    assert_eq!(journal.phase(), RecoveryPhase::Complete);
}

#[test]
fn a_completed_recovery_allows_a_later_independent_recovery_generation() {
    let fixture = Fixture::new();
    let guard = fixture.guard();
    fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
        )
        .expect("first recovery");
    fs::write(
        fixture.data.as_path().join("tokenmaster.sqlite3"),
        b"later-definitive-corruption",
    )
    .expect("later corruption fixture");

    fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
        )
        .expect("second recovery");
    let journal = match fixture.journal.load().expect("journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected completed journal, got {other:?}"),
    };
    assert_eq!(journal.operation_generation(), 2);
    assert_eq!(journal.phase(), RecoveryPhase::Complete);
}

#[test]
fn automatic_restore_is_forced_to_data_only_and_leaves_settings_unchanged() {
    let fixture = Fixture::new();
    let before = fixture.settings.load().expect("settings before");
    let guard = fixture.guard();

    fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::AutomaticDataOnly,
            &guard,
            &fixture.control,
        )
        .expect("automatic data-only restore");
    let after = fixture.settings.load().expect("settings after");
    assert_eq!(after.generation(), before.generation());
    assert_eq!(after.value(), before.value());
    let journal = match fixture.journal.load().expect("journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected completed automatic journal, got {other:?}"),
    };
    assert_eq!(
        journal.settings_mode(),
        tokenmaster_state::RecoverySettingsMode::AutomaticDataOnly
    );
    assert_eq!(journal.settings_target(), None);
}

#[test]
fn candidate_digest_drift_aborts_before_main_replacement() {
    let fixture = Fixture::new();
    let active_path = fixture.data.as_path().join("tokenmaster.sqlite3");
    let old_main = fs::read(&active_path).expect("old active");
    let staging = fixture.reliable.as_path().join("staging");
    let guard = fixture.guard();

    let error = fixture
        .coordinator()
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
            |boundary| {
                if boundary == RecoveryBoundary::JournalDurable(RecoveryPhase::Prepared) {
                    let candidate = fs::read_dir(&staging)
                        .expect("recovery staging")
                        .map(|entry| entry.expect("staging entry").path())
                        .find(|path| {
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .is_some_and(|name| {
                                    name.starts_with("restore-") && name.ends_with(".sqlite3")
                                })
                        })
                        .expect("sealed recovery candidate");
                    fs::write(candidate, b"candidate-digest-drift").expect("tamper candidate");
                }
                Ok(())
            },
        )
        .expect_err("candidate drift must fail closed");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(fs::read(active_path).expect("unchanged active"), old_main);
    let journal = match fixture.journal.load().expect("journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected pending journal, got {other:?}"),
    };
    assert_eq!(journal.phase(), RecoveryPhase::SidecarsQuarantined);
}

#[test]
fn active_identity_drift_aborts_before_sidecar_quarantine_or_replacement() {
    let fixture = Fixture::new();
    let active_path = fixture.data.as_path().join("tokenmaster.sqlite3");
    let guard = fixture.guard();

    let error = fixture
        .coordinator()
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
            |boundary| {
                if boundary == RecoveryBoundary::JournalDurable(RecoveryPhase::Prepared) {
                    fs::write(&active_path, b"changed-after-prepared-proof")
                        .expect("change active identity");
                }
                Ok(())
            },
        )
        .expect_err("active drift must fail closed");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(
        fs::read(active_path).expect("changed active retained"),
        b"changed-after-prepared-proof"
    );
    let journal = match fixture.journal.load().expect("journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected prepared journal, got {other:?}"),
    };
    assert_eq!(journal.phase(), RecoveryPhase::Prepared);
}

#[test]
fn settings_publication_failure_rolls_database_back_to_the_exact_old_main() {
    let fixture = Fixture::new();
    let active_path = fixture.data.as_path().join("tokenmaster.sqlite3");
    let old_main = fs::read(&active_path).expect("old active");
    let old_settings = fixture.settings.load().expect("old settings");
    let guard = fixture.guard();
    let reliable = fixture.reliable.as_path().to_path_buf();

    let error = fixture
        .coordinator()
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataAndPortableSettings,
            &guard,
            &fixture.control,
            |phase| {
                if phase == RecoveryBoundary::JournalDurable(RecoveryPhase::ReopenedVerified) {
                    for attempt in 0..DURABLE_STAGE_ATTEMPTS {
                        fs::write(
                            reliable
                                .join(format!(".settings-b.tms.tokenmaster-stage-{attempt:02}")),
                            b"occupied",
                        )
                        .expect("occupy settings stage");
                    }
                }
                Ok(())
            },
        )
        .expect_err("settings publication must fail");
    assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    assert_eq!(fs::read(active_path).expect("rolled-back active"), old_main);
    let after = fixture.settings.load().expect("settings after rollback");
    assert_eq!(after.generation(), old_settings.generation());
    assert_eq!(after.value(), old_settings.value());
    let journal = match fixture.journal.load().expect("pending rollback journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected pending journal, got {other:?}"),
    };
    assert_eq!(journal.phase(), RecoveryPhase::ReopenedVerified);
}

#[test]
fn error_after_settings_slot_publication_is_reclassified_from_the_exact_target() {
    let fixture = Fixture::new();
    let guard = fixture.guard();
    let receipt = fixture
        .coordinator()
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataAndPortableSettings,
            &guard,
            &fixture.control,
            |boundary| {
                if boundary == RecoveryBoundary::SettingsRecordPublishedBeforeReread {
                    return Err(tokenmaster_state::StateError::from_code(
                        StateErrorCode::RecoveryRequired,
                    ));
                }
                Ok(())
            },
        )
        .expect("published settings target must roll forward");
    assert_eq!(
        receipt.settings_mode(),
        tokenmaster_state::RecoverySettingsMode::DataAndPortableSettings
    );
    assert_eq!(
        fixture.settings.load().expect("settings").generation(),
        Some(2)
    );
    let journal = match fixture.journal.load().expect("journal") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected completed journal, got {other:?}"),
    };
    assert_eq!(journal.phase(), RecoveryPhase::Complete);
    assert!(
        fs::read(fixture.data.as_path().join("tokenmaster.sqlite3"))
            .expect("active database")
            .starts_with(b"SQLite format 3\0")
    );
}

#[test]
fn post_replacement_validation_failure_rolls_back_and_preserves_failed_bytes() {
    let fixture = Fixture::new();
    let active_path = fixture.data.as_path().join("tokenmaster.sqlite3");
    let old_main = fs::read(&active_path).expect("old active");
    let guard = fixture.guard();
    let active_for_hook = active_path.clone();

    let error = fixture
        .coordinator()
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
            |phase| {
                if phase == RecoveryBoundary::JournalDurable(RecoveryPhase::MainReplaced) {
                    fs::write(&active_for_hook, b"changed-after-replacement")
                        .expect("sabotage promoted active");
                }
                Ok(())
            },
        )
        .expect_err("active revalidation must fail");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(fs::read(active_path).expect("rolled-back active"), old_main);
    let quarantine = fixture.reliable.as_path().join("quarantine");
    let set = fs::read_dir(quarantine)
        .expect("quarantine")
        .next()
        .expect("one set")
        .expect("set entry")
        .path();
    assert_eq!(
        fs::read(set.join("failed-main.sqlite3")).expect("failed promoted bytes"),
        b"changed-after-replacement"
    );
}

#[test]
fn stale_catalog_package_aborts_before_active_mutation() {
    let fixture = Fixture::new();
    let active_path = fixture.data.as_path().join("tokenmaster.sqlite3");
    let before = fs::read(&active_path).expect("active before");
    let package = fixture
        .reliable
        .as_path()
        .join("backups")
        .join("point-00.tmbackup");
    let mut bytes = fs::read(&package).expect("package bytes");
    bytes.push(0xff);
    fs::write(package, bytes).expect("stale package mutation");
    let guard = fixture.guard();

    let error = fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
        )
        .expect_err("stale catalog");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(fs::read(active_path).expect("active unchanged"), before);
    assert!(matches!(
        fixture.journal.load().expect("no journal"),
        RecoveryJournalLoad::Absent
    ));
    assert_eq!(
        fs::read_dir(fixture.reliable.as_path().join("quarantine"))
            .expect("quarantine scan")
            .count(),
        0
    );
}

#[test]
fn verified_backup_authorizes_recovery_when_the_prior_main_is_missing() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.data.as_path().join("tokenmaster.sqlite3"))
        .expect("remove corrupt prior artifact");
    let guard = fixture.guard();
    fixture
        .coordinator()
        .restore_definitively_corrupt_selected(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::AutomaticDataOnly,
            &guard,
            &fixture.control,
        )
        .expect("missing prior main is a recoverable damaged installation");
    assert!(
        fs::read(fixture.data.as_path().join("tokenmaster.sqlite3"))
            .expect("recovered main")
            .starts_with(b"SQLite format 3\0")
    );
    assert_eq!(
        fixture
            .settings
            .load()
            .expect("settings unchanged")
            .value()
            .portable()
            .backup()
            .retention_budget_bytes(),
        BACKUP_RETENTION_DEFAULT_BYTES
    );
}

#[test]
fn absent_journal_discards_only_abandoned_recovery_staging_before_startup_returns() {
    let fixture = Fixture::new();
    let guard = fixture.guard();
    let operation = fixture
        .scope
        .reserve_operation(&guard)
        .expect("abandoned operation");
    let bytes = b"abandoned-before-journal";
    let digest: [u8; 32] = sha2::Sha256::digest(bytes).into();
    let mut candidate = fixture
        .scope
        .create_candidate(&operation, bytes.len() as u64)
        .expect("abandoned candidate");
    candidate.write_chunk(bytes).expect("candidate bytes");
    candidate
        .seal(bytes.len() as u64, digest)
        .expect("candidate seal");
    drop(candidate);
    assert_eq!(
        fs::read_dir(fixture.reliable.as_path().join("staging"))
            .expect("staging")
            .count(),
        2
    );

    assert!(
        fixture
            .coordinator()
            .resume(&fixture.backups, &fixture.catalog, &guard, &fixture.control)
            .expect("no pending recovery")
            .is_none()
    );
    assert_eq!(
        fs::read_dir(fixture.reliable.as_path().join("staging"))
            .expect("clean staging")
            .count(),
        0
    );
}

#[test]
fn wrong_guard_cannot_delete_any_prejournal_recovery_evidence() {
    let fixture = Fixture::new();
    let guard = fixture.guard();
    let operation = fixture
        .scope
        .reserve_operation(&guard)
        .expect("recovery reservation");
    let bytes = b"sealed-platform-candidate";
    let digest: [u8; 32] = sha2::Sha256::digest(bytes).into();
    let mut candidate = fixture
        .scope
        .create_candidate(&operation, bytes.len() as u64)
        .expect("platform candidate");
    candidate.write_chunk(bytes).expect("candidate bytes");
    candidate
        .seal(bytes.len() as u64, digest)
        .expect("candidate seal");
    drop(candidate);
    let staging = fixture.reliable.as_path().join("staging");
    fs::write(
        staging.join(".tokenmaster-recovery-00.sqlite3"),
        b"store-verifier-evidence",
    )
    .expect("store verifier evidence");
    let snapshot = || {
        let mut entries = fs::read_dir(&staging)
            .expect("staging scan")
            .map(|entry| {
                let path = entry.expect("staging entry").path();
                (
                    path.file_name()
                        .expect("entry name")
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).expect("entry bytes"),
                )
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries
    };
    let before = snapshot();
    assert_eq!(before.len(), 3);
    fs::write(fixture.data.as_path().join("other.sqlite3"), b"other").expect("other archive");
    let wrong_guard =
        ExclusiveFileLease::for_archive(&fixture.data.as_path().join("other.sqlite3"))
            .expect("other lease")
            .try_acquire()
            .expect("held other lease");

    let error = fixture
        .coordinator()
        .resume(
            &fixture.backups,
            &fixture.catalog,
            &wrong_guard,
            &fixture.control,
        )
        .expect_err("wrong authority must fail before cleanup");

    assert_eq!(error.code(), StateErrorCode::Unavailable);
    assert_eq!(snapshot(), before);
}

#[test]
fn recovery_coordinator_rejects_cross_root_components_and_backup_directory() {
    let left = Fixture::new();
    let right = Fixture::new();
    assert!(
        RecoveryCoordinator::new(
            &left.scope,
            &right.verification_staging,
            &right.journal,
            &right.settings,
        )
        .is_err()
    );

    let before =
        fs::read(left.data.as_path().join("tokenmaster.sqlite3")).expect("left active before");
    let guard = left.guard();
    let error = left
        .coordinator()
        .resume(&right.backups, &right.catalog, &guard, &left.control)
        .expect_err("cross-root backup directory");
    assert_eq!(error.code(), StateErrorCode::Unavailable);
    assert_eq!(
        fs::read(left.data.as_path().join("tokenmaster.sqlite3")).expect("left active after"),
        before
    );
}

#[test]
fn recovery_never_exceeds_three_live_staging_artifacts() {
    let fixture = Fixture::new();
    let guard = fixture.guard();
    let staging = fixture.reliable.as_path().join("staging");
    let mut observed = Vec::new();

    fixture
        .coordinator()
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataOnly,
            &guard,
            &fixture.control,
            |boundary| {
                if matches!(
                    boundary,
                    RecoveryBoundary::CandidateVerifierFileCreated
                        | RecoveryBoundary::CorruptionVerifierFileCreated
                ) {
                    observed.push((
                        boundary,
                        fs::read_dir(&staging).expect("staging scan").count(),
                    ));
                }
                Ok(())
            },
        )
        .expect("bounded recovery");

    assert_eq!(
        observed,
        vec![
            (RecoveryBoundary::CandidateVerifierFileCreated, 3),
            (RecoveryBoundary::CorruptionVerifierFileCreated, 3),
        ]
    );
}

#[test]
fn recovery_crash_child() {
    let Some((root, target_phase)) = recovery_crash_fixture::child_root_and_phase() else {
        return;
    };
    let fixture = Fixture::initialize(&root, None);
    let guard = fixture.guard();
    fixture
        .coordinator()
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &fixture.catalog,
            fixture.selection,
            RestoreMode::DataAndPortableSettings,
            &guard,
            &fixture.control,
            |boundary| {
                let boundary_name = match boundary {
                    RecoveryBoundary::BeforeJournalPublication => {
                        "BeforeJournalPublication".to_owned()
                    }
                    RecoveryBoundary::ReconstructionCandidateCreated => {
                        "ReconstructionCandidateCreated".to_owned()
                    }
                    RecoveryBoundary::ReconstructionCandidateStaged => {
                        "ReconstructionCandidateStaged".to_owned()
                    }
                    RecoveryBoundary::JournalDurable(phase) => format!("{phase:?}"),
                    RecoveryBoundary::SidecarsQuarantinedBeforeJournal => {
                        "SidecarsQuarantinedBeforeJournal".to_owned()
                    }
                    RecoveryBoundary::MainPromotedBeforeJournal => {
                        "MainPromotedBeforeJournal".to_owned()
                    }
                    RecoveryBoundary::SettingsCommittedBeforeJournal => {
                        "SettingsCommittedBeforeJournal".to_owned()
                    }
                    RecoveryBoundary::SettingsRecordPublishedBeforeReread => {
                        "SettingsRecordPublishedBeforeReread".to_owned()
                    }
                    RecoveryBoundary::CandidateVerifierFileCreated => {
                        "CandidateVerifierFileCreated".to_owned()
                    }
                    RecoveryBoundary::ActiveVerifierFileCreated => {
                        "ActiveVerifierFileCreated".to_owned()
                    }
                    RecoveryBoundary::CorruptionVerifierFileCreated => {
                        "CorruptionVerifierFileCreated".to_owned()
                    }
                    RecoveryBoundary::FirstPreparedJournalSlotPublished => {
                        "FirstPreparedJournalSlotPublished".to_owned()
                    }
                };
                if boundary_name == target_phase {
                    recovery_crash_fixture::signal_and_wait(&root);
                }
                Ok(())
            },
        )
        .expect("child restore completes only without requested crash phase");
}

#[test]
fn forced_termination_after_every_durable_phase_resumes_to_one_complete_generation() {
    for phase in [
        "Prepared",
        "FirstPreparedJournalSlotPublished",
        "SidecarsQuarantinedBeforeJournal",
        "SidecarsQuarantined",
        "MainPromotedBeforeJournal",
        "MainReplaced",
        "ActiveVerifierFileCreated",
        "ReopenedVerified",
        "SettingsRecordPublishedBeforeReread",
        "SettingsCommittedBeforeJournal",
        "SettingsPublished",
        "Complete",
    ] {
        let root = tempfile::tempdir().expect("persistent crash root");
        recovery_crash_fixture::kill_after_durable_phase(root.path(), phase);
        let fixture = Fixture::open_existing(root.path());
        let active_before_resume =
            fs::read(fixture.data.as_path().join("tokenmaster.sqlite3")).expect("active bytes");
        assert!(
            active_before_resume == b"definitively-corrupt-active"
                || active_before_resume.starts_with(b"SQLite format 3\0"),
            "forced termination at {phase} left mixed main bytes"
        );
        let guard = fixture.guard();
        let cold_catalog =
            BackupCatalog::rebuild(&fixture.backups, None).expect("cold startup catalog");
        let receipt = fixture
            .coordinator()
            .resume(&fixture.backups, &cold_catalog, &guard, &fixture.control)
            .unwrap_or_else(|error| panic!("resume after forced termination at {phase}: {error:?}"))
            .expect("pending or complete recovery receipt");
        assert_eq!(
            receipt.candidate().schema_version(),
            tokenmaster_store::USAGE_SCHEMA_VERSION as u32
        );
        let journal = match fixture.journal.load().expect("completed journal") {
            RecoveryJournalLoad::Pending(journal) => journal,
            other => panic!("expected completed journal after {phase}, got {other:?}"),
        };
        assert_eq!(journal.phase(), RecoveryPhase::Complete);
        assert_eq!(
            fixture.settings.load().expect("settings").generation(),
            Some(2)
        );
        assert!(
            fs::read(fixture.data.as_path().join("tokenmaster.sqlite3"))
                .expect("completed active")
                .starts_with(b"SQLite format 3\0")
        );
    }
}

#[test]
fn forced_termination_during_prejournal_store_verification_cleans_and_retries() {
    for phase in [
        "CandidateVerifierFileCreated",
        "CorruptionVerifierFileCreated",
    ] {
        let root = tempfile::tempdir().expect("persistent verifier crash root");
        recovery_crash_fixture::kill_after_durable_phase(root.path(), phase);
        let fixture = Fixture::open_existing(root.path());
        let guard = fixture.guard();
        assert!(
            fixture
                .coordinator()
                .resume(&fixture.backups, &fixture.catalog, &guard, &fixture.control)
                .unwrap_or_else(|error| panic!("cleanup after {phase}: {error:?}"))
                .is_none()
        );
        assert_eq!(
            fs::read_dir(fixture.reliable.as_path().join("staging"))
                .expect("clean verifier staging")
                .count(),
            0
        );
        assert_eq!(
            fs::read(fixture.data.as_path().join("tokenmaster.sqlite3"))
                .expect("old active retained"),
            b"definitively-corrupt-active"
        );
        fixture
            .coordinator()
            .restore_definitively_corrupt_selected(
                &fixture.backups,
                &fixture.catalog,
                fixture.selection,
                RestoreMode::DataOnly,
                &guard,
                &fixture.control,
            )
            .unwrap_or_else(|error| panic!("retry after {phase}: {error:?}"));
    }
}
