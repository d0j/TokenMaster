use std::{fmt, sync::Arc, time::Duration};

use tokenmaster_domain::{
    QuotaConfidence as StoreConfidence, QuotaEvidenceSource as StoreEvidenceSource,
    QuotaPresentationDirection as StorePresentation, QuotaRatio,
    QuotaResetEvidence as StoreResetEvidence, QuotaResetThresholds, QuotaSample,
    QuotaSampleQuality as StoreSampleQuality, QuotaUnits, QuotaWindowDefinition, QuotaWindowKey,
    QuotaWindowSemantics as StoreSemantics,
};
use tokenmaster_quota::{
    QuotaAllowanceChangeKind as StoreAllowanceKind, QuotaDetectionTime as StoreDetectionTime,
    QuotaTransitionKind as StoreTransitionKind,
};
use tokenmaster_store::{
    MAX_QUOTA_CURRENT_WINDOWS, QuotaCurrentCapture as StoreCurrentCapture,
    QuotaCurrentEpoch as StoreCurrentEpoch, QuotaCurrentQuery as StoreCurrentQuery,
    QuotaCurrentWindow as StoreCurrentWindow, QuotaTransitionCursor as StoreCursor,
    QuotaTransitionPageCapture as StoreTransitionPageCapture,
    QuotaTransitionPageQuery as StoreTransitionPageQuery,
    QuotaTransitionRecord as StoreTransitionRecord,
};

use crate::{
    PageSize, QueryError, QueryErrorCode, QueryFreshness, QueryQuality, QuotaRevision,
    QuotaWarningCode, QuotaWindowFilter,
};

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaCurrentRequest {
    filters: Box<[QuotaWindowFilter]>,
}

impl QuotaCurrentRequest {
    pub fn new(windows: Vec<QuotaWindowKey>) -> Result<Self, QueryError> {
        if windows.len() > MAX_QUOTA_CURRENT_WINDOWS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        if has_duplicate_windows(&windows) {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self {
            filters: windows
                .into_iter()
                .map(QuotaWindowFilter::new)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn filters(&self) -> &[QuotaWindowFilter] {
        &self.filters
    }
}

impl fmt::Debug for QuotaCurrentRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaCurrentRequest")
            .field("filter_count", &self.filters.len())
            .field("filters", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaTransitionCursor {
    quota_revision: QuotaRevision,
    store_revision: tokenmaster_store::QuotaRevision,
    filter: QuotaWindowFilter,
    inner: StoreCursor,
}

impl QuotaTransitionCursor {
    #[must_use]
    pub const fn quota_revision(&self) -> QuotaRevision {
        self.quota_revision
    }
}

impl fmt::Debug for QuotaTransitionCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaTransitionCursor")
            .field("quota_revision", &self.quota_revision)
            .field("filter", &"[redacted]")
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaTransitionPageRequest {
    filter: QuotaWindowFilter,
    page_size: PageSize,
    continuation: Option<QuotaTransitionCursor>,
}

impl QuotaTransitionPageRequest {
    pub fn first(window: QuotaWindowKey, page_size: PageSize) -> Result<Self, QueryError> {
        Self::new(window, page_size, None)
    }

    pub fn continuation(
        window: QuotaWindowKey,
        page_size: PageSize,
        cursor: QuotaTransitionCursor,
    ) -> Result<Self, QueryError> {
        Self::new(window, page_size, Some(cursor))
    }

    fn new(
        window: QuotaWindowKey,
        page_size: PageSize,
        continuation: Option<QuotaTransitionCursor>,
    ) -> Result<Self, QueryError> {
        let filter = QuotaWindowFilter::new(window);
        if continuation
            .as_ref()
            .is_some_and(|cursor| cursor.filter != filter)
        {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        Ok(Self {
            filter,
            page_size,
            continuation,
        })
    }

    #[must_use]
    pub const fn filter(&self) -> &QuotaWindowFilter {
        &self.filter
    }

    #[must_use]
    pub const fn page_size(&self) -> PageSize {
        self.page_size
    }

    #[must_use]
    pub const fn is_continuation(&self) -> bool {
        self.continuation.is_some()
    }
}

impl fmt::Debug for QuotaTransitionPageRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaTransitionPageRequest")
            .field("filter", &"[redacted]")
            .field("page_size", &self.page_size)
            .field("continuation", &self.continuation)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaPresentation {
    Used,
    Remaining,
    Pace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaWindowSemantics {
    Fixed,
    Rolling,
    Credit,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaSampleQuality {
    Authoritative,
    Partial,
    Conflict,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaEvidenceSource {
    ProviderLocal,
    ProviderOfficial,
    LocalResetEvent,
    Manual,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaResetEvidence {
    None,
    ExplicitProvider,
    ExplicitLocal,
    ManualOrBanked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaTransitionKind {
    ScheduledReset,
    EarlyReset,
    ManualOrBankedReset,
    UnknownReset,
    AllowanceChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaAllowanceChangeKind {
    Increased,
    Decreased,
    UnitChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaDetectionTime {
    Exact { at_ms: i64 },
    Interval { after_ms: i64, at_or_before_ms: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotaRatioValue(u32);

impl QuotaRatioValue {
    #[must_use]
    pub const fn parts_per_million(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaUnitsValue {
    unit_id: Arc<str>,
    used: Option<u64>,
    remaining: Option<u64>,
    capacity: Option<u64>,
}

impl QuotaUnitsValue {
    #[must_use]
    pub fn unit_id(&self) -> &str {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaResetThresholdsValue {
    maximum_post_reset_used_ratio: Option<QuotaRatioValue>,
    minimum_post_reset_remaining_ratio: Option<QuotaRatioValue>,
    minimum_used_ratio_drop: Option<QuotaRatioValue>,
}

impl QuotaResetThresholdsValue {
    #[must_use]
    pub const fn maximum_post_reset_used_ratio(&self) -> Option<QuotaRatioValue> {
        self.maximum_post_reset_used_ratio
    }

    #[must_use]
    pub const fn minimum_post_reset_remaining_ratio(&self) -> Option<QuotaRatioValue> {
        self.minimum_post_reset_remaining_ratio
    }

    #[must_use]
    pub const fn minimum_used_ratio_drop(&self) -> Option<QuotaRatioValue> {
        self.minimum_used_ratio_drop
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaDefinitionValue {
    revision: u64,
    label_key: Arc<str>,
    presentation: QuotaPresentation,
    semantics: QuotaWindowSemantics,
    nominal_duration_seconds: Option<u64>,
    reset_thresholds: Option<QuotaResetThresholdsValue>,
}

impl QuotaDefinitionValue {
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn label_key(&self) -> &str {
        &self.label_key
    }

    #[must_use]
    pub const fn presentation(&self) -> QuotaPresentation {
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
    pub const fn reset_thresholds(&self) -> Option<&QuotaResetThresholdsValue> {
        self.reset_thresholds.as_ref()
    }
}

impl fmt::Debug for QuotaDefinitionValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaDefinitionValue")
            .field("revision", &self.revision)
            .field("label_key", &"[redacted]")
            .field("presentation", &self.presentation)
            .field("semantics", &self.semantics)
            .field("nominal_duration_seconds", &self.nominal_duration_seconds)
            .field("reset_thresholds", &self.reset_thresholds)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaSampleValue {
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    provider_epoch_id: Option<Arc<str>>,
    used_ratio: Option<QuotaRatioValue>,
    remaining_ratio: Option<QuotaRatioValue>,
    units: Option<QuotaUnitsValue>,
    advertised_resets_at_ms: Option<i64>,
    quality: QuotaSampleQuality,
    source: QuotaEvidenceSource,
    confidence: QuotaConfidence,
    reset_evidence: QuotaResetEvidence,
    reset_occurred_at_ms: Option<i64>,
}

impl QuotaSampleValue {
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
    pub fn provider_epoch_id(&self) -> Option<&str> {
        self.provider_epoch_id.as_deref()
    }

    #[must_use]
    pub const fn used_ratio(&self) -> Option<QuotaRatioValue> {
        self.used_ratio
    }

    #[must_use]
    pub const fn remaining_ratio(&self) -> Option<QuotaRatioValue> {
        self.remaining_ratio
    }

    #[must_use]
    pub const fn units(&self) -> Option<&QuotaUnitsValue> {
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

impl fmt::Debug for QuotaSampleValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaSampleValue")
            .field("observed_at_ms", &self.observed_at_ms)
            .field("fresh_until_ms", &self.fresh_until_ms)
            .field("stale_after_ms", &self.stale_after_ms)
            .field(
                "provider_epoch_id",
                &self.provider_epoch_id.as_ref().map(|_| "[redacted]"),
            )
            .field("used_ratio", &self.used_ratio)
            .field("remaining_ratio", &self.remaining_ratio)
            .field("units", &self.units)
            .field("advertised_resets_at_ms", &self.advertised_resets_at_ms)
            .field("quality", &self.quality)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .field("reset_evidence", &self.reset_evidence)
            .field("reset_occurred_at_ms", &self.reset_occurred_at_ms)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaEpochValue {
    epoch_definition_revision: u64,
    definition_revision: u64,
    first_sample: QuotaSampleValue,
    first_observed_at_ms: i64,
    last_observed_at_ms: i64,
    maximum_used_ratio: Option<QuotaRatioValue>,
    maximum_used_units: Option<QuotaUnitsValue>,
    provider_epoch_id: Option<Arc<str>>,
    advertised_resets_at_ms: Option<i64>,
    last_transition_sequence: u64,
}

impl QuotaEpochValue {
    #[must_use]
    pub const fn epoch_definition_revision(&self) -> u64 {
        self.epoch_definition_revision
    }

    #[must_use]
    pub const fn definition_revision(&self) -> u64 {
        self.definition_revision
    }

    #[must_use]
    pub const fn first_sample(&self) -> &QuotaSampleValue {
        &self.first_sample
    }

    #[must_use]
    pub const fn first_observed_at_ms(&self) -> i64 {
        self.first_observed_at_ms
    }

    #[must_use]
    pub const fn last_observed_at_ms(&self) -> i64 {
        self.last_observed_at_ms
    }

    #[must_use]
    pub const fn maximum_used_ratio(&self) -> Option<QuotaRatioValue> {
        self.maximum_used_ratio
    }

    #[must_use]
    pub const fn maximum_used_units(&self) -> Option<&QuotaUnitsValue> {
        self.maximum_used_units.as_ref()
    }

    #[must_use]
    pub fn provider_epoch_id(&self) -> Option<&str> {
        self.provider_epoch_id.as_deref()
    }

    #[must_use]
    pub const fn advertised_resets_at_ms(&self) -> Option<i64> {
        self.advertised_resets_at_ms
    }

    #[must_use]
    pub const fn last_transition_sequence(&self) -> u64 {
        self.last_transition_sequence
    }
}

impl fmt::Debug for QuotaEpochValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaEpochValue")
            .field("epoch_definition_revision", &self.epoch_definition_revision)
            .field("definition_revision", &self.definition_revision)
            .field("first_sample", &self.first_sample)
            .field("first_observed_at_ms", &self.first_observed_at_ms)
            .field("last_observed_at_ms", &self.last_observed_at_ms)
            .field("maximum_used_ratio", &self.maximum_used_ratio)
            .field("maximum_used_units", &self.maximum_used_units)
            .field(
                "provider_epoch_id",
                &self.provider_epoch_id.as_ref().map(|_| "[redacted]"),
            )
            .field("advertised_resets_at_ms", &self.advertised_resets_at_ms)
            .field("last_transition_sequence", &self.last_transition_sequence)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaAllowanceChangeValue {
    kind: QuotaAllowanceChangeKind,
    old_units: QuotaUnitsValue,
    new_units: QuotaUnitsValue,
}

impl QuotaAllowanceChangeValue {
    #[must_use]
    pub const fn kind(&self) -> QuotaAllowanceChangeKind {
        self.kind
    }

    #[must_use]
    pub const fn old_units(&self) -> &QuotaUnitsValue {
        &self.old_units
    }

    #[must_use]
    pub const fn new_units(&self) -> &QuotaUnitsValue {
        &self.new_units
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaTransitionValue {
    definition_revision: u64,
    sequence: u64,
    kind: QuotaTransitionKind,
    pre_sample: QuotaSampleValue,
    post_sample: QuotaSampleValue,
    maximum_used_ratio_before: Option<QuotaRatioValue>,
    maximum_used_units_before: Option<QuotaUnitsValue>,
    old_resets_at_ms: Option<i64>,
    new_resets_at_ms: Option<i64>,
    allowance_change: Option<QuotaAllowanceChangeValue>,
    source: QuotaEvidenceSource,
    confidence: QuotaConfidence,
    detection_time: QuotaDetectionTime,
}

impl QuotaTransitionValue {
    #[must_use]
    pub const fn definition_revision(&self) -> u64 {
        self.definition_revision
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn kind(&self) -> QuotaTransitionKind {
        self.kind
    }

    #[must_use]
    pub const fn pre_sample(&self) -> &QuotaSampleValue {
        &self.pre_sample
    }

    #[must_use]
    pub const fn post_sample(&self) -> &QuotaSampleValue {
        &self.post_sample
    }

    #[must_use]
    pub const fn maximum_used_ratio_before(&self) -> Option<QuotaRatioValue> {
        self.maximum_used_ratio_before
    }

    #[must_use]
    pub const fn maximum_used_units_before(&self) -> Option<&QuotaUnitsValue> {
        self.maximum_used_units_before.as_ref()
    }

    #[must_use]
    pub const fn old_resets_at_ms(&self) -> Option<i64> {
        self.old_resets_at_ms
    }

    #[must_use]
    pub const fn new_resets_at_ms(&self) -> Option<i64> {
        self.new_resets_at_ms
    }

    #[must_use]
    pub const fn allowance_change(&self) -> Option<&QuotaAllowanceChangeValue> {
        self.allowance_change.as_ref()
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
    pub const fn detection_time(&self) -> QuotaDetectionTime {
        self.detection_time
    }
}

impl fmt::Debug for QuotaTransitionValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaTransitionValue")
            .field("definition_revision", &self.definition_revision)
            .field("sequence", &self.sequence)
            .field("kind", &self.kind)
            .field("pre_sample", &self.pre_sample)
            .field("post_sample", &self.post_sample)
            .field("maximum_used_ratio_before", &self.maximum_used_ratio_before)
            .field("maximum_used_units_before", &self.maximum_used_units_before)
            .field("old_resets_at_ms", &self.old_resets_at_ms)
            .field("new_resets_at_ms", &self.new_resets_at_ms)
            .field("allowance_change", &self.allowance_change)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .field("detection_time", &self.detection_time)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaWindowValue {
    definition: QuotaDefinitionValue,
    current_sample: QuotaSampleValue,
    epoch: QuotaEpochValue,
    last_transition: Option<QuotaTransitionValue>,
    freshness: QueryFreshness,
    quality: QueryQuality,
}

impl QuotaWindowValue {
    #[must_use]
    pub const fn definition(&self) -> &QuotaDefinitionValue {
        &self.definition
    }

    #[must_use]
    pub const fn current_sample(&self) -> &QuotaSampleValue {
        &self.current_sample
    }

    #[must_use]
    pub const fn epoch(&self) -> &QuotaEpochValue {
        &self.epoch
    }

    #[must_use]
    pub const fn last_transition(&self) -> Option<&QuotaTransitionValue> {
        self.last_transition.as_ref()
    }

    #[must_use]
    pub const fn freshness(&self) -> QueryFreshness {
        self.freshness
    }

    #[must_use]
    pub const fn quality(&self) -> QueryQuality {
        self.quality
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QuotaWindowResult {
    filter: QuotaWindowFilter,
    snapshot: Option<QuotaWindowValue>,
}

impl QuotaWindowResult {
    #[must_use]
    pub const fn filter(&self) -> &QuotaWindowFilter {
        &self.filter
    }

    #[must_use]
    pub const fn snapshot(&self) -> Option<&QuotaWindowValue> {
        self.snapshot.as_ref()
    }
}

impl fmt::Debug for QuotaWindowResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaWindowResult")
            .field("filter", &"[redacted]")
            .field("snapshot", &self.snapshot)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaCurrentSnapshot {
    windows: Arc<[QuotaWindowResult]>,
}

impl QuotaCurrentSnapshot {
    #[must_use]
    pub const fn windows(&self) -> &Arc<[QuotaWindowResult]> {
        &self.windows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaTransitionPage {
    transitions: Arc<[QuotaTransitionValue]>,
    next_cursor: Option<QuotaTransitionCursor>,
    has_more: bool,
}

impl QuotaTransitionPage {
    #[must_use]
    pub const fn transitions(&self) -> &Arc<[QuotaTransitionValue]> {
        &self.transitions
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&QuotaTransitionCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

pub(crate) struct MappedQuotaPayload<T> {
    pub payload: T,
    pub quota_revision: QuotaRevision,
    pub data_through_ms: Option<i64>,
    pub freshness: QueryFreshness,
    pub quality: QueryQuality,
    pub filters: Vec<QuotaWindowFilter>,
    pub warnings: Vec<QuotaWarningCode>,
}

pub(crate) fn build_current_query(
    request: &QuotaCurrentRequest,
    deadline: Duration,
) -> Result<StoreCurrentQuery, QueryError> {
    StoreCurrentQuery::new(
        request
            .filters
            .iter()
            .map(|filter| filter.key().clone())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        deadline,
    )
    .map_err(crate::service::map_store_error)
}

pub(crate) fn build_transition_query(
    request: &QuotaTransitionPageRequest,
    deadline: Duration,
) -> Result<StoreTransitionPageQuery, QueryError> {
    let expected_revision = request
        .continuation
        .as_ref()
        .map(|cursor| cursor.store_revision);
    StoreTransitionPageQuery::new(
        request.filter.key().clone(),
        expected_revision,
        request
            .continuation
            .as_ref()
            .map(|cursor| cursor.inner.clone()),
        request.page_size.get(),
        deadline,
    )
    .map_err(crate::service::map_store_error)
}

pub(crate) fn map_current_capture(
    capture: &StoreCurrentCapture,
    request: &QuotaCurrentRequest,
    generated_at_ms: i64,
) -> Result<MappedQuotaPayload<QuotaCurrentSnapshot>, QueryError> {
    if capture.windows().len() > request.filters.len() {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    let quota_revision = QuotaRevision::new(capture.quota_revision().get())
        .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?;
    let mut matched = vec![false; capture.windows().len()];
    let mut windows = Vec::with_capacity(request.filters.len());
    let mut warnings = Vec::new();
    let mut freshness = if request.filters.is_empty() {
        QueryFreshness::Unavailable
    } else {
        QueryFreshness::Fresh
    };
    let mut quality = QueryQuality::Authoritative;
    let mut data_through_ms: Option<i64> = None;

    for filter in request.filters.iter() {
        let position = capture
            .windows()
            .iter()
            .enumerate()
            .find(|(index, window)| !matched[*index] && window.definition().key() == filter.key())
            .map(|(index, _window)| index);
        let snapshot = if let Some(index) = position {
            matched[index] = true;
            let value =
                map_current_window(&capture.windows()[index], generated_at_ms, &mut warnings)?;
            data_through_ms = Some(match data_through_ms {
                Some(current) => current.min(value.current_sample.observed_at_ms),
                None => value.current_sample.observed_at_ms,
            });
            freshness = merge_freshness(freshness, value.freshness);
            quality = merge_quality(quality, value.quality);
            Some(value)
        } else {
            push_warning(&mut warnings, QuotaWarningCode::WindowUnavailable);
            freshness = QueryFreshness::Unavailable;
            quality = QueryQuality::Unknown;
            None
        };
        windows.push(QuotaWindowResult {
            filter: filter.clone(),
            snapshot,
        });
    }
    if matched.iter().any(|matched| !matched) {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    Ok(MappedQuotaPayload {
        payload: QuotaCurrentSnapshot {
            windows: Arc::from(windows),
        },
        quota_revision,
        data_through_ms,
        freshness,
        quality,
        filters: request.filters.to_vec(),
        warnings,
    })
}

pub(crate) fn map_transition_capture(
    capture: &StoreTransitionPageCapture,
    request: &QuotaTransitionPageRequest,
    generated_at_ms: i64,
) -> Result<MappedQuotaPayload<QuotaTransitionPage>, QueryError> {
    if capture.transitions().len() > request.page_size.get()
        || capture
            .transitions()
            .windows(2)
            .any(|pair| pair[0].sequence() <= pair[1].sequence())
        || capture
            .transitions()
            .iter()
            .any(|record| record.transition().key() != request.filter.key())
    {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    let quota_revision = QuotaRevision::new(capture.quota_revision().get())
        .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive))?;
    let mut warnings = Vec::new();
    let mut transitions = Vec::with_capacity(capture.transitions().len());
    let mut quality = QueryQuality::Authoritative;
    for record in capture.transitions() {
        quality = merge_quality(quality, map_sample_quality(record.pre_sample().quality()));
        quality = merge_quality(quality, map_sample_quality(record.post_sample().quality()));
        add_quality_warning(&mut warnings, record.pre_sample().quality());
        add_quality_warning(&mut warnings, record.post_sample().quality());
        transitions.push(map_transition(record));
    }
    let data_through_ms = capture
        .transitions()
        .first()
        .map(|record| record.post_sample().observed_at_ms());
    let freshness = capture
        .transitions()
        .first()
        .map_or(QueryFreshness::Unavailable, |record| {
            map_sample_freshness(record.post_sample(), generated_at_ms, &mut warnings)
        });
    let next_cursor = capture.next_cursor().map(|inner| QuotaTransitionCursor {
        quota_revision,
        store_revision: capture.quota_revision(),
        filter: request.filter.clone(),
        inner: inner.clone(),
    });
    if capture.has_more() != next_cursor.is_some()
        || (capture.has_more() && transitions.len() != request.page_size.get())
    {
        return Err(QueryError::new(QueryErrorCode::CorruptArchive));
    }
    Ok(MappedQuotaPayload {
        payload: QuotaTransitionPage {
            transitions: Arc::from(transitions),
            next_cursor,
            has_more: capture.has_more(),
        },
        quota_revision,
        data_through_ms,
        freshness,
        quality,
        filters: vec![request.filter.clone()],
        warnings,
    })
}

fn map_current_window(
    window: &StoreCurrentWindow,
    generated_at_ms: i64,
    warnings: &mut Vec<QuotaWarningCode>,
) -> Result<QuotaWindowValue, QueryError> {
    let freshness = map_sample_freshness(window.sample(), generated_at_ms, warnings);
    let quality = map_sample_quality(window.sample().quality());
    add_quality_warning(warnings, window.sample().quality());
    Ok(QuotaWindowValue {
        definition: map_definition(window.definition()),
        current_sample: map_sample(window.sample()),
        epoch: map_epoch(window.epoch()),
        last_transition: window.last_transition().map(map_transition),
        freshness,
        quality,
    })
}

fn map_definition(definition: &QuotaWindowDefinition) -> QuotaDefinitionValue {
    QuotaDefinitionValue {
        revision: definition.revision(),
        label_key: Arc::from(definition.label_key()),
        presentation: match definition.presentation() {
            StorePresentation::Used => QuotaPresentation::Used,
            StorePresentation::Remaining => QuotaPresentation::Remaining,
            StorePresentation::Pace => QuotaPresentation::Pace,
        },
        semantics: match definition.semantics() {
            StoreSemantics::Fixed => QuotaWindowSemantics::Fixed,
            StoreSemantics::Rolling => QuotaWindowSemantics::Rolling,
            StoreSemantics::Credit => QuotaWindowSemantics::Credit,
            StoreSemantics::Unknown => QuotaWindowSemantics::Unknown,
        },
        nominal_duration_seconds: definition.nominal_duration_seconds(),
        reset_thresholds: definition.reset_thresholds().map(map_thresholds),
    }
}

fn map_thresholds(value: &QuotaResetThresholds) -> QuotaResetThresholdsValue {
    QuotaResetThresholdsValue {
        maximum_post_reset_used_ratio: value.maximum_post_reset_used_ratio().map(map_ratio),
        minimum_post_reset_remaining_ratio: value
            .minimum_post_reset_remaining_ratio()
            .map(map_ratio),
        minimum_used_ratio_drop: value.minimum_used_ratio_drop().map(map_ratio),
    }
}

fn map_epoch(epoch: &StoreCurrentEpoch) -> QuotaEpochValue {
    QuotaEpochValue {
        epoch_definition_revision: epoch.epoch_definition_revision(),
        definition_revision: epoch.definition_revision(),
        first_sample: map_sample(epoch.first_sample()),
        first_observed_at_ms: epoch.state().first_observed_at_ms(),
        last_observed_at_ms: epoch.state().last_observed_at_ms(),
        maximum_used_ratio: epoch.maximum_used_ratio().map(map_ratio),
        maximum_used_units: epoch.maximum_used_units().map(map_units),
        provider_epoch_id: epoch
            .provider_epoch_id()
            .map(|value| Arc::from(value.as_str())),
        advertised_resets_at_ms: epoch.advertised_resets_at_ms(),
        last_transition_sequence: epoch.last_transition_sequence(),
    }
}

fn map_transition(record: &StoreTransitionRecord) -> QuotaTransitionValue {
    QuotaTransitionValue {
        definition_revision: record.definition_revision(),
        sequence: record.sequence(),
        kind: match record.kind() {
            StoreTransitionKind::ScheduledReset => QuotaTransitionKind::ScheduledReset,
            StoreTransitionKind::EarlyReset => QuotaTransitionKind::EarlyReset,
            StoreTransitionKind::ManualOrBankedReset => QuotaTransitionKind::ManualOrBankedReset,
            StoreTransitionKind::UnknownReset => QuotaTransitionKind::UnknownReset,
            StoreTransitionKind::AllowanceChanged => QuotaTransitionKind::AllowanceChanged,
        },
        pre_sample: map_sample(record.pre_sample()),
        post_sample: map_sample(record.post_sample()),
        maximum_used_ratio_before: record.maximum_used_ratio_before().map(map_ratio),
        maximum_used_units_before: record.maximum_used_units_before().map(map_units),
        old_resets_at_ms: record.old_resets_at_ms(),
        new_resets_at_ms: record.new_resets_at_ms(),
        allowance_change: record
            .allowance_change()
            .map(|change| QuotaAllowanceChangeValue {
                kind: match change.kind() {
                    StoreAllowanceKind::Increased => QuotaAllowanceChangeKind::Increased,
                    StoreAllowanceKind::Decreased => QuotaAllowanceChangeKind::Decreased,
                    StoreAllowanceKind::UnitChanged => QuotaAllowanceChangeKind::UnitChanged,
                },
                old_units: map_units(change.old_units()),
                new_units: map_units(change.new_units()),
            }),
        source: map_source(record.source()),
        confidence: map_confidence(record.confidence()),
        detection_time: match record.detection_time() {
            StoreDetectionTime::Exact(at_ms) => QuotaDetectionTime::Exact { at_ms },
            StoreDetectionTime::Interval {
                after_ms,
                at_or_before_ms,
            } => QuotaDetectionTime::Interval {
                after_ms,
                at_or_before_ms,
            },
        },
    }
}

fn map_sample(sample: &QuotaSample) -> QuotaSampleValue {
    QuotaSampleValue {
        observed_at_ms: sample.observed_at_ms(),
        fresh_until_ms: sample.fresh_until_ms(),
        stale_after_ms: sample.stale_after_ms(),
        provider_epoch_id: sample
            .provider_epoch_id()
            .map(|value| Arc::from(value.as_str())),
        used_ratio: sample.used_ratio().map(map_ratio),
        remaining_ratio: sample.remaining_ratio().map(map_ratio),
        units: sample.units().map(map_units),
        advertised_resets_at_ms: sample.advertised_resets_at_ms(),
        quality: match sample.quality() {
            StoreSampleQuality::Authoritative => QuotaSampleQuality::Authoritative,
            StoreSampleQuality::Partial => QuotaSampleQuality::Partial,
            StoreSampleQuality::Conflict => QuotaSampleQuality::Conflict,
            StoreSampleQuality::Unknown => QuotaSampleQuality::Unknown,
        },
        source: map_source(sample.source()),
        confidence: map_confidence(sample.confidence()),
        reset_evidence: match sample.reset_evidence() {
            StoreResetEvidence::None => QuotaResetEvidence::None,
            StoreResetEvidence::ExplicitProvider => QuotaResetEvidence::ExplicitProvider,
            StoreResetEvidence::ExplicitLocal => QuotaResetEvidence::ExplicitLocal,
            StoreResetEvidence::ManualOrBanked => QuotaResetEvidence::ManualOrBanked,
        },
        reset_occurred_at_ms: sample.reset_occurred_at_ms(),
    }
}

fn map_ratio(value: QuotaRatio) -> QuotaRatioValue {
    QuotaRatioValue(value.parts_per_million())
}

fn map_units(value: &QuotaUnits) -> QuotaUnitsValue {
    QuotaUnitsValue {
        unit_id: Arc::from(value.unit_id().as_str()),
        used: value.used(),
        remaining: value.remaining(),
        capacity: value.capacity(),
    }
}

const fn map_source(value: StoreEvidenceSource) -> QuotaEvidenceSource {
    match value {
        StoreEvidenceSource::ProviderLocal => QuotaEvidenceSource::ProviderLocal,
        StoreEvidenceSource::ProviderOfficial => QuotaEvidenceSource::ProviderOfficial,
        StoreEvidenceSource::LocalResetEvent => QuotaEvidenceSource::LocalResetEvent,
        StoreEvidenceSource::Manual => QuotaEvidenceSource::Manual,
        StoreEvidenceSource::Unknown => QuotaEvidenceSource::Unknown,
    }
}

const fn map_confidence(value: StoreConfidence) -> QuotaConfidence {
    match value {
        StoreConfidence::High => QuotaConfidence::High,
        StoreConfidence::Medium => QuotaConfidence::Medium,
        StoreConfidence::Low => QuotaConfidence::Low,
        StoreConfidence::Unknown => QuotaConfidence::Unknown,
    }
}

fn map_sample_freshness(
    sample: &QuotaSample,
    generated_at_ms: i64,
    warnings: &mut Vec<QuotaWarningCode>,
) -> QueryFreshness {
    if generated_at_ms < sample.observed_at_ms() {
        push_warning(warnings, QuotaWarningCode::ClockDiscontinuity);
        QueryFreshness::Unavailable
    } else if generated_at_ms <= sample.fresh_until_ms() {
        QueryFreshness::Fresh
    } else if generated_at_ms <= sample.stale_after_ms() {
        QueryFreshness::Aging
    } else {
        QueryFreshness::Stale
    }
}

const fn map_sample_quality(value: StoreSampleQuality) -> QueryQuality {
    match value {
        StoreSampleQuality::Authoritative => QueryQuality::Authoritative,
        StoreSampleQuality::Partial => QueryQuality::Partial,
        StoreSampleQuality::Conflict => QueryQuality::Conflict,
        StoreSampleQuality::Unknown => QueryQuality::Unknown,
    }
}

fn add_quality_warning(warnings: &mut Vec<QuotaWarningCode>, value: StoreSampleQuality) {
    match value {
        StoreSampleQuality::Authoritative => {}
        StoreSampleQuality::Partial => {
            push_warning(warnings, QuotaWarningCode::PartialEvidence);
        }
        StoreSampleQuality::Conflict => {
            push_warning(warnings, QuotaWarningCode::ConflictingEvidence);
        }
        StoreSampleQuality::Unknown => {
            push_warning(warnings, QuotaWarningCode::UnknownEvidence);
        }
    }
}

fn push_warning(warnings: &mut Vec<QuotaWarningCode>, warning: QuotaWarningCode) {
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
}

const fn merge_freshness(left: QueryFreshness, right: QueryFreshness) -> QueryFreshness {
    if freshness_rank(left) >= freshness_rank(right) {
        left
    } else {
        right
    }
}

const fn freshness_rank(value: QueryFreshness) -> u8 {
    match value {
        QueryFreshness::Fresh => 0,
        QueryFreshness::Aging => 1,
        QueryFreshness::Stale => 2,
        QueryFreshness::Unavailable => 3,
    }
}

const fn merge_quality(left: QueryQuality, right: QueryQuality) -> QueryQuality {
    if quality_rank(left) >= quality_rank(right) {
        left
    } else {
        right
    }
}

const fn quality_rank(value: QueryQuality) -> u8 {
    match value {
        QueryQuality::Authoritative => 0,
        QueryQuality::Derived => 1,
        QueryQuality::Estimated => 2,
        QueryQuality::Partial => 3,
        QueryQuality::Conflict => 4,
        QueryQuality::Unknown => 5,
    }
}

fn has_duplicate_windows(windows: &[QuotaWindowKey]) -> bool {
    windows
        .iter()
        .enumerate()
        .any(|(index, window)| windows[..index].contains(window))
}
