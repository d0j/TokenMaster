use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::{
    DurableFileError, DurableFileReader, DurableFileReceipt, DurableFileTarget, DurableStagedFile,
    PhysicalFileIdentity, ValidatedLocalDirectory,
};

#[cfg(windows)]
use crate::windows::select_native_file;

const MAX_SELECTED_PATH_UNITS: usize = 32_767;

/// Exact portable file type accepted by a native or controlled selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileDialogFileType {
    Config,
    Backup,
    EncryptedBackup,
}

impl FileDialogFileType {
    #[must_use]
    pub const fn filter_name(self) -> &'static str {
        match self {
            Self::Config => "TokenMaster (.tmconfig)",
            Self::Backup => "TokenMaster (.tmbackup)",
            Self::EncryptedBackup => "TokenMaster (.tmbackup.age)",
        }
    }

    #[must_use]
    pub const fn filter_pattern(self) -> &'static str {
        match self {
            Self::Config => "*.tmconfig",
            Self::Backup => "*.tmbackup",
            Self::EncryptedBackup => "*.tmbackup.age",
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Config => "tmconfig",
            Self::Backup => "tmbackup",
            Self::EncryptedBackup => "tmbackup.age",
        }
    }

    #[must_use]
    pub const fn default_file_name(self) -> &'static str {
        match self {
            Self::Config => "tokenmaster-config.tmconfig",
            Self::Backup => "tokenmaster-backup.tmbackup",
            Self::EncryptedBackup => "tokenmaster-backup.tmbackup.age",
        }
    }

    fn accepts_name(self, name: &str) -> bool {
        let suffix = match self {
            Self::Config => ".tmconfig",
            Self::Backup => ".tmbackup",
            Self::EncryptedBackup => ".tmbackup.age",
        };
        name.len() >= suffix.len()
            && name
                .get(name.len() - suffix.len()..)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(suffix))
    }
}

/// Stable path-private native selection failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileDialogErrorCode {
    Unavailable,
    InvalidSelection,
    UnsupportedLocation,
    UnexpectedType,
    CapacityExceeded,
    SelectionChanged,
    InvalidState,
    Integrity,
}

impl FileDialogErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::InvalidSelection => "invalid_selection",
            Self::UnsupportedLocation => "unsupported_location",
            Self::UnexpectedType => "unexpected_type",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::SelectionChanged => "selection_changed",
            Self::InvalidState => "invalid_state",
            Self::Integrity => "integrity",
        }
    }
}

/// Stable path-private file-dialog error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileDialogError {
    code: FileDialogErrorCode,
}

impl FileDialogError {
    const fn new(code: FileDialogErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> FileDialogErrorCode {
        self.code
    }
}

impl fmt::Display for FileDialogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.stable_code())
    }
}

impl std::error::Error for FileDialogError {}

/// UI-safe result that exposes only selected, cancelled, or one stable failure.
pub enum FileDialogResult<T> {
    Selected(T),
    Cancelled,
    Failed(FileDialogError),
}

impl<T> fmt::Debug for FileDialogResult<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selected(_) => formatter.write_str("FileDialogResult::Selected([redacted])"),
            Self::Cancelled => formatter.write_str("FileDialogResult::Cancelled"),
            Self::Failed(error) => write!(formatter, "FileDialogResult::Failed({error})"),
        }
    }
}

/// Path-free bounded read capability returned after exact selection validation.
pub struct SelectedInputFile {
    reader: DurableFileReader,
}

impl SelectedInputFile {
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.reader.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<usize, FileDialogError> {
        self.reader.read_chunk(buffer).map_err(map_durable_error)
    }

    #[must_use]
    pub fn into_reader(self) -> DurableFileReader {
        self.reader
    }
}

impl fmt::Debug for SelectedInputFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SelectedInputFile([redacted])")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum OutputExpectation {
    Absent,
    Existing(PhysicalFileIdentity),
}

/// Path-free staged-write capability bound to the exact selection-time target.
pub struct SelectedOutputFile {
    target: DurableFileTarget,
    expectation: OutputExpectation,
    stage_created: bool,
}

impl SelectedOutputFile {
    pub fn create_staged(&mut self, max_bytes: u64) -> Result<DurableStagedFile, FileDialogError> {
        if self.stage_created {
            return Err(FileDialogError::new(FileDialogErrorCode::InvalidState));
        }
        self.ensure_selection_current()?;
        let staged = self
            .target
            .create_staged(max_bytes)
            .map_err(map_durable_error)?;
        self.stage_created = true;
        Ok(staged)
    }

    pub fn publish(
        &mut self,
        staged: &mut DurableStagedFile,
    ) -> Result<DurableFileReceipt, FileDialogError> {
        self.ensure_selection_current()?;
        let receipt = match self.expectation {
            OutputExpectation::Absent => staged.publish_new(&self.target),
            OutputExpectation::Existing(expected) => {
                staged.replace_selected(&self.target, expected)
            }
        }
        .map_err(map_publication_error)?;
        let identity = self
            .target
            .selected_identity()
            .map_err(map_durable_error)?
            .ok_or_else(|| FileDialogError::new(FileDialogErrorCode::Integrity))?;
        self.expectation = OutputExpectation::Existing(identity);
        Ok(receipt)
    }

    pub fn open_reader(&self, max_bytes: u64) -> Result<SelectedInputFile, FileDialogError> {
        self.ensure_selection_current()?;
        self.target
            .open_selected_reader(max_bytes)
            .map(|reader| SelectedInputFile { reader })
            .map_err(map_durable_error)
    }

    fn ensure_selection_current(&self) -> Result<(), FileDialogError> {
        let current = self.target.selected_identity().map_err(map_durable_error)?;
        let unchanged = match (self.expectation, current) {
            (OutputExpectation::Absent, None) => true,
            (OutputExpectation::Existing(expected), Some(current)) => expected == current,
            (OutputExpectation::Absent | OutputExpectation::Existing(_), _) => false,
        };
        if unchanged {
            Ok(())
        } else {
            Err(FileDialogError::new(FileDialogErrorCode::SelectionChanged))
        }
    }
}

impl fmt::Debug for SelectedOutputFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SelectedOutputFile([redacted])")
    }
}

/// Narrow file-selection port consumed by application composition.
pub trait FileDialogSelector {
    fn select_input(
        &self,
        file_type: FileDialogFileType,
        max_bytes: u64,
    ) -> FileDialogResult<SelectedInputFile>;

    fn select_output(&self, file_type: FileDialogFileType) -> FileDialogResult<SelectedOutputFile>;
}

/// Real operating-system selector. Unsupported platforms fail with one stable code.
#[derive(Clone, Debug, Default)]
pub struct NativeFileDialog {
    _thread_affinity: PhantomData<Rc<()>>,
}

impl FileDialogSelector for NativeFileDialog {
    fn select_input(
        &self,
        file_type: FileDialogFileType,
        max_bytes: u64,
    ) -> FileDialogResult<SelectedInputFile> {
        match native_path(NativeFileDialogAction::Open, file_type) {
            Ok(Some(path)) => seal_input_path(&path, file_type, max_bytes),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed(error),
        }
    }

    fn select_output(&self, file_type: FileDialogFileType) -> FileDialogResult<SelectedOutputFile> {
        match native_path(NativeFileDialogAction::Save, file_type) {
            Ok(Some(path)) => seal_output_path(&path, file_type),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed(error),
        }
    }
}

/// Deterministic path-private selector for tests and unsupported-platform hosts.
pub struct ControlledFileDialog {
    outcome: ControlledOutcome,
}

enum ControlledOutcome {
    Selected(DurableFileTarget),
    Cancelled,
    Failed(FileDialogError),
}

impl ControlledFileDialog {
    pub fn selected(
        directory: &ValidatedLocalDirectory,
        child_name: &str,
    ) -> Result<Self, FileDialogError> {
        let target =
            DurableFileTarget::selected_child(directory, child_name).map_err(map_durable_error)?;
        Ok(Self {
            outcome: ControlledOutcome::Selected(target),
        })
    }

    #[must_use]
    pub const fn cancelled() -> Self {
        Self {
            outcome: ControlledOutcome::Cancelled,
        }
    }

    #[must_use]
    pub const fn failed(code: FileDialogErrorCode) -> Self {
        Self {
            outcome: ControlledOutcome::Failed(FileDialogError::new(code)),
        }
    }
}

impl fmt::Debug for ControlledFileDialog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ControlledFileDialog([redacted])")
    }
}

impl FileDialogSelector for ControlledFileDialog {
    fn select_input(
        &self,
        file_type: FileDialogFileType,
        max_bytes: u64,
    ) -> FileDialogResult<SelectedInputFile> {
        match &self.outcome {
            ControlledOutcome::Selected(target) => seal_input_target(target, file_type, max_bytes),
            ControlledOutcome::Cancelled => FileDialogResult::Cancelled,
            ControlledOutcome::Failed(error) => FileDialogResult::Failed(*error),
        }
    }

    fn select_output(&self, file_type: FileDialogFileType) -> FileDialogResult<SelectedOutputFile> {
        match &self.outcome {
            ControlledOutcome::Selected(target) => seal_output_target(target, file_type),
            ControlledOutcome::Cancelled => FileDialogResult::Cancelled,
            ControlledOutcome::Failed(error) => FileDialogResult::Failed(*error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeFileDialogAction {
    Open,
    Save,
}

fn seal_input_path(
    path: &Path,
    file_type: FileDialogFileType,
    max_bytes: u64,
) -> FileDialogResult<SelectedInputFile> {
    match target_from_selected_path(path) {
        Ok(target) => seal_input_target(&target, file_type, max_bytes),
        Err(error) => FileDialogResult::Failed(error),
    }
}

fn seal_output_path(
    path: &Path,
    file_type: FileDialogFileType,
) -> FileDialogResult<SelectedOutputFile> {
    match target_from_selected_path(path) {
        Ok(target) => seal_output_target(&target, file_type),
        Err(error) => FileDialogResult::Failed(error),
    }
}

fn seal_input_target(
    target: &DurableFileTarget,
    file_type: FileDialogFileType,
    max_bytes: u64,
) -> FileDialogResult<SelectedInputFile> {
    if !target_has_type(target, file_type) {
        return FileDialogResult::Failed(FileDialogError::new(
            FileDialogErrorCode::InvalidSelection,
        ));
    }
    match target.open_selected_reader(max_bytes) {
        Ok(reader) => FileDialogResult::Selected(SelectedInputFile { reader }),
        Err(error) => FileDialogResult::Failed(map_durable_error(error)),
    }
}

fn seal_output_target(
    target: &DurableFileTarget,
    file_type: FileDialogFileType,
) -> FileDialogResult<SelectedOutputFile> {
    if !target_has_type(target, file_type) {
        return FileDialogResult::Failed(FileDialogError::new(
            FileDialogErrorCode::InvalidSelection,
        ));
    }
    match target.selected_identity() {
        Ok(None) => FileDialogResult::Selected(SelectedOutputFile {
            target: target.clone(),
            expectation: OutputExpectation::Absent,
            stage_created: false,
        }),
        Ok(Some(identity)) => FileDialogResult::Selected(SelectedOutputFile {
            target: target.clone(),
            expectation: OutputExpectation::Existing(identity),
            stage_created: false,
        }),
        Err(error) => FileDialogResult::Failed(map_durable_error(error)),
    }
}

fn target_from_selected_path(path: &Path) -> Result<DurableFileTarget, FileDialogError> {
    if !path.is_absolute() || selected_path_units(path) > MAX_SELECTED_PATH_UNITS {
        return Err(FileDialogError::new(
            FileDialogErrorCode::UnsupportedLocation,
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| FileDialogError::new(FileDialogErrorCode::InvalidSelection))?;
    let child_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| FileDialogError::new(FileDialogErrorCode::InvalidSelection))?;
    let directory = ValidatedLocalDirectory::new(parent).map_err(map_directory_error)?;
    DurableFileTarget::selected_child(&directory, child_name).map_err(map_durable_error)
}

fn target_has_type(target: &DurableFileTarget, file_type: FileDialogFileType) -> bool {
    target
        .exact_path()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| file_type.accepts_name(name))
}

#[cfg(windows)]
fn native_path(
    action: NativeFileDialogAction,
    file_type: FileDialogFileType,
) -> Result<Option<PathBuf>, FileDialogError> {
    select_native_file(action, file_type)
        .map_err(|_| FileDialogError::new(FileDialogErrorCode::Unavailable))
}

#[cfg(not(windows))]
fn native_path(
    _action: NativeFileDialogAction,
    _file_type: FileDialogFileType,
) -> Result<Option<PathBuf>, FileDialogError> {
    Err(FileDialogError::new(FileDialogErrorCode::Unavailable))
}

#[cfg(windows)]
fn selected_path_units(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().count()
}

#[cfg(unix)]
fn selected_path_units(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().len()
}

#[cfg(not(any(unix, windows)))]
fn selected_path_units(_path: &Path) -> usize {
    usize::MAX
}

const fn map_directory_error(error: crate::LocalDirectoryError) -> FileDialogError {
    let code = match error {
        crate::LocalDirectoryError::InvalidPath => FileDialogErrorCode::InvalidSelection,
        crate::LocalDirectoryError::UnsupportedLocation => FileDialogErrorCode::UnsupportedLocation,
        crate::LocalDirectoryError::Unavailable => FileDialogErrorCode::Unavailable,
    };
    FileDialogError::new(code)
}

const fn map_durable_error(error: DurableFileError) -> FileDialogError {
    let code = match error {
        DurableFileError::InvalidName => FileDialogErrorCode::InvalidSelection,
        DurableFileError::UnsupportedLocation => FileDialogErrorCode::UnsupportedLocation,
        DurableFileError::UnexpectedType => FileDialogErrorCode::UnexpectedType,
        DurableFileError::CapacityExceeded => FileDialogErrorCode::CapacityExceeded,
        DurableFileError::TargetExists | DurableFileError::TargetMissing => {
            FileDialogErrorCode::SelectionChanged
        }
        DurableFileError::Integrity | DurableFileError::RecoveryRequired => {
            FileDialogErrorCode::Integrity
        }
        DurableFileError::CollisionLimit
        | DurableFileError::InvalidState
        | DurableFileError::Unavailable => FileDialogErrorCode::Unavailable,
    };
    FileDialogError::new(code)
}

const fn map_publication_error(error: DurableFileError) -> FileDialogError {
    match error {
        DurableFileError::TargetExists | DurableFileError::TargetMissing => {
            FileDialogError::new(FileDialogErrorCode::SelectionChanged)
        }
        other => map_durable_error(other),
    }
}
