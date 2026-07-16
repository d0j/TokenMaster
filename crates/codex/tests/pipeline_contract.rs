use std::collections::BTreeSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use tempfile::TempDir;
use tokenmaster_accounting::Canonicalizer;
use tokenmaster_codex::{
    CodexProvider, CodexRootInput, ConfiguredCodexRoot, ReaderOutcome, RebuildReason, SinkDecision,
    build_discovery_request, enumerate_profile_sources, read_source_batch,
};
use tokenmaster_provider::DiscoveryProvider;

#[path = "support/pipeline.rs"]
mod pipeline;

use pipeline::{PipelineError, PipelineOptions, probe_current_rebuild_reason, run_pipeline};
use tokenmaster_store::UsageStore;

const PRIVACY_SENTINEL: &str = "PIPELINE_PRIVATE_SENTINEL_91A7";

fn write_baseline(root: &Path) {
    let parent = concat!(
        r#"{"timestamp":"2026-07-14T10:00:00Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12},"total_token_usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}}},"ignored_private":"PIPELINE_PRIVATE_SENTINEL_91A7"}"#,
        "\n",
    );
    let child = concat!(
        r#"{"type":"session_meta","payload":{"id":"child","forked_from_id":"parent"}}"#,
        "\n",
        r#"{"timestamp":"2026-07-14T10:00:01Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12},"total_token_usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}}}}"#,
        "\n",
        r#"{"timestamp":"2026-07-14T10:00:02Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":11,"output_tokens":3,"total_tokens":14},"total_token_usage":{"input_tokens":111,"output_tokens":23,"total_tokens":134}}}}"#,
        "\n",
    );
    fs::write(root.join("parent.jsonl"), parent).expect("write parent fixture");
    fs::write(root.join("child.jsonl"), child).expect("write child fixture");
}

fn expected_visible_ids(root: &Path) -> BTreeSet<String> {
    let configured = [ConfiguredCodexRoot::new(
        root,
        Some("Fixture".to_owned()),
        true,
    )];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("build discovery request");
    let provider = CodexProvider::new().expect("Codex provider");
    let snapshot = provider.discover(&request).expect("discover fixture");
    let canonicalizer = Canonicalizer::new();
    let mut visible = BTreeSet::new();
    enumerate_profile_sources(
        snapshot.sources(),
        || false,
        |descriptor| {
            let mut checkpoint = None;
            loop {
                let batch = match read_source_batch(&descriptor, checkpoint.as_ref(), || false)
                    .expect("read oracle fixture")
                {
                    ReaderOutcome::Batch(batch) => batch,
                    other => panic!("oracle batch expected, got {other:?}"),
                };
                for draft in batch.events() {
                    if (draft.session_id().as_str() == "parent" && draft.session_ordinal() == 0)
                        || (draft.session_id().as_str() == "child" && draft.session_ordinal() == 1)
                    {
                        visible.insert(
                            canonicalizer
                                .canonicalize(draft)
                                .expect("canonical oracle event")
                                .id()
                                .as_str()
                                .to_owned(),
                        );
                    }
                }
                let reached_end = batch.reached_snapshot_end();
                checkpoint = Some(batch.checkpoint().clone());
                if reached_end {
                    break;
                }
            }
            SinkDecision::Continue
        },
    )
    .expect("enumerate oracle fixture");
    visible
}

#[test]
fn baseline_real_jsonl_is_atomic_replay_safe_and_private() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("codex-private-root");
    fs::create_dir(&root).expect("create Codex fixture root");
    write_baseline(&root);
    let expected_ids = expected_visible_ids(&root);
    let database = directory.path().join("pipeline.sqlite3");

    let result = run_pipeline(&root, &database, PipelineOptions::default())
        .expect("baseline pipeline must promote");

    assert_eq!(result.registered_files, 2);
    assert_eq!(result.visible_before_promotion, 0);
    assert_eq!(result.visible_events, 2);
    assert_eq!(result.visible_total_tokens, 26);
    assert_eq!(result.visible_event_ids, expected_ids);
    assert_eq!(result.quality.eligible(), 2);
    assert_eq!(result.quality.replay(), 1);
    assert_eq!(result.quality.pending(), 0);
    assert_eq!(result.quality.conflict(), 0);
    assert!(result.max_reader_batch <= 256);
    assert!(result.max_event_page <= 256);
    assert_eq!(result.restarts, 1);
    assert!(result.scan_bound);

    let debug = format!("{result:?}");
    assert!(!debug.contains(root.to_string_lossy().as_ref()));
    assert!(!debug.contains(PRIVACY_SENTINEL));
}

#[test]
fn source_batch_exposes_only_latest_transient_repository_hint() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("codex-root");
    let first = directory
        .path()
        .join("PRIVATE_PIPELINE_FIRST")
        .join("project-first");
    let second = directory
        .path()
        .join("PRIVATE_PIPELINE_SECOND")
        .join("project-second");
    fs::create_dir(&root).expect("Codex root");
    fs::create_dir_all(&first).expect("first repository");
    fs::create_dir_all(&second).expect("second repository");
    let source = root.join("session.jsonl");
    let content = [
        serde_json::json!({
            "timestamp": "2026-07-10T08:00:00Z",
            "type": "session_meta",
            "payload": {"id": "session-first", "cwd": first}
        })
        .to_string(),
        serde_json::json!({
            "timestamp": "2026-07-10T08:01:00Z",
            "type": "turn_context",
            "payload": {"session_id": "session-second", "cwd": second}
        })
        .to_string(),
        serde_json::json!({
            "timestamp": "2026-07-10T08:02:00Z",
            "model": "gpt-test",
            "usage": {"total_tokens": 1}
        })
        .to_string(),
    ]
    .join("\n");
    fs::write(&source, format!("{content}\n")).expect("source");

    let configured = [ConfiguredCodexRoot::new(&root, None, true)];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("request");
    let snapshot = CodexProvider::new()
        .expect("provider")
        .discover(&request)
        .expect("discovery");
    let mut observed = None;
    enumerate_profile_sources(
        snapshot.sources(),
        || false,
        |descriptor| {
            let ReaderOutcome::Batch(mut batch) =
                read_source_batch(&descriptor, None, || false).expect("read")
            else {
                panic!("batch expected");
            };
            let checkpoint =
                serde_json::to_string(batch.checkpoint().resume()).expect("resume serializes");
            let debug = format!("{batch:?}");
            let hint = batch
                .take_latest_repository_activity_hint()
                .expect("latest transient hint");
            assert_eq!(hint.session_id().as_str(), "session-second");
            assert_eq!(hint.candidate().as_path(), second.canonicalize().unwrap());
            assert!(batch.take_latest_repository_activity_hint().is_none());
            for marker in ["PRIVATE_PIPELINE_FIRST", "PRIVATE_PIPELINE_SECOND"] {
                assert!(!checkpoint.contains(marker));
                assert!(!debug.contains(marker));
            }
            observed = Some(hint);
            SinkDecision::Continue
        },
    )
    .expect("enumeration");
    assert!(observed.is_some());

    let database = directory.path().join("repository-hint.sqlite3");
    let result = run_pipeline(&root, &database, PipelineOptions::default())
        .expect("repository metadata pipeline");
    assert_eq!(result.visible_events, 1);
    for archive_file in [
        database.clone(),
        database.with_extension("sqlite3-wal"),
        database.with_extension("sqlite3-shm"),
    ] {
        let Ok(bytes) = fs::read(archive_file) else {
            continue;
        };
        for marker in ["PRIVATE_PIPELINE_FIRST", "PRIVATE_PIPELINE_SECOND"] {
            assert!(
                !bytes
                    .windows(marker.len())
                    .any(|window| window == marker.as_bytes()),
                "private path reached durable archive"
            );
        }
    }
}

fn write_usage_lines(path: &Path, count: u64) {
    let mut content = String::new();
    for index in 0..count {
        content.push_str(&format!(
            "{{\"timestamp\":{},\"model\":\"gpt-test\",\"usage\":{{\"total_tokens\":1}}}}\n",
            1_720_598_400_u64 + index
        ));
    }
    fs::write(path, content).expect("write usage fixture");
}

#[test]
fn restart_after_first_batch_resumes_more_than_256_events_without_duplicates() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("restart-root");
    fs::create_dir(&root).expect("create restart root");
    write_usage_lines(&root.join("restart.jsonl"), 300);
    let database = directory.path().join("restart.sqlite3");

    let result = run_pipeline(
        &root,
        &database,
        PipelineOptions {
            collect_event_ids: false,
            restart_after_batches: Some(1),
            ..PipelineOptions::default()
        },
    )
    .expect("restarted pipeline must promote");

    assert_eq!(result.registered_files, 1);
    assert_eq!(result.restarts, 2, "one injected restart plus final reopen");
    assert_eq!(result.visible_events, 300);
    assert_eq!(result.visible_total_tokens, 300);
    assert_eq!(result.quality.eligible(), 300);
    assert_eq!(result.quality.total(), 300);
    assert_eq!(result.max_reader_batch, 256);
    assert_eq!(result.max_event_page, 256);
    assert!(result.visible_event_ids.is_empty());
}

#[test]
fn more_than_256_files_use_the_disk_backed_manifest_and_bounded_pages() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("many-files-root");
    fs::create_dir(&root).expect("create many-files root");
    for index in 0..300_u32 {
        let line = format!(
            "{{\"timestamp\":{},\"model\":\"gpt-test\",\"usage\":{{\"total_tokens\":1}}}}\n",
            1_720_700_000_u64 + u64::from(index)
        );
        fs::write(root.join(format!("session-{index:03}.jsonl")), line)
            .expect("write many-files fixture");
    }
    let database = directory.path().join("many-files.sqlite3");

    let result = run_pipeline(
        &root,
        &database,
        PipelineOptions {
            collect_event_ids: false,
            ..PipelineOptions::default()
        },
    )
    .expect("large manifest pipeline must promote");

    assert_eq!(result.registered_files, 300);
    assert_eq!(result.visible_events, 300);
    assert_eq!(result.visible_total_tokens, 300);
    assert_eq!(result.quality.eligible(), 300);
    assert_eq!(result.quality.total(), 300);
    assert_eq!(result.max_reader_batch, 1);
    assert_eq!(result.max_event_page, 256);
}

#[cfg(windows)]
fn atomic_replace(replaced: &Path, replacement: &Path) {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Storage::FileSystem::{REPLACE_FILE_FLAGS, ReplaceFileW};
    use windows::core::PCWSTR;

    let replaced = replaced
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replacement = replacement
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both UTF-16 buffers are NUL-terminated and remain alive for the call;
    // the paths are same-directory temporary test files and no optional pointers exist.
    unsafe {
        ReplaceFileW(
            PCWSTR(replaced.as_ptr()),
            PCWSTR(replacement.as_ptr()),
            PCWSTR::null(),
            REPLACE_FILE_FLAGS::default(),
            None,
            None,
        )
    }
    .expect("atomically replace fixture");
}

#[test]
fn append_and_atomic_replacement_keep_old_projection_until_exact_promotion() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("replacement-root");
    fs::create_dir(&root).expect("create replacement root");
    write_baseline(&root);
    let database = directory.path().join("replacement.sqlite3");
    let baseline =
        run_pipeline(&root, &database, PipelineOptions::default()).expect("baseline must promote");
    assert_eq!(baseline.visible_events, 2);

    let child = root.join("child.jsonl");
    OpenOptions::new()
        .append(true)
        .open(&child)
        .expect("open append fixture")
        .write_all(
            concat!(
                r#"{"timestamp":"2026-07-14T10:00:03Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":12,"output_tokens":4,"total_tokens":16},"total_token_usage":{"input_tokens":123,"output_tokens":27,"total_tokens":150}}}}"#,
                "\n"
            )
            .as_bytes(),
        )
        .expect("append complete usage line");
    let appended = run_pipeline(&root, &database, PipelineOptions::default())
        .expect("append rebuild must promote");
    assert_eq!(appended.visible_before_promotion, 2);
    assert_eq!(appended.visible_events, 3);
    assert_eq!(appended.visible_total_tokens, 42);
    assert_eq!(appended.quality.eligible(), 3);
    assert_eq!(appended.quality.replay(), 1);

    let replacement = root.join("child.replacement");
    let mut replacement_content = fs::read_to_string(&child).expect("read current child");
    replacement_content.push_str(concat!(
        r#"{"timestamp":"2026-07-14T10:00:04Z","type":"event_msg","payload":{"type":"token_count","model":"gpt-test","info":{"last_token_usage":{"input_tokens":13,"output_tokens":5,"total_tokens":18},"total_token_usage":{"input_tokens":136,"output_tokens":32,"total_tokens":168}}}}"#,
        "\n"
    ));
    fs::write(&replacement, replacement_content).expect("write replacement file");
    atomic_replace(&child, &replacement);
    assert_eq!(
        probe_current_rebuild_reason(&root, &database, Path::new("child.jsonl"))
            .expect("probe replaced current source"),
        Some(RebuildReason::IdentityChanged)
    );

    let replaced = run_pipeline(&root, &database, PipelineOptions::default())
        .expect("physical replacement rebuild must promote");
    assert_eq!(replaced.visible_before_promotion, 3);
    assert_eq!(replaced.visible_events, 4);
    assert_eq!(replaced.visible_total_tokens, 60);
    assert_eq!(replaced.quality.eligible(), 4);
    assert_eq!(replaced.quality.replay(), 1);
}

fn visible_totals(database: &Path) -> (u64, u64, bool) {
    let store = UsageStore::open(database).expect("open archive summary");
    let mut before = None;
    let mut count = 0_u64;
    let mut total = 0_u64;
    loop {
        let page = store
            .event_page_before(before, 256)
            .expect("read archive summary page");
        if page.is_empty() {
            break;
        }
        for event in &page {
            count += 1;
            total += event.total_tokens().unwrap_or(0);
        }
        before = page.last().map(tokenmaster_store::StoredUsageEvent::cursor);
    }
    (
        count,
        total,
        store
            .archive_state()
            .expect("archive state")
            .rebuild_staging(),
    )
}

#[test]
fn truncation_is_classified_and_promotes_with_retained_prior_usage() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("truncate-root");
    fs::create_dir(&root).expect("create truncate root");
    write_baseline(&root);
    let database = directory.path().join("truncate.sqlite3");
    run_pipeline(&root, &database, PipelineOptions::default()).expect("baseline promotion");

    let child = root.join("child.jsonl");
    let original = fs::read_to_string(&child).expect("read child fixture");
    let truncated = original.lines().take(2).collect::<Vec<_>>().join("\n") + "\n";
    fs::write(&child, truncated).expect("truncate at complete-line boundary");
    assert_eq!(
        probe_current_rebuild_reason(&root, &database, Path::new("child.jsonl"))
            .expect("probe truncated current source"),
        Some(RebuildReason::Truncated)
    );

    let truncated_result = run_pipeline(&root, &database, PipelineOptions::default())
        .expect("complete truncation rebuild must retain prior accounted usage");
    assert_eq!(truncated_result.visible_before_promotion, 2);
    assert_eq!(truncated_result.visible_events, 2);
    assert_eq!(truncated_result.visible_total_tokens, 26);
    assert_eq!(truncated_result.quality.eligible(), 1);
    assert_eq!(truncated_result.quality.replay(), 1);
    assert_eq!(truncated_result.quality.pending(), 0);
    assert_eq!(truncated_result.quality.conflict(), 0);
    assert_eq!(visible_totals(&database), (2, 26, false));
}

#[test]
fn enumeration_and_reader_cancellation_leave_no_staging_projection() {
    let enumeration_directory = TempDir::new().expect("enumeration directory");
    let enumeration_root = enumeration_directory.path().join("enumeration-root");
    fs::create_dir(&enumeration_root).expect("create enumeration root");
    write_usage_lines(&enumeration_root.join("one.jsonl"), 1);
    write_usage_lines(&enumeration_root.join("two.jsonl"), 1);
    let enumeration_database = enumeration_directory.path().join("enumeration.sqlite3");
    let enumeration_error = run_pipeline(
        &enumeration_root,
        &enumeration_database,
        PipelineOptions {
            cancel_enumeration_after_files: Some(1),
            ..PipelineOptions::default()
        },
    )
    .expect_err("cancelled enumeration cannot begin replay");
    assert_eq!(enumeration_error, PipelineError::EnumerationIncomplete);
    assert_eq!(visible_totals(&enumeration_database), (0, 0, false));
    assert!(
        UsageStore::open(&enumeration_database)
            .expect("reopen cancelled scan archive")
            .running_scan_set()
            .expect("cancelled scan state")
            .is_none()
    );

    let reader_directory = TempDir::new().expect("reader directory");
    let reader_root = reader_directory.path().join("reader-root");
    fs::create_dir(&reader_root).expect("create reader root");
    write_baseline(&reader_root);
    let reader_database = reader_directory.path().join("reader.sqlite3");
    run_pipeline(&reader_root, &reader_database, PipelineOptions::default())
        .expect("reader cancellation baseline");
    write_usage_lines(&reader_root.join("bulk.jsonl"), 300);

    let reader_error = run_pipeline(
        &reader_root,
        &reader_database,
        PipelineOptions {
            cancel_reader_after_batches: Some(1),
            ..PipelineOptions::default()
        },
    )
    .expect_err("reader cancellation must discard staging");
    assert_eq!(reader_error, PipelineError::Cancelled);
    assert_eq!(visible_totals(&reader_database), (2, 26, false));
}

#[test]
fn malformed_and_incomplete_tails_discard_partial_rebuilds() {
    for (name, suffix, expected) in [
        (
            "malformed",
            "{\"usage\":not-valid-json}\n",
            PipelineError::MalformedInput,
        ),
        (
            "incomplete",
            "{\"timestamp\":1720599000,\"usage\":{\"total_tokens\":99}",
            PipelineError::IncompleteTail,
        ),
    ] {
        let directory = TempDir::new().expect("temporary directory");
        let root = directory.path().join(format!("{name}-root"));
        fs::create_dir(&root).expect("create failure root");
        write_baseline(&root);
        let database = directory.path().join(format!("{name}.sqlite3"));
        run_pipeline(&root, &database, PipelineOptions::default())
            .expect("failure fixture baseline");
        OpenOptions::new()
            .append(true)
            .open(root.join("child.jsonl"))
            .expect("open failure fixture")
            .write_all(suffix.as_bytes())
            .expect("append failure fixture");

        let error = run_pipeline(&root, &database, PipelineOptions::default())
            .expect_err("invalid source cannot promote");
        assert_eq!(error, expected, "{name}");
        assert_eq!(visible_totals(&database), (2, 26, false), "{name}");
    }
}
