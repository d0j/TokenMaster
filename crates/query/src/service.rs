use std::{
    fmt,
    path::Path,
    time::{Duration, Instant},
};

use tokenmaster_domain::{ModelKey, TokenCount, TokenUsage, UsageProfileId, UsageProviderId};
use tokenmaster_pricing::{CostMode, PricingEngine};
use tokenmaster_store::{
    ArchivePublicationQuality, StoreError, StoreErrorCode, UsageActivityQuery, UsageQueryCapture,
    UsageQueryDatasetIdentity, UsageQueryEvent, UsageQueryPublication, UsageReadStore,
};

use crate::{
    ActivityCursor, ActivityItem, BenefitChangePage, BenefitChangePageRequest,
    BenefitCurrentRequest, BenefitCurrentSnapshot, BenefitEnvelope, BenefitQueryHeader,
    BenefitQueryHeaderParts, DatasetGeneration, DatasetIdentity, GitEnvelope, GitOutputRequest,
    GitOutputSnapshot, LatestActivityPage, PageSize, PublicationGeneration, QueryClock,
    QueryEnvelope, QueryError, QueryErrorCode, QueryFreshness, QueryHeader, QueryHeaderParts,
    QueryQuality, QueryScope, QueryWarningCode, QuotaCurrentRequest, QuotaCurrentSnapshot,
    QuotaEnvelope, QuotaQueryHeader, QuotaQueryHeaderParts, QuotaTransitionPage,
    QuotaTransitionPageRequest, ReplayRevision, SnapshotGeneration, UsageAnalytics,
    UsageAnalyticsRequest, UsageSessionDetailResult, UsageSessionKey, UsageSessionPage,
    UsageSessionPageRequest, analytics, benefit, quota, session,
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
    pricing: PricingEngine,
    cost_mode: CostMode,
    last_generation: Option<SnapshotGeneration>,
}

impl<C: QueryClock> QueryService<C> {
    pub fn open(path: impl AsRef<Path>, clock: C) -> Result<Self, QueryError> {
        Self::open_with_pricing(path, clock, PricingEngine::embedded(), CostMode::Auto)
    }

    pub fn open_with_pricing(
        path: impl AsRef<Path>,
        clock: C,
        pricing: PricingEngine,
        cost_mode: CostMode,
    ) -> Result<Self, QueryError> {
        Ok(Self {
            store: UsageReadStore::open(path).map_err(map_store_error)?,
            clock,
            pricing,
            cost_mode,
            last_generation: None,
        })
    }

    pub fn replace_pricing(&mut self, pricing: PricingEngine, cost_mode: CostMode) {
        self.pricing = pricing;
        self.cost_mode = cost_mode;
    }

    #[must_use]
    pub const fn cost_mode(&self) -> CostMode {
        self.cost_mode
    }

    pub fn product_data_status(&mut self) -> Result<crate::ProductDataStatusEnvelope, QueryError> {
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let capture = self
            .store
            .capture_product_data_status(
                tokenmaster_store::ProductDataStatusQuery::new(Duration::from_millis(
                    QUERY_DEADLINE_MS,
                ))
                .map_err(map_store_error)?,
            )
            .map_err(map_store_error)?;
        let generation = self.next_generation()?;
        let envelope = crate::status::map_capture(capture, generation, time.wall_time_ms())?;
        self.last_generation = Some(generation);
        Ok(envelope)
    }

    pub fn quota_windows(
        &mut self,
        request: QuotaCurrentRequest,
    ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let store_query =
            quota::build_current_query(&request, Duration::from_millis(QUERY_DEADLINE_MS))?;
        let capture = self
            .store
            .capture_quota_windows(store_query)
            .map_err(map_store_error)?;
        let mapped = quota::map_current_capture(&capture, &request, time.wall_time_ms())?;
        let generation = self.next_generation()?;
        let header = QuotaQueryHeader::new(QuotaQueryHeaderParts {
            snapshot_generation: generation,
            quota_revision: mapped.quota_revision,
            generated_at_ms: time.wall_time_ms(),
            data_through_ms: mapped.data_through_ms,
            freshness: mapped.freshness,
            quality: mapped.quality,
            filters: mapped.filters,
            warnings: mapped.warnings,
        })?;
        self.last_generation = Some(generation);
        Ok(QuotaEnvelope::new(header, mapped.payload))
    }

    pub fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let store_query = quota::build_overview_query(Duration::from_millis(QUERY_DEADLINE_MS))?;
        let capture = self
            .store
            .capture_quota_overview(store_query)
            .map_err(map_store_error)?;
        let mapped = quota::map_overview_capture(&capture, time.wall_time_ms())?;
        let generation = self.next_generation()?;
        let header = QuotaQueryHeader::new(QuotaQueryHeaderParts {
            snapshot_generation: generation,
            quota_revision: mapped.quota_revision,
            generated_at_ms: time.wall_time_ms(),
            data_through_ms: mapped.data_through_ms,
            freshness: mapped.freshness,
            quality: mapped.quality,
            filters: mapped.filters,
            warnings: mapped.warnings,
        })?;
        self.last_generation = Some(generation);
        Ok(QuotaEnvelope::new(header, mapped.payload))
    }

    pub fn benefit_inventory(
        &mut self,
        request: BenefitCurrentRequest,
    ) -> Result<BenefitEnvelope<BenefitCurrentSnapshot>, QueryError> {
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let store_query =
            benefit::build_current_query(&request, Duration::from_millis(QUERY_DEADLINE_MS))?;
        let capture = self
            .store
            .capture_benefit_current(store_query)
            .map_err(map_store_error)?;
        let mapped = benefit::map_current_capture(&capture, &request, time.wall_time_ms())?;
        let generation = self.next_generation()?;
        let header = BenefitQueryHeader::new(BenefitQueryHeaderParts {
            snapshot_generation: generation,
            benefit_revision: mapped.benefit_revision,
            generated_at_ms: time.wall_time_ms(),
            data_through_ms: mapped.data_through_ms,
            freshness: mapped.freshness,
            quality: mapped.quality,
            filter: mapped.filter,
            warnings: mapped.warnings,
        })?;
        self.last_generation = Some(generation);
        Ok(BenefitEnvelope::new(header, mapped.payload))
    }

    pub fn benefit_changes(
        &mut self,
        request: BenefitChangePageRequest,
    ) -> Result<BenefitEnvelope<BenefitChangePage>, QueryError> {
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let store_query =
            benefit::build_change_query(&request, Duration::from_millis(QUERY_DEADLINE_MS))?;
        let capture = self
            .store
            .capture_benefit_changes(store_query)
            .map_err(map_store_error)?;
        let mapped = benefit::map_change_capture(&capture, &request, time.wall_time_ms())?;
        let generation = self.next_generation()?;
        let header = BenefitQueryHeader::new(BenefitQueryHeaderParts {
            snapshot_generation: generation,
            benefit_revision: mapped.benefit_revision,
            generated_at_ms: time.wall_time_ms(),
            data_through_ms: mapped.data_through_ms,
            freshness: mapped.freshness,
            quality: mapped.quality,
            filter: mapped.filter,
            warnings: mapped.warnings,
        })?;
        self.last_generation = Some(generation);
        Ok(BenefitEnvelope::new(header, mapped.payload))
    }

    pub fn quota_transitions(
        &mut self,
        request: QuotaTransitionPageRequest,
    ) -> Result<QuotaEnvelope<QuotaTransitionPage>, QueryError> {
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let store_query =
            quota::build_transition_query(&request, Duration::from_millis(QUERY_DEADLINE_MS))?;
        let capture = self
            .store
            .capture_quota_transitions(store_query)
            .map_err(map_store_error)?;
        let mapped = quota::map_transition_capture(&capture, &request, time.wall_time_ms())?;
        let generation = self.next_generation()?;
        let header = QuotaQueryHeader::new(QuotaQueryHeaderParts {
            snapshot_generation: generation,
            quota_revision: mapped.quota_revision,
            generated_at_ms: time.wall_time_ms(),
            data_through_ms: mapped.data_through_ms,
            freshness: mapped.freshness,
            quality: mapped.quality,
            filters: mapped.filters,
            warnings: mapped.warnings,
        })?;
        self.last_generation = Some(generation);
        Ok(QuotaEnvelope::new(header, mapped.payload))
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
        let envelope = map_activity_capture(capture, generation, time.wall_time_ms())?;
        self.last_generation = Some(generation);
        Ok(envelope)
    }

    pub fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        let generation = self.next_generation()?;
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let (plan, store_query) = analytics::build_store_query(
            &request,
            time.wall_time_ms(),
            Duration::from_millis(QUERY_DEADLINE_MS),
        )?;
        let capture = self
            .store
            .capture_usage_analytics(store_query)
            .map_err(map_store_error)?;
        let price_query = analytics::build_store_price_query(
            &plan,
            capture.publication().dataset_identity(),
            request.scopes(),
            Duration::from_millis(QUERY_DEADLINE_MS),
        )?;
        let price_capture = self
            .store
            .capture_usage_price_basis_batch(price_query)
            .map_err(map_store_error)?;
        let breakdown_price_queries = analytics::build_store_breakdown_price_queries(
            &plan,
            &capture,
            request.scopes(),
            Duration::from_millis(QUERY_DEADLINE_MS),
        )?;
        let breakdown_price_captures = breakdown_price_queries
            .into_iter()
            .map(|query| {
                query
                    .map(|query| {
                        self.store
                            .capture_usage_breakdown_price_basis(query)
                            .map_err(map_store_error)
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let payload = analytics::map_capture(
            plan,
            &capture,
            &price_capture,
            &breakdown_price_captures,
            &self.pricing,
            self.cost_mode,
        )?;
        let header = map_header(
            capture.publication(),
            generation,
            time.wall_time_ms(),
            request.scopes().to_vec(),
        )?;
        self.last_generation = Some(generation);
        Ok(QueryEnvelope::new(header, payload))
    }

    pub fn git_output(
        &mut self,
        request: GitOutputRequest,
    ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
        let generation = self.next_generation()?;
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let started = Instant::now();
        let usage_request = crate::git_output::usage_request(&request)?;
        let plan = analytics::build_plan(&usage_request, time.wall_time_ms())?;
        let range = crate::git_output::range_from_plan(&plan)?;
        let end_day_index = range
            .end_day_index_exclusive()
            .checked_sub(1)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let git_query = tokenmaster_store::GitOutputQuery::new(
            range.start_day_index(),
            end_day_index,
            request.max_repositories(),
            remaining(started)?,
        )
        .map_err(map_store_error)?;
        let capture = self
            .store
            .capture_git_output(git_query)
            .map_err(map_store_error)?;

        let usage_join = (|| {
            let usage_query =
                analytics::build_store_query_from_plan(&plan, &usage_request, remaining(started)?)?;
            let usage_capture = self
                .store
                .capture_usage_analytics(usage_query)
                .map_err(map_store_error)?;
            let price_queries = analytics::build_store_breakdown_price_queries(
                &plan,
                &usage_capture,
                usage_request.scopes(),
                remaining(started)?,
            )?;
            if price_queries.len() != 1 {
                return Err(QueryError::new(QueryErrorCode::CorruptArchive));
            }
            let price_capture = price_queries
                .into_iter()
                .next()
                .flatten()
                .map(|query| {
                    self.store
                        .capture_usage_breakdown_price_basis(query)
                        .map_err(map_store_error)
                })
                .transpose()?;
            let store_breakdown = usage_capture
                .breakdowns()
                .first()
                .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
            let breakdown = analytics::map_breakdown(
                store_breakdown,
                price_capture.as_ref(),
                &self.pricing,
                self.cost_mode,
            )?;
            let projects = crate::git_output::project_aliases(store_breakdown)?;
            let project_keys = capture
                .repositories()
                .iter()
                .filter_map(tokenmaster_store::GitOutputRepositoryCapture::project_key)
                .collect::<Vec<_>>();
            let matches = tokenmaster_store::GitProjectMatchQuery::new(
                project_keys,
                projects,
                remaining(started)?,
            )
            .map_err(map_store_error)?;
            let matches = self
                .store
                .capture_git_project_matches(matches)
                .map_err(map_store_error)?;
            Ok::<_, QueryError>((
                crate::git_output::GitUsageEvidence::Available {
                    publication: usage_capture.publication().clone(),
                    breakdown,
                },
                matches.project_indices().to_vec(),
            ))
        })();
        let (evidence, project_indices) = match usage_join {
            Ok(value) => value,
            Err(error) => {
                let reason = match error.code() {
                    QueryErrorCode::DeadlineExceeded => {
                        crate::GitEfficiencyUnavailableReason::UsageDeadlineExceeded
                    }
                    QueryErrorCode::Unavailable | QueryErrorCode::StaleSnapshot => {
                        crate::GitEfficiencyUnavailableReason::UsageEvidenceUnavailable
                    }
                    QueryErrorCode::CapacityExceeded
                    | QueryErrorCode::CorruptArchive
                    | QueryErrorCode::Overflow
                    | QueryErrorCode::VersionMismatch => {
                        crate::GitEfficiencyUnavailableReason::UsageEvidenceInvalid
                    }
                    _ => return Err(error),
                };
                (
                    crate::git_output::GitUsageEvidence::Unavailable(reason),
                    Vec::new(),
                )
            }
        };
        let envelope = crate::git_output::map_snapshot(
            capture,
            range,
            generation,
            time.wall_time_ms(),
            evidence,
            &project_indices,
            request.scopes().to_vec(),
        )?;
        self.last_generation = Some(generation);
        Ok(envelope)
    }

    pub fn usage_sessions(
        &mut self,
        request: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
        let generation = self.next_generation()?;
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let store_query =
            session::build_page_query(&request, Duration::from_millis(QUERY_DEADLINE_MS))?;
        let capture = self
            .store
            .capture_usage_session_page(store_query)
            .map_err(map_store_error)?;
        let price_query =
            session::build_page_price_query(&capture, Duration::from_millis(QUERY_DEADLINE_MS))?;
        let price_capture = price_query
            .map(|query| {
                self.store
                    .capture_usage_session_price_basis_batch(query)
                    .map_err(map_store_error)
            })
            .transpose()?;
        let payload = session::map_page_capture(
            &capture,
            price_capture.as_ref(),
            &request,
            &self.pricing,
            self.cost_mode,
        )?;
        let header = map_header(
            capture.publication(),
            generation,
            time.wall_time_ms(),
            request.scopes().to_vec(),
        )?;
        self.last_generation = Some(generation);
        Ok(QueryEnvelope::new(header, payload))
    }

    pub fn usage_session_detail(
        &mut self,
        key: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        let generation = self.next_generation()?;
        let time = self.clock.sample()?;
        time.monotonic_ms()
            .checked_add(QUERY_DEADLINE_MS)
            .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
        let store_query =
            session::build_detail_query(&key, Duration::from_millis(QUERY_DEADLINE_MS))?;
        let scopes = vec![key.scope().clone()];
        let capture = self
            .store
            .capture_usage_session_detail(store_query)
            .map_err(map_store_error)?;
        let summary_price = if capture.detail().is_some() {
            let query =
                session::build_detail_price_query(&key, Duration::from_millis(QUERY_DEADLINE_MS))?;
            Some(
                self.store
                    .capture_usage_session_price_basis(query)
                    .map_err(map_store_error)?,
            )
        } else {
            None
        };
        let breakdown_price_queries = session::build_detail_breakdown_price_queries(
            &capture,
            &key,
            Duration::from_millis(QUERY_DEADLINE_MS),
        )?;
        let breakdown_price_captures = breakdown_price_queries
            .into_iter()
            .map(|query| {
                query
                    .map(|query| {
                        self.store
                            .capture_usage_session_breakdown_price_basis(query)
                            .map_err(map_store_error)
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let payload = session::map_detail_capture(
            &capture,
            summary_price.as_ref(),
            &breakdown_price_captures,
            &self.pricing,
            self.cost_mode,
        )?;
        let header = map_header(
            capture.publication(),
            generation,
            time.wall_time_ms(),
            scopes,
        )?;
        self.last_generation = Some(generation);
        Ok(QueryEnvelope::new(header, payload))
    }

    fn next_generation(&self) -> Result<SnapshotGeneration, QueryError> {
        match self.last_generation {
            Some(current) => current.checked_next(),
            None => SnapshotGeneration::new(1),
        }
    }
}

fn remaining(started: Instant) -> Result<Duration, QueryError> {
    Duration::from_millis(QUERY_DEADLINE_MS)
        .checked_sub(started.elapsed())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| QueryError::new(QueryErrorCode::DeadlineExceeded))
}

impl<C> fmt::Debug for QueryService<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QueryService([redacted])")
    }
}

fn map_activity_capture(
    capture: UsageQueryCapture,
    generation: SnapshotGeneration,
    generated_at_ms: i64,
) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
    let publication = capture.publication();
    let mut items = Vec::with_capacity(capture.events().len());
    for event in capture.events() {
        items.push(map_event(event)?);
    }
    let next_cursor = capture.next_cursor().map(ActivityCursor::from_store);
    let page = LatestActivityPage::new(items, next_cursor, capture.has_more())?;
    let header = map_header(publication, generation, generated_at_ms, Vec::new())?;
    Ok(QueryEnvelope::new(header, page))
}

fn map_header(
    publication: &UsageQueryPublication,
    generation: SnapshotGeneration,
    generated_at_ms: i64,
    scopes: Vec<QueryScope>,
) -> Result<QueryHeader, QueryError> {
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
    QueryHeader::new(QueryHeaderParts {
        snapshot_generation: generation,
        publication_generation: PublicationGeneration::new(publication.generation())?,
        dataset_identity,
        generated_at_ms,
        data_through_ms: publication.data_through_ms(),
        freshness,
        quality,
        scopes,
        warnings,
    })
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

pub(crate) fn map_quality(
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

pub(crate) fn map_freshness(
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

pub(crate) const fn to_store_identity(identity: DatasetIdentity) -> UsageQueryDatasetIdentity {
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

pub(crate) fn from_store_identity(
    identity: UsageQueryDatasetIdentity,
) -> Result<DatasetIdentity, QueryError> {
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

pub(crate) fn map_store_error(error: StoreError) -> QueryError {
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
        StoreErrorCode::RebuildRequired => QueryErrorCode::Unavailable,
        StoreErrorCode::StaleCheckpoint
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
