#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::record::{
    MAX_RECORD_PAYLOAD_BYTES, RecordKind, RecordLoad, RecordRedundancy, RecordSaveBoundary,
    RecordSaveHook, RecordValue, RecordValueError, RedundantRecordStore,
};
use crate::{StateError, StateErrorCode};
use serde::de::DeserializeOwned;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokenmaster_platform::ValidatedLocalDirectory;

const HEADER_BYTES: usize = 64;
const FOOTER_BYTES: usize = 40;
const MAGIC: &[u8; 8] = b"TMREC001";
const FOOTER_MAGIC: &[u8; 8] = b"TMEND001";
static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);
static CRASH_SERIALIZE_CALLS: AtomicU64 = AtomicU64::new(0);
static CHANGING_SERIALIZE_CALLS: AtomicU64 = AtomicU64::new(0);

const CRASH_CHILD: &str = "TM_RECORD_CRASH_CHILD";
const CRASH_ROOT: &str = "TM_RECORD_CRASH_ROOT";
const CRASH_PHASE: &str = "TM_RECORD_CRASH_PHASE";
const CRASH_MARKER: &str = "TM_RECORD_CRASH_MARKER";

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tokenmaster-record-contract-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create record root");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TestRecord {
    schema: u16,
    label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct CrashRecord {
    schema: u16,
    label: String,
    padding: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangingRecord {
    schema: u16,
    label: String,
}

fn decode_test_record<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, RecordValueError> {
    serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)
}

impl RecordValue for TestRecord {
    fn decode_json(bytes: &[u8]) -> Result<Self, RecordValueError> {
        decode_test_record(bytes)
    }
}

impl RecordValue for CrashRecord {
    fn decode_json(bytes: &[u8]) -> Result<Self, RecordValueError> {
        decode_test_record(bytes)
    }
}

impl RecordValue for ChangingRecord {
    fn decode_json(bytes: &[u8]) -> Result<Self, RecordValueError> {
        decode_test_record(bytes)
    }
}

impl Serialize for ChangingRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let call = CHANGING_SERIALIZE_CALLS.fetch_add(1, Ordering::SeqCst);
        let mut record = serializer.serialize_struct("ChangingRecord", 2)?;
        record.serialize_field("schema", &self.schema)?;
        record.serialize_field("label", if call == 0 { self.label.as_str() } else { "z" })?;
        record.end()
    }
}

impl Serialize for CrashRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let call = CRASH_SERIALIZE_CALLS.fetch_add(1, Ordering::SeqCst);
        let phase = std::env::var(CRASH_PHASE).ok();
        let should_interrupt = std::env::var_os(CRASH_CHILD).is_some() && call == 1;
        let mut record = serializer.serialize_struct("CrashRecord", 3)?;
        record.serialize_field("schema", &self.schema)?;
        if should_interrupt && phase.as_deref() == Some("before-flush") {
            signal_crash_parent_and_wait("before-flush");
        }
        record.serialize_field("label", &self.label)?;
        record.serialize_field("padding", &self.padding)?;
        record.end()
    }
}

struct CrashSaveHook {
    phase: String,
}

impl RecordSaveHook for CrashSaveHook {
    fn hit(&mut self, boundary: RecordSaveBoundary) -> Result<(), StateError> {
        let expected = match boundary {
            RecordSaveBoundary::BeforePublication => "before-publish",
            RecordSaveBoundary::AfterPublication => "after-publish",
        };
        if self.phase == expected {
            signal_crash_parent_and_wait(expected);
        }
        Ok(())
    }
}

struct BreakReadbackHook {
    root: PathBuf,
}

impl RecordSaveHook for BreakReadbackHook {
    fn hit(&mut self, boundary: RecordSaveBoundary) -> Result<(), StateError> {
        if boundary == RecordSaveBoundary::AfterPublication {
            fs::rename(
                self.root.join("settings-a.tms"),
                self.root.join("published-preserved.tms"),
            )
            .expect("preserve published record");
            fs::create_dir(self.root.join("settings-a.tms")).expect("block readback");
        }
        Ok(())
    }
}

fn signal_crash_parent_and_wait(phase: &str) -> ! {
    let marker = std::env::var_os(CRASH_MARKER).expect("crash marker path");
    fs::write(PathBuf::from(marker), phase.as_bytes()).expect("signal crash parent");
    loop {
        std::thread::park_timeout(Duration::from_secs(1));
    }
}

fn value(label: &str) -> TestRecord {
    TestRecord {
        schema: 1,
        label: label.to_owned(),
    }
}

fn fixture() -> (TestDirectory, ValidatedLocalDirectory) {
    let root = TestDirectory::new();
    let directory = ValidatedLocalDirectory::new(root.path()).expect("validated record root");
    (root, directory)
}

fn record_store(directory: &ValidatedLocalDirectory) -> RedundantRecordStore<TestRecord> {
    RedundantRecordStore::new(directory, RecordKind::Settings, 1024).expect("record store")
}

fn crash_store(directory: &ValidatedLocalDirectory) -> RedundantRecordStore<CrashRecord> {
    RedundantRecordStore::new(directory, RecordKind::Settings, 512 * 1024)
        .expect("crash record store")
}

fn encode_raw(generation: u64, version: u16, payload: &[u8]) -> Vec<u8> {
    let mut header = [0_u8; HEADER_BYTES];
    header[0..8].copy_from_slice(MAGIC);
    header[8..10].copy_from_slice(&version.to_le_bytes());
    header[10..12].copy_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
    header[12..16].copy_from_slice(&0_u32.to_le_bytes());
    header[16..24].copy_from_slice(&generation.to_le_bytes());
    header[24..32].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    header[32..64].copy_from_slice(&Sha256::digest(payload));

    let mut record_hasher = Sha256::new();
    record_hasher.update(header);
    record_hasher.update(payload);
    record_hasher.update(FOOTER_MAGIC);
    let record_digest = record_hasher.finalize();

    let mut encoded = Vec::with_capacity(HEADER_BYTES + payload.len() + FOOTER_BYTES);
    encoded.extend_from_slice(&header);
    encoded.extend_from_slice(payload);
    encoded.extend_from_slice(FOOTER_MAGIC);
    encoded.extend_from_slice(&record_digest);
    encoded
}

fn reseal_raw(encoded: &mut [u8]) {
    let footer_digest = encoded.len() - 32;
    let digest = Sha256::digest(&encoded[..footer_digest]);
    encoded[footer_digest..].copy_from_slice(&digest);
}

fn expect_loaded(load: RecordLoad<TestRecord>) -> (u64, RecordRedundancy, TestRecord) {
    let RecordLoad::Loaded(record) = load else {
        panic!("expected one valid record");
    };
    (
        record.generation(),
        record.redundancy(),
        record.into_value(),
    )
}

#[test]
fn exact_envelope_round_trips_without_exposing_paths_or_payloads() {
    let (root, directory) = fixture();
    let store = record_store(&directory);
    let receipt = store.save(&value("first")).expect("first save");
    assert_eq!(receipt.generation(), 1);
    assert_eq!(receipt.redundancy(), RecordRedundancy::Single);

    let encoded = fs::read(root.path().join("settings-a.tms")).expect("first slot");
    assert_eq!(&encoded[0..8], MAGIC);
    assert_eq!(u16::from_le_bytes(encoded[8..10].try_into().unwrap()), 1);
    assert_eq!(
        u16::from_le_bytes(encoded[10..12].try_into().unwrap()),
        HEADER_BYTES as u16
    );
    assert_eq!(u32::from_le_bytes(encoded[12..16].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(encoded[16..24].try_into().unwrap()), 1);
    let payload_len = u64::from_le_bytes(encoded[24..32].try_into().unwrap()) as usize;
    assert_eq!(encoded.len(), HEADER_BYTES + payload_len + FOOTER_BYTES);
    assert_eq!(
        &encoded[32..64],
        Sha256::digest(&encoded[HEADER_BYTES..HEADER_BYTES + payload_len]).as_slice()
    );
    assert_eq!(
        &encoded[HEADER_BYTES + payload_len..HEADER_BYTES + payload_len + 8],
        FOOTER_MAGIC
    );

    let (generation, redundancy, loaded) = expect_loaded(store.load().expect("load"));
    assert_eq!(generation, 1);
    assert_eq!(redundancy, RecordRedundancy::Single);
    assert_eq!(loaded, value("first"));
    for debug in [
        format!("{store:?}"),
        format!("{receipt:?}"),
        format!("{:?}", store.load().expect("debug load")),
    ] {
        assert!(!debug.contains(root.path().to_string_lossy().as_ref()));
        assert!(!debug.contains("first"));
        assert!(!debug.contains('{'));
    }
}

#[test]
fn highest_valid_generation_wins_and_slots_alternate_without_a_third_file() {
    let (root, directory) = fixture();
    let store = record_store(&directory);
    for (expected_generation, label) in [(1, "one"), (2, "two"), (3, "three")] {
        let receipt = store.save(&value(label)).expect("alternating save");
        assert_eq!(receipt.generation(), expected_generation);
        let (generation, _, loaded) = expect_loaded(store.load().expect("selected load"));
        assert_eq!(generation, expected_generation);
        assert_eq!(loaded, value(label));
    }
    let mut names = fs::read_dir(root.path())
        .expect("slot directory")
        .map(|entry| entry.expect("entry").file_name())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names.len(), 2);
    assert_eq!(names[0], "settings-a.tms");
    assert_eq!(names[1], "settings-b.tms");
}

#[test]
fn newest_first_middle_and_footer_corruption_falls_back_to_older_slot() {
    for corruption in ["first", "middle", "footer"] {
        let (root, directory) = fixture();
        let store = record_store(&directory);
        store.save(&value("old")).expect("old");
        store.save(&value("new")).expect("new");
        let newest = root.path().join("settings-b.tms");
        let mut bytes = fs::read(&newest).expect("newest slot");
        let offset = match corruption {
            "first" => 0,
            "middle" => HEADER_BYTES + (bytes.len() - HEADER_BYTES - FOOTER_BYTES) / 2,
            "footer" => bytes.len() - 1,
            _ => unreachable!(),
        };
        bytes[offset] ^= 0x5a;
        fs::write(newest, bytes).expect("corrupt newest");

        let (generation, redundancy, loaded) = expect_loaded(store.load().expect("fallback"));
        assert_eq!(generation, 1);
        assert_eq!(redundancy, RecordRedundancy::Fallback);
        assert_eq!(loaded, value("old"));
    }
}

#[test]
fn every_header_truncation_and_two_invalid_slots_return_no_valid_record() {
    let boundaries = [0, 1, 7, 8, 9, 10, 11, 12, 15, 16, 23, 24, 31, 32, 63, 64];
    for length in boundaries {
        let (root, directory) = fixture();
        let store = record_store(&directory);
        let encoded = encode_raw(1, 1, br#"{"schema":1,"label":"ok"}"#);
        fs::write(root.path().join("settings-a.tms"), &encoded[..length]).expect("truncation");
        fs::write(root.path().join("settings-b.tms"), b"invalid").expect("invalid peer");
        assert!(matches!(
            store.load().expect("contained corruption"),
            RecordLoad::NoValidRecord
        ));
    }
}

#[test]
fn malformed_json_unknown_version_oversize_and_trailing_bytes_are_rejected() {
    let cases = [
        encode_raw(1, 1, br#"{"schema":1,"schema":2,"label":"duplicate"}"#),
        encode_raw(1, 1, b"\xff"),
        encode_raw(1, 2, br#"{"schema":1,"label":"future"}"#),
        encode_raw(1, 1, br#"{"schema":1,"label":"ok"}x"#),
    ];
    for encoded in cases {
        let (root, directory) = fixture();
        let store = record_store(&directory);
        fs::write(root.path().join("settings-a.tms"), encoded).expect("invalid record");
        assert!(matches!(
            store.load().expect("invalid slot is contained"),
            RecordLoad::NoValidRecord
        ));
    }

    let (root, directory) = fixture();
    let store = record_store(&directory);
    let oversized = vec![b'x'; 1025];
    fs::write(
        root.path().join("settings-a.tms"),
        encode_raw(1, 1, &oversized),
    )
    .expect("oversized record");
    assert!(matches!(
        store.load().expect("oversize is contained"),
        RecordLoad::NoValidRecord
    ));

    let (root, directory) = fixture();
    let store = record_store(&directory);
    let mut trailing = encode_raw(1, 1, br#"{"schema":1,"label":"ok"}"#);
    trailing.push(0);
    fs::write(root.path().join("settings-a.tms"), trailing).expect("trailing byte");
    assert!(matches!(
        store.load().expect("trailing is contained"),
        RecordLoad::NoValidRecord
    ));
}

#[test]
fn every_header_field_is_authoritative_even_with_a_resealed_footer() {
    let payload = br#"{"schema":1,"label":"valid-json"}"#;
    let base = encode_raw(1, 1, payload);
    let mut cases = Vec::new();

    let mut magic = base.clone();
    magic[0] ^= 1;
    cases.push(("magic", magic));

    let mut header_len = base.clone();
    header_len[10..12].copy_from_slice(&63_u16.to_le_bytes());
    cases.push(("header-length", header_len));

    let mut flags = base.clone();
    flags[12..16].copy_from_slice(&1_u32.to_le_bytes());
    cases.push(("reserved-flags", flags));

    let mut generation = base.clone();
    generation[16..24].copy_from_slice(&0_u64.to_le_bytes());
    cases.push(("zero-generation", generation));

    let mut payload_len = base.clone();
    payload_len[24..32].copy_from_slice(&((payload.len() as u64) + 1).to_le_bytes());
    cases.push(("payload-length", payload_len));

    let mut payload_digest = base;
    payload_digest[32..64].fill(0);
    cases.push(("payload-digest", payload_digest));

    for (field, mut encoded) in cases {
        reseal_raw(&mut encoded);
        let (root, directory) = fixture();
        let store = record_store(&directory);
        fs::write(root.path().join("settings-a.tms"), encoded).expect("mutated record");
        assert!(
            matches!(
                store.load().expect("invalid header is contained"),
                RecordLoad::NoValidRecord
            ),
            "field {field}"
        );
    }
}

#[test]
fn equal_generations_require_the_same_payload_digest() {
    let (root, directory) = fixture();
    let store = record_store(&directory);
    let identical = encode_raw(7, 1, br#"{"schema":1,"label":"same"}"#);
    fs::write(root.path().join("settings-a.tms"), &identical).expect("first identical slot");
    fs::write(root.path().join("settings-b.tms"), &identical).expect("second identical slot");
    let (generation, redundancy, loaded) = expect_loaded(store.load().expect("identical tie"));
    assert_eq!(generation, 7);
    assert_eq!(redundancy, RecordRedundancy::Complete);
    assert_eq!(loaded, value("same"));
    assert_eq!(
        store
            .save(&value("next"))
            .expect("advance tie")
            .generation(),
        8
    );

    let (root, directory) = fixture();
    let store = record_store(&directory);
    let first = encode_raw(7, 1, br#"{"schema":1,"label":"first"}"#);
    let second = encode_raw(7, 1, br#"{"schema":1,"label":"second"}"#);
    fs::write(root.path().join("settings-a.tms"), &first).expect("first conflicting slot");
    fs::write(root.path().join("settings-b.tms"), &second).expect("second conflicting slot");
    assert_eq!(
        store.load().expect_err("conflicting generation"),
        crate::StateError::from_code(StateErrorCode::Integrity)
    );
    assert_eq!(
        store
            .save(&value("must-not-repair"))
            .expect_err("ambiguous save")
            .code(),
        StateErrorCode::Integrity
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).expect("first unchanged"),
        first
    );
    assert_eq!(
        fs::read(root.path().join("settings-b.tms")).expect("second unchanged"),
        second
    );
}

#[test]
fn every_failure_after_publication_is_recovery_required() {
    let (root, directory) = fixture();
    let store = record_store(&directory);
    let error = store
        .save_with_hook(
            &value("published"),
            &mut BreakReadbackHook {
                root: root.path().to_owned(),
            },
        )
        .expect_err("post-publication readback failure");
    assert_eq!(error.code(), StateErrorCode::RecoveryRequired);
    assert!(root.path().join("published-preserved.tms").is_file());
}

#[test]
fn generation_overflow_and_invalid_existing_slots_write_nothing() {
    let (root, directory) = fixture();
    let store = record_store(&directory);
    let maximum = encode_raw(u64::MAX, 1, br#"{"schema":1,"label":"maximum"}"#);
    let slot = root.path().join("settings-a.tms");
    fs::write(&slot, &maximum).expect("maximum generation");
    let error = store.save(&value("must-not-write")).expect_err("overflow");
    assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    assert_eq!(fs::read(&slot).expect("unchanged maximum"), maximum);
    assert!(!root.path().join("settings-b.tms").exists());

    fs::write(root.path().join("settings-b.tms"), b"also-invalid").expect("invalid peer");
    fs::write(&slot, b"invalid").expect("invalid first");
    let before_a = fs::read(&slot).expect("before a");
    let before_b = fs::read(root.path().join("settings-b.tms")).expect("before b");
    let error = store
        .save(&value("must-not-repair"))
        .expect_err("ambiguous state");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(fs::read(&slot).expect("after a"), before_a);
    assert_eq!(
        fs::read(root.path().join("settings-b.tms")).expect("after b"),
        before_b
    );
}

#[test]
fn payload_limit_and_fixed_record_kinds_are_bounded() {
    let (_root, directory) = fixture();
    assert_eq!(
        RedundantRecordStore::<TestRecord>::new(
            &directory,
            RecordKind::Settings,
            MAX_RECORD_PAYLOAD_BYTES + 1,
        )
        .expect_err("absolute payload maximum"),
        crate::StateError::from_code(StateErrorCode::CapacityExceeded)
    );

    for kind in [
        RecordKind::Settings,
        RecordKind::RunState,
        RecordKind::RecoveryJournal,
    ] {
        let store = RedundantRecordStore::<TestRecord>::new(&directory, kind, 1024)
            .expect("fixed record kind");
        assert_eq!(format!("{store:?}"), "RedundantRecordStore([redacted])");
    }
}

#[test]
fn interrupted_record_save_child() {
    if std::env::var_os(CRASH_CHILD).is_none() {
        return;
    }
    CRASH_SERIALIZE_CALLS.store(0, Ordering::SeqCst);
    let root = PathBuf::from(std::env::var_os(CRASH_ROOT).expect("crash root"));
    let directory = ValidatedLocalDirectory::new(&root).expect("validated crash root");
    let phase = std::env::var(CRASH_PHASE).expect("crash phase");
    let store = crash_store(&directory);
    let next = CrashRecord {
        schema: 1,
        label: "new".to_owned(),
        padding: "x".repeat(256 * 1024),
    };
    let receipt = store
        .save_with_hook(
            &next,
            &mut CrashSaveHook {
                phase: phase.clone(),
            },
        )
        .expect("child save");
    assert_eq!(receipt.generation(), 3);
    panic!("crash phase returned without interruption: {phase}");
}

#[test]
fn process_death_before_flush_before_publish_and_after_publish_recovers_atomically() {
    for (phase, expected_generation, expected_label, expected_redundancy) in [
        ("before-flush", 2, "old-two", RecordRedundancy::Complete),
        ("before-publish", 2, "old-two", RecordRedundancy::Complete),
        ("after-publish", 3, "new", RecordRedundancy::Complete),
    ] {
        let (root, directory) = fixture();
        let store = crash_store(&directory);
        store
            .save(&CrashRecord {
                schema: 1,
                label: "old-one".to_owned(),
                padding: String::new(),
            })
            .expect("seed crash record");
        store
            .save(&CrashRecord {
                schema: 1,
                label: "old-two".to_owned(),
                padding: String::new(),
            })
            .expect("seed inactive crash slot");

        let marker = root.path().join(format!("{phase}.marker"));
        let executable = std::env::current_exe().expect("current test executable");
        let mut child = Command::new(executable)
            .args([
                "--exact",
                "record_contract_tests::interrupted_record_save_child",
                "--nocapture",
            ])
            .env(CRASH_CHILD, "1")
            .env(CRASH_ROOT, root.path())
            .env(CRASH_PHASE, phase)
            .env(CRASH_MARKER, &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn interrupted save child");

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if marker.exists() {
                break;
            }
            if let Some(status) = child.try_wait().expect("poll crash child") {
                panic!("crash child exited before marker for {phase}: {status}");
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("crash child did not reach {phase}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill().expect("kill crash child");
        child.wait().expect("reap crash child");

        let loaded = match store.load().expect("load after process death") {
            RecordLoad::Loaded(loaded) => loaded,
            RecordLoad::NoValidRecord => panic!("lost both records after {phase}"),
        };
        assert_eq!(loaded.generation(), expected_generation, "phase {phase}");
        assert_eq!(loaded.redundancy(), expected_redundancy, "phase {phase}");
        assert_eq!(loaded.into_value().label, expected_label, "phase {phase}");
    }
}

#[test]
fn oversized_or_nondeterministic_serialization_never_publishes_a_record() {
    let (root, directory) = fixture();
    let small_store = RedundantRecordStore::<TestRecord>::new(&directory, RecordKind::Settings, 16)
        .expect("small record store");
    let error = small_store
        .save(&value("larger-than-the-record-limit"))
        .expect_err("bounded serialization");
    assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    assert!(
        fs::read_dir(root.path())
            .expect("empty root")
            .next()
            .is_none()
    );

    CHANGING_SERIALIZE_CALLS.store(0, Ordering::SeqCst);
    let changing_store =
        RedundantRecordStore::<ChangingRecord>::new(&directory, RecordKind::Settings, 1024)
            .expect("changing record store");
    let error = changing_store
        .save(&ChangingRecord {
            schema: 1,
            label: "a".to_owned(),
        })
        .expect_err("two serialization passes must agree");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert!(
        fs::read_dir(root.path())
            .expect("empty root")
            .next()
            .is_none()
    );
}
