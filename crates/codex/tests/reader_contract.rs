use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokenmaster_codex::{
    BoundaryAnchor, LogicalFileIdentity, MAX_ANCHOR_BYTES, MAX_BATCH_COMPLETE_BYTES,
    MAX_BATCH_EVENTS, MAX_LINE_BYTES, MAX_RESUME_BYTES, PARSER_SCHEMA_VERSION,
    ParserDiagnosticCode, ParserResumeStateV1, ParserState, READ_BUFFER_BYTES,
    READER_CHECKPOINT_SCHEMA_VERSION, ReadBatch, ReaderCheckpointErrorCode, ReaderCheckpointParts,
    ReaderCheckpointV1, ReaderDiagnosticCode, ReaderOutcome, SOURCE_CHUNK_BYTES, SinkDecision,
    SourceFileDescriptor, VerificationLevel, enumerate_profile_sources, logical_file_identity,
    read_source_batch,
};
use tokenmaster_platform::PhysicalFileIdentity;
use tokenmaster_provider::{ProfileId, SourceDescriptor, SourceId, SourceKind};

fn checkpoint_parts() -> ReaderCheckpointParts {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("identity.jsonl");
    std::fs::write(&path, b"").expect("write fixture");
    let identity =
        PhysicalFileIdentity::from_file(&std::fs::File::open(path).expect("open identity fixture"))
            .expect("physical identity");

    ReaderCheckpointParts {
        parser_schema_version: PARSER_SCHEMA_VERSION,
        physical_identity: Some(identity),
        logical_identity: LogicalFileIdentity::from_bytes([7; 32]),
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: 0,
        modified_time_ns: None,
        anchor: BoundaryAnchor::new(0, 0, [0; 32]).expect("empty anchor"),
        resume: ParserState::new().snapshot(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: VerificationLevel::Incremental,
    }
}

fn source(kind: SourceKind, root: impl Into<PathBuf>, source_suffix: &str) -> SourceDescriptor {
    SourceDescriptor::new(
        SourceId::new(format!("source_{source_suffix}")).expect("valid source ID"),
        ProfileId::new("profile_reader_fixture").expect("valid profile ID"),
        kind,
        root,
    )
    .expect("valid source descriptor")
}

fn collect_logical_identity(source: &SourceDescriptor) -> LogicalFileIdentity {
    let mut identity = None;
    enumerate_profile_sources(
        std::slice::from_ref(source),
        || false,
        |descriptor| {
            assert!(identity.is_none(), "fixture must contain exactly one file");
            identity = Some(logical_file_identity(&descriptor));
            SinkDecision::Continue
        },
    )
    .expect("fixture enumeration must pass");
    identity.expect("fixture must emit one file")
}

fn write_nested_fixture(root: &Path) {
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).expect("create nested fixture");
    std::fs::write(nested.join("session.jsonl"), b"{}\n").expect("write JSONL fixture");
}

fn only_descriptor(source: &SourceDescriptor) -> SourceFileDescriptor {
    let mut descriptor = None;
    enumerate_profile_sources(
        std::slice::from_ref(source),
        || false,
        |file| {
            assert!(
                descriptor.is_none(),
                "fixture must contain exactly one file"
            );
            descriptor = Some(file);
            SinkDecision::Continue
        },
    )
    .expect("fixture enumeration must pass");
    descriptor.expect("fixture must emit one file")
}

fn expect_batch(outcome: ReaderOutcome) -> ReadBatch {
    match outcome {
        ReaderOutcome::Batch(batch) => batch,
        other => panic!("batch expected, got {other:?}"),
    }
}

fn usage_line(timestamp: &str, input: u64) -> Vec<u8> {
    format!(
        r#"{{"timestamp":"{timestamp}","model":"gpt-5.6-sol","usage":{{"input_tokens":{input},"output_tokens":2,"total_tokens":{}}}}}"#,
        input + 2
    )
    .into_bytes()
}

#[test]
fn checkpoint_requires_complete_line_and_bounded_offsets() {
    let valid = checkpoint_parts();
    let checkpoint = ReaderCheckpointV1::new(valid.clone()).expect("valid checkpoint");
    assert_eq!(checkpoint.committed_offset(), 0);

    let mut before_commit = valid.clone();
    before_commit.committed_offset = 1;
    assert_eq!(
        ReaderCheckpointV1::new(before_commit)
            .expect_err("scan before commit must fail")
            .code(),
        ReaderCheckpointErrorCode::InvalidOffset,
    );

    let mut uncommitted = valid.clone();
    uncommitted.scan_offset = 1;
    uncommitted.observed_file_length = 1;
    assert_eq!(
        ReaderCheckpointV1::new(uncommitted)
            .expect_err("ordinary scan cannot pass commit")
            .code(),
        ReaderCheckpointErrorCode::InvalidOffset,
    );

    let mut beyond_file = valid.clone();
    beyond_file.committed_offset = 1;
    beyond_file.scan_offset = 1;
    assert_eq!(
        ReaderCheckpointV1::new(beyond_file)
            .expect_err("offset beyond observed length must fail")
            .code(),
        ReaderCheckpointErrorCode::InvalidOffset,
    );

    let mut past_commit = valid.clone();
    past_commit.anchor = BoundaryAnchor::new(0, 1, [1; 32]).expect("bounded anchor");
    assert_eq!(
        ReaderCheckpointV1::new(past_commit)
            .expect_err("anchor beyond commit must fail")
            .code(),
        ReaderCheckpointErrorCode::InvalidAnchor,
    );

    let mut wrong_parser = valid.clone();
    wrong_parser.parser_schema_version = PARSER_SCHEMA_VERSION + 1;
    assert_eq!(
        ReaderCheckpointV1::new(wrong_parser)
            .expect_err("parser version mismatch must fail")
            .code(),
        ReaderCheckpointErrorCode::UnsupportedParserVersion,
    );

    let mut discard_without_tail = valid.clone();
    discard_without_tail.scan_offset = 1;
    discard_without_tail.observed_file_length = 1;
    discard_without_tail.discarding_oversized_line = true;
    assert_eq!(
        ReaderCheckpointV1::new(discard_without_tail)
            .expect_err("discard mode requires an incomplete tail")
            .code(),
        ReaderCheckpointErrorCode::InvalidFlags,
    );

    let mut empty_discard = valid.clone();
    empty_discard.discarding_oversized_line = true;
    empty_discard.incomplete_tail = true;
    assert_eq!(
        ReaderCheckpointV1::new(empty_discard)
            .expect_err("discard mode must advance scan offset")
            .code(),
        ReaderCheckpointErrorCode::InvalidFlags,
    );

    let mut active_discard = valid;
    active_discard.scan_offset = 1;
    active_discard.observed_file_length = 1;
    active_discard.discarding_oversized_line = true;
    active_discard.incomplete_tail = true;
    assert!(ReaderCheckpointV1::new(active_discard).is_ok());
}

#[test]
fn checkpoint_rejects_invalid_anchor_and_resume_values() {
    assert_eq!(
        BoundaryAnchor::new(0, MAX_ANCHOR_BYTES + 1, [0; 32])
            .expect_err("oversized anchor must fail")
            .code(),
        ReaderCheckpointErrorCode::InvalidAnchor,
    );
    assert_eq!(
        BoundaryAnchor::new(u64::MAX, 1, [0; 32])
            .expect_err("anchor end overflow must fail")
            .code(),
        ReaderCheckpointErrorCode::InvalidAnchor,
    );

    let mut resume_json =
        serde_json::to_value(ParserState::new().snapshot()).expect("resume fixture must serialize");
    resume_json["version"] = serde_json::json!(PARSER_SCHEMA_VERSION + 1);
    let invalid_resume: ParserResumeStateV1 =
        serde_json::from_value(resume_json).expect("unsupported resume must decode structurally");
    let mut parts = checkpoint_parts();
    parts.resume = invalid_resume;
    assert_eq!(
        ReaderCheckpointV1::new(parts)
            .expect_err("unsupported embedded resume must fail")
            .code(),
        ReaderCheckpointErrorCode::InvalidResume,
    );
}

#[test]
fn reader_values_are_fixed_size_and_debug_private() {
    assert_eq!(READER_CHECKPOINT_SCHEMA_VERSION, 1);
    assert_eq!(READ_BUFFER_BYTES, 128 * 1024);
    assert_eq!(MAX_BATCH_EVENTS, 256);
    assert_eq!(MAX_BATCH_COMPLETE_BYTES, 1 << 20);
    assert_eq!(MAX_ANCHOR_BYTES, 4096);
    assert_eq!(SOURCE_CHUNK_BYTES, 1 << 20);
    assert_eq!(MAX_RESUME_BYTES, 32 * 1024);

    let logical = LogicalFileIdentity::from_bytes([7; 32]);
    assert_eq!(logical.as_bytes(), &[7; 32]);
    assert_eq!(format!("{logical:?}"), "LogicalFileIdentity([redacted])");

    let anchor = BoundaryAnchor::new(0, 0, [9; 32]).expect("empty anchor");
    assert_eq!(
        format!("{anchor:?}"),
        "BoundaryAnchor { start: 0, len: 0, sha256: [redacted] }"
    );

    let checkpoint = ReaderCheckpointV1::new(checkpoint_parts()).expect("valid checkpoint");
    let debug = format!("{checkpoint:?}");
    assert!(debug.contains("physical_identity: [redacted]"));
    assert!(debug.contains("logical_identity: [redacted]"));
    assert!(debug.contains("anchor: [redacted]"));
    assert!(debug.contains("resume: [redacted]"));
    assert!(!debug.contains("7, 7, 7"));
}

#[test]
fn active_and_archived_paths_share_identity_while_direct_sources_do_not() {
    let root = TempDir::new().expect("temporary directory");
    let active_root = root.path().join("active");
    let archived_root = root.path().join("archived");
    let direct_root = root.path().join("direct");
    write_nested_fixture(&active_root);
    write_nested_fixture(&archived_root);
    write_nested_fixture(&direct_root);

    let active = collect_logical_identity(&source(SourceKind::Active, active_root, "active"));
    let archived =
        collect_logical_identity(&source(SourceKind::Archived, archived_root, "archived"));
    assert_eq!(active, archived);
    #[cfg(windows)]
    assert_eq!(
        active.as_bytes(),
        &[
            0x6f, 0x3f, 0x6c, 0x34, 0x97, 0x42, 0xc6, 0xa1, 0x66, 0x0d, 0x95, 0x0d, 0xf0, 0xba,
            0xb1, 0xb9, 0xe2, 0x47, 0xa5, 0xfb, 0x3d, 0xc3, 0xae, 0xb9, 0xe1, 0x06, 0xf6, 0xd7,
            0x05, 0xec, 0xf3, 0x32,
        ],
        "logical identity is a persistent store key and must not drift",
    );

    let direct_one =
        collect_logical_identity(&source(SourceKind::Direct, &direct_root, "direct_one"));
    let direct_two =
        collect_logical_identity(&source(SourceKind::Direct, &direct_root, "direct_two"));
    assert_ne!(direct_one, direct_two);
}

#[test]
fn incomplete_tail_is_reread_without_persisting_raw_bytes() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("session.jsonl");
    let metadata = br#"{"type":"session_meta","payload":{"id":"session_reader"}}"#;
    let first_usage = usage_line("2026-07-14T10:00:00Z", 10);
    let second_usage = br#"{"private_tail":"TAIL_SECRET_71E2","timestamp":"2026-07-14T10:01:00Z","model":"gpt-5.6-sol","usage":{"input_tokens":20,"output_tokens":3,"total_tokens":23}}"#;
    let split = second_usage
        .windows(b"timestamp".len())
        .position(|window| window == b"timestamp")
        .expect("fixture split marker");

    let mut initial = Vec::new();
    initial.extend_from_slice(metadata);
    initial.push(b'\n');
    let first_offset = initial.len() as u64;
    initial.extend_from_slice(&first_usage);
    initial.push(b'\n');
    let second_offset = initial.len() as u64;
    initial.extend_from_slice(&second_usage[..split]);
    std::fs::write(&path, &initial).expect("write incomplete fixture");

    let descriptor = only_descriptor(&source(SourceKind::Direct, root.path(), "tail"));
    let first =
        expect_batch(read_source_batch(&descriptor, None, || false).expect("first read must pass"));
    assert_eq!(first.events().len(), 1);
    assert_eq!(first.events()[0].source_offset(), first_offset);
    assert_eq!(first.checkpoint().committed_offset(), second_offset);
    assert_eq!(first.checkpoint().scan_offset(), second_offset);
    assert!(first.checkpoint().incomplete_tail());
    assert!(first.reached_snapshot_end());
    assert!(!format!("{first:?}").contains("TAIL_SECRET_71E2"));
    assert!(!format!("{:?}", first.checkpoint()).contains("TAIL_SECRET_71E2"));

    use std::io::Write;
    let mut append = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open fixture for append");
    append
        .write_all(&second_usage[split..])
        .expect("append tail remainder");
    append.write_all(b"\n").expect("append final newline");
    drop(append);

    let second = expect_batch(
        read_source_batch(&descriptor, Some(first.checkpoint()), || false)
            .expect("second read must pass"),
    );
    assert_eq!(second.events().len(), 1);
    assert_eq!(second.events()[0].source_offset(), second_offset);
    assert_eq!(
        second.checkpoint().committed_offset(),
        std::fs::metadata(&path).expect("metadata").len()
    );
    assert!(!second.checkpoint().incomplete_tail());
    assert!(second.reached_snapshot_end());
}

#[test]
fn crlf_terminator_counts_both_bytes_but_is_not_parser_input() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("crlf.jsonl");
    let line = usage_line("2026-07-14T11:00:00Z", 30);
    let mut bytes = line.clone();
    bytes.extend_from_slice(b"\r\n");
    std::fs::write(&path, bytes).expect("write CRLF fixture");
    let descriptor = only_descriptor(&source(SourceKind::Direct, root.path(), "crlf"));

    let batch =
        expect_batch(read_source_batch(&descriptor, None, || false).expect("CRLF read must pass"));
    assert_eq!(batch.events().len(), 1);
    assert_eq!(batch.events()[0].source_offset(), 0);
    assert_eq!(batch.checkpoint().committed_offset(), line.len() as u64 + 2);
    assert_eq!(batch.diagnostics().count(ReaderDiagnosticCode::CrlfLine), 1);
}

#[test]
fn oversized_incomplete_line_uses_numeric_discard_and_is_rejected_once() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("oversized.jsonl");
    let mut oversized = b"OVERSIZED_SECRET_9C41".to_vec();
    oversized.resize(MAX_LINE_BYTES + 1, b'x');
    std::fs::write(&path, &oversized).expect("write oversized fixture");
    let descriptor = only_descriptor(&source(SourceKind::Direct, root.path(), "oversized"));

    let first = expect_batch(
        read_source_batch(&descriptor, None, || false).expect("discard read must pass"),
    );
    assert!(first.events().is_empty());
    assert_eq!(first.checkpoint().committed_offset(), 0);
    assert_eq!(first.checkpoint().scan_offset(), oversized.len() as u64);
    assert!(first.checkpoint().discarding_oversized_line());
    assert!(first.checkpoint().incomplete_tail());
    assert!(first.diagnostics().max_line_bytes_retained() <= MAX_LINE_BYTES as u64);
    assert_eq!(
        first
            .parser_diagnostics()
            .count(ParserDiagnosticCode::LineTooLarge),
        0,
        "an incomplete oversized line is rejected only when newline arrives",
    );
    assert!(!format!("{first:?}").contains("OVERSIZED_SECRET_9C41"));

    use std::io::Write;
    let usage = usage_line("2026-07-14T12:00:00Z", 40);
    let mut append = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open oversized fixture for append");
    append.write_all(b"\n").expect("terminate oversized line");
    append.write_all(&usage).expect("append valid usage");
    append.write_all(b"\n").expect("terminate valid usage");
    drop(append);

    let second = expect_batch(
        read_source_batch(&descriptor, Some(first.checkpoint()), || false)
            .expect("discard resume must pass"),
    );
    assert_eq!(second.events().len(), 1);
    assert_eq!(
        second.events()[0].source_offset(),
        oversized.len() as u64 + 1
    );
    assert_eq!(
        second
            .parser_diagnostics()
            .count(ParserDiagnosticCode::LineTooLarge),
        1,
    );
    assert_eq!(
        second
            .diagnostics()
            .count(ReaderDiagnosticCode::OversizedLine),
        1,
    );
    assert!(!second.checkpoint().discarding_oversized_line());
    assert!(!second.checkpoint().incomplete_tail());
}

#[test]
fn event_batch_limit_is_hard_and_resume_continues_at_exact_offset() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("events.jsonl");
    let mut bytes = Vec::new();
    let mut offsets = Vec::new();
    for index in 0..=MAX_BATCH_EVENTS {
        offsets.push(bytes.len() as u64);
        let timestamp = 1_800_000_000_u64 + index as u64;
        let line = format!(
            r#"{{"timestamp":{timestamp},"model":"gpt-5.6-sol","usage":{{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}}"#
        );
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    std::fs::write(&path, bytes).expect("write event-limit fixture");
    let descriptor = only_descriptor(&source(SourceKind::Direct, root.path(), "events"));

    let first = expect_batch(
        read_source_batch(&descriptor, None, || false).expect("first event batch must pass"),
    );
    assert_eq!(first.events().len(), MAX_BATCH_EVENTS);
    assert!(!first.reached_snapshot_end());
    assert_eq!(
        first.checkpoint().committed_offset(),
        offsets[MAX_BATCH_EVENTS]
    );

    let second = expect_batch(
        read_source_batch(&descriptor, Some(first.checkpoint()), || false)
            .expect("second event batch must pass"),
    );
    assert_eq!(second.events().len(), 1);
    assert_eq!(
        second.events()[0].source_offset(),
        offsets[MAX_BATCH_EVENTS]
    );
    assert!(second.reached_snapshot_end());
}

#[test]
fn one_valid_line_may_exceed_soft_byte_budget_and_still_reach_snapshot_end() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("large-valid.jsonl");
    let mut line = br#"{"padding":""#.to_vec();
    line.resize(MAX_BATCH_COMPLETE_BYTES as usize + 128, b'a');
    line.extend_from_slice(
        br#"","timestamp":"2026-07-14T13:00:00Z","model":"gpt-5.6-sol","usage":{"input_tokens":50,"output_tokens":2,"total_tokens":52}}"#,
    );
    assert!(line.len() as u64 > MAX_BATCH_COMPLETE_BYTES);
    assert!(line.len() <= MAX_LINE_BYTES);
    line.push(b'\n');
    std::fs::write(&path, &line).expect("write large valid fixture");
    let descriptor = only_descriptor(&source(SourceKind::Direct, root.path(), "large"));

    let batch = expect_batch(
        read_source_batch(&descriptor, None, || false).expect("large valid read must pass"),
    );
    assert_eq!(batch.events().len(), 1);
    assert_eq!(batch.checkpoint().committed_offset(), line.len() as u64);
    assert!(batch.reached_snapshot_end());
}
