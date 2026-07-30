use std::sync::Arc;

use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitKind, BenefitState,
};
use tokenmaster_product::ProductSnapshot;
use tokenmaster_query::{
    BenefitReminderCoverage, BenefitReminderProfileSource, BenefitWarningCode,
};

use crate::dashboard::{add_evidence_state, base_section, degrade, map_freshness, map_quality};
use crate::{
    DesktopDashboardSectionState, DesktopFreshness, DesktopQuality, DesktopSectionReasonCodes,
};

pub const MAX_NOTIFICATION_SCOPES: usize = 32;
pub const MAX_NOTIFICATION_LOTS: usize = 256;
pub const MAX_NOTIFICATION_LEADS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopBenefitExpiry {
    ExactUtc {
        at_ms: i64,
    },
    BoundedUtc {
        earliest_at_ms: i64,
        latest_at_ms: i64,
    },
    ProviderLocal {
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        millisecond: u16,
        time_zone: Arc<str>,
    },
    ProviderDate {
        year: i32,
        month: u8,
        day: u8,
        time_zone: Option<Arc<str>>,
    },
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopReminderScopeRow {
    ordinal: u8,
    current_lot_count: u16,
    inventory_revision: u64,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    completeness: &'static str,
    nearest_expiry_at_ms: Option<i64>,
    nearest_due_at_ms: Option<i64>,
    profile_revision: u64,
    profile_source: &'static str,
    reminder_coverage: &'static str,
    lead_seconds: Arc<[u32]>,
    freshness: DesktopFreshness,
    quality: DesktopQuality,
    warning_codes: DesktopSectionReasonCodes,
}

impl DesktopReminderScopeRow {
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    #[must_use]
    pub const fn current_lot_count(&self) -> u16 {
        self.current_lot_count
    }

    #[must_use]
    pub const fn inventory_revision(&self) -> u64 {
        self.inventory_revision
    }

    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    #[must_use]
    pub const fn fresh_until_ms(&self) -> i64 {
        self.fresh_until_ms
    }

    #[must_use]
    pub const fn stale_after_ms(&self) -> i64 {
        self.stale_after_ms
    }

    #[must_use]
    pub const fn completeness(&self) -> &'static str {
        self.completeness
    }

    #[must_use]
    pub const fn nearest_expiry_at_ms(&self) -> Option<i64> {
        self.nearest_expiry_at_ms
    }

    #[must_use]
    pub const fn nearest_due_at_ms(&self) -> Option<i64> {
        self.nearest_due_at_ms
    }

    #[must_use]
    pub const fn profile_revision(&self) -> u64 {
        self.profile_revision
    }

    #[must_use]
    pub const fn profile_source(&self) -> &'static str {
        self.profile_source
    }

    #[must_use]
    pub const fn reminder_coverage(&self) -> &'static str {
        self.reminder_coverage
    }

    #[must_use]
    pub fn lead_seconds(&self) -> &[u32] {
        &self.lead_seconds
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
    pub const fn warning_codes(&self) -> DesktopSectionReasonCodes {
        self.warning_codes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopBenefitLotRow {
    scope_ordinal: u8,
    kind: &'static str,
    quantity: u64,
    state: &'static str,
    label_key: Arc<str>,
    granted_at_ms: Option<i64>,
    expiry: DesktopBenefitExpiry,
    evidence_source: &'static str,
    confidence: &'static str,
    detail_kind: &'static str,
}

impl DesktopBenefitLotRow {
    #[must_use]
    pub const fn scope_ordinal(&self) -> u8 {
        self.scope_ordinal
    }

    #[must_use]
    pub const fn kind(&self) -> &'static str {
        self.kind
    }

    #[must_use]
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }

    #[must_use]
    pub const fn state(&self) -> &'static str {
        self.state
    }

    #[must_use]
    pub fn label_key(&self) -> &str {
        &self.label_key
    }

    #[must_use]
    pub const fn granted_at_ms(&self) -> Option<i64> {
        self.granted_at_ms
    }

    #[must_use]
    pub const fn expiry(&self) -> &DesktopBenefitExpiry {
        &self.expiry
    }

    #[must_use]
    pub const fn evidence_source(&self) -> &'static str {
        self.evidence_source
    }

    #[must_use]
    pub const fn confidence(&self) -> &'static str {
        self.confidence
    }

    #[must_use]
    pub const fn detail_kind(&self) -> &'static str {
        self.detail_kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopNotificationsProjection {
    state: DesktopDashboardSectionState,
    reason_codes: DesktopSectionReasonCodes,
    freshness: Option<DesktopFreshness>,
    quality: Option<DesktopQuality>,
    scopes: Arc<[DesktopReminderScopeRow]>,
    lots: Arc<[DesktopBenefitLotRow]>,
    scopes_truncated: bool,
    lots_truncated: bool,
}

impl DesktopNotificationsProjection {
    #[must_use]
    pub fn from_snapshot(snapshot: &ProductSnapshot) -> Self {
        let mut section = base_section(snapshot.benefit());
        let Some(envelope) = snapshot.benefit().payload() else {
            return Self::unavailable(section.state(), section.reason_codes());
        };
        add_evidence_state(
            &mut section,
            envelope.header().freshness(),
            envelope.header().quality(),
            true,
        );
        for warning in envelope.header().warnings().iter() {
            degrade(&mut section, warning.stable_code());
        }

        let scopes_truncated = envelope.payload().scopes().len() > MAX_NOTIFICATION_SCOPES;
        let mut lots = Vec::new();
        let mut scope_rows = Vec::new();
        let mut lots_truncated = false;
        for (scope_index, scope) in envelope
            .payload()
            .scopes()
            .iter()
            .take(MAX_NOTIFICATION_SCOPES)
            .enumerate()
        {
            let ordinal = ordinal(scope_index);
            let leads_truncated =
                scope.reminder_profile().lead_times().len() > MAX_NOTIFICATION_LEADS;
            if leads_truncated {
                degrade(&mut section, "notification_leads_truncated");
            }
            let warning_codes = map_warning_codes(scope.warnings());
            for warning in warning_codes.iter() {
                degrade(&mut section, warning);
            }
            scope_rows.push(DesktopReminderScopeRow {
                ordinal,
                current_lot_count: u16::try_from(scope.current_lots().len()).unwrap_or(u16::MAX),
                inventory_revision: scope.inventory_revision(),
                observed_at_ms: scope.observed_at_ms(),
                fresh_until_ms: scope.fresh_until_ms(),
                stale_after_ms: scope.stale_after_ms(),
                completeness: map_completeness(scope.completeness()),
                nearest_expiry_at_ms: scope.nearest_expiry_at_ms(),
                nearest_due_at_ms: scope.nearest_due_at_ms(),
                profile_revision: scope.reminder_profile().revision(),
                profile_source: map_profile_source(scope.reminder_profile().source()),
                reminder_coverage: map_coverage(scope.reminder_profile().coverage()),
                lead_seconds: Arc::from(
                    scope
                        .reminder_profile()
                        .lead_times()
                        .iter()
                        .take(MAX_NOTIFICATION_LEADS)
                        .map(|lead| lead.seconds())
                        .collect::<Vec<_>>(),
                ),
                freshness: map_freshness(scope.freshness()),
                quality: map_quality(scope.quality()),
                warning_codes,
            });
            for lot in scope.current_lots().iter() {
                if lots.len() == MAX_NOTIFICATION_LOTS {
                    lots_truncated = true;
                    break;
                }
                lots.push(DesktopBenefitLotRow {
                    scope_ordinal: ordinal,
                    kind: map_kind(lot.kind()),
                    quantity: lot.quantity(),
                    state: map_state(lot.state()),
                    label_key: Arc::from(lot.label_key()),
                    granted_at_ms: lot.granted_at_ms(),
                    expiry: map_expiry(lot.expiry()),
                    evidence_source: map_evidence_source(lot.source()),
                    confidence: map_confidence(lot.confidence()),
                    detail_kind: map_detail_kind(lot.detail_kind()),
                });
            }
            if lots_truncated {
                break;
            }
        }
        if scopes_truncated {
            degrade(&mut section, "notification_scopes_truncated");
        }
        if lots_truncated {
            degrade(&mut section, "notification_lots_truncated");
        }
        Self {
            state: section.state(),
            reason_codes: section.reason_codes(),
            freshness: Some(map_freshness(envelope.header().freshness())),
            quality: Some(map_quality(envelope.header().quality())),
            scopes: Arc::from(scope_rows),
            lots: Arc::from(lots),
            scopes_truncated,
            lots_truncated,
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
            scopes: Arc::from(Vec::new()),
            lots: Arc::from(Vec::new()),
            scopes_truncated: false,
            lots_truncated: false,
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
    pub const fn scopes(&self) -> &Arc<[DesktopReminderScopeRow]> {
        &self.scopes
    }

    #[must_use]
    pub const fn lots(&self) -> &Arc<[DesktopBenefitLotRow]> {
        &self.lots
    }

    #[must_use]
    pub const fn scopes_truncated(&self) -> bool {
        self.scopes_truncated
    }

    #[must_use]
    pub const fn lots_truncated(&self) -> bool {
        self.lots_truncated
    }
}

fn map_expiry(value: &BenefitExpiry) -> DesktopBenefitExpiry {
    match value {
        BenefitExpiry::ExactUtc { at_ms } => DesktopBenefitExpiry::ExactUtc { at_ms: *at_ms },
        BenefitExpiry::BoundedUtc {
            earliest_at_ms,
            latest_at_ms,
        } => DesktopBenefitExpiry::BoundedUtc {
            earliest_at_ms: *earliest_at_ms,
            latest_at_ms: *latest_at_ms,
        },
        BenefitExpiry::ProviderLocal { local, time_zone } => {
            let date = local.date();
            let time = local.time();
            DesktopBenefitExpiry::ProviderLocal {
                year: date.year(),
                month: date.month(),
                day: date.day(),
                hour: time.hour(),
                minute: time.minute(),
                second: time.second(),
                millisecond: time.millisecond(),
                time_zone: Arc::from(time_zone.as_str()),
            }
        }
        BenefitExpiry::ProviderDate { date, time_zone } => DesktopBenefitExpiry::ProviderDate {
            year: date.year(),
            month: date.month(),
            day: date.day(),
            time_zone: time_zone.as_ref().map(|value| Arc::from(value.as_str())),
        },
        BenefitExpiry::Unknown => DesktopBenefitExpiry::Unknown,
    }
}

fn map_warning_codes(values: &[BenefitWarningCode]) -> DesktopSectionReasonCodes {
    let mut reasons = DesktopSectionReasonCodes::empty();
    for value in values {
        reasons.push(value.stable_code());
    }
    reasons
}

const fn ordinal(index: usize) -> u8 {
    let ordinal = index.saturating_add(1);
    if ordinal > u8::MAX as usize {
        u8::MAX
    } else {
        ordinal as u8
    }
}

const fn map_completeness(value: BenefitInventoryCompleteness) -> &'static str {
    match value {
        BenefitInventoryCompleteness::Complete => "complete",
        BenefitInventoryCompleteness::CompleteQuantityPartialDetails => {
            "complete_quantity_partial_details"
        }
        BenefitInventoryCompleteness::Partial => "partial",
    }
}

const fn map_profile_source(value: BenefitReminderProfileSource) -> &'static str {
    match value {
        BenefitReminderProfileSource::Inherited => "inherited",
        BenefitReminderProfileSource::Override => "override",
    }
}

const fn map_coverage(value: BenefitReminderCoverage) -> &'static str {
    match value {
        BenefitReminderCoverage::Disabled => "disabled",
        BenefitReminderCoverage::InAppOnly => "in_app_only",
    }
}

const fn map_kind(value: BenefitKind) -> &'static str {
    match value {
        BenefitKind::BankedRateLimitReset => "banked_rate_limit_reset",
        BenefitKind::UsageCredit => "usage_credit",
        BenefitKind::TemporaryUsage => "temporary_usage",
        BenefitKind::Unknown => "unknown",
    }
}

const fn map_state(value: BenefitState) -> &'static str {
    match value {
        BenefitState::Available => "available",
        BenefitState::ActivationPending => "activation_pending",
        BenefitState::Activated => "activated",
        BenefitState::Expired => "expired",
        BenefitState::Revoked => "revoked",
        BenefitState::Ambiguous => "ambiguous",
    }
}

const fn map_evidence_source(value: BenefitEvidenceSource) -> &'static str {
    match value {
        BenefitEvidenceSource::ProviderOfficial => "provider_official",
        BenefitEvidenceSource::ProviderLocal => "provider_local",
        BenefitEvidenceSource::Manual => "manual",
        BenefitEvidenceSource::Unknown => "unknown",
    }
}

const fn map_confidence(value: BenefitConfidence) -> &'static str {
    match value {
        BenefitConfidence::High => "high",
        BenefitConfidence::Medium => "medium",
        BenefitConfidence::Low => "low",
        BenefitConfidence::Unknown => "unknown",
    }
}

const fn map_detail_kind(value: BenefitDetailKind) -> &'static str {
    match value {
        BenefitDetailKind::ProviderDetail => "provider_detail",
        BenefitDetailKind::ProviderAggregate => "provider_aggregate",
        BenefitDetailKind::Manual => "manual",
    }
}
