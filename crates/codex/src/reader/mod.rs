use std::fmt;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Component;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokenmaster_domain::{ObservationDraft, ObservationVerification, SessionRelationDraft};
use tokenmaster_platform::PhysicalFileIdentity;
use tokenmaster_provider::SourceKind;

use crate::{PARSER_SCHEMA_VERSION, ParserDiagnostics, ParserState, SourceFileDescriptor};

mod checkpoint;
mod framing;
mod source;

pub use checkpoint::{
    BoundaryAnchor, MAX_ANCHOR_BYTES, MAX_RESUME_BYTES, READER_CHECKPOINT_SCHEMA_VERSION,
    ReaderCheckpointError, ReaderCheckpointErrorCode, ReaderCheckpointParts, ReaderCheckpointV1,
    VerificationLevel,
};

pub const READ_BUFFER_BYTES: usize = 128 * 1024;
pub const MAX_BATCH_EVENTS: usize = 256;
pub const MAX_BATCH_COMPLETE_BYTES: u64 = 1 << 20;
pub const SOURCE_CHUNK_BYTES: u64 = 1 << 20;

const LOGICAL_FILE_DOMAIN: &[u8] = b"tm-source-path-v1";
const PROFILE_HISTORY_CLASS: &[u8] = b"profile-history";
const DIRECT_CLASS: &[u8] = b"direct";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderErrorCode {
    InvalidDescriptor,
    NonRegular,
    ReparsePoint,
    OpenFailed,
    ReadFailed,
    SeekFailed,
    SourceChanged,
    CheckpointInvalid,
    AnchorMismatch,
    ResumeInvalid,
    Cancelled,
    CapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReaderError {
    code: ReaderErrorCode,
    limit: Option<u64>,
}

impl ReaderError {
    pub(super) const fn new(code: ReaderErrorCode) -> Self {
        Self { code, limit: None }
    }

    pub(super) const fn with_limit(code: ReaderErrorCode, limit: u64) -> Self {
        Self {
            code,
            limit: Some(limit),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ReaderErrorCode {
        self.code
    }

    #[must_use]
    pub const fn limit(&self) -> Option<u64> {
        self.limit
    }
}

impl fmt::Display for ReaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            ReaderErrorCode::InvalidDescriptor => "invalid source descriptor",
            ReaderErrorCode::NonRegular => "source is not a regular file",
            ReaderErrorCode::ReparsePoint => "source is a reparse point",
            ReaderErrorCode::OpenFailed => "source open failed",
            ReaderErrorCode::ReadFailed => "source read failed",
            ReaderErrorCode::SeekFailed => "source seek failed",
            ReaderErrorCode::SourceChanged => "source changed during read",
            ReaderErrorCode::CheckpointInvalid => "reader checkpoint is invalid",
            ReaderErrorCode::AnchorMismatch => "reader checkpoint anchor mismatched",
            ReaderErrorCode::ResumeInvalid => "parser resume state is invalid",
            ReaderErrorCode::Cancelled => "reader operation was cancelled",
            ReaderErrorCode::CapacityExceeded => "reader capacity was exceeded",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ReaderError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderDiagnosticCode {
    CompleteLine,
    CrlfLine,
    IncompleteTail,
    OversizedLine,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ReaderDiagnostics {
    complete_lines: u64,
    crlf_lines: u64,
    incomplete_tails: u64,
    oversized_lines: u64,
    max_line_bytes_retained: u64,
}

impl ReaderDiagnostics {
    pub(super) fn record(&mut self, code: ReaderDiagnosticCode) {
        let counter = match code {
            ReaderDiagnosticCode::CompleteLine => &mut self.complete_lines,
            ReaderDiagnosticCode::CrlfLine => &mut self.crlf_lines,
            ReaderDiagnosticCode::IncompleteTail => &mut self.incomplete_tails,
            ReaderDiagnosticCode::OversizedLine => &mut self.oversized_lines,
        };
        *counter = counter.saturating_add(1);
    }

    pub(super) fn observe_line_bytes(&mut self, bytes: usize) {
        self.max_line_bytes_retained = self
            .max_line_bytes_retained
            .max(u64::try_from(bytes).unwrap_or(u64::MAX));
    }

    #[must_use]
    pub const fn count(&self, code: ReaderDiagnosticCode) -> u64 {
        match code {
            ReaderDiagnosticCode::CompleteLine => self.complete_lines,
            ReaderDiagnosticCode::CrlfLine => self.crlf_lines,
            ReaderDiagnosticCode::IncompleteTail => self.incomplete_tails,
            ReaderDiagnosticCode::OversizedLine => self.oversized_lines,
        }
    }

    #[must_use]
    pub const fn max_line_bytes_retained(&self) -> u64 {
        self.max_line_bytes_retained
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RebuildReason {
    IdentityChanged,
    Truncated,
    RewriteDetected,
    ParserVersionChanged,
    AnchorMismatch,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SourceChunkDigest {
    index: u64,
    covered_len: u32,
    sha256: [u8; 32],
}

impl SourceChunkDigest {
    pub fn from_persisted_parts(
        index: u64,
        covered_len: u32,
        sha256: [u8; 32],
    ) -> Result<Self, ReaderError> {
        if covered_len == 0 || u64::from(covered_len) > SOURCE_CHUNK_BYTES {
            return Err(ReaderError::new(ReaderErrorCode::CheckpointInvalid));
        }
        index
            .checked_mul(SOURCE_CHUNK_BYTES)
            .and_then(|start| start.checked_add(u64::from(covered_len)))
            .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
        Ok(Self::from_verified_parts(index, covered_len, sha256))
    }

    pub(super) const fn from_verified_parts(
        index: u64,
        covered_len: u32,
        sha256: [u8; 32],
    ) -> Self {
        Self {
            index,
            covered_len,
            sha256,
        }
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

impl fmt::Debug for SourceChunkDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceChunkDigest")
            .field("index", &self.index)
            .field("covered_len", &self.covered_len)
            .field("sha256", &Redacted)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityReport {
    Verified { chunks: u64, covered_bytes: u64 },
    Mismatch { chunk_index: u64 },
    Cancelled,
}

// Keeping the batch inline avoids one heap allocation on every reader refresh.
#[allow(clippy::large_enum_variant)]
pub enum ReaderOutcome {
    Unchanged(SourceProbe),
    Batch(ReadBatch),
    RebuildRequired(RebuildReason),
}

impl fmt::Debug for ReaderOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unchanged(probe) => formatter.debug_tuple("Unchanged").field(probe).finish(),
            Self::Batch(batch) => formatter.debug_tuple("Batch").field(batch).finish(),
            Self::RebuildRequired(reason) => formatter
                .debug_tuple("RebuildRequired")
                .field(reason)
                .finish(),
        }
    }
}

pub struct ReadBatch {
    checkpoint: ReaderCheckpointV1,
    events: Vec<ObservationDraft>,
    relations: Vec<SessionRelationDraft>,
    diagnostics: ReaderDiagnostics,
    parser_diagnostics: ParserDiagnostics,
    bytes_read: u64,
    reached_snapshot_end: bool,
    source_chunks: Vec<SourceChunkDigest>,
    previous_partial_chunk: Option<SourceChunkDigest>,
}

impl ReadBatch {
    #[must_use]
    pub const fn checkpoint(&self) -> &ReaderCheckpointV1 {
        &self.checkpoint
    }

    #[must_use]
    pub fn events(&self) -> &[ObservationDraft] {
        &self.events
    }

    #[must_use]
    pub fn relations(&self) -> &[SessionRelationDraft] {
        &self.relations
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &ReaderDiagnostics {
        &self.diagnostics
    }

    #[must_use]
    pub const fn parser_diagnostics(&self) -> &ParserDiagnostics {
        &self.parser_diagnostics
    }

    #[must_use]
    pub const fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    #[must_use]
    pub const fn reached_snapshot_end(&self) -> bool {
        self.reached_snapshot_end
    }

    #[must_use]
    pub fn source_chunks(&self) -> &[SourceChunkDigest] {
        &self.source_chunks
    }

    #[must_use]
    pub const fn previous_partial_chunk(&self) -> Option<SourceChunkDigest> {
        self.previous_partial_chunk
    }
}

impl fmt::Debug for ReadBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadBatch")
            .field("checkpoint", &self.checkpoint)
            .field("events_count", &self.events.len())
            .field("relations_count", &self.relations.len())
            .field("diagnostics", &self.diagnostics)
            .field("parser_diagnostics", &self.parser_diagnostics)
            .field("bytes_read", &self.bytes_read)
            .field("reached_snapshot_end", &self.reached_snapshot_end)
            .field("source_chunks_count", &self.source_chunks.len())
            .field(
                "has_previous_partial_chunk",
                &self.previous_partial_chunk.is_some(),
            )
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct SourceProbe {
    physical_identity: Option<PhysicalFileIdentity>,
    logical_identity: LogicalFileIdentity,
    file_length: u64,
    modified_time_ns: Option<i64>,
}

impl SourceProbe {
    #[must_use]
    pub const fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        self.physical_identity
    }

    #[must_use]
    pub const fn logical_identity(&self) -> LogicalFileIdentity {
        self.logical_identity
    }

    #[must_use]
    pub const fn file_length(&self) -> u64 {
        self.file_length
    }

    #[must_use]
    pub const fn modified_time_ns(&self) -> Option<i64> {
        self.modified_time_ns
    }
}

impl fmt::Debug for SourceProbe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceProbe")
            .field("physical_identity", &Redacted)
            .field("logical_identity", &Redacted)
            .field("file_length", &self.file_length)
            .field("modified_time_ns", &self.modified_time_ns)
            .finish()
    }
}

/// Stable path-private identity for one logical provider source file.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct LogicalFileIdentity([u8; 32]);

impl LogicalFileIdentity {
    /// Constructs an identity from a controlled persistent representation.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the opaque digest for equality checks and controlled persistence.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for LogicalFileIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LogicalFileIdentity([redacted])")
    }
}

/// Derives a stable identity without retaining or exposing the source path.
#[must_use]
pub fn logical_file_identity(descriptor: &SourceFileDescriptor) -> LogicalFileIdentity {
    let mut hasher = Sha256::new();
    hasher.update(LOGICAL_FILE_DOMAIN);
    update_frame(&mut hasher, descriptor.profile_id().as_str().as_bytes());
    match descriptor.source_kind() {
        SourceKind::Active | SourceKind::Archived => {
            update_frame(&mut hasher, PROFILE_HISTORY_CLASS);
        }
        SourceKind::Direct => {
            update_frame(&mut hasher, DIRECT_CLASS);
            update_frame(&mut hasher, descriptor.source_id().as_str().as_bytes());
        }
    }
    for component in descriptor.relative_path().components() {
        if let Component::Normal(value) = component {
            update_native_component(&mut hasher, value);
        }
    }
    LogicalFileIdentity::from_bytes(hasher.finalize().into())
}

/// Opens and validates one source, then creates a zero-offset checkpoint without
/// reading source content.
pub fn initialize_source_checkpoint(
    descriptor: &SourceFileDescriptor,
) -> Result<ReaderCheckpointV1, ReaderError> {
    let source = source::open_source(descriptor)?;
    ReaderCheckpointV1::new(ReaderCheckpointParts {
        parser_schema_version: PARSER_SCHEMA_VERSION,
        physical_identity: source.physical_identity,
        logical_identity: logical_file_identity(descriptor),
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: source.file_length,
        modified_time_ns: source.modified_time_ns,
        anchor: BoundaryAnchor::new(0, 0, [0; 32])
            .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?,
        resume: ParserState::new().snapshot(),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: VerificationLevel::FullPrefix,
    })
    .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))
}

pub fn read_source_batch(
    descriptor: &SourceFileDescriptor,
    checkpoint: Option<&ReaderCheckpointV1>,
    mut should_cancel: impl FnMut() -> bool,
) -> Result<ReaderOutcome, ReaderError> {
    if should_cancel() {
        return Err(ReaderError::new(ReaderErrorCode::Cancelled));
    }
    let logical_identity = logical_file_identity(descriptor);
    let mut source = source::open_source(descriptor)?;
    let probe = SourceProbe {
        physical_identity: source.physical_identity,
        logical_identity,
        file_length: source.file_length,
        modified_time_ns: source.modified_time_ns,
    };

    let (start_offset, committed_offset, state, verification) = if let Some(checkpoint) = checkpoint
    {
        if checkpoint.logical_identity() != logical_identity
            || checkpoint.physical_identity() != source.physical_identity
        {
            return Ok(ReaderOutcome::RebuildRequired(
                RebuildReason::IdentityChanged,
            ));
        }
        if source.file_length < checkpoint.observed_file_length() {
            return Ok(ReaderOutcome::RebuildRequired(RebuildReason::Truncated));
        }
        if source.file_length == checkpoint.observed_file_length()
            && source.modified_time_ns != checkpoint.modified_time_ns()
        {
            return Ok(ReaderOutcome::RebuildRequired(
                RebuildReason::RewriteDetected,
            ));
        }
        if !checkpoint.anchor().is_empty() {
            let anchor = checkpoint.anchor();
            let observed =
                source::hash_range(&mut source.file, anchor.start(), u64::from(anchor.len()))?;
            if &observed != anchor.sha256() {
                return Ok(ReaderOutcome::RebuildRequired(
                    RebuildReason::AnchorMismatch,
                ));
            }
        }
        let same_observation = source.file_length == checkpoint.observed_file_length()
            && source.modified_time_ns == checkpoint.modified_time_ns();
        if same_observation
            && ((checkpoint.incomplete_tail() && !checkpoint.discarding_oversized_line())
                || checkpoint.scan_offset() == source.file_length)
        {
            return Ok(ReaderOutcome::Unchanged(probe));
        }
        let state = ParserState::from_resume(checkpoint.resume().clone())
            .map_err(|_| ReaderError::new(ReaderErrorCode::ResumeInvalid))?;
        (
            checkpoint.scan_offset(),
            checkpoint.committed_offset(),
            state,
            checkpoint.verification(),
        )
    } else {
        (0, 0, ParserState::new(), VerificationLevel::Incremental)
    };

    source
        .file
        .seek(SeekFrom::Start(start_offset))
        .map_err(|_| ReaderError::new(ReaderErrorCode::SeekFailed))?;
    let remaining = source.file_length.saturating_sub(start_offset);
    let mut reader = BufReader::with_capacity(READ_BUFFER_BYTES, source.file.take(remaining));
    let framed = framing::read_lines(
        &mut reader,
        framing::FramingInput {
            descriptor,
            start_offset,
            committed_offset,
            state,
            snapshot_end_offset: source.file_length,
            discarding_oversized_line: checkpoint
                .is_some_and(ReaderCheckpointV1::discarding_oversized_line),
            source_verification: match verification {
                VerificationLevel::Incremental => ObservationVerification::Incremental,
                VerificationLevel::FullPrefix => ObservationVerification::FullPrefix,
            },
        },
        &mut should_cancel,
    )?;
    let mut file = reader.into_inner().into_inner();
    if should_cancel() {
        return Err(ReaderError::new(ReaderErrorCode::Cancelled));
    }
    let observed_range_sha256 = source::hash_range(&mut file, start_offset, framed.bytes_read)
        .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    if observed_range_sha256 != framed.consumed_sha256 {
        return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
    }

    let anchor = source::boundary_anchor(&mut file, framed.committed_offset)
        .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    let verified_chunk_end = if framed.discarding_oversized_line {
        framed.scan_offset
    } else {
        framed.committed_offset
    };
    let (source_chunks, previous_partial_chunk) = if verified_chunk_end > start_offset {
        let first_chunk = start_offset / SOURCE_CHUNK_BYTES;
        if start_offset % SOURCE_CHUNK_BYTES == 0 {
            (
                source::source_chunks_for_range(&mut file, first_chunk, verified_chunk_end)
                    .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?,
                None,
            )
        } else {
            let (previous, current) =
                source::extended_partial_chunk(&mut file, start_offset, verified_chunk_end)
                    .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
            let mut chunks = source::source_chunks_for_range(
                &mut file,
                first_chunk.saturating_add(1),
                verified_chunk_end,
            )
            .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
            chunks.insert(0, current);
            (chunks, Some(previous))
        }
    } else {
        (Vec::new(), None)
    };
    let (final_identity, final_length, final_modified_time_ns) =
        source::current_handle_observation(&file)
            .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    if observation_invalidated(
        source.physical_identity,
        source.file_length,
        source.modified_time_ns,
        final_identity,
        final_length,
        final_modified_time_ns,
    ) {
        return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
    }
    if let Some(expected) = source.physical_identity {
        source::revalidate_path_identity(descriptor, expected)
            .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    }
    let checkpoint = ReaderCheckpointV1::new(ReaderCheckpointParts {
        parser_schema_version: PARSER_SCHEMA_VERSION,
        physical_identity: source.physical_identity,
        logical_identity,
        committed_offset: framed.committed_offset,
        scan_offset: framed.scan_offset,
        observed_file_length: source.file_length,
        modified_time_ns: source.modified_time_ns,
        anchor,
        resume: framed.state.snapshot(),
        discarding_oversized_line: framed.discarding_oversized_line,
        incomplete_tail: framed.incomplete_tail,
        verification,
    })
    .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;

    Ok(ReaderOutcome::Batch(ReadBatch {
        checkpoint,
        events: framed.events,
        relations: framed.relations,
        diagnostics: framed.diagnostics,
        parser_diagnostics: framed.parser_diagnostics,
        bytes_read: framed.bytes_read,
        reached_snapshot_end: framed.reached_snapshot_end,
        source_chunks,
        previous_partial_chunk,
    }))
}

pub fn verify_full_prefix(
    descriptor: &SourceFileDescriptor,
    checkpoint: &ReaderCheckpointV1,
    mut expected_chunk: impl FnMut(u64) -> Option<SourceChunkDigest>,
    mut should_cancel: impl FnMut() -> bool,
) -> Result<IntegrityReport, ReaderError> {
    if should_cancel() {
        return Ok(IntegrityReport::Cancelled);
    }
    if checkpoint.logical_identity() != logical_file_identity(descriptor) {
        return Err(ReaderError::new(ReaderErrorCode::CheckpointInvalid));
    }
    let mut source = source::open_source(descriptor)?;
    if checkpoint.physical_identity() != source.physical_identity {
        return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
    }
    let covered_bytes = checkpoint.scan_offset();
    if source.file_length < covered_bytes {
        return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
    }

    let chunks = covered_bytes.div_ceil(SOURCE_CHUNK_BYTES);
    for index in 0..chunks {
        if should_cancel() {
            return Ok(IntegrityReport::Cancelled);
        }
        let start = index
            .checked_mul(SOURCE_CHUNK_BYTES)
            .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
        let len = covered_bytes.saturating_sub(start).min(SOURCE_CHUNK_BYTES);
        let actual = SourceChunkDigest::from_verified_parts(
            index,
            u32::try_from(len).map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?,
            source::hash_range(&mut source.file, start, len)?,
        );
        let expected = expected_chunk(index)
            .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
        if actual != expected {
            return Ok(IntegrityReport::Mismatch { chunk_index: index });
        }
    }

    if should_cancel() {
        return Ok(IntegrityReport::Cancelled);
    }
    let (final_identity, final_length, final_modified_time_ns) =
        source::current_handle_observation(&source.file)
            .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    if observation_invalidated(
        source.physical_identity,
        source.file_length,
        source.modified_time_ns,
        final_identity,
        final_length,
        final_modified_time_ns,
    ) {
        return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
    }
    if let Some(expected) = source.physical_identity {
        source::revalidate_path_identity(descriptor, expected)
            .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    }
    Ok(IntegrityReport::Verified {
        chunks,
        covered_bytes,
    })
}

fn observation_invalidated(
    initial_identity: Option<PhysicalFileIdentity>,
    initial_length: u64,
    initial_modified_time_ns: Option<i64>,
    final_identity: Option<PhysicalFileIdentity>,
    final_length: u64,
    final_modified_time_ns: Option<i64>,
) -> bool {
    final_identity != initial_identity
        || final_length < initial_length
        || (final_length == initial_length && final_modified_time_ns != initial_modified_time_ns)
}

fn update_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(windows)]
fn update_native_component(hasher: &mut Sha256, value: &std::ffi::OsStr) {
    use std::os::windows::ffi::OsStrExt;

    let byte_len = value.encode_wide().count().saturating_mul(2);
    hasher.update(u64::try_from(byte_len).unwrap_or(u64::MAX).to_le_bytes());
    for unit in value.encode_wide() {
        hasher.update(unit.to_le_bytes());
    }
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

#[cfg(not(windows))]
fn update_native_component(hasher: &mut Sha256, value: &std::ffi::OsStr) {
    use std::os::unix::ffi::OsStrExt;

    update_frame(hasher, value.as_bytes());
}
