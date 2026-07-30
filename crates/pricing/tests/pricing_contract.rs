use tokenmaster_pricing::{
    CATALOG_ID, CATALOG_RETRIEVED_DATE, ContextClass, ObservedTokens, PriceErrorCode, PriceRates,
    ServiceTier, TokenPriceBasis, UsdPerMillion, calculate_cost, embedded_catalog,
};

fn rate(value: &str) -> UsdPerMillion {
    UsdPerMillion::parse(value).expect("fixture rate must be valid")
}

#[test]
fn catalog_identity_is_release_pinned() {
    assert_eq!(CATALOG_ID, "openai-api-2026-07-16-v1");
    assert_eq!(CATALOG_RETRIEVED_DATE, "2026-07-16");
}

#[test]
fn decimal_rates_are_strict_exact_and_bounded() {
    assert_eq!(rate("0").as_micros(), 0);
    assert_eq!(rate("1").as_micros(), 1_000_000);
    assert_eq!(rate("1.75").as_micros(), 1_750_000);
    assert_eq!(rate("0.000001").as_micros(), 1);

    for invalid in [
        "",
        " 1",
        "1 ",
        "+1",
        "-1",
        ".5",
        "1.",
        "1.0000000",
        "1e3",
        "NaN",
        "inf",
        "1000000.000001",
    ] {
        let error = UsdPerMillion::parse(invalid).expect_err("rate must fail closed");
        assert_eq!(error.code(), PriceErrorCode::InvalidRate, "{invalid}");
    }
}

#[test]
fn billable_basis_includes_reasoning_without_double_counting_cache() {
    let derived = ObservedTokens {
        input: Some(100),
        cached_input: Some(40),
        output: Some(20),
        reasoning: Some(5),
        total: Some(125),
    }
    .derive_price_basis()
    .expect("consistent complete usage");

    assert_eq!(derived.uncached_input_tokens(), 60);
    assert_eq!(derived.cached_input_tokens(), 40);
    assert_eq!(derived.billable_output_tokens(), 25);

    let fallback = ObservedTokens {
        input: Some(100),
        cached_input: Some(0),
        output: Some(20),
        reasoning: Some(5),
        total: None,
    }
    .derive_price_basis()
    .expect("output plus reasoning is a complete fallback");
    assert_eq!(fallback.billable_output_tokens(), 25);
}

#[test]
fn incomplete_or_inconsistent_billable_basis_fails_closed() {
    let missing = ObservedTokens {
        input: Some(100),
        cached_input: None,
        output: Some(20),
        reasoning: None,
        total: Some(120),
    }
    .derive_price_basis()
    .expect_err("missing cache availability cannot mean zero");
    assert_eq!(missing.code(), PriceErrorCode::TokenBasisUnavailable);

    let inconsistent = ObservedTokens {
        input: Some(10),
        cached_input: Some(11),
        output: Some(1),
        reasoning: Some(0),
        total: Some(11),
    }
    .derive_price_basis()
    .expect_err("cached input cannot exceed input");
    assert_eq!(inconsistent.code(), PriceErrorCode::InconsistentTokenBasis);

    let conflicting_complete_buckets = ObservedTokens {
        input: Some(100),
        cached_input: Some(0),
        output: Some(20),
        reasoning: Some(5),
        total: Some(124),
    }
    .derive_price_basis()
    .expect_err("total and complete output buckets must agree");
    assert_eq!(
        conflicting_complete_buckets.code(),
        PriceErrorCode::InconsistentTokenBasis
    );
}

#[test]
fn fixed_point_cost_rounds_once_and_explicit_zero_is_not_missing() {
    let rates = PriceRates {
        uncached_input: rate("1.75"),
        cached_input: rate("0.175"),
        billable_output: rate("14"),
    };
    let amount =
        calculate_cost(rates, TokenPriceBasis::new(1, 1, 1)).expect("calculation must fit");
    assert_eq!(amount.get(), 16);

    let zero = PriceRates {
        uncached_input: rate("0"),
        cached_input: rate("0"),
        billable_output: rate("0"),
    };
    assert_eq!(
        calculate_cost(zero, TokenPriceBasis::new(u64::MAX, u64::MAX, u64::MAX))
            .expect("explicit free pricing")
            .get(),
        0
    );

    let maximum_rate = UsdPerMillion::parse("1000000").expect("maximum supported rate");
    let overflow = calculate_cost(
        PriceRates {
            uncached_input: maximum_rate,
            cached_input: maximum_rate,
            billable_output: maximum_rate,
        },
        TokenPriceBasis::new(u64::MAX, u64::MAX, u64::MAX),
    )
    .expect_err("public USD micro result cannot silently truncate");
    assert_eq!(overflow.code(), PriceErrorCode::ArithmeticOverflow);
}

#[test]
fn gpt_5_6_standard_long_and_priority_rates_are_exact() {
    let basis = TokenPriceBasis::new(1_000_000, 1_000_000, 1_000_000);
    let catalog = embedded_catalog();

    let standard = catalog
        .price(
            "gpt-5.6-sol",
            ServiceTier::StandardReported,
            ContextClass::Short,
            basis,
        )
        .expect("standard pricing");
    assert_eq!(standard.amount().get(), 35_500_000);
    assert!(!standard.assumed_standard());
    assert_eq!(standard.catalog_id(), CATALOG_ID);

    let assumed = catalog
        .price(
            "gpt-5.6",
            ServiceTier::StandardAssumed,
            ContextClass::Long,
            basis,
        )
        .expect("reviewed alias and long-context pricing");
    assert_eq!(assumed.amount().get(), 56_000_000);
    assert_eq!(assumed.canonical_model(), "gpt-5.6-sol");
    assert!(assumed.assumed_standard());

    let priority = catalog
        .price(
            "gpt-5.6-sol",
            ServiceTier::Priority,
            ContextClass::Short,
            basis,
        )
        .expect("priority short-context pricing");
    assert_eq!(priority.amount().get(), 71_000_000);
}

#[test]
fn unsupported_or_unknown_catalog_combinations_never_become_zero() {
    let catalog = embedded_catalog();
    let basis = TokenPriceBasis::new(1, 1, 1);

    let priority_long = catalog
        .price(
            "gpt-5.6-sol",
            ServiceTier::Priority,
            ContextClass::Long,
            basis,
        )
        .expect_err("Priority excludes long context");
    assert_eq!(priority_long.code(), PriceErrorCode::TierContextUnsupported);

    let undocumented_pro_long = catalog
        .price(
            "gpt-5.5-pro-2026-04-23",
            ServiceTier::StandardReported,
            ContextClass::Long,
            basis,
        )
        .expect_err("an undocumented long-context rate must not be inferred");
    assert_eq!(
        undocumented_pro_long.code(),
        PriceErrorCode::TierContextUnsupported
    );

    let unknown_tier = catalog
        .price(
            "gpt-5.6-sol",
            ServiceTier::Unknown,
            ContextClass::Short,
            basis,
        )
        .expect_err("unknown tier");
    assert_eq!(unknown_tier.code(), PriceErrorCode::TierUnknown);

    let unknown_context = catalog
        .price(
            "gpt-5.6-sol",
            ServiceTier::StandardReported,
            ContextClass::Unavailable,
            basis,
        )
        .expect_err("context is required for a tiered model");
    assert_eq!(unknown_context.code(), PriceErrorCode::ContextUnavailable);

    for model in [
        "future-model",
        "gpt-5.60",
        "openai/gpt-5.6-sol",
        "gpt-5.6-sol-2099-01-01",
    ] {
        let error = catalog
            .price(
                model,
                ServiceTier::StandardReported,
                ContextClass::Short,
                basis,
            )
            .expect_err("unreviewed model matching must fail");
        assert_eq!(error.code(), PriceErrorCode::ModelUnpriced, "{model}");
    }
}
