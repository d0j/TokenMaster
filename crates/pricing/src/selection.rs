use std::fmt;

use crate::{
    CATALOG_ID, ContextClass, OverrideRevision, PriceError, PriceErrorCode, PricingEngine,
    RuleSource, ServiceTier, TokenPriceBasis, UsdMicros, calculate_numerator, round_numerator,
};

pub const MAX_PRICE_BASIS_ROWS: usize = 512;
pub const MAX_MISSING_COST_DETAILS: usize = 32;

const TOKENS_PER_MILLION: u128 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CostMode {
    Auto,
    Calculated,
    Reported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CostAvailability {
    Complete,
    Partial,
    Unavailable,
    Zero,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CostComposition {
    None,
    Calculated,
    Reported,
    Mixed,
}

#[derive(Clone, Copy)]
pub struct PriceBasisRow<'a> {
    pub model: &'a str,
    pub tier: ServiceTier,
    pub context: ContextClass,
    pub event_count: u64,
    pub calculable_event_count: u64,
    pub basis: TokenPriceBasis,
    pub reported_event_count: u64,
    pub reported_cost: Option<UsdMicros>,
}

impl fmt::Debug for PriceBasisRow<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PriceBasisRow")
            .field("model", &"<redacted>")
            .field("tier", &self.tier)
            .field("context", &self.context)
            .field("event_count", &self.event_count)
            .field("calculable_event_count", &self.calculable_event_count)
            .field("reported_event_count", &self.reported_event_count)
            .field("has_reported_cost", &self.reported_cost.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MissingReasonCode {
    ModelUnpriced,
    TierUnknown,
    ContextUnavailable,
    TierContextUnsupported,
    TokenBasisUnavailable,
    ReportedCostMissing,
    KeyLimitReached,
    ArithmeticOverflow,
}

#[derive(Clone, Eq, PartialEq)]
pub struct MissingCost {
    reason: MissingReasonCode,
    key: Option<Box<str>>,
}

impl MissingCost {
    #[must_use]
    pub const fn reason(&self) -> MissingReasonCode {
        self.reason
    }

    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
}

impl fmt::Debug for MissingCost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MissingCost")
            .field("reason", &self.reason)
            .field("has_key", &self.key.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CostCounters {
    pub total_events: u64,
    pub priced_events: u64,
    pub reported_events: u64,
    pub assumed_events: u64,
    pub unpriced_events: u64,
    pub omitted_events: u64,
    pub conflict_events: u64,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CostResult {
    mode: CostMode,
    availability: CostAvailability,
    amount: Option<UsdMicros>,
    composition: CostComposition,
    counters: CostCounters,
    missing: Box<[MissingCost]>,
    omitted_missing_details: u64,
    calculated_catalog_id: Option<&'static str>,
    override_revision: Option<OverrideRevision>,
    used_override: bool,
}

impl fmt::Debug for CostResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CostResult")
            .field("mode", &self.mode)
            .field("availability", &self.availability)
            .field("amount", &self.amount)
            .field("composition", &self.composition)
            .field("counters", &self.counters)
            .field("missing_count", &self.missing.len())
            .field("omitted_missing_details", &self.omitted_missing_details)
            .field("calculated_catalog_id", &self.calculated_catalog_id)
            .field("override_revision", &self.override_revision)
            .field("used_override", &self.used_override)
            .finish()
    }
}

impl CostResult {
    #[must_use]
    pub const fn mode(&self) -> CostMode {
        self.mode
    }

    #[must_use]
    pub const fn availability(&self) -> CostAvailability {
        self.availability
    }

    #[must_use]
    pub const fn amount(&self) -> Option<UsdMicros> {
        self.amount
    }

    #[must_use]
    pub const fn composition(&self) -> CostComposition {
        self.composition
    }

    #[must_use]
    pub const fn counters(&self) -> CostCounters {
        self.counters
    }

    #[must_use]
    pub fn missing(&self) -> &[MissingCost] {
        &self.missing
    }

    #[must_use]
    pub const fn omitted_missing_details(&self) -> u64 {
        self.omitted_missing_details
    }

    #[must_use]
    pub const fn catalog_id(&self) -> Option<&'static str> {
        self.calculated_catalog_id
    }

    #[must_use]
    pub const fn override_revision(&self) -> Option<OverrideRevision> {
        self.override_revision
    }

    #[must_use]
    pub const fn used_override(&self) -> bool {
        self.used_override
    }
}

struct CalculatedRow {
    numerator: u128,
    rounded: Option<UsdMicros>,
    source: RuleSource,
    assumed_standard: bool,
}

pub fn select_cost(
    engine: &PricingEngine,
    mode: CostMode,
    rows: &[PriceBasisRow<'_>],
    omitted_event_count: u64,
) -> Result<CostResult, PriceError> {
    if rows.len() > MAX_PRICE_BASIS_ROWS {
        return Err(PriceError::new(PriceErrorCode::TooManyPriceRows));
    }

    let mut counters = CostCounters {
        omitted_events: omitted_event_count,
        total_events: omitted_event_count,
        ..CostCounters::default()
    };
    let mut selected_numerator = 0_u128;
    let mut missing = Vec::new();
    let mut used_calculated = false;
    let mut used_override = false;
    let mut numeric_failure = false;

    if omitted_event_count > 0 {
        missing.push(MissingCost {
            reason: MissingReasonCode::KeyLimitReached,
            key: None,
        });
    }

    for row in rows {
        validate_row(row)?;
        counters.total_events = checked_add(counters.total_events, row.event_count)?;

        let calculated = calculate_row(engine, row);
        if row.reported_event_count == row.event_count
            && row.calculable_event_count == row.event_count
            && let (Some(reported), Ok(calculated)) = (row.reported_cost, &calculated)
            && calculated
                .rounded
                .is_some_and(|amount| cost_values_conflict(reported, amount))
        {
            counters.conflict_events = checked_add(counters.conflict_events, row.event_count)?;
        }

        let select_reported =
            mode == CostMode::Reported || (mode == CostMode::Auto && row.reported_cost.is_some());
        if select_reported {
            if let Some(amount) = row.reported_cost {
                let numerator = u128::from(amount.get())
                    .checked_mul(TOKENS_PER_MILLION)
                    .ok_or_else(|| PriceError::new(PriceErrorCode::ArithmeticOverflow))?;
                if let Some(sum) = selected_numerator.checked_add(numerator) {
                    selected_numerator = sum;
                    counters.reported_events =
                        checked_add(counters.reported_events, row.reported_event_count)?;
                } else {
                    numeric_failure = true;
                }
            } else {
                counters.unpriced_events = checked_add(counters.unpriced_events, row.event_count)?;
                push_missing(
                    &mut missing,
                    MissingReasonCode::ReportedCostMissing,
                    Some(row.model),
                );
            }
            continue;
        }

        match calculated {
            Ok(calculated) => {
                if let Some(sum) = selected_numerator.checked_add(calculated.numerator) {
                    selected_numerator = sum;
                    counters.priced_events =
                        checked_add(counters.priced_events, row.calculable_event_count)?;
                    if calculated.assumed_standard {
                        counters.assumed_events =
                            checked_add(counters.assumed_events, row.calculable_event_count)?;
                    }
                    if row.calculable_event_count < row.event_count {
                        let unavailable = row.event_count - row.calculable_event_count;
                        counters.unpriced_events =
                            checked_add(counters.unpriced_events, unavailable)?;
                        push_missing(
                            &mut missing,
                            MissingReasonCode::TokenBasisUnavailable,
                            Some(row.model),
                        );
                    }
                    used_calculated = true;
                    used_override |= calculated.source == RuleSource::Override;
                } else {
                    numeric_failure = true;
                }
            }
            Err(error) => {
                counters.unpriced_events = checked_add(counters.unpriced_events, row.event_count)?;
                push_missing(&mut missing, missing_reason(error.code()), Some(row.model));
            }
        }
    }

    let selected_events = checked_add(counters.priced_events, counters.reported_events)?;
    let mut amount = if selected_events > 0 || counters.total_events == 0 {
        match round_numerator(selected_numerator) {
            Ok(amount) => Some(amount),
            Err(_) => {
                numeric_failure = true;
                None
            }
        }
    } else {
        None
    };

    if numeric_failure {
        push_missing(&mut missing, MissingReasonCode::ArithmeticOverflow, None);
        counters.priced_events = 0;
        counters.reported_events = 0;
        counters.assumed_events = 0;
        counters.unpriced_events = counters.total_events - counters.omitted_events;
        amount = None;
        used_calculated = false;
        used_override = false;
    }

    missing.sort_unstable_by(|left, right| {
        left.reason
            .cmp(&right.reason)
            .then_with(|| left.key.cmp(&right.key))
    });
    missing.dedup();
    let omitted_missing_details = missing.len().saturating_sub(MAX_MISSING_COST_DETAILS) as u64;
    missing.truncate(MAX_MISSING_COST_DETAILS);

    let selected_events = checked_add(counters.priced_events, counters.reported_events)?;
    let availability = if numeric_failure {
        CostAvailability::Unavailable
    } else if counters.total_events == 0 {
        CostAvailability::Zero
    } else if selected_events == 0 {
        CostAvailability::Unavailable
    } else if counters.unpriced_events == 0 && counters.omitted_events == 0 {
        if amount.is_some_and(|value| value.get() == 0) {
            CostAvailability::Zero
        } else {
            CostAvailability::Complete
        }
    } else {
        CostAvailability::Partial
    };
    let composition = match (counters.priced_events > 0, counters.reported_events > 0) {
        (false, false) => CostComposition::None,
        (true, false) => CostComposition::Calculated,
        (false, true) => CostComposition::Reported,
        (true, true) => CostComposition::Mixed,
    };

    Ok(CostResult {
        mode,
        availability,
        amount,
        composition,
        counters,
        missing: missing.into_boxed_slice(),
        omitted_missing_details,
        calculated_catalog_id: used_calculated.then_some(CATALOG_ID),
        override_revision: used_calculated.then_some(engine.override_revision()),
        used_override,
    })
}

fn validate_row(row: &PriceBasisRow<'_>) -> Result<(), PriceError> {
    let reported_is_coherent = match (row.reported_event_count, row.reported_cost) {
        (0, None) => true,
        (count, Some(_)) => count == row.event_count,
        _ => false,
    };
    if !crate::overrides::is_valid_model_key(row.model)
        || row.event_count == 0
        || row.calculable_event_count > row.event_count
        || !reported_is_coherent
    {
        return Err(PriceError::new(PriceErrorCode::InvalidPriceRow));
    }
    Ok(())
}

fn calculate_row(
    engine: &PricingEngine,
    row: &PriceBasisRow<'_>,
) -> Result<CalculatedRow, PriceError> {
    if row.calculable_event_count == 0 {
        return Err(PriceError::new(PriceErrorCode::TokenBasisUnavailable));
    }
    let resolved = engine.resolve(row.model, row.tier, row.context)?;
    let numerator = calculate_numerator(resolved.rates, row.basis)?;
    Ok(CalculatedRow {
        numerator,
        rounded: round_numerator(numerator).ok(),
        source: resolved.source,
        assumed_standard: resolved.assumed_standard,
    })
}

fn checked_add(left: u64, right: u64) -> Result<u64, PriceError> {
    left.checked_add(right)
        .ok_or_else(|| PriceError::new(PriceErrorCode::ArithmeticOverflow))
}

fn push_missing(missing: &mut Vec<MissingCost>, reason: MissingReasonCode, key: Option<&str>) {
    missing.push(MissingCost {
        reason,
        key: key.map(|value| value.to_owned().into_boxed_str()),
    });
}

fn missing_reason(code: PriceErrorCode) -> MissingReasonCode {
    match code {
        PriceErrorCode::ModelUnpriced => MissingReasonCode::ModelUnpriced,
        PriceErrorCode::TierUnknown => MissingReasonCode::TierUnknown,
        PriceErrorCode::ContextUnavailable => MissingReasonCode::ContextUnavailable,
        PriceErrorCode::TierContextUnsupported => MissingReasonCode::TierContextUnsupported,
        PriceErrorCode::TokenBasisUnavailable | PriceErrorCode::InconsistentTokenBasis => {
            MissingReasonCode::TokenBasisUnavailable
        }
        PriceErrorCode::InvalidRate
        | PriceErrorCode::ArithmeticOverflow
        | PriceErrorCode::InvalidPriceRow
        | PriceErrorCode::TooManyPriceRows => MissingReasonCode::ArithmeticOverflow,
    }
}

#[must_use]
pub fn cost_values_conflict(left: UsdMicros, right: UsdMicros) -> bool {
    let left = left.get();
    let right = right.get();
    let delta = left.abs_diff(right);
    let larger = left.max(right);
    delta > 10_000 && u128::from(delta) * 100 > u128::from(larger) * 2
}
