#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{DURABLE_STAGE_ATTEMPTS, ValidatedLocalDirectory};
use tokenmaster_state::{
    BACKUP_INTERVAL_DEFAULT_SECONDS, BACKUP_INTERVAL_MAX_SECONDS, BACKUP_QUIET_DEFAULT_SECONDS,
    BACKUP_QUIET_MIN_SECONDS, BACKUP_RETENTION_DEFAULT_BYTES, BACKUP_RETENTION_MAX_BYTES,
    BACKUP_RETENTION_MIN_BYTES, BoardPreferences, BoardSectionKey, BoardSectionPreference,
    DeviceRoute, DeviceSettings, PortableSettings, PortableSettingsCandidate,
    PortableSettingsTarget, PresentationColorScheme, PresentationDensity, PresentationLayout,
    PresentationLocale, PresentationSettings, PresentationSkin, ReminderPolicy,
    SETTINGS_SCHEMA_VERSION, SettingsChangeCategory, SettingsHealthCode, SettingsLoadOutcome,
    SettingsStore, SettingsValue, StateErrorCode,
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
        PortableSettings::new(reminders, backup, PresentationSettings::refined()),
        DeviceSettings::new(route),
    )
}

fn legacy_v1_portable_json() -> Value {
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
        }
    })
}

fn legacy_v1_settings_json(route: &str) -> Value {
    let mut legacy = legacy_v1_portable_json();
    legacy["device"] = json!({ "last_route": route });
    legacy
}

fn legacy_v2_settings_json(route: &str, density: &str) -> Value {
    json!({
        "schema_version": 2,
        "portable": {
            "reminders": { "enabled": true, "lead_seconds": [3600] },
            "backup": {
                "periodic_enabled": false,
                "quiet_seconds": 300,
                "interval_seconds": 21600,
                "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES
            },
            "presentation": { "density": density }
        },
        "device": { "last_route": route }
    })
}

fn legacy_v3_settings_json(route: &str, density: &str, skin: &str) -> Value {
    json!({
        "schema_version": 3,
        "portable": {
            "reminders": { "enabled": true, "lead_seconds": [3600] },
            "backup": {
                "periodic_enabled": false,
                "quiet_seconds": 300,
                "interval_seconds": 21600,
                "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES
            },
            "presentation": { "density": density, "skin": skin }
        },
        "device": { "last_route": route }
    })
}

fn legacy_v4_settings_json(route: &str, density: &str, skin: &str, color_scheme: &str) -> Value {
    json!({
        "schema_version": 4,
        "portable": {
            "reminders": { "enabled": true, "lead_seconds": [3600] },
            "backup": {
                "periodic_enabled": false,
                "quiet_seconds": 300,
                "interval_seconds": 21600,
                "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES
            },
            "presentation": {
                "density": density,
                "skin": skin,
                "color_scheme": color_scheme
            }
        },
        "device": { "last_route": route }
    })
}

fn legacy_v5_settings_json(
    route: &str,
    density: &str,
    skin: &str,
    color_scheme: &str,
    layout: &str,
) -> Value {
    json!({
        "schema_version": 5,
        "portable": {
            "reminders": { "enabled": true, "lead_seconds": [3600] },
            "backup": {
                "periodic_enabled": false,
                "quiet_seconds": 300,
                "interval_seconds": 21600,
                "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES
            },
            "presentation": {
                "density": density,
                "skin": skin,
                "color_scheme": color_scheme,
                "layout": layout
            }
        },
        "device": { "last_route": route }
    })
}

fn canonical_board_json() -> Value {
    json!([
        { "key": "plan_usage", "visible": true, "collapsed": false },
        { "key": "code_output", "visible": true, "collapsed": false },
        { "key": "trend", "visible": true, "collapsed": false },
        { "key": "sessions", "visible": true, "collapsed": false },
        { "key": "activity", "visible": true, "collapsed": false },
        { "key": "models", "visible": true, "collapsed": false }
    ])
}

#[test]
fn presentation_skin_serialization_contract() {
    assert_eq!(
        serde_json::to_value(PresentationSkin::Refined).expect("refined key"),
        json!("refined")
    );
    assert_eq!(
        serde_json::to_value(PresentationSkin::Graphite).expect("graphite key"),
        json!("graphite")
    );
    assert_eq!(
        serde_json::to_value(PresentationSkin::Ember).expect("ember key"),
        json!("ember")
    );

    for invalid in [json!("future"), json!(1), Value::Null] {
        assert!(serde_json::from_value::<PresentationSkin>(invalid).is_err());
    }
}

#[test]
fn presentation_color_scheme_serialization_and_default_contract() {
    for (scheme, key) in [
        (PresentationColorScheme::System, "system"),
        (PresentationColorScheme::Light, "light"),
        (PresentationColorScheme::Dark, "dark"),
    ] {
        assert_eq!(
            serde_json::to_value(scheme).expect("color scheme key"),
            json!(key)
        );
    }
    for invalid in [json!("future"), json!(1), Value::Null] {
        assert!(serde_json::from_value::<PresentationColorScheme>(invalid).is_err());
    }

    assert_eq!(
        PresentationSettings::refined().color_scheme(),
        PresentationColorScheme::System
    );
}

#[test]
fn presentation_layout_serialization_and_default_contract() {
    for (layout, key) in [
        (PresentationLayout::Refined, "refined"),
        (PresentationLayout::ControlCenter, "control_center"),
        (PresentationLayout::Workbench, "workbench"),
    ] {
        assert_eq!(
            serde_json::to_value(layout).expect("layout key"),
            json!(key)
        );
    }
    for invalid in [json!("future"), json!(1), Value::Null] {
        assert!(serde_json::from_value::<PresentationLayout>(invalid).is_err());
    }

    assert_eq!(
        PresentationSettings::refined().layout(),
        PresentationLayout::Refined
    );
}

#[test]
fn board_preferences_are_a_closed_visible_permutation() {
    let canonical = BoardPreferences::canonical();
    assert_eq!(
        canonical.rows(),
        &[
            BoardSectionPreference::new(BoardSectionKey::PlanUsage, true, false),
            BoardSectionPreference::new(BoardSectionKey::CodeOutput, true, false),
            BoardSectionPreference::new(BoardSectionKey::Trend, true, false),
            BoardSectionPreference::new(BoardSectionKey::Sessions, true, false),
            BoardSectionPreference::new(BoardSectionKey::Activity, true, false),
            BoardSectionPreference::new(BoardSectionKey::Models, true, false),
        ]
    );

    let encoded = serde_json::to_value(canonical).expect("encode board");
    assert_eq!(encoded[0]["key"], "plan_usage");
    assert!(serde_json::from_value::<BoardPreferences>(encoded.clone()).is_ok());

    for mutation in [
        json!([
            { "key": "plan_usage", "visible": true, "collapsed": false },
            { "key": "plan_usage", "visible": true, "collapsed": false },
            { "key": "trend", "visible": true, "collapsed": false },
            { "key": "sessions", "visible": true, "collapsed": false },
            { "key": "activity", "visible": true, "collapsed": false },
            { "key": "models", "visible": true, "collapsed": false }
        ]),
        json!([
            { "key": "plan_usage", "visible": false, "collapsed": false },
            { "key": "code_output", "visible": false, "collapsed": false },
            { "key": "trend", "visible": false, "collapsed": false },
            { "key": "sessions", "visible": false, "collapsed": false },
            { "key": "activity", "visible": false, "collapsed": false },
            { "key": "models", "visible": false, "collapsed": false }
        ]),
        json!([
            { "key": "plan_usage", "visible": true, "collapsed": false },
            { "key": "code_output", "visible": true, "collapsed": false },
            { "key": "trend", "visible": true, "collapsed": false },
            { "key": "sessions", "visible": true, "collapsed": false },
            { "key": "activity", "visible": true, "collapsed": false }
        ]),
        json!([
            { "key": "plan_usage", "visible": true, "collapsed": false },
            { "key": "code_output", "visible": true, "collapsed": false },
            { "key": "trend", "visible": true, "collapsed": false },
            { "key": "sessions", "visible": true, "collapsed": false },
            { "key": "activity", "visible": true, "collapsed": false },
            { "key": "unknown", "visible": true, "collapsed": false }
        ]),
    ] {
        assert!(serde_json::from_value::<BoardPreferences>(mutation).is_err());
    }
}

#[test]
fn settings_schema_v7_requires_one_complete_locale_and_board_axis() {
    assert_eq!(SETTINGS_SCHEMA_VERSION, 7);
    let encoded = serde_json::to_value(SettingsValue::safe_defaults()).expect("encode settings");
    assert_eq!(encoded["schema_version"], 7);
    assert_eq!(
        encoded["portable"]["presentation"],
        json!({
            "density": "comfortable",
            "skin": "refined",
            "color_scheme": "system",
            "layout": "refined",
            "locale": "en",
            "board": canonical_board_json()
        })
    );

    let mut missing = encoded.clone();
    missing["portable"]["presentation"]
        .as_object_mut()
        .expect("presentation object")
        .remove("locale");
    assert!(serde_json::from_value::<SettingsValue>(missing).is_err());

    let mut unknown = encoded;
    unknown["portable"]["presentation"]["locale"] = json!("future");
    assert!(serde_json::from_value::<SettingsValue>(unknown).is_err());

    let mut wrong_type =
        serde_json::to_value(SettingsValue::safe_defaults()).expect("encode settings");
    wrong_type["portable"]["presentation"]["locale"] = json!(1);
    assert!(serde_json::from_value::<SettingsValue>(wrong_type).is_err());

    let duplicate = br#"{"schema_version":7,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":"system","layout":"refined","locale":"en","locale":"ru","board":[{"key":"plan_usage","visible":true,"collapsed":false},{"key":"code_output","visible":true,"collapsed":false},{"key":"trend","visible":true,"collapsed":false},{"key":"sessions","visible":true,"collapsed":false},{"key":"activity","visible":true,"collapsed":false},{"key":"models","visible":true,"collapsed":false}]}},"device":{"last_route":"dashboard"}}"#;
    assert!(serde_json::from_slice::<SettingsValue>(duplicate).is_err());
}

#[test]
fn settings_schema_v7_contract() {
    assert_eq!(SETTINGS_SCHEMA_VERSION, 7);
    let refined = PresentationSettings::refined();
    assert_eq!(refined.density(), PresentationDensity::Comfortable);
    assert_eq!(refined.skin(), PresentationSkin::Refined);
    assert_eq!(refined.color_scheme(), PresentationColorScheme::System);
    assert_eq!(refined.layout(), PresentationLayout::Refined);
    assert_eq!(refined.locale(), PresentationLocale::English);
    assert_eq!(refined.board(), BoardPreferences::canonical());

    let encoded = serde_json::to_value(SettingsValue::safe_defaults()).expect("encode settings");
    assert_eq!(
        encoded,
        json!({
            "schema_version": 7,
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
                },
                "presentation": {
                    "density": "comfortable",
                    "skin": "refined",
                    "color_scheme": "system",
                    "layout": "refined",
                    "locale": "en",
                    "board": canonical_board_json()
                }
            },
            "device": { "last_route": "dashboard" }
        })
    );

    for payload in [
        br#"{"schema_version":7,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":"system","layout":"refined","locale":"en"}},"device":{"last_route":"dashboard"}}"#.as_slice(),
        br#"{"schema_version":7,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"future","color_scheme":"system","layout":"refined","locale":"en","board":[]}},"device":{"last_route":"dashboard"}}"#.as_slice(),
        br#"{"schema_version":7,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":"system","color_scheme":"dark","layout":"refined","locale":"en","board":[]}},"device":{"last_route":"dashboard"}}"#.as_slice(),
        br#"{"schema_version":7,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":1,"layout":"refined","locale":"en","board":[]}},"device":{"last_route":"dashboard"}}"#.as_slice(),
    ] {
        assert!(serde_json::from_slice::<SettingsValue>(payload).is_err());
    }
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
fn settings_schema_v7_serializes_only_owned_portable_presentation() {
    let value = SettingsValue::safe_defaults();
    let encoded = serde_json::to_value(&value).expect("encode settings");
    assert_eq!(encoded["schema_version"], 7);
    assert_eq!(
        encoded["portable"],
        json!({
            "reminders": {
                "enabled": true,
                "lead_seconds": [604800, 86400, 43200, 21600, 3600]
            },
            "backup": {
                "periodic_enabled": true,
                "quiet_seconds": BACKUP_QUIET_DEFAULT_SECONDS,
                "interval_seconds": BACKUP_INTERVAL_DEFAULT_SECONDS,
                "retention_budget_bytes": BACKUP_RETENTION_DEFAULT_BYTES
            },
            "presentation": {
                "density": "comfortable",
                "skin": "refined",
                "color_scheme": "system",
                "layout": "refined",
                "locale": "en",
                "board": canonical_board_json()
            }
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
fn schema_v1_v2_v3_dispatch_migrates_dark_and_explicit_save_writes_v5() {
    let (root, directory) = fixture();
    let payload =
        serde_json::to_vec(&legacy_v1_settings_json("projects")).expect("legacy settings");
    let record = encode_record(7, &payload);
    fs::write(root.path().join("settings-a.tms"), &record).expect("legacy record");
    let store = SettingsStore::new(&directory).expect("settings store");
    let loaded = store.load().expect("migrated load");
    assert_eq!(loaded.generation(), Some(7));
    assert_eq!(loaded.value().device().last_route(), DeviceRoute::Projects);
    assert_eq!(
        loaded.value().portable().reminders().lead_seconds(),
        &[604_800, 86_400, 43_200, 21_600, 3_600]
    );
    assert_eq!(
        loaded.value().portable().backup().retention_budget_bytes(),
        BACKUP_RETENTION_DEFAULT_BYTES
    );
    assert_eq!(
        loaded.value().portable().presentation().density(),
        PresentationDensity::Comfortable
    );
    assert_eq!(
        loaded.value().portable().presentation().skin(),
        PresentationSkin::Refined
    );
    assert_eq!(
        loaded.value().portable().presentation().color_scheme(),
        PresentationColorScheme::Dark
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        record
    );
    assert!(!root.path().join("settings-b.tms").exists());
    store.save(loaded.value()).expect("explicit v5 save");
    let newest = store.load().expect("v5 reread");
    assert_eq!(newest.generation(), Some(8));
    assert_eq!(newest.value(), loaded.value());

    let (root, directory) = fixture();
    let v2 =
        serde_json::to_vec(&legacy_v2_settings_json("history", "compact")).expect("v2 settings");
    let record = encode_record(9, &v2);
    fs::write(root.path().join("settings-a.tms"), &record).expect("v2 record");
    let store = SettingsStore::new(&directory).expect("settings store");
    let loaded = store.load().expect("v2 migrated load");
    assert_eq!(loaded.generation(), Some(9));
    assert_eq!(loaded.value().device().last_route(), DeviceRoute::History);
    assert_eq!(
        loaded.value().portable().reminders().lead_seconds(),
        &[3_600]
    );
    assert!(!loaded.value().portable().backup().periodic_enabled());
    assert_eq!(
        loaded.value().portable().presentation().density(),
        PresentationDensity::Compact
    );
    assert_eq!(
        loaded.value().portable().presentation().skin(),
        PresentationSkin::Refined
    );
    assert_eq!(
        loaded.value().portable().presentation().color_scheme(),
        PresentationColorScheme::Dark
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        record
    );
    assert!(!root.path().join("settings-b.tms").exists());

    let (root, directory) = fixture();
    let v3 = serde_json::to_vec(&legacy_v3_settings_json(
        "settings",
        "ultra_compact",
        "ember",
    ))
    .expect("v3 settings");
    let record = encode_record(10, &v3);
    fs::write(root.path().join("settings-a.tms"), &record).expect("v3 record");
    let loaded = SettingsStore::new(&directory)
        .expect("settings store")
        .load()
        .expect("v3 migrated load");
    assert_eq!(
        *loaded.value().portable().presentation(),
        PresentationSettings::new(
            PresentationDensity::UltraCompact,
            PresentationSkin::Ember,
            PresentationColorScheme::Dark,
            PresentationLayout::Refined,
            PresentationLocale::English,
        )
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        record
    );
}

#[test]
fn schema_v4_preserves_complete_legacy_style_and_defaults_layout_to_refined() {
    let (root, directory) = fixture();
    let payload = serde_json::to_vec(&legacy_v4_settings_json(
        "settings", "compact", "graphite", "light",
    ))
    .expect("v4 settings");
    let record = encode_record(11, &payload);
    fs::write(root.path().join("settings-a.tms"), &record).expect("v4 record");

    let loaded = SettingsStore::new(&directory)
        .expect("settings store")
        .load()
        .expect("v4 migrated load");
    assert_eq!(
        *loaded.value().portable().presentation(),
        PresentationSettings::new(
            PresentationDensity::Compact,
            PresentationSkin::Graphite,
            PresentationColorScheme::Light,
            PresentationLayout::Refined,
            PresentationLocale::English,
        )
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).expect("v4 evidence"),
        record
    );
    assert!(!root.path().join("settings-b.tms").exists());
}

#[test]
fn schema_v5_preserves_complete_style_and_defaults_board_without_a_startup_write() {
    let (root, directory) = fixture();
    let payload = serde_json::to_vec(&legacy_v5_settings_json(
        "settings",
        "compact",
        "graphite",
        "light",
        "workbench",
    ))
    .expect("v5 settings");
    let record = encode_record(12, &payload);
    fs::write(root.path().join("settings-a.tms"), &record).expect("v5 record");
    let loaded = SettingsStore::new(&directory)
        .expect("settings store")
        .load()
        .expect("v5 migrated load");
    assert_eq!(
        *loaded.value().portable().presentation(),
        PresentationSettings::new(
            PresentationDensity::Compact,
            PresentationSkin::Graphite,
            PresentationColorScheme::Light,
            PresentationLayout::Workbench,
            PresentationLocale::English,
        )
    );
    assert_eq!(
        loaded.value().portable().presentation().board(),
        BoardPreferences::canonical()
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).expect("v5 evidence"),
        record
    );
    assert!(!root.path().join("settings-b.tms").exists());
}

#[test]
fn schema_v6_candidate_and_record_migrate_locale_to_english() {
    let expected_board = BoardPreferences::new([
        BoardSectionPreference::new(BoardSectionKey::Models, true, true),
        BoardSectionPreference::new(BoardSectionKey::PlanUsage, true, false),
        BoardSectionPreference::new(BoardSectionKey::Activity, false, true),
        BoardSectionPreference::new(BoardSectionKey::Sessions, true, false),
        BoardSectionPreference::new(BoardSectionKey::Trend, false, true),
        BoardSectionPreference::new(BoardSectionKey::CodeOutput, true, false),
    ])
    .expect("valid noncanonical board");
    let mut legacy_record =
        serde_json::to_value(SettingsValue::safe_defaults()).expect("current settings record");
    legacy_record["schema_version"] = json!(6);
    legacy_record["portable"]["presentation"]
        .as_object_mut()
        .expect("presentation object")
        .remove("locale");
    legacy_record["portable"]["presentation"]["board"] = json!([
        { "key": "models", "visible": true, "collapsed": true },
        { "key": "plan_usage", "visible": true, "collapsed": false },
        { "key": "activity", "visible": false, "collapsed": true },
        { "key": "sessions", "visible": true, "collapsed": false },
        { "key": "trend", "visible": false, "collapsed": true },
        { "key": "code_output", "visible": true, "collapsed": false }
    ]);
    let legacy_candidate = json!({
        "schema_version": 6,
        "portable": legacy_record["portable"].clone(),
    });

    let (root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    let preview = store
        .preview_import(&serde_json::to_vec(&legacy_candidate).expect("v6 candidate"))
        .expect("v6 candidate preview");
    store.commit_import(&preview).expect("commit v6 candidate");
    let candidate_loaded = store.load().expect("candidate migration load");
    assert_eq!(
        candidate_loaded.value().portable().presentation().locale(),
        PresentationLocale::English
    );
    assert_eq!(
        candidate_loaded.value().portable().presentation().board(),
        expected_board
    );

    let record = encode_record(
        2,
        &serde_json::to_vec(&legacy_record).expect("v6 record payload"),
    );
    fs::write(root.path().join("settings-b.tms"), &record).expect("v6 record");
    let loaded = SettingsStore::new(&directory)
        .expect("record settings store")
        .load()
        .expect("v6 record migration");
    assert_eq!(loaded.generation(), Some(2));
    assert_eq!(
        loaded.value().portable().presentation().locale(),
        PresentationLocale::English
    );
    assert_eq!(
        loaded.value().portable().presentation().board(),
        expected_board
    );
}

#[test]
fn legacy_migration_retains_non_default_reminder_and_backup_values() {
    for (source_version, portable) in [
        (
            1,
            json!({
                "reminders": { "enabled": true, "lead_seconds": [7200, 3600] },
                "backup": { "periodic_enabled": false, "quiet_seconds": 600, "interval_seconds": 43200, "retention_budget_bytes": 3_221_225_472_u64 }
            }),
        ),
        (
            2,
            json!({
                "reminders": { "enabled": true, "lead_seconds": [10800] },
                "backup": { "periodic_enabled": true, "quiet_seconds": 900, "interval_seconds": 64800, "retention_budget_bytes": 4_294_967_296_u64 },
                "presentation": { "density": "ultra_compact" }
            }),
        ),
    ] {
        let (root, directory) = fixture();
        let payload = serde_json::to_vec(&json!({
            "schema_version": source_version,
            "portable": portable,
            "device": { "last_route": "settings" }
        }))
        .expect("legacy settings");
        fs::write(
            root.path().join("settings-a.tms"),
            encode_record(1, &payload),
        )
        .expect("legacy record");

        let loaded = SettingsStore::new(&directory)
            .expect("settings store")
            .load()
            .expect("migrated settings");
        let portable = loaded.value().portable();
        let expected_leads: &[u32] = if source_version == 1 {
            &[7_200, 3_600]
        } else {
            &[10_800]
        };
        assert_eq!(portable.reminders().lead_seconds(), expected_leads);
        assert_eq!(
            portable.backup().quiet_seconds(),
            if source_version == 1 { 600 } else { 900 }
        );
        assert_eq!(
            portable.backup().interval_seconds(),
            if source_version == 1 { 43_200 } else { 64_800 }
        );
        assert_eq!(
            portable.backup().retention_budget_bytes(),
            if source_version == 1 {
                3_221_225_472
            } else {
                4_294_967_296
            }
        );
    }
}

#[test]
fn portable_v1_migration_preserves_dark_appearance_in_v5() {
    let (_root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    let legacy = serde_json::to_vec(&legacy_v1_portable_json()).expect("legacy portable");
    let migrated = store.preview_import(&legacy).expect("legacy preview");
    assert_eq!(
        migrated.categories(),
        &[SettingsChangeCategory::Presentation]
    );
    let receipt = store.commit_import(&migrated).expect("migrated commit");
    let defaults = SettingsValue::safe_defaults();
    let canonical = PortableSettingsCandidate::new(PortableSettings::new(
        defaults.portable().reminders().clone(),
        defaults.portable().backup().clone(),
        PresentationSettings::new(
            PresentationDensity::Comfortable,
            PresentationSkin::Refined,
            PresentationColorScheme::Dark,
            PresentationLayout::Refined,
            PresentationLocale::English,
        ),
    ))
    .expect("canonical migrated candidate");
    assert_eq!(receipt.portable_digest(), canonical.digest());

    let candidate = PortableSettingsCandidate::new(PortableSettings::new(
        SettingsValue::safe_defaults()
            .portable()
            .reminders()
            .clone(),
        SettingsValue::safe_defaults().portable().backup().clone(),
        PresentationSettings::new(
            PresentationDensity::Compact,
            PresentationSkin::Refined,
            PresentationColorScheme::System,
            PresentationLayout::Refined,
            PresentationLocale::English,
        ),
    ))
    .expect("compact candidate");
    let preview = store.preview_candidate(candidate).expect("preview");
    assert_eq!(
        preview.categories(),
        &[SettingsChangeCategory::Presentation]
    );
    assert_eq!(preview.changed_field_count(), 1);

    let all_categories = PortableSettingsCandidate::new(PortableSettings::new(
        ReminderPolicy::new(true, &[3_600]).expect("reminder policy"),
        tokenmaster_state::BackupPolicy::new(
            false,
            BACKUP_QUIET_DEFAULT_SECONDS + 1,
            BACKUP_INTERVAL_DEFAULT_SECONDS + 1,
            BACKUP_RETENTION_MIN_BYTES,
        )
        .expect("backup policy"),
        PresentationSettings::new(
            PresentationDensity::UltraCompact,
            PresentationSkin::Refined,
            PresentationColorScheme::System,
            PresentationLayout::Refined,
            PresentationLocale::English,
        ),
    ))
    .expect("four-category candidate");
    let preview = store
        .preview_candidate(all_categories)
        .expect("four-category preview");
    assert_eq!(
        preview.categories(),
        &[
            SettingsChangeCategory::ReminderProfile,
            SettingsChangeCategory::BackupSchedule,
            SettingsChangeCategory::BackupRetention,
            SettingsChangeCategory::Presentation,
        ]
    );
    assert_eq!(preview.changed_category_count(), 4);
}

#[test]
fn candidate_and_record_versions_and_presentation_are_strict() {
    let defaults = SettingsValue::safe_defaults();
    let current_record = serde_json::to_value(&defaults).expect("current record");
    let current_candidate = json!({
        "schema_version": SETTINGS_SCHEMA_VERSION,
        "portable": current_record["portable"].clone(),
    });
    let duplicate_presentation = br#"{"schema_version":5,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":"system","layout":"refined"},"presentation":{"density":"compact","skin":"refined","color_scheme":"dark","layout":"workbench"}}}"#;
    let duplicate_record_presentation = br#"{"schema_version":5,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":"system","layout":"refined"},"presentation":{"density":"compact","skin":"refined","color_scheme":"dark","layout":"workbench"}},"device":{"last_route":"dashboard"}}"#;

    let mut version_zero_candidate = current_candidate.clone();
    version_zero_candidate["schema_version"] = json!(0);
    let mut version_eight_candidate = current_candidate.clone();
    version_eight_candidate["schema_version"] = json!(8);
    let mut missing_presentation_candidate = current_candidate.clone();
    missing_presentation_candidate["portable"]
        .as_object_mut()
        .expect("portable object")
        .remove("presentation");
    let mut unknown_skin_candidate = current_candidate.clone();
    unknown_skin_candidate["portable"]["presentation"]["skin"] = json!("unsupported");
    let mut invalid_density_candidate = current_candidate.clone();
    invalid_density_candidate["portable"]["presentation"]["density"] = json!("spacious");
    let mut wrong_density_type_candidate = current_candidate.clone();
    wrong_density_type_candidate["portable"]["presentation"]["density"] = json!(1);
    let mut missing_scheme_candidate = current_candidate.clone();
    missing_scheme_candidate["portable"]["presentation"]
        .as_object_mut()
        .expect("presentation object")
        .remove("color_scheme");
    let mut unknown_scheme_candidate = current_candidate.clone();
    unknown_scheme_candidate["portable"]["presentation"]["color_scheme"] = json!("future");
    let mut missing_locale_candidate = current_candidate.clone();
    missing_locale_candidate["portable"]["presentation"]
        .as_object_mut()
        .expect("presentation object")
        .remove("locale");
    let mut unknown_locale_candidate = current_candidate.clone();
    unknown_locale_candidate["portable"]["presentation"]["locale"] = json!("future");
    let mut wrong_locale_type_candidate = current_candidate.clone();
    wrong_locale_type_candidate["portable"]["presentation"]["locale"] = json!(1);

    let (_root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    for (candidate, expected) in [
        (version_zero_candidate, StateErrorCode::UnsupportedVersion),
        (version_eight_candidate, StateErrorCode::UnsupportedVersion),
        (missing_presentation_candidate, StateErrorCode::InvalidInput),
        (unknown_skin_candidate, StateErrorCode::InvalidInput),
        (invalid_density_candidate, StateErrorCode::InvalidInput),
        (wrong_density_type_candidate, StateErrorCode::InvalidInput),
        (missing_scheme_candidate, StateErrorCode::InvalidInput),
        (unknown_scheme_candidate, StateErrorCode::InvalidInput),
        (missing_locale_candidate, StateErrorCode::InvalidInput),
        (unknown_locale_candidate, StateErrorCode::InvalidInput),
        (wrong_locale_type_candidate, StateErrorCode::InvalidInput),
    ] {
        let bytes = serde_json::to_vec(&candidate).expect("candidate bytes");
        assert_eq!(
            store
                .preview_import(&bytes)
                .expect_err("strict candidate")
                .code(),
            expected
        );
    }
    assert_eq!(
        store
            .preview_import(duplicate_presentation)
            .expect_err("duplicate presentation")
            .code(),
        StateErrorCode::InvalidInput
    );
    let duplicate_locale_candidate = br#"{"schema_version":7,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":"system","layout":"refined","locale":"en","locale":"ru","board":[{"key":"plan_usage","visible":true,"collapsed":false},{"key":"code_output","visible":true,"collapsed":false},{"key":"trend","visible":true,"collapsed":false},{"key":"sessions","visible":true,"collapsed":false},{"key":"activity","visible":true,"collapsed":false},{"key":"models","visible":true,"collapsed":false}]}}}"#;
    assert_eq!(
        store
            .preview_import(duplicate_locale_candidate)
            .expect_err("duplicate locale candidate")
            .code(),
        StateErrorCode::InvalidInput
    );

    for (payload, unsupported) in [
        (
            {
                let mut value = current_record.clone();
                value["schema_version"] = json!(0);
                value
            },
            true,
        ),
        (
            {
                let mut value = current_record.clone();
                value["schema_version"] = json!(8);
                value
            },
            true,
        ),
        (
            {
                let mut value = current_record.clone();
                value["portable"]
                    .as_object_mut()
                    .expect("portable object")
                    .remove("presentation");
                value
            },
            false,
        ),
        (
            {
                let mut value = current_record.clone();
                value["portable"]["presentation"]["skin"] = json!("unsupported");
                value
            },
            false,
        ),
        (
            {
                let mut value = current_record.clone();
                value["portable"]["presentation"]["density"] = json!("spacious");
                value
            },
            false,
        ),
        (
            {
                let mut value = current_record.clone();
                value["portable"]["presentation"]["density"] = json!(1);
                value
            },
            false,
        ),
        (
            {
                let mut value = current_record.clone();
                value["portable"]["presentation"]
                    .as_object_mut()
                    .expect("presentation object")
                    .remove("locale");
                value
            },
            false,
        ),
        (
            {
                let mut value = current_record.clone();
                value["portable"]["presentation"]["locale"] = json!("unsupported");
                value
            },
            false,
        ),
        (
            {
                let mut value = current_record.clone();
                value["portable"]["presentation"]["locale"] = json!(1);
                value
            },
            false,
        ),
    ] {
        let (root, directory) = fixture();
        let bytes = serde_json::to_vec(&payload).expect("record payload");
        let record = encode_record(7, &bytes);
        fs::write(root.path().join("settings-a.tms"), &record).expect("invalid record");
        let store = SettingsStore::new(&directory).expect("settings store");
        if unsupported {
            assert_eq!(
                store.load().expect_err("unsupported record").code(),
                StateErrorCode::UnsupportedVersion
            );
        } else {
            assert_eq!(
                store.load().expect("invalid record defaults").outcome(),
                SettingsLoadOutcome::Defaults
            );
        }
        assert_eq!(
            fs::read(root.path().join("settings-a.tms")).unwrap(),
            record
        );
        assert!(!root.path().join("settings-b.tms").exists());
    }

    let (root, directory) = fixture();
    let record = encode_record(7, duplicate_record_presentation);
    fs::write(root.path().join("settings-a.tms"), &record).expect("duplicate record");
    let store = SettingsStore::new(&directory).expect("settings store");
    assert_eq!(
        store.load().expect("duplicate record defaults").outcome(),
        SettingsLoadOutcome::Defaults
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        record
    );
    assert!(!root.path().join("settings-b.tms").exists());

    let duplicate_locale_record = br#"{"schema_version":7,"portable":{"reminders":{"enabled":true,"lead_seconds":[3600]},"backup":{"periodic_enabled":true,"quiet_seconds":300,"interval_seconds":21600,"retention_budget_bytes":2147483648},"presentation":{"density":"comfortable","skin":"refined","color_scheme":"system","layout":"refined","locale":"en","locale":"ru","board":[{"key":"plan_usage","visible":true,"collapsed":false},{"key":"code_output","visible":true,"collapsed":false},{"key":"trend","visible":true,"collapsed":false},{"key":"sessions","visible":true,"collapsed":false},{"key":"activity","visible":true,"collapsed":false},{"key":"models","visible":true,"collapsed":false}]}},"device":{"last_route":"dashboard"}}"#;
    let (root, directory) = fixture();
    let record = encode_record(7, duplicate_locale_record);
    fs::write(root.path().join("settings-a.tms"), &record).expect("duplicate locale record");
    let store = SettingsStore::new(&directory).expect("settings store");
    assert_eq!(
        store.load().expect("record invalid boundary").outcome(),
        SettingsLoadOutcome::Defaults
    );
    assert_eq!(
        fs::read(root.path().join("settings-a.tms")).unwrap(),
        record
    );
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
fn prepared_restore_records_the_exact_target_before_preserving_device_state_once() {
    let (_root, directory) = fixture();
    let store = SettingsStore::new(&directory).expect("settings store");
    store
        .save(&changed_value(
            DeviceRoute::Settings,
            BACKUP_RETENTION_DEFAULT_BYTES,
        ))
        .expect("current settings");
    let candidate = PortableSettingsCandidate::new(
        changed_value(DeviceRoute::History, BACKUP_RETENTION_MIN_BYTES)
            .portable()
            .clone(),
    )
    .expect("restore candidate");

    let prepared = store
        .prepare_restore(&candidate)
        .expect("prepared settings restore");
    assert_eq!(prepared.target().generation(), 2);
    assert_eq!(prepared.target().digest(), candidate.digest());
    assert_eq!(store.load().expect("unchanged load").generation(), Some(1));
    assert_eq!(
        format!("{prepared:?}"),
        "PreparedSettingsRestore([redacted])"
    );

    let committed = store
        .commit_prepared_restore(&prepared)
        .expect("prepared settings commit");
    assert_eq!(committed.target(), prepared.target());
    assert_eq!(
        store
            .load()
            .expect("restored settings")
            .value()
            .device()
            .last_route(),
        DeviceRoute::Settings
    );
    let repeated = store
        .commit_prepared_restore(&prepared)
        .expect("idempotent post-crash commit");
    assert_eq!(repeated.target(), prepared.target());
    assert_eq!(store.load().expect("same generation").generation(), Some(2));
}

#[test]
fn prepared_restore_rejects_a_conflicting_settings_generation() {
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
    .expect("restore candidate");
    let prepared = store.prepare_restore(&candidate).expect("prepared restore");
    store
        .save(&changed_value(
            DeviceRoute::Settings,
            BACKUP_RETENTION_DEFAULT_BYTES,
        ))
        .expect("conflicting settings save");

    assert_eq!(
        store
            .commit_prepared_restore(&prepared)
            .expect_err("conflicting generation")
            .code(),
        StateErrorCode::Integrity
    );
    assert!(
        !store
            .verify_target(prepared.target())
            .expect("target absent")
    );
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
            br#"{"schema_version":8,"portable":{}}"#.as_slice(),
            StateErrorCode::UnsupportedVersion,
        ),
        (
            br#"{"schema_version":0,"portable":{}}"#.as_slice(),
            StateErrorCode::UnsupportedVersion,
        ),
        (
            br#"{"schema_version":2,"portable":{"reminders":[]}}"#.as_slice(),
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
        newer["schema_version"] = json!(8);
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
