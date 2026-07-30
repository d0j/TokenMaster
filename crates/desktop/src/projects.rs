use std::sync::Arc;

use tokenmaster_domain::GitOutputQuality;
use tokenmaster_product::ProductSnapshot;
use tokenmaster_query::{
    CostAvailability, DatasetIdentity, GitEfficiency, GitOutputSnapshot, QueryFreshness,
    UsageBreakdownIdentity, UsageBreakdownKind,
};

use crate::dashboard::{
    add_evidence_state, base_section, combine_sections, degrade, map_cost, map_freshness,
    map_git_quality, map_quality, map_tokens, worst_freshness, worst_git_quality,
};
use crate::{
    DesktopCalendarDate, DesktopCostValue, DesktopDashboardSectionKey,
    DesktopDashboardSectionState, DesktopFreshness, DesktopHistoryRange, DesktopQuality,
    DesktopSectionReasonCodes, DesktopTokenValue,
};

pub const MAX_PROJECT_ROWS: usize = 32;

const UNASSOCIATED_LABEL: &str = "Unassociated";

fn project_identity(identity: &UsageBreakdownIdentity) -> Option<(&str, bool)> {
    match identity {
        UsageBreakdownIdentity::Project(project) => Some((project.as_str(), false)),
        UsageBreakdownIdentity::UnassociatedProject => Some((UNASSOCIATED_LABEL, true)),
        UsageBreakdownIdentity::Model(_)
        | UsageBreakdownIdentity::Provider(_)
        | UsageBreakdownIdentity::Profile(_) => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProjectUsageRow {
    project: Arc<str>,
    unassociated: bool,
    event_count: u64,
    input: DesktopTokenValue,
    cached: DesktopTokenValue,
    output: DesktopTokenValue,
    reasoning: DesktopTokenValue,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
    code_available: bool,
    code_complete: bool,
    repository_count: u8,
    commits: Option<u64>,
    added_lines: Option<u64>,
    removed_lines: Option<u64>,
    net_lines: Option<i128>,
    code_freshness: Option<DesktopFreshness>,
    code_quality: Option<DesktopQuality>,
    cost_per_100_added_lines_micros: Option<u64>,
    efficiency_unavailable_reason: Option<&'static str>,
}

impl DesktopProjectUsageRow {
    #[must_use]
    pub fn project(&self) -> &str {
        &self.project
    }

    #[must_use]
    pub const fn unassociated(&self) -> bool {
        self.unassociated
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

    #[must_use]
    pub const fn code_available(&self) -> bool {
        self.code_available
    }

    #[must_use]
    pub const fn code_complete(&self) -> bool {
        self.code_complete
    }

    #[must_use]
    pub const fn repository_count(&self) -> u8 {
        self.repository_count
    }

    #[must_use]
    pub const fn commits(&self) -> Option<u64> {
        self.commits
    }

    #[must_use]
    pub const fn added_lines(&self) -> Option<u64> {
        self.added_lines
    }

    #[must_use]
    pub const fn removed_lines(&self) -> Option<u64> {
        self.removed_lines
    }

    #[must_use]
    pub const fn net_lines(&self) -> Option<i128> {
        self.net_lines
    }

    #[must_use]
    pub const fn code_freshness(&self) -> Option<DesktopFreshness> {
        self.code_freshness
    }

    #[must_use]
    pub const fn code_quality(&self) -> Option<DesktopQuality> {
        self.code_quality
    }

    #[must_use]
    pub const fn cost_per_100_added_lines_micros(&self) -> Option<u64> {
        self.cost_per_100_added_lines_micros
    }

    #[must_use]
    pub const fn efficiency_unavailable_reason(&self) -> Option<&'static str> {
        self.efficiency_unavailable_reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProjectsProjection {
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    usage_range_start: Option<DesktopCalendarDate>,
    usage_range_end: Option<DesktopCalendarDate>,
    usage_time_zone_id: Option<Arc<str>>,
    event_count: Option<u64>,
    total: DesktopTokenValue,
    cost: DesktopCostValue,
    usage_freshness: Option<DesktopFreshness>,
    usage_quality: Option<DesktopQuality>,
    code_range_start: Option<DesktopCalendarDate>,
    code_range_end: Option<DesktopCalendarDate>,
    code_time_zone_id: Option<Arc<str>>,
    loaded_repository_count: Option<u8>,
    code_freshness: Option<DesktopFreshness>,
    code_quality: Option<DesktopQuality>,
    code_complete: bool,
    rows: Arc<[DesktopProjectUsageRow]>,
    token_maximum: Option<u64>,
    usage_truncated: bool,
    code_truncated: bool,
}

impl DesktopProjectsProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let mut usage_section = base_section(snapshot.history());
        let mut code_section = base_section(snapshot.git());

        let history = snapshot.history().payload();
        let git = snapshot.git().payload();

        let (
            usage_range_start,
            usage_range_end,
            usage_time_zone_id,
            event_count,
            total,
            cost,
            usage_freshness,
            usage_quality,
            mut rows,
            usage_truncated,
        ) = if let Some(envelope) = history {
            let payload = envelope.payload();
            let metrics = payload.overview();
            add_evidence_state(
                &mut usage_section,
                envelope.header().freshness(),
                envelope.header().quality(),
                metrics.event_count() > 0,
            );
            match payload.overview_cost().availability() {
                CostAvailability::Partial => degrade(&mut usage_section, "cost_partial"),
                CostAvailability::Unavailable if metrics.event_count() > 0 => {
                    degrade(&mut usage_section, "cost_unavailable");
                }
                CostAvailability::Complete
                | CostAvailability::Zero
                | CostAvailability::Unavailable => {}
            }
            let breakdown = payload
                .breakdowns()
                .iter()
                .find(|breakdown| breakdown.kind() == UsageBreakdownKind::Project);
            let (rows, truncated) = match breakdown {
                None => {
                    degrade(&mut usage_section, "projects_breakdown_unavailable");
                    (Vec::new(), false)
                }
                Some(breakdown) => {
                    let rows = breakdown
                        .items()
                        .iter()
                        .filter_map(|item| {
                            let (project, unassociated) = project_identity(item.identity())?;
                            let metrics = item.metrics();
                            Some(DesktopProjectUsageRow {
                                project: Arc::from(project),
                                unassociated,
                                event_count: metrics.event_count(),
                                input: map_tokens(metrics.input(), metrics.event_count()),
                                cached: map_tokens(metrics.cached(), metrics.event_count()),
                                output: map_tokens(metrics.output(), metrics.event_count()),
                                reasoning: map_tokens(metrics.reasoning(), metrics.event_count()),
                                total: map_tokens(metrics.total(), metrics.event_count()),
                                cost: map_cost(item.cost()),
                                code_available: false,
                                code_complete: false,
                                repository_count: 0,
                                commits: None,
                                added_lines: None,
                                removed_lines: None,
                                net_lines: None,
                                code_freshness: None,
                                code_quality: None,
                                cost_per_100_added_lines_micros: None,
                                efficiency_unavailable_reason: Some(if unassociated {
                                    "unassociated_project"
                                } else {
                                    "git_unavailable"
                                }),
                            })
                        })
                        .take(MAX_PROJECT_ROWS)
                        .collect::<Vec<_>>();
                    let truncated =
                        breakdown.truncated() || breakdown.items().len() > MAX_PROJECT_ROWS;
                    if truncated {
                        degrade(&mut usage_section, "projects_truncated");
                    }
                    (rows, truncated)
                }
            };
            let range = payload.range();
            let start = range.start_date();
            let end = range.end_date();
            (
                Some((start.year(), start.month(), start.day())),
                Some((end.year(), end.month(), end.day())),
                Some(Arc::from(range.time_zone_id())),
                Some(metrics.event_count()),
                map_tokens(metrics.total(), metrics.event_count()),
                map_cost(payload.overview_cost()),
                Some(map_freshness(envelope.header().freshness())),
                Some(map_quality(envelope.header().quality())),
                rows,
                truncated,
            )
        } else {
            (
                None,
                None,
                None,
                None,
                DesktopTokenValue::UNAVAILABLE,
                DesktopCostValue::UNAVAILABLE,
                None,
                None,
                Vec::new(),
                false,
            )
        };

        let (
            code_range_start,
            code_range_end,
            code_time_zone_id,
            loaded_repository_count,
            code_freshness,
            code_quality,
            code_complete,
            code_truncated,
        ) = if let Some(envelope) = git {
            let payload = envelope.payload();
            let repositories = payload.repositories();
            let mut complete = !payload.has_more_repositories();
            let mut freshness = QueryFreshness::Fresh;
            let mut quality = GitOutputQuality::Complete;
            for repository in repositories.iter() {
                complete &= repository.range_complete()
                    && repository.quality() == GitOutputQuality::Complete
                    && repository.unavailable_reason().is_none()
                    && !repository.rebuild_required();
                freshness = worst_freshness(freshness, repository.freshness());
                quality = worst_git_quality(quality, repository.quality());
            }
            if !repositories.is_empty() {
                add_evidence_state(&mut code_section, freshness, map_git_quality(quality), true);
            }
            if payload.has_more_repositories() {
                degrade(&mut code_section, "git_repositories_truncated");
            }
            if !complete && !repositories.is_empty() {
                degrade(&mut code_section, "git_incomplete");
            }
            let range = payload.range();
            let start = range.start_date();
            let end = range.end_date();
            (
                Some((start.year(), start.month(), start.day())),
                Some((end.year(), end.month(), end.day())),
                Some(Arc::from(range.time_zone_id())),
                Some(
                    u8::try_from(repositories.len())
                        .map_or(u8::MAX, |repository_count| repository_count),
                ),
                (!repositories.is_empty()).then(|| map_freshness(freshness)),
                (!repositories.is_empty()).then(|| map_quality(map_git_quality(quality))),
                complete,
                payload.has_more_repositories(),
            )
        } else {
            (None, None, None, None, None, None, false, false)
        };

        let mut code_overflow = false;
        if let Some(envelope) = git {
            for row in &mut rows {
                let code = map_project_code(
                    (!row.unassociated).then_some(row.project.as_ref()),
                    envelope.payload(),
                );
                code_overflow |= code.overflow;
                row.code_available = code.available;
                row.code_complete = code.complete;
                row.repository_count = code.repository_count;
                row.commits = code.commits;
                row.added_lines = code.added_lines;
                row.removed_lines = code.removed_lines;
                row.net_lines = code.net_lines;
                row.code_freshness = code.freshness;
                row.code_quality = code.quality;
                row.cost_per_100_added_lines_micros = code.efficiency;
                row.efficiency_unavailable_reason = code.efficiency_reason;
            }
        }
        if code_overflow {
            degrade(&mut code_section, "overflow");
        }

        let section = combine_sections(
            DesktopDashboardSectionKey::CodeOutput,
            usage_section,
            code_section,
        );
        let token_maximum = rows.iter().filter_map(|row| row.total.known_sum()).max();

        Self {
            state: section.state(),
            reason_codes: section.reason_codes(),
            usage_range_start,
            usage_range_end,
            usage_time_zone_id,
            event_count,
            total,
            cost,
            usage_freshness,
            usage_quality,
            code_range_start,
            code_range_end,
            code_time_zone_id,
            loaded_repository_count,
            code_freshness,
            code_quality,
            code_complete,
            rows: Arc::from(rows),
            token_maximum,
            usage_truncated,
            code_truncated,
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
    pub const fn usage_range(&self) -> Option<DesktopHistoryRange> {
        match (self.usage_range_start, self.usage_range_end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }

    #[must_use]
    pub fn usage_time_zone_id(&self) -> Option<&str> {
        self.usage_time_zone_id.as_deref()
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
    pub const fn usage_freshness(&self) -> Option<DesktopFreshness> {
        self.usage_freshness
    }

    #[must_use]
    pub const fn usage_quality(&self) -> Option<DesktopQuality> {
        self.usage_quality
    }

    #[must_use]
    pub const fn code_range(&self) -> Option<DesktopHistoryRange> {
        match (self.code_range_start, self.code_range_end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }

    #[must_use]
    pub fn code_time_zone_id(&self) -> Option<&str> {
        self.code_time_zone_id.as_deref()
    }

    #[must_use]
    pub const fn loaded_repository_count(&self) -> Option<u8> {
        self.loaded_repository_count
    }

    #[must_use]
    pub const fn code_freshness(&self) -> Option<DesktopFreshness> {
        self.code_freshness
    }

    #[must_use]
    pub const fn code_quality(&self) -> Option<DesktopQuality> {
        self.code_quality
    }

    #[must_use]
    pub const fn code_complete(&self) -> bool {
        self.code_complete
    }

    #[must_use]
    pub const fn rows(&self) -> &Arc<[DesktopProjectUsageRow]> {
        &self.rows
    }

    #[must_use]
    pub const fn token_maximum(&self) -> Option<u64> {
        self.token_maximum
    }

    #[must_use]
    pub const fn usage_truncated(&self) -> bool {
        self.usage_truncated
    }

    #[must_use]
    pub const fn code_truncated(&self) -> bool {
        self.code_truncated
    }
}

struct ProjectCode {
    available: bool,
    complete: bool,
    repository_count: u8,
    commits: Option<u64>,
    added_lines: Option<u64>,
    removed_lines: Option<u64>,
    net_lines: Option<i128>,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    efficiency: Option<u64>,
    efficiency_reason: Option<&'static str>,
    overflow: bool,
}

#[derive(Default)]
struct CheckedCodeTotals {
    commits: u64,
    added: u64,
    removed: u64,
}

impl CheckedCodeTotals {
    fn add(&mut self, commits: u64, added: u64, removed: u64) -> Result<(), ()> {
        let next_commits = self.commits.checked_add(commits).ok_or(())?;
        let next_added = self.added.checked_add(added).ok_or(())?;
        let next_removed = self.removed.checked_add(removed).ok_or(())?;
        self.commits = next_commits;
        self.added = next_added;
        self.removed = next_removed;
        Ok(())
    }
}

#[derive(Default)]
struct ProjectEfficiencyAccumulator {
    identity: Option<DatasetIdentity>,
    cost: Option<u64>,
    added_lines: u64,
    reason: Option<&'static str>,
}

impl ProjectEfficiencyAccumulator {
    fn add_available(
        &mut self,
        identity: DatasetIdentity,
        cost: u64,
        added_lines: u64,
    ) -> Result<(), ()> {
        if self.reason.is_some() {
            return Ok(());
        }
        if self.identity.is_some_and(|current| current != identity)
            || self.cost.is_some_and(|current| current != cost)
        {
            self.reason = Some("efficiency_evidence_mismatch");
            return Ok(());
        }
        self.identity = Some(identity);
        self.cost = Some(cost);
        self.added_lines = self.added_lines.checked_add(added_lines).ok_or(())?;
        Ok(())
    }

    fn add_unavailable(&mut self, reason: &'static str) {
        self.reason.get_or_insert(reason);
    }

    fn finish(self) -> Result<(Option<u64>, Option<&'static str>), ()> {
        if let Some(reason) = self.reason {
            return Ok((None, Some(reason)));
        }
        let Some(cost) = self.cost else {
            return Ok((None, Some("zero_product_code_lines")));
        };
        if self.added_lines == 0 {
            return Ok((None, Some("zero_product_code_lines")));
        }
        let value = u128::from(cost)
            .checked_mul(100)
            .and_then(|value| value.checked_add(u128::from(self.added_lines / 2)))
            .map(|value| value / u128::from(self.added_lines))
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(())?;
        Ok((Some(value), None))
    }
}

impl ProjectCode {
    fn unavailable(reason: &'static str) -> Self {
        Self {
            available: false,
            complete: false,
            repository_count: 0,
            commits: None,
            added_lines: None,
            removed_lines: None,
            net_lines: None,
            freshness: None,
            quality: None,
            efficiency: None,
            efficiency_reason: Some(reason),
            overflow: reason == "overflow",
        }
    }
}

fn map_project_code(project: Option<&str>, snapshot: &GitOutputSnapshot) -> ProjectCode {
    let Some(project) = project else {
        return ProjectCode::unavailable("unassociated_project");
    };
    let repositories = snapshot
        .repositories()
        .iter()
        .filter(|repository| {
            repository
                .project_alias()
                .is_some_and(|alias| alias.as_str() == project)
        })
        .collect::<Vec<_>>();
    if repositories.is_empty() {
        return ProjectCode::unavailable("repository_not_linked");
    }

    let mut totals = CheckedCodeTotals::default();
    let mut complete = !snapshot.has_more_repositories();
    let mut freshness = QueryFreshness::Fresh;
    let mut quality = GitOutputQuality::Complete;
    let mut efficiency = ProjectEfficiencyAccumulator::default();

    for repository in &repositories {
        let repository_totals = repository.range_totals();
        if totals
            .add(
                repository_totals.commits(),
                repository_totals.lines().added(),
                repository_totals.lines().removed(),
            )
            .is_err()
        {
            return ProjectCode::unavailable("overflow");
        }
        complete &= repository.range_complete()
            && repository.quality() == GitOutputQuality::Complete
            && repository.unavailable_reason().is_none()
            && !repository.rebuild_required();
        freshness = worst_freshness(freshness, repository.freshness());
        quality = worst_git_quality(quality, repository.quality());

        let efficiency_result = match repository.efficiency() {
            GitEfficiency::Available(value) => efficiency.add_available(
                value.usage_dataset_identity(),
                value.usage_cost().get(),
                value.product_code_added_lines(),
            ),
            GitEfficiency::Unavailable(reason) => {
                efficiency.add_unavailable(reason.stable_code());
                Ok(())
            }
        };
        if efficiency_result.is_err() {
            return ProjectCode::unavailable("overflow");
        }
    }

    let Ok((efficiency, efficiency_reason)) = efficiency.finish() else {
        return ProjectCode::unavailable("overflow");
    };

    ProjectCode {
        available: true,
        complete,
        repository_count: u8::try_from(repositories.len()).map_or(u8::MAX, |value| value),
        commits: Some(totals.commits),
        added_lines: Some(totals.added),
        removed_lines: Some(totals.removed),
        net_lines: Some(i128::from(totals.added) - i128::from(totals.removed)),
        freshness: Some(map_freshness(freshness)),
        quality: Some(map_quality(map_git_quality(quality))),
        efficiency,
        efficiency_reason,
        overflow: false,
    }
}

#[cfg(test)]
mod tests {
    use tokenmaster_query::{DatasetIdentity, UsageBreakdownIdentity};

    use super::{CheckedCodeTotals, ProjectEfficiencyAccumulator, project_identity};

    #[test]
    fn mismatched_breakdown_identity_cannot_become_a_project_label() {
        let model = match tokenmaster_domain::ModelKey::new("gpt-5.6") {
            Ok(model) => model,
            Err(error) => panic!("valid test model rejected: {error}"),
        };
        assert_eq!(
            project_identity(&UsageBreakdownIdentity::Model(model)),
            None
        );
        assert_eq!(
            project_identity(&UsageBreakdownIdentity::UnassociatedProject),
            Some(("Unassociated", true))
        );
    }

    #[test]
    fn efficiency_rejects_mixed_dataset_identity() {
        let mut efficiency = ProjectEfficiencyAccumulator::default();
        assert_eq!(
            efficiency.add_available(DatasetIdentity::Empty, 10_000, 100),
            Ok(())
        );
        assert_eq!(
            efficiency.add_available(DatasetIdentity::LegacySnapshotV1, 10_000, 100),
            Ok(())
        );
        assert_eq!(
            efficiency.finish(),
            Ok((None, Some("efficiency_evidence_mismatch")))
        );
    }

    #[test]
    fn efficiency_rejects_mixed_usage_cost_for_one_dataset() {
        let mut efficiency = ProjectEfficiencyAccumulator::default();
        assert_eq!(
            efficiency.add_available(DatasetIdentity::Empty, 10_000, 100),
            Ok(())
        );
        assert_eq!(
            efficiency.add_available(DatasetIdentity::Empty, 20_000, 100),
            Ok(())
        );
        assert_eq!(
            efficiency.finish(),
            Ok((None, Some("efficiency_evidence_mismatch")))
        );
    }

    #[test]
    fn efficiency_rejects_line_and_ratio_overflow() {
        let mut line_overflow = ProjectEfficiencyAccumulator::default();
        assert_eq!(
            line_overflow.add_available(DatasetIdentity::Empty, 1, u64::MAX),
            Ok(())
        );
        assert_eq!(
            line_overflow.add_available(DatasetIdentity::Empty, 1, 1),
            Err(())
        );

        let mut ratio_overflow = ProjectEfficiencyAccumulator::default();
        assert_eq!(
            ratio_overflow.add_available(DatasetIdentity::Empty, u64::MAX, 1),
            Ok(())
        );
        assert_eq!(ratio_overflow.finish(), Err(()));
    }

    #[test]
    fn efficiency_marks_zero_product_code_lines_unavailable() {
        let mut efficiency = ProjectEfficiencyAccumulator::default();
        assert_eq!(
            efficiency.add_available(DatasetIdentity::Empty, 10_000, 0),
            Ok(())
        );
        assert_eq!(
            efficiency.finish(),
            Ok((None, Some("zero_product_code_lines")))
        );
    }

    #[test]
    fn code_totals_reject_each_overflow_without_partial_update() {
        let mut totals = CheckedCodeTotals {
            commits: u64::MAX,
            added: 10,
            removed: 20,
        };
        assert_eq!(totals.add(1, 1, 1), Err(()));
        assert_eq!(
            (totals.commits, totals.added, totals.removed),
            (u64::MAX, 10, 20)
        );

        let mut totals = CheckedCodeTotals {
            commits: 10,
            added: u64::MAX,
            removed: 20,
        };
        assert_eq!(totals.add(1, 1, 1), Err(()));
        assert_eq!(
            (totals.commits, totals.added, totals.removed),
            (10, u64::MAX, 20)
        );

        let mut totals = CheckedCodeTotals {
            commits: 10,
            added: 20,
            removed: u64::MAX,
        };
        assert_eq!(totals.add(1, 1, 1), Err(()));
        assert_eq!(
            (totals.commits, totals.added, totals.removed),
            (10, 20, u64::MAX)
        );
    }
}
