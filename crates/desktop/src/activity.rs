use std::sync::Arc;

use tokenmaster_product::ProductSnapshot;
use tokenmaster_query::UsageWeekday;

use crate::dashboard::{
    add_evidence_state, base_section, degrade, map_freshness, map_quality, map_token_count,
    map_tokens,
};
use crate::{
    DesktopDashboardSectionState, DesktopFreshness, DesktopHistoryRange, DesktopQuality,
    DesktopSectionReasonCodes, DesktopTokenValue,
};

pub const MAX_ACTIVITY_ROWS: usize = 12;
pub const ACTIVITY_RHYTHM_HOURS: usize = 24;
pub const ACTIVITY_RHYTHM_WEEKDAYS: usize = 7;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopActivityRhythmHourRow {
    hour: u8,
    event_count: u64,
    total: DesktopTokenValue,
    elapsed_minutes: u64,
    occurrence_count: u16,
}

impl DesktopActivityRhythmHourRow {
    #[must_use]
    pub const fn hour(&self) -> u8 {
        self.hour
    }
    #[must_use]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }
    #[must_use]
    pub const fn total_tokens(&self) -> DesktopTokenValue {
        self.total
    }
    #[must_use]
    pub const fn elapsed_minutes(&self) -> u64 {
        self.elapsed_minutes
    }
    #[must_use]
    pub const fn occurrence_count(&self) -> u16 {
        self.occurrence_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopActivityRhythmWeekdayRow {
    weekday: UsageWeekday,
    event_count: u64,
    total: DesktopTokenValue,
    elapsed_minutes: u64,
    occurrence_count: u16,
}

impl DesktopActivityRhythmWeekdayRow {
    #[must_use]
    pub const fn weekday(&self) -> UsageWeekday {
        self.weekday
    }
    #[must_use]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }
    #[must_use]
    pub const fn total_tokens(&self) -> DesktopTokenValue {
        self.total
    }
    #[must_use]
    pub const fn elapsed_minutes(&self) -> u64 {
        self.elapsed_minutes
    }
    #[must_use]
    pub const fn occurrence_count(&self) -> u16 {
        self.occurrence_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopActivityRhythmProjection {
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    time_zone_id: Option<Arc<str>>,
    range_start: Option<(i16, u8, u8)>,
    range_end: Option<(i16, u8, u8)>,
    hour_rows: Arc<[DesktopActivityRhythmHourRow]>,
    weekday_rows: Arc<[DesktopActivityRhythmWeekdayRow]>,
}

impl DesktopActivityRhythmProjection {
    fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let mut section = base_section(snapshot.history());
        let Some(envelope) = snapshot.history().payload() else {
            return Self::unavailable(section.state(), section.reason_codes());
        };
        let payload = envelope.payload();
        let Some(rhythm) = payload.rhythm() else {
            degrade(&mut section, "rhythm_unavailable");
            return Self::unavailable(section.state(), section.reason_codes());
        };
        add_evidence_state(
            &mut section,
            envelope.header().freshness(),
            envelope.header().quality(),
            payload.overview().event_count() > 0,
        );
        let hour_rows = rhythm
            .hours()
            .iter()
            .map(|row| {
                let metrics = row.metrics();
                DesktopActivityRhythmHourRow {
                    hour: row.hour(),
                    event_count: metrics.event_count(),
                    total: map_tokens(metrics.total(), metrics.event_count()),
                    elapsed_minutes: row.elapsed_minutes(),
                    occurrence_count: row.occurrence_count(),
                }
            })
            .collect::<Vec<_>>();
        let weekday_rows = rhythm
            .weekdays()
            .iter()
            .map(|row| {
                let metrics = row.metrics();
                DesktopActivityRhythmWeekdayRow {
                    weekday: row.weekday(),
                    event_count: metrics.event_count(),
                    total: map_tokens(metrics.total(), metrics.event_count()),
                    elapsed_minutes: row.elapsed_minutes(),
                    occurrence_count: row.occurrence_count(),
                }
            })
            .collect::<Vec<_>>();
        let range = payload.range();
        let start = range.start_date();
        let end = range.end_date();
        Self {
            state: section.state(),
            reason_codes: section.reason_codes(),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            time_zone_id: Some(Arc::from(payload.range().time_zone_id())),
            range_start: Some((start.year(), start.month(), start.day())),
            range_end: Some((end.year(), end.month(), end.day())),
            hour_rows: Arc::from(hour_rows),
            weekday_rows: Arc::from(weekday_rows),
        }
    }

    fn unavailable(
        state: DesktopDashboardSectionState,
        reason_codes: DesktopSectionReasonCodes,
    ) -> Self {
        Self {
            state,
            reason_codes,
            freshness: None,
            quality: None,
            time_zone_id: None,
            range_start: None,
            range_end: None,
            hour_rows: Arc::from(Vec::new()),
            weekday_rows: Arc::from(Vec::new()),
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
    pub fn time_zone_id(&self) -> Option<&str> {
        self.time_zone_id.as_deref()
    }
    #[must_use]
    pub const fn range(&self) -> Option<DesktopHistoryRange> {
        match (self.range_start, self.range_end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }
    #[must_use]
    pub const fn hour_rows(&self) -> &Arc<[DesktopActivityRhythmHourRow]> {
        &self.hour_rows
    }
    #[must_use]
    pub const fn weekday_rows(&self) -> &Arc<[DesktopActivityRhythmWeekdayRow]> {
        &self.weekday_rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopRecentActivityRow {
    timestamp_seconds: i64,
    timestamp_nanos: u32,
    model: Arc<str>,
    input: DesktopTokenValue,
    cached: DesktopTokenValue,
    output: DesktopTokenValue,
    reasoning: DesktopTokenValue,
    total: DesktopTokenValue,
}

impl DesktopRecentActivityRow {
    #[must_use]
    pub const fn timestamp_seconds(&self) -> i64 {
        self.timestamp_seconds
    }

    #[must_use]
    pub const fn timestamp_nanos(&self) -> u32 {
        self.timestamp_nanos
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopActivityProjection {
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    has_more: Option<bool>,
    rows: Arc<[DesktopRecentActivityRow]>,
    rhythm: DesktopActivityRhythmProjection,
}

impl DesktopActivityProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let rhythm = DesktopActivityRhythmProjection::from_snapshot(snapshot);
        let mut section = base_section(snapshot.activity());
        let Some(envelope) = snapshot.activity().payload() else {
            return Self::unavailable(section.state(), section.reason_codes(), rhythm);
        };
        let page = envelope.payload();
        add_evidence_state(
            &mut section,
            envelope.header().freshness(),
            envelope.header().quality(),
            true,
        );
        let truncated = page.items().len() > MAX_ACTIVITY_ROWS;
        let rows = page
            .items()
            .iter()
            .take(MAX_ACTIVITY_ROWS)
            .map(|item| {
                let usage = item.usage();
                DesktopRecentActivityRow {
                    timestamp_seconds: item.timestamp_seconds(),
                    timestamp_nanos: item.timestamp_nanos(),
                    model: Arc::from(item.model().as_str()),
                    input: map_token_count(usage.input()),
                    cached: map_token_count(usage.cached()),
                    output: map_token_count(usage.output()),
                    reasoning: map_token_count(usage.reasoning()),
                    total: map_token_count(usage.total()),
                }
            })
            .collect::<Vec<_>>();
        if truncated {
            degrade(&mut section, "activity_truncated");
        }
        Self {
            state: section.state(),
            reason_codes: section.reason_codes(),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            has_more: Some(page.has_more() || truncated),
            rows: Arc::from(rows),
            rhythm,
        }
    }

    fn unavailable(
        state: DesktopDashboardSectionState,
        reason_codes: DesktopSectionReasonCodes,
        rhythm: DesktopActivityRhythmProjection,
    ) -> Self {
        Self {
            state,
            reason_codes,
            freshness: None,
            quality: None,
            has_more: None,
            rows: Arc::from(Vec::new()),
            rhythm,
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
    pub const fn rows(&self) -> &Arc<[DesktopRecentActivityRow]> {
        &self.rows
    }

    #[must_use]
    pub const fn rhythm(&self) -> &DesktopActivityRhythmProjection {
        &self.rhythm
    }
}
