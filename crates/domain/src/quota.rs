use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::usage::UsageProviderId;

const MAX_QUOTA_ID_BYTES: usize = 128;
const MAX_QUOTA_LABEL_KEY_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum QuotaError {
    #[error("{field} must be a non-empty bounded ASCII identifier")]
    InvalidId { field: &'static str },
    #[error("quota ratio must be within 0..=1000000 parts per million")]
    InvalidRatio,
    #[error("quota units require at least one numeric value")]
    EmptyUnits,
    #[error("{field} cannot exceed capacity")]
    UnitsExceedCapacity { field: &'static str },
    #[error("quota reset thresholds require a post-reset boundary")]
    EmptyResetThresholds,
    #[error("minimum used-ratio drop must be greater than zero")]
    InvalidMinimumDrop,
    #[error("quota definition revision must be positive")]
    InvalidDefinitionRevision,
    #[error("quota definition label key must be a bounded ASCII identifier")]
    InvalidLabelKey,
    #[error("quota nominal duration must be positive")]
    InvalidNominalDuration,
    #[error("quota reset thresholds are valid only for fixed windows")]
    ThresholdsRequireFixedWindow,
    #[error("quota sample times must satisfy 0 < observed <= fresh <= stale")]
    InvalidSampleTimes,
    #[error("advertised reset time must be positive")]
    InvalidAdvertisedResetTime,
    #[error("quota sample contains no observable quota fact")]
    EmptySample,
    #[error("exact reset time requires explicit reset evidence")]
    ResetTimeWithoutEvidence,
    #[error("exact reset time must be within 1..=observed_at")]
    InvalidResetOccurredAt,
}

fn valid_quota_id(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn validate_quota_id(
    value: String,
    field: &'static str,
    max_bytes: usize,
) -> Result<Box<str>, QuotaError> {
    if !valid_quota_id(&value, max_bytes) {
        return Err(QuotaError::InvalidId { field });
    }
    Ok(value.into_boxed_str())
}

macro_rules! quota_id {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, QuotaError> {
                validate_quota_id(value.into(), $field, MAX_QUOTA_ID_BYTES).map(Self)
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

quota_id!(QuotaAccountId, "account_id");
quota_id!(QuotaWorkspaceId, "workspace_id");
quota_id!(QuotaWindowId, "window_id");
quota_id!(QuotaUnitId, "unit_id");
quota_id!(QuotaProviderEpochId, "provider_epoch_id");

#[derive(Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuotaObservationId([u8; 32]);

impl QuotaObservationId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for QuotaObservationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QuotaObservationId([redacted])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaScope {
    provider_id: UsageProviderId,
    account_id: QuotaAccountId,
    workspace_id: Option<QuotaWorkspaceId>,
}

impl QuotaScope {
    #[must_use]
    pub const fn new(
        provider_id: UsageProviderId,
        account_id: QuotaAccountId,
        workspace_id: Option<QuotaWorkspaceId>,
    ) -> Self {
        Self {
            provider_id,
            account_id,
            workspace_id,
        }
    }

    #[must_use]
    pub const fn provider_id(&self) -> &UsageProviderId {
        &self.provider_id
    }

    #[must_use]
    pub const fn account_id(&self) -> &QuotaAccountId {
        &self.account_id
    }

    #[must_use]
    pub const fn workspace_id(&self) -> Option<&QuotaWorkspaceId> {
        self.workspace_id.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaWindowKey {
    scope: QuotaScope,
    window_id: QuotaWindowId,
}

impl QuotaWindowKey {
    #[must_use]
    pub const fn new(scope: QuotaScope, window_id: QuotaWindowId) -> Self {
        Self { scope, window_id }
    }

    #[must_use]
    pub const fn scope(&self) -> &QuotaScope {
        &self.scope
    }

    #[must_use]
    pub const fn window_id(&self) -> &QuotaWindowId {
        &self.window_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
#[serde(transparent)]
pub struct QuotaRatio(u32);

impl QuotaRatio {
    pub fn new(parts_per_million: u32) -> Result<Self, QuotaError> {
        if parts_per_million > 1_000_000 {
            return Err(QuotaError::InvalidRatio);
        }
        Ok(Self(parts_per_million))
    }

    #[must_use]
    pub const fn parts_per_million(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for QuotaRatio {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QuotaUnits {
    unit_id: QuotaUnitId,
    used: Option<u64>,
    remaining: Option<u64>,
    capacity: Option<u64>,
}

impl QuotaUnits {
    pub fn new(
        unit_id: QuotaUnitId,
        used: Option<u64>,
        remaining: Option<u64>,
        capacity: Option<u64>,
    ) -> Result<Self, QuotaError> {
        if used.is_none() && remaining.is_none() && capacity.is_none() {
            return Err(QuotaError::EmptyUnits);
        }
        if used
            .zip(capacity)
            .is_some_and(|(used, capacity)| used > capacity)
        {
            return Err(QuotaError::UnitsExceedCapacity { field: "used" });
        }
        if remaining
            .zip(capacity)
            .is_some_and(|(remaining, capacity)| remaining > capacity)
        {
            return Err(QuotaError::UnitsExceedCapacity { field: "remaining" });
        }
        Ok(Self {
            unit_id,
            used,
            remaining,
            capacity,
        })
    }

    #[must_use]
    pub const fn unit_id(&self) -> &QuotaUnitId {
        &self.unit_id
    }

    #[must_use]
    pub const fn used(&self) -> Option<u64> {
        self.used
    }

    #[must_use]
    pub const fn remaining(&self) -> Option<u64> {
        self.remaining
    }

    #[must_use]
    pub const fn capacity(&self) -> Option<u64> {
        self.capacity
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaUnitsWire {
    unit_id: QuotaUnitId,
    used: Option<u64>,
    remaining: Option<u64>,
    capacity: Option<u64>,
}

impl<'de> Deserialize<'de> for QuotaUnits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = QuotaUnitsWire::deserialize(deserializer)?;
        Self::new(wire.unit_id, wire.used, wire.remaining, wire.capacity)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaWindowSemantics {
    Fixed,
    Rolling,
    Credit,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaPresentationDirection {
    Used,
    Remaining,
    Pace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaSampleQuality {
    Authoritative,
    Partial,
    Conflict,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaEvidenceSource {
    ProviderLocal,
    ProviderOfficial,
    LocalResetEvent,
    Manual,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaResetEvidence {
    None,
    ExplicitProvider,
    ExplicitLocal,
    ManualOrBanked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QuotaResetThresholds {
    maximum_post_reset_used_ratio: Option<QuotaRatio>,
    minimum_post_reset_remaining_ratio: Option<QuotaRatio>,
    minimum_used_ratio_drop: Option<QuotaRatio>,
}

impl QuotaResetThresholds {
    pub fn new(
        maximum_post_reset_used_ratio: Option<QuotaRatio>,
        minimum_post_reset_remaining_ratio: Option<QuotaRatio>,
        minimum_used_ratio_drop: Option<QuotaRatio>,
    ) -> Result<Self, QuotaError> {
        if maximum_post_reset_used_ratio.is_none() && minimum_post_reset_remaining_ratio.is_none() {
            return Err(QuotaError::EmptyResetThresholds);
        }
        if minimum_used_ratio_drop.is_some_and(|ratio| ratio.parts_per_million() == 0) {
            return Err(QuotaError::InvalidMinimumDrop);
        }
        Ok(Self {
            maximum_post_reset_used_ratio,
            minimum_post_reset_remaining_ratio,
            minimum_used_ratio_drop,
        })
    }

    #[must_use]
    pub const fn maximum_post_reset_used_ratio(&self) -> Option<QuotaRatio> {
        self.maximum_post_reset_used_ratio
    }

    #[must_use]
    pub const fn minimum_post_reset_remaining_ratio(&self) -> Option<QuotaRatio> {
        self.minimum_post_reset_remaining_ratio
    }

    #[must_use]
    pub const fn minimum_used_ratio_drop(&self) -> Option<QuotaRatio> {
        self.minimum_used_ratio_drop
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaResetThresholdsWire {
    maximum_post_reset_used_ratio: Option<QuotaRatio>,
    minimum_post_reset_remaining_ratio: Option<QuotaRatio>,
    minimum_used_ratio_drop: Option<QuotaRatio>,
}

impl<'de> Deserialize<'de> for QuotaResetThresholds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = QuotaResetThresholdsWire::deserialize(deserializer)?;
        Self::new(
            wire.maximum_post_reset_used_ratio,
            wire.minimum_post_reset_remaining_ratio,
            wire.minimum_used_ratio_drop,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaWindowDefinitionParts {
    pub key: QuotaWindowKey,
    pub revision: u64,
    pub label_key: String,
    pub presentation: QuotaPresentationDirection,
    pub semantics: QuotaWindowSemantics,
    pub nominal_duration_seconds: Option<u64>,
    pub reset_thresholds: Option<QuotaResetThresholds>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QuotaWindowDefinition {
    key: QuotaWindowKey,
    revision: u64,
    label_key: Box<str>,
    presentation: QuotaPresentationDirection,
    semantics: QuotaWindowSemantics,
    nominal_duration_seconds: Option<u64>,
    reset_thresholds: Option<QuotaResetThresholds>,
}

impl QuotaWindowDefinition {
    pub fn new(parts: QuotaWindowDefinitionParts) -> Result<Self, QuotaError> {
        if parts.revision == 0 {
            return Err(QuotaError::InvalidDefinitionRevision);
        }
        if !valid_quota_id(&parts.label_key, MAX_QUOTA_LABEL_KEY_BYTES) {
            return Err(QuotaError::InvalidLabelKey);
        }
        if parts.nominal_duration_seconds == Some(0) {
            return Err(QuotaError::InvalidNominalDuration);
        }
        if parts.reset_thresholds.is_some() && parts.semantics != QuotaWindowSemantics::Fixed {
            return Err(QuotaError::ThresholdsRequireFixedWindow);
        }
        Ok(Self {
            key: parts.key,
            revision: parts.revision,
            label_key: parts.label_key.into_boxed_str(),
            presentation: parts.presentation,
            semantics: parts.semantics,
            nominal_duration_seconds: parts.nominal_duration_seconds,
            reset_thresholds: parts.reset_thresholds,
        })
    }

    #[must_use]
    pub const fn key(&self) -> &QuotaWindowKey {
        &self.key
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn label_key(&self) -> &str {
        &self.label_key
    }

    #[must_use]
    pub const fn presentation(&self) -> QuotaPresentationDirection {
        self.presentation
    }

    #[must_use]
    pub const fn semantics(&self) -> QuotaWindowSemantics {
        self.semantics
    }

    #[must_use]
    pub const fn nominal_duration_seconds(&self) -> Option<u64> {
        self.nominal_duration_seconds
    }

    #[must_use]
    pub const fn reset_thresholds(&self) -> Option<&QuotaResetThresholds> {
        self.reset_thresholds.as_ref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaWindowDefinitionWire {
    key: QuotaWindowKey,
    revision: u64,
    label_key: String,
    presentation: QuotaPresentationDirection,
    semantics: QuotaWindowSemantics,
    nominal_duration_seconds: Option<u64>,
    reset_thresholds: Option<QuotaResetThresholds>,
}

impl<'de> Deserialize<'de> for QuotaWindowDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = QuotaWindowDefinitionWire::deserialize(deserializer)?;
        Self::new(QuotaWindowDefinitionParts {
            key: wire.key,
            revision: wire.revision,
            label_key: wire.label_key,
            presentation: wire.presentation,
            semantics: wire.semantics,
            nominal_duration_seconds: wire.nominal_duration_seconds,
            reset_thresholds: wire.reset_thresholds,
        })
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaSampleParts {
    pub key: QuotaWindowKey,
    pub observation_id: QuotaObservationId,
    pub observed_at_ms: i64,
    pub fresh_until_ms: i64,
    pub stale_after_ms: i64,
    pub provider_epoch_id: Option<QuotaProviderEpochId>,
    pub used_ratio: Option<QuotaRatio>,
    pub remaining_ratio: Option<QuotaRatio>,
    pub units: Option<QuotaUnits>,
    pub advertised_resets_at_ms: Option<i64>,
    pub quality: QuotaSampleQuality,
    pub source: QuotaEvidenceSource,
    pub confidence: QuotaConfidence,
    pub reset_evidence: QuotaResetEvidence,
    pub reset_occurred_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QuotaSample {
    key: QuotaWindowKey,
    observation_id: QuotaObservationId,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    provider_epoch_id: Option<QuotaProviderEpochId>,
    used_ratio: Option<QuotaRatio>,
    remaining_ratio: Option<QuotaRatio>,
    units: Option<QuotaUnits>,
    advertised_resets_at_ms: Option<i64>,
    quality: QuotaSampleQuality,
    source: QuotaEvidenceSource,
    confidence: QuotaConfidence,
    reset_evidence: QuotaResetEvidence,
    reset_occurred_at_ms: Option<i64>,
}

impl QuotaSample {
    pub fn new(parts: QuotaSampleParts) -> Result<Self, QuotaError> {
        if parts.observed_at_ms <= 0
            || parts.observed_at_ms > parts.fresh_until_ms
            || parts.fresh_until_ms > parts.stale_after_ms
        {
            return Err(QuotaError::InvalidSampleTimes);
        }
        if parts
            .advertised_resets_at_ms
            .is_some_and(|reset_time| reset_time <= 0)
        {
            return Err(QuotaError::InvalidAdvertisedResetTime);
        }
        let has_explicit_reset = parts.reset_evidence != QuotaResetEvidence::None;
        if parts.provider_epoch_id.is_none()
            && parts.used_ratio.is_none()
            && parts.remaining_ratio.is_none()
            && parts.units.is_none()
            && parts.advertised_resets_at_ms.is_none()
            && !has_explicit_reset
        {
            return Err(QuotaError::EmptySample);
        }
        if parts.reset_occurred_at_ms.is_some() && !has_explicit_reset {
            return Err(QuotaError::ResetTimeWithoutEvidence);
        }
        if parts
            .reset_occurred_at_ms
            .is_some_and(|occurred_at| occurred_at <= 0 || occurred_at > parts.observed_at_ms)
        {
            return Err(QuotaError::InvalidResetOccurredAt);
        }
        Ok(Self {
            key: parts.key,
            observation_id: parts.observation_id,
            observed_at_ms: parts.observed_at_ms,
            fresh_until_ms: parts.fresh_until_ms,
            stale_after_ms: parts.stale_after_ms,
            provider_epoch_id: parts.provider_epoch_id,
            used_ratio: parts.used_ratio,
            remaining_ratio: parts.remaining_ratio,
            units: parts.units,
            advertised_resets_at_ms: parts.advertised_resets_at_ms,
            quality: parts.quality,
            source: parts.source,
            confidence: parts.confidence,
            reset_evidence: parts.reset_evidence,
            reset_occurred_at_ms: parts.reset_occurred_at_ms,
        })
    }

    #[must_use]
    pub const fn key(&self) -> &QuotaWindowKey {
        &self.key
    }

    #[must_use]
    pub const fn observation_id(&self) -> QuotaObservationId {
        self.observation_id
    }

    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    #[must_use]
    pub const fn fresh_until_ms(&self) -> i64 {
        self.fresh_until_ms
    }

    #[must_use]
    pub const fn stale_after_ms(&self) -> i64 {
        self.stale_after_ms
    }

    #[must_use]
    pub const fn provider_epoch_id(&self) -> Option<&QuotaProviderEpochId> {
        self.provider_epoch_id.as_ref()
    }

    #[must_use]
    pub const fn used_ratio(&self) -> Option<QuotaRatio> {
        self.used_ratio
    }

    #[must_use]
    pub const fn remaining_ratio(&self) -> Option<QuotaRatio> {
        self.remaining_ratio
    }

    #[must_use]
    pub const fn units(&self) -> Option<&QuotaUnits> {
        self.units.as_ref()
    }

    #[must_use]
    pub const fn advertised_resets_at_ms(&self) -> Option<i64> {
        self.advertised_resets_at_ms
    }

    #[must_use]
    pub const fn quality(&self) -> QuotaSampleQuality {
        self.quality
    }

    #[must_use]
    pub const fn source(&self) -> QuotaEvidenceSource {
        self.source
    }

    #[must_use]
    pub const fn confidence(&self) -> QuotaConfidence {
        self.confidence
    }

    #[must_use]
    pub const fn reset_evidence(&self) -> QuotaResetEvidence {
        self.reset_evidence
    }

    #[must_use]
    pub const fn reset_occurred_at_ms(&self) -> Option<i64> {
        self.reset_occurred_at_ms
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaSampleWire {
    key: QuotaWindowKey,
    observation_id: QuotaObservationId,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    provider_epoch_id: Option<QuotaProviderEpochId>,
    used_ratio: Option<QuotaRatio>,
    remaining_ratio: Option<QuotaRatio>,
    units: Option<QuotaUnits>,
    advertised_resets_at_ms: Option<i64>,
    quality: QuotaSampleQuality,
    source: QuotaEvidenceSource,
    confidence: QuotaConfidence,
    reset_evidence: QuotaResetEvidence,
    reset_occurred_at_ms: Option<i64>,
}

impl<'de> Deserialize<'de> for QuotaSample {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = QuotaSampleWire::deserialize(deserializer)?;
        Self::new(QuotaSampleParts {
            key: wire.key,
            observation_id: wire.observation_id,
            observed_at_ms: wire.observed_at_ms,
            fresh_until_ms: wire.fresh_until_ms,
            stale_after_ms: wire.stale_after_ms,
            provider_epoch_id: wire.provider_epoch_id,
            used_ratio: wire.used_ratio,
            remaining_ratio: wire.remaining_ratio,
            units: wire.units,
            advertised_resets_at_ms: wire.advertised_resets_at_ms,
            quality: wire.quality,
            source: wire.source,
            confidence: wire.confidence,
            reset_evidence: wire.reset_evidence,
            reset_occurred_at_ms: wire.reset_occurred_at_ms,
        })
        .map_err(serde::de::Error::custom)
    }
}
