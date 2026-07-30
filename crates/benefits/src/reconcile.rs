use std::collections::BTreeMap;
use std::fmt;

use tokenmaster_domain::{
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitLotId, BenefitLotObservation,
    BenefitScope, BenefitState, MAX_BENEFIT_LOTS_PER_OBSERVATION,
};

use crate::identity::{BenefitChangeId, BenefitScopeId, benefit_change_id, benefit_scope_id};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitCoreError {
    ScopeMismatch,
    ConflictingObservationIdentity,
    InvalidRevision,
    InvalidSequence,
    CapacityExceeded,
    InvalidTime,
}

impl fmt::Display for BenefitCoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ScopeMismatch => "benefit scope mismatch",
            Self::ConflictingObservationIdentity => "conflicting benefit observation identity",
            Self::InvalidRevision => "invalid benefit revision",
            Self::InvalidSequence => "invalid benefit sequence",
            Self::CapacityExceeded => "benefit capacity exceeded",
            Self::InvalidTime => "invalid benefit time",
        })
    }
}

impl std::error::Error for BenefitCoreError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BenefitRevision(u64);

impl BenefitRevision {
    pub fn new(value: u64) -> Result<Self, BenefitCoreError> {
        if value > i64::MAX as u64 {
            return Err(BenefitCoreError::InvalidRevision);
        }
        Ok(Self(value))
    }

    pub(crate) fn next(self) -> Result<Self, BenefitCoreError> {
        Self::new(
            self.0
                .checked_add(1)
                .ok_or(BenefitCoreError::InvalidRevision)?,
        )
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BenefitSequence(u64);

impl BenefitSequence {
    pub fn new(value: u64) -> Result<Self, BenefitCoreError> {
        if value > i64::MAX as u64 {
            return Err(BenefitCoreError::InvalidSequence);
        }
        Ok(Self(value))
    }

    fn next(self) -> Result<Self, BenefitCoreError> {
        Self::new(
            self.0
                .checked_add(1)
                .ok_or(BenefitCoreError::InvalidSequence)?,
        )
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitCurrentLot {
    lot: BenefitLotObservation,
    revision: BenefitRevision,
}

impl BenefitCurrentLot {
    pub fn new(
        lot: BenefitLotObservation,
        revision: BenefitRevision,
    ) -> Result<Self, BenefitCoreError> {
        if revision.get() == 0 {
            return Err(BenefitCoreError::InvalidRevision);
        }
        Ok(Self { lot, revision })
    }

    #[must_use]
    pub const fn lot(&self) -> &BenefitLotObservation {
        &self.lot
    }

    #[must_use]
    pub const fn revision(&self) -> BenefitRevision {
        self.revision
    }
}

impl fmt::Debug for BenefitCurrentLot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitCurrentLot")
            .field("lot", &self.lot)
            .field("revision", &self.revision)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitInventoryState {
    scope: BenefitScope,
    scope_id: BenefitScopeId,
    revision: BenefitRevision,
    last_change_sequence: BenefitSequence,
    last_observation_id: Option<tokenmaster_domain::BenefitObservationId>,
    last_observed_at_ms: Option<i64>,
    lots: Box<[BenefitCurrentLot]>,
}

impl BenefitInventoryState {
    #[must_use]
    pub fn empty(scope: BenefitScope) -> Self {
        let scope_id = benefit_scope_id(&scope);
        Self {
            scope,
            scope_id,
            revision: BenefitRevision(0),
            last_change_sequence: BenefitSequence(0),
            last_observation_id: None,
            last_observed_at_ms: None,
            lots: Box::new([]),
        }
    }

    pub fn from_parts(
        scope: BenefitScope,
        revision: BenefitRevision,
        last_change_sequence: BenefitSequence,
        last_observation_id: Option<tokenmaster_domain::BenefitObservationId>,
        last_observed_at_ms: Option<i64>,
        lots: Vec<BenefitCurrentLot>,
    ) -> Result<Self, BenefitCoreError> {
        if last_observed_at_ms.is_some_and(|value| value <= 0)
            || last_observation_id.is_some() != last_observed_at_ms.is_some()
            || lots.len() > MAX_BENEFIT_LOTS_PER_OBSERVATION
        {
            return Err(if lots.len() > MAX_BENEFIT_LOTS_PER_OBSERVATION {
                BenefitCoreError::CapacityExceeded
            } else {
                BenefitCoreError::InvalidTime
            });
        }
        let mut keyed = BTreeMap::new();
        for lot in lots {
            if keyed.insert(*lot.lot().lot_id().as_bytes(), lot).is_some() {
                return Err(BenefitCoreError::CapacityExceeded);
            }
        }
        Ok(Self {
            scope_id: benefit_scope_id(&scope),
            scope,
            revision,
            last_change_sequence,
            last_observation_id,
            last_observed_at_ms,
            lots: keyed.into_values().collect::<Vec<_>>().into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn scope(&self) -> &BenefitScope {
        &self.scope
    }

    #[must_use]
    pub const fn scope_id(&self) -> BenefitScopeId {
        self.scope_id
    }

    #[must_use]
    pub const fn revision(&self) -> BenefitRevision {
        self.revision
    }

    #[must_use]
    pub const fn last_change_sequence(&self) -> BenefitSequence {
        self.last_change_sequence
    }

    #[must_use]
    pub const fn last_observed_at_ms(&self) -> Option<i64> {
        self.last_observed_at_ms
    }

    #[must_use]
    pub const fn lots(&self) -> &[BenefitCurrentLot] {
        &self.lots
    }
}

impl fmt::Debug for BenefitInventoryState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitInventoryState")
            .field("scope", &"[redacted]")
            .field("scope_id", &self.scope_id)
            .field("revision", &self.revision)
            .field("last_change_sequence", &self.last_change_sequence)
            .field("last_observation_id", &self.last_observation_id)
            .field("last_observed_at_ms", &self.last_observed_at_ms)
            .field("lot_count", &self.lots.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitChangeKind {
    Awarded,
    QuantityChanged,
    StateChanged,
    ExpiryChanged,
    Corrected,
    DisappearedAmbiguous,
    Reappeared,
    RetiredTerminal,
}

impl BenefitChangeKind {
    const fn code(self) -> u8 {
        match self {
            Self::Awarded => 1,
            Self::QuantityChanged => 2,
            Self::StateChanged => 3,
            Self::ExpiryChanged => 4,
            Self::Corrected => 5,
            Self::DisappearedAmbiguous => 6,
            Self::Reappeared => 7,
            Self::RetiredTerminal => 8,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BenefitChange {
    id: BenefitChangeId,
    sequence: BenefitSequence,
    lot_id: BenefitLotId,
    lot_revision: BenefitRevision,
    kind: BenefitChangeKind,
    before: Option<BenefitLotObservation>,
    after: Option<BenefitLotObservation>,
}

impl BenefitChange {
    #[must_use]
    pub const fn id(&self) -> BenefitChangeId {
        self.id
    }

    #[must_use]
    pub const fn sequence(&self) -> BenefitSequence {
        self.sequence
    }

    #[must_use]
    pub const fn lot_id(&self) -> BenefitLotId {
        self.lot_id
    }

    #[must_use]
    pub const fn lot_revision(&self) -> BenefitRevision {
        self.lot_revision
    }

    #[must_use]
    pub const fn kind(&self) -> BenefitChangeKind {
        self.kind
    }

    #[must_use]
    pub const fn before(&self) -> Option<&BenefitLotObservation> {
        self.before.as_ref()
    }

    #[must_use]
    pub const fn after(&self) -> Option<&BenefitLotObservation> {
        self.after.as_ref()
    }
}

impl fmt::Debug for BenefitChange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BenefitChange")
            .field("id", &self.id)
            .field("sequence", &self.sequence)
            .field("lot_id", &self.lot_id)
            .field("lot_revision", &self.lot_revision)
            .field("kind", &self.kind)
            .field("has_before", &self.before.is_some())
            .field("has_after", &self.after.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenefitReconciliationStatus {
    Duplicate,
    Stale,
    FreshnessOnly,
    Changed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenefitReconciliation {
    status: BenefitReconciliationStatus,
    state: BenefitInventoryState,
    changes: Box<[BenefitChange]>,
}

impl BenefitReconciliation {
    #[must_use]
    pub const fn status(&self) -> BenefitReconciliationStatus {
        self.status
    }

    #[must_use]
    pub const fn state(&self) -> &BenefitInventoryState {
        &self.state
    }

    #[must_use]
    pub const fn changes(&self) -> &[BenefitChange] {
        &self.changes
    }
}

pub fn reconcile_inventory(
    current: &BenefitInventoryState,
    observation: &BenefitInventoryObservation,
) -> Result<BenefitReconciliation, BenefitCoreError> {
    if current.scope() != observation.scope() {
        return Err(BenefitCoreError::ScopeMismatch);
    }
    if current
        .last_observed_at_ms
        .is_some_and(|last| observation.observed_at_ms() < last)
    {
        return Ok(BenefitReconciliation {
            status: BenefitReconciliationStatus::Stale,
            state: current.clone(),
            changes: Box::new([]),
        });
    }

    let mut prior = current
        .lots
        .iter()
        .cloned()
        .map(|lot| (*lot.lot().lot_id().as_bytes(), lot))
        .collect::<BTreeMap<_, _>>();
    let mut next = BTreeMap::new();
    let mut pending = Vec::new();
    let mut observed = observation.lots().to_vec();
    observed.sort_unstable_by_key(|lot| *lot.lot_id().as_bytes());

    for lot in observed {
        let key = *lot.lot_id().as_bytes();
        match prior.remove(&key) {
            None => {
                let revision = BenefitRevision::new(1)?;
                next.insert(key, BenefitCurrentLot::new(lot.clone(), revision)?);
                pending.push((
                    lot.lot_id(),
                    revision,
                    BenefitChangeKind::Awarded,
                    None,
                    Some(lot),
                ));
            }
            Some(previous) if previous.lot() == &lot => {
                next.insert(key, previous);
            }
            Some(previous) => {
                let revision = previous.revision().next()?;
                let kind = classify_change(previous.lot(), &lot);
                next.insert(key, BenefitCurrentLot::new(lot.clone(), revision)?);
                pending.push((
                    lot.lot_id(),
                    revision,
                    kind,
                    Some(previous.lot().clone()),
                    Some(lot),
                ));
            }
        }
    }

    let complete = observation.completeness() != BenefitInventoryCompleteness::Partial;
    for (key, previous) in prior {
        if !complete {
            next.insert(key, previous);
            continue;
        }
        if is_terminal(previous.lot().state()) {
            let revision = previous.revision().next()?;
            pending.push((
                previous.lot().lot_id(),
                revision,
                BenefitChangeKind::RetiredTerminal,
                Some(previous.lot().clone()),
                None,
            ));
        } else if previous.lot().state() == BenefitState::Ambiguous {
            next.insert(key, previous);
        } else {
            let mut parts = previous.lot().clone().into_parts();
            parts.state = BenefitState::Ambiguous;
            let ambiguous = BenefitLotObservation::new(parts)
                .map_err(|_| BenefitCoreError::CapacityExceeded)?;
            let revision = previous.revision().next()?;
            next.insert(key, BenefitCurrentLot::new(ambiguous.clone(), revision)?);
            pending.push((
                ambiguous.lot_id(),
                revision,
                BenefitChangeKind::DisappearedAmbiguous,
                Some(previous.lot().clone()),
                Some(ambiguous),
            ));
        }
    }

    if next.len() > MAX_BENEFIT_LOTS_PER_OBSERVATION {
        return Err(BenefitCoreError::CapacityExceeded);
    }

    let same_identity = current.last_observation_id == Some(observation.observation_id());
    if same_identity {
        if pending.is_empty() {
            return Ok(BenefitReconciliation {
                status: BenefitReconciliationStatus::Duplicate,
                state: current.clone(),
                changes: Box::new([]),
            });
        }
        return Err(BenefitCoreError::ConflictingObservationIdentity);
    }

    let mut sequence = current.last_change_sequence;
    let mut changes = Vec::with_capacity(pending.len());
    for (lot_id, lot_revision, kind, before, after) in pending {
        sequence = sequence.next()?;
        changes.push(BenefitChange {
            id: benefit_change_id(
                current.scope_id,
                sequence.get(),
                lot_id,
                lot_revision,
                kind.code(),
            ),
            sequence,
            lot_id,
            lot_revision,
            kind,
            before,
            after,
        });
    }

    let status = if changes.is_empty() {
        BenefitReconciliationStatus::FreshnessOnly
    } else {
        BenefitReconciliationStatus::Changed
    };
    let revision = current.revision.next()?;
    let state = BenefitInventoryState {
        scope: current.scope.clone(),
        scope_id: current.scope_id,
        revision,
        last_change_sequence: sequence,
        last_observation_id: Some(observation.observation_id()),
        last_observed_at_ms: Some(observation.observed_at_ms()),
        lots: next.into_values().collect::<Vec<_>>().into_boxed_slice(),
    };
    Ok(BenefitReconciliation {
        status,
        state,
        changes: changes.into_boxed_slice(),
    })
}

fn classify_change(
    before: &BenefitLotObservation,
    after: &BenefitLotObservation,
) -> BenefitChangeKind {
    if before.state() == BenefitState::Ambiguous && after.state() != BenefitState::Ambiguous {
        return BenefitChangeKind::Reappeared;
    }
    let quantity_changed = before.quantity() != after.quantity();
    let state_changed = before.state() != after.state();
    let expiry_changed = before.expiry() != after.expiry();
    let other_changed = before.kind() != after.kind()
        || before.target() != after.target()
        || before.granted_at_ms() != after.granted_at_ms()
        || before.source() != after.source()
        || before.confidence() != after.confidence()
        || before.detail_kind() != after.detail_kind()
        || before.label_key() != after.label_key();
    match (
        quantity_changed,
        state_changed,
        expiry_changed,
        other_changed,
    ) {
        (true, false, false, false) => BenefitChangeKind::QuantityChanged,
        (false, true, false, false) => BenefitChangeKind::StateChanged,
        (false, false, true, false) => BenefitChangeKind::ExpiryChanged,
        _ => BenefitChangeKind::Corrected,
    }
}

const fn is_terminal(state: BenefitState) -> bool {
    matches!(
        state,
        BenefitState::Activated | BenefitState::Expired | BenefitState::Revoked
    )
}
