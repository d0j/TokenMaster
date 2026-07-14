use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tokenmaster_domain::{UsageProfileId, UsageSessionId, UsageSourceId};
use tokenmaster_provider::{SourceDescriptor, SourceKind};

mod walk;

pub const MAX_ENUMERATION_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnumerationCompletion {
    Complete,
    Partial,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinkDecision {
    Continue,
    Cancel,
    Fail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnumerationErrorCode {
    EmptySourceSet,
    MixedProfiles,
    DuplicateSourceKind,
    DirectSourceConflict,
    InvalidRoot,
    SinkFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EnumerationError {
    code: EnumerationErrorCode,
    limit: Option<u64>,
}

impl EnumerationError {
    pub(crate) const fn new(code: EnumerationErrorCode) -> Self {
        Self { code, limit: None }
    }

    pub(crate) const fn with_limit(code: EnumerationErrorCode, limit: u64) -> Self {
        Self {
            code,
            limit: Some(limit),
        }
    }

    #[must_use]
    pub const fn code(&self) -> EnumerationErrorCode {
        self.code
    }

    #[must_use]
    pub const fn limit(&self) -> Option<u64> {
        self.limit
    }
}

impl fmt::Display for EnumerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            EnumerationErrorCode::EmptySourceSet => "source set is empty",
            EnumerationErrorCode::MixedProfiles => "source set contains mixed profiles",
            EnumerationErrorCode::DuplicateSourceKind => "source kind is duplicated",
            EnumerationErrorCode::DirectSourceConflict => {
                "direct source conflicts with profile sources"
            }
            EnumerationErrorCode::InvalidRoot => "source root is invalid",
            EnumerationErrorCode::SinkFailed => "enumeration sink failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for EnumerationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumerationDiagnosticCode {
    VisitedDirectory,
    EmittedFile,
    ArchiveShadowed,
    NonJsonl,
    NonRegular,
    ReparsePoint,
    PathRejected,
    DepthRejected,
    UnreadableEntry,
    Cancelled,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct EnumerationDiagnostics {
    visited_directories: u64,
    emitted_files: u64,
    archive_shadowed: u64,
    non_jsonl: u64,
    non_regular: u64,
    reparse_points: u64,
    path_rejected: u64,
    depth_rejected: u64,
    unreadable_entries: u64,
    cancelled: u64,
}

impl EnumerationDiagnostics {
    pub(crate) fn record(&mut self, code: EnumerationDiagnosticCode) {
        let counter = match code {
            EnumerationDiagnosticCode::VisitedDirectory => &mut self.visited_directories,
            EnumerationDiagnosticCode::EmittedFile => &mut self.emitted_files,
            EnumerationDiagnosticCode::ArchiveShadowed => &mut self.archive_shadowed,
            EnumerationDiagnosticCode::NonJsonl => &mut self.non_jsonl,
            EnumerationDiagnosticCode::NonRegular => &mut self.non_regular,
            EnumerationDiagnosticCode::ReparsePoint => &mut self.reparse_points,
            EnumerationDiagnosticCode::PathRejected => &mut self.path_rejected,
            EnumerationDiagnosticCode::DepthRejected => &mut self.depth_rejected,
            EnumerationDiagnosticCode::UnreadableEntry => &mut self.unreadable_entries,
            EnumerationDiagnosticCode::Cancelled => &mut self.cancelled,
        };
        *counter = counter.saturating_add(1);
    }

    #[must_use]
    pub const fn count(&self, code: EnumerationDiagnosticCode) -> u64 {
        match code {
            EnumerationDiagnosticCode::VisitedDirectory => self.visited_directories,
            EnumerationDiagnosticCode::EmittedFile => self.emitted_files,
            EnumerationDiagnosticCode::ArchiveShadowed => self.archive_shadowed,
            EnumerationDiagnosticCode::NonJsonl => self.non_jsonl,
            EnumerationDiagnosticCode::NonRegular => self.non_regular,
            EnumerationDiagnosticCode::ReparsePoint => self.reparse_points,
            EnumerationDiagnosticCode::PathRejected => self.path_rejected,
            EnumerationDiagnosticCode::DepthRejected => self.depth_rejected,
            EnumerationDiagnosticCode::UnreadableEntry => self.unreadable_entries,
            EnumerationDiagnosticCode::Cancelled => self.cancelled,
        }
    }

    #[must_use]
    pub const fn emitted_files(&self) -> u64 {
        self.emitted_files
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EnumerationReport {
    completion: EnumerationCompletion,
    diagnostics: EnumerationDiagnostics,
}

impl EnumerationReport {
    pub(crate) const fn new(
        completion: EnumerationCompletion,
        diagnostics: EnumerationDiagnostics,
    ) -> Self {
        Self {
            completion,
            diagnostics,
        }
    }

    #[must_use]
    pub const fn completion(&self) -> EnumerationCompletion {
        self.completion
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &EnumerationDiagnostics {
        &self.diagnostics
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct FileMetadataHint {
    len: u64,
    modified_unix_nanos: Option<i128>,
}

impl FileMetadataHint {
    pub(crate) const fn new(len: u64, modified_unix_nanos: Option<i128>) -> Self {
        Self {
            len,
            modified_unix_nanos,
        }
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn modified_unix_nanos(&self) -> Option<i128> {
        self.modified_unix_nanos
    }
}

pub struct SourceFileDescriptor {
    profile_id: Arc<UsageProfileId>,
    source_id: Arc<UsageSourceId>,
    source_kind: SourceKind,
    absolute_path: PathBuf,
    relative_path: PathBuf,
    filename_session_hint: Option<UsageSessionId>,
    hashed_session_hint: UsageSessionId,
    metadata_hint: FileMetadataHint,
}

impl SourceFileDescriptor {
    #[must_use]
    pub const fn provider_id(&self) -> &'static str {
        "codex"
    }

    #[must_use]
    pub fn profile_id(&self) -> &UsageProfileId {
        self.profile_id.as_ref()
    }

    #[must_use]
    pub fn source_id(&self) -> &UsageSourceId {
        self.source_id.as_ref()
    }

    #[must_use]
    pub const fn source_kind(&self) -> SourceKind {
        self.source_kind
    }

    #[must_use]
    pub fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }

    #[must_use]
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    #[must_use]
    pub fn filename_session_hint(&self) -> Option<&UsageSessionId> {
        self.filename_session_hint.as_ref()
    }

    #[must_use]
    pub const fn hashed_session_hint(&self) -> &UsageSessionId {
        &self.hashed_session_hint
    }

    #[must_use]
    pub const fn metadata_hint(&self) -> FileMetadataHint {
        self.metadata_hint
    }
}

impl fmt::Debug for SourceFileDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceFileDescriptor")
            .field("provider_id", &"codex")
            .field("profile_id", &self.profile_id)
            .field("source_id", &self.source_id)
            .field("source_kind", &self.source_kind)
            .field("absolute_path", &"[redacted]")
            .field("relative_path", &"[redacted]")
            .field("filename_session_hint", &"[redacted]")
            .field("hashed_session_hint", &"[redacted]")
            .field("metadata_hint", &self.metadata_hint)
            .finish()
    }
}

pub(super) struct EnumerationState {
    completion: EnumerationCompletion,
    diagnostics: EnumerationDiagnostics,
}

impl Default for EnumerationState {
    fn default() -> Self {
        Self {
            completion: EnumerationCompletion::Complete,
            diagnostics: EnumerationDiagnostics::default(),
        }
    }
}

impl EnumerationState {
    pub(super) fn record(&mut self, code: EnumerationDiagnosticCode) {
        self.diagnostics.record(code);
    }

    pub(super) fn degrade(&mut self, code: EnumerationDiagnosticCode) {
        self.record(code);
        if self.completion == EnumerationCompletion::Complete {
            self.completion = EnumerationCompletion::Partial;
        }
    }

    pub(super) fn cancel(&mut self) {
        self.record(EnumerationDiagnosticCode::Cancelled);
        self.completion = EnumerationCompletion::Cancelled;
    }

    #[cfg(test)]
    pub(super) const fn completion(&self) -> EnumerationCompletion {
        self.completion
    }

    #[cfg(test)]
    pub(super) const fn diagnostics(&self) -> &EnumerationDiagnostics {
        &self.diagnostics
    }

    fn into_report(self) -> EnumerationReport {
        EnumerationReport::new(self.completion, self.diagnostics)
    }
}

pub fn enumerate_profile_sources(
    sources: &[SourceDescriptor],
    mut should_cancel: impl FnMut() -> bool,
    mut emit: impl FnMut(SourceFileDescriptor) -> SinkDecision,
) -> Result<EnumerationReport, EnumerationError> {
    validate_source_set(sources)?;
    for source in sources {
        walk::validate_root(source.path())?;
    }

    let first = sources
        .first()
        .ok_or(EnumerationError::new(EnumerationErrorCode::EmptySourceSet))?;
    let profile_id = Arc::new(
        UsageProfileId::new(first.profile_id().as_str().to_owned())
            .map_err(|_| EnumerationError::new(EnumerationErrorCode::InvalidRoot))?,
    );
    let active_root = sources
        .iter()
        .find(|source| source.kind() == SourceKind::Active)
        .map(SourceDescriptor::path);
    let mut state = EnumerationState::default();
    for kind in [SourceKind::Active, SourceKind::Direct, SourceKind::Archived] {
        for source in sources.iter().filter(|source| source.kind() == kind) {
            let identity = walk::WalkIdentity::new(
                Arc::clone(&profile_id),
                Arc::new(
                    UsageSourceId::new(source.id().as_str().to_owned())
                        .map_err(|_| EnumerationError::new(EnumerationErrorCode::InvalidRoot))?,
                ),
            );
            match walk::walk_source(
                source,
                &identity,
                if kind == SourceKind::Archived {
                    active_root
                } else {
                    None
                },
                &mut should_cancel,
                &mut emit,
                &mut state,
            )? {
                walk::WalkControl::Continue => {}
                walk::WalkControl::Cancelled => {
                    return Ok(state.into_report());
                }
            }
        }
    }

    Ok(state.into_report())
}

fn validate_source_set(sources: &[SourceDescriptor]) -> Result<(), EnumerationError> {
    let Some(first) = sources.first() else {
        return Err(EnumerationError::new(EnumerationErrorCode::EmptySourceSet));
    };
    let mut active = false;
    let mut direct = false;
    let mut archived = false;
    for source in sources {
        if source.profile_id() != first.profile_id() {
            return Err(EnumerationError::new(EnumerationErrorCode::MixedProfiles));
        }
        let seen = match source.kind() {
            SourceKind::Active => &mut active,
            SourceKind::Direct => &mut direct,
            SourceKind::Archived => &mut archived,
        };
        if *seen {
            return Err(EnumerationError::new(
                EnumerationErrorCode::DuplicateSourceKind,
            ));
        }
        *seen = true;
    }
    if direct && (active || archived) {
        return Err(EnumerationError::new(
            EnumerationErrorCode::DirectSourceConflict,
        ));
    }
    Ok(())
}
