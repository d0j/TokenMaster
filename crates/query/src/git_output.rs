use std::sync::Arc;

use tokenmaster_domain::{
    GitActivityAssociationId, GitLineMetrics, GitOutputCategory, GitOutputCategoryMetrics,
    GitOutputDay, GitOutputQuality, GitOutputTotals, GitOutputUnavailableReason, GitOutputWarning,
    GitRepositoryId, MAX_GIT_OUTPUT_REPOSITORIES, ProjectAlias,
};
use tokenmaster_pricing::{CostAvailability, CostResult, UsdMicros};
use tokenmaster_store::{
    ArchivePublicationQuality, GitOutputCapture as StoreCapture,
    GitOutputRepositoryCapture as StoreRepository, UsageBreakdown as StoreBreakdown,
    UsageQueryPublication,
};

use crate::{
    CalendarDate, DatasetIdentity, QueryError, QueryErrorCode, QueryFreshness, QueryScope,
    SnapshotGeneration, UsageAnalyticsRequest, UsageBreakdown, UsageBreakdownIdentity,
    UsageBreakdownKind, UsageRange, UsageSeriesSelection, UsageTimeZone, WeekStart,
    analytics::UsageAnalyticsPlan,
};

pub const GIT_QUERY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutputRequest {
    range: UsageRange,
    week_start: WeekStart,
    scopes: Box<[QueryScope]>,
    max_repositories: usize,
}

impl GitOutputRequest {
    pub fn new(
        range: UsageRange,
        week_start: WeekStart,
        mut scopes: Vec<QueryScope>,
        max_repositories: usize,
    ) -> Result<Self, QueryError> {
        if max_repositories == 0 {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        if max_repositories > MAX_GIT_OUTPUT_REPOSITORIES || scopes.len() > crate::MAX_QUERY_SCOPES
        {
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
        Ok(Self {
            range,
            week_start,
            scopes: scopes.into_boxed_slice(),
            max_repositories,
        })
    }

    #[must_use]
    pub const fn range(&self) -> &UsageRange {
        &self.range
    }

    #[must_use]
    pub const fn week_start(&self) -> WeekStart {
        self.week_start
    }

    #[must_use]
    pub const fn scopes(&self) -> &[QueryScope] {
        &self.scopes
    }

    #[must_use]
    pub const fn max_repositories(&self) -> usize {
        self.max_repositories
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GitPublicationRevision(u64);

impl GitPublicationRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitQueryHeader {
    snapshot_generation: SnapshotGeneration,
    publication_revision: GitPublicationRevision,
    generated_at_ms: i64,
    published_at_ms: Option<i64>,
    scopes: Arc<[QueryScope]>,
}

impl GitQueryHeader {
    pub(crate) fn new(
        snapshot_generation: SnapshotGeneration,
        publication_revision: u64,
        generated_at_ms: i64,
        published_at_ms: Option<i64>,
        scopes: Vec<QueryScope>,
    ) -> Result<Self, QueryError> {
        if (publication_revision == 0) != published_at_ms.is_none() {
            return Err(QueryError::new(QueryErrorCode::CorruptArchive));
        }
        Ok(Self {
            snapshot_generation,
            publication_revision: GitPublicationRevision::new(publication_revision),
            generated_at_ms,
            published_at_ms,
            scopes: Arc::from(scopes),
        })
    }

    #[must_use]
    pub const fn snapshot_generation(&self) -> SnapshotGeneration {
        self.snapshot_generation
    }

    #[must_use]
    pub const fn publication_revision(&self) -> GitPublicationRevision {
        self.publication_revision
    }

    #[must_use]
    pub const fn generated_at_ms(&self) -> i64 {
        self.generated_at_ms
    }

    #[must_use]
    pub const fn published_at_ms(&self) -> Option<i64> {
        self.published_at_ms
    }

    #[must_use]
    pub const fn scopes(&self) -> &Arc<[QueryScope]> {
        &self.scopes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitEnvelope<T> {
    header: GitQueryHeader,
    payload: T,
}

impl<T> GitEnvelope<T> {
    pub(crate) const fn new(header: GitQueryHeader, payload: T) -> Self {
        Self { header, payload }
    }

    #[must_use]
    pub const fn header(&self) -> &GitQueryHeader {
        &self.header
    }

    #[must_use]
    pub const fn payload(&self) -> &T {
        &self.payload
    }

    #[must_use]
    pub fn is_newer_than(&self, other: Option<&Self>) -> bool {
        other.is_none_or(|current| {
            self.header.snapshot_generation > current.header.snapshot_generation
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutputRange {
    start_date: CalendarDate,
    end_date: CalendarDate,
    start_seconds: i64,
    end_seconds: i64,
    start_day_index: i32,
    end_day_index_exclusive: i32,
}

impl GitOutputRange {
    #[must_use]
    pub const fn time_zone_id(&self) -> &'static str {
        "UTC"
    }

    #[must_use]
    pub const fn start_date(&self) -> CalendarDate {
        self.start_date
    }

    #[must_use]
    pub const fn end_date(&self) -> CalendarDate {
        self.end_date
    }

    #[must_use]
    pub const fn start_seconds(&self) -> i64 {
        self.start_seconds
    }

    #[must_use]
    pub const fn end_seconds(&self) -> i64 {
        self.end_seconds
    }

    #[must_use]
    pub const fn start_day_index(&self) -> i32 {
        self.start_day_index
    }

    #[must_use]
    pub const fn end_day_index_exclusive(&self) -> i32 {
        self.end_day_index_exclusive
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitRangeMetrics {
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
}

impl GitRangeMetrics {
    #[must_use]
    pub const fn commits(self) -> u64 {
        self.commits
    }

    #[must_use]
    pub const fn merge_commits(self) -> u64 {
        self.merge_commits
    }

    #[must_use]
    pub const fn lines(self) -> GitLineMetrics {
        self.lines
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitEfficiencyUnavailableReason {
    AssociationUnavailable,
    GitQualityIncomplete,
    GitRangeIncomplete,
    GitStale,
    ProjectNotInUsageSnapshot,
    RangeMismatch,
    UsageCostUnavailable,
    UsageDeadlineExceeded,
    UsageEvidenceInvalid,
    UsageEvidenceUnavailable,
    UsageQualityIncomplete,
    UsageStale,
    ZeroProductCodeLines,
}

impl GitEfficiencyUnavailableReason {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::AssociationUnavailable => "association_unavailable",
            Self::GitQualityIncomplete => "git_quality_incomplete",
            Self::GitRangeIncomplete => "git_range_incomplete",
            Self::GitStale => "git_stale",
            Self::ProjectNotInUsageSnapshot => "project_not_in_usage_snapshot",
            Self::RangeMismatch => "range_mismatch",
            Self::UsageCostUnavailable => "usage_cost_unavailable",
            Self::UsageDeadlineExceeded => "usage_deadline_exceeded",
            Self::UsageEvidenceInvalid => "usage_evidence_invalid",
            Self::UsageEvidenceUnavailable => "usage_evidence_unavailable",
            Self::UsageQualityIncomplete => "usage_quality_incomplete",
            Self::UsageStale => "usage_stale",
            Self::ZeroProductCodeLines => "zero_product_code_lines",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitEfficiencyValue {
    usage_dataset_identity: DatasetIdentity,
    usage_cost: UsdMicros,
    product_code_added_lines: u64,
    cost_per_100_added_lines: UsdMicros,
}

impl GitEfficiencyValue {
    #[must_use]
    pub const fn usage_dataset_identity(&self) -> DatasetIdentity {
        self.usage_dataset_identity
    }

    #[must_use]
    pub const fn usage_cost(&self) -> UsdMicros {
        self.usage_cost
    }

    #[must_use]
    pub const fn product_code_added_lines(&self) -> u64 {
        self.product_code_added_lines
    }

    #[must_use]
    pub const fn cost_per_100_added_lines(&self) -> UsdMicros {
        self.cost_per_100_added_lines
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitEfficiency {
    Available(GitEfficiencyValue),
    Unavailable(GitEfficiencyUnavailableReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutputRepository {
    repository_id: GitRepositoryId,
    association_id: GitActivityAssociationId,
    project_alias: Option<ProjectAlias>,
    scan_revision: u64,
    observed_at_ms: i64,
    data_through_ms: Option<i64>,
    freshness: QueryFreshness,
    quality: GitOutputQuality,
    unavailable_reason: Option<GitOutputUnavailableReason>,
    rebuild_required: bool,
    daily_history_truncated: bool,
    retained_from_day_index: Option<i32>,
    range_complete: bool,
    all_time_totals: GitOutputTotals,
    range_totals: GitRangeMetrics,
    all_time_categories: Arc<[GitOutputCategoryMetrics]>,
    range_categories: Arc<[GitOutputCategoryMetrics]>,
    days: Arc<[GitOutputDay]>,
    warnings: Arc<[GitOutputWarning]>,
    efficiency: GitEfficiency,
}

impl GitOutputRepository {
    #[must_use]
    pub const fn repository_id(&self) -> GitRepositoryId {
        self.repository_id
    }

    #[must_use]
    pub const fn association_id(&self) -> GitActivityAssociationId {
        self.association_id
    }

    #[must_use]
    pub const fn project_alias(&self) -> Option<&ProjectAlias> {
        self.project_alias.as_ref()
    }

    #[must_use]
    pub const fn scan_revision(&self) -> u64 {
        self.scan_revision
    }

    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
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
    pub const fn quality(&self) -> GitOutputQuality {
        self.quality
    }

    #[must_use]
    pub const fn unavailable_reason(&self) -> Option<GitOutputUnavailableReason> {
        self.unavailable_reason
    }

    #[must_use]
    pub const fn rebuild_required(&self) -> bool {
        self.rebuild_required
    }

    #[must_use]
    pub const fn daily_history_truncated(&self) -> bool {
        self.daily_history_truncated
    }

    #[must_use]
    pub const fn retained_from_day_index(&self) -> Option<i32> {
        self.retained_from_day_index
    }

    #[must_use]
    pub const fn range_complete(&self) -> bool {
        self.range_complete
    }

    #[must_use]
    pub const fn all_time_totals(&self) -> &GitOutputTotals {
        &self.all_time_totals
    }

    #[must_use]
    pub const fn range_totals(&self) -> GitRangeMetrics {
        self.range_totals
    }

    #[must_use]
    pub const fn all_time_categories(&self) -> &Arc<[GitOutputCategoryMetrics]> {
        &self.all_time_categories
    }

    #[must_use]
    pub const fn range_categories(&self) -> &Arc<[GitOutputCategoryMetrics]> {
        &self.range_categories
    }

    #[must_use]
    pub const fn days(&self) -> &Arc<[GitOutputDay]> {
        &self.days
    }

    #[must_use]
    pub const fn warnings(&self) -> &Arc<[GitOutputWarning]> {
        &self.warnings
    }

    #[must_use]
    pub const fn efficiency(&self) -> &GitEfficiency {
        &self.efficiency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutputSnapshot {
    range: GitOutputRange,
    repositories: Arc<[GitOutputRepository]>,
    has_more_repositories: bool,
}

impl GitOutputSnapshot {
    #[must_use]
    pub const fn range(&self) -> &GitOutputRange {
        &self.range
    }

    #[must_use]
    pub const fn repositories(&self) -> &Arc<[GitOutputRepository]> {
        &self.repositories
    }

    #[must_use]
    pub const fn has_more_repositories(&self) -> bool {
        self.has_more_repositories
    }
}

pub(crate) fn usage_request(
    request: &GitOutputRequest,
) -> Result<UsageAnalyticsRequest, QueryError> {
    UsageAnalyticsRequest::new(
        request.range.clone(),
        UsageTimeZone::iana("UTC")?,
        request.week_start,
        UsageSeriesSelection::None,
        request.scopes.to_vec(),
        vec![UsageBreakdownKind::Project],
    )
}

pub(crate) fn range_from_plan(plan: &UsageAnalyticsPlan) -> Result<GitOutputRange, QueryError> {
    let overview = plan.overview();
    if overview.start_seconds().rem_euclid(86_400) != 0
        || overview.end_seconds().rem_euclid(86_400) != 0
        || overview.start_seconds() >= overview.end_seconds()
    {
        return Err(QueryError::new(QueryErrorCode::UnsupportedTimeBoundary));
    }
    let start = i32::try_from(overview.start_seconds().div_euclid(86_400))
        .map_err(|_| QueryError::new(QueryErrorCode::Overflow))?;
    let end = i32::try_from(overview.end_seconds().div_euclid(86_400))
        .map_err(|_| QueryError::new(QueryErrorCode::Overflow))?;
    Ok(GitOutputRange {
        start_date: overview.start_date(),
        end_date: overview.end_date(),
        start_seconds: overview.start_seconds(),
        end_seconds: overview.end_seconds(),
        start_day_index: start,
        end_day_index_exclusive: end,
    })
}

pub(crate) enum GitUsageEvidence {
    Available {
        publication: UsageQueryPublication,
        breakdown: UsageBreakdown,
    },
    Unavailable(GitEfficiencyUnavailableReason),
}

pub(crate) fn map_snapshot(
    capture: StoreCapture,
    range: GitOutputRange,
    generation: SnapshotGeneration,
    generated_at_ms: i64,
    evidence: GitUsageEvidence,
    project_indices: &[Option<usize>],
    scopes: Vec<QueryScope>,
) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
    let (usage_identity, usage_freshness, usage_quality_complete, projects, forced_unavailable) =
        match &evidence {
            GitUsageEvidence::Available {
                publication,
                breakdown,
            } => {
                if project_indices.len()
                    != capture
                        .repositories()
                        .iter()
                        .filter(|repository| repository.project_key().is_some())
                        .count()
                {
                    return Err(QueryError::new(QueryErrorCode::CorruptArchive));
                }
                let identity = crate::service::from_store_identity(publication.dataset_identity())?;
                let freshness = freshness(generated_at_ms, publication.data_through_ms());
                let quality_complete = publication.quality() == ArchivePublicationQuality::Complete
                    && publication.accounting_versions_current()
                    && !matches!(identity, DatasetIdentity::LegacySnapshotV1);
                let projects = breakdown
                    .items()
                    .iter()
                    .filter_map(|item| match item.identity() {
                        UsageBreakdownIdentity::Project(project) => Some((project, item.cost())),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                (identity, freshness, quality_complete, projects, None)
            }
            GitUsageEvidence::Unavailable(reason) => (
                DatasetIdentity::Empty,
                QueryFreshness::Unavailable,
                false,
                Vec::new(),
                Some(*reason),
            ),
        };
    let mut match_cursor = 0_usize;
    let repositories = capture
        .repositories()
        .iter()
        .map(|repository| {
            let project_index =
                if forced_unavailable.is_none() && repository.project_key().is_some() {
                    let value = project_indices
                        .get(match_cursor)
                        .copied()
                        .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
                    match_cursor += 1;
                    value
                } else {
                    None
                };
            map_repository(
                repository,
                generated_at_ms,
                usage_identity,
                usage_freshness,
                usage_quality_complete,
                &projects,
                project_index,
                forced_unavailable,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let header = GitQueryHeader::new(
        generation,
        capture.publication_revision(),
        generated_at_ms,
        capture.published_at_ms(),
        scopes,
    )?;
    Ok(GitEnvelope::new(
        header,
        GitOutputSnapshot {
            range,
            repositories: Arc::from(repositories),
            has_more_repositories: capture.has_more_repositories(),
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn map_repository(
    repository: &StoreRepository,
    generated_at_ms: i64,
    usage_identity: DatasetIdentity,
    usage_freshness: QueryFreshness,
    usage_quality_complete: bool,
    projects: &[(&ProjectAlias, &CostResult)],
    project_index: Option<usize>,
    forced_unavailable: Option<GitEfficiencyUnavailableReason>,
) -> Result<GitOutputRepository, QueryError> {
    let freshness = freshness(generated_at_ms, Some(repository.observed_at_ms()));
    let project = project_index
        .map(|index| {
            projects
                .get(index)
                .copied()
                .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))
        })
        .transpose()?;
    let product_code_added_lines = repository
        .range_categories()
        .iter()
        .find(|item| item.category() == GitOutputCategory::ProductCode)
        .map_or(0, |item| item.lines().added());
    let efficiency = efficiency(
        repository,
        freshness,
        EfficiencyUsageEvidence {
            identity: usage_identity,
            freshness: usage_freshness,
            quality_complete: usage_quality_complete,
            cost: project.map(|value| value.1),
            forced_unavailable,
        },
        product_code_added_lines,
    )?;
    Ok(GitOutputRepository {
        repository_id: repository.repository_id(),
        association_id: repository.association_id(),
        project_alias: project.map(|value| value.0.clone()),
        scan_revision: repository.scan_revision(),
        observed_at_ms: repository.observed_at_ms(),
        data_through_ms: repository.data_through_ms(),
        freshness,
        quality: repository.quality(),
        unavailable_reason: repository.unavailable_reason(),
        rebuild_required: repository.rebuild_required(),
        daily_history_truncated: repository.daily_history_truncated(),
        retained_from_day_index: repository.retained_from_day_index(),
        range_complete: repository.range_complete(),
        all_time_totals: repository.all_time_totals().clone(),
        range_totals: GitRangeMetrics {
            commits: repository.range_totals().commits(),
            merge_commits: repository.range_totals().merge_commits(),
            lines: repository.range_totals().lines(),
        },
        all_time_categories: Arc::from(repository.all_time_categories()),
        range_categories: Arc::from(repository.range_categories()),
        days: Arc::from(repository.days()),
        warnings: Arc::from(repository.warnings()),
        efficiency,
    })
}

struct EfficiencyUsageEvidence<'a> {
    identity: DatasetIdentity,
    freshness: QueryFreshness,
    quality_complete: bool,
    cost: Option<&'a CostResult>,
    forced_unavailable: Option<GitEfficiencyUnavailableReason>,
}

fn efficiency(
    repository: &StoreRepository,
    git_freshness: QueryFreshness,
    usage: EfficiencyUsageEvidence<'_>,
    product_code_added_lines: u64,
) -> Result<GitEfficiency, QueryError> {
    let unavailable = if !repository.range_complete() {
        Some(GitEfficiencyUnavailableReason::GitRangeIncomplete)
    } else if repository.quality() != GitOutputQuality::Complete {
        Some(GitEfficiencyUnavailableReason::GitQualityIncomplete)
    } else if repository.project_key().is_none() {
        Some(GitEfficiencyUnavailableReason::AssociationUnavailable)
    } else if product_code_added_lines == 0 {
        Some(GitEfficiencyUnavailableReason::ZeroProductCodeLines)
    } else if matches!(
        git_freshness,
        QueryFreshness::Stale | QueryFreshness::Unavailable
    ) {
        Some(GitEfficiencyUnavailableReason::GitStale)
    } else if let Some(reason) = usage.forced_unavailable {
        Some(reason)
    } else if usage.cost.is_none() {
        Some(GitEfficiencyUnavailableReason::ProjectNotInUsageSnapshot)
    } else if !usage.quality_complete {
        Some(GitEfficiencyUnavailableReason::UsageQualityIncomplete)
    } else if matches!(
        usage.freshness,
        QueryFreshness::Stale | QueryFreshness::Unavailable
    ) {
        Some(GitEfficiencyUnavailableReason::UsageStale)
    } else {
        let cost = usage
            .cost
            .ok_or_else(|| QueryError::new(QueryErrorCode::Internal))?;
        if !matches!(
            cost.availability(),
            CostAvailability::Complete | CostAvailability::Zero
        ) || cost.counters().conflict_events != 0
        {
            Some(GitEfficiencyUnavailableReason::UsageCostUnavailable)
        } else {
            None
        }
    };
    if let Some(reason) = unavailable {
        return Ok(GitEfficiency::Unavailable(reason));
    }
    let usage_cost = usage
        .cost
        .and_then(CostResult::amount)
        .ok_or_else(|| QueryError::new(QueryErrorCode::CorruptArchive))?;
    let numerator = u128::from(usage_cost.get())
        .checked_mul(100)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?;
    let divisor = u128::from(product_code_added_lines);
    let rounded = numerator
        .checked_add(divisor / 2)
        .ok_or_else(|| QueryError::new(QueryErrorCode::Overflow))?
        / divisor;
    let rounded = u64::try_from(rounded).map_err(|_| QueryError::new(QueryErrorCode::Overflow))?;
    Ok(GitEfficiency::Available(GitEfficiencyValue {
        usage_dataset_identity: usage.identity,
        usage_cost,
        product_code_added_lines,
        cost_per_100_added_lines: UsdMicros::new(rounded),
    }))
}

const fn freshness(generated_at_ms: i64, observed_at_ms: Option<i64>) -> QueryFreshness {
    let Some(observed_at_ms) = observed_at_ms else {
        return QueryFreshness::Unavailable;
    };
    let Some(age_ms) = generated_at_ms.checked_sub(observed_at_ms) else {
        return QueryFreshness::Unavailable;
    };
    if age_ms < 0 {
        QueryFreshness::Unavailable
    } else if age_ms <= crate::QUERY_FRESH_MAX_AGE_MS {
        QueryFreshness::Fresh
    } else if age_ms <= crate::QUERY_STALE_MIN_AGE_MS {
        QueryFreshness::Aging
    } else {
        QueryFreshness::Stale
    }
}

pub(crate) fn project_aliases(breakdown: &StoreBreakdown) -> Result<Vec<ProjectAlias>, QueryError> {
    breakdown
        .items()
        .iter()
        .filter_map(|item| match item.identity() {
            tokenmaster_store::UsageBreakdownIdentity::Project(value) => Some(
                ProjectAlias::new(value.to_string())
                    .map_err(|_error| QueryError::new(QueryErrorCode::CorruptArchive)),
            ),
            _ => None,
        })
        .collect()
}
