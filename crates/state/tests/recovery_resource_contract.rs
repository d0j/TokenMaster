#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::atomic::AtomicBool;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use rusqlite::Connection;
#[cfg(windows)]
use serde_json::json;
#[cfg(windows)]
use tempfile::TempDir;
#[cfg(windows)]
use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, DurableFileTarget, ExclusiveFileLease,
    MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory,
};
#[cfg(windows)]
use tokenmaster_state::{
    BackupCatalog, BackupCompression, BackupEncryptionContext, BackupMetadata, BackupPackage,
    BackupPassphrase, BackupPurpose, EncryptedBackupPackage, MAX_RETAINED_VERIFIED_POINTS,
    PortableSettingsCandidate, RecoveryCoordinator, RecoveryJournalStore, RestoreMode,
    RetentionAdmission, RetentionPolicy, SettingsStore, SettingsValue,
};
#[cfg(windows)]
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, RecoveryVerificationBoundary, UsageStore,
    create_compact_snapshot, create_online_snapshot, verify_backup_candidate,
    verify_recovery_archive_with_observer,
};

#[cfg(windows)]
#[path = "support/resource.rs"]
mod resource_support;

#[cfg(windows)]
const WARMUP_CYCLES: usize = 64;
#[cfg(windows)]
const MEASURED_CYCLES: usize = 256;
#[cfg(windows)]
const PRIVATE_RETURN_TOLERANCE: usize = 16 * 1024 * 1024;
#[cfg(windows)]
const DAY_MS: i64 = 86_400_000;
#[cfg(windows)]
const BASE_TIME_MS: i64 = 1_700_000_000_000;

#[cfg(windows)]
struct ResourceFixture {
    _root: TempDir,
    source: BackupSource,
    staging: BackupStaging,
    staging_path: PathBuf,
    backups: BackupDirectory,
    data_root: ValidatedLocalDirectory,
    encrypted_root: ValidatedLocalDirectory,
    settings: PortableSettingsCandidate,
}

#[cfg(windows)]
impl ResourceFixture {
    fn new() -> Self {
        let root = TempDir::new().expect("resource fixture root");
        let archive = root.path().join("tokenmaster.sqlite3");
        drop(UsageStore::open(&archive).expect("create current archive"));
        let connection = Connection::open(&archive).expect("open resource fixture");
        connection
            .execute_batch(
                "PRAGMA wal_autocheckpoint=0;
                 CREATE TABLE tm_resource_filler(
                     id INTEGER PRIMARY KEY,
                     payload BLOB NOT NULL
                 ) STRICT;
                 INSERT INTO tm_resource_filler VALUES(1, zeroblob(1048576));
                 INSERT INTO tm_resource_filler VALUES(2, zeroblob(1048576));
                 DROP TABLE tm_resource_filler;
                 PRAGMA wal_checkpoint(TRUNCATE);",
            )
            .expect("create bounded resource fixture");
        drop(connection);

        let staging_path = root.path().join("reliable-state").join("staging");
        let reliable_path = root.path().join("reliable-state");
        let encrypted_path = root.path().join("encrypted");
        fs::create_dir(&reliable_path).expect("create reliable root");
        fs::create_dir(&staging_path).expect("create verification staging");
        fs::create_dir(&encrypted_path).expect("create encrypted staging root");
        let data_root = ValidatedLocalDirectory::new(root.path()).expect("validated data root");
        let staging_root =
            ValidatedLocalDirectory::new(&staging_path).expect("validated verification staging");
        let reliable_root =
            ValidatedLocalDirectory::new(&reliable_path).expect("validated reliable root");
        let encrypted_root =
            ValidatedLocalDirectory::new(&encrypted_path).expect("validated encrypted root");

        Self {
            _root: root,
            source: BackupSource::new(&data_root).expect("backup source"),
            staging: BackupStaging::new(&staging_root).expect("backup staging"),
            staging_path,
            backups: BackupDirectory::open_or_create(&reliable_root).expect("backup directory"),
            data_root,
            encrypted_root,
            settings: PortableSettingsCandidate::new(
                SettingsValue::safe_defaults().portable().clone(),
            )
            .expect("portable settings"),
        }
    }

    fn run_backup_verify_import_cancel(
        &self,
        catalog: BackupCatalog,
        sequence: usize,
    ) -> (BackupCatalog, u64) {
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("backup control");
        let candidate = verify_backup_candidate(
            create_online_snapshot(&self.source, &self.staging, &control).expect("online snapshot"),
            &control,
        )
        .expect("verify online snapshot");
        let reader = candidate.open_reader(&control).expect("candidate reader");
        let mut stage = self
            .backups
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .expect("backup stage");
        BackupPackage::write_verified_candidate_to_backup_stage(
            &self.settings,
            reader,
            BackupCompression::Automatic,
            BackupMetadata::new(
                BASE_TIME_MS + i64::try_from(sequence).expect("sequence time") * DAY_MS,
                BackupPurpose::Periodic,
            )
            .expect("backup metadata"),
            &mut stage,
        )
        .expect("write automatic package");
        let verified = BackupPackage::verify_backup_stage(&stage).expect("verify package stage");
        let package_bytes = verified.receipt().package_len();
        let admission =
            RetentionAdmission::preflight(&catalog, &verified, RetentionPolicy::default())
                .expect("retention preflight");
        let published_entry = self.backups.publish(&mut stage).expect("publish package");

        let mut import_reader = self
            .backups
            .open_reader(&published_entry, MAX_DURABLE_FILE_BYTES)
            .expect("open published package for import preview");
        let imported = BackupPackage::inspect(&mut import_reader).expect("verify import preview");
        assert_eq!(imported.receipt(), verified.receipt());
        drop(imported); // Cancelling an import retains no reader, expanded database, or preview.

        let mut published =
            BackupCatalog::rebuild(&self.backups, Some(&catalog)).expect("rebuild after publish");
        let selection = published
            .bind_published(&verified)
            .expect("bind published package");
        let retention = admission
            .confirm_published(&published, selection)
            .expect("confirm retention publication");
        while retention
            .delete_next(&published, &self.backups)
            .expect("bounded retention deletion")
        {
            published = BackupCatalog::rebuild(&self.backups, Some(&published))
                .expect("rebuild after one deletion");
        }
        drop(candidate);
        assert!(published.points().len() <= MAX_RETAINED_VERIFIED_POINTS);
        assert_eq!(tree_bytes(&self.staging_path), 0);
        (published, package_bytes)
    }

    fn force_cancel_after_candidate_and_recover(&self) {
        let target = DurableFileTarget::exact_child(&self.data_root, "tokenmaster.sqlite3")
            .expect("active archive target");
        let reader = target
            .open_reader(MAX_DURABLE_FILE_BYTES)
            .expect("open active recovery reader")
            .expect("active archive exists");
        let cancelled = Arc::new(AtomicBool::new(false));
        let control = BackupControl::new(Arc::clone(&cancelled), Duration::from_secs(5))
            .expect("cancelled backup control");
        let error =
            verify_recovery_archive_with_observer(reader, &self.staging, &control, |boundary| {
                if boundary == RecoveryVerificationBoundary::CandidateCreated {
                    cancelled.store(true, std::sync::atomic::Ordering::Release);
                }
            })
            .expect_err("mid-operation recovery verification cancellation must fail closed");
        assert_eq!(error.code(), tokenmaster_store::StoreErrorCode::Cancelled);
        self.staging
            .recover_abandoned_candidates()
            .expect("recover cancelled staging");
        assert_eq!(tree_bytes(&self.staging_path), 0);
    }

    fn restore_cycle() {
        let root = TempDir::new().expect("restore resource fixture root");
        let archive = root.path().join("tokenmaster.sqlite3");
        drop(UsageStore::open(&archive).expect("create restore source archive"));
        let reliable_path = root.path().join("reliable-state");
        let staging_path = reliable_path.join("staging");
        fs::create_dir(&reliable_path).expect("create restore reliable root");
        fs::create_dir(&staging_path).expect("create restore staging root");
        let data_root = ValidatedLocalDirectory::new(root.path()).expect("restore data root");
        let reliable_root =
            ValidatedLocalDirectory::new(&reliable_path).expect("restore reliable root");
        let staging_root =
            ValidatedLocalDirectory::new(&staging_path).expect("restore staging root");
        let staging = BackupStaging::new(&staging_root).expect("restore backup staging");
        let backups =
            BackupDirectory::open_or_create(&reliable_root).expect("restore backup directory");
        let settings_store = SettingsStore::new(&reliable_root).expect("restore settings store");
        settings_store
            .save(&SettingsValue::safe_defaults())
            .expect("initial restore settings");
        let journal = RecoveryJournalStore::new(&reliable_root).expect("restore journal");
        let source = BackupSource::new(&data_root).expect("restore backup source");
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("restore backup control");
        let candidate = verify_backup_candidate(
            create_online_snapshot(&source, &staging, &control).expect("restore online snapshot"),
            &control,
        )
        .expect("restore verified snapshot");
        let reader = candidate.open_reader(&control).expect("restore reader");
        let empty = BackupCatalog::rebuild(&backups, None).expect("empty restore catalog");
        let mut stage = backups
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .expect("restore package stage");
        let package_settings =
            PortableSettingsCandidate::new(SettingsValue::safe_defaults().portable().clone())
                .expect("restore package settings");
        BackupPackage::write_verified_candidate_to_backup_stage(
            &package_settings,
            reader,
            BackupCompression::Automatic,
            BackupMetadata::new(BASE_TIME_MS, BackupPurpose::Manual)
                .expect("restore package metadata"),
            &mut stage,
        )
        .expect("restore package");
        let package = BackupPackage::verify_backup_stage(&stage).expect("restore verified package");
        backups
            .publish(&mut stage)
            .expect("publish restore package");
        let mut catalog = BackupCatalog::rebuild(&backups, Some(&empty)).expect("restore catalog");
        let selection = catalog
            .bind_published(&package)
            .expect("bind restore package");
        drop(candidate);

        fs::write(&archive, b"definitively-corrupt-active")
            .expect("create definitively corrupt active archive");
        let guard = ExclusiveFileLease::for_archive(&archive)
            .expect("archive lease")
            .try_acquire()
            .expect("exclusive archive lease");
        let recovery_scope =
            ArchiveRecoveryScope::new(&data_root, &reliable_root).expect("recovery scope");
        let coordinator =
            RecoveryCoordinator::new(&recovery_scope, &staging, &journal, &settings_store)
                .expect("recovery coordinator");
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("restore control");
        let receipt = coordinator
            .restore_definitively_corrupt_selected(
                &backups,
                &catalog,
                selection,
                RestoreMode::DataOnly,
                &guard,
                &control,
            )
            .expect("complete measured restore");
        assert_eq!(
            receipt.candidate().schema_version(),
            tokenmaster_store::USAGE_SCHEMA_VERSION as u32
        );
    }

    fn encrypted_compact_cycle(&self) {
        let control =
            BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(120))
                .expect("compact backup control");
        let snapshot = verify_backup_candidate(
            create_online_snapshot(&self.source, &self.staging, &control)
                .expect("online snapshot for compact export"),
            &control,
        )
        .expect("verify snapshot for compact export");
        let compact = create_compact_snapshot(&snapshot, &self.staging, &control)
            .expect("create compact candidate");
        let reader = compact.open_reader(&control).expect("compact reader");
        let mut package_stage = self
            .backups
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .expect("compact package stage");
        BackupPackage::write_verified_candidate_to_backup_stage(
            &self.settings,
            reader,
            BackupCompression::Compact,
            BackupMetadata::new(BASE_TIME_MS - DAY_MS, BackupPurpose::Manual)
                .expect("compact metadata"),
            &mut package_stage,
        )
        .expect("write compact package");
        let verified =
            BackupPackage::verify_backup_stage(&package_stage).expect("verify compact package");
        let mut package_reader = package_stage.open_reader().expect("compact package reader");
        let target =
            DurableFileTarget::exact_child(&self.encrypted_root, "manual-compact.tmbackup.age")
                .expect("encrypted output target");
        let mut encrypted_stage = target
            .create_staged(MAX_DURABLE_FILE_BYTES)
            .expect("encrypted output stage");
        EncryptedBackupPackage::encrypt(
            BackupEncryptionContext::ManualExport,
            &mut package_reader,
            &verified,
            new_passphrase(),
            &mut encrypted_stage,
        )
        .expect("encrypt compact package");
        encrypted_stage
            .discard()
            .expect("discard encrypted export stage");
        package_stage
            .discard()
            .expect("discard compact package stage");
    }
}

#[cfg(windows)]
fn new_passphrase() -> BackupPassphrase {
    let mut input = "correct horse battery staple 42".to_owned();
    let mut confirmation = input.clone();
    BackupPassphrase::new(&mut input, &mut confirmation).expect("valid bounded passphrase")
}

#[cfg(windows)]
fn tree_bytes(path: &Path) -> u64 {
    let mut total = 0_u64;
    for entry in fs::read_dir(path).expect("read bounded fixture tree") {
        let entry = entry.expect("fixture child");
        let metadata = entry.metadata().expect("fixture child metadata");
        if metadata.is_dir() {
            total = total
                .checked_add(tree_bytes(&entry.path()))
                .expect("fixture byte count");
        } else if metadata.is_file() {
            total = total
                .checked_add(metadata.len())
                .expect("fixture byte count");
        }
    }
    total
}

#[cfg(windows)]
#[test]
fn repeated_backup_verify_import_cancel_cycles_return_resources_and_disk() {
    let fixture = ResourceFixture::new();
    let mut catalog = BackupCatalog::rebuild(&fixture.backups, None).expect("initial catalog");
    let mut largest_package = 0_u64;
    for sequence in 0..WARMUP_CYCLES {
        (catalog, largest_package) = {
            let (next, package_bytes) = fixture.run_backup_verify_import_cancel(catalog, sequence);
            (next, largest_package.max(package_bytes))
        };
    }
    let baseline_disk_bytes = fixture
        .backups
        .scan()
        .expect("warm backup scan")
        .entries()
        .iter()
        .map(tokenmaster_platform::BackupDirectoryEntry::len)
        .sum::<u64>();
    fixture.force_cancel_after_candidate_and_recover();
    ResourceFixture::restore_cycle();
    let baseline = resource_support::sample();
    assert_eq!(baseline.child_processes, 0);
    let mut disk_high_water = baseline_disk_bytes;
    let mut forced_failures = 0_u64;
    let mut restore_cycles = 0_u64;

    for measured in 0..MEASURED_CYCLES {
        if measured % 16 == 0 {
            fixture.force_cancel_after_candidate_and_recover();
            forced_failures += 1;
        }
        let sequence = WARMUP_CYCLES + measured;
        let (next, package_bytes) = fixture.run_backup_verify_import_cancel(catalog, sequence);
        catalog = next;
        largest_package = largest_package.max(package_bytes);
        let disk_bytes = fixture
            .backups
            .scan()
            .expect("measured backup scan")
            .entries()
            .iter()
            .map(tokenmaster_platform::BackupDirectoryEntry::len)
            .sum::<u64>();
        disk_high_water = disk_high_water.max(disk_bytes);
        assert_eq!(
            disk_bytes, baseline_disk_bytes,
            "retention bytes did not return to the filled-tier plateau"
        );
        if measured % 16 == 15 {
            ResourceFixture::restore_cycle();
            restore_cycles += 1;
        }
        if measured % 16 == 15 {
            let settled = resource_support::settle_to(
                baseline,
                PRIVATE_RETURN_TOLERANCE,
                1,
                Duration::from_secs(10),
            );
            assert_eq!(settled.child_processes, 0);
        }
    }

    resource_support::settle_to(
        baseline,
        PRIVATE_RETURN_TOLERANCE,
        1,
        Duration::from_secs(10),
    );
    let encryption_monitor = resource_support::ResourceMonitor::start();
    fixture.encrypted_compact_cycle();
    let encryption_resources = encryption_monitor.finish();
    assert!(
        encryption_resources.peak.threads
            <= encryption_resources.baseline.threads.saturating_add(1),
        "encrypted compact export created a thread beyond the sampler: {encryption_resources:?}"
    );
    assert_eq!(encryption_resources.peak.child_processes, 0);
    let final_resources = resource_support::settle_to(
        baseline,
        PRIVATE_RETURN_TOLERANCE,
        1,
        Duration::from_secs(20),
    );
    assert_eq!(tree_bytes(&fixture.staging_path), 0);
    assert_eq!(final_resources.child_processes, 0);

    println!(
        "P3D0_RECOVERY_RESOURCES={}",
        json!({
            "schema": "tokenmaster.p3d0.recovery-resources.v1",
            "warmup_cycles": WARMUP_CYCLES,
            "measured_cycles": MEASURED_CYCLES,
            "forced_failure_recovery_cycles": forced_failures,
            "restore_cycles": restore_cycles,
            "retained_points": catalog.points().len(),
            "baseline_disk_bytes": baseline_disk_bytes,
            "disk_high_water_bytes": disk_high_water,
            "largest_package_bytes": largest_package,
            "baseline_private_bytes": baseline.private_bytes,
            "final_private_bytes": final_resources.private_bytes,
            "baseline_handles": baseline.handles,
            "final_handles": final_resources.handles,
            "baseline_threads": baseline.threads,
            "final_threads": final_resources.threads,
            "baseline_user_objects": baseline.user_objects,
            "final_user_objects": final_resources.user_objects,
            "baseline_gdi_objects": baseline.gdi_objects,
            "final_gdi_objects": final_resources.gdi_objects,
            "encrypted_compact_private_high_water_bytes": encryption_resources.peak.private_bytes,
            "encrypted_compact_private_return_bytes": final_resources.private_bytes,
            "result": "pass",
        })
    );
}

#[cfg(not(windows))]
#[test]
fn recovery_resource_contract_is_windows_only() {
    println!("recovery_resource_contract: skipped (Windows-only release gate)");
}
