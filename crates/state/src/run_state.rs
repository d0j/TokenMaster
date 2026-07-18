use core::fmt;

use serde::{Deserialize, Serialize};
use tokenmaster_platform::ValidatedLocalDirectory;

use crate::record::{
    MAX_RECORD_PAYLOAD_BYTES, RecordKind, RecordLoad, RecordRedundancy, RecordValue,
    RecordValueError, RedundantRecordStore,
};
use crate::{RecoveryCandidateIdentity, StateError};

const RUN_STATE_SCHEMA_VERSION: u16 = 1;
const MAX_RECOVERY_LAUNCHES: u8 = 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RunPhase {
    Clean,
    Unclean,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryLaunchState {
    operation_generation: u64,
    candidate: RecoveryCandidateIdentity,
    launches: u8,
}

impl RecoveryLaunchState {
    fn validate(self) -> Result<(), StateError> {
        RecoveryCandidateIdentity::from_persisted(
            self.candidate.schema_version(),
            self.candidate.len(),
            *self.candidate.sha256(),
        )?;
        if self.operation_generation == 0 || !(1..=MAX_RECOVERY_LAUNCHES).contains(&self.launches) {
            return Err(StateError::invalid_input());
        }
        Ok(())
    }
}

impl fmt::Debug for RecoveryLaunchState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryLaunchState")
            .field("launches", &self.launches)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RunStateValue {
    schema_version: u16,
    phase: RunPhase,
    last_recovery_generation: Option<u64>,
    recovery: Option<RecoveryLaunchState>,
}

impl RunStateValue {
    const fn unclean(
        last_recovery_generation: Option<u64>,
        recovery: Option<RecoveryLaunchState>,
    ) -> Self {
        Self {
            schema_version: RUN_STATE_SCHEMA_VERSION,
            phase: RunPhase::Unclean,
            last_recovery_generation,
            recovery,
        }
    }

    const fn clean(last_recovery_generation: Option<u64>) -> Self {
        Self {
            schema_version: RUN_STATE_SCHEMA_VERSION,
            phase: RunPhase::Clean,
            last_recovery_generation,
            recovery: None,
        }
    }

    fn validate(self) -> Result<(), StateError> {
        if self.schema_version != RUN_STATE_SCHEMA_VERSION
            || (self.phase == RunPhase::Clean && self.recovery.is_some())
            || self.last_recovery_generation == Some(0)
        {
            return Err(StateError::invalid_input());
        }
        if let Some(recovery) = self.recovery {
            recovery.validate()?;
            if self
                .last_recovery_generation
                .is_some_and(|generation| generation >= recovery.operation_generation)
            {
                return Err(StateError::invalid_input());
            }
        }
        Ok(())
    }
}

impl RecordValue for RunStateValue {
    fn decode_json(bytes: &[u8]) -> Result<Self, RecordValueError> {
        let value: Self = serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
        if value.schema_version != RUN_STATE_SCHEMA_VERSION {
            return Err(RecordValueError::UnsupportedVersion);
        }
        value.validate().map_err(|_| RecordValueError::Invalid)?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PriorRunCondition {
    Clean,
    Unclean,
    Missing,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunStateInspection {
    condition: PriorRunCondition,
    recovery_launches: Option<u8>,
    last_recovery_generation: Option<u64>,
}

impl RunStateInspection {
    #[must_use]
    pub const fn condition(self) -> PriorRunCondition {
        self.condition
    }

    #[must_use]
    pub const fn requires_quick_check(self) -> bool {
        !matches!(self.condition, PriorRunCondition::Clean)
    }

    #[must_use]
    pub const fn recovery_launches(self) -> Option<u8> {
        self.recovery_launches
    }

    #[must_use]
    pub const fn last_recovery_generation(self) -> Option<u64> {
        self.last_recovery_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryLaunchDecision {
    NotTracked,
    AlreadyAccepted { operation_generation: u64 },
    Start { launch: u8 },
    SafeMode { failed_launches: u8 },
}

struct LoadedRunState {
    inspection: RunStateInspection,
    generation: Option<u64>,
    payload_sha256: Option<[u8; 32]>,
    value: Option<RunStateValue>,
}

#[derive(Clone)]
pub struct RunStateStore {
    records: RedundantRecordStore<RunStateValue>,
}

impl RunStateStore {
    pub fn new(directory: &ValidatedLocalDirectory) -> Result<Self, StateError> {
        Ok(Self {
            records: RedundantRecordStore::new(
                directory,
                RecordKind::RunState,
                MAX_RECORD_PAYLOAD_BYTES,
            )?,
        })
    }

    pub fn inspect(&self) -> Result<RunStateInspection, StateError> {
        self.load().map(|loaded| loaded.inspection)
    }

    pub(crate) fn authorize_directory(
        &self,
        directory: &ValidatedLocalDirectory,
    ) -> Result<(), StateError> {
        self.records
            .authorize_directory(directory, RecordKind::RunState)
    }

    pub fn begin(&self) -> Result<RunSession, StateError> {
        let prior = self.load()?;
        let last_recovery_generation = prior.value.and_then(|value| value.last_recovery_generation);
        let recovery = prior.value.and_then(|value| {
            (value.phase == RunPhase::Unclean)
                .then_some(value.recovery)
                .flatten()
        });
        let receipt = self
            .records
            .save_explicit(&RunStateValue::unclean(last_recovery_generation, recovery))?;
        Ok(RunSession {
            store: self.clone(),
            prior: prior.inspection,
            generation: receipt.generation(),
            payload_sha256: receipt.payload_sha256(),
            recovery,
            last_recovery_generation,
            launch_authorized: false,
        })
    }

    fn load(&self) -> Result<LoadedRunState, StateError> {
        match self.records.load()? {
            RecordLoad::Loaded(record) => {
                let generation = record.generation();
                let payload_sha256 = record.payload_sha256();
                let redundancy = record.redundancy();
                let value = record.into_value();
                let condition = if redundancy == RecordRedundancy::Fallback {
                    PriorRunCondition::Invalid
                } else {
                    match value.phase {
                        RunPhase::Clean => PriorRunCondition::Clean,
                        RunPhase::Unclean => PriorRunCondition::Unclean,
                    }
                };
                Ok(LoadedRunState {
                    inspection: RunStateInspection {
                        condition,
                        recovery_launches: value.recovery.map(|recovery| recovery.launches),
                        last_recovery_generation: value.last_recovery_generation,
                    },
                    generation: Some(generation),
                    payload_sha256: Some(payload_sha256),
                    value: Some(value),
                })
            }
            RecordLoad::NoValidRecord => {
                let condition = if self.records.has_any_artifact()? {
                    PriorRunCondition::Invalid
                } else {
                    PriorRunCondition::Missing
                };
                Ok(LoadedRunState {
                    inspection: RunStateInspection {
                        condition,
                        recovery_launches: None,
                        last_recovery_generation: None,
                    },
                    generation: None,
                    payload_sha256: None,
                    value: None,
                })
            }
        }
    }
}

impl fmt::Debug for RunStateStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RunStateStore([redacted])")
    }
}

pub struct RunSession {
    store: RunStateStore,
    prior: RunStateInspection,
    generation: u64,
    payload_sha256: [u8; 32],
    recovery: Option<RecoveryLaunchState>,
    last_recovery_generation: Option<u64>,
    launch_authorized: bool,
}

impl RunSession {
    #[must_use]
    pub const fn prior(&self) -> RunStateInspection {
        self.prior
    }

    #[must_use]
    pub const fn current_generation(&self) -> u64 {
        self.generation
    }

    /// Records that startup validation completed and application owners may start.
    /// Clean publication remains a separate post-join action.
    pub fn authorize_healthy_launch(&mut self) {
        self.launch_authorized = true;
    }

    pub fn start_recovered_candidate(
        &mut self,
        operation_generation: u64,
        candidate: RecoveryCandidateIdentity,
    ) -> Result<RecoveryLaunchDecision, StateError> {
        if operation_generation == 0 {
            return Err(StateError::invalid_input());
        }
        if let Some(recovery) = self.recovery {
            if recovery.operation_generation == operation_generation {
                if recovery.candidate != candidate {
                    return Err(StateError::integrity());
                }
                return self.start_candidate(recovery, recovery.launches);
            }
            if operation_generation < recovery.operation_generation {
                return Err(StateError::integrity());
            }
        }
        if let Some(accepted) = self.last_recovery_generation {
            if operation_generation < accepted {
                return Err(StateError::integrity());
            }
            if operation_generation == accepted {
                self.launch_authorized = true;
                return Ok(RecoveryLaunchDecision::AlreadyAccepted {
                    operation_generation,
                });
            }
        }
        self.start_candidate(
            RecoveryLaunchState {
                operation_generation,
                candidate,
                launches: 1,
            },
            0,
        )
    }

    pub fn continue_recovered_candidate(&mut self) -> Result<RecoveryLaunchDecision, StateError> {
        let Some(recovery) = self.recovery else {
            return Ok(RecoveryLaunchDecision::NotTracked);
        };
        self.start_candidate(recovery, recovery.launches)
    }

    pub fn mark_clean(&mut self) -> Result<(), StateError> {
        if !self.launch_authorized {
            return Err(StateError::invalid_input());
        }
        let accepted = self
            .recovery
            .map(|recovery| recovery.operation_generation)
            .or(self.last_recovery_generation);
        self.publish(RunStateValue::clean(accepted))?;
        self.launch_authorized = false;
        Ok(())
    }

    fn start_candidate(
        &mut self,
        mut recovery: RecoveryLaunchState,
        completed_launches: u8,
    ) -> Result<RecoveryLaunchDecision, StateError> {
        if completed_launches >= MAX_RECOVERY_LAUNCHES {
            return Ok(RecoveryLaunchDecision::SafeMode {
                failed_launches: completed_launches,
            });
        }
        let launch = completed_launches
            .checked_add(1)
            .ok_or_else(StateError::capacity_exceeded)?;
        recovery.launches = launch;
        self.publish(RunStateValue::unclean(
            self.last_recovery_generation,
            Some(recovery),
        ))?;
        self.launch_authorized = true;
        Ok(RecoveryLaunchDecision::Start { launch })
    }

    fn publish(&mut self, value: RunStateValue) -> Result<(), StateError> {
        let current = self.store.load()?;
        if current.generation != Some(self.generation)
            || current.payload_sha256 != Some(self.payload_sha256)
            || current.value.map(|current| current.phase) != Some(RunPhase::Unclean)
        {
            return Err(StateError::integrity());
        }
        let receipt = self.store.records.save_explicit(&value)?;
        self.generation = receipt.generation();
        self.payload_sha256 = receipt.payload_sha256();
        self.recovery = value.recovery;
        self.last_recovery_generation = value.last_recovery_generation;
        Ok(())
    }
}

impl fmt::Debug for RunSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunSession")
            .field("prior", &self.prior)
            .field("generation", &self.generation)
            .field(
                "recovery_launches",
                &self.recovery.map(|recovery| recovery.launches),
            )
            .field("launch_authorized", &self.launch_authorized)
            .finish()
    }
}
