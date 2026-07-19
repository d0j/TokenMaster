use std::sync::Arc;

use tokenmaster_product::ProductSnapshot;
use tokenmaster_query::{UsageBreakdownIdentity, UsageBreakdownKind};

use crate::dashboard::{
    add_evidence_state, base_section, degrade, map_cost, map_freshness, map_quality, map_tokens,
};
use crate::{
    DesktopCalendarDate, DesktopCostValue, DesktopDashboardSectionState, DesktopFreshness,
    DesktopHistoryRange, DesktopQuality, DesktopSectionReasonCodes, DesktopTokenValue,
};

pub const MAX_MODEL_ROWS: usize = 64;

fn model_identity_label(identity: &UsageBreakdownIdentity) -> Option<&str> {
    match identity {
        UsageBreakdownIdentity::Model(model) => Some(model.as_str()),
        UsageBreakdownIdentity::Project(_)
        | UsageBreakdownIdentity::UnassociatedProject
        | UsageBreakdownIdentity::Provider(_)
        | UsageBreakdownIdentity::Profile(_) => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopModelUsageRow {
    model: Arc<str>,
    event_count: u64,
    input: DesktopTokenValue,
    cached: DesktopTokenValue,
    output: DesktopTokenValue,
    reasoning: DesktopTokenValue,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
}

impl DesktopModelUsageRow {
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
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
pub struct DesktopModelsProjection {
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    range_start: Option<DesktopCalendarDate>,
    range_end: Option<DesktopCalendarDate>,
    time_zone_id: Option<Arc<str>>,
    event_count: Option<u64>,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    rows: Arc<[DesktopModelUsageRow]>,
    token_maximum: Option<u64>,
    truncated: bool,
}

impl DesktopModelsProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let mut section = base_section(snapshot.history());
        let Some(envelope) = snapshot.history().payload() else {
            return Self::unavailable(section.state(), section.reason_codes());
        };

        let payload = envelope.payload();
        let metrics = payload.overview();
        add_evidence_state(
            &mut section,
            envelope.header().freshness(),
            envelope.header().quality(),
            metrics.event_count() > 0,
        );

        let model_breakdown = payload
            .breakdowns()
            .iter()
            .find(|breakdown| breakdown.kind() == UsageBreakdownKind::Model);
        let (rows, truncated) = match model_breakdown {
            None => {
                degrade(&mut section, "models_breakdown_unavailable");
                (Vec::new(), false)
            }
            Some(breakdown) => {
                let rows = breakdown
                    .items()
                    .iter()
                    .filter_map(|item| {
                        let model = model_identity_label(item.identity())?;
                        let metrics = item.metrics();
                        Some(DesktopModelUsageRow {
                            model: Arc::from(model),
                            event_count: metrics.event_count(),
                            input: map_tokens(metrics.input(), metrics.event_count()),
                            cached: map_tokens(metrics.cached(), metrics.event_count()),
                            output: map_tokens(metrics.output(), metrics.event_count()),
                            reasoning: map_tokens(metrics.reasoning(), metrics.event_count()),
                            total: map_tokens(metrics.total(), metrics.event_count()),
                            cost: map_cost(item.cost()),
                        })
                    })
                    .take(MAX_MODEL_ROWS)
                    .collect::<Vec<_>>();
                let truncated = breakdown.truncated() || breakdown.items().len() > MAX_MODEL_ROWS;
                if truncated {
                    degrade(&mut section, "models_truncated");
                }
                (rows, truncated)
            }
        };
        let token_maximum = rows.iter().filter_map(|row| row.total.known_sum()).max();
        let range = payload.range();
        let start = range.start_date();
        let end = range.end_date();

        Self {
            state: section.state(),
            reason_codes: section.reason_codes(),
            range_start: Some((start.year(), start.month(), start.day())),
            range_end: Some((end.year(), end.month(), end.day())),
            time_zone_id: Some(Arc::from(range.time_zone_id())),
            event_count: Some(metrics.event_count()),
            total: map_tokens(metrics.total(), metrics.event_count()),
            cost: map_cost(payload.overview_cost()),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            rows: Arc::from(rows),
            token_maximum,
            truncated,
        }
    }

    fn unavailable(
        state: DesktopDashboardSectionState,
        reason_codes: DesktopSectionReasonCodes,
    ) -> Self {
        Self {
            state,
            reason_codes,
            range_start: None,
            range_end: None,
            time_zone_id: None,
            event_count: None,
            total: DesktopTokenValue::UNAVAILABLE,
            cost: DesktopCostValue::UNAVAILABLE,
            freshness: None,
            quality: None,
            rows: Arc::from(Vec::new()),
            token_maximum: None,
            truncated: false,
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
    pub const fn rows(&self) -> &Arc<[DesktopModelUsageRow]> {
        &self.rows
    }

    #[must_use]
    pub const fn token_maximum(&self) -> Option<u64> {
        self.token_maximum
    }

    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

#[cfg(test)]
mod tests {
    use tokenmaster_query::UsageBreakdownIdentity;

    use super::model_identity_label;

    #[test]
    fn mismatched_breakdown_identity_cannot_become_a_model_label() {
        assert_eq!(
            model_identity_label(&UsageBreakdownIdentity::UnassociatedProject),
            None
        );
    }
}
