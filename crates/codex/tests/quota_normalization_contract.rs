use serde_json::{Value, json};
use tokenmaster_codex::{
    CODEX_QUOTA_FRESH_MILLIS, CODEX_QUOTA_STALE_MILLIS, CodexQuotaErrorCode, CodexQuotaNormalizer,
    MAX_CODEX_QUOTA_JSON_BYTES,
};
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitExpiry, BenefitInventoryCompleteness, BenefitKind,
    BenefitState, BenefitTarget, QuotaConfidence, QuotaEvidenceSource, QuotaSampleQuality,
    QuotaWindowSemantics,
};

const OBSERVED_AT_MS: i64 = 1_700_000_000_000;
const PRIVATE_EMAIL: &str = "Private@Example.com";
const PRIVATE_CREDIT_ID: &str = "credit_private_76e5";
const PRIVATE_HOME: &str = r"C:\private\codex-home";

fn account(email: Option<&str>) -> Value {
    json!({
        "requiresOpenaiAuth": true,
        "account": {
            "type": "chatgpt",
            "email": email,
            "planType": "pro"
        }
    })
}

fn window(used_percent: i64, resets_at: i64, duration_minutes: i64) -> Value {
    json!({
        "usedPercent": used_percent,
        "resetsAt": resets_at,
        "windowDurationMins": duration_minutes
    })
}

fn snapshot(
    limit_id: &str,
    limit_name: Option<&str>,
    primary: Option<Value>,
    secondary: Option<Value>,
) -> Value {
    json!({
        "credits": null,
        "individualLimit": null,
        "limitId": limit_id,
        "limitName": limit_name,
        "planType": "pro",
        "primary": primary,
        "rateLimitReachedType": null,
        "secondary": secondary
    })
}

fn reset_credits() -> Value {
    json!({
        "availableCount": 1,
        "credits": [{
            "description": "private fixture description",
            "expiresAt": 1_700_200_000,
            "grantedAt": 1_699_000_000,
            "id": PRIVATE_CREDIT_ID,
            "resetType": "codexRateLimits",
            "status": "available",
            "title": "private fixture title"
        }]
    })
}

fn current_response() -> Value {
    let default = snapshot("codex", None, Some(window(42, 1_700_100_000, 10_080)), None);
    let model = snapshot(
        "codex_bengalfox",
        Some("GPT-5.3-Codex-Spark"),
        Some(window(7, 1_700_100_000, 10_080)),
        None,
    );
    json!({
        "rateLimitResetCredits": reset_credits(),
        "rateLimits": default,
        "rateLimitsByLimitId": {
            "codex": default,
            "codex_bengalfox": model
        }
    })
}

fn normalize(
    account: &Value,
    response: &Value,
    observed_at_ms: i64,
) -> tokenmaster_codex::CodexQuotaSnapshot {
    let account = serde_json::to_vec(account).expect("account fixture");
    let response = serde_json::to_vec(response).expect("quota fixture");
    CodexQuotaNormalizer::normalize(&account, &response, observed_at_ms)
        .expect("fixture must normalize")
}

#[test]
fn current_multi_bucket_response_normalizes_without_legacy_duplication() {
    let snapshot = normalize(
        &account(Some(PRIVATE_EMAIL)),
        &current_response(),
        OBSERVED_AT_MS,
    );

    assert_eq!(
        snapshot.account_id().as_str(),
        "acct_45771b654d2155290e57022a8faed51f9ae246e50e8a871a6b51c4ee07ec4501"
    );
    assert_eq!(snapshot.observations().len(), 2);
    let benefits = snapshot
        .benefit_observation()
        .expect("reset-credit inventory");
    assert_eq!(
        benefits.completeness(),
        BenefitInventoryCompleteness::Complete
    );
    assert_eq!(benefits.lots().len(), 1);
    let reset = &benefits.lots()[0];
    assert_eq!(reset.kind(), BenefitKind::BankedRateLimitReset);
    assert_eq!(reset.quantity(), 1);
    assert_eq!(reset.state(), BenefitState::Available);
    assert_eq!(reset.target(), &BenefitTarget::Provider);
    assert_eq!(reset.granted_at_ms(), Some(1_699_000_000_000));
    assert_eq!(
        reset.expiry(),
        &BenefitExpiry::exact_utc(1_700_200_000_000).expect("expiry")
    );
    assert_eq!(reset.confidence(), BenefitConfidence::High);
    assert_eq!(reset.detail_kind(), BenefitDetailKind::ProviderDetail);
    assert_eq!(reset.label_key(), "benefit.codex.banked_reset");

    let default = &snapshot.observations()[0];
    assert_eq!(
        default.definition().key().window_id().as_str(),
        "codex.primary.10080"
    );
    assert_eq!(
        default.definition().label_key(),
        "quota.codex.codex.primary"
    );
    assert_eq!(default.definition().revision(), 1);
    assert_eq!(
        default.definition().semantics(),
        QuotaWindowSemantics::Fixed
    );
    assert_eq!(
        default.definition().nominal_duration_seconds(),
        Some(604_800)
    );
    let thresholds = default
        .definition()
        .reset_thresholds()
        .expect("Codex fixed window thresholds");
    assert_eq!(
        thresholds
            .maximum_post_reset_used_ratio()
            .expect("maximum used ratio")
            .parts_per_million(),
        1_000_000
    );
    assert_eq!(
        thresholds
            .minimum_used_ratio_drop()
            .expect("minimum drop")
            .parts_per_million(),
        10_000
    );

    let sample = default.sample();
    assert_eq!(sample.observed_at_ms(), OBSERVED_AT_MS);
    assert_eq!(
        sample.fresh_until_ms(),
        OBSERVED_AT_MS + CODEX_QUOTA_FRESH_MILLIS
    );
    assert_eq!(
        sample.stale_after_ms(),
        OBSERVED_AT_MS + CODEX_QUOTA_STALE_MILLIS
    );
    assert_eq!(
        sample.used_ratio().expect("used ratio").parts_per_million(),
        420_000
    );
    assert_eq!(sample.remaining_ratio(), None);
    assert_eq!(sample.units(), None);
    assert_eq!(sample.advertised_resets_at_ms(), Some(1_700_100_000_000));
    assert_eq!(sample.quality(), QuotaSampleQuality::Authoritative);
    assert_eq!(sample.source(), QuotaEvidenceSource::ProviderOfficial);
    assert_eq!(sample.confidence(), QuotaConfidence::Medium);

    let model = &snapshot.observations()[1];
    assert_eq!(
        model.definition().key().window_id().as_str(),
        "codex_bengalfox.primary.10080"
    );
    assert_eq!(model.display_label(), Some("GPT-5.3-Codex-Spark"));
    assert_ne!(
        default.sample().observation_id(),
        model.sample().observation_id()
    );

    let duplicate = normalize(
        &account(Some(PRIVATE_EMAIL)),
        &current_response(),
        OBSERVED_AT_MS,
    );
    assert_eq!(
        snapshot.observations()[0].sample().observation_id(),
        duplicate.observations()[0].sample().observation_id()
    );
    assert_eq!(
        benefits.observation_id(),
        duplicate
            .benefit_observation()
            .expect("duplicate benefits")
            .observation_id()
    );
    let later = normalize(
        &account(Some(PRIVATE_EMAIL)),
        &current_response(),
        OBSERVED_AT_MS + 1,
    );
    assert_ne!(
        snapshot.observations()[0].sample().observation_id(),
        later.observations()[0].sample().observation_id()
    );
    assert_ne!(
        benefits.observation_id(),
        later
            .benefit_observation()
            .expect("later benefits")
            .observation_id()
    );
}

#[test]
fn legacy_only_snapshot_expands_primary_and_secondary_in_stable_order() {
    let response = json!({
        "rateLimitResetCredits": null,
        "rateLimits": snapshot(
            "codex",
            Some("Codex"),
            Some(window(25, 1_700_100_000, 300)),
            Some(window(50, 1_700_200_000, 10_080))
        ),
        "rateLimitsByLimitId": null
    });

    let normalized = normalize(&account(Some(PRIVATE_EMAIL)), &response, OBSERVED_AT_MS);
    let ids = normalized
        .observations()
        .iter()
        .map(|observation| observation.definition().key().window_id().as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["codex.primary.300", "codex.secondary.10080"]);
    assert!(
        normalized
            .observations()
            .iter()
            .all(|observation| observation.display_label() == Some("Codex"))
    );
    assert_eq!(normalized.benefit_observation(), None);
}

#[test]
fn reset_credit_detail_and_aggregate_gap_remain_distinct_and_account_scoped() {
    let mut response = current_response();
    response["rateLimitResetCredits"]["availableCount"] = json!(3);
    response["rateLimitResetCredits"]["credits"]
        .as_array_mut()
        .expect("credit rows")
        .push(json!({
            "description": null,
            "expiresAt": 1_700_300_000,
            "grantedAt": 1_699_100_000,
            "id": "credit_redeemed_private",
            "resetType": "codexRateLimits",
            "status": "redeemed",
            "title": null
        }));

    let first = normalize(&account(Some(PRIVATE_EMAIL)), &response, OBSERVED_AT_MS);
    let inventory = first.benefit_observation().expect("benefit inventory");
    assert_eq!(
        inventory.completeness(),
        BenefitInventoryCompleteness::CompleteQuantityPartialDetails
    );
    assert_eq!(inventory.lots().len(), 3);
    assert_eq!(
        inventory
            .lots()
            .iter()
            .filter(|lot| lot.detail_kind() == BenefitDetailKind::ProviderDetail)
            .count(),
        2
    );
    let aggregate = inventory
        .lots()
        .iter()
        .find(|lot| lot.detail_kind() == BenefitDetailKind::ProviderAggregate)
        .expect("aggregate gap");
    assert_eq!(aggregate.quantity(), 2);
    assert_eq!(aggregate.state(), BenefitState::Available);
    assert_eq!(aggregate.expiry(), &BenefitExpiry::Unknown);
    assert_eq!(aggregate.confidence(), BenefitConfidence::Medium);

    let second = normalize(
        &account(Some("another@example.com")),
        &response,
        OBSERVED_AT_MS,
    );
    let second_inventory = second
        .benefit_observation()
        .expect("second account inventory");
    assert!(
        inventory
            .lots()
            .iter()
            .zip(second_inventory.lots())
            .all(|(left, right)| left.lot_id() != right.lot_id())
    );
}

#[test]
fn account_pseudonym_changes_without_exposing_the_email() {
    let first = normalize(
        &account(Some(PRIVATE_EMAIL)),
        &current_response(),
        OBSERVED_AT_MS,
    );
    let second = normalize(
        &account(Some("another@example.com")),
        &current_response(),
        OBSERVED_AT_MS,
    );

    assert_ne!(first.account_id(), second.account_id());
    let debug = format!("{first:?}");
    assert!(!debug.contains(PRIVATE_EMAIL));
    assert!(!debug.contains("Private@Example.com"));
    assert!(!debug.contains(PRIVATE_CREDIT_ID));
    assert!(!debug.contains(PRIVATE_HOME));
    assert!(!debug.contains("GPT-5.3-Codex-Spark"));
}

#[test]
fn malformed_or_ambiguous_provider_data_fails_closed() {
    let mut unknown = current_response();
    unknown["unexpected"] = json!("do not echo this private value");
    assert_error(unknown, CodexQuotaErrorCode::InvalidData);

    let mut invalid_percent = current_response();
    invalid_percent["rateLimitsByLimitId"]["codex"]["primary"]["usedPercent"] = json!(101);
    assert_error(invalid_percent, CodexQuotaErrorCode::InvalidData);

    let mut mismatched_bucket = current_response();
    mismatched_bucket["rateLimitsByLimitId"]["codex"]["limitId"] = json!("another_limit");
    assert_error(mismatched_bucket, CodexQuotaErrorCode::InvalidData);

    let mut invalid_credit = current_response();
    invalid_credit["rateLimitResetCredits"]["credits"][0]["unexpected"] =
        json!("private credit payload");
    assert_error(invalid_credit, CodexQuotaErrorCode::InvalidData);

    let mut duplicate_credit = current_response();
    let duplicate_row = duplicate_credit["rateLimitResetCredits"]["credits"][0].clone();
    duplicate_credit["rateLimitResetCredits"]["credits"]
        .as_array_mut()
        .expect("credit rows")
        .push(duplicate_row);
    duplicate_credit["rateLimitResetCredits"]["availableCount"] = json!(2);
    assert_error(duplicate_credit, CodexQuotaErrorCode::InvalidData);

    let mut incoherent_available = current_response();
    incoherent_available["rateLimitResetCredits"]["availableCount"] = json!(0);
    assert_error(incoherent_available, CodexQuotaErrorCode::InvalidData);

    let mut invalid_expiry = current_response();
    invalid_expiry["rateLimitResetCredits"]["credits"][0]["expiresAt"] = json!(1_698_000_000);
    assert_error(invalid_expiry, CodexQuotaErrorCode::InvalidData);

    let missing_email = serde_json::to_vec(&account(None)).expect("account fixture");
    let response = serde_json::to_vec(&current_response()).expect("quota fixture");
    let error = CodexQuotaNormalizer::normalize(&missing_email, &response, OBSERVED_AT_MS)
        .expect_err("missing stable account identity must fail");
    assert_eq!(
        error.code(),
        CodexQuotaErrorCode::AccountIdentityUnavailable
    );
    assert_private_error(&error);

    let account_without_plan = json!({
        "requiresOpenaiAuth": true,
        "account": {
            "type": "chatgpt",
            "email": PRIVATE_EMAIL
        }
    });
    let account_without_plan = serde_json::to_vec(&account_without_plan).expect("account fixture");
    let error = CodexQuotaNormalizer::normalize(&account_without_plan, &response, OBSERVED_AT_MS)
        .expect_err("required ChatGPT account shape must fail");
    assert_eq!(
        error.code(),
        CodexQuotaErrorCode::AccountIdentityUnavailable
    );

    let control_email =
        serde_json::to_vec(&account(Some("private\n@example.com"))).expect("account fixture");
    let error = CodexQuotaNormalizer::normalize(&control_email, &response, OBSERVED_AT_MS)
        .expect_err("control-bearing account identity must fail");
    assert_eq!(
        error.code(),
        CodexQuotaErrorCode::AccountIdentityUnavailable
    );
}

#[test]
fn window_count_and_clock_arithmetic_are_bounded() {
    let mut buckets = serde_json::Map::new();
    for index in 0..17 {
        let limit_id = format!("limit_{index}");
        buckets.insert(
            limit_id.clone(),
            snapshot(
                &limit_id,
                None,
                Some(window(10, 1_700_100_000, 300)),
                Some(window(20, 1_700_200_000, 10_080)),
            ),
        );
    }
    let response = json!({
        "rateLimitResetCredits": null,
        "rateLimits": snapshot(
            "fallback",
            None,
            Some(window(1, 1_700_100_000, 300)),
            None
        ),
        "rateLimitsByLimitId": Value::Object(buckets)
    });
    let account = serde_json::to_vec(&account(Some(PRIVATE_EMAIL))).expect("account fixture");
    let response = serde_json::to_vec(&response).expect("quota fixture");
    let error = CodexQuotaNormalizer::normalize(&account, &response, OBSERVED_AT_MS)
        .expect_err("expanded window cap must fail");
    assert_eq!(error.code(), CodexQuotaErrorCode::CapacityExceeded);
    assert_private_error(&error);

    let response = serde_json::to_vec(&current_response()).expect("quota fixture");
    let error = CodexQuotaNormalizer::normalize(&account, &response, i64::MAX)
        .expect_err("freshness overflow must fail");
    assert_eq!(error.code(), CodexQuotaErrorCode::InvalidTime);
    assert_private_error(&error);

    let oversized = vec![b' '; MAX_CODEX_QUOTA_JSON_BYTES + 1];
    let error = CodexQuotaNormalizer::normalize(&account, &oversized, OBSERVED_AT_MS)
        .expect_err("quota JSON byte cap must fail before parsing");
    assert_eq!(error.code(), CodexQuotaErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_CODEX_QUOTA_JSON_BYTES));
}

fn assert_error(response: Value, expected: CodexQuotaErrorCode) {
    let account = serde_json::to_vec(&account(Some(PRIVATE_EMAIL))).expect("account fixture");
    let response = serde_json::to_vec(&response).expect("quota fixture");
    let error = CodexQuotaNormalizer::normalize(&account, &response, OBSERVED_AT_MS)
        .expect_err("fixture must fail closed");
    assert_eq!(error.code(), expected);
    assert_private_error(&error);
}

fn assert_private_error(error: &tokenmaster_codex::CodexQuotaError) {
    let rendered = format!("{error:?} {error}");
    for private in [
        PRIVATE_EMAIL,
        PRIVATE_CREDIT_ID,
        PRIVATE_HOME,
        "private credit payload",
        "do not echo this private value",
    ] {
        assert!(!rendered.contains(private));
    }
}
