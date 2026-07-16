use std::collections::{BTreeMap, BTreeSet};

use tokenmaster_domain::{
    GitLineMetrics, GitOutputCategory, GitOutputCategoryMetrics, GitOutputDay, GitOutputTotals,
    MAX_GIT_OUTPUT_DAYS,
};

use crate::{
    GitCommitFingerprint, GitCoreError, MAX_GIT_COMMITS_PER_BATCH, MAX_GIT_PATHS_PER_COMMIT,
    classify_destination_path,
};

const CATEGORY_COUNT: usize = 8;
const MAX_PARENT_COUNT: u16 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GitPathStatKind {
    Text(GitLineMetrics),
    Binary,
    Submodule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitPathStat {
    category: GitOutputCategory,
    kind: GitPathStatKind,
}

impl GitPathStat {
    pub fn text(path: &[u8], added: u64, removed: u64) -> Result<Self, GitCoreError> {
        Ok(Self {
            category: classify_destination_path(path)?,
            kind: GitPathStatKind::Text(GitLineMetrics::new(added, removed)),
        })
    }

    pub fn binary(path: &[u8]) -> Result<Self, GitCoreError> {
        Ok(Self {
            category: classify_destination_path(path)?,
            kind: GitPathStatKind::Binary,
        })
    }

    pub fn submodule(path: &[u8]) -> Result<Self, GitCoreError> {
        Ok(Self {
            category: classify_destination_path(path)?,
            kind: GitPathStatKind::Submodule,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCommitAccumulator {
    fingerprint: GitCommitFingerprint,
    day_index: i32,
    parent_count: u16,
    categories: [GitLineMetrics; CATEGORY_COUNT],
    binary_files: u64,
    submodule_changes: u64,
    changed_paths: usize,
}

impl GitCommitAccumulator {
    pub fn new(
        fingerprint: GitCommitFingerprint,
        day_index: i32,
        parent_count: u16,
    ) -> Result<Self, GitCoreError> {
        GitOutputDay::new(day_index, 0, 0, GitLineMetrics::new(0, 0))
            .map_err(|_| GitCoreError::InvalidTimestamp)?;
        if parent_count > MAX_PARENT_COUNT {
            return Err(GitCoreError::CapacityExceeded {
                limit: usize::from(MAX_PARENT_COUNT),
            });
        }
        Ok(Self {
            fingerprint,
            day_index,
            parent_count,
            categories: [GitLineMetrics::new(0, 0); CATEGORY_COUNT],
            binary_files: 0,
            submodule_changes: 0,
            changed_paths: 0,
        })
    }

    pub fn record(&mut self, stat: GitPathStat) -> Result<(), GitCoreError> {
        if self.parent_count > 1 {
            return Err(GitCoreError::IncoherentState);
        }
        if self.changed_paths == MAX_GIT_PATHS_PER_COMMIT {
            return Err(GitCoreError::CapacityExceeded {
                limit: MAX_GIT_PATHS_PER_COMMIT,
            });
        }
        self.changed_paths = self
            .changed_paths
            .checked_add(1)
            .ok_or(GitCoreError::Overflow)?;
        match stat.kind {
            GitPathStatKind::Text(lines) => {
                let index = category_index(stat.category);
                self.categories[index] = self.categories[index]
                    .checked_add(lines)
                    .map_err(|_| GitCoreError::Overflow)?;
            }
            GitPathStatKind::Binary => {
                self.binary_files = self
                    .binary_files
                    .checked_add(1)
                    .ok_or(GitCoreError::Overflow)?;
            }
            GitPathStatKind::Submodule => {
                self.submodule_changes = self
                    .submodule_changes
                    .checked_add(1)
                    .ok_or(GitCoreError::Overflow)?;
            }
        }
        Ok(())
    }

    pub fn finish(self) -> Result<GitCommitAggregate, GitCoreError> {
        let mut lines = GitLineMetrics::new(0, 0);
        for category in self.categories {
            lines = lines
                .checked_add(category)
                .map_err(|_| GitCoreError::Overflow)?;
        }
        Ok(GitCommitAggregate {
            fingerprint: self.fingerprint,
            day_index: self.day_index,
            parent_count: self.parent_count,
            lines,
            categories: self.categories,
            binary_files: self.binary_files,
            submodule_changes: self.submodule_changes,
            changed_paths: self.changed_paths,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCommitAggregate {
    fingerprint: GitCommitFingerprint,
    day_index: i32,
    parent_count: u16,
    lines: GitLineMetrics,
    categories: [GitLineMetrics; CATEGORY_COUNT],
    binary_files: u64,
    submodule_changes: u64,
    changed_paths: usize,
}

impl GitCommitAggregate {
    #[must_use]
    pub const fn fingerprint(&self) -> GitCommitFingerprint {
        self.fingerprint
    }

    #[must_use]
    pub const fn day_index(&self) -> i32 {
        self.day_index
    }

    #[must_use]
    pub const fn parent_count(&self) -> u16 {
        self.parent_count
    }

    #[must_use]
    pub const fn is_merge(&self) -> bool {
        self.parent_count > 1
    }

    #[must_use]
    pub const fn lines(&self) -> GitLineMetrics {
        self.lines
    }

    #[must_use]
    pub const fn category_lines(&self, category: GitOutputCategory) -> GitLineMetrics {
        self.categories[category_index(category)]
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
    pub const fn changed_paths(&self) -> usize {
        self.changed_paths
    }
}

pub trait GitCommitSink {
    fn push_commit(&mut self, commit: GitCommitAggregate) -> Result<(), GitCoreError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitAggregateBatch {
    commits: Vec<GitCommitAggregate>,
}

impl GitAggregateBatch {
    pub fn new(commits: Vec<GitCommitAggregate>) -> Result<Self, GitCoreError> {
        if commits.len() > MAX_GIT_COMMITS_PER_BATCH {
            return Err(GitCoreError::CapacityExceeded {
                limit: MAX_GIT_COMMITS_PER_BATCH,
            });
        }
        let mut identities = BTreeSet::new();
        if commits
            .iter()
            .any(|commit| !identities.insert(commit.fingerprint))
        {
            return Err(GitCoreError::DuplicateValue);
        }
        Ok(Self { commits })
    }

    #[must_use]
    pub fn commits(&self) -> &[GitCommitAggregate] {
        &self.commits
    }
}

impl GitCommitSink for GitAggregateBatch {
    fn push_commit(&mut self, commit: GitCommitAggregate) -> Result<(), GitCoreError> {
        if self.commits.len() == MAX_GIT_COMMITS_PER_BATCH {
            return Err(GitCoreError::CapacityExceeded {
                limit: MAX_GIT_COMMITS_PER_BATCH,
            });
        }
        if self
            .commits
            .iter()
            .any(|current| current.fingerprint == commit.fingerprint)
        {
            return Err(GitCoreError::DuplicateValue);
        }
        self.commits.push(commit);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DayAggregate {
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
}

impl Default for DayAggregate {
    fn default() -> Self {
        Self {
            commits: 0,
            merge_commits: 0,
            lines: GitLineMetrics::new(0, 0),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitScanAccumulator {
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
    categories: [GitLineMetrics; CATEGORY_COUNT],
    binary_files: u64,
    submodule_changes: u64,
    retained_days: BTreeMap<i32, DayAggregate>,
}

impl Default for GitScanAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl GitScanAccumulator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commits: 0,
            merge_commits: 0,
            lines: GitLineMetrics::new(0, 0),
            categories: [GitLineMetrics::new(0, 0); CATEGORY_COUNT],
            binary_files: 0,
            submodule_changes: 0,
            retained_days: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, commit: GitCommitAggregate) -> Result<(), GitCoreError> {
        self.commits = self.commits.checked_add(1).ok_or(GitCoreError::Overflow)?;
        if commit.is_merge() {
            self.merge_commits = self
                .merge_commits
                .checked_add(1)
                .ok_or(GitCoreError::Overflow)?;
        }
        self.lines = self
            .lines
            .checked_add(commit.lines)
            .map_err(|_| GitCoreError::Overflow)?;
        for category in all_categories() {
            let index = category_index(category);
            self.categories[index] = self.categories[index]
                .checked_add(commit.categories[index])
                .map_err(|_| GitCoreError::Overflow)?;
        }
        self.binary_files = self
            .binary_files
            .checked_add(commit.binary_files)
            .ok_or(GitCoreError::Overflow)?;
        self.submodule_changes = self
            .submodule_changes
            .checked_add(commit.submodule_changes)
            .ok_or(GitCoreError::Overflow)?;
        self.record_day(&commit)?;
        Ok(())
    }

    fn record_day(&mut self, commit: &GitCommitAggregate) -> Result<(), GitCoreError> {
        if !self.retained_days.contains_key(&commit.day_index)
            && self.retained_days.len() == MAX_GIT_OUTPUT_DAYS
        {
            let Some(oldest) = self.retained_days.first_key_value().map(|(day, _)| *day) else {
                return Err(GitCoreError::IncoherentState);
            };
            if commit.day_index <= oldest {
                return Ok(());
            }
            self.retained_days.remove(&oldest);
        }
        let day = self.retained_days.entry(commit.day_index).or_default();
        day.commits = day.commits.checked_add(1).ok_or(GitCoreError::Overflow)?;
        if commit.is_merge() {
            day.merge_commits = day
                .merge_commits
                .checked_add(1)
                .ok_or(GitCoreError::Overflow)?;
        }
        day.lines = day
            .lines
            .checked_add(commit.lines)
            .map_err(|_| GitCoreError::Overflow)?;
        Ok(())
    }

    pub fn finish(self) -> Result<GitScanSummary, GitCoreError> {
        let totals = GitOutputTotals::new(
            self.commits,
            self.merge_commits,
            self.lines,
            self.binary_files,
            self.submodule_changes,
            0,
            0,
        )
        .map_err(|_| GitCoreError::IncoherentState)?;
        let retained_days = self
            .retained_days
            .into_iter()
            .map(|(day_index, day)| {
                GitOutputDay::new(day_index, day.commits, day.merge_commits, day.lines)
                    .map_err(|_| GitCoreError::IncoherentState)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let categories = all_categories()
            .into_iter()
            .map(|category| {
                GitOutputCategoryMetrics::new(category, self.categories[category_index(category)])
            })
            .collect::<Vec<_>>();
        Ok(GitScanSummary {
            totals,
            retained_days,
            categories,
        })
    }
}

impl GitCommitSink for GitScanAccumulator {
    fn push_commit(&mut self, commit: GitCommitAggregate) -> Result<(), GitCoreError> {
        self.push(commit)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitScanSummary {
    totals: GitOutputTotals,
    retained_days: Vec<GitOutputDay>,
    categories: Vec<GitOutputCategoryMetrics>,
}

impl GitScanSummary {
    #[must_use]
    pub const fn totals(&self) -> &GitOutputTotals {
        &self.totals
    }

    #[must_use]
    pub fn retained_days(&self) -> &[GitOutputDay] {
        &self.retained_days
    }

    #[must_use]
    pub fn categories(&self) -> &[GitOutputCategoryMetrics] {
        &self.categories
    }

    #[must_use]
    pub fn category_lines(&self, category: GitOutputCategory) -> GitLineMetrics {
        self.categories[category_index(category)].lines()
    }
}

const fn category_index(category: GitOutputCategory) -> usize {
    match category {
        GitOutputCategory::ProductCode => 0,
        GitOutputCategory::Test => 1,
        GitOutputCategory::DocsSpec => 2,
        GitOutputCategory::ConfigBuild => 3,
        GitOutputCategory::SchemaMigration => 4,
        GitOutputCategory::VendorGenerated => 5,
        GitOutputCategory::Asset => 6,
        GitOutputCategory::Other => 7,
    }
}

const fn all_categories() -> [GitOutputCategory; CATEGORY_COUNT] {
    [
        GitOutputCategory::ProductCode,
        GitOutputCategory::Test,
        GitOutputCategory::DocsSpec,
        GitOutputCategory::ConfigBuild,
        GitOutputCategory::SchemaMigration,
        GitOutputCategory::VendorGenerated,
        GitOutputCategory::Asset,
        GitOutputCategory::Other,
    ]
}
