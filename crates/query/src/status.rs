use std::sync::Arc;

use tokenmaster_store::{ProductAggregateState as StoreAggregateState, ProductDataStatusCapture};

use crate::{
    BenefitRevision, DatasetIdentity, GitPublicationRevision, PublicationGeneration, QueryError,
    QueryFreshness, QueryQuality, QueryWarningCode, QuotaRevision, SnapshotGeneration,
};

pub const PRODUCT_DATA_STATUS_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductComponentState {
    Empty,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductAggregateState {
    Ready,
    RebuildRequired,
    Rebuilding,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductAggregateProgress {
    processed_events: u64,
    total_events: u64,
}

impl ProductAggregateProgress {
    #[must_use]
    pub const fn processed_events(self) -> u64 {
        self.processed_events
    }

    #[must_use]
    pub const fn total_events(self) -> u64 {
        self.total_events
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductAggregateStatus {
    state: ProductAggregateState,
    expected_dataset_generation: u64,
    active_generation: u64,
    current_event_count: u64,
    legacy_event_count: u64,
    progress: Option<ProductAggregateProgress>,
}

impl ProductAggregateStatus {
    #[must_use]
    pub const fn state(self) -> ProductAggregateState {
        self.state
    }

    #[must_use]
    pub const fn expected_dataset_generation(self) -> u64 {
        self.expected_dataset_generation
    }

    #[must_use]
    pub const fn active_generation(self) -> u64 {
        self.active_generation
    }

    #[must_use]
    pub const fn current_event_count(self) -> u64 {
        self.current_event_count
    }

    #[must_use]
    pub const fn legacy_event_count(self) -> u64 {
        self.legacy_event_count
    }

    #[must_use]
    pub const fn progress(self) -> Option<ProductAggregateProgress> {
        self.progress
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductDataWarningCode {
    LegacyUnverified,
    Partial,
    RecoveryPending,
    ClockDiscontinuity,
    AccountingVersionStale,
    ReplayStaging,
    AggregateRebuildRequired,
    AggregateRebuilding,
    AggregateFailed,
}

impl ProductDataWarningCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::LegacyUnverified => "legacy_unverified",
            Self::Partial => "partial",
            Self::RecoveryPending => "recovery_pending",
            Self::ClockDiscontinuity => "clock_discontinuity",
            Self::AccountingVersionStale => "accounting_version_stale",
            Self::ReplayStaging => "replay_staging",
            Self::AggregateRebuildRequired => "aggregate_rebuild_required",
            Self::AggregateRebuilding => "aggregate_rebuilding",
            Self::AggregateFailed => "aggregate_failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductUsageDataStatus {
    publication_generation: PublicationGeneration,
    dataset_identity: DatasetIdentity,
    data_through_ms: Option<i64>,
    freshness: QueryFreshness,
    quality: QueryQuality,
    scope_count: usize,
    aggregate: ProductAggregateStatus,
    warnings: Arc<[ProductDataWarningCode]>,
}

impl ProductUsageDataStatus {
    #[must_use]
    pub const fn publication_generation(&self) -> PublicationGeneration {
        self.publication_generation
    }

    #[must_use]
    pub const fn dataset_identity(&self) -> DatasetIdentity {
        self.dataset_identity
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
    pub const fn scope_count(&self) -> usize {
        self.scope_count
    }

    #[must_use]
    pub const fn aggregate(&self) -> ProductAggregateStatus {
        self.aggregate
    }

    #[must_use]
    pub const fn warnings(&self) -> &Arc<[ProductDataWarningCode]> {
        &self.warnings
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductQuotaDataStatus {
    state: ProductComponentState,
    revision: QuotaRevision,
    retained_sample_count: u64,
    retained_epoch_count: u64,
    retained_transition_count: u64,
    last_published_at_ms: Option<i64>,
}

impl ProductQuotaDataStatus {
    #[must_use]
    pub const fn state(self) -> ProductComponentState {
        self.state
    }

    #[must_use]
    pub const fn revision(self) -> QuotaRevision {
        self.revision
    }

    #[must_use]
    pub const fn retained_sample_count(self) -> u64 {
        self.retained_sample_count
    }

    #[must_use]
    pub const fn retained_epoch_count(self) -> u64 {
        self.retained_epoch_count
    }

    #[must_use]
    pub const fn retained_transition_count(self) -> u64 {
        self.retained_transition_count
    }

    #[must_use]
    pub const fn last_published_at_ms(self) -> Option<i64> {
        self.last_published_at_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductBenefitDataStatus {
    state: ProductComponentState,
    revision: BenefitRevision,
    current_lot_count: u64,
    retained_change_count: u64,
    pending_due_count: u64,
    retained_delivery_count: u64,
    last_published_at_ms: Option<i64>,
}

impl ProductBenefitDataStatus {
    #[must_use]
    pub const fn state(self) -> ProductComponentState {
        self.state
    }

    #[must_use]
    pub const fn revision(self) -> BenefitRevision {
        self.revision
    }

    #[must_use]
    pub const fn current_lot_count(self) -> u64 {
        self.current_lot_count
    }

    #[must_use]
    pub const fn retained_change_count(self) -> u64 {
        self.retained_change_count
    }

    #[must_use]
    pub const fn pending_due_count(self) -> u64 {
        self.pending_due_count
    }

    #[must_use]
    pub const fn retained_delivery_count(self) -> u64 {
        self.retained_delivery_count
    }

    #[must_use]
    pub const fn last_published_at_ms(self) -> Option<i64> {
        self.last_published_at_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductGitDataStatus {
    state: ProductComponentState,
    revision: GitPublicationRevision,
    repository_count: u64,
    association_count: u64,
    last_published_at_ms: Option<i64>,
}

impl ProductGitDataStatus {
    #[must_use]
    pub const fn state(self) -> ProductComponentState {
        self.state
    }

    #[must_use]
    pub const fn revision(self) -> GitPublicationRevision {
        self.revision
    }

    #[must_use]
    pub const fn repository_count(self) -> u64 {
        self.repository_count
    }

    #[must_use]
    pub const fn association_count(self) -> u64 {
        self.association_count
    }

    #[must_use]
    pub const fn last_published_at_ms(self) -> Option<i64> {
        self.last_published_at_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductDataStatusSnapshot {
    usage: ProductUsageDataStatus,
    quota: ProductQuotaDataStatus,
    benefit: ProductBenefitDataStatus,
    git: ProductGitDataStatus,
}

impl ProductDataStatusSnapshot {
    #[must_use]
    pub const fn usage(&self) -> &ProductUsageDataStatus {
        &self.usage
    }

    #[must_use]
    pub const fn quota(&self) -> ProductQuotaDataStatus {
        self.quota
    }

    #[must_use]
    pub const fn benefit(&self) -> ProductBenefitDataStatus {
        self.benefit
    }

    #[must_use]
    pub const fn git(&self) -> ProductGitDataStatus {
        self.git
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductDataStatusEnvelope {
    snapshot_generation: SnapshotGeneration,
    generated_at_ms: i64,
    payload: ProductDataStatusSnapshot,
}

impl ProductDataStatusEnvelope {
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        PRODUCT_DATA_STATUS_SCHEMA_VERSION
    }

    #[must_use]
    pub const fn snapshot_generation(&self) -> SnapshotGeneration {
        self.snapshot_generation
    }

    #[must_use]
    pub const fn generated_at_ms(&self) -> i64 {
        self.generated_at_ms
    }

    #[must_use]
    pub const fn payload(&self) -> &ProductDataStatusSnapshot {
        &self.payload
    }

    #[must_use]
    pub fn is_newer_than(&self, current: Option<&Self>) -> bool {
        self.snapshot_generation
            .is_newer_than(current.map(|current| current.snapshot_generation))
    }
}

pub(crate) fn map_capture(
    capture: ProductDataStatusCapture,
    snapshot_generation: SnapshotGeneration,
    generated_at_ms: i64,
) -> Result<ProductDataStatusEnvelope, QueryError> {
    let store_usage = capture.usage();
    let dataset_identity = crate::service::from_store_identity(store_usage.dataset_identity())?;
    let mut usage_warnings = Vec::with_capacity(9);
    let mut common_warnings = Vec::with_capacity(5);
    let quality = crate::service::map_quality(
        dataset_identity,
        store_usage.quality(),
        store_usage.accounting_versions_current(),
        &mut common_warnings,
    )?;
    let freshness = crate::service::map_freshness(
        generated_at_ms,
        store_usage.data_through_ms(),
        &mut common_warnings,
    );
    usage_warnings.extend(common_warnings.into_iter().map(map_common_warning));
    if store_usage.replay_staging() {
        usage_warnings.push(ProductDataWarningCode::ReplayStaging);
    }
    let store_aggregate = store_usage.aggregate();
    let (aggregate_state, aggregate_warning) = match store_aggregate.state() {
        StoreAggregateState::Ready => (ProductAggregateState::Ready, None),
        StoreAggregateState::RebuildRequired => (
            ProductAggregateState::RebuildRequired,
            Some(ProductDataWarningCode::AggregateRebuildRequired),
        ),
        StoreAggregateState::Rebuilding => (
            ProductAggregateState::Rebuilding,
            Some(ProductDataWarningCode::AggregateRebuilding),
        ),
        StoreAggregateState::Failed => (
            ProductAggregateState::Failed,
            Some(ProductDataWarningCode::AggregateFailed),
        ),
    };
    if let Some(warning) = aggregate_warning {
        usage_warnings.push(warning);
    }
    let aggregate = ProductAggregateStatus {
        state: aggregate_state,
        expected_dataset_generation: store_aggregate.expected_dataset_generation(),
        active_generation: store_aggregate.active_generation(),
        current_event_count: store_aggregate.current_event_count(),
        legacy_event_count: store_aggregate.legacy_event_count(),
        progress: store_aggregate
            .progress()
            .map(|progress| ProductAggregateProgress {
                processed_events: progress.processed_events(),
                total_events: progress.total_events(),
            }),
    };
    let quota = capture.quota();
    let benefit = capture.benefit();
    let git = capture.git();
    Ok(ProductDataStatusEnvelope {
        snapshot_generation,
        generated_at_ms,
        payload: ProductDataStatusSnapshot {
            usage: ProductUsageDataStatus {
                publication_generation: PublicationGeneration::new(
                    store_usage.publication_generation(),
                )?,
                dataset_identity,
                data_through_ms: store_usage.data_through_ms(),
                freshness,
                quality,
                scope_count: store_usage.scope_count(),
                aggregate,
                warnings: Arc::from(usage_warnings),
            },
            quota: ProductQuotaDataStatus {
                state: component_state(quota.revision()),
                revision: QuotaRevision::new(quota.revision())?,
                retained_sample_count: quota.retained_sample_count(),
                retained_epoch_count: quota.retained_epoch_count(),
                retained_transition_count: quota.retained_transition_count(),
                last_published_at_ms: quota.last_published_at_ms(),
            },
            benefit: ProductBenefitDataStatus {
                state: component_state(benefit.revision()),
                revision: BenefitRevision::new(benefit.revision())?,
                current_lot_count: benefit.current_lot_count(),
                retained_change_count: benefit.retained_change_count(),
                pending_due_count: benefit.pending_due_count(),
                retained_delivery_count: benefit.retained_delivery_count(),
                last_published_at_ms: benefit.last_published_at_ms(),
            },
            git: ProductGitDataStatus {
                state: component_state(git.publication_revision()),
                revision: GitPublicationRevision::new(git.publication_revision()),
                repository_count: git.repository_count(),
                association_count: git.association_count(),
                last_published_at_ms: git.last_published_at_ms(),
            },
        },
    })
}

const fn component_state(revision: u64) -> ProductComponentState {
    if revision == 0 {
        ProductComponentState::Empty
    } else {
        ProductComponentState::Published
    }
}

const fn map_common_warning(value: QueryWarningCode) -> ProductDataWarningCode {
    match value {
        QueryWarningCode::LegacyUnverified => ProductDataWarningCode::LegacyUnverified,
        QueryWarningCode::Partial => ProductDataWarningCode::Partial,
        QueryWarningCode::RecoveryPending => ProductDataWarningCode::RecoveryPending,
        QueryWarningCode::ClockDiscontinuity => ProductDataWarningCode::ClockDiscontinuity,
        QueryWarningCode::AccountingVersionStale => ProductDataWarningCode::AccountingVersionStale,
    }
}
