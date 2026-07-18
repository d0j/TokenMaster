#![allow(dead_code)]

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
        std::fs::create_dir(&stage_path).expect("replace stage path with directory");
        std::fs::write(stage_path.join("blocker"), b"not removable as a file")
            .expect("make replacement directory nonempty");
    }
}

pub fn settings() -> PortableSettingsCandidate {
    PortableSettingsCandidate::new(SettingsValue::safe_defaults().portable().clone())
        .expect("portable settings")
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
