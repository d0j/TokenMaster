use std::{
    fs::{self, OpenOptions},
    hint::black_box,
    io::Write,
    path::{Component, Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, bail};
use slint::{ModelRc, SharedString, VecModel};
use tokenmaster_domain::{AppState, RouteId};
use tokenmaster_gates::{
    GateStatus, M0Report, MemoryScenario, ReportKind, evaluate_cpu, evaluate_memory,
};
use tokenmaster_store::ProbeStore;

use crate::{
    MainWindow, SessionRow,
    args::{Args, StressKind},
    metrics::ProcessSample,
    seed_probe_models,
    shell::{RendererChoice, select_renderer},
    wire_skin_callbacks,
};

pub fn retained_mib(baseline_bytes: u64, final_bytes: u64) -> f64 {
    final_bytes.saturating_sub(baseline_bytes) as f64 / (1024.0 * 1024.0)
}

pub fn require_pass(status: GateStatus) -> anyhow::Result<()> {
    if status == GateStatus::Pass {
        Ok(())
    } else {
        bail!("stress gates did not pass: {status:?}")
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StressReceipt {
    schema_version: u32,
    kind: StressKind,
    renderer: &'static str,
    iterations: u32,
    rows: u64,
    stress_elapsed_ms: f64,
    idle_seconds: u64,
    retained_mib: f64,
    baseline: ProcessSample,
    idle_start: ProcessSample,
    final_sample: ProcessSample,
    summary: M0Report,
}

pub fn run(args: &Args, root: &Path) -> anyhow::Result<String> {
    let kind = args.stress.context("stress kind is required")?;
    select_renderer(RendererChoice::Software).context("select software renderer for stress")?;
    let window = MainWindow::new().context("create stress window")?;
    wire_skin_callbacks(&window);
    seed_probe_models(&window);

    let mut store = None;
    if args.rows > 0 {
        let mut seeded = ProbeStore::in_memory().context("create stress store")?;
        seeded
            .seed_sessions(args.rows)
            .context("seed stress sessions")?;
        let rows = seeded
            .page_before(None, 256)
            .context("read stress session page")?;
        let presentation = rows
            .into_iter()
            .map(|row| {
                Ok(SessionRow {
                    id: i32::try_from(row.id).context("session id conversion")?,
                    label: SharedString::from(format!("Session {}", row.id)),
                    tokens_label: SharedString::from(format!("{} tokens", row.total_tokens)),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        window.set_session_rows(ModelRc::new(VecModel::from(presentation)));
        store = Some(seeded);
    }

    exercise(kind, &window, 100);
    thread::sleep(Duration::from_millis(100));
    let baseline = ProcessSample::capture().context("capture stress baseline")?;
    let started = Instant::now();
    exercise(kind, &window, args.iterations);
    black_box(&store);
    let stress_elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let idle_start = ProcessSample::capture().context("capture idle start")?;
    thread::sleep(Duration::from_secs(args.duration_seconds));
    let final_sample = ProcessSample::capture().context("capture stress final")?;

    let retained = retained_mib(baseline.private_bytes, final_sample.private_bytes);
    let absolute_mib = final_sample.private_bytes as f64 / (1024.0 * 1024.0);
    let memory_scenario = if args.rows == 0 {
        MemoryScenario::Empty
    } else if args.rows <= 100_000 {
        MemoryScenario::HundredThousand
    } else {
        MemoryScenario::MillionRows
    };
    let retained_scenario = match kind {
        StressKind::Switches => MemoryScenario::TenThousandSwitches,
        StressKind::Routes => MemoryScenario::TenThousandRoutes,
    };
    let cpu_percent = idle_cpu_percent(&idle_start, &final_sample)?;
    let gates = vec![
        evaluate_memory(memory_scenario, absolute_mib),
        evaluate_memory(retained_scenario, retained),
        evaluate_cpu(cpu_percent),
    ];
    let summary = M0Report::new(
        ReportKind::DeveloperSmoke,
        RendererChoice::Software.backend_name(),
        args.rows,
        3,
        gates,
    );
    let receipt = StressReceipt {
        schema_version: 1,
        kind,
        renderer: RendererChoice::Software.backend_name(),
        iterations: args.iterations,
        rows: args.rows,
        stress_elapsed_ms,
        idle_seconds: args.duration_seconds,
        retained_mib: retained,
        baseline,
        idle_start,
        final_sample,
        summary,
    };
    let json = serde_json::to_string_pretty(&receipt).context("serialize stress receipt")?;
    if let Some(relative) = &args.report {
        fs::create_dir_all(root.join("reports")).context("create reports directory")?;
        let destination = validated_report_path(root, relative)?;
        write_new_atomic(&destination, json.as_bytes())?;
    }
    require_pass(receipt.summary.overall)?;
    Ok(json)
}

fn exercise(kind: StressKind, window: &MainWindow, iterations: u32) {
    match kind {
        StressKind::Switches => {
            for index in 0..iterations {
                window.invoke_switch_layout((index % 3) as i32);
                window.invoke_switch_theme((index % 3) as i32);
            }
        }
        StressKind::Routes => {
            let mut state = AppState::default();
            let routes = [RouteId::Dashboard, RouteId::Sessions, RouteId::Settings];
            for index in 0..iterations {
                state.navigate(routes[index as usize % routes.len()]);
            }
            black_box(state.revision());
        }
    }
}

fn idle_cpu_percent(start: &ProcessSample, end: &ProcessSample) -> anyhow::Result<f64> {
    let elapsed_ns = end.monotonic_ns.saturating_sub(start.monotonic_ns);
    if elapsed_ns == 0 {
        bail!("idle sample timestamps are not increasing")
    }
    let cpu_ticks = end
        .kernel_time_100ns
        .saturating_add(end.user_time_100ns)
        .saturating_sub(
            start
                .kernel_time_100ns
                .saturating_add(start.user_time_100ns),
        );
    let processors = thread::available_parallelism()
        .context("read processor count")?
        .get() as f64;
    Ok((cpu_ticks as f64 * 100.0 / elapsed_ns as f64) * 100.0 / processors)
}

fn write_new_atomic(destination: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if destination.exists() {
        bail!("report destination already exists")
    }
    let extension = format!("tmp-{}", std::process::id());
    let temporary = destination.with_extension(extension);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .context("create temporary report")?;
    file.write_all(bytes).context("write temporary report")?;
    file.sync_all().context("sync temporary report")?;
    drop(file);
    fs::rename(&temporary, destination).context("publish stress report")?;
    Ok(())
}

pub fn validated_report_path(root: &Path, relative: &Path) -> anyhow::Result<PathBuf> {
    if relative.is_absolute()
        || relative.extension().and_then(|value| value.to_str()) != Some("json")
    {
        bail!("report path must be a relative .json path beneath reports")
    }
    let mut components = relative.components();
    if components.next() != Some(Component::Normal("reports".as_ref()))
        || components.any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("report path must remain beneath reports")
    }

    let report_root = root
        .join("reports")
        .canonicalize()
        .context("canonicalize report directory")?;
    let candidate = root.join(relative);
    let parent = candidate
        .parent()
        .context("report path has no parent")?
        .canonicalize()
        .context("canonicalize report parent")?;
    if !parent.starts_with(&report_root) {
        bail!("report path escapes reports directory")
    }
    Ok(candidate)
}
