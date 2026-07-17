use std::time::{Duration, Instant};

use rusqlite::TransactionBehavior;

use super::{
    MAX_QUERY_DURATION, PROGRESS_OP_INTERVAL, UsageQueryDatasetIdentity, UsageReadStore, map_sql,
};
use crate::{ArchivePublicationQuality, StoreError, StoreErrorCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductDataStatusQuery {
    deadline: Duration,
}

impl ProductDataStatusQuery {
    pub fn new(deadline: Duration) -> Result<Self, StoreError> {
        if deadline.is_zero() || deadline > MAX_QUERY_DURATION {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self { deadline })
    }
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
pub struct ProductUsageStatus {
    publication_generation: u64,
    dataset_identity: UsageQueryDatasetIdentity,
    accounting_versions_current: bool,
    data_through_ms: Option<i64>,
    quality: ArchivePublicationQuality,
    scope_count: usize,
    replay_staging: bool,
    aggregate: ProductAggregateStatus,
}

impl ProductUsageStatus {
    #[must_use]
    pub const fn publication_generation(self) -> u64 {
        self.publication_generation
    }

    #[must_use]
    pub const fn dataset_identity(self) -> UsageQueryDatasetIdentity {
        self.dataset_identity
    }

    #[must_use]
    pub const fn accounting_versions_current(self) -> bool {
        self.accounting_versions_current
    }

    #[must_use]
    pub const fn data_through_ms(self) -> Option<i64> {
        self.data_through_ms
    }

    #[must_use]
    pub const fn quality(self) -> ArchivePublicationQuality {
        self.quality
    }

    #[must_use]
    pub const fn scope_count(self) -> usize {
        self.scope_count
    }

    #[must_use]
    pub const fn replay_staging(self) -> bool {
        self.replay_staging
    }

    #[must_use]
    pub const fn aggregate(self) -> ProductAggregateStatus {
        self.aggregate
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductQuotaStatus {
    revision: u64,
    retained_sample_count: u64,
    retained_epoch_count: u64,
    retained_transition_count: u64,
    last_published_at_ms: Option<i64>,
}

impl ProductQuotaStatus {
    #[must_use]
    pub const fn revision(self) -> u64 {
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
pub struct ProductBenefitStatus {
    revision: u64,
    current_lot_count: u64,
    retained_change_count: u64,
    pending_due_count: u64,
    retained_delivery_count: u64,
    last_published_at_ms: Option<i64>,
}

impl ProductBenefitStatus {
    #[must_use]
    pub const fn revision(self) -> u64 {
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
pub struct ProductGitStatus {
    publication_revision: u64,
    repository_count: u64,
    association_count: u64,
    last_published_at_ms: Option<i64>,
}

impl ProductGitStatus {
    #[must_use]
    pub const fn publication_revision(self) -> u64 {
        self.publication_revision
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductDataStatusCapture {
    usage: ProductUsageStatus,
    quota: ProductQuotaStatus,
    benefit: ProductBenefitStatus,
    git: ProductGitStatus,
}

impl ProductDataStatusCapture {
    #[must_use]
    pub const fn usage(self) -> ProductUsageStatus {
        self.usage
    }

    #[must_use]
    pub const fn quota(self) -> ProductQuotaStatus {
        self.quota
    }

    #[must_use]
    pub const fn benefit(self) -> ProductBenefitStatus {
        self.benefit
    }

    #[must_use]
    pub const fn git(self) -> ProductGitStatus {
        self.git
    }
}

impl UsageReadStore {
    pub fn capture_product_data_status(
        &mut self,
        query: ProductDataStatusQuery,
    ) -> Result<ProductDataStatusCapture, StoreError> {
        self.capture_product_data_status_with_options(query, PROGRESS_OP_INTERVAL, false, || Ok(()))
    }

    fn capture_product_data_status_with_options<F>(
        &mut self,
        query: ProductDataStatusQuery,
        progress_interval: i32,
        cancel_immediately: bool,
        after_publication: F,
    ) -> Result<ProductDataStatusCapture, StoreError>
    where
        F: FnOnce() -> Result<(), StoreError>,
    {
        let started = Instant::now();
        let deadline = query.deadline;
        map_sql(self.connection.progress_handler(
            progress_interval,
            Some(move || cancel_immediately || started.elapsed() >= deadline),
        ))?;
        let result = capture_product_data_status(&mut self.connection, after_publication).and_then(
            |capture| {
                if started.elapsed() >= deadline {
                    Err(StoreError::new(StoreErrorCode::DeadlineExceeded))
                } else {
                    Ok(capture)
                }
            },
        );
        let clear_result = map_sql(self.connection.progress_handler(0, None::<fn() -> bool>));
        match (result, clear_result) {
            (Ok(capture), Ok(())) => Ok(capture),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

fn capture_product_data_status(
    connection: &mut rusqlite::Connection,
    after_publication: impl FnOnce() -> Result<(), StoreError>,
) -> Result<ProductDataStatusCapture, StoreError> {
    let transaction = map_sql(connection.transaction_with_behavior(TransactionBehavior::Deferred))?;
    let raw_publication = super::load_raw_publication(&transaction)?;
    let dataset_identity = raw_publication.dataset_identity()?;
    let accounting_versions_current = raw_publication.accounting_versions_current()?;
    let (data_through_ms, scope_count) =
        load_status_scan_truth(&transaction, raw_publication.latest_complete_scan_set_id)?;
    let publication_generation = nonnegative(raw_publication.archive_generation)?;
    let quality = ArchivePublicationQuality::from_sql(&raw_publication.quality)?;
    after_publication()?;
    let replay_staging_count: i64 = map_sql(transaction.query_row(
        "SELECT count(*) FROM usage_replay_revision WHERE status = 'staging'",
        [],
        |row| row.get(0),
    ))?;
    if !(0..=1).contains(&replay_staging_count) {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let aggregate = load_aggregate_status(&transaction, dataset_generation(dataset_identity)?)?;
    let quota = load_quota_status(&transaction)?;
    let benefit = load_benefit_status(&transaction)?;
    let git = load_git_status(&transaction)?;
    map_sql(transaction.commit())?;
    Ok(ProductDataStatusCapture {
        usage: ProductUsageStatus {
            publication_generation,
            dataset_identity,
            accounting_versions_current,
            data_through_ms,
            quality,
            scope_count,
            replay_staging: replay_staging_count == 1,
            aggregate,
        },
        quota,
        benefit,
        git,
    })
}

fn load_status_scan_truth(
    connection: &rusqlite::Connection,
    scan_set_id: Option<i64>,
) -> Result<(Option<i64>, usize), StoreError> {
    let Some(scan_set_id) = scan_set_id else {
        return Ok((None, 0));
    };
    let raw: (Option<i64>, String, i64, i64) = map_sql(connection.query_row(
        "SELECT scan_set.completed_at_ms, scan_set.completion_state,
                scan_set.expected_scope_count,
                (SELECT count(*) FROM usage_scan AS scan
                 WHERE scan.scan_set_id = scan_set.scan_set_id
                   AND scan.completion_state = 'complete')
         FROM usage_scan_set AS scan_set WHERE scan_set.scan_set_id = ?1",
        [scan_set_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ))?;
    let expected = usize::try_from(raw.2)
        .ok()
        .filter(|count| *count <= crate::MAX_SCAN_SCOPES)
        .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let completed =
        usize::try_from(raw.3).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
    let completed_at_ms = positive_time(raw.0)?;
    if raw.1 != "complete" || completed_at_ms.is_none() || completed != expected {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok((completed_at_ms, expected))
}

fn dataset_generation(identity: UsageQueryDatasetIdentity) -> Result<u64, StoreError> {
    match identity {
        UsageQueryDatasetIdentity::ReplayRevision {
            dataset_generation, ..
        } => Ok(dataset_generation),
        UsageQueryDatasetIdentity::Empty | UsageQueryDatasetIdentity::LegacySnapshotV1 => Ok(0),
    }
}

fn load_aggregate_status(
    connection: &rusqlite::Connection,
    dataset_generation: u64,
) -> Result<ProductAggregateStatus, StoreError> {
    let raw: (
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        Option<i64>,
        Option<String>,
    ) = map_sql(connection.query_row(
        "SELECT state, expected_dataset_generation, active_aggregate_generation,
                    current_event_count, legacy_event_count, rebuild_processed_events,
                    rebuild_total_events, rebuild_aggregate_generation, failure_code
             FROM usage_aggregate_state WHERE singleton_id = 1",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        },
    ))?;
    let expected_dataset_generation = nonnegative(raw.1)?;
    let active_generation = nonnegative(raw.2)?;
    let current_event_count = nonnegative(raw.3)?;
    let legacy_event_count = nonnegative(raw.4)?;
    let processed_events = nonnegative(raw.5)?;
    let total_events = nonnegative(raw.6)?;
    if expected_dataset_generation != dataset_generation || processed_events > total_events {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    let (state, progress) = match raw.0.as_str() {
        "ready" if raw.7.is_none() && raw.8.is_none() && processed_events == 0 => {
            (ProductAggregateState::Ready, None)
        }
        "rebuild_required" if raw.7.is_none() && raw.8.is_none() && processed_events == 0 => {
            (ProductAggregateState::RebuildRequired, None)
        }
        "rebuilding" if raw.7.is_some() && raw.8.is_none() => (
            ProductAggregateState::Rebuilding,
            Some(ProductAggregateProgress {
                processed_events,
                total_events,
            }),
        ),
        "failed" if raw.8.is_some() => (ProductAggregateState::Failed, None),
        _ => return Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    };
    Ok(ProductAggregateStatus {
        state,
        expected_dataset_generation,
        active_generation,
        current_event_count,
        legacy_event_count,
        progress,
    })
}

fn load_quota_status(connection: &rusqlite::Connection) -> Result<ProductQuotaStatus, StoreError> {
    let raw: (i64, i64, i64, i64, Option<i64>) = map_sql(connection.query_row(
        "SELECT revision, retained_sample_count, retained_epoch_count,
                retained_transition_count, last_published_at_ms
         FROM quota_state WHERE singleton_id = 1",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    ))?;
    let status = ProductQuotaStatus {
        revision: nonnegative(raw.0)?,
        retained_sample_count: nonnegative(raw.1)?,
        retained_epoch_count: nonnegative(raw.2)?,
        retained_transition_count: nonnegative(raw.3)?,
        last_published_at_ms: positive_time(raw.4)?,
    };
    validate_publication_time(status.revision, status.last_published_at_ms)?;
    Ok(status)
}

fn load_benefit_status(
    connection: &rusqlite::Connection,
) -> Result<ProductBenefitStatus, StoreError> {
    let raw: (i64, i64, i64, i64, i64, Option<i64>) = map_sql(connection.query_row(
        "SELECT revision, current_lot_count, retained_change_count, pending_due_count,
                retained_delivery_count, last_published_at_ms
         FROM benefit_state WHERE singleton_id = 1",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        },
    ))?;
    let status = ProductBenefitStatus {
        revision: nonnegative(raw.0)?,
        current_lot_count: nonnegative(raw.1)?,
        retained_change_count: nonnegative(raw.2)?,
        pending_due_count: nonnegative(raw.3)?,
        retained_delivery_count: nonnegative(raw.4)?,
        last_published_at_ms: positive_time(raw.5)?,
    };
    if status.current_lot_count > 64 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    validate_publication_time(status.revision, status.last_published_at_ms)?;
    Ok(status)
}

fn load_git_status(connection: &rusqlite::Connection) -> Result<ProductGitStatus, StoreError> {
    let raw: (i64, i64, i64, Option<i64>) = map_sql(connection.query_row(
        "SELECT publication_revision, repository_count, association_count,
                last_published_at_ms
         FROM git_installation_state WHERE singleton_id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ))?;
    let status = ProductGitStatus {
        publication_revision: nonnegative(raw.0)?,
        repository_count: nonnegative(raw.1)?,
        association_count: nonnegative(raw.2)?,
        last_published_at_ms: positive_time(raw.3)?,
    };
    if status.repository_count > 32 || status.association_count > 4_096 {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    validate_publication_time(status.publication_revision, status.last_published_at_ms)?;
    Ok(status)
}

fn validate_publication_time(
    revision: u64,
    published_at_ms: Option<i64>,
) -> Result<(), StoreError> {
    if (revision == 0) != published_at_ms.is_none() {
        return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
    }
    Ok(())
}

fn positive_time(value: Option<i64>) -> Result<Option<i64>, StoreError> {
    value
        .map(|value| {
            if value > 0 {
                Ok(value)
            } else {
                Err(StoreError::new(StoreErrorCode::InvalidStoredValue))
            }
        })
        .transpose()
}

fn nonnegative(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;
    use crate::UsageStore;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn capture_keeps_one_transaction_when_an_independent_revision_commits_mid_read() -> TestResult {
        let directory = TempDir::new()?;
        let path = directory.path().join("status-snapshot.sqlite3");
        drop(UsageStore::open(&path)?);
        let mut reader = UsageReadStore::open(&path)?;
        let writer = Connection::open(&path)?;

        let capture = reader.capture_product_data_status_with_options(
            ProductDataStatusQuery::new(Duration::from_secs(2))?,
            PROGRESS_OP_INTERVAL,
            false,
            || {
                writer
                    .execute(
                        "UPDATE quota_state
                             SET revision = 1, last_published_at_ms = 1800000000000
                             WHERE singleton_id = 1",
                        [],
                    )
                    .map_err(|_| StoreError::new(StoreErrorCode::Database))?;
                Ok(())
            },
        )?;
        assert_eq!(capture.quota().revision(), 0);

        let next = reader
            .capture_product_data_status(ProductDataStatusQuery::new(Duration::from_secs(2))?)?;
        assert_eq!(next.quota().revision(), 1);
        Ok(())
    }
}
