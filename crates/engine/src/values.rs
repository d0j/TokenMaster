use core::fmt;

use crate::{EngineError, EngineErrorCode};

pub const MAX_PROVIDER_ID_BYTES: usize = 64;
pub const MAX_PROFILE_ID_BYTES: usize = 128;
pub const MAX_SOURCE_ID_BYTES: usize = 128;
pub const MAX_SCOPE_MANIFEST_ENTRIES: usize = 256;
pub const MAX_ADAPTER_CHECKPOINT_BYTES: usize = 32 * 1024;
pub const MAX_CHUNK_PROOFS_PER_BATCH: usize = 18;
pub const SOURCE_CHUNK_BYTES: u64 = 1 << 20;

fn validate_id(value: &str, max_bytes: usize) -> Result<(), EngineError> {
    if value.is_empty() {
        return Err(EngineError::new(EngineErrorCode::InvalidValue));
    }
    if value.len() > max_bytes {
        return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(EngineError::new(EngineErrorCode::InvalidValue));
    }
    Ok(())
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScopeIdentity {
    provider_id: Box<str>,
    profile_id: Box<str>,
}

impl ScopeIdentity {
    pub fn new(
        provider_id: impl Into<Box<str>>,
        profile_id: impl Into<Box<str>>,
    ) -> Result<Self, EngineError> {
        let provider_id = provider_id.into();
        let profile_id = profile_id.into();
        validate_id(&provider_id, MAX_PROVIDER_ID_BYTES)?;
        validate_id(&profile_id, MAX_PROFILE_ID_BYTES)?;
        Ok(Self {
            provider_id,
            profile_id,
        })
    }

    #[must_use]
    pub const fn provider_id(&self) -> &str {
        &self.provider_id
    }

    #[must_use]
    pub const fn profile_id(&self) -> &str {
        &self.profile_id
    }
}

impl fmt::Debug for ScopeIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ScopeIdentity([redacted])")
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct SourceIdentity {
    scope: ScopeIdentity,
    source_id: Box<str>,
}

impl SourceIdentity {
    pub fn new(scope: ScopeIdentity, source_id: impl Into<Box<str>>) -> Result<Self, EngineError> {
        let source_id = source_id.into();
        validate_id(&source_id, MAX_SOURCE_ID_BYTES)?;
        Ok(Self { scope, source_id })
    }

    #[must_use]
    pub const fn scope(&self) -> &ScopeIdentity {
        &self.scope
    }

    #[must_use]
    pub const fn source_id(&self) -> &str {
        &self.source_id
    }
}

impl fmt::Debug for SourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SourceIdentity([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Active,
    Direct,
    Archived,
}

#[derive(Clone, Eq, PartialEq)]
pub struct DiscoveredSource {
    identity: SourceIdentity,
    kind: SourceKind,
    logical_identity: [u8; 32],
}

impl DiscoveredSource {
    #[must_use]
    pub const fn new(
        identity: SourceIdentity,
        kind: SourceKind,
        logical_identity: [u8; 32],
    ) -> Self {
        Self {
            identity,
            kind,
            logical_identity,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn kind(&self) -> SourceKind {
        self.kind
    }

    #[must_use]
    pub const fn logical_identity(&self) -> &[u8; 32] {
        &self.logical_identity
    }
}

impl fmt::Debug for DiscoveredSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscoveredSource")
            .field("identity", &Redacted)
            .field("kind", &self.kind)
            .field("logical_identity", &Redacted)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ScopeManifest {
    scopes: Box<[ScopeIdentity]>,
}

impl ScopeManifest {
    pub fn new(scopes: Box<[ScopeIdentity]>) -> Result<Self, EngineError> {
        if scopes.is_empty() {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        if scopes.len() > MAX_SCOPE_MANIFEST_ENTRIES {
            return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
        }
        let mut scopes = scopes.into_vec();
        scopes.sort_unstable();
        if scopes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        Ok(Self {
            scopes: scopes.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn scopes(&self) -> &[ScopeIdentity] {
        &self.scopes
    }

    #[must_use]
    pub const fn scope_count(&self) -> usize {
        self.scopes.len()
    }
}

impl fmt::Debug for ScopeManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopeManifest")
            .field("scope_count", &self.scopes.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct AdapterCheckpoint {
    bytes: Box<[u8]>,
}

impl AdapterCheckpoint {
    pub fn new(bytes: Box<[u8]>) -> Result<Self, EngineError> {
        if bytes.len() > MAX_ADAPTER_CHECKPOINT_BYTES {
            return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
        }
        Ok(Self { bytes })
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

impl fmt::Debug for AdapterCheckpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterCheckpoint")
            .field("byte_len", &self.bytes.len())
            .field("bytes", &Redacted)
            .finish()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ChunkProof {
    index: u64,
    covered_len: u32,
    sha256: [u8; 32],
}

impl ChunkProof {
    pub fn new(index: u64, covered_len: u64, sha256: [u8; 32]) -> Result<Self, EngineError> {
        if covered_len == 0 {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        if covered_len > SOURCE_CHUNK_BYTES {
            return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
        }
        let covered_len = u32::try_from(covered_len)
            .map_err(|_| EngineError::new(EngineErrorCode::CapacityExceeded))?;
        Ok(Self {
            index,
            covered_len,
            sha256,
        })
    }

    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }

    #[must_use]
    pub const fn covered_len(&self) -> u32 {
        self.covered_len
    }

    #[must_use]
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

impl fmt::Debug for ChunkProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChunkProof")
            .field("index", &self.index)
            .field("covered_len", &self.covered_len)
            .field("sha256", &Redacted)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkProofBatch {
    previous_partial: Option<ChunkProof>,
    updates: Box<[ChunkProof]>,
}

impl ChunkProofBatch {
    pub fn new(
        previous_partial: Option<ChunkProof>,
        updates: Box<[ChunkProof]>,
    ) -> Result<Self, EngineError> {
        if updates.len() > MAX_CHUNK_PROOFS_PER_BATCH {
            return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
        }
        let mut updates = updates.into_vec();
        updates.sort_unstable_by_key(ChunkProof::index);
        if updates
            .windows(2)
            .any(|pair| pair[0].index == pair[1].index)
        {
            return Err(EngineError::new(EngineErrorCode::InvalidValue));
        }
        Ok(Self {
            previous_partial,
            updates: updates.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn previous_partial(&self) -> Option<&ChunkProof> {
        self.previous_partial.as_ref()
    }

    #[must_use]
    pub const fn updates(&self) -> &[ChunkProof] {
        &self.updates
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdapterCounters {
    files_read: u64,
    bytes_read: u64,
    events_observed: u64,
    diagnostics: u64,
}

impl AdapterCounters {
    pub const fn new(
        files_read: u64,
        bytes_read: u64,
        events_observed: u64,
        diagnostics: u64,
    ) -> Result<Self, EngineError> {
        if files_read > i64::MAX as u64
            || bytes_read > i64::MAX as u64
            || events_observed > i64::MAX as u64
            || diagnostics > i64::MAX as u64
        {
            return Err(EngineError::new(EngineErrorCode::CapacityExceeded));
        }
        Ok(Self {
            files_read,
            bytes_read,
            events_observed,
            diagnostics,
        })
    }

    pub fn checked_add(self, other: Self) -> Result<Self, EngineError> {
        let files_read = self
            .files_read
            .checked_add(other.files_read)
            .ok_or_else(|| EngineError::new(EngineErrorCode::CapacityExceeded))?;
        let bytes_read = self
            .bytes_read
            .checked_add(other.bytes_read)
            .ok_or_else(|| EngineError::new(EngineErrorCode::CapacityExceeded))?;
        let events_observed = self
            .events_observed
            .checked_add(other.events_observed)
            .ok_or_else(|| EngineError::new(EngineErrorCode::CapacityExceeded))?;
        let diagnostics = self
            .diagnostics
            .checked_add(other.diagnostics)
            .ok_or_else(|| EngineError::new(EngineErrorCode::CapacityExceeded))?;
        Self::new(files_read, bytes_read, events_observed, diagnostics)
    }

    #[must_use]
    pub const fn files_read(self) -> u64 {
        self.files_read
    }

    #[must_use]
    pub const fn bytes_read(self) -> u64 {
        self.bytes_read
    }

    #[must_use]
    pub const fn events_observed(self) -> u64 {
        self.events_observed
    }

    #[must_use]
    pub const fn diagnostics(self) -> u64 {
        self.diagnostics
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AdapterDiagnosticCode {
    InvalidSource,
    PermissionDenied,
    SourceChanged,
    IncompleteInput,
    MalformedInput,
    OversizedInput,
    Unsupported,
    Other,
}

impl AdapterDiagnosticCode {
    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdapterDiagnostics([u64; 8]);

impl AdapterDiagnostics {
    pub fn record(&mut self, code: AdapterDiagnosticCode) -> Result<(), EngineError> {
        let count = self.0[code.index()]
            .checked_add(1)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or_else(|| EngineError::new(EngineErrorCode::CapacityExceeded))?;
        self.0[code.index()] = count;
        Ok(())
    }

    #[must_use]
    pub const fn count(&self, code: AdapterDiagnosticCode) -> u64 {
        self.0[code.index()]
    }

    pub fn total(&self) -> Result<u64, EngineError> {
        self.0.iter().try_fold(0_u64, |total, count| {
            total
                .checked_add(*count)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or_else(|| EngineError::new(EngineErrorCode::CapacityExceeded))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionQuality {
    Complete,
    Partial,
    Cancelled,
    Failed,
    TimedOut,
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}
