use std::{cell::RefCell, rc::Rc, sync::Arc};

use tokenmaster_product::ProductSnapshot;

use crate::dashboard::{
    add_evidence_state, base_section, map_cost, map_freshness, map_quality, map_tokens,
};
use crate::{
    DesktopCostValue, DesktopDashboardSectionState, DesktopFreshness, DesktopHistoryRangeIntent,
    DesktopHistoryRangePreset, DesktopQuality, DesktopSectionReasonCodes, DesktopTokenValue,
};

pub const MAX_HISTORY_DAYS: usize = 30;
pub type DesktopCalendarDate = (i16, u8, u8);
pub type DesktopHistoryRange = (DesktopCalendarDate, DesktopCalendarDate);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopHistoryRangeIntentAdmission {
    Accepted,
    Rejected,
}

pub trait DesktopHistoryRangeIntentSink {
    fn submit(&self, intent: DesktopHistoryRangeIntent) -> DesktopHistoryRangeIntentAdmission;
}

#[derive(Default)]
pub struct DesktopHistoryRangeIntentRouter {
    sink: RefCell<Option<Rc<dyn DesktopHistoryRangeIntentSink>>>,
}

impl DesktopHistoryRangeIntentRouter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sink: RefCell::new(None),
        }
    }

    pub fn install(
        &self,
        sink: Rc<dyn DesktopHistoryRangeIntentSink>,
    ) -> Result<(), DesktopHistoryRangeIntentRouterError> {
        let mut slot = self
            .sink
            .try_borrow_mut()
            .map_err(|_| DesktopHistoryRangeIntentRouterError)?;
        if slot.is_some() {
            return Err(DesktopHistoryRangeIntentRouterError);
        }
        *slot = Some(sink);
        Ok(())
    }
}

impl DesktopHistoryRangeIntentSink for DesktopHistoryRangeIntentRouter {
    fn submit(&self, intent: DesktopHistoryRangeIntent) -> DesktopHistoryRangeIntentAdmission {
        let Ok(slot) = self.sink.try_borrow() else {
            return DesktopHistoryRangeIntentAdmission::Rejected;
        };
        slot.as_ref()
            .map_or(DesktopHistoryRangeIntentAdmission::Rejected, |sink| {
                sink.submit(intent)
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopHistoryRangeIntentRouterError;

pub(crate) struct UnavailableDesktopHistoryRangeIntentSink;

impl DesktopHistoryRangeIntentSink for UnavailableDesktopHistoryRangeIntentSink {
    fn submit(&self, _intent: DesktopHistoryRangeIntent) -> DesktopHistoryRangeIntentAdmission {
        DesktopHistoryRangeIntentAdmission::Rejected
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopHistoryRow {
    year: i16,
    month: u8,
    day: u8,
    event_count: u64,
    input: DesktopTokenValue,
    cached: DesktopTokenValue,
    output: DesktopTokenValue,
    reasoning: DesktopTokenValue,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
}

impl DesktopHistoryRow {
    #[must_use]
    pub const fn date(&self) -> (i16, u8, u8) {
        (self.year, self.month, self.day)
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
pub struct DesktopHistoryProjection {
    range_preset: DesktopHistoryRangePreset,
    range_pending: bool,
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    range_start: Option<(i16, u8, u8)>,
    range_end: Option<(i16, u8, u8)>,
    time_zone_id: Option<Arc<str>>,
    event_count: Option<u64>,
    input: DesktopTokenValue,
    cached: DesktopTokenValue,
    output: DesktopTokenValue,
    reasoning: DesktopTokenValue,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    rows: Arc<[DesktopHistoryRow]>,
    token_maximum: Option<u64>,
    cost_maximum_micros: Option<u64>,
}

impl DesktopHistoryProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        Self::from_snapshot_with_range(snapshot, DesktopHistoryRangePreset::Recent30Days, false)
    }

    #[must_use]
    pub(crate) fn from_snapshot_with_range(
        snapshot: &ProductSnapshot,
        range_preset: DesktopHistoryRangePreset,
        range_pending: bool,
    ) -> Self {
        let mut section = base_section(snapshot.history());
        let Some(envelope) = snapshot.history().payload() else {
            return Self::unavailable(section.state(), section.reason_codes(), range_preset);
        };

        let payload = envelope.payload();
        let metrics = payload.overview();
        add_evidence_state(
            &mut section,
            envelope.header().freshness(),
            envelope.header().quality(),
            metrics.event_count() > 0,
        );
        let rows = payload
            .series()
            .iter()
            .rev()
            .take(MAX_HISTORY_DAYS)
            .map(|point| {
                let metrics = point.metrics();
                DesktopHistoryRow {
                    year: point.start_date().year(),
                    month: point.start_date().month(),
                    day: point.start_date().day(),
                    event_count: metrics.event_count(),
                    input: map_tokens(metrics.input(), metrics.event_count()),
                    cached: map_tokens(metrics.cached(), metrics.event_count()),
                    output: map_tokens(metrics.output(), metrics.event_count()),
                    reasoning: map_tokens(metrics.reasoning(), metrics.event_count()),
                    total: map_tokens(metrics.total(), metrics.event_count()),
                    cost: map_cost(point.cost()),
                }
            })
            .collect::<Vec<_>>();
        let token_maximum = rows.iter().filter_map(|row| row.total.known_sum()).max();
        let cost_maximum_micros = rows.iter().filter_map(|row| row.cost.micros()).max();
        let range = payload.range();
        let start = range.start_date();
        let end = range.end_date();

        Self {
            range_preset,
            range_pending,
            state: section.state(),
            reason_codes: section.reason_codes(),
            range_start: Some((start.year(), start.month(), start.day())),
            range_end: Some((end.year(), end.month(), end.day())),
            time_zone_id: Some(Arc::from(range.time_zone_id())),
            event_count: Some(metrics.event_count()),
            input: map_tokens(metrics.input(), metrics.event_count()),
            cached: map_tokens(metrics.cached(), metrics.event_count()),
            output: map_tokens(metrics.output(), metrics.event_count()),
            reasoning: map_tokens(metrics.reasoning(), metrics.event_count()),
            total: map_tokens(metrics.total(), metrics.event_count()),
            cost: map_cost(payload.overview_cost()),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            rows: Arc::from(rows),
            token_maximum,
            cost_maximum_micros,
        }
    }

    #[must_use]
    pub(crate) fn with_range_state(
        mut self,
        range_preset: DesktopHistoryRangePreset,
        range_pending: bool,
    ) -> Self {
        self.range_preset = range_preset;
        self.range_pending = range_pending;
        self
    }

    fn unavailable(
        state: DesktopDashboardSectionState,
        reason_codes: DesktopSectionReasonCodes,
        range_preset: DesktopHistoryRangePreset,
    ) -> Self {
        Self {
            range_preset,
            range_pending: false,
            state,
            reason_codes,
            range_start: None,
            range_end: None,
            time_zone_id: None,
            event_count: None,
            input: DesktopTokenValue::UNAVAILABLE,
            cached: DesktopTokenValue::UNAVAILABLE,
            output: DesktopTokenValue::UNAVAILABLE,
            reasoning: DesktopTokenValue::UNAVAILABLE,
            total: DesktopTokenValue::UNAVAILABLE,
            cost: DesktopCostValue::UNAVAILABLE,
            freshness: None,
            quality: None,
            rows: Arc::from(Vec::new()),
            token_maximum: None,
            cost_maximum_micros: None,
        }
    }

    #[must_use]
    pub const fn range_preset(&self) -> DesktopHistoryRangePreset {
        self.range_preset
    }

    #[must_use]
    pub const fn range_pending(&self) -> bool {
        self.range_pending
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
    pub const fn range(&self) -> Option<DesktopHistoryRange> {
        match (self.range_start, self.range_end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }

    #[must_use]
    pub fn time_zone_id(&self) -> Option<&str> {
        self.time_zone_id.as_deref()
    }

    #[must_use]
    pub const fn event_count(&self) -> Option<u64> {
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

    #[must_use]
    pub const fn freshness(&self) -> Option<DesktopFreshness> {
        self.freshness
    }

    #[must_use]
    pub const fn quality(&self) -> Option<DesktopQuality> {
        self.quality
    }

    #[must_use]
    pub const fn rows(&self) -> &Arc<[DesktopHistoryRow]> {
        &self.rows
    }

    #[must_use]
    pub const fn token_maximum(&self) -> Option<u64> {
        self.token_maximum
    }

    #[must_use]
    pub const fn cost_maximum_micros(&self) -> Option<u64> {
        self.cost_maximum_micros
    }
}
