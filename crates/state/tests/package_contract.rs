#![allow(clippy::expect_used, clippy::unwrap_used)]

mod package_support;

use sha2::{Digest, Sha256};
use tokenmaster_state::{
    BackupCompression, BackupMetadata, BackupPackage, BackupPurpose, MAX_CONFIG_PACKAGE_BYTES,
    MAX_DATABASE_PACKAGE_BYTES, MAX_PACKAGE_ENTRIES, MAX_PACKAGE_MANIFEST_BYTES,
    MAX_PACKAGE_TOTAL_EXPANDED_BYTES, MAX_SETTINGS_PACKAGE_BYTES, PACKAGE_DECODER_WINDOW_BYTES,
    PACKAGE_IO_BUFFER_BYTES, StateErrorCode,
};

use package_support::{
    ControlledRoot, PACKAGE_MAX_BYTES, backup_bytes_with, config_bytes_at, digest,
    read_backup_bytes, read_config_bytes, settings,
};

const PACKAGE_TIME: i64 = 1_721_234_567_890;

#[test]
fn v1_config_golden_vector_is_deterministic_typed_and_round_trips() {
    let settings = settings();
    let (first, first_receipt) = config_bytes_at(PACKAGE_TIME);
    let (second, second_receipt) = config_bytes_at(PACKAGE_TIME);

    assert_eq!(first, second);
    assert_eq!(first_receipt, second_receipt);
    assert_eq!(&first[0..8], b"TMPKG001");
    assert_eq!(u16::from_le_bytes(first[8..10].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(first[10..12].try_into().unwrap()), 32);
    assert_eq!(first[12], 1, "config package kind");
    assert_eq!(first[13], 1, "one settings entry");
    assert_eq!(&first[32..40], b"TMMNF001");
    assert_eq!(u16::from_le_bytes(first[40..42].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(first[42..44].try_into().unwrap()), 40);
    assert_eq!(
        i64::from_le_bytes(first[52..60].try_into().unwrap()),
        PACKAGE_TIME
    );
    assert_eq!(&first[72..80], b"TMENTR01");
    assert_eq!(first[80], 1, "settings entry kind");
    assert_eq!(first[81], 1, "zstd codec");
    assert_eq!(first[82], 2, "normal compression profile");
    assert_eq!(first[83], 3, "checksum and content-size flags");
    assert_eq!(u32::from_le_bytes(first[84..88].try_into().unwrap()), 64);
    assert_eq!(&first[136..140], &[0x28, 0xb5, 0x2f, 0xfd]);
    assert_ne!(first[140] & 0b0000_0100, 0, "frame checksum flag");
    assert_eq!(&first[first.len() - 40..first.len() - 32], b"TMEND001");
    assert_eq!(first_receipt.package_len(), first.len() as u64);
    assert_eq!(
        first_receipt.package_sha256(),
        &digest(&first[..first.len() - 32])
    );

    // Frozen v1 compatibility vector: changing either value requires a new format decision.
    assert_eq!(first.len(), 405, "frozen v1 golden length");
    assert_eq!(
        digest(&first),
        [
            82, 69, 18, 205, 24, 116, 131, 70, 102, 53, 114, 15, 232, 148, 11, 35, 178, 26, 7, 94,
            87, 173, 178, 143, 128, 33, 62, 53, 155, 128, 244, 153,
        ],
        "frozen v1 golden SHA-256"
    );

    let verified = read_config_bytes(&first).expect("read config");
    assert_eq!(verified.settings().digest(), settings.digest());
    assert_eq!(verified.created_at_utc_ms(), PACKAGE_TIME);
    assert_eq!(verified.receipt(), first_receipt);
    assert!(read_config_bytes(&first[..first.len() - 1]).is_err());
}

#[test]
fn backup_vector_round_trips_every_allowed_profile_and_purpose() {
    let settings = settings();
    let database = b"SQLite format 3\0bounded synthetic database payload";

    for profile in [
        BackupCompression::Automatic,
        BackupCompression::Normal,
        BackupCompression::Compact,
    ] {
        for purpose in [
            BackupPurpose::Periodic,
            BackupPurpose::Manual,
            BackupPurpose::PreMigration,
            BackupPurpose::PostMigration,
            BackupPurpose::PreRestore,
            BackupPurpose::PreDestructiveMaintenance,
        ] {
            let (encoded, _) = backup_bytes_with(database, profile, purpose);
            let (verified, restored) = read_backup_bytes(&encoded).expect("read backup");
            assert_eq!(verified.settings().digest(), settings.digest());
            assert_eq!(verified.database_schema_version(), 13);
            assert_eq!(verified.database_len(), database.len() as u64);
            assert_eq!(verified.database_sha256(), &digest(database));
            assert_eq!(verified.compression(), profile);
            assert_eq!(verified.metadata().created_at_utc_ms(), PACKAGE_TIME);
            assert_eq!(verified.metadata().purpose(), purpose);
            assert_eq!(restored, database);
            assert!(read_config_bytes(&encoded).is_err());
        }
    }
}

#[test]
fn hard_bounds_and_profiles_are_exact() {
    assert_eq!(MAX_PACKAGE_ENTRIES, 8);
    assert_eq!(MAX_PACKAGE_MANIFEST_BYTES, 64 * 1024);
    assert_eq!(MAX_SETTINGS_PACKAGE_BYTES, 1024 * 1024);
    assert_eq!(MAX_CONFIG_PACKAGE_BYTES, 2 * 1024 * 1024);
    assert_eq!(MAX_DATABASE_PACKAGE_BYTES, 64 * 1024 * 1024 * 1024);
    assert_eq!(
        MAX_PACKAGE_TOTAL_EXPANDED_BYTES,
        (64 * 1024 + 2) * 1024 * 1024
    );
    assert_eq!(PACKAGE_DECODER_WINDOW_BYTES, 8 * 1024 * 1024);
    assert_eq!(PACKAGE_IO_BUFFER_BYTES, 64 * 1024);
    assert_eq!(BackupCompression::Automatic.level(), 6);
    assert_eq!(BackupCompression::Normal.level(), 12);
    assert_eq!(BackupCompression::Compact.level(), 19);
    assert!(BackupMetadata::new(-1, BackupPurpose::Periodic).is_err());
}

#[test]
fn config_reader_rejects_the_encoded_ceiling_before_parsing() {
    let oversized = vec![0_u8; (MAX_CONFIG_PACKAGE_BYTES + 1) as usize];
    assert_eq!(
        read_config_bytes(&oversized)
            .expect_err("oversized config")
            .code(),
        StateErrorCode::CapacityExceeded
    );
}

#[test]
fn large_database_is_streamed_between_controlled_files() {
    const DATABASE_BYTES: u64 = 24 * 1024 * 1024;
    let root = ControlledRoot::new();
    let chunk = [0x5a_u8; 64 * 1024];
    let mut expected_hasher = Sha256::new();
    let (database_target, mut database_stage) = root.stage("large.sqlite3", DATABASE_BYTES);
    for _ in 0..DATABASE_BYTES / chunk.len() as u64 {
        database_stage
            .write_chunk(&chunk)
            .expect("stream database chunk");
        expected_hasher.update(chunk);
    }
    let expected_digest: [u8; 32] = expected_hasher.finalize().into();
    database_stage
        .seal(DATABASE_BYTES, expected_digest)
        .expect("seal database");
    database_stage
        .publish_new(&database_target)
        .expect("publish database");

    let mut database_reader = root.open(&database_target);
    let (package_target, mut package_stage) = root.stage("large.tmbackup", PACKAGE_MAX_BYTES);
    BackupPackage::write(
        &settings(),
        &mut database_reader,
        DATABASE_BYTES,
        expected_digest,
        13,
        BackupCompression::Automatic,
        BackupMetadata::new(PACKAGE_TIME, BackupPurpose::Periodic).expect("backup metadata"),
        &mut package_stage,
    )
    .expect("stream backup");
    package_stage
        .publish_new(&package_target)
        .expect("publish package");
    assert!(
        root.read_bytes(&package_target).len() < 64 * 1024,
        "compressible input stays compact"
    );

    let mut package_reader = root.open(&package_target);
    let (restored_target, mut restored_stage) = root.stage("restored.sqlite3", DATABASE_BYTES);
    let verified =
        BackupPackage::read(&mut package_reader, &mut restored_stage).expect("stream restore");
    restored_stage
        .publish_new(&restored_target)
        .expect("publish restored database");
    let mut restored_reader = root.open(&restored_target);
    let mut restored_hasher = Sha256::new();
    let mut restored_len = 0_u64;
    let mut buffer = [0_u8; PACKAGE_IO_BUFFER_BYTES];
    loop {
        let count = restored_reader
            .read_chunk(&mut buffer)
            .expect("read restored chunk");
        if count == 0 {
            break;
        }
        restored_len += count as u64;
        restored_hasher.update(&buffer[..count]);
    }
    assert_eq!(restored_len, DATABASE_BYTES);
    assert_eq!(restored_hasher.finalize().as_slice(), expected_digest);
    assert_eq!(verified.database_len(), DATABASE_BYTES);
}

#[test]
fn controlled_stages_cannot_publish_after_writer_or_reader_failure() {
    let root = ControlledRoot::new();
    let source_target = root.publish_bytes("wrong-source.sqlite3", &[7_u8; 65]);
    let mut source = root.open(&source_target);
    let (package_target, mut package_stage) = root.stage("failed.tmbackup", PACKAGE_MAX_BYTES);
    let error = BackupPackage::write(
        &settings(),
        &mut source,
        64,
        [0_u8; 32],
        13,
        BackupCompression::Automatic,
        BackupMetadata::new(PACKAGE_TIME, BackupPurpose::Periodic).expect("backup metadata"),
        &mut package_stage,
    )
    .expect_err("declared source length mismatch");
    assert_eq!(error.code(), tokenmaster_state::StateErrorCode::Integrity);
    assert_eq!(
        package_stage
            .seal(0, digest(b""))
            .expect_err("failed writer stage is poisoned"),
        tokenmaster_platform::DurableFileError::InvalidState
    );
    assert_eq!(
        package_stage
            .publish_new(&package_target)
            .expect_err("failed package must remain unsealed"),
        tokenmaster_platform::DurableFileError::InvalidState
    );

    let database = b"SQLite format 3\0late-footer fixture";
    let mut corrupted =
        backup_bytes_with(database, BackupCompression::Normal, BackupPurpose::Manual).0;
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0xff;
    let corrupt_target = root.publish_bytes("corrupt.tmbackup", &corrupted);
    let mut corrupt_reader = root.open(&corrupt_target);
    let (restore_target, mut restore_stage) = root.stage("failed-restore.sqlite3", 1024);
    BackupPackage::read(&mut corrupt_reader, &mut restore_stage)
        .expect_err("corrupt package must fail");
    assert_eq!(
        restore_stage.written_len(),
        database.len() as u64,
        "database was fully extracted before late footer rejection"
    );
    assert_eq!(
        restore_stage
            .seal(database.len() as u64, digest(database))
            .expect_err("late-failure restore stage is poisoned"),
        tokenmaster_platform::DurableFileError::InvalidState
    );
    assert_eq!(
        restore_stage
            .publish_new(&restore_target)
            .expect_err("failed restore must remain unsealed"),
        tokenmaster_platform::DurableFileError::InvalidState
    );
}
