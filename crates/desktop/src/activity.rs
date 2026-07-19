use std::sync::Arc;

use tokenmaster_product::ProductSnapshot;

use crate::dashboard::{
    add_evidence_state, base_section, degrade, map_freshness, map_quality, map_token_count,
};
use crate::{
    DesktopDashboardSectionState, DesktopFreshness, DesktopQuality, DesktopSectionReasonCodes,
    DesktopTokenValue,
};

pub const MAX_ACTIVITY_ROWS: usize = 12;

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
}

impl DesktopActivityProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let mut section = base_section(snapshot.activity());
        let Some(envelope) = snapshot.activity().payload() else {
            return Self::unavailable(section.state(), section.reason_codes());
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
            has_more: None,
            rows: Arc::from(Vec::new()),
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
}
