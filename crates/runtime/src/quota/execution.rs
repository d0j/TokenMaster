use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokenmaster_codex::{
    CodexQuotaErrorCode, CodexQuotaSnapshot, CodexQuotaTransport, MAX_CODEX_QUOTA_WINDOWS,
};
use tokenmaster_engine::{
    Clock, OperationControl, PortErrorCode, RefreshOutcome, RefreshPermit, WriterLease,
};
use tokenmaster_store::{BenefitApplyStatus, QuotaApplyStatus, StoreErrorCode, UsageStore};

use super::{
    CodexExecutableDiscoveryErrorCode, CodexQuotaClockErrorCode, CodexQuotaPublicationErrorCode,
    CodexQuotaRefreshFailure, CodexQuotaRefreshSnapshot, CodexQuotaRetryMode,
    CodexQuotaRuntimeConfig,
};
use crate::{RuntimeError, RuntimeWriterLease};

pub(super) trait CodexQuotaWallClock: Send + Sync + 'static {
    fn now_millis(&self) -> Result<i64, CodexQuotaClockErrorCode>;
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SystemCodexQuotaWallClock;

impl CodexQuotaWallClock for SystemCodexQuotaWallClock {
    fn now_millis(&self) -> Result<i64, CodexQuotaClockErrorCode> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| CodexQuotaClockErrorCode::Unavailable)?
            .as_millis();
        let millis = i64::try_from(millis).map_err(|_| CodexQuotaClockErrorCode::InvalidTime)?;
        if millis <= 0 {
            return Err(CodexQuotaClockErrorCode::InvalidTime);
        }
        Ok(millis)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CodexQuotaSourceFailure {
    Discovery(CodexExecutableDiscoveryErrorCode),
    Transport(CodexQuotaErrorCode),
}

pub(super) trait CodexQuotaSource: Send + 'static {
    fn poll(&mut self, observed_at_ms: i64) -> Result<CodexQuotaSnapshot, CodexQuotaSourceFailure>;
}

pub(super) struct RuntimeCodexQuotaSource {
    config: CodexQuotaRuntimeConfig,
}

impl RuntimeCodexQuotaSource {
    pub(super) const fn new(config: CodexQuotaRuntimeConfig) -> Self {
        Self { config }
    }
}

impl CodexQuotaSource for RuntimeCodexQuotaSource {
    fn poll(&mut self, observed_at_ms: i64) -> Result<CodexQuotaSnapshot, CodexQuotaSourceFailure> {
        let command = self
            .config
            .resolve_current_command()
            .map_err(|error| CodexQuotaSourceFailure::Discovery(error.code()))?;
        let transport = CodexQuotaTransport::new(command, self.config.transport_timeout())
            .map_err(|error| CodexQuotaSourceFailure::Transport(error.code()))?;
        transport
            .poll(observed_at_ms)
            .map_err(|error| CodexQuotaSourceFailure::Transport(error.code()))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct QuotaPublicationSummary {
    processed_count: u16,
    changed_count: u16,
    started_count: u16,
    advanced_count: u16,
    duplicate_count: u16,
    stale_count: u16,
    allowance_change_count: u16,
    reset_count: u16,
}

impl QuotaPublicationSummary {
    fn record(&mut self, status: QuotaApplyStatus) -> Result<(), CodexQuotaPublicationErrorCode> {
        increment(&mut self.processed_count)?;
        match status {
            QuotaApplyStatus::Started => {
                increment(&mut self.changed_count)?;
                increment(&mut self.started_count)
            }
            QuotaApplyStatus::Advanced => {
                increment(&mut self.changed_count)?;
                increment(&mut self.advanced_count)
            }
            QuotaApplyStatus::Duplicate => increment(&mut self.duplicate_count),
            QuotaApplyStatus::Stale => increment(&mut self.stale_count),
            QuotaApplyStatus::AllowanceChanged => {
                increment(&mut self.changed_count)?;
                increment(&mut self.allowance_change_count)
            }
            QuotaApplyStatus::Reset => {
                increment(&mut self.changed_count)?;
                increment(&mut self.reset_count)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct BenefitPublicationSummary {
    observation_count: u8,
    processed_count: u8,
    changed_count: u8,
    freshness_only_count: u8,
    duplicate_count: u8,
    stale_count: u8,
    lot_change_count: u16,
    pending_due_count: u16,
}

impl BenefitPublicationSummary {
    fn record(
        &mut self,
        status: BenefitApplyStatus,
        lot_change_count: u16,
        pending_due_count: u16,
    ) -> Result<(), CodexQuotaPublicationErrorCode> {
        increment_u8(&mut self.processed_count)?;
        match status {
            BenefitApplyStatus::Duplicate => increment_u8(&mut self.duplicate_count)?,
            BenefitApplyStatus::Stale => increment_u8(&mut self.stale_count)?,
            BenefitApplyStatus::FreshnessOnly => {
                increment_u8(&mut self.freshness_only_count)?;
            }
            BenefitApplyStatus::Changed => increment_u8(&mut self.changed_count)?,
        }
        self.lot_change_count = lot_change_count;
        self.pending_due_count = pending_due_count;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct QuotaPublicationReport {
    quota: QuotaPublicationSummary,
    benefit: BenefitPublicationSummary,
    quota_failure: Option<CodexQuotaPublicationErrorCode>,
    benefit_failure: Option<CodexQuotaPublicationErrorCode>,
}

impl QuotaPublicationReport {
    fn for_snapshot(snapshot: &CodexQuotaSnapshot) -> Self {
        Self {
            benefit: BenefitPublicationSummary {
                observation_count: u8::from(snapshot.benefit_observation().is_some()),
                ..BenefitPublicationSummary::default()
            },
            ..Self::default()
        }
    }

    const fn has_domain_failure(self) -> bool {
        self.quota_failure.is_some() || self.benefit_failure.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct QuotaPublicationError {
    code: CodexQuotaPublicationErrorCode,
    report: QuotaPublicationReport,
}

impl QuotaPublicationError {
    const fn with_report(
        code: CodexQuotaPublicationErrorCode,
        report: QuotaPublicationReport,
    ) -> Self {
        Self { code, report }
    }

    pub(super) const fn code(self) -> CodexQuotaPublicationErrorCode {
        self.code
    }

    pub(super) const fn report(self) -> QuotaPublicationReport {
        self.report
    }
}

pub(super) trait QuotaPublisher: Send + 'static {
    fn publish(
        &mut self,
        snapshot: &CodexQuotaSnapshot,
        control: &OperationControl<'_>,
    ) -> Result<QuotaPublicationReport, QuotaPublicationError>;
}

pub(super) struct StoreQuotaPublisher {
    archive_path: PathBuf,
    lease: RuntimeWriterLease,
}

impl StoreQuotaPublisher {
    pub(super) fn new(archive_path: &Path) -> Result<Self, RuntimeError> {
        Ok(Self {
            archive_path: archive_path.to_path_buf(),
            lease: RuntimeWriterLease::new(archive_path)?,
        })
    }
}

impl QuotaPublisher for StoreQuotaPublisher {
    fn publish(
        &mut self,
        snapshot: &CodexQuotaSnapshot,
        control: &OperationControl<'_>,
    ) -> Result<QuotaPublicationReport, QuotaPublicationError> {
        let mut report = QuotaPublicationReport::for_snapshot(snapshot);
        control
            .check()
            .map_err(|error| control_publication_error(error.code(), report))?;
        let guard = self
            .lease
            .try_acquire()
            .map_err(|error| lease_publication_error(error.code(), report))?;
        let mut store = UsageStore::open(&self.archive_path)
            .map_err(|error| store_publication_error(error.code(), report))?;
        for observation in snapshot.observations() {
            control
                .check()
                .map_err(|error| control_publication_error(error.code(), report))?;
            match store.apply_quota_observation(observation.definition(), observation.sample()) {
                Ok(result) => {
                    report
                        .quota
                        .record(result.status())
                        .map_err(|code| QuotaPublicationError::with_report(code, report))?;
                }
                Err(error) => {
                    let code = store_publication_code(error.code());
                    if matches!(
                        code,
                        CodexQuotaPublicationErrorCode::Cancelled
                            | CodexQuotaPublicationErrorCode::DeadlineExceeded
                    ) {
                        return Err(QuotaPublicationError::with_report(code, report));
                    }
                    report.quota_failure = Some(code);
                    break;
                }
            }
        }
        control
            .check()
            .map_err(|error| control_publication_error(error.code(), report))?;
        if let Some(observation) = snapshot.benefit_observation() {
            match store.apply_benefit_observation(observation) {
                Ok(result) => {
                    report
                        .benefit
                        .record(
                            result.status(),
                            result.change_count(),
                            result.pending_due_count(),
                        )
                        .map_err(|code| QuotaPublicationError::with_report(code, report))?;
                }
                Err(error) => {
                    let code = store_publication_code(error.code());
                    if matches!(
                        code,
                        CodexQuotaPublicationErrorCode::Cancelled
                            | CodexQuotaPublicationErrorCode::DeadlineExceeded
                    ) {
                        return Err(QuotaPublicationError::with_report(code, report));
                    }
                    report.benefit_failure = Some(code);
                }
            }
        }
        control
            .check()
            .map_err(|error| control_publication_error(error.code(), report))?;
        drop(store);
        drop(guard);
        Ok(report)
    }
}

pub(super) struct CodexQuotaExecution<C, S, P>
where
    C: CodexQuotaWallClock,
    S: CodexQuotaSource,
    P: QuotaPublisher,
{
    monotonic_clock: Arc<dyn Clock>,
    wall_clock: C,
    source: S,
    publisher: P,
    latest: Arc<Mutex<CodexQuotaRefreshSnapshot>>,
}

impl<C, S, P> CodexQuotaExecution<C, S, P>
where
    C: CodexQuotaWallClock,
    S: CodexQuotaSource,
    P: QuotaPublisher,
{
    pub(super) fn new(
        monotonic_clock: Arc<dyn Clock>,
        wall_clock: C,
        source: S,
        publisher: P,
        latest: Arc<Mutex<CodexQuotaRefreshSnapshot>>,
    ) -> Self {
        Self {
            monotonic_clock,
            wall_clock,
            source,
            publisher,
            latest,
        }
    }

    pub(super) fn run(&mut self, permit: &RefreshPermit) -> RefreshOutcome {
        let started_at = self.monotonic_clock.now().as_millis();
        let control = OperationControl::new(permit, self.monotonic_clock.as_ref());
        if let Err(error) = control.check() {
            return self.finish(
                started_at,
                AttemptResult::control(
                    error.code(),
                    QuotaPublicationReport::default(),
                    0,
                    None,
                    false,
                ),
            );
        }
        let observed_at_ms = match self.wall_clock.now_millis() {
            Ok(observed_at_ms) if observed_at_ms > 0 => observed_at_ms,
            Ok(_) => {
                return self.finish(
                    started_at,
                    AttemptResult::clock(CodexQuotaClockErrorCode::InvalidTime),
                );
            }
            Err(error) => return self.finish(started_at, AttemptResult::clock(error)),
        };
        let snapshot = match self.source.poll(observed_at_ms) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return self.finish(started_at, AttemptResult::source(error, observed_at_ms));
            }
        };
        let observation_count = u16::try_from(snapshot.observations().len()).unwrap_or(u16::MAX);
        let publication_report = QuotaPublicationReport::for_snapshot(&snapshot);
        if snapshot.observations().len() > MAX_CODEX_QUOTA_WINDOWS {
            return self.finish(
                started_at,
                AttemptResult::transport_capacity(
                    observed_at_ms,
                    observation_count,
                    publication_report,
                ),
            );
        }
        if let Err(error) = control.check() {
            return self.finish(
                started_at,
                AttemptResult::control(
                    error.code(),
                    publication_report,
                    observation_count,
                    Some(observed_at_ms),
                    false,
                ),
            );
        }
        let publication = match self.publisher.publish(&snapshot, &control) {
            Ok(mut report)
                if !publication_report_is_consistent(
                    report,
                    observation_count,
                    publication_report.benefit.observation_count,
                ) =>
            {
                mark_inconsistent_domains_failed(
                    &mut report,
                    observation_count,
                    publication_report.benefit.observation_count,
                );
                AttemptResult::domain_publication(observed_at_ms, observation_count, report)
            }
            Ok(report) if report.has_domain_failure() => {
                AttemptResult::domain_publication(observed_at_ms, observation_count, report)
            }
            Ok(report) => AttemptResult::completed(observed_at_ms, observation_count, report),
            Err(error) => AttemptResult::publication(
                observed_at_ms,
                observation_count,
                publication_report.benefit.observation_count,
                error,
            ),
        };
        self.finish(started_at, publication)
    }

    fn finish(&self, started_at: u64, result: AttemptResult) -> RefreshOutcome {
        let elapsed_millis = self
            .monotonic_clock
            .now()
            .as_millis()
            .saturating_sub(started_at);
        let mut latest = match self.latest.lock() {
            Ok(latest) => latest,
            Err(_) => return RefreshOutcome::Failed,
        };
        let Some(attempt_sequence) = latest.attempt_sequence.checked_add(1) else {
            return RefreshOutcome::Failed;
        };
        let last_success_observed_at_ms = if result.outcome == RefreshOutcome::Completed {
            result.observed_at_ms
        } else {
            latest.last_success_observed_at_ms
        };
        let last_quota_success_observed_at_ms = if result.quota_succeeded {
            result.observed_at_ms
        } else {
            latest.last_quota_success_observed_at_ms
        };
        let last_benefit_success_observed_at_ms = if result.benefit_succeeded {
            result.observed_at_ms
        } else {
            latest.last_benefit_success_observed_at_ms
        };
        *latest = CodexQuotaRefreshSnapshot {
            attempt_sequence,
            outcome: Some(result.outcome),
            failure: result.failure,
            retry_mode: result.retry_mode,
            observation_count: result.observation_count,
            processed_count: result.report.quota.processed_count,
            changed_count: result.report.quota.changed_count,
            started_count: result.report.quota.started_count,
            advanced_count: result.report.quota.advanced_count,
            duplicate_count: result.report.quota.duplicate_count,
            stale_count: result.report.quota.stale_count,
            allowance_change_count: result.report.quota.allowance_change_count,
            reset_count: result.report.quota.reset_count,
            quota_failure: result.report.quota_failure,
            benefit_observation_count: result.report.benefit.observation_count,
            benefit_processed_count: result.report.benefit.processed_count,
            benefit_changed_count: result.report.benefit.changed_count,
            benefit_freshness_only_count: result.report.benefit.freshness_only_count,
            benefit_duplicate_count: result.report.benefit.duplicate_count,
            benefit_stale_count: result.report.benefit.stale_count,
            benefit_lot_change_count: result.report.benefit.lot_change_count,
            benefit_pending_due_count: result.report.benefit.pending_due_count,
            benefit_failure: result.report.benefit_failure,
            observed_at_ms: result.observed_at_ms,
            elapsed_millis,
            last_success_observed_at_ms,
            last_quota_success_observed_at_ms,
            last_benefit_success_observed_at_ms,
        };
        result.outcome
    }
}

struct AttemptResult {
    outcome: RefreshOutcome,
    failure: Option<CodexQuotaRefreshFailure>,
    retry_mode: CodexQuotaRetryMode,
    observation_count: u16,
    report: QuotaPublicationReport,
    observed_at_ms: Option<i64>,
    quota_succeeded: bool,
    benefit_succeeded: bool,
}

impl AttemptResult {
    fn completed(
        observed_at_ms: i64,
        observation_count: u16,
        report: QuotaPublicationReport,
    ) -> Self {
        Self {
            outcome: RefreshOutcome::Completed,
            failure: None,
            retry_mode: CodexQuotaRetryMode::Normal,
            observation_count,
            report,
            observed_at_ms: Some(observed_at_ms),
            quota_succeeded: report.quota.processed_count == observation_count,
            benefit_succeeded: report.benefit.observation_count > 0
                && report.benefit.processed_count == report.benefit.observation_count,
        }
    }

    fn clock(error: CodexQuotaClockErrorCode) -> Self {
        Self {
            outcome: RefreshOutcome::Failed,
            failure: Some(CodexQuotaRefreshFailure::Clock(error)),
            retry_mode: CodexQuotaRetryMode::Normal,
            observation_count: 0,
            report: QuotaPublicationReport::default(),
            observed_at_ms: None,
            quota_succeeded: false,
            benefit_succeeded: false,
        }
    }

    fn source(error: CodexQuotaSourceFailure, observed_at_ms: i64) -> Self {
        match error {
            CodexQuotaSourceFailure::Discovery(error) => Self {
                outcome: RefreshOutcome::Failed,
                failure: Some(CodexQuotaRefreshFailure::Discovery(error)),
                retry_mode: CodexQuotaRetryMode::Normal,
                observation_count: 0,
                report: QuotaPublicationReport::default(),
                observed_at_ms: Some(observed_at_ms),
                quota_succeeded: false,
                benefit_succeeded: false,
            },
            CodexQuotaSourceFailure::Transport(error) => Self {
                outcome: transport_outcome(error),
                failure: Some(CodexQuotaRefreshFailure::Transport(error)),
                retry_mode: transport_retry_mode(error),
                observation_count: 0,
                report: QuotaPublicationReport::default(),
                observed_at_ms: Some(observed_at_ms),
                quota_succeeded: false,
                benefit_succeeded: false,
            },
        }
    }

    fn transport_capacity(
        observed_at_ms: i64,
        observation_count: u16,
        report: QuotaPublicationReport,
    ) -> Self {
        Self {
            outcome: RefreshOutcome::Failed,
            failure: Some(CodexQuotaRefreshFailure::Transport(
                CodexQuotaErrorCode::CapacityExceeded,
            )),
            retry_mode: CodexQuotaRetryMode::Normal,
            observation_count,
            report,
            observed_at_ms: Some(observed_at_ms),
            quota_succeeded: false,
            benefit_succeeded: false,
        }
    }

    fn control(
        error: PortErrorCode,
        report: QuotaPublicationReport,
        observation_count: u16,
        observed_at_ms: Option<i64>,
        publication_attempted: bool,
    ) -> Self {
        let (quota_succeeded, benefit_succeeded) =
            publication_successes(report, observation_count, publication_attempted);
        Self {
            outcome: control_outcome(error),
            failure: Some(CodexQuotaRefreshFailure::Control(error)),
            retry_mode: CodexQuotaRetryMode::Normal,
            observation_count,
            report,
            observed_at_ms,
            quota_succeeded,
            benefit_succeeded,
        }
    }

    fn publication(
        observed_at_ms: i64,
        observation_count: u16,
        benefit_observation_count: u8,
        error: QuotaPublicationError,
    ) -> Self {
        let code = error.code();
        let mut report = error.report();
        if report.benefit.processed_count == 0 {
            report.benefit.observation_count = benefit_observation_count;
        }
        mark_unpublished_domains_failed(&mut report, observation_count, code);
        match code {
            CodexQuotaPublicationErrorCode::Cancelled => Self::control(
                PortErrorCode::Cancelled,
                report,
                observation_count,
                Some(observed_at_ms),
                true,
            ),
            CodexQuotaPublicationErrorCode::DeadlineExceeded => Self::control(
                PortErrorCode::DeadlineExceeded,
                report,
                observation_count,
                Some(observed_at_ms),
                true,
            ),
            _ => {
                let (quota_succeeded, benefit_succeeded) =
                    publication_successes(report, observation_count, true);
                Self {
                    outcome: publication_outcome(code),
                    failure: Some(CodexQuotaRefreshFailure::Publication(code)),
                    retry_mode: publication_retry_mode(code),
                    observation_count,
                    report,
                    observed_at_ms: Some(observed_at_ms),
                    quota_succeeded,
                    benefit_succeeded,
                }
            }
        }
    }

    fn domain_publication(
        observed_at_ms: i64,
        observation_count: u16,
        report: QuotaPublicationReport,
    ) -> Self {
        let (outcome, retry_mode) = domain_publication_classification(report);
        let failure = report
            .quota_failure
            .map(CodexQuotaRefreshFailure::QuotaPublication)
            .or_else(|| {
                report
                    .benefit_failure
                    .map(CodexQuotaRefreshFailure::BenefitPublication)
            });
        let (quota_succeeded, benefit_succeeded) =
            publication_successes(report, observation_count, true);
        Self {
            outcome,
            failure,
            retry_mode,
            observation_count,
            report,
            observed_at_ms: Some(observed_at_ms),
            quota_succeeded,
            benefit_succeeded,
        }
    }
}

const fn increment(value: &mut u16) -> Result<(), CodexQuotaPublicationErrorCode> {
    match value.checked_add(1) {
        Some(next) => {
            *value = next;
            Ok(())
        }
        None => Err(CodexQuotaPublicationErrorCode::CapacityExceeded),
    }
}

const fn increment_u8(value: &mut u8) -> Result<(), CodexQuotaPublicationErrorCode> {
    match value.checked_add(1) {
        Some(next) => {
            *value = next;
            Ok(())
        }
        None => Err(CodexQuotaPublicationErrorCode::CapacityExceeded),
    }
}

fn publication_successes(
    report: QuotaPublicationReport,
    observation_count: u16,
    publication_attempted: bool,
) -> (bool, bool) {
    if !publication_attempted {
        return (false, false);
    }
    let quota_succeeded =
        report.quota_failure.is_none() && report.quota.processed_count == observation_count;
    let benefit_succeeded = report.benefit.observation_count > 0
        && report.benefit_failure.is_none()
        && report.benefit.processed_count == report.benefit.observation_count;
    (quota_succeeded, benefit_succeeded)
}

fn mark_unpublished_domains_failed(
    report: &mut QuotaPublicationReport,
    observation_count: u16,
    code: CodexQuotaPublicationErrorCode,
) {
    if report.quota_failure.is_none() && report.quota.processed_count < observation_count {
        report.quota_failure = Some(code);
    }
    if report.benefit.observation_count > report.benefit.processed_count
        && report.benefit_failure.is_none()
    {
        report.benefit_failure = Some(code);
    }
}

fn publication_report_is_consistent(
    report: QuotaPublicationReport,
    quota_observation_count: u16,
    benefit_observation_count: u8,
) -> bool {
    quota_report_is_consistent(report, quota_observation_count)
        && benefit_report_is_consistent(report, benefit_observation_count)
}

fn quota_report_is_consistent(report: QuotaPublicationReport, observation_count: u16) -> bool {
    let classified_changes = u32::from(report.quota.started_count)
        + u32::from(report.quota.advanced_count)
        + u32::from(report.quota.allowance_change_count)
        + u32::from(report.quota.reset_count);
    let classified_processed = u32::from(report.quota.changed_count)
        + u32::from(report.quota.duplicate_count)
        + u32::from(report.quota.stale_count);
    let completion_matches_failure = match report.quota_failure {
        None => report.quota.processed_count == observation_count,
        Some(_) => report.quota.processed_count < observation_count,
    };
    classified_changes == u32::from(report.quota.changed_count)
        && classified_processed == u32::from(report.quota.processed_count)
        && report.quota.processed_count <= observation_count
        && completion_matches_failure
}

fn benefit_report_is_consistent(report: QuotaPublicationReport, observation_count: u8) -> bool {
    let classified_processed = u16::from(report.benefit.changed_count)
        + u16::from(report.benefit.freshness_only_count)
        + u16::from(report.benefit.duplicate_count)
        + u16::from(report.benefit.stale_count);
    let completion_matches_failure = match report.benefit_failure {
        None => report.benefit.processed_count == observation_count,
        Some(_) => report.benefit.processed_count < observation_count,
    };
    report.benefit.observation_count == observation_count
        && classified_processed == u16::from(report.benefit.processed_count)
        && report.benefit.processed_count <= observation_count
        && completion_matches_failure
}

fn mark_inconsistent_domains_failed(
    report: &mut QuotaPublicationReport,
    quota_observation_count: u16,
    benefit_observation_count: u8,
) {
    if !quota_report_is_consistent(*report, quota_observation_count) {
        report.quota_failure = Some(CodexQuotaPublicationErrorCode::InvalidData);
    }
    if !benefit_report_is_consistent(*report, benefit_observation_count) {
        report.benefit.observation_count = benefit_observation_count;
        report.benefit_failure = Some(CodexQuotaPublicationErrorCode::InvalidData);
    }
}

fn domain_publication_classification(
    report: QuotaPublicationReport,
) -> (RefreshOutcome, CodexQuotaRetryMode) {
    let quota = report.quota_failure;
    let benefit = report.benefit_failure;
    let outcome = if quota == Some(CodexQuotaPublicationErrorCode::Cancelled)
        || benefit == Some(CodexQuotaPublicationErrorCode::Cancelled)
    {
        RefreshOutcome::Cancelled
    } else if quota == Some(CodexQuotaPublicationErrorCode::DeadlineExceeded)
        || benefit == Some(CodexQuotaPublicationErrorCode::DeadlineExceeded)
    {
        RefreshOutcome::DeadlineExceeded
    } else if quota == Some(CodexQuotaPublicationErrorCode::Busy)
        || benefit == Some(CodexQuotaPublicationErrorCode::Busy)
    {
        RefreshOutcome::Busy
    } else {
        RefreshOutcome::Failed
    };
    let retry_mode = if quota
        .is_some_and(|code| publication_retry_mode(code) == CodexQuotaRetryMode::Accelerated)
        || benefit
            .is_some_and(|code| publication_retry_mode(code) == CodexQuotaRetryMode::Accelerated)
    {
        CodexQuotaRetryMode::Accelerated
    } else {
        CodexQuotaRetryMode::Normal
    };
    (outcome, retry_mode)
}

const fn control_outcome(error: PortErrorCode) -> RefreshOutcome {
    match error {
        PortErrorCode::Cancelled => RefreshOutcome::Cancelled,
        PortErrorCode::DeadlineExceeded => RefreshOutcome::DeadlineExceeded,
        PortErrorCode::Busy => RefreshOutcome::Busy,
        PortErrorCode::InvalidData
        | PortErrorCode::CapacityExceeded
        | PortErrorCode::StaleState
        | PortErrorCode::RebuildRequired
        | PortErrorCode::Unavailable
        | PortErrorCode::Failed => RefreshOutcome::Failed,
    }
}

const fn transport_outcome(error: CodexQuotaErrorCode) -> RefreshOutcome {
    match error {
        CodexQuotaErrorCode::DeadlineExceeded => RefreshOutcome::DeadlineExceeded,
        CodexQuotaErrorCode::InvalidData
        | CodexQuotaErrorCode::CapacityExceeded
        | CodexQuotaErrorCode::AccountIdentityUnavailable
        | CodexQuotaErrorCode::InvalidTime
        | CodexQuotaErrorCode::Unavailable
        | CodexQuotaErrorCode::InvalidCommand
        | CodexQuotaErrorCode::SpawnFailed
        | CodexQuotaErrorCode::ProtocolError
        | CodexQuotaErrorCode::UnsupportedVersion
        | CodexQuotaErrorCode::RpcError
        | CodexQuotaErrorCode::ProcessExited
        | CodexQuotaErrorCode::ProcessCleanupFailed => RefreshOutcome::Failed,
    }
}

const fn transport_retry_mode(error: CodexQuotaErrorCode) -> CodexQuotaRetryMode {
    match error {
        CodexQuotaErrorCode::Unavailable
        | CodexQuotaErrorCode::SpawnFailed
        | CodexQuotaErrorCode::DeadlineExceeded
        | CodexQuotaErrorCode::ProcessExited
        | CodexQuotaErrorCode::ProcessCleanupFailed => CodexQuotaRetryMode::Accelerated,
        CodexQuotaErrorCode::InvalidData
        | CodexQuotaErrorCode::CapacityExceeded
        | CodexQuotaErrorCode::AccountIdentityUnavailable
        | CodexQuotaErrorCode::InvalidTime
        | CodexQuotaErrorCode::InvalidCommand
        | CodexQuotaErrorCode::ProtocolError
        | CodexQuotaErrorCode::UnsupportedVersion
        | CodexQuotaErrorCode::RpcError => CodexQuotaRetryMode::Normal,
    }
}

const fn publication_outcome(error: CodexQuotaPublicationErrorCode) -> RefreshOutcome {
    match error {
        CodexQuotaPublicationErrorCode::Busy => RefreshOutcome::Busy,
        CodexQuotaPublicationErrorCode::Cancelled => RefreshOutcome::Cancelled,
        CodexQuotaPublicationErrorCode::DeadlineExceeded => RefreshOutcome::DeadlineExceeded,
        CodexQuotaPublicationErrorCode::StoreUnavailable
        | CodexQuotaPublicationErrorCode::InvalidData
        | CodexQuotaPublicationErrorCode::CapacityExceeded => RefreshOutcome::Failed,
    }
}

const fn publication_retry_mode(error: CodexQuotaPublicationErrorCode) -> CodexQuotaRetryMode {
    match error {
        CodexQuotaPublicationErrorCode::Busy => CodexQuotaRetryMode::Accelerated,
        CodexQuotaPublicationErrorCode::Cancelled
        | CodexQuotaPublicationErrorCode::DeadlineExceeded
        | CodexQuotaPublicationErrorCode::StoreUnavailable
        | CodexQuotaPublicationErrorCode::InvalidData
        | CodexQuotaPublicationErrorCode::CapacityExceeded => CodexQuotaRetryMode::Normal,
    }
}

const fn control_publication_error(
    error: PortErrorCode,
    report: QuotaPublicationReport,
) -> QuotaPublicationError {
    let code = match error {
        PortErrorCode::Cancelled => CodexQuotaPublicationErrorCode::Cancelled,
        PortErrorCode::DeadlineExceeded => CodexQuotaPublicationErrorCode::DeadlineExceeded,
        PortErrorCode::Busy => CodexQuotaPublicationErrorCode::Busy,
        PortErrorCode::CapacityExceeded => CodexQuotaPublicationErrorCode::CapacityExceeded,
        PortErrorCode::InvalidData | PortErrorCode::StaleState | PortErrorCode::RebuildRequired => {
            CodexQuotaPublicationErrorCode::InvalidData
        }
        PortErrorCode::Unavailable | PortErrorCode::Failed => {
            CodexQuotaPublicationErrorCode::StoreUnavailable
        }
    };
    QuotaPublicationError::with_report(code, report)
}

const fn lease_publication_error(
    error: PortErrorCode,
    report: QuotaPublicationReport,
) -> QuotaPublicationError {
    control_publication_error(error, report)
}

const fn store_publication_error(
    error: StoreErrorCode,
    report: QuotaPublicationReport,
) -> QuotaPublicationError {
    QuotaPublicationError::with_report(store_publication_code(error), report)
}

const fn store_publication_code(error: StoreErrorCode) -> CodexQuotaPublicationErrorCode {
    match error {
        StoreErrorCode::CapacityExceeded => CodexQuotaPublicationErrorCode::CapacityExceeded,
        StoreErrorCode::DeadlineExceeded => CodexQuotaPublicationErrorCode::DeadlineExceeded,
        StoreErrorCode::InvalidValue
        | StoreErrorCode::InvalidStoredValue
        | StoreErrorCode::AccountingVersionMismatch
        | StoreErrorCode::IncompleteManifest
        | StoreErrorCode::UnsealedRevision
        | StoreErrorCode::ArchiveModeMismatch
        | StoreErrorCode::StaleCheckpoint
        | StoreErrorCode::StaleRevision
        | StoreErrorCode::StaleScan
        | StoreErrorCode::PendingScan
        | StoreErrorCode::PendingContinuation => CodexQuotaPublicationErrorCode::InvalidData,
        StoreErrorCode::ScanInProgress => CodexQuotaPublicationErrorCode::Busy,
        StoreErrorCode::Database
        | StoreErrorCode::VersionMismatch
        | StoreErrorCode::SchemaTooNew
        | StoreErrorCode::SchemaMismatch
        | StoreErrorCode::PolicyMismatch
        | StoreErrorCode::RebuildRequired => CodexQuotaPublicationErrorCode::StoreUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    };

    use serde_json::json;
    use tempfile::TempDir;
    use tokenmaster_codex::{CodexQuotaErrorCode, CodexQuotaNormalizer, CodexQuotaSnapshot};
    use tokenmaster_engine::{
        Clock, MonotonicTime, OperationControl, PortErrorCode, RefreshAdmission,
        RefreshCoordinator, RefreshDeadline, RefreshOutcome, RefreshUrgency, WriterLease,
    };

    use super::{
        BenefitPublicationSummary, CodexQuotaExecution, CodexQuotaSource, CodexQuotaSourceFailure,
        CodexQuotaWallClock, QuotaPublicationError, QuotaPublicationReport,
        QuotaPublicationSummary, QuotaPublisher, StoreQuotaPublisher,
    };
    use crate::RuntimeWriterLease;
    use crate::quota::{
        CodexQuotaClockErrorCode, CodexQuotaPublicationErrorCode, CodexQuotaRefreshFailure,
        CodexQuotaRefreshSnapshot, CodexQuotaRetryMode,
    };

    const OBSERVED_AT_MS: i64 = 1_700_000_000_000;
    const PRIVATE_EMAIL: &str = "private-runtime@example.com";

    trait TestResultExt<T, E> {
        fn test_value(self, context: &str) -> T;
        fn test_error(self, context: &str) -> E;
    }

    impl<T, E> TestResultExt<T, E> for Result<T, E> {
        fn test_value(self, context: &str) -> T {
            match self {
                Ok(value) => value,
                Err(_) => panic!("{context}"),
            }
        }

        fn test_error(self, context: &str) -> E {
            match self {
                Ok(_) => panic!("{context}"),
                Err(error) => error,
            }
        }
    }

    #[derive(Default)]
    struct FakeMonotonicClock {
        millis: AtomicU64,
    }

    impl FakeMonotonicClock {
        fn set(&self, millis: u64) {
            self.millis.store(millis, Ordering::Release);
        }
    }

    impl Clock for FakeMonotonicClock {
        fn now(&self) -> MonotonicTime {
            MonotonicTime::from_millis(self.millis.load(Ordering::Acquire))
        }
    }

    #[derive(Clone, Copy)]
    struct FixedWallClock(Result<i64, CodexQuotaClockErrorCode>);

    impl CodexQuotaWallClock for FixedWallClock {
        fn now_millis(&self) -> Result<i64, CodexQuotaClockErrorCode> {
            self.0
        }
    }

    struct FakeSource {
        events: Arc<Mutex<Vec<&'static str>>>,
        result: Result<CodexQuotaSnapshot, CodexQuotaSourceFailure>,
        on_poll: Option<Box<dyn FnOnce() + Send>>,
    }

    impl CodexQuotaSource for FakeSource {
        fn poll(
            &mut self,
            _observed_at_ms: i64,
        ) -> Result<CodexQuotaSnapshot, CodexQuotaSourceFailure> {
            self.events.lock().test_value("event lock").push("source");
            if let Some(on_poll) = self.on_poll.take() {
                on_poll();
            }
            self.result.clone()
        }
    }

    struct FakePublisher {
        events: Arc<Mutex<Vec<&'static str>>>,
        calls: Arc<AtomicUsize>,
        result: Result<QuotaPublicationReport, QuotaPublicationError>,
    }

    impl QuotaPublisher for FakePublisher {
        fn publish(
            &mut self,
            _snapshot: &CodexQuotaSnapshot,
            _control: &OperationControl<'_>,
        ) -> Result<QuotaPublicationReport, QuotaPublicationError> {
            self.events.lock().test_value("event lock").push("publish");
            self.calls.fetch_add(1, Ordering::AcqRel);
            self.result
        }
    }

    fn normalized(observed_at_ms: i64) -> CodexQuotaSnapshot {
        let account = serde_json::to_vec(&json!({
            "requiresOpenaiAuth": true,
            "account": {
                "type": "chatgpt",
                "email": PRIVATE_EMAIL,
                "planType": "pro"
            }
        }))
        .test_value("account fixture");
        let quota = serde_json::to_vec(&json!({
            "rateLimitResetCredits": {
                "availableCount": 1,
                "credits": [{
                    "description": "private runtime benefit description",
                    "expiresAt": 1_700_300_000,
                    "grantedAt": 1_699_000_000,
                    "id": "private_runtime_credit_id",
                    "resetType": "codexRateLimits",
                    "status": "available",
                    "title": "private runtime benefit title"
                }]
            },
            "rateLimits": {
                "credits": null,
                "individualLimit": null,
                "limitId": "codex",
                "limitName": "private runtime label",
                "planType": "pro",
                "primary": {
                    "usedPercent": 25,
                    "resetsAt": 1_700_100_000,
                    "windowDurationMins": 300
                },
                "rateLimitReachedType": null,
                "secondary": {
                    "usedPercent": 50,
                    "resetsAt": 1_700_200_000,
                    "windowDurationMins": 10_080
                }
            },
            "rateLimitsByLimitId": null
        }))
        .test_value("quota fixture");
        CodexQuotaNormalizer::normalize(&account, &quota, observed_at_ms)
            .test_value("normalized fixture")
    }

    fn permit(
        deadline: Option<RefreshDeadline>,
    ) -> (RefreshCoordinator, tokenmaster_engine::RefreshPermit) {
        let mut coordinator = RefreshCoordinator::new();
        let RefreshAdmission::Started(permit) = coordinator
            .submit(
                RefreshUrgency::Recovery,
                deadline,
                MonotonicTime::from_millis(0),
            )
            .test_value("refresh admission")
        else {
            panic!("refresh must start");
        };
        (coordinator, permit)
    }

    fn completed_summary() -> QuotaPublicationSummary {
        QuotaPublicationSummary {
            processed_count: 2,
            changed_count: 2,
            started_count: 0,
            advanced_count: 0,
            duplicate_count: 0,
            stale_count: 0,
            allowance_change_count: 1,
            reset_count: 1,
        }
    }

    fn completed_report() -> QuotaPublicationReport {
        QuotaPublicationReport {
            quota: completed_summary(),
            benefit: BenefitPublicationSummary {
                observation_count: 1,
                processed_count: 1,
                changed_count: 1,
                ..BenefitPublicationSummary::default()
            },
            quota_failure: None,
            benefit_failure: None,
        }
    }

    #[test]
    fn source_completes_before_publication_and_success_health_is_bounded() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let monotonic = Arc::new(FakeMonotonicClock::default());
        monotonic.set(10);
        let source = FakeSource {
            events: Arc::clone(&events),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: None,
        };
        let publisher = FakePublisher {
            events: Arc::clone(&events),
            calls: Arc::clone(&calls),
            result: Ok(completed_report()),
        };
        let mut execution = CodexQuotaExecution::new(
            monotonic,
            FixedWallClock(Ok(OBSERVED_AT_MS)),
            source,
            publisher,
            Arc::clone(&latest),
        );
        let (_coordinator, permit) = permit(None);

        assert_eq!(execution.run(&permit), RefreshOutcome::Completed);
        assert_eq!(
            *events.lock().test_value("events"),
            vec!["source", "publish"]
        );
        assert_eq!(calls.load(Ordering::Acquire), 1);
        let snapshot = *latest.lock().test_value("latest");
        assert_eq!(snapshot.attempt_sequence(), 1);
        assert_eq!(snapshot.outcome(), Some(RefreshOutcome::Completed));
        assert_eq!(snapshot.failure(), None);
        assert_eq!(snapshot.retry_mode(), CodexQuotaRetryMode::Normal);
        assert_eq!(snapshot.observation_count(), 2);
        assert_eq!(snapshot.processed_count(), 2);
        assert_eq!(snapshot.changed_count(), 2);
        assert_eq!(snapshot.quota_failure_count(), 0);
        assert_eq!(snapshot.quota_failure(), None);
        assert_eq!(snapshot.allowance_change_count(), 1);
        assert_eq!(snapshot.reset_count(), 1);
        assert_eq!(snapshot.benefit_observation_count(), 1);
        assert_eq!(snapshot.benefit_processed_count(), 1);
        assert_eq!(snapshot.benefit_changed_count(), 1);
        assert_eq!(snapshot.benefit_failure_count(), 0);
        assert_eq!(snapshot.benefit_failure(), None);
        assert_eq!(snapshot.observed_at_ms(), Some(OBSERVED_AT_MS));
        assert_eq!(snapshot.last_success_observed_at_ms(), Some(OBSERVED_AT_MS));
        assert_eq!(
            snapshot.last_quota_success_observed_at_ms(),
            Some(OBSERVED_AT_MS)
        );
        assert_eq!(
            snapshot.last_benefit_success_observed_at_ms(),
            Some(OBSERVED_AT_MS)
        );
        let debug = format!("{snapshot:?}");
        assert!(!debug.contains(PRIVATE_EMAIL));
        assert!(!debug.contains("private runtime label"));
    }

    #[test]
    fn cancellation_after_source_completion_performs_no_publication() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let monotonic = Arc::new(FakeMonotonicClock::default());
        let (coordinator, permit) = permit(None);
        let coordinator = Arc::new(Mutex::new(coordinator));
        let request_id = permit.id();
        let cancel = Arc::clone(&coordinator);
        let source = FakeSource {
            events: Arc::clone(&events),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: Some(Box::new(move || {
                cancel
                    .lock()
                    .test_value("coordinator")
                    .cancel(request_id)
                    .test_value("cancel active request");
            })),
        };
        let publisher = FakePublisher {
            events: Arc::clone(&events),
            calls: Arc::clone(&calls),
            result: Ok(completed_report()),
        };
        let mut execution = CodexQuotaExecution::new(
            monotonic,
            FixedWallClock(Ok(OBSERVED_AT_MS)),
            source,
            publisher,
            Arc::clone(&latest),
        );

        assert_eq!(execution.run(&permit), RefreshOutcome::Cancelled);
        assert_eq!(*events.lock().test_value("events"), vec!["source"]);
        assert_eq!(calls.load(Ordering::Acquire), 0);
        let snapshot = *latest.lock().test_value("latest");
        assert_eq!(
            snapshot.failure(),
            Some(CodexQuotaRefreshFailure::Control(PortErrorCode::Cancelled))
        );
        assert_eq!(snapshot.observation_count(), 2);
        assert_eq!(snapshot.processed_count(), 0);
        assert_eq!(snapshot.benefit_observation_count(), 1);
        assert_eq!(snapshot.benefit_processed_count(), 0);
        assert_eq!(snapshot.last_success_observed_at_ms(), None);
        assert_eq!(snapshot.last_quota_success_observed_at_ms(), None);
        assert_eq!(snapshot.last_benefit_success_observed_at_ms(), None);
    }

    #[test]
    fn store_publisher_maps_contention_to_busy_without_opening_the_archive() {
        let root = TempDir::new().test_value("temporary root");
        let archive = root.path().join("usage.sqlite3");
        let mut competing = RuntimeWriterLease::new(&archive).test_value("competing lease");
        let _guard = competing.try_acquire().test_value("hold competing lease");
        let mut publisher = StoreQuotaPublisher::new(&archive).test_value("publisher");
        let monotonic = FakeMonotonicClock::default();
        let (_coordinator, permit) = permit(None);
        let control = OperationControl::new(&permit, &monotonic);

        let error = publisher
            .publish(&normalized(OBSERVED_AT_MS), &control)
            .test_error("writer contention");
        assert_eq!(error.code(), CodexQuotaPublicationErrorCode::Busy);
        assert_eq!(error.report().quota, QuotaPublicationSummary::default());
        assert_eq!(error.report().benefit.observation_count, 1);
        assert!(!archive.exists(), "busy publication must not open SQLite");
    }

    #[test]
    fn store_publication_is_idempotent_and_reports_exact_status_counts() {
        let root = TempDir::new().test_value("temporary root");
        let archive = root.path().join("usage.sqlite3");
        let mut publisher = StoreQuotaPublisher::new(&archive).test_value("publisher");
        let monotonic = FakeMonotonicClock::default();
        let (_coordinator, permit) = permit(None);
        let control = OperationControl::new(&permit, &monotonic);
        let snapshot = normalized(OBSERVED_AT_MS);

        let first = publisher
            .publish(&snapshot, &control)
            .test_value("first publish");
        assert_eq!(first.quota.processed_count, 2);
        assert_eq!(first.quota.changed_count, 2);
        assert_eq!(first.quota.started_count, 2);
        assert_eq!(first.quota.duplicate_count, 0);
        assert_eq!(first.benefit.observation_count, 1);
        assert_eq!(first.benefit.processed_count, 1);
        assert_eq!(first.benefit.changed_count, 1);
        assert_eq!(first.benefit.duplicate_count, 0);
        assert_eq!(first.benefit_failure, None);

        drop(publisher);
        let mut restarted = StoreQuotaPublisher::new(&archive).test_value("restarted publisher");
        let duplicate = restarted
            .publish(&snapshot, &control)
            .test_value("idempotent restart publish");
        assert_eq!(duplicate.quota.processed_count, 2);
        assert_eq!(duplicate.quota.changed_count, 0);
        assert_eq!(duplicate.quota.duplicate_count, 2);
        assert_eq!(duplicate.quota.stale_count, 0);
        assert_eq!(duplicate.benefit.processed_count, 1);
        assert_eq!(duplicate.benefit.changed_count, 0);
        assert_eq!(duplicate.benefit.duplicate_count, 1);
        assert_eq!(duplicate.benefit.stale_count, 0);
        assert_eq!(duplicate.quota_failure, None);
        assert_eq!(duplicate.benefit_failure, None);
    }

    #[test]
    fn quota_success_and_benefit_failure_publish_separate_health_without_false_atomic_success() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let report = QuotaPublicationReport {
            quota: completed_summary(),
            benefit: BenefitPublicationSummary {
                observation_count: 1,
                ..BenefitPublicationSummary::default()
            },
            quota_failure: None,
            benefit_failure: Some(CodexQuotaPublicationErrorCode::InvalidData),
        };
        let source = FakeSource {
            events: Arc::clone(&events),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: None,
        };
        let publisher = FakePublisher {
            events,
            calls: Arc::new(AtomicUsize::new(0)),
            result: Ok(report),
        };
        let mut execution = CodexQuotaExecution::new(
            Arc::new(FakeMonotonicClock::default()),
            FixedWallClock(Ok(OBSERVED_AT_MS)),
            source,
            publisher,
            Arc::clone(&latest),
        );
        let (_coordinator, permit) = permit(None);

        assert_eq!(execution.run(&permit), RefreshOutcome::Failed);
        let snapshot = *latest.lock().test_value("latest");
        assert_eq!(
            snapshot.failure(),
            Some(CodexQuotaRefreshFailure::BenefitPublication(
                CodexQuotaPublicationErrorCode::InvalidData
            ))
        );
        assert_eq!(snapshot.quota_failure_count(), 0);
        assert_eq!(snapshot.quota_failure(), None);
        assert_eq!(snapshot.processed_count(), 2);
        assert_eq!(snapshot.changed_count(), 2);
        assert_eq!(snapshot.benefit_failure_count(), 1);
        assert_eq!(
            snapshot.benefit_failure(),
            Some(CodexQuotaPublicationErrorCode::InvalidData)
        );
        assert_eq!(snapshot.benefit_processed_count(), 0);
        assert_eq!(snapshot.last_success_observed_at_ms(), None);
        assert_eq!(
            snapshot.last_quota_success_observed_at_ms(),
            Some(OBSERVED_AT_MS)
        );
        assert_eq!(snapshot.last_benefit_success_observed_at_ms(), None);
    }

    #[test]
    fn benefit_contention_keeps_quota_success_and_selects_accelerated_retry() {
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let report = QuotaPublicationReport {
            quota: completed_summary(),
            benefit: BenefitPublicationSummary {
                observation_count: 1,
                ..BenefitPublicationSummary::default()
            },
            quota_failure: None,
            benefit_failure: Some(CodexQuotaPublicationErrorCode::Busy),
        };
        let source = FakeSource {
            events: Arc::new(Mutex::new(Vec::new())),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: None,
        };
        let publisher = FakePublisher {
            events: Arc::new(Mutex::new(Vec::new())),
            calls: Arc::new(AtomicUsize::new(0)),
            result: Ok(report),
        };
        let mut execution = CodexQuotaExecution::new(
            Arc::new(FakeMonotonicClock::default()),
            FixedWallClock(Ok(OBSERVED_AT_MS)),
            source,
            publisher,
            Arc::clone(&latest),
        );
        let (_coordinator, permit) = permit(None);

        assert_eq!(execution.run(&permit), RefreshOutcome::Busy);
        let snapshot = *latest.lock().test_value("latest");
        assert_eq!(snapshot.retry_mode(), CodexQuotaRetryMode::Accelerated);
        assert_eq!(
            snapshot.failure(),
            Some(CodexQuotaRefreshFailure::BenefitPublication(
                CodexQuotaPublicationErrorCode::Busy
            ))
        );
        assert_eq!(
            snapshot.last_quota_success_observed_at_ms(),
            Some(OBSERVED_AT_MS)
        );
        assert_eq!(snapshot.last_benefit_success_observed_at_ms(), None);
    }

    #[test]
    fn benefit_success_remains_visible_when_quota_publication_fails() {
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let report = QuotaPublicationReport {
            quota: QuotaPublicationSummary::default(),
            benefit: BenefitPublicationSummary {
                observation_count: 1,
                processed_count: 1,
                changed_count: 1,
                lot_change_count: 1,
                pending_due_count: 5,
                ..BenefitPublicationSummary::default()
            },
            quota_failure: Some(CodexQuotaPublicationErrorCode::InvalidData),
            benefit_failure: None,
        };
        let source = FakeSource {
            events: Arc::new(Mutex::new(Vec::new())),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: None,
        };
        let publisher = FakePublisher {
            events: Arc::new(Mutex::new(Vec::new())),
            calls: Arc::new(AtomicUsize::new(0)),
            result: Ok(report),
        };
        let mut execution = CodexQuotaExecution::new(
            Arc::new(FakeMonotonicClock::default()),
            FixedWallClock(Ok(OBSERVED_AT_MS)),
            source,
            publisher,
            Arc::clone(&latest),
        );
        let (_coordinator, permit) = permit(None);

        assert_eq!(execution.run(&permit), RefreshOutcome::Failed);
        let snapshot = *latest.lock().test_value("latest");
        assert_eq!(
            snapshot.failure(),
            Some(CodexQuotaRefreshFailure::QuotaPublication(
                CodexQuotaPublicationErrorCode::InvalidData
            ))
        );
        assert_eq!(snapshot.quota_failure_count(), 1);
        assert_eq!(snapshot.benefit_failure_count(), 0);
        assert_eq!(snapshot.benefit_processed_count(), 1);
        assert_eq!(snapshot.benefit_lot_change_count(), 1);
        assert_eq!(snapshot.benefit_pending_due_count(), 5);
        assert_eq!(snapshot.last_quota_success_observed_at_ms(), None);
        assert_eq!(
            snapshot.last_benefit_success_observed_at_ms(),
            Some(OBSERVED_AT_MS)
        );
    }

    #[test]
    fn inconsistent_publisher_report_fails_closed_without_advancing_success() {
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let report = QuotaPublicationReport {
            quota: QuotaPublicationSummary {
                processed_count: 2,
                changed_count: 2,
                started_count: 1,
                ..QuotaPublicationSummary::default()
            },
            benefit: BenefitPublicationSummary {
                observation_count: 0,
                processed_count: 0,
                ..BenefitPublicationSummary::default()
            },
            quota_failure: None,
            benefit_failure: None,
        };
        let source = FakeSource {
            events: Arc::new(Mutex::new(Vec::new())),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: None,
        };
        let publisher = FakePublisher {
            events: Arc::new(Mutex::new(Vec::new())),
            calls: Arc::new(AtomicUsize::new(0)),
            result: Ok(report),
        };
        let mut execution = CodexQuotaExecution::new(
            Arc::new(FakeMonotonicClock::default()),
            FixedWallClock(Ok(OBSERVED_AT_MS)),
            source,
            publisher,
            Arc::clone(&latest),
        );
        let (_coordinator, permit) = permit(None);

        assert_eq!(execution.run(&permit), RefreshOutcome::Failed);
        let snapshot = *latest.lock().test_value("latest");
        assert_eq!(
            snapshot.failure(),
            Some(CodexQuotaRefreshFailure::QuotaPublication(
                CodexQuotaPublicationErrorCode::InvalidData
            ))
        );
        assert_eq!(
            snapshot.quota_failure(),
            Some(CodexQuotaPublicationErrorCode::InvalidData)
        );
        assert_eq!(
            snapshot.benefit_failure(),
            Some(CodexQuotaPublicationErrorCode::InvalidData)
        );
        assert_eq!(snapshot.last_success_observed_at_ms(), None);
        assert_eq!(snapshot.last_quota_success_observed_at_ms(), None);
        assert_eq!(snapshot.last_benefit_success_observed_at_ms(), None);
    }

    #[test]
    fn failure_health_preserves_partial_counts_and_retry_classification() {
        let cases = [
            (
                Err(CodexQuotaSourceFailure::Transport(
                    CodexQuotaErrorCode::UnsupportedVersion,
                )),
                None,
                RefreshOutcome::Failed,
                CodexQuotaRetryMode::Normal,
                Some(CodexQuotaRefreshFailure::Transport(
                    CodexQuotaErrorCode::UnsupportedVersion,
                )),
            ),
            (
                Err(CodexQuotaSourceFailure::Transport(
                    CodexQuotaErrorCode::DeadlineExceeded,
                )),
                None,
                RefreshOutcome::DeadlineExceeded,
                CodexQuotaRetryMode::Accelerated,
                Some(CodexQuotaRefreshFailure::Transport(
                    CodexQuotaErrorCode::DeadlineExceeded,
                )),
            ),
            (
                Ok(normalized(OBSERVED_AT_MS)),
                Some(QuotaPublicationError::with_report(
                    CodexQuotaPublicationErrorCode::InvalidData,
                    QuotaPublicationReport {
                        quota: QuotaPublicationSummary {
                            processed_count: 1,
                            changed_count: 1,
                            started_count: 1,
                            ..QuotaPublicationSummary::default()
                        },
                        benefit: BenefitPublicationSummary {
                            observation_count: 1,
                            ..BenefitPublicationSummary::default()
                        },
                        ..QuotaPublicationReport::default()
                    },
                )),
                RefreshOutcome::Failed,
                CodexQuotaRetryMode::Normal,
                Some(CodexQuotaRefreshFailure::Publication(
                    CodexQuotaPublicationErrorCode::InvalidData,
                )),
            ),
        ];

        for (source_result, publication_error, outcome, retry_mode, failure) in cases {
            let events = Arc::new(Mutex::new(Vec::new()));
            let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
            let monotonic = Arc::new(FakeMonotonicClock::default());
            let publisher_result =
                publication_error.map_or_else(|| Ok(QuotaPublicationReport::default()), Err);
            let source = FakeSource {
                events: Arc::clone(&events),
                result: source_result,
                on_poll: None,
            };
            let publisher = FakePublisher {
                events,
                calls: Arc::new(AtomicUsize::new(0)),
                result: publisher_result,
            };
            let mut execution = CodexQuotaExecution::new(
                monotonic,
                FixedWallClock(Ok(OBSERVED_AT_MS)),
                source,
                publisher,
                Arc::clone(&latest),
            );
            let (_coordinator, permit) = permit(None);

            assert_eq!(execution.run(&permit), outcome);
            let snapshot = *latest.lock().test_value("latest");
            assert_eq!(snapshot.outcome(), Some(outcome));
            assert_eq!(snapshot.retry_mode(), retry_mode);
            assert_eq!(snapshot.failure(), failure);
            if failure
                == Some(CodexQuotaRefreshFailure::Publication(
                    CodexQuotaPublicationErrorCode::InvalidData,
                ))
            {
                assert_eq!(snapshot.processed_count(), 1);
                assert_eq!(snapshot.changed_count(), 1);
            }
        }
    }

    #[test]
    fn invalid_wall_clock_and_post_poll_deadline_fail_before_publication() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let monotonic = Arc::new(FakeMonotonicClock::default());
        let source = FakeSource {
            events: Arc::clone(&events),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: None,
        };
        let publisher = FakePublisher {
            events,
            calls: Arc::clone(&calls),
            result: Ok(completed_report()),
        };
        let monotonic_clock: Arc<dyn Clock> = monotonic.clone();
        let mut execution = CodexQuotaExecution::new(
            monotonic_clock,
            FixedWallClock(Err(CodexQuotaClockErrorCode::InvalidTime)),
            source,
            publisher,
            Arc::clone(&latest),
        );
        let (_coordinator, first_permit) = permit(None);
        assert_eq!(execution.run(&first_permit), RefreshOutcome::Failed);
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(
            latest.lock().test_value("latest").failure(),
            Some(CodexQuotaRefreshFailure::Clock(
                CodexQuotaClockErrorCode::InvalidTime
            ))
        );

        let latest = Arc::new(Mutex::new(CodexQuotaRefreshSnapshot::not_run()));
        let calls = Arc::new(AtomicUsize::new(0));
        let advance = Arc::clone(&monotonic);
        let source = FakeSource {
            events: Arc::new(Mutex::new(Vec::new())),
            result: Ok(normalized(OBSERVED_AT_MS)),
            on_poll: Some(Box::new(move || advance.set(10))),
        };
        let publisher = FakePublisher {
            events: Arc::new(Mutex::new(Vec::new())),
            calls: Arc::clone(&calls),
            result: Ok(completed_report()),
        };
        let monotonic_clock: Arc<dyn Clock> = monotonic.clone();
        let mut execution = CodexQuotaExecution::new(
            monotonic_clock,
            FixedWallClock(Ok(OBSERVED_AT_MS)),
            source,
            publisher,
            Arc::clone(&latest),
        );
        let (_coordinator, permit) = permit(Some(RefreshDeadline::from_millis(10)));
        assert_eq!(execution.run(&permit), RefreshOutcome::DeadlineExceeded);
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(
            latest.lock().test_value("latest").failure(),
            Some(CodexQuotaRefreshFailure::Control(
                PortErrorCode::DeadlineExceeded
            ))
        );
    }
}
