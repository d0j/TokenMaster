use std::{
    fmt,
    num::NonZeroU64,
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use tokenmaster_engine::{
    Clock, MonotonicTime, RefreshAdmission, RefreshDeadline, RefreshOutcome, RefreshPermit,
    RefreshRequestId, RefreshSubmitter, RefreshUrgency, RefreshWorker, WorkerCompletion,
    WorkerCompletionKind, WorkerCompletionNotifier, WorkerError, WorkerErrorCode, WorkerPhase,
};
use tokenmaster_product::{
    ProductAttemptGeneration, ProductGeneration, ProductGitRuntimeHealth, ProductPublishOutcome,
    ProductQuotaRuntimeHealth, ProductReducer, ProductReminderRuntimeHealth,
    ProductRuntimeGeneration, ProductRuntimeObservationError, ProductSessionDetailSelection,
    ProductSessionDetailSelectionGeneration, ProductSnapshot, ProductUsageRuntimeHealth,
};
use tokenmaster_query::{
    BenefitOverviewEnvelope, BenefitOverviewRequest, BenefitOverviewSnapshot, GitEnvelope,
    GitOutputRequest, GitOutputSnapshot, LatestActivityPage, LatestActivityRequest, PageSize,
    ProductDataStatusEnvelope, QueryClock, QueryEnvelope, QueryError, QueryErrorCode, QueryService,
    QuotaCurrentSnapshot, QuotaEnvelope, SystemQueryClock, UsageAnalytics, UsageAnalyticsRequest,
    UsageBreakdownKind, UsageRange, UsageSeriesSelection, UsageSessionDetailResult,
    UsageSessionKey, UsageSessionPage, UsageSessionPageRequest, UsageTimeZone, WeekStart,
};

use crate::presentation::DesktopSnapshotEpoch;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopQueryPlan {
    analytics: UsageAnalyticsRequest,
    git: GitOutputRequest,
    activity: LatestActivityRequest,
    sessions: UsageSessionPageRequest,
}

impl DesktopQueryPlan {
    pub const MAX_SERIES_POINTS: usize = 240;
    pub const HISTORY_DAYS: u16 = 30;
    pub const MAX_DASHBOARD_ROWS: usize = 12;
    pub const MAX_SESSION_ROWS: usize = 64;
    pub const MAX_REPOSITORIES: usize = 32;

    pub fn overview() -> Result<Self, DesktopControllerError> {
        let overview_page_size =
            PageSize::new(Self::MAX_DASHBOARD_ROWS).map_err(map_query_error)?;
        let session_page_size = PageSize::new(Self::MAX_SESSION_ROWS).map_err(map_query_error)?;
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
        let git = GitOutputRequest::new(
            UsageRange::today(),
            WeekStart::Monday,
            Vec::new(),
            Self::MAX_REPOSITORIES,
        )
        .map_err(map_query_error)?;
        let sessions = UsageSessionPageRequest::first(session_page_size, Vec::new())
            .map_err(map_query_error)?;
        Ok(Self {
            analytics,
            git,
            activity: LatestActivityRequest::first(overview_page_size),
            sessions,
        })
    }

    #[must_use]
    pub const fn default_history_range_preset() -> DesktopHistoryRangePreset {
        DesktopHistoryRangePreset::Recent30Days
    }

    fn history_request(
        preset: DesktopHistoryRangePreset,
    ) -> Result<UsageAnalyticsRequest, DesktopControllerError> {
        UsageAnalyticsRequest::new(
            UsageRange::recent_days(preset.day_count()).map_err(map_query_error)?,
            UsageTimeZone::system(),
            WeekStart::Monday,
            UsageSeriesSelection::Daily,
            Vec::new(),
            vec![UsageBreakdownKind::Model, UsageBreakdownKind::Project],
        )
        .map_err(map_query_error)
    }
}

pub trait DesktopQuerySource: Send + 'static {
    fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError>;

    fn usage_analytics(
        &mut self,
        request: UsageAnalyticsRequest,
    ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError>;

    fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError>;

    fn benefit_overview(
        &mut self,
        request: BenefitOverviewRequest,
    ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError>;

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

    fn usage_session_detail(
        &mut self,
        key: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError>;
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

    fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
        QueryService::quota_overview(self)
    }

    fn benefit_overview(
        &mut self,
        request: BenefitOverviewRequest,
    ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
        QueryService::benefit_overview(self, request)
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

    fn usage_session_detail(
        &mut self,
        key: UsageSessionKey,
    ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
        QueryService::usage_session_detail(self, key)
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
pub struct DesktopSessionDetailIntent {
    snapshot_epoch: DesktopSnapshotEpoch,
    product_generation: tokenmaster_product::ProductGeneration,
    selection: ProductSessionDetailSelection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopHistoryRangePreset {
    Recent1Day,
    Recent7Days,
    Recent30Days,
}

impl DesktopHistoryRangePreset {
    #[must_use]
    pub const fn day_count(self) -> u16 {
        match self {
            Self::Recent1Day => 1,
            Self::Recent7Days => 7,
            Self::Recent30Days => 30,
        }
    }

    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Recent1Day => "recent_1_day",
            Self::Recent7Days => "recent_7_days",
            Self::Recent30Days => "recent_30_days",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopHistoryRangeGeneration(NonZeroU64);

impl DesktopHistoryRangeGeneration {
    #[must_use]
    pub const fn new(generation: u64) -> Option<Self> {
        match NonZeroU64::new(generation) {
            Some(generation) => Some(Self(generation)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopHistoryRangeIntent {
    snapshot_epoch: DesktopSnapshotEpoch,
    product_generation: ProductGeneration,
    generation: DesktopHistoryRangeGeneration,
    preset: DesktopHistoryRangePreset,
}

impl DesktopHistoryRangeIntent {
    #[must_use]
    pub const fn new(
        snapshot_epoch: DesktopSnapshotEpoch,
        product_generation: ProductGeneration,
        generation: DesktopHistoryRangeGeneration,
        preset: DesktopHistoryRangePreset,
    ) -> Self {
        Self {
            snapshot_epoch,
            product_generation,
            generation,
            preset,
        }
    }

    #[must_use]
    pub const fn snapshot_epoch(self) -> DesktopSnapshotEpoch {
        self.snapshot_epoch
    }
    #[must_use]
    pub const fn product_generation(self) -> ProductGeneration {
        self.product_generation
    }
    #[must_use]
    pub const fn generation(self) -> DesktopHistoryRangeGeneration {
        self.generation
    }
    #[must_use]
    pub const fn preset(self) -> DesktopHistoryRangePreset {
        self.preset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSessionPageDirection {
    Newest,
    Next,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopSessionNavigationGeneration(NonZeroU64);

impl DesktopSessionNavigationGeneration {
    #[must_use]
    pub const fn new(generation: u64) -> Option<Self> {
        match NonZeroU64::new(generation) {
            Some(generation) => Some(Self(generation)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopSessionPageIntent {
    snapshot_epoch: DesktopSnapshotEpoch,
    product_generation: tokenmaster_product::ProductGeneration,
    navigation_generation: DesktopSessionNavigationGeneration,
    direction: DesktopSessionPageDirection,
}

impl DesktopSessionPageIntent {
    #[must_use]
    pub const fn new(
        snapshot_epoch: DesktopSnapshotEpoch,
        product_generation: tokenmaster_product::ProductGeneration,
        navigation_generation: DesktopSessionNavigationGeneration,
        direction: DesktopSessionPageDirection,
    ) -> Self {
        Self {
            snapshot_epoch,
            product_generation,
            navigation_generation,
            direction,
        }
    }

    #[must_use]
    pub const fn snapshot_epoch(self) -> DesktopSnapshotEpoch {
        self.snapshot_epoch
    }

    #[must_use]
    pub const fn product_generation(self) -> tokenmaster_product::ProductGeneration {
        self.product_generation
    }

    #[must_use]
    pub const fn navigation_generation(self) -> DesktopSessionNavigationGeneration {
        self.navigation_generation
    }

    #[must_use]
    pub const fn direction(self) -> DesktopSessionPageDirection {
        self.direction
    }
}

impl DesktopSessionDetailIntent {
    #[must_use]
    pub const fn new(
        snapshot_epoch: DesktopSnapshotEpoch,
        product_generation: tokenmaster_product::ProductGeneration,
        selection: ProductSessionDetailSelection,
    ) -> Self {
        Self {
            snapshot_epoch,
            product_generation,
            selection,
        }
    }

    #[must_use]
    pub const fn snapshot_epoch(self) -> DesktopSnapshotEpoch {
        self.snapshot_epoch
    }

    #[must_use]
    pub const fn product_generation(self) -> tokenmaster_product::ProductGeneration {
        self.product_generation
    }

    #[must_use]
    pub const fn selection(self) -> ProductSessionDetailSelection {
        self.selection
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopRuntimeObservation {
    generation: ProductRuntimeGeneration,
    usage: Result<ProductUsageRuntimeHealth, ProductRuntimeObservationError>,
    quota: Result<ProductQuotaRuntimeHealth, ProductRuntimeObservationError>,
    reminder: Result<ProductReminderRuntimeHealth, ProductRuntimeObservationError>,
    git: Result<ProductGitRuntimeHealth, ProductRuntimeObservationError>,
}

impl DesktopRuntimeObservation {
    #[must_use]
    pub const fn new(
        generation: ProductRuntimeGeneration,
        usage: Result<ProductUsageRuntimeHealth, ProductRuntimeObservationError>,
        quota: Result<ProductQuotaRuntimeHealth, ProductRuntimeObservationError>,
        reminder: Result<ProductReminderRuntimeHealth, ProductRuntimeObservationError>,
        git: Result<ProductGitRuntimeHealth, ProductRuntimeObservationError>,
    ) -> Self {
        Self {
            generation,
            usage,
            quota,
            reminder,
            git,
        }
    }

    #[must_use]
    pub const fn generation(self) -> ProductRuntimeGeneration {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopRuntimeObservationOutcome {
    Accepted,
    IgnoredNotNewer,
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
    Busy,
    NotifierAlreadyAttached,
    CapacityExceeded,
    Unavailable,
    InvalidPlan,
    VersionMismatch,
    CorruptArchive,
    StaleSelection,
    StaleNavigation,
    StaleHistoryRange,
    Internal,
}

impl DesktopControllerErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Faulted => "faulted",
            Self::Busy => "busy",
            Self::NotifierAlreadyAttached => "notifier_already_attached",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::Unavailable => "unavailable",
            Self::InvalidPlan => "invalid_plan",
            Self::VersionMismatch => "version_mismatch",
            Self::CorruptArchive => "corrupt_archive",
            Self::StaleSelection => "stale_selection",
            Self::StaleNavigation => "stale_navigation",
            Self::StaleHistoryRange => "stale_history_range",
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
type SnapshotNotifier = Arc<Mutex<Option<Arc<dyn DesktopSnapshotNotifier>>>>;
type TerminalNavigationNotifier = Arc<Mutex<Option<Arc<dyn DesktopTerminalNavigationNotifier>>>>;
type TerminalHistoryRangeNotifier =
    Arc<Mutex<Option<Arc<dyn DesktopTerminalHistoryRangeNotifier>>>>;
type PublishedProductGeneration = Arc<Mutex<Option<ProductGeneration>>>;
type RuntimeObservationSlot = Arc<Mutex<RuntimeObservationState>>;
type DesktopWorkSlot = Arc<Mutex<DesktopWorkState>>;

#[derive(Clone)]
struct DesktopPublication {
    latest: LatestSnapshot,
    notifier: SnapshotNotifier,
    published_generation: PublishedProductGeneration,
    runtime_observation: RuntimeObservationSlot,
}

#[derive(Default)]
struct RuntimeObservationState {
    latest_generation: Option<ProductRuntimeGeneration>,
    pending: Option<DesktopRuntimeObservation>,
}

struct DesktopWorkState {
    refresh_attempt: Option<u64>,
    latest_selection_generation: Option<ProductSessionDetailSelectionGeneration>,
    pending_selection: Option<PendingDesktopSessionDetail>,
    active_selection_attempt: Option<u64>,
    navigation_high_water: Option<DesktopSessionNavigationGeneration>,
    current_navigation: Option<ActiveDesktopSessionPage>,
    pending_navigation: Option<PendingDesktopSessionPage>,
    published_history_preset: DesktopHistoryRangePreset,
    history_range_high_water: Option<DesktopHistoryRangeGeneration>,
    current_history_range: Option<ActiveDesktopHistoryRange>,
    pending_history_range: Option<PendingDesktopHistoryRange>,
}

impl Default for DesktopWorkState {
    fn default() -> Self {
        Self {
            refresh_attempt: None,
            latest_selection_generation: None,
            pending_selection: None,
            active_selection_attempt: None,
            navigation_high_water: None,
            current_navigation: None,
            pending_navigation: None,
            published_history_preset: DesktopQueryPlan::default_history_range_preset(),
            history_range_high_water: None,
            current_history_range: None,
            pending_history_range: None,
        }
    }
}

#[derive(Clone, Copy)]
struct DesktopWorkBatch {
    refresh: bool,
    selection: Option<DesktopSessionDetailIntent>,
    navigation: Option<DesktopSessionPageIntent>,
    history_range: Option<DesktopHistoryRangeIntent>,
}

#[derive(Clone, Copy)]
struct PendingDesktopSessionDetail {
    attempt: u64,
    prerequisite_attempt: Option<u64>,
    intent: DesktopSessionDetailIntent,
}

#[derive(Clone, Copy)]
struct PendingDesktopSessionPage {
    attempt: u64,
    intent: DesktopSessionPageIntent,
}

#[derive(Clone, Copy)]
struct ActiveDesktopSessionPage {
    attempt: u64,
    prerequisite_attempt: Option<u64>,
    intent: DesktopSessionPageIntent,
}

#[derive(Clone, Copy)]
struct PendingDesktopHistoryRange {
    attempt: u64,
    intent: DesktopHistoryRangeIntent,
}

#[derive(Clone, Copy)]
struct ActiveDesktopHistoryRange {
    attempt: u64,
    prerequisite_attempt: Option<u64>,
    rebound_product_generation: Option<ProductGeneration>,
    intent: DesktopHistoryRangeIntent,
}

struct DesktopExecutionContext<'a> {
    plan: &'a DesktopQueryPlan,
    clock: &'a dyn Clock,
    publication: &'a DesktopPublication,
    snapshot_epoch: &'a AtomicU64,
    work: &'a DesktopWorkSlot,
}

pub trait DesktopSnapshotNotifier: Send + Sync + 'static {
    fn snapshot_ready(&self);
}

pub trait DesktopTerminalNavigationNotifier: Send + Sync + 'static {
    fn navigation_terminal(&self, intent: DesktopSessionPageIntent);
}

pub trait DesktopTerminalHistoryRangeNotifier: Send + Sync + 'static {
    fn history_range_terminal(&self, intent: DesktopHistoryRangeIntent);
}

struct DesktopWorkCompletionNotifier {
    work: DesktopWorkSlot,
    terminal_notifier: TerminalNavigationNotifier,
    terminal_history_range_notifier: TerminalHistoryRangeNotifier,
}

impl WorkerCompletionNotifier for DesktopWorkCompletionNotifier {
    fn completion_ready(&self, completion: WorkerCompletion) {
        let _ = handle_worker_completion(
            &self.work,
            &self.terminal_notifier,
            &self.terminal_history_range_notifier,
            completion,
        );
    }
}

fn notify_terminal_navigation(
    notifier: &TerminalNavigationNotifier,
    intent: Option<DesktopSessionPageIntent>,
) {
    let Some(intent) = intent else {
        return;
    };
    let notifier = match lock_terminal_navigation_notifier(notifier) {
        Ok(notifier) => notifier.clone(),
        Err(_) => return,
    };
    if let Some(notifier) = notifier {
        notifier.navigation_terminal(intent);
    }
}

fn notify_terminal_history_range(
    notifier: &TerminalHistoryRangeNotifier,
    intent: Option<DesktopHistoryRangeIntent>,
) {
    let Some(intent) = intent else {
        return;
    };
    let notifier = match lock_terminal_history_range_notifier(notifier) {
        Ok(notifier) => notifier.clone(),
        Err(_) => return,
    };
    if let Some(notifier) = notifier {
        notifier.history_range_terminal(intent);
    }
}

fn handle_worker_completion(
    work: &DesktopWorkSlot,
    notifier: &TerminalNavigationNotifier,
    history_range_notifier: &TerminalHistoryRangeNotifier,
    completion: WorkerCompletion,
) -> Result<(), DesktopControllerError> {
    reconcile_session_detail_completion(work, completion)?;
    let navigation = reconcile_navigation_completion(work, completion)?;
    let history_range = reconcile_history_range_completion(work, completion)?;
    notify_terminal_navigation(notifier, navigation);
    notify_terminal_history_range(history_range_notifier, history_range);
    Ok(())
}

#[derive(Clone)]
pub struct DesktopSnapshotReceiver {
    latest: LatestSnapshot,
}

impl DesktopSnapshotReceiver {
    #[cfg(test)]
    pub(crate) fn empty_for_test() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
        }
    }

    pub fn take_snapshot(&self) -> Result<Option<Arc<ProductSnapshot>>, DesktopControllerError> {
        Ok(lock_latest(&self.latest)?.take())
    }

    pub fn has_snapshot(&self) -> Result<bool, DesktopControllerError> {
        Ok(lock_latest(&self.latest)?.is_some())
    }

    #[cfg(test)]
    pub(crate) fn replace_snapshot(
        &self,
        snapshot: Arc<ProductSnapshot>,
    ) -> Result<(), DesktopControllerError> {
        *lock_latest(&self.latest)? = Some(snapshot);
        Ok(())
    }
}

pub struct DesktopController {
    clock: Arc<dyn Clock>,
    worker: RefreshWorker,
    publication: DesktopPublication,
    snapshot_epoch: Arc<AtomicU64>,
    work: DesktopWorkSlot,
    terminal_navigation_notifier: TerminalNavigationNotifier,
    terminal_history_range_notifier: TerminalHistoryRangeNotifier,
}

#[derive(Clone)]
pub struct DesktopRefreshIngress {
    worker: RefreshSubmitter,
    clock: Arc<dyn Clock>,
    work: DesktopWorkSlot,
    terminal_navigation_notifier: TerminalNavigationNotifier,
    terminal_history_range_notifier: TerminalHistoryRangeNotifier,
}

impl DesktopRefreshIngress {
    pub fn refresh(
        &self,
        urgency: DesktopRefreshUrgency,
    ) -> Result<DesktopRefreshAdmission, DesktopControllerError> {
        let mut work = lock_work(&self.work)?;
        let deadline_ms = self
            .clock
            .now()
            .as_millis()
            .checked_add(urgency.budget_ms())
            .ok_or_else(|| DesktopControllerError::new(DesktopControllerErrorCode::Internal))?;
        let admission = self
            .worker
            .submit(
                urgency.engine(),
                Some(RefreshDeadline::from_millis(deadline_ms)),
            )
            .map(map_admission)
            .map_err(map_worker_error)?;
        if let Some(attempt) = scheduled_work_attempt(admission)? {
            work.refresh_attempt = Some(attempt);
            let superseded_navigation = work.current_navigation.map(|active| active.intent);
            let superseded_history_range = work.current_history_range.map(|active| active.intent);
            invalidate_navigation(&mut work);
            invalidate_history_range(&mut work);
            drop(work);
            notify_terminal_navigation(&self.terminal_navigation_notifier, superseded_navigation);
            notify_terminal_history_range(
                &self.terminal_history_range_notifier,
                superseded_history_range,
            );
        }
        Ok(admission)
    }
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
        let notifier = Arc::new(Mutex::new(None));
        let published_generation = Arc::new(Mutex::new(None));
        let runtime_observation = Arc::new(Mutex::new(RuntimeObservationState::default()));
        let publication = DesktopPublication {
            latest,
            notifier,
            published_generation,
            runtime_observation,
        };
        let worker_publication = publication.clone();
        let snapshot_epoch = Arc::new(AtomicU64::new(0));
        let work = Arc::new(Mutex::new(DesktopWorkState::default()));
        let terminal_navigation_notifier = Arc::new(Mutex::new(None));
        let terminal_history_range_notifier = Arc::new(Mutex::new(None));
        let worker_snapshot_epoch = Arc::clone(&snapshot_epoch);
        let worker_work = Arc::clone(&work);
        let execute_clock = clock.clone();
        let mut reducer = ProductReducer::new();
        let worker = RefreshWorker::spawn_notified(
            worker_clock,
            Arc::new(DesktopWorkCompletionNotifier {
                work: Arc::clone(&work),
                terminal_notifier: Arc::clone(&terminal_navigation_notifier),
                terminal_history_range_notifier: Arc::clone(&terminal_history_range_notifier),
            }),
            move |permit| {
                let context = DesktopExecutionContext {
                    plan: &plan,
                    clock: execute_clock.as_ref(),
                    publication: &worker_publication,
                    snapshot_epoch: &worker_snapshot_epoch,
                    work: &worker_work,
                };
                execute_work(&mut source, &mut reducer, permit, &context)
            },
        )
        .map_err(map_worker_error)?;
        Ok(Self {
            clock,
            worker,
            publication,
            snapshot_epoch,
            work,
            terminal_navigation_notifier,
            terminal_history_range_notifier,
        })
    }

    pub fn bind_snapshot_epoch(
        &mut self,
        epoch: DesktopSnapshotEpoch,
    ) -> Result<(), DesktopControllerError> {
        let worker = self.worker.snapshot().map_err(map_worker_error)?;
        match worker.phase() {
            WorkerPhase::Running => {}
            WorkerPhase::Faulted => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Faulted,
                ));
            }
            WorkerPhase::ShuttingDown | WorkerPhase::Stopped => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Closed,
                ));
            }
        }
        if self.snapshot_epoch.load(Ordering::Acquire) != 0
            || worker.active_request_id().is_some()
            || worker.pending_count() != 0
        {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Busy,
            ));
        }
        self.snapshot_epoch
            .compare_exchange(0, epoch.get(), Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Busy))
    }

    #[must_use]
    pub fn snapshot_epoch(&self) -> Option<DesktopSnapshotEpoch> {
        DesktopSnapshotEpoch::new(self.snapshot_epoch.load(Ordering::Acquire))
    }

    pub fn refresh(
        &self,
        urgency: DesktopRefreshUrgency,
    ) -> Result<DesktopRefreshAdmission, DesktopControllerError> {
        self.refresh_ingress().refresh(urgency)
    }

    #[must_use]
    pub fn refresh_ingress(&self) -> DesktopRefreshIngress {
        DesktopRefreshIngress {
            worker: self.worker.submitter(),
            clock: Arc::clone(&self.clock),
            work: Arc::clone(&self.work),
            terminal_navigation_notifier: Arc::clone(&self.terminal_navigation_notifier),
            terminal_history_range_notifier: Arc::clone(&self.terminal_history_range_notifier),
        }
    }

    pub fn request_session_detail(
        &self,
        intent: DesktopSessionDetailIntent,
    ) -> Result<DesktopRefreshAdmission, DesktopControllerError> {
        if self.snapshot_epoch() != Some(intent.snapshot_epoch()) {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleSelection,
            ));
        }
        let mut work = lock_work(&self.work)?;
        if history_range_is_active(&work) {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Busy,
            ));
        }
        if work.current_navigation.is_some() {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleSelection,
            ));
        }
        if work
            .latest_selection_generation
            .is_some_and(|current| intent.selection().generation() <= current)
        {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleSelection,
            ));
        }
        let admission = self.submit(DesktopRefreshUrgency::Interactive)?;
        if let Some(pending) = scheduled_session_detail(admission, intent)? {
            work.latest_selection_generation = Some(intent.selection().generation());
            work.pending_selection = Some(pending);
        }
        Ok(admission)
    }

    pub fn request_session_page(
        &self,
        intent: DesktopSessionPageIntent,
    ) -> Result<DesktopRefreshAdmission, DesktopControllerError> {
        if self.snapshot_epoch() != Some(intent.snapshot_epoch()) {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleNavigation,
            ));
        }
        let mut work = lock_work(&self.work)?;
        if history_range_is_active(&work) {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Busy,
            ));
        }
        if *lock_published_generation(&self.publication.published_generation)?
            != Some(intent.product_generation())
        {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleNavigation,
            ));
        }
        if work
            .navigation_high_water
            .is_some_and(|current| intent.navigation_generation() <= current)
        {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleNavigation,
            ));
        }
        let admission = self.submit(DesktopRefreshUrgency::Interactive)?;
        if let Some(active) = scheduled_navigation(admission, intent)? {
            work.navigation_high_water = Some(intent.navigation_generation());
            work.current_navigation = Some(active);
            work.pending_navigation = Some(PendingDesktopSessionPage {
                attempt: active.attempt,
                intent,
            });
            work.latest_selection_generation = None;
            work.pending_selection = None;
        }
        Ok(admission)
    }

    pub fn request_history_range(
        &self,
        intent: DesktopHistoryRangeIntent,
    ) -> Result<DesktopRefreshAdmission, DesktopControllerError> {
        let mut work = lock_work(&self.work)?;
        if self.snapshot_epoch() != Some(intent.snapshot_epoch())
            || *lock_published_generation(&self.publication.published_generation)?
                != Some(intent.product_generation())
        {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleHistoryRange,
            ));
        }
        if session_interaction_is_active(&work) {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Busy,
            ));
        }
        if intent.preset() == work.published_history_preset
            || work
                .history_range_high_water
                .is_some_and(|current| intent.generation() <= current)
        {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::StaleHistoryRange,
            ));
        }
        let admission = self.submit(DesktopRefreshUrgency::Interactive)?;
        if let Some(active) = scheduled_history_range(admission, intent)? {
            work.history_range_high_water = Some(intent.generation());
            work.current_history_range = Some(active);
            work.pending_history_range = Some(PendingDesktopHistoryRange {
                attempt: active.attempt,
                intent,
            });
        }
        Ok(admission)
    }

    fn submit(
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

    pub fn observe_runtime(
        &self,
        observation: DesktopRuntimeObservation,
    ) -> Result<DesktopRuntimeObservationOutcome, DesktopControllerError> {
        let mut state = lock_runtime_observation(&self.publication.runtime_observation)?;
        if state
            .latest_generation
            .is_some_and(|generation| observation.generation() <= generation)
        {
            return Ok(DesktopRuntimeObservationOutcome::IgnoredNotNewer);
        }
        state.latest_generation = Some(observation.generation());
        state.pending = Some(observation);
        Ok(DesktopRuntimeObservationOutcome::Accepted)
    }

    pub fn try_completion(
        &self,
    ) -> Result<Option<DesktopRefreshCompletion>, DesktopControllerError> {
        self.worker
            .try_completion()
            .map_err(map_worker_error)?
            .map(|completion| {
                handle_worker_completion(
                    &self.work,
                    &self.terminal_navigation_notifier,
                    &self.terminal_history_range_notifier,
                    completion,
                )?;
                Ok(map_completion(completion))
            })
            .transpose()
    }

    #[must_use]
    pub fn snapshot_receiver(&self) -> DesktopSnapshotReceiver {
        DesktopSnapshotReceiver {
            latest: self.publication.latest.clone(),
        }
    }

    pub fn attach_snapshot_notifier(
        &mut self,
        notifier: Arc<dyn DesktopSnapshotNotifier>,
    ) -> Result<(), DesktopControllerError> {
        let worker = self.worker.snapshot().map_err(map_worker_error)?;
        match worker.phase() {
            WorkerPhase::Running => {}
            WorkerPhase::Faulted => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Faulted,
                ));
            }
            WorkerPhase::ShuttingDown | WorkerPhase::Stopped => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Closed,
                ));
            }
        }
        if worker.active_request_id().is_some() || worker.pending_count() != 0 {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Busy,
            ));
        }
        let notify_existing = self.snapshot_receiver().has_snapshot()?;
        let mut current = lock_notifier(&self.publication.notifier)?;
        if current.is_some() {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::NotifierAlreadyAttached,
            ));
        }
        *current = Some(notifier.clone());
        drop(current);
        if notify_existing {
            notifier.snapshot_ready();
        }
        Ok(())
    }

    pub fn attach_terminal_navigation_notifier(
        &mut self,
        notifier: Arc<dyn DesktopTerminalNavigationNotifier>,
    ) -> Result<(), DesktopControllerError> {
        let worker = self.worker.snapshot().map_err(map_worker_error)?;
        match worker.phase() {
            WorkerPhase::Running => {}
            WorkerPhase::Faulted => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Faulted,
                ));
            }
            WorkerPhase::ShuttingDown | WorkerPhase::Stopped => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Closed,
                ));
            }
        }
        if worker.active_request_id().is_some() || worker.pending_count() != 0 {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Busy,
            ));
        }
        let mut current = lock_terminal_navigation_notifier(&self.terminal_navigation_notifier)?;
        if current.is_some() {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::NotifierAlreadyAttached,
            ));
        }
        *current = Some(notifier);
        Ok(())
    }

    pub fn attach_terminal_history_range_notifier(
        &mut self,
        notifier: Arc<dyn DesktopTerminalHistoryRangeNotifier>,
    ) -> Result<(), DesktopControllerError> {
        let worker = self.worker.snapshot().map_err(map_worker_error)?;
        match worker.phase() {
            WorkerPhase::Running => {}
            WorkerPhase::Faulted => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Faulted,
                ));
            }
            WorkerPhase::ShuttingDown | WorkerPhase::Stopped => {
                return Err(DesktopControllerError::new(
                    DesktopControllerErrorCode::Closed,
                ));
            }
        }
        if worker.active_request_id().is_some() || worker.pending_count() != 0 {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::Busy,
            ));
        }
        let mut current =
            lock_terminal_history_range_notifier(&self.terminal_history_range_notifier)?;
        if current.is_some() {
            return Err(DesktopControllerError::new(
                DesktopControllerErrorCode::NotifierAlreadyAttached,
            ));
        }
        *current = Some(notifier);
        Ok(())
    }

    pub fn take_snapshot(&self) -> Result<Option<Arc<ProductSnapshot>>, DesktopControllerError> {
        self.snapshot_receiver().take_snapshot()
    }

    pub fn published_product_generation(
        &self,
    ) -> Result<Option<ProductGeneration>, DesktopControllerError> {
        Ok(*lock_published_generation(
            &self.publication.published_generation,
        )?)
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

fn execute_work<S: DesktopQuerySource>(
    source: &mut S,
    reducer: &mut ProductReducer,
    permit: &RefreshPermit,
    context: &DesktopExecutionContext<'_>,
) -> RefreshOutcome {
    let batch = match lock_work(context.work) {
        Ok(mut state) => take_work_batch(&mut state, permit.id().get()),
        Err(_) => return RefreshOutcome::Failed,
    };

    if let Some(selection) = batch.selection {
        let outcome = execute_session_detail(source, reducer, permit, context, selection);
        if outcome != RefreshOutcome::Completed {
            return outcome;
        }
    }
    if let Some(history_range) = batch.history_range {
        let outcome = execute_history_range(source, reducer, permit, context, history_range);
        if outcome != RefreshOutcome::Completed {
            return outcome;
        }
    }
    if let Some(navigation) = batch.navigation {
        let outcome = execute_session_page(source, reducer, permit, context, navigation);
        if outcome != RefreshOutcome::Completed {
            return outcome;
        }
    }
    if batch.refresh {
        execute_refresh(source, reducer, permit, context)
    } else {
        RefreshOutcome::Completed
    }
}

fn execute_history_range<S: DesktopQuerySource>(
    source: &mut S,
    reducer: &mut ProductReducer,
    permit: &RefreshPermit,
    context: &DesktopExecutionContext<'_>,
    intent: DesktopHistoryRangeIntent,
) -> RefreshOutcome {
    let Some(attempt) = ProductAttemptGeneration::new(permit.id().get()) else {
        return RefreshOutcome::Failed;
    };
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }
    if !history_range_is_current(reducer, context, permit.id().get(), intent) {
        return RefreshOutcome::Completed;
    }
    let request = match DesktopQueryPlan::history_request(intent.preset()) {
        Ok(request) => request,
        Err(_) => return RefreshOutcome::Failed,
    };
    let result = source
        .usage_analytics(request)
        .map_err(|error| error.code());
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }
    commit_history_range(reducer, context, attempt, permit.id().get(), intent, result)
}

fn commit_history_range(
    reducer: &mut ProductReducer,
    context: &DesktopExecutionContext<'_>,
    product_attempt: ProductAttemptGeneration,
    worker_attempt: u64,
    intent: DesktopHistoryRangeIntent,
    result: Result<QueryEnvelope<UsageAnalytics>, QueryErrorCode>,
) -> RefreshOutcome {
    let mut work = match lock_work(context.work) {
        Ok(work) => work,
        Err(_) => return RefreshOutcome::Failed,
    };
    let valid = context.snapshot_epoch.load(Ordering::Acquire) == intent.snapshot_epoch().get()
        && work.current_history_range.is_some_and(|current| {
            current.intent == intent
                && history_range_generation_is_current(
                    current,
                    worker_attempt,
                    reducer.snapshot().generation(),
                )
        });
    if !valid {
        if work
            .current_history_range
            .is_some_and(|current| current.attempt == worker_attempt)
        {
            invalidate_history_range(&mut work);
        }
        return RefreshOutcome::Completed;
    }
    let successful = result.is_ok();
    let reduced = match result {
        Ok(value) => reducer.publish_history(product_attempt, value),
        Err(code) => reducer.fail_history(product_attempt, code),
    };
    let Ok(outcome) = reduced else {
        return RefreshOutcome::Failed;
    };
    match history_range_publication_action(outcome, successful) {
        HistoryRangePublicationAction::TerminalRollback => return RefreshOutcome::Completed,
        HistoryRangePublicationAction::PublishAndAdvancePreset => {
            work.published_history_preset = intent.preset();
        }
        HistoryRangePublicationAction::PublishWithoutPresetAdvance => {}
    }
    let snapshot = reducer.snapshot();
    let notifier = match lock_notifier(&context.publication.notifier) {
        Ok(notifier) => notifier.clone(),
        Err(_) => return RefreshOutcome::Failed,
    };
    let mut published_generation =
        match lock_published_generation(&context.publication.published_generation) {
            Ok(generation) => generation,
            Err(_) => return RefreshOutcome::Failed,
        };
    let mut latest = match lock_latest(&context.publication.latest) {
        Ok(latest) => latest,
        Err(_) => return RefreshOutcome::Failed,
    };
    *published_generation = Some(snapshot.generation());
    *latest = Some(snapshot);
    drop(latest);
    drop(published_generation);
    invalidate_history_range(&mut work);
    drop(work);
    if let Some(notifier) = notifier {
        notifier.snapshot_ready();
    }
    RefreshOutcome::Completed
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryRangePublicationAction {
    PublishAndAdvancePreset,
    PublishWithoutPresetAdvance,
    TerminalRollback,
}

const fn history_range_publication_action(
    outcome: ProductPublishOutcome,
    successful: bool,
) -> HistoryRangePublicationAction {
    match outcome {
        ProductPublishOutcome::Accepted if successful => {
            HistoryRangePublicationAction::PublishAndAdvancePreset
        }
        ProductPublishOutcome::Accepted => {
            HistoryRangePublicationAction::PublishWithoutPresetAdvance
        }
        ProductPublishOutcome::Coalesced
        | ProductPublishOutcome::RejectedOlder
        | ProductPublishOutcome::RejectedIncompatible => {
            HistoryRangePublicationAction::TerminalRollback
        }
    }
}

fn execute_session_detail<S: DesktopQuerySource>(
    source: &mut S,
    reducer: &mut ProductReducer,
    permit: &RefreshPermit,
    context: &DesktopExecutionContext<'_>,
    intent: DesktopSessionDetailIntent,
) -> RefreshOutcome {
    let Some(attempt) = ProductAttemptGeneration::new(permit.id().get()) else {
        return RefreshOutcome::Failed;
    };
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }
    if context.snapshot_epoch.load(Ordering::Acquire) != intent.snapshot_epoch().get() {
        return RefreshOutcome::Completed;
    }

    let current = reducer.snapshot();
    if current.generation() != intent.product_generation() {
        return RefreshOutcome::Completed;
    }
    let key = current
        .sessions()
        .payload()
        .and_then(|sessions| {
            sessions
                .payload()
                .sessions()
                .get(usize::from(intent.selection().row_ordinal()))
        })
        .map(|summary| summary.key().clone());
    drop(current);

    let result = match key {
        Some(key) => source
            .usage_session_detail(key)
            .map_err(|error| error.code()),
        None => Err(QueryErrorCode::InvalidValue),
    };
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let latest = match lock_work(context.work) {
        Ok(state) => state.latest_selection_generation == Some(intent.selection().generation()),
        Err(_) => return RefreshOutcome::Failed,
    };
    if !latest {
        return RefreshOutcome::Completed;
    }

    let reduced = match result {
        Ok(value) => reducer.publish_session_detail(attempt, intent.selection(), value),
        Err(code) => reducer.fail_session_detail(attempt, intent.selection(), code),
    };
    if reduced.is_err() {
        return RefreshOutcome::Failed;
    }
    publish_snapshot(reducer.snapshot(), context.publication)
}

fn execute_session_page<S: DesktopQuerySource>(
    source: &mut S,
    reducer: &mut ProductReducer,
    permit: &RefreshPermit,
    context: &DesktopExecutionContext<'_>,
    intent: DesktopSessionPageIntent,
) -> RefreshOutcome {
    let Some(attempt) = ProductAttemptGeneration::new(permit.id().get()) else {
        return RefreshOutcome::Failed;
    };
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }
    if !navigation_is_current(reducer, context, permit.id().get(), intent) {
        return RefreshOutcome::Completed;
    }

    let request = match intent.direction() {
        DesktopSessionPageDirection::Newest => Ok(context.plan.sessions.clone()),
        DesktopSessionPageDirection::Next => reducer
            .snapshot()
            .sessions()
            .payload()
            .and_then(|sessions| sessions.payload().next_cursor())
            .cloned()
            .ok_or(QueryErrorCode::InvalidValue)
            .and_then(|cursor| {
                UsageSessionPageRequest::continuation(
                    context.plan.sessions.page_size(),
                    cursor,
                    context.plan.sessions.scopes().to_vec(),
                )
                .map_err(|error| error.code())
            }),
    };
    let result = match request {
        Ok(request) => source.usage_sessions(request).map_err(|error| error.code()),
        Err(code) => Err(code),
    };
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }
    commit_session_page(reducer, context, attempt, permit.id().get(), intent, result)
}

fn commit_session_page(
    reducer: &mut ProductReducer,
    context: &DesktopExecutionContext<'_>,
    product_attempt: ProductAttemptGeneration,
    worker_attempt: u64,
    intent: DesktopSessionPageIntent,
    result: Result<QueryEnvelope<UsageSessionPage>, QueryErrorCode>,
) -> RefreshOutcome {
    commit_session_page_with_hook(
        reducer,
        context,
        product_attempt,
        worker_attempt,
        intent,
        result,
        || {},
    )
}

fn commit_session_page_with_hook<F>(
    reducer: &mut ProductReducer,
    context: &DesktopExecutionContext<'_>,
    product_attempt: ProductAttemptGeneration,
    worker_attempt: u64,
    intent: DesktopSessionPageIntent,
    result: Result<QueryEnvelope<UsageSessionPage>, QueryErrorCode>,
    after_validation: F,
) -> RefreshOutcome
where
    F: FnOnce(),
{
    let mut work = match lock_work(context.work) {
        Ok(value) => value,
        Err(_) => return RefreshOutcome::Failed,
    };
    let valid = context.snapshot_epoch.load(Ordering::Acquire) == intent.snapshot_epoch().get()
        && reducer.snapshot().generation() == intent.product_generation()
        && work.current_navigation.is_some_and(|active| {
            active.attempt == worker_attempt
                && active.intent.navigation_generation() == intent.navigation_generation()
        });
    if !valid {
        if work
            .current_navigation
            .is_some_and(|active| active.attempt == worker_attempt)
        {
            invalidate_navigation(&mut work);
        }
        return RefreshOutcome::Completed;
    }
    after_validation();
    let reduced = match result {
        Ok(value) => reducer.publish_sessions(product_attempt, value),
        Err(code) => reducer.fail_sessions(product_attempt, code),
    };
    if reduced.is_err() {
        return RefreshOutcome::Failed;
    }
    let notifier = match lock_notifier(&context.publication.notifier) {
        Ok(value) => value.clone(),
        Err(_) => return RefreshOutcome::Failed,
    };
    let snapshot = reducer.snapshot();
    let generation = snapshot.generation();
    let mut published_generation =
        match lock_published_generation(&context.publication.published_generation) {
            Ok(value) => value,
            Err(_) => return RefreshOutcome::Failed,
        };
    let mut latest = match lock_latest(&context.publication.latest) {
        Ok(value) => value,
        Err(_) => return RefreshOutcome::Failed,
    };
    *published_generation = Some(generation);
    *latest = Some(snapshot);
    drop(latest);
    drop(published_generation);
    invalidate_navigation(&mut work);
    drop(work);
    if let Some(notifier) = notifier {
        notifier.snapshot_ready();
    }
    RefreshOutcome::Completed
}

fn execute_refresh<S: DesktopQuerySource>(
    source: &mut S,
    reducer: &mut ProductReducer,
    permit: &RefreshPermit,
    context: &DesktopExecutionContext<'_>,
) -> RefreshOutcome {
    let Some(attempt) = ProductAttemptGeneration::new(permit.id().get()) else {
        return RefreshOutcome::Failed;
    };
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let observation = match lock_runtime_observation(&context.publication.runtime_observation) {
        Ok(mut state) => state.pending.take(),
        Err(_) => return RefreshOutcome::Failed,
    };
    if let Some(observation) = observation
        && apply_runtime_observation(reducer, observation).is_err()
    {
        return RefreshOutcome::Failed;
    }

    let result = match source.product_data_status() {
        Ok(value) => reducer.publish_data_status(attempt, value),
        Err(error) => reducer.fail_data_status(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let result = match source.usage_analytics(context.plan.analytics.clone()) {
        Ok(value) => reducer.publish_analytics(attempt, value),
        Err(error) => reducer.fail_analytics(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let history_request = match lock_work(context.work)
        .and_then(|work| DesktopQueryPlan::history_request(work.published_history_preset))
    {
        Ok(request) => request,
        Err(_) => return RefreshOutcome::Failed,
    };
    let result = match source.usage_analytics(history_request) {
        Ok(value) => reducer.publish_history(attempt, value),
        Err(error) => reducer.fail_history(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let result = match source.quota_overview() {
        Ok(value) => reducer.publish_quota(attempt, value),
        Err(error) => reducer.fail_quota(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let result = match source.benefit_overview(BenefitOverviewRequest::new()) {
        Ok(value) => reducer.publish_benefit(attempt, value),
        Err(error) => reducer.fail_benefit(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let result = match source.git_output(context.plan.git.clone()) {
        Ok(value) => reducer.publish_git(attempt, value),
        Err(error) => reducer.fail_git(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let result = match source.latest_activity(context.plan.activity) {
        Ok(value) => reducer.publish_activity(attempt, value),
        Err(error) => reducer.fail_activity(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let result = match source.usage_sessions(context.plan.sessions.clone()) {
        Ok(value) => reducer.publish_sessions(attempt, value),
        Err(error) => reducer.fail_sessions(attempt, error.code()),
    };
    if result.is_err() {
        return RefreshOutcome::Failed;
    }
    if let Some(outcome) = stop_outcome(permit, context.clock) {
        return outcome;
    }

    let outcome = publish_snapshot(reducer.snapshot(), context.publication);
    if outcome == RefreshOutcome::Completed {
        let generation = reducer.snapshot().generation();
        let mut work = match lock_work(context.work) {
            Ok(work) => work,
            Err(_) => return RefreshOutcome::Failed,
        };
        rebind_history_range_after_refresh(&mut work, permit.id().get(), generation);
    }
    outcome
}

fn publish_snapshot(
    snapshot: Arc<ProductSnapshot>,
    publication: &DesktopPublication,
) -> RefreshOutcome {
    publish_snapshot_with_hook(snapshot, publication, || {})
}

fn publish_snapshot_with_hook<F>(
    snapshot: Arc<ProductSnapshot>,
    publication: &DesktopPublication,
    after_locks_acquired: F,
) -> RefreshOutcome
where
    F: FnOnce(),
{
    let generation = snapshot.generation();
    let notifier = match lock_notifier(&publication.notifier) {
        Ok(notifier) => notifier.clone(),
        Err(_) => return RefreshOutcome::Failed,
    };
    let mut published_generation =
        match lock_published_generation(&publication.published_generation) {
            Ok(value) => value,
            Err(_) => return RefreshOutcome::Failed,
        };
    let mut latest = match lock_latest(&publication.latest) {
        Ok(value) => value,
        Err(_) => return RefreshOutcome::Failed,
    };
    after_locks_acquired();
    *published_generation = Some(generation);
    *latest = Some(snapshot);
    drop(latest);
    drop(published_generation);
    if let Some(notifier) = notifier {
        notifier.snapshot_ready();
    }
    RefreshOutcome::Completed
}

fn apply_runtime_observation(
    reducer: &mut ProductReducer,
    observation: DesktopRuntimeObservation,
) -> Result<(), tokenmaster_product::ProductReducerError> {
    match observation.usage {
        Ok(health) => {
            reducer.publish_usage_runtime_health(observation.generation, health)?;
        }
        Err(error) => {
            reducer.fail_usage_runtime_observation(observation.generation, error)?;
        }
    }
    match observation.quota {
        Ok(health) => {
            reducer.publish_quota_runtime_health(observation.generation, health)?;
        }
        Err(error) => {
            reducer.fail_quota_runtime_observation(observation.generation, error)?;
        }
    }
    match observation.reminder {
        Ok(health) => {
            reducer.publish_reminder_runtime_health(observation.generation, health)?;
        }
        Err(error) => {
            reducer.fail_reminder_runtime_observation(observation.generation, error)?;
        }
    }
    match observation.git {
        Ok(health) => {
            reducer.publish_git_runtime_health(observation.generation, health)?;
        }
        Err(error) => {
            reducer.fail_git_runtime_observation(observation.generation, error)?;
        }
    }
    Ok(())
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

fn lock_notifier(
    notifier: &SnapshotNotifier,
) -> Result<MutexGuard<'_, Option<Arc<dyn DesktopSnapshotNotifier>>>, DesktopControllerError> {
    notifier
        .lock()
        .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))
}

fn lock_terminal_navigation_notifier(
    notifier: &TerminalNavigationNotifier,
) -> Result<
    MutexGuard<'_, Option<Arc<dyn DesktopTerminalNavigationNotifier>>>,
    DesktopControllerError,
> {
    notifier
        .lock()
        .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))
}

fn lock_terminal_history_range_notifier(
    notifier: &TerminalHistoryRangeNotifier,
) -> Result<
    MutexGuard<'_, Option<Arc<dyn DesktopTerminalHistoryRangeNotifier>>>,
    DesktopControllerError,
> {
    notifier
        .lock()
        .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))
}

fn lock_published_generation(
    generation: &PublishedProductGeneration,
) -> Result<MutexGuard<'_, Option<ProductGeneration>>, DesktopControllerError> {
    generation
        .lock()
        .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))
}

fn lock_runtime_observation(
    observation: &RuntimeObservationSlot,
) -> Result<MutexGuard<'_, RuntimeObservationState>, DesktopControllerError> {
    observation
        .lock()
        .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))
}

fn lock_work(
    work: &DesktopWorkSlot,
) -> Result<MutexGuard<'_, DesktopWorkState>, DesktopControllerError> {
    work.lock()
        .map_err(|_| DesktopControllerError::new(DesktopControllerErrorCode::Internal))
}

fn invalidate_navigation(state: &mut DesktopWorkState) {
    state.current_navigation = None;
    state.pending_navigation = None;
}

fn invalidate_history_range(state: &mut DesktopWorkState) {
    state.current_history_range = None;
    state.pending_history_range = None;
}

fn history_range_is_active(state: &DesktopWorkState) -> bool {
    state.current_history_range.is_some() || state.pending_history_range.is_some()
}

fn session_interaction_is_active(state: &DesktopWorkState) -> bool {
    state.pending_selection.is_some()
        || state.active_selection_attempt.is_some()
        || state.current_navigation.is_some()
        || state.pending_navigation.is_some()
}

fn history_range_is_current(
    reducer: &ProductReducer,
    context: &DesktopExecutionContext<'_>,
    attempt: u64,
    intent: DesktopHistoryRangeIntent,
) -> bool {
    if context.snapshot_epoch.load(Ordering::Acquire) != intent.snapshot_epoch().get() {
        return false;
    }
    match lock_work(context.work) {
        Ok(state) => state.current_history_range.is_some_and(|current| {
            current.intent == intent
                && history_range_generation_is_current(
                    current,
                    attempt,
                    reducer.snapshot().generation(),
                )
        }),
        Err(_) => false,
    }
}

fn history_range_generation_is_current(
    current: ActiveDesktopHistoryRange,
    attempt: u64,
    product_generation: ProductGeneration,
) -> bool {
    current.attempt == attempt
        && current
            .rebound_product_generation
            .unwrap_or(current.intent.product_generation())
            == product_generation
}

fn rebind_history_range_after_refresh(
    state: &mut DesktopWorkState,
    refresh_attempt: u64,
    product_generation: ProductGeneration,
) {
    if let Some(current) = state.current_history_range.as_mut()
        && current.prerequisite_attempt == Some(refresh_attempt)
    {
        current.rebound_product_generation = Some(product_generation);
    }
}

fn navigation_is_current(
    reducer: &ProductReducer,
    context: &DesktopExecutionContext<'_>,
    attempt: u64,
    intent: DesktopSessionPageIntent,
) -> bool {
    if context.snapshot_epoch.load(Ordering::Acquire) != intent.snapshot_epoch().get()
        || reducer.snapshot().generation() != intent.product_generation()
    {
        return false;
    }
    match lock_work(context.work) {
        Ok(state) => state.current_navigation.is_some_and(|current| {
            current.attempt == attempt
                && current.intent.navigation_generation() == intent.navigation_generation()
        }),
        Err(_) => false,
    }
}

fn scheduled_work_attempt(
    admission: DesktopRefreshAdmission,
) -> Result<Option<u64>, DesktopControllerError> {
    match admission {
        DesktopRefreshAdmission::Started { attempt } => Ok(Some(attempt.get())),
        DesktopRefreshAdmission::Coalesced { receipt, .. } => {
            receipt.get().checked_add(1).map(Some).ok_or_else(|| {
                DesktopControllerError::new(DesktopControllerErrorCode::CapacityExceeded)
            })
        }
        DesktopRefreshAdmission::DeadlineExceeded { .. } => Ok(None),
    }
}

fn scheduled_session_detail(
    admission: DesktopRefreshAdmission,
    intent: DesktopSessionDetailIntent,
) -> Result<Option<PendingDesktopSessionDetail>, DesktopControllerError> {
    match admission {
        DesktopRefreshAdmission::Started { attempt } => Ok(Some(PendingDesktopSessionDetail {
            attempt: attempt.get(),
            prerequisite_attempt: None,
            intent,
        })),
        DesktopRefreshAdmission::Coalesced {
            receipt,
            active_attempt,
        } => Ok(Some(PendingDesktopSessionDetail {
            attempt: receipt.get().checked_add(1).ok_or_else(|| {
                DesktopControllerError::new(DesktopControllerErrorCode::CapacityExceeded)
            })?,
            prerequisite_attempt: Some(active_attempt.get()),
            intent,
        })),
        DesktopRefreshAdmission::DeadlineExceeded { .. } => Ok(None),
    }
}

fn scheduled_navigation(
    admission: DesktopRefreshAdmission,
    intent: DesktopSessionPageIntent,
) -> Result<Option<ActiveDesktopSessionPage>, DesktopControllerError> {
    match admission {
        DesktopRefreshAdmission::Started { attempt } => Ok(Some(ActiveDesktopSessionPage {
            attempt: attempt.get(),
            prerequisite_attempt: None,
            intent,
        })),
        DesktopRefreshAdmission::Coalesced {
            receipt,
            active_attempt,
        } => Ok(Some(ActiveDesktopSessionPage {
            attempt: receipt.get().checked_add(1).ok_or_else(|| {
                DesktopControllerError::new(DesktopControllerErrorCode::CapacityExceeded)
            })?,
            prerequisite_attempt: Some(active_attempt.get()),
            intent,
        })),
        DesktopRefreshAdmission::DeadlineExceeded { .. } => Ok(None),
    }
}

fn scheduled_history_range(
    admission: DesktopRefreshAdmission,
    intent: DesktopHistoryRangeIntent,
) -> Result<Option<ActiveDesktopHistoryRange>, DesktopControllerError> {
    match admission {
        DesktopRefreshAdmission::Started { attempt } => Ok(Some(ActiveDesktopHistoryRange {
            attempt: attempt.get(),
            prerequisite_attempt: None,
            rebound_product_generation: None,
            intent,
        })),
        DesktopRefreshAdmission::Coalesced {
            receipt,
            active_attempt,
        } => Ok(Some(ActiveDesktopHistoryRange {
            attempt: receipt.get().checked_add(1).ok_or_else(|| {
                DesktopControllerError::new(DesktopControllerErrorCode::CapacityExceeded)
            })?,
            prerequisite_attempt: Some(active_attempt.get()),
            rebound_product_generation: None,
            intent,
        })),
        DesktopRefreshAdmission::DeadlineExceeded { .. } => Ok(None),
    }
}

fn reconcile_navigation_completion(
    work: &DesktopWorkSlot,
    completion: tokenmaster_engine::WorkerCompletion,
) -> Result<Option<DesktopSessionPageIntent>, DesktopControllerError> {
    let mut state = lock_work(work)?;
    let completed = completion.request_id().get();
    let clear_current = state.current_navigation.is_some_and(|current| {
        current.attempt == completed
            || (!completion.follow_up_started()
                && (completion.pending_deadline_exceeded()
                    || completion.pending_capacity_exceeded()
                    || completion.follow_up_abandoned())
                && current.prerequisite_attempt == Some(completed))
    });
    if clear_current {
        let intent = state.current_navigation.map(|active| active.intent);
        invalidate_navigation(&mut state);
        return Ok(intent);
    }
    Ok(None)
}

fn reconcile_history_range_completion(
    work: &DesktopWorkSlot,
    completion: tokenmaster_engine::WorkerCompletion,
) -> Result<Option<DesktopHistoryRangeIntent>, DesktopControllerError> {
    let mut state = lock_work(work)?;
    let completed = completion.request_id().get();
    let clear_current = state.current_history_range.is_some_and(|current| {
        current.attempt == completed
            || (!completion.follow_up_started()
                && (completion.pending_deadline_exceeded()
                    || completion.pending_capacity_exceeded()
                    || completion.follow_up_abandoned())
                && current.prerequisite_attempt == Some(completed))
    });
    if clear_current {
        let intent = state.current_history_range.map(|active| active.intent);
        invalidate_history_range(&mut state);
        return Ok(intent);
    }
    Ok(None)
}

fn reconcile_session_detail_completion(
    work: &DesktopWorkSlot,
    completion: WorkerCompletion,
) -> Result<(), DesktopControllerError> {
    let mut state = lock_work(work)?;
    let completed = completion.request_id().get();
    if state.active_selection_attempt == Some(completed) {
        state.active_selection_attempt = None;
    }
    let clear_pending = state.pending_selection.is_some_and(|pending| {
        pending.attempt == completed
            || (!completion.follow_up_started()
                && (completion.pending_deadline_exceeded()
                    || completion.pending_capacity_exceeded()
                    || completion.follow_up_abandoned())
                && pending.prerequisite_attempt == Some(completed))
    });
    if clear_pending {
        state.pending_selection = None;
    }
    Ok(())
}

fn take_work_batch(state: &mut DesktopWorkState, attempt: u64) -> DesktopWorkBatch {
    let refresh = match state.refresh_attempt {
        Some(expected) if expected == attempt => {
            state.refresh_attempt = None;
            true
        }
        Some(expected) if expected < attempt => {
            state.refresh_attempt = None;
            false
        }
        Some(_) | None => false,
    };
    let selection = match state.pending_selection {
        Some(pending) if pending.attempt == attempt => {
            state.pending_selection = None;
            state.active_selection_attempt = Some(attempt);
            Some(pending.intent)
        }
        Some(pending) if pending.attempt < attempt => {
            state.pending_selection = None;
            None
        }
        Some(_) | None => None,
    };
    let navigation = match state.pending_navigation {
        Some(pending) if pending.attempt == attempt => {
            state.pending_navigation = None;
            Some(pending.intent)
        }
        Some(pending) if pending.attempt < attempt => {
            state.pending_navigation = None;
            if state
                .current_navigation
                .is_some_and(|current| current.attempt == pending.attempt)
            {
                state.current_navigation = None;
            }
            None
        }
        Some(_) | None => None,
    };
    let history_range = match state.pending_history_range {
        Some(pending) if pending.attempt == attempt => {
            state.pending_history_range = None;
            Some(pending.intent)
        }
        Some(pending) if pending.attempt < attempt => {
            state.pending_history_range = None;
            if state
                .current_history_range
                .is_some_and(|current| current.attempt == pending.attempt)
            {
                state.current_history_range = None;
            }
            None
        }
        Some(_) | None => None,
    };
    DesktopWorkBatch {
        refresh,
        selection,
        navigation,
        history_range,
    }
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
            mpsc::sync_channel,
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

    #[test]
    fn work_batch_is_exactly_bound_to_its_scheduled_attempt() {
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        let selection = ProductSessionDetailSelection::new(
            ProductSessionDetailSelectionGeneration::new(1).expect("selection generation"),
            0,
        );
        let intent = DesktopSessionDetailIntent::new(
            epoch,
            tokenmaster_product::ProductGeneration::INITIAL,
            selection,
        );
        let mut state = DesktopWorkState {
            refresh_attempt: Some(2),
            latest_selection_generation: Some(selection.generation()),
            pending_selection: Some(PendingDesktopSessionDetail {
                attempt: 2,
                prerequisite_attempt: None,
                intent,
            }),
            ..DesktopWorkState::default()
        };

        let early = take_work_batch(&mut state, 1);
        assert!(!early.refresh);
        assert!(early.selection.is_none());
        assert!(early.navigation.is_none());
        assert_eq!(state.refresh_attempt, Some(2));
        assert!(state.pending_selection.is_some());

        let exact = take_work_batch(&mut state, 2);
        assert!(exact.refresh);
        assert_eq!(exact.selection, Some(intent));
        assert!(exact.navigation.is_none());
        assert_eq!(state.refresh_attempt, None);
        assert!(state.pending_selection.is_none());

        state.refresh_attempt = Some(2);
        state.pending_selection = Some(PendingDesktopSessionDetail {
            attempt: 2,
            prerequisite_attempt: None,
            intent,
        });
        let stale = take_work_batch(&mut state, 3);
        assert!(!stale.refresh);
        assert!(stale.selection.is_none());
        assert!(stale.navigation.is_none());
        assert_eq!(state.refresh_attempt, None);
        assert!(state.pending_selection.is_none());
    }

    #[test]
    fn history_range_publish_outcome_matrix_never_publishes_nonaccepted_results() {
        assert_eq!(
            history_range_publication_action(ProductPublishOutcome::Accepted, true),
            HistoryRangePublicationAction::PublishAndAdvancePreset
        );
        assert_eq!(
            history_range_publication_action(ProductPublishOutcome::Accepted, false),
            HistoryRangePublicationAction::PublishWithoutPresetAdvance
        );
        for outcome in [
            ProductPublishOutcome::Coalesced,
            ProductPublishOutcome::RejectedOlder,
            ProductPublishOutcome::RejectedIncompatible,
        ] {
            assert_eq!(
                history_range_publication_action(outcome, true),
                HistoryRangePublicationAction::TerminalRollback
            );
        }
    }

    #[test]
    fn nonaccepted_history_commits_keep_current_work_for_exact_terminal_rollback() {
        for (seed_attempt, worker_attempt) in [(2_u64, 2_u64), (3_u64, 2_u64)] {
            let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
            let mut reducer = ProductReducer::new();
            reducer
                .fail_history(
                    ProductAttemptGeneration::new(seed_attempt).expect("seed attempt"),
                    invalid_query().code(),
                )
                .expect("seed history generation");
            let intent = DesktopHistoryRangeIntent::new(
                epoch,
                reducer.snapshot().generation(),
                DesktopHistoryRangeGeneration::new(1).expect("range generation"),
                DesktopHistoryRangePreset::Recent1Day,
            );
            let work = Arc::new(Mutex::new(DesktopWorkState {
                history_range_high_water: Some(intent.generation()),
                current_history_range: Some(ActiveDesktopHistoryRange {
                    attempt: worker_attempt,
                    prerequisite_attempt: None,
                    rebound_product_generation: None,
                    intent,
                }),
                ..DesktopWorkState::default()
            }));
            let latest = Arc::new(Mutex::new(None));
            let publication = DesktopPublication {
                latest: Arc::clone(&latest),
                notifier: Arc::new(Mutex::new(None)),
                published_generation: Arc::new(Mutex::new(None)),
                runtime_observation: Arc::new(Mutex::new(RuntimeObservationState::default())),
            };
            let snapshot_epoch = AtomicU64::new(epoch.get());
            let clock = ManualClock::new(0);
            let plan = DesktopQueryPlan::overview().expect("plan");
            let context = DesktopExecutionContext {
                plan: &plan,
                clock: &clock,
                publication: &publication,
                snapshot_epoch: &snapshot_epoch,
                work: &work,
            };

            assert_eq!(
                commit_history_range(
                    &mut reducer,
                    &context,
                    ProductAttemptGeneration::new(worker_attempt).expect("worker attempt"),
                    worker_attempt,
                    intent,
                    Err(QueryErrorCode::InvalidValue),
                ),
                RefreshOutcome::Completed
            );
            assert!(lock_latest(&latest).expect("latest lock").is_none());
            let state = lock_work(&work).expect("work lock");
            assert_eq!(
                state.published_history_preset,
                DesktopHistoryRangePreset::Recent30Days
            );
            assert_eq!(
                state.current_history_range.map(|current| current.attempt),
                Some(worker_attempt)
            );
            assert_eq!(
                state.current_history_range.map(|current| current.intent),
                Some(intent)
            );
        }
    }

    #[test]
    fn history_range_rebind_is_limited_to_its_exact_prerequisite_refresh() {
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        let intent = DesktopHistoryRangeIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopHistoryRangeGeneration::new(1).expect("generation"),
            DesktopHistoryRangePreset::Recent1Day,
        );
        let mut reducer = ProductReducer::new();
        reducer
            .fail_history(
                ProductAttemptGeneration::new(1).expect("attempt"),
                invalid_query().code(),
            )
            .expect("advance generation");
        let rebound = reducer.snapshot().generation();
        let mut state = DesktopWorkState {
            current_history_range: Some(ActiveDesktopHistoryRange {
                attempt: 2,
                prerequisite_attempt: Some(1),
                rebound_product_generation: None,
                intent,
            }),
            ..DesktopWorkState::default()
        };
        rebind_history_range_after_refresh(&mut state, 3, rebound);
        let active = state.current_history_range.expect("current range");
        assert!(!history_range_generation_is_current(active, 2, rebound));
        rebind_history_range_after_refresh(&mut state, 1, rebound);
        let active = state.current_history_range.expect("current range");
        assert!(history_range_generation_is_current(active, 2, rebound));
        assert!(!history_range_generation_is_current(active, 3, rebound));
        assert!(!history_range_generation_is_current(
            active,
            2,
            ProductGeneration::INITIAL
        ));
    }

    #[test]
    fn history_request_is_a_fixed_system_zone_daily_model_project_shape() {
        for (preset, day_count) in [
            (DesktopHistoryRangePreset::Recent1Day, 1),
            (DesktopHistoryRangePreset::Recent7Days, 7),
            (DesktopHistoryRangePreset::Recent30Days, 30),
        ] {
            let request = DesktopQueryPlan::history_request(preset).expect("history request");
            assert_eq!(
                request.range(),
                &UsageRange::recent_days(day_count).expect("bounded recent range")
            );
            assert_eq!(request.time_zone(), &UsageTimeZone::system());
            assert_eq!(request.week_start(), WeekStart::Monday);
            assert_eq!(request.series(), UsageSeriesSelection::Daily);
            assert!(request.scopes().is_empty());
            assert_eq!(
                request.breakdowns(),
                [UsageBreakdownKind::Model, UsageBreakdownKind::Project]
            );
        }
    }

    #[test]
    fn range_and_session_slots_are_mutually_exclusive_in_active_and_pending_states() {
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        let range = DesktopHistoryRangeIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopHistoryRangeGeneration::new(1).expect("range generation"),
            DesktopHistoryRangePreset::Recent1Day,
        );
        let detail = DesktopSessionDetailIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            ProductSessionDetailSelection::new(
                ProductSessionDetailSelectionGeneration::new(1).expect("selection generation"),
                0,
            ),
        );
        let page = DesktopSessionPageIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopSessionNavigationGeneration::new(1).expect("page generation"),
            DesktopSessionPageDirection::Newest,
        );
        let range_active = ActiveDesktopHistoryRange {
            attempt: 1,
            prerequisite_attempt: None,
            rebound_product_generation: None,
            intent: range,
        };
        let mut state = DesktopWorkState {
            current_history_range: Some(range_active),
            ..DesktopWorkState::default()
        };
        assert!(history_range_is_active(&state));
        assert!(!session_interaction_is_active(&state));
        state.pending_history_range = Some(PendingDesktopHistoryRange {
            attempt: 2,
            intent: range,
        });
        assert!(history_range_is_active(&state));

        state = DesktopWorkState {
            pending_selection: Some(PendingDesktopSessionDetail {
                attempt: 2,
                prerequisite_attempt: Some(1),
                intent: detail,
            }),
            ..DesktopWorkState::default()
        };
        assert!(session_interaction_is_active(&state));
        assert!(!history_range_is_active(&state));
        state.active_selection_attempt = Some(2);
        assert!(session_interaction_is_active(&state));

        state = DesktopWorkState {
            current_navigation: Some(ActiveDesktopSessionPage {
                attempt: 1,
                prerequisite_attempt: None,
                intent: page,
            }),
            pending_navigation: Some(PendingDesktopSessionPage {
                attempt: 2,
                intent: page,
            }),
            ..DesktopWorkState::default()
        };
        assert!(session_interaction_is_active(&state));
        assert!(!history_range_is_active(&state));
    }

    #[test]
    fn generic_publication_holds_generation_and_latest_until_the_pair_is_consistent() {
        let latest = Arc::new(Mutex::new(None));
        let published_generation = Arc::new(Mutex::new(None));
        let publication = DesktopPublication {
            latest: Arc::clone(&latest),
            notifier: Arc::new(Mutex::new(None)),
            published_generation: Arc::clone(&published_generation),
            runtime_observation: Arc::new(Mutex::new(RuntimeObservationState::default())),
        };
        let snapshot = ProductReducer::new().snapshot();
        let expected_generation = snapshot.generation();
        let latest_for_hook = Arc::clone(&latest);
        let generation_for_hook = Arc::clone(&published_generation);

        assert_eq!(
            publish_snapshot_with_hook(snapshot, &publication, move || {
                assert!(matches!(
                    generation_for_hook.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                ));
                assert!(matches!(
                    latest_for_hook.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                ));
            }),
            RefreshOutcome::Completed
        );

        assert_eq!(
            *lock_published_generation(&published_generation).expect("generation lock"),
            Some(expected_generation)
        );
        assert_eq!(
            lock_latest(&latest)
                .expect("latest lock")
                .as_ref()
                .map(|snapshot| snapshot.generation()),
            Some(expected_generation)
        );
    }

    #[test]
    fn history_range_admission_rechecks_product_generation_after_waiting_for_work() {
        let clock = Arc::new(ManualClock::new(0));
        let controller_clock: Arc<dyn Clock> = clock.clone();
        let mut controller = DesktopController::spawn_with_clock(
            DeadlineSource { clock },
            DesktopQueryPlan::overview().expect("overview plan"),
            controller_clock,
        )
        .expect("controller starts");
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        controller.bind_snapshot_epoch(epoch).expect("bind epoch");
        *lock_published_generation(&controller.publication.published_generation)
            .expect("published generation") = Some(ProductGeneration::INITIAL);
        let page = DesktopSessionPageIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopSessionNavigationGeneration::new(1).expect("navigation generation"),
            DesktopSessionPageDirection::Newest,
        );
        {
            let mut work = lock_work(&controller.work).expect("work lock");
            work.current_navigation = Some(ActiveDesktopSessionPage {
                attempt: 1,
                prerequisite_attempt: None,
                intent: page,
            });
        }
        let range = DesktopHistoryRangeIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopHistoryRangeGeneration::new(1).expect("range generation"),
            DesktopHistoryRangePreset::Recent1Day,
        );
        let (commit_locked_sender, commit_locked_receiver) = sync_channel(1);
        let (commit_release_sender, commit_release_receiver) = sync_channel(1);
        let (range_sender, range_receiver) = sync_channel(1);
        thread::scope(|scope| {
            let controller_ref = &controller;
            scope.spawn(move || {
                let plan = DesktopQueryPlan::overview().expect("plan");
                let snapshot_epoch = AtomicU64::new(epoch.get());
                let clock = ManualClock::new(0);
                let context = DesktopExecutionContext {
                    plan: &plan,
                    clock: &clock,
                    publication: &controller_ref.publication,
                    snapshot_epoch: &snapshot_epoch,
                    work: &controller_ref.work,
                };
                let mut reducer = ProductReducer::new();
                assert_eq!(
                    commit_session_page_with_hook(
                        &mut reducer,
                        &context,
                        ProductAttemptGeneration::new(1).expect("attempt"),
                        1,
                        page,
                        Err(QueryErrorCode::InvalidValue),
                        || {
                            commit_locked_sender.send(()).expect("commit holds work");
                            commit_release_receiver.recv().expect("commit release");
                        },
                    ),
                    RefreshOutcome::Completed
                );
            });
            commit_locked_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("page commit holds work");
            let controller_ref = &controller;
            scope.spawn(move || {
                range_sender
                    .send(controller_ref.request_history_range(range))
                    .expect("range result send");
            });
            assert!(
                range_receiver.try_recv().is_err(),
                "range is blocked behind page commit work lock"
            );
            commit_release_sender.send(()).expect("release page commit");
            assert_eq!(
                range_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .expect("range result")
                    .expect_err("new publication makes stale range synchronous")
                    .stable_code(),
                "stale_history_range"
            );
        });
        let state = lock_work(&controller.work).expect("work lock");
        assert!(state.history_range_high_water.is_none());
        assert!(state.current_history_range.is_none());
        drop(state);
        controller.shutdown().expect("controller stops");
    }

    struct ReentrantWorkNotifier {
        work: DesktopWorkSlot,
        called: Arc<AtomicU64>,
    }

    impl DesktopSnapshotNotifier for ReentrantWorkNotifier {
        fn snapshot_ready(&self) {
            drop(lock_work(&self.work).expect("notifier acquires released work lock"));
            self.called.store(1, Ordering::Release);
        }
    }

    #[test]
    fn session_page_commit_linearizes_latest_before_supersession_and_notifies_after_unlock() {
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        let intent = DesktopSessionPageIntent::new(
            epoch,
            tokenmaster_product::ProductGeneration::INITIAL,
            DesktopSessionNavigationGeneration::new(1).expect("generation"),
            DesktopSessionPageDirection::Newest,
        );
        let work = Arc::new(Mutex::new(DesktopWorkState {
            navigation_high_water: Some(intent.navigation_generation()),
            current_navigation: Some(ActiveDesktopSessionPage {
                attempt: 1,
                prerequisite_attempt: None,
                intent,
            }),
            ..DesktopWorkState::default()
        }));
        let latest = Arc::new(Mutex::new(None));
        let called = Arc::new(AtomicU64::new(0));
        let publication = DesktopPublication {
            latest: Arc::clone(&latest),
            notifier: Arc::new(Mutex::new(Some(Arc::new(ReentrantWorkNotifier {
                work: Arc::clone(&work),
                called: Arc::clone(&called),
            })))),
            published_generation: Arc::new(Mutex::new(None)),
            runtime_observation: Arc::new(Mutex::new(RuntimeObservationState::default())),
        };
        let snapshot_epoch = AtomicU64::new(epoch.get());
        let clock = ManualClock::new(0);
        let plan = DesktopQueryPlan::overview().expect("plan");
        let context = DesktopExecutionContext {
            plan: &plan,
            clock: &clock,
            publication: &publication,
            snapshot_epoch: &snapshot_epoch,
            work: &work,
        };
        let (started_sender, started_receiver) = sync_channel(1);
        let (acquired_sender, acquired_receiver) = sync_channel(1);
        let work_for_hook = Arc::clone(&work);
        let latest_for_hook = Arc::clone(&latest);
        let mut reducer = ProductReducer::new();
        let outcome = commit_session_page_with_hook(
            &mut reducer,
            &context,
            ProductAttemptGeneration::new(1).expect("attempt"),
            1,
            intent,
            Err(QueryErrorCode::InvalidValue),
            move || {
                thread::spawn(move || {
                    started_sender.send(()).expect("thread started");
                    let state = lock_work(&work_for_hook).expect("supersession acquires work");
                    assert!(state.current_navigation.is_none());
                    assert!(
                        lock_latest(&latest_for_hook)
                            .expect("latest lock")
                            .is_some()
                    );
                    acquired_sender.send(()).expect("thread acquired");
                });
                started_receiver
                    .recv_timeout(Duration::from_secs(1))
                    .expect("hook thread starts");
            },
        );
        assert_eq!(outcome, RefreshOutcome::Completed);
        acquired_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("supersession after commit");
        assert_eq!(called.load(Ordering::Acquire), 1);
    }

    struct DeadlineSource {
        clock: Arc<ManualClock>,
    }

    struct PreStartDeadlineSource {
        clock: Arc<ManualClock>,
        entered: std::sync::mpsc::SyncSender<()>,
        release: std::sync::mpsc::Receiver<()>,
        session_calls: Arc<AtomicU64>,
        panic_after_release: bool,
    }

    struct DroppedDetailSource {
        clock: Arc<ManualClock>,
        entered: Option<std::sync::mpsc::SyncSender<()>>,
        release: Option<std::sync::mpsc::Receiver<()>>,
        detail_calls: Arc<AtomicU64>,
    }

    struct ActiveRangeDeadlineSource {
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

        fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
            unreachable!("deadline stops before quota")
        }

        fn benefit_overview(
            &mut self,
            _request: BenefitOverviewRequest,
        ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
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

        fn usage_session_detail(
            &mut self,
            _key: UsageSessionKey,
        ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
            unreachable!("deadline stops before session detail")
        }
    }

    impl DesktopQuerySource for PreStartDeadlineSource {
        fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
            self.entered.send(()).expect("refresh entered");
            self.release.recv().expect("refresh released");
            assert!(!self.panic_after_release, "test abandoned follow-up");
            self.clock.set(
                DesktopRefreshUrgency::Recovery
                    .budget_ms()
                    .saturating_add(1),
            );
            Err(invalid_query())
        }

        fn usage_analytics(
            &mut self,
            _request: UsageAnalyticsRequest,
        ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
            unreachable!("deadline stops before analytics")
        }

        fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
            unreachable!("deadline stops before quota")
        }

        fn benefit_overview(
            &mut self,
            _request: BenefitOverviewRequest,
        ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
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
            self.session_calls.fetch_add(1, Ordering::AcqRel);
            unreachable!("not-started navigation must not query sessions")
        }

        fn usage_session_detail(
            &mut self,
            _key: UsageSessionKey,
        ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
            unreachable!("deadline stops before session detail")
        }
    }

    impl DesktopQuerySource for DroppedDetailSource {
        fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
            if let Some(entered) = self.entered.take() {
                entered.send(()).expect("refresh entered");
                self.release
                    .take()
                    .expect("refresh release receiver")
                    .recv()
                    .expect("refresh released");
                self.clock.set(
                    DesktopRefreshUrgency::Recovery
                        .budget_ms()
                        .saturating_add(1),
                );
            }
            Err(invalid_query())
        }

        fn usage_analytics(
            &mut self,
            _request: UsageAnalyticsRequest,
        ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
            Err(invalid_query())
        }

        fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
            Err(invalid_query())
        }

        fn benefit_overview(
            &mut self,
            _request: BenefitOverviewRequest,
        ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
            Err(invalid_query())
        }

        fn git_output(
            &mut self,
            _request: GitOutputRequest,
        ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
            Err(invalid_query())
        }

        fn latest_activity(
            &mut self,
            _request: LatestActivityRequest,
        ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
            Err(invalid_query())
        }

        fn usage_sessions(
            &mut self,
            _request: UsageSessionPageRequest,
        ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
            Err(invalid_query())
        }

        fn usage_session_detail(
            &mut self,
            _key: UsageSessionKey,
        ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
            self.detail_calls.fetch_add(1, Ordering::AcqRel);
            Err(invalid_query())
        }
    }

    impl DesktopQuerySource for ActiveRangeDeadlineSource {
        fn product_data_status(&mut self) -> Result<ProductDataStatusEnvelope, QueryError> {
            Err(invalid_query())
        }

        fn usage_analytics(
            &mut self,
            _request: UsageAnalyticsRequest,
        ) -> Result<QueryEnvelope<UsageAnalytics>, QueryError> {
            self.clock
                .set(DesktopRefreshUrgency::Interactive.budget_ms());
            Err(invalid_query())
        }

        fn quota_overview(&mut self) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError> {
            Err(invalid_query())
        }

        fn benefit_overview(
            &mut self,
            _request: BenefitOverviewRequest,
        ) -> Result<BenefitOverviewEnvelope<BenefitOverviewSnapshot>, QueryError> {
            Err(invalid_query())
        }

        fn git_output(
            &mut self,
            _request: GitOutputRequest,
        ) -> Result<GitEnvelope<GitOutputSnapshot>, QueryError> {
            Err(invalid_query())
        }

        fn latest_activity(
            &mut self,
            _request: LatestActivityRequest,
        ) -> Result<QueryEnvelope<LatestActivityPage>, QueryError> {
            Err(invalid_query())
        }

        fn usage_sessions(
            &mut self,
            _request: UsageSessionPageRequest,
        ) -> Result<QueryEnvelope<UsageSessionPage>, QueryError> {
            Err(invalid_query())
        }

        fn usage_session_detail(
            &mut self,
            _key: UsageSessionKey,
        ) -> Result<QueryEnvelope<UsageSessionDetailResult>, QueryError> {
            Err(invalid_query())
        }
    }

    struct RecordingTerminalNotifier {
        sender: std::sync::mpsc::SyncSender<DesktopSessionPageIntent>,
    }

    impl DesktopTerminalNavigationNotifier for RecordingTerminalNotifier {
        fn navigation_terminal(&self, intent: DesktopSessionPageIntent) {
            let _ = self.sender.try_send(intent);
        }
    }

    struct RecordingHistoryTerminalNotifier {
        sender: std::sync::mpsc::SyncSender<DesktopHistoryRangeIntent>,
    }

    impl DesktopTerminalHistoryRangeNotifier for RecordingHistoryTerminalNotifier {
        fn history_range_terminal(&self, intent: DesktopHistoryRangeIntent) {
            let _ = self.sender.try_send(intent);
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

    #[test]
    fn active_history_range_deadline_rolls_back_once_without_snapshot_or_preset_advance() {
        let clock = Arc::new(ManualClock::new(0));
        let controller_clock: Arc<dyn Clock> = clock.clone();
        let mut controller = DesktopController::spawn_with_clock(
            ActiveRangeDeadlineSource { clock },
            DesktopQueryPlan::overview().expect("overview plan"),
            controller_clock,
        )
        .expect("controller starts");
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        controller.bind_snapshot_epoch(epoch).expect("bind epoch");
        *lock_published_generation(&controller.publication.published_generation)
            .expect("published generation") = Some(ProductGeneration::INITIAL);
        let (terminal_sender, terminal_receiver) = sync_channel(1);
        controller
            .attach_terminal_history_range_notifier(Arc::new(RecordingHistoryTerminalNotifier {
                sender: terminal_sender,
            }))
            .expect("attach terminal notifier");
        let intent = DesktopHistoryRangeIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopHistoryRangeGeneration::new(1).expect("range generation"),
            DesktopHistoryRangePreset::Recent1Day,
        );
        assert!(matches!(
            controller
                .request_history_range(intent)
                .expect("active range starts"),
            DesktopRefreshAdmission::Started { .. }
        ));
        assert_eq!(
            terminal_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("terminal rollback"),
            intent
        );
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
        assert!(controller.take_snapshot().expect("mailbox").is_none());
        assert_eq!(
            lock_work(&controller.work)
                .expect("work lock")
                .published_history_preset,
            DesktopHistoryRangePreset::Recent30Days
        );
        assert!(
            terminal_receiver.try_recv().is_err(),
            "terminal rollback is once"
        );
        controller.shutdown().expect("controller stops");
    }

    #[test]
    fn not_started_navigation_deadline_emits_the_exact_terminal_rollback_without_query_or_snapshot()
    {
        let clock = Arc::new(ManualClock::new(0));
        let controller_clock: Arc<dyn Clock> = clock.clone();
        let (entered_sender, entered_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        let session_calls = Arc::new(AtomicU64::new(0));
        let mut controller = DesktopController::spawn_with_clock(
            PreStartDeadlineSource {
                clock,
                entered: entered_sender,
                release: release_receiver,
                session_calls: Arc::clone(&session_calls),
                panic_after_release: false,
            },
            DesktopQueryPlan::overview().expect("overview plan"),
            controller_clock,
        )
        .expect("controller starts");
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        controller.bind_snapshot_epoch(epoch).expect("bind epoch");
        *lock_published_generation(&controller.publication.published_generation)
            .expect("published generation") = Some(ProductGeneration::INITIAL);
        let (terminal_sender, terminal_receiver) = sync_channel(1);
        controller
            .attach_terminal_navigation_notifier(Arc::new(RecordingTerminalNotifier {
                sender: terminal_sender,
            }))
            .expect("attach terminal notifier");

        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("blocking refresh starts");
        entered_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("refresh entered");
        let intent = DesktopSessionPageIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopSessionNavigationGeneration::new(1).expect("navigation generation"),
            DesktopSessionPageDirection::Newest,
        );
        assert!(matches!(
            controller
                .request_session_page(intent)
                .expect("navigation queues"),
            DesktopRefreshAdmission::Coalesced { .. }
        ));
        release_sender.send(()).expect("release refresh");

        let terminal = terminal_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("worker notifier delivers terminal rollback without completion polling");
        assert_eq!(terminal, intent);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if controller
                .try_completion()
                .expect("worker healthy")
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline, "completion timed out");
            thread::yield_now();
        }
        assert_eq!(session_calls.load(Ordering::Acquire), 0);
        assert!(controller.take_snapshot().expect("mailbox").is_none());
        controller.shutdown().expect("controller stops");
    }

    #[test]
    fn not_started_history_range_deadline_emits_exact_terminal_rollback_without_snapshot() {
        let clock = Arc::new(ManualClock::new(0));
        let controller_clock: Arc<dyn Clock> = clock.clone();
        let (entered_sender, entered_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        let session_calls = Arc::new(AtomicU64::new(0));
        let mut controller = DesktopController::spawn_with_clock(
            PreStartDeadlineSource {
                clock,
                entered: entered_sender,
                release: release_receiver,
                session_calls: Arc::clone(&session_calls),
                panic_after_release: false,
            },
            DesktopQueryPlan::overview().expect("overview plan"),
            controller_clock,
        )
        .expect("controller starts");
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        controller.bind_snapshot_epoch(epoch).expect("bind epoch");
        *lock_published_generation(&controller.publication.published_generation)
            .expect("published generation") = Some(ProductGeneration::INITIAL);
        let (terminal_sender, terminal_receiver) = sync_channel(1);
        controller
            .attach_terminal_history_range_notifier(Arc::new(RecordingHistoryTerminalNotifier {
                sender: terminal_sender,
            }))
            .expect("attach terminal notifier");

        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("blocking refresh starts");
        entered_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("refresh entered");
        let intent = DesktopHistoryRangeIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopHistoryRangeGeneration::new(1).expect("range generation"),
            DesktopHistoryRangePreset::Recent1Day,
        );
        assert!(matches!(
            controller
                .request_history_range(intent)
                .expect("range queues"),
            DesktopRefreshAdmission::Coalesced { .. }
        ));
        release_sender.send(()).expect("release refresh");

        assert_eq!(
            terminal_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("worker terminal rollback"),
            intent
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if controller
                .try_completion()
                .expect("worker healthy")
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline, "completion timed out");
            thread::yield_now();
        }
        assert_eq!(session_calls.load(Ordering::Acquire), 0);
        assert!(controller.take_snapshot().expect("mailbox").is_none());
        controller.shutdown().expect("controller stops");
    }

    #[test]
    fn dropped_detail_follow_up_clears_interaction_slots_and_never_replays() {
        let clock = Arc::new(ManualClock::new(0));
        let controller_clock: Arc<dyn Clock> = clock.clone();
        let (entered_sender, entered_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        let detail_calls = Arc::new(AtomicU64::new(0));
        let mut controller = DesktopController::spawn_with_clock(
            DroppedDetailSource {
                clock,
                entered: Some(entered_sender),
                release: Some(release_receiver),
                detail_calls: Arc::clone(&detail_calls),
            },
            DesktopQueryPlan::overview().expect("overview plan"),
            controller_clock,
        )
        .expect("controller starts");
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        controller.bind_snapshot_epoch(epoch).expect("bind epoch");
        *lock_published_generation(&controller.publication.published_generation)
            .expect("published generation") = Some(ProductGeneration::INITIAL);

        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("blocking refresh starts");
        entered_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("refresh entered");
        let selection = ProductSessionDetailSelection::new(
            ProductSessionDetailSelectionGeneration::new(1).expect("selection generation"),
            0,
        );
        assert!(matches!(
            controller
                .request_session_detail(DesktopSessionDetailIntent::new(
                    epoch,
                    ProductGeneration::INITIAL,
                    selection,
                ))
                .expect("detail queues"),
            DesktopRefreshAdmission::Coalesced { .. }
        ));
        release_sender.send(()).expect("release refresh");
        let deadline = Instant::now() + Duration::from_secs(2);
        while controller
            .try_completion()
            .expect("worker healthy")
            .is_none()
        {
            assert!(Instant::now() < deadline, "completion timed out");
            thread::yield_now();
        }

        let range = DesktopHistoryRangeIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopHistoryRangeGeneration::new(1).expect("range generation"),
            DesktopHistoryRangePreset::Recent1Day,
        );
        assert!(matches!(
            controller
                .request_history_range(range)
                .expect("dropped detail no longer blocks range"),
            DesktopRefreshAdmission::Started { .. }
        ));
        let _ = loop {
            if let Some(completion) = controller.try_completion().expect("worker healthy") {
                break completion;
            }
            assert!(Instant::now() < deadline, "range completion timed out");
            thread::yield_now();
        };
        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("later refresh starts without replaying detail");
        let _ = loop {
            if let Some(completion) = controller.try_completion().expect("worker healthy") {
                break completion;
            }
            assert!(
                Instant::now() < deadline,
                "later refresh completion timed out"
            );
            thread::yield_now();
        };
        assert_eq!(detail_calls.load(Ordering::Acquire), 0);
        controller.shutdown().expect("controller stops");
    }

    #[test]
    fn panicked_refresh_abandons_pending_history_range_and_notifies_exactly_once() {
        let clock = Arc::new(ManualClock::new(0));
        let controller_clock: Arc<dyn Clock> = clock.clone();
        let (entered_sender, entered_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        let session_calls = Arc::new(AtomicU64::new(0));
        let mut controller = DesktopController::spawn_with_clock(
            PreStartDeadlineSource {
                clock,
                entered: entered_sender,
                release: release_receiver,
                session_calls: Arc::clone(&session_calls),
                panic_after_release: true,
            },
            DesktopQueryPlan::overview().expect("overview plan"),
            controller_clock,
        )
        .expect("controller starts");
        let epoch = DesktopSnapshotEpoch::new(1).expect("epoch");
        controller.bind_snapshot_epoch(epoch).expect("bind epoch");
        *lock_published_generation(&controller.publication.published_generation)
            .expect("published generation") = Some(ProductGeneration::INITIAL);
        let (terminal_sender, terminal_receiver) = sync_channel(1);
        controller
            .attach_terminal_history_range_notifier(Arc::new(RecordingHistoryTerminalNotifier {
                sender: terminal_sender,
            }))
            .expect("attach terminal notifier");
        controller
            .refresh(DesktopRefreshUrgency::Hint)
            .expect("blocking refresh starts");
        entered_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("refresh entered");
        let intent = DesktopHistoryRangeIntent::new(
            epoch,
            ProductGeneration::INITIAL,
            DesktopHistoryRangeGeneration::new(1).expect("range generation"),
            DesktopHistoryRangePreset::Recent1Day,
        );
        assert!(matches!(
            controller
                .request_history_range(intent)
                .expect("range queues"),
            DesktopRefreshAdmission::Coalesced { .. }
        ));
        release_sender.send(()).expect("release refresh");
        assert_eq!(
            terminal_receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("abandoned follow-up terminal rollback"),
            intent
        );
        assert!(terminal_receiver.try_recv().is_err());
        assert_eq!(session_calls.load(Ordering::Acquire), 0);
        assert!(controller.take_snapshot().expect("mailbox").is_none());
        assert_eq!(
            controller
                .shutdown()
                .expect_err("faulted worker does not stop cleanly")
                .stable_code(),
            "faulted"
        );
    }

    fn invalid_query() -> QueryError {
        PageSize::new(0).expect_err("zero page size is invalid")
    }
}
