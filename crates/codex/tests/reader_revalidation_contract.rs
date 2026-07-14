use std::collections::BTreeMap;
use std::fs::{FileTimes, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use tempfile::TempDir;
use tokenmaster_codex::{
    IntegrityReport, ReadBatch, ReaderCheckpointParts, ReaderCheckpointV1, ReaderErrorCode,
    ReaderOutcome, RebuildReason, SOURCE_CHUNK_BYTES, SinkDecision, SourceChunkDigest,
    SourceFileDescriptor, VerificationLevel, enumerate_profile_sources, read_source_batch,
    verify_full_prefix,
};
use tokenmaster_provider::{ProfileId, SourceDescriptor, SourceId, SourceKind};

fn source(root: impl Into<PathBuf>, suffix: &str) -> SourceDescriptor {
    SourceDescriptor::new(
        SourceId::new(format!("source_{suffix}")).expect("valid source ID"),
        ProfileId::new("profile_revalidation").expect("valid profile ID"),
        SourceKind::Direct,
        root,
    )
    .expect("valid source descriptor")
}

fn only_descriptor(source: &SourceDescriptor) -> SourceFileDescriptor {
    let mut descriptor = None;
    enumerate_profile_sources(
        std::slice::from_ref(source),
        || false,
        |file| {
            assert!(descriptor.is_none(), "fixture must contain one file");
            descriptor = Some(file);
            SinkDecision::Continue
        },
    )
    .expect("fixture enumeration must pass");
    descriptor.expect("fixture must emit one file")
}

fn usage_line(input: u64) -> Vec<u8> {
    let mut line = format!(
        r#"{{"timestamp":"2026-07-14T14:00:00Z","model":"gpt-5.6-sol","usage":{{"input_tokens":{input},"output_tokens":2,"total_tokens":{}}}}}"#,
        input + 2
    )
    .into_bytes();
    line.push(b'\n');
    line
}

fn expect_batch(outcome: ReaderOutcome) -> ReadBatch {
    match outcome {
        ReaderOutcome::Batch(batch) => batch,
        other => panic!("batch expected, got {other:?}"),
    }
}

fn initial_checkpoint(descriptor: &SourceFileDescriptor) -> ReaderCheckpointV1 {
    expect_batch(read_source_batch(descriptor, None, || false).expect("initial read must pass"))
        .checkpoint()
        .clone()
}

fn assert_rebuild(outcome: ReaderOutcome, expected: RebuildReason) {
    match outcome {
        ReaderOutcome::RebuildRequired(reason) => assert_eq!(reason, expected),
        other => panic!("rebuild expected, got {other:?}"),
    }
}

fn write_fixture(path: &Path, input: u64) {
    std::fs::write(path, usage_line(input)).expect("write usage fixture");
}

#[cfg(windows)]
fn create_junction(target: &Path, junction: &Path) {
    let status = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "New-Item -ItemType Junction -Path $env:TOKENMASTER_TEST_JUNCTION -Target $env:TOKENMASTER_TEST_TARGET | Out-Null",
        ])
        .env("TOKENMASTER_TEST_JUNCTION", junction)
        .env("TOKENMASTER_TEST_TARGET", target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Windows PowerShell must be available for the junction security gate");
    assert!(status.success(), "junction fixture must be created");
}

#[test]
fn unchanged_append_truncate_replace_and_rewrite_are_classified() {
    let root = TempDir::new().expect("temporary directory");

    let unchanged_path = root.path().join("unchanged-private.jsonl");
    write_fixture(&unchanged_path, 10);
    let unchanged_descriptor = only_descriptor(&source(root.path(), "unchanged"));
    let unchanged_checkpoint = initial_checkpoint(&unchanged_descriptor);
    let unchanged = read_source_batch(&unchanged_descriptor, Some(&unchanged_checkpoint), || false)
        .expect("unchanged probe must pass");
    assert!(matches!(unchanged, ReaderOutcome::Unchanged(_)));

    let append_root = root.path().join("append");
    std::fs::create_dir(&append_root).expect("create append root");
    let append_path = append_root.join("append-private.jsonl");
    write_fixture(&append_path, 10);
    let append_descriptor = only_descriptor(&source(&append_root, "append"));
    let append_checkpoint = initial_checkpoint(&append_descriptor);
    assert!(!append_checkpoint.anchor().is_empty());
    OpenOptions::new()
        .append(true)
        .open(&append_path)
        .expect("open append fixture")
        .write_all(&usage_line(20))
        .expect("append usage fixture");
    let append = expect_batch(
        read_source_batch(&append_descriptor, Some(&append_checkpoint), || false)
            .expect("verified append must pass"),
    );
    assert_eq!(append.events().len(), 1);

    let truncate_root = root.path().join("truncate");
    std::fs::create_dir(&truncate_root).expect("create truncate root");
    let truncate_path = truncate_root.join("truncate-private.jsonl");
    write_fixture(&truncate_path, 10);
    let truncate_descriptor = only_descriptor(&source(&truncate_root, "truncate"));
    let truncate_checkpoint = initial_checkpoint(&truncate_descriptor);
    OpenOptions::new()
        .write(true)
        .open(&truncate_path)
        .expect("open truncate fixture")
        .set_len(0)
        .expect("truncate fixture");
    assert_rebuild(
        read_source_batch(&truncate_descriptor, Some(&truncate_checkpoint), || false)
            .expect("truncate classification must pass"),
        RebuildReason::Truncated,
    );

    let replace_root = root.path().join("replace");
    std::fs::create_dir(&replace_root).expect("create replacement root");
    let replace_path = replace_root.join("replace-private.jsonl");
    write_fixture(&replace_path, 10);
    let replace_descriptor = only_descriptor(&source(&replace_root, "replace"));
    let replace_checkpoint = initial_checkpoint(&replace_descriptor);
    std::fs::rename(&replace_path, replace_root.join("old.jsonl")).expect("rename old file");
    write_fixture(&replace_path, 10);
    assert_rebuild(
        read_source_batch(&replace_descriptor, Some(&replace_checkpoint), || false)
            .expect("replacement classification must pass"),
        RebuildReason::IdentityChanged,
    );

    let rewrite_root = root.path().join("rewrite");
    std::fs::create_dir(&rewrite_root).expect("create rewrite root");
    let rewrite_path = rewrite_root.join("rewrite-private.jsonl");
    write_fixture(&rewrite_path, 10);
    let rewrite_descriptor = only_descriptor(&source(&rewrite_root, "rewrite"));
    let rewrite_checkpoint = initial_checkpoint(&rewrite_descriptor);
    let rewritten = usage_line(20);
    assert_eq!(
        rewritten.len(),
        std::fs::metadata(&rewrite_path).expect("metadata").len() as usize
    );
    std::fs::write(&rewrite_path, rewritten).expect("rewrite same-size fixture");
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&rewrite_path)
        .expect("open rewritten fixture")
        .set_times(FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(5)))
        .expect("set deterministic modified time");
    assert_rebuild(
        read_source_batch(&rewrite_descriptor, Some(&rewrite_checkpoint), || false)
            .expect("rewrite classification must pass"),
        RebuildReason::RewriteDetected,
    );

    let debug = format!("{unchanged:?}");
    assert!(!debug.contains(root.path().to_string_lossy().as_ref()));
    assert!(!debug.contains("unchanged-private.jsonl"));
}

#[test]
fn append_is_rejected_when_boundary_anchor_changed() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("anchor-private.jsonl");
    write_fixture(&path, 10);
    let descriptor = only_descriptor(&source(root.path(), "anchor"));
    let checkpoint = initial_checkpoint(&descriptor);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open anchor fixture");
    file.seek(SeekFrom::Start(checkpoint.committed_offset() - 2))
        .expect("seek inside anchor");
    file.write_all(b"x").expect("mutate anchor byte");
    file.seek(SeekFrom::End(0)).expect("seek append");
    file.write_all(&usage_line(20)).expect("append usage");
    drop(file);

    assert_rebuild(
        read_source_batch(&descriptor, Some(&checkpoint), || false)
            .expect("anchor classification must pass"),
        RebuildReason::AnchorMismatch,
    );
}

#[test]
fn public_errors_and_results_never_expose_paths() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("error-private.jsonl");
    write_fixture(&path, 10);
    let descriptor = only_descriptor(&source(root.path(), "error"));
    std::fs::remove_file(&path).expect("remove error fixture");

    let error = read_source_batch(&descriptor, None, || false).expect_err("open must fail");
    let debug = format!("{error:?} {error}");
    assert!(!debug.contains(root.path().to_string_lossy().as_ref()));
    assert!(!debug.contains("error-private.jsonl"));
}

#[test]
fn source_change_after_read_discards_the_batch_and_preserves_caller_checkpoint() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("concurrent-private.jsonl");
    write_fixture(&path, 10);
    let descriptor = only_descriptor(&source(root.path(), "concurrent"));
    let checkpoint = initial_checkpoint(&descriptor);
    let caller_copy = checkpoint.clone();
    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open append fixture")
        .write_all(&usage_line(20))
        .expect("append usage fixture");

    let mut cancellation_probes = 0_u8;
    let error = read_source_batch(&descriptor, Some(&checkpoint), || {
        cancellation_probes = cancellation_probes.saturating_add(1);
        if cancellation_probes == 4 {
            std::fs::rename(&path, root.path().join("old.jsonl"))
                .expect("replace source after framed read");
            write_fixture(&path, 30);
        }
        false
    })
    .expect_err("post-read path replacement must discard the batch");

    assert_eq!(error.code(), ReaderErrorCode::SourceChanged);
    assert_eq!(checkpoint, caller_copy);
    let debug = format!("{error:?} {error}");
    assert!(!debug.contains(root.path().to_string_lossy().as_ref()));
    assert!(!debug.contains("concurrent-private.jsonl"));
}

#[test]
fn growth_after_snapshot_is_deferred_to_the_next_batch() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("growth-private.jsonl");
    write_fixture(&path, 10);
    let descriptor = only_descriptor(&source(root.path(), "growth"));
    let checkpoint = initial_checkpoint(&descriptor);
    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open first append")
        .write_all(&usage_line(20))
        .expect("append first new event");

    let mut cancellation_probes = 0_u8;
    let first = expect_batch(
        read_source_batch(&descriptor, Some(&checkpoint), || {
            cancellation_probes = cancellation_probes.saturating_add(1);
            if cancellation_probes == 4 {
                OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .expect("open concurrent append")
                    .write_all(&usage_line(30))
                    .expect("append after snapshot");
            }
            false
        })
        .expect("growth after snapshot must not invalidate consumed bytes"),
    );
    assert_eq!(first.events().len(), 1);

    let second = expect_batch(
        read_source_batch(&descriptor, Some(first.checkpoint()), || false)
            .expect("deferred growth must be consumed next"),
    );
    assert_eq!(second.events().len(), 1);
    assert!(second.reached_snapshot_end());
}

#[test]
fn cancellation_releases_source_handles_on_every_attempt() {
    let root = TempDir::new().expect("temporary directory");
    let source_root = root.path().join("cancel-root");
    std::fs::create_dir(&source_root).expect("create cancellation root");
    let path = source_root.join("cancel-private.jsonl");
    write_fixture(&path, 10);
    let descriptor = only_descriptor(&source(&source_root, "cancel"));

    for _ in 0..100 {
        let mut probes = 0_u8;
        let error = read_source_batch(&descriptor, None, || {
            probes = probes.saturating_add(1);
            probes >= 2
        })
        .expect_err("read must cancel after the source is opened");
        assert_eq!(error.code(), ReaderErrorCode::Cancelled);
    }

    let initial = expect_batch(
        read_source_batch(&descriptor, None, || false).expect("initial integrity read must pass"),
    );
    let expected = initial.source_chunks().to_vec();
    let mut probes = 0_u8;
    let report = verify_full_prefix(
        &descriptor,
        initial.checkpoint(),
        |index| expected.get(index as usize).copied(),
        || {
            probes = probes.saturating_add(1);
            probes >= 2
        },
    )
    .expect("full verification cancellation must be clean");
    assert_eq!(report, IntegrityReport::Cancelled);

    drop(descriptor);
    let renamed = root.path().join("cancel-root-renamed");
    std::fs::rename(&source_root, &renamed).expect("rename proves no source handle leaked");
    std::fs::remove_dir_all(renamed).expect("delete proves no source handle leaked");
}

#[test]
fn full_prefix_finds_old_chunk_mutation_outside_incremental_anchor() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("integrity-private.jsonl");
    let long_line_len = usize::try_from(SOURCE_CHUNK_BYTES + 257).expect("chunk size fits");
    let mut long_line = vec![b'x'; long_line_len];
    long_line.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("create integrity fixture");
    file.write_all(&long_line).expect("write first long line");
    file.write_all(&long_line).expect("write second long line");
    drop(file);

    let descriptor = only_descriptor(&source(root.path(), "integrity"));
    let mut checkpoint = None;
    let mut expected = BTreeMap::<u64, SourceChunkDigest>::new();
    loop {
        let batch = expect_batch(
            read_source_batch(&descriptor, checkpoint.as_ref(), || false)
                .expect("bounded initial read must pass"),
        );
        for chunk in batch.source_chunks() {
            expected.insert(chunk.index(), *chunk);
        }
        let reached_end = batch.reached_snapshot_end();
        checkpoint = Some(batch.checkpoint().clone());
        if reached_end {
            break;
        }
    }
    let checkpoint = checkpoint.expect("initial checkpoint");
    assert!(checkpoint.committed_offset() > SOURCE_CHUNK_BYTES * 2);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open integrity fixture");
    file.seek(SeekFrom::Start(128)).expect("seek old chunk");
    file.write_all(b"y").expect("mutate old chunk");
    file.seek(SeekFrom::End(0)).expect("seek append");
    file.write_all(&usage_line(40)).expect("append valid usage");
    drop(file);

    let append = expect_batch(
        read_source_batch(&descriptor, Some(&checkpoint), || false)
            .expect("anchor-valid append remains incremental"),
    );
    assert_eq!(
        append.checkpoint().verification(),
        VerificationLevel::Incremental
    );
    for chunk in append.source_chunks() {
        expected.insert(chunk.index(), *chunk);
    }
    let final_checkpoint = append.checkpoint().clone();
    let report = verify_full_prefix(
        &descriptor,
        &final_checkpoint,
        |index| expected.get(&index).copied(),
        || false,
    )
    .expect("full-prefix verification must complete");
    assert_eq!(report, IntegrityReport::Mismatch { chunk_index: 0 });

    let debug = format!("{report:?} {expected:?}");
    assert!(!debug.contains(root.path().to_string_lossy().as_ref()));
    assert!(!debug.contains("integrity-private.jsonl"));
}

#[test]
fn oversized_discard_emits_only_bounded_chunk_updates() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("oversized-private.jsonl");
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("create sparse oversized fixture")
        .set_len(SOURCE_CHUNK_BYTES * 20)
        .expect("extend sparse oversized fixture");
    let descriptor = only_descriptor(&source(root.path(), "oversized"));
    let mut checkpoint = None;
    let mut expected = BTreeMap::<u64, SourceChunkDigest>::new();

    loop {
        let batch = expect_batch(
            read_source_batch(&descriptor, checkpoint.as_ref(), || false)
                .expect("bounded discard read must pass"),
        );
        assert!(batch.source_chunks().len() <= 18);
        for chunk in batch.source_chunks() {
            expected.insert(chunk.index(), *chunk);
        }
        let reached_end = batch.reached_snapshot_end();
        let discarding = batch.checkpoint().discarding_oversized_line();
        checkpoint = Some(batch.checkpoint().clone());
        if reached_end && discarding {
            break;
        }
    }

    let discard_checkpoint = checkpoint.as_ref().expect("discard checkpoint");
    let discard_report = verify_full_prefix(
        &descriptor,
        discard_checkpoint,
        |index| expected.get(&index).copied(),
        || false,
    )
    .expect("discard cursor prefix must be verifiable");
    assert_eq!(
        discard_report,
        IntegrityReport::Verified {
            chunks: 20,
            covered_bytes: SOURCE_CHUNK_BYTES * 20,
        }
    );

    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open oversized fixture append")
        .write_all(b"\n")
        .expect("terminate oversized line");
    let final_batch = expect_batch(
        read_source_batch(&descriptor, checkpoint.as_ref(), || false)
            .expect("oversized completion must pass"),
    );
    assert!(final_batch.source_chunks().len() <= 2);
    for chunk in final_batch.source_chunks() {
        expected.insert(chunk.index(), *chunk);
    }
    let final_checkpoint = final_batch.checkpoint().clone();
    assert!(!final_checkpoint.discarding_oversized_line());

    let report = verify_full_prefix(
        &descriptor,
        &final_checkpoint,
        |index| expected.get(&index).copied(),
        || false,
    )
    .expect("streaming full-prefix verification must pass");
    assert_eq!(
        report,
        IntegrityReport::Verified {
            chunks: 21,
            covered_bytes: SOURCE_CHUNK_BYTES * 20 + 1,
        }
    );
}

#[test]
fn append_proves_the_previous_partial_chunk_before_replacing_its_digest() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("partial-proof-private.jsonl");
    let mut initial_line = vec![b'x'; 64 * 1024];
    initial_line.push(b'\n');
    std::fs::write(&path, initial_line).expect("write partial chunk fixture");
    let descriptor = only_descriptor(&source(root.path(), "partial-proof"));
    let initial = expect_batch(
        read_source_batch(&descriptor, None, || false).expect("initial read must pass"),
    );
    let original = *initial
        .source_chunks()
        .first()
        .expect("initial partial chunk digest");
    let checkpoint = initial.checkpoint().clone();

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open partial chunk fixture");
    file.seek(SeekFrom::Start(128))
        .expect("seek old partial bytes");
    file.write_all(b"y").expect("mutate old partial bytes");
    file.seek(SeekFrom::End(0)).expect("seek append");
    file.write_all(&usage_line(50)).expect("append usage");
    drop(file);

    let append = expect_batch(
        read_source_batch(&descriptor, Some(&checkpoint), || false)
            .expect("mutation outside anchor remains incrementally classifiable"),
    );
    let proof = append
        .previous_partial_chunk()
        .expect("append must prove the stored partial prefix");
    assert_eq!(proof.index(), original.index());
    assert_eq!(proof.covered_len(), original.covered_len());
    assert_ne!(proof.sha256(), original.sha256());
}

#[test]
fn persisted_chunk_digest_construction_is_strict() {
    let valid = SourceChunkDigest::from_persisted_parts(7, 1, [3; 32])
        .expect("one-byte chunk must be valid");
    assert_eq!(valid.index(), 7);
    assert_eq!(valid.covered_len(), 1);
    assert_eq!(valid.sha256(), &[3; 32]);

    for (index, len) in [
        (0, 0),
        (
            0,
            u32::try_from(SOURCE_CHUNK_BYTES).expect("chunk fits") + 1,
        ),
        (u64::MAX, 1),
    ] {
        let error = SourceChunkDigest::from_persisted_parts(index, len, [0; 32])
            .expect_err("invalid persisted chunk must fail");
        assert_eq!(error.code(), ReaderErrorCode::CheckpointInvalid);
    }
}

#[test]
fn missing_or_changed_physical_identity_fails_closed() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("identity-private.jsonl");
    write_fixture(&path, 10);
    let descriptor = only_descriptor(&source(root.path(), "identity"));
    let checkpoint = initial_checkpoint(&descriptor);
    assert!(checkpoint.physical_identity().is_some());
    let without_physical = ReaderCheckpointV1::new(ReaderCheckpointParts {
        parser_schema_version: checkpoint.parser_schema_version(),
        physical_identity: None,
        logical_identity: checkpoint.logical_identity(),
        committed_offset: checkpoint.committed_offset(),
        scan_offset: checkpoint.scan_offset(),
        observed_file_length: checkpoint.observed_file_length(),
        modified_time_ns: checkpoint.modified_time_ns(),
        anchor: checkpoint.anchor(),
        resume: checkpoint.resume().clone(),
        discarding_oversized_line: checkpoint.discarding_oversized_line(),
        incomplete_tail: checkpoint.incomplete_tail(),
        verification: checkpoint.verification(),
    })
    .expect("identity-free checkpoint remains structurally valid");

    assert_rebuild(
        read_source_batch(&descriptor, Some(&without_physical), || false)
            .expect("identity classification must be stable"),
        RebuildReason::IdentityChanged,
    );
    let error = verify_full_prefix(&descriptor, &without_physical, |_| None, || false)
        .expect_err("full verification must reject changed identity evidence");
    assert_eq!(error.code(), ReaderErrorCode::SourceChanged);
}

#[cfg(windows)]
#[test]
fn intermediate_junction_inserted_after_enumeration_is_rejected() {
    let temp = TempDir::new().expect("temporary directory");
    let source_root = temp.path().join("source-root");
    let original_dir = source_root.join("nested");
    std::fs::create_dir_all(&original_dir).expect("create original source directory");
    let original_path = original_dir.join("junction-private.jsonl");
    write_fixture(&original_path, 10);
    let descriptor = only_descriptor(&source(&source_root, "junction-race"));

    std::fs::rename(&original_dir, source_root.join("old-nested"))
        .expect("move enumerated directory");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).expect("create outside directory");
    write_fixture(&outside.join("junction-private.jsonl"), 20);
    create_junction(&outside, &original_dir);

    let error = read_source_batch(&descriptor, None, || false)
        .expect_err("intermediate junction must never be followed");
    assert_eq!(error.code(), ReaderErrorCode::ReparsePoint);
    std::fs::remove_dir(&original_dir).expect("remove junction without following it");
}
