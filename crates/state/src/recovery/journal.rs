use core::fmt;

use serde::{Deserialize, Serialize};
use tokenmaster_platform::{
    ArchiveRecoveryError, ArchiveSetExpectation, ArchiveSetObservation, RecoveryOperationId,
    ValidatedLocalDirectory,
};

use crate::record::{
    MAX_RECORD_PAYLOAD_BYTES, RecordKind, RecordLoad, RecordSaveBoundary, RecordSaveHook,
    RecordValue, RecordValueError, RedundantRecordStore,
};
use crate::{PortableSettingsTarget, StateError};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPhase {
    Prepared,
    SidecarsQuarantined,
    MainReplaced,
    ReopenedVerified,
    SettingsPublished,
    Complete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoverySettingsMode {
    DataOnly,
    DataAndPortableSettings,
    AutomaticDataOnly,
    ReconstructionDataOnly,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryFileFact {
    len: u64,
    sha256: [u8; 32],
}

impl RecoveryFileFact {
    pub fn from_persisted(len: u64, sha256: [u8; 32]) -> Result<Self, StateError> {
        if sha256 == [0_u8; 32] {
            return Err(StateError::invalid_input());
        }
        Ok(Self { len, sha256 })
    }

    #[must_use]
    pub const fn len(self) -> u64 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

impl fmt::Debug for RecoveryFileFact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryFileFact([redacted])")
    }
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryArchiveFacts {
    main: Option<RecoveryFileFact>,
    wal: Option<RecoveryFileFact>,
    shm: Option<RecoveryFileFact>,
}

impl RecoveryArchiveFacts {
    pub fn from_observation(observation: ArchiveSetObservation) -> Result<Self, StateError> {
        Self::from_persisted(
            observation
                .main()
                .map(|receipt| (receipt.len(), *receipt.sha256())),
            observation
                .wal()
                .map(|receipt| (receipt.len(), *receipt.sha256())),
            observation
                .shm()
                .map(|receipt| (receipt.len(), *receipt.sha256())),
        )
    }

    pub fn from_persisted(
        main: Option<(u64, [u8; 32])>,
        wal: Option<(u64, [u8; 32])>,
        shm: Option<(u64, [u8; 32])>,
    ) -> Result<Self, StateError> {
        if main.is_some_and(|(len, _)| len == 0) {
            return Err(StateError::invalid_input());
        }
        Ok(Self {
            main: decode_file_fact(main)?,
            wal: decode_file_fact(wal)?,
            shm: decode_file_fact(shm)?,
        })
    }

    #[must_use]
    pub const fn main(self) -> Option<RecoveryFileFact> {
        self.main
    }

    #[must_use]
    pub const fn wal(self) -> Option<RecoveryFileFact> {
        self.wal
    }

    #[must_use]
    pub const fn shm(self) -> Option<RecoveryFileFact> {
        self.shm
    }

    #[must_use]
    pub const fn has_any_archive_artifact(self) -> bool {
        self.main.is_some() || self.wal.is_some() || self.shm.is_some()
    }

    pub fn to_platform(self) -> Result<ArchiveSetExpectation, StateError> {
        ArchiveSetExpectation::from_persisted(
            self.main.map(|fact| (fact.len, fact.sha256)),
            self.wal.map(|fact| (fact.len, fact.sha256)),
            self.shm.map(|fact| (fact.len, fact.sha256)),
        )
        .map_err(map_archive_expectation_error)
    }

    fn validate(self) -> Result<(), StateError> {
        if self.main.is_some_and(RecoveryFileFact::is_empty) {
            return Err(StateError::invalid_input());
        }
        for fact in [self.main, self.wal, self.shm].into_iter().flatten() {
            RecoveryFileFact::from_persisted(fact.len, fact.sha256)?;
        }
        Ok(())
    }
}

impl fmt::Debug for RecoveryArchiveFacts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryArchiveFacts")
            .field("main_present", &self.main.is_some())
            .field("wal_present", &self.wal.is_some())
            .field("shm_present", &self.shm.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryBackupIdentity {
    backup_slot: u8,
    package_len: u64,
    package_sha256: [u8; 32],
}

impl RecoveryBackupIdentity {
    pub fn from_persisted(
        backup_slot: u8,
        package_len: u64,
        package_sha256: [u8; 32],
    ) -> Result<Self, StateError> {
        if package_len == 0 || package_sha256 == [0_u8; 32] {
            return Err(StateError::invalid_input());
        }
        Ok(Self {
            backup_slot,
            package_len,
            package_sha256,
        })
    }

    #[must_use]
    pub const fn backup_slot(self) -> u8 {
        self.backup_slot
    }

    #[must_use]
    pub const fn package_len(self) -> u64 {
        self.package_len
    }

    #[must_use]
    pub const fn package_sha256(&self) -> &[u8; 32] {
        &self.package_sha256
    }

    fn validate(self) -> Result<(), StateError> {
        Self::from_persisted(self.backup_slot, self.package_len, self.package_sha256).map(|_| ())
    }
}

impl fmt::Debug for RecoveryBackupIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryBackupIdentity([redacted])")
    }
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCandidateIdentity {
    schema_version: u32,
    len: u64,
    sha256: [u8; 32],
}

impl RecoveryCandidateIdentity {
    pub fn from_persisted(
        schema_version: u32,
        len: u64,
        sha256: [u8; 32],
    ) -> Result<Self, StateError> {
        if schema_version == 0 || len == 0 || sha256 == [0_u8; 32] {
            return Err(StateError::invalid_input());
        }
        Ok(Self {
            schema_version,
            len,
            sha256,
        })
    }

    #[must_use]
    pub const fn schema_version(self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn len(self) -> u64 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    fn validate(self) -> Result<(), StateError> {
        Self::from_persisted(self.schema_version, self.len, self.sha256).map(|_| ())
    }
}

impl fmt::Debug for RecoveryCandidateIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryCandidateIdentity([redacted])")
    }
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySettingsTarget {
    generation: u64,
    digest: [u8; 32],
}

impl RecoverySettingsTarget {
    #[must_use]
    pub fn from_portable(target: PortableSettingsTarget) -> Self {
        Self {
            generation: target.generation(),
            digest: *target.digest().as_bytes(),
        }
    }

    pub fn to_portable(self) -> Result<PortableSettingsTarget, StateError> {
        PortableSettingsTarget::from_persisted(self.generation, self.digest)
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    fn validate(self) -> Result<(), StateError> {
        self.to_portable().map(|_| ())
    }
}

impl fmt::Debug for RecoverySettingsTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoverySettingsTarget([redacted])")
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryJournal {
    schema_version: u16,
    operation_generation: u64,
    operation_id: [u8; 16],
    #[serde(default)]
    backup: Option<RecoveryBackupIdentity>,
    candidate: RecoveryCandidateIdentity,
    before: RecoveryArchiveFacts,
    settings_mode: RecoverySettingsMode,
    settings_target: Option<RecoverySettingsTarget>,
    attempt: u8,
    phase: RecoveryPhase,
}

impl RecoveryJournal {
    #[must_use]
    pub const fn operation_generation(&self) -> u64 {
        self.operation_generation
    }

    #[allow(clippy::too_many_arguments)]
    pub fn manual(
        operation_generation: u64,
        operation_id: RecoveryOperationId,
        backup: RecoveryBackupIdentity,
        candidate: RecoveryCandidateIdentity,
        before: RecoveryArchiveFacts,
        settings_target: Option<PortableSettingsTarget>,
        attempt: u8,
    ) -> Result<Self, StateError> {
        let (settings_mode, settings_target) = match settings_target {
            Some(target) => (
                RecoverySettingsMode::DataAndPortableSettings,
                Some(RecoverySettingsTarget::from_portable(target)),
            ),
            None => (RecoverySettingsMode::DataOnly, None),
        };
        Self::new(
            operation_generation,
            operation_id,
            Some(backup),
            candidate,
            before,
            settings_mode,
            settings_target,
            attempt,
        )
    }

    pub fn automatic(
        operation_generation: u64,
        operation_id: RecoveryOperationId,
        backup: RecoveryBackupIdentity,
        candidate: RecoveryCandidateIdentity,
        before: RecoveryArchiveFacts,
        attempt: u8,
    ) -> Result<Self, StateError> {
        Self::new(
            operation_generation,
            operation_id,
            Some(backup),
            candidate,
            before,
            RecoverySettingsMode::AutomaticDataOnly,
            None,
            attempt,
        )
    }

    pub fn reconstruction(
        operation_generation: u64,
        operation_id: RecoveryOperationId,
        candidate: RecoveryCandidateIdentity,
        before: RecoveryArchiveFacts,
    ) -> Result<Self, StateError> {
        Self::new(
            operation_generation,
            operation_id,
            None,
            candidate,
            before,
            RecoverySettingsMode::ReconstructionDataOnly,
            None,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        operation_generation: u64,
        operation_id: RecoveryOperationId,
        backup: Option<RecoveryBackupIdentity>,
        candidate: RecoveryCandidateIdentity,
        before: RecoveryArchiveFacts,
        settings_mode: RecoverySettingsMode,
        settings_target: Option<RecoverySettingsTarget>,
        attempt: u8,
    ) -> Result<Self, StateError> {
        if attempt > 2 {
            return Err(StateError::capacity_exceeded());
        }
        let value = Self {
            schema_version: 1,
            operation_generation,
            operation_id: operation_id.to_persisted_bytes(),
            backup,
            candidate,
            before,
            settings_mode,
            settings_target,
            attempt,
            phase: RecoveryPhase::Prepared,
        };
        value.validate()?;
        Ok(value)
    }

    #[must_use]
    pub const fn phase(&self) -> RecoveryPhase {
        self.phase
    }

    #[must_use]
    pub const fn settings_mode(&self) -> RecoverySettingsMode {
        self.settings_mode
    }

    #[must_use]
    pub const fn settings_target(&self) -> Option<RecoverySettingsTarget> {
        self.settings_target
    }

    #[must_use]
    pub const fn operation_id(&self) -> [u8; 16] {
        self.operation_id
    }

    #[must_use]
    pub const fn candidate(&self) -> RecoveryCandidateIdentity {
        self.candidate
    }

    #[must_use]
    pub const fn backup(&self) -> Option<RecoveryBackupIdentity> {
        self.backup
    }

    #[must_use]
    pub const fn before(&self) -> RecoveryArchiveFacts {
        self.before
    }

    fn validate(&self) -> Result<(), StateError> {
        if self.schema_version != 1
            || self.operation_generation == 0
            || self.operation_id == [0_u8; 16]
            || !(1..=2).contains(&self.attempt)
        {
            return Err(StateError::invalid_input());
        }
        if let Some(backup) = self.backup {
            backup.validate()?;
        }
        self.candidate.validate()?;
        self.before.validate()?;
        match (self.backup, self.settings_mode, self.settings_target) {
            (Some(_), RecoverySettingsMode::DataAndPortableSettings, Some(target)) => {
                target.validate()
            }
            (
                Some(_),
                RecoverySettingsMode::DataOnly | RecoverySettingsMode::AutomaticDataOnly,
                None,
            )
            | (None, RecoverySettingsMode::ReconstructionDataOnly, None) => Ok(()),
            _ => Err(StateError::invalid_input()),
        }
    }
}

impl fmt::Debug for RecoveryJournal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryJournal([redacted])")
    }
}

impl RecordValue for RecoveryJournal {
    fn decode_json(bytes: &[u8]) -> Result<Self, RecordValueError> {
        let value: Self = serde_json::from_slice(bytes).map_err(|_| RecordValueError::Invalid)?;
        if value.schema_version != 1 {
            return Err(RecordValueError::UnsupportedVersion);
        }
        value.validate().map_err(|_| RecordValueError::Invalid)?;
        Ok(value)
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "the fixed bounded journal avoids a heap allocation on every load"
)]
pub enum RecoveryJournalLoad {
    Absent,
    Invalid,
    Pending(RecoveryJournal),
}

impl fmt::Debug for RecoveryJournalLoad {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => formatter.write_str("RecoveryJournalLoad::Absent"),
            Self::Invalid => formatter.write_str("RecoveryJournalLoad::Invalid"),
            Self::Pending(value) => formatter
                .debug_tuple("RecoveryJournalLoad::Pending")
                .field(value)
                .finish(),
        }
    }
}

pub struct RecoveryJournalStore {
    records: RedundantRecordStore<RecoveryJournal>,
}

impl RecoveryJournalStore {
    pub fn new(directory: &ValidatedLocalDirectory) -> Result<Self, StateError> {
        Ok(Self {
            records: RedundantRecordStore::new(
                directory,
                RecordKind::RecoveryJournal,
                MAX_RECORD_PAYLOAD_BYTES,
            )?,
        })
    }

    pub fn load(&self) -> Result<RecoveryJournalLoad, StateError> {
        match self.records.load()? {
            RecordLoad::Loaded(record) => Ok(RecoveryJournalLoad::Pending(record.into_value())),
            RecordLoad::NoValidRecord if self.records.has_any_artifact()? => {
                Ok(RecoveryJournalLoad::Invalid)
            }
            RecordLoad::NoValidRecord => Ok(RecoveryJournalLoad::Absent),
        }
    }

    pub(crate) fn authorize_directory(
        &self,
        directory: &ValidatedLocalDirectory,
    ) -> Result<(), StateError> {
        self.records
            .authorize_directory(directory, RecordKind::RecoveryJournal)
    }

    pub fn begin(&self, prepared: &RecoveryJournal) -> Result<(), StateError> {
        self.begin_with_observer(prepared, || Ok(()))
    }

    #[doc(hidden)]
    pub fn begin_with_observer<F>(
        &self,
        prepared: &RecoveryJournal,
        mut observer: F,
    ) -> Result<(), StateError>
    where
        F: FnMut() -> Result<(), StateError>,
    {
        if prepared.phase != RecoveryPhase::Prepared {
            return Err(StateError::invalid_input());
        }
        match self.load()? {
            RecoveryJournalLoad::Absent if prepared.operation_generation == 1 => Ok(()),
            RecoveryJournalLoad::Pending(loaded) if loaded == *prepared => Ok(()),
            RecoveryJournalLoad::Pending(loaded)
                if loaded.phase == RecoveryPhase::Complete
                    && loaded.operation_generation.checked_add(1)
                        == Some(prepared.operation_generation) =>
            {
                Ok(())
            }
            RecoveryJournalLoad::Invalid => Err(StateError::recovery_required()),
            RecoveryJournalLoad::Absent | RecoveryJournalLoad::Pending(_) => {
                Err(StateError::integrity())
            }
        }?;
        let mut hook = FirstPublicationHook {
            observer: &mut observer,
        };
        self.records.save_explicit_with_hook(prepared, &mut hook)?;
        self.records.save_explicit(prepared)?;
        match self.load()? {
            RecoveryJournalLoad::Pending(loaded) if loaded == *prepared => Ok(()),
            _ => Err(StateError::recovery_required()),
        }
    }

    pub(crate) fn next_operation_generation(&self) -> Result<u64, StateError> {
        match self.load()? {
            RecoveryJournalLoad::Absent => Ok(1),
            RecoveryJournalLoad::Pending(journal) if journal.phase == RecoveryPhase::Complete => {
                journal
                    .operation_generation
                    .checked_add(1)
                    .ok_or_else(StateError::capacity_exceeded)
            }
            RecoveryJournalLoad::Pending(_) | RecoveryJournalLoad::Invalid => {
                Err(StateError::recovery_required())
            }
        }
    }

    pub fn advance(
        &self,
        current: &RecoveryJournal,
        next: RecoveryPhase,
    ) -> Result<RecoveryJournal, StateError> {
        let loaded = match self.load()? {
            RecoveryJournalLoad::Pending(loaded) => loaded,
            RecoveryJournalLoad::Absent => return Err(StateError::integrity()),
            RecoveryJournalLoad::Invalid => return Err(StateError::recovery_required()),
        };
        if loaded != *current {
            return Err(StateError::integrity());
        }
        if next == current.phase {
            return Ok(current.clone());
        }
        if next_phase(current.phase) != Some(next) {
            return Err(StateError::integrity());
        }
        let mut advanced = current.clone();
        advanced.phase = next;
        self.records.save_explicit(&advanced)?;
        match self.load()? {
            RecoveryJournalLoad::Pending(loaded) if loaded == advanced => Ok(advanced),
            _ => Err(StateError::recovery_required()),
        }
    }
}

struct FirstPublicationHook<'a, F> {
    observer: &'a mut F,
}

impl<F> RecordSaveHook for FirstPublicationHook<'_, F>
where
    F: FnMut() -> Result<(), StateError>,
{
    fn hit(&mut self, boundary: RecordSaveBoundary) -> Result<(), StateError> {
        if boundary == RecordSaveBoundary::AfterPublication {
            (self.observer)()?;
        }
        Ok(())
    }
}

impl fmt::Debug for RecoveryJournalStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryJournalStore([redacted])")
    }
}

const fn next_phase(phase: RecoveryPhase) -> Option<RecoveryPhase> {
    match phase {
        RecoveryPhase::Prepared => Some(RecoveryPhase::SidecarsQuarantined),
        RecoveryPhase::SidecarsQuarantined => Some(RecoveryPhase::MainReplaced),
        RecoveryPhase::MainReplaced => Some(RecoveryPhase::ReopenedVerified),
        RecoveryPhase::ReopenedVerified => Some(RecoveryPhase::SettingsPublished),
        RecoveryPhase::SettingsPublished => Some(RecoveryPhase::Complete),
        RecoveryPhase::Complete => None,
    }
}

fn decode_file_fact(
    value: Option<(u64, [u8; 32])>,
) -> Result<Option<RecoveryFileFact>, StateError> {
    value
        .map(|(len, digest)| RecoveryFileFact::from_persisted(len, digest))
        .transpose()
}

const fn map_archive_expectation_error(error: ArchiveRecoveryError) -> StateError {
    match error {
        ArchiveRecoveryError::CapacityExceeded | ArchiveRecoveryError::CollisionLimit => {
            StateError::capacity_exceeded()
        }
        ArchiveRecoveryError::DiskCapacity => {
            StateError::from_code(crate::StateErrorCode::DiskCapacity)
        }
        ArchiveRecoveryError::ArtifactMismatch | ArchiveRecoveryError::UnexpectedArtifact => {
            StateError::integrity()
        }
        ArchiveRecoveryError::RecoveryRequired => StateError::recovery_required(),
        ArchiveRecoveryError::InvalidState => StateError::internal_invariant(),
        ArchiveRecoveryError::UnsupportedLocation
        | ArchiveRecoveryError::WrongLease
        | ArchiveRecoveryError::Unavailable => StateError::unavailable(),
    }
}
