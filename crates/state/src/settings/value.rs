use core::fmt;

use serde::de::{Error as _, SeqAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::StateError;
use crate::record::{RecordValue, RecordValueError};

pub const SETTINGS_SCHEMA_VERSION: u16 = 4;
pub(crate) const MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION: u16 = 1;
pub const MAX_REMINDER_THRESHOLDS: usize = 8;
pub const REMINDER_LEAD_MIN_SECONDS: u32 = 60;
pub const REMINDER_LEAD_MAX_SECONDS: u32 = 365 * 24 * 60 * 60;
pub const BACKUP_QUIET_MIN_SECONDS: u32 = 5 * 60;
pub const BACKUP_QUIET_DEFAULT_SECONDS: u32 = 5 * 60;
pub const BACKUP_QUIET_MAX_SECONDS: u32 = 60 * 60;
pub const BACKUP_INTERVAL_MIN_SECONDS: u32 = 6 * 60 * 60;
pub const BACKUP_INTERVAL_DEFAULT_SECONDS: u32 = 6 * 60 * 60;
pub const BACKUP_INTERVAL_MAX_SECONDS: u32 = 7 * 24 * 60 * 60;
pub const BACKUP_RETENTION_MIN_BYTES: u64 = 256 * 1024 * 1024;
pub const BACKUP_RETENTION_DEFAULT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const BACKUP_RETENTION_MAX_BYTES: u64 = 64 * 1024 * 1024 * 1024;

const RECOMMENDED_REMINDER_LEADS: [u32; 5] = [604_800, 86_400, 43_200, 21_600, 3_600];

#[derive(Clone, Eq, PartialEq)]
pub struct ReminderPolicy {
    enabled: bool,
    lead_seconds: Box<[u32]>,
}

impl ReminderPolicy {
    pub fn new(enabled: bool, lead_seconds: &[u32]) -> Result<Self, StateError> {
        if lead_seconds.len() > MAX_REMINDER_THRESHOLDS {
            return Err(StateError::capacity_exceeded());
        }
        if enabled == lead_seconds.is_empty() {
            return Err(StateError::invalid_input());
        }
        for (index, lead) in lead_seconds.iter().enumerate() {
            if !(REMINDER_LEAD_MIN_SECONDS..=REMINDER_LEAD_MAX_SECONDS).contains(lead)
                || lead_seconds[..index].contains(lead)
            {
                return Err(StateError::invalid_input());
            }
        }
        let mut normalized = lead_seconds.to_vec();
        normalized.sort_unstable_by(|left, right| right.cmp(left));
        Ok(Self {
            enabled,
            lead_seconds: normalized.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn lead_seconds(&self) -> &[u32] {
        &self.lead_seconds
    }
}

impl fmt::Debug for ReminderPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ReminderPolicy([redacted])")
    }
}

impl Serialize for ReminderPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ReminderPolicy", 2)?;
        state.serialize_field("enabled", &self.enabled)?;
        state.serialize_field("lead_seconds", &self.lead_seconds)?;
        state.end()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReminderPolicyWire {
    enabled: bool,
    lead_seconds: BoundedLeadSeconds,
}

struct BoundedLeadSeconds(Vec<u32>);

impl<'de> Deserialize<'de> for BoundedLeadSeconds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LeadVisitor;

        impl<'de> Visitor<'de> for LeadVisitor {
            type Value = BoundedLeadSeconds;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded reminder lead sequence")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut leads = Vec::with_capacity(MAX_REMINDER_THRESHOLDS);
                while let Some(lead) = sequence.next_element()? {
                    if leads.len() == MAX_REMINDER_THRESHOLDS {
                        return Err(A::Error::custom("reminder lead capacity exceeded"));
                    }
                    leads.push(lead);
                }
                Ok(BoundedLeadSeconds(leads))
            }
        }

        deserializer.deserialize_seq(LeadVisitor)
    }
}

impl<'de> Deserialize<'de> for ReminderPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ReminderPolicyWire::deserialize(deserializer)?;
        Self::new(wire.enabled, &wire.lead_seconds.0)
            .map_err(|_| D::Error::custom("invalid policy"))
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BackupPolicy {
    periodic_enabled: bool,
    quiet_seconds: u32,
    interval_seconds: u32,
    retention_budget_bytes: u64,
}

impl BackupPolicy {
    pub fn new(
        periodic_enabled: bool,
        quiet_seconds: u32,
        interval_seconds: u32,
        retention_budget_bytes: u64,
    ) -> Result<Self, StateError> {
        if !(BACKUP_QUIET_MIN_SECONDS..=BACKUP_QUIET_MAX_SECONDS).contains(&quiet_seconds)
            || !(BACKUP_INTERVAL_MIN_SECONDS..=BACKUP_INTERVAL_MAX_SECONDS)
                .contains(&interval_seconds)
            || quiet_seconds >= interval_seconds
        {
            return Err(StateError::invalid_input());
        }
        if !(BACKUP_RETENTION_MIN_BYTES..=BACKUP_RETENTION_MAX_BYTES)
            .contains(&retention_budget_bytes)
        {
            return Err(StateError::invalid_input());
        }
        Ok(Self {
            periodic_enabled,
            quiet_seconds,
            interval_seconds,
            retention_budget_bytes,
        })
    }

    #[must_use]
    pub const fn periodic_enabled(&self) -> bool {
        self.periodic_enabled
    }

    #[must_use]
    pub const fn quiet_seconds(&self) -> u32 {
        self.quiet_seconds
    }

    #[must_use]
    pub const fn interval_seconds(&self) -> u32 {
        self.interval_seconds
    }

    #[must_use]
    pub const fn retention_budget_bytes(&self) -> u64 {
        self.retention_budget_bytes
    }
}

impl fmt::Debug for BackupPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupPolicy([redacted])")
    }
}

impl Serialize for BackupPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("BackupPolicy", 4)?;
        state.serialize_field("periodic_enabled", &self.periodic_enabled)?;
        state.serialize_field("quiet_seconds", &self.quiet_seconds)?;
        state.serialize_field("interval_seconds", &self.interval_seconds)?;
        state.serialize_field("retention_budget_bytes", &self.retention_budget_bytes)?;
        state.end()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupPolicyWire {
    periodic_enabled: bool,
    quiet_seconds: u32,
    interval_seconds: u32,
    retention_budget_bytes: u64,
}

impl<'de> Deserialize<'de> for BackupPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = BackupPolicyWire::deserialize(deserializer)?;
        Self::new(
            wire.periodic_enabled,
            wire.quiet_seconds,
            wire.interval_seconds,
            wire.retention_budget_bytes,
        )
        .map_err(|_| D::Error::custom("invalid policy"))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRoute {
    Dashboard,
    History,
    Sessions,
    Models,
    Projects,
    Activity,
    DataHealth,
    Notifications,
    Settings,
    HelpAbout,
    CompactWidget,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationDensity {
    Comfortable,
    Compact,
    UltraCompact,
}

impl PresentationDensity {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
            Self::UltraCompact => "ultra_compact",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationSkin {
    Refined,
    Graphite,
    Ember,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationColorScheme {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationSettings {
    density: PresentationDensity,
    skin: PresentationSkin,
    color_scheme: PresentationColorScheme,
}

impl PresentationSettings {
    #[must_use]
    pub const fn new(
        density: PresentationDensity,
        skin: PresentationSkin,
        color_scheme: PresentationColorScheme,
    ) -> Self {
        Self {
            density,
            skin,
            color_scheme,
        }
    }

    #[must_use]
    pub const fn refined() -> Self {
        Self::new(
            PresentationDensity::Comfortable,
            PresentationSkin::Refined,
            PresentationColorScheme::System,
        )
    }

    #[must_use]
    pub(crate) const fn legacy_dark(density: PresentationDensity, skin: PresentationSkin) -> Self {
        Self::new(density, skin, PresentationColorScheme::Dark)
    }

    #[must_use]
    pub const fn density(self) -> PresentationDensity {
        self.density
    }

    #[must_use]
    pub const fn skin(self) -> PresentationSkin {
        self.skin
    }

    #[must_use]
    pub const fn color_scheme(self) -> PresentationColorScheme {
        self.color_scheme
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PortableSettings {
    reminders: ReminderPolicy,
    backup: BackupPolicy,
    presentation: PresentationSettings,
}

impl PortableSettings {
    #[must_use]
    pub const fn new(
        reminders: ReminderPolicy,
        backup: BackupPolicy,
        presentation: PresentationSettings,
    ) -> Self {
        Self {
            reminders,
            backup,
            presentation,
        }
    }

    #[must_use]
    pub const fn reminders(&self) -> &ReminderPolicy {
        &self.reminders
    }

    #[must_use]
    pub const fn backup(&self) -> &BackupPolicy {
        &self.backup
    }

    #[must_use]
    pub const fn presentation(&self) -> &PresentationSettings {
        &self.presentation
    }
}

impl fmt::Debug for PortableSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PortableSettings([redacted])")
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSettings {
    last_route: DeviceRoute,
}

impl DeviceSettings {
    #[must_use]
    pub const fn new(last_route: DeviceRoute) -> Self {
        Self { last_route }
    }

    #[must_use]
    pub const fn last_route(&self) -> DeviceRoute {
        self.last_route
    }
}

impl fmt::Debug for DeviceSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DeviceSettings([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SettingsValue {
    portable: PortableSettings,
    device: DeviceSettings,
}

impl SettingsValue {
    #[must_use]
    pub const fn new(portable: PortableSettings, device: DeviceSettings) -> Self {
        Self { portable, device }
    }

    #[must_use]
    pub fn safe_defaults() -> Self {
        let reminders = ReminderPolicy::new(true, &RECOMMENDED_REMINDER_LEADS)
            .unwrap_or_else(|_| unreachable!("fixed reminder defaults are valid"));
        let backup = BackupPolicy::new(
            true,
            BACKUP_QUIET_DEFAULT_SECONDS,
            BACKUP_INTERVAL_DEFAULT_SECONDS,
            BACKUP_RETENTION_DEFAULT_BYTES,
        )
        .unwrap_or_else(|_| unreachable!("fixed backup defaults are valid"));
        Self::new(
            PortableSettings::new(reminders, backup, PresentationSettings::refined()),
            DeviceSettings::new(DeviceRoute::Dashboard),
        )
    }

    #[must_use]
    pub const fn portable(&self) -> &PortableSettings {
        &self.portable
    }

    #[must_use]
    pub const fn device(&self) -> &DeviceSettings {
        &self.device
    }
}

impl fmt::Debug for SettingsValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SettingsValue([redacted])")
    }
}

impl Serialize for SettingsValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SettingsValue", 3)?;
        state.serialize_field("schema_version", &SETTINGS_SCHEMA_VERSION)?;
        state.serialize_field("portable", &self.portable)?;
        state.serialize_field("device", &self.device)?;
        state.end()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsValueWire {
    schema_version: u16,
    portable: PortableSettings,
    device: DeviceSettings,
}

impl<'de> Deserialize<'de> for SettingsValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SettingsValueWire::deserialize(deserializer)?;
        if wire.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(D::Error::custom("unsupported version"));
        }
        Ok(Self::new(wire.portable, wire.device))
    }
}

impl RecordValue for SettingsValue {
    fn decode_json(bytes: &[u8]) -> Result<Self, RecordValueError> {
        super::migration::decode_settings_record(bytes)
    }
}
