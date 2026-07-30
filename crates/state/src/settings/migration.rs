use serde::Deserialize;

use super::value::{
    BackupPolicy, BoardPreferences, DeviceSettings, MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION,
    PortableSettings, PresentationColorScheme, PresentationDensity, PresentationLayout,
    PresentationLocale, PresentationSettings, PresentationSkin, ReminderPolicy,
    SETTINGS_SCHEMA_VERSION, SettingsValue,
};
use crate::StateError;
use crate::record::{MAX_RECORD_PAYLOAD_BYTES, RecordValueError};

#[derive(Deserialize)]
struct VersionProbe {
    schema_version: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableSettingsV1Wire {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
}

impl PortableSettingsV1Wire {
    fn migrate(self) -> PortableSettings {
        PortableSettings::new(
            self.reminders,
            self.backup,
            PresentationSettings::legacy_dark(
                PresentationDensity::Comfortable,
                PresentationSkin::Refined,
            ),
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationSettingsV2Wire {
    density: PresentationDensity,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationSettingsV3Wire {
    density: PresentationDensity,
    skin: PresentationSkin,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationSettingsV4Wire {
    density: PresentationDensity,
    skin: PresentationSkin,
    color_scheme: PresentationColorScheme,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationSettingsV5Wire {
    density: PresentationDensity,
    skin: PresentationSkin,
    color_scheme: PresentationColorScheme,
    layout: PresentationLayout,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationSettingsV6Wire {
    density: PresentationDensity,
    skin: PresentationSkin,
    color_scheme: PresentationColorScheme,
    layout: PresentationLayout,
    board: BoardPreferences,
}

impl PresentationSettingsV6Wire {
    fn migrate(self) -> PresentationSettings {
        PresentationSettings::new(
            self.density,
            self.skin,
            self.color_scheme,
            self.layout,
            PresentationLocale::English,
        )
        .with_board(self.board)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableSettingsV6Wire {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
    presentation: PresentationSettingsV6Wire,
}

impl PortableSettingsV6Wire {
    fn migrate(self) -> PortableSettings {
        PortableSettings::new(self.reminders, self.backup, self.presentation.migrate())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableSettingsV2Wire {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
    presentation: PresentationSettingsV2Wire,
}

impl PortableSettingsV2Wire {
    fn migrate(self) -> PortableSettings {
        PortableSettings::new(
            self.reminders,
            self.backup,
            PresentationSettings::legacy_dark(self.presentation.density, PresentationSkin::Refined),
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableSettingsV3Wire {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
    presentation: PresentationSettingsV3Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableSettingsV4Wire {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
    presentation: PresentationSettingsV4Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableSettingsV5Wire {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
    presentation: PresentationSettingsV5Wire,
}

impl PortableSettingsV5Wire {
    fn migrate(self) -> PortableSettings {
        PortableSettings::new(
            self.reminders,
            self.backup,
            PresentationSettings::new(
                self.presentation.density,
                self.presentation.skin,
                self.presentation.color_scheme,
                self.presentation.layout,
                PresentationLocale::English,
            ),
        )
    }
}

impl PortableSettingsV4Wire {
    fn migrate(self) -> PortableSettings {
        PortableSettings::new(
            self.reminders,
            self.backup,
            PresentationSettings::new(
                self.presentation.density,
                self.presentation.skin,
                self.presentation.color_scheme,
                PresentationLayout::Refined,
                PresentationLocale::English,
            ),
        )
    }
}

impl PortableSettingsV3Wire {
    fn migrate(self) -> PortableSettings {
        PortableSettings::new(
            self.reminders,
            self.backup,
            PresentationSettings::legacy_dark(self.presentation.density, self.presentation.skin),
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateV1Wire {
    schema_version: u16,
    portable: PortableSettingsV1Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateV2Wire {
    schema_version: u16,
    portable: PortableSettingsV2Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateV3Wire {
    schema_version: u16,
    portable: PortableSettingsV3Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateV4Wire {
    schema_version: u16,
    portable: PortableSettingsV4Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateV5Wire {
    schema_version: u16,
    portable: PortableSettingsV5Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateV6Wire {
    schema_version: u16,
    portable: PortableSettingsV6Wire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateV7Wire {
    schema_version: u16,
    portable: PortableSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueV1Wire {
    schema_version: u16,
    portable: PortableSettingsV1Wire,
    device: DeviceSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueV2Wire {
    schema_version: u16,
    portable: PortableSettingsV2Wire,
    device: DeviceSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueV3Wire {
    schema_version: u16,
    portable: PortableSettingsV3Wire,
    device: DeviceSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueV4Wire {
    schema_version: u16,
    portable: PortableSettingsV4Wire,
    device: DeviceSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueV5Wire {
    schema_version: u16,
    portable: PortableSettingsV5Wire,
    device: DeviceSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueV6Wire {
    schema_version: u16,
    portable: PortableSettingsV6Wire,
    device: DeviceSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueV7Wire {
    schema_version: u16,
    portable: PortableSettings,
    device: DeviceSettings,
}

pub(super) struct DecodedPortableSettings {
    pub(super) portable: PortableSettings,
    pub(super) source_schema_version: u16,
}

pub(super) fn decode_portable_candidate(
    bytes: &[u8],
) -> Result<DecodedPortableSettings, StateError> {
    enforce_payload_bound(bytes)?;
    let probe: VersionProbe =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    match probe.schema_version {
        1 => decode_portable_v1(bytes),
        2 => decode_portable_v2(bytes),
        3 => decode_portable_v3(bytes),
        4 => decode_portable_v4(bytes),
        5 => decode_portable_v5(bytes),
        6 => decode_portable_v6(bytes),
        SETTINGS_SCHEMA_VERSION => decode_portable_v7(bytes),
        _ => Err(StateError::unsupported_version()),
    }
}

pub(super) fn decode_settings_record(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let probe: VersionProbe =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    match probe.schema_version {
        1 => decode_settings_v1(bytes),
        2 => decode_settings_v2(bytes),
        3 => decode_settings_v3(bytes),
        4 => decode_settings_v4(bytes),
        5 => decode_settings_v5(bytes),
        6 => decode_settings_v6(bytes),
        SETTINGS_SCHEMA_VERSION => decode_settings_v7(bytes),
        _ => Err(RecordValueError::UnsupportedVersion),
    }
}

fn enforce_payload_bound(bytes: &[u8]) -> Result<(), StateError> {
    let len = u64::try_from(bytes.len()).map_err(|_| StateError::capacity_exceeded())?;
    if len > MAX_RECORD_PAYLOAD_BYTES {
        return Err(StateError::capacity_exceeded());
    }
    Ok(())
}

fn decode_portable_v1(bytes: &[u8]) -> Result<DecodedPortableSettings, StateError> {
    let wire: PortableCandidateV1Wire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION {
        return Err(StateError::unsupported_version());
    }
    Ok(DecodedPortableSettings {
        portable: wire.portable.migrate(),
        source_schema_version: MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION,
    })
}

fn decode_portable_v2(bytes: &[u8]) -> Result<DecodedPortableSettings, StateError> {
    let wire: PortableCandidateV2Wire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != 2 {
        return Err(StateError::unsupported_version());
    }
    Ok(DecodedPortableSettings {
        portable: wire.portable.migrate(),
        source_schema_version: 2,
    })
}

fn decode_portable_v3(bytes: &[u8]) -> Result<DecodedPortableSettings, StateError> {
    let wire: PortableCandidateV3Wire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != 3 {
        return Err(StateError::unsupported_version());
    }
    Ok(DecodedPortableSettings {
        portable: wire.portable.migrate(),
        source_schema_version: 3,
    })
}

fn decode_portable_v4(bytes: &[u8]) -> Result<DecodedPortableSettings, StateError> {
    let wire: PortableCandidateV4Wire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != 4 {
        return Err(StateError::unsupported_version());
    }
    Ok(DecodedPortableSettings {
        portable: wire.portable.migrate(),
        source_schema_version: 4,
    })
}

fn decode_portable_v5(bytes: &[u8]) -> Result<DecodedPortableSettings, StateError> {
    let wire: PortableCandidateV5Wire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != 5 {
        return Err(StateError::unsupported_version());
    }
    Ok(DecodedPortableSettings {
        portable: wire.portable.migrate(),
        source_schema_version: 5,
    })
}

fn decode_portable_v6(bytes: &[u8]) -> Result<DecodedPortableSettings, StateError> {
    let wire: PortableCandidateV6Wire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != 6 {
        return Err(StateError::unsupported_version());
    }
    Ok(DecodedPortableSettings {
        portable: wire.portable.migrate(),
        source_schema_version: 6,
    })
}

fn decode_portable_v7(bytes: &[u8]) -> Result<DecodedPortableSettings, StateError> {
    let wire: PortableCandidateV7Wire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(StateError::unsupported_version());
    }
    Ok(DecodedPortableSettings {
        portable: wire.portable,
        source_schema_version: SETTINGS_SCHEMA_VERSION,
    })
}

fn decode_settings_v1(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let wire: SettingsValueV1Wire =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    if wire.schema_version != MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION {
        return Err(RecordValueError::UnsupportedVersion);
    }
    Ok(SettingsValue::new(wire.portable.migrate(), wire.device))
}

fn decode_settings_v2(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let wire: SettingsValueV2Wire =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    if wire.schema_version != 2 {
        return Err(RecordValueError::UnsupportedVersion);
    }
    Ok(SettingsValue::new(wire.portable.migrate(), wire.device))
}

fn decode_settings_v3(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let wire: SettingsValueV3Wire =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    if wire.schema_version != 3 {
        return Err(RecordValueError::UnsupportedVersion);
    }
    Ok(SettingsValue::new(wire.portable.migrate(), wire.device))
}

fn decode_settings_v4(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let wire: SettingsValueV4Wire =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    if wire.schema_version != 4 {
        return Err(RecordValueError::UnsupportedVersion);
    }
    Ok(SettingsValue::new(wire.portable.migrate(), wire.device))
}

fn decode_settings_v5(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let wire: SettingsValueV5Wire =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    if wire.schema_version != 5 {
        return Err(RecordValueError::UnsupportedVersion);
    }
    Ok(SettingsValue::new(wire.portable.migrate(), wire.device))
}

fn decode_settings_v6(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let wire: SettingsValueV6Wire =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    if wire.schema_version != 6 {
        return Err(RecordValueError::UnsupportedVersion);
    }
    Ok(SettingsValue::new(wire.portable.migrate(), wire.device))
}

fn decode_settings_v7(bytes: &[u8]) -> Result<SettingsValue, RecordValueError> {
    let wire: SettingsValueV7Wire =
        serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
    if wire.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(RecordValueError::UnsupportedVersion);
    }
    Ok(SettingsValue::new(wire.portable, wire.device))
}
