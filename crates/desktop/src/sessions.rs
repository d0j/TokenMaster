use std::sync::Arc;

use tokenmaster_product::ProductSnapshot;

use crate::dashboard::{
    add_evidence_state, base_section, degrade, map_cost, map_freshness, map_quality, map_tokens,
};
use crate::{
    DesktopCostValue, DesktopDashboardSectionState, DesktopFreshness, DesktopQuality,
    DesktopSectionReasonCodes, DesktopTokenValue,
};

pub const MAX_SESSION_ROWS: usize = 64;

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
pub struct DesktopSessionsProjection {
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    has_more: Option<bool>,
    rows: Arc<[DesktopSessionListRow]>,
}

impl DesktopSessionsProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let mut section = base_section(snapshot.sessions());
        let Some(envelope) = snapshot.sessions().payload() else {
            return Self {
                state: section.state(),
                reason_codes: section.reason_codes(),
                freshness: None,
                quality: None,
                has_more: None,
                rows: Arc::from(Vec::new()),
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
            .map(|session| {
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
            })
            .collect::<Vec<_>>();

        Self {
            state: section.state(),
            reason_codes: section.reason_codes(),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            has_more: Some(payload.has_more() || source_rows.len() > MAX_SESSION_ROWS),
            rows: Arc::from(rows),
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
}
