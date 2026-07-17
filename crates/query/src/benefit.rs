use std::{cmp::Ordering, fmt, sync::Arc, time::Duration};

use tokenmaster_benefits::BenefitChangeKind as StoreChangeKind;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitKind, BenefitLotId, BenefitLotObservation, BenefitScope,
    BenefitState, BenefitTarget, NotificationChannel, ReminderLeadTime,
};
use tokenmaster_store::{
    BenefitChangeCursor as StoreCursor, BenefitChangePageCapture as StoreChangePageCapture,
    BenefitChangePageQuery as StoreChangePageQuery, BenefitChangeRecord as StoreChangeRecord,
    BenefitCurrentCapture as StoreCurrentCapture, BenefitCurrentQuery as StoreCurrentQuery,
    BenefitInventoryRevision as StoreRevision, BenefitOverviewCapture as StoreOverviewCapture,
    BenefitOverviewQuery as StoreOverviewQuery,
    BenefitOverviewScopeCapture as StoreOverviewScopeCapture,
    BenefitReminderProfileSnapshot as StoreProfileSnapshot,
    BenefitScopeSnapshot as StoreScopeSnapshot, MAX_BENEFIT_OVERVIEW_LOTS,
    MAX_BENEFIT_OVERVIEW_SCOPES,
};

use crate::{
    MAX_QUERY_WARNINGS, PageSize, QueryError, QueryErrorCode, QueryFreshness, QueryQuality,
    SnapshotGeneration,
};

pub const BENEFIT_QUERY_SCHEMA_VERSION: u16 = 1;
pub const BENEFIT_OVERVIEW_QUERY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BenefitRevision(u64);

impl BenefitRevision {
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
pub struct BenefitScopeFilter {
    scope: BenefitScope,
}

impl BenefitScopeFilter {
    #[must_use]
    pub const fn new(scope: BenefitScope) -> Self {
        Self { scope }
    }

    pub(crate) const fn scope(&self) -> &BenefitScope {
        &self.scope
    }
}

impl fmt::Debug for BenefitScopeFilter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BenefitScopeFilter([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitWarningCode {
    InventoryAbsent,
    PartialInventory,
    QuantityPartialDetails,
    UnknownExpiry,
    UnknownEvidence,
    ClockDiscontinuity,
    OsScheduledUnavailable,
}

impl BenefitWarningCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InventoryAbsent => "inventory_absent",
            Self::PartialInventory => "partial_inventory",
            Self::QuantityPartialDetails => "quantity_partial_details",
            Self::UnknownExpiry => "unknown_expiry",
            Self::UnknownEvidence => "unknown_evidence",
            Self::ClockDiscontinuity => "clock_discontinuity",
            Self::OsScheduledUnavailable => "os_scheduled_unavailable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitQueryHeaderParts {
    pub snapshot_generation: SnapshotGeneration,
    pub benefit_revision: BenefitRevision,
    pub generated_at_ms: i64,
    pub data_through_ms: Option<i64>,
    pub freshness: QueryFreshness,
    pub quality: QueryQuality,
    pub filter: BenefitScopeFilter,
    pub warnings: Vec<BenefitWarningCode>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitQueryHeader {
    snapshot_generation: SnapshotGeneration,
    benefit_revision: BenefitRevision,
    generated_at_ms: i64,
    data_through_ms: Option<i64>,
    freshness: QueryFreshness,
    quality: QueryQuality,
    filter: BenefitScopeFilter,
    warnings: Arc<[BenefitWarningCode]>,
}

impl BenefitQueryHeader {
    pub fn new(parts: BenefitQueryHeaderParts) -> Result<Self, QueryError> {
        if parts.warnings.len() > MAX_QUERY_WARNINGS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        Ok(Self {
            snapshot_generation: parts.snapshot_generation,
            benefit_revision: parts.benefit_revision,
            generated_at_ms: parts.generated_at_ms,
            data_through_ms: parts.data_through_ms,
            freshness: parts.freshness,
            quality: parts.quality,
            filter: parts.filter,
            warnings: Arc::from(parts.warnings),
        })
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        BENEFIT_QUERY_SCHEMA_VERSION
    }

    #[must_use]
    pub const fn snapshot_generation(&self) -> SnapshotGeneration {
        self.snapshot_generation
    }

    #[must_use]
    pub const fn benefit_revision(&self) -> BenefitRevision {
        self.benefit_revision
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
    pub const fn filter(&self) -> &BenefitScopeFilter {
        &self.filter
    }

    #[must_use]
    pub const fn warnings(&self) -> &Arc<[BenefitWarningCode]> {
        &self.warnings
    }
}

impl fmt::Debug for BenefitQueryHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitQueryHeader")
            .field("schema_version", &self.schema_version())
            .field("snapshot_generation", &self.snapshot_generation)
            .field("benefit_revision", &self.benefit_revision)
            .field("generated_at_ms", &self.generated_at_ms)
            .field("data_through_ms", &self.data_through_ms)
            .field("freshness", &self.freshness)
            .field("quality", &self.quality)
            .field("filter", &"[redacted]")
            .field("warnings", &self.warnings)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitEnvelope<T> {
    header: BenefitQueryHeader,
    payload: T,
}

impl<T> BenefitEnvelope<T> {
    #[must_use]
    pub const fn new(header: BenefitQueryHeader, payload: T) -> Self {
        Self { header, payload }
    }

    #[must_use]
    pub const fn header(&self) -> &BenefitQueryHeader {
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
    pub fn into_parts(self) -> (BenefitQueryHeader, T) {
        (self.header, self.payload)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitOverviewQueryHeaderParts {
    pub snapshot_generation: SnapshotGeneration,
    pub benefit_revision: BenefitRevision,
    pub generated_at_ms: i64,
    pub data_through_ms: Option<i64>,
    pub freshness: QueryFreshness,
    pub quality: QueryQuality,
    pub warnings: Vec<BenefitWarningCode>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitOverviewQueryHeader {
    snapshot_generation: SnapshotGeneration,
    benefit_revision: BenefitRevision,
    generated_at_ms: i64,
    data_through_ms: Option<i64>,
    freshness: QueryFreshness,
    quality: QueryQuality,
    warnings: Arc<[BenefitWarningCode]>,
}

impl BenefitOverviewQueryHeader {
    pub fn new(parts: BenefitOverviewQueryHeaderParts) -> Result<Self, QueryError> {
        if parts.warnings.len() > MAX_QUERY_WARNINGS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        Ok(Self {
            snapshot_generation: parts.snapshot_generation,
            benefit_revision: parts.benefit_revision,
            generated_at_ms: parts.generated_at_ms,
            data_through_ms: parts.data_through_ms,
            freshness: parts.freshness,
            quality: parts.quality,
            warnings: Arc::from(parts.warnings),
        })
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        BENEFIT_OVERVIEW_QUERY_SCHEMA_VERSION
    }

    #[must_use]
    pub const fn snapshot_generation(&self) -> SnapshotGeneration {
        self.snapshot_generation
    }

    #[must_use]
    pub const fn benefit_revision(&self) -> BenefitRevision {
        self.benefit_revision
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
    pub const fn warnings(&self) -> &Arc<[BenefitWarningCode]> {
        &self.warnings
    }
}

impl fmt::Debug for BenefitOverviewQueryHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitOverviewQueryHeader")
            .field("schema_version", &self.schema_version())
            .field("snapshot_generation", &self.snapshot_generation)
            .field("benefit_revision", &self.benefit_revision)
            .field("generated_at_ms", &self.generated_at_ms)
            .field("data_through_ms", &self.data_through_ms)
            .field("freshness", &self.freshness)
            .field("quality", &self.quality)
            .field("warnings", &self.warnings)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitOverviewEnvelope<T> {
    header: BenefitOverviewQueryHeader,
    payload: T,
}

impl<T> BenefitOverviewEnvelope<T> {
    #[must_use]
    pub const fn new(header: BenefitOverviewQueryHeader, payload: T) -> Self {
        Self { header, payload }
    }

    #[must_use]
    pub const fn header(&self) -> &BenefitOverviewQueryHeader {
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
    pub fn into_parts(self) -> (BenefitOverviewQueryHeader, T) {
        (self.header, self.payload)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitCurrentRequest {
    filter: BenefitScopeFilter,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BenefitOverviewRequest;

impl BenefitOverviewRequest {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl BenefitCurrentRequest {
    #[must_use]
    pub const fn new(scope: BenefitScope) -> Self {
        Self {
            filter: BenefitScopeFilter::new(scope),
        }
    }
}

impl fmt::Debug for BenefitCurrentRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BenefitCurrentRequest([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitChangeCursor {
    benefit_revision: BenefitRevision,
    store_revision: StoreRevision,
    filter: BenefitScopeFilter,
    inner: StoreCursor,
}

impl BenefitChangeCursor {
    #[must_use]
    pub const fn benefit_revision(&self) -> BenefitRevision {
        self.benefit_revision
    }
}

impl fmt::Debug for BenefitChangeCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitChangeCursor")
            .field("benefit_revision", &self.benefit_revision)
            .field("filter", &"[redacted]")
            .field("identity", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitChangePageRequest {
    filter: BenefitScopeFilter,
    page_size: PageSize,
    continuation: Option<BenefitChangeCursor>,
}

impl BenefitChangePageRequest {
    pub fn first(scope: BenefitScope, page_size: PageSize) -> Result<Self, QueryError> {
        Self::new(scope, page_size, None)
    }

    pub fn continuation(
        scope: BenefitScope,
        page_size: PageSize,
        cursor: BenefitChangeCursor,
    ) -> Result<Self, QueryError> {
        Self::new(scope, page_size, Some(cursor))
    }

    fn new(
        scope: BenefitScope,
        page_size: PageSize,
        continuation: Option<BenefitChangeCursor>,
    ) -> Result<Self, QueryError> {
        let filter = BenefitScopeFilter::new(scope);
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
}

impl fmt::Debug for BenefitChangePageRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitChangePageRequest")
            .field("filter", &"[redacted]")
            .field("page_size", &self.page_size)
            .field("continuation", &self.continuation)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitReminderProfileSource {
    Inherited,
    Override,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitReminderCoverage {
    Disabled,
    InAppOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitReminderProfileValue {
    revision: u64,
    lead_times: Arc<[ReminderLeadTime]>,
    configured_channels: Arc<[NotificationChannel]>,
    source: BenefitReminderProfileSource,
    coverage: BenefitReminderCoverage,
}

impl BenefitReminderProfileValue {
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn lead_times(&self) -> &Arc<[ReminderLeadTime]> {
        &self.lead_times
    }

    #[must_use]
    pub const fn configured_channels(&self) -> &Arc<[NotificationChannel]> {
        &self.configured_channels
    }

    #[must_use]
    pub const fn source(&self) -> BenefitReminderProfileSource {
        self.source
    }

    #[must_use]
    pub const fn coverage(&self) -> BenefitReminderCoverage {
        self.coverage
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitLotValue {
    opaque_id: BenefitLotId,
    revision: u64,
    kind: BenefitKind,
    quantity: u64,
    state: BenefitState,
    target: BenefitTarget,
    granted_at_ms: Option<i64>,
    expiry: BenefitExpiry,
    source: BenefitEvidenceSource,
    confidence: BenefitConfidence,
    detail_kind: BenefitDetailKind,
    label_key: Arc<str>,
}

impl BenefitLotValue {
    #[must_use]
    pub const fn opaque_id(&self) -> BenefitLotId {
        self.opaque_id
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
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
        &self.label_key
    }
}

impl fmt::Debug for BenefitLotValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitLotValue")
            .field("opaque_id", &"[redacted]")
            .field("revision", &self.revision)
            .field("kind", &self.kind)
            .field("quantity", &self.quantity)
            .field("state", &self.state)
            .field("target", &"[redacted]")
            .field("granted_at_ms", &self.granted_at_ms)
            .field("expiry", &self.expiry)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .field("detail_kind", &self.detail_kind)
            .field("label_key", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitInventoryValue {
    inventory_revision: u64,
    last_change_sequence: u64,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    completeness: BenefitInventoryCompleteness,
    current_lots: Arc<[BenefitLotValue]>,
    nearest_expiry_at_ms: Option<i64>,
    nearest_due_at_ms: Option<i64>,
    reminder_profile: BenefitReminderProfileValue,
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitOverviewLotValue {
    kind: BenefitKind,
    quantity: u64,
    state: BenefitState,
    granted_at_ms: Option<i64>,
    expiry: BenefitExpiry,
    source: BenefitEvidenceSource,
    confidence: BenefitConfidence,
    detail_kind: BenefitDetailKind,
    label_key: Arc<str>,
}

impl BenefitOverviewLotValue {
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
        &self.label_key
    }
}

impl fmt::Debug for BenefitOverviewLotValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitOverviewLotValue")
            .field("kind", &self.kind)
            .field("quantity", &self.quantity)
            .field("state", &self.state)
            .field("granted_at_ms", &self.granted_at_ms)
            .field("expiry", &self.expiry)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .field("detail_kind", &self.detail_kind)
            .field("label_key", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitOverviewScopeValue {
    inventory_revision: u64,
    last_change_sequence: u64,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    completeness: BenefitInventoryCompleteness,
    current_lots: Arc<[BenefitOverviewLotValue]>,
    nearest_expiry_at_ms: Option<i64>,
    nearest_due_at_ms: Option<i64>,
    reminder_profile: BenefitReminderProfileValue,
    freshness: QueryFreshness,
    quality: QueryQuality,
    warnings: Arc<[BenefitWarningCode]>,
}

impl BenefitOverviewScopeValue {
    #[must_use]
    pub const fn inventory_revision(&self) -> u64 {
        self.inventory_revision
    }

    #[must_use]
    pub const fn last_change_sequence(&self) -> u64 {
        self.last_change_sequence
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
    pub const fn current_lots(&self) -> &Arc<[BenefitOverviewLotValue]> {
        &self.current_lots
    }

    #[must_use]
    pub const fn nearest_expiry_at_ms(&self) -> Option<i64> {
        self.nearest_expiry_at_ms
    }

    #[must_use]
    pub const fn nearest_due_at_ms(&self) -> Option<i64> {
        self.nearest_due_at_ms
    }

    #[must_use]
    pub const fn reminder_profile(&self) -> &BenefitReminderProfileValue {
        &self.reminder_profile
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
    pub const fn warnings(&self) -> &Arc<[BenefitWarningCode]> {
        &self.warnings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitOverviewSnapshot {
    scopes: Arc<[BenefitOverviewScopeValue]>,
}

impl BenefitOverviewSnapshot {
    #[must_use]
    pub const fn scopes(&self) -> &Arc<[BenefitOverviewScopeValue]> {
        &self.scopes
    }
}

impl BenefitInventoryValue {
    #[must_use]
    pub const fn inventory_revision(&self) -> u64 {
        self.inventory_revision
    }

    #[must_use]
    pub const fn last_change_sequence(&self) -> u64 {
        self.last_change_sequence
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
    pub const fn current_lots(&self) -> &Arc<[BenefitLotValue]> {
        &self.current_lots
    }

    #[must_use]
    pub const fn nearest_expiry_at_ms(&self) -> Option<i64> {
        self.nearest_expiry_at_ms
    }

    #[must_use]
    pub const fn nearest_due_at_ms(&self) -> Option<i64> {
        self.nearest_due_at_ms
    }

    #[must_use]
    pub const fn reminder_profile(&self) -> &BenefitReminderProfileValue {
        &self.reminder_profile
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitCurrentSnapshot {
    inventory: Option<BenefitInventoryValue>,
    reminder_profile: BenefitReminderProfileValue,
}

impl BenefitCurrentSnapshot {
    #[must_use]
    pub const fn inventory(&self) -> Option<&BenefitInventoryValue> {
        self.inventory.as_ref()
    }

    #[must_use]
    pub const fn reminder_profile(&self) -> &BenefitReminderProfileValue {
        &self.reminder_profile
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitChangeKind {
    Awarded,
    QuantityChanged,
    StateChanged,
    ExpiryChanged,
    Corrected,
    DisappearedAmbiguous,
    Reappeared,
    RetiredTerminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitChangeValue {
    sequence: u64,
    lot_revision: u64,
    kind: BenefitChangeKind,
    before: Option<BenefitLotValue>,
    after: Option<BenefitLotValue>,
    observed_at_ms: i64,
}

impl BenefitChangeValue {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn lot_revision(&self) -> u64 {
        self.lot_revision
    }

    #[must_use]
    pub const fn kind(&self) -> BenefitChangeKind {
        self.kind
    }

    #[must_use]
    pub const fn before(&self) -> Option<&BenefitLotValue> {
        self.before.as_ref()
    }

    #[must_use]
    pub const fn after(&self) -> Option<&BenefitLotValue> {
        self.after.as_ref()
    }

    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitChangePage {
    changes: Arc<[BenefitChangeValue]>,
    next_cursor: Option<BenefitChangeCursor>,
    has_more: bool,
}

impl BenefitChangePage {
    #[must_use]
    pub const fn changes(&self) -> &Arc<[BenefitChangeValue]> {
        &self.changes
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&BenefitChangeCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

pub(crate) struct MappedBenefitPayload<T> {
    pub payload: T,
    pub benefit_revision: BenefitRevision,
    pub data_through_ms: Option<i64>,
    pub freshness: QueryFreshness,
    pub quality: QueryQuality,
    pub filter: BenefitScopeFilter,
    pub warnings: Vec<BenefitWarningCode>,
}

pub(crate) struct MappedBenefitOverviewPayload<T> {
    pub payload: T,
    pub benefit_revision: BenefitRevision,
    pub data_through_ms: Option<i64>,
    pub freshness: QueryFreshness,
    pub quality: QueryQuality,
    pub warnings: Vec<BenefitWarningCode>,
}

pub(crate) fn build_current_query(
    request: &BenefitCurrentRequest,
    deadline: Duration,
) -> Result<StoreCurrentQuery, QueryError> {
    StoreCurrentQuery::new(request.filter.scope().clone(), deadline)
        .map_err(crate::service::map_store_error)
}

pub(crate) fn build_overview_query(
    _request: BenefitOverviewRequest,
    deadline: Duration,
) -> Result<StoreOverviewQuery, QueryError> {
    StoreOverviewQuery::new(deadline).map_err(crate::service::map_store_error)
}

pub(crate) fn build_change_query(
    request: &BenefitChangePageRequest,
    deadline: Duration,
) -> Result<StoreChangePageQuery, QueryError> {
    StoreChangePageQuery::new(
        request.filter.scope().clone(),
        request
            .continuation
            .as_ref()
            .map(|cursor| cursor.store_revision),
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
    request: &BenefitCurrentRequest,
    generated_at_ms: i64,
) -> Result<MappedBenefitPayload<BenefitCurrentSnapshot>, QueryError> {
    let benefit_revision = map_revision(capture.benefit_revision())?;
    let mut warnings = Vec::new();
    let profile = map_profile(capture, &mut warnings);
    let inventory = match capture.scope() {
        None => {
            if !capture.lots().is_empty() || capture.nearest_due().is_some() {
                return Err(corrupt());
            }
            push_warning(&mut warnings, BenefitWarningCode::InventoryAbsent);
            None
        }
        Some(scope) => {
            if capture.lots().len() > tokenmaster_domain::MAX_BENEFIT_LOTS_PER_OBSERVATION
                || !current_lots_are_ordered(capture.lots())
            {
                return Err(corrupt());
            }
            let lots = capture
                .lots()
                .iter()
                .map(map_current_lot)
                .collect::<Vec<_>>();
            for lot in &lots {
                if lot.expiry.conservative_utc_ms().is_none() {
                    push_warning(&mut warnings, BenefitWarningCode::UnknownExpiry);
                }
                if lot.confidence == BenefitConfidence::Unknown
                    || lot.source == BenefitEvidenceSource::Unknown
                {
                    push_warning(&mut warnings, BenefitWarningCode::UnknownEvidence);
                }
            }
            add_completeness_warning(scope.completeness(), &mut warnings);
            Some(BenefitInventoryValue {
                inventory_revision: scope.inventory_revision().get(),
                last_change_sequence: scope.last_change_sequence(),
                observed_at_ms: scope.observed_at_ms(),
                fresh_until_ms: scope.fresh_until_ms(),
                stale_after_ms: scope.stale_after_ms(),
                completeness: scope.completeness(),
                nearest_expiry_at_ms: lots
                    .iter()
                    .filter(|lot| lot.state == BenefitState::Available)
                    .filter_map(|lot| lot.expiry.conservative_utc_ms())
                    .min(),
                nearest_due_at_ms: capture.nearest_due().map(|due| due.due_at_ms()),
                current_lots: Arc::from(lots),
                reminder_profile: profile.clone(),
            })
        }
    };
    let (data_through_ms, freshness, quality) =
        map_scope_status(capture.scope(), generated_at_ms, &mut warnings);
    Ok(MappedBenefitPayload {
        payload: BenefitCurrentSnapshot {
            inventory,
            reminder_profile: profile,
        },
        benefit_revision,
        data_through_ms,
        freshness,
        quality,
        filter: request.filter.clone(),
        warnings,
    })
}

pub(crate) fn map_overview_capture(
    capture: &StoreOverviewCapture,
    generated_at_ms: i64,
) -> Result<MappedBenefitOverviewPayload<BenefitOverviewSnapshot>, QueryError> {
    if capture.scopes().len() > MAX_BENEFIT_OVERVIEW_SCOPES {
        return Err(corrupt());
    }
    let total_lots = capture.scopes().iter().try_fold(0_usize, |total, scope| {
        total.checked_add(scope.lots().len()).ok_or_else(corrupt)
    })?;
    if total_lots > MAX_BENEFIT_OVERVIEW_LOTS {
        return Err(corrupt());
    }

    let benefit_revision = map_revision(capture.benefit_revision())?;
    let mut warnings = Vec::new();
    let mut scopes = Vec::with_capacity(capture.scopes().len());
    let mut data_through_ms: Option<i64> = None;
    let mut freshness = if capture.scopes().is_empty() {
        QueryFreshness::Unavailable
    } else {
        QueryFreshness::Fresh
    };
    let mut quality = if capture.scopes().is_empty() {
        QueryQuality::Unknown
    } else {
        QueryQuality::Authoritative
    };
    for scope in capture.scopes() {
        let mapped = map_overview_scope(scope, generated_at_ms)?;
        data_through_ms = Some(match data_through_ms {
            Some(current) => current.min(mapped.observed_at_ms),
            None => mapped.observed_at_ms,
        });
        freshness = merge_freshness(freshness, mapped.freshness);
        quality = merge_quality(quality, mapped.quality);
        for warning in mapped.warnings.iter().copied() {
            push_warning(&mut warnings, warning);
        }
        scopes.push(mapped);
    }
    if scopes.is_empty() {
        push_warning(&mut warnings, BenefitWarningCode::InventoryAbsent);
    }
    Ok(MappedBenefitOverviewPayload {
        payload: BenefitOverviewSnapshot {
            scopes: Arc::from(scopes),
        },
        benefit_revision,
        data_through_ms,
        freshness,
        quality,
        warnings,
    })
}

pub(crate) fn map_change_capture(
    capture: &StoreChangePageCapture,
    request: &BenefitChangePageRequest,
    generated_at_ms: i64,
) -> Result<MappedBenefitPayload<BenefitChangePage>, QueryError> {
    if capture.changes().len() > request.page_size.get()
        || capture
            .changes()
            .windows(2)
            .any(|pair| pair[0].sequence() <= pair[1].sequence())
    {
        return Err(corrupt());
    }
    let benefit_revision = map_revision(capture.benefit_revision())?;
    let changes = capture.changes().iter().map(map_change).collect::<Vec<_>>();
    let next_cursor = capture.next_cursor().map(|inner| BenefitChangeCursor {
        benefit_revision,
        store_revision: capture.benefit_revision(),
        filter: request.filter.clone(),
        inner: inner.clone(),
    });
    if capture.has_more() != next_cursor.is_some()
        || (capture.has_more() && changes.len() != request.page_size.get())
    {
        return Err(corrupt());
    }
    let mut warnings = Vec::new();
    if capture.scope().is_none() {
        push_warning(&mut warnings, BenefitWarningCode::InventoryAbsent);
    }
    if let Some(scope) = capture.scope() {
        add_completeness_warning(scope.completeness(), &mut warnings);
    }
    let (data_through_ms, freshness, quality) =
        map_scope_status(capture.scope(), generated_at_ms, &mut warnings);
    Ok(MappedBenefitPayload {
        payload: BenefitChangePage {
            changes: Arc::from(changes),
            next_cursor,
            has_more: capture.has_more(),
        },
        benefit_revision,
        data_through_ms,
        freshness,
        quality,
        filter: request.filter.clone(),
        warnings,
    })
}

fn map_overview_scope(
    capture: &StoreOverviewScopeCapture,
    generated_at_ms: i64,
) -> Result<BenefitOverviewScopeValue, QueryError> {
    if capture.lots().len() > tokenmaster_domain::MAX_BENEFIT_LOTS_PER_OBSERVATION
        || !current_lots_are_ordered(capture.lots())
    {
        return Err(corrupt());
    }
    let mut warnings = Vec::new();
    let reminder_profile = map_profile_snapshot(capture.reminder_profile(), &mut warnings);
    let lots = capture
        .lots()
        .iter()
        .map(map_overview_lot)
        .collect::<Vec<_>>();
    for lot in &lots {
        if lot.expiry.conservative_utc_ms().is_none() {
            push_warning(&mut warnings, BenefitWarningCode::UnknownExpiry);
        }
        if lot.confidence == BenefitConfidence::Unknown
            || lot.source == BenefitEvidenceSource::Unknown
        {
            push_warning(&mut warnings, BenefitWarningCode::UnknownEvidence);
        }
    }
    add_completeness_warning(capture.scope().completeness(), &mut warnings);
    let (_data_through_ms, freshness, quality) =
        map_scope_status(Some(capture.scope()), generated_at_ms, &mut warnings);
    Ok(BenefitOverviewScopeValue {
        inventory_revision: capture.scope().inventory_revision().get(),
        last_change_sequence: capture.scope().last_change_sequence(),
        observed_at_ms: capture.scope().observed_at_ms(),
        fresh_until_ms: capture.scope().fresh_until_ms(),
        stale_after_ms: capture.scope().stale_after_ms(),
        completeness: capture.scope().completeness(),
        nearest_expiry_at_ms: lots
            .iter()
            .filter(|lot| lot.state == BenefitState::Available)
            .filter_map(|lot| lot.expiry.conservative_utc_ms())
            .min(),
        nearest_due_at_ms: capture.nearest_due().map(|due| due.due_at_ms()),
        current_lots: Arc::from(lots),
        reminder_profile,
        freshness,
        quality,
        warnings: Arc::from(warnings),
    })
}

fn map_revision(revision: StoreRevision) -> Result<BenefitRevision, QueryError> {
    BenefitRevision::new(revision.get()).map_err(|_error| corrupt())
}

fn map_profile(
    capture: &StoreCurrentCapture,
    warnings: &mut Vec<BenefitWarningCode>,
) -> BenefitReminderProfileValue {
    map_profile_snapshot(capture.reminder_profile(), warnings)
}

fn map_profile_snapshot(
    snapshot: &StoreProfileSnapshot,
    warnings: &mut Vec<BenefitWarningCode>,
) -> BenefitReminderProfileValue {
    let profile = snapshot.profile();
    let has_in_app = profile.channels().contains(&NotificationChannel::InApp);
    if profile
        .channels()
        .contains(&NotificationChannel::OsScheduled)
    {
        push_warning(warnings, BenefitWarningCode::OsScheduledUnavailable);
    }
    BenefitReminderProfileValue {
        revision: profile.revision().get(),
        lead_times: Arc::from(profile.lead_times()),
        configured_channels: Arc::from(profile.channels()),
        source: if snapshot.inherited() {
            BenefitReminderProfileSource::Inherited
        } else {
            BenefitReminderProfileSource::Override
        },
        coverage: if has_in_app {
            BenefitReminderCoverage::InAppOnly
        } else {
            BenefitReminderCoverage::Disabled
        },
    }
}

fn map_scope_status(
    scope: Option<&StoreScopeSnapshot>,
    generated_at_ms: i64,
    warnings: &mut Vec<BenefitWarningCode>,
) -> (Option<i64>, QueryFreshness, QueryQuality) {
    let Some(scope) = scope else {
        return (None, QueryFreshness::Unavailable, QueryQuality::Unknown);
    };
    let freshness = if generated_at_ms < scope.observed_at_ms() {
        push_warning(warnings, BenefitWarningCode::ClockDiscontinuity);
        QueryFreshness::Aging
    } else if generated_at_ms <= scope.fresh_until_ms() {
        QueryFreshness::Fresh
    } else if generated_at_ms <= scope.stale_after_ms() {
        QueryFreshness::Aging
    } else {
        QueryFreshness::Stale
    };
    let quality = match scope.completeness() {
        BenefitInventoryCompleteness::Complete => QueryQuality::Authoritative,
        BenefitInventoryCompleteness::CompleteQuantityPartialDetails
        | BenefitInventoryCompleteness::Partial => QueryQuality::Partial,
    };
    (Some(scope.observed_at_ms()), freshness, quality)
}

fn add_completeness_warning(
    completeness: BenefitInventoryCompleteness,
    warnings: &mut Vec<BenefitWarningCode>,
) {
    match completeness {
        BenefitInventoryCompleteness::Complete => {}
        BenefitInventoryCompleteness::CompleteQuantityPartialDetails => {
            push_warning(warnings, BenefitWarningCode::QuantityPartialDetails);
        }
        BenefitInventoryCompleteness::Partial => {
            push_warning(warnings, BenefitWarningCode::PartialInventory);
        }
    }
}

fn map_current_lot(value: &tokenmaster_benefits::BenefitCurrentLot) -> BenefitLotValue {
    map_lot(value.lot(), value.revision().get())
}

fn map_overview_lot(value: &tokenmaster_benefits::BenefitCurrentLot) -> BenefitOverviewLotValue {
    let lot = value.lot();
    BenefitOverviewLotValue {
        kind: lot.kind(),
        quantity: lot.quantity(),
        state: lot.state(),
        granted_at_ms: lot.granted_at_ms(),
        expiry: lot.expiry().clone(),
        source: lot.source(),
        confidence: lot.confidence(),
        detail_kind: lot.detail_kind(),
        label_key: Arc::from(lot.label_key()),
    }
}

fn map_lot(value: &BenefitLotObservation, revision: u64) -> BenefitLotValue {
    BenefitLotValue {
        opaque_id: value.lot_id(),
        revision,
        kind: value.kind(),
        quantity: value.quantity(),
        state: value.state(),
        target: value.target().clone(),
        granted_at_ms: value.granted_at_ms(),
        expiry: value.expiry().clone(),
        source: value.source(),
        confidence: value.confidence(),
        detail_kind: value.detail_kind(),
        label_key: Arc::from(value.label_key()),
    }
}

fn map_change(value: &StoreChangeRecord) -> BenefitChangeValue {
    BenefitChangeValue {
        sequence: value.sequence(),
        lot_revision: value.lot_revision().get(),
        kind: map_change_kind(value.kind()),
        before: value
            .before()
            .zip(value.before_revision())
            .map(|(lot, revision)| map_lot(lot, revision.get())),
        after: value
            .after()
            .zip(value.after_revision())
            .map(|(lot, revision)| map_lot(lot, revision.get())),
        observed_at_ms: value.observed_at_ms(),
    }
}

const fn map_change_kind(value: StoreChangeKind) -> BenefitChangeKind {
    match value {
        StoreChangeKind::Awarded => BenefitChangeKind::Awarded,
        StoreChangeKind::QuantityChanged => BenefitChangeKind::QuantityChanged,
        StoreChangeKind::StateChanged => BenefitChangeKind::StateChanged,
        StoreChangeKind::ExpiryChanged => BenefitChangeKind::ExpiryChanged,
        StoreChangeKind::Corrected => BenefitChangeKind::Corrected,
        StoreChangeKind::DisappearedAmbiguous => BenefitChangeKind::DisappearedAmbiguous,
        StoreChangeKind::Reappeared => BenefitChangeKind::Reappeared,
        StoreChangeKind::RetiredTerminal => BenefitChangeKind::RetiredTerminal,
    }
}

fn current_lots_are_ordered(values: &[tokenmaster_benefits::BenefitCurrentLot]) -> bool {
    values
        .windows(2)
        .all(|pair| compare_current_lots(&pair[0], &pair[1]) != Ordering::Greater)
}

fn compare_current_lots(
    left: &tokenmaster_benefits::BenefitCurrentLot,
    right: &tokenmaster_benefits::BenefitCurrentLot,
) -> Ordering {
    left.lot()
        .expiry()
        .conservative_utc_ms()
        .is_none()
        .cmp(&right.lot().expiry().conservative_utc_ms().is_none())
        .then_with(|| {
            left.lot()
                .expiry()
                .conservative_utc_ms()
                .cmp(&right.lot().expiry().conservative_utc_ms())
        })
        .then_with(|| {
            benefit_kind_code(left.lot().kind()).cmp(&benefit_kind_code(right.lot().kind()))
        })
        .then_with(|| {
            left.lot()
                .lot_id()
                .as_bytes()
                .cmp(right.lot().lot_id().as_bytes())
        })
}

const fn benefit_kind_code(value: BenefitKind) -> u8 {
    match value {
        BenefitKind::BankedRateLimitReset => 1,
        BenefitKind::UsageCredit => 2,
        BenefitKind::TemporaryUsage => 3,
        BenefitKind::Unknown => 4,
    }
}

fn push_warning(warnings: &mut Vec<BenefitWarningCode>, warning: BenefitWarningCode) {
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

const fn corrupt() -> QueryError {
    QueryError::new(QueryErrorCode::CorruptArchive)
}
