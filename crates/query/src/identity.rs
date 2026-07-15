use std::sync::Arc;

use tokenmaster_domain::{UsageProfileId, UsageProviderId};

use crate::{QueryError, QueryErrorCode};

pub const QUERY_SCHEMA_VERSION: u16 = 1;
pub const MAX_QUERY_SCOPES: usize = 32;
pub const MAX_QUERY_WARNINGS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SnapshotGeneration(u64);

impl SnapshotGeneration {
    pub fn new(value: u64) -> Result<Self, QueryError> {
        if value == 0 {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, QueryError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))
    }

    #[must_use]
    pub const fn is_newer_than(self, current: Option<Self>) -> bool {
        match current {
            Some(current) => self.0 > current.0,
            None => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PublicationGeneration(u64);

impl PublicationGeneration {
    pub fn new(value: u64) -> Result<Self, QueryError> {
        if value > i64::MAX as u64 {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReplayRevision(u64);

impl ReplayRevision {
    pub fn new(value: u64) -> Result<Self, QueryError> {
        if value > i64::MAX as u64 {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetIdentity {
    Empty,
    LegacySnapshotV1,
    ReplayRevision(ReplayRevision),
}

impl DatasetIdentity {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::LegacySnapshotV1 => "legacy_snapshot_v1",
            Self::ReplayRevision(_) => "replay_revision",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryFreshness {
    Fresh,
    Aging,
    Stale,
    Unavailable,
}

impl QueryFreshness {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Aging => "aging",
            Self::Stale => "stale",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryQuality {
    Authoritative,
    Derived,
    Estimated,
    Partial,
    Conflict,
    Unknown,
}

impl QueryQuality {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Authoritative => "authoritative",
            Self::Derived => "derived",
            Self::Estimated => "estimated",
            Self::Partial => "partial",
            Self::Conflict => "conflict",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryWarningCode {
    LegacyUnverified,
    Partial,
    RecoveryPending,
    ClockDiscontinuity,
    AccountingVersionStale,
}

impl QueryWarningCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::LegacyUnverified => "legacy_unverified",
            Self::Partial => "partial",
            Self::RecoveryPending => "recovery_pending",
            Self::ClockDiscontinuity => "clock_discontinuity",
            Self::AccountingVersionStale => "accounting_version_stale",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryScope {
    provider_id: UsageProviderId,
    profile_id: UsageProfileId,
}

impl QueryScope {
    #[must_use]
    pub const fn new(provider_id: UsageProviderId, profile_id: UsageProfileId) -> Self {
        Self {
            provider_id,
            profile_id,
        }
    }

    #[must_use]
    pub const fn provider_id(&self) -> &UsageProviderId {
        &self.provider_id
    }

    #[must_use]
    pub const fn profile_id(&self) -> &UsageProfileId {
        &self.profile_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryHeaderParts {
    pub snapshot_generation: SnapshotGeneration,
    pub publication_generation: PublicationGeneration,
    pub dataset_identity: DatasetIdentity,
    pub generated_at_ms: i64,
    pub data_through_ms: Option<i64>,
    pub freshness: QueryFreshness,
    pub quality: QueryQuality,
    pub scopes: Vec<QueryScope>,
    pub warnings: Vec<QueryWarningCode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryHeader {
    snapshot_generation: SnapshotGeneration,
    publication_generation: PublicationGeneration,
    dataset_identity: DatasetIdentity,
    generated_at_ms: i64,
    data_through_ms: Option<i64>,
    freshness: QueryFreshness,
    quality: QueryQuality,
    scopes: Arc<[QueryScope]>,
    warnings: Arc<[QueryWarningCode]>,
}

impl QueryHeader {
    pub fn new(parts: QueryHeaderParts) -> Result<Self, QueryError> {
        if parts.scopes.len() > MAX_QUERY_SCOPES || parts.warnings.len() > MAX_QUERY_WARNINGS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        Ok(Self {
            snapshot_generation: parts.snapshot_generation,
            publication_generation: parts.publication_generation,
            dataset_identity: parts.dataset_identity,
            generated_at_ms: parts.generated_at_ms,
            data_through_ms: parts.data_through_ms,
            freshness: parts.freshness,
            quality: parts.quality,
            scopes: Arc::from(parts.scopes),
            warnings: Arc::from(parts.warnings),
        })
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        QUERY_SCHEMA_VERSION
    }

    #[must_use]
    pub const fn snapshot_generation(&self) -> SnapshotGeneration {
        self.snapshot_generation
    }

    #[must_use]
    pub const fn publication_generation(&self) -> PublicationGeneration {
        self.publication_generation
    }

    #[must_use]
    pub const fn dataset_identity(&self) -> DatasetIdentity {
        self.dataset_identity
    }

    #[must_use]
    pub const fn generated_at_ms(&self) -> i64 {
        self.generated_at_ms
    }

    #[must_use]
    pub const fn data_through_ms(&self) -> Option<i64> {
        self.data_through_ms
    }

    #[must_use]
    pub const fn freshness(&self) -> QueryFreshness {
        self.freshness
    }

    #[must_use]
    pub const fn quality(&self) -> QueryQuality {
        self.quality
    }

    #[must_use]
    pub const fn scopes(&self) -> &Arc<[QueryScope]> {
        &self.scopes
    }

    #[must_use]
    pub const fn warnings(&self) -> &Arc<[QueryWarningCode]> {
        &self.warnings
    }

    #[must_use]
    pub fn into_parts(self) -> QueryHeaderParts {
        QueryHeaderParts {
            snapshot_generation: self.snapshot_generation,
            publication_generation: self.publication_generation,
            dataset_identity: self.dataset_identity,
            generated_at_ms: self.generated_at_ms,
            data_through_ms: self.data_through_ms,
            freshness: self.freshness,
            quality: self.quality,
            scopes: self.scopes.iter().cloned().collect(),
            warnings: self.warnings.iter().copied().collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryEnvelope<T> {
    header: QueryHeader,
    payload: T,
}

impl<T> QueryEnvelope<T> {
    #[must_use]
    pub const fn new(header: QueryHeader, payload: T) -> Self {
        Self { header, payload }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.header.schema_version()
    }

    #[must_use]
    pub const fn header(&self) -> &QueryHeader {
        &self.header
    }

    #[must_use]
    pub const fn payload(&self) -> &T {
        &self.payload
    }

    #[must_use]
    pub fn is_newer_than(&self, current: Option<&Self>) -> bool {
        self.header
            .snapshot_generation
            .is_newer_than(current.map(|current| current.header.snapshot_generation))
    }

    #[must_use]
    pub fn into_parts(self) -> (QueryHeader, T) {
        (self.header, self.payload)
    }
}
