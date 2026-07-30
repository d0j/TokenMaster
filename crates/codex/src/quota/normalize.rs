use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    QuotaConfidence, QuotaEvidenceSource, QuotaObservationId, QuotaPresentationDirection,
    QuotaRatio, QuotaResetEvidence, QuotaResetThresholds, QuotaSample, QuotaSampleParts,
    QuotaSampleQuality, QuotaScope, QuotaWindowDefinition, QuotaWindowDefinitionParts,
    QuotaWindowId, QuotaWindowKey, QuotaWindowSemantics, UsageProviderId,
};

use super::wire::{
    AccountResponseWire, AccountWire, RateLimitResetCreditWire, RateLimitResetCreditsSummaryWire,
    RateLimitSnapshotWire, RateLimitWindowWire, RateLimitsResponseWire,
};
use super::{
    CODEX_QUOTA_FRESH_MILLIS, CODEX_QUOTA_STALE_MILLIS, CodexQuotaError, CodexQuotaErrorCode,
    CodexQuotaObservation, CodexQuotaSnapshot, MAX_CODEX_QUOTA_JSON_BYTES, MAX_CODEX_QUOTA_WINDOWS,
    MAX_CODEX_RESET_CREDIT_DETAILS,
};

const ACCOUNT_DOMAIN: &[u8] = b"tokenmaster.codex.quota-account.v1";
const LIMIT_DOMAIN: &[u8] = b"tokenmaster.codex.quota-limit.v1";
const OBSERVATION_DOMAIN: &[u8] = b"tokenmaster.codex.quota-observation.v1";
const BENEFIT_LOT_DOMAIN: &[u8] = b"tokenmaster.codex.benefit-lot.v1";
const BENEFIT_AGGREGATE_DOMAIN: &[u8] = b"tokenmaster.codex.benefit-aggregate.v1";
const BENEFIT_OBSERVATION_DOMAIN: &[u8] = b"tokenmaster.codex.benefit-observation.v1";
const MAX_EMAIL_BYTES: usize = 320;
const MAX_PROVIDER_STRING_BYTES: usize = 512;
const MAX_SAFE_LIMIT_ID_BYTES: usize = 80;
const MAX_CREDIT_ID_BYTES: usize = 512;
const DEFINITION_REVISION: u64 = 1;

#[derive(Clone, Copy)]
enum WindowSlot {
    Primary,
    Secondary,
}

impl WindowSlot {
    const fn code(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }
}

pub(super) fn normalize_json(
    account_json: &[u8],
    quota_json: &[u8],
    observed_at_ms: i64,
) -> Result<CodexQuotaSnapshot, CodexQuotaError> {
    validate_json_size(account_json)?;
    validate_json_size(quota_json)?;
    let account: AccountResponseWire =
        serde_json::from_slice(account_json).map_err(|_| invalid_data())?;
    let quota: RateLimitsResponseWire =
        serde_json::from_slice(quota_json).map_err(|_| invalid_data())?;
    normalize_wire(account, quota, observed_at_ms)
}

pub(super) fn normalize_wire(
    account: AccountResponseWire,
    quota: RateLimitsResponseWire,
    observed_at_ms: i64,
) -> Result<CodexQuotaSnapshot, CodexQuotaError> {
    let fresh_until_ms = observed_at_ms
        .checked_add(CODEX_QUOTA_FRESH_MILLIS)
        .ok_or_else(invalid_time)?;
    let stale_after_ms = observed_at_ms
        .checked_add(CODEX_QUOTA_STALE_MILLIS)
        .ok_or_else(invalid_time)?;
    if observed_at_ms <= 0 {
        return Err(invalid_time());
    }

    let account_id = account_id(account)?;
    let benefit_observation = normalize_reset_credits(
        &account_id,
        quota.rate_limit_reset_credits,
        observed_at_ms,
        fresh_until_ms,
        stale_after_ms,
    )?;
    validate_snapshot_auxiliary(&quota.rate_limits)?;

    let mut observations = Vec::new();
    let mut window_ids = BTreeSet::new();
    match quota
        .rate_limits_by_limit_id
        .filter(|snapshots| !snapshots.is_empty())
    {
        Some(snapshots) => {
            for (map_limit_id, snapshot) in snapshots {
                validate_snapshot_auxiliary(&snapshot)?;
                if snapshot
                    .limit_id
                    .as_deref()
                    .is_some_and(|embedded| embedded != map_limit_id)
                {
                    return Err(invalid_data());
                }
                append_snapshot(
                    &account_id,
                    &map_limit_id,
                    snapshot,
                    observed_at_ms,
                    fresh_until_ms,
                    stale_after_ms,
                    &mut window_ids,
                    &mut observations,
                )?;
            }
        }
        None => {
            let legacy_snapshot = quota.rate_limits;
            let legacy_limit_id = legacy_snapshot
                .limit_id
                .clone()
                .unwrap_or_else(|| String::from("default"));
            append_snapshot(
                &account_id,
                &legacy_limit_id,
                legacy_snapshot,
                observed_at_ms,
                fresh_until_ms,
                stale_after_ms,
                &mut window_ids,
                &mut observations,
            )?;
        }
    }
    if observations.is_empty() {
        return Err(CodexQuotaError::new(CodexQuotaErrorCode::Unavailable));
    }
    Ok(CodexQuotaSnapshot::new(
        account_id,
        observations,
        benefit_observation,
    ))
}

fn validate_json_size(bytes: &[u8]) -> Result<(), CodexQuotaError> {
    if bytes.len() > MAX_CODEX_QUOTA_JSON_BYTES {
        return Err(CodexQuotaError::with_limit(
            CodexQuotaErrorCode::CapacityExceeded,
            MAX_CODEX_QUOTA_JSON_BYTES,
        ));
    }
    Ok(())
}

fn account_id(account: AccountResponseWire) -> Result<QuotaAccountId, CodexQuotaError> {
    let AccountResponseWire {
        requires_openai_auth,
        account,
    } = account;
    let _ = requires_openai_auth;
    let AccountWire {
        kind,
        email,
        plan_type,
        credential_source,
    } = account.ok_or_else(account_unavailable)?;
    if kind != "chatgpt" || plan_type.is_none() || credential_source.is_some() {
        return Err(account_unavailable());
    }
    let email = email.ok_or_else(account_unavailable)?;
    if email.len() > MAX_EMAIL_BYTES
        || email
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(account_unavailable());
    }
    let normalized = email.trim().to_lowercase();
    if normalized.is_empty() || normalized.len() > MAX_EMAIL_BYTES {
        return Err(account_unavailable());
    }
    let mut hasher = Sha256::new();
    update_field(&mut hasher, ACCOUNT_DOMAIN);
    update_field(&mut hasher, normalized.as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut value = String::with_capacity(69);
    value.push_str("acct_");
    push_hex(&mut value, &digest);
    QuotaAccountId::new(value).map_err(|_| invalid_data())
}

#[allow(clippy::too_many_arguments)]
fn append_snapshot(
    account_id: &QuotaAccountId,
    raw_limit_id: &str,
    snapshot: RateLimitSnapshotWire,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    window_ids: &mut BTreeSet<String>,
    observations: &mut Vec<CodexQuotaObservation>,
) -> Result<(), CodexQuotaError> {
    validate_provider_text(raw_limit_id, MAX_PROVIDER_STRING_BYTES, false)?;
    let stable_limit_id = stable_limit_id(raw_limit_id);
    let display_label = snapshot
        .limit_name
        .map(validate_display_label)
        .transpose()?;
    for (slot, window) in [
        (WindowSlot::Primary, snapshot.primary),
        (WindowSlot::Secondary, snapshot.secondary),
    ] {
        let Some(window) = window else {
            continue;
        };
        if observations.len() == MAX_CODEX_QUOTA_WINDOWS {
            return Err(CodexQuotaError::with_limit(
                CodexQuotaErrorCode::CapacityExceeded,
                MAX_CODEX_QUOTA_WINDOWS,
            ));
        }
        let observation = normalize_window(
            account_id,
            &stable_limit_id,
            slot,
            window,
            observed_at_ms,
            fresh_until_ms,
            stale_after_ms,
            display_label.clone(),
        )?;
        let window_id = observation
            .definition()
            .key()
            .window_id()
            .as_str()
            .to_owned();
        if !window_ids.insert(window_id) {
            return Err(invalid_data());
        }
        observations.push(observation);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn normalize_window(
    account_id: &QuotaAccountId,
    stable_limit_id: &str,
    slot: WindowSlot,
    window: RateLimitWindowWire,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    display_label: Option<Box<str>>,
) -> Result<CodexQuotaObservation, CodexQuotaError> {
    let used_percent = u32::try_from(window.used_percent).map_err(|_| invalid_data())?;
    if used_percent > 100 {
        return Err(invalid_data());
    }
    let used_ratio_ppm = used_percent.checked_mul(10_000).ok_or_else(invalid_data)?;
    let used_ratio = QuotaRatio::new(used_ratio_ppm).map_err(|_| invalid_data())?;
    let duration_seconds = window
        .window_duration_mins
        .map(|minutes| {
            let minutes = u64::try_from(minutes).map_err(|_| invalid_data())?;
            if minutes == 0 {
                return Err(invalid_data());
            }
            minutes.checked_mul(60).ok_or_else(invalid_data)
        })
        .transpose()?;
    let advertised_resets_at_ms = window
        .resets_at
        .map(|seconds| {
            if seconds <= 0 {
                return Err(invalid_data());
            }
            seconds.checked_mul(1_000).ok_or_else(invalid_data)
        })
        .transpose()?;
    let duration_id = window
        .window_duration_mins
        .map_or_else(|| String::from("unknown"), |minutes| minutes.to_string());
    let window_id_text = format!("{}.{}.{}", stable_limit_id, slot.code(), duration_id);
    let window_id = QuotaWindowId::new(window_id_text).map_err(|_| invalid_data())?;
    let scope = QuotaScope::new(
        UsageProviderId::new("codex").map_err(|_| invalid_data())?,
        account_id.clone(),
        None,
    );
    let key = QuotaWindowKey::new(scope, window_id);
    let thresholds = QuotaResetThresholds::new(
        Some(QuotaRatio::new(1_000_000).map_err(|_| invalid_data())?),
        None,
        Some(QuotaRatio::new(10_000).map_err(|_| invalid_data())?),
    )
    .map_err(|_| invalid_data())?;
    let definition = QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: key.clone(),
        revision: DEFINITION_REVISION,
        label_key: format!("quota.codex.{}.{}", stable_limit_id, slot.code()),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: duration_seconds,
        reset_thresholds: Some(thresholds),
    })
    .map_err(|_| invalid_data())?;
    let observation_id = observation_id(
        &key,
        observed_at_ms,
        fresh_until_ms,
        stale_after_ms,
        used_ratio,
        advertised_resets_at_ms,
        duration_seconds,
    );
    let sample = QuotaSample::new(QuotaSampleParts {
        key,
        observation_id,
        observed_at_ms,
        fresh_until_ms,
        stale_after_ms,
        provider_epoch_id: None,
        used_ratio: Some(used_ratio),
        remaining_ratio: None,
        units: None,
        advertised_resets_at_ms,
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderOfficial,
        confidence: QuotaConfidence::Medium,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    })
    .map_err(|_| invalid_data())?;
    Ok(CodexQuotaObservation::new(
        definition,
        sample,
        display_label,
    ))
}

fn validate_snapshot_auxiliary(snapshot: &RateLimitSnapshotWire) -> Result<(), CodexQuotaError> {
    if let Some(limit_id) = snapshot.limit_id.as_deref() {
        validate_provider_text(limit_id, MAX_PROVIDER_STRING_BYTES, false)?;
    }
    if let Some(limit_name) = snapshot.limit_name.as_deref() {
        validate_provider_text(limit_name, MAX_PROVIDER_STRING_BYTES, false)?;
    }
    let _ = (snapshot.plan_type, snapshot.rate_limit_reached_type);
    if let Some(credits) = &snapshot.credits {
        if let Some(balance) = credits.balance.as_deref() {
            validate_provider_text(balance, 128, true)?;
        }
        let _ = (credits.has_credits, credits.unlimited);
    }
    if let Some(limit) = &snapshot.individual_limit {
        validate_provider_text(&limit.limit, 128, false)?;
        validate_provider_text(&limit.used, 128, false)?;
        if !(0..=100).contains(&limit.remaining_percent) || limit.resets_at <= 0 {
            return Err(invalid_data());
        }
    }
    Ok(())
}

fn normalize_reset_credits(
    account_id: &QuotaAccountId,
    summary: Option<RateLimitResetCreditsSummaryWire>,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
) -> Result<Option<BenefitInventoryObservation>, CodexQuotaError> {
    let Some(summary) = summary else {
        return Ok(None);
    };
    let available_count = u64::try_from(summary.available_count).map_err(|_| invalid_data())?;
    if available_count > i64::MAX as u64 {
        return Err(invalid_data());
    }
    let credits = summary.credits.unwrap_or_default();
    if credits.len() > MAX_CODEX_RESET_CREDIT_DETAILS {
        return Err(CodexQuotaError::with_limit(
            CodexQuotaErrorCode::CapacityExceeded,
            MAX_CODEX_RESET_CREDIT_DETAILS,
        ));
    }
    let mut raw_ids = BTreeSet::new();
    let mut detailed_available_count = 0_u64;
    let mut lots = Vec::with_capacity(credits.len().saturating_add(1));
    for credit in credits {
        validate_reset_credit(&credit)?;
        if !raw_ids.insert(credit.id.clone()) {
            return Err(invalid_data());
        }
        if matches!(
            credit.status,
            super::wire::RateLimitResetCreditStatusWire::Available
        ) {
            detailed_available_count = detailed_available_count
                .checked_add(1)
                .ok_or_else(invalid_data)?;
        }
        lots.push(normalize_reset_credit(account_id, credit)?);
    }
    if detailed_available_count > available_count {
        return Err(invalid_data());
    }
    let aggregate_quantity = available_count - detailed_available_count;
    let completeness = if aggregate_quantity == 0 {
        BenefitInventoryCompleteness::Complete
    } else {
        if lots.len() == MAX_CODEX_RESET_CREDIT_DETAILS {
            return Err(CodexQuotaError::with_limit(
                CodexQuotaErrorCode::CapacityExceeded,
                MAX_CODEX_RESET_CREDIT_DETAILS,
            ));
        }
        lots.push(aggregate_reset_lot(account_id, aggregate_quantity)?);
        BenefitInventoryCompleteness::CompleteQuantityPartialDetails
    };
    lots.sort_unstable_by_key(|lot| *lot.lot_id().as_bytes());
    let scope = BenefitScope::new(
        UsageProviderId::new("codex").map_err(|_| invalid_data())?,
        account_id.clone(),
        None,
    );
    let observation_id = benefit_observation_id(
        &scope,
        observed_at_ms,
        fresh_until_ms,
        stale_after_ms,
        completeness,
        &lots,
    );
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id,
        observed_at_ms,
        fresh_until_ms,
        stale_after_ms,
        completeness,
        lots,
    })
    .map(Some)
    .map_err(|_| invalid_data())
}

fn normalize_reset_credit(
    account_id: &QuotaAccountId,
    credit: RateLimitResetCreditWire,
) -> Result<BenefitLotObservation, CodexQuotaError> {
    let lot_id = detailed_benefit_lot_id(account_id, &credit.id);
    let kind = match credit.reset_type {
        super::wire::RateLimitResetTypeWire::CodexRateLimits => BenefitKind::BankedRateLimitReset,
        super::wire::RateLimitResetTypeWire::Unknown => BenefitKind::Unknown,
    };
    let state = match credit.status {
        super::wire::RateLimitResetCreditStatusWire::Available => BenefitState::Available,
        super::wire::RateLimitResetCreditStatusWire::Redeeming => BenefitState::ActivationPending,
        super::wire::RateLimitResetCreditStatusWire::Redeemed => BenefitState::Activated,
        super::wire::RateLimitResetCreditStatusWire::Unknown => BenefitState::Ambiguous,
    };
    let granted_at_ms = credit
        .granted_at
        .checked_mul(1_000)
        .ok_or_else(invalid_data)?;
    let expiry = credit
        .expires_at
        .map(|expires_at| {
            expires_at
                .checked_mul(1_000)
                .ok_or_else(invalid_data)
                .and_then(|expires_at_ms| {
                    BenefitExpiry::exact_utc(expires_at_ms).map_err(|_| invalid_data())
                })
        })
        .transpose()?
        .unwrap_or_else(BenefitExpiry::unknown);
    let known = kind == BenefitKind::BankedRateLimitReset
        && !matches!(
            credit.status,
            super::wire::RateLimitResetCreditStatusWire::Unknown
        );
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id,
        kind,
        quantity: 1,
        state,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(granted_at_ms),
        expiry,
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: if known {
            BenefitConfidence::High
        } else {
            BenefitConfidence::Low
        },
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new(if kind == BenefitKind::BankedRateLimitReset {
            "benefit.codex.banked_reset"
        } else {
            "benefit.codex.unknown"
        })
        .map_err(|_| invalid_data())?,
    })
    .map_err(|_| invalid_data())
}

fn aggregate_reset_lot(
    account_id: &QuotaAccountId,
    quantity: u64,
) -> Result<BenefitLotObservation, CodexQuotaError> {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: aggregate_benefit_lot_id(account_id),
        kind: BenefitKind::BankedRateLimitReset,
        quantity,
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: None,
        expiry: BenefitExpiry::unknown(),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::Medium,
        detail_kind: BenefitDetailKind::ProviderAggregate,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset")
            .map_err(|_| invalid_data())?,
    })
    .map_err(|_| invalid_data())
}

fn detailed_benefit_lot_id(account_id: &QuotaAccountId, raw_id: &str) -> BenefitLotId {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, BENEFIT_LOT_DOMAIN);
    update_field(&mut hasher, account_id.as_str().as_bytes());
    update_field(&mut hasher, raw_id.as_bytes());
    BenefitLotId::from_bytes(hasher.finalize().into())
}

fn aggregate_benefit_lot_id(account_id: &QuotaAccountId) -> BenefitLotId {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, BENEFIT_AGGREGATE_DOMAIN);
    update_field(&mut hasher, account_id.as_str().as_bytes());
    update_field(&mut hasher, b"banked_rate_limit_reset");
    update_field(&mut hasher, b"unexplained_available");
    BenefitLotId::from_bytes(hasher.finalize().into())
}

fn benefit_observation_id(
    scope: &BenefitScope,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    completeness: BenefitInventoryCompleteness,
    lots: &[BenefitLotObservation],
) -> BenefitObservationId {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, BENEFIT_OBSERVATION_DOMAIN);
    update_field(&mut hasher, scope.provider_id().as_str().as_bytes());
    update_field(&mut hasher, scope.account_id().as_str().as_bytes());
    update_field(&mut hasher, &observed_at_ms.to_be_bytes());
    update_field(&mut hasher, &fresh_until_ms.to_be_bytes());
    update_field(&mut hasher, &stale_after_ms.to_be_bytes());
    update_field(&mut hasher, &[benefit_completeness_code(completeness)]);
    update_field(&mut hasher, &(lots.len() as u64).to_be_bytes());
    for lot in lots {
        update_field(&mut hasher, lot.lot_id().as_bytes());
        update_field(&mut hasher, &[benefit_kind_code(lot.kind())]);
        update_field(&mut hasher, &lot.quantity().to_be_bytes());
        update_field(&mut hasher, &[benefit_state_code(lot.state())]);
        update_optional_i64(&mut hasher, lot.granted_at_ms());
        update_benefit_expiry(&mut hasher, lot.expiry());
        update_field(&mut hasher, &[benefit_confidence_code(lot.confidence())]);
        update_field(&mut hasher, &[benefit_detail_code(lot.detail_kind())]);
        update_field(&mut hasher, lot.label_key().as_bytes());
    }
    BenefitObservationId::from_bytes(hasher.finalize().into())
}

fn validate_reset_credit(credit: &RateLimitResetCreditWire) -> Result<(), CodexQuotaError> {
    validate_provider_text(&credit.id, MAX_CREDIT_ID_BYTES, false)?;
    if credit
        .id
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(invalid_data());
    }
    if credit.granted_at <= 0
        || credit
            .expires_at
            .is_some_and(|expires| expires <= 0 || expires < credit.granted_at)
    {
        return Err(invalid_data());
    }
    if let Some(title) = credit.title.as_deref() {
        validate_provider_text(title, MAX_PROVIDER_STRING_BYTES, true)?;
    }
    if let Some(description) = credit.description.as_deref() {
        validate_provider_text(description, MAX_PROVIDER_STRING_BYTES, true)?;
    }
    let _ = (credit.reset_type, credit.status);
    Ok(())
}

fn update_benefit_expiry(hasher: &mut Sha256, expiry: &BenefitExpiry) {
    match expiry {
        BenefitExpiry::ExactUtc { at_ms } => {
            update_field(hasher, &[1]);
            update_field(hasher, &at_ms.to_be_bytes());
        }
        BenefitExpiry::ProviderLocal { local, time_zone } => {
            update_field(hasher, &[2]);
            update_field(hasher, &local.date().year().to_be_bytes());
            update_field(hasher, &[local.date().month(), local.date().day()]);
            update_field(
                hasher,
                &[
                    local.time().hour(),
                    local.time().minute(),
                    local.time().second(),
                ],
            );
            update_field(hasher, &local.time().millisecond().to_be_bytes());
            update_field(hasher, time_zone.as_str().as_bytes());
        }
        BenefitExpiry::ProviderDate { date, time_zone } => {
            update_field(hasher, &[3]);
            update_field(hasher, &date.year().to_be_bytes());
            update_field(hasher, &[date.month(), date.day()]);
            match time_zone {
                Some(time_zone) => {
                    update_field(hasher, &[1]);
                    update_field(hasher, time_zone.as_str().as_bytes());
                }
                None => update_field(hasher, &[0]),
            }
        }
        BenefitExpiry::BoundedUtc {
            earliest_at_ms,
            latest_at_ms,
        } => {
            update_field(hasher, &[4]);
            update_field(hasher, &earliest_at_ms.to_be_bytes());
            update_field(hasher, &latest_at_ms.to_be_bytes());
        }
        BenefitExpiry::Unknown => update_field(hasher, &[5]),
    }
}

const fn benefit_completeness_code(value: BenefitInventoryCompleteness) -> u8 {
    match value {
        BenefitInventoryCompleteness::Complete => 1,
        BenefitInventoryCompleteness::CompleteQuantityPartialDetails => 2,
        BenefitInventoryCompleteness::Partial => 3,
    }
}

const fn benefit_kind_code(value: BenefitKind) -> u8 {
    match value {
        BenefitKind::BankedRateLimitReset => 1,
        BenefitKind::UsageCredit => 2,
        BenefitKind::TemporaryUsage => 3,
        BenefitKind::Unknown => 4,
    }
}

const fn benefit_state_code(value: BenefitState) -> u8 {
    match value {
        BenefitState::Available => 1,
        BenefitState::ActivationPending => 2,
        BenefitState::Activated => 3,
        BenefitState::Expired => 4,
        BenefitState::Revoked => 5,
        BenefitState::Ambiguous => 6,
    }
}

const fn benefit_confidence_code(value: BenefitConfidence) -> u8 {
    match value {
        BenefitConfidence::High => 1,
        BenefitConfidence::Medium => 2,
        BenefitConfidence::Low => 3,
        BenefitConfidence::Unknown => 4,
    }
}

const fn benefit_detail_code(value: BenefitDetailKind) -> u8 {
    match value {
        BenefitDetailKind::ProviderDetail => 1,
        BenefitDetailKind::ProviderAggregate => 2,
        BenefitDetailKind::Manual => 3,
    }
}

fn validate_display_label(label: String) -> Result<Box<str>, CodexQuotaError> {
    validate_provider_text(&label, MAX_PROVIDER_STRING_BYTES, false)?;
    Ok(label.into_boxed_str())
}

fn validate_provider_text(
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<(), CodexQuotaError> {
    if (!allow_empty && value.is_empty())
        || value.len() > max_bytes
        || contains_unsupported_text(value)
    {
        return Err(invalid_data());
    }
    Ok(())
}

fn contains_unsupported_text(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn stable_limit_id(raw_limit_id: &str) -> String {
    if raw_limit_id.len() <= MAX_SAFE_LIMIT_ID_BYTES
        && raw_limit_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return raw_limit_id.to_owned();
    }
    let mut hasher = Sha256::new();
    update_field(&mut hasher, LIMIT_DOMAIN);
    update_field(&mut hasher, raw_limit_id.as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut value = String::with_capacity(70);
    value.push_str("limit_");
    push_hex(&mut value, &digest);
    value
}

fn observation_id(
    key: &QuotaWindowKey,
    observed_at_ms: i64,
    fresh_until_ms: i64,
    stale_after_ms: i64,
    used_ratio: QuotaRatio,
    advertised_resets_at_ms: Option<i64>,
    duration_seconds: Option<u64>,
) -> QuotaObservationId {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, OBSERVATION_DOMAIN);
    update_field(&mut hasher, key.scope().provider_id().as_str().as_bytes());
    update_field(&mut hasher, key.scope().account_id().as_str().as_bytes());
    update_field(&mut hasher, key.window_id().as_str().as_bytes());
    update_field(&mut hasher, &DEFINITION_REVISION.to_be_bytes());
    update_field(&mut hasher, &observed_at_ms.to_be_bytes());
    update_field(&mut hasher, &fresh_until_ms.to_be_bytes());
    update_field(&mut hasher, &stale_after_ms.to_be_bytes());
    update_field(&mut hasher, &used_ratio.parts_per_million().to_be_bytes());
    update_optional_i64(&mut hasher, advertised_resets_at_ms);
    update_optional_u64(&mut hasher, duration_seconds);
    update_field(&mut hasher, &[1]);
    update_field(&mut hasher, &[1]);
    update_field(&mut hasher, &[1]);
    QuotaObservationId::from_bytes(hasher.finalize().into())
}

fn update_optional_i64(hasher: &mut Sha256, value: Option<i64>) {
    match value {
        Some(value) => {
            update_field(hasher, &[1]);
            update_field(hasher, &value.to_be_bytes());
        }
        None => update_field(hasher, &[0]),
    }
}

fn update_optional_u64(hasher: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            update_field(hasher, &[1]);
            update_field(hasher, &value.to_be_bytes());
        }
        None => update_field(hasher, &[0]),
    }
}

fn update_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update((field.len() as u64).to_be_bytes());
    hasher.update(field);
}

fn push_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

const fn invalid_data() -> CodexQuotaError {
    CodexQuotaError::new(CodexQuotaErrorCode::InvalidData)
}

const fn invalid_time() -> CodexQuotaError {
    CodexQuotaError::new(CodexQuotaErrorCode::InvalidTime)
}

const fn account_unavailable() -> CodexQuotaError {
    CodexQuotaError::new(CodexQuotaErrorCode::AccountIdentityUnavailable)
}
