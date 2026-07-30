use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{QuotaAccountId, QuotaWindowId, QuotaWorkspaceId, UsageProviderId};

pub const MAX_BENEFIT_LOTS_PER_OBSERVATION: usize = 64;
pub const MAX_REMINDER_THRESHOLDS: usize = 8;
pub const MIN_REMINDER_LEAD_SECONDS: u32 = 60;
pub const MAX_REMINDER_LEAD_SECONDS: u32 = 365 * 24 * 60 * 60;
pub const RECOMMENDED_REMINDER_LEAD_SECONDS: [u32; 5] = [
    7 * 24 * 60 * 60,
    24 * 60 * 60,
    12 * 60 * 60,
    6 * 60 * 60,
    60 * 60,
];

const MAX_BENEFIT_LABEL_KEY_BYTES: usize = 128;
const MAX_BENEFIT_TIME_ZONE_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BenefitError {
    #[error("{field} must be a bounded identifier")]
    InvalidId { field: &'static str },
    #[error("benefit label key must be a bounded ASCII identifier")]
    InvalidLabelKey,
    #[error("benefit time-zone ID must be a bounded IANA-style identifier")]
    InvalidTimeZoneId,
    #[error("benefit local date is invalid")]
    InvalidLocalDate,
    #[error("benefit local time is invalid")]
    InvalidLocalTime,
    #[error("benefit expiry is invalid")]
    InvalidExpiry,
    #[error("benefit quantity must be within 1..=i64::MAX")]
    InvalidQuantity,
    #[error("benefit granted time must be positive")]
    InvalidGrantedTime,
    #[error("benefit observation times must satisfy 0 < observed <= fresh <= stale")]
    InvalidObservationTimes,
    #[error("benefit observation contains a duplicate lot identity")]
    DuplicateLotId,
    #[error("value exceeds capacity {limit}")]
    CapacityExceeded { limit: usize },
    #[error("reminder lead time must be from one minute through 365 days")]
    InvalidReminderLeadTime,
    #[error("reminder profile revision must be positive")]
    InvalidReminderProfileRevision,
    #[error("serialized benefit value is invalid")]
    InvalidSerializedValue,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BenefitLotId([u8; 32]);

impl BenefitLotId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for BenefitLotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BenefitLotId([redacted])")
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BenefitObservationId([u8; 32]);

impl BenefitObservationId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for BenefitObservationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BenefitObservationId([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenefitScope {
    provider_id: UsageProviderId,
    account_id: QuotaAccountId,
    workspace_id: Option<QuotaWorkspaceId>,
}

impl BenefitScope {
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

impl fmt::Debug for BenefitScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BenefitScope([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenefitKind {
    BankedRateLimitReset,
    UsageCredit,
    TemporaryUsage,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenefitState {
    Available,
    ActivationPending,
    Activated,
    Expired,
    Revoked,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenefitEvidenceSource {
    ProviderOfficial,
    ProviderLocal,
    Manual,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenefitConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenefitDetailKind {
    ProviderDetail,
    ProviderAggregate,
    Manual,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenefitInventoryCompleteness {
    Complete,
    CompleteQuantityPartialDetails,
    Partial,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "window_id", rename_all = "snake_case")]
pub enum BenefitTarget {
    Provider,
    QuotaWindow(QuotaWindowId),
}

#[derive(Clone, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BenefitLabelKey(Box<str>);

impl BenefitLabelKey {
    pub fn new(value: impl Into<String>) -> Result<Self, BenefitError> {
        let value = value.into();
        if !valid_ascii_identifier(&value, MAX_BENEFIT_LABEL_KEY_BYTES, false) {
            return Err(BenefitError::InvalidLabelKey);
        }
        Ok(Self(value.into_boxed_str()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for BenefitLabelKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BenefitLabelKey")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for BenefitLabelKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BenefitTimeZoneId(Box<str>);

impl BenefitTimeZoneId {
    pub fn new(value: impl Into<String>) -> Result<Self, BenefitError> {
        let value = value.into();
        if !valid_ascii_identifier(&value, MAX_BENEFIT_TIME_ZONE_ID_BYTES, true)
            || value.starts_with('.')
            || value.ends_with('.')
            || value.contains("..")
            || value.starts_with('/')
            || value.ends_with('/')
        {
            return Err(BenefitError::InvalidTimeZoneId);
        }
        Ok(Self(value.into_boxed_str()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for BenefitTimeZoneId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BenefitTimeZoneId")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for BenefitTimeZoneId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct BenefitLocalDate {
    year: i32,
    month: u8,
    day: u8,
}

impl BenefitLocalDate {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, BenefitError> {
        if !(1..=9999).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
        {
            return Err(BenefitError::InvalidLocalDate);
        }
        Ok(Self { year, month, day })
    }

    #[must_use]
    pub const fn year(self) -> i32 {
        self.year
    }

    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenefitLocalDateWire {
    year: i32,
    month: u8,
    day: u8,
}

impl<'de> Deserialize<'de> for BenefitLocalDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = BenefitLocalDateWire::deserialize(deserializer)?;
        Self::new(wire.year, wire.month, wire.day).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct BenefitLocalTime {
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
}

impl BenefitLocalTime {
    pub fn new(hour: u8, minute: u8, second: u8, millisecond: u16) -> Result<Self, BenefitError> {
        if hour > 23 || minute > 59 || second > 59 || millisecond > 999 {
            return Err(BenefitError::InvalidLocalTime);
        }
        Ok(Self {
            hour,
            minute,
            second,
            millisecond,
        })
    }

    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    #[must_use]
    pub const fn minute(self) -> u8 {
        self.minute
    }

    #[must_use]
    pub const fn second(self) -> u8 {
        self.second
    }

    #[must_use]
    pub const fn millisecond(self) -> u16 {
        self.millisecond
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenefitLocalTimeWire {
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
}

impl<'de> Deserialize<'de> for BenefitLocalTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = BenefitLocalTimeWire::deserialize(deserializer)?;
        Self::new(wire.hour, wire.minute, wire.second, wire.millisecond)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenefitLocalDateTime {
    date: BenefitLocalDate,
    time: BenefitLocalTime,
}

impl BenefitLocalDateTime {
    #[must_use]
    pub const fn new(date: BenefitLocalDate, time: BenefitLocalTime) -> Self {
        Self { date, time }
    }

    #[must_use]
    pub const fn date(self) -> BenefitLocalDate {
        self.date
    }

    #[must_use]
    pub const fn time(self) -> BenefitLocalTime {
        self.time
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BenefitExpiry {
    ExactUtc {
        at_ms: i64,
    },
    ProviderLocal {
        local: BenefitLocalDateTime,
        time_zone: BenefitTimeZoneId,
    },
    ProviderDate {
        date: BenefitLocalDate,
        time_zone: Option<BenefitTimeZoneId>,
    },
    BoundedUtc {
        earliest_at_ms: i64,
        latest_at_ms: i64,
    },
    Unknown,
}

impl BenefitExpiry {
    pub fn exact_utc(at_ms: i64) -> Result<Self, BenefitError> {
        if at_ms <= 0 {
            return Err(BenefitError::InvalidExpiry);
        }
        Ok(Self::ExactUtc { at_ms })
    }

    #[must_use]
    pub fn provider_local(local: BenefitLocalDateTime, time_zone: BenefitTimeZoneId) -> Self {
        Self::ProviderLocal { local, time_zone }
    }

    #[must_use]
    pub fn provider_date(date: BenefitLocalDate, time_zone: Option<BenefitTimeZoneId>) -> Self {
        Self::ProviderDate { date, time_zone }
    }

    pub fn bounded_utc(earliest_at_ms: i64, latest_at_ms: i64) -> Result<Self, BenefitError> {
        if earliest_at_ms <= 0 || latest_at_ms < earliest_at_ms {
            return Err(BenefitError::InvalidExpiry);
        }
        Ok(Self::BoundedUtc {
            earliest_at_ms,
            latest_at_ms,
        })
    }

    #[must_use]
    pub const fn unknown() -> Self {
        Self::Unknown
    }

    #[must_use]
    pub const fn conservative_utc_ms(&self) -> Option<i64> {
        match self {
            Self::ExactUtc { at_ms } => Some(*at_ms),
            Self::BoundedUtc { earliest_at_ms, .. } => Some(*earliest_at_ms),
            Self::ProviderLocal { .. } | Self::ProviderDate { .. } | Self::Unknown => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum BenefitExpiryWire {
    ExactUtc {
        at_ms: i64,
    },
    ProviderLocal {
        local: BenefitLocalDateTime,
        time_zone: BenefitTimeZoneId,
    },
    ProviderDate {
        date: BenefitLocalDate,
        time_zone: Option<BenefitTimeZoneId>,
    },
    BoundedUtc {
        earliest_at_ms: i64,
        latest_at_ms: i64,
    },
    Unknown,
}

impl<'de> Deserialize<'de> for BenefitExpiry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match BenefitExpiryWire::deserialize(deserializer)? {
            BenefitExpiryWire::ExactUtc { at_ms } => Self::exact_utc(at_ms),
            BenefitExpiryWire::ProviderLocal { local, time_zone } => {
                Ok(Self::provider_local(local, time_zone))
            }
            BenefitExpiryWire::ProviderDate { date, time_zone } => {
                Ok(Self::provider_date(date, time_zone))
            }
            BenefitExpiryWire::BoundedUtc {
                earliest_at_ms,
                latest_at_ms,
            } => Self::bounded_utc(earliest_at_ms, latest_at_ms),
            BenefitExpiryWire::Unknown => Ok(Self::Unknown),
        }
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitLotObservationParts {
    pub lot_id: BenefitLotId,
    pub kind: BenefitKind,
    pub quantity: u64,
    pub state: BenefitState,
    pub target: BenefitTarget,
    pub granted_at_ms: Option<i64>,
    pub expiry: BenefitExpiry,
    pub source: BenefitEvidenceSource,
    pub confidence: BenefitConfidence,
    pub detail_kind: BenefitDetailKind,
    pub label_key: BenefitLabelKey,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct BenefitLotObservation {
    lot_id: BenefitLotId,
    kind: BenefitKind,
    quantity: u64,
    state: BenefitState,
    target: BenefitTarget,
    granted_at_ms: Option<i64>,
    expiry: BenefitExpiry,
    source: BenefitEvidenceSource,
    confidence: BenefitConfidence,
    detail_kind: BenefitDetailKind,
    label_key: BenefitLabelKey,
}

impl BenefitLotObservation {
    pub fn new(parts: BenefitLotObservationParts) -> Result<Self, BenefitError> {
        if parts.quantity == 0 || parts.quantity > i64::MAX as u64 {
            return Err(BenefitError::InvalidQuantity);
        }
        if parts
            .granted_at_ms
            .is_some_and(|granted_at_ms| granted_at_ms <= 0)
        {
            return Err(BenefitError::InvalidGrantedTime);
        }
        Ok(Self {
            lot_id: parts.lot_id,
            kind: parts.kind,
            quantity: parts.quantity,
            state: parts.state,
            target: parts.target,
            granted_at_ms: parts.granted_at_ms,
            expiry: parts.expiry,
            source: parts.source,
            confidence: parts.confidence,
            detail_kind: parts.detail_kind,
            label_key: parts.label_key,
        })
    }

    #[must_use]
    pub const fn lot_id(&self) -> BenefitLotId {
        self.lot_id
    }

    #[must_use]
    pub const fn kind(&self) -> BenefitKind {
        self.kind
    }

    #[must_use]
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }

    #[must_use]
    pub const fn state(&self) -> BenefitState {
        self.state
    }

    #[must_use]
    pub const fn target(&self) -> &BenefitTarget {
        &self.target
    }

    #[must_use]
    pub const fn granted_at_ms(&self) -> Option<i64> {
        self.granted_at_ms
    }

    #[must_use]
    pub const fn expiry(&self) -> &BenefitExpiry {
        &self.expiry
    }

    #[must_use]
    pub const fn source(&self) -> BenefitEvidenceSource {
        self.source
    }

    #[must_use]
    pub const fn confidence(&self) -> BenefitConfidence {
        self.confidence
    }

    #[must_use]
    pub const fn detail_kind(&self) -> BenefitDetailKind {
        self.detail_kind
    }

    #[must_use]
    pub fn label_key(&self) -> &str {
        self.label_key.as_str()
    }

    #[must_use]
    pub fn into_parts(self) -> BenefitLotObservationParts {
        BenefitLotObservationParts {
            lot_id: self.lot_id,
            kind: self.kind,
            quantity: self.quantity,
            state: self.state,
            target: self.target,
            granted_at_ms: self.granted_at_ms,
            expiry: self.expiry,
            source: self.source,
            confidence: self.confidence,
            detail_kind: self.detail_kind,
            label_key: self.label_key,
        }
    }
}

impl fmt::Debug for BenefitLotObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitLotObservation")
            .field("lot_id", &self.lot_id)
            .field("kind", &self.kind)
            .field("quantity", &self.quantity)
            .field("state", &self.state)
            .field("target", &"[redacted]")
            .field("granted_at_ms", &self.granted_at_ms)
            .field("expiry", &self.expiry)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .field("detail_kind", &self.detail_kind)
            .field("label_key", &self.label_key)
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenefitLotObservationWire {
    lot_id: BenefitLotId,
    kind: BenefitKind,
    quantity: u64,
    state: BenefitState,
    target: BenefitTarget,
    granted_at_ms: Option<i64>,
    expiry: BenefitExpiry,
    source: BenefitEvidenceSource,
    confidence: BenefitConfidence,
    detail_kind: BenefitDetailKind,
    label_key: BenefitLabelKey,
}

impl<'de> Deserialize<'de> for BenefitLotObservation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = BenefitLotObservationWire::deserialize(deserializer)?;
        Self::new(BenefitLotObservationParts {
            lot_id: wire.lot_id,
            kind: wire.kind,
            quantity: wire.quantity,
            state: wire.state,
            target: wire.target,
            granted_at_ms: wire.granted_at_ms,
            expiry: wire.expiry,
            source: wire.source,
            confidence: wire.confidence,
            detail_kind: wire.detail_kind,
            label_key: wire.label_key,
        })
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitInventoryObservationParts {
    pub scope: BenefitScope,
    pub observation_id: BenefitObservationId,
    pub observed_at_ms: i64,
    pub fresh_until_ms: i64,
    pub stale_after_ms: i64,
    pub completeness: BenefitInventoryCompleteness,
    pub lots: Vec<BenefitLotObservation>,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct BenefitInventoryObservation {
    scope: BenefitScope,
    observation_id: BenefitObservationId,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    completeness: BenefitInventoryCompleteness,
    lots: Box<[BenefitLotObservation]>,
}

impl BenefitInventoryObservation {
    pub fn new(parts: BenefitInventoryObservationParts) -> Result<Self, BenefitError> {
        if parts.observed_at_ms <= 0
            || parts.observed_at_ms > parts.fresh_until_ms
            || parts.fresh_until_ms > parts.stale_after_ms
        {
            return Err(BenefitError::InvalidObservationTimes);
        }
        if parts.lots.len() > MAX_BENEFIT_LOTS_PER_OBSERVATION {
            return Err(BenefitError::CapacityExceeded {
                limit: MAX_BENEFIT_LOTS_PER_OBSERVATION,
            });
        }
        let mut identities = BTreeSet::new();
        if parts
            .lots
            .iter()
            .any(|lot| !identities.insert(*lot.lot_id().as_bytes()))
        {
            return Err(BenefitError::DuplicateLotId);
        }
        Ok(Self {
            scope: parts.scope,
            observation_id: parts.observation_id,
            observed_at_ms: parts.observed_at_ms,
            fresh_until_ms: parts.fresh_until_ms,
            stale_after_ms: parts.stale_after_ms,
            completeness: parts.completeness,
            lots: parts.lots.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn scope(&self) -> &BenefitScope {
        &self.scope
    }

    #[must_use]
    pub const fn observation_id(&self) -> BenefitObservationId {
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
    pub const fn completeness(&self) -> BenefitInventoryCompleteness {
        self.completeness
    }

    #[must_use]
    pub const fn lots(&self) -> &[BenefitLotObservation] {
        &self.lots
    }
}

impl fmt::Debug for BenefitInventoryObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitInventoryObservation")
            .field("scope", &"[redacted]")
            .field("observation_id", &self.observation_id)
            .field("observed_at_ms", &self.observed_at_ms)
            .field("fresh_until_ms", &self.fresh_until_ms)
            .field("stale_after_ms", &self.stale_after_ms)
            .field("completeness", &self.completeness)
            .field("lot_count", &self.lots.len())
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenefitInventoryObservationWire {
    scope: BenefitScope,
    observation_id: BenefitObservationId,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    completeness: BenefitInventoryCompleteness,
    lots: Vec<BenefitLotObservation>,
}

impl<'de> Deserialize<'de> for BenefitInventoryObservation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = BenefitInventoryObservationWire::deserialize(deserializer)?;
        Self::new(BenefitInventoryObservationParts {
            scope: wire.scope,
            observation_id: wire.observation_id,
            observed_at_ms: wire.observed_at_ms,
            fresh_until_ms: wire.fresh_until_ms,
            stale_after_ms: wire.stale_after_ms,
            completeness: wire.completeness,
            lots: wire.lots,
        })
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ReminderLeadTime(u32);

impl ReminderLeadTime {
    pub fn new(seconds: u32) -> Result<Self, BenefitError> {
        if !(MIN_REMINDER_LEAD_SECONDS..=MAX_REMINDER_LEAD_SECONDS).contains(&seconds) {
            return Err(BenefitError::InvalidReminderLeadTime);
        }
        Ok(Self(seconds))
    }

    #[must_use]
    pub const fn seconds(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ReminderLeadTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u32::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ReminderProfileRevision(u64);

impl ReminderProfileRevision {
    pub fn new(value: u64) -> Result<Self, BenefitError> {
        if value == 0 || value > i64::MAX as u64 {
            return Err(BenefitError::InvalidReminderProfileRevision);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ReminderProfileRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    InApp,
    OsScheduled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReminderProfileParts {
    pub revision: ReminderProfileRevision,
    pub lead_times: Vec<ReminderLeadTime>,
    pub channels: Vec<NotificationChannel>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReminderProfile {
    revision: ReminderProfileRevision,
    lead_times: Box<[ReminderLeadTime]>,
    channels: Box<[NotificationChannel]>,
}

impl ReminderProfile {
    pub fn new(mut parts: ReminderProfileParts) -> Result<Self, BenefitError> {
        parts
            .lead_times
            .sort_unstable_by(|left, right| right.cmp(left));
        parts.lead_times.dedup();
        if parts.lead_times.len() > MAX_REMINDER_THRESHOLDS {
            return Err(BenefitError::CapacityExceeded {
                limit: MAX_REMINDER_THRESHOLDS,
            });
        }
        parts.channels.sort_unstable();
        parts.channels.dedup();
        Ok(Self {
            revision: parts.revision,
            lead_times: parts.lead_times.into_boxed_slice(),
            channels: parts.channels.into_boxed_slice(),
        })
    }

    pub fn recommended(revision: ReminderProfileRevision) -> Result<Self, BenefitError> {
        let lead_times = RECOMMENDED_REMINDER_LEAD_SECONDS
            .into_iter()
            .map(ReminderLeadTime::new)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(ReminderProfileParts {
            revision,
            lead_times,
            channels: vec![NotificationChannel::InApp],
        })
    }

    #[must_use]
    pub const fn revision(&self) -> ReminderProfileRevision {
        self.revision
    }

    #[must_use]
    pub const fn lead_times(&self) -> &[ReminderLeadTime] {
        &self.lead_times
    }

    #[must_use]
    pub const fn channels(&self) -> &[NotificationChannel] {
        &self.channels
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReminderProfileWire {
    revision: ReminderProfileRevision,
    lead_times: Vec<ReminderLeadTime>,
    channels: Vec<NotificationChannel>,
}

impl<'de> Deserialize<'de> for ReminderProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ReminderProfileWire::deserialize(deserializer)?;
        Self::new(ReminderProfileParts {
            revision: wire.revision,
            lead_times: wire.lead_times,
            channels: wire.channels,
        })
        .map_err(serde::de::Error::custom)
    }
}

fn valid_ascii_identifier(value: &str, max_bytes: usize, allow_slash: bool) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'_' | b'-' | b'+')
                || (allow_slash && byte == b'/')
        })
}

const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
