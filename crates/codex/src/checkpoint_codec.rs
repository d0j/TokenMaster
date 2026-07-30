use std::fmt;

use tokenmaster_platform::PhysicalFileIdentity;

use crate::{
    BoundaryAnchor, LogicalFileIdentity, MAX_RESUME_BYTES, ParserResumeState,
    READER_CHECKPOINT_SCHEMA_VERSION, ReaderCheckpointParts, ReaderCheckpointV1, VerificationLevel,
};

pub const MAX_CODEX_CHECKPOINT_BYTES: usize = 32 * 1024;

const MAGIC: [u8; 4] = *b"TMCP";
const ENVELOPE_VERSION: u16 = 1;
const FLAG_PHYSICAL_IDENTITY: u16 = 1 << 0;
const FLAG_MODIFIED_TIME: u16 = 1 << 1;
const FLAG_DISCARDING_OVERSIZED_LINE: u16 = 1 << 2;
const FLAG_INCOMPLETE_TAIL: u16 = 1 << 3;
const FLAG_FULL_PREFIX: u16 = 1 << 4;
const KNOWN_FLAGS: u16 = FLAG_PHYSICAL_IDENTITY
    | FLAG_MODIFIED_TIME
    | FLAG_DISCARDING_OVERSIZED_LINE
    | FLAG_INCOMPLETE_TAIL
    | FLAG_FULL_PREFIX;
const FIXED_BYTES: usize = 154;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexCheckpointErrorCode {
    InvalidEnvelope,
    UnsupportedVersion,
    CapacityExceeded,
    InvalidFlags,
    IdentityMismatch,
    TrailingBytes,
    InvalidResume,
    InvalidCheckpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodexCheckpointError {
    code: CodexCheckpointErrorCode,
}

impl CodexCheckpointError {
    const fn new(code: CodexCheckpointErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> CodexCheckpointErrorCode {
        self.code
    }
}

impl fmt::Display for CodexCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            CodexCheckpointErrorCode::InvalidEnvelope => "invalid Codex checkpoint envelope",
            CodexCheckpointErrorCode::UnsupportedVersion => "unsupported Codex checkpoint version",
            CodexCheckpointErrorCode::CapacityExceeded => "Codex checkpoint capacity exceeded",
            CodexCheckpointErrorCode::InvalidFlags => "invalid Codex checkpoint flags",
            CodexCheckpointErrorCode::IdentityMismatch => "Codex checkpoint identity mismatched",
            CodexCheckpointErrorCode::TrailingBytes => "Codex checkpoint has trailing bytes",
            CodexCheckpointErrorCode::InvalidResume => "invalid Codex parser resume state",
            CodexCheckpointErrorCode::InvalidCheckpoint => "invalid Codex reader checkpoint",
        })
    }
}

impl std::error::Error for CodexCheckpointError {}

#[derive(Clone, Eq, PartialEq)]
pub struct CodexCheckpointV1 {
    reader: ReaderCheckpointV1,
}

impl CodexCheckpointV1 {
    #[must_use]
    pub const fn new(reader: ReaderCheckpointV1) -> Self {
        Self { reader }
    }

    #[must_use]
    pub const fn reader(&self) -> &ReaderCheckpointV1 {
        &self.reader
    }

    #[must_use]
    pub fn into_reader(self) -> ReaderCheckpointV1 {
        self.reader
    }

    pub fn encode(&self) -> Result<Vec<u8>, CodexCheckpointError> {
        let resume = serde_json::to_vec(self.reader.resume())
            .map_err(|_| CodexCheckpointError::new(CodexCheckpointErrorCode::InvalidResume))?;
        let total = FIXED_BYTES
            .checked_add(resume.len())
            .ok_or_else(|| CodexCheckpointError::new(CodexCheckpointErrorCode::CapacityExceeded))?;
        if resume.len() > MAX_RESUME_BYTES || total > MAX_CODEX_CHECKPOINT_BYTES {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::CapacityExceeded,
            ));
        }
        let resume_len = u32::try_from(resume.len())
            .map_err(|_| CodexCheckpointError::new(CodexCheckpointErrorCode::CapacityExceeded))?;

        let mut flags = 0_u16;
        flags |= self
            .reader
            .physical_identity()
            .map_or(0, |_| FLAG_PHYSICAL_IDENTITY);
        flags |= self
            .reader
            .modified_time_ns()
            .map_or(0, |_| FLAG_MODIFIED_TIME);
        if self.reader.discarding_oversized_line() {
            flags |= FLAG_DISCARDING_OVERSIZED_LINE;
        }
        if self.reader.incomplete_tail() {
            flags |= FLAG_INCOMPLETE_TAIL;
        }
        if self.reader.verification() == VerificationLevel::FullPrefix {
            flags |= FLAG_FULL_PREFIX;
        }

        let mut encoded = Vec::with_capacity(total);
        encoded.extend_from_slice(&MAGIC);
        push_u16(&mut encoded, ENVELOPE_VERSION);
        push_u16(&mut encoded, self.reader.checkpoint_schema_version());
        push_u16(&mut encoded, self.reader.parser_schema_version());
        push_u16(&mut encoded, flags);
        encoded.extend_from_slice(self.reader.logical_identity().as_bytes());
        if let Some(identity) = self.reader.physical_identity() {
            encoded.extend_from_slice(identity.as_bytes());
        } else {
            encoded.extend_from_slice(&[0; 32]);
        }
        push_u64(&mut encoded, self.reader.committed_offset());
        push_u64(&mut encoded, self.reader.scan_offset());
        push_u64(&mut encoded, self.reader.observed_file_length());
        push_i64(&mut encoded, self.reader.modified_time_ns().unwrap_or(0));
        push_u64(&mut encoded, self.reader.anchor().start());
        push_u16(&mut encoded, self.reader.anchor().len());
        encoded.extend_from_slice(self.reader.anchor().sha256());
        push_u32(&mut encoded, resume_len);
        encoded.extend_from_slice(&resume);
        debug_assert_eq!(encoded.len(), total);
        Ok(encoded)
    }

    pub fn decode(
        encoded: &[u8],
        expected_identity: LogicalFileIdentity,
    ) -> Result<Self, CodexCheckpointError> {
        if encoded.len() > MAX_CODEX_CHECKPOINT_BYTES {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::CapacityExceeded,
            ));
        }
        if encoded.len() < FIXED_BYTES || encoded[..4] != MAGIC {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::InvalidEnvelope,
            ));
        }

        let mut cursor = 4;
        if take_u16(encoded, &mut cursor) != ENVELOPE_VERSION
            || take_u16(encoded, &mut cursor) != READER_CHECKPOINT_SCHEMA_VERSION
        {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::UnsupportedVersion,
            ));
        }
        let parser_schema_version = take_u16(encoded, &mut cursor);
        let flags = take_u16(encoded, &mut cursor);
        if flags & !KNOWN_FLAGS != 0 {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::InvalidFlags,
            ));
        }

        let logical_identity = LogicalFileIdentity::from_bytes(take_32(encoded, &mut cursor));
        if logical_identity != expected_identity {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::IdentityMismatch,
            ));
        }
        let physical_bytes = take_32(encoded, &mut cursor);
        let physical_identity = if flags & FLAG_PHYSICAL_IDENTITY != 0 {
            Some(PhysicalFileIdentity::from_persisted_bytes(physical_bytes))
        } else {
            if physical_bytes != [0; 32] {
                return Err(CodexCheckpointError::new(
                    CodexCheckpointErrorCode::InvalidFlags,
                ));
            }
            None
        };
        let committed_offset = take_u64(encoded, &mut cursor);
        let scan_offset = take_u64(encoded, &mut cursor);
        let observed_file_length = take_u64(encoded, &mut cursor);
        let modified_raw = take_i64(encoded, &mut cursor);
        let modified_time_ns = if flags & FLAG_MODIFIED_TIME != 0 {
            Some(modified_raw)
        } else {
            if modified_raw != 0 {
                return Err(CodexCheckpointError::new(
                    CodexCheckpointErrorCode::InvalidFlags,
                ));
            }
            None
        };
        let anchor_start = take_u64(encoded, &mut cursor);
        let anchor_len = take_u16(encoded, &mut cursor);
        let anchor_sha256 = take_32(encoded, &mut cursor);
        let resume_len = usize::try_from(take_u32(encoded, &mut cursor))
            .map_err(|_| CodexCheckpointError::new(CodexCheckpointErrorCode::CapacityExceeded))?;
        if resume_len > MAX_RESUME_BYTES {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::CapacityExceeded,
            ));
        }
        let expected_len = FIXED_BYTES
            .checked_add(resume_len)
            .ok_or_else(|| CodexCheckpointError::new(CodexCheckpointErrorCode::CapacityExceeded))?;
        if encoded.len() < expected_len {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::InvalidEnvelope,
            ));
        }
        if encoded.len() > expected_len {
            return Err(CodexCheckpointError::new(
                CodexCheckpointErrorCode::TrailingBytes,
            ));
        }
        let resume: ParserResumeState = serde_json::from_slice(&encoded[cursor..expected_len])
            .map_err(|_| CodexCheckpointError::new(CodexCheckpointErrorCode::InvalidResume))?;
        let verification = if flags & FLAG_FULL_PREFIX == 0 {
            VerificationLevel::Incremental
        } else {
            VerificationLevel::FullPrefix
        };
        let anchor = BoundaryAnchor::new(anchor_start, anchor_len, anchor_sha256)
            .map_err(|_| CodexCheckpointError::new(CodexCheckpointErrorCode::InvalidCheckpoint))?;
        let reader = ReaderCheckpointV1::new(ReaderCheckpointParts {
            parser_schema_version,
            physical_identity,
            logical_identity,
            committed_offset,
            scan_offset,
            observed_file_length,
            modified_time_ns,
            anchor,
            resume,
            discarding_oversized_line: flags & FLAG_DISCARDING_OVERSIZED_LINE != 0,
            incomplete_tail: flags & FLAG_INCOMPLETE_TAIL != 0,
            verification,
        })
        .map_err(|_| CodexCheckpointError::new(CodexCheckpointErrorCode::InvalidCheckpoint))?;
        Ok(Self { reader })
    }
}

impl fmt::Debug for CodexCheckpointV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexCheckpointV1")
            .field("checkpoint", &Redacted)
            .finish()
    }
}

fn push_u16(target: &mut Vec<u8>, value: u16) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(target: &mut Vec<u8>, value: u64) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_i64(target: &mut Vec<u8>, value: i64) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn take_u16(source: &[u8], cursor: &mut usize) -> u16 {
    u16::from_le_bytes(take_array(source, cursor))
}

fn take_u32(source: &[u8], cursor: &mut usize) -> u32 {
    u32::from_le_bytes(take_array(source, cursor))
}

fn take_u64(source: &[u8], cursor: &mut usize) -> u64 {
    u64::from_le_bytes(take_array(source, cursor))
}

fn take_i64(source: &[u8], cursor: &mut usize) -> i64 {
    i64::from_le_bytes(take_array(source, cursor))
}

fn take_32(source: &[u8], cursor: &mut usize) -> [u8; 32] {
    take_array(source, cursor)
}

fn take_array<const N: usize>(source: &[u8], cursor: &mut usize) -> [u8; N] {
    let mut value = [0_u8; N];
    value.copy_from_slice(&source[*cursor..*cursor + N]);
    *cursor += N;
    value
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}
