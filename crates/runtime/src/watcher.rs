use std::fmt;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use tokenmaster_engine::RefreshUrgency;
use tokenmaster_provider::{MAX_PATH_BYTES, MAX_PROFILES};

use crate::RefreshHintSink;

pub const MAX_WATCH_ROOTS: usize = MAX_PROFILES;
pub(crate) const MAX_CHANGED_PATHS: usize = 256;

#[derive(Clone)]
pub(crate) struct ChangedPathBuffer {
    state: Arc<Mutex<ChangedPathState>>,
}

#[derive(Default)]
struct ChangedPathState {
    paths: Vec<PathBuf>,
    reconcile_required: bool,
}

pub(crate) enum ChangedPathBatch {
    Paths(Vec<PathBuf>),
    Reconcile,
}

impl ChangedPathBuffer {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ChangedPathState::default())),
        }
    }

    fn record(&self, paths: Vec<PathBuf>) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.reconcile_required {
            return false;
        }
        if paths.is_empty() {
            state.paths.clear();
            state.reconcile_required = true;
            return false;
        }
        for path in paths {
            if !path.is_absolute()
                || path_byte_len(&path) > MAX_PATH_BYTES
                || validate_namespace(&path).is_err()
            {
                state.paths.clear();
                state.reconcile_required = true;
                return false;
            }
            if state.paths.iter().any(|current| current == &path) {
                continue;
            }
            if state.paths.len() == MAX_CHANGED_PATHS {
                state.paths.clear();
                state.reconcile_required = true;
                return false;
            }
            state.paths.push(path);
        }
        true
    }

    fn require_reconcile(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.paths.clear();
            state.reconcile_required = true;
        }
    }

    pub(crate) fn take(&self) -> ChangedPathBatch {
        let Ok(mut state) = self.state.lock() else {
            return ChangedPathBatch::Reconcile;
        };
        if state.reconcile_required {
            state.reconcile_required = false;
            state.paths.clear();
            return ChangedPathBatch::Reconcile;
        }
        ChangedPathBatch::Paths(std::mem::take(&mut state.paths))
    }
}

impl fmt::Debug for ChangedPathBuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ChangedPathBuffer([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatcherErrorCode {
    CapacityExceeded,
    InvalidRoot,
    BackendUnavailable,
}

impl fmt::Display for WatcherErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CapacityExceeded => "capacity_exceeded",
            Self::InvalidRoot => "invalid_root",
            Self::BackendUnavailable => "backend_unavailable",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherError {
    code: WatcherErrorCode,
}

impl fmt::Display for WatcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code.fmt(formatter)
    }
}

impl std::error::Error for WatcherError {}

impl WatcherError {
    const fn new(code: WatcherErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> WatcherErrorCode {
        self.code
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherSnapshot {
    generation: u64,
    root_count: usize,
}

impl WatcherSnapshot {
    pub(crate) const fn stopped(generation: u64) -> Self {
        Self {
            generation,
            root_count: 0,
        }
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn root_count(self) -> usize {
        self.root_count
    }
}

pub struct BoundedFilesystemWatcher {
    hints: RefreshHintSink,
    changed_paths: ChangedPathBuffer,
    active_generation: Arc<AtomicU64>,
    watcher: Option<RecommendedWatcher>,
    generation: u64,
    root_count: usize,
}

impl BoundedFilesystemWatcher {
    pub fn new(hints: RefreshHintSink) -> Result<Self, WatcherError> {
        Self::with_changed_paths(hints, ChangedPathBuffer::new())
    }

    pub(crate) fn with_changed_paths(
        hints: RefreshHintSink,
        changed_paths: ChangedPathBuffer,
    ) -> Result<Self, WatcherError> {
        Ok(Self {
            hints,
            changed_paths,
            active_generation: Arc::new(AtomicU64::new(0)),
            watcher: None,
            generation: 0,
            root_count: 0,
        })
    }

    pub fn replace_roots(&mut self, roots: &[PathBuf]) -> Result<WatcherSnapshot, WatcherError> {
        if roots.len() > MAX_WATCH_ROOTS {
            return Err(WatcherError::new(WatcherErrorCode::CapacityExceeded));
        }
        let validated = match validate_roots(roots) {
            Ok(validated) => validated,
            Err(error) => {
                if error.code() == WatcherErrorCode::BackendUnavailable {
                    let _ = self.hints.watcher_error();
                }
                return Err(error);
            }
        };
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| WatcherError::new(WatcherErrorCode::CapacityExceeded))?;

        let active_generation = self.active_generation.clone();
        let callback_hints = self.hints.clone();
        let callback_changed_paths = self.changed_paths.clone();
        let mut next_watcher = if validated.existing.is_empty() {
            None
        } else {
            let watcher = recommended_watcher(move |result: notify::Result<notify::Event>| {
                if active_generation.load(Ordering::Acquire) != next_generation {
                    return;
                }
                match result {
                    Ok(event) if event.need_rescan() => {
                        callback_changed_paths.require_reconcile();
                        let _ = callback_hints.watcher_rescan_required();
                    }
                    Ok(event) => {
                        if callback_changed_paths.record(event.paths) {
                            let _ = callback_hints.filesystem_changed();
                        } else {
                            let _ = callback_hints.force_reconcile(RefreshUrgency::Recovery);
                        }
                    }
                    Err(_error) => {
                        callback_changed_paths.require_reconcile();
                        let _ = callback_hints.watcher_error();
                    }
                }
            })
            .map_err(|_| WatcherError::new(WatcherErrorCode::BackendUnavailable));
            match watcher {
                Ok(watcher) => Some(watcher),
                Err(error) => {
                    let _ = self.hints.watcher_error();
                    return Err(error);
                }
            }
        };

        let mut watched_count = 0_usize;
        for root in &validated.existing {
            if let Some(watcher) = &mut next_watcher {
                if watcher.watch(root, RecursiveMode::Recursive).is_err() {
                    let _ = self.hints.watcher_error();
                    return Err(WatcherError::new(WatcherErrorCode::BackendUnavailable));
                }
                watched_count = watched_count
                    .checked_add(1)
                    .ok_or_else(|| WatcherError::new(WatcherErrorCode::CapacityExceeded))?;
            }
        }

        self.active_generation
            .store(next_generation, Ordering::Release);
        self.watcher = next_watcher;
        self.generation = next_generation;
        self.root_count = watched_count;
        if validated.missing {
            let _ = self.hints.watcher_error();
        } else {
            let _ = self.hints.watcher_healthy();
            let _ = self.hints.force_reconcile(RefreshUrgency::Recovery);
        }
        Ok(self.snapshot())
    }

    #[must_use]
    pub const fn snapshot(&self) -> WatcherSnapshot {
        WatcherSnapshot {
            generation: self.generation,
            root_count: self.root_count,
        }
    }

    pub fn shutdown(&mut self) {
        self.active_generation.store(0, Ordering::Release);
        self.watcher = None;
        self.root_count = 0;
    }
}

impl fmt::Debug for BoundedFilesystemWatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundedFilesystemWatcher")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl Drop for BoundedFilesystemWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct ValidatedRoots {
    existing: Vec<PathBuf>,
    missing: bool,
}

fn validate_roots(roots: &[PathBuf]) -> Result<ValidatedRoots, WatcherError> {
    let mut existing = Vec::with_capacity(roots.len());
    let mut missing = false;
    for (index, root) in roots.iter().enumerate() {
        if !root.is_absolute()
            || path_byte_len(root) > MAX_PATH_BYTES
            || validate_namespace(root).is_err()
            || roots[..index].iter().any(|prior| prior == root)
        {
            return Err(WatcherError::new(WatcherErrorCode::InvalidRoot));
        }
        match std::fs::symlink_metadata(root) {
            Ok(metadata)
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || is_reparse_point(&metadata) =>
            {
                return Err(WatcherError::new(WatcherErrorCode::InvalidRoot));
            }
            Ok(_) => {
                reject_linked_ancestors(root)?;
                let canonical = std::fs::canonicalize(root)
                    .map_err(|_| WatcherError::new(WatcherErrorCode::BackendUnavailable))?;
                if path_byte_len(&canonical) > MAX_PATH_BYTES
                    || validate_namespace(&canonical).is_err()
                    || existing.iter().any(|prior| prior == &canonical)
                {
                    return Err(WatcherError::new(WatcherErrorCode::InvalidRoot));
                }
                reject_linked_ancestors(&canonical)?;
                existing.push(canonical);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => missing = true,
            Err(_) => return Err(WatcherError::new(WatcherErrorCode::BackendUnavailable)),
        }
    }
    Ok(ValidatedRoots { existing, missing })
}

fn reject_linked_ancestors(path: &Path) -> Result<(), WatcherError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|_| WatcherError::new(WatcherErrorCode::BackendUnavailable))?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(WatcherError::new(WatcherErrorCode::InvalidRoot));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn validate_namespace(path: &Path) -> Result<(), ()> {
    use std::path::Prefix;

    match path.components().next() {
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) =>
        {
            Ok(())
        }
        _ => Err(()),
    }
}

#[cfg(not(windows))]
fn validate_namespace(path: &Path) -> Result<(), ()> {
    if path.is_absolute() { Ok(()) } else { Err(()) }
}

#[cfg(windows)]
fn path_byte_len(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().count().saturating_mul(2)
}

#[cfg(not(windows))]
fn path_byte_len(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().len()
}

#[cfg(test)]
mod tests {
    use super::{ChangedPathBatch, ChangedPathBuffer, MAX_CHANGED_PATHS};
    use std::path::PathBuf;

    fn absolute_path(index: usize) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(format!(r"C:\tokenmaster\session-{index}.jsonl"))
        }
        #[cfg(not(windows))]
        {
            PathBuf::from(format!("/tokenmaster/session-{index}.jsonl"))
        }
    }

    #[test]
    fn changed_paths_are_deduplicated_and_drained_without_exposure() {
        let buffer = ChangedPathBuffer::new();
        let path = absolute_path(1);
        assert!(buffer.record(vec![path.clone(), path.clone()]));
        let ChangedPathBatch::Paths(paths) = buffer.take() else {
            panic!("recorded paths");
        };
        assert_eq!(paths, vec![path.clone()]);
        assert!(matches!(buffer.take(), ChangedPathBatch::Paths(paths) if paths.is_empty()));
        assert_eq!(format!("{buffer:?}"), "ChangedPathBuffer([redacted])");
        assert!(!format!("{buffer:?}").contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn changed_path_capacity_forces_one_reconciliation() {
        let buffer = ChangedPathBuffer::new();
        let paths = (0..=MAX_CHANGED_PATHS).map(absolute_path).collect();
        assert!(!buffer.record(paths));
        assert!(matches!(buffer.take(), ChangedPathBatch::Reconcile));
        assert!(matches!(buffer.take(), ChangedPathBatch::Paths(paths) if paths.is_empty()));
    }
}
