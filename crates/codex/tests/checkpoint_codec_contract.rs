use std::path::PathBuf;

use tempfile::TempDir;
use tokenmaster_codex::{
    CodexCheckpointErrorCode, CodexCheckpointV1, MAX_CODEX_CHECKPOINT_BYTES, ReaderOutcome,
    SinkDecision, SourceFileDescriptor, enumerate_profile_sources, initialize_source_checkpoint,
    logical_file_identity, read_source_batch,
};
use tokenmaster_provider::{ProfileId, SourceDescriptor, SourceId, SourceKind};

fn source(root: impl Into<PathBuf>) -> SourceDescriptor {
    SourceDescriptor::new(
        SourceId::new("source_checkpoint_codec").expect("valid source ID"),
        ProfileId::new("profile_checkpoint_codec").expect("valid profile ID"),
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

#[test]
fn initial_checkpoint_is_a_zero_offset_probe_and_round_trips_without_paths() {
    let root = TempDir::new().expect("temporary directory");
    let path = root.path().join("private-session.jsonl");
    let payload = br#"{"timestamp":"2026-07-15T00:00:00Z","model":"gpt-5","usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5}}
"#;
    std::fs::write(&path, payload).expect("write fixture");
    let descriptor = only_descriptor(&source(root.path()));

    let initial = initialize_source_checkpoint(&descriptor).expect("initialize checkpoint");
    assert_eq!(initial.committed_offset(), 0);
    assert_eq!(initial.scan_offset(), 0);
    assert_eq!(initial.observed_file_length(), payload.len() as u64);
    assert_eq!(
        initial.logical_identity(),
        logical_file_identity(&descriptor)
    );
    assert_eq!(
        initial.verification(),
        tokenmaster_codex::VerificationLevel::FullPrefix
    );

    let encoded = CodexCheckpointV1::new(initial.clone())
        .encode()
        .expect("encode checkpoint");
    assert!(encoded.len() <= MAX_CODEX_CHECKPOINT_BYTES);
    assert!(
        !encoded
            .windows(path.as_os_str().len())
            .any(|window| { window == path.as_os_str().to_string_lossy().as_bytes() })
    );
    assert!(
        !encoded
            .windows(b"input_tokens".len())
            .any(|window| window == b"input_tokens")
    );

    let decoded = CodexCheckpointV1::decode(&encoded, initial.logical_identity())
        .expect("decode checkpoint")
        .into_reader();
    assert_eq!(decoded, initial);

    let outcome = read_source_batch(&descriptor, Some(&decoded), || false)
        .expect("read from initialized checkpoint");
    let ReaderOutcome::Batch(batch) = outcome else {
        panic!("non-empty initialized source must produce a batch");
    };
    assert_eq!(batch.events().len(), 1);
}

#[test]
fn decoder_rejects_version_identity_trailing_and_oversized_envelopes() {
    let root = TempDir::new().expect("temporary directory");
    std::fs::write(root.path().join("session.jsonl"), b"{}\n").expect("write fixture");
    let descriptor = only_descriptor(&source(root.path()));
    let initial = initialize_source_checkpoint(&descriptor).expect("initialize checkpoint");
    let encoded = CodexCheckpointV1::new(initial.clone())
        .encode()
        .expect("encode checkpoint");

    let mut unknown_version = encoded.clone();
    unknown_version[4..6].copy_from_slice(&2_u16.to_le_bytes());
    let error = CodexCheckpointV1::decode(&unknown_version, initial.logical_identity())
        .expect_err("unknown version must fail");
    assert_eq!(error.code(), CodexCheckpointErrorCode::UnsupportedVersion);

    let error = CodexCheckpointV1::decode(
        &encoded,
        tokenmaster_codex::LogicalFileIdentity::from_bytes([0xA5; 32]),
    )
    .expect_err("logical identity mismatch must fail");
    assert_eq!(error.code(), CodexCheckpointErrorCode::IdentityMismatch);

    let mut trailing = encoded.clone();
    trailing.push(0);
    let error = CodexCheckpointV1::decode(&trailing, initial.logical_identity())
        .expect_err("trailing byte must fail");
    assert_eq!(error.code(), CodexCheckpointErrorCode::TrailingBytes);

    let oversized = vec![0_u8; MAX_CODEX_CHECKPOINT_BYTES + 1];
    let error = CodexCheckpointV1::decode(&oversized, initial.logical_identity())
        .expect_err("oversized envelope must fail before decoding");
    assert_eq!(error.code(), CodexCheckpointErrorCode::CapacityExceeded);
}

#[test]
fn checkpoint_debug_is_payload_and_identity_private() {
    let root = TempDir::new().expect("temporary directory");
    let private_name = "private-checkpoint-name.jsonl";
    std::fs::write(root.path().join(private_name), b"{}\n").expect("write fixture");
    let descriptor = only_descriptor(&source(root.path()));
    let checkpoint = CodexCheckpointV1::new(
        initialize_source_checkpoint(&descriptor).expect("initialize checkpoint"),
    );

    let debug = format!("{checkpoint:?}");
    assert!(!debug.contains(private_name));
    assert!(!debug.contains("profile_checkpoint_codec"));
    assert!(!debug.contains("source_checkpoint_codec"));
    assert!(!debug.contains("[165, 165"));
}
