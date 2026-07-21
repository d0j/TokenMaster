use std::{fmt, sync::Arc, time::Duration};

use tokenmaster_domain::{UsageProfileId, UsageProviderId};
use tokenmaster_pricing::{CostMode, CostResult, PricingEngine};
use tokenmaster_store::{
    ScanScope, UsageSessionBreakdownPriceBasisQuery as StoreBreakdownPriceQuery,
    UsageSessionCursor as StoreCursor, UsageSessionDetailCapture as StoreDetailCapture,
    UsageSessionDetailQuery as StoreDetailQuery, UsageSessionKey as StoreKey,
    UsageSessionPageCapture as StorePageCapture, UsageSessionPageQuery as StorePageQuery,
    UsageSessionPriceBasisBatchQuery as StorePriceBatchQuery,
    UsageSessionPriceBasisQuery as StorePriceQuery, UsageSessionSummary as StoreSummary,
};

use crate::{
    DatasetIdentity, PageSize, QueryError, QueryErrorCode, QueryScope, UsageBreakdown,
    UsageMetrics, analytics, service,
};

#[derive(Clone, Eq, PartialEq)]
pub struct UsageSessionKey {
    dataset_identity: DatasetIdentity,
    scope: QueryScope,
    inner: StoreKey,
}

impl UsageSessionKey {
    #[must_use]
    pub const fn dataset_identity(&self) -> DatasetIdentity {
        self.dataset_identity
    }

    #[must_use]
    pub const fn scope(&self) -> &QueryScope {
        &self.scope
    }
}

impl fmt::Debug for UsageSessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsageSessionKey")
            .field("dataset_identity", &self.dataset_identity)
            .field("scope", &self.scope)
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct UsageSessionCursor {
    dataset_identity: DatasetIdentity,
    scopes: Box<[QueryScope]>,
    inner: StoreCursor,
}

impl UsageSessionCursor {
    #[must_use]
    pub const fn dataset_identity(&self) -> DatasetIdentity {
        self.dataset_identity
    }
}

impl fmt::Debug for UsageSessionCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsageSessionCursor")
            .field("dataset_identity", &self.dataset_identity)
            .field("scope_count", &self.scopes.len())
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionPageRequest {
    page_size: PageSize,
    scopes: Box<[QueryScope]>,
    continuation: Option<UsageSessionCursor>,
}

impl UsageSessionPageRequest {
    pub fn first(page_size: PageSize, scopes: Vec<QueryScope>) -> Result<Self, QueryError> {
        Self::new(page_size, scopes, None)
    }

    pub fn continuation(
        page_size: PageSize,
        cursor: UsageSessionCursor,
        scopes: Vec<QueryScope>,
    ) -> Result<Self, QueryError> {
        Self::new(page_size, scopes, Some(cursor))
    }

    fn new(
        page_size: PageSize,
        mut scopes: Vec<QueryScope>,
        continuation: Option<UsageSessionCursor>,
    ) -> Result<Self, QueryError> {
        if scopes.len() > crate::MAX_QUERY_SCOPES {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        scopes.sort_by(|left, right| {
            left.provider_id()
                .as_str()
                .cmp(right.provider_id().as_str())
                .then_with(|| left.profile_id().as_str().cmp(right.profile_id().as_str()))
        });
        if scopes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        if continuation
            .as_ref()
            .is_some_and(|cursor| cursor.dataset_identity == DatasetIdentity::Empty)
        {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        if continuation
            .as_ref()
            .is_some_and(|cursor| cursor.scopes.as_ref() != scopes.as_slice())
        {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self {
            page_size,
            scopes: scopes.into_boxed_slice(),
            continuation,
        })
    }

    #[must_use]
    pub const fn page_size(&self) -> PageSize {
        self.page_size
    }

    #[must_use]
    pub const fn scopes(&self) -> &[QueryScope] {
        &self.scopes
    }

    #[must_use]
    pub const fn is_continuation(&self) -> bool {
        self.continuation.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionSummary {
    key: UsageSessionKey,
    first_timestamp_seconds: i64,
    first_timestamp_nanos: u32,
    last_timestamp_seconds: i64,
    last_timestamp_nanos: u32,
    metrics: UsageMetrics,
    cost: CostResult,
}

impl UsageSessionSummary {
    #[must_use]
    pub const fn key(&self) -> &UsageSessionKey {
        &self.key
    }

    #[must_use]
    pub const fn scope(&self) -> &QueryScope {
        self.key.scope()
    }

    #[must_use]
    pub const fn first_timestamp_seconds(&self) -> i64 {
        self.first_timestamp_seconds
    }

    #[must_use]
    pub const fn first_timestamp_nanos(&self) -> u32 {
        self.first_timestamp_nanos
    }

    #[must_use]
    pub const fn last_timestamp_seconds(&self) -> i64 {
        self.last_timestamp_seconds
    }

    #[must_use]
    pub const fn last_timestamp_nanos(&self) -> u32 {
        self.last_timestamp_nanos
    }

    #[must_use]
    pub const fn metrics(&self) -> &UsageMetrics {
        &self.metrics
    }

    #[must_use]
    pub const fn cost(&self) -> &CostResult {
        &self.cost
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionPage {
    page_kind: UsageSessionPageKind,
    sessions: Arc<[UsageSessionSummary]>,
    next_cursor: Option<UsageSessionCursor>,
    has_more: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageSessionPageKind {
    Newest,
    Continuation,
}

impl UsageSessionPage {
    #[must_use]
    pub const fn page_kind(&self) -> UsageSessionPageKind {
        self.page_kind
    }

    #[must_use]
    pub const fn sessions(&self) -> &Arc<[UsageSessionSummary]> {
        &self.sessions
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&UsageSessionCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionDetail {
    summary: UsageSessionSummary,
    breakdowns: Arc<[UsageBreakdown]>,
}

impl UsageSessionDetail {
    #[must_use]
    pub const fn summary(&self) -> &UsageSessionSummary {
        &self.summary
    }

    #[must_use]
    pub const fn breakdowns(&self) -> &Arc<[UsageBreakdown]> {
        &self.breakdowns
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSessionDetailResult {
    detail: Option<UsageSessionDetail>,
}

impl UsageSessionDetailResult {
    #[must_use]
    pub const fn detail(&self) -> Option<&UsageSessionDetail> {
        self.detail.as_ref()
    }
}

pub(crate) fn build_page_query(
    request: &UsageSessionPageRequest,
    deadline: Duration,
) -> Result<StorePageQuery, QueryError> {
    let (expected_dataset, before) = match &request.continuation {
        Some(cursor) => (
            Some(service::to_store_identity(cursor.dataset_identity)),
            Some(cursor.inner.clone()),
        ),
        None => (None, None),
    };
    let scopes = request
        .scopes
        .iter()
        .map(|scope| ScanScope::new(scope.provider_id().as_str(), scope.profile_id().as_str()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| QueryError::new(QueryErrorCode::InvalidValue))?
        .into_boxed_slice();
    StorePageQuery::new(
        expected_dataset,
        before,
        scopes,
        request.page_size.get(),
        deadline,
    )
    .map_err(service::map_store_error)
}

pub(crate) fn map_page_capture(
    capture: &StorePageCapture,
    prices: Option<&tokenmaster_store::UsagePriceBasisBatchCapture>,
    request: &UsageSessionPageRequest,
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<UsageSessionPage, QueryError> {
    let dataset_identity = service::from_store_identity(capture.publication().dataset_identity())?;
    if dataset_identity == DatasetIdentity::Empty && !capture.sessions().is_empty() {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    if capture.sessions().is_empty() {
        if prices.is_some() {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
    } else {
        let prices = prices.ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
        if prices.publication().dataset_identity() != capture.publication().dataset_identity()
            || prices.targets().len() != capture.sessions().len()
        {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
    }
    let sessions = capture
        .sessions()
        .iter()
        .enumerate()
        .map(|(index, summary)| {
            let price = prices
                .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?
                .targets()
                .get(index)
                .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
            map_summary(summary, dataset_identity, price, pricing, cost_mode)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = capture.next_cursor().map(|inner| UsageSessionCursor {
        dataset_identity,
        scopes: request.scopes.clone(),
        inner: inner.clone(),
    });
    if capture.has_more() != next_cursor.is_some() {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    Ok(UsageSessionPage {
        page_kind: if request.is_continuation() {
            UsageSessionPageKind::Continuation
        } else {
            UsageSessionPageKind::Newest
        },
        sessions: Arc::from(sessions),
        next_cursor,
        has_more: capture.has_more(),
    })
}

pub(crate) fn build_page_price_query(
    capture: &StorePageCapture,
    deadline: Duration,
) -> Result<Option<StorePriceBatchQuery>, QueryError> {
    if capture.sessions().is_empty() {
        return Ok(None);
    }
    let sessions = capture
        .sessions()
        .iter()
        .map(|summary| summary.key().clone())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    StorePriceBatchQuery::new(capture.publication().dataset_identity(), sessions, deadline)
        .map(Some)
        .map_err(service::map_store_error)
}

pub(crate) fn build_detail_query(
    key: &UsageSessionKey,
    deadline: Duration,
) -> Result<StoreDetailQuery, QueryError> {
    StoreDetailQuery::new(
        service::to_store_identity(key.dataset_identity),
        key.inner.clone(),
        deadline,
    )
    .map_err(service::map_store_error)
}

pub(crate) fn build_detail_price_query(
    key: &UsageSessionKey,
    deadline: Duration,
) -> Result<StorePriceQuery, QueryError> {
    StorePriceQuery::new(
        service::to_store_identity(key.dataset_identity),
        key.inner.clone(),
        deadline,
    )
    .map_err(service::map_store_error)
}

pub(crate) fn build_detail_breakdown_price_queries(
    capture: &StoreDetailCapture,
    key: &UsageSessionKey,
    deadline: Duration,
) -> Result<Vec<Option<StoreBreakdownPriceQuery>>, QueryError> {
    let Some(detail) = capture.detail() else {
        return Ok(Vec::new());
    };
    detail
        .breakdowns()
        .iter()
        .map(|breakdown| {
            if breakdown.items().is_empty() {
                return Ok(None);
            }
            let targets = breakdown
                .items()
                .iter()
                .map(|item| item.identity().clone())
                .collect::<Vec<_>>()
                .into_boxed_slice();
            StoreBreakdownPriceQuery::new(
                capture.publication().dataset_identity(),
                key.inner.clone(),
                breakdown.kind(),
                targets,
                deadline,
            )
            .map(Some)
            .map_err(service::map_store_error)
        })
        .collect()
}

pub(crate) fn map_detail_capture(
    capture: &StoreDetailCapture,
    summary_price: Option<&tokenmaster_store::UsagePriceBasisCapture>,
    breakdown_prices: &[Option<tokenmaster_store::UsagePriceBasisBatchCapture>],
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<UsageSessionDetailResult, QueryError> {
    let dataset_identity = service::from_store_identity(capture.publication().dataset_identity())?;
    if capture.detail().is_none() {
        if summary_price.is_some() || !breakdown_prices.is_empty() {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
        return Ok(UsageSessionDetailResult { detail: None });
    }
    let summary_price =
        summary_price.ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
    if summary_price.publication().dataset_identity() != capture.publication().dataset_identity() {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    let detail = capture
        .detail()
        .map(|detail| {
            if breakdown_prices.len() != detail.breakdowns().len() {
                return Err(QueryError::new(QueryErrorCode::CorruptArchive));
            }
            let summary_metrics = UsageMetrics::from_store(detail.summary().metrics())?;
            let summary_cost = analytics::map_single_cost(
                summary_price,
                summary_metrics.event_count(),
                pricing,
                cost_mode,
            )?;
            let summary = map_summary_with_metrics(
                detail.summary(),
                dataset_identity,
                summary_metrics,
                summary_cost,
            )?;
            let breakdowns = detail
                .breakdowns()
                .iter()
                .zip(breakdown_prices)
                .map(|(breakdown, prices)| {
                    if prices.as_ref().is_some_and(|prices| {
                        prices.publication().dataset_identity()
                            != capture.publication().dataset_identity()
                    }) {
                        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
                    }
                    analytics::map_breakdown(breakdown, prices.as_ref(), pricing, cost_mode)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(UsageSessionDetail {
                summary,
                breakdowns: Arc::from(breakdowns),
            })
        })
        .transpose()?;
    Ok(UsageSessionDetailResult { detail })
}

fn map_summary(
    value: &StoreSummary,
    dataset_identity: DatasetIdentity,
    price: &tokenmaster_store::UsagePriceBasisTargetCapture,
    pricing: &PricingEngine,
    cost_mode: CostMode,
) -> Result<UsageSessionSummary, QueryError> {
    let metrics = UsageMetrics::from_store(value.metrics())?;
    let cost = analytics::map_cost(price, metrics.event_count(), pricing, cost_mode)?;
    map_summary_with_metrics(value, dataset_identity, metrics, cost)
}

fn map_summary_with_metrics(
    value: &StoreSummary,
    dataset_identity: DatasetIdentity,
    metrics: UsageMetrics,
    cost: CostResult,
) -> Result<UsageSessionSummary, QueryError> {
    let scope = QueryScope::new(
        UsageProviderId::new(value.provider_id().to_owned())
            .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?,
        UsageProfileId::new(value.profile_id().to_owned())
            .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?,
    );
    Ok(UsageSessionSummary {
        key: UsageSessionKey {
            dataset_identity,
            scope,
            inner: value.key().clone(),
        },
        first_timestamp_seconds: value.first_timestamp_seconds(),
        first_timestamp_nanos: value.first_timestamp_nanos(),
        last_timestamp_seconds: value.last_timestamp_seconds(),
        last_timestamp_nanos: value.last_timestamp_nanos(),
        metrics,
        cost,
    })
}
