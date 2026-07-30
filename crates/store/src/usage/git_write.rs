use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use tokenmaster_domain::{GitLineMetrics, GitOutputQuality};
use tokenmaster_git::{
    GitAuthorFingerprint, GitIdentitySalt, GitMailmapFingerprint, GitObjectFormat,
    GitRefFingerprint, GitScanSummary,
};

use crate::{StoreError, StoreErrorCode};

use super::{
    GitIncrementalAuthority, GitProjectionInput, GitPublication, GitRefreshInput, UsageStore,
};

impl UsageStore {
    pub fn git_identity_salt(&self) -> Result<GitIdentitySalt, StoreError> {
        let bytes = self.connection.query_row(
            "SELECT installation_salt FROM git_installation_state WHERE singleton_id = 1",
            [],
            |row| row.get::<_, Vec<u8>>(0),
        )?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        Ok(GitIdentitySalt::from_bytes(bytes))
    }

    pub fn publish_git_rebuild(
        &mut self,
        input: &GitProjectionInput,
    ) -> Result<GitPublication, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = git_state(&transaction)?;
        let repository = existing_repository(&transaction, input.repository_id().as_bytes())?;
        if repository.is_none() && state.repository_count == 32 {
            return Err(StoreError::with_limit(StoreErrorCode::CapacityExceeded, 32));
        }
        if repository
            .as_ref()
            .is_some_and(|current| input.observed_at_ms() <= current.observed_at_ms)
        {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        let scan_revision = repository
            .as_ref()
            .map_or(Ok(1_u64), |current| checked_next(current.scan_revision))?;
        let aggregate_generation = repository
            .as_ref()
            .map_or(Ok(1_u64), |current| checked_next(current.active_generation))?;
        let publication_revision = checked_next(state.publication_revision)?;

        write_repository(&transaction, input, scan_revision, aggregate_generation)?;
        write_aggregates(
            &transaction,
            input.repository_id().as_bytes(),
            aggregate_generation,
            input.summary(),
            input.warnings(),
        )?;
        ensure_association_capacity(
            &transaction,
            input.association_id().as_bytes(),
            state.association_count,
        )?;
        upsert_association(&transaction, input)?;
        if let Some(current) = repository {
            delete_generation(
                &transaction,
                input.repository_id().as_bytes(),
                current.active_generation,
            )?;
        }
        update_git_state(&transaction, publication_revision, input.observed_at_ms())?;
        transaction.commit()?;
        Ok(GitPublication::new(
            publication_revision,
            scan_revision,
            aggregate_generation,
        ))
    }

    pub fn publish_git_append(
        &mut self,
        authority: GitIncrementalAuthority,
        input: &GitProjectionInput,
    ) -> Result<GitPublication, StoreError> {
        if authority.repository_id() != input.repository_id() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = git_state(&transaction)?;
        let current = incremental_repository(&transaction, authority.repository_id().as_bytes())?
            .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
        validate_incremental_authority(authority, &current)?;
        validate_append_input(input, &current)?;

        let scan_revision = checked_next(current.scan_revision)?;
        let aggregate_generation = checked_next(current.active_generation)?;
        let publication_revision = checked_next(state.publication_revision)?;
        copy_generation(
            &transaction,
            input.repository_id().as_bytes(),
            current.active_generation,
            aggregate_generation,
        )?;
        let daily_history_truncated = merge_delta_aggregates(
            &transaction,
            input.repository_id().as_bytes(),
            aggregate_generation,
            input
                .summary()
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?,
            input.warnings(),
        )?;
        write_appended_repository(
            &transaction,
            input,
            &current,
            scan_revision,
            aggregate_generation,
            daily_history_truncated,
        )?;
        ensure_association_capacity(
            &transaction,
            input.association_id().as_bytes(),
            state.association_count,
        )?;
        upsert_association(&transaction, input)?;
        delete_generation(
            &transaction,
            input.repository_id().as_bytes(),
            current.active_generation,
        )?;
        update_git_state(&transaction, publication_revision, input.observed_at_ms())?;
        transaction.commit()?;
        Ok(GitPublication::new(
            publication_revision,
            scan_revision,
            aggregate_generation,
        ))
    }

    pub fn refresh_git_unchanged(
        &mut self,
        input: &GitRefreshInput,
    ) -> Result<GitPublication, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = git_state(&transaction)?;
        let authority = input.authority();
        let current = incremental_repository(&transaction, authority.repository_id().as_bytes())?
            .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
        validate_incremental_authority(authority, &current)?;
        if current.publication_state != "ready"
            || input.observed_at_ms() <= current.observed_at_ms
            || !cache_matches(input.cache(), &current)
        {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        let scan_revision = checked_next(current.scan_revision)?;
        let publication_revision = checked_next(state.publication_revision)?;
        let changed = transaction.execute(
            "UPDATE git_repository
             SET scan_revision = ?1, observed_at_ms = ?2
             WHERE repository_id = ?3
               AND scan_revision = ?4
               AND heads_fingerprint = ?5
               AND publication_state = 'ready'",
            params![
                to_i64(scan_revision)?,
                input.observed_at_ms(),
                authority.repository_id().as_bytes().as_slice(),
                to_i64(authority.expected_scan_revision())?,
                authority.expected_heads_fingerprint().as_bytes().as_slice(),
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        ensure_association_capacity(
            &transaction,
            input.association_id().as_bytes(),
            state.association_count,
        )?;
        upsert_association_values(
            &transaction,
            input.association_id().as_bytes(),
            authority.repository_id().as_bytes(),
            input.project_key(),
            input.activity_at_ms(),
        )?;
        update_git_state(&transaction, publication_revision, input.observed_at_ms())?;
        transaction.commit()?;
        Ok(GitPublication::new(
            publication_revision,
            scan_revision,
            current.active_generation,
        ))
    }

    pub fn mark_git_rebuild_required(
        &mut self,
        authority: GitIncrementalAuthority,
        detected_at_ms: i64,
    ) -> Result<GitPublication, StoreError> {
        if detected_at_ms <= 0 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = git_state(&transaction)?;
        let current = incremental_repository(&transaction, authority.repository_id().as_bytes())?
            .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
        validate_incremental_authority(authority, &current)?;
        if current.publication_state != "ready" || detected_at_ms <= current.observed_at_ms {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        let scan_revision = checked_next(current.scan_revision)?;
        let publication_revision = checked_next(state.publication_revision)?;
        let changed = transaction.execute(
            "UPDATE git_repository
             SET scan_revision = ?1, publication_state = 'rebuild_required'
             WHERE repository_id = ?2
               AND scan_revision = ?3
               AND heads_fingerprint = ?4
               AND publication_state = 'ready'",
            params![
                to_i64(scan_revision)?,
                authority.repository_id().as_bytes().as_slice(),
                to_i64(authority.expected_scan_revision())?,
                authority.expected_heads_fingerprint().as_bytes().as_slice(),
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleRevision));
        }
        update_git_state(&transaction, publication_revision, detected_at_ms)?;
        transaction.commit()?;
        Ok(GitPublication::new(
            publication_revision,
            scan_revision,
            current.active_generation,
        ))
    }
}

#[derive(Clone, Copy)]
struct GitState {
    publication_revision: u64,
    repository_count: u64,
    association_count: u64,
}

fn git_state(transaction: &Transaction<'_>) -> Result<GitState, StoreError> {
    transaction
        .query_row(
            "SELECT publication_revision, repository_count, association_count
             FROM git_installation_state
             WHERE singleton_id = 1
               AND (SELECT count(*) FROM git_installation_state) = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))
        .and_then(|(revision, repositories, associations)| {
            Ok(GitState {
                publication_revision: to_u64(revision)?,
                repository_count: to_u64(repositories)?,
                association_count: to_u64(associations)?,
            })
        })
}

#[derive(Clone, Copy)]
struct ExistingRepository {
    active_generation: u64,
    scan_revision: u64,
    observed_at_ms: i64,
}

fn existing_repository(
    transaction: &Transaction<'_>,
    repository_id: &[u8; 32],
) -> Result<Option<ExistingRepository>, StoreError> {
    transaction
        .query_row(
            "SELECT active_generation, scan_revision, observed_at_ms
             FROM git_repository WHERE repository_id = ?1",
            [repository_id.as_slice()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?
        .map(|(generation, revision, observed_at_ms)| {
            Ok(ExistingRepository {
                active_generation: to_u64(generation)?,
                scan_revision: to_u64(revision)?,
                observed_at_ms,
            })
        })
        .transpose()
}

fn write_repository(
    transaction: &Transaction<'_>,
    input: &GitProjectionInput,
    scan_revision: u64,
    aggregate_generation: u64,
) -> Result<(), StoreError> {
    let totals = input.summary().map(GitScanSummary::totals);
    let lines = totals.map_or(GitLineMetrics::new(0, 0), |value| value.lines());
    let cache = input.cache();
    let object_format = cache.map(|value| object_format_code(value.object_format()));
    let heads = cache.map(|value| *value.heads_fingerprint().as_bytes());
    let mailmap = cache.map(|value| *value.mailmap_fingerprint().as_bytes());
    let author = cache.map(|value| *value.author_fingerprint().as_bytes());
    let category_version = cache.map(|value| i64::from(value.category_version()));
    let shallow = cache.map(|value| if value.is_shallow() { 1_i64 } else { 0_i64 });
    transaction.execute(
        "INSERT INTO git_repository(
           repository_id, active_generation, scan_revision, object_format,
           heads_fingerprint, mailmap_fingerprint, author_fingerprint,
           category_version, shallow, observed_at_ms, data_through_ms,
           quality, unavailable_reason, publication_state,
           commits, merge_commits, added_lines, removed_lines,
           binary_files, submodule_changes, omitted_commits, omitted_paths
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'ready',
           ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
         )
         ON CONFLICT(repository_id) DO UPDATE SET
           active_generation = excluded.active_generation,
           scan_revision = excluded.scan_revision,
           object_format = excluded.object_format,
           heads_fingerprint = excluded.heads_fingerprint,
           mailmap_fingerprint = excluded.mailmap_fingerprint,
           author_fingerprint = excluded.author_fingerprint,
           category_version = excluded.category_version,
           shallow = excluded.shallow,
           observed_at_ms = excluded.observed_at_ms,
           data_through_ms = excluded.data_through_ms,
           quality = excluded.quality,
           unavailable_reason = excluded.unavailable_reason,
           publication_state = excluded.publication_state,
           commits = excluded.commits,
           merge_commits = excluded.merge_commits,
           added_lines = excluded.added_lines,
           removed_lines = excluded.removed_lines,
           binary_files = excluded.binary_files,
           submodule_changes = excluded.submodule_changes,
           omitted_commits = excluded.omitted_commits,
           omitted_paths = excluded.omitted_paths",
        params![
            input.repository_id().as_bytes().as_slice(),
            to_i64(aggregate_generation)?,
            to_i64(scan_revision)?,
            object_format,
            heads.as_ref().map(|value| value.as_slice()),
            mailmap.as_ref().map(|value| value.as_slice()),
            author.as_ref().map(|value| value.as_slice()),
            category_version,
            shallow,
            input.observed_at_ms(),
            input.data_through_ms(),
            input.quality().stable_code(),
            input
                .unavailable_reason()
                .map(|reason| reason.stable_code()),
            to_i64(totals.map_or(0, |value| value.commits()))?,
            to_i64(totals.map_or(0, |value| value.merge_commits()))?,
            to_i64(lines.added())?,
            to_i64(lines.removed())?,
            to_i64(totals.map_or(0, |value| value.binary_files()))?,
            to_i64(totals.map_or(0, |value| value.submodule_changes()))?,
            to_i64(totals.map_or(0, |value| value.omitted_commits()))?,
            to_i64(totals.map_or(0, |value| value.omitted_paths()))?,
        ],
    )?;
    Ok(())
}

fn write_aggregates(
    transaction: &Transaction<'_>,
    repository_id: &[u8; 32],
    generation: u64,
    summary: Option<&GitScanSummary>,
    warnings: &[tokenmaster_domain::GitOutputWarning],
) -> Result<(), StoreError> {
    let generation = to_i64(generation)?;
    if let Some(summary) = summary {
        let mut day_statement = transaction.prepare_cached(
            "INSERT INTO git_day_aggregate(
               repository_id, aggregate_generation, day_index, commits,
               merge_commits, added_lines, removed_lines
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for day in summary.retained_days() {
            day_statement.execute(params![
                repository_id.as_slice(),
                generation,
                i64::from(day.day_index()),
                to_i64(day.commits())?,
                to_i64(day.merge_commits())?,
                to_i64(day.lines().added())?,
                to_i64(day.lines().removed())?,
            ])?;
        }
        drop(day_statement);

        let mut day_category_statement = transaction.prepare_cached(
            "INSERT INTO git_day_category_aggregate(
               repository_id, aggregate_generation, day_index, category,
               added_lines, removed_lines
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for item in summary.retained_day_categories() {
            day_category_statement.execute(params![
                repository_id.as_slice(),
                generation,
                i64::from(item.day_index()),
                item.category().stable_code(),
                to_i64(item.lines().added())?,
                to_i64(item.lines().removed())?,
            ])?;
        }
        drop(day_category_statement);

        let mut category_statement = transaction.prepare_cached(
            "INSERT INTO git_category_aggregate(
               repository_id, aggregate_generation, category, added_lines, removed_lines
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for item in summary.categories() {
            category_statement.execute(params![
                repository_id.as_slice(),
                generation,
                item.category().stable_code(),
                to_i64(item.lines().added())?,
                to_i64(item.lines().removed())?,
            ])?;
        }
    }

    let mut warning_statement = transaction.prepare_cached(
        "INSERT INTO git_warning(
           repository_id, aggregate_generation, warning
         ) VALUES (?1, ?2, ?3)",
    )?;
    for warning in warnings {
        warning_statement.execute(params![
            repository_id.as_slice(),
            generation,
            warning.stable_code()
        ])?;
    }
    Ok(())
}

fn upsert_association(
    transaction: &Transaction<'_>,
    input: &GitProjectionInput,
) -> Result<(), StoreError> {
    upsert_association_values(
        transaction,
        input.association_id().as_bytes(),
        input.repository_id().as_bytes(),
        input.project_key(),
        input.activity_at_ms(),
    )
}

fn upsert_association_values(
    transaction: &Transaction<'_>,
    association_id: &[u8; 32],
    repository_id: &[u8; 32],
    project_key: Option<super::GitProjectKey>,
    activity_at_ms: i64,
) -> Result<(), StoreError> {
    let changed = transaction.execute(
        "INSERT INTO git_activity_association(
           association_id, repository_id, project_key,
           first_activity_at_ms, last_activity_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(association_id) DO UPDATE SET
           project_key = excluded.project_key,
           first_activity_at_ms = min(
             excluded.first_activity_at_ms,
             git_activity_association.first_activity_at_ms
           ),
           last_activity_at_ms = max(
             excluded.last_activity_at_ms,
             git_activity_association.last_activity_at_ms
           )
         WHERE git_activity_association.repository_id = excluded.repository_id",
        params![
            association_id.as_slice(),
            repository_id.as_slice(),
            project_key.map(|value| value.as_bytes()),
            activity_at_ms,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    Ok(())
}

fn ensure_association_capacity(
    transaction: &Transaction<'_>,
    association_id: &[u8; 32],
    association_count: u64,
) -> Result<(), StoreError> {
    let exists = transaction
        .query_row(
            "SELECT 1 FROM git_activity_association WHERE association_id = ?1",
            [association_id.as_slice()],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !exists && association_count == 4_096 {
        return Err(StoreError::with_limit(
            StoreErrorCode::CapacityExceeded,
            4_096,
        ));
    }
    Ok(())
}

struct IncrementalRepository {
    active_generation: u64,
    scan_revision: u64,
    object_format: String,
    heads_fingerprint: GitRefFingerprint,
    mailmap_fingerprint: GitMailmapFingerprint,
    author_fingerprint: GitAuthorFingerprint,
    category_version: u16,
    shallow: bool,
    observed_at_ms: i64,
    data_through_ms: i64,
    quality: GitOutputQuality,
    publication_state: String,
    commits: u64,
    merge_commits: u64,
    added_lines: u64,
    removed_lines: u64,
    binary_files: u64,
    submodule_changes: u64,
    omitted_commits: u64,
    omitted_paths: u64,
}

fn incremental_repository(
    transaction: &Transaction<'_>,
    repository_id: &[u8; 32],
) -> Result<Option<IncrementalRepository>, StoreError> {
    let row = transaction
        .query_row(
            "SELECT active_generation, scan_revision, object_format,
                    heads_fingerprint, mailmap_fingerprint, author_fingerprint,
                    category_version, shallow, observed_at_ms, data_through_ms,
                    quality, publication_state, commits, merge_commits,
                    added_lines, removed_lines, binary_files, submodule_changes,
                    omitted_commits, omitted_paths
             FROM git_repository WHERE repository_id = ?1",
            [repository_id.as_slice()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Option<Vec<u8>>>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<bool>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, i64>(13)?,
                    row.get::<_, i64>(14)?,
                    row.get::<_, i64>(15)?,
                    row.get::<_, i64>(16)?,
                    row.get::<_, i64>(17)?,
                    row.get::<_, i64>(18)?,
                    row.get::<_, i64>(19)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let object_format = row
        .2
        .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?;
    let heads = row
        .3
        .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?;
    let mailmap = row
        .4
        .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?;
    let author = row
        .5
        .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?;
    let category_version = u16::try_from(
        row.6
            .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?,
    )
    .ok()
    .filter(|value| *value > 0)
    .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let data_through_ms = row
        .9
        .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?;
    Ok(Some(IncrementalRepository {
        active_generation: to_u64(row.0)?,
        scan_revision: to_u64(row.1)?,
        object_format,
        heads_fingerprint: GitRefFingerprint::from_bytes(to_array(heads)?),
        mailmap_fingerprint: GitMailmapFingerprint::from_bytes(to_array(mailmap)?),
        author_fingerprint: GitAuthorFingerprint::from_bytes(to_array(author)?),
        category_version,
        shallow: row
            .7
            .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?,
        observed_at_ms: row.8,
        data_through_ms,
        quality: parse_quality(&row.10)?,
        publication_state: row.11,
        commits: to_u64(row.12)?,
        merge_commits: to_u64(row.13)?,
        added_lines: to_u64(row.14)?,
        removed_lines: to_u64(row.15)?,
        binary_files: to_u64(row.16)?,
        submodule_changes: to_u64(row.17)?,
        omitted_commits: to_u64(row.18)?,
        omitted_paths: to_u64(row.19)?,
    }))
}

fn validate_incremental_authority(
    authority: GitIncrementalAuthority,
    current: &IncrementalRepository,
) -> Result<(), StoreError> {
    if current.scan_revision != authority.expected_scan_revision()
        || current.heads_fingerprint != authority.expected_heads_fingerprint()
    {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    Ok(())
}

fn validate_append_input(
    input: &GitProjectionInput,
    current: &IncrementalRepository,
) -> Result<(), StoreError> {
    let cache = input
        .cache()
        .ok_or_else(|| StoreError::new(StoreErrorCode::RebuildRequired))?;
    if current.publication_state != "ready"
        || current.quality == GitOutputQuality::Unavailable
        || input.quality() == GitOutputQuality::Unavailable
        || input.observed_at_ms() <= current.observed_at_ms
        || cache.heads_fingerprint() == current.heads_fingerprint
        || !cache_compatible_for_append(cache, current)
    {
        return Err(StoreError::new(StoreErrorCode::RebuildRequired));
    }
    Ok(())
}

fn cache_matches(cache: super::GitCacheIdentity, current: &IncrementalRepository) -> bool {
    cache.heads_fingerprint() == current.heads_fingerprint
        && cache_compatible_for_append(cache, current)
}

fn cache_compatible_for_append(
    cache: super::GitCacheIdentity,
    current: &IncrementalRepository,
) -> bool {
    object_format_code(cache.object_format()) == current.object_format
        && cache.mailmap_fingerprint() == current.mailmap_fingerprint
        && cache.author_fingerprint() == current.author_fingerprint
        && cache.category_version() == current.category_version
        && cache.is_shallow() == current.shallow
}

fn copy_generation(
    transaction: &Transaction<'_>,
    repository_id: &[u8; 32],
    old_generation: u64,
    new_generation: u64,
) -> Result<(), StoreError> {
    let old_generation = to_i64(old_generation)?;
    let new_generation = to_i64(new_generation)?;
    transaction.execute(
        "INSERT INTO git_day_aggregate(
           repository_id, aggregate_generation, day_index, commits,
           merge_commits, added_lines, removed_lines
         )
         SELECT repository_id, ?3, day_index, commits,
                merge_commits, added_lines, removed_lines
         FROM git_day_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2",
        params![repository_id.as_slice(), old_generation, new_generation],
    )?;
    transaction.execute(
        "INSERT INTO git_day_category_aggregate(
           repository_id, aggregate_generation, day_index, category,
           added_lines, removed_lines
         )
         SELECT repository_id, ?3, day_index, category, added_lines, removed_lines
         FROM git_day_category_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2",
        params![repository_id.as_slice(), old_generation, new_generation],
    )?;
    transaction.execute(
        "INSERT INTO git_category_aggregate(
           repository_id, aggregate_generation, category, added_lines, removed_lines
         )
         SELECT repository_id, ?3, category, added_lines, removed_lines
         FROM git_category_aggregate
         WHERE repository_id = ?1 AND aggregate_generation = ?2",
        params![repository_id.as_slice(), old_generation, new_generation],
    )?;
    transaction.execute(
        "INSERT INTO git_warning(repository_id, aggregate_generation, warning)
         SELECT repository_id, ?3, warning
         FROM git_warning
         WHERE repository_id = ?1 AND aggregate_generation = ?2",
        params![repository_id.as_slice(), old_generation, new_generation],
    )?;
    Ok(())
}

fn merge_delta_aggregates(
    transaction: &Transaction<'_>,
    repository_id: &[u8; 32],
    generation: u64,
    summary: &GitScanSummary,
    warnings: &[tokenmaster_domain::GitOutputWarning],
) -> Result<bool, StoreError> {
    let generation = to_i64(generation)?;
    for day in summary.retained_days() {
        let prior = transaction
            .query_row(
                "SELECT commits, merge_commits, added_lines, removed_lines
                 FROM git_day_aggregate
                 WHERE repository_id = ?1 AND aggregate_generation = ?2 AND day_index = ?3",
                params![
                    repository_id.as_slice(),
                    generation,
                    i64::from(day.day_index())
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        transaction.execute(
            "DELETE FROM git_day_aggregate
             WHERE repository_id = ?1 AND aggregate_generation = ?2 AND day_index = ?3",
            params![
                repository_id.as_slice(),
                generation,
                i64::from(day.day_index())
            ],
        )?;
        let prior = prior
            .map(|value| -> Result<_, StoreError> {
                Ok((
                    to_u64(value.0)?,
                    to_u64(value.1)?,
                    to_u64(value.2)?,
                    to_u64(value.3)?,
                ))
            })
            .transpose()?
            .unwrap_or((0, 0, 0, 0));
        transaction.execute(
            "INSERT INTO git_day_aggregate(
               repository_id, aggregate_generation, day_index, commits,
               merge_commits, added_lines, removed_lines
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                repository_id.as_slice(),
                generation,
                i64::from(day.day_index()),
                to_i64(checked_add(prior.0, day.commits())?)?,
                to_i64(checked_add(prior.1, day.merge_commits())?)?,
                to_i64(checked_add(prior.2, day.lines().added())?)?,
                to_i64(checked_add(prior.3, day.lines().removed())?)?,
            ],
        )?;
    }
    for item in summary.retained_day_categories() {
        merge_line_row(
            transaction,
            "git_day_category_aggregate",
            repository_id,
            generation,
            Some(i64::from(item.day_index())),
            item.category().stable_code(),
            item.lines(),
        )?;
    }
    for item in summary.categories() {
        merge_line_row(
            transaction,
            "git_category_aggregate",
            repository_id,
            generation,
            None,
            item.category().stable_code(),
            item.lines(),
        )?;
    }
    for warning in warnings {
        transaction.execute(
            "INSERT OR IGNORE INTO git_warning(
               repository_id, aggregate_generation, warning
             ) VALUES (?1, ?2, ?3)",
            params![repository_id.as_slice(), generation, warning.stable_code()],
        )?;
    }
    let pruned = prune_oldest_days(transaction, repository_id, generation)?;
    if pruned {
        transaction.execute(
            "INSERT OR IGNORE INTO git_warning(
               repository_id, aggregate_generation, warning
             ) VALUES (?1, ?2, 'daily_history_truncated')",
            params![repository_id.as_slice(), generation],
        )?;
    }
    Ok(pruned)
}

fn prune_oldest_days(
    transaction: &Transaction<'_>,
    repository_id: &[u8; 32],
    generation: i64,
) -> Result<bool, StoreError> {
    let oldest_retained = transaction
        .query_row(
            "SELECT day_index
             FROM git_day_aggregate
             WHERE repository_id = ?1 AND aggregate_generation = ?2
             ORDER BY day_index DESC
             LIMIT 1 OFFSET 399",
            params![repository_id.as_slice(), generation],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if let Some(oldest_retained) = oldest_retained {
        transaction.execute(
            "DELETE FROM git_day_category_aggregate
             WHERE repository_id = ?1 AND aggregate_generation = ?2 AND day_index < ?3",
            params![repository_id.as_slice(), generation, oldest_retained],
        )?;
        let deleted = transaction.execute(
            "DELETE FROM git_day_aggregate
             WHERE repository_id = ?1 AND aggregate_generation = ?2 AND day_index < ?3",
            params![repository_id.as_slice(), generation, oldest_retained],
        )?;
        return Ok(deleted > 0);
    }
    Ok(false)
}

fn merge_line_row(
    transaction: &Transaction<'_>,
    table: &str,
    repository_id: &[u8; 32],
    generation: i64,
    day_index: Option<i64>,
    category: &str,
    lines: GitLineMetrics,
) -> Result<(), StoreError> {
    let (select, delete, insert) = if day_index.is_some() {
        (
            format!(
                "SELECT added_lines, removed_lines FROM {table}
                 WHERE repository_id = ?1 AND aggregate_generation = ?2
                   AND day_index = ?3 AND category = ?4"
            ),
            format!(
                "DELETE FROM {table}
                 WHERE repository_id = ?1 AND aggregate_generation = ?2
                   AND day_index = ?3 AND category = ?4"
            ),
            format!(
                "INSERT INTO {table}(
                   repository_id, aggregate_generation, day_index, category,
                   added_lines, removed_lines
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ),
        )
    } else {
        (
            format!(
                "SELECT added_lines, removed_lines FROM {table}
                 WHERE repository_id = ?1 AND aggregate_generation = ?2 AND category = ?4"
            ),
            format!(
                "DELETE FROM {table}
                 WHERE repository_id = ?1 AND aggregate_generation = ?2 AND category = ?4"
            ),
            format!(
                "INSERT INTO {table}(
                   repository_id, aggregate_generation, category, added_lines, removed_lines
                 ) VALUES (?1, ?2, ?4, ?5, ?6)"
            ),
        )
    };
    let key = day_index.unwrap_or(0);
    let prior = transaction
        .query_row(
            &select,
            params![repository_id.as_slice(), generation, key, category],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .map(|value| -> Result<_, StoreError> { Ok((to_u64(value.0)?, to_u64(value.1)?)) })
        .transpose()?
        .unwrap_or((0, 0));
    transaction.execute(
        &delete,
        params![repository_id.as_slice(), generation, key, category],
    )?;
    transaction.execute(
        &insert,
        params![
            repository_id.as_slice(),
            generation,
            key,
            category,
            to_i64(checked_add(prior.0, lines.added())?)?,
            to_i64(checked_add(prior.1, lines.removed())?)?,
        ],
    )?;
    Ok(())
}

fn write_appended_repository(
    transaction: &Transaction<'_>,
    input: &GitProjectionInput,
    current: &IncrementalRepository,
    scan_revision: u64,
    aggregate_generation: u64,
    daily_history_truncated: bool,
) -> Result<(), StoreError> {
    let delta = input
        .summary()
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?
        .totals();
    let quality = if current.quality == GitOutputQuality::Partial
        || input.quality() == GitOutputQuality::Partial
        || daily_history_truncated
    {
        GitOutputQuality::Partial
    } else {
        GitOutputQuality::Complete
    };
    let data_through_ms = current
        .data_through_ms
        .max(input.data_through_ms().unwrap_or(current.data_through_ms));
    let cache = input
        .cache()
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?;
    let changed = transaction.execute(
        "UPDATE git_repository
         SET active_generation = ?1, scan_revision = ?2,
             object_format = ?3, heads_fingerprint = ?4,
             mailmap_fingerprint = ?5, author_fingerprint = ?6,
             category_version = ?7, shallow = ?8,
             observed_at_ms = ?9, data_through_ms = ?10,
             quality = ?11, unavailable_reason = NULL, publication_state = 'ready',
             commits = ?12, merge_commits = ?13,
             added_lines = ?14, removed_lines = ?15,
             binary_files = ?16, submodule_changes = ?17,
             omitted_commits = ?18, omitted_paths = ?19
         WHERE repository_id = ?20 AND scan_revision = ?21",
        params![
            to_i64(aggregate_generation)?,
            to_i64(scan_revision)?,
            object_format_code(cache.object_format()),
            cache.heads_fingerprint().as_bytes().as_slice(),
            cache.mailmap_fingerprint().as_bytes().as_slice(),
            cache.author_fingerprint().as_bytes().as_slice(),
            i64::from(cache.category_version()),
            if cache.is_shallow() { 1_i64 } else { 0_i64 },
            input.observed_at_ms(),
            data_through_ms,
            quality.stable_code(),
            to_i64(checked_add(current.commits, delta.commits())?)?,
            to_i64(checked_add(current.merge_commits, delta.merge_commits())?)?,
            to_i64(checked_add(current.added_lines, delta.lines().added())?)?,
            to_i64(checked_add(current.removed_lines, delta.lines().removed())?)?,
            to_i64(checked_add(current.binary_files, delta.binary_files())?)?,
            to_i64(checked_add(
                current.submodule_changes,
                delta.submodule_changes()
            )?)?,
            to_i64(checked_add(
                current.omitted_commits,
                delta.omitted_commits()
            )?)?,
            to_i64(checked_add(current.omitted_paths, delta.omitted_paths())?)?,
            input.repository_id().as_bytes().as_slice(),
            to_i64(current.scan_revision)?,
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::new(StoreErrorCode::StaleRevision));
    }
    Ok(())
}

fn delete_generation(
    transaction: &Transaction<'_>,
    repository_id: &[u8; 32],
    generation: u64,
) -> Result<(), StoreError> {
    let generation = to_i64(generation)?;
    for table in [
        "git_warning",
        "git_day_category_aggregate",
        "git_category_aggregate",
        "git_day_aggregate",
    ] {
        let sql =
            format!("DELETE FROM {table} WHERE repository_id = ?1 AND aggregate_generation = ?2");
        transaction.execute(&sql, params![repository_id.as_slice(), generation])?;
    }
    Ok(())
}

fn update_git_state(
    transaction: &Transaction<'_>,
    publication_revision: u64,
    published_at_ms: i64,
) -> Result<(), StoreError> {
    let changed = transaction.execute(
        "UPDATE git_installation_state
         SET publication_revision = ?1,
             repository_count = (SELECT count(*) FROM git_repository),
             association_count = (SELECT count(*) FROM git_activity_association),
             last_published_at_ms = CASE
               WHEN last_published_at_ms IS NULL OR last_published_at_ms < ?2 THEN ?2
               ELSE last_published_at_ms
             END
         WHERE singleton_id = 1",
        params![to_i64(publication_revision)?, published_at_ms],
    )?;
    if changed != 1 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn object_format_code(format: GitObjectFormat) -> &'static str {
    match format {
        GitObjectFormat::Sha1 => "sha1",
        GitObjectFormat::Sha256 => "sha256",
    }
}

fn checked_next(value: u64) -> Result<u64, StoreError> {
    value
        .checked_add(1)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn checked_add(left: u64, right: u64) -> Result<u64, StoreError> {
    left.checked_add(right)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))
}

fn to_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))
}

fn to_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn to_array(value: Vec<u8>) -> Result<[u8; 32], StoreError> {
    value
        .try_into()
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn parse_quality(value: &str) -> Result<GitOutputQuality, StoreError> {
    match value {
        "complete" => Ok(GitOutputQuality::Complete),
        "partial" => Ok(GitOutputQuality::Partial),
        "unavailable" => Ok(GitOutputQuality::Unavailable),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}
