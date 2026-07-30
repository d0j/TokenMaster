use tokenmaster_pricing::{
    AliasOverrideDraft, ContextClass, ModelOverrideDraft, OverrideDraft, OverrideErrorCode,
    OverrideSnapshot, PriceRatesOverride, PricingEngine, RuleSource, ServiceTier, TokenPriceBasis,
    UsdPerMillion,
};

fn rate(value: &str) -> UsdPerMillion {
    UsdPerMillion::parse(value).expect("fixture rate")
}

fn rate_patch(
    input: Option<&str>,
    cached: Option<&str>,
    output: Option<&str>,
) -> PriceRatesOverride {
    PriceRatesOverride {
        uncached_input: input.map(rate),
        cached_input: cached.map(rate),
        billable_output: output.map(rate),
    }
}

fn model<'a>(name: &'a str, standard_short: Option<PriceRatesOverride>) -> OverrideDraft<'a> {
    OverrideDraft::Model(ModelOverrideDraft {
        model: name,
        standard_short,
        standard_long: None,
        priority_short: None,
        priority_long: None,
    })
}

#[test]
fn candidate_bounds_and_keys_fail_closed() {
    for invalid in ["", "has space", "has\\path", &"x".repeat(65)] {
        let error = OverrideSnapshot::build(&[model(
            invalid,
            Some(rate_patch(Some("1"), Some("1"), Some("1"))),
        )])
        .expect_err("invalid bounded key");
        assert_eq!(error.code(), OverrideErrorCode::InvalidKey);
    }

    let names = (0..513)
        .map(|index| format!("custom-{index}"))
        .collect::<Vec<_>>();
    let drafts = names
        .iter()
        .map(|name| {
            OverrideDraft::Alias(AliasOverrideDraft {
                alias: name,
                target: "gpt-5",
            })
        })
        .collect::<Vec<_>>();
    let error = OverrideSnapshot::build(&drafts).expect_err("override count is hard bounded");
    assert_eq!(error.code(), OverrideErrorCode::TooManyEntries);
    assert_eq!(
        OverrideSnapshot::build(&drafts[..512])
            .expect("the documented maximum is accepted")
            .len(),
        512
    );
}

#[test]
fn duplicates_incomplete_models_and_alias_indirection_reject_the_whole_candidate() {
    let duplicate = [
        model(
            "custom",
            Some(rate_patch(Some("1"), Some("0.1"), Some("2"))),
        ),
        model(
            "custom",
            Some(rate_patch(Some("2"), Some("0.2"), Some("4"))),
        ),
    ];
    assert_eq!(
        OverrideSnapshot::build(&duplicate)
            .expect_err("duplicate")
            .code(),
        OverrideErrorCode::DuplicateKey
    );

    assert_eq!(
        OverrideSnapshot::build(&[model(
            "new-model",
            Some(rate_patch(Some("1"), None, Some("2"))),
        )])
        .expect_err("new rules must resolve completely")
        .code(),
        OverrideErrorCode::IncompleteRule
    );

    let chain = [
        OverrideDraft::Alias(AliasOverrideDraft {
            alias: "alias-a",
            target: "alias-b",
        }),
        OverrideDraft::Alias(AliasOverrideDraft {
            alias: "alias-b",
            target: "gpt-5",
        }),
    ];
    assert_eq!(
        OverrideSnapshot::build(&chain)
            .expect_err("alias chains are forbidden")
            .code(),
        OverrideErrorCode::AliasChain
    );

    let cycle = [
        OverrideDraft::Alias(AliasOverrideDraft {
            alias: "alias-a",
            target: "alias-b",
        }),
        OverrideDraft::Alias(AliasOverrideDraft {
            alias: "alias-b",
            target: "alias-a",
        }),
    ];
    assert_eq!(
        OverrideSnapshot::build(&cycle)
            .expect_err("alias cycles are forbidden")
            .code(),
        OverrideErrorCode::AliasChain
    );

    assert_eq!(
        OverrideSnapshot::build(&[OverrideDraft::Alias(AliasOverrideDraft {
            alias: "alias-a",
            target: "missing-model",
        })])
        .expect_err("unknown canonical target")
        .code(),
        OverrideErrorCode::AliasTargetUnknown
    );
}

#[test]
fn snapshots_are_order_independent_immutable_and_provenanced() {
    let patch = model("gpt-5.6-sol", Some(rate_patch(None, None, Some("31"))));
    let alias = OverrideDraft::Alias(AliasOverrideDraft {
        alias: "my-sol",
        target: "gpt-5.6-sol",
    });
    let first = OverrideSnapshot::build(&[patch, alias]).expect("valid candidate");
    let reordered = OverrideSnapshot::build(&[alias, patch]).expect("same effective snapshot");
    assert_eq!(first.revision(), reordered.revision());
    assert_eq!(first.len(), 2);

    let changed = OverrideSnapshot::build(&[model(
        "gpt-5.6-sol",
        Some(rate_patch(None, None, Some("32"))),
    )])
    .expect("changed candidate");
    assert_ne!(first.revision(), changed.revision());

    let engine = PricingEngine::new(first);
    let quote = engine
        .price(
            "my-sol",
            ServiceTier::StandardReported,
            ContextClass::Short,
            TokenPriceBasis::new(1_000_000, 1_000_000, 1_000_000),
        )
        .expect("alias to overridden rule");
    assert_eq!(quote.amount().get(), 36_500_000);
    assert_eq!(quote.canonical_model(), "gpt-5.6-sol");
    assert_eq!(quote.source(), RuleSource::Override);
    assert_eq!(quote.override_revision(), engine.override_revision());

    let empty = OverrideSnapshot::build(&[]).expect("empty snapshot");
    assert_eq!(
        empty.revision(),
        PricingEngine::embedded().override_revision()
    );
}

#[test]
fn rejected_candidate_cannot_mutate_an_active_engine_and_debug_is_private() {
    let engine = PricingEngine::embedded();
    let revision = engine.override_revision();
    let draft = model(
        "private-model-name",
        Some(rate_patch(Some("1"), None, Some("2"))),
    );
    let error = OverrideSnapshot::build(&[draft]).expect_err("invalid candidate");

    assert_eq!(engine.override_revision(), revision);
    assert!(!format!("{draft:?}").contains("private-model-name"));
    assert!(!format!("{error:?}").contains("private-model-name"));
    assert!(!error.to_string().contains("private-model-name"));
}
