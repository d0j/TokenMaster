#![forbid(unsafe_code)]
#![deny(clippy::expect_used, clippy::unwrap_used)]

use std::fmt;

pub const CATALOG_ID: &str = "openai-api-2026-07-16-v1";
pub const CATALOG_RETRIEVED_DATE: &str = "2026-07-16";
pub const MAX_RATE_MICROS_PER_MILLION: u64 = 1_000_000_000_000;

const TOKENS_PER_MILLION: u128 = 1_000_000;
const HALF_TOKEN_MILLION: u128 = TOKENS_PER_MILLION / 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceTier {
    StandardReported,
    StandardAssumed,
    Priority,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextClass {
    Short,
    Long,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PriceErrorCode {
    ModelUnpriced,
    TierUnknown,
    ContextUnavailable,
    TierContextUnsupported,
    TokenBasisUnavailable,
    InconsistentTokenBasis,
    InvalidRate,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceError {
    code: PriceErrorCode,
}

impl PriceError {
    const fn new(code: PriceErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> PriceErrorCode {
        self.code
    }
}

impl fmt::Display for PriceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            PriceErrorCode::ModelUnpriced => "model pricing is unavailable",
            PriceErrorCode::TierUnknown => "service tier pricing is unavailable",
            PriceErrorCode::ContextUnavailable => "context pricing state is unavailable",
            PriceErrorCode::TierContextUnsupported => {
                "service tier and context combination is unsupported"
            }
            PriceErrorCode::TokenBasisUnavailable => "token price basis is unavailable",
            PriceErrorCode::InconsistentTokenBasis => "token price basis is inconsistent",
            PriceErrorCode::InvalidRate => "price rate is invalid",
            PriceErrorCode::ArithmeticOverflow => "price arithmetic exceeds supported range",
        })
    }
}

impl std::error::Error for PriceError {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsdMicros(u64);

impl UsdMicros {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsdPerMillion(u64);

impl UsdPerMillion {
    pub fn parse(value: &str) -> Result<Self, PriceError> {
        let (whole, fractional) = value
            .split_once('.')
            .map_or((value, None), |parts| (parts.0, Some(parts.1)));
        if whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || value.matches('.').count() > 1
            || fractional.is_some_and(|digits| {
                digits.is_empty()
                    || digits.len() > 6
                    || !digits.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(PriceError::new(PriceErrorCode::InvalidRate));
        }

        let whole = parse_ascii_u64(whole)?;
        let whole_micros = whole
            .checked_mul(1_000_000)
            .ok_or_else(|| PriceError::new(PriceErrorCode::InvalidRate))?;
        let fractional_micros = match fractional {
            Some(digits) => {
                let parsed = parse_ascii_u64(digits)?;
                parsed
                    .checked_mul(10_u64.pow(6_u32.saturating_sub(digits.len() as u32)))
                    .ok_or_else(|| PriceError::new(PriceErrorCode::InvalidRate))?
            }
            None => 0,
        };
        let micros = whole_micros
            .checked_add(fractional_micros)
            .filter(|value| *value <= MAX_RATE_MICROS_PER_MILLION)
            .ok_or_else(|| PriceError::new(PriceErrorCode::InvalidRate))?;
        Ok(Self(micros))
    }

    #[must_use]
    pub const fn as_micros(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TokenPriceBasis {
    uncached_input_tokens: u64,
    cached_input_tokens: u64,
    billable_output_tokens: u64,
}

impl TokenPriceBasis {
    #[must_use]
    pub const fn new(
        uncached_input_tokens: u64,
        cached_input_tokens: u64,
        billable_output_tokens: u64,
    ) -> Self {
        Self {
            uncached_input_tokens,
            cached_input_tokens,
            billable_output_tokens,
        }
    }

    #[must_use]
    pub const fn uncached_input_tokens(self) -> u64 {
        self.uncached_input_tokens
    }

    #[must_use]
    pub const fn cached_input_tokens(self) -> u64 {
        self.cached_input_tokens
    }

    #[must_use]
    pub const fn billable_output_tokens(self) -> u64 {
        self.billable_output_tokens
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObservedTokens {
    pub input: Option<u64>,
    pub cached_input: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
    pub total: Option<u64>,
}

impl ObservedTokens {
    pub fn derive_price_basis(self) -> Result<TokenPriceBasis, PriceError> {
        let input = self
            .input
            .ok_or_else(|| PriceError::new(PriceErrorCode::TokenBasisUnavailable))?;
        let cached = self
            .cached_input
            .ok_or_else(|| PriceError::new(PriceErrorCode::TokenBasisUnavailable))?;
        let uncached = input
            .checked_sub(cached)
            .ok_or_else(|| PriceError::new(PriceErrorCode::InconsistentTokenBasis))?;

        let observed_output = match (self.output, self.reasoning) {
            (Some(output), Some(reasoning)) => Some(
                output
                    .checked_add(reasoning)
                    .ok_or_else(|| PriceError::new(PriceErrorCode::ArithmeticOverflow))?,
            ),
            _ => None,
        };
        let billable_output = match self.total {
            Some(total) => {
                let derived = total
                    .checked_sub(input)
                    .ok_or_else(|| PriceError::new(PriceErrorCode::InconsistentTokenBasis))?;
                if observed_output.is_some_and(|observed| observed != derived) {
                    return Err(PriceError::new(PriceErrorCode::InconsistentTokenBasis));
                }
                derived
            }
            None => observed_output
                .ok_or_else(|| PriceError::new(PriceErrorCode::TokenBasisUnavailable))?,
        };
        Ok(TokenPriceBasis::new(uncached, cached, billable_output))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceRates {
    pub uncached_input: UsdPerMillion,
    pub cached_input: UsdPerMillion,
    pub billable_output: UsdPerMillion,
}

pub fn calculate_cost(rates: PriceRates, basis: TokenPriceBasis) -> Result<UsdMicros, PriceError> {
    let mut numerator = 0_u128;
    for (tokens, rate) in [
        (basis.uncached_input_tokens, rates.uncached_input),
        (basis.cached_input_tokens, rates.cached_input),
        (basis.billable_output_tokens, rates.billable_output),
    ] {
        let component = u128::from(tokens)
            .checked_mul(u128::from(rate.0))
            .ok_or_else(|| PriceError::new(PriceErrorCode::ArithmeticOverflow))?;
        numerator = numerator
            .checked_add(component)
            .ok_or_else(|| PriceError::new(PriceErrorCode::ArithmeticOverflow))?;
    }
    let rounded = numerator
        .checked_add(HALF_TOKEN_MILLION)
        .ok_or_else(|| PriceError::new(PriceErrorCode::ArithmeticOverflow))?
        / TOKENS_PER_MILLION;
    let amount =
        u64::try_from(rounded).map_err(|_| PriceError::new(PriceErrorCode::ArithmeticOverflow))?;
    Ok(UsdMicros(amount))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceQuote {
    amount: UsdMicros,
    canonical_model: &'static str,
    assumed_standard: bool,
}

impl PriceQuote {
    #[must_use]
    pub const fn amount(self) -> UsdMicros {
        self.amount
    }

    #[must_use]
    pub const fn canonical_model(self) -> &'static str {
        self.canonical_model
    }

    #[must_use]
    pub const fn assumed_standard(self) -> bool {
        self.assumed_standard
    }

    #[must_use]
    pub const fn catalog_id(self) -> &'static str {
        CATALOG_ID
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EmbeddedCatalog;

impl EmbeddedCatalog {
    pub fn price(
        self,
        model: &str,
        tier: ServiceTier,
        context: ContextClass,
        basis: TokenPriceBasis,
    ) -> Result<PriceQuote, PriceError> {
        let rule = CATALOG
            .iter()
            .find(|rule| rule.canonical_model == model || rule.aliases.contains(&model))
            .ok_or_else(|| PriceError::new(PriceErrorCode::ModelUnpriced))?;
        let rates = match (tier, context) {
            (ServiceTier::Unknown, _) => {
                return Err(PriceError::new(PriceErrorCode::TierUnknown));
            }
            (_, ContextClass::Unavailable) => {
                return Err(PriceError::new(PriceErrorCode::ContextUnavailable));
            }
            (ServiceTier::StandardReported | ServiceTier::StandardAssumed, ContextClass::Short) => {
                Some(rule.standard_short)
            }
            (ServiceTier::StandardReported | ServiceTier::StandardAssumed, ContextClass::Long) => {
                rule.standard_long
            }
            (ServiceTier::Priority, ContextClass::Short) => rule.priority_short,
            (ServiceTier::Priority, ContextClass::Long) => rule.priority_long,
        }
        .ok_or_else(|| PriceError::new(PriceErrorCode::TierContextUnsupported))?;
        Ok(PriceQuote {
            amount: calculate_cost(rates, basis)?,
            canonical_model: rule.canonical_model,
            assumed_standard: tier == ServiceTier::StandardAssumed,
        })
    }
}

#[must_use]
pub const fn embedded_catalog() -> EmbeddedCatalog {
    EmbeddedCatalog
}

#[derive(Clone, Copy)]
struct ModelRule {
    canonical_model: &'static str,
    aliases: &'static [&'static str],
    standard_short: PriceRates,
    standard_long: Option<PriceRates>,
    priority_short: Option<PriceRates>,
    priority_long: Option<PriceRates>,
}

const fn rate_micros(value: u64) -> UsdPerMillion {
    UsdPerMillion(value)
}

const fn rates(input: u64, cached: u64, output: u64) -> PriceRates {
    PriceRates {
        uncached_input: rate_micros(input),
        cached_input: rate_micros(cached),
        billable_output: rate_micros(output),
    }
}

const CATALOG: &[ModelRule] = &[
    ModelRule {
        canonical_model: "gpt-5.6-sol",
        aliases: &["gpt-5.6"],
        standard_short: rates(5_000_000, 500_000, 30_000_000),
        standard_long: Some(rates(10_000_000, 1_000_000, 45_000_000)),
        priority_short: Some(rates(10_000_000, 1_000_000, 60_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.6-terra",
        aliases: &[],
        standard_short: rates(2_500_000, 250_000, 15_000_000),
        standard_long: Some(rates(5_000_000, 500_000, 22_500_000)),
        priority_short: Some(rates(5_000_000, 500_000, 30_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.6-luna",
        aliases: &[],
        standard_short: rates(1_000_000, 100_000, 6_000_000),
        standard_long: Some(rates(2_000_000, 200_000, 9_000_000)),
        priority_short: Some(rates(2_000_000, 200_000, 12_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.5",
        aliases: &["gpt-5.5-2026-04-23"],
        standard_short: rates(5_000_000, 500_000, 30_000_000),
        standard_long: Some(rates(10_000_000, 1_000_000, 45_000_000)),
        priority_short: Some(rates(12_500_000, 1_250_000, 75_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.5-pro",
        aliases: &["gpt-5.5-pro-2026-04-23"],
        standard_short: rates(30_000_000, 30_000_000, 180_000_000),
        standard_long: None,
        priority_short: None,
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.4",
        aliases: &["gpt-5.4-2026-03-05"],
        standard_short: rates(2_500_000, 250_000, 15_000_000),
        standard_long: Some(rates(5_000_000, 500_000, 22_500_000)),
        priority_short: Some(rates(5_000_000, 500_000, 30_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.4-pro",
        aliases: &["gpt-5.4-pro-2026-03-05"],
        standard_short: rates(30_000_000, 30_000_000, 180_000_000),
        standard_long: Some(rates(60_000_000, 60_000_000, 270_000_000)),
        priority_short: None,
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.4-mini",
        aliases: &["gpt-5.4-mini-2026-03-17"],
        standard_short: rates(750_000, 75_000, 4_500_000),
        standard_long: None,
        priority_short: Some(rates(1_500_000, 150_000, 9_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.4-nano",
        aliases: &["gpt-5.4-nano-2026-03-17"],
        standard_short: rates(200_000, 20_000, 1_250_000),
        standard_long: None,
        priority_short: None,
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.3-codex",
        aliases: &[],
        standard_short: rates(1_750_000, 175_000, 14_000_000),
        standard_long: None,
        priority_short: None,
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.2",
        aliases: &["gpt-5.2-2025-12-11", "gpt-5.2-codex"],
        standard_short: rates(1_750_000, 175_000, 14_000_000),
        standard_long: None,
        priority_short: Some(rates(3_500_000, 350_000, 28_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5.1",
        aliases: &["gpt-5.1-codex", "gpt-5.1-codex-max"],
        standard_short: rates(1_250_000, 125_000, 10_000_000),
        standard_long: None,
        priority_short: Some(rates(2_500_000, 250_000, 20_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5",
        aliases: &["gpt-5-2025-08-07", "gpt-5-codex"],
        standard_short: rates(1_250_000, 125_000, 10_000_000),
        standard_long: None,
        priority_short: Some(rates(2_500_000, 250_000, 20_000_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5-mini",
        aliases: &["gpt-5-mini-2025-08-07"],
        standard_short: rates(250_000, 25_000, 2_000_000),
        standard_long: None,
        priority_short: Some(rates(450_000, 45_000, 3_600_000)),
        priority_long: None,
    },
    ModelRule {
        canonical_model: "gpt-5-nano",
        aliases: &["gpt-5-nano-2025-08-07"],
        standard_short: rates(50_000, 5_000, 400_000),
        standard_long: None,
        priority_short: None,
        priority_long: None,
    },
];

fn parse_ascii_u64(value: &str) -> Result<u64, PriceError> {
    let mut result = 0_u64;
    for byte in value.bytes() {
        result = result
            .checked_mul(10)
            .and_then(|current| current.checked_add(u64::from(byte - b'0')))
            .ok_or_else(|| PriceError::new(PriceErrorCode::InvalidRate))?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{CATALOG, MAX_RATE_MICROS_PER_MILLION};

    #[test]
    fn embedded_catalog_names_are_unique_and_rates_are_bounded() {
        let mut names = BTreeSet::new();
        for rule in CATALOG {
            assert!(names.insert(rule.canonical_model));
            for alias in rule.aliases {
                assert!(names.insert(alias));
            }
            for rates in [
                Some(rule.standard_short),
                rule.standard_long,
                rule.priority_short,
                rule.priority_long,
            ]
            .into_iter()
            .flatten()
            {
                assert!(rates.uncached_input.as_micros() <= MAX_RATE_MICROS_PER_MILLION);
                assert!(rates.cached_input.as_micros() <= MAX_RATE_MICROS_PER_MILLION);
                assert!(rates.billable_output.as_micros() <= MAX_RATE_MICROS_PER_MILLION);
            }
        }
    }
}
