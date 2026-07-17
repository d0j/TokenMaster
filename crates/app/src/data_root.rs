use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use tokenmaster_platform::ValidatedLocalDirectory;

const PORTABLE_MARKER: &str = "tokenmaster.portable";
const PORTABLE_DATA_DIRECTORY: &str = "data";
const INSTALLED_DATA_DIRECTORY: &str = "TokenMaster";
const ARCHIVE_FILE_NAME: &str = "tokenmaster.sqlite3";

#[derive(Clone, Eq, PartialEq)]
pub struct ApplicationEnvironment {
    current_executable: PathBuf,
    local_app_data: Option<PathBuf>,
    user_profile: Option<PathBuf>,
    codex_home: Option<OsString>,
}

impl ApplicationEnvironment {
    #[must_use]
    pub fn new(
        current_executable: PathBuf,
        local_app_data: Option<PathBuf>,
        user_profile: Option<PathBuf>,
        codex_home: Option<OsString>,
    ) -> Self {
        Self {
            current_executable,
            local_app_data,
            user_profile,
            codex_home,
        }
    }

    pub fn capture() -> Result<Self, DataRootError> {
        let current_executable =
            std::env::current_exe().map_err(|_| DataRootError::invalid_environment())?;
        Ok(Self::new(
            current_executable,
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            std::env::var_os("USERPROFILE").map(PathBuf::from),
            std::env::var_os("CODEX_HOME"),
        ))
    }

    pub(crate) fn user_profile(&self) -> Option<&Path> {
        self.user_profile.as_deref()
    }

    pub(crate) fn codex_home(&self) -> Option<&OsString> {
        self.codex_home.as_ref()
    }
}

impl fmt::Debug for ApplicationEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationEnvironment")
            .field("current_executable", &"[redacted]")
            .field("local_app_data", &"[redacted]")
            .field("user_profile", &"[redacted]")
            .field("codex_home", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataMode {
    Installed,
    Portable,
}

#[derive(Clone, Eq, PartialEq)]
pub struct DataRoot {
    mode: DataMode,
    directory: ValidatedLocalDirectory,
    archive_path: PathBuf,
}

impl DataRoot {
    pub fn resolve(environment: &ApplicationEnvironment) -> Result<Self, DataRootError> {
        validate_executable(&environment.current_executable)?;
        let executable_directory = environment
            .current_executable
            .parent()
            .ok_or_else(DataRootError::invalid_environment)?;
        let executable_directory = ValidatedLocalDirectory::new(executable_directory)
            .map_err(|_| DataRootError::unsupported_data_location())?;
        let marker = executable_directory.as_path().join(PORTABLE_MARKER);
        let mode = marker_mode(&marker)?;
        let (base, child) = match mode {
            DataMode::Portable => (executable_directory, PORTABLE_DATA_DIRECTORY),
            DataMode::Installed => {
                let base = environment
                    .local_app_data
                    .as_deref()
                    .ok_or_else(DataRootError::invalid_environment)?;
                let base = ValidatedLocalDirectory::new(base)
                    .map_err(|_| DataRootError::unsupported_data_location())?;
                (base, INSTALLED_DATA_DIRECTORY)
            }
        };
        let child_path = base.as_path().join(child);
        create_exact_child(&child_path)?;
        let directory = ValidatedLocalDirectory::new(&child_path)
            .map_err(|_| DataRootError::unsupported_data_location())?;
        let archive_path = directory.as_path().join(ARCHIVE_FILE_NAME);
        Ok(Self {
            mode,
            directory,
            archive_path,
        })
    }

    #[must_use]
    pub const fn mode(&self) -> DataMode {
        self.mode
    }

    #[must_use]
    pub fn directory(&self) -> &Path {
        self.directory.as_path()
    }

    #[must_use]
    pub fn archive_path(&self) -> &Path {
        &self.archive_path
    }
}

impl fmt::Debug for DataRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DataRoot")
            .field("mode", &self.mode)
            .field("directory", &"[redacted]")
            .field("archive_path", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataRootErrorCode {
    InvalidEnvironment,
    InvalidPortableMarker,
    UnsupportedDataLocation,
    DataDirectoryUnavailable,
}

impl DataRootErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidEnvironment => "invalid_environment",
            Self::InvalidPortableMarker => "invalid_portable_marker",
            Self::UnsupportedDataLocation => "unsupported_data_location",
            Self::DataDirectoryUnavailable => "data_directory_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataRootError {
    code: DataRootErrorCode,
}

impl DataRootError {
    const fn invalid_environment() -> Self {
        Self {
            code: DataRootErrorCode::InvalidEnvironment,
        }
    }

    const fn invalid_portable_marker() -> Self {
        Self {
            code: DataRootErrorCode::InvalidPortableMarker,
        }
    }

    const fn unsupported_data_location() -> Self {
        Self {
            code: DataRootErrorCode::UnsupportedDataLocation,
        }
    }

    const fn data_directory_unavailable() -> Self {
        Self {
            code: DataRootErrorCode::DataDirectoryUnavailable,
        }
    }

    #[must_use]
    pub const fn code(self) -> DataRootErrorCode {
        self.code
    }
}

impl fmt::Display for DataRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.stable_code())
    }
}

impl std::error::Error for DataRootError {}

fn validate_executable(path: &Path) -> Result<(), DataRootError> {
    if !path.is_absolute() {
        return Err(DataRootError::invalid_environment());
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| DataRootError::invalid_environment())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(DataRootError::invalid_environment());
    }
    Ok(())
}

fn marker_mode(marker: &Path) -> Result<DataMode, DataRootError> {
    match fs::symlink_metadata(marker) {
        Ok(metadata)
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && !is_reparse_point(&metadata)
                && metadata.len() == 0 =>
        {
            Ok(DataMode::Portable)
        }
        Ok(_) => Err(DataRootError::invalid_portable_marker()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(DataMode::Installed),
        Err(_) => Err(DataRootError::invalid_portable_marker()),
    }
}

fn create_exact_child(path: &Path) -> Result<(), DataRootError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        Err(_) => Err(DataRootError::data_directory_unavailable()),
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
