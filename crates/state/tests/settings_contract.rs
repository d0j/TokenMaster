#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{DURABLE_STAGE_ATTEMPTS, ValidatedLocalDirectory};
use tokenmaster_state::{
    BACKUP_INTERVAL_DEFAULT_SECONDS, BACKUP_INTERVAL_MAX_SECONDS, BACKUP_QUIET_DEFAULT_SECONDS,
    BACKUP_QUIET_MIN_SECONDS, BACKUP_RETENTION_DEFAULT_BYTES, BACKUP_RETENTION_MAX_BYTES,
    BACKUP_RETENTION_MIN_BYTES, DeviceRoute, DeviceSettings, PortableSettings,
    PortableSettingsCandidate, PortableSettingsTarget, ReminderPolicy, SettingsChangeCategory,
    SettingsHealthCode, SettingsLoadOutcome, SettingsStore, SettingsValue, StateErrorCode,
};

const HEADER_BYTES: usize = 64;
const FOOTER_BYTES: usize = 40;

fn fixture() -> (TempDir, ValidatedLocalDirectory) {
    let root = tempfile::tempdir().expect("temporary reliable-state root");
    let directory =
        ValidatedLocalDirectory::new(root.path()).expect("validated reliable-state root");
    (root, directory)
}

fn changed_value(route: DeviceRoute, retention_bytes: u64) -> SettingsValue {
    let reminders = ReminderPolicy::new(true, &[86_400, 10_800]).expect("reminder policy");
    let backup = tokenmaster_state::BackupPolicy::new(
        true,
        BACKUP_QUIET_DEFAULT_SECONDS,
        BACKUP_INTERVAL_DEFAULT_SECONDS,
        retention_bytes,
    )
    .expect("backup policy");
    SettingsValue::new(
        PortableSettings::new(reminders, backup),
        DeviceSettings::new(route),
    )
}

fn encode_record(generation: u64, payload: &[u8]) -> Vec<u8> {
    let payload_digest: [u8; 32] = Sha256::digest(payload).into();
    let mut header = [0_u8; HEADER_BYTES];
    header[0..8].copy_from_slice(b"TMREC001");
    header[8..10].copy_from_slice(&1_u16.to_le_bytes());
    header[10..12].copy_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
    header[16..24].copy_from_slice(&generation.to_le_bytes());
    header[24..32].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    header[32..64].copy_from_slice(&payload_digest);

    let mut bytes = Vec::with_capacity(HEADER_BYTES + payload.len() + FOOTER_BYTES);
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(b"TMEND001");
    let record_digest: [u8; 32] = Sha256::digest(&bytes).into();
    bytes.extend_from_slice(&record_digest);
    bytes
}

#[test]
fn schema_v1_is_exact_strict_and_uses_only_owned_fields() {
    let value = SettingsValue::safe_defaults();
    let encoded = serde_json::to_value(&value).expect("encode settings");
    assert_eq!(
        encoded,
        json!({
            "schema_version": 1,
            "portable": {
                "reminders": {
                    "enabled": true,
                    "lead_seconds": [604800, 86400, 43200, 21600, 3600]
                },
                "backup": {
                    "periodic_enabled": true,
                    "quiet_seconds": BACKUP_QUIET_DEFAULT_SECONDS,
                    "interval_seconds": BACKUP_INTERVAL_DEFAULT_SECONDS,
                    "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES
                }
            },
            "device": { "last_route": "dashboard" }
        })
    );

    let mut unknown = encoded;
    unknown
        .as_object_mut()
        .expect("object")
        .insert("skin".to_owned(), json!("future-placeholder"));
    assert!(serde_json::from_value::<SettingsValue>(unknown).is_err());
}

#[test]
fn validation_rejects_capacity_duplicates_ranges_and_relationships() {
    assert_eq!(BACKUP_QUIET_MIN_SECONDS, 300);
    assert_eq!(BACKUP_INTERVAL_DEFAULT_SECONDS, 21_600);
    assert_eq!(
        ReminderPolicy::new(true, &[3_600, 3_600])
            .expect_err("duplicate")
            .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        ReminderPolicy::new(true, &[])
            .expect_err("enabled empty")
            .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        ReminderPolicy::new(false, &[3_600])
            .expect_err("disabled nonempty")
            .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        ReminderPolicy::new(true, &[59])
            .expect_err("below range")
            .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        ReminderPolicy::new(true, &[60; 9])
            .expect_err("too many")
            .code(),
        StateErrorCode::CapacityExceeded
    );
    assert_eq!(
        tokenmaster_state::BackupPolicy::new(
            true,
            BACKUP_QUIET_DEFAULT_SECONDS,
            BACKUP_INTERVAL_DEFAULT_SECONDS,
            BACKUP_RETENTION_MIN_BYTES - 1,
        )
        .expect_err("retention below range")
        .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        tokenmaster_state::BackupPolicy::new(
            true,
            BACKUP_QUIET_DEFAULT_SECONDS,
            BACKUP_INTERVAL_DEFAULT_SECONDS,
            BACKUP_RETENTION_MAX_BYTES + 1,
        )
        .expect_err("retention above range")
        .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        tokenmaster_state::BackupPolicy::new(
            true,
            BACKUP_QUIET_MIN_SECONDS - 1,
            BACKUP_INTERVAL_DEFAULT_SECONDS,
            BACKUP_RETENTION_DEFAULT_BYTES,
        )
        .expect_err("quiet below range")
        .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        tokenmaster_state::BackupPolicy::new(
            true,
            BACKUP_QUIET_DEFAULT_SECONDS,
            BACKUP_INTERVAL_MAX_SECONDS + 1,
            BACKUP_RETENTION_DEFAULT_BYTES,
        )
        .expect_err("interval above range")
        .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        tokenmaster_state::BackupPolicy::new(
            true,
            BACKUP_INTERVAL_DEFAULT_SECONDS,
            BACKUP_INTERVAL_DEFAULT_SECONDS,
            BACKUP_RETENTION_DEFAULT_BYTES,
        )
        .expect_err("quiet must be shorter than interval")
        .code(),
        StateErrorCode::InvalidInput
    );

    let oversized = vec![b' '; 1024 * 1024 + 1];
    let (_root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    assert_eq!(
        store
            .preview_import(&oversized)
            .expect_err("oversized")
            .code(),
        StateErrorCode::CapacityExceeded
    );

    let many_leads = serde_json::to_vec(&json!({
        "schema_version": 1,
        "portable": {
            "reminders": { "enabled": true, "lead_seconds": vec![60_u32; 100_000] },
            "backup": {
                "periodic_enabled": true,
                "quiet_seconds": 300,
                "interval_seconds": 21_600,
                "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES
            }
        }
    }))
    .expect("many reminder leads");
    assert_eq!(
        store
            .preview_import(&many_leads)
            .expect_err("bounded reminder sequence")
            .code(),
        StateErrorCode::InvalidInput
    );
}

#[test]
fn load_selects_current_and_falls_back_from_corrupt_newest_slot() {
    let (root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    let first = SettingsValue::safe_defaults();
    let second = changed_value(DeviceRoute::Settings, BACKUP_RETENTION_DEFAULT_BYTES);

    assert_eq!(store.save(&first).expect("first save").generation(), 1);
    let loaded = store.load().expect("first load");
    assert_eq!(loaded.outcome(), SettingsLoadOutcome::Current);
    assert_eq!(loaded.health_code(), SettingsHealthCode::Healthy);
    assert_eq!(loaded.health_code().as_str(), "healthy");
    assert_eq!(loaded.value(), &first);

    assert_eq!(store.save(&second).expect("second save").generation(), 2);
    fs::write(root.path().join("settings-b.tms"), b"corrupt-newest").expect("corrupt newest");
    let fallback = store.load().expect("fallback load");
    assert_eq!(fallback.outcome(), SettingsLoadOutcome::Fallback);
    assert_eq!(
        fallback.health_code(),
        SettingsHealthCode::FallbackCorruptSlot
    );
    assert_eq!(fallback.health_code().as_str(), "fallback_corrupt_slot");
    assert_eq!(fallback.generation(), Some(1));
    assert_eq!(fallback.value(), &first);
}

#[test]
fn both_invalid_slots_load_defaults_without_touching_evidence() {
    let (root, directory) = fixture();
    let first = b"invalid-a";
    let second = b"invalid-b";
    fs::write(root.path().join("settings-a.tms"), first).expect("write invalid a");
    fs::write(root.path().join("settings-b.tms"), second).expect("write invalid b");
    let store = SettingsStore::new(&directory).expect("settings store");

    let loaded = store.load().expect("default load");
    assert_eq!(loaded.outcome(), SettingsLoadOutcome::Defaults);
    assert_eq!(
        loaded.health_code(),
        SettingsHealthCode::DefaultsNoValidRecord
    );
    assert_eq!(loaded.generation(), None);
    assert_eq!(loaded.value(), &SettingsValue::safe_defaults());
    assert_eq!(fs::read(root.path().join("settings-a.tms")).unwrap(), first);
    assert_eq!(
        fs::read(root.path().join("settings-b.tms")).unwrap(),
        second
    );

    let receipt = store
        .save(&SettingsValue::safe_defaults())
        .expect("explicit recovery save");
    assert_eq!(receipt.generation(), 1);
    assert_eq!(
        store.load().unwrap().outcome(),
        SettingsLoadOutcome::Fallback
    );
    assert_eq!(
        fs::read(root.path().join("settings-b.tms")).unwrap(),
        second
    );
}

#[test]
fn import_preview_is_bounded_typed_stale_safe_and_idempotent() {
    let (_root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    store
        .save(&SettingsValue::safe_defaults())
        .expect("initial settings");
    let candidate = PortableSettingsCandidate::new(
        changed_value(DeviceRoute::History, BACKUP_RETENTION_MIN_BYTES)
            .portable()
            .clone(),
    )
    .expect("portable candidate");
    let encoded = candidate.encode_json().expect("candidate JSON");
    let preview = store.preview_import(&encoded).expect("preview");
    assert_eq!(preview.changed_category_count(), 2);
    assert_eq!(preview.changed_field_count(), 2);
    assert_eq!(
        preview.categories(),
        &[
            SettingsChangeCategory::ReminderProfile,
            SettingsChangeCategory::BackupRetention,
        ]
    );
    assert!(!format!("{preview:?}").contains("1073741824"));

    let stale = store.preview_import(&encoded).expect("stale preview");
    store
        .save(&changed_value(
            DeviceRoute::Settings,
            BACKUP_RETENTION_DEFAULT_BYTES,
        ))
        .expect("intervening save");
    assert_eq!(
        store
            .commit_import(&stale)
            .expect_err("stale commit")
            .code(),
        StateErrorCode::Integrity
    );

    let fresh = store.preview_import(&encoded).expect("fresh preview");
    let committed = store.commit_import(&fresh).expect("commit import");
    assert_eq!(committed.generation(), 3);
    let target = committed.target();
    assert!(store.verify_target(target).expect("verify exact target"));
    assert_eq!(
        PortableSettingsTarget::from_persisted(0, *target.digest().as_bytes())
            .expect_err("zero generation")
            .code(),
        StateErrorCode::InvalidInput
    );
    assert_eq!(
        PortableSettingsTarget::from_persisted(target.generation(), *target.digest().as_bytes())
            .expect("reconstructed target"),
        target
    );
    assert_eq!(
        store.load().unwrap().value().device().last_route(),
        DeviceRoute::Settings
    );
    let unchanged = store.preview_import(&encoded).expect("unchanged preview");
    assert_eq!(unchanged.changed_category_count(), 0);
    assert_eq!(
        store
            .commit_import(&unchanged)
            .expect("idempotent")
            .generation(),
        3
    );
    store
        .save(&SettingsValue::safe_defaults())
        .expect("later settings generation");
    assert!(!store.verify_target(target).expect("stale target mismatch"));
}

#[test]
fn unsupported_or_malformed_import_never_writes_slots() {
    let (root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    store
        .save(&SettingsValue::safe_defaults())
        .expect("initial settings");
    let before = fs::read(root.path().join("settings-a.tms")).expect("read first");

    for (bytes, expected) in [
        (
            br#"{"schema_version":2,"portable":{}}"#.as_slice(),
            StateErrorCode::UnsupportedVersion,
        ),
        (
            br#"{"schema_version":0,"portable":{}}"#.as_slice(),
            StateErrorCode::UnsupportedVersion,
        ),
        (
            br#"{"schema_version":1,"portable":{"reminders":[]}}"#.as_slice(),
            StateErrorCode::InvalidInput,
        ),
    ] {
        assert_eq!(
            store
                .preview_import(bytes)
                .expect_err("invalid import")
                .code(),
            expected
        );
    }
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        before
    );
    assert!(!root.path().join("settings-b.tms").exists());
}

#[test]
fn valid_record_with_unsupported_settings_version_is_never_defaults_or_overwritten() {
    for include_current_peer in [false, true] {
        let (root, directory) = fixture();
        let current = SettingsValue::safe_defaults();
        if include_current_peer {
            let current_payload = serde_json::to_vec(&current).expect("current payload");
            fs::write(
                root.path().join("settings-a.tms"),
                encode_record(1, &current_payload),
            )
            .expect("current peer");
        }
        let mut newer = serde_json::to_value(&current).expect("newer settings value");
        newer["schema_version"] = json!(2);
        let newer_record = encode_record(
            2,
            &serde_json::to_vec(&newer).expect("newer settings payload"),
        );
        fs::write(root.path().join("settings-b.tms"), &newer_record)
            .expect("newer settings record");
        let before_a = fs::read(root.path().join("settings-a.tms")).ok();
        let before_b = fs::read(root.path().join("settings-b.tms")).expect("before b");
        let store = SettingsStore::new(&directory).expect("settings store");

        assert_eq!(
            store.load().expect_err("newer load").code(),
            StateErrorCode::UnsupportedVersion
        );
        assert_eq!(
            store.save(&current).expect_err("newer save").code(),
            StateErrorCode::UnsupportedVersion
        );
        assert_eq!(fs::read(root.path().join("settings-a.tms")).ok(), before_a);
        assert_eq!(
            fs::read(root.path().join("settings-b.tms")).unwrap(),
            before_b
        );
    }
}

#[test]
fn generation_overflow_and_staging_failure_preserve_selected_truth() {
    let (root, directory) = fixture();
    let value = SettingsValue::safe_defaults();
    let payload = serde_json::to_vec(&value).expect("payload");
    let record = encode_record(u64::MAX, &payload);
    fs::write(root.path().join("settings-a.tms"), &record).expect("record a");
    fs::write(root.path().join("settings-b.tms"), &record).expect("record b");
    let store = SettingsStore::new(&directory).expect("settings store");
    assert_eq!(
        store.save(&value).expect_err("overflow").code(),
        StateErrorCode::CapacityExceeded
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        record
    );

    let (root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    store.save(&value).expect("first save");
    let before = fs::read(root.path().join("settings-a.tms")).expect("before");
    for attempt in 0..DURABLE_STAGE_ATTEMPTS {
        fs::write(
            root.path()
                .join(format!(".settings-b.tms.tokenmaster-stage-{attempt:02}")),
            b"occupied",
        )
        .expect("occupy stage");
    }
    assert_eq!(
        store.save(&value).expect_err("stage collision").code(),
        StateErrorCode::CapacityExceeded
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        before
    );
    assert!(!root.path().join("settings-b.tms").exists());
}

#[test]
fn privacy_canaries_never_enter_values_errors_debug_or_preview() {
    let (_root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    let canaries = [
        "tm-password-canary",
        "C:\\private\\source.jsonl",
        "tm-credential-canary",
        "raw prompt response command source content",
    ];
    let malicious = json!({
        "schema_version": 1,
        "portable": {
            "reminders": {
                "enabled": true,
                "lead_seconds": [3600],
                "password": canaries[0]
            },
            "backup": {
                "periodic_enabled": true,
                "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES,
                "source_path": canaries[1]
            },
            "credential": canaries[2],
            "raw": canaries[3]
        }
    });
    let bytes = serde_json::to_vec(&malicious).expect("malicious input");
    let error = store.preview_import(&bytes).expect_err("forbidden fields");
    let output = format!("{error:?} {error}");
    for canary in canaries {
        assert!(!output.contains(canary));
        assert!(!format!("{store:?}").contains(canary));
        assert!(
            !serde_json::to_string(&SettingsValue::safe_defaults())
                .unwrap()
                .contains(canary)
        );
    }
}

#[test]
fn full_backup_candidate_is_portable_only_and_import_preserves_device_state() {
    let (_source_root, source_directory) = fixture();
    let source = SettingsStore::new(&source_directory).expect("source store");
    let source_value = changed_value(DeviceRoute::History, BACKUP_RETENTION_MIN_BYTES);
    source.save(&source_value).expect("source settings");
    let candidate = source
        .full_backup_candidate()
        .expect("portable backup candidate");
    let encoded = candidate.encode_json().expect("candidate JSON");
    let json: Value = serde_json::from_slice(&encoded).expect("candidate value");
    assert!(json.get("device").is_none());
    assert!(json.to_string().find("history").is_none());

    let (_target_root, target_directory) = fixture();
    let target = SettingsStore::new(&target_directory).expect("target store");
    target
        .save(&changed_value(
            DeviceRoute::Settings,
            BACKUP_RETENTION_DEFAULT_BYTES,
        ))
        .expect("target settings");
    let preview = target.preview_import(&encoded).expect("preview");
    let receipt = target.commit_import(&preview).expect("commit");
    assert_eq!(
        target.load().unwrap().value().device().last_route(),
        DeviceRoute::Settings
    );
    assert_eq!(receipt.portable_digest(), candidate.digest());
    assert_ne!(receipt.portable_digest().as_bytes(), &[0_u8; 32]);
}
