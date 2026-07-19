use std::{cell::RefCell, rc::Rc, sync::Arc};

use tokenmaster_product::ProductSnapshot;
use tokenmaster_query::{
    UsageBreakdownIdentity, UsageBreakdownKind, UsageSessionDetail, UsageSessionSummary,
};

use crate::dashboard::{
    add_evidence_state, base_section, degrade, map_cost, map_freshness, map_quality, map_tokens,
};
use crate::{
    DesktopCostValue, DesktopDashboardSectionState, DesktopFreshness, DesktopQuality,
    DesktopSectionReasonCodes, DesktopTokenValue, controller::DesktopSessionDetailIntent,
};

pub const MAX_SESSION_ROWS: usize = 64;
pub const MAX_SESSION_DETAIL_MODEL_ROWS: usize = 32;
pub const MAX_SESSION_DETAIL_PROJECT_ROWS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSessionDetailIntentAdmission {
    Accepted,
    Rejected,
}

pub trait DesktopSessionDetailIntentSink {
    fn submit(&self, intent: DesktopSessionDetailIntent) -> DesktopSessionDetailIntentAdmission;
}

#[derive(Default)]
pub struct DesktopSessionDetailIntentRouter {
    sink: RefCell<Option<Rc<dyn DesktopSessionDetailIntentSink>>>,
}

impl DesktopSessionDetailIntentRouter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sink: RefCell::new(None),
        }
    }

    pub fn install(
        &self,
        sink: Rc<dyn DesktopSessionDetailIntentSink>,
    ) -> Result<(), DesktopSessionDetailIntentRouterError> {
        let mut slot = self
            .sink
            .try_borrow_mut()
            .map_err(|_| DesktopSessionDetailIntentRouterError)?;
        if slot.is_some() {
            return Err(DesktopSessionDetailIntentRouterError);
        }
        *slot = Some(sink);
        Ok(())
    }
}

impl DesktopSessionDetailIntentSink for DesktopSessionDetailIntentRouter {
    fn submit(&self, intent: DesktopSessionDetailIntent) -> DesktopSessionDetailIntentAdmission {
        let Ok(slot) = self.sink.try_borrow() else {
            return DesktopSessionDetailIntentAdmission::Rejected;
        };
        slot.as_ref()
            .map_or(DesktopSessionDetailIntentAdmission::Rejected, |sink| {
                sink.submit(intent)
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopSessionDetailIntentRouterError;

pub(crate) struct UnavailableDesktopSessionDetailIntentSink;

impl DesktopSessionDetailIntentSink for UnavailableDesktopSessionDetailIntentSink {
    fn submit(&self, _intent: DesktopSessionDetailIntent) -> DesktopSessionDetailIntentAdmission {
        DesktopSessionDetailIntentAdmission::Rejected
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSessionDetailState {
    Idle,
    Loading,
    Ready,
    Missing,
    Unavailable,
}

impl DesktopSessionDetailState {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSessionBreakdownKind {
    Model,
    Project,
}

impl DesktopSessionBreakdownKind {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Project => "project",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopSessionListRow {
    first_timestamp_seconds: i64,
    first_timestamp_nanos: u32,
    last_timestamp_seconds: i64,
    last_timestamp_nanos: u32,
    event_count: u64,
    input: DesktopTokenValue,
    cached: DesktopTokenValue,
    output: DesktopTokenValue,
    reasoning: DesktopTokenValue,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
}

impl DesktopSessionListRow {
    #[must_use]
    pub const fn first_timestamp_seconds(&self) -> i64 {
        self.first_timestamp_seconds
    }

    #[must_use]
    pub const fn first_timestamp_nanos(&self) -> u32 {
        self.first_timestamp_nanos
    }

    #[must_use]
    pub const fn last_timestamp_seconds(&self) -> i64 {
        self.last_timestamp_seconds
    }

    #[must_use]
    pub const fn last_timestamp_nanos(&self) -> u32 {
        self.last_timestamp_nanos
    }

    #[must_use]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }

    #[must_use]
    pub const fn input(&self) -> DesktopTokenValue {
        self.input
    }

    #[must_use]
    pub const fn cached(&self) -> DesktopTokenValue {
        self.cached
    }

    #[must_use]
    pub const fn output(&self) -> DesktopTokenValue {
        self.output
    }

    #[must_use]
    pub const fn reasoning(&self) -> DesktopTokenValue {
        self.reasoning
    }

    #[must_use]
    pub const fn total_tokens(&self) -> DesktopTokenValue {
        self.total
    }

    #[must_use]
    pub const fn cost(&self) -> DesktopCostValue {
        self.cost
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopSessionBreakdownRow {
    kind: DesktopSessionBreakdownKind,
    label: Arc<str>,
    event_count: u64,
    input: DesktopTokenValue,
    cached: DesktopTokenValue,
    output: DesktopTokenValue,
    reasoning: DesktopTokenValue,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
}

impl DesktopSessionBreakdownRow {
    #[must_use]
    pub const fn kind(&self) -> DesktopSessionBreakdownKind {
        self.kind
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }

    #[must_use]
    pub const fn input(&self) -> DesktopTokenValue {
        self.input
    }

    #[must_use]
    pub const fn cached(&self) -> DesktopTokenValue {
        self.cached
    }

    #[must_use]
    pub const fn output(&self) -> DesktopTokenValue {
        self.output
    }

    #[must_use]
    pub const fn reasoning(&self) -> DesktopTokenValue {
        self.reasoning
    }

    #[must_use]
    pub const fn total_tokens(&self) -> DesktopTokenValue {
        self.total
    }

    #[must_use]
    pub const fn cost(&self) -> DesktopCostValue {
        self.cost
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopSessionDetailProjection {
    state: DesktopSessionDetailState,
    selected_ordinal: Option<u8>,
    failure_code: Option<&'static str>,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    summary: Option<DesktopSessionListRow>,
    breakdown_rows: Arc<[DesktopSessionBreakdownRow]>,
    truncated: bool,
}

impl DesktopSessionDetailProjection {
    fn idle() -> Self {
        Self {
            state: DesktopSessionDetailState::Idle,
            selected_ordinal: None,
            failure_code: None,
            freshness: None,
            quality: None,
            summary: None,
            breakdown_rows: Arc::from(Vec::new()),
            truncated: false,
        }
    }

    fn loading(selected_ordinal: u8) -> Self {
        Self {
            state: DesktopSessionDetailState::Loading,
            selected_ordinal: Some(selected_ordinal),
            ..Self::idle()
        }
    }

    fn unavailable(selected_ordinal: u8, failure_code: &'static str) -> Self {
        Self {
            state: DesktopSessionDetailState::Unavailable,
            selected_ordinal: Some(selected_ordinal),
            failure_code: Some(failure_code),
            ..Self::idle()
        }
    }

    fn from_snapshot(
        snapshot: &ProductSnapshot,
        active: Option<DesktopSessionDetailIntent>,
    ) -> Self {
        let Some(active) = active else {
            return Self::idle();
        };
        let ordinal = active.selection().row_ordinal();
        if snapshot.session_detail_selection() != Some(active.selection()) {
            return Self::loading(ordinal);
        }
        let section = snapshot.session_detail();
        if let Some(failure) = section.failure() {
            return Self::unavailable(ordinal, failure.code().stable_code());
        }
        let Some(envelope) = section.payload() else {
            return Self::loading(ordinal);
        };
        let Some(detail) = envelope.payload().detail() else {
            return Self {
                state: DesktopSessionDetailState::Missing,
                selected_ordinal: Some(ordinal),
                failure_code: None,
                freshness: None,
                quality: None,
                summary: None,
                breakdown_rows: Arc::from(Vec::new()),
                truncated: false,
            };
        };
        Self::ready(
            ordinal,
            detail,
            map_freshness(envelope.header().freshness()),
            map_quality(envelope.header().quality()),
        )
    }

    fn ready(
        selected_ordinal: u8,
        detail: &UsageSessionDetail,
        freshness: DesktopFreshness,
        quality: DesktopQuality,
    ) -> Self {
        let mut rows =
            Vec::with_capacity(MAX_SESSION_DETAIL_MODEL_ROWS + MAX_SESSION_DETAIL_PROJECT_ROWS);
        let mut model_count = 0_usize;
        let mut project_count = 0_usize;
        let mut truncated = false;
        for breakdown in detail.breakdowns().iter() {
            truncated |= breakdown.truncated();
            for item in breakdown.items().iter() {
                let mapped = match (breakdown.kind(), item.identity()) {
                    (UsageBreakdownKind::Model, UsageBreakdownIdentity::Model(value)) => {
                        if model_count >= MAX_SESSION_DETAIL_MODEL_ROWS {
                            truncated = true;
                            continue;
                        }
                        model_count += 1;
                        Some((DesktopSessionBreakdownKind::Model, value.as_str()))
                    }
                    (UsageBreakdownKind::Project, UsageBreakdownIdentity::Project(value)) => {
                        if project_count >= MAX_SESSION_DETAIL_PROJECT_ROWS {
                            truncated = true;
                            continue;
                        }
                        project_count += 1;
                        Some((DesktopSessionBreakdownKind::Project, value.as_str()))
                    }
                    (UsageBreakdownKind::Project, UsageBreakdownIdentity::UnassociatedProject) => {
                        if project_count >= MAX_SESSION_DETAIL_PROJECT_ROWS {
                            truncated = true;
                            continue;
                        }
                        project_count += 1;
                        Some((DesktopSessionBreakdownKind::Project, "Unassociated project"))
                    }
                    _ => None,
                };
                let Some((kind, label)) = mapped else {
                    truncated = true;
                    continue;
                };
                let metrics = item.metrics();
                rows.push(DesktopSessionBreakdownRow {
                    kind,
                    label: Arc::from(label),
                    event_count: metrics.event_count(),
                    input: map_tokens(metrics.input(), metrics.event_count()),
                    cached: map_tokens(metrics.cached(), metrics.event_count()),
                    output: map_tokens(metrics.output(), metrics.event_count()),
                    reasoning: map_tokens(metrics.reasoning(), metrics.event_count()),
                    total: map_tokens(metrics.total(), metrics.event_count()),
                    cost: map_cost(item.cost()),
                });
            }
        }
        Self {
            state: DesktopSessionDetailState::Ready,
            selected_ordinal: Some(selected_ordinal),
            failure_code: None,
            freshness: Some(freshness),
            quality: Some(quality),
            summary: Some(map_session_row(detail.summary())),
            breakdown_rows: Arc::from(rows),
            truncated,
        }
    }

    #[must_use]
    pub const fn state(&self) -> DesktopSessionDetailState {
        self.state
    }

    #[must_use]
    pub const fn selected_ordinal(&self) -> Option<u8> {
        self.selected_ordinal
    }

    #[must_use]
    pub const fn failure_code(&self) -> Option<&'static str> {
        self.failure_code
    }

    #[must_use]
    pub const fn freshness(&self) -> Option<DesktopFreshness> {
        self.freshness
    }

    #[must_use]
    pub const fn quality(&self) -> Option<DesktopQuality> {
        self.quality
    }

    #[must_use]
    pub const fn summary(&self) -> Option<&DesktopSessionListRow> {
        self.summary.as_ref()
    }

    #[must_use]
    pub const fn breakdown_rows(&self) -> &Arc<[DesktopSessionBreakdownRow]> {
        &self.breakdown_rows
    }

    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopSessionsProjection {
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    has_more: Option<bool>,
    rows: Arc<[DesktopSessionListRow]>,
    detail: DesktopSessionDetailProjection,
}

impl DesktopSessionsProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        Self::from_snapshot_with_selection(snapshot, None)
    }

    pub(crate) fn from_snapshot_with_selection(
        snapshot: &ProductSnapshot,
        active: Option<DesktopSessionDetailIntent>,
    ) -> Self {
        let mut section = base_section(snapshot.sessions());
        let Some(envelope) = snapshot.sessions().payload() else {
            return Self {
                state: section.state(),
                reason_codes: section.reason_codes(),
                freshness: None,
                quality: None,
                has_more: None,
                rows: Arc::from(Vec::new()),
                detail: DesktopSessionDetailProjection::from_snapshot(snapshot, active),
            };
        };

        let payload = envelope.payload();
        let source_rows = payload.sessions();
        add_evidence_state(
            &mut section,
            envelope.header().freshness(),
            envelope.header().quality(),
            !source_rows.is_empty(),
        );
        if source_rows.len() > MAX_SESSION_ROWS {
            degrade(&mut section, "sessions_truncated");
        }
        let rows = source_rows
            .iter()
            .take(MAX_SESSION_ROWS)
            .map(map_session_row)
            .collect::<Vec<_>>();

        Self {
            state: section.state(),
            reason_codes: section.reason_codes(),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            has_more: Some(payload.has_more() || source_rows.len() > MAX_SESSION_ROWS),
            rows: Arc::from(rows),
            detail: DesktopSessionDetailProjection::from_snapshot(snapshot, active),
        }
    }

    #[must_use]
    pub const fn state(&self) -> DesktopDashboardSectionState {
        self.state
    }

    #[must_use]
    pub const fn reason_codes(&self) -> DesktopSectionReasonCodes {
        self.reason_codes
    }

    #[must_use]
    pub const fn freshness(&self) -> Option<DesktopFreshness> {
        self.freshness
    }

    #[must_use]
    pub const fn quality(&self) -> Option<DesktopQuality> {
        self.quality
    }

    #[must_use]
    pub const fn has_more(&self) -> Option<bool> {
        self.has_more
    }

    #[must_use]
    pub const fn rows(&self) -> &Arc<[DesktopSessionListRow]> {
        &self.rows
    }

    #[must_use]
    pub const fn detail(&self) -> &DesktopSessionDetailProjection {
        &self.detail
    }

    pub(crate) fn start_detail(&mut self, selected_ordinal: u8) {
        self.detail = DesktopSessionDetailProjection::loading(selected_ordinal);
    }

    pub(crate) fn reject_detail(&mut self, selected_ordinal: u8) {
        self.detail =
            DesktopSessionDetailProjection::unavailable(selected_ordinal, "request_rejected");
    }
}

fn map_session_row(session: &UsageSessionSummary) -> DesktopSessionListRow {
    let metrics = session.metrics();
    DesktopSessionListRow {
        first_timestamp_seconds: session.first_timestamp_seconds(),
        first_timestamp_nanos: session.first_timestamp_nanos(),
        last_timestamp_seconds: session.last_timestamp_seconds(),
        last_timestamp_nanos: session.last_timestamp_nanos(),
        event_count: metrics.event_count(),
        input: map_tokens(metrics.input(), metrics.event_count()),
        cached: map_tokens(metrics.cached(), metrics.event_count()),
        output: map_tokens(metrics.output(), metrics.event_count()),
        reasoning: map_tokens(metrics.reasoning(), metrics.event_count()),
        total: map_tokens(metrics.total(), metrics.event_count()),
        cost: map_cost(session.cost()),
    }
}
