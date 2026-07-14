use std::fmt;

use serde::Serialize;
use tokenmaster_platform::PhysicalFileIdentity;

use super::LogicalFileIdentity;
use crate::{PARSER_SCHEMA_VERSION, ParserResumeStateV1, ParserState};

pub const READER_CHECKPOINT_SCHEMA_VERSION: u16 = 1;
pub const MAX_ANCHOR_BYTES: u16 = 4096;
pub const MAX_RESUME_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationLevel {
    Incremental,
    FullPrefix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderCheckpointErrorCode {
    UnsupportedParserVersion,
    InvalidOffset,
    InvalidAnchor,
    InvalidFlags,
    InvalidResume,
    ResumeTooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReaderCheckpointError {
    code: ReaderCheckpointErrorCode,
    limit: Option<u64>,
}

impl ReaderCheckpointError {
    const fn new(code: ReaderCheckpointErrorCode) -> Self {
        Self { code, limit: None }
    }

    const fn with_limit(code: ReaderCheckpointErrorCode, limit: u64) -> Self {
        Self {
            code,
            limit: Some(limit),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ReaderCheckpointErrorCode {
        self.code
    }

    #[must_use]
    pub const fn limit(&self) -> Option<u64> {
        self.limit
    }
}

impl fmt::Display for ReaderCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            ReaderCheckpointErrorCode::UnsupportedParserVersion => {
                "unsupported parser checkpoint version"
            }
            ReaderCheckpointErrorCode::InvalidOffset => "invalid reader checkpoint offset",
            ReaderCheckpointErrorCode::InvalidAnchor => "invalid reader checkpoint anchor",
            ReaderCheckpointErrorCode::InvalidFlags => "invalid reader checkpoint flags",
            ReaderCheckpointErrorCode::InvalidResume => "invalid parser resume checkpoint",
            ReaderCheckpointErrorCode::ResumeTooLarge => "parser resume checkpoint is too large",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ReaderCheckpointError {}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BoundaryAnchor {
    start: u64,
    len: u16,
    sha256: [u8; 32],
}

impl BoundaryAnchor {
    pub fn new(start: u64, len: u16, sha256: [u8; 32]) -> Result<Self, ReaderCheckpointError> {
        if len > MAX_ANCHOR_BYTES || start.checked_add(u64::from(len)).is_none() {
            return Err(ReaderCheckpointError::with_limit(
                ReaderCheckpointErrorCode::InvalidAnchor,
                u64::from(MAX_ANCHOR_BYTES),
            ));
        }
        Ok(Self { start, len, sha256 })
    }

    #[must_use]
    pub const fn start(&self) -> u64 {
        self.start
    }

    #[must_use]
    pub const fn len(&self) -> u16 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    const fn end(&self) -> Option<u64> {
        self.start.checked_add(self.len as u64)
    }
}

impl fmt::Debug for BoundaryAnchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundaryAnchor")
            .field("start", &self.start)
            .field("len", &self.len)
            .field("sha256", &Redacted)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ReaderCheckpointParts {
    pub parser_schema_version: u16,
    pub physical_identity: Option<PhysicalFileIdentity>,
    pub logical_identity: LogicalFileIdentity,
    pub committed_offset: u64,
    pub scan_offset: u64,
    pub observed_file_length: u64,
    pub modified_time_ns: Option<i64>,
    pub anchor: BoundaryAnchor,
    pub resume: ParserResumeStateV1,
    pub discarding_oversized_line: bool,
    pub incomplete_tail: bool,
    pub verification: VerificationLevel,
}

impl fmt::Debug for ReaderCheckpointParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        checkpoint_debug("ReaderCheckpointParts", self, formatter)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ReaderCheckpointV1 {
    parts: ReaderCheckpointParts,
}

impl ReaderCheckpointV1 {
    pub fn new(parts: ReaderCheckpointParts) -> Result<Self, ReaderCheckpointError> {
        if parts.parser_schema_version != PARSER_SCHEMA_VERSION {
            return Err(ReaderCheckpointError::new(
                ReaderCheckpointErrorCode::UnsupportedParserVersion,
            ));
        }
        if ParserState::from_resume(parts.resume.clone()).is_err() {
            return Err(ReaderCheckpointError::new(
                ReaderCheckpointErrorCode::InvalidResume,
            ));
        }
        let resume_bytes = serde_json::to_vec(&parts.resume)
            .map_err(|_| ReaderCheckpointError::new(ReaderCheckpointErrorCode::InvalidResume))?;
        if resume_bytes.len() > MAX_RESUME_BYTES {
            return Err(ReaderCheckpointError::with_limit(
                ReaderCheckpointErrorCode::ResumeTooLarge,
                MAX_RESUME_BYTES as u64,
            ));
        }
        if parts.scan_offset < parts.committed_offset
            || parts.scan_offset > parts.observed_file_length
            || (!parts.discarding_oversized_line && parts.scan_offset != parts.committed_offset)
        {
            return Err(ReaderCheckpointError::new(
                ReaderCheckpointErrorCode::InvalidOffset,
            ));
        }
        if parts
            .anchor
            .end()
            .is_none_or(|end| end > parts.committed_offset)
        {
            return Err(ReaderCheckpointError::new(
                ReaderCheckpointErrorCode::InvalidAnchor,
            ));
        }
        if parts.discarding_oversized_line
            && (!parts.incomplete_tail || parts.scan_offset == parts.committed_offset)
        {
            return Err(ReaderCheckpointError::new(
                ReaderCheckpointErrorCode::InvalidFlags,
            ));
        }
        Ok(Self { parts })
    }

    #[must_use]
    pub const fn checkpoint_schema_version(&self) -> u16 {
        READER_CHECKPOINT_SCHEMA_VERSION
    }

    #[must_use]
    pub const fn parser_schema_version(&self) -> u16 {
        self.parts.parser_schema_version
    }

    #[must_use]
    pub const fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        self.parts.physical_identity
    }

    #[must_use]
    pub const fn logical_identity(&self) -> LogicalFileIdentity {
        self.parts.logical_identity
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
    pub const fn anchor(&self) -> BoundaryAnchor {
        self.parts.anchor
    }

    #[must_use]
    pub const fn resume(&self) -> &ParserResumeStateV1 {
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
    pub const fn verification(&self) -> VerificationLevel {
        self.parts.verification
    }
}

impl fmt::Debug for ReaderCheckpointV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        checkpoint_debug("ReaderCheckpointV1", &self.parts, formatter)
    }
}

fn checkpoint_debug(
    name: &str,
    parts: &ReaderCheckpointParts,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    formatter
        .debug_struct(name)
        .field(
            "checkpoint_schema_version",
            &READER_CHECKPOINT_SCHEMA_VERSION,
        )
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

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}
