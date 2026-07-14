use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_USAGE_ID_BYTES: usize = 128;
pub const MAX_SESSION_ID_BYTES: usize = 512;
pub const MAX_MODEL_KEY_BYTES: usize = 64;
pub const MAX_METADATA_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UsageError {
    #[error("{field} must contain between 1 and {max_bytes} UTF-8 bytes")]
    InvalidText {
        field: &'static str,
        max_bytes: usize,
    },
    #[error("{field} contains unsupported characters")]
    InvalidCharacters { field: &'static str },
    #[error("timestamp nanoseconds must be below one billion")]
    InvalidTimestamp,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TokenCount {
    Available(u64),
    #[default]
    Unavailable,
}

impl Serialize for TokenCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Available(value) => serializer.serialize_u64(*value),
            Self::Unavailable => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for TokenCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<u64>::deserialize(deserializer)
            .map(|value| value.map_or(Self::Unavailable, Self::Available))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    input: TokenCount,
    cached: TokenCount,
    output: TokenCount,
    reasoning: TokenCount,
    total: TokenCount,
}

impl TokenUsage {
    #[must_use]
    pub const fn new(
        input: TokenCount,
        cached: TokenCount,
        output: TokenCount,
        reasoning: TokenCount,
        total: TokenCount,
    ) -> Self {
        Self {
            input,
            cached,
            output,
            reasoning,
            total,
        }
    }

    #[must_use]
    pub const fn input(&self) -> TokenCount {
        self.input
    }

    #[must_use]
    pub const fn cached(&self) -> TokenCount {
        self.cached
    }

    #[must_use]
    pub const fn output(&self) -> TokenCount {
        self.output
    }

    #[must_use]
    pub const fn reasoning(&self) -> TokenCount {
        self.reasoning
    }

    #[must_use]
    pub const fn total(&self) -> TokenCount {
        self.total
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongContextState {
    Yes,
    No,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Read,
    EditWrite,
    Search,
    Git,
    BuildTest,
    Web,
    Subagents,
    Terminal,
}

impl ActivityKind {
    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivityCounts([u64; 8]);

impl ActivityCounts {
    pub fn add(&mut self, kind: ActivityKind, count: u64) {
        let slot = &mut self.0[kind.index()];
        *slot = slot.saturating_add(count);
    }

    pub fn increment(&mut self, kind: ActivityKind) {
        self.add(kind, 1);
    }

    #[must_use]
    pub const fn get(&self, kind: ActivityKind) -> u64 {
        self.0[kind.index()]
    }

    #[must_use]
    pub const fn as_array(&self) -> &[u64; 8] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct UtcTimestamp {
    unix_seconds: i64,
    subsec_nanos: u32,
}

impl UtcTimestamp {
    pub fn new(unix_seconds: i64, subsec_nanos: u32) -> Result<Self, UsageError> {
        if subsec_nanos >= 1_000_000_000 {
            return Err(UsageError::InvalidTimestamp);
        }
        Ok(Self {
            unix_seconds,
            subsec_nanos,
        })
    }

    #[must_use]
    pub const fn unix_seconds(&self) -> i64 {
        self.unix_seconds
    }

    #[must_use]
    pub const fn subsec_nanos(&self) -> u32 {
        self.subsec_nanos
    }
}

#[derive(Deserialize)]
struct UtcTimestampWire {
    unix_seconds: i64,
    subsec_nanos: u32,
}

impl<'de> Deserialize<'de> for UtcTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = UtcTimestampWire::deserialize(deserializer)?;
        Self::new(wire.unix_seconds, wire.subsec_nanos).map_err(serde::de::Error::custom)
    }
}

fn is_usage_id_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

fn is_model_key_byte(byte: u8) -> bool {
    is_usage_id_byte(byte) || matches!(byte, b'/' | b':')
}

fn validate_ascii_value(
    value: String,
    field: &'static str,
    max_bytes: usize,
    is_allowed: fn(u8) -> bool,
) -> Result<Box<str>, UsageError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(UsageError::InvalidText { field, max_bytes });
    }
    if !value.bytes().all(is_allowed) {
        return Err(UsageError::InvalidCharacters { field });
    }
    Ok(value.into_boxed_str())
}

fn validate_trimmed_value(
    value: String,
    field: &'static str,
    max_bytes: usize,
) -> Result<Box<str>, UsageError> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_bytes {
        return Err(UsageError::InvalidText { field, max_bytes });
    }
    if value.chars().any(char::is_control) {
        return Err(UsageError::InvalidCharacters { field });
    }
    Ok(value.to_owned().into_boxed_str())
}

macro_rules! ascii_value {
    ($name:ident, $field:literal, $max_bytes:expr, $validator:path) => {
        #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, UsageError> {
                validate_ascii_value(value.into(), $field, $max_bytes, $validator).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

macro_rules! trimmed_value {
    ($name:ident, $field:literal, $max_bytes:expr) => {
        #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, UsageError> {
                validate_trimmed_value(value.into(), $field, $max_bytes).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

ascii_value!(
    UsageProfileId,
    "profile_id",
    MAX_USAGE_ID_BYTES,
    is_usage_id_byte
);
ascii_value!(
    UsageSourceId,
    "source_id",
    MAX_USAGE_ID_BYTES,
    is_usage_id_byte
);
ascii_value!(ModelKey, "model", MAX_MODEL_KEY_BYTES, is_model_key_byte);
trimmed_value!(UsageSessionId, "session_id", MAX_SESSION_ID_BYTES);
trimmed_value!(MetadataValue, "metadata", MAX_METADATA_BYTES);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProjectAlias(Box<str>);

impl ProjectAlias {
    pub fn new(value: impl Into<String>) -> Result<Self, UsageError> {
        let value = validate_trimmed_value(value.into(), "project", MAX_METADATA_BYTES)?;
        if value.bytes().any(|byte| matches!(byte, b'/' | b'\\')) {
            return Err(UsageError::InvalidCharacters { field: "project" });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProjectAlias {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CanonicalUsageEventParts {
    pub profile_id: UsageProfileId,
    pub session_id: UsageSessionId,
    pub source_id: UsageSourceId,
    pub source_offset: u64,
    pub timestamp: UtcTimestamp,
    pub model: ModelKey,
    pub raw_model: Option<MetadataValue>,
    pub usage: TokenUsage,
    pub fallback_model: bool,
    pub long_context: LongContextState,
    pub service_tier: Option<MetadataValue>,
    pub project: Option<ProjectAlias>,
    pub originator: Option<MetadataValue>,
    pub activity: ActivityCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct EventFingerprint([u8; 32]);

impl EventFingerprint {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            push_hex_byte(&mut output, byte);
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
#[serde(transparent)]
pub struct EventId(Box<str>);

impl EventId {
    fn from_fingerprint(fingerprint: EventFingerprint) -> Self {
        let mut value = String::with_capacity(26);
        value.push_str("event_");
        for byte in fingerprint.0.into_iter().take(10) {
            push_hex_byte(&mut value, byte);
        }
        Self(value.into_boxed_str())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn push_hex_byte(output: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(char::from(HEX[usize::from(byte >> 4)]));
    output.push(char::from(HEX[usize::from(byte & 0x0f)]));
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CanonicalUsageEvent {
    parts: CanonicalUsageEventParts,
    fingerprint: EventFingerprint,
    id: EventId,
}

impl CanonicalUsageEvent {
    #[must_use]
    pub fn new(parts: CanonicalUsageEventParts, fingerprint: EventFingerprint) -> Self {
        Self {
            parts,
            fingerprint,
            id: EventId::from_fingerprint(fingerprint),
        }
    }

    #[must_use]
    pub const fn profile_id(&self) -> &UsageProfileId {
        &self.parts.profile_id
    }

    #[must_use]
    pub const fn session_id(&self) -> &UsageSessionId {
        &self.parts.session_id
    }

    #[must_use]
    pub const fn source_id(&self) -> &UsageSourceId {
        &self.parts.source_id
    }

    #[must_use]
    pub const fn source_offset(&self) -> u64 {
        self.parts.source_offset
    }

    #[must_use]
    pub const fn timestamp(&self) -> &UtcTimestamp {
        &self.parts.timestamp
    }

    #[must_use]
    pub const fn model(&self) -> &ModelKey {
        &self.parts.model
    }

    #[must_use]
    pub fn raw_model(&self) -> Option<&MetadataValue> {
        self.parts.raw_model.as_ref()
    }

    #[must_use]
    pub const fn usage(&self) -> &TokenUsage {
        &self.parts.usage
    }

    #[must_use]
    pub const fn fallback_model(&self) -> bool {
        self.parts.fallback_model
    }

    #[must_use]
    pub const fn long_context(&self) -> LongContextState {
        self.parts.long_context
    }

    #[must_use]
    pub fn service_tier(&self) -> Option<&MetadataValue> {
        self.parts.service_tier.as_ref()
    }

    #[must_use]
    pub fn project(&self) -> Option<&ProjectAlias> {
        self.parts.project.as_ref()
    }

    #[must_use]
    pub fn originator(&self) -> Option<&MetadataValue> {
        self.parts.originator.as_ref()
    }

    #[must_use]
    pub const fn activity(&self) -> &ActivityCounts {
        &self.parts.activity
    }

    #[must_use]
    pub const fn fingerprint(&self) -> &EventFingerprint {
        &self.fingerprint
    }

    #[must_use]
    pub const fn id(&self) -> &EventId {
        &self.id
    }
}
