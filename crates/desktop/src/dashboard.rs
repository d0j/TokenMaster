use std::sync::Arc;

use tokenmaster_domain::{BenefitKind, BenefitState, GitOutputQuality};
use tokenmaster_product::{ProductSection, ProductSectionKind, ProductSnapshot};
use tokenmaster_query::{
    AggregateTokenValue, BenefitReminderCoverage, CostAvailability, CostResult, GitEfficiency,
    QueryFreshness, QueryQuality, QuotaConfidence, QuotaPresentation, QuotaTransitionKind,
    QuotaWindowSemantics, UsageBreakdownIdentity, UsageBreakdownKind, UsageMetrics,
};

pub const DESKTOP_DASHBOARD_SECTION_COUNT: usize = 6;
pub const MAX_DASHBOARD_QUOTA_ROWS: usize = 32;
pub const MAX_DASHBOARD_BENEFIT_SCOPES: usize = 32;
pub const MAX_DASHBOARD_TREND_POINTS: usize = 240;
pub const MAX_DASHBOARD_SESSIONS: usize = 12;
pub const DASHBOARD_ACTIVITY_ROWS: usize = 8;
pub const MAX_DASHBOARD_MODELS: usize = 12;
pub const MAX_DASHBOARD_REPOSITORIES: usize = 32;
const MAX_SECTION_REASONS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DesktopDashboardSectionKey {
    PlanUsage,
    CodeOutput,
    Trend,
    Sessions,
    Activity,
    Models,
}

impl DesktopDashboardSectionKey {
    pub const ALL: [Self; DESKTOP_DASHBOARD_SECTION_COUNT] = [
        Self::PlanUsage,
        Self::CodeOutput,
        Self::Trend,
        Self::Sessions,
        Self::Activity,
        Self::Models,
    ];

    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::PlanUsage => "plan_usage",
            Self::CodeOutput => "code_output",
            Self::Trend => "trend",
            Self::Sessions => "sessions",
            Self::Activity => "activity",
            Self::Models => "models",
        }
    }

    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::PlanUsage => "dashboard.plan_usage",
            Self::CodeOutput => "dashboard.code_output",
            Self::Trend => "dashboard.trend",
            Self::Sessions => "dashboard.sessions",
            Self::Activity => "dashboard.activity",
            Self::Models => "dashboard.models",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopDashboardSectionState {
    Waiting,
    Ready,
    Degraded,
    Unavailable,
}

impl DesktopDashboardSectionState {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopSectionReasonCodes {
    values: [Option<&'static str>; MAX_SECTION_REASONS],
    len: u8,
}

impl DesktopSectionReasonCodes {
    const fn empty() -> Self {
        Self {
            values: [None; MAX_SECTION_REASONS],
            len: 0,
        }
    }

    fn push(&mut self, value: &'static str) {
        if self.iter().any(|current| current == value) || self.len() >= MAX_SECTION_REASONS {
            return;
        }
        self.values[self.len()] = Some(value);
        self.len = self.len.saturating_add(1);
    }

    fn extend(&mut self, other: Self) {
        for value in other.iter() {
            self.push(value);
        }
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.len as usize
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.values[..self.len()].iter().filter_map(|value| *value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopDashboardSectionProjection {
    key: DesktopDashboardSectionKey,
    state: DesktopDashboardSectionState,
    has_data: bool,
    reason_codes: DesktopSectionReasonCodes,
}

impl DesktopDashboardSectionProjection {
    #[must_use]
    pub const fn key(self) -> DesktopDashboardSectionKey {
        self.key
    }

    #[must_use]
    pub const fn state(self) -> DesktopDashboardSectionState {
        self.state
    }

    #[must_use]
    pub const fn has_data(self) -> bool {
        self.has_data
    }

    #[must_use]
    pub const fn reason_codes(self) -> DesktopSectionReasonCodes {
        self.reason_codes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopValueAvailability {
    Unavailable,
    Known,
    Partial,
    Complete,
    LegitimateZero,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopTokenValue {
    availability: DesktopValueAvailability,
    known_sum: Option<u64>,
    known_count: u64,
    event_count: u64,
}

impl DesktopTokenValue {
    pub(crate) const UNAVAILABLE: Self = Self {
        availability: DesktopValueAvailability::Unavailable,
        known_sum: None,
        known_count: 0,
        event_count: 0,
    };

    #[must_use]
    pub const fn availability(self) -> DesktopValueAvailability {
        self.availability
    }

    #[must_use]
    pub const fn known_sum(self) -> Option<u64> {
        self.known_sum
    }

    #[must_use]
    pub const fn known_count(self) -> u64 {
        self.known_count
    }

    #[must_use]
    pub const fn event_count(self) -> u64 {
        self.event_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopCostValue {
    availability: DesktopValueAvailability,
    micros: Option<u64>,
    total_events: u64,
    priced_events: Option<u64>,
}

impl DesktopCostValue {
    pub(crate) const UNAVAILABLE: Self = Self {
        availability: DesktopValueAvailability::Unavailable,
        micros: None,
        total_events: 0,
        priced_events: None,
    };

    #[must_use]
    pub const fn availability(self) -> DesktopValueAvailability {
        self.availability
    }

    #[must_use]
    pub const fn micros(self) -> Option<u64> {
        self.micros
    }

    #[must_use]
    pub const fn total_events(self) -> u64 {
        self.total_events
    }

    #[must_use]
    pub const fn priced_events(self) -> Option<u64> {
        self.priced_events
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopFreshness {
    Fresh,
    Aging,
    Stale,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopQuality {
    Authoritative,
    Derived,
    Estimated,
    Partial,
    Conflict,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopHeaderProjection {
    tokens: DesktopTokenValue,
    cost: DesktopCostValue,
    event_count: Option<u64>,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    refresh_state: DesktopDashboardSectionState,
}

impl DesktopHeaderProjection {
    #[must_use]
    pub const fn tokens(self) -> DesktopTokenValue {
        self.tokens
    }

    #[must_use]
    pub const fn cost(self) -> DesktopCostValue {
        self.cost
    }

    #[must_use]
    pub const fn event_count(self) -> Option<u64> {
        self.event_count
    }

    #[must_use]
    pub const fn freshness(self) -> Option<DesktopFreshness> {
        self.freshness
    }

    #[must_use]
    pub const fn quality(self) -> Option<DesktopQuality> {
        self.quality
    }

    #[must_use]
    pub const fn refresh_state(self) -> DesktopDashboardSectionState {
        self.refresh_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopQuotaRow {
    ordinal: u8,
    label_key: Arc<str>,
    presentation: &'static str,
    semantics: &'static str,
    used_ppm: Option<u32>,
    remaining_ppm: Option<u32>,
    unit_key: Option<Arc<str>>,
    used_units: Option<u64>,
    remaining_units: Option<u64>,
    capacity_units: Option<u64>,
    advertised_reset_at_ms: Option<i64>,
    freshness: DesktopFreshness,
    quality: DesktopQuality,
    confidence: &'static str,
    transition_kind: Option<&'static str>,
    transition_sequence: Option<u64>,
}

impl DesktopQuotaRow {
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    #[must_use]
    pub fn label_key(&self) -> &str {
        &self.label_key
    }
    #[must_use]
    pub const fn presentation(&self) -> &'static str {
        self.presentation
    }
    #[must_use]
    pub const fn semantics(&self) -> &'static str {
        self.semantics
    }
    #[must_use]
    pub const fn used_ppm(&self) -> Option<u32> {
        self.used_ppm
    }
    #[must_use]
    pub const fn remaining_ppm(&self) -> Option<u32> {
        self.remaining_ppm
    }
    #[must_use]
    pub fn unit_key(&self) -> Option<&str> {
        self.unit_key.as_deref()
    }
    #[must_use]
    pub const fn used_units(&self) -> Option<u64> {
        self.used_units
    }
    #[must_use]
    pub const fn remaining_units(&self) -> Option<u64> {
        self.remaining_units
    }
    #[must_use]
    pub const fn capacity_units(&self) -> Option<u64> {
        self.capacity_units
    }
    #[must_use]
    pub const fn advertised_reset_at_ms(&self) -> Option<i64> {
        self.advertised_reset_at_ms
    }
    #[must_use]
    pub const fn freshness(&self) -> DesktopFreshness {
        self.freshness
    }
    #[must_use]
    pub const fn quality(&self) -> DesktopQuality {
        self.quality
    }
    #[must_use]
    pub const fn confidence(&self) -> &'static str {
        self.confidence
    }
    #[must_use]
    pub const fn transition_kind(&self) -> Option<&'static str> {
        self.transition_kind
    }
    #[must_use]
    pub const fn transition_sequence(&self) -> Option<u64> {
        self.transition_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopBenefitScope {
    ordinal: u8,
    available_reset_quantity: u64,
    available_credit_quantity: u64,
    available_temporary_quantity: u64,
    available_unknown_quantity: u64,
    non_available_quantity: u64,
    nearest_reset_expiry_at_ms: Option<i64>,
    nearest_due_at_ms: Option<i64>,
    reminder_coverage: &'static str,
    freshness: DesktopFreshness,
    quality: DesktopQuality,
}

impl DesktopBenefitScope {
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    #[must_use]
    pub const fn available_reset_quantity(&self) -> u64 {
        self.available_reset_quantity
    }
    #[must_use]
    pub const fn available_credit_quantity(&self) -> u64 {
        self.available_credit_quantity
    }
    #[must_use]
    pub const fn available_temporary_quantity(&self) -> u64 {
        self.available_temporary_quantity
    }
    #[must_use]
    pub const fn available_unknown_quantity(&self) -> u64 {
        self.available_unknown_quantity
    }
    #[must_use]
    pub const fn non_available_quantity(&self) -> u64 {
        self.non_available_quantity
    }
    #[must_use]
    pub const fn nearest_reset_expiry_at_ms(&self) -> Option<i64> {
        self.nearest_reset_expiry_at_ms
    }
    #[must_use]
    pub const fn nearest_due_at_ms(&self) -> Option<i64> {
        self.nearest_due_at_ms
    }
    #[must_use]
    pub const fn reminder_coverage(&self) -> &'static str {
        self.reminder_coverage
    }
    #[must_use]
    pub const fn freshness(&self) -> DesktopFreshness {
        self.freshness
    }
    #[must_use]
    pub const fn quality(&self) -> DesktopQuality {
        self.quality
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopCodeOutputProjection {
    repository_count: u8,
    commits: u64,
    added_lines: u64,
    removed_lines: u64,
    net_lines: i128,
    complete: bool,
    has_more_repositories: bool,
    freshness: DesktopFreshness,
    quality: DesktopQuality,
    cost_per_100_added_lines_micros: Option<u64>,
}

impl DesktopCodeOutputProjection {
    const EMPTY: Self = Self {
        repository_count: 0,
        commits: 0,
        added_lines: 0,
        removed_lines: 0,
        net_lines: 0,
        complete: false,
        has_more_repositories: false,
        freshness: DesktopFreshness::Unavailable,
        quality: DesktopQuality::Unknown,
        cost_per_100_added_lines_micros: None,
    };

    #[must_use]
    pub const fn repository_count(self) -> u8 {
        self.repository_count
    }
    #[must_use]
    pub const fn commits(self) -> u64 {
        self.commits
    }
    #[must_use]
    pub const fn added_lines(self) -> u64 {
        self.added_lines
    }
    #[must_use]
    pub const fn removed_lines(self) -> u64 {
        self.removed_lines
    }
    #[must_use]
    pub const fn net_lines(self) -> i128 {
        self.net_lines
    }
    #[must_use]
    pub const fn complete(self) -> bool {
        self.complete
    }
    #[must_use]
    pub const fn has_more_repositories(self) -> bool {
        self.has_more_repositories
    }
    #[must_use]
    pub const fn freshness(self) -> DesktopFreshness {
        self.freshness
    }
    #[must_use]
    pub const fn quality(self) -> DesktopQuality {
        self.quality
    }
    #[must_use]
    pub const fn cost_per_100_added_lines_micros(self) -> Option<u64> {
        self.cost_per_100_added_lines_micros
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopTrendPoint {
    start_year: i16,
    start_month: u8,
    start_day: u8,
    end_year: i16,
    end_month: u8,
    end_day: u8,
    tokens: DesktopTokenValue,
    cost: DesktopCostValue,
}

impl DesktopTrendPoint {
    #[must_use]
    pub const fn start_date(&self) -> (i16, u8, u8) {
        (self.start_year, self.start_month, self.start_day)
    }
    #[must_use]
    pub const fn end_date(&self) -> (i16, u8, u8) {
        (self.end_year, self.end_month, self.end_day)
    }
    #[must_use]
    pub const fn tokens(&self) -> DesktopTokenValue {
        self.tokens
    }
    #[must_use]
    pub const fn cost(&self) -> DesktopCostValue {
        self.cost
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopSessionRow {
    ordinal: u8,
    first_timestamp_seconds: i64,
    last_timestamp_seconds: i64,
    tokens: DesktopTokenValue,
    cost: DesktopCostValue,
}

impl DesktopSessionRow {
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    #[must_use]
    pub const fn first_timestamp_seconds(&self) -> i64 {
        self.first_timestamp_seconds
    }
    #[must_use]
    pub const fn last_timestamp_seconds(&self) -> i64 {
        self.last_timestamp_seconds
    }
    #[must_use]
    pub const fn tokens(&self) -> DesktopTokenValue {
        self.tokens
    }
    #[must_use]
    pub const fn cost(&self) -> DesktopCostValue {
        self.cost
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DesktopActivityKey {
    Read,
    EditWrite,
    Search,
    Git,
    BuildTest,
    Web,
    Subagents,
    Terminal,
}

impl DesktopActivityKey {
    pub const ALL: [Self; DASHBOARD_ACTIVITY_ROWS] = [
        Self::Read,
        Self::EditWrite,
        Self::Search,
        Self::Git,
        Self::BuildTest,
        Self::Web,
        Self::Subagents,
        Self::Terminal,
    ];

    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::EditWrite => "edit_write",
            Self::Search => "search",
            Self::Git => "git",
            Self::BuildTest => "build_test",
            Self::Web => "web",
            Self::Subagents => "subagents",
            Self::Terminal => "terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopActivityRow {
    key: DesktopActivityKey,
    count: u64,
}

impl DesktopActivityRow {
    #[must_use]
    pub const fn key(self) -> DesktopActivityKey {
        self.key
    }
    #[must_use]
    pub const fn count(self) -> u64 {
        self.count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopModelRow {
    ordinal: u8,
    model: Arc<str>,
    tokens: DesktopTokenValue,
    cost: DesktopCostValue,
}

impl DesktopModelRow {
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
    #[must_use]
    pub const fn tokens(&self) -> DesktopTokenValue {
        self.tokens
    }
    #[must_use]
    pub const fn cost(&self) -> DesktopCostValue {
        self.cost
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopDashboardProjection {
    sections: [DesktopDashboardSectionProjection; DESKTOP_DASHBOARD_SECTION_COUNT],
    header: DesktopHeaderProjection,
    quota_rows: Arc<[DesktopQuotaRow]>,
    benefit_scopes: Arc<[DesktopBenefitScope]>,
    code_output: DesktopCodeOutputProjection,
    trend_points: Arc<[DesktopTrendPoint]>,
    trend_max_tokens: Option<u64>,
    trend_max_cost_micros: Option<u64>,
    sessions: Arc<[DesktopSessionRow]>,
    sessions_truncated: bool,
    activity: [DesktopActivityRow; DASHBOARD_ACTIVITY_ROWS],
    models: Arc<[DesktopModelRow]>,
    models_truncated: bool,
}

impl DesktopDashboardProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let analytics = map_analytics(snapshot);
        let (quota_rows, benefit_scopes, plan_state) = map_plan_usage(snapshot);
        let (code_output, code_state) = map_code_output(snapshot);
        let (sessions, sessions_truncated, sessions_state) = map_sessions(snapshot);
        let sections = [
            plan_state,
            code_state,
            section_for_key(
                DesktopDashboardSectionKey::Trend,
                analytics.trend_section,
                !analytics.trend_points.is_empty(),
            ),
            sessions_state,
            section_for_key(
                DesktopDashboardSectionKey::Activity,
                analytics.activity_section,
                analytics.header.event_count.is_some_and(|count| count > 0),
            ),
            section_for_key(
                DesktopDashboardSectionKey::Models,
                analytics.models_section,
                !analytics.models.is_empty(),
            ),
        ];
        Self {
            sections,
            header: analytics.header,
            quota_rows,
            benefit_scopes,
            code_output,
            trend_points: analytics.trend_points,
            trend_max_tokens: analytics.trend_max_tokens,
            trend_max_cost_micros: analytics.trend_max_cost_micros,
            sessions,
            sessions_truncated,
            activity: analytics.activity,
            models: analytics.models,
            models_truncated: analytics.models_truncated,
        }
    }

    #[must_use]
    pub const fn sections(
        &self,
    ) -> &[DesktopDashboardSectionProjection; DESKTOP_DASHBOARD_SECTION_COUNT] {
        &self.sections
    }
    #[must_use]
    pub const fn section(
        &self,
        key: DesktopDashboardSectionKey,
    ) -> DesktopDashboardSectionProjection {
        self.sections[key.index()]
    }
    #[must_use]
    pub const fn header(&self) -> DesktopHeaderProjection {
        self.header
    }
    #[must_use]
    pub const fn quota_rows(&self) -> &Arc<[DesktopQuotaRow]> {
        &self.quota_rows
    }
    #[must_use]
    pub const fn benefit_scopes(&self) -> &Arc<[DesktopBenefitScope]> {
        &self.benefit_scopes
    }
    #[must_use]
    pub const fn code_output(&self) -> DesktopCodeOutputProjection {
        self.code_output
    }
    #[must_use]
    pub const fn trend_points(&self) -> &Arc<[DesktopTrendPoint]> {
        &self.trend_points
    }
    #[must_use]
    pub const fn trend_max_tokens(&self) -> Option<u64> {
        self.trend_max_tokens
    }
    #[must_use]
    pub const fn trend_max_cost_micros(&self) -> Option<u64> {
        self.trend_max_cost_micros
    }
    #[must_use]
    pub const fn sessions(&self) -> &Arc<[DesktopSessionRow]> {
        &self.sessions
    }
    #[must_use]
    pub const fn sessions_truncated(&self) -> bool {
        self.sessions_truncated
    }
    #[must_use]
    pub const fn activity(&self) -> &[DesktopActivityRow; DASHBOARD_ACTIVITY_ROWS] {
        &self.activity
    }
    #[must_use]
    pub const fn models(&self) -> &Arc<[DesktopModelRow]> {
        &self.models
    }
    #[must_use]
    pub const fn models_truncated(&self) -> bool {
        self.models_truncated
    }
}

struct MappedAnalytics {
    header: DesktopHeaderProjection,
    trend_section: DesktopDashboardSectionProjection,
    activity_section: DesktopDashboardSectionProjection,
    models_section: DesktopDashboardSectionProjection,
    trend_points: Arc<[DesktopTrendPoint]>,
    trend_max_tokens: Option<u64>,
    trend_max_cost_micros: Option<u64>,
    activity: [DesktopActivityRow; DASHBOARD_ACTIVITY_ROWS],
    models: Arc<[DesktopModelRow]>,
    models_truncated: bool,
}

fn map_analytics(snapshot: &ProductSnapshot) -> MappedAnalytics {
    let base = base_section(snapshot.analytics());
    let Some(envelope) = snapshot.analytics().payload() else {
        return MappedAnalytics {
            header: DesktopHeaderProjection {
                tokens: DesktopTokenValue::UNAVAILABLE,
                cost: DesktopCostValue::UNAVAILABLE,
                event_count: None,
                freshness: None,
                quality: None,
                refresh_state: base.state,
            },
            trend_section: base,
            activity_section: base,
            models_section: base,
            trend_points: Arc::from(Vec::new()),
            trend_max_tokens: None,
            trend_max_cost_micros: None,
            activity: empty_activity(),
            models: Arc::from(Vec::new()),
            models_truncated: false,
        };
    };
    let payload = envelope.payload();
    let metrics = payload.overview();
    let mut common_state = base;
    add_evidence_state(
        &mut common_state,
        envelope.header().freshness(),
        envelope.header().quality(),
        metrics.event_count() > 0,
    );
    let trend = payload
        .series()
        .iter()
        .take(MAX_DASHBOARD_TREND_POINTS)
        .map(|point| DesktopTrendPoint {
            start_year: point.start_date().year(),
            start_month: point.start_date().month(),
            start_day: point.start_date().day(),
            end_year: point.end_date().year(),
            end_month: point.end_date().month(),
            end_day: point.end_date().day(),
            tokens: map_tokens(point.metrics().total(), point.metrics().event_count()),
            cost: map_cost(point.cost()),
        })
        .collect::<Vec<_>>();
    let mut trend_section = common_state;
    if payload.series().len() > MAX_DASHBOARD_TREND_POINTS {
        degrade(&mut trend_section, "trend_truncated");
    }
    let trend_max_tokens = trend
        .iter()
        .filter_map(|point| point.tokens.known_sum)
        .max();
    let trend_max_cost_micros = trend.iter().filter_map(|point| point.cost.micros).max();
    let (models, models_truncated) = map_models(payload.breakdowns());
    let mut models_section = common_state;
    if models_truncated {
        degrade(&mut models_section, "models_truncated");
    }
    MappedAnalytics {
        header: DesktopHeaderProjection {
            tokens: map_tokens(metrics.total(), metrics.event_count()),
            cost: map_cost(payload.overview_cost()),
            event_count: Some(metrics.event_count()),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            refresh_state: common_state.state,
        },
        trend_section,
        activity_section: common_state,
        models_section,
        trend_points: Arc::from(trend),
        trend_max_tokens,
        trend_max_cost_micros,
        activity: map_activity(metrics),
        models,
        models_truncated,
    }
}

fn map_plan_usage(
    snapshot: &ProductSnapshot,
) -> (
    Arc<[DesktopQuotaRow]>,
    Arc<[DesktopBenefitScope]>,
    DesktopDashboardSectionProjection,
) {
    let mut quota_state = base_section(snapshot.quota());
    let quota_rows = snapshot
        .quota()
        .payload()
        .map_or_else(Vec::new, |envelope| {
            let rows = envelope
                .payload()
                .windows()
                .iter()
                .take(MAX_DASHBOARD_QUOTA_ROWS)
                .enumerate()
                .map(|(index, result)| map_quota_row(index, result.snapshot()))
                .collect::<Vec<_>>();
            if envelope.payload().windows().len() > MAX_DASHBOARD_QUOTA_ROWS {
                degrade(&mut quota_state, "quota_truncated");
            }
            add_evidence_state(
                &mut quota_state,
                envelope.header().freshness(),
                envelope.header().quality(),
                !rows.is_empty(),
            );
            rows
        });

    let mut benefit_state = base_section(snapshot.benefit());
    let benefit_result = snapshot.benefit().payload().map_or_else(
        || Ok(Vec::new()),
        |envelope| {
            let scopes = envelope
                .payload()
                .scopes()
                .iter()
                .take(MAX_DASHBOARD_BENEFIT_SCOPES)
                .enumerate()
                .map(|(index, scope)| map_benefit_scope(index, scope))
                .collect::<Result<Vec<_>, _>>()?;
            if envelope.payload().scopes().len() > MAX_DASHBOARD_BENEFIT_SCOPES {
                degrade(&mut benefit_state, "benefit_truncated");
            }
            add_evidence_state(
                &mut benefit_state,
                envelope.header().freshness(),
                envelope.header().quality(),
                !scopes.is_empty(),
            );
            Ok(scopes)
        },
    );
    let benefit_scopes = match benefit_result {
        Ok(scopes) => scopes,
        Err(()) => {
            benefit_state.state = DesktopDashboardSectionState::Unavailable;
            benefit_state.has_data = false;
            benefit_state.reason_codes.push("overflow");
            Vec::new()
        }
    };
    let mut plan = combine_sections(
        DesktopDashboardSectionKey::PlanUsage,
        quota_state,
        benefit_state,
    );
    plan.has_data = !quota_rows.is_empty() || !benefit_scopes.is_empty();
    (Arc::from(quota_rows), Arc::from(benefit_scopes), plan)
}

fn map_code_output(
    snapshot: &ProductSnapshot,
) -> (
    DesktopCodeOutputProjection,
    DesktopDashboardSectionProjection,
) {
    let mut section = base_section(snapshot.git());
    let Some(envelope) = snapshot.git().payload() else {
        section.key = DesktopDashboardSectionKey::CodeOutput;
        return (DesktopCodeOutputProjection::EMPTY, section);
    };
    let repositories = envelope.payload().repositories();
    let mut commits = 0_u64;
    let mut added = 0_u64;
    let mut removed = 0_u64;
    let mut efficiency_usage = 0_u64;
    let mut efficiency_lines = 0_u64;
    let mut efficiency_complete = !repositories.is_empty();
    let mut complete = !envelope.payload().has_more_repositories();
    let mut freshness = QueryFreshness::Fresh;
    let mut quality = GitOutputQuality::Complete;
    for repository in repositories.iter().take(MAX_DASHBOARD_REPOSITORIES) {
        let totals = repository.range_totals();
        let Some(next_commits) = commits.checked_add(totals.commits()) else {
            return code_overflow(section);
        };
        let Some(next_added) = added.checked_add(totals.lines().added()) else {
            return code_overflow(section);
        };
        let Some(next_removed) = removed.checked_add(totals.lines().removed()) else {
            return code_overflow(section);
        };
        commits = next_commits;
        added = next_added;
        removed = next_removed;
        complete &= repository.range_complete()
            && repository.quality() == GitOutputQuality::Complete
            && repository.unavailable_reason().is_none()
            && !repository.rebuild_required();
        freshness = worst_freshness(freshness, repository.freshness());
        quality = worst_git_quality(quality, repository.quality());
        match repository.efficiency() {
            GitEfficiency::Available(value) => {
                let Some(next_usage) = efficiency_usage.checked_add(value.usage_cost().get())
                else {
                    return code_overflow(section);
                };
                let Some(next_lines) =
                    efficiency_lines.checked_add(value.product_code_added_lines())
                else {
                    return code_overflow(section);
                };
                efficiency_usage = next_usage;
                efficiency_lines = next_lines;
            }
            GitEfficiency::Unavailable(_) => efficiency_complete = false,
        }
    }
    section.key = DesktopDashboardSectionKey::CodeOutput;
    section.has_data = !repositories.is_empty();
    if !repositories.is_empty() {
        add_evidence_state(&mut section, freshness, map_git_quality(quality), true);
    }
    if repositories.len() > MAX_DASHBOARD_REPOSITORIES {
        degrade(&mut section, "repository_truncated");
        complete = false;
    }
    if !complete && !repositories.is_empty() {
        degrade(&mut section, "git_incomplete");
    }
    let efficiency = if efficiency_complete && efficiency_lines > 0 {
        u128::from(efficiency_usage)
            .checked_mul(100)
            .and_then(|value| value.checked_add(u128::from(efficiency_lines / 2)))
            .map(|value| value / u128::from(efficiency_lines))
            .and_then(|value| u64::try_from(value).ok())
    } else {
        None
    };
    (
        DesktopCodeOutputProjection {
            repository_count: u8::try_from(repositories.len().min(MAX_DASHBOARD_REPOSITORIES))
                .map_or(u8::MAX, |value| value),
            commits,
            added_lines: added,
            removed_lines: removed,
            net_lines: i128::from(added) - i128::from(removed),
            complete,
            has_more_repositories: envelope.payload().has_more_repositories(),
            freshness: if repositories.is_empty() {
                DesktopFreshness::Unavailable
            } else {
                map_freshness(freshness)
            },
            quality: if repositories.is_empty() {
                DesktopQuality::Unknown
            } else {
                map_quality(map_git_quality(quality))
            },
            cost_per_100_added_lines_micros: efficiency,
        },
        section,
    )
}

fn map_sessions(
    snapshot: &ProductSnapshot,
) -> (
    Arc<[DesktopSessionRow]>,
    bool,
    DesktopDashboardSectionProjection,
) {
    let mut section = base_section(snapshot.sessions());
    section.key = DesktopDashboardSectionKey::Sessions;
    let Some(envelope) = snapshot.sessions().payload() else {
        return (Arc::from(Vec::new()), false, section);
    };
    let payload = envelope.payload();
    let sessions = payload
        .sessions()
        .iter()
        .take(MAX_DASHBOARD_SESSIONS)
        .enumerate()
        .map(|(index, session)| DesktopSessionRow {
            ordinal: ordinal(index),
            first_timestamp_seconds: session.first_timestamp_seconds(),
            last_timestamp_seconds: session.last_timestamp_seconds(),
            tokens: map_tokens(session.metrics().total(), session.metrics().event_count()),
            cost: map_cost(session.cost()),
        })
        .collect::<Vec<_>>();
    let truncated = payload.sessions().len() > MAX_DASHBOARD_SESSIONS || payload.has_more();
    if truncated {
        degrade(&mut section, "sessions_truncated");
    }
    add_evidence_state(
        &mut section,
        envelope.header().freshness(),
        envelope.header().quality(),
        !sessions.is_empty(),
    );
    section.has_data = !sessions.is_empty();
    (Arc::from(sessions), truncated, section)
}

fn map_models(breakdowns: &[tokenmaster_query::UsageBreakdown]) -> (Arc<[DesktopModelRow]>, bool) {
    let Some(models) = breakdowns
        .iter()
        .find(|breakdown| breakdown.kind() == UsageBreakdownKind::Model)
    else {
        return (Arc::from(Vec::new()), false);
    };
    let rows = models
        .items()
        .iter()
        .filter_map(|item| match item.identity() {
            UsageBreakdownIdentity::Model(model) => Some(DesktopModelRow {
                ordinal: 0,
                model: Arc::from(model.as_str()),
                tokens: map_tokens(item.metrics().total(), item.metrics().event_count()),
                cost: map_cost(item.cost()),
            }),
            UsageBreakdownIdentity::Project(_)
            | UsageBreakdownIdentity::UnassociatedProject
            | UsageBreakdownIdentity::Provider(_)
            | UsageBreakdownIdentity::Profile(_) => None,
        })
        .take(MAX_DASHBOARD_MODELS)
        .enumerate()
        .map(|(index, mut row)| {
            row.ordinal = ordinal(index);
            row
        })
        .collect::<Vec<_>>();
    let truncated = models.items().len() > MAX_DASHBOARD_MODELS || models.truncated();
    (Arc::from(rows), truncated)
}

fn map_quota_row(
    index: usize,
    snapshot: Option<&tokenmaster_query::QuotaWindowValue>,
) -> DesktopQuotaRow {
    let Some(snapshot) = snapshot else {
        return DesktopQuotaRow {
            ordinal: ordinal(index),
            label_key: Arc::from("quota.window_unavailable"),
            presentation: "unknown",
            semantics: "unknown",
            used_ppm: None,
            remaining_ppm: None,
            unit_key: None,
            used_units: None,
            remaining_units: None,
            capacity_units: None,
            advertised_reset_at_ms: None,
            freshness: DesktopFreshness::Unavailable,
            quality: DesktopQuality::Unknown,
            confidence: "unknown",
            transition_kind: None,
            transition_sequence: None,
        };
    };
    let sample = snapshot.current_sample();
    let units = sample.units();
    DesktopQuotaRow {
        ordinal: ordinal(index),
        label_key: Arc::from(snapshot.definition().label_key()),
        presentation: match snapshot.definition().presentation() {
            QuotaPresentation::Used => "used",
            QuotaPresentation::Remaining => "remaining",
            QuotaPresentation::Pace => "pace",
        },
        semantics: match snapshot.definition().semantics() {
            QuotaWindowSemantics::Fixed => "fixed",
            QuotaWindowSemantics::Rolling => "rolling",
            QuotaWindowSemantics::Credit => "credit",
            QuotaWindowSemantics::Unknown => "unknown",
        },
        used_ppm: sample.used_ratio().map(|value| value.parts_per_million()),
        remaining_ppm: sample
            .remaining_ratio()
            .map(|value| value.parts_per_million()),
        unit_key: units.map(|value| Arc::from(value.unit_id())),
        used_units: units.and_then(|value| value.used()),
        remaining_units: units.and_then(|value| value.remaining()),
        capacity_units: units.and_then(|value| value.capacity()),
        advertised_reset_at_ms: sample.advertised_resets_at_ms(),
        freshness: map_freshness(snapshot.freshness()),
        quality: map_quality(snapshot.quality()),
        confidence: map_confidence(sample.confidence()),
        transition_kind: snapshot
            .last_transition()
            .map(|value| map_transition(value.kind())),
        transition_sequence: snapshot.last_transition().map(|value| value.sequence()),
    }
}

fn map_benefit_scope(
    index: usize,
    scope: &tokenmaster_query::BenefitOverviewScopeValue,
) -> Result<DesktopBenefitScope, ()> {
    let mut available_reset_quantity = 0_u64;
    let mut available_credit_quantity = 0_u64;
    let mut available_temporary_quantity = 0_u64;
    let mut available_unknown_quantity = 0_u64;
    let mut non_available_quantity = 0_u64;
    let mut nearest_reset_expiry_at_ms = None;
    for lot in scope.current_lots().iter() {
        if lot.state() == BenefitState::Available {
            let target = match lot.kind() {
                BenefitKind::BankedRateLimitReset => &mut available_reset_quantity,
                BenefitKind::UsageCredit => &mut available_credit_quantity,
                BenefitKind::TemporaryUsage => &mut available_temporary_quantity,
                BenefitKind::Unknown => &mut available_unknown_quantity,
            };
            *target = target.checked_add(lot.quantity()).ok_or(())?;
            if lot.kind() == BenefitKind::BankedRateLimitReset
                && let Some(expiry) = lot.expiry().conservative_utc_ms()
            {
                nearest_reset_expiry_at_ms = Some(
                    nearest_reset_expiry_at_ms.map_or(expiry, |current: i64| current.min(expiry)),
                );
            }
        } else {
            non_available_quantity = non_available_quantity
                .checked_add(lot.quantity())
                .ok_or(())?;
        }
    }
    Ok(DesktopBenefitScope {
        ordinal: ordinal(index),
        available_reset_quantity,
        available_credit_quantity,
        available_temporary_quantity,
        available_unknown_quantity,
        non_available_quantity,
        nearest_reset_expiry_at_ms,
        nearest_due_at_ms: scope.nearest_due_at_ms(),
        reminder_coverage: match scope.reminder_profile().coverage() {
            BenefitReminderCoverage::Disabled => "disabled",
            BenefitReminderCoverage::InAppOnly => "in_app_only",
        },
        freshness: map_freshness(scope.freshness()),
        quality: map_quality(scope.quality()),
    })
}

fn map_activity(metrics: &UsageMetrics) -> [DesktopActivityRow; DASHBOARD_ACTIVITY_ROWS] {
    let activity = metrics.activity();
    let counts = [
        activity.read(),
        activity.edit_write(),
        activity.search(),
        activity.git(),
        activity.build_test(),
        activity.web(),
        activity.subagents(),
        activity.terminal(),
    ];
    std::array::from_fn(|index| DesktopActivityRow {
        key: DesktopActivityKey::ALL[index],
        count: counts[index],
    })
}

fn empty_activity() -> [DesktopActivityRow; DASHBOARD_ACTIVITY_ROWS] {
    std::array::from_fn(|index| DesktopActivityRow {
        key: DesktopActivityKey::ALL[index],
        count: 0,
    })
}

pub(crate) fn map_tokens(value: AggregateTokenValue, event_count: u64) -> DesktopTokenValue {
    match value {
        AggregateTokenValue::Unavailable => DesktopTokenValue {
            event_count,
            ..DesktopTokenValue::UNAVAILABLE
        },
        AggregateTokenValue::Known(value) => DesktopTokenValue {
            availability: DesktopValueAvailability::Known,
            known_sum: Some(value),
            known_count: event_count,
            event_count,
        },
        AggregateTokenValue::Partial {
            known_sum,
            known_count,
            event_count,
        } => DesktopTokenValue {
            availability: DesktopValueAvailability::Partial,
            known_sum: Some(known_sum),
            known_count,
            event_count,
        },
    }
}

pub(crate) fn map_cost(value: &CostResult) -> DesktopCostValue {
    let counters = value.counters();
    let priced_events = counters
        .priced_events
        .checked_add(counters.reported_events)
        .filter(|count| *count <= counters.total_events);
    let counters_valid = priced_events.is_some();
    DesktopCostValue {
        availability: if counters_valid {
            match value.availability() {
                CostAvailability::Complete => DesktopValueAvailability::Complete,
                CostAvailability::Partial => DesktopValueAvailability::Partial,
                CostAvailability::Unavailable => DesktopValueAvailability::Unavailable,
                CostAvailability::Zero => DesktopValueAvailability::LegitimateZero,
            }
        } else {
            DesktopValueAvailability::Unavailable
        },
        micros: if counters_valid {
            value.amount().map(|amount| amount.get())
        } else {
            None
        },
        total_events: counters.total_events,
        priced_events,
    }
}

pub(crate) fn base_section<T>(section: &ProductSection<T>) -> DesktopDashboardSectionProjection {
    let mut reasons = DesktopSectionReasonCodes::empty();
    if let Some(failure) = section.failure() {
        reasons.push(failure.code().stable_code());
    }
    let state = match section.kind() {
        ProductSectionKind::Waiting => DesktopDashboardSectionState::Waiting,
        ProductSectionKind::Ready => DesktopDashboardSectionState::Ready,
        ProductSectionKind::Unavailable if section.retains_payload() => {
            DesktopDashboardSectionState::Degraded
        }
        ProductSectionKind::Unavailable => DesktopDashboardSectionState::Unavailable,
    };
    DesktopDashboardSectionProjection {
        key: DesktopDashboardSectionKey::Trend,
        state,
        has_data: section.payload().is_some(),
        reason_codes: reasons,
    }
}

fn section_for_key(
    key: DesktopDashboardSectionKey,
    mut section: DesktopDashboardSectionProjection,
    has_data: bool,
) -> DesktopDashboardSectionProjection {
    section.key = key;
    section.has_data = has_data;
    section
}

fn combine_sections(
    key: DesktopDashboardSectionKey,
    left: DesktopDashboardSectionProjection,
    right: DesktopDashboardSectionProjection,
) -> DesktopDashboardSectionProjection {
    let state = match (left.state, right.state) {
        (DesktopDashboardSectionState::Waiting, DesktopDashboardSectionState::Waiting) => {
            DesktopDashboardSectionState::Waiting
        }
        (DesktopDashboardSectionState::Ready, DesktopDashboardSectionState::Ready) => {
            DesktopDashboardSectionState::Ready
        }
        (DesktopDashboardSectionState::Unavailable, DesktopDashboardSectionState::Unavailable) => {
            DesktopDashboardSectionState::Unavailable
        }
        (DesktopDashboardSectionState::Waiting, DesktopDashboardSectionState::Unavailable)
        | (DesktopDashboardSectionState::Unavailable, DesktopDashboardSectionState::Waiting) => {
            DesktopDashboardSectionState::Unavailable
        }
        _ => DesktopDashboardSectionState::Degraded,
    };
    let mut reasons = left.reason_codes;
    reasons.extend(right.reason_codes);
    DesktopDashboardSectionProjection {
        key,
        state,
        has_data: left.has_data || right.has_data,
        reason_codes: reasons,
    }
}

pub(crate) fn add_evidence_state(
    section: &mut DesktopDashboardSectionProjection,
    freshness: QueryFreshness,
    quality: QueryQuality,
    has_data: bool,
) {
    section.has_data = has_data;
    if !has_data || section.state != DesktopDashboardSectionState::Ready {
        return;
    }
    match freshness {
        QueryFreshness::Fresh => {}
        QueryFreshness::Aging => degrade(section, "aging"),
        QueryFreshness::Stale => degrade(section, "stale"),
        QueryFreshness::Unavailable => degrade(section, "freshness_unavailable"),
    }
    match quality {
        QueryQuality::Authoritative | QueryQuality::Derived => {}
        QueryQuality::Estimated => degrade(section, "estimated"),
        QueryQuality::Partial => degrade(section, "partial"),
        QueryQuality::Conflict => degrade(section, "conflict"),
        QueryQuality::Unknown => degrade(section, "quality_unknown"),
    }
}

pub(crate) fn degrade(section: &mut DesktopDashboardSectionProjection, reason: &'static str) {
    if section.state == DesktopDashboardSectionState::Ready {
        section.state = DesktopDashboardSectionState::Degraded;
    }
    section.reason_codes.push(reason);
}

fn code_overflow(
    mut section: DesktopDashboardSectionProjection,
) -> (
    DesktopCodeOutputProjection,
    DesktopDashboardSectionProjection,
) {
    section.key = DesktopDashboardSectionKey::CodeOutput;
    section.state = DesktopDashboardSectionState::Unavailable;
    section.has_data = false;
    section.reason_codes.push("overflow");
    (DesktopCodeOutputProjection::EMPTY, section)
}

pub(crate) const fn map_freshness(value: QueryFreshness) -> DesktopFreshness {
    match value {
        QueryFreshness::Fresh => DesktopFreshness::Fresh,
        QueryFreshness::Aging => DesktopFreshness::Aging,
        QueryFreshness::Stale => DesktopFreshness::Stale,
        QueryFreshness::Unavailable => DesktopFreshness::Unavailable,
    }
}

pub(crate) const fn map_quality(value: QueryQuality) -> DesktopQuality {
    match value {
        QueryQuality::Authoritative => DesktopQuality::Authoritative,
        QueryQuality::Derived => DesktopQuality::Derived,
        QueryQuality::Estimated => DesktopQuality::Estimated,
        QueryQuality::Partial => DesktopQuality::Partial,
        QueryQuality::Conflict => DesktopQuality::Conflict,
        QueryQuality::Unknown => DesktopQuality::Unknown,
    }
}

const fn map_git_quality(value: GitOutputQuality) -> QueryQuality {
    match value {
        GitOutputQuality::Complete => QueryQuality::Authoritative,
        GitOutputQuality::Partial => QueryQuality::Partial,
        GitOutputQuality::Unavailable => QueryQuality::Unknown,
    }
}

const fn worst_freshness(left: QueryFreshness, right: QueryFreshness) -> QueryFreshness {
    match (left, right) {
        (QueryFreshness::Unavailable, _) | (_, QueryFreshness::Unavailable) => {
            QueryFreshness::Unavailable
        }
        (QueryFreshness::Stale, _) | (_, QueryFreshness::Stale) => QueryFreshness::Stale,
        (QueryFreshness::Aging, _) | (_, QueryFreshness::Aging) => QueryFreshness::Aging,
        (QueryFreshness::Fresh, QueryFreshness::Fresh) => QueryFreshness::Fresh,
    }
}

const fn worst_git_quality(left: GitOutputQuality, right: GitOutputQuality) -> GitOutputQuality {
    match (left, right) {
        (GitOutputQuality::Unavailable, _) | (_, GitOutputQuality::Unavailable) => {
            GitOutputQuality::Unavailable
        }
        (GitOutputQuality::Partial, _) | (_, GitOutputQuality::Partial) => {
            GitOutputQuality::Partial
        }
        (GitOutputQuality::Complete, GitOutputQuality::Complete) => GitOutputQuality::Complete,
    }
}

const fn map_confidence(value: QuotaConfidence) -> &'static str {
    match value {
        QuotaConfidence::High => "high",
        QuotaConfidence::Medium => "medium",
        QuotaConfidence::Low => "low",
        QuotaConfidence::Unknown => "unknown",
    }
}

const fn map_transition(value: QuotaTransitionKind) -> &'static str {
    match value {
        QuotaTransitionKind::ScheduledReset => "scheduled_reset",
        QuotaTransitionKind::EarlyReset => "early_reset",
        QuotaTransitionKind::ManualOrBankedReset => "manual_or_banked_reset",
        QuotaTransitionKind::UnknownReset => "unknown_reset",
        QuotaTransitionKind::AllowanceChanged => "allowance_changed",
    }
}

fn ordinal(index: usize) -> u8 {
    u8::try_from(index.saturating_add(1)).map_or(u8::MAX, |value| value)
}
