use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokenmaster_domain::{
    GitActivityAssociationId, GitOutputQuality, GitOutputUnavailableReason, GitOutputWarning,
    GitRepositoryId,
};
use tokenmaster_engine::{Clock, RefreshOutcome, RefreshPermit, WriterLease};
use tokenmaster_git::{
    GitBackendErrorCode, GitCancellation, GitIdentitySalt, GitProcess, GitRefreshKind,
    GitRepositoryCandidate, GitRepositoryRefresh, GitRunControl, derive_activity_association_id,
    derive_project_fingerprint,
};
use tokenmaster_provider::RepositoryActivityHint;
use tokenmaster_store::{
    GitCacheIdentity, GitIncrementalAuthority, GitProjectKey, GitProjectionInput,
    GitProjectionInputParts, GitRefreshInput, GitRefreshInputParts, StoreError, StoreErrorCode,
    UsageStore,
};

use super::runtime::{GitFrontierRecord, GitHintState};
use super::{GitPublicationErrorCode, GitRefreshFailure, GitRefreshSnapshot, GitRuntimeConfig};
use crate::RuntimeWriterLease;

const GIT_CATEGORY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone)]
struct WorkItem {
    hint: RepositoryActivityHint,
    sequence: u64,
    previous: Option<GitFrontierRecord>,
}

struct StagedRefresh {
    work: WorkItem,
    refresh: GitRepositoryRefresh,
}

struct StagedUnavailable {
    work: WorkItem,
    repository_id: GitRepositoryId,
    reason: GitOutputUnavailableReason,
}

enum StagedResult {
    Refreshed(Box<StagedRefresh>),
    Unavailable(Box<StagedUnavailable>),
}

impl StagedResult {
    const fn work(&self) -> &WorkItem {
        match self {
            Self::Refreshed(staged) => &staged.work,
            Self::Unavailable(staged) => &staged.work,
        }
    }
}

#[derive(Default)]
struct AttemptDelta {
    scanned: u64,
    published: u64,
    rebuild: u64,
    append: u64,
    unchanged: u64,
    partial: u64,
    unavailable: u64,
    cancelled: u64,
    stale: u64,
}

pub(super) struct GitExecution {
    config: GitRuntimeConfig,
    salt: GitIdentitySalt,
    lease: RuntimeWriterLease,
    clock: Arc<dyn Clock>,
    hints: Arc<Mutex<GitHintState>>,
    latest: Arc<Mutex<GitRefreshSnapshot>>,
}

impl GitExecution {
    pub(super) const fn new(
        config: GitRuntimeConfig,
        salt: GitIdentitySalt,
        lease: RuntimeWriterLease,
        clock: Arc<dyn Clock>,
        hints: Arc<Mutex<GitHintState>>,
        latest: Arc<Mutex<GitRefreshSnapshot>>,
    ) -> Self {
        Self {
            config,
            salt,
            lease,
            clock,
            hints,
            latest,
        }
    }

    pub(super) fn run(&mut self, permit: &RefreshPermit) -> RefreshOutcome {
        let started_at = self.clock.now().as_millis();
        let work = match self.snapshot_work() {
            Ok(work) => work,
            Err(failure) => {
                return self.finish(
                    started_at,
                    RefreshOutcome::Failed,
                    Some(failure),
                    AttemptDelta::default(),
                );
            }
        };
        if work.is_empty() {
            return self.finish(
                started_at,
                RefreshOutcome::Completed,
                None,
                AttemptDelta::default(),
            );
        }
        let executable = match self.config.resolve_executable() {
            Ok(executable) => executable,
            Err(error) => {
                let delta = AttemptDelta {
                    unavailable: u64::try_from(work.len()).unwrap_or(u64::MAX),
                    ..AttemptDelta::default()
                };
                return self.finish(
                    started_at,
                    RefreshOutcome::Failed,
                    Some(GitRefreshFailure::Git(error.code())),
                    delta,
                );
            }
        };

        let cancellation_token = permit.cancellation_token();
        let cancellation = GitCancellation::linked(move || cancellation_token.is_cancelled());
        let control = match GitRunControl::new(self.config.scan_timeout(), cancellation) {
            Ok(control) => control,
            Err(error) => {
                return self.finish(
                    started_at,
                    RefreshOutcome::Failed,
                    Some(GitRefreshFailure::Git(error.code())),
                    AttemptDelta::default(),
                );
            }
        };
        let process = GitProcess::new(executable, control);
        let mut staged = Vec::with_capacity(work.len());
        let mut delta = AttemptDelta::default();
        let mut failure = None;
        for item in work {
            if permit.is_cancelled() {
                delta.cancelled = delta.cancelled.saturating_add(1);
                return self.finish(
                    started_at,
                    RefreshOutcome::Cancelled,
                    Some(GitRefreshFailure::Git(GitBackendErrorCode::Cancelled)),
                    delta,
                );
            }
            let candidate =
                match GitRepositoryCandidate::new(item.hint.candidate().as_path().to_path_buf()) {
                    Ok(candidate) => candidate,
                    Err(error) => {
                        delta.unavailable = delta.unavailable.saturating_add(1);
                        failure = Some(GitRefreshFailure::Git(error.code()));
                        continue;
                    }
                };
            match process.refresh(
                &candidate,
                self.salt,
                item.previous.as_ref().map(|previous| &previous.frontier),
            ) {
                Ok(refresh) => {
                    delta.scanned = delta.scanned.saturating_add(1);
                    staged.push(StagedResult::Refreshed(Box::new(StagedRefresh {
                        work: item,
                        refresh,
                    })));
                }
                Err(error) if error.code() == GitBackendErrorCode::Cancelled => {
                    delta.cancelled = delta.cancelled.saturating_add(1);
                    return self.finish(
                        started_at,
                        RefreshOutcome::Cancelled,
                        Some(GitRefreshFailure::Git(error.code())),
                        delta,
                    );
                }
                Err(error) => {
                    delta.unavailable = delta.unavailable.saturating_add(1);
                    failure = Some(GitRefreshFailure::Git(error.code()));
                    if let Some(reason) = unavailable_reason(error.code())
                        && let Ok(repository_id) =
                            process.identify_repository(&candidate, self.salt)
                    {
                        staged.push(StagedResult::Unavailable(Box::new(StagedUnavailable {
                            work: item,
                            repository_id,
                            reason,
                        })));
                    }
                }
            }
        }
        if permit.is_cancelled() {
            delta.cancelled = delta.cancelled.saturating_add(1);
            return self.finish(
                started_at,
                RefreshOutcome::Cancelled,
                Some(GitRefreshFailure::Git(GitBackendErrorCode::Cancelled)),
                delta,
            );
        }
        if staged.is_empty() {
            return self.finish(started_at, RefreshOutcome::Failed, failure, delta);
        }

        let _guard = match self.lease.try_acquire() {
            Ok(guard) => guard,
            Err(error) => {
                let publication = match error.code() {
                    tokenmaster_engine::PortErrorCode::Busy => GitPublicationErrorCode::Busy,
                    _ => GitPublicationErrorCode::StoreUnavailable,
                };
                let outcome = if publication == GitPublicationErrorCode::Busy {
                    RefreshOutcome::Busy
                } else {
                    RefreshOutcome::Failed
                };
                return self.finish(
                    started_at,
                    outcome,
                    Some(GitRefreshFailure::Publication(publication)),
                    delta,
                );
            }
        };
        let mut store = match UsageStore::open(self.config.archive_path()) {
            Ok(store) => store,
            Err(_) => {
                return self.finish(
                    started_at,
                    RefreshOutcome::Failed,
                    Some(GitRefreshFailure::Publication(
                        GitPublicationErrorCode::StoreUnavailable,
                    )),
                    delta,
                );
            }
        };
        let mut hints = match self.hints.lock() {
            Ok(hints) => hints,
            Err(_) => {
                return self.finish(
                    started_at,
                    RefreshOutcome::Failed,
                    Some(GitRefreshFailure::Control),
                    delta,
                );
            }
        };
        if !hints.accepting || permit.is_cancelled() {
            delta.cancelled = delta.cancelled.saturating_add(1);
            drop(hints);
            return self.finish(
                started_at,
                RefreshOutcome::Cancelled,
                Some(GitRefreshFailure::Git(GitBackendErrorCode::Cancelled)),
                delta,
            );
        }
        for staged_result in staged {
            let work = staged_result.work();
            let position = hints.slots.iter().position(|slot| {
                slot.sequence == work.sequence
                    && slot.hint.candidate().as_path() == work.hint.candidate().as_path()
            });
            let Some(position) = position else {
                delta.stale = delta.stale.saturating_add(1);
                continue;
            };
            let result = match &staged_result {
                StagedResult::Refreshed(staged) => publish(&mut store, &self.salt, staged),
                StagedResult::Unavailable(staged) => {
                    publish_unavailable(&mut store, &self.salt, staged)
                }
            };
            match result {
                Ok(publication) => {
                    delta.published = delta.published.saturating_add(1);
                    match staged_result {
                        StagedResult::Refreshed(staged) => {
                            let slot = &mut hints.slots[position];
                            slot.frontier = Some(GitFrontierRecord {
                                frontier: staged.refresh.frontier().clone(),
                                scan_revision: publication.scan_revision(),
                            });
                            match staged.refresh.kind() {
                                GitRefreshKind::Rebuild => {
                                    delta.rebuild = delta.rebuild.saturating_add(1);
                                }
                                GitRefreshKind::Append => {
                                    delta.append = delta.append.saturating_add(1);
                                }
                                GitRefreshKind::Unchanged => {
                                    delta.unchanged = delta.unchanged.saturating_add(1);
                                }
                            }
                            if !projection_warnings(&staged.refresh).is_empty() {
                                delta.partial = delta.partial.saturating_add(1);
                            }
                        }
                        StagedResult::Unavailable(_) => {
                            hints.slots[position].frontier = None;
                        }
                    }
                }
                Err(error) => {
                    let code = publish_error_code(&error);
                    if code == GitPublicationErrorCode::Stale {
                        hints.slots[position].frontier = None;
                        delta.stale = delta.stale.saturating_add(1);
                    }
                    failure = Some(GitRefreshFailure::Publication(code));
                }
            }
        }
        drop(hints);
        let outcome = if failure.is_some() {
            RefreshOutcome::Failed
        } else {
            RefreshOutcome::Completed
        };
        self.finish(started_at, outcome, failure, delta)
    }

    fn snapshot_work(&self) -> Result<Vec<WorkItem>, GitRefreshFailure> {
        let hints = self.hints.lock().map_err(|_| GitRefreshFailure::Control)?;
        if !hints.accepting {
            return Ok(Vec::new());
        }
        Ok(hints
            .slots
            .iter()
            .map(|slot| WorkItem {
                hint: slot.hint.clone(),
                sequence: slot.sequence,
                previous: slot.frontier.clone(),
            })
            .collect())
    }

    fn finish(
        &self,
        started_at: u64,
        outcome: RefreshOutcome,
        failure: Option<GitRefreshFailure>,
        delta: AttemptDelta,
    ) -> RefreshOutcome {
        let elapsed_millis = self.clock.now().as_millis().saturating_sub(started_at);
        let Ok(mut latest) = self.latest.lock() else {
            return RefreshOutcome::Failed;
        };
        let Some(attempt_sequence) = latest.attempt_sequence.checked_add(1) else {
            return RefreshOutcome::Failed;
        };
        *latest = GitRefreshSnapshot {
            attempt_sequence,
            outcome: Some(outcome),
            failure,
            scanned_count: latest.scanned_count.saturating_add(delta.scanned),
            published_count: latest.published_count.saturating_add(delta.published),
            rebuild_count: latest.rebuild_count.saturating_add(delta.rebuild),
            append_count: latest.append_count.saturating_add(delta.append),
            unchanged_count: latest.unchanged_count.saturating_add(delta.unchanged),
            partial_count: latest.partial_count.saturating_add(delta.partial),
            unavailable_count: latest.unavailable_count.saturating_add(delta.unavailable),
            cancelled_count: latest.cancelled_count.saturating_add(delta.cancelled),
            stale_count: latest.stale_count.saturating_add(delta.stale),
            elapsed_millis,
        };
        outcome
    }
}

fn publish(
    store: &mut UsageStore,
    salt: &GitIdentitySalt,
    staged: &StagedRefresh,
) -> Result<tokenmaster_store::GitPublication, PublishError> {
    let context = publication_context(salt, &staged.work.hint)?;
    let frontier = staged.refresh.frontier();
    let cache = GitCacheIdentity::new(
        frontier.object_format(),
        frontier.ref_fingerprint(),
        frontier.mailmap_fingerprint(),
        frontier.author_fingerprint(),
        GIT_CATEGORY_SCHEMA_VERSION,
        frontier.is_shallow(),
    )?;
    match staged.refresh.kind() {
        GitRefreshKind::Rebuild | GitRefreshKind::Append => {
            let warnings = projection_warnings(&staged.refresh);
            let quality = if warnings.is_empty() {
                GitOutputQuality::Complete
            } else {
                GitOutputQuality::Partial
            };
            let input = GitProjectionInput::new(GitProjectionInputParts {
                repository_id: frontier.repository_id(),
                association_id: context.association_id,
                project_key: context.project_key,
                activity_at_ms: context.activity_at_ms,
                observed_at_ms: context.observed_at_ms,
                data_through_ms: Some(context.observed_at_ms),
                quality,
                unavailable_reason: None,
                warnings,
                summary: staged.refresh.summary().cloned(),
                cache: Some(cache),
            })?;
            if staged.refresh.kind() == GitRefreshKind::Rebuild {
                Ok(store.publish_git_rebuild(&input)?)
            } else {
                let previous = staged
                    .work
                    .previous
                    .as_ref()
                    .ok_or(PublishError::InvalidData)?;
                let authority = GitIncrementalAuthority::new(
                    previous.frontier.repository_id(),
                    previous.scan_revision,
                    previous.frontier.ref_fingerprint(),
                )?;
                Ok(store.publish_git_append(authority, &input)?)
            }
        }
        GitRefreshKind::Unchanged => {
            let previous = staged
                .work
                .previous
                .as_ref()
                .ok_or(PublishError::InvalidData)?;
            let authority = GitIncrementalAuthority::new(
                previous.frontier.repository_id(),
                previous.scan_revision,
                previous.frontier.ref_fingerprint(),
            )?;
            let input = GitRefreshInput::new(GitRefreshInputParts {
                authority,
                association_id: context.association_id,
                project_key: context.project_key,
                activity_at_ms: context.activity_at_ms,
                observed_at_ms: context.observed_at_ms,
                cache,
            })?;
            Ok(store.refresh_git_unchanged(&input)?)
        }
    }
}

fn publish_unavailable(
    store: &mut UsageStore,
    salt: &GitIdentitySalt,
    staged: &StagedUnavailable,
) -> Result<tokenmaster_store::GitPublication, PublishError> {
    let context = publication_context(salt, &staged.work.hint)?;
    if let Some(previous) = staged
        .work
        .previous
        .as_ref()
        .filter(|previous| previous.frontier.repository_id() == staged.repository_id)
    {
        let authority = GitIncrementalAuthority::new(
            previous.frontier.repository_id(),
            previous.scan_revision,
            previous.frontier.ref_fingerprint(),
        )?;
        return Ok(store.mark_git_rebuild_required(authority, context.observed_at_ms)?);
    }
    let input = GitProjectionInput::new(GitProjectionInputParts {
        repository_id: staged.repository_id,
        association_id: context.association_id,
        project_key: context.project_key,
        activity_at_ms: context.activity_at_ms,
        observed_at_ms: context.observed_at_ms,
        data_through_ms: None,
        quality: GitOutputQuality::Unavailable,
        unavailable_reason: Some(staged.reason),
        warnings: Vec::new(),
        summary: None,
        cache: None,
    })?;
    Ok(store.publish_git_rebuild(&input)?)
}

#[derive(Clone, Copy)]
struct PublicationContext {
    association_id: GitActivityAssociationId,
    project_key: Option<GitProjectKey>,
    activity_at_ms: i64,
    observed_at_ms: i64,
}

fn publication_context(
    salt: &GitIdentitySalt,
    hint: &RepositoryActivityHint,
) -> Result<PublicationContext, PublishError> {
    let activity_at_ms = activity_millis(hint).ok_or(PublishError::InvalidData)?;
    let observed_at_ms = wall_millis()
        .ok_or(PublishError::InvalidData)?
        .max(activity_at_ms);
    let association_id = derive_activity_association_id(
        salt,
        hint.provider_id(),
        hint.profile_id(),
        hint.source_id(),
        hint.session_id(),
    )
    .map_err(|_| PublishError::InvalidData)?;
    let project_key = hint
        .project()
        .map(|project| {
            derive_project_fingerprint(salt, project)
                .map(|value| GitProjectKey::from_bytes(*value.as_bytes()))
                .map_err(|_| PublishError::InvalidData)
        })
        .transpose()?;
    Ok(PublicationContext {
        association_id,
        project_key,
        activity_at_ms,
        observed_at_ms,
    })
}

fn projection_warnings(refresh: &GitRepositoryRefresh) -> Vec<GitOutputWarning> {
    let mut warnings = Vec::with_capacity(4);
    if refresh.frontier().is_shallow() {
        warnings.push(GitOutputWarning::ShallowHistory);
    }
    if let Some(summary) = refresh.summary() {
        if summary.totals().binary_files() > 0 {
            warnings.push(GitOutputWarning::BinaryFilesOmitted);
        }
        if summary.totals().submodule_changes() > 0 {
            warnings.push(GitOutputWarning::SubmoduleLinesOmitted);
        }
        if summary.daily_history_truncated() {
            warnings.push(GitOutputWarning::DailyHistoryTruncated);
        }
    }
    warnings
}

fn activity_millis(hint: &RepositoryActivityHint) -> Option<i64> {
    hint.observed_at()
        .unix_seconds()
        .checked_mul(1_000)?
        .checked_add(i64::from(hint.observed_at().subsec_nanos() / 1_000_000))
        .filter(|value| *value > 0)
}

fn wall_millis() -> Option<i64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    i64::try_from(millis).ok().filter(|value| *value > 0)
}

enum PublishError {
    Store(StoreError),
    InvalidData,
}

impl From<StoreError> for PublishError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

fn unavailable_reason(code: GitBackendErrorCode) -> Option<GitOutputUnavailableReason> {
    match code {
        GitBackendErrorCode::AuthorIdentityMissing => {
            Some(GitOutputUnavailableReason::AuthorIdentityMissing)
        }
        GitBackendErrorCode::Cancelled => None,
        GitBackendErrorCode::CapacityExceeded => {
            Some(GitOutputUnavailableReason::HistoryLimitExceeded)
        }
        GitBackendErrorCode::DeadlineExceeded | GitBackendErrorCode::InvalidTime => {
            Some(GitOutputUnavailableReason::DeadlineExceeded)
        }
        GitBackendErrorCode::HistoryChangedDuringScan => {
            Some(GitOutputUnavailableReason::HistoryChangedDuringScan)
        }
        GitBackendErrorCode::InvalidExecutable => Some(GitOutputUnavailableReason::GitNotNative),
        GitBackendErrorCode::ProcessCleanupFailed
        | GitBackendErrorCode::ProcessFailed
        | GitBackendErrorCode::ProtocolError
        | GitBackendErrorCode::SpawnFailed
        | GitBackendErrorCode::StderrLimitExceeded => {
            Some(GitOutputUnavailableReason::ProcessFailed)
        }
        GitBackendErrorCode::RepositoryNotFound => {
            Some(GitOutputUnavailableReason::RepositoryNotFound)
        }
        GitBackendErrorCode::RepositoryPathRejected => {
            Some(GitOutputUnavailableReason::RepositoryPathRejected)
        }
        GitBackendErrorCode::StdoutLimitExceeded => {
            Some(GitOutputUnavailableReason::OutputLimitExceeded)
        }
        GitBackendErrorCode::TooManyRefs => Some(GitOutputUnavailableReason::TooManyRefs),
        GitBackendErrorCode::Unavailable => Some(GitOutputUnavailableReason::GitNotFound),
        GitBackendErrorCode::UnsupportedObjectFormat => {
            Some(GitOutputUnavailableReason::UnsupportedObjectFormat)
        }
        GitBackendErrorCode::UnsupportedVersion => {
            Some(GitOutputUnavailableReason::UnsupportedGitVersion)
        }
    }
}

fn publish_error_code(error: &PublishError) -> GitPublicationErrorCode {
    match error {
        PublishError::Store(error) => map_store_error(error),
        PublishError::InvalidData => GitPublicationErrorCode::InvalidData,
    }
}

fn map_store_error(error: &StoreError) -> GitPublicationErrorCode {
    match error.code() {
        StoreErrorCode::StaleRevision
        | StoreErrorCode::StaleCheckpoint
        | StoreErrorCode::StaleScan
        | StoreErrorCode::PendingScan
        | StoreErrorCode::PendingContinuation => GitPublicationErrorCode::Stale,
        StoreErrorCode::CapacityExceeded => GitPublicationErrorCode::CapacityExceeded,
        StoreErrorCode::ScanInProgress => GitPublicationErrorCode::Busy,
        StoreErrorCode::InvalidValue
        | StoreErrorCode::InvalidStoredValue
        | StoreErrorCode::AccountingVersionMismatch
        | StoreErrorCode::IncompleteManifest
        | StoreErrorCode::UnsealedRevision
        | StoreErrorCode::ArchiveModeMismatch => GitPublicationErrorCode::InvalidData,
        StoreErrorCode::DeadlineExceeded
        | StoreErrorCode::RebuildRequired
        | StoreErrorCode::Database
        | StoreErrorCode::VersionMismatch
        | StoreErrorCode::SchemaTooNew
        | StoreErrorCode::SchemaMismatch
        | StoreErrorCode::PolicyMismatch => GitPublicationErrorCode::StoreUnavailable,
    }
}
