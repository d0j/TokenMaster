//! Path-bearing file descriptors intentionally do not implement serialization.
//!
//! ```compile_fail
//! fn assert_serialize<T: serde::Serialize>() {}
//! assert_serialize::<tokenmaster_codex::SourceFileDescriptor>();
//! ```
//!
//! Reader identities and validated checkpoints also require explicit store conversion.
//!
//! ```compile_fail
//! fn assert_serialize<T: serde::Serialize>() {}
//! assert_serialize::<tokenmaster_codex::LogicalFileIdentity>();
//! ```
//!
//! ```compile_fail
//! fn assert_serialize<T: serde::Serialize>() {}
//! assert_serialize::<tokenmaster_codex::ReaderCheckpointV1>();
//! ```
//!
//! ```compile_fail
//! fn assert_serialize<T: serde::Serialize>() {}
//! assert_serialize::<tokenmaster_codex::SourceChunkDigest>();
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod file_identity;
mod files;
mod identity;
mod parser;
mod path_policy;
mod provider;
mod reader;
mod roots;

pub use files::{
    EnumerationCompletion, EnumerationDiagnosticCode, EnumerationDiagnostics, EnumerationError,
    EnumerationErrorCode, EnumerationReport, FileMetadataHint, MAX_ENUMERATION_DEPTH, SinkDecision,
    SourceFileDescriptor, enumerate_profile_sources,
};
pub use identity::profile_id_for_root;
pub use parser::{
    LONG_CONTEXT_THRESHOLD, MAX_LINE_BYTES, MAX_TOOL_NAME_BYTES, MAX_TOOL_NAMES,
    PARSER_SCHEMA_VERSION, ParseContext, ParseOutcome, ParserDiagnosticCode, ParserDiagnostics,
    ParserResumeError, ParserResumeErrorCode, ParserResumeState, ParserState, ToolCountEntry,
    parse_line,
};
pub use provider::CodexProvider;
pub use reader::{
    BoundaryAnchor, IntegrityReport, LogicalFileIdentity, MAX_ANCHOR_BYTES,
    MAX_BATCH_COMPLETE_BYTES, MAX_BATCH_EVENTS, MAX_RESUME_BYTES, READ_BUFFER_BYTES,
    READER_CHECKPOINT_SCHEMA_VERSION, ReadBatch, ReaderCheckpointError, ReaderCheckpointErrorCode,
    ReaderCheckpointParts, ReaderCheckpointV1, ReaderDiagnosticCode, ReaderDiagnostics,
    ReaderError, ReaderErrorCode, ReaderOutcome, RebuildReason, SOURCE_CHUNK_BYTES,
    SourceChunkDigest, SourceProbe, VerificationLevel, logical_file_identity, read_source_batch,
    verify_full_prefix,
};
pub use roots::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
