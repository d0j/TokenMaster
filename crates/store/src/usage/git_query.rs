use std::time::{Duration, Instant};

use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use tokenmaster_domain::{
    GitActivityAssociationId, GitLineMetrics, GitOutputCategory, GitOutputCategoryMetrics,
    GitOutputDay, GitOutputQuality, GitOutputTotals, GitOutputUnavailableReason, GitOutputWarning,
    GitRepositoryId, MAX_GIT_OUTPUT_CATEGORIES, MAX_GIT_OUTPUT_DAYS, MAX_GIT_OUTPUT_REPOSITORIES,
    MAX_GIT_OUTPUT_WARNINGS, ProjectAlias,
};
use tokenmaster_git::{GitIdentitySalt, derive_project_fingerprint};

use crate::{StoreError, StoreErrorCode};

use super::GitProjectKey;
use super::query::{
    MAX_QUERY_DURATION, MAX_USAGE_BREAKDOWN_ITEMS, PROGRESS_OP_INTERVAL, UsageReadStore, map_sql,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitOutputQuery {
    start_day_index: i32,
    end_day_index: i32,
    max_repositories: usize,
    deadline: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProjectMatchQuery {
    project_keys: Box<[GitProjectKey]>,
    projects: Box<[ProjectAlias]>,
    deadline: Duration,
}

impl GitProjectMatchQuery {
    pub fn new(
        project_keys: Vec<GitProjectKey>,
        projects: Vec<ProjectAlias>,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        if project_keys.len() > MAX_GIT_OUTPUT_REPOSITORIES
            || projects.len() > MAX_USAGE_BREAKDOWN_ITEMS
        {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                if project_keys.len() > MAX_GIT_OUTPUT_REPOSITORIES {
                    MAX_GIT_OUTPUT_REPOSITORIES as u64
                } else {
                    MAX_USAGE_BREAKDOWN_ITEMS as u64
                },
            ));
        }
        if deadline.is_zero()
            || deadline > MAX_QUERY_DURATION
            || projects
                .iter()
                .enumerate()
                .any(|(index, project)| projects[..index].contains(project))
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            project_keys: project_keys.into_boxed_slice(),
            projects: projects.into_boxed_slice(),
            deadline,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProjectMatchCapture {
    project_indices: Box<[Option<usize>]>,
}

impl GitProjectMatchCapture {
    #[must_use]
    pub fn project_indices(&self) -> &[Option<usize>] {
        &self.project_indices
    }
}

impl GitOutputQuery {
    pub fn new(
        start_day_index: i32,
        end_day_index: i32,
        max_repositories: usize,
        deadline: Duration,
    ) -> Result<Self, StoreError> {
        if start_day_index > end_day_index
            || max_repositories == 0
            || deadline.is_zero()
            || deadline > MAX_QUERY_DURATION
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let day_count = i64::from(end_day_index)
            .checked_sub(i64::from(start_day_index))
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?;
        if day_count > MAX_GIT_OUTPUT_DAYS as i64 {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_GIT_OUTPUT_DAYS as u64,
            ));
        }
        if max_repositories > MAX_GIT_OUTPUT_REPOSITORIES {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_GIT_OUTPUT_REPOSITORIES as u64,
            ));
        }
        GitOutputDay::new(start_day_index, 0, 0, GitLineMetrics::new(0, 0))
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?;
        GitOutputDay::new(end_day_index, 0, 0, GitLineMetrics::new(0, 0))
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?;
        Ok(Self {
            start_day_index,
            end_day_index,
            max_repositories,
            deadline,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitRangeMetrics {
    commits: u64,
    merge_commits: u64,
    lines: GitLineMetrics,
}

impl GitRangeMetrics {
    fn new(commits: u64, merge_commits: u64, lines: GitLineMetrics) -> Result<Self, StoreError> {
        if merge_commits > commits {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        Ok(Self {
            commits,
            merge_commits,
            lines,
        })
    }

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutputRepositoryCapture {
    repository_id: GitRepositoryId,
    association_id: GitActivityAssociationId,
    project_key: Option<GitProjectKey>,
    scan_revision: u64,
    observed_at_ms: i64,
    data_through_ms: Option<i64>,
    quality: GitOutputQuality,
    unavailable_reason: Option<GitOutputUnavailableReason>,
    rebuild_required: bool,
    daily_history_truncated: bool,
    retained_from_day_index: Option<i32>,
    range_complete: bool,
    all_time_totals: GitOutputTotals,
    range_totals: GitRangeMetrics,
    all_time_categories: Box<[GitOutputCategoryMetrics]>,
    range_categories: Box<[GitOutputCategoryMetrics]>,
    days: Box<[GitOutputDay]>,
    warnings: Box<[GitOutputWarning]>,
}

impl GitOutputRepositoryCapture {
    #[must_use]
    pub const fn repository_id(&self) -> GitRepositoryId {
        self.repository_id
    }

    #[must_use]
    pub const fn association_id(&self) -> GitActivityAssociationId {
        self.association_id
    }

    #[must_use]
    pub const fn project_key(&self) -> Option<GitProjectKey> {
        self.project_key
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
    pub fn all_time_categories(&self) -> &[GitOutputCategoryMetrics] {
        &self.all_time_categories
    }

    #[must_use]
    pub fn range_categories(&self) -> &[GitOutputCategoryMetrics] {
        &self.range_categories
    }

    #[must_use]
    pub fn days(&self) -> &[GitOutputDay] {
        &self.days
    }

    #[must_use]
    pub fn warnings(&self) -> &[GitOutputWarning] {
        &self.warnings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutputCapture {
    publication_revision: u64,
    published_at_ms: Option<i64>,
    repositories: Box<[GitOutputRepositoryCapture]>,
    has_more_repositories: bool,
}

impl GitOutputCapture {
    #[must_use]
    pub const fn publication_revision(&self) -> u64 {
        self.publication_revision
    }

    #[must_use]
    pub const fn published_at_ms(&self) -> Option<i64> {
        self.published_at_ms
    }

    #[must_use]
    pub fn repositories(&self) -> &[GitOutputRepositoryCapture] {
        &self.repositories
    }

    #[must_use]
    pub const fn has_more_repositories(&self) -> bool {
        self.has_more_repositories
    }
}

impl UsageReadStore {
    pub fn capture_git_output(
        &mut self,
        query: GitOutputQuery,
    ) -> Result<GitOutputCapture, StoreError> {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            PROGRESS_OP_INTERVAL,
            Some(move || started.elapsed() >= deadline),
        ))?;
        let mut result = capture_git_output(&mut self.connection, query);
        if started.elapsed() >= deadline {
            result = Err(StoreError::new(StoreErrorCode::DeadlineExceeded));
        }
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    pub fn capture_git_project_matches(
        &mut self,
        query: GitProjectMatchQuery,
    ) -> Result<GitProjectMatchCapture, StoreError> {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            PROGRESS_OP_INTERVAL,
            Some(move || started.elapsed() >= deadline),
        ))?;
        let mut result = capture_git_project_matches(&self.connection, &query);
        if started.elapsed() >= deadline {
            result = Err(StoreError::new(StoreErrorCode::DeadlineExceeded));
        }
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

fn capture_git_project_matches(
    connection: &Connection,
    query: &GitProjectMatchQuery,
) -> Result<GitProjectMatchCapture, StoreError> {
    let salt = connection.query_row(
        "SELECT installation_salt
         FROM git_installation_state WHERE singleton_id = 1",
        [],
        |row| row.get::<_, Vec<u8>>(0),
    )?;
    let salt: [u8; 32] = salt
        .try_into()
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let salt = GitIdentitySalt::from_bytes(salt);
    let fingerprints = query
        .projects
        .iter()
        .map(|project| {
            derive_project_fingerprint(&salt, project)
                .map(|value| GitProjectKey::from_bytes(*value.as_bytes()))
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let project_indices = query
        .project_keys
        .iter()
        .map(|key| fingerprints.iter().position(|candidate| candidate == key))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Ok(GitProjectMatchCapture { project_indices })
}

fn capture_git_output(
    connection: &mut Connection,
    query: GitOutputQuery,
) -> Result<GitOutputCapture, StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
    let (publication_revision, published_at_ms) = transaction.query_row(
        "SELECT publication_revision, last_published_at_ms
         FROM git_installation_state WHERE singleton_id = 1",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
    )?;
    let (repositories, has_more_repositories) = capture_repositories(&transaction, query)?;
    transaction.commit()?;
    Ok(GitOutputCapture {
        publication_revision: to_u64(publication_revision)?,
        published_at_ms,
        repositories: repositories.into_boxed_slice(),
        has_more_repositories,
    })
}

struct RepositoryRow {
    repository_id: GitRepositoryId,
    active_generation: u64,
    scan_revision: u64,
    observed_at_ms: i64,
    data_through_ms: Option<i64>,
    quality: GitOutputQuality,
    unavailable_reason: Option<GitOutputUnavailableReason>,
    rebuild_required: bool,
    totals: GitOutputTotals,
}

fn capture_repositories(
    transaction: &Transaction<'_>,
    query: GitOutputQuery,
) -> Result<(Vec<GitOutputRepositoryCapture>, bool), StoreError> {
    let mut statement = transaction.prepare(
        "SELECT repository_id, active_generation, scan_revision,
                observed_at_ms, data_through_ms, quality, unavailable_reason,
                publication_state, commits, merge_commits, added_lines, removed_lines,
                binary_files, submodule_changes, omitted_commits, omitted_paths
         FROM git_repository
         ORDER BY repository_id
         LIMIT ?1",
    )?;
    let rows = statement.query_map(
        [i64::try_from(query.max_repositories + 1)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?],
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, i64>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, i64>(15)?,
            ))
        },
    )?;
    let mut captured = Vec::with_capacity(query.max_repositories + 1);
    for row in rows {
        let row = row?;
        let repository = RepositoryRow {
            repository_id: GitRepositoryId::from_bytes(to_array(row.0)?),
            active_generation: to_u64(row.1)?,
            scan_revision: to_u64(row.2)?,
            observed_at_ms: row.3,
            data_through_ms: row.4,
            quality: parse_quality(&row.5)?,
            unavailable_reason: row.6.as_deref().map(parse_unavailable).transpose()?,
            rebuild_required: match row.7.as_str() {
                "ready" => false,
                "rebuild_required" => true,
                _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
            },
            totals: GitOutputTotals::new(
                to_u64(row.8)?,
                to_u64(row.9)?,
                GitLineMetrics::new(to_u64(row.10)?, to_u64(row.11)?),
                to_u64(row.12)?,
                to_u64(row.13)?,
                to_u64(row.14)?,
                to_u64(row.15)?,
            )
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        };
        captured.push(capture_repository(transaction, query, repository)?);
    }
    let has_more_repositories = captured.len() > query.max_repositories;
    captured.truncate(query.max_repositories);
    Ok((captured, has_more_repositories))
}

fn capture_repository(
    transaction: &Transaction<'_>,
    query: GitOutputQuery,
    repository: RepositoryRow,
) -> Result<GitOutputRepositoryCapture, StoreError> {
    let association = read_association(transaction, &repository)?;
    let days = read_days(transaction, query, &repository)?;
    let range_totals = sum_days(&days)?;
    let range_categories = read_range_categories(transaction, query, &repository, days.len())?;
    let all_time_categories = read_all_time_categories(transaction, &repository)?;
    let mut warnings = read_warnings(transaction, &repository)?;
    let daily_history_truncated = warnings.contains(&GitOutputWarning::DailyHistoryTruncated);
    let retained_from_day_index = read_retained_from_day(transaction, &repository)?;
    let range_complete = repository.quality != GitOutputQuality::Unavailable
        && (!daily_history_truncated
            || retained_from_day_index.is_some_and(|day| query.start_day_index >= day));
    if sum_categories(&range_categories)? != range_totals.lines()
        || (repository.quality != GitOutputQuality::Unavailable
            && sum_categories(&all_time_categories)? != repository.totals.lines())
        || (repository.quality == GitOutputQuality::Complete && !warnings.is_empty())
        || (repository.quality == GitOutputQuality::Partial && warnings.is_empty())
        || (repository.quality == GitOutputQuality::Unavailable
            && (!warnings.is_empty()
                || !days.is_empty()
                || !range_categories.is_empty()
                || !all_time_categories.is_empty()))
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    if repository.rebuild_required {
        insert_effective_warning(&mut warnings, GitOutputWarning::IncrementalRebuildPending)?;
    }
    if !association.complete && repository.quality != GitOutputQuality::Unavailable {
        insert_effective_warning(&mut warnings, GitOutputWarning::AssociationIncomplete)?;
    }
    let quality = if repository.rebuild_required
        || (!association.complete && repository.quality != GitOutputQuality::Unavailable)
    {
        GitOutputQuality::Partial
    } else {
        repository.quality
    };
    Ok(GitOutputRepositoryCapture {
        repository_id: repository.repository_id,
        association_id: association.association_id,
        project_key: association.project_key,
        scan_revision: repository.scan_revision,
        observed_at_ms: repository.observed_at_ms,
        data_through_ms: repository.data_through_ms,
        quality,
        unavailable_reason: repository.unavailable_reason,
        rebuild_required: repository.rebuild_required,
        daily_history_truncated,
        retained_from_day_index,
        range_complete,
        all_time_totals: repository.totals,
        range_totals,
        all_time_categories: all_time_categories.into_boxed_slice(),
        range_categories: range_categories.into_boxed_slice(),
        days: days.into_boxed_slice(),
        warnings: warnings.into_boxed_slice(),
    })
}

struct AssociationCapture {
    association_id: GitActivityAssociationId,
    project_key: Option<GitProjectKey>,
    complete: bool,
}

fn read_association(
    transaction: &Transaction<'_>,
    repository: &RepositoryRow,
) -> Result<AssociationCapture, StoreError> {
    let row = transaction.query_row(
        "SELECT
           (SELECT association_id
            FROM git_activity_association
            WHERE repository_id = ?1
            ORDER BY last_activity_at_ms DESC, association_id
            LIMIT 1),
           count(*), count(project_key), count(DISTINCT project_key), min(project_key)
         FROM git_activity_association
         WHERE repository_id = ?1",
        [repository.repository_id.as_bytes().as_slice()],
        |row| {
            Ok((
                row.get::<_, Option<Vec<u8>>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<Vec<u8>>>(4)?,
            ))
        },
    )?;
    let association_id = row
        .0
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let complete = row.1 > 0 && row.2 == row.1 && row.3 == 1;
    let project_key = if complete {
        Some(GitProjectKey::from_bytes(to_array(row.4.ok_or_else(
            || StoreError::new(StoreErrorCode::InvalidStoredValue),
        )?)?))
    } else {
        None
    };
    Ok(AssociationCapture {
        association_id: GitActivityAssociationId::from_bytes(to_array(association_id)?),
        project_key,
        complete,
    })
}

fn insert_effective_warning(
    warnings: &mut Vec<GitOutputWarning>,
    warning: GitOutputWarning,
) -> Result<(), StoreError> {
    match warnings.binary_search(&warning) {
        Ok(_) => Ok(()),
        Err(index) => {
            if warnings.len() == MAX_GIT_OUTPUT_WARNINGS {
                return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
            }
            warnings.insert(index, warning);
            Ok(())
        }
    }
}

fn read_retained_from_day(
    transaction: &Transaction<'_>,
    repository: &RepositoryRow,
) -> Result<Option<i32>, StoreError> {
    let value = transaction.query_row(
        "SELECT min(day_index)
         FROM git_day_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2",
        params![
            repository.repository_id.as_bytes().as_slice(),
            to_i64(repository.active_generation)?
        ],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    value
        .map(|day| {
            i32::try_from(day).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
        })
        .transpose()
}

fn read_days(
    transaction: &Transaction<'_>,
    query: GitOutputQuery,
    repository: &RepositoryRow,
) -> Result<Vec<GitOutputDay>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT day_index, commits, merge_commits, added_lines, removed_lines
         FROM git_day_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2
           AND day_index BETWEEN ?3 AND ?4
         ORDER BY day_index",
    )?;
    let rows = statement.query_map(
        params![
            repository.repository_id.as_bytes().as_slice(),
            to_i64(repository.active_generation)?,
            i64::from(query.start_day_index),
            i64::from(query.end_day_index),
        ],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        },
    )?;
    let mut days = Vec::with_capacity(MAX_GIT_OUTPUT_DAYS.min(32));
    for row in rows {
        let row = row?;
        days.push(
            GitOutputDay::new(
                i32::try_from(row.0)
                    .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
                to_u64(row.1)?,
                to_u64(row.2)?,
                GitLineMetrics::new(to_u64(row.3)?, to_u64(row.4)?),
            )
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        );
    }
    if days.len() > MAX_GIT_OUTPUT_DAYS {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(days)
}

fn sum_days(days: &[GitOutputDay]) -> Result<GitRangeMetrics, StoreError> {
    let mut commits = 0_u64;
    let mut merges = 0_u64;
    let mut lines = GitLineMetrics::new(0, 0);
    for day in days {
        commits = checked_add(commits, day.commits())?;
        merges = checked_add(merges, day.merge_commits())?;
        lines = lines
            .checked_add(day.lines())
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    }
    GitRangeMetrics::new(commits, merges, lines)
}

fn read_range_categories(
    transaction: &Transaction<'_>,
    query: GitOutputQuery,
    repository: &RepositoryRow,
    expected_day_count: usize,
) -> Result<Vec<GitOutputCategoryMetrics>, StoreError> {
    if repository.quality == GitOutputQuality::Unavailable {
        return Ok(Vec::new());
    }
    let stored_count: i64 = transaction.query_row(
        "SELECT count(*)
         FROM git_day_category_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2
           AND day_index BETWEEN ?3 AND ?4",
        params![
            repository.repository_id.as_bytes().as_slice(),
            to_i64(repository.active_generation)?,
            i64::from(query.start_day_index),
            i64::from(query.end_day_index),
        ],
        |row| row.get(0),
    )?;
    let expected_count = expected_day_count
        .checked_mul(MAX_GIT_OUTPUT_CATEGORIES)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    if stored_count != expected_count {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let mut lines = [GitLineMetrics::new(0, 0); MAX_GIT_OUTPUT_CATEGORIES];
    let mut statement = transaction.prepare(
        "SELECT category, sum(added_lines), sum(removed_lines)
         FROM git_day_category_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2
           AND day_index BETWEEN ?3 AND ?4
         GROUP BY category",
    )?;
    let rows = statement.query_map(
        params![
            repository.repository_id.as_bytes().as_slice(),
            to_i64(repository.active_generation)?,
            i64::from(query.start_day_index),
            i64::from(query.end_day_index),
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    for row in rows {
        let row = row?;
        let category = parse_category(&row.0)?;
        lines[category_index(category)] = GitLineMetrics::new(to_u64(row.1)?, to_u64(row.2)?);
    }
    Ok(all_categories()
        .into_iter()
        .map(|category| GitOutputCategoryMetrics::new(category, lines[category_index(category)]))
        .collect())
}

fn read_all_time_categories(
    transaction: &Transaction<'_>,
    repository: &RepositoryRow,
) -> Result<Vec<GitOutputCategoryMetrics>, StoreError> {
    if repository.quality == GitOutputQuality::Unavailable {
        return Ok(Vec::new());
    }
    let mut items = Vec::with_capacity(MAX_GIT_OUTPUT_CATEGORIES);
    let mut statement = transaction.prepare(
        "SELECT category, added_lines, removed_lines
         FROM git_category_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2",
    )?;
    let rows = statement.query_map(
        params![
            repository.repository_id.as_bytes().as_slice(),
            to_i64(repository.active_generation)?
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    for row in rows {
        let row = row?;
        items.push(GitOutputCategoryMetrics::new(
            parse_category(&row.0)?,
            GitLineMetrics::new(to_u64(row.1)?, to_u64(row.2)?),
        ));
    }
    items.sort_unstable_by_key(|item| item.category());
    if items.len() != MAX_GIT_OUTPUT_CATEGORIES
        || items
            .windows(2)
            .any(|pair| pair[0].category() >= pair[1].category())
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(items)
}

fn read_warnings(
    transaction: &Transaction<'_>,
    repository: &RepositoryRow,
) -> Result<Vec<GitOutputWarning>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT warning FROM git_warning
         WHERE repository_id = ?1 AND aggregate_generation = ?2",
    )?;
    let rows = statement.query_map(
        params![
            repository.repository_id.as_bytes().as_slice(),
            to_i64(repository.active_generation)?
        ],
        |row| row.get::<_, String>(0),
    )?;
    let mut warnings = Vec::with_capacity(MAX_GIT_OUTPUT_WARNINGS);
    for row in rows {
        warnings.push(parse_warning(&row?)?);
    }
    warnings.sort_unstable();
    if warnings.len() > MAX_GIT_OUTPUT_WARNINGS
        || warnings.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(warnings)
}

fn parse_quality(value: &str) -> Result<GitOutputQuality, StoreError> {
    match value {
        "complete" => Ok(GitOutputQuality::Complete),
        "partial" => Ok(GitOutputQuality::Partial),
        "unavailable" => Ok(GitOutputQuality::Unavailable),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn parse_unavailable(value: &str) -> Result<GitOutputUnavailableReason, StoreError> {
    match value {
        "git_not_found" => Ok(GitOutputUnavailableReason::GitNotFound),
        "git_not_native" => Ok(GitOutputUnavailableReason::GitNotNative),
        "repository_not_found" => Ok(GitOutputUnavailableReason::RepositoryNotFound),
        "repository_path_rejected" => Ok(GitOutputUnavailableReason::RepositoryPathRejected),
        "author_identity_missing" => Ok(GitOutputUnavailableReason::AuthorIdentityMissing),
        "unsupported_git_version" => Ok(GitOutputUnavailableReason::UnsupportedGitVersion),
        "unsupported_object_format" => Ok(GitOutputUnavailableReason::UnsupportedObjectFormat),
        "too_many_repositories" => Ok(GitOutputUnavailableReason::TooManyRepositories),
        "too_many_refs" => Ok(GitOutputUnavailableReason::TooManyRefs),
        "history_limit_exceeded" => Ok(GitOutputUnavailableReason::HistoryLimitExceeded),
        "output_limit_exceeded" => Ok(GitOutputUnavailableReason::OutputLimitExceeded),
        "deadline_exceeded" => Ok(GitOutputUnavailableReason::DeadlineExceeded),
        "process_failed" => Ok(GitOutputUnavailableReason::ProcessFailed),
        "history_changed_during_scan" => Ok(GitOutputUnavailableReason::HistoryChangedDuringScan),
        "cache_incompatible" => Ok(GitOutputUnavailableReason::CacheIncompatible),
        "store_unavailable" => Ok(GitOutputUnavailableReason::StoreUnavailable),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn parse_warning(value: &str) -> Result<GitOutputWarning, StoreError> {
    match value {
        "shallow_history" => Ok(GitOutputWarning::ShallowHistory),
        "binary_files_omitted" => Ok(GitOutputWarning::BinaryFilesOmitted),
        "submodule_lines_omitted" => Ok(GitOutputWarning::SubmoduleLinesOmitted),
        "oversized_fields_omitted" => Ok(GitOutputWarning::OversizedFieldsOmitted),
        "invalid_commit_omitted" => Ok(GitOutputWarning::InvalidCommitOmitted),
        "daily_history_truncated" => Ok(GitOutputWarning::DailyHistoryTruncated),
        "incremental_rebuild_pending" => Ok(GitOutputWarning::IncrementalRebuildPending),
        "association_incomplete" => Ok(GitOutputWarning::AssociationIncomplete),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn parse_category(value: &str) -> Result<GitOutputCategory, StoreError> {
    match value {
        "product_code" => Ok(GitOutputCategory::ProductCode),
        "test" => Ok(GitOutputCategory::Test),
        "docs_spec" => Ok(GitOutputCategory::DocsSpec),
        "config_build" => Ok(GitOutputCategory::ConfigBuild),
        "schema_migration" => Ok(GitOutputCategory::SchemaMigration),
        "vendor_generated" => Ok(GitOutputCategory::VendorGenerated),
        "asset" => Ok(GitOutputCategory::Asset),
        "other" => Ok(GitOutputCategory::Other),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
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

const fn all_categories() -> [GitOutputCategory; MAX_GIT_OUTPUT_CATEGORIES] {
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

fn checked_add(left: u64, right: u64) -> Result<u64, StoreError> {
    left.checked_add(right)
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn sum_categories(items: &[GitOutputCategoryMetrics]) -> Result<GitLineMetrics, StoreError> {
    let mut lines = GitLineMetrics::new(0, 0);
    for item in items {
        lines = lines
            .checked_add(item.lines())
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    }
    Ok(lines)
}

fn to_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn to_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn to_array(value: Vec<u8>) -> Result<[u8; 32], StoreError> {
    value
        .try_into()
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}
