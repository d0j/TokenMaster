use std::fs::{self, ReadDir};
use std::path::{Component, Path};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use tokenmaster_domain::{UsageProfileId, UsageSourceId};
use tokenmaster_provider::{MAX_PATH_BYTES, SourceDescriptor};

use super::{
    EnumerationDiagnosticCode, EnumerationError, EnumerationErrorCode, EnumerationState,
    FileMetadataHint, MAX_ENUMERATION_DEPTH, SinkDecision, SourceFileDescriptor,
};
use crate::file_identity::{filename_session_hint, hashed_session_hint};
use crate::path_policy::{is_reparse_point, path_byte_len, validate_local_root_namespace};

pub(super) enum WalkControl {
    Continue,
    Cancelled,
}

pub(super) struct WalkIdentity {
    profile_id: Arc<UsageProfileId>,
    source_id: Arc<UsageSourceId>,
}

impl WalkIdentity {
    pub(super) const fn new(
        profile_id: Arc<UsageProfileId>,
        source_id: Arc<UsageSourceId>,
    ) -> Self {
        Self {
            profile_id,
            source_id,
        }
    }
}

struct DirectoryFrame {
    entries: ReadDir,
    depth: usize,
}

pub(super) fn validate_root(root: &Path) -> Result<(), EnumerationError> {
    if path_byte_len(root) > MAX_PATH_BYTES {
        return Err(EnumerationError::with_limit(
            EnumerationErrorCode::InvalidRoot,
            MAX_PATH_BYTES as u64,
        ));
    }
    if validate_local_root_namespace(root).is_err() {
        return Err(EnumerationError::new(EnumerationErrorCode::InvalidRoot));
    }
    let metadata = fs::symlink_metadata(root)
        .map_err(|_| EnumerationError::new(EnumerationErrorCode::InvalidRoot))?;
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(EnumerationError::new(EnumerationErrorCode::InvalidRoot));
    }
    fs::read_dir(root)
        .map(|_| ())
        .map_err(|_| EnumerationError::new(EnumerationErrorCode::InvalidRoot))
}

pub(super) fn walk_source(
    source: &SourceDescriptor,
    identity: &WalkIdentity,
    active_root: Option<&Path>,
    should_cancel: &mut impl FnMut() -> bool,
    emit: &mut impl FnMut(SourceFileDescriptor) -> SinkDecision,
    state: &mut EnumerationState,
) -> Result<WalkControl, EnumerationError> {
    let root = source.path();
    let entries =
        fs::read_dir(root).map_err(|_| EnumerationError::new(EnumerationErrorCode::InvalidRoot))?;
    let mut frames = Vec::with_capacity(MAX_ENUMERATION_DEPTH.saturating_add(1));
    frames.push(DirectoryFrame { entries, depth: 0 });
    state.record(EnumerationDiagnosticCode::VisitedDirectory);

    while !frames.is_empty() {
        if should_cancel() {
            state.cancel();
            return Ok(WalkControl::Cancelled);
        }

        let (next, depth) = match frames.last_mut() {
            Some(frame) => (frame.entries.next(), frame.depth),
            None => break,
        };
        let Some(entry_result) = next else {
            frames.pop();
            continue;
        };
        let entry = match entry_result {
            Ok(value) => value,
            Err(_) => {
                state.degrade(EnumerationDiagnosticCode::UnreadableEntry);
                continue;
            }
        };
        let absolute_path = entry.path();
        if path_byte_len(&absolute_path) > MAX_PATH_BYTES {
            state.degrade(EnumerationDiagnosticCode::PathRejected);
            continue;
        }
        let metadata = match classify_candidate(&absolute_path, state) {
            CandidateClassification::Directory => {
                if depth >= MAX_ENUMERATION_DEPTH {
                    state.degrade(EnumerationDiagnosticCode::DepthRejected);
                    continue;
                }
                match fs::read_dir(&absolute_path) {
                    Ok(entries) => {
                        frames.push(DirectoryFrame {
                            entries,
                            depth: depth.saturating_add(1),
                        });
                        state.record(EnumerationDiagnosticCode::VisitedDirectory);
                    }
                    Err(_) => state.degrade(EnumerationDiagnosticCode::UnreadableEntry),
                }
                continue;
            }
            CandidateClassification::File(metadata) => metadata,
            CandidateClassification::Skip => continue,
        };
        if !has_jsonl_extension(&absolute_path) {
            state.record(EnumerationDiagnosticCode::NonJsonl);
            continue;
        }

        let relative_path = match absolute_path.strip_prefix(root) {
            Ok(value) if valid_relative_path(value) => value.to_path_buf(),
            _ => {
                state.degrade(EnumerationDiagnosticCode::PathRejected);
                continue;
            }
        };
        if active_root.is_some_and(|active| archive_is_shadowed(active, &relative_path, state)) {
            state.record(EnumerationDiagnosticCode::ArchiveShadowed);
            continue;
        }
        let hashed_session_hint = match hashed_session_hint(&identity.profile_id, &relative_path) {
            Ok(value) => value,
            Err(_) => {
                state.degrade(EnumerationDiagnosticCode::PathRejected);
                continue;
            }
        };
        let metadata_hint = FileMetadataHint::new(
            metadata.len(),
            metadata.modified().ok().map(system_time_to_unix_nanos),
        );
        let filename_session_hint = filename_session_hint(&relative_path);
        let descriptor = SourceFileDescriptor {
            profile_id: Arc::clone(&identity.profile_id),
            source_id: Arc::clone(&identity.source_id),
            source_kind: source.kind(),
            absolute_path,
            relative_path,
            filename_session_hint,
            hashed_session_hint,
            metadata_hint,
        };
        state.record(EnumerationDiagnosticCode::EmittedFile);
        match emit(descriptor) {
            SinkDecision::Continue => {}
            SinkDecision::Cancel => {
                state.cancel();
                return Ok(WalkControl::Cancelled);
            }
            SinkDecision::Fail => {
                return Err(EnumerationError::new(EnumerationErrorCode::SinkFailed));
            }
        }
    }

    Ok(WalkControl::Continue)
}

pub(super) enum CandidateClassification {
    Directory,
    File(fs::Metadata),
    Skip,
}

pub(super) fn classify_candidate(
    path: &Path,
    state: &mut EnumerationState,
) -> CandidateClassification {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(_) => {
            state.degrade(EnumerationDiagnosticCode::UnreadableEntry);
            return CandidateClassification::Skip;
        }
    };
    if is_reparse_point(&metadata) {
        state.degrade(EnumerationDiagnosticCode::ReparsePoint);
        return CandidateClassification::Skip;
    }
    if metadata.is_dir() {
        CandidateClassification::Directory
    } else if metadata.is_file() {
        CandidateClassification::File(metadata)
    } else {
        state.record(EnumerationDiagnosticCode::NonRegular);
        CandidateClassification::Skip
    }
}

fn archive_is_shadowed(
    active_root: &Path,
    relative_path: &Path,
    state: &mut EnumerationState,
) -> bool {
    let mut candidate = active_root.to_path_buf();
    let mut components = relative_path.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(value) = component else {
            state.degrade(EnumerationDiagnosticCode::PathRejected);
            return false;
        };
        candidate.push(value);
        if path_byte_len(&candidate) > MAX_PATH_BYTES {
            state.degrade(EnumerationDiagnosticCode::PathRejected);
            return false;
        }
        let metadata = match fs::symlink_metadata(&candidate) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
            Err(_) => {
                state.degrade(EnumerationDiagnosticCode::UnreadableEntry);
                return false;
            }
        };
        if is_reparse_point(&metadata) {
            state.degrade(EnumerationDiagnosticCode::ReparsePoint);
            return false;
        }
        if components.peek().is_some() {
            if !metadata.is_dir() {
                return false;
            }
        } else {
            return metadata.is_file() && has_jsonl_extension(&candidate);
        }
    }
    state.degrade(EnumerationDiagnosticCode::PathRejected);
    false
}

fn valid_relative_path(path: &Path) -> bool {
    let mut count = 0_usize;
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return false;
        }
        count = count.saturating_add(1);
    }
    count != 0
}

#[cfg(windows)]
fn has_jsonl_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("jsonl"))
}

#[cfg(not(windows))]
fn has_jsonl_extension(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("jsonl")
}

fn system_time_to_unix_nanos(value: std::time::SystemTime) -> i128 {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration_to_nanos(duration),
        Err(error) => -duration_to_nanos(error.duration()),
    }
}

fn duration_to_nanos(duration: Duration) -> i128 {
    i128::from(duration.as_secs())
        .saturating_mul(1_000_000_000)
        .saturating_add(i128::from(duration.subsec_nanos()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{CandidateClassification, classify_candidate};
    use crate::files::{EnumerationDiagnosticCode, EnumerationState};

    #[test]
    fn missing_entry_metadata_degrades_completion() {
        let temp = match TempDir::new() {
            Ok(value) => value,
            Err(error) => panic!("temporary directory must be created: {error}"),
        };
        let candidate = temp.path().join("removed.jsonl");
        if let Err(error) = fs::write(&candidate, b"{}\n") {
            panic!("candidate fixture must be created: {error}");
        }
        if let Err(error) = fs::remove_file(&candidate) {
            panic!("candidate fixture must be removed: {error}");
        }
        let mut state = EnumerationState::default();

        let classification = classify_candidate(&candidate, &mut state);

        assert!(matches!(classification, CandidateClassification::Skip));
        assert_eq!(state.completion(), crate::EnumerationCompletion::Partial);
        assert_eq!(
            state
                .diagnostics()
                .count(EnumerationDiagnosticCode::UnreadableEntry),
            1
        );
        assert_eq!(state.diagnostics().emitted_files(), 0);
    }
}
