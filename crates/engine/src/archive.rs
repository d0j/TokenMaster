use crate::{
    AdapterCompletion, AdapterSourceState, CanonicalBatch, CompletionQuality, DiscoveredSource,
    EngineError, EngineErrorCode, PortError, ScopeIdentity, ScopeManifest, SourceIdentity,
};

macro_rules! nonzero_archive_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(value: u64) -> Result<Self, EngineError> {
                if value == 0 {
                    return Err(EngineError::new(EngineErrorCode::InvalidValue));
                }
                Ok(Self(value))
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

nonzero_archive_id!(ArchiveScanSetId);
nonzero_archive_id!(ArchiveRevisionId);
nonzero_archive_id!(ArchiveEpoch);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveReplay {
    revision_id: ArchiveRevisionId,
    epoch: ArchiveEpoch,
}

impl ArchiveReplay {
    #[must_use]
    pub const fn new(revision_id: ArchiveRevisionId, epoch: ArchiveEpoch) -> Self {
        Self { revision_id, epoch }
    }

    #[must_use]
    pub const fn revision_id(self) -> ArchiveRevisionId {
        self.revision_id
    }

    #[must_use]
    pub const fn epoch(self) -> ArchiveEpoch {
        self.epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayContinuationState {
    Pending,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayContinuation {
    replay: ArchiveReplay,
    state: ReplayContinuationState,
}

impl ReplayContinuation {
    #[must_use]
    pub const fn new(replay: ArchiveReplay, state: ReplayContinuationState) -> Self {
        Self { replay, state }
    }

    #[must_use]
    pub const fn replay(self) -> ArchiveReplay {
        self.replay
    }

    #[must_use]
    pub const fn state(self) -> ReplayContinuationState {
        self.state
    }
}

pub trait Archive: Send {
    fn begin_scan_set(&mut self, manifest: &ScopeManifest) -> Result<ArchiveScanSetId, PortError>;

    fn observe_source(
        &mut self,
        scan_set: ArchiveScanSetId,
        source: &DiscoveredSource,
        initial_state: &AdapterSourceState,
    ) -> Result<(), PortError>;

    fn finish_scope(
        &mut self,
        scan_set: ArchiveScanSetId,
        scope: &ScopeIdentity,
        completion: AdapterCompletion,
    ) -> Result<(), PortError>;

    fn finish_scan_set(
        &mut self,
        scan_set: ArchiveScanSetId,
    ) -> Result<CompletionQuality, PortError>;

    fn begin_replay(&mut self, scan_set: ArchiveScanSetId) -> Result<ArchiveReplay, PortError>;

    fn prepare_replay_source(
        &mut self,
        replay: ArchiveReplay,
        source: &DiscoveredSource,
        initial_state: &AdapterSourceState,
    ) -> Result<ArchiveReplay, PortError>;

    fn append_replay_batch(
        &mut self,
        replay: ArchiveReplay,
        source: &SourceIdentity,
        batch: CanonicalBatch,
    ) -> Result<ArchiveReplay, PortError>;

    fn continue_replay(&mut self, replay: ArchiveReplay) -> Result<ReplayContinuation, PortError>;

    fn seal_replay(&mut self, replay: ArchiveReplay) -> Result<ArchiveReplay, PortError>;

    fn promote_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError>;

    fn discard_replay(&mut self, replay: ArchiveReplay) -> Result<(), PortError>;
}
