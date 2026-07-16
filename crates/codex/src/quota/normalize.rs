use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaRatio, QuotaResetEvidence, QuotaResetThresholds, QuotaSample,
    QuotaSampleParts, QuotaSampleQuality, QuotaScope, QuotaWindowDefinition,
    QuotaWindowDefinitionParts, QuotaWindowId, QuotaWindowKey, QuotaWindowSemantics,
    UsageProviderId,
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
    validate_reset_credits(quota.rate_limit_reset_credits.as_ref())?;
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
    Ok(CodexQuotaSnapshot::new(account_id, observations))
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

fn validate_reset_credits(
    summary: Option<&RateLimitResetCreditsSummaryWire>,
) -> Result<(), CodexQuotaError> {
    let Some(summary) = summary else {
        return Ok(());
    };
    let available_count = usize::try_from(summary.available_count).map_err(|_| invalid_data())?;
    if let Some(credits) = &summary.credits {
        if credits.len() > MAX_CODEX_RESET_CREDIT_DETAILS {
            return Err(CodexQuotaError::with_limit(
                CodexQuotaErrorCode::CapacityExceeded,
                MAX_CODEX_RESET_CREDIT_DETAILS,
            ));
        }
        if credits.len() > available_count {
            return Err(invalid_data());
        }
        for credit in credits {
            validate_reset_credit(credit)?;
        }
    }
    Ok(())
}

fn validate_reset_credit(credit: &RateLimitResetCreditWire) -> Result<(), CodexQuotaError> {
    validate_provider_text(&credit.id, MAX_CREDIT_ID_BYTES, false)?;
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
