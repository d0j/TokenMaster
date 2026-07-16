use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

pub const MAX_GIT_OUTPUT_DAYS: usize = 400;
pub const MAX_GIT_OUTPUT_CATEGORIES: usize = 8;
pub const MAX_GIT_OUTPUT_WARNINGS: usize = 16;
pub const MAX_GIT_OUTPUT_REPOSITORIES: usize = 32;

const MIN_GIT_DAY_INDEX: i32 = -719_162;
const MAX_GIT_DAY_INDEX: i32 = 2_932_896;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GitOutputError {
    #[error("value exceeds capacity {limit}")]
    CapacityExceeded { limit: usize },
    #[error("Git output value is incoherent")]
    IncoherentState,
    #[error("Git output values are not in canonical order")]
    InvalidOrdering,
    #[error("Git output time is invalid")]
    InvalidTime,
    #[error("Git output counter overflow")]
    Overflow,
}

macro_rules! opaque_git_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }
    };
}

opaque_git_id!(GitRepositoryId);
opaque_git_id!(GitActivityAssociationId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOutputCategory {
    ProductCode,
    Test,
    DocsSpec,
    ConfigBuild,
    SchemaMigration,
    VendorGenerated,
    Asset,
    Other,
}

impl GitOutputCategory {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::ProductCode => "product_code",
            Self::Test => "test",
            Self::DocsSpec => "docs_spec",
            Self::ConfigBuild => "config_build",
            Self::SchemaMigration => "schema_migration",
            Self::VendorGenerated => "vendor_generated",
            Self::Asset => "asset",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOutputQuality {
    Complete,
    Partial,
    Unavailable,
}

impl GitOutputQuality {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOutputUnavailableReason {
    GitNotFound,
    GitNotNative,
    RepositoryNotFound,
    RepositoryPathRejected,
    AuthorIdentityMissing,
    UnsupportedGitVersion,
    UnsupportedObjectFormat,
    TooManyRepositories,
    TooManyRefs,
    HistoryLimitExceeded,
    OutputLimitExceeded,
    DeadlineExceeded,
    ProcessFailed,
    HistoryChangedDuringScan,
    CacheIncompatible,
    StoreUnavailable,
}

impl GitOutputUnavailableReason {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::GitNotFound => "git_not_found",
            Self::GitNotNative => "git_not_native",
            Self::RepositoryNotFound => "repository_not_found",
            Self::RepositoryPathRejected => "repository_path_rejected",
            Self::AuthorIdentityMissing => "author_identity_missing",
            Self::UnsupportedGitVersion => "unsupported_git_version",
            Self::UnsupportedObjectFormat => "unsupported_object_format",
            Self::TooManyRepositories => "too_many_repositories",
            Self::TooManyRefs => "too_many_refs",
            Self::HistoryLimitExceeded => "history_limit_exceeded",
            Self::OutputLimitExceeded => "output_limit_exceeded",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::ProcessFailed => "process_failed",
            Self::HistoryChangedDuringScan => "history_changed_during_scan",
            Self::CacheIncompatible => "cache_incompatible",
            Self::StoreUnavailable => "store_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOutputWarning {
    ShallowHistory,
    BinaryFilesOmitted,
    SubmoduleLinesOmitted,
    OversizedFieldsOmitted,
    InvalidCommitOmitted,
    IncrementalRebuildPending,
    AssociationIncomplete,
}

impl GitOutputWarning {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::ShallowHistory => "shallow_history",
            Self::BinaryFilesOmitted => "binary_files_omitted",
            Self::SubmoduleLinesOmitted => "submodule_lines_omitted",
            Self::OversizedFieldsOmitted => "oversized_fields_omitted",
            Self::InvalidCommitOmitted => "invalid_commit_omitted",
            Self::IncrementalRebuildPending => "incremental_rebuild_pending",
            Self::AssociationIncomplete => "association_incomplete",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GitLineMetrics {
    added: u64,
    removed: u64,
}

impl GitLineMetrics {
    #[must_use]
    pub const fn new(added: u64, removed: u64) -> Self {
        Self { added, removed }
    }

    #[must_use]
    pub const fn added(self) -> u64 {
        self.added
    }

    #[must_use]
    pub const fn removed(self) -> u64 {
        self.removed
    }

    #[must_use]
    pub fn net_lines(self) -> i128 {
        i128::from(self.added) - i128::from(self.removed)
    }

    pub fn checked_add(self, other: Self) -> Result<Self, GitOutputError> {
        Ok(Self {
            added: self
                .added
                .checked_add(other.added)
                .ok_or(GitOutputError::Overflow)?,
            removed: self
                .removed
                .checked_add(other.removed)
                .ok_or(GitOutputError::Overflow)?,
        })
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.added == 0 && self.removed == 0
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitLineMetricsWire {
    added: u64,
    removed: u64,
}

impl<'de> Deserialize<'de> for GitLineMetrics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GitLineMetricsWire::deserialize(deserializer)?;
        Ok(Self::new(wire.added, wire.removed))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitOutputTotals {
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
    binary_files: u64,
    submodule_changes: u64,
    omitted_commits: u64,
    omitted_paths: u64,
}

impl GitOutputTotals {
    pub fn new(
        commits: u64,
        merge_commits: u64,
        lines: GitLineMetrics,
        binary_files: u64,
        submodule_changes: u64,
        omitted_commits: u64,
        omitted_paths: u64,
    ) -> Result<Self, GitOutputError> {
        if merge_commits > commits {
            return Err(GitOutputError::IncoherentState);
        }
        Ok(Self {
            commits,
            merge_commits,
            lines,
            binary_files,
            submodule_changes,
            omitted_commits,
            omitted_paths,
        })
    }

    #[must_use]
    pub const fn commits(&self) -> u64 {
        self.commits
    }

    #[must_use]
    pub const fn merge_commits(&self) -> u64 {
        self.merge_commits
    }

    #[must_use]
    pub const fn lines(&self) -> GitLineMetrics {
        self.lines
    }

    #[must_use]
    pub const fn binary_files(&self) -> u64 {
        self.binary_files
    }

    #[must_use]
    pub const fn submodule_changes(&self) -> u64 {
        self.submodule_changes
    }

    #[must_use]
    pub const fn omitted_commits(&self) -> u64 {
        self.omitted_commits
    }

    #[must_use]
    pub const fn omitted_paths(&self) -> u64 {
        self.omitted_paths
    }

    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.commits == 0
            && self.merge_commits == 0
            && self.lines.is_zero()
            && self.binary_files == 0
            && self.submodule_changes == 0
            && self.omitted_commits == 0
            && self.omitted_paths == 0
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitOutputTotalsWire {
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
    binary_files: u64,
    submodule_changes: u64,
    omitted_commits: u64,
    omitted_paths: u64,
}

impl<'de> Deserialize<'de> for GitOutputTotals {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GitOutputTotalsWire::deserialize(deserializer)?;
        Self::new(
            wire.commits,
            wire.merge_commits,
            wire.lines,
            wire.binary_files,
            wire.submodule_changes,
            wire.omitted_commits,
            wire.omitted_paths,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitOutputDay {
    day_index: i32,
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
}

impl GitOutputDay {
    pub fn new(
        day_index: i32,
        commits: u64,
        merge_commits: u64,
        lines: GitLineMetrics,
    ) -> Result<Self, GitOutputError> {
        if !(MIN_GIT_DAY_INDEX..=MAX_GIT_DAY_INDEX).contains(&day_index) {
            return Err(GitOutputError::InvalidTime);
        }
        if merge_commits > commits {
            return Err(GitOutputError::IncoherentState);
        }
        Ok(Self {
            day_index,
            commits,
            merge_commits,
            lines,
        })
    }

    #[must_use]
    pub const fn day_index(&self) -> i32 {
        self.day_index
    }

    #[must_use]
    pub const fn commits(&self) -> u64 {
        self.commits
    }

    #[must_use]
    pub const fn merge_commits(&self) -> u64 {
        self.merge_commits
    }

    #[must_use]
    pub const fn lines(&self) -> GitLineMetrics {
        self.lines
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitOutputDayWire {
    day_index: i32,
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
}

impl<'de> Deserialize<'de> for GitOutputDay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GitOutputDayWire::deserialize(deserializer)?;
        Self::new(wire.day_index, wire.commits, wire.merge_commits, wire.lines)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitOutputCategoryMetrics {
    category: GitOutputCategory,
    lines: GitLineMetrics,
}

impl GitOutputCategoryMetrics {
    #[must_use]
    pub const fn new(category: GitOutputCategory, lines: GitLineMetrics) -> Self {
        Self { category, lines }
    }

    #[must_use]
    pub const fn category(&self) -> GitOutputCategory {
        self.category
    }

    #[must_use]
    pub const fn lines(&self) -> GitLineMetrics {
        self.lines
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitOutputCategoryMetricsWire {
    category: GitOutputCategory,
    lines: GitLineMetrics,
}

impl<'de> Deserialize<'de> for GitOutputCategoryMetrics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GitOutputCategoryMetricsWire::deserialize(deserializer)?;
        Ok(Self::new(wire.category, wire.lines))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutputProjectionParts {
    pub repository_id: GitRepositoryId,
    pub association_id: GitActivityAssociationId,
    pub scan_revision: u64,
    pub observed_at_ms: i64,
    pub data_through_ms: Option<i64>,
    pub quality: GitOutputQuality,
    pub unavailable_reason: Option<GitOutputUnavailableReason>,
    pub warnings: Vec<GitOutputWarning>,
    pub totals: GitOutputTotals,
    pub days: Vec<GitOutputDay>,
    pub categories: Vec<GitOutputCategoryMetrics>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitOutputProjection {
    repository_id: GitRepositoryId,
    association_id: GitActivityAssociationId,
    scan_revision: u64,
    observed_at_ms: i64,
    data_through_ms: Option<i64>,
    quality: GitOutputQuality,
    unavailable_reason: Option<GitOutputUnavailableReason>,
    warnings: Vec<GitOutputWarning>,
    totals: GitOutputTotals,
    days: Vec<GitOutputDay>,
    categories: Vec<GitOutputCategoryMetrics>,
}

impl GitOutputProjection {
    pub fn new(parts: GitOutputProjectionParts) -> Result<Self, GitOutputError> {
        validate_projection_parts(&parts)?;
        Ok(Self {
            repository_id: parts.repository_id,
            association_id: parts.association_id,
            scan_revision: parts.scan_revision,
            observed_at_ms: parts.observed_at_ms,
            data_through_ms: parts.data_through_ms,
            quality: parts.quality,
            unavailable_reason: parts.unavailable_reason,
            warnings: parts.warnings,
            totals: parts.totals,
            days: parts.days,
            categories: parts.categories,
        })
    }

    #[must_use]
    pub const fn repository_id(&self) -> GitRepositoryId {
        self.repository_id
    }

    #[must_use]
    pub const fn association_id(&self) -> GitActivityAssociationId {
        self.association_id
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
    pub const fn quality(&self) -> GitOutputQuality {
        self.quality
    }

    #[must_use]
    pub const fn unavailable_reason(&self) -> Option<GitOutputUnavailableReason> {
        self.unavailable_reason
    }

    #[must_use]
    pub fn warnings(&self) -> &[GitOutputWarning] {
        &self.warnings
    }

    #[must_use]
    pub const fn totals(&self) -> &GitOutputTotals {
        &self.totals
    }

    #[must_use]
    pub fn days(&self) -> &[GitOutputDay] {
        &self.days
    }

    #[must_use]
    pub fn categories(&self) -> &[GitOutputCategoryMetrics] {
        &self.categories
    }

    #[must_use]
    pub fn into_parts(self) -> GitOutputProjectionParts {
        GitOutputProjectionParts {
            repository_id: self.repository_id,
            association_id: self.association_id,
            scan_revision: self.scan_revision,
            observed_at_ms: self.observed_at_ms,
            data_through_ms: self.data_through_ms,
            quality: self.quality,
            unavailable_reason: self.unavailable_reason,
            warnings: self.warnings,
            totals: self.totals,
            days: self.days,
            categories: self.categories,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitOutputProjectionWire {
    repository_id: GitRepositoryId,
    association_id: GitActivityAssociationId,
    scan_revision: u64,
    observed_at_ms: i64,
    data_through_ms: Option<i64>,
    quality: GitOutputQuality,
    unavailable_reason: Option<GitOutputUnavailableReason>,
    warnings: Vec<GitOutputWarning>,
    totals: GitOutputTotals,
    days: Vec<GitOutputDay>,
    categories: Vec<GitOutputCategoryMetrics>,
}

impl<'de> Deserialize<'de> for GitOutputProjection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GitOutputProjectionWire::deserialize(deserializer)?;
        Self::new(GitOutputProjectionParts {
            repository_id: wire.repository_id,
            association_id: wire.association_id,
            scan_revision: wire.scan_revision,
            observed_at_ms: wire.observed_at_ms,
            data_through_ms: wire.data_through_ms,
            quality: wire.quality,
            unavailable_reason: wire.unavailable_reason,
            warnings: wire.warnings,
            totals: wire.totals,
            days: wire.days,
            categories: wire.categories,
        })
        .map_err(serde::de::Error::custom)
    }
}

fn validate_projection_parts(parts: &GitOutputProjectionParts) -> Result<(), GitOutputError> {
    if parts.scan_revision == 0 || parts.observed_at_ms <= 0 {
        return Err(GitOutputError::InvalidTime);
    }
    if parts
        .data_through_ms
        .is_some_and(|value| value <= 0 || value > parts.observed_at_ms)
    {
        return Err(GitOutputError::InvalidTime);
    }
    validate_capacity(parts.days.len(), MAX_GIT_OUTPUT_DAYS)?;
    validate_capacity(parts.categories.len(), MAX_GIT_OUTPUT_CATEGORIES)?;
    validate_capacity(parts.warnings.len(), MAX_GIT_OUTPUT_WARNINGS)?;
    validate_strict_order(parts.days.iter().map(GitOutputDay::day_index))?;
    validate_strict_order(
        parts
            .categories
            .iter()
            .map(GitOutputCategoryMetrics::category),
    )?;
    validate_strict_order(parts.warnings.iter().copied())?;

    match parts.quality {
        GitOutputQuality::Complete => {
            if parts.unavailable_reason.is_some()
                || !parts.warnings.is_empty()
                || parts.data_through_ms.is_none()
                || parts.totals.binary_files != 0
                || parts.totals.submodule_changes != 0
                || parts.totals.omitted_commits != 0
                || parts.totals.omitted_paths != 0
            {
                return Err(GitOutputError::IncoherentState);
            }
        }
        GitOutputQuality::Partial => {
            if parts.unavailable_reason.is_some()
                || parts.warnings.is_empty()
                || parts.data_through_ms.is_none()
            {
                return Err(GitOutputError::IncoherentState);
            }
        }
        GitOutputQuality::Unavailable => {
            if parts.unavailable_reason.is_none()
                || parts.data_through_ms.is_some()
                || !parts.warnings.is_empty()
                || !parts.totals.is_zero()
                || !parts.days.is_empty()
                || !parts.categories.is_empty()
            {
                return Err(GitOutputError::IncoherentState);
            }
            return Ok(());
        }
    }
    if (parts.totals.binary_files > 0)
        != parts
            .warnings
            .contains(&GitOutputWarning::BinaryFilesOmitted)
        || (parts.totals.submodule_changes > 0)
            != parts
                .warnings
                .contains(&GitOutputWarning::SubmoduleLinesOmitted)
        || (parts.totals.omitted_paths > 0)
            != parts
                .warnings
                .contains(&GitOutputWarning::OversizedFieldsOmitted)
        || (parts.totals.omitted_commits > 0)
            != parts
                .warnings
                .contains(&GitOutputWarning::InvalidCommitOmitted)
    {
        return Err(GitOutputError::IncoherentState);
    }

    let mut day_commits = 0_u64;
    let mut day_merges = 0_u64;
    let mut day_lines = GitLineMetrics::new(0, 0);
    for day in &parts.days {
        day_commits = day_commits
            .checked_add(day.commits)
            .ok_or(GitOutputError::Overflow)?;
        day_merges = day_merges
            .checked_add(day.merge_commits)
            .ok_or(GitOutputError::Overflow)?;
        day_lines = day_lines.checked_add(day.lines)?;
    }
    if day_commits != parts.totals.commits
        || day_merges != parts.totals.merge_commits
        || day_lines != parts.totals.lines
    {
        return Err(GitOutputError::IncoherentState);
    }

    let mut category_lines = GitLineMetrics::new(0, 0);
    for category in &parts.categories {
        category_lines = category_lines.checked_add(category.lines)?;
    }
    if category_lines != parts.totals.lines {
        return Err(GitOutputError::IncoherentState);
    }

    Ok(())
}

fn validate_capacity(actual: usize, limit: usize) -> Result<(), GitOutputError> {
    if actual > limit {
        return Err(GitOutputError::CapacityExceeded { limit });
    }
    Ok(())
}

fn validate_strict_order<T: Ord>(
    values: impl IntoIterator<Item = T>,
) -> Result<(), GitOutputError> {
    let mut previous = None;
    for value in values {
        if previous.as_ref().is_some_and(|previous| previous >= &value) {
            return Err(GitOutputError::InvalidOrdering);
        }
        previous = Some(value);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitOutputPortfolio {
    projections: Vec<GitOutputProjection>,
}

impl GitOutputPortfolio {
    pub fn new(projections: Vec<GitOutputProjection>) -> Result<Self, GitOutputError> {
        validate_capacity(projections.len(), MAX_GIT_OUTPUT_REPOSITORIES)?;
        validate_strict_order(projections.iter().map(GitOutputProjection::repository_id))?;
        Ok(Self { projections })
    }

    #[must_use]
    pub fn projections(&self) -> &[GitOutputProjection] {
        &self.projections
    }

    #[must_use]
    pub fn into_projections(self) -> Vec<GitOutputProjection> {
        self.projections
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitOutputPortfolioWire {
    projections: Vec<GitOutputProjection>,
}

impl<'de> Deserialize<'de> for GitOutputPortfolio {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GitOutputPortfolioWire::deserialize(deserializer)?;
        Self::new(wire.projections).map_err(serde::de::Error::custom)
    }
}
