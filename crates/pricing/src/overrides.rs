use std::{collections::BTreeSet, fmt};

use sha2::{Digest, Sha256};

use crate::{
    CATALOG, CATALOG_ID, ContextClass, ModelRule, PriceError, PriceErrorCode, PriceRates,
    ServiceTier, TokenPriceBasis, UsdMicros, UsdPerMillion, calculate_cost,
};

pub const MAX_OVERRIDE_ENTRIES: usize = 512;
pub const MAX_OVERRIDE_MODEL_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverrideErrorCode {
    TooManyEntries,
    InvalidKey,
    DuplicateKey,
    KeyConflict,
    IncompleteRule,
    AliasChain,
    AliasTargetUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverrideError {
    code: OverrideErrorCode,
}

impl OverrideError {
    const fn new(code: OverrideErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> OverrideErrorCode {
        self.code
    }
}

impl fmt::Display for OverrideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            OverrideErrorCode::TooManyEntries => "override count exceeds supported range",
            OverrideErrorCode::InvalidKey => "override key is invalid",
            OverrideErrorCode::DuplicateKey => "override key is duplicated",
            OverrideErrorCode::KeyConflict => "override key conflicts with the embedded catalog",
            OverrideErrorCode::IncompleteRule => "override rule is incomplete",
            OverrideErrorCode::AliasChain => "override aliases must resolve in one hop",
            OverrideErrorCode::AliasTargetUnknown => "override alias target is unavailable",
        })
    }
}

impl std::error::Error for OverrideError {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PriceRatesOverride {
    pub uncached_input: Option<UsdPerMillion>,
    pub cached_input: Option<UsdPerMillion>,
    pub billable_output: Option<UsdPerMillion>,
}

impl PriceRatesOverride {
    fn has_value(self) -> bool {
        self.uncached_input.is_some()
            || self.cached_input.is_some()
            || self.billable_output.is_some()
    }
}

#[derive(Clone, Copy)]
pub struct ModelOverrideDraft<'a> {
    pub model: &'a str,
    pub standard_short: Option<PriceRatesOverride>,
    pub standard_long: Option<PriceRatesOverride>,
    pub priority_short: Option<PriceRatesOverride>,
    pub priority_long: Option<PriceRatesOverride>,
}

impl fmt::Debug for ModelOverrideDraft<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelOverrideDraft")
            .field("model", &"<redacted>")
            .field("has_standard_short", &self.standard_short.is_some())
            .field("has_standard_long", &self.standard_long.is_some())
            .field("has_priority_short", &self.priority_short.is_some())
            .field("has_priority_long", &self.priority_long.is_some())
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct AliasOverrideDraft<'a> {
    pub alias: &'a str,
    pub target: &'a str,
}

impl fmt::Debug for AliasOverrideDraft<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AliasOverrideDraft")
            .field("alias", &"<redacted>")
            .field("target", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub enum OverrideDraft<'a> {
    Model(ModelOverrideDraft<'a>),
    Alias(AliasOverrideDraft<'a>),
}

impl OverrideDraft<'_> {
    fn key(&self) -> &str {
        match self {
            Self::Model(draft) => draft.model,
            Self::Alias(draft) => draft.alias,
        }
    }
}

impl fmt::Debug for OverrideDraft<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(draft) => draft.fmt(formatter),
            Self::Alias(draft) => draft.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct OverrideRevision([u8; 32]);

impl OverrideRevision {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for OverrideRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OverrideRevision(")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str(")")
    }
}

#[derive(Clone)]
struct EffectiveModelRule {
    canonical_model: Box<str>,
    standard_short: PriceRates,
    standard_long: Option<PriceRates>,
    priority_short: Option<PriceRates>,
    priority_long: Option<PriceRates>,
}

#[derive(Clone)]
struct AliasRule {
    alias: Box<str>,
    target: Box<str>,
}

#[derive(Clone)]
pub struct OverrideSnapshot {
    models: Box<[EffectiveModelRule]>,
    aliases: Box<[AliasRule]>,
    entry_count: usize,
    revision: OverrideRevision,
}

impl fmt::Debug for OverrideSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OverrideSnapshot")
            .field("entry_count", &self.entry_count)
            .field("revision", &self.revision)
            .finish()
    }
}

impl OverrideSnapshot {
    pub fn build(drafts: &[OverrideDraft<'_>]) -> Result<Self, OverrideError> {
        if drafts.len() > MAX_OVERRIDE_ENTRIES {
            return Err(OverrideError::new(OverrideErrorCode::TooManyEntries));
        }

        let mut candidate_keys = BTreeSet::new();
        for draft in drafts {
            let key = draft.key();
            if !is_valid_model_key(key) {
                return Err(OverrideError::new(OverrideErrorCode::InvalidKey));
            }
            if !candidate_keys.insert(key) {
                return Err(OverrideError::new(OverrideErrorCode::DuplicateKey));
            }
            match draft {
                OverrideDraft::Model(model) => {
                    if embedded_alias_target(model.model).is_some() {
                        return Err(OverrideError::new(OverrideErrorCode::KeyConflict));
                    }
                }
                OverrideDraft::Alias(alias) => {
                    if embedded_rule(alias.alias).is_some()
                        || embedded_alias_target(alias.alias).is_some()
                    {
                        return Err(OverrideError::new(OverrideErrorCode::KeyConflict));
                    }
                    if !is_valid_model_key(alias.target) {
                        return Err(OverrideError::new(OverrideErrorCode::InvalidKey));
                    }
                }
            }
        }

        let mut models = Vec::new();
        for draft in drafts {
            let OverrideDraft::Model(draft) = draft else {
                continue;
            };
            let has_patch = [
                draft.standard_short,
                draft.standard_long,
                draft.priority_short,
                draft.priority_long,
            ]
            .into_iter()
            .flatten()
            .any(PriceRatesOverride::has_value);
            if !has_patch {
                return Err(OverrideError::new(OverrideErrorCode::IncompleteRule));
            }

            let base = embedded_rule(draft.model);
            let standard_short =
                merge_rates(base.map(|rule| rule.standard_short), draft.standard_short)?
                    .ok_or_else(|| OverrideError::new(OverrideErrorCode::IncompleteRule))?;
            models.push(EffectiveModelRule {
                canonical_model: draft.model.to_owned().into_boxed_str(),
                standard_short,
                standard_long: merge_rates(
                    base.and_then(|rule| rule.standard_long),
                    draft.standard_long,
                )?,
                priority_short: merge_rates(
                    base.and_then(|rule| rule.priority_short),
                    draft.priority_short,
                )?,
                priority_long: merge_rates(
                    base.and_then(|rule| rule.priority_long),
                    draft.priority_long,
                )?,
            });
        }
        models.sort_unstable_by(|left, right| left.canonical_model.cmp(&right.canonical_model));

        let alias_keys = drafts
            .iter()
            .filter_map(|draft| match draft {
                OverrideDraft::Alias(alias) => Some(alias.alias),
                OverrideDraft::Model(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let mut aliases = Vec::new();
        for draft in drafts {
            let OverrideDraft::Alias(draft) = draft else {
                continue;
            };
            if alias_keys.contains(draft.target) || embedded_alias_target(draft.target).is_some() {
                return Err(OverrideError::new(OverrideErrorCode::AliasChain));
            }
            let known_target = embedded_rule(draft.target).is_some()
                || models
                    .binary_search_by(|rule| rule.canonical_model.as_ref().cmp(draft.target))
                    .is_ok();
            if !known_target {
                return Err(OverrideError::new(OverrideErrorCode::AliasTargetUnknown));
            }
            aliases.push(AliasRule {
                alias: draft.alias.to_owned().into_boxed_str(),
                target: draft.target.to_owned().into_boxed_str(),
            });
        }
        aliases.sort_unstable_by(|left, right| left.alias.cmp(&right.alias));

        Ok(Self::from_validated(models, aliases, drafts.len()))
    }

    fn empty() -> Self {
        Self::from_validated(Vec::new(), Vec::new(), 0)
    }

    fn from_validated(
        models: Vec<EffectiveModelRule>,
        aliases: Vec<AliasRule>,
        entry_count: usize,
    ) -> Self {
        let revision = revision_for(&models, &aliases);
        Self {
            models: models.into_boxed_slice(),
            aliases: aliases.into_boxed_slice(),
            entry_count,
            revision,
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.entry_count
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    #[must_use]
    pub const fn revision(&self) -> OverrideRevision {
        self.revision
    }
}

fn merge_rates(
    base: Option<PriceRates>,
    patch: Option<PriceRatesOverride>,
) -> Result<Option<PriceRates>, OverrideError> {
    let Some(patch) = patch else {
        return Ok(base);
    };
    let uncached_input = patch
        .uncached_input
        .or_else(|| base.map(|rates| rates.uncached_input));
    let cached_input = patch
        .cached_input
        .or_else(|| base.map(|rates| rates.cached_input));
    let billable_output = patch
        .billable_output
        .or_else(|| base.map(|rates| rates.billable_output));
    match (uncached_input, cached_input, billable_output) {
        (Some(uncached_input), Some(cached_input), Some(billable_output)) => Ok(Some(PriceRates {
            uncached_input,
            cached_input,
            billable_output,
        })),
        _ => Err(OverrideError::new(OverrideErrorCode::IncompleteRule)),
    }
}

pub(crate) fn is_valid_model_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OVERRIDE_MODEL_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/' | b':')
        })
}

fn embedded_rule(model: &str) -> Option<&'static ModelRule> {
    CATALOG.iter().find(|rule| rule.canonical_model == model)
}

fn embedded_alias_target(alias: &str) -> Option<&'static str> {
    CATALOG.iter().find_map(|rule| {
        rule.aliases
            .contains(&alias)
            .then_some(rule.canonical_model)
    })
}

fn revision_for(models: &[EffectiveModelRule], aliases: &[AliasRule]) -> OverrideRevision {
    let mut hasher = Sha256::new();
    hasher.update(b"tokenmaster-pricing-overrides-v1\0");
    for rule in models {
        hasher.update(b"model\0");
        hash_string(&mut hasher, &rule.canonical_model);
        hash_rates(&mut hasher, Some(rule.standard_short));
        hash_rates(&mut hasher, rule.standard_long);
        hash_rates(&mut hasher, rule.priority_short);
        hash_rates(&mut hasher, rule.priority_long);
    }
    for alias in aliases {
        hasher.update(b"alias\0");
        hash_string(&mut hasher, &alias.alias);
        hash_string(&mut hasher, &alias.target);
    }
    OverrideRevision(hasher.finalize().into())
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    hasher.update([value.len() as u8]);
    hasher.update(value.as_bytes());
}

fn hash_rates(hasher: &mut Sha256, rates: Option<PriceRates>) {
    let Some(rates) = rates else {
        hasher.update([0]);
        return;
    };
    hasher.update([1]);
    for rate in [
        rates.uncached_input,
        rates.cached_input,
        rates.billable_output,
    ] {
        hasher.update(rate.as_micros().to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleSource {
    Embedded,
    Override,
}

pub(crate) struct ResolvedPriceRule<'a> {
    pub(crate) canonical_model: &'a str,
    pub(crate) rates: PriceRates,
    pub(crate) source: RuleSource,
    pub(crate) assumed_standard: bool,
}

#[derive(Clone)]
pub struct PricingEngine {
    overrides: OverrideSnapshot,
}

impl fmt::Debug for PricingEngine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PricingEngine")
            .field("catalog_id", &CATALOG_ID)
            .field("override_revision", &self.overrides.revision)
            .field("override_count", &self.overrides.entry_count)
            .finish()
    }
}

impl PricingEngine {
    #[must_use]
    pub const fn new(overrides: OverrideSnapshot) -> Self {
        Self { overrides }
    }

    #[must_use]
    pub fn embedded() -> Self {
        Self::new(OverrideSnapshot::empty())
    }

    #[must_use]
    pub const fn override_revision(&self) -> OverrideRevision {
        self.overrides.revision
    }

    pub fn price<'a>(
        &'a self,
        model: &'a str,
        tier: ServiceTier,
        context: ContextClass,
        basis: TokenPriceBasis,
    ) -> Result<EngineQuote<'a>, PriceError> {
        let resolved = self.resolve(model, tier, context)?;
        Ok(EngineQuote {
            amount: calculate_cost(resolved.rates, basis)?,
            canonical_model: resolved.canonical_model,
            source: resolved.source,
            override_revision: self.overrides.revision,
        })
    }

    pub(crate) fn resolve<'a>(
        &'a self,
        model: &'a str,
        tier: ServiceTier,
        context: ContextClass,
    ) -> Result<ResolvedPriceRule<'a>, PriceError> {
        let canonical_model = self
            .overrides
            .aliases
            .binary_search_by(|alias| alias.alias.as_ref().cmp(model))
            .ok()
            .map_or(model, |index| self.overrides.aliases[index].target.as_ref());
        let canonical_model = embedded_alias_target(canonical_model).unwrap_or(canonical_model);

        if let Ok(index) = self
            .overrides
            .models
            .binary_search_by(|rule| rule.canonical_model.as_ref().cmp(canonical_model))
        {
            let rule = &self.overrides.models[index];
            return Ok(ResolvedPriceRule {
                canonical_model: &rule.canonical_model,
                rates: select_effective_rates(rule, tier, context)?,
                source: RuleSource::Override,
                assumed_standard: tier == ServiceTier::StandardAssumed,
            });
        }

        let rule = embedded_rule(canonical_model)
            .ok_or_else(|| PriceError::new(PriceErrorCode::ModelUnpriced))?;
        Ok(ResolvedPriceRule {
            canonical_model: rule.canonical_model,
            rates: crate::select_rule_rates(rule, tier, context)?,
            source: RuleSource::Embedded,
            assumed_standard: tier == ServiceTier::StandardAssumed,
        })
    }
}

fn select_effective_rates(
    rule: &EffectiveModelRule,
    tier: ServiceTier,
    context: ContextClass,
) -> Result<PriceRates, PriceError> {
    match (tier, context) {
        (ServiceTier::Unknown, _) => Err(PriceError::new(PriceErrorCode::TierUnknown)),
        (_, ContextClass::Unavailable) => Err(PriceError::new(PriceErrorCode::ContextUnavailable)),
        (ServiceTier::StandardReported | ServiceTier::StandardAssumed, ContextClass::Short) => {
            Ok(rule.standard_short)
        }
        (ServiceTier::StandardReported | ServiceTier::StandardAssumed, ContextClass::Long) => rule
            .standard_long
            .ok_or_else(|| PriceError::new(PriceErrorCode::TierContextUnsupported)),
        (ServiceTier::Priority, ContextClass::Short) => rule
            .priority_short
            .ok_or_else(|| PriceError::new(PriceErrorCode::TierContextUnsupported)),
        (ServiceTier::Priority, ContextClass::Long) => rule
            .priority_long
            .ok_or_else(|| PriceError::new(PriceErrorCode::TierContextUnsupported)),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct EngineQuote<'a> {
    amount: UsdMicros,
    canonical_model: &'a str,
    source: RuleSource,
    override_revision: OverrideRevision,
}

impl fmt::Debug for EngineQuote<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EngineQuote")
            .field("amount", &self.amount)
            .field("source", &self.source)
            .field("override_revision", &self.override_revision)
            .finish()
    }
}

impl<'a> EngineQuote<'a> {
    #[must_use]
    pub const fn amount(self) -> UsdMicros {
        self.amount
    }

    #[must_use]
    pub const fn canonical_model(self) -> &'a str {
        self.canonical_model
    }

    #[must_use]
    pub const fn source(self) -> RuleSource {
        self.source
    }

    #[must_use]
    pub const fn catalog_id(self) -> &'static str {
        CATALOG_ID
    }

    #[must_use]
    pub const fn override_revision(self) -> OverrideRevision {
        self.override_revision
    }
}
