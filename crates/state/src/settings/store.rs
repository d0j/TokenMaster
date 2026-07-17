use core::fmt;

use serde::Serialize;
use tokenmaster_platform::ValidatedLocalDirectory;

use super::preview::{
    PortableSettingsCandidate, PortableSettingsDigest, PortableSettingsTarget,
    SettingsImportPreview,
};
use super::value::SettingsValue;
use crate::StateError;
use crate::record::{
    MAX_RECORD_PAYLOAD_BYTES, RecordKind, RecordLoad, RecordRedundancy, RedundantRecordStore,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsLoadOutcome {
    Current,
    Fallback,
    Defaults,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsHealthCode {
    Healthy,
    FallbackCorruptSlot,
    DefaultsNoValidRecord,
}

impl SettingsHealthCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::FallbackCorruptSlot => "fallback_corrupt_slot",
            Self::DefaultsNoValidRecord => "defaults_no_valid_record",
        }
    }
}

pub struct SettingsLoad {
    value: SettingsValue,
    outcome: SettingsLoadOutcome,
    health_code: SettingsHealthCode,
    generation: Option<u64>,
    record_digest: Option<[u8; 32]>,
}

impl SettingsLoad {
    #[must_use]
    pub const fn value(&self) -> &SettingsValue {
        &self.value
    }

    #[must_use]
    pub const fn outcome(&self) -> SettingsLoadOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn health_code(&self) -> SettingsHealthCode {
        self.health_code
    }

    #[must_use]
    pub const fn generation(&self) -> Option<u64> {
        self.generation
    }
}

impl fmt::Debug for SettingsLoad {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SettingsLoad")
            .field("outcome", &self.outcome)
            .field("health_code", &self.health_code)
            .field("generation", &self.generation)
            .finish()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SettingsCommitReceipt {
    generation: u64,
    portable_digest: PortableSettingsDigest,
}

impl SettingsCommitReceipt {
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn portable_digest(self) -> PortableSettingsDigest {
        self.portable_digest
    }

    #[must_use]
    pub const fn target(self) -> PortableSettingsTarget {
        PortableSettingsTarget::new(self.generation, self.portable_digest)
    }
}

impl fmt::Debug for SettingsCommitReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SettingsCommitReceipt([redacted])")
    }
}

pub struct SettingsStore {
    records: RedundantRecordStore<SettingsValue>,
}

impl SettingsStore {
    pub fn new(directory: &ValidatedLocalDirectory) -> Result<Self, StateError> {
        Ok(Self {
            records: RedundantRecordStore::new(
                directory,
                RecordKind::Settings,
                MAX_RECORD_PAYLOAD_BYTES,
            )?,
        })
    }

    pub fn load(&self) -> Result<SettingsLoad, StateError> {
        Ok(match self.records.load()? {
            RecordLoad::Loaded(record) => {
                let (outcome, health_code) = match record.redundancy() {
                    RecordRedundancy::Complete | RecordRedundancy::Single => {
                        (SettingsLoadOutcome::Current, SettingsHealthCode::Healthy)
                    }
                    RecordRedundancy::Fallback => (
                        SettingsLoadOutcome::Fallback,
                        SettingsHealthCode::FallbackCorruptSlot,
                    ),
                };
                let generation = record.generation();
                let record_digest = record.payload_sha256();
                SettingsLoad {
                    value: record.into_value(),
                    outcome,
                    health_code,
                    generation: Some(generation),
                    record_digest: Some(record_digest),
                }
            }
            RecordLoad::NoValidRecord => SettingsLoad {
                value: SettingsValue::safe_defaults(),
                outcome: SettingsLoadOutcome::Defaults,
                health_code: SettingsHealthCode::DefaultsNoValidRecord,
                generation: None,
                record_digest: None,
            },
        })
    }

    pub fn save(&self, value: &SettingsValue) -> Result<SettingsCommitReceipt, StateError> {
        let candidate = PortableSettingsCandidate::new(value.portable().clone())?;
        let receipt = self.records.save_explicit(value)?;
        let reread = self.load().map_err(|_| StateError::recovery_required())?;
        if reread.generation() != Some(receipt.generation())
            || reread.record_digest != Some(receipt.payload_sha256())
            || reread.value() != value
        {
            return Err(StateError::recovery_required());
        }
        Ok(SettingsCommitReceipt {
            generation: receipt.generation(),
            portable_digest: candidate.digest(),
        })
    }

    pub fn preview_import(&self, bytes: &[u8]) -> Result<SettingsImportPreview, StateError> {
        let candidate = PortableSettingsCandidate::decode(bytes)?;
        let current = self.load()?;
        Ok(SettingsImportPreview::new(
            current.generation,
            current.record_digest,
            current.value.portable(),
            candidate,
        ))
    }

    pub fn commit_import(
        &self,
        preview: &SettingsImportPreview,
    ) -> Result<SettingsCommitReceipt, StateError> {
        let current = self.load()?;
        if current.generation != preview.base_generation
            || current.record_digest != preview.base_record_digest
        {
            return Err(StateError::integrity());
        }
        if current.generation.is_some() && current.value.portable() == preview.candidate.portable()
        {
            return Ok(SettingsCommitReceipt {
                generation: current
                    .generation
                    .ok_or_else(StateError::internal_invariant)?,
                portable_digest: preview.candidate.digest(),
            });
        }
        let value = SettingsValue::new(
            preview.candidate.portable().clone(),
            current.value.device().clone(),
        );
        let receipt = self.save(&value)?;
        if receipt.portable_digest() != preview.candidate.digest() {
            return Err(StateError::recovery_required());
        }
        Ok(receipt)
    }

    pub fn full_backup_candidate(&self) -> Result<PortableSettingsCandidate, StateError> {
        PortableSettingsCandidate::new(self.load()?.value.portable().clone())
    }

    pub fn verify_target(&self, target: PortableSettingsTarget) -> Result<bool, StateError> {
        let current = self.load()?;
        if current.generation != Some(target.generation()) {
            return Ok(false);
        }
        Ok(
            PortableSettingsCandidate::new(current.value.portable().clone())?.digest()
                == target.digest(),
        )
    }
}

impl fmt::Debug for SettingsStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SettingsStore([redacted])")
    }
}
