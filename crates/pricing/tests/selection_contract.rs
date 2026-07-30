use tokenmaster_pricing::{
    ContextClass, CostAvailability, CostComposition, CostMode, MissingReasonCode,
    ModelOverrideDraft, OverrideDraft, OverrideSnapshot, PriceBasisRow, PriceErrorCode,
    PriceRatesOverride, PricingEngine, ServiceTier, TokenPriceBasis, UsdMicros, UsdPerMillion,
    cost_values_conflict, select_cost,
};

fn rate(value: &str) -> UsdPerMillion {
    UsdPerMillion::parse(value).expect("fixture rate")
}

fn row<'a>(model: &'a str, events: u64, reported: Option<u64>) -> PriceBasisRow<'a> {
    PriceBasisRow {
        model,
        tier: ServiceTier::StandardReported,
        context: ContextClass::Short,
        event_count: events,
        calculable_event_count: events,
        basis: TokenPriceBasis::new(1_000_000, 1_000_000, 1_000_000),
        reported_event_count: reported.map_or(0, |_| events),
        reported_cost: reported.map(UsdMicros::new),
    }
}

#[test]
fn all_modes_select_their_documented_sources_without_hiding_conflict() {
    let engine = PricingEngine::embedded();
    let rows = [
        row("gpt-5.6-sol", 2, Some(30_000_000)),
        row("gpt-5.6-sol", 2, None),
    ];

    let auto = select_cost(&engine, CostMode::Auto, &rows, 0).expect("auto result");
    assert_eq!(auto.availability(), CostAvailability::Complete);
    assert_eq!(auto.amount().map(UsdMicros::get), Some(65_500_000));
    assert_eq!(auto.composition(), CostComposition::Mixed);
    assert_eq!(auto.counters().reported_events, 2);
    assert_eq!(auto.counters().priced_events, 2);
    assert_eq!(auto.counters().conflict_events, 2);

    let calculated =
        select_cost(&engine, CostMode::Calculated, &rows, 0).expect("calculated result");
    assert_eq!(calculated.availability(), CostAvailability::Complete);
    assert_eq!(calculated.amount().map(UsdMicros::get), Some(71_000_000));
    assert_eq!(calculated.composition(), CostComposition::Calculated);
    assert_eq!(calculated.counters().priced_events, 4);
    assert_eq!(calculated.counters().reported_events, 0);
    assert_eq!(calculated.counters().conflict_events, 2);

    let reported = select_cost(&engine, CostMode::Reported, &rows, 0).expect("reported result");
    assert_eq!(reported.availability(), CostAvailability::Partial);
    assert_eq!(reported.amount().map(UsdMicros::get), Some(30_000_000));
    assert_eq!(reported.composition(), CostComposition::Reported);
    assert_eq!(reported.counters().reported_events, 2);
    assert_eq!(reported.counters().unpriced_events, 2);
    assert!(
        reported
            .missing()
            .iter()
            .any(|item| item.reason() == MissingReasonCode::ReportedCostMissing)
    );
}

#[test]
fn zero_partial_unavailable_and_omitted_are_distinct() {
    let zero_rates = PriceRatesOverride {
        uncached_input: Some(rate("0")),
        cached_input: Some(rate("0")),
        billable_output: Some(rate("0")),
    };
    let snapshot = OverrideSnapshot::build(&[OverrideDraft::Model(ModelOverrideDraft {
        model: "free-model",
        standard_short: Some(zero_rates),
        standard_long: None,
        priority_short: None,
        priority_long: None,
    })])
    .expect("complete zero rule");
    let engine = PricingEngine::new(snapshot);

    let zero = select_cost(
        &engine,
        CostMode::Calculated,
        &[row("free-model", 1, None)],
        0,
    )
    .expect("explicit free pricing");
    assert_eq!(zero.availability(), CostAvailability::Zero);
    assert_eq!(zero.amount().map(UsdMicros::get), Some(0));
    assert_eq!(zero.catalog_id(), Some(tokenmaster_pricing::CATALOG_ID));
    assert_eq!(zero.override_revision(), Some(engine.override_revision()));
    assert!(zero.used_override());

    let unavailable = select_cost(
        &engine,
        CostMode::Calculated,
        &[row("unknown-model", 1, None)],
        0,
    )
    .expect("truthful missing result");
    assert_eq!(unavailable.availability(), CostAvailability::Unavailable);
    assert_eq!(unavailable.amount(), None);
    assert!(
        unavailable
            .missing()
            .iter()
            .any(|item| item.reason() == MissingReasonCode::ModelUnpriced)
    );

    let partial = select_cost(
        &engine,
        CostMode::Calculated,
        &[row("free-model", 1, None), row("unknown-model", 1, None)],
        0,
    )
    .expect("bounded partial result");
    assert_eq!(partial.availability(), CostAvailability::Partial);
    assert_eq!(partial.amount().map(UsdMicros::get), Some(0));

    let omitted = select_cost(
        &engine,
        CostMode::Calculated,
        &[row("free-model", 1, None)],
        3,
    )
    .expect("omission remains explicit");
    assert_eq!(omitted.availability(), CostAvailability::Partial);
    assert_eq!(omitted.counters().omitted_events, 3);
    assert!(
        omitted
            .missing()
            .iter()
            .any(|item| item.reason() == MissingReasonCode::KeyLimitReached)
    );
}

#[test]
fn conflict_requires_more_than_one_cent_and_more_than_two_percent() {
    assert!(!cost_values_conflict(
        UsdMicros::new(1_000_000),
        UsdMicros::new(1_010_000)
    ));
    assert!(!cost_values_conflict(
        UsdMicros::new(1_000_000),
        UsdMicros::new(1_020_408)
    ));
    assert!(cost_values_conflict(
        UsdMicros::new(1_000_000),
        UsdMicros::new(1_020_409)
    ));
}

#[test]
fn rows_and_diagnostics_are_hard_bounded_and_debug_private() {
    let engine = PricingEngine::embedded();
    let invalid = PriceBasisRow {
        model: "gpt-5",
        tier: ServiceTier::StandardReported,
        context: ContextClass::Short,
        event_count: 1,
        calculable_event_count: 2,
        basis: TokenPriceBasis::new(0, 0, 0),
        reported_event_count: 0,
        reported_cost: None,
    };
    assert_eq!(
        select_cost(&engine, CostMode::Auto, &[invalid], 0)
            .expect_err("incoherent counts")
            .code(),
        PriceErrorCode::InvalidPriceRow
    );

    let rows = (0..513).map(|_| row("gpt-5", 1, None)).collect::<Vec<_>>();
    assert_eq!(
        select_cost(&engine, CostMode::Auto, &rows, 0)
            .expect_err("row count bound")
            .code(),
        PriceErrorCode::TooManyPriceRows
    );

    let private = select_cost(
        &engine,
        CostMode::Calculated,
        &[row("private-model-name", 1, None)],
        0,
    )
    .expect("missing price result");
    assert!(private.missing().len() <= 32);
    assert!(!format!("{private:?}").contains("private-model-name"));
    assert_eq!(private.missing()[0].key(), Some("private-model-name"));
    assert!(!format!("{:?}", private.missing()[0]).contains("private-model-name"));

    let names = (0..40)
        .map(|index| format!("missing-{index:02}"))
        .collect::<Vec<_>>();
    let forward = names
        .iter()
        .map(|name| row(name, 1, None))
        .collect::<Vec<_>>();
    let reverse = forward.iter().copied().rev().collect::<Vec<_>>();
    let first =
        select_cost(&engine, CostMode::Calculated, &forward, 0).expect("bounded diagnostics");
    let second = select_cost(&engine, CostMode::Calculated, &reverse, 0)
        .expect("order-independent diagnostics");
    assert_eq!(first.missing(), second.missing());
    assert_eq!(first.missing().len(), 32);
    assert_eq!(first.omitted_missing_details(), 8);
}

#[test]
fn counter_and_public_amount_overflow_fail_closed() {
    let engine = PricingEngine::embedded();
    assert_eq!(
        select_cost(
            &engine,
            CostMode::Reported,
            &[row("gpt-5", 1, Some(0))],
            u64::MAX,
        )
        .expect_err("event counters cannot wrap")
        .code(),
        PriceErrorCode::ArithmeticOverflow
    );

    let overflow = select_cost(
        &engine,
        CostMode::Reported,
        &[row("gpt-5", 1, Some(u64::MAX)), row("gpt-5", 1, Some(1))],
        0,
    )
    .expect("numeric failure is a truthful result");
    assert_eq!(overflow.availability(), CostAvailability::Unavailable);
    assert_eq!(overflow.amount(), None);
    assert_eq!(overflow.composition(), CostComposition::None);
    assert_eq!(overflow.counters().unpriced_events, 2);
    assert!(
        overflow
            .missing()
            .iter()
            .any(|item| item.reason() == MissingReasonCode::ArithmeticOverflow)
    );
}
