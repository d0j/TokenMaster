use std::{fmt, sync::Arc};

use tokenmaster_domain::QuotaWindowKey;

use crate::{
    MAX_QUERY_SCOPES, MAX_QUERY_WARNINGS, QueryError, QueryErrorCode, QueryFreshness, QueryQuality,
    SnapshotGeneration,
};

pub const QUOTA_QUERY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct QuotaRevision(u64);

impl QuotaRevision {
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

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaWindowFilter {
    key: QuotaWindowKey,
}

impl QuotaWindowFilter {
    #[must_use]
    pub const fn new(key: QuotaWindowKey) -> Self {
        Self { key }
    }

    #[must_use]
    pub const fn key(&self) -> &QuotaWindowKey {
        &self.key
    }
}

impl fmt::Debug for QuotaWindowFilter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QuotaWindowFilter([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaWarningCode {
    WindowUnavailable,
    ClockDiscontinuity,
    PartialEvidence,
    ConflictingEvidence,
    UnknownEvidence,
}

impl QuotaWarningCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::WindowUnavailable => "window_unavailable",
            Self::ClockDiscontinuity => "clock_discontinuity",
            Self::PartialEvidence => "partial_evidence",
            Self::ConflictingEvidence => "conflicting_evidence",
            Self::UnknownEvidence => "unknown_evidence",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaQueryHeaderParts {
    pub snapshot_generation: SnapshotGeneration,
    pub quota_revision: QuotaRevision,
    pub generated_at_ms: i64,
    pub data_through_ms: Option<i64>,
    pub freshness: QueryFreshness,
    pub quality: QueryQuality,
    pub filters: Vec<QuotaWindowFilter>,
    pub warnings: Vec<QuotaWarningCode>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaQueryHeader {
    snapshot_generation: SnapshotGeneration,
    quota_revision: QuotaRevision,
    generated_at_ms: i64,
    data_through_ms: Option<i64>,
    freshness: QueryFreshness,
    quality: QueryQuality,
    filters: Arc<[QuotaWindowFilter]>,
    warnings: Arc<[QuotaWarningCode]>,
}

impl QuotaQueryHeader {
    pub fn new(parts: QuotaQueryHeaderParts) -> Result<Self, QueryError> {
        if parts.filters.len() > MAX_QUERY_SCOPES || parts.warnings.len() > MAX_QUERY_WARNINGS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        if parts
            .filters
            .iter()
            .enumerate()
            .any(|(index, filter)| parts.filters[..index].contains(filter))
        {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self {
            snapshot_generation: parts.snapshot_generation,
            quota_revision: parts.quota_revision,
            generated_at_ms: parts.generated_at_ms,
            data_through_ms: parts.data_through_ms,
            freshness: parts.freshness,
            quality: parts.quality,
            filters: Arc::from(parts.filters),
            warnings: Arc::from(parts.warnings),
        })
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        QUOTA_QUERY_SCHEMA_VERSION
    }

    #[must_use]
    pub const fn snapshot_generation(&self) -> SnapshotGeneration {
        self.snapshot_generation
    }

    #[must_use]
    pub const fn quota_revision(&self) -> QuotaRevision {
        self.quota_revision
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
    pub const fn filters(&self) -> &Arc<[QuotaWindowFilter]> {
        &self.filters
    }

    #[must_use]
    pub const fn warnings(&self) -> &Arc<[QuotaWarningCode]> {
        &self.warnings
    }
}

impl fmt::Debug for QuotaQueryHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaQueryHeader")
            .field("schema_version", &self.schema_version())
            .field("snapshot_generation", &self.snapshot_generation)
            .field("quota_revision", &self.quota_revision)
            .field("generated_at_ms", &self.generated_at_ms)
            .field("data_through_ms", &self.data_through_ms)
            .field("freshness", &self.freshness)
            .field("quality", &self.quality)
            .field("filter_count", &self.filters.len())
            .field("filters", &"[redacted]")
            .field("warnings", &self.warnings)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaEnvelope<T> {
    header: QuotaQueryHeader,
    payload: T,
}

impl<T> QuotaEnvelope<T> {
    #[must_use]
    pub const fn new(header: QuotaQueryHeader, payload: T) -> Self {
        Self { header, payload }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.header.schema_version()
    }

    #[must_use]
    pub const fn header(&self) -> &QuotaQueryHeader {
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
    pub fn into_parts(self) -> (QuotaQueryHeader, T) {
        (self.header, self.payload)
    }
}
