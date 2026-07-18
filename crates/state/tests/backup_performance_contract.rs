use std::fs;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{BackupDirectory, MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory};
use tokenmaster_state::{
    BackupCompression, BackupMetadata, BackupPackage, BackupPurpose, MaintenanceAdmission,
    MaintenanceCoordinator, MaintenanceExecution, MaintenancePurpose, MaintenanceSchedule,
    MaintenanceSourceState, MaintenanceTick, PACKAGE_DECODER_WINDOW_BYTES, PACKAGE_IO_BUFFER_BYTES,
    PortableSettingsCandidate, SettingsValue,
};
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, UsageStore, create_compact_snapshot,
    create_online_snapshot, verify_backup_candidate,
};

#[cfg(windows)]
#[path = "support/resource.rs"]
mod resource_support;

const SMALL_FIXTURE_MIB: u64 = 8;
const LARGE_FIXTURE_MIB: u64 = 96;
const MIB: u64 = 1024 * 1024;
const MAX_SCENARIO_DURATION: Duration = Duration::from_secs(120);
const MAX_STREAMING_PRIVATE_GROWTH: usize = 64 * 1024 * 1024;
const MIN_DATABASE_HEADROOM: u64 = 16 * MIB;
const FINAL_PRIVATE_TOLERANCE: usize = 16 * 1024 * 1024;

struct Fixture {
    _root: TempDir,
    source: BackupSource,
    staging: BackupStaging,
    backups: BackupDirectory,
    archive_bytes: u64,
    fixture_sha256: [u8; 32],
}

#[derive(Debug)]
struct ScenarioMeasurement {
    fixture_mib: u64,
    compression: BackupCompression,
    archive_bytes: u64,
    package_bytes: u64,
    elapsed_ms: f64,
    throughput_mib_s: f64,
    private_growth: Option<usize>,
    private_peak: Option<usize>,
    final_private: Option<usize>,
    baseline_threads: Option<u32>,
    peak_threads: Option<u32>,
}

fn fixture(target_mib: u64) -> Fixture {
    let root = TempDir::new().expect("performance fixture root");
    let archive = root.path().join("tokenmaster.sqlite3");
    drop(UsageStore::open(&archive).expect("create current archive"));

    let connection = Connection::open(&archive).expect("open performance fixture");
    connection
        .execute_batch(
            "PRAGMA wal_autocheckpoint=0;
             UPDATE git_installation_state SET installation_salt=zeroblob(32)
             WHERE singleton_id=1;
             CREATE TABLE tm_backup_perf_filler(
                 id INTEGER PRIMARY KEY,
                 payload BLOB NOT NULL
             ) STRICT;",
        )
        .expect("create bounded fixture filler");
    for id in 0..target_mib {
        connection
            .execute(
                "INSERT INTO tm_backup_perf_filler(id, payload) VALUES (?1, zeroblob(1048576))",
                [i64::try_from(id).expect("fixture id")],
            )
            .expect("append one MiB fixture page group");
    }
    connection
        .execute_batch(
            "DROP TABLE tm_backup_perf_filler;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .expect("freeze deterministic freelist fixture");
    drop(connection);

    let archive_bytes = fs::metadata(&archive).expect("fixture metadata").len();
    assert!(
        archive_bytes >= target_mib * MIB,
        "fixture did not retain the requested deterministic page scale: target_mib={target_mib}, archive_bytes={archive_bytes}"
    );
    let fixture_sha256 = hash_file(&archive);
    let staging_path = root.path().join("verification");
    let reliable_path = root.path().join("reliable");
    fs::create_dir(&staging_path).expect("create verification staging");
    fs::create_dir(&reliable_path).expect("create reliable root");
    let data_root = ValidatedLocalDirectory::new(root.path()).expect("validated data root");
    let staging_root =
        ValidatedLocalDirectory::new(&staging_path).expect("validated verification staging");
    let reliable_root =
        ValidatedLocalDirectory::new(&reliable_path).expect("validated reliable root");

    Fixture {
        _root: root,
        source: BackupSource::new(&data_root).expect("backup source"),
        staging: BackupStaging::new(&staging_root).expect("backup staging"),
        backups: BackupDirectory::open_or_create(&reliable_root).expect("backup directory"),
        archive_bytes,
        fixture_sha256,
    }
}

fn hash_file(path: &std::path::Path) -> [u8; 32] {
    use std::io::Read as _;

    let mut file = fs::File::open(path).expect("open fixture for hash");
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).expect("read fixture for hash");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    hasher.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn run_scenario(
    fixture: &Fixture,
    fixture_mib: u64,
    compression: BackupCompression,
    handle_return_tolerance: u32,
) -> ScenarioMeasurement {
    let control = BackupControl::new(Arc::new(AtomicBool::new(false)), MAX_SCENARIO_DURATION)
        .expect("backup control");
    #[cfg(windows)]
    let monitor = resource_support::ResourceMonitor::start();
    let started = Instant::now();
    let snapshot = verify_backup_candidate(
        create_online_snapshot(&fixture.source, &fixture.staging, &control)
            .expect("online snapshot"),
        &control,
    )
    .expect("verified online snapshot");
    let compact = if compression == BackupCompression::Compact {
        Some(
            create_compact_snapshot(&snapshot, &fixture.staging, &control)
                .expect("verified compact snapshot"),
        )
    } else {
        None
    };
    let selected = compact.as_ref().unwrap_or(&snapshot);
    let expanded_bytes = selected.len();
    let reader = selected.open_reader(&control).expect("candidate reader");
    let mut stage = fixture
        .backups
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("package stage");
    let settings =
        PortableSettingsCandidate::new(SettingsValue::safe_defaults().portable().clone())
            .expect("portable settings");
    BackupPackage::write_verified_candidate_to_backup_stage(
        &settings,
        reader,
        compression,
        BackupMetadata::new(
            1_735_689_600_000 + i64::from(compression as u8),
            if compression == BackupCompression::Automatic {
                BackupPurpose::Periodic
            } else {
                BackupPurpose::Manual
            },
        )
        .expect("backup metadata"),
        &mut stage,
    )
    .expect("stream typed backup package");
    let verified = BackupPackage::verify_backup_stage(&stage).expect("verify package stage");
    assert_eq!(verified.database_len(), expanded_bytes);
    assert_eq!(verified.compression(), compression);
    let package_bytes = verified.receipt().package_len();
    black_box(&verified);
    stage.discard().expect("discard measured package stage");
    drop(verified);
    drop(compact);
    drop(snapshot);
    let elapsed = started.elapsed();
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let throughput_mib_s = (expanded_bytes as f64 / MIB as f64) / elapsed.as_secs_f64();
    assert!(
        elapsed <= MAX_SCENARIO_DURATION,
        "backup scenario exceeded its bounded deadline"
    );

    #[cfg(windows)]
    let resources = monitor.finish();
    #[cfg(windows)]
    let final_resources = resource_support::settle_to(
        resources.baseline,
        FINAL_PRIVATE_TOLERANCE,
        handle_return_tolerance,
        Duration::from_secs(10),
    );
    #[cfg(windows)]
    assert!(
        resources.peak.threads <= resources.baseline.threads.saturating_add(1),
        "compression created a thread beyond the one measurement sampler: resources={resources:?}"
    );
    #[cfg(windows)]
    assert_eq!(
        resources.peak.child_processes, 0,
        "backup pipeline must not create a child process"
    );

    ScenarioMeasurement {
        fixture_mib,
        compression,
        archive_bytes: fixture.archive_bytes,
        package_bytes,
        elapsed_ms,
        throughput_mib_s,
        #[cfg(windows)]
        private_growth: Some(resources.private_growth()),
        #[cfg(not(windows))]
        private_growth: None,
        #[cfg(windows)]
        private_peak: Some(resources.peak.private_bytes),
        #[cfg(not(windows))]
        private_peak: None,
        #[cfg(windows)]
        final_private: Some(final_resources.private_bytes),
        #[cfg(not(windows))]
        final_private: None,
        #[cfg(windows)]
        baseline_threads: Some(resources.baseline.threads),
        #[cfg(not(windows))]
        baseline_threads: None,
        #[cfg(windows)]
        peak_threads: Some(resources.peak.threads),
        #[cfg(not(windows))]
        peak_threads: None,
    }
}

fn throughput_floor(compression: BackupCompression) -> f64 {
    match compression {
        BackupCompression::Automatic => 1.0,
        BackupCompression::Normal => 0.5,
        BackupCompression::Compact => 0.25,
    }
}

#[test]
#[ignore = "P3-D.0 release-mode developer evidence"]
fn deterministic_backup_profiles_are_streaming_bounded_and_single_threaded() {
    assert_eq!(PACKAGE_IO_BUFFER_BYTES, 64 * 1024);
    assert_eq!(PACKAGE_DECODER_WINDOW_BYTES, 8 * MIB);
    let small = fixture(SMALL_FIXTURE_MIB);
    let large = fixture(LARGE_FIXTURE_MIB);
    let mut measurements = Vec::new();

    // SQLite, Zstd, and the Windows test process initialize their one-time runtime
    // state here. Every recorded scenario below starts from this post-warm-up envelope.
    black_box(run_scenario(
        &small,
        SMALL_FIXTURE_MIB,
        BackupCompression::Automatic,
        4,
    ));

    for compression in [
        BackupCompression::Automatic,
        BackupCompression::Normal,
        BackupCompression::Compact,
    ] {
        let small_measurement = run_scenario(&small, SMALL_FIXTURE_MIB, compression, 1);
        let large_measurement = run_scenario(&large, LARGE_FIXTURE_MIB, compression, 1);
        for measurement in [&small_measurement, &large_measurement] {
            assert!(
                measurement.throughput_mib_s >= throughput_floor(compression),
                "backup throughput below conservative release floor: measurement={measurement:?}"
            );
        }
        for measurement in [&small_measurement, &large_measurement] {
            if let Some(private_growth) = measurement.private_growth {
                assert!(
                    private_growth <= MAX_STREAMING_PRIVATE_GROWTH,
                    "backup exceeded the fixed streaming private-memory envelope: measurement={measurement:?}"
                );
            }
        }
        if let Some(large_growth) = large_measurement.private_growth {
            assert!(
                u64::try_from(large_growth)
                    .expect("private-memory growth fits u64")
                    .saturating_add(MIN_DATABASE_HEADROOM)
                    < large_measurement.archive_bytes,
                "large fixture did not retain a database-sized-allocation safety margin: measurement={large_measurement:?}"
            );
        }
        measurements.push(small_measurement);
        measurements.push(large_measurement);
    }

    let receipt = json!({
        "schema": "tokenmaster.p3d0.backup-performance.v1",
        "fixture_small": {
            "kind": "schema13-freelist-v1",
            "target_mib": SMALL_FIXTURE_MIB,
            "archive_bytes": small.archive_bytes,
            "sha256": hex(&small.fixture_sha256),
        },
        "fixture_large": {
            "kind": "schema13-freelist-v1",
            "target_mib": LARGE_FIXTURE_MIB,
            "archive_bytes": large.archive_bytes,
            "sha256": hex(&large.fixture_sha256),
        },
        "decoder_window_bytes": PACKAGE_DECODER_WINDOW_BYTES,
        "io_buffer_bytes": PACKAGE_IO_BUFFER_BYTES,
        "maximum_streaming_private_growth_bytes": MAX_STREAMING_PRIVATE_GROWTH,
        "minimum_database_headroom_bytes": MIN_DATABASE_HEADROOM,
        "measurements": measurements.iter().map(|item| json!({
            "fixture_mib": item.fixture_mib,
            "compression": format!("{:?}", item.compression).to_ascii_lowercase(),
            "archive_bytes": item.archive_bytes,
            "package_bytes": item.package_bytes,
            "elapsed_ms": item.elapsed_ms,
            "throughput_mib_s": item.throughput_mib_s,
            "private_growth_bytes": item.private_growth,
            "private_peak_bytes": item.private_peak,
            "final_private_bytes": item.final_private,
            "baseline_threads": item.baseline_threads,
            "peak_threads": item.peak_threads,
            "gate": "pass",
        })).collect::<Vec<_>>(),
        "result": "pass",
    });
    println!("P3D0_BACKUP_PERFORMANCE={receipt}");
}

#[test]
#[ignore = "P3-D.0 release-mode developer evidence"]
fn ten_thousand_triggers_and_resume_keep_one_follow_up_without_a_burst() {
    let mut coordinator = MaintenanceCoordinator::new(MaintenanceSourceState::Healthy, true);
    let active = match coordinator.submit(MaintenancePurpose::Periodic) {
        MaintenanceAdmission::Started(permit) => permit,
        other => panic!("first periodic request must start: {other:?}"),
    };
    for _ in 0..10_000 {
        assert!(matches!(
            coordinator.submit(MaintenancePurpose::Periodic),
            MaintenanceAdmission::Coalesced { .. }
        ));
    }
    assert_eq!(coordinator.snapshot().active_count(), 1);
    assert_eq!(coordinator.snapshot().pending_count(), 1);
    active.begin_publication().expect("publication boundary");
    let transition = coordinator
        .finish(active.id(), MaintenanceExecution::Published { bytes: 1 })
        .expect("finish active request");
    assert!(transition.follow_up().is_some());

    let policy = SettingsValue::safe_defaults().portable().backup().clone();
    let mut schedule = MaintenanceSchedule::new(
        &policy,
        MaintenanceTick::from_millis(0),
        MaintenanceSourceState::HealthyUnpublished,
    );
    schedule.mark_healthy_publication(MaintenanceTick::from_millis(0));
    schedule.pause(MaintenanceTick::from_millis(1_000));
    schedule.resume(MaintenanceTick::from_millis(21_700_000));
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(21_700_000)),
        Some(MaintenancePurpose::Periodic)
    );
    assert_eq!(
        schedule.poll(MaintenanceTick::from_millis(21_700_000)),
        None
    );
    println!(
        "P3D0_COALESCING={}",
        json!({
            "schema": "tokenmaster.p3d0.coalescing.v1",
            "triggers": 10_000,
            "active": 1,
            "follow_up": 1,
            "resume_catch_up": 1,
            "resume_burst": 0,
            "result": "pass",
        })
    );
}
