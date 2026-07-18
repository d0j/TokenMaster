#![allow(clippy::expect_used, clippy::unwrap_used)]

mod package_support;

use std::io::Write;

use sha2::{Digest, Sha256};
use tokenmaster_platform::DurableFileError;
use tokenmaster_state::{
    BackupCompression, BackupMetadata, BackupPackage, BackupPurpose, StateErrorCode,
};

use package_support::{
    ControlledRoot, PACKAGE_MAX_BYTES, backup_bytes, backup_bytes_with, config_bytes,
    read_backup_bytes, read_config_bytes, settings,
};

fn reseal_descriptors_and_package(package: &mut [u8]) {
    let prefixes: Vec<usize> = package
        .windows(8)
        .enumerate()
        .filter_map(|(offset, window)| (window == b"TMENTR01").then_some(offset))
        .collect();
    let suffixes: Vec<usize> = package
        .windows(8)
        .enumerate()
        .filter_map(|(offset, window)| (window == b"TMENEND1").then_some(offset))
        .collect();
    let mut binding_hasher = Sha256::new();
    binding_hasher.update(&package[32..72]);
    for (prefix, suffix) in prefixes.into_iter().zip(suffixes) {
        binding_hasher.update(&package[prefix..prefix + 64]);
        binding_hasher.update(&package[suffix..suffix + 24]);
    }
    let binding_offset = package.len() - 72;
    package[binding_offset..binding_offset + 32].copy_from_slice(&binding_hasher.finalize());
    let package_digest: [u8; 32] = Sha256::digest(&package[..package.len() - 32]).into();
    let package_digest_offset = package.len() - 32;
    package[package_digest_offset..].copy_from_slice(&package_digest);
}

#[test]
fn truncation_and_flips_at_every_structural_region_fail_closed() {
    let original = config_bytes();
    let suffix = original
        .windows(8)
        .position(|window| window == b"TMENEND1")
        .expect("entry suffix");
    let boundaries = [
        0,
        1,
        8,
        10,
        12,
        16,
        20,
        28,
        32,
        40,
        56,
        80,
        suffix,
        suffix + 24,
        original.len() - 72,
        original.len() - 40,
        original.len() - 32,
        original.len() - 1,
    ];

    for boundary in boundaries {
        assert!(
            read_config_bytes(&original[..boundary]).is_err(),
            "truncate {boundary}"
        );
        if boundary < original.len() - 32 {
            let mut flipped = original.clone();
            flipped[boundary] ^= 0x80;
            assert!(
                read_config_bytes(flipped.as_slice()).is_err(),
                "flip {boundary}"
            );
        }
    }
}

#[test]
fn unknown_version_flags_kind_count_and_manifest_length_are_rejected_before_allocation() {
    let cases = [
        (8, 2_u8, StateErrorCode::UnsupportedVersion),
        (14, 1_u8, StateErrorCode::UnsupportedVersion),
        (12, 9_u8, StateErrorCode::UnsupportedVersion),
        (13, 9_u8, StateErrorCode::CapacityExceeded),
    ];
    for (offset, value, expected) in cases {
        let mut bytes = config_bytes();
        bytes[offset] = value;
        assert_eq!(
            read_config_bytes(bytes.as_slice()).unwrap_err().code(),
            expected
        );
    }

    let mut oversized_manifest = config_bytes();
    oversized_manifest[16..20].copy_from_slice(&(64_u32 * 1024 + 1).to_le_bytes());
    assert_eq!(
        read_config_bytes(oversized_manifest.as_slice())
            .unwrap_err()
            .code(),
        StateErrorCode::CapacityExceeded
    );
}

#[test]
fn duplicate_unknown_entries_codecs_and_trailing_data_fail_closed() {
    let original = backup_bytes();
    let first = original
        .windows(8)
        .position(|window| window == b"TMENTR01")
        .expect("first descriptor");
    let second = original[first + 8..]
        .windows(8)
        .position(|window| window == b"TMENTR01")
        .map(|offset| first + 8 + offset)
        .expect("second descriptor");

    for (offset, value) in [(second + 8, 1_u8), (second + 8, 9_u8), (second + 9, 9_u8)] {
        let mut mutated = original.clone();
        mutated[offset] = value;
        assert!(read_backup_bytes(mutated.as_slice()).is_err());
    }

    let mut trailing = original.clone();
    trailing.extend_from_slice(b"trailing");
    assert!(read_backup_bytes(trailing.as_slice()).is_err());
}

#[test]
fn concatenated_frames_missing_checksum_false_lengths_and_wrong_digest_fail_closed() {
    let original = config_bytes();
    let frame = original
        .windows(4)
        .position(|window| window == [0x28, 0xb5, 0x2f, 0xfd])
        .expect("zstd frame");
    let suffix = original
        .windows(8)
        .position(|window| window == b"TMENEND1")
        .expect("entry suffix");

    let mut no_checksum = original.clone();
    no_checksum[frame + 4] &= !0b0000_0100;
    assert_eq!(
        read_config_bytes(no_checksum.as_slice())
            .unwrap_err()
            .code(),
        StateErrorCode::UnsupportedVersion
    );

    let mut concatenated = original.clone();
    let frame_bytes = original[frame..suffix].to_vec();
    concatenated.splice(suffix..suffix, frame_bytes);
    assert!(read_config_bytes(concatenated.as_slice()).is_err());

    let mut false_expanded = original.clone();
    false_expanded[88..96].copy_from_slice(&1_u64.to_le_bytes());
    assert!(read_config_bytes(false_expanded.as_slice()).is_err());

    let mut wrong_frame_checksum = original.clone();
    wrong_frame_checksum[suffix - 1] ^= 0xff;
    assert!(read_config_bytes(wrong_frame_checksum.as_slice()).is_err());

    let mut wrong_package_digest = original;
    let final_byte = wrong_package_digest.len() - 1;
    wrong_package_digest[final_byte] ^= 0xff;
    assert!(read_config_bytes(wrong_package_digest.as_slice()).is_err());
}

#[test]
fn missing_zstd_frame_end_fails_even_when_outer_digests_are_resealed() {
    let mut package = config_bytes();
    let suffix = package
        .windows(8)
        .position(|window| window == b"TMENEND1")
        .expect("entry suffix");
    let compressed_len = u64::from_le_bytes(
        package[suffix + 8..suffix + 16]
            .try_into()
            .expect("compressed length"),
    );
    package.remove(suffix - 1);
    let shortened_suffix = suffix - 1;
    package[shortened_suffix + 8..shortened_suffix + 16]
        .copy_from_slice(&(compressed_len - 1).to_le_bytes());
    reseal_descriptors_and_package(&mut package);

    let error = read_config_bytes(&package).expect_err("missing frame end must fail");
    assert_eq!(error.code(), StateErrorCode::Integrity);
}

#[test]
fn decompression_bomb_content_size_lie_never_writes_past_declared_bound() {
    const ACTUAL_BYTES: usize = 300;
    const DECLARED_BYTES: usize = 256;
    let database = [0x5a_u8; ACTUAL_BYTES];
    let mut package = backup_bytes_with(
        &database,
        BackupCompression::Automatic,
        BackupPurpose::Periodic,
    )
    .0;
    let prefixes: Vec<usize> = package
        .windows(8)
        .enumerate()
        .filter_map(|(offset, window)| (window == b"TMENTR01").then_some(offset))
        .collect();
    let suffixes: Vec<usize> = package
        .windows(8)
        .enumerate()
        .filter_map(|(offset, window)| (window == b"TMENEND1").then_some(offset))
        .collect();
    assert_eq!(prefixes.len(), 2);
    assert_eq!(suffixes.len(), 2);
    let database_prefix = prefixes[1];
    let database_frame = database_prefix + 64;
    let database_suffix = suffixes[1];
    let decoded = zstd::stream::decode_all(&package[database_frame..database_suffix])
        .expect("baseline frame decodes");
    assert_eq!(decoded, database);

    let descriptor = package[database_frame + 4];
    assert_ne!(descriptor & 0b0010_0000, 0, "single-segment frame");
    assert_eq!(descriptor >> 6, 1, "two-byte content-size field");
    assert_eq!(
        u16::from_le_bytes(
            package[database_frame + 5..database_frame + 7]
                .try_into()
                .expect("content size")
        ),
        (ACTUAL_BYTES - 256) as u16
    );
    package[database_frame + 5..database_frame + 7].copy_from_slice(&0_u16.to_le_bytes());
    assert_eq!(
        zstd::zstd_safe::get_frame_content_size(&package[database_frame..database_suffix])
            .expect("frame content size"),
        Some(DECLARED_BYTES as u64)
    );

    let original_total = u64::from_le_bytes(package[20..28].try_into().unwrap());
    package[20..28]
        .copy_from_slice(&(original_total - (ACTUAL_BYTES - DECLARED_BYTES) as u64).to_le_bytes());
    package[database_prefix + 16..database_prefix + 24]
        .copy_from_slice(&(DECLARED_BYTES as u64).to_le_bytes());
    package[database_prefix + 24..database_prefix + 56]
        .copy_from_slice(&Sha256::digest(&database[..DECLARED_BYTES]));
    package[database_suffix + 16..database_suffix + 24]
        .copy_from_slice(&(DECLARED_BYTES as u64).to_le_bytes());
    reseal_descriptors_and_package(&mut package);

    let root = ControlledRoot::new();
    let package_target = root.publish_bytes("bomb.tmbackup", &package);
    let mut package_reader = root.open(&package_target);
    let (restore_target, mut restore_stage) = root.stage("bomb.sqlite3", 1024);
    let error = BackupPackage::read(&mut package_reader, &mut restore_stage)
        .expect_err("content-size lie must fail");
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert!(restore_stage.written_len() <= DECLARED_BYTES as u64);
    assert_eq!(
        restore_stage
            .seal(restore_stage.written_len(), [0_u8; 32])
            .expect_err("bomb output is poisoned"),
        DurableFileError::InvalidState
    );
    assert_eq!(
        restore_stage
            .publish_new(&restore_target)
            .expect_err("bomb output must remain unsealed"),
        DurableFileError::InvalidState
    );
}

#[test]
fn checked_total_and_entry_lengths_reject_overflow_and_hard_limit_excess() {
    let mut total_overflow = config_bytes();
    total_overflow[20..28].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        read_config_bytes(total_overflow.as_slice())
            .unwrap_err()
            .code(),
        StateErrorCode::CapacityExceeded
    );

    let mut settings_oversized = config_bytes();
    settings_oversized[88..96].copy_from_slice(&(1024_u64 * 1024 + 1).to_le_bytes());
    assert_eq!(
        read_config_bytes(settings_oversized.as_slice())
            .unwrap_err()
            .code(),
        StateErrorCode::CapacityExceeded
    );
}

#[test]
fn zstd_frame_advertising_more_than_the_eight_mib_window_is_rejected() {
    const DATABASE_BYTES: usize = 9 * 1024 * 1024;
    let database = vec![0x5a_u8; DATABASE_BYTES];
    let mut package = backup_bytes_with(
        &database,
        BackupCompression::Automatic,
        BackupPurpose::Periodic,
    )
    .0;

    let prefixes: Vec<usize> = package
        .windows(8)
        .enumerate()
        .filter_map(|(offset, window)| (window == b"TMENTR01").then_some(offset))
        .collect();
    let suffixes: Vec<usize> = package
        .windows(8)
        .enumerate()
        .filter_map(|(offset, window)| (window == b"TMENEND1").then_some(offset))
        .collect();
    assert_eq!(prefixes.len(), 2);
    assert_eq!(suffixes.len(), 2);

    let mut oversized_window_frame = Vec::new();
    {
        let mut encoder = zstd::stream::write::Encoder::new(&mut oversized_window_frame, 6)
            .expect("zstd encoder");
        encoder.include_checksum(true).expect("checksum");
        encoder.include_contentsize(true).expect("content size");
        encoder
            .long_distance_matching(false)
            .expect("no long-distance matching");
        encoder.window_log(24).expect("16 MiB window");
        encoder
            .set_pledged_src_size(Some(DATABASE_BYTES as u64))
            .expect("pledged size");
        encoder.write_all(&database).expect("compress fixture");
        encoder.finish().expect("finish fixture");
    }

    let second_frame_start = prefixes[1] + 64;
    package.splice(
        second_frame_start..suffixes[1],
        oversized_window_frame.iter().copied(),
    );
    let second_suffix = second_frame_start + oversized_window_frame.len();
    package[second_suffix + 8..second_suffix + 16]
        .copy_from_slice(&(oversized_window_frame.len() as u64).to_le_bytes());

    let first_suffix = package
        .windows(8)
        .position(|window| window == b"TMENEND1")
        .expect("first suffix after replacement");
    let second_prefix = package[first_suffix + 8..]
        .windows(8)
        .position(|window| window == b"TMENTR01")
        .map(|offset| first_suffix + 8 + offset)
        .expect("second prefix after replacement");
    let mut binding_hasher = Sha256::new();
    binding_hasher.update(&package[32..72]);
    binding_hasher.update(&package[prefixes[0]..prefixes[0] + 64]);
    binding_hasher.update(&package[first_suffix..first_suffix + 24]);
    binding_hasher.update(&package[second_prefix..second_prefix + 64]);
    binding_hasher.update(&package[second_suffix..second_suffix + 24]);
    let binding_offset = package.len() - 72;
    package[binding_offset..binding_offset + 32].copy_from_slice(&binding_hasher.finalize());
    let package_digest: [u8; 32] = Sha256::digest(&package[..package.len() - 32]).into();
    let package_digest_offset = package.len() - 32;
    package[package_digest_offset..].copy_from_slice(&package_digest);

    let error = read_backup_bytes(package.as_slice()).unwrap_err();
    assert_eq!(error.code(), StateErrorCode::Integrity);
}

#[test]
fn writer_requires_exact_source_length_and_digest() {
    let root = ControlledRoot::new();
    let too_long_target = root.publish_bytes("too-long.sqlite3", &[7_u8; 65]);
    let mut too_long = root.open(&too_long_target);
    let (output_target, mut output) = root.stage("too-long.tmbackup", PACKAGE_MAX_BYTES);
    let error = BackupPackage::write(
        &settings(),
        &mut too_long,
        64,
        [0_u8; 32],
        13,
        BackupCompression::Automatic,
        BackupMetadata::new(1_721_234_567_890, BackupPurpose::Periodic).expect("backup metadata"),
        &mut output,
    )
    .unwrap_err();
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert_eq!(
        output
            .seal(output.written_len(), [0_u8; 32])
            .expect_err("length-failure output is poisoned"),
        DurableFileError::InvalidState
    );
    assert_eq!(
        output
            .publish_new(&output_target)
            .expect_err("failed output remains unsealed"),
        DurableFileError::InvalidState
    );

    let exact_target = root.publish_bytes("exact.sqlite3", &[7_u8; 64]);
    let mut exact_source = root.open(&exact_target);
    let (digest_target, mut output) = root.stage("wrong-digest.tmbackup", PACKAGE_MAX_BYTES);
    let error = BackupPackage::write(
        &settings(),
        &mut exact_source,
        64,
        [0_u8; 32],
        13,
        BackupCompression::Automatic,
        BackupMetadata::new(1_721_234_567_890, BackupPurpose::Periodic).expect("backup metadata"),
        &mut output,
    )
    .unwrap_err();
    assert_eq!(error.code(), StateErrorCode::Integrity);
    assert!(
        output.written_len() > 0,
        "writer reached partial package output"
    );
    assert_eq!(
        output
            .seal(output.written_len(), [0_u8; 32])
            .expect_err("digest-failure output is poisoned"),
        DurableFileError::InvalidState
    );
    assert_eq!(
        output
            .publish_new(&digest_target)
            .expect_err("digest failure remains unsealed"),
        DurableFileError::InvalidState
    );
}

#[test]
fn package_wire_and_debug_surfaces_contain_no_private_archive_metadata() {
    let bytes = backup_bytes();
    let lower = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    for forbidden in [
        "filename",
        "path",
        "permission",
        "device",
        "credential",
        "prompt",
        "response",
        "reasoning",
        "command",
        "source",
        "c:\\",
        "/home/",
    ] {
        assert!(
            !lower.contains(forbidden),
            "forbidden wire field {forbidden}"
        );
    }

    let (verified, _) = read_backup_bytes(bytes.as_slice()).expect("verified");
    let debug = format!("{verified:?}").to_ascii_lowercase();
    assert!(!debug.contains("sqlite"));
    assert!(!debug.contains("adversarial"));
    assert!(!debug.contains("path"));
}

#[test]
fn synthetic_exported_archive_is_free_of_private_input_canaries() {
    let archive = backup_bytes();
    for canary in [
        r"C:\private\codex-home",
        "/home/private/tokenmaster",
        "Private@Example.com",
        "PRIVATE_SESSION_NAME.jsonl",
        "PIPELINE_PRIVATE_SENTINEL_91A7",
        "Authorization: Bearer private",
        "prompt-private-canary",
        "response-private-canary",
        "reasoning-private-canary",
        "command-private-canary",
        "source-private-canary",
    ] {
        assert!(
            !archive
                .windows(canary.len())
                .any(|window| window.eq_ignore_ascii_case(canary.as_bytes())),
            "synthetic exported archive exposed private input canary"
        );
    }
}
