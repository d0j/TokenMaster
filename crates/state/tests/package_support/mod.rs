#![allow(dead_code)]

use std::io::Write;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{
    DurableFileReader, DurableFileTarget, DurableStagedFile, MAX_DURABLE_FILE_BYTES,
    MAX_DURABLE_WRITE_CHUNK_BYTES, ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BackupCompression, BackupMetadata, BackupPackage, BackupPurpose, ConfigPackage, PackageReceipt,
    PortableSettingsCandidate, SettingsValue, StateError, VerifiedBackupPackage,
    VerifiedConfigPackage,
};

pub const PACKAGE_MAX_BYTES: u64 = MAX_DURABLE_FILE_BYTES;

pub struct ControlledRoot {
    _root: TempDir,
    directory: ValidatedLocalDirectory,
}

impl ControlledRoot {
    pub fn new() -> Self {
        let root = TempDir::new().expect("temporary package root");
        let directory = ValidatedLocalDirectory::new(root.path()).expect("validated package root");
        Self {
            _root: root,
            directory,
        }
    }

    pub fn target(&self, name: &str) -> DurableFileTarget {
        DurableFileTarget::exact_child(&self.directory, name).expect("exact package child")
    }

    pub fn publish_bytes(&self, name: &str, bytes: &[u8]) -> DurableFileTarget {
        let target = self.target(name);
        let mut staged = target
            .create_staged(PACKAGE_MAX_BYTES)
            .expect("create package fixture stage");
        for chunk in bytes.chunks(MAX_DURABLE_WRITE_CHUNK_BYTES) {
            staged.write_chunk(chunk).expect("write package fixture");
        }
        staged
            .seal(bytes.len() as u64, digest(bytes))
            .expect("seal package fixture");
        staged
            .publish_new(&target)
            .expect("publish package fixture");
        target
    }

    pub fn open(&self, target: &DurableFileTarget) -> DurableFileReader {
        target
            .open_reader(PACKAGE_MAX_BYTES)
            .expect("open controlled reader")
            .expect("controlled file exists")
    }

    pub fn stage(&self, name: &str, max_bytes: u64) -> (DurableFileTarget, DurableStagedFile) {
        let target = self.target(name);
        let staged = target
            .create_staged(max_bytes)
            .expect("create output stage");
        (target, staged)
    }

    pub fn read_bytes(&self, target: &DurableFileTarget) -> Vec<u8> {
        target
            .read_bounded(PACKAGE_MAX_BYTES)
            .expect("read controlled bytes")
            .expect("controlled bytes exist")
    }

    pub fn append_child(&self, name: &str, bytes: &[u8]) {
        use std::io::Write as _;

        let path = self.directory.as_path().join(name);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open controlled child for append");
        file.write_all(bytes).expect("append controlled child");
    }

    pub fn unpublished_stage_count(&self) -> usize {
        std::fs::read_dir(self.directory.as_path())
            .expect("enumerate controlled test root")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".tokenmaster-stage-")
            })
            .count()
    }

    pub fn sabotage_only_stage_cleanup(&self) {
        let stages = std::fs::read_dir(self.directory.as_path())
            .expect("enumerate controlled test root")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".tokenmaster-stage-")
            })
            .collect::<Vec<_>>();
        assert_eq!(stages.len(), 1, "one stage required for cleanup sabotage");
        let stage_path = stages[0].path();
        let held_path = self.directory.as_path().join("held-stage-bytes");
        std::fs::rename(&stage_path, held_path).expect("move open stage fixture");
        let linked_path = self.directory.as_path().join("linked-stage-bytes");
        std::fs::hard_link(
            self.directory.as_path().join("held-stage-bytes"),
            linked_path,
        )
        .expect("make exact stage cleanup identity ambiguous");
        std::fs::create_dir(&stage_path).expect("replace stage path with directory");
        std::fs::write(stage_path.join("blocker"), b"not removable as a file")
            .expect("make replacement directory nonempty");
    }
}

pub fn settings() -> PortableSettingsCandidate {
    PortableSettingsCandidate::new(SettingsValue::safe_defaults().portable().clone())
        .expect("portable settings")
}

pub fn legacy_v1_portable_json() -> Vec<u8> {
    br#"{"schema_version":1,"portable":{"reminders":{"enabled":true,"lead_seconds":[604800,86400,43200,21600,3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648}}}"#.to_vec()
}

pub fn legacy_config_bytes_v1() -> Vec<u8> {
    package_with_settings_source_schema(1, &legacy_v1_portable_json(), None)
}

pub fn legacy_backup_bytes_v1(database: &[u8]) -> Vec<u8> {
    package_with_settings_source_schema(1, &legacy_v1_portable_json(), Some(database))
}

pub fn package_with_settings_source_schema(
    settings_schema_version: u16,
    settings_json: &[u8],
    database: Option<&[u8]>,
) -> Vec<u8> {
    const HEADER_BYTES: usize = 32;
    const MANIFEST_BYTES: usize = 40;
    const ENTRY_PREFIX_BYTES: usize = 64;
    const ENTRY_SUFFIX_BYTES: usize = 24;
    const PACKAGE_WINDOW_LOG: u32 = 23;
    const PACKAGE_TIME: i64 = 1_721_234_567_890;

    let settings: serde_json::Value =
        serde_json::from_slice(settings_json).expect("strict settings JSON fixture");
    assert!(matches!(settings_schema_version, 1 | 2));
    assert!(matches!(settings["schema_version"].as_u64(), Some(1 | 2)));
    assert!(!settings_json.is_empty());

    let (kind, entry_count, database_schema_version, backup_purpose) = match database {
        Some(_) => (2_u8, 2_u8, 13_u16, 2_u8),
        None => (1_u8, 1_u8, 0_u16, 0_u8),
    };
    let total_expanded = settings_json
        .len()
        .checked_add(database.map_or(0, <[u8]>::len))
        .expect("bounded fixture length");
    let mut bytes = Vec::new();

    let mut header = [0_u8; HEADER_BYTES];
    header[0..8].copy_from_slice(b"TMPKG001");
    header[8..10].copy_from_slice(&1_u16.to_le_bytes());
    header[10..12].copy_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
    header[12] = kind;
    header[13] = entry_count;
    header[16..20].copy_from_slice(&(MANIFEST_BYTES as u32).to_le_bytes());
    header[20..28].copy_from_slice(&(total_expanded as u64).to_le_bytes());
    bytes.extend_from_slice(&header);
    assert_eq!(bytes.len(), HEADER_BYTES);

    let mut manifest = [0_u8; MANIFEST_BYTES];
    manifest[0..8].copy_from_slice(b"TMMNF001");
    manifest[8..10].copy_from_slice(&1_u16.to_le_bytes());
    manifest[10..12].copy_from_slice(&(MANIFEST_BYTES as u16).to_le_bytes());
    manifest[12] = kind;
    manifest[13] = entry_count;
    manifest[14..16].copy_from_slice(&settings_schema_version.to_le_bytes());
    manifest[16..18].copy_from_slice(&database_schema_version.to_le_bytes());
    manifest[18] = 2;
    manifest[19] = backup_purpose;
    manifest[20..28].copy_from_slice(&PACKAGE_TIME.to_le_bytes());
    bytes.extend_from_slice(&manifest);
    assert_eq!(bytes.len(), HEADER_BYTES + MANIFEST_BYTES);
    assert_eq!(
        u16::from_le_bytes(bytes[46..48].try_into().unwrap()),
        settings_schema_version
    );

    let mut descriptor_hasher = Sha256::new();
    descriptor_hasher.update(manifest);
    append_fixture_entry(
        &mut bytes,
        &mut descriptor_hasher,
        1,
        settings_json,
        PACKAGE_WINDOW_LOG,
    );
    if let Some(database) = database {
        append_fixture_entry(
            &mut bytes,
            &mut descriptor_hasher,
            2,
            database,
            PACKAGE_WINDOW_LOG,
        );
    }

    let binding: [u8; 32] = descriptor_hasher.finalize().into();
    let binding_offset = bytes.len();
    bytes.extend_from_slice(&binding);
    assert_eq!(bytes.len(), binding_offset + 32);
    bytes.extend_from_slice(b"TMEND001");
    assert_eq!(
        &bytes[binding_offset + 32..binding_offset + 40],
        b"TMEND001"
    );
    let package_digest: [u8; 32] = Sha256::digest(&bytes).into();
    bytes.extend_from_slice(&package_digest);
    assert_eq!(bytes.len(), binding_offset + 72);
    bytes
}

fn append_fixture_entry(
    bytes: &mut Vec<u8>,
    descriptor_hasher: &mut Sha256,
    kind: u8,
    source: &[u8],
    window_log: u32,
) {
    const ENTRY_PREFIX_BYTES: usize = 64;
    const ENTRY_SUFFIX_BYTES: usize = 24;

    let prefix_offset = bytes.len();
    let mut prefix = [0_u8; ENTRY_PREFIX_BYTES];
    prefix[0..8].copy_from_slice(b"TMENTR01");
    prefix[8] = kind;
    prefix[9] = 1;
    prefix[10] = 2;
    prefix[11] = 3;
    prefix[12..16].copy_from_slice(&(ENTRY_PREFIX_BYTES as u32).to_le_bytes());
    prefix[16..24].copy_from_slice(&(source.len() as u64).to_le_bytes());
    prefix[24..56].copy_from_slice(&Sha256::digest(source));
    prefix[56..60].copy_from_slice(&window_log.to_le_bytes());
    bytes.extend_from_slice(&prefix);
    descriptor_hasher.update(prefix);
    assert_eq!(bytes.len(), prefix_offset + ENTRY_PREFIX_BYTES);

    let mut frame = Vec::new();
    {
        let mut encoder =
            zstd::stream::write::Encoder::new(&mut frame, 12).expect("fixture zstd encoder");
        encoder.include_checksum(true).expect("fixture checksum");
        encoder
            .include_contentsize(true)
            .expect("fixture content size");
        encoder
            .long_distance_matching(false)
            .expect("fixture no long-distance matching");
        encoder.window_log(window_log).expect("fixture window log");
        encoder
            .set_pledged_src_size(Some(source.len() as u64))
            .expect("fixture pledged source size");
        encoder.write_all(source).expect("fixture source bytes");
        encoder.finish().expect("finish fixture frame");
    }
    assert!(!frame.is_empty());
    bytes.extend_from_slice(&frame);

    let mut suffix = [0_u8; ENTRY_SUFFIX_BYTES];
    suffix[0..8].copy_from_slice(b"TMENEND1");
    suffix[8..16].copy_from_slice(&(frame.len() as u64).to_le_bytes());
    suffix[16..24].copy_from_slice(&(source.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&suffix);
    descriptor_hasher.update(suffix);
    assert_eq!(
        bytes.len(),
        prefix_offset + ENTRY_PREFIX_BYTES + frame.len() + ENTRY_SUFFIX_BYTES
    );
}

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn config_bytes_at(created_at_utc_ms: i64) -> (Vec<u8>, PackageReceipt) {
    let root = ControlledRoot::new();
    let (target, mut staged) = root.stage("settings.tmconfig", PACKAGE_MAX_BYTES);
    let receipt = ConfigPackage::write(&settings(), created_at_utc_ms, &mut staged)
        .expect("write config package");
    staged.publish_new(&target).expect("publish config package");
    (root.read_bytes(&target), receipt)
}

pub fn config_bytes() -> Vec<u8> {
    config_bytes_at(1_721_234_567_890).0
}

pub fn read_config_bytes(bytes: &[u8]) -> Result<VerifiedConfigPackage, StateError> {
    let root = ControlledRoot::new();
    let target = root.publish_bytes("input.tmconfig", bytes);
    let mut reader = root.open(&target);
    ConfigPackage::read(&mut reader)
}

pub fn backup_bytes_with(
    database: &[u8],
    compression: BackupCompression,
    purpose: BackupPurpose,
) -> (Vec<u8>, PackageReceipt) {
    backup_bytes_at(database, compression, purpose, 1_721_234_567_890)
}

pub fn backup_bytes_at(
    database: &[u8],
    compression: BackupCompression,
    purpose: BackupPurpose,
    created_at_utc_ms: i64,
) -> (Vec<u8>, PackageReceipt) {
    let root = ControlledRoot::new();
    let database_target = root.publish_bytes("snapshot.sqlite3", database);
    let mut database_reader = root.open(&database_target);
    let (package_target, mut package_stage) = root.stage("backup.tmbackup", PACKAGE_MAX_BYTES);
    let receipt = BackupPackage::write(
        &settings(),
        &mut database_reader,
        database.len() as u64,
        digest(database),
        13,
        compression,
        BackupMetadata::new(created_at_utc_ms, purpose).expect("backup metadata"),
        &mut package_stage,
    )
    .expect("write backup package");
    package_stage
        .publish_new(&package_target)
        .expect("publish backup package");
    (root.read_bytes(&package_target), receipt)
}

pub fn backup_bytes() -> Vec<u8> {
    backup_bytes_with(
        b"SQLite format 3\0adversarial fixture",
        BackupCompression::Normal,
        BackupPurpose::Manual,
    )
    .0
}

pub fn read_backup_bytes(bytes: &[u8]) -> Result<(VerifiedBackupPackage, Vec<u8>), StateError> {
    let root = ControlledRoot::new();
    let source_target = root.publish_bytes("input.tmbackup", bytes);
    let mut source = root.open(&source_target);
    let (database_target, mut database_stage) = root.stage("restored.sqlite3", PACKAGE_MAX_BYTES);
    let verified = BackupPackage::read(&mut source, &mut database_stage)?;
    database_stage
        .publish_new(&database_target)
        .map_err(|_| StateError::from_code(tokenmaster_state::StateErrorCode::Unavailable))?;
    Ok((verified, root.read_bytes(&database_target)))
}
