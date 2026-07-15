use core::fmt;

use tokenmaster_accounting::CanonicalUsageEvent;
use tokenmaster_domain::{ObservationDraft, SessionRelationDraft};

use crate::{
    AdapterCheckpoint, AdapterCounters, AdapterDiagnostics, ChunkProofBatch, EngineError,
    EngineErrorCode, SourceIdentity,
};

pub const MAX_OBSERVATIONS_PER_BATCH: usize = 256;
pub const MAX_RELATIONS_PER_BATCH: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchState {
    More,
    SnapshotEnd,
}

pub struct AdapterBatchParts {
    pub observations: Box<[ObservationDraft]>,
    pub relations: Box<[SessionRelationDraft]>,
    pub chunk_proofs: ChunkProofBatch,
    pub next_checkpoint: AdapterCheckpoint,
    pub state: BatchState,
    pub counters: AdapterCounters,
    pub diagnostics: AdapterDiagnostics,
}

pub struct AdapterBatch {
    parts: AdapterBatchParts,
}

impl AdapterBatch {
    pub fn new(source: &SourceIdentity, parts: AdapterBatchParts) -> Result<Self, EngineError> {
        validate_batch_capacity(parts.observations.len(), parts.relations.len())?;
        if parts
            .observations
            .iter()
            .any(|observation| !observation_matches_source(observation, source))
            || parts
                .relations
                .iter()
                .any(|relation| !relation_matches_source(relation, source))
        {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        validate_counts(parts.observations.len(), parts.counters, &parts.diagnostics)?;
        Ok(Self { parts })
    }

    #[must_use]
    pub const fn observations(&self) -> &[ObservationDraft] {
        &self.parts.observations
    }

    #[must_use]
    pub const fn relations(&self) -> &[SessionRelationDraft] {
        &self.parts.relations
    }

    #[must_use]
    pub const fn chunk_proofs(&self) -> &ChunkProofBatch {
        &self.parts.chunk_proofs
    }

    #[must_use]
    pub const fn next_checkpoint(&self) -> &AdapterCheckpoint {
        &self.parts.next_checkpoint
    }

    #[must_use]
    pub const fn state(&self) -> BatchState {
        self.parts.state
    }

    #[must_use]
    pub const fn counters(&self) -> AdapterCounters {
        self.parts.counters
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &AdapterDiagnostics {
        &self.parts.diagnostics
    }

    #[must_use]
    pub fn into_parts(self) -> AdapterBatchParts {
        self.parts
    }
}

impl fmt::Debug for AdapterBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterBatch")
            .field("observation_count", &self.parts.observations.len())
            .field("relation_count", &self.parts.relations.len())
            .field(
                "chunk_proof_count",
                &self.parts.chunk_proofs.updates().len(),
            )
            .field(
                "checkpoint_byte_len",
                &self.parts.next_checkpoint.byte_len(),
            )
            .field("state", &self.parts.state)
            .field("counters", &self.parts.counters)
            .field("diagnostics", &self.parts.diagnostics)
            .finish()
    }
}

pub struct CanonicalBatchParts {
    pub events: Box<[CanonicalUsageEvent]>,
    pub relations: Box<[SessionRelationDraft]>,
    pub chunk_proofs: ChunkProofBatch,
    pub next_checkpoint: AdapterCheckpoint,
    pub state: BatchState,
    pub counters: AdapterCounters,
    pub diagnostics: AdapterDiagnostics,
}

pub struct CanonicalBatch {
    parts: CanonicalBatchParts,
}

impl CanonicalBatch {
    pub fn new(source: &SourceIdentity, parts: CanonicalBatchParts) -> Result<Self, EngineError> {
        validate_batch_capacity(parts.events.len(), parts.relations.len())?;
        if parts
            .events
            .iter()
            .any(|event| !event_matches_source(event, source))
            || parts
                .relations
                .iter()
                .any(|relation| !relation_matches_source(relation, source))
        {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        validate_counts(parts.events.len(), parts.counters, &parts.diagnostics)?;
        Ok(Self { parts })
    }

    #[must_use]
    pub const fn events(&self) -> &[CanonicalUsageEvent] {
        &self.parts.events
    }

    #[must_use]
    pub const fn relations(&self) -> &[SessionRelationDraft] {
        &self.parts.relations
    }

    #[must_use]
    pub const fn chunk_proofs(&self) -> &ChunkProofBatch {
        &self.parts.chunk_proofs
    }

    #[must_use]
    pub const fn next_checkpoint(&self) -> &AdapterCheckpoint {
        &self.parts.next_checkpoint
    }

    #[must_use]
    pub const fn state(&self) -> BatchState {
        self.parts.state
    }

    #[must_use]
    pub const fn counters(&self) -> AdapterCounters {
        self.parts.counters
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &AdapterDiagnostics {
        &self.parts.diagnostics
    }

    #[must_use]
    pub fn into_parts(self) -> CanonicalBatchParts {
        self.parts
    }
}

impl fmt::Debug for CanonicalBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalBatch")
            .field("event_count", &self.parts.events.len())
            .field("relation_count", &self.parts.relations.len())
            .field(
                "chunk_proof_count",
                &self.parts.chunk_proofs.updates().len(),
            )
            .field(
                "checkpoint_byte_len",
                &self.parts.next_checkpoint.byte_len(),
            )
            .field("state", &self.parts.state)
            .field("counters", &self.parts.counters)
            .field("diagnostics", &self.parts.diagnostics)
            .finish()
    }
}

fn validate_batch_capacity(event_count: usize, relation_count: usize) -> Result<(), EngineError> {
    if event_count > MAX_OBSERVATIONS_PER_BATCH || relation_count > MAX_RELATIONS_PER_BATCH {
        return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
    }
    Ok(())
}

fn validate_counts(
    event_count: usize,
    counters: AdapterCounters,
    diagnostics: &AdapterDiagnostics,
) -> Result<(), EngineError> {
    let event_count = u64::try_from(event_count)
        .map_err(|_| EngineError::new(EngineErrorCode::CapacityExceeded))?;
    if counters.events_observed() != event_count || counters.diagnostics() != diagnostics.total()? {
        return Err(EngineError::new(EngineErrorCode::InvalidValue));
    }
    Ok(())
}

fn observation_matches_source(observation: &ObservationDraft, source: &SourceIdentity) -> bool {
    observation.provider_id().as_str() == source.scope().provider_id()
        && observation.profile_id().as_str() == source.scope().profile_id()
        && observation.source_id().as_str() == source.source_id()
}

fn event_matches_source(event: &CanonicalUsageEvent, source: &SourceIdentity) -> bool {
    event.provider_id().as_str() == source.scope().provider_id()
        && event.profile_id().as_str() == source.scope().profile_id()
        && event.source_id().as_str() == source.source_id()
}

fn relation_matches_source(relation: &SessionRelationDraft, source: &SourceIdentity) -> bool {
    relation.provider_id().as_str() == source.scope().provider_id()
        && relation.profile_id().as_str() == source.scope().profile_id()
        && relation.source_id().as_str() == source.source_id()
}
