use std::fs;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};
use rusqlite::Connection;
use serde_json::json;
use slint::ComponentHandle;
use tempfile::TempDir;
use tokenmaster_desktop::{
    DesktopController, DesktopQueryPlan, DesktopRefreshAdmission, DesktopRefreshOutcome,
    DesktopRefreshUrgency, DesktopShell,
};
use tokenmaster_platform::{BackupDirectory, MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory};
use tokenmaster_state::{
    BackupCompression, BackupMetadata, BackupPackage, BackupPurpose, PortableSettingsCandidate,
    SettingsValue,
};
use tokenmaster_store::{
    BackupControl, BackupSource, BackupStaging, UsageStore, create_online_snapshot,
    verify_backup_candidate,
};

const SAMPLES: usize = 40;
const WARMUP_SAMPLES: usize = 8;
const MAX_BACKUP_LATENCY_DELTA_MS: f64 = 10.0;

struct LatencyFixture {
    _root: TempDir,
    archive: std::path::PathBuf,
    source: BackupSource,
    staging: BackupStaging,
    backups: BackupDirectory,
}

impl LatencyFixture {
    fn new() -> Self {
        let root = TempDir::new().expect("latency fixture root");
        let archive = root.path().join("tokenmaster.sqlite3");
        drop(UsageStore::open(&archive).expect("create current archive"));
        let connection = Connection::open(&archive).expect("open latency fixture");
        connection
            .execute_batch(
                "PRAGMA wal_autocheckpoint=0;
                 CREATE TABLE tm_latency_filler(
                     id INTEGER PRIMARY KEY,
                     payload BLOB NOT NULL
                 ) STRICT;",
            )
            .expect("create latency filler");
        for id in 0..96_i64 {
            connection
                .execute(
                    "INSERT INTO tm_latency_filler(id, payload) VALUES (?1, zeroblob(1048576))",
                    [id],
                )
                .expect("append latency fixture page group");
        }
        connection
            .execute_batch(
                "DROP TABLE tm_latency_filler;
                 PRAGMA wal_checkpoint(TRUNCATE);",
            )
            .expect("freeze latency fixture");
        drop(connection);

        let staging_path = root.path().join("verification");
        let reliable_path = root.path().join("reliable");
        fs::create_dir(&staging_path).expect("create verification staging");
        fs::create_dir(&reliable_path).expect("create reliable root");
        let data_root = ValidatedLocalDirectory::new(root.path()).expect("validated data root");
        let staging_root =
            ValidatedLocalDirectory::new(&staging_path).expect("validated verification staging");
        let reliable_root =
            ValidatedLocalDirectory::new(&reliable_path).expect("validated reliable root");

        Self {
            _root: root,
            archive,
            source: BackupSource::new(&data_root).expect("backup source"),
            staging: BackupStaging::new(&staging_root).expect("backup staging"),
            backups: BackupDirectory::open_or_create(&reliable_root).expect("backup directory"),
        }
    }

    fn start_automatic_backup_load(self) -> AutomaticBackupLoad {
        let root = self._root;
        let stop = Arc::new(AtomicBool::new(false));
        let started_cycles = Arc::new(AtomicUsize::new(0));
        let completed_cycles = Arc::new(AtomicUsize::new(0));
        let worker_stop = Arc::clone(&stop);
        let worker_started_cycles = Arc::clone(&started_cycles);
        let worker_completed_cycles = Arc::clone(&completed_cycles);
        let source = self.source;
        let staging = self.staging;
        let backups = self.backups;
        let worker = thread::Builder::new()
            .name("p3d0-automatic-backup-load".to_owned())
            .spawn(move || -> Result<(), &'static str> {
                let settings = PortableSettingsCandidate::new(
                    SettingsValue::safe_defaults().portable().clone(),
                )
                .map_err(|_| "settings")?;
                while !worker_stop.load(Ordering::Acquire) {
                    worker_started_cycles.fetch_add(1, Ordering::AcqRel);
                    let control = BackupControl::new(
                        Arc::new(AtomicBool::new(false)),
                        Duration::from_secs(30),
                    )
                    .map_err(|_| "control")?;
                    let candidate = create_online_snapshot(&source, &staging, &control)
                        .and_then(|candidate| verify_backup_candidate(candidate, &control))
                        .map_err(|_| "snapshot")?;
                    let reader = candidate.open_reader(&control).map_err(|_| "reader")?;
                    let mut stage = backups
                        .create_staged(MAX_DURABLE_FILE_BYTES)
                        .map_err(|_| "stage")?;
                    BackupPackage::write_verified_candidate_to_backup_stage(
                        &settings,
                        reader,
                        BackupCompression::Automatic,
                        BackupMetadata::new(1_735_689_600_000, BackupPurpose::Periodic)
                            .map_err(|_| "metadata")?,
                        &mut stage,
                    )
                    .map_err(|_| "package")?;
                    BackupPackage::verify_backup_stage(&stage).map_err(|_| "verify")?;
                    stage.discard().map_err(|_| "discard")?;
                    worker_completed_cycles.fetch_add(1, Ordering::AcqRel);
                }
                Ok(())
            })
            .expect("spawn automatic backup load");
        AutomaticBackupLoad {
            _root: root,
            stop,
            started_cycles,
            completed_cycles,
            worker: Some(worker),
        }
    }
}

struct AutomaticBackupLoad {
    _root: TempDir,
    stop: Arc<AtomicBool>,
    started_cycles: Arc<AtomicUsize>,
    completed_cycles: Arc<AtomicUsize>,
    worker: Option<JoinHandle<Result<(), &'static str>>>,
}

impl AutomaticBackupLoad {
    fn wait_for_in_progress_cycle_after(&self, completed_before: usize) -> usize {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let completed = self.completed_cycles.load(Ordering::Acquire);
            let started = self.started_cycles.load(Ordering::Acquire);
            if completed >= completed_before && started > completed {
                return started;
            }
            assert!(
                Instant::now() < deadline,
                "automatic backup did not enter the required in-progress cycle"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn assert_cycle_in_progress(&self, cycle: usize) {
        assert!(
            self.started_cycles.load(Ordering::Acquire) >= cycle
                && self.completed_cycles.load(Ordering::Acquire) < cycle,
            "automatic backup cycle {cycle} did not span the complete latency sample window"
        );
    }

    fn finish(mut self) -> usize {
        self.stop.store(true, Ordering::Release);
        let result = self
            .worker
            .take()
            .expect("automatic backup worker handle")
            .join()
            .expect("automatic backup worker joins");
        result.expect("automatic backup workload stays healthy");
        self.completed_cycles.load(Ordering::Acquire)
    }
}

impl Drop for AutomaticBackupLoad {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn refresh_once(controller: &DesktopController) -> Duration {
    let started = Instant::now();
    let attempt = match controller
        .refresh(DesktopRefreshUrgency::Interactive)
        .expect("submit Dashboard query")
    {
        DesktopRefreshAdmission::Started { attempt } => attempt,
        other => panic!("sequential Dashboard query must start: {other:?}"),
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(completion) = controller
            .try_completion()
            .expect("read Dashboard query completion")
        {
            assert_eq!(completion.attempt(), attempt);
            assert_eq!(completion.outcome(), DesktopRefreshOutcome::Completed);
            return started.elapsed();
        }
        assert!(Instant::now() < deadline, "Dashboard query timeout");
        thread::yield_now();
    }
}

fn measure_query_p95(controller: &DesktopController) -> f64 {
    for _ in 0..WARMUP_SAMPLES {
        black_box(refresh_once(controller));
    }
    let samples = (0..SAMPLES)
        .map(|_| refresh_once(controller).as_secs_f64() * 1000.0)
        .collect::<Vec<_>>();
    percentile_95(samples)
}

fn measure_input_to_paint_p95(shell: &DesktopShell) -> f64 {
    for sample in 0..WARMUP_SAMPLES {
        black_box(input_to_paint(shell, sample));
    }
    let samples = (0..SAMPLES)
        .map(|sample| input_to_paint(shell, sample).as_secs_f64() * 1000.0)
        .collect::<Vec<_>>();
    percentile_95(samples)
}

fn input_to_paint(shell: &DesktopShell, sample: usize) -> Duration {
    let route = if sample.is_multiple_of(2) {
        "settings"
    } else {
        "dashboard"
    };
    let started = Instant::now();
    shell.window().invoke_select_route(route.into());
    let pixels = shell
        .window()
        .window()
        .take_snapshot()
        .expect("software-rendered headless paint");
    assert!(pixels.width() > 0 && pixels.height() > 0);
    black_box(pixels.as_bytes());
    started.elapsed()
}

fn percentile_95(mut samples: Vec<f64>) -> f64 {
    assert_eq!(samples.len(), SAMPLES);
    samples.sort_by(f64::total_cmp);
    let index = (samples.len() * 95).div_ceil(100).saturating_sub(1);
    samples[index]
}

#[test]
fn automatic_backup_adds_at_most_ten_ms_to_dashboard_query_and_real_paint_p95() {
    if cfg!(debug_assertions) {
        println!("backup_ui_latency_contract: skipped (release-only measurement)");
        return;
    }

    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
        mock_time: false,
        threading: true,
        renderer_name: Some("software".into()),
    })))
    .expect("install software-rendered testing backend");

    let fixture = LatencyFixture::new();
    let mut controller = DesktopController::open(
        &fixture.archive,
        DesktopQueryPlan::overview().expect("Dashboard overview plan"),
    )
    .expect("Dashboard controller");
    let baseline_query_p95_ms = measure_query_p95(&controller);
    let current = controller
        .take_snapshot()
        .expect("take current Dashboard snapshot")
        .expect("Dashboard snapshot exists");
    let shell = DesktopShell::new(&current).expect("desktop shell");
    shell
        .window()
        .window()
        .set_size(slint::PhysicalSize::new(1120, 720));
    shell.window().show().expect("show headless window");
    let baseline_paint_p95_ms = measure_input_to_paint_p95(&shell);

    let load = fixture.start_automatic_backup_load();
    let overlap_cycle = load.wait_for_in_progress_cycle_after(1);
    load.assert_cycle_in_progress(overlap_cycle);
    let backup_query_p95_ms = measure_query_p95(&controller);
    load.assert_cycle_in_progress(overlap_cycle);
    let backup_paint_p95_ms = measure_input_to_paint_p95(&shell);
    load.assert_cycle_in_progress(overlap_cycle);
    let backup_cycles = load.finish();
    assert!(
        backup_cycles >= overlap_cycle,
        "the measured automatic backup cycle did not complete during joined shutdown"
    );

    let query_delta_ms = (backup_query_p95_ms - baseline_query_p95_ms).max(0.0);
    let paint_delta_ms = (backup_paint_p95_ms - baseline_paint_p95_ms).max(0.0);
    assert!(
        query_delta_ms <= MAX_BACKUP_LATENCY_DELTA_MS,
        "automatic backup added too much cached Dashboard query latency: baseline={baseline_query_p95_ms:.3}ms, backup={backup_query_p95_ms:.3}ms, delta={query_delta_ms:.3}ms"
    );
    assert!(
        paint_delta_ms <= MAX_BACKUP_LATENCY_DELTA_MS,
        "automatic backup added too much input-to-software-paint latency: baseline={baseline_paint_p95_ms:.3}ms, backup={backup_paint_p95_ms:.3}ms, delta={paint_delta_ms:.3}ms"
    );

    shell.window().hide().expect("hide headless window");
    drop(shell);
    controller.shutdown().expect("Dashboard controller joins");
    println!(
        "P3D0_UI_LATENCY={}",
        json!({
            "schema": "tokenmaster.p3d0.ui-latency.v1",
            "samples": SAMPLES,
            "warmup_samples": WARMUP_SAMPLES,
            "renderer": "software-headless",
            "baseline_query_p95_ms": baseline_query_p95_ms,
            "backup_query_p95_ms": backup_query_p95_ms,
            "query_delta_ms": query_delta_ms,
            "baseline_input_to_paint_p95_ms": baseline_paint_p95_ms,
            "backup_input_to_paint_p95_ms": backup_paint_p95_ms,
            "input_to_paint_delta_ms": paint_delta_ms,
            "automatic_backup_cycles": backup_cycles,
            "overlap_cycle": overlap_cycle,
            "maximum_delta_ms": MAX_BACKUP_LATENCY_DELTA_MS,
            "result": "pass",
        })
    );
}
