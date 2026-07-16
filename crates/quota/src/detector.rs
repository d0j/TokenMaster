use std::fmt;

use tokenmaster_domain::{
    QuotaConfidence, QuotaEvidenceSource, QuotaObservationId, QuotaProviderEpochId, QuotaRatio,
    QuotaResetEvidence, QuotaResetThresholds, QuotaSample, QuotaSampleQuality, QuotaUnits,
    QuotaWindowDefinition, QuotaWindowKey, QuotaWindowSemantics,
};

use crate::identity::{
    QuotaEpochId, QuotaTransitionId, TransitionIdentityInput, quota_epoch_id, quota_transition_id,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaErrorCode {
    UnexpectedPrevious,
    MissingPrevious,
    SampleWindowMismatch,
    StateWindowMismatch,
    PreviousWindowMismatch,
    StatePreviousMismatch,
    DefinitionRevisionRegressed,
    DuplicateConflict,
    InvalidTransitionSequence,
    TransitionSequenceOverflow,
    InvalidEpochState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotaError {
    code: QuotaErrorCode,
}

impl QuotaError {
    const fn new(code: QuotaErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> QuotaErrorCode {
        self.code
    }
}

impl fmt::Display for QuotaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            QuotaErrorCode::UnexpectedPrevious => {
                "previous sample is invalid without current state"
            }
            QuotaErrorCode::MissingPrevious => "current quota state requires its previous sample",
            QuotaErrorCode::SampleWindowMismatch => "quota sample window does not match definition",
            QuotaErrorCode::StateWindowMismatch => "quota state window does not match definition",
            QuotaErrorCode::PreviousWindowMismatch => {
                "previous quota sample window does not match definition"
            }
            QuotaErrorCode::StatePreviousMismatch => {
                "quota state does not match the supplied previous sample"
            }
            QuotaErrorCode::DefinitionRevisionRegressed => {
                "quota definition revision cannot regress"
            }
            QuotaErrorCode::DuplicateConflict => {
                "quota observation identity was reused with different content"
            }
            QuotaErrorCode::InvalidTransitionSequence => {
                "quota transition sequence is not the exact next value"
            }
            QuotaErrorCode::TransitionSequenceOverflow => {
                "quota transition sequence cannot advance"
            }
            QuotaErrorCode::InvalidEpochState => "restored quota epoch state is incoherent",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for QuotaError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaTransitionKind {
    ScheduledReset,
    EarlyReset,
    ManualOrBankedReset,
    UnknownReset,
    AllowanceChanged,
}

impl QuotaTransitionKind {
    const fn identity_code(self) -> u8 {
        match self {
            Self::ScheduledReset => 1,
            Self::EarlyReset => 2,
            Self::ManualOrBankedReset => 3,
            Self::UnknownReset => 4,
            Self::AllowanceChanged => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaAllowanceChangeKind {
    Increased,
    Decreased,
    UnitChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaAllowanceChange {
    kind: QuotaAllowanceChangeKind,
    old_units: QuotaUnits,
    new_units: QuotaUnits,
}

impl QuotaAllowanceChange {
    #[must_use]
    pub const fn kind(&self) -> QuotaAllowanceChangeKind {
        self.kind
    }

    #[must_use]
    pub const fn old_units(&self) -> &QuotaUnits {
        &self.old_units
    }

    #[must_use]
    pub const fn new_units(&self) -> &QuotaUnits {
        &self.new_units
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaDetectionTime {
    Exact(i64),
    Interval { after_ms: i64, at_or_before_ms: i64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaEpochStateParts {
    pub key: QuotaWindowKey,
    pub epoch_definition_revision: u64,
    pub definition_revision: u64,
    pub epoch_id: QuotaEpochId,
    pub first_observation_id: QuotaObservationId,
    pub last_observation_id: QuotaObservationId,
    pub first_observed_at_ms: i64,
    pub last_observed_at_ms: i64,
    pub maximum_used_ratio: Option<QuotaRatio>,
    pub maximum_used_ratio_observation_id: Option<QuotaObservationId>,
    pub maximum_used_units: Option<QuotaUnits>,
    pub maximum_used_units_observation_id: Option<QuotaObservationId>,
    pub provider_epoch_id: Option<QuotaProviderEpochId>,
    pub advertised_resets_at_ms: Option<i64>,
    pub last_transition_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaEpochState {
    key: QuotaWindowKey,
    epoch_definition_revision: u64,
    definition_revision: u64,
    epoch_id: QuotaEpochId,
    first_observation_id: QuotaObservationId,
    last_observation_id: QuotaObservationId,
    first_observed_at_ms: i64,
    last_observed_at_ms: i64,
    maximum_used_ratio: Option<QuotaRatio>,
    maximum_used_ratio_observation_id: Option<QuotaObservationId>,
    maximum_used_units: Option<QuotaUnits>,
    maximum_used_units_observation_id: Option<QuotaObservationId>,
    provider_epoch_id: Option<QuotaProviderEpochId>,
    advertised_resets_at_ms: Option<i64>,
    last_transition_sequence: u64,
}

impl QuotaEpochState {
    pub fn restore(parts: QuotaEpochStateParts) -> Result<Self, QuotaError> {
        if parts.epoch_definition_revision == 0
            || parts.definition_revision < parts.epoch_definition_revision
            || parts.first_observed_at_ms <= 0
            || parts.first_observed_at_ms > parts.last_observed_at_ms
            || parts
                .advertised_resets_at_ms
                .is_some_and(|value| value <= 0)
            || parts.maximum_used_ratio.is_some()
                != parts.maximum_used_ratio_observation_id.is_some()
            || parts.maximum_used_units.is_some()
                != parts.maximum_used_units_observation_id.is_some()
            || parts
                .maximum_used_units
                .as_ref()
                .is_some_and(|units| units.used().is_none())
            || parts.epoch_id
                != quota_epoch_id(
                    &parts.key,
                    parts.epoch_definition_revision,
                    parts.first_observation_id,
                )
        {
            return Err(QuotaError::new(QuotaErrorCode::InvalidEpochState));
        }
        Ok(Self {
            key: parts.key,
            epoch_definition_revision: parts.epoch_definition_revision,
            definition_revision: parts.definition_revision,
            epoch_id: parts.epoch_id,
            first_observation_id: parts.first_observation_id,
            last_observation_id: parts.last_observation_id,
            first_observed_at_ms: parts.first_observed_at_ms,
            last_observed_at_ms: parts.last_observed_at_ms,
            maximum_used_ratio: parts.maximum_used_ratio,
            maximum_used_ratio_observation_id: parts.maximum_used_ratio_observation_id,
            maximum_used_units: parts.maximum_used_units,
            maximum_used_units_observation_id: parts.maximum_used_units_observation_id,
            provider_epoch_id: parts.provider_epoch_id,
            advertised_resets_at_ms: parts.advertised_resets_at_ms,
            last_transition_sequence: parts.last_transition_sequence,
        })
    }

    #[must_use]
    pub fn to_parts(&self) -> QuotaEpochStateParts {
        QuotaEpochStateParts {
            key: self.key.clone(),
            epoch_definition_revision: self.epoch_definition_revision,
            definition_revision: self.definition_revision,
            epoch_id: self.epoch_id,
            first_observation_id: self.first_observation_id,
            last_observation_id: self.last_observation_id,
            first_observed_at_ms: self.first_observed_at_ms,
            last_observed_at_ms: self.last_observed_at_ms,
            maximum_used_ratio: self.maximum_used_ratio,
            maximum_used_ratio_observation_id: self.maximum_used_ratio_observation_id,
            maximum_used_units: self.maximum_used_units.clone(),
            maximum_used_units_observation_id: self.maximum_used_units_observation_id,
            provider_epoch_id: self.provider_epoch_id.clone(),
            advertised_resets_at_ms: self.advertised_resets_at_ms,
            last_transition_sequence: self.last_transition_sequence,
        }
    }

    #[must_use]
    pub const fn key(&self) -> &QuotaWindowKey {
        &self.key
    }

    #[must_use]
    pub const fn definition_revision(&self) -> u64 {
        self.definition_revision
    }

    #[must_use]
    pub const fn epoch_definition_revision(&self) -> u64 {
        self.epoch_definition_revision
    }

    #[must_use]
    pub const fn epoch_id(&self) -> QuotaEpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn first_observation_id(&self) -> QuotaObservationId {
        self.first_observation_id
    }

    #[must_use]
    pub const fn last_observation_id(&self) -> QuotaObservationId {
        self.last_observation_id
    }

    #[must_use]
    pub const fn first_observed_at_ms(&self) -> i64 {
        self.first_observed_at_ms
    }

    #[must_use]
    pub const fn last_observed_at_ms(&self) -> i64 {
        self.last_observed_at_ms
    }

    #[must_use]
    pub const fn maximum_used_ratio(&self) -> Option<QuotaRatio> {
        self.maximum_used_ratio
    }

    #[must_use]
    pub const fn maximum_used_ratio_observation_id(&self) -> Option<QuotaObservationId> {
        self.maximum_used_ratio_observation_id
    }

    #[must_use]
    pub const fn maximum_used_units(&self) -> Option<&QuotaUnits> {
        self.maximum_used_units.as_ref()
    }

    #[must_use]
    pub const fn maximum_used_units_observation_id(&self) -> Option<QuotaObservationId> {
        self.maximum_used_units_observation_id
    }

    #[must_use]
    pub const fn provider_epoch_id(&self) -> Option<&QuotaProviderEpochId> {
        self.provider_epoch_id.as_ref()
    }

    #[must_use]
    pub const fn advertised_resets_at_ms(&self) -> Option<i64> {
        self.advertised_resets_at_ms
    }

    #[must_use]
    pub const fn last_transition_sequence(&self) -> u64 {
        self.last_transition_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaTransition {
    id: QuotaTransitionId,
    sequence: u64,
    key: QuotaWindowKey,
    kind: QuotaTransitionKind,
    previous_epoch_id: QuotaEpochId,
    current_epoch_id: QuotaEpochId,
    pre_observation_id: QuotaObservationId,
    post_observation_id: QuotaObservationId,
    maximum_used_ratio_before: Option<QuotaRatio>,
    maximum_used_ratio_observation_id_before: Option<QuotaObservationId>,
    maximum_used_units_before: Option<QuotaUnits>,
    maximum_used_units_observation_id_before: Option<QuotaObservationId>,
    old_resets_at_ms: Option<i64>,
    new_resets_at_ms: Option<i64>,
    allowance_change: Option<QuotaAllowanceChange>,
    source: QuotaEvidenceSource,
    confidence: QuotaConfidence,
    detection_time: QuotaDetectionTime,
}

impl QuotaTransition {
    #[must_use]
    pub const fn id(&self) -> QuotaTransitionId {
        self.id
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn key(&self) -> &QuotaWindowKey {
        &self.key
    }

    #[must_use]
    pub const fn kind(&self) -> QuotaTransitionKind {
        self.kind
    }

    #[must_use]
    pub const fn previous_epoch_id(&self) -> QuotaEpochId {
        self.previous_epoch_id
    }

    #[must_use]
    pub const fn current_epoch_id(&self) -> QuotaEpochId {
        self.current_epoch_id
    }

    #[must_use]
    pub const fn pre_observation_id(&self) -> QuotaObservationId {
        self.pre_observation_id
    }

    #[must_use]
    pub const fn post_observation_id(&self) -> QuotaObservationId {
        self.post_observation_id
    }

    #[must_use]
    pub const fn maximum_used_ratio_before(&self) -> Option<QuotaRatio> {
        self.maximum_used_ratio_before
    }

    #[must_use]
    pub const fn maximum_used_ratio_observation_id_before(&self) -> Option<QuotaObservationId> {
        self.maximum_used_ratio_observation_id_before
    }

    #[must_use]
    pub const fn maximum_used_units_before(&self) -> Option<&QuotaUnits> {
        self.maximum_used_units_before.as_ref()
    }

    #[must_use]
    pub const fn maximum_used_units_observation_id_before(&self) -> Option<QuotaObservationId> {
        self.maximum_used_units_observation_id_before
    }

    #[must_use]
    pub const fn old_resets_at_ms(&self) -> Option<i64> {
        self.old_resets_at_ms
    }

    #[must_use]
    pub const fn new_resets_at_ms(&self) -> Option<i64> {
        self.new_resets_at_ms
    }

    #[must_use]
    pub const fn allowance_change(&self) -> Option<&QuotaAllowanceChange> {
        self.allowance_change.as_ref()
    }

    #[must_use]
    pub const fn source(&self) -> QuotaEvidenceSource {
        self.source
    }

    #[must_use]
    pub const fn confidence(&self) -> QuotaConfidence {
        self.confidence
    }

    #[must_use]
    pub const fn detection_time(&self) -> QuotaDetectionTime {
        self.detection_time
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuotaEvaluation {
    Started {
        state: QuotaEpochState,
    },
    Duplicate,
    Stale,
    Advanced {
        state: QuotaEpochState,
    },
    AllowanceChanged {
        state: QuotaEpochState,
        transition: QuotaTransition,
    },
    Reset {
        state: QuotaEpochState,
        transition: QuotaTransition,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResetSignal {
    ManualOrBanked,
    ProviderEpoch,
    Explicit,
    ThresholdAdvanced,
    ThresholdUnknown,
}

pub fn evaluate_sample(
    definition: &QuotaWindowDefinition,
    current: Option<&QuotaEpochState>,
    previous: Option<&QuotaSample>,
    sample: &QuotaSample,
    next_transition_sequence: u64,
) -> Result<QuotaEvaluation, QuotaError> {
    if sample.key() != definition.key() {
        return Err(QuotaError::new(QuotaErrorCode::SampleWindowMismatch));
    }
    let Some(current) = current else {
        if previous.is_some() {
            return Err(QuotaError::new(QuotaErrorCode::UnexpectedPrevious));
        }
        return Ok(QuotaEvaluation::Started {
            state: start_epoch(definition, sample),
        });
    };
    if current.key() != definition.key() {
        return Err(QuotaError::new(QuotaErrorCode::StateWindowMismatch));
    }
    if definition.revision() < current.definition_revision() {
        return Err(QuotaError::new(QuotaErrorCode::DefinitionRevisionRegressed));
    }
    let previous = previous.ok_or_else(|| QuotaError::new(QuotaErrorCode::MissingPrevious))?;
    if previous.key() != definition.key() {
        return Err(QuotaError::new(QuotaErrorCode::PreviousWindowMismatch));
    }
    if current.last_observation_id() != previous.observation_id()
        || current.last_observed_at_ms() != previous.observed_at_ms()
        || current.provider_epoch_id() != previous.provider_epoch_id()
        || current.advertised_resets_at_ms() != previous.advertised_resets_at_ms()
    {
        return Err(QuotaError::new(QuotaErrorCode::StatePreviousMismatch));
    }
    if sample.observation_id() == previous.observation_id() {
        return if sample == previous {
            Ok(QuotaEvaluation::Duplicate)
        } else {
            Err(QuotaError::new(QuotaErrorCode::DuplicateConflict))
        };
    }
    if sample.observed_at_ms() <= previous.observed_at_ms() {
        return Ok(QuotaEvaluation::Stale);
    }

    let allowance_change = detect_allowance_change(previous, sample);
    if let Some(signal) = detect_reset_signal(definition, previous, sample) {
        let sequence = exact_next_sequence(current, next_transition_sequence)?;
        let next_state = start_epoch_with_sequence(definition, sample, sequence);
        let kind = reset_kind(signal, previous, sample);
        let transition = make_transition(
            definition,
            current,
            &next_state,
            previous,
            sample,
            sequence,
            kind,
            allowance_change,
            reset_confidence(signal, sample),
            reset_detection_time(previous, sample),
        );
        return Ok(QuotaEvaluation::Reset {
            state: next_state,
            transition,
        });
    }

    let mut next_state = advance_epoch(definition, current, sample);
    if let Some(allowance_change) = allowance_change {
        let sequence = exact_next_sequence(current, next_transition_sequence)?;
        next_state.last_transition_sequence = sequence;
        let transition = make_transition(
            definition,
            current,
            &next_state,
            previous,
            sample,
            sequence,
            QuotaTransitionKind::AllowanceChanged,
            Some(allowance_change),
            sample.confidence(),
            QuotaDetectionTime::Interval {
                after_ms: previous.observed_at_ms(),
                at_or_before_ms: sample.observed_at_ms(),
            },
        );
        return Ok(QuotaEvaluation::AllowanceChanged {
            state: next_state,
            transition,
        });
    }
    Ok(QuotaEvaluation::Advanced { state: next_state })
}

fn start_epoch(definition: &QuotaWindowDefinition, sample: &QuotaSample) -> QuotaEpochState {
    start_epoch_with_sequence(definition, sample, 0)
}

fn start_epoch_with_sequence(
    definition: &QuotaWindowDefinition,
    sample: &QuotaSample,
    last_transition_sequence: u64,
) -> QuotaEpochState {
    let maximum_used_ratio = sample.used_ratio();
    let maximum_used_ratio_observation_id = maximum_used_ratio.map(|_| sample.observation_id());
    let maximum_used_units = sample
        .units()
        .filter(|units| units.used().is_some())
        .cloned();
    let maximum_used_units_observation_id =
        maximum_used_units.as_ref().map(|_| sample.observation_id());
    QuotaEpochState {
        key: sample.key().clone(),
        epoch_definition_revision: definition.revision(),
        definition_revision: definition.revision(),
        epoch_id: quota_epoch_id(sample.key(), definition.revision(), sample.observation_id()),
        first_observation_id: sample.observation_id(),
        last_observation_id: sample.observation_id(),
        first_observed_at_ms: sample.observed_at_ms(),
        last_observed_at_ms: sample.observed_at_ms(),
        maximum_used_ratio,
        maximum_used_ratio_observation_id,
        maximum_used_units,
        maximum_used_units_observation_id,
        provider_epoch_id: sample.provider_epoch_id().cloned(),
        advertised_resets_at_ms: sample.advertised_resets_at_ms(),
        last_transition_sequence,
    }
}

fn advance_epoch(
    definition: &QuotaWindowDefinition,
    current: &QuotaEpochState,
    sample: &QuotaSample,
) -> QuotaEpochState {
    let (maximum_used_ratio, maximum_used_ratio_observation_id) = maximum_ratio(
        current.maximum_used_ratio,
        current.maximum_used_ratio_observation_id,
        sample.used_ratio(),
        sample.observation_id(),
    );
    let (maximum_used_units, maximum_used_units_observation_id) = maximum_units(
        current.maximum_used_units.as_ref(),
        current.maximum_used_units_observation_id,
        sample.units(),
        sample.observation_id(),
    );
    QuotaEpochState {
        key: current.key.clone(),
        epoch_definition_revision: current.epoch_definition_revision,
        definition_revision: definition.revision(),
        epoch_id: current.epoch_id,
        first_observation_id: current.first_observation_id,
        last_observation_id: sample.observation_id(),
        first_observed_at_ms: current.first_observed_at_ms,
        last_observed_at_ms: sample.observed_at_ms(),
        maximum_used_ratio,
        maximum_used_ratio_observation_id,
        maximum_used_units,
        maximum_used_units_observation_id,
        provider_epoch_id: sample.provider_epoch_id().cloned(),
        advertised_resets_at_ms: sample.advertised_resets_at_ms(),
        last_transition_sequence: current.last_transition_sequence,
    }
}

fn maximum_ratio(
    current: Option<QuotaRatio>,
    current_observation_id: Option<QuotaObservationId>,
    sample: Option<QuotaRatio>,
    sample_observation_id: QuotaObservationId,
) -> (Option<QuotaRatio>, Option<QuotaObservationId>) {
    match (current, sample) {
        (Some(current), Some(sample))
            if sample.parts_per_million() > current.parts_per_million() =>
        {
            (Some(sample), Some(sample_observation_id))
        }
        (Some(current), _) => (Some(current), current_observation_id),
        (None, Some(sample)) => (Some(sample), Some(sample_observation_id)),
        (None, None) => (None, None),
    }
}

fn maximum_units(
    current: Option<&QuotaUnits>,
    current_observation_id: Option<QuotaObservationId>,
    sample: Option<&QuotaUnits>,
    sample_observation_id: QuotaObservationId,
) -> (Option<QuotaUnits>, Option<QuotaObservationId>) {
    let sample = sample.filter(|units| units.used().is_some());
    match (current, sample) {
        (Some(current), Some(sample)) if current.unit_id() == sample.unit_id() => {
            if sample.used() > current.used() {
                (Some(sample.clone()), Some(sample_observation_id))
            } else {
                (Some(current.clone()), current_observation_id)
            }
        }
        (Some(current), None) => (Some(current.clone()), current_observation_id),
        (None, Some(sample)) => (Some(sample.clone()), Some(sample_observation_id)),
        (Some(_), Some(_)) | (None, None) => (None, None),
    }
}

fn detect_reset_signal(
    definition: &QuotaWindowDefinition,
    previous: &QuotaSample,
    sample: &QuotaSample,
) -> Option<ResetSignal> {
    if sample.reset_evidence() == QuotaResetEvidence::ManualOrBanked {
        return Some(ResetSignal::ManualOrBanked);
    }
    if previous
        .provider_epoch_id()
        .zip(sample.provider_epoch_id())
        .is_some_and(|(previous, current)| previous != current)
    {
        return Some(ResetSignal::ProviderEpoch);
    }
    if matches!(
        sample.reset_evidence(),
        QuotaResetEvidence::ExplicitProvider | QuotaResetEvidence::ExplicitLocal
    ) {
        return Some(ResetSignal::Explicit);
    }
    if definition.semantics() != QuotaWindowSemantics::Fixed
        || matches!(
            sample.quality(),
            QuotaSampleQuality::Conflict | QuotaSampleQuality::Unknown
        )
        || matches!(
            sample.confidence(),
            QuotaConfidence::Low | QuotaConfidence::Unknown
        )
    {
        return None;
    }
    let thresholds = definition.reset_thresholds()?;
    if !threshold_transition(thresholds, previous, sample) {
        return None;
    }
    match (
        previous.advertised_resets_at_ms(),
        sample.advertised_resets_at_ms(),
    ) {
        (Some(previous), Some(current)) if current > previous => {
            Some(ResetSignal::ThresholdAdvanced)
        }
        (Some(_), Some(_)) => None,
        (None, _) | (_, None) => Some(ResetSignal::ThresholdUnknown),
    }
}

fn threshold_transition(
    thresholds: &QuotaResetThresholds,
    previous: &QuotaSample,
    sample: &QuotaSample,
) -> bool {
    let used_boundary = thresholds
        .maximum_post_reset_used_ratio()
        .is_none_or(|maximum| {
            sample
                .used_ratio()
                .is_some_and(|used| used.parts_per_million() <= maximum.parts_per_million())
        });
    let remaining_boundary =
        thresholds
            .minimum_post_reset_remaining_ratio()
            .is_none_or(|minimum| {
                sample.remaining_ratio().is_some_and(|remaining| {
                    remaining.parts_per_million() >= minimum.parts_per_million()
                })
            });
    let drop_boundary = thresholds.minimum_used_ratio_drop().is_none_or(|minimum| {
        previous
            .used_ratio()
            .zip(sample.used_ratio())
            .and_then(|(previous, current)| {
                previous
                    .parts_per_million()
                    .checked_sub(current.parts_per_million())
            })
            .is_some_and(|drop| drop >= minimum.parts_per_million())
    });
    let directional_recovery =
        previous
            .used_ratio()
            .zip(sample.used_ratio())
            .is_some_and(|(previous, current)| {
                current.parts_per_million() < previous.parts_per_million()
            })
            || previous
                .remaining_ratio()
                .zip(sample.remaining_ratio())
                .is_some_and(|(previous, current)| {
                    current.parts_per_million() > previous.parts_per_million()
                });
    used_boundary && remaining_boundary && drop_boundary && directional_recovery
}

fn detect_allowance_change(
    previous: &QuotaSample,
    sample: &QuotaSample,
) -> Option<QuotaAllowanceChange> {
    let (old_units, new_units) = previous.units().zip(sample.units())?;
    let (old_capacity, new_capacity) = old_units.capacity().zip(new_units.capacity())?;
    let kind = if old_units.unit_id() != new_units.unit_id() {
        QuotaAllowanceChangeKind::UnitChanged
    } else if new_capacity > old_capacity {
        QuotaAllowanceChangeKind::Increased
    } else if new_capacity < old_capacity {
        QuotaAllowanceChangeKind::Decreased
    } else {
        return None;
    };
    Some(QuotaAllowanceChange {
        kind,
        old_units: old_units.clone(),
        new_units: new_units.clone(),
    })
}

fn exact_next_sequence(
    current: &QuotaEpochState,
    next_transition_sequence: u64,
) -> Result<u64, QuotaError> {
    let expected = current
        .last_transition_sequence
        .checked_add(1)
        .ok_or_else(|| QuotaError::new(QuotaErrorCode::TransitionSequenceOverflow))?;
    if next_transition_sequence != expected {
        return Err(QuotaError::new(QuotaErrorCode::InvalidTransitionSequence));
    }
    Ok(expected)
}

fn reset_kind(
    signal: ResetSignal,
    previous: &QuotaSample,
    sample: &QuotaSample,
) -> QuotaTransitionKind {
    if signal == ResetSignal::ManualOrBanked {
        return QuotaTransitionKind::ManualOrBankedReset;
    }
    if signal == ResetSignal::ThresholdUnknown {
        return QuotaTransitionKind::UnknownReset;
    }
    let Some(boundary) = previous.advertised_resets_at_ms() else {
        return QuotaTransitionKind::UnknownReset;
    };
    let comparison_time = sample
        .reset_occurred_at_ms()
        .filter(|occurred| *occurred > previous.observed_at_ms())
        .unwrap_or_else(|| sample.observed_at_ms());
    if comparison_time >= boundary {
        QuotaTransitionKind::ScheduledReset
    } else {
        QuotaTransitionKind::EarlyReset
    }
}

fn reset_confidence(signal: ResetSignal, sample: &QuotaSample) -> QuotaConfidence {
    match signal {
        ResetSignal::ThresholdUnknown => QuotaConfidence::Low,
        ResetSignal::ThresholdAdvanced if sample.quality() == QuotaSampleQuality::Partial => {
            QuotaConfidence::Low
        }
        ResetSignal::ManualOrBanked
        | ResetSignal::ProviderEpoch
        | ResetSignal::Explicit
        | ResetSignal::ThresholdAdvanced => sample.confidence(),
    }
}

fn reset_detection_time(previous: &QuotaSample, sample: &QuotaSample) -> QuotaDetectionTime {
    match sample
        .reset_occurred_at_ms()
        .filter(|occurred| *occurred > previous.observed_at_ms())
    {
        Some(occurred_at_ms) => QuotaDetectionTime::Exact(occurred_at_ms),
        None => QuotaDetectionTime::Interval {
            after_ms: previous.observed_at_ms(),
            at_or_before_ms: sample.observed_at_ms(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn make_transition(
    definition: &QuotaWindowDefinition,
    previous_state: &QuotaEpochState,
    current_state: &QuotaEpochState,
    previous: &QuotaSample,
    sample: &QuotaSample,
    sequence: u64,
    kind: QuotaTransitionKind,
    allowance_change: Option<QuotaAllowanceChange>,
    confidence: QuotaConfidence,
    detection_time: QuotaDetectionTime,
) -> QuotaTransition {
    let id = quota_transition_id(TransitionIdentityInput {
        key: definition.key(),
        definition_revision: definition.revision(),
        sequence,
        kind_code: kind.identity_code(),
        previous_epoch_id: previous_state.epoch_id,
        current_epoch_id: current_state.epoch_id,
        pre_observation_id: previous.observation_id(),
        post_observation_id: sample.observation_id(),
    });
    QuotaTransition {
        id,
        sequence,
        key: definition.key().clone(),
        kind,
        previous_epoch_id: previous_state.epoch_id,
        current_epoch_id: current_state.epoch_id,
        pre_observation_id: previous.observation_id(),
        post_observation_id: sample.observation_id(),
        maximum_used_ratio_before: previous_state.maximum_used_ratio,
        maximum_used_ratio_observation_id_before: previous_state.maximum_used_ratio_observation_id,
        maximum_used_units_before: previous_state.maximum_used_units.clone(),
        maximum_used_units_observation_id_before: previous_state.maximum_used_units_observation_id,
        old_resets_at_ms: previous.advertised_resets_at_ms(),
        new_resets_at_ms: sample.advertised_resets_at_ms(),
        allowance_change,
        source: sample.source(),
        confidence,
        detection_time,
    }
}
