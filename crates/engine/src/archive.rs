use core::fmt;

use crate::{
    AdapterCheckpoint, AdapterCompletion, CanonicalBatch, CompletionQuality, DiscoveredSource,
    EngineError, EngineErrorCode, PortError, ScopeIdentity, ScopeManifest, SourceIdentity,
};

pub const MAX_REPLAY_SOURCES_PER_PAGE: usize = 256;

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

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArchiveSourceCursor([u8; 32]);

impl ArchiveSourceCursor {
    #[must_use]
    pub const fn new(value: [u8; 32]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ArchiveSourceCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ArchiveSourceCursor([redacted])")
    }
}

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

#[derive(Clone, Eq, PartialEq)]
pub struct ReplaySource {
    identity: SourceIdentity,
    checkpoint: AdapterCheckpoint,
}

impl ReplaySource {
    #[must_use]
    pub const fn new(identity: SourceIdentity, checkpoint: AdapterCheckpoint) -> Self {
        Self {
            identity,
            checkpoint,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn checkpoint(&self) -> &AdapterCheckpoint {
        &self.checkpoint
    }
}

impl fmt::Debug for ReplaySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplaySource")
            .field("identity", &Redacted)
            .field("checkpoint_byte_len", &self.checkpoint.byte_len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ReplaySourcePage {
    sources: Box<[ReplaySource]>,
    next_cursor: Option<ArchiveSourceCursor>,
}

impl ReplaySourcePage {
    pub fn new(
        sources: Box<[ReplaySource]>,
        next_cursor: Option<ArchiveSourceCursor>,
    ) -> Result<Self, EngineError> {
        if sources.len() > MAX_REPLAY_SOURCES_PER_PAGE {
            return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
        }
        if sources.is_empty() && next_cursor.is_some() {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        if sources.iter().enumerate().any(|(index, source)| {
            sources[index + 1..]
                .iter()
                .any(|other| source.identity == other.identity)
        }) {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        Ok(Self {
            sources,
            next_cursor,
        })
    }

    #[must_use]
    pub const fn sources(&self) -> &[ReplaySource] {
        &self.sources
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ArchiveSourceCursor> {
        self.next_cursor.as_ref()
    }
}

impl fmt::Debug for ReplaySourcePage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplaySourcePage")
            .field("source_count", &self.sources.len())
            .field("has_next_cursor", &self.next_cursor.is_some())
            .finish()
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
        initial_checkpoint: &AdapterCheckpoint,
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

    fn replay_source_page(
        &mut self,
        replay: ArchiveReplay,
        after: Option<&ArchiveSourceCursor>,
    ) -> Result<ReplaySourcePage, PortError>;

    fn prepare_replay_source(
        &mut self,
        replay: ArchiveReplay,
        source: &ReplaySource,
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

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}
