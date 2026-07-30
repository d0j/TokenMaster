#![allow(clippy::expect_used, clippy::unwrap_used)]

mod package_support;

use std::io::Write as _;
use std::time::{Duration, Instant};

use tokenmaster_state::{
    AGE_SCRYPT_LOG_N, BackupEncryptionContext, BackupPassphrase, EncryptedBackupPackage,
    MAX_BACKUP_PASSPHRASE_SCALARS, MIN_BACKUP_PASSPHRASE_SCALARS, StateErrorCode,
};

use package_support::{ControlledRoot, PACKAGE_MAX_BYTES, backup_bytes, digest, read_backup_bytes};

const PASSPHRASE: &str = "correct horse battery staple 42";
const OTHER_PASSPHRASE: &str = "wrong horse battery staple 99";

fn new_passphrase(value: &str) -> BackupPassphrase {
    let mut input = value.to_owned();
    let mut confirmation = value.to_owned();
    let secret = BackupPassphrase::new(&mut input, &mut confirmation).expect("valid passphrase");
    assert!(input.is_empty());
    assert!(confirmation.is_empty());
    secret
}

fn existing_passphrase(value: &str) -> BackupPassphrase {
    let mut input = value.to_owned();
    let secret = BackupPassphrase::existing(&mut input).expect("valid existing passphrase");
    assert!(input.is_empty());
    secret
}

fn encrypted_backup() -> Vec<u8> {
    let root = ControlledRoot::new();
    let plaintext = backup_bytes();
    let (verified, _) = read_backup_bytes(&plaintext).expect("verify manual backup");
    let source_target = root.publish_bytes("manual.tmbackup", &plaintext);
    let mut source = root.open(&source_target);
    let (target, mut stage) = root.stage("manual.tmbackup.age", PACKAGE_MAX_BYTES);
    let receipt = EncryptedBackupPackage::encrypt(
        BackupEncryptionContext::ManualExport,
        &mut source,
        &verified,
        new_passphrase(PASSPHRASE),
        &mut stage,
    )
    .expect("encrypt manual backup");
    assert_eq!(receipt.output_len(), stage.written_len());
    stage
        .publish_new(&target)
        .expect("publish encrypted backup");
    let encoded = root.read_bytes(&target);
    assert_eq!(receipt.output_sha256(), &digest(&encoded));
    encoded
}

fn decrypt_bytes(
    encoded: &[u8],
    passphrase: &str,
) -> Result<(Vec<u8>, tokenmaster_state::VerifiedBackupPackage), StateErrorCode> {
    let root = ControlledRoot::new();
    let source_target = root.publish_bytes("input.tmbackup.age", encoded);
    let mut source = root.open(&source_target);
    let (target, mut stage) = root.stage("decrypted.sqlite3", PACKAGE_MAX_BYTES);
    let verified =
        EncryptedBackupPackage::decrypt(&mut source, existing_passphrase(passphrase), &mut stage)
            .map_err(|error| error.code())?;
    stage
        .publish_new(&target)
        .map_err(|_| StateErrorCode::Unavailable)?;
    Ok((root.read_bytes(&target), verified))
}

fn encrypt_arbitrary_age_bytes(plaintext: &[u8]) -> Vec<u8> {
    let mut recipient =
        age::scrypt::Recipient::new(age::secrecy::SecretString::from(PASSPHRASE.to_owned()));
    recipient.set_work_factor(AGE_SCRYPT_LOG_N);
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .expect("one scrypt recipient");
    let mut encoded = Vec::new();
    let mut writer = encryptor.wrap_output(&mut encoded).expect("age header");
    writer.write_all(plaintext).expect("age plaintext");
    writer.finish().expect("age final tag");
    encoded
}

#[test]
fn age_v1_manual_export_round_trips_one_complete_backup() {
    let plaintext = backup_bytes();
    let (expected, expected_database) =
        read_backup_bytes(&plaintext).expect("verify expected backup");
    let encoded = encrypted_backup();

    assert!(encoded.starts_with(b"age-encryption.org/v1\n"));
    assert_ne!(encoded, plaintext);
    let (database, verified) = decrypt_bytes(&encoded, PASSPHRASE).expect("decrypt backup");
    assert_eq!(database, expected_database);
    assert_eq!(verified.receipt(), expected.receipt());
    assert_eq!(verified.settings().digest(), expected.settings().digest());
    assert_eq!(verified.database_len(), expected.database_len());
    assert_eq!(verified.database_sha256(), expected.database_sha256());
    assert_eq!(verified.compression(), expected.compression());
    assert_eq!(verified.metadata(), expected.metadata());
}

#[test]
fn export_pins_scrypt_16_and_import_rejects_more_before_derivation() {
    assert_eq!(AGE_SCRYPT_LOG_N, 16);
    let encoded = encrypted_backup();
    let header_end = encoded
        .windows(4)
        .position(|window| window == b"--- ")
        .expect("age header mac");
    let header = std::str::from_utf8(&encoded[..header_end]).expect("text age header");
    assert!(header.lines().any(|line| {
        line.starts_with("-> scrypt ") && line.ends_with(&format!(" {AGE_SCRYPT_LOG_N}"))
    }));

    let marker = b" 16\n";
    let position = encoded
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("scrypt work factor");
    let mut malicious = encoded;
    malicious[position + 1..position + 3].copy_from_slice(b"63");
    let started = Instant::now();
    assert_eq!(
        decrypt_bytes(&malicious, PASSPHRASE).unwrap_err(),
        StateErrorCode::CapacityExceeded
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "attacker-selected scrypt work must be rejected before derivation"
    );
}

#[test]
fn wrong_password_and_every_outer_integrity_failure_poison_database_stage() {
    let encoded = encrypted_backup();
    let mut corrupt_header = encoded.clone();
    corrupt_header[0] ^= 1;
    let mut corrupt_header_mac = encoded.clone();
    let mac = corrupt_header_mac
        .windows(4)
        .position(|window| window == b"--- ")
        .expect("header mac");
    corrupt_header_mac[mac + 4] ^= 1;
    let mut corrupt_body = encoded.clone();
    let middle = corrupt_body.len() / 2;
    corrupt_body[middle] ^= 1;
    let mut corrupt_footer = encoded.clone();
    let last = corrupt_footer.len() - 1;
    corrupt_footer[last] ^= 1;
    let truncated = &encoded[..encoded.len() - 1];
    let mut trailing = encoded.clone();
    trailing.extend_from_slice(b"not-an-age-continuation");

    let cases: [(&str, &[u8], &str); 7] = [
        ("wrong-password", &encoded, OTHER_PASSPHRASE),
        ("header", &corrupt_header, PASSPHRASE),
        ("header-mac", &corrupt_header_mac, PASSPHRASE),
        ("body", &corrupt_body, PASSPHRASE),
        ("footer", &corrupt_footer, PASSPHRASE),
        ("truncated", truncated, PASSPHRASE),
        ("trailing", &trailing, PASSPHRASE),
    ];

    for (name, bytes, passphrase) in cases {
        let root = ControlledRoot::new();
        let source_target = root.publish_bytes("input.tmbackup.age", bytes);
        let mut source = root.open(&source_target);
        let (target, mut stage) = root.stage("plaintext.tmbackup", PACKAGE_MAX_BYTES);
        let error = EncryptedBackupPackage::decrypt(
            &mut source,
            existing_passphrase(passphrase),
            &mut stage,
        )
        .expect_err(name);
        assert!(
            matches!(
                error.code(),
                StateErrorCode::Integrity | StateErrorCode::CapacityExceeded
            ),
            "{name}: {error:?}"
        );
        assert_eq!(root.unpublished_stage_count(), 0, "{name}");
        assert!(stage.seal(0, digest(b"")).is_err(), "{name}");
        assert!(stage.publish_new(&target).is_err(), "{name}");
    }
}

#[test]
fn authenticated_non_backup_plaintext_never_becomes_publishable() {
    let encoded = encrypt_arbitrary_age_bytes(b"authenticated but not a TokenMaster package");
    let root = ControlledRoot::new();
    let source_target = root.publish_bytes("arbitrary.tmbackup.age", &encoded);
    let mut source = root.open(&source_target);
    let (target, mut stage) = root.stage("arbitrary.sqlite3", PACKAGE_MAX_BYTES);
    let error =
        EncryptedBackupPackage::decrypt(&mut source, existing_passphrase(PASSPHRASE), &mut stage)
            .expect_err("inner typed package validation is mandatory");
    assert!(matches!(
        error.code(),
        StateErrorCode::Integrity | StateErrorCode::InvalidInput
    ));
    assert_eq!(root.unpublished_stage_count(), 0);
    assert!(stage.publish_new(&target).is_err());
}

#[test]
fn destination_capacity_and_cleanup_failure_are_fail_closed() {
    let plaintext = backup_bytes();
    let (verified, _) = read_backup_bytes(&plaintext).expect("verify source backup");

    let encrypt_root = ControlledRoot::new();
    let source_target = encrypt_root.publish_bytes("source.tmbackup", &plaintext);
    let mut source = encrypt_root.open(&source_target);
    let (target, mut stage) = encrypt_root.stage("small.tmbackup.age", 16);
    let error = EncryptedBackupPackage::encrypt(
        BackupEncryptionContext::ManualExport,
        &mut source,
        &verified,
        new_passphrase(PASSPHRASE),
        &mut stage,
    )
    .expect_err("ciphertext capacity is bounded");
    assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    assert_eq!(encrypt_root.unpublished_stage_count(), 0);
    assert!(stage.publish_new(&target).is_err());

    let encoded = encrypted_backup();
    let decrypt_root = ControlledRoot::new();
    let encrypted_target = decrypt_root.publish_bytes("source.tmbackup.age", &encoded);
    let mut encrypted_source = decrypt_root.open(&encrypted_target);
    let (database_target, mut database_stage) = decrypt_root.stage("small.sqlite3", 8);
    let error = EncryptedBackupPackage::decrypt(
        &mut encrypted_source,
        existing_passphrase(PASSPHRASE),
        &mut database_stage,
    )
    .expect_err("expanded database capacity is bounded");
    assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    assert_eq!(decrypt_root.unpublished_stage_count(), 0);
    assert!(database_stage.publish_new(&database_target).is_err());

    let cleanup_root = ControlledRoot::new();
    let cleanup_source_target = cleanup_root.publish_bytes("cleanup.tmbackup", &plaintext);
    let mut cleanup_source = cleanup_root.open(&cleanup_source_target);
    let (_cleanup_target, mut cleanup_stage) =
        cleanup_root.stage("cleanup.tmbackup.age", PACKAGE_MAX_BYTES);
    cleanup_root.sabotage_only_stage_cleanup();
    let error = EncryptedBackupPackage::encrypt(
        BackupEncryptionContext::AutomaticBackup,
        &mut cleanup_source,
        &verified,
        new_passphrase(PASSPHRASE),
        &mut cleanup_stage,
    )
    .expect_err("cleanup uncertainty is recovery required");
    assert_eq!(error.code(), StateErrorCode::RecoveryRequired);
    assert!(cleanup_stage.seal(0, digest(b"")).is_err());
}

#[test]
fn encryption_source_failure_and_automatic_mode_discard_ciphertext_stage() {
    let root = ControlledRoot::new();
    let plaintext = backup_bytes();
    let (verified, _) = read_backup_bytes(&plaintext).expect("verify source backup");
    let source_target = root.publish_bytes("source.tmbackup", &plaintext);
    let mut source = root.open(&source_target);
    root.append_child("source.tmbackup", b"late-byte");
    let (target, mut stage) = root.stage("failed.tmbackup.age", PACKAGE_MAX_BYTES);
    let error = EncryptedBackupPackage::encrypt(
        BackupEncryptionContext::ManualExport,
        &mut source,
        &verified,
        new_passphrase(PASSPHRASE),
        &mut stage,
    )
    .expect_err("changed source must fail");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(root.unpublished_stage_count(), 0);
    assert!(stage.seal(0, digest(b"")).is_err());
    assert!(stage.publish_new(&target).is_err());

    let clean_target = root.publish_bytes("automatic-source.tmbackup", &plaintext);
    let mut clean_source = root.open(&clean_target);
    let (automatic_target, mut automatic_stage) =
        root.stage("automatic.tmbackup.age", PACKAGE_MAX_BYTES);
    let error = EncryptedBackupPackage::encrypt(
        BackupEncryptionContext::AutomaticBackup,
        &mut clean_source,
        &verified,
        new_passphrase(PASSPHRASE),
        &mut automatic_stage,
    )
    .expect_err("automatic encryption is forbidden");
    assert_eq!(error.code(), StateErrorCode::InvalidInput);
    assert_eq!(automatic_stage.written_len(), 0);
    assert_eq!(root.unpublished_stage_count(), 0);
    assert!(automatic_stage.publish_new(&automatic_target).is_err());

    let mut substituted = plaintext;
    substituted[0] ^= 1;
    let substituted_target = root.publish_bytes("substituted.tmbackup", &substituted);
    let mut substituted_source = root.open(&substituted_target);
    let (substituted_output_target, mut substituted_stage) =
        root.stage("substituted.tmbackup.age", PACKAGE_MAX_BYTES);
    let error = EncryptedBackupPackage::encrypt(
        BackupEncryptionContext::ManualExport,
        &mut substituted_source,
        &verified,
        new_passphrase(PASSPHRASE),
        &mut substituted_stage,
    )
    .expect_err("same-length source substitution must fail verified identity");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(root.unpublished_stage_count(), 0);
    assert!(
        substituted_stage
            .publish_new(&substituted_output_target)
            .is_err()
    );
}

#[test]
fn passphrase_contract_is_exact_clears_inputs_and_redacts_every_surface() {
    assert_eq!(MIN_BACKUP_PASSPHRASE_SCALARS, 12);
    assert_eq!(MAX_BACKUP_PASSPHRASE_SCALARS, 128);
    assert!(std::mem::needs_drop::<BackupPassphrase>());

    for valid in [
        "x".repeat(12),
        "🦀".repeat(12),
        " ".repeat(12),
        "x".repeat(128),
    ] {
        let mut confirmation = valid.clone();
        let mut input = valid;
        let secret = BackupPassphrase::new(&mut input, &mut confirmation).expect("exact bound");
        assert!(input.is_empty());
        assert!(confirmation.is_empty());
        assert_eq!(format!("{secret:?}"), "BackupPassphrase([redacted])");
    }

    for invalid in ["x".repeat(11), "x".repeat(129)] {
        let mut confirmation = invalid.clone();
        let mut input = invalid;
        let error = BackupPassphrase::new(&mut input, &mut confirmation).unwrap_err();
        assert_eq!(error.code(), StateErrorCode::InvalidInput);
        assert!(input.is_empty());
        assert!(confirmation.is_empty());
    }

    let mut input = "e\u{301}xxxxxxxxxxx".to_owned();
    let mut confirmation = "éxxxxxxxxxxx".to_owned();
    let error = BackupPassphrase::new(&mut input, &mut confirmation).unwrap_err();
    assert_eq!(error.code(), StateErrorCode::InvalidInput);
    assert!(input.is_empty());
    assert!(confirmation.is_empty());

    let mut input = format!(" {PASSPHRASE} ");
    let mut trimmed_confirmation = PASSPHRASE.to_owned();
    assert!(BackupPassphrase::new(&mut input, &mut trimmed_confirmation).is_err());
    assert!(input.is_empty());
    assert!(trimmed_confirmation.is_empty());

    let encoded = encrypted_backup();
    assert!(
        !encoded
            .windows(PASSPHRASE.len())
            .any(|window| window == PASSPHRASE.as_bytes())
    );
    let error = decrypt_bytes(&encoded, OTHER_PASSPHRASE).unwrap_err();
    assert!(!format!("{error:?}").contains(OTHER_PASSPHRASE));
    assert!(!format!("{error}").contains(OTHER_PASSPHRASE));
    assert!(std::env::args().all(|argument| !argument.contains(PASSPHRASE)));
    assert!(
        std::env::vars()
            .all(|(key, value)| { !key.contains(PASSPHRASE) && !value.contains(PASSPHRASE) })
    );
}
