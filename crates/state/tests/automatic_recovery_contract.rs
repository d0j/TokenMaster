use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, ExclusiveFileLease, MAX_DURABLE_FILE_BYTES,
    ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCatalog, BackupCompression, BackupMetadata, BackupPackage, BackupPurpose,
    BootstrapOutcome, PortableSettingsCandidate, RecoveryBoundary, RecoveryCoordinator,
    RecoveryJournalStore, RecoveryLaunchDecision, RecoveryPhase, RestoreMode, RunStateStore,
    SettingsStore, SettingsValue, StateBootstrap, StateError, StateErrorCode,
};
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, UsageStore, create_online_snapshot,
    verify_backup_candidate,
};

struct Fixture {
    _root: TempDir,
    data: ValidatedLocalDirectory,
    reliable: ValidatedLocalDirectory,
    scope: ArchiveRecoveryScope,
    backups: BackupDirectory,
    verification_staging: BackupStaging,
    settings: SettingsStore,
    journal: RecoveryJournalStore,
    run_state: RunStateStore,
    control: BackupControl,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("automatic recovery root");
        let reliable_path = root.path().join("reliable-state");
        fs::create_dir(&reliable_path).expect("reliable-state");
        let data = ValidatedLocalDirectory::new(root.path()).expect("data root");
        let reliable = ValidatedLocalDirectory::new(&reliable_path).expect("reliable root");
        let scope = ArchiveRecoveryScope::new(&data, &reliable).expect("recovery scope");
        let backups = BackupDirectory::open_or_create(&reliable).expect("backup directory");
        let verification_root =
            ValidatedLocalDirectory::new(&reliable_path.join("staging")).expect("staging root");
        let verification_staging =
            BackupStaging::new(&verification_root).expect("verification staging");
        let settings = SettingsStore::new(&reliable).expect("settings store");
        let journal = RecoveryJournalStore::new(&reliable).expect("journal store");
        let run_state = RunStateStore::new(&reliable).expect("run-state store");
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("backup control");
        Self {
            _root: root,
            data,
            reliable,
            scope,
            backups,
            verification_staging,
            settings,
            journal,
            run_state,
            control,
        }
    }

    fn archive(&self) -> std::path::PathBuf {
        self.data.as_path().join("tokenmaster.sqlite3")
    }

    fn bootstrap(&self) -> StateBootstrap<'_> {
        StateBootstrap::new(
            &self.data,
            &self.scope,
            &self.verification_staging,
            &self.journal,
            &self.settings,
            &self.run_state,
            &self.backups,
        )
        .expect("bound bootstrap capabilities")
    }

    fn guard(&self) -> tokenmaster_platform::ExclusiveFileLeaseGuard {
        ExclusiveFileLease::for_archive(&self.archive())
            .expect("archive lease")
            .try_acquire()
            .expect("archive guard")
    }

    fn publish_backup(&self, created_at_utc_ms: i64) {
        let source_root = tempfile::tempdir().expect("backup source root");
        let staging_path = source_root.path().join("staging");
        fs::create_dir(&staging_path).expect("source staging");
        let source_archive = source_root.path().join("tokenmaster.sqlite3");
        drop(UsageStore::open(&source_archive).expect("source archive"));
        let source_data =
            ValidatedLocalDirectory::new(source_root.path()).expect("source data root");
        let source_staging_root =
            ValidatedLocalDirectory::new(&staging_path).expect("source staging root");
        let source = BackupSource::new(&source_data).expect("backup source");
        let staging = BackupStaging::new(&source_staging_root).expect("backup staging");
        let verified = verify_backup_candidate(
            create_online_snapshot(&source, &staging, &self.control).expect("snapshot"),
            &self.control,
        )
        .expect("verified snapshot");
        let database = verified
            .open_reader(&self.control)
            .expect("verified database reader");
        let portable =
            PortableSettingsCandidate::new(SettingsValue::safe_defaults().portable().clone())
                .expect("portable settings");
        let mut package = self
            .backups
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .expect("package stage");
        BackupPackage::write_verified_candidate_to_backup_stage(
            &portable,
            database,
            BackupCompression::Automatic,
            BackupMetadata::new(created_at_utc_ms, BackupPurpose::Manual).expect("backup metadata"),
            &mut package,
        )
        .expect("write package");
        BackupPackage::verify_backup_stage(&package).expect("verify package");
        self.backups.publish(&mut package).expect("publish package");
    }
}

#[test]
fn corruption_skips_a_newer_fully_invalid_package_and_restores_the_next_verified_point() {
    let fixture = Fixture::new();
    fixture.publish_backup(1_735_689_600_000);
    fixture.publish_backup(1_735_689_700_000);
    let newer = fixture
        .reliable
        .as_path()
        .join("backups")
        .join("point-01.tmbackup");
    let mut bytes = fs::read(&newer).expect("newer package bytes");
    let last = bytes.last_mut().expect("package footer");
    *last ^= 0x5a;
    fs::write(&newer, bytes).expect("corrupt newer footer");
    fs::write(fixture.archive(), b"definitively-corrupt-active").expect("corrupt active");
    let guard = fixture.guard();

    let prepared = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("automatic recovery");

    assert_eq!(prepared.report().outcome(), BootstrapOutcome::Healthy);
    assert!(prepared.report().recovery_resumed());
    assert_eq!(
        prepared.report().recovery_launch(),
        RecoveryLaunchDecision::Start { launch: 1 }
    );
    let archive = fs::read(fixture.archive()).expect("restored active");
    assert!(archive.starts_with(b"SQLite format 3\0"));
    let catalog = BackupCatalog::rebuild(&fixture.backups, None).expect("bounded catalog rebuild");
    assert_eq!(catalog.points().len(), 2);
}

#[test]
fn cancelled_verification_never_authorizes_automatic_replacement() {
    let fixture = Fixture::new();
    fixture.publish_backup(1_735_689_600_000);
    fs::write(fixture.archive(), b"definitively-corrupt-active").expect("corrupt active");
    let before = fs::read(fixture.archive()).expect("active before cancellation");
    let cancelled = BackupControl::new(Arc::new(AtomicBool::new(true)), Duration::from_secs(30))
        .expect("cancelled control");
    let guard = fixture.guard();

    let prepared = fixture
        .bootstrap()
        .prepare(&guard, &cancelled)
        .expect("cancelled diagnosis");

    assert_eq!(prepared.report().outcome(), BootstrapOutcome::Unavailable);
    assert!(!prepared.report().recovery_resumed());
    assert_eq!(
        fs::read(fixture.archive()).expect("active after cancellation"),
        before
    );
}

#[test]
fn a_missing_main_with_prior_backup_evidence_uses_recovery_not_empty_creation() {
    let fixture = Fixture::new();
    fixture.publish_backup(1_735_689_600_000);
    assert!(!fixture.archive().exists());
    let guard = fixture.guard();

    let prepared = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("missing-main recovery");

    assert_eq!(prepared.report().outcome(), BootstrapOutcome::Healthy);
    assert!(prepared.report().recovery_resumed());
    assert_eq!(
        prepared.report().recovery_launch(),
        RecoveryLaunchDecision::Start { launch: 1 }
    );
    assert!(
        fs::read(fixture.archive())
            .expect("promoted active")
            .starts_with(b"SQLite format 3\0")
    );
}

#[test]
fn the_same_automatic_recovery_receipt_stops_after_two_unclean_launches() {
    let fixture = Fixture::new();
    fixture.publish_backup(1_735_689_600_000);
    fs::write(fixture.archive(), b"definitively-corrupt-active").expect("corrupt active");

    for expected_launch in 1..=2 {
        let guard = fixture.guard();
        let prepared = fixture
            .bootstrap()
            .prepare(&guard, &fixture.control)
            .expect("recovered launch");
        assert_eq!(prepared.report().outcome(), BootstrapOutcome::Healthy);
        assert_eq!(
            prepared.report().recovery_launch(),
            RecoveryLaunchDecision::Start {
                launch: expected_launch
            }
        );
    }

    let guard = fixture.guard();
    let prepared = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("cutoff diagnosis");
    assert_eq!(prepared.report().outcome(), BootstrapOutcome::SafeMode);
    assert_eq!(
        prepared.report().recovery_launch(),
        RecoveryLaunchDecision::SafeMode { failed_launches: 2 }
    );
}

#[test]
fn an_accepted_completed_journal_does_not_block_a_later_independent_recovery() {
    let fixture = Fixture::new();
    fixture.publish_backup(1_735_689_600_000);
    fs::write(fixture.archive(), b"first-corrupt-active").expect("first corruption");
    let guard = fixture.guard();
    let mut first = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("first recovery");
    assert_eq!(
        first.report().recovery_launch(),
        RecoveryLaunchDecision::Start { launch: 1 }
    );
    first
        .session_mut()
        .mark_clean()
        .expect("accepted first recovery");
    drop(first);
    drop(guard);

    fs::write(fixture.archive(), b"later-independent-corruption").expect("later corruption");
    let guard = fixture.guard();
    let second = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("later recovery");

    assert_eq!(
        second.report().outcome(),
        BootstrapOutcome::Healthy,
        "later recovery report: {:?}",
        second.report()
    );
    assert_eq!(
        second.report().recovery_launch(),
        RecoveryLaunchDecision::Start { launch: 1 }
    );
}

#[test]
fn bootstrap_resumes_a_pending_journal_from_a_cold_catalog_before_archive_inspection() {
    let fixture = Fixture::new();
    fixture.publish_backup(1_735_689_600_000);
    fs::write(fixture.archive(), b"definitively-corrupt-active").expect("corrupt active");
    let mut catalog = BackupCatalog::rebuild(&fixture.backups, None).expect("cold catalog");
    let selection = catalog.points()[0].selection();
    let entry = fixture.backups.scan().expect("backup scan").entries()[0].clone();
    let mut reader = fixture
        .backups
        .open_reader(&entry, MAX_DURABLE_FILE_BYTES)
        .expect("backup reader");
    let package = BackupPackage::inspect(&mut reader).expect("full package verification");
    catalog
        .bind_verified(selection, &package)
        .expect("process-local verified binding");
    let guard = fixture.guard();
    let coordinator = RecoveryCoordinator::new(
        &fixture.scope,
        &fixture.verification_staging,
        &fixture.journal,
        &fixture.settings,
    )
    .expect("bound recovery capabilities");
    let error = coordinator
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &catalog,
            selection,
            RestoreMode::AutomaticDataOnly,
            &guard,
            &fixture.control,
            |boundary| {
                if boundary == RecoveryBoundary::JournalDurable(RecoveryPhase::Prepared) {
                    Err(StateError::from_code(StateErrorCode::Unavailable))
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("simulated interruption after durable journal");
    assert_eq!(error.code(), StateErrorCode::Unavailable);
    assert_eq!(
        fs::read(fixture.archive()).expect("not yet inspected active"),
        b"definitively-corrupt-active"
    );
    drop(guard);

    let guard = fixture.guard();
    let prepared = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("journal resume");
    assert_eq!(prepared.report().outcome(), BootstrapOutcome::Healthy);
    assert!(prepared.report().recovery_resumed());
    assert_eq!(
        prepared.report().recovery_launch(),
        RecoveryLaunchDecision::Start { launch: 1 }
    );
}

#[test]
fn bootstrap_cleans_a_prejournal_store_verifier_crash_before_strict_platform_scan() {
    let fixture = Fixture::new();
    fixture.publish_backup(1_735_689_600_000);
    fs::write(fixture.archive(), b"definitively-corrupt-active").expect("corrupt active");
    let mut catalog = BackupCatalog::rebuild(&fixture.backups, None).expect("cold catalog");
    let selection = catalog.points()[0].selection();
    let entry = fixture.backups.scan().expect("backup scan").entries()[0].clone();
    let mut reader = fixture
        .backups
        .open_reader(&entry, MAX_DURABLE_FILE_BYTES)
        .expect("backup reader");
    let package = BackupPackage::inspect(&mut reader).expect("full package verification");
    catalog
        .bind_verified(selection, &package)
        .expect("process-local verified binding");
    let guard = fixture.guard();
    let coordinator = RecoveryCoordinator::new(
        &fixture.scope,
        &fixture.verification_staging,
        &fixture.journal,
        &fixture.settings,
    )
    .expect("bound recovery capabilities");
    let error = coordinator
        .restore_definitively_corrupt_selected_with_observer(
            &fixture.backups,
            &catalog,
            selection,
            RestoreMode::AutomaticDataOnly,
            &guard,
            &fixture.control,
            |boundary| {
                if boundary == RecoveryBoundary::CandidateVerifierFileCreated {
                    Err(StateError::from_code(StateErrorCode::Unavailable))
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("simulated verifier interruption before journal");
    assert_eq!(error.code(), StateErrorCode::Unavailable);
    fs::write(
        fixture
            .reliable
            .as_path()
            .join("staging")
            .join(".tokenmaster-recovery-00.sqlite3"),
        b"crash-left-store-verifier",
    )
    .expect("store verifier crash artifact");
    assert!(
        fs::read_dir(fixture.reliable.as_path().join("staging"))
            .expect("staging evidence")
            .next()
            .is_some()
    );
    drop(guard);

    let guard = fixture.guard();
    let prepared = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("prejournal cleanup and retry");
    assert_eq!(prepared.report().outcome(), BootstrapOutcome::Healthy);
    assert_eq!(
        prepared.report().recovery_launch(),
        RecoveryLaunchDecision::Start { launch: 1 }
    );
}
