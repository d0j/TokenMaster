use std::fmt;

use tokenmaster_domain::{
    GitActivityAssociationId, GitOutputQuality, GitOutputUnavailableReason, GitOutputWarning,
    GitRepositoryId, MAX_GIT_OUTPUT_CATEGORIES, MAX_GIT_OUTPUT_DAYS, MAX_GIT_OUTPUT_WARNINGS,
};
use tokenmaster_git::{
    GitAuthorFingerprint, GitMailmapFingerprint, GitObjectFormat, GitRefFingerprint, GitScanSummary,
};

use crate::{StoreError, StoreErrorCode};

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct GitProjectKey([u8; 32]);

impl GitProjectKey {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for GitProjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitProjectKey([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitCacheIdentity {
    object_format: GitObjectFormat,
    heads_fingerprint: GitRefFingerprint,
    mailmap_fingerprint: GitMailmapFingerprint,
    author_fingerprint: GitAuthorFingerprint,
    category_version: u16,
    shallow: bool,
}

impl GitCacheIdentity {
    pub fn new(
        object_format: GitObjectFormat,
        heads_fingerprint: GitRefFingerprint,
        mailmap_fingerprint: GitMailmapFingerprint,
        author_fingerprint: GitAuthorFingerprint,
        category_version: u16,
        shallow: bool,
    ) -> Result<Self, StoreError> {
        if category_version == 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            object_format,
            heads_fingerprint,
            mailmap_fingerprint,
            author_fingerprint,
            category_version,
            shallow,
        })
    }

    #[must_use]
    pub const fn object_format(self) -> GitObjectFormat {
        self.object_format
    }

    #[must_use]
    pub const fn heads_fingerprint(self) -> GitRefFingerprint {
        self.heads_fingerprint
    }

    #[must_use]
    pub const fn mailmap_fingerprint(self) -> GitMailmapFingerprint {
        self.mailmap_fingerprint
    }

    #[must_use]
    pub const fn author_fingerprint(self) -> GitAuthorFingerprint {
        self.author_fingerprint
    }

    #[must_use]
    pub const fn category_version(self) -> u16 {
        self.category_version
    }

    #[must_use]
    pub const fn is_shallow(self) -> bool {
        self.shallow
    }
}

pub struct GitProjectionInputParts {
    pub repository_id: GitRepositoryId,
    pub association_id: GitActivityAssociationId,
    pub project_key: Option<GitProjectKey>,
    pub activity_at_ms: i64,
    pub observed_at_ms: i64,
    pub data_through_ms: Option<i64>,
    pub quality: GitOutputQuality,
    pub unavailable_reason: Option<GitOutputUnavailableReason>,
    pub warnings: Vec<GitOutputWarning>,
    pub summary: Option<GitScanSummary>,
    pub cache: Option<GitCacheIdentity>,
}

pub struct GitProjectionInput {
    repository_id: GitRepositoryId,
    association_id: GitActivityAssociationId,
    project_key: Option<GitProjectKey>,
    activity_at_ms: i64,
    observed_at_ms: i64,
    data_through_ms: Option<i64>,
    quality: GitOutputQuality,
    unavailable_reason: Option<GitOutputUnavailableReason>,
    warnings: Vec<GitOutputWarning>,
    summary: Option<GitScanSummary>,
    cache: Option<GitCacheIdentity>,
}

impl GitProjectionInput {
    pub fn new(parts: GitProjectionInputParts) -> Result<Self, StoreError> {
        validate_projection_input(&parts)?;
        Ok(Self {
            repository_id: parts.repository_id,
            association_id: parts.association_id,
            project_key: parts.project_key,
            activity_at_ms: parts.activity_at_ms,
            observed_at_ms: parts.observed_at_ms,
            data_through_ms: parts.data_through_ms,
            quality: parts.quality,
            unavailable_reason: parts.unavailable_reason,
            warnings: parts.warnings,
            summary: parts.summary,
            cache: parts.cache,
        })
    }

    pub(super) const fn repository_id(&self) -> GitRepositoryId {
        self.repository_id
    }

    pub(super) const fn association_id(&self) -> GitActivityAssociationId {
        self.association_id
    }

    pub(super) const fn project_key(&self) -> Option<GitProjectKey> {
        self.project_key
    }

    pub(super) const fn activity_at_ms(&self) -> i64 {
        self.activity_at_ms
    }

    pub(super) const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    pub(super) const fn data_through_ms(&self) -> Option<i64> {
        self.data_through_ms
    }

    pub(super) const fn quality(&self) -> GitOutputQuality {
        self.quality
    }

    pub(super) const fn unavailable_reason(&self) -> Option<GitOutputUnavailableReason> {
        self.unavailable_reason
    }

    pub(super) fn warnings(&self) -> &[GitOutputWarning] {
        &self.warnings
    }

    pub(super) const fn summary(&self) -> Option<&GitScanSummary> {
        self.summary.as_ref()
    }

    pub(super) const fn cache(&self) -> Option<GitCacheIdentity> {
        self.cache
    }
}

impl fmt::Debug for GitProjectionInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitProjectionInput")
            .field("repository_id", &self.repository_id)
            .field("association_id", &self.association_id)
            .field("has_project_key", &self.project_key.is_some())
            .field("activity_at_ms", &self.activity_at_ms)
            .field("observed_at_ms", &self.observed_at_ms)
            .field("data_through_ms", &self.data_through_ms)
            .field("quality", &self.quality)
            .field("unavailable_reason", &self.unavailable_reason)
            .field("warning_count", &self.warnings.len())
            .field("has_summary", &self.summary.is_some())
            .field("cache", &self.cache)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitIncrementalAuthority {
    repository_id: GitRepositoryId,
    expected_scan_revision: u64,
    expected_heads_fingerprint: GitRefFingerprint,
}

impl GitIncrementalAuthority {
    pub fn new(
        repository_id: GitRepositoryId,
        expected_scan_revision: u64,
        expected_heads_fingerprint: GitRefFingerprint,
    ) -> Result<Self, StoreError> {
        if expected_scan_revision == 0 || expected_scan_revision > i64::MAX as u64 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            repository_id,
            expected_scan_revision,
            expected_heads_fingerprint,
        })
    }

    pub(super) const fn repository_id(self) -> GitRepositoryId {
        self.repository_id
    }

    pub(super) const fn expected_scan_revision(self) -> u64 {
        self.expected_scan_revision
    }

    pub(super) const fn expected_heads_fingerprint(self) -> GitRefFingerprint {
        self.expected_heads_fingerprint
    }
}

pub struct GitRefreshInputParts {
    pub authority: GitIncrementalAuthority,
    pub association_id: GitActivityAssociationId,
    pub project_key: Option<GitProjectKey>,
    pub activity_at_ms: i64,
    pub observed_at_ms: i64,
    pub cache: GitCacheIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitRefreshInput {
    authority: GitIncrementalAuthority,
    association_id: GitActivityAssociationId,
    project_key: Option<GitProjectKey>,
    activity_at_ms: i64,
    observed_at_ms: i64,
    cache: GitCacheIdentity,
}

impl GitRefreshInput {
    pub fn new(parts: GitRefreshInputParts) -> Result<Self, StoreError> {
        if parts.activity_at_ms <= 0
            || parts.observed_at_ms <= 0
            || parts.activity_at_ms > parts.observed_at_ms
            || parts.cache.heads_fingerprint() != parts.authority.expected_heads_fingerprint()
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            authority: parts.authority,
            association_id: parts.association_id,
            project_key: parts.project_key,
            activity_at_ms: parts.activity_at_ms,
            observed_at_ms: parts.observed_at_ms,
            cache: parts.cache,
        })
    }

    pub(super) const fn authority(self) -> GitIncrementalAuthority {
        self.authority
    }

    pub(super) const fn association_id(self) -> GitActivityAssociationId {
        self.association_id
    }

    pub(super) const fn project_key(self) -> Option<GitProjectKey> {
        self.project_key
    }

    pub(super) const fn activity_at_ms(self) -> i64 {
        self.activity_at_ms
    }

    pub(super) const fn observed_at_ms(self) -> i64 {
        self.observed_at_ms
    }

    pub(super) const fn cache(self) -> GitCacheIdentity {
        self.cache
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitPublication {
    publication_revision: u64,
    scan_revision: u64,
    aggregate_generation: u64,
}

impl GitPublication {
    pub(super) const fn new(
        publication_revision: u64,
        scan_revision: u64,
        aggregate_generation: u64,
    ) -> Self {
        Self {
            publication_revision,
            scan_revision,
            aggregate_generation,
        }
    }

    #[must_use]
    pub const fn publication_revision(self) -> u64 {
        self.publication_revision
    }

    #[must_use]
    pub const fn scan_revision(self) -> u64 {
        self.scan_revision
    }

    #[must_use]
    pub const fn aggregate_generation(self) -> u64 {
        self.aggregate_generation
    }
}

fn validate_projection_input(parts: &GitProjectionInputParts) -> Result<(), StoreError> {
    if parts.activity_at_ms <= 0
        || parts.observed_at_ms <= 0
        || parts.activity_at_ms > parts.observed_at_ms
        || parts
            .data_through_ms
            .is_some_and(|value| value <= 0 || value > parts.observed_at_ms)
        || parts.warnings.len() > MAX_GIT_OUTPUT_WARNINGS
        || parts.warnings.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    match parts.quality {
        GitOutputQuality::Complete => {
            if parts.unavailable_reason.is_some()
                || !parts.warnings.is_empty()
                || parts.data_through_ms.is_none()
                || parts.summary.is_none()
                || parts.cache.is_none()
            {
                return Err(StoreError::new(StoreErrorCode::InvalidValue));
            }
        }
        GitOutputQuality::Partial => {
            if parts.unavailable_reason.is_some()
                || parts.warnings.is_empty()
                || parts.data_through_ms.is_none()
                || parts.summary.is_none()
                || parts.cache.is_none()
            {
                return Err(StoreError::new(StoreErrorCode::InvalidValue));
            }
        }
        GitOutputQuality::Unavailable => {
            if parts.unavailable_reason.is_none()
                || parts.data_through_ms.is_some()
                || !parts.warnings.is_empty()
                || parts.summary.is_some()
                || parts.cache.is_some()
            {
                return Err(StoreError::new(StoreErrorCode::InvalidValue));
            }
            return Ok(());
        }
    }
    let summary = parts
        .summary
        .as_ref()
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?;
    if summary.retained_days().len() > MAX_GIT_OUTPUT_DAYS
        || summary.categories().len() != MAX_GIT_OUTPUT_CATEGORIES
        || summary.retained_day_categories().len()
            != summary
                .retained_days()
                .len()
                .checked_mul(MAX_GIT_OUTPUT_CATEGORIES)
                .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?
        || (summary.totals().binary_files() > 0)
            != parts
                .warnings
                .contains(&GitOutputWarning::BinaryFilesOmitted)
        || (summary.totals().submodule_changes() > 0)
            != parts
                .warnings
                .contains(&GitOutputWarning::SubmoduleLinesOmitted)
        || (summary.totals().omitted_paths() > 0)
            != parts
                .warnings
                .contains(&GitOutputWarning::OversizedFieldsOmitted)
        || (summary.totals().omitted_commits() > 0)
            != parts
                .warnings
                .contains(&GitOutputWarning::InvalidCommitOmitted)
        || summary.daily_history_truncated()
            != parts
                .warnings
                .contains(&GitOutputWarning::DailyHistoryTruncated)
        || parts.cache.is_some_and(GitCacheIdentity::is_shallow)
            != parts.warnings.contains(&GitOutputWarning::ShallowHistory)
    {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    Ok(())
}
