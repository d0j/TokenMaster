use std::{
    fmt,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use tokenmaster_engine::{
    Clock, MonotonicTime, RefreshAdmission, RefreshDeadline, RefreshOutcome, RefreshPermit,
    RefreshRequestId, RefreshUrgency, RefreshWorker, WorkerCompletionKind, WorkerError,
    WorkerErrorCode, WorkerPhase,
};
use tokenmaster_product::{ProductAttemptGeneration, ProductReducer, ProductSnapshot};
use tokenmaster_query::{
    BenefitCurrentRequest, BenefitCurrentSnapshot, BenefitEnvelope, GitEnvelope, GitOutputRequest,
    GitOutputSnapshot, LatestActivityPage, LatestActivityRequest, PageSize,
    ProductDataStatusEnvelope, QueryClock, QueryEnvelope, QueryError, QueryErrorCode, QueryService,
    QuotaCurrentRequest, QuotaCurrentSnapshot, QuotaEnvelope, SystemQueryClock, UsageAnalytics,
    UsageAnalyticsRequest, UsageBreakdownKind, UsageRange, UsageSeriesSelection, UsageSessionPage,
    UsageSessionPageRequest, UsageTimeZone, WeekStart,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopQueryPlan {
    analytics: UsageAnalyticsRequest,
    quota: QuotaCurrentRequest,
    benefit: Option<BenefitCurrentRequest>,
    git: GitOutputRequest,
    activity: LatestActivityRequest,
    sessions: UsageSessionPageRequest,
}

impl DesktopQueryPlan {
    pub const MAX_SERIES_POINTS: usize = 240;
    pub const MAX_PAGE_ROWS: usize = 256;
    pub const MAX_REPOSITORIES: usize = 32;

    pub fn overview() -> Result<Self, DesktopControllerError> {
        let page_size = PageSize::new(Self::MAX_PAGE_ROWS).map_err(map_query_error)?;
        let analytics = UsageAnalyticsRequest::new(
            UsageRange::today(),
            UsageTimeZone::system(),
            WeekStart::Monday,
            UsageSeriesSelection::Daily,
            Vec::new(),
            vec![
                UsageBreakdownKind::Model,
                UsageBreakdownKind::Project,
                UsageBreakdownKind::Provider,
                UsageBreakdownKind::Profile,
            ],
        )
        .map_err(map_query_error)?;
        let quota = QuotaCurrentRequest::new(Vec::new()).map_err(map_query_error)?;
        let git = GitOutputRequest::new(
            UsageRange::today(),
            WeekStart::Monday,
            Vec::new(),
            Self::MAX_REPOSITORIES,
        )
        .map_err(map_query_error)?;
        let sessions =
            UsageSessionPageRequest::first(page_size, Vec::new()).map_err(map_query_error)?;
        Ok(Self {
            analytics,
            quota,
            benefit: None,
            git,
            activity: LatestActivityRequest::first(page_size),
            sessions,
        })
    }

    #[must_use]
    pub fn with_benefit_request(mut self, request: BenefitCurrentRequest) -> Self {
        self.benefit = Some(request);
        self
    }

    #[must_use]
    pub const fn benefit_request(&self) -> Option<&BenefitCurrentRequest> {
        self.benefit.as_ref()
    }
}

pub trait DesktopQuerySource: Send + 'static {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError>;

    fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError>;

    fn quota_windows(
        &mut self,
        request: QuotaCurrentRequest,
    ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError>;

    fn benefit_inventory(
        &mut self,
        request: BenefitCurrentRequest,
    ) -> Result<BenefitEnvelope<BenefitCurrentSnapshot>, QueryError>;

    fn git_output(
        &mut self,
        request: GitOutputRequest,
    ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError>;

    fn latest_activity(
        &mut self,
        request: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError>;

    fn usage_sessions(
        &mut self,
        request: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError>;
}

impl<C> DesktopQuerySource for QueryService<C>
where
    C: QueryClock + Send + 'static,
{
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
        QueryService::product_data_status(self)
    }

    fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
        QueryService::usage_analytics(self, request)
    }

    fn quota_windows(
        &mut self,
        request: QuotaCurrentRequest,
    ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        QueryService::quota_windows(self, request)
    }

    fn benefit_inventory(
        &mut self,
        request: BenefitCurrentRequest,
    ) -> Result<BenefitEnvelope<BenefitCurrentSnapshot>, QueryError> {
        QueryService::benefit_inventory(self, request)
    }

    fn git_output(
        &mut self,
        request: GitOutputRequest,
    ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
        QueryService::git_output(self, request)
    }

    fn latest_activity(
        &mut self,
        request: LatestActivityRequest,
    ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
        QueryService::latest_activity(self, request)
    }

    fn usage_sessions(
        &mut self,
        request: UsageSessionPageRequest,
    ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
        QueryService::usage_sessions(self, request)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopAttempt(u64);

impl DesktopAttempt {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<RefreshRequestId> for DesktopAttempt {
    fn from(value: RefreshRequestId) -> Self {
        Self(value.get())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopRefreshReceipt(u64);

impl DesktopRefreshReceipt {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<RefreshRequestId> for DesktopRefreshReceipt {
    fn from(value: RefreshRequestId) -> Self {
        Self(value.get())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopRefreshUrgency {
    Hint,
    Periodic,
    Interactive,
    Recovery,
}

impl DesktopRefreshUrgency {
    const fn engine(self) -> RefreshUrgency {
        match self {
            Self::Hint => RefreshUrgency::Hint,
            Self::Periodic => RefreshUrgency::Periodic,
            Self::Interactive => RefreshUrgency::Interactive,
            Self::Recovery => RefreshUrgency::Recovery,
        }
    }

    const fn budget_ms(self) -> u64 {
        match self {
            Self::Hint => 4_000,
            Self::Periodic => 8_000,
            Self::Interactive => 12_000,
            Self::Recovery => 16_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopRefreshAdmission {
    Started {
        attempt: DesktopAttempt,
    },
    Coalesced {
        receipt: DesktopRefreshReceipt,
        active_attempt: DesktopAttempt,
    },
    DeadlineExceeded {
        receipt: DesktopRefreshReceipt,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopRefreshOutcome {
    Completed,
    Busy,
    Cancelled,
    DeadlineExceeded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopRefreshCompletion {
    attempt: DesktopAttempt,
    outcome: DesktopRefreshOutcome,
    follow_up_started: bool,
}

impl DesktopRefreshCompletion {
    #[must_use]
    pub const fn attempt(self) -> DesktopAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn outcome(self) -> DesktopRefreshOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn follow_up_started(self) -> bool {
        self.follow_up_started
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopControllerErrorCode {
    Closed,
    Faulted,
    CapacityExceeded,
    Unavailable,
    InvalidPlan,
    VersionMismatch,
    CorruptArchive,
    Internal,
}

impl DesktopControllerErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Faulted => "faulted",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::Unavailable => "unavailable",
            Self::InvalidPlan => "invalid_plan",
            Self::VersionMismatch => "version_mismatch",
            Self::CorruptArchive => "corrupt_archive",
            Self::Internal => "internal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopControllerError {
    code: DesktopControllerErrorCode,
}

impl DesktopControllerError {
    const fn new(code: DesktopControllerErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> DesktopControllerErrorCode {
        self.code
    }

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        self.code.stable_code()
    }
}

impl fmt::Display for DesktopControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

impl std::error::Error for DesktopControllerError {}

struct DesktopMonotonicClock {
    started: Instant,
}

impl DesktopMonotonicClock {
    fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Clock for DesktopMonotonicClock {
    fn now(&self) -> MonotonicTime {
        let milliseconds = self.started.elapsed().as_millis();
        let bounded = u64::try_from(milliseconds).unwrap_or(u64::MAX);
        MonotonicTime::from_millis(bounded)
    }
}

type LatestSnapshot = Arc<Mutex<Option<Arc<ProductSnapshot>>>>;

pub struct DesktopController {
    clock: Arc<dyn Clock>,
    worker: RefreshWorker,
    latest: LatestSnapshot,
}

impl DesktopController {
    pub fn open(
        path: impl AsRef<Path>,
        plan: DesktopQueryPlan,
    ) -> Result<Self, DesktopControllerError> {
        let source = QueryService::open(path, SystemQueryClock::new()).map_err(map_query_error)?;
        Self::spawn(source, plan)
    }

    pub fn spawn<S>(source: S, plan: DesktopQueryPlan) -> Result<Self, DesktopControllerError>
    where
        S: DesktopQuerySource,
    {
        let clock: Arc<dyn Clock> = Arc::new(DesktopMonotonicClock::new());
        Self::spawn_with_clock(source, plan, clock)
    }

    fn spawn_with_clock<S>(
        mut source: S,
        plan: DesktopQueryPlan,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, DesktopControllerError>
    where
        S: DesktopQuerySource,
    {
        let worker_clock = clock.clone();
        let latest = Arc::new(Mutex::new(None));
        let worker_latest = latest.clone();
        let execute_clock = clock.clone();
        let mut reducer = ProductReducer::new();
        let worker = RefreshWorker::spawn(worker_clock, move |permit| {
            execute_attempt(
                &mut source,
                &plan,
                &mut reducer,
                permit,
                execute_clock.as_ref(),
                &worker_latest,
            )
        })
        .map_err(map_worker_error)?;
        Ok(Self {
            clock,
            worker,
            latest,
        })
    }

    pub fn refresh(
        &self,
        urgency: DesktopRefreshUrgency,
    ) -> Result<DesktopRefreshAdmission, DesktopControllerError> {
        let deadline_ms = self
            .clock
            .now()
            .as_millis()
            .checked_add(urgency.budget_ms())
            .ok_or_else(|| DesktopControllerError::new(DesktopControllerErrorCode::Internal))?;
        self.worker
            .submit(
                urgency.engine(),
                Some(RefreshDeadline::from_millis(deadline_ms)),
            )
            .map(map_admission)
            .map_err(map_worker_error)
    }

    pub fn cancel(&self, attempt: DesktopAttempt) -> Result<(), DesktopControllerError> {
        let request_id = RefreshRequestId::new(attempt.get())
            .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))?;
        self.worker.cancel(request_id).map_err(map_worker_error)
    }

    pub fn try_completion(
        &self,
    ) -> Result<Option<DesktopRefreshCompletion>, DesktopControllerError> {
        self.worker
            .try_completion()
            .map(|completion| completion.map(map_completion))
            .map_err(map_worker_error)
    }

    pub fn take_snapshot(&self) -> Result<Option<Arc<ProductSnapshot>>, DesktopControllerError> {
        Ok(lock_latest(&self.latest)?.take())
    }

    pub fn shutdown(&mut self) -> Result<(), DesktopControllerError> {
        match self.worker.shutdown().map_err(map_worker_error)? {
            WorkerPhase::Stopped => Ok(()),
            WorkerPhase::Faulted => Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Faulted,
            )),
            WorkerPhase::Running | WorkerPhase::ShuttingDown => Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Internal,
            )),
        }
    }
}

fn execute_attempt<S: DesktopQuerySource>(
    source: &mut S,
    plan: &DesktopQueryPlan,
    reducer: &mut ProductReducer,
    permit: &RefreshPermit,
    clock: &dyn Clock,
    latest: &LatestSnapshot,
) -> RefreshOutcome {
    let Some(attempt) = ProductAttemptGeneration::new(permit.id().get()) else {
        return RefreshOutcome::Failed;
    };
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    let result = match source.product_data_status() {
        Ok(value) => reducer.publish_data_status(attempt, value),
        Err(error) => reducer.fail_data_status(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    let result = match source.usage_analytics(plan.analytics.clone()) {
        Ok(value) => reducer.publish_analytics(attempt, value),
        Err(error) => reducer.fail_analytics(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    let result = match source.quota_windows(plan.quota.clone()) {
        Ok(value) => reducer.publish_quota(attempt, value),
        Err(error) => reducer.fail_quota(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    let result = match plan.benefit.clone() {
        Some(request) => match source.benefit_inventory(request) {
            Ok(value) => reducer.publish_benefit(attempt, value),
            Err(error) => reducer.fail_benefit(attempt, error.code()),
        },
        None => reducer.fail_benefit(attempt, QueryErrorCode::Unavailable),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    let result = match source.git_output(plan.git.clone()) {
        Ok(value) => reducer.publish_git(attempt, value),
        Err(error) => reducer.fail_git(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    let result = match source.latest_activity(plan.activity) {
        Ok(value) => reducer.publish_activity(attempt, value),
        Err(error) => reducer.fail_activity(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    let result = match source.usage_sessions(plan.sessions.clone()) {
        Ok(value) => reducer.publish_sessions(attempt, value),
        Err(error) => reducer.fail_sessions(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, clock) {
        return outcome;
    }

    match lock_latest(latest) {
        Ok(mut slot) => {
            *slot = Some(reducer.snapshot());
            RefreshOutcome::Completed
        }
        Err(_) => RefreshOutcome::Failed,
    }
}

fn stop_outcome(permit: &RefreshPermit, clock: &dyn Clock) -> Option<RefreshOutcome> {
    if permit.is_cancelled() {
        Some(RefreshOutcome::Cancelled)
    } else if permit.deadline_exceeded(clock.now()) {
        Some(RefreshOutcome::DeadlineExceeded)
    } else {
        None
    }
}

fn lock_latest(
    latest: &LatestSnapshot,
) -> Result<MutexGuard<'_, Option<Arc<ProductSnapshot>>>, DesktopControllerError> {
    latest
        .lock()
        .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))
}

fn map_admission(value: RefreshAdmission) -> DesktopRefreshAdmission {
    match value {
        RefreshAdmission::Started(permit) => DesktopRefreshAdmission::Started {
            attempt: permit.id().into(),
        },
        RefreshAdmission::Coalesced {
            request_id,
            active_request_id,
        } => DesktopRefreshAdmission::Coalesced {
            receipt: request_id.into(),
            active_attempt: active_request_id.into(),
        },
        RefreshAdmission::DeadlineExceeded { request_id } => {
            DesktopRefreshAdmission::DeadlineExceeded {
                receipt: request_id.into(),
            }
        }
    }
}

fn map_completion(value: tokenmaster_engine::WorkerCompletion) -> DesktopRefreshCompletion {
    let outcome = if value.kind() == WorkerCompletionKind::Panicked {
        DesktopRefreshOutcome::Failed
    } else {
        match value.outcome() {
            RefreshOutcome::Completed => DesktopRefreshOutcome::Completed,
            RefreshOutcome::Busy => DesktopRefreshOutcome::Busy,
            RefreshOutcome::Cancelled => DesktopRefreshOutcome::Cancelled,
            RefreshOutcome::DeadlineExceeded => DesktopRefreshOutcome::DeadlineExceeded,
            RefreshOutcome::Failed => DesktopRefreshOutcome::Failed,
        }
    };
    DesktopRefreshCompletion {
        attempt: value.request_id().into(),
        outcome,
        follow_up_started: value.follow_up_started(),
    }
}

fn map_worker_error(error: WorkerError) -> DesktopControllerError {
    let code = match error.code() {
        WorkerErrorCode::Closed => DesktopControllerErrorCode::Closed,
        WorkerErrorCode::Faulted => DesktopControllerErrorCode::Faulted,
        WorkerErrorCode::CapacityExceeded => DesktopControllerErrorCode::CapacityExceeded,
        WorkerErrorCode::StaleRequest => DesktopControllerErrorCode::InvalidPlan,
        WorkerErrorCode::Unavailable => DesktopControllerErrorCode::Unavailable,
        WorkerErrorCode::Internal => DesktopControllerErrorCode::Internal,
    };
    DesktopControllerError::new(code)
}

fn map_query_error(error: QueryError) -> DesktopControllerError {
    let code = match error.code() {
        QueryErrorCode::InvalidValue
        | QueryErrorCode::InvalidTimeZone
        | QueryErrorCode::SystemTimeZoneUnavailable
        | QueryErrorCode::UnsupportedTimeBoundary => DesktopControllerErrorCode::InvalidPlan,
        QueryErrorCode::CapacityExceeded => DesktopControllerErrorCode::CapacityExceeded,
        QueryErrorCode::VersionMismatch => DesktopControllerErrorCode::VersionMismatch,
        QueryErrorCode::CorruptArchive => DesktopControllerErrorCode::CorruptArchive,
        QueryErrorCode::Unavailable
        | QueryErrorCode::StaleSnapshot
        | QueryErrorCode::DeadlineExceeded => DesktopControllerErrorCode::Unavailable,
        QueryErrorCode::Overflow | QueryErrorCode::Internal => DesktopControllerErrorCode::Internal,
    };
    DesktopControllerError::new(code)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    struct ManualClock(AtomicU64);

    impl ManualClock {
        fn new(milliseconds: u64) -> Self {
            Self(AtomicU64::new(milliseconds))
        }

        fn set(&self, milliseconds: u64) {
            self.0.store(milliseconds, Ordering::Release);
        }
    }

    impl Clock for ManualClock {
        fn now(&self) -> MonotonicTime {
            MonotonicTime::from_millis(self.0.load(Ordering::Acquire))
        }
    }

    struct DeadlineSource {
        clock: Arc<ManualClock>,
    }

    impl DesktopQuerySource for DeadlineSource {
        fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
            self.clock.set(DesktopRefreshUrgency::Hint.budget_ms());
            Err(invalid_query())
        }

        fn usage_analytics(
            &mut self,
            _request: UsageAnalyticsRequest,
        ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
            unreachable!("deadline stops before analytics")
        }

        fn quota_windows(
            &mut self,
            _request: QuotaCurrentRequest,
        ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
            unreachable!("deadline stops before quota")
        }

        fn benefit_inventory(
            &mut self,
            _request: BenefitCurrentRequest,
        ) -> Result<BenefitEnvelope<BenefitCurrentSnapshot>, QueryError> {
            unreachable!("deadline stops before benefit")
        }

        fn git_output(
            &mut self,
            _request: GitOutputRequest,
        ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
            unreachable!("deadline stops before git")
        }

        fn latest_activity(
            &mut self,
            _request: LatestActivityRequest,
        ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
            unreachable!("deadline stops before activity")
        }

        fn usage_sessions(
            &mut self,
            _request: UsageSessionPageRequest,
        ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
            unreachable!("deadline stops before sessions")
        }
    }

    #[test]
    fn attempt_deadline_discards_partial_reducer_state() {
        let clock = Arc::new(ManualClock::new(0));
        let controller_clock: Arc<dyn Clock> = clock.clone();
        let mut controller = DesktopController::spawn_with_clock(
            DeadlineSource { clock },
            DesktopQueryPlan::overview().expect("overview plan"),
            controller_clock,
        )
        .expect("controller starts");
        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("refresh starts");

        let deadline = Instant::now() + Duration::from_secs(2);
        let completion = loop {
            if let Some(completion) = controller.try_completion().expect("worker healthy") {
                break completion;
            }
            assert!(Instant::now() < deadline, "completion timed out");
            thread::yield_now();
        };
        assert_eq!(
            completion.outcome(),
            DesktopRefreshOutcome::DeadlineExceeded
        );
        assert!(
            controller
                .take_snapshot()
                .expect("latest slot healthy")
                .is_none()
        );
        controller.shutdown().expect("controller stops");
    }

    fn invalid_query() -> QueryError {
        PageSize::new(0).expect_err("zero page size is invalid")
    }
}
