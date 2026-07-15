use std::{fmt, path::Path, time::Duration};

use tokenmaster_domain::{ModelKey, TokenCount, TokenUsage, UsageProfileId, UsageProviderId};
use tokenmaster_store::{
    ArchivePublicationQuality, StoreError, StoreErrorCode, UsageActivityQuery, UsageQueryCapture,
    UsageQueryDatasetIdentity, UsageQueryEvent, UsageReadStore,
};

use crate::{
    ActivityCursor, ActivityItem, DatasetGeneration, DatasetIdentity, LatestActivityPage, PageSize,
    PublicationGeneration, QueryClock, QueryEnvelope, QueryError, QueryErrorCode, QueryFreshness,
    QueryHeader, QueryHeaderParts, QueryQuality, QueryScope, QueryWarningCode, ReplayRevision,
    SnapshotGeneration,
};

pub const QUERY_FRESH_MAX_AGE_MS: i64 = 20 * 60 * 1_000;
pub const QUERY_STALE_MIN_AGE_MS: i64 = 2 * 60 * 60 * 1_000;
const QUERY_DEADLINE_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatestActivityRequest {
    page_size: PageSize,
    continuation: Option<(DatasetIdentity, ActivityCursor)>,
}

impl LatestActivityRequest {
    #[must_use]
    pub const fn first(page_size: PageSize) -> Self {
        Self {
            page_size,
            continuation: None,
        }
    }

    #[must_use]
    pub const fn continuation(
        page_size: PageSize,
        dataset_identity: DatasetIdentity,
        cursor: ActivityCursor,
    ) -> Self {
        Self {
            page_size,
            continuation: Some((dataset_identity, cursor)),
        }
    }
}

pub struct QueryService<C> {
    store: UsageReadStore,
    clock: C,
    last_generation: Option<SnapshotGeneration>,
}

impl<C: QueryClock> QueryService<C> {
    pub fn open(path: impl AsRef<Path>, clock: C) -> Result<Self, QueryError> {
        Ok(Self {
            store: UsageReadStore::open(path).map_err(map_store_error)?,
            clock,
            last_generation: None,
        })
    }

    pub fn latest_activity(
        &mut self,
        request: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
        let generation = match self.last_generation {
            Some(current) => current.checked_next()?,
            None => SnapshotGeneration::new(1)?,
        };
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;

        let (expected_dataset, before) = match request.continuation {
            Some((DatasetIdentity::Empty, _)) => {
                return Err(QueryError::new(QueryErrorCode::InvalidValue));
            }
            Some((identity, cursor)) => (
                Some(to_store_identity(identity)),
                Some(cursor.store_cursor()),
            ),
            None => (None, None),
        };
        let store_query = UsageActivityQuery::new(
            expected_dataset,
            before,
            request.page_size.get(),
            Duration::from_millis(QUERY_DEADLINE_MS),
        )
        .map_err(map_store_error)?;
        let capture = self
            .store
            .capture_activity_page(store_query)
            .map_err(map_store_error)?;
        let envelope = map_capture(capture, generation, time.wall_time_ms())?;
        self.last_generation = Some(generation);
        Ok(envelope)
    }
}

impl<C> fmt::Debug for QueryService<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QueryService([redacted])")
    }
}

fn map_capture(
    capture: UsageQueryCapture,
    generation: SnapshotGeneration,
    generated_at_ms: i64,
) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
    let publication = capture.publication();
    let dataset_identity = from_store_identity(publication.dataset_identity())?;
    let mut warnings = Vec::with_capacity(2);
    let quality = map_quality(
        dataset_identity,
        publication.quality(),
        publication.accounting_versions_current(),
        &mut warnings,
    )?;
    let freshness = map_freshness(
        generated_at_ms,
        publication.data_through_ms(),
        &mut warnings,
    );
    let mut items = Vec::with_capacity(capture.events().len());
    for event in capture.events() {
        items.push(map_event(event)?);
    }
    let next_cursor = capture.next_cursor().map(ActivityCursor::from_store);
    let page = LatestActivityPage::new(items, next_cursor, capture.has_more())?;
    let header = QueryHeader::new(QueryHeaderParts {
        snapshot_generation: generation,
        publication_generation: PublicationGeneration::new(publication.generation())?,
        dataset_identity,
        generated_at_ms,
        data_through_ms: publication.data_through_ms(),
        freshness,
        quality,
        scopes: Vec::new(),
        warnings,
    })?;
    Ok(QueryEnvelope::new(header, page))
}

fn map_event(event: &UsageQueryEvent) -> Result<ActivityItem, QueryError> {
    let provider_id = UsageProviderId::new(event.provider_id().to_owned())
        .map_err(|_| QueryError::new(QueryErrorCode::CorruptArchive))?;
    let profile_id = UsageProfileId::new(event.profile_id().to_owned())
        .map_err(|_| QueryError::new(QueryErrorCode::CorruptArchive))?;
    let model = ModelKey::new(event.model().to_owned())
        .map_err(|_| QueryError::new(QueryErrorCode::CorruptArchive))?;
    let usage = TokenUsage::new(
        token_count(event.input_tokens()),
        token_count(event.cached_tokens()),
        token_count(event.output_tokens()),
        token_count(event.reasoning_tokens()),
        token_count(event.total_tokens()),
    );
    ActivityItem::new_with_cursor(
        QueryScope::new(provider_id, profile_id),
        event.event_id().to_owned(),
        model,
        usage,
        ActivityCursor::from_store(event.cursor()),
    )
    .map_err(|_| QueryError::new(QueryErrorCode::CorruptArchive))
}

const fn token_count(value: Option<u64>) -> TokenCount {
    match value {
        Some(value) => TokenCount::Available(value),
        None => TokenCount::Unavailable,
    }
}

fn map_quality(
    identity: DatasetIdentity,
    quality: ArchivePublicationQuality,
    accounting_versions_current: bool,
    warnings: &mut Vec<QueryWarningCode>,
) -> Result<QueryQuality, QueryError> {
    if identity == DatasetIdentity::LegacySnapshotV1 {
        warnings.push(QueryWarningCode::LegacyUnverified);
        return Ok(QueryQuality::Unknown);
    }
    let mapped = match quality {
        ArchivePublicationQuality::Empty if identity == DatasetIdentity::Empty => {
            Ok(QueryQuality::Authoritative)
        }
        ArchivePublicationQuality::Complete => Ok(QueryQuality::Authoritative),
        ArchivePublicationQuality::Partial => {
            warnings.push(QueryWarningCode::Partial);
            Ok(QueryQuality::Partial)
        }
        ArchivePublicationQuality::RecoveryPending => {
            warnings.push(QueryWarningCode::RecoveryPending);
            Ok(QueryQuality::Partial)
        }
        ArchivePublicationQuality::Empty => Err(QueryError::new(QueryErrorCode::CorruptArchive)),
    }?;
    if !accounting_versions_current {
        warnings.push(QueryWarningCode::AccountingVersionStale);
        Ok(QueryQuality::Unknown)
    } else {
        Ok(mapped)
    }
}

fn map_freshness(
    generated_at_ms: i64,
    data_through_ms: Option<i64>,
    warnings: &mut Vec<QueryWarningCode>,
) -> QueryFreshness {
    let Some(data_through_ms) = data_through_ms else {
        return QueryFreshness::Unavailable;
    };
    let Some(age_ms) = generated_at_ms.checked_sub(data_through_ms) else {
        warnings.push(QueryWarningCode::ClockDiscontinuity);
        return QueryFreshness::Unavailable;
    };
    if age_ms < 0 {
        warnings.push(QueryWarningCode::ClockDiscontinuity);
        QueryFreshness::Unavailable
    } else if age_ms <= QUERY_FRESH_MAX_AGE_MS {
        QueryFreshness::Fresh
    } else if age_ms <= QUERY_STALE_MIN_AGE_MS {
        QueryFreshness::Aging
    } else {
        QueryFreshness::Stale
    }
}

const fn to_store_identity(identity: DatasetIdentity) -> UsageQueryDatasetIdentity {
    match identity {
        DatasetIdentity::Empty => UsageQueryDatasetIdentity::Empty,
        DatasetIdentity::LegacySnapshotV1 => UsageQueryDatasetIdentity::LegacySnapshotV1,
        DatasetIdentity::ReplayRevision {
            revision,
            dataset_generation,
        } => UsageQueryDatasetIdentity::ReplayRevision {
            revision_id: revision.get(),
            dataset_generation: dataset_generation.get(),
        },
    }
}

fn from_store_identity(identity: UsageQueryDatasetIdentity) -> Result<DatasetIdentity, QueryError> {
    match identity {
        UsageQueryDatasetIdentity::Empty => Ok(DatasetIdentity::Empty),
        UsageQueryDatasetIdentity::LegacySnapshotV1 => Ok(DatasetIdentity::LegacySnapshotV1),
        UsageQueryDatasetIdentity::ReplayRevision {
            revision_id,
            dataset_generation,
        } => Ok(DatasetIdentity::ReplayRevision {
            revision: ReplayRevision::new(revision_id)
                .map_err(|_| QueryError::new(QueryErrorCode::CorruptArchive))?,
            dataset_generation: DatasetGeneration::new(dataset_generation)
                .map_err(|_| QueryError::new(QueryErrorCode::CorruptArchive))?,
        }),
    }
}

fn map_store_error(error: StoreError) -> QueryError {
    let code = match error.code() {
        StoreErrorCode::InvalidValue => QueryErrorCode::InvalidValue,
        StoreErrorCode::CapacityExceeded => QueryErrorCode::CapacityExceeded,
        StoreErrorCode::VersionMismatch => QueryErrorCode::VersionMismatch,
        StoreErrorCode::StaleRevision => QueryErrorCode::StaleSnapshot,
        StoreErrorCode::DeadlineExceeded => QueryErrorCode::DeadlineExceeded,
        StoreErrorCode::Database => QueryErrorCode::Unavailable,
        StoreErrorCode::SchemaTooNew
        | StoreErrorCode::SchemaMismatch
        | StoreErrorCode::PolicyMismatch
        | StoreErrorCode::InvalidStoredValue
        | StoreErrorCode::AccountingVersionMismatch
        | StoreErrorCode::ArchiveModeMismatch => QueryErrorCode::CorruptArchive,
        StoreErrorCode::StaleCheckpoint
        | StoreErrorCode::RebuildRequired
        | StoreErrorCode::IncompleteManifest
        | StoreErrorCode::UnsealedRevision
        | StoreErrorCode::PendingContinuation
        | StoreErrorCode::ScanInProgress
        | StoreErrorCode::StaleScan
        | StoreErrorCode::PendingScan => QueryErrorCode::Internal,
    };
    QueryError::new(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_and_partial_quality_remain_explicit() -> Result<(), QueryError> {
        let mut legacy_warnings = Vec::new();
        assert_eq!(
            map_quality(
                DatasetIdentity::LegacySnapshotV1,
                ArchivePublicationQuality::Empty,
                true,
                &mut legacy_warnings,
            )?,
            QueryQuality::Unknown
        );
        assert_eq!(legacy_warnings, vec![QueryWarningCode::LegacyUnverified]);

        let mut partial_warnings = Vec::new();
        assert_eq!(
            map_quality(
                DatasetIdentity::ReplayRevision {
                    revision: ReplayRevision::new(0)?,
                    dataset_generation: DatasetGeneration::new(1)?,
                },
                ArchivePublicationQuality::Partial,
                true,
                &mut partial_warnings,
            )?,
            QueryQuality::Partial
        );
        assert_eq!(partial_warnings, vec![QueryWarningCode::Partial]);
        Ok(())
    }
}
