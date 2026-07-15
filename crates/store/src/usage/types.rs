use std::fmt;

use tokenmaster_accounting::CanonicalUsageEvent;
use tokenmaster_accounting::{
    CANONICALIZER_VERSION, EVENT_FINGERPRINT_VERSION, REPLAY_SIGNATURE_VERSION,
};
use tokenmaster_domain::SessionRelationDraft;

use crate::{StoreError, StoreErrorCode};

pub const MAX_RESUME_BYTES: usize = 32 * 1024;
pub const MAX_USAGE_EVENT_PAGE_SIZE: usize = 256;
pub const MAX_APPEND_EVENTS: usize = 256;
pub const MAX_APPEND_CHUNK_UPDATES: usize = 18;
pub const MAX_REPLAY_SOURCES: usize = 256;
pub const MAX_SCAN_SCOPES: usize = 256;
pub const SOURCE_CHUNK_BYTES: u64 = 1 << 20;
const MAX_ANCHOR_BYTES: u16 = 4096;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct SourceKey([u8; 32]);

impl SourceKey {
    pub fn from_slice(value: &[u8]) -> Result<Self, StoreError> {
        let bytes = <[u8; 32]>::try_from(value)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?;
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for SourceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SourceKey([redacted])")
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScanScope {
    provider_id: Box<str>,
    profile_id: Box<str>,
}

impl ScanScope {
    pub fn new(
        provider_id: impl Into<Box<str>>,
        profile_id: impl Into<Box<str>>,
    ) -> Result<Self, StoreError> {
        let provider_id = provider_id.into();
        let profile_id = profile_id.into();
        if !valid_ascii_id(&provider_id, 64) || !valid_ascii_id(&profile_id, 128) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
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

impl fmt::Debug for ScanScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScanScope")
            .field("provider_id", &Redacted)
            .field("profile_id", &Redacted)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ScanSetManifest {
    scopes: Box<[ScanScope]>,
}

impl ScanSetManifest {
    pub fn new(scopes: Box<[ScanScope]>) -> Result<Self, StoreError> {
        if scopes.is_empty() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if scopes.len() > MAX_SCAN_SCOPES {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_SCAN_SCOPES as u64,
            ));
        }
        let mut scopes = scopes.into_vec();
        scopes.sort_unstable();
        if scopes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            scopes: scopes.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn scopes(&self) -> &[ScanScope] {
        &self.scopes
    }

    #[must_use]
    pub const fn scope_count(&self) -> usize {
        self.scopes.len()
    }
}

impl fmt::Debug for ScanSetManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScanSetManifest")
            .field("scope_count", &self.scopes.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScanSetId(u64);

impl ScanSetId {
    pub fn new(value: u64) -> Result<Self, StoreError> {
        if value > i64::MAX as u64 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    pub(super) fn from_stored(value: i64) -> Result<Self, StoreError> {
        let value = u64::try_from(value)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        Ok(Self(value))
    }

    pub(super) fn as_sql(self) -> Result<i64, StoreError> {
        i64::try_from(self.0).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScanId(u64);

impl ScanId {
    pub fn new(value: u64) -> Result<Self, StoreError> {
        if value > i64::MAX as u64 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    pub(super) fn from_stored(value: i64) -> Result<Self, StoreError> {
        let value = u64::try_from(value)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        Ok(Self(value))
    }

    pub(super) fn as_sql(self) -> Result<i64, StoreError> {
        i64::try_from(self.0).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanOutcome {
    Complete,
    Partial,
    Cancelled,
    Failed,
    TimedOut,
}

impl ScanOutcome {
    pub(super) const fn as_sql(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
        }
    }

    pub(super) fn from_sql(value: &str) -> Result<Self, StoreError> {
        match value {
            "complete" => Ok(Self::Complete),
            "partial" => Ok(Self::Partial),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            "timed_out" => Ok(Self::TimedOut),
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScanCounters {
    files_read: u64,
    bytes_read: u64,
    events_observed: u64,
    diagnostics: u64,
}

impl ScanCounters {
    pub fn new(
        files_read: u64,
        bytes_read: u64,
        events_observed: u64,
        diagnostics: u64,
    ) -> Result<Self, StoreError> {
        if [files_read, bytes_read, events_observed, diagnostics]
            .into_iter()
            .any(|value| value > i64::MAX as u64)
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            files_read,
            bytes_read,
            events_observed,
            diagnostics,
        })
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanSnapshot {
    pub(super) id: ScanId,
    pub(super) scan_set_id: ScanSetId,
    pub(super) scope: ScanScope,
    pub(super) started_at_ms: i64,
    pub(super) completed_at_ms: Option<i64>,
    pub(super) outcome: Option<ScanOutcome>,
    pub(super) sources_seen: u64,
    pub(super) counters: ScanCounters,
}

impl ScanSnapshot {
    #[must_use]
    pub const fn id(&self) -> ScanId {
        self.id
    }

    #[must_use]
    pub const fn scan_set_id(&self) -> ScanSetId {
        self.scan_set_id
    }

    #[must_use]
    pub const fn scope(&self) -> &ScanScope {
        &self.scope
    }

    #[must_use]
    pub const fn started_at_ms(&self) -> i64 {
        self.started_at_ms
    }

    #[must_use]
    pub const fn completed_at_ms(&self) -> Option<i64> {
        self.completed_at_ms
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<ScanOutcome> {
        self.outcome
    }

    #[must_use]
    pub const fn sources_seen(&self) -> u64 {
        self.sources_seen
    }

    #[must_use]
    pub const fn counters(&self) -> ScanCounters {
        self.counters
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanSetSnapshot {
    pub(super) id: ScanSetId,
    pub(super) started_at_ms: i64,
    pub(super) completed_at_ms: Option<i64>,
    pub(super) outcome: Option<ScanOutcome>,
    pub(super) expected_scope_count: u64,
}

impl ScanSetSnapshot {
    #[must_use]
    pub const fn id(self) -> ScanSetId {
        self.id
    }

    #[must_use]
    pub const fn started_at_ms(self) -> i64 {
        self.started_at_ms
    }

    #[must_use]
    pub const fn completed_at_ms(self) -> Option<i64> {
        self.completed_at_ms
    }

    #[must_use]
    pub const fn outcome(self) -> Option<ScanOutcome> {
        self.outcome
    }

    #[must_use]
    pub const fn expected_scope_count(self) -> u64 {
        self.expected_scope_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredVerification {
    Incremental,
    FullPrefix,
}

impl StoredVerification {
    pub(super) const fn as_sql(self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::FullPrefix => "full_prefix",
        }
    }

    pub(super) fn from_sql(value: &str) -> Result<Self, StoreError> {
        match value {
            "incremental" => Ok(Self::Incremental),
            "full_prefix" => Ok(Self::FullPrefix),
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct StoredSourceChunk {
    index: u64,
    covered_len: u32,
    sha256: [u8; 32],
}

impl StoredSourceChunk {
    pub fn new(index: u64, covered_len: u32, sha256: [u8; 32]) -> Result<Self, StoreError> {
        if covered_len == 0 || u64::from(covered_len) > SOURCE_CHUNK_BYTES {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let end = index
            .checked_mul(SOURCE_CHUNK_BYTES)
            .and_then(|start| start.checked_add(u64::from(covered_len)))
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))?;
        if end > i64::MAX as u64 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
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

    pub(super) fn end_offset(&self) -> Result<u64, StoreError> {
        self.index
            .checked_mul(SOURCE_CHUNK_BYTES)
            .and_then(|start| start.checked_add(u64::from(self.covered_len)))
            .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidValue))
    }
}

impl fmt::Debug for StoredSourceChunk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredSourceChunk")
            .field("index", &self.index)
            .field("covered_len", &self.covered_len)
            .field("sha256", &Redacted)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Active,
    Direct,
    Archived,
}

impl SourceKind {
    pub(super) const fn as_sql(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Direct => "direct",
            Self::Archived => "archived",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SourceRegistrationParts {
    pub source_key: SourceKey,
    pub provider_id: Box<str>,
    pub profile_id: Box<str>,
    pub source_id: Box<str>,
    pub source_kind: SourceKind,
    pub logical_identity: [u8; 32],
    pub physical_identity: Option<[u8; 32]>,
    pub initial_checkpoint: StoredCheckpoint,
}

impl fmt::Debug for SourceRegistrationParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        registration_debug("SourceRegistrationParts", self, formatter)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SourceRegistration {
    parts: SourceRegistrationParts,
}

impl SourceRegistration {
    pub fn new(parts: SourceRegistrationParts) -> Result<Self, StoreError> {
        if !valid_ascii_id(&parts.provider_id, 64)
            || !valid_ascii_id(&parts.profile_id, 128)
            || !valid_ascii_id(&parts.source_id, 128)
            || parts.initial_checkpoint.logical_identity() != &parts.logical_identity
            || parts.initial_checkpoint.physical_identity() != parts.physical_identity.as_ref()
            || parts.initial_checkpoint.committed_offset() != 0
            || parts.initial_checkpoint.scan_offset() != 0
            || parts.initial_checkpoint.observed_file_length() != 0
            || parts.initial_checkpoint.anchor_start() != 0
            || parts.initial_checkpoint.anchor_len() != 0
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self { parts })
    }

    pub(super) const fn parts(&self) -> &SourceRegistrationParts {
        &self.parts
    }
}

impl fmt::Debug for SourceRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        registration_debug("SourceRegistration", &self.parts, formatter)
    }
}

fn registration_debug(
    name: &str,
    parts: &SourceRegistrationParts,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    formatter
        .debug_struct(name)
        .field("source_key", &parts.source_key)
        .field("provider_id", &parts.provider_id)
        .field("profile_id", &parts.profile_id)
        .field("source_id", &parts.source_id)
        .field("source_kind", &parts.source_kind)
        .field("logical_identity", &Redacted)
        .field("physical_identity", &Redacted)
        .field("initial_checkpoint", &parts.initial_checkpoint)
        .finish()
}

#[derive(Clone)]
pub struct AppendBatchParts {
    pub source_key: SourceKey,
    pub expected_generation: u64,
    pub expected_committed_offset: u64,
    pub expected_scan_offset: u64,
    pub events: Box<[CanonicalUsageEvent]>,
    pub previous_partial_chunk: Option<StoredSourceChunk>,
    pub chunk_updates: Box<[StoredSourceChunk]>,
    pub next_checkpoint: StoredCheckpoint,
    pub diagnostic_count_delta: u64,
}

impl fmt::Debug for AppendBatchParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        append_debug("AppendBatchParts", self, formatter)
    }
}

#[derive(Clone)]
pub struct AppendBatch {
    parts: AppendBatchParts,
}

impl AppendBatch {
    pub fn new(parts: AppendBatchParts) -> Result<Self, StoreError> {
        for value in [
            parts.expected_generation,
            parts.expected_committed_offset,
            parts.expected_scan_offset,
            parts.diagnostic_count_delta,
        ] {
            if value > i64::MAX as u64 {
                return Err(StoreError::new(StoreErrorCode::InvalidValue));
            }
        }
        if parts.events.len() > MAX_APPEND_EVENTS
            || parts.chunk_updates.len() > MAX_APPEND_CHUNK_UPDATES
            || parts.expected_scan_offset < parts.expected_committed_offset
            || parts.next_checkpoint.committed_offset() < parts.expected_committed_offset
            || parts.next_checkpoint.scan_offset() < parts.expected_scan_offset
            || parts.next_checkpoint.observed_file_length() > i64::MAX as u64
            || parts.next_checkpoint.committed_offset() > i64::MAX as u64
            || parts.next_checkpoint.scan_offset() > i64::MAX as u64
            || parts.next_checkpoint.anchor_start() > i64::MAX as u64
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if parts.events.iter().any(|event| {
            event.source_offset() < parts.expected_committed_offset
                || event.source_offset() >= parts.next_checkpoint.committed_offset()
        }) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        validate_chunk_updates(&parts)?;
        Ok(Self { parts })
    }

    pub(super) const fn parts(&self) -> &AppendBatchParts {
        &self.parts
    }
}

impl fmt::Debug for AppendBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        append_debug("AppendBatch", &self.parts, formatter)
    }
}

fn validate_chunk_updates(parts: &AppendBatchParts) -> Result<(), StoreError> {
    for pair in parts.chunk_updates.windows(2) {
        if pair[1].index() != pair[0].index().saturating_add(1)
            || pair[0].covered_len() != SOURCE_CHUNK_BYTES as u32
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
    }
    let target_offset = if parts.next_checkpoint.discarding_oversized_line() {
        parts.next_checkpoint.scan_offset()
    } else {
        parts.next_checkpoint.committed_offset()
    };
    match parts.chunk_updates.last() {
        Some(last) if last.end_offset()? != target_offset => {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        None if target_offset != parts.expected_scan_offset => {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        _ => {}
    }
    if let Some(first) = parts.chunk_updates.first()
        && first.index() != parts.expected_scan_offset / SOURCE_CHUNK_BYTES
    {
        return Err(StoreError::new(StoreErrorCode::InvalidValue));
    }
    if let Some(proof) = parts.previous_partial_chunk {
        let Some(first) = parts.chunk_updates.first() else {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        };
        if proof.index() != first.index()
            || proof.covered_len() >= SOURCE_CHUNK_BYTES as u32
            || first.covered_len() <= proof.covered_len()
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
    }
    Ok(())
}

fn append_debug(
    name: &str,
    parts: &AppendBatchParts,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    formatter
        .debug_struct(name)
        .field("source_key", &parts.source_key)
        .field("expected_generation", &parts.expected_generation)
        .field(
            "expected_committed_offset",
            &parts.expected_committed_offset,
        )
        .field("expected_scan_offset", &parts.expected_scan_offset)
        .field("events_count", &parts.events.len())
        .field("previous_partial_chunk", &parts.previous_partial_chunk)
        .field("chunk_updates_count", &parts.chunk_updates.len())
        .field("next_checkpoint", &parts.next_checkpoint)
        .field("diagnostic_count_delta", &parts.diagnostic_count_delta)
        .finish()
}

fn valid_ascii_id(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[derive(Clone, Eq, PartialEq)]
pub struct StoredCheckpointParts {
    pub parser_schema_version: u16,
    pub physical_identity: Option<[u8; 32]>,
    pub logical_identity: [u8; 32],
    pub committed_offset: u64,
    pub scan_offset: u64,
    pub observed_file_length: u64,
    pub modified_time_ns: Option<i64>,
    pub anchor_start: u64,
    pub anchor_len: u16,
    pub anchor_sha256: [u8; 32],
    pub resume: Box<[u8]>,
    pub discarding_oversized_line: bool,
    pub incomplete_tail: bool,
    pub verification: StoredVerification,
}

impl fmt::Debug for StoredCheckpointParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        checkpoint_debug("StoredCheckpointParts", self, formatter)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct StoredCheckpoint {
    parts: StoredCheckpointParts,
}

impl StoredCheckpoint {
    pub fn new(parts: StoredCheckpointParts) -> Result<Self, StoreError> {
        if parts.parser_schema_version == 0
            || parts.scan_offset < parts.committed_offset
            || parts.scan_offset > parts.observed_file_length
            || (!parts.discarding_oversized_line && parts.scan_offset != parts.committed_offset)
            || parts.anchor_len > MAX_ANCHOR_BYTES
            || parts.anchor_start > parts.committed_offset
            || u64::from(parts.anchor_len)
                > parts.committed_offset.saturating_sub(parts.anchor_start)
            || (parts.discarding_oversized_line
                && (!parts.incomplete_tail || parts.scan_offset == parts.committed_offset))
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if parts.resume.len() > MAX_RESUME_BYTES {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_RESUME_BYTES as u64,
            ));
        }
        Ok(Self { parts })
    }

    #[must_use]
    pub const fn parser_schema_version(&self) -> u16 {
        self.parts.parser_schema_version
    }

    #[must_use]
    pub const fn physical_identity(&self) -> Option<&[u8; 32]> {
        self.parts.physical_identity.as_ref()
    }

    #[must_use]
    pub const fn logical_identity(&self) -> &[u8; 32] {
        &self.parts.logical_identity
    }

    #[must_use]
    pub const fn committed_offset(&self) -> u64 {
        self.parts.committed_offset
    }

    #[must_use]
    pub const fn scan_offset(&self) -> u64 {
        self.parts.scan_offset
    }

    #[must_use]
    pub const fn observed_file_length(&self) -> u64 {
        self.parts.observed_file_length
    }

    #[must_use]
    pub const fn modified_time_ns(&self) -> Option<i64> {
        self.parts.modified_time_ns
    }

    #[must_use]
    pub const fn anchor_start(&self) -> u64 {
        self.parts.anchor_start
    }

    #[must_use]
    pub const fn anchor_len(&self) -> u16 {
        self.parts.anchor_len
    }

    #[must_use]
    pub const fn anchor_sha256(&self) -> &[u8; 32] {
        &self.parts.anchor_sha256
    }

    #[must_use]
    pub fn resume(&self) -> &[u8] {
        &self.parts.resume
    }

    #[must_use]
    pub const fn discarding_oversized_line(&self) -> bool {
        self.parts.discarding_oversized_line
    }

    #[must_use]
    pub const fn incomplete_tail(&self) -> bool {
        self.parts.incomplete_tail
    }

    #[must_use]
    pub const fn verification(&self) -> StoredVerification {
        self.parts.verification
    }
}

impl fmt::Debug for StoredCheckpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        checkpoint_debug("StoredCheckpoint", &self.parts, formatter)
    }
}

fn checkpoint_debug(
    name: &str,
    parts: &StoredCheckpointParts,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    formatter
        .debug_struct(name)
        .field("parser_schema_version", &parts.parser_schema_version)
        .field("physical_identity", &Redacted)
        .field("logical_identity", &Redacted)
        .field("committed_offset", &parts.committed_offset)
        .field("scan_offset", &parts.scan_offset)
        .field("observed_file_length", &parts.observed_file_length)
        .field("modified_time_ns", &parts.modified_time_ns)
        .field("anchor", &Redacted)
        .field("resume", &Redacted)
        .field(
            "discarding_oversized_line",
            &parts.discarding_oversized_line,
        )
        .field("incomplete_tail", &parts.incomplete_tail)
        .field("verification", &parts.verification)
        .finish()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationStatus {
    Staging,
    Current,
}

impl GenerationStatus {
    pub(super) fn from_sql(value: &str) -> Result<Self, StoreError> {
        match value {
            "staging" => Ok(Self::Staging),
            "current" => Ok(Self::Current),
            _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationSnapshot {
    pub(super) source_key: SourceKey,
    pub(super) generation: u64,
    pub(super) status: GenerationStatus,
    pub(super) checkpoint: StoredCheckpoint,
}

impl GenerationSnapshot {
    #[must_use]
    pub const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn status(&self) -> GenerationStatus {
        self.status
    }

    #[must_use]
    pub const fn checkpoint(&self) -> &StoredCheckpoint {
        &self.checkpoint
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsageStoreCounts {
    pub(super) sources: u64,
    pub(super) generations: u64,
    pub(super) observations: u64,
    pub(super) canonical_events: u64,
    pub(super) chunks: u64,
    pub(super) scans: u64,
}

impl UsageStoreCounts {
    #[must_use]
    pub const fn sources(&self) -> u64 {
        self.sources
    }

    #[must_use]
    pub const fn generations(&self) -> u64 {
        self.generations
    }

    #[must_use]
    pub const fn observations(&self) -> u64 {
        self.observations
    }

    #[must_use]
    pub const fn canonical_events(&self) -> u64 {
        self.canonical_events
    }

    #[must_use]
    pub const fn chunks(&self) -> u64 {
        self.chunks
    }

    #[must_use]
    pub const fn scans(&self) -> u64 {
        self.scans
    }

    #[must_use]
    pub const fn total(&self) -> u64 {
        self.sources
            .saturating_add(self.generations)
            .saturating_add(self.observations)
            .saturating_add(self.canonical_events)
            .saturating_add(self.chunks)
            .saturating_add(self.scans)
    }
}

#[derive(Clone)]
pub struct ReplayAppendBatchParts {
    pub revision_id: ReplayRevisionId,
    pub expected_epoch: ReplayEpoch,
    pub append_batch: AppendBatch,
}

impl fmt::Debug for ReplayAppendBatchParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        replay_append_debug("ReplayAppendBatchParts", self, formatter)
    }
}

#[derive(Clone)]
pub struct ReplayAppendBatch {
    parts: ReplayAppendBatchParts,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ReplayRelation {
    pub(super) revision_id: ReplayRevisionId,
    pub(super) expected_epoch: ReplayEpoch,
    pub(super) source_key: SourceKey,
    pub(super) provider_id: Box<str>,
    pub(super) profile_id: Box<str>,
    pub(super) session_id: Box<str>,
    pub(super) parent_session_id: Option<Box<str>>,
    pub(super) declared_conflict: bool,
    pub(super) source_id: Box<str>,
    pub(super) source_offset: u64,
}

impl ReplayRelation {
    pub fn new(
        revision_id: ReplayRevisionId,
        expected_epoch: ReplayEpoch,
        source_key: SourceKey,
        relation: &SessionRelationDraft,
    ) -> Result<Self, StoreError> {
        if relation.source_offset() > i64::MAX as u64 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            revision_id,
            expected_epoch,
            source_key,
            provider_id: relation.provider_id().as_str().into(),
            profile_id: relation.profile_id().as_str().into(),
            session_id: relation.session_id().as_str().into(),
            parent_session_id: Some(relation.parent_session_id().as_str().into()),
            declared_conflict: relation.declared_conflict(),
            source_id: relation.source_id().as_str().into(),
            source_offset: relation.source_offset(),
        })
    }
}

impl fmt::Debug for ReplayRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayRelation")
            .field("revision_id", &self.revision_id)
            .field("expected_epoch", &self.expected_epoch)
            .field("source", &Redacted)
            .field("relation", &Redacted)
            .field("declared_conflict", &self.declared_conflict)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayContinuationResult {
    pub(super) processed_count: u16,
    pub(super) remaining_work: bool,
    pub(super) epoch: ReplayEpoch,
}

impl ReplayContinuationResult {
    #[must_use]
    pub const fn processed_count(self) -> u16 {
        self.processed_count
    }

    #[must_use]
    pub const fn remaining_work(self) -> bool {
        self.remaining_work
    }

    #[must_use]
    pub const fn epoch(self) -> ReplayEpoch {
        self.epoch
    }
}

impl ReplayAppendBatch {
    #[must_use]
    pub const fn new(parts: ReplayAppendBatchParts) -> Self {
        Self { parts }
    }

    pub(super) const fn parts(&self) -> &ReplayAppendBatchParts {
        &self.parts
    }
}

impl fmt::Debug for ReplayAppendBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        replay_append_debug("ReplayAppendBatch", &self.parts, formatter)
    }
}

fn replay_append_debug(
    name: &str,
    parts: &ReplayAppendBatchParts,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    formatter
        .debug_struct(name)
        .field("revision_id", &parts.revision_id)
        .field("expected_epoch", &parts.expected_epoch)
        .field("append_batch", &parts.append_batch)
        .finish()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountingVersions {
    canonicalizer: u16,
    fingerprint: u16,
    replay_signature: u16,
}

impl AccountingVersions {
    pub(super) const fn compiled() -> Self {
        Self {
            canonicalizer: CANONICALIZER_VERSION,
            fingerprint: EVENT_FINGERPRINT_VERSION,
            replay_signature: REPLAY_SIGNATURE_VERSION,
        }
    }

    pub(super) fn from_stored(
        canonicalizer: i64,
        fingerprint: i64,
        replay_signature: i64,
    ) -> Result<Self, StoreError> {
        let versions = Self {
            canonicalizer: u16::try_from(canonicalizer)
                .ok()
                .filter(|value| *value != 0)
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
            fingerprint: u16::try_from(fingerprint)
                .ok()
                .filter(|value| *value != 0)
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
            replay_signature: u16::try_from(replay_signature)
                .ok()
                .filter(|value| *value != 0)
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
        };
        Ok(versions)
    }

    #[must_use]
    pub const fn canonicalizer(self) -> u16 {
        self.canonicalizer
    }

    #[must_use]
    pub const fn fingerprint(self) -> u16 {
        self.fingerprint
    }

    #[must_use]
    pub const fn replay_signature(self) -> u16 {
        self.replay_signature
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReplayRevisionId(u64);

impl ReplayRevisionId {
    pub fn new(value: u64) -> Result<Self, StoreError> {
        if value > i64::MAX as u64 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    pub(super) fn from_stored(value: i64) -> Result<Self, StoreError> {
        let value = u64::try_from(value)
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        Ok(Self(value))
    }

    pub(super) fn as_sql(self) -> Result<i64, StoreError> {
        i64::try_from(self.0).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReplayEpoch(u64);

impl ReplayEpoch {
    pub fn new(value: u64) -> Result<Self, StoreError> {
        if value > i64::MAX as u64 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self(value))
    }

    pub(super) fn as_sql(self) -> Result<i64, StoreError> {
        i64::try_from(self.0).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ReplayManifest {
    source_keys: Box<[SourceKey]>,
}

impl ReplayManifest {
    pub fn new(source_keys: Box<[SourceKey]>) -> Result<Self, StoreError> {
        if source_keys.is_empty() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        if source_keys.len() > MAX_REPLAY_SOURCES {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_REPLAY_SOURCES as u64,
            ));
        }
        let mut source_keys = source_keys.into_vec();
        source_keys.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        if source_keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            source_keys: source_keys.into_boxed_slice(),
        })
    }

    pub(super) const fn source_keys(&self) -> &[SourceKey] {
        &self.source_keys
    }

    #[must_use]
    pub const fn source_count(&self) -> usize {
        self.source_keys.len()
    }
}

impl fmt::Debug for ReplayManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayManifest")
            .field("source_count", &self.source_keys.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayRevisionStatus {
    Staging,
    Current,
}

impl ReplayRevisionStatus {
    pub(super) const fn as_sql(self) -> &'static str {
        match self {
            Self::Staging => "staging",
            Self::Current => "current",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayRevisionSnapshot {
    pub(super) id: ReplayRevisionId,
    pub(super) epoch: ReplayEpoch,
    pub(super) status: ReplayRevisionStatus,
    pub(super) versions: AccountingVersions,
    pub(super) expected_source_count: u64,
    pub(super) sealed: bool,
    pub(super) promoted: bool,
}

impl ReplayRevisionSnapshot {
    #[must_use]
    pub const fn id(self) -> ReplayRevisionId {
        self.id
    }

    #[must_use]
    pub const fn epoch(self) -> ReplayEpoch {
        self.epoch
    }

    #[must_use]
    pub const fn status(self) -> ReplayRevisionStatus {
        self.status
    }

    #[must_use]
    pub const fn versions(self) -> AccountingVersions {
        self.versions
    }

    #[must_use]
    pub const fn expected_source_count(self) -> u64 {
        self.expected_source_count
    }

    #[must_use]
    pub const fn sealed(self) -> bool {
        self.sealed
    }

    #[must_use]
    pub const fn promoted(self) -> bool {
        self.promoted
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveMode {
    Empty,
    LegacyUnverified,
    ReplayVerified,
    ReplayVersionStale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveState {
    pub(super) mode: ArchiveMode,
    pub(super) active_revision: Option<ReplayRevisionId>,
    pub(super) rebuild_staging: bool,
}

impl ArchiveState {
    #[must_use]
    pub const fn mode(self) -> ArchiveMode {
        self.mode
    }

    #[must_use]
    pub const fn active_revision(self) -> Option<ReplayRevisionId> {
        self.active_revision
    }

    #[must_use]
    pub const fn rebuild_staging(self) -> bool {
        self.rebuild_staging
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReplayQualityCounts {
    pub(super) eligible: u64,
    pub(super) replay: u64,
    pub(super) pending: u64,
    pub(super) conflict: u64,
}

impl ReplayQualityCounts {
    #[must_use]
    pub const fn eligible(self) -> u64 {
        self.eligible
    }

    #[must_use]
    pub const fn replay(self) -> u64 {
        self.replay
    }

    #[must_use]
    pub const fn pending(self) -> u64 {
        self.pending
    }

    #[must_use]
    pub const fn conflict(self) -> u64 {
        self.conflict
    }

    #[must_use]
    pub const fn total(self) -> u64 {
        self.eligible
            .saturating_add(self.replay)
            .saturating_add(self.pending)
            .saturating_add(self.conflict)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct EventCursor {
    timestamp_seconds: i64,
    timestamp_nanos: u32,
    fingerprint: [u8; 32],
}

impl EventCursor {
    pub fn new(
        timestamp_seconds: i64,
        timestamp_nanos: u32,
        fingerprint: [u8; 32],
    ) -> Result<Self, StoreError> {
        if timestamp_nanos >= 1_000_000_000 {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        Ok(Self {
            timestamp_seconds,
            timestamp_nanos,
            fingerprint,
        })
    }

    pub(super) const fn timestamp_seconds(self) -> i64 {
        self.timestamp_seconds
    }

    pub(super) const fn timestamp_nanos(self) -> u32 {
        self.timestamp_nanos
    }

    pub(super) const fn fingerprint(self) -> [u8; 32] {
        self.fingerprint
    }
}

impl fmt::Debug for EventCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventCursor")
            .field("timestamp_seconds", &self.timestamp_seconds)
            .field("timestamp_nanos", &self.timestamp_nanos)
            .field("fingerprint", &Redacted)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct StoredUsageEvent {
    pub(super) event_id: Box<str>,
    pub(super) timestamp_seconds: i64,
    pub(super) timestamp_nanos: u32,
    pub(super) model: Box<str>,
    pub(super) total_tokens: Option<u64>,
    pub(super) fingerprint: [u8; 32],
}

impl StoredUsageEvent {
    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    #[must_use]
    pub const fn timestamp_seconds(&self) -> i64 {
        self.timestamp_seconds
    }

    #[must_use]
    pub const fn timestamp_nanos(&self) -> u32 {
        self.timestamp_nanos
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub const fn total_tokens(&self) -> Option<u64> {
        self.total_tokens
    }

    #[must_use]
    pub const fn cursor(&self) -> EventCursor {
        EventCursor {
            timestamp_seconds: self.timestamp_seconds,
            timestamp_nanos: self.timestamp_nanos,
            fingerprint: self.fingerprint,
        }
    }
}

impl fmt::Debug for StoredUsageEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredUsageEvent")
            .field("event_id", &self.event_id)
            .field("timestamp_seconds", &self.timestamp_seconds)
            .field("timestamp_nanos", &self.timestamp_nanos)
            .field("model", &self.model)
            .field("total_tokens", &self.total_tokens)
            .field("fingerprint", &Redacted)
            .finish()
    }
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}
