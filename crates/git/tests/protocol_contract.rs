use tokenmaster_domain::{GitLineMetrics, GitOutputCategory};
use tokenmaster_git::{
    GitCoreError, GitIdentitySalt, GitLogParseConfig, GitLogStreamParser, GitScanAccumulator,
    GitStreamLimits, derive_author_fingerprint,
};

const OID_A: &str = "0123456789012345678901234567890123456789";
const OID_B: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
const PARENT_A: &str = "1111111111111111111111111111111111111111";
const PARENT_B: &str = "2222222222222222222222222222222222222222";

fn append_header(output: &mut Vec<u8>, oid: &str, timestamp: i64, author: &str, parents: &str) {
    output.push(0x1e);
    output.extend_from_slice(oid.as_bytes());
    output.push(0);
    output.extend_from_slice(timestamp.to_string().as_bytes());
    output.push(0);
    output.extend_from_slice(author.as_bytes());
    output.push(0);
    output.extend_from_slice(parents.as_bytes());
    output.push(0);
}

fn append_raw(
    output: &mut Vec<u8>,
    old_mode: &str,
    new_mode: &str,
    status: &str,
    source: &[u8],
    destination: Option<&[u8]>,
) {
    output.extend_from_slice(format!(":{old_mode} {new_mode} 0000000 1111111 {status}").as_bytes());
    output.push(0);
    output.extend_from_slice(source);
    output.push(0);
    if let Some(destination) = destination {
        output.extend_from_slice(destination);
        output.push(0);
    }
}

fn append_numstat(
    output: &mut Vec<u8>,
    added: &str,
    removed: &str,
    source: &[u8],
    destination: Option<&[u8]>,
) {
    output.extend_from_slice(added.as_bytes());
    output.push(b'\t');
    output.extend_from_slice(removed.as_bytes());
    output.push(b'\t');
    if let Some(destination) = destination {
        output.push(0);
        output.extend_from_slice(source);
        output.push(0);
        output.extend_from_slice(destination);
        output.push(0);
    } else {
        output.extend_from_slice(source);
        output.push(0);
    }
}

fn config() -> GitLogParseConfig {
    let salt = GitIdentitySalt::from_bytes([9; 32]);
    let author = derive_author_fingerprint(&salt, b"user@example.com").expect("author fingerprint");
    GitLogParseConfig::new(salt, vec![author], GitStreamLimits::default()).expect("parse config")
}

fn fixture() -> Vec<u8> {
    let mut output = Vec::new();
    append_header(
        &mut output,
        OID_A,
        20_000 * 86_400 + 1,
        "USER@example.com",
        &format!("{PARENT_A} {PARENT_B}"),
    );
    output.push(b'\n');
    append_raw(&mut output, "100644", "100644", "M", b"src/main.rs", None);
    append_raw(
        &mut output,
        "100644",
        "100644",
        "R100",
        b"src/old.rs",
        Some(b"tests/new.rs"),
    );
    append_raw(
        &mut output,
        "100644",
        "100644",
        "M",
        b"assets/logo.png",
        None,
    );
    append_raw(
        &mut output,
        "160000",
        "160000",
        "M",
        b"vendor/submodule",
        None,
    );
    append_numstat(&mut output, "20", "3", b"src/main.rs", None);
    append_numstat(&mut output, "8", "4", b"src/old.rs", Some(b"tests/new.rs"));
    append_numstat(&mut output, "-", "-", b"assets/logo.png", None);
    append_numstat(&mut output, "1", "1", b"vendor/submodule", None);

    append_header(
        &mut output,
        OID_B,
        20_001 * 86_400 + 1,
        "other@example.com",
        PARENT_A,
    );
    output.push(b'\n');
    append_raw(
        &mut output,
        "100644",
        "100644",
        "M",
        b"src/ignored.rs",
        None,
    );
    append_numstat(&mut output, "100", "0", b"src/ignored.rs", None);
    output
}

#[test]
fn real_git_nul_shape_parses_incrementally_and_filters_author() {
    let bytes = fixture();
    for chunk_size in [1, 2, 7, 64, bytes.len()] {
        let mut parser = GitLogStreamParser::new(config());
        let mut scan = GitScanAccumulator::new();
        for chunk in bytes.chunks(chunk_size) {
            parser.push(chunk, &mut scan).expect("parse chunk");
        }
        parser.finish(&mut scan).expect("finish parser");
        let summary = scan.finish().expect("finish scan");

        assert_eq!(summary.totals().commits(), 1);
        assert_eq!(summary.totals().merge_commits(), 1);
        assert_eq!(summary.totals().lines(), GitLineMetrics::new(28, 7));
        assert_eq!(summary.totals().binary_files(), 1);
        assert_eq!(summary.totals().submodule_changes(), 1);
        assert_eq!(
            summary.category_lines(GitOutputCategory::ProductCode),
            GitLineMetrics::new(20, 3)
        );
        assert_eq!(
            summary.category_lines(GitOutputCategory::Test),
            GitLineMetrics::new(8, 4)
        );
        assert_eq!(summary.retained_days().len(), 1);
        assert_eq!(summary.retained_days()[0].day_index(), 20_000);
    }
}

#[test]
fn protocol_rejects_truncation_path_mismatch_and_field_limits() {
    let bytes = fixture();
    let mut parser = GitLogStreamParser::new(config());
    let mut scan = GitScanAccumulator::new();
    parser
        .push(&bytes[..bytes.len() - 1], &mut scan)
        .expect("bounded prefix");
    assert_eq!(
        parser.finish(&mut scan),
        Err(GitCoreError::IncompleteProtocol)
    );

    let mut mismatch = fixture();
    let needle = b"20\t3\tsrc/main.rs\0";
    let position = mismatch
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("numstat position");
    mismatch[position + needle.len() - 3] = b'x';
    let mut parser = GitLogStreamParser::new(config());
    let mut scan = GitScanAccumulator::new();
    assert_eq!(
        parser.push(&mismatch, &mut scan),
        Err(GitCoreError::ProtocolMismatch)
    );

    let mut invalid_rename_source = fixture();
    let rename_source = b"src/old.rs\0tests/new.rs";
    let position = invalid_rename_source
        .windows(rename_source.len())
        .position(|window| window == rename_source)
        .expect("rename source position");
    invalid_rename_source.splice(
        position..position + b"src/old.rs".len(),
        b"../old.rs".iter().copied(),
    );
    let mut parser = GitLogStreamParser::new(config());
    let mut scan = GitScanAccumulator::new();
    assert_eq!(
        parser.push(&invalid_rename_source, &mut scan),
        Err(GitCoreError::InvalidPath)
    );

    let mut invalid_foreign_timestamp = Vec::new();
    append_header(
        &mut invalid_foreign_timestamp,
        OID_B,
        i64::MAX,
        "other@example.com",
        PARENT_A,
    );
    let mut parser = GitLogStreamParser::new(config());
    let mut scan = GitScanAccumulator::new();
    assert_eq!(
        parser.push(&invalid_foreign_timestamp, &mut scan),
        Err(GitCoreError::InvalidTimestamp)
    );

    let salt = GitIdentitySalt::from_bytes([9; 32]);
    let author = derive_author_fingerprint(&salt, b"user@example.com").expect("author fingerprint");
    let limits = GitStreamLimits::new(8, 8, 4, 4).expect("small valid limits");
    let config = GitLogParseConfig::new(salt, vec![author], limits).expect("small parse config");
    let mut parser = GitLogStreamParser::new(config);
    let mut scan = GitScanAccumulator::new();
    assert_eq!(
        parser.push(&bytes, &mut scan),
        Err(GitCoreError::CapacityExceeded { limit: 8 })
    );
}

#[test]
fn parser_debug_and_errors_never_retain_raw_identity_or_paths() {
    let parser = GitLogStreamParser::new(config());
    let debug = format!("{parser:?}");
    for forbidden in [
        "user@example.com",
        "src/main.rs",
        OID_A,
        PARENT_A,
        r"C:\private\repo",
    ] {
        assert!(!debug.contains(forbidden), "leaked {forbidden}: {debug}");
    }

    assert_eq!(
        GitCoreError::InvalidProtocol.stable_code(),
        "invalid_protocol"
    );
    assert_eq!(
        GitCoreError::ProtocolMismatch.stable_code(),
        "protocol_mismatch"
    );
}
