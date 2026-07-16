use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{GitBackendError, GitBackendErrorCode};

#[derive(Clone, Eq, PartialEq)]
pub struct GitExecutable {
    path: PathBuf,
}

impl GitExecutable {
    pub fn new(path: PathBuf) -> Result<Self, GitBackendError> {
        validate_absolute_local_path(&path)?;
        let initial_metadata = fs::symlink_metadata(&path).map_err(|_| invalid_executable())?;
        validate_initial_executable_type(&initial_metadata)?;
        validate_native_name(&path)?;
        let path = fs::canonicalize(path).map_err(|_| invalid_executable())?;
        validate_absolute_local_path(&path)?;
        validate_native_name(&path)?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| invalid_executable())?;
        if !metadata.is_file() || is_reparse_point(&metadata) {
            return Err(invalid_executable());
        }
        validate_native_file(&path, &metadata)?;
        Ok(Self { path })
    }

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(&self.path);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_git_environment(&mut command);
        hide_child_window(&mut command);
        command
    }
}

impl fmt::Debug for GitExecutable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitExecutable([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct GitRepositoryCandidate {
    path: PathBuf,
}

impl GitRepositoryCandidate {
    pub fn new(path: PathBuf) -> Result<Self, GitBackendError> {
        validate_absolute_local_path(&path)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryNotFound))?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err(GitBackendError::new(
                GitBackendErrorCode::RepositoryPathRejected,
            ));
        }
        let path = fs::canonicalize(path)
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryNotFound))?;
        validate_absolute_local_path(&path)
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryPathRejected))?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Debug for GitRepositoryCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitRepositoryCandidate([redacted])")
    }
}

const REMOVED_GIT_ENVIRONMENT: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_ASKPASS",
    "GIT_ATTR_SOURCE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_SYSTEM",
    "GIT_DIFF_OPTS",
    "GIT_DIR",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_EDITOR",
    "GIT_EXEC_PATH",
    "GIT_EXTERNAL_DIFF",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_PAGER_IN_USE",
    "GIT_PROXY_COMMAND",
    "GIT_QUARANTINE_PATH",
    "GIT_REDIRECT_STDERR",
    "GIT_REPLACE_REF_BASE",
    "GIT_SEQUENCE_EDITOR",
    "GIT_SHALLOW_FILE",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_TRACE",
    "GIT_TRACE2",
    "GIT_TRACE2_BRIEF",
    "GIT_TRACE2_CONFIG_PARAMS",
    "GIT_TRACE2_ENV_VARS",
    "GIT_TRACE2_EVENT",
    "GIT_TRACE2_PERF",
    "GIT_TRACE_CURL",
    "GIT_TRACE_CURL_NO_DATA",
    "GIT_TRACE_FSMONITOR",
    "GIT_TRACE_PACK_ACCESS",
    "GIT_TRACE_PACKET",
    "GIT_TRACE_PERFORMANCE",
    "GIT_TRACE_REDACT",
    "GIT_TRACE_SETUP",
    "GIT_TRACE_SHALLOW",
    "GIT_WORK_TREE",
    "SSH_ASKPASS",
];

fn configure_git_environment(command: &mut Command) {
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("NO_COLOR", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_PROTOCOL_FROM_USER", "0");
    for variable in REMOVED_GIT_ENVIRONMENT {
        command.env_remove(variable);
    }
}

pub(crate) fn validate_private_directory(path: &Path) -> Result<PathBuf, GitBackendError> {
    validate_absolute_local_path(path)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryPathRejected))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryNotFound))?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(GitBackendError::new(
            GitBackendErrorCode::RepositoryPathRejected,
        ));
    }
    fs::canonicalize(path)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryNotFound))
}

fn validate_native_name(path: &Path) -> Result<(), GitBackendError> {
    let expected = if cfg!(windows) { "git.exe" } else { "git" };
    let valid = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            if cfg!(windows) {
                name.eq_ignore_ascii_case(expected)
            } else {
                name == expected
            }
        });
    if !valid {
        return Err(invalid_executable());
    }
    Ok(())
}

#[cfg(windows)]
fn validate_initial_executable_type(metadata: &fs::Metadata) -> Result<(), GitBackendError> {
    if !metadata.is_file() || is_reparse_point(metadata) {
        return Err(invalid_executable());
    }
    Ok(())
}

#[cfg(unix)]
fn validate_initial_executable_type(metadata: &fs::Metadata) -> Result<(), GitBackendError> {
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        return Err(invalid_executable());
    }
    Ok(())
}

#[cfg(windows)]
fn validate_native_file(path: &Path, _metadata: &fs::Metadata) -> Result<(), GitBackendError> {
    let mut file = fs::File::open(path).map_err(|_| invalid_executable())?;
    let mut magic = [0_u8; 2];
    file.read_exact(&mut magic)
        .map_err(|_| invalid_executable())?;
    if magic != *b"MZ" {
        return Err(invalid_executable());
    }
    Ok(())
}

#[cfg(unix)]
fn validate_native_file(_path: &Path, metadata: &fs::Metadata) -> Result<(), GitBackendError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(invalid_executable());
    }
    Ok(())
}

fn validate_absolute_local_path(path: &Path) -> Result<(), GitBackendError> {
    if !path.is_absolute() {
        return Err(invalid_executable());
    }
    #[cfg(windows)]
    {
        use std::path::Prefix;

        let valid = matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        );
        if !valid {
            return Err(invalid_executable());
        }
    }
    #[cfg(not(windows))]
    if !matches!(path.components().next(), Some(Component::RootDir)) {
        return Err(invalid_executable());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid_executable());
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn hide_child_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_child_window(_command: &mut Command) {}

const fn invalid_executable() -> GitBackendError {
    GitBackendError::new(GitBackendErrorCode::InvalidExecutable)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::process::Command;

    use super::{REMOVED_GIT_ENVIRONMENT, configure_git_environment};

    #[test]
    fn process_environment_removes_git_redirection_and_trace_authority() {
        let mut command = Command::new("git");
        configure_git_environment(&mut command);
        let environment = command.get_envs().collect::<Vec<_>>();

        for variable in REMOVED_GIT_ENVIRONMENT {
            assert!(
                environment
                    .iter()
                    .any(|(name, value)| *name == OsStr::new(variable) && value.is_none()),
                "{variable} was not removed"
            );
        }
        for (name, expected) in [
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_CONFIG_COUNT", "0"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_PROTOCOL_FROM_USER", "0"),
        ] {
            assert!(environment.iter().any(|(current, value)| {
                *current == OsStr::new(name) && *value == Some(OsStr::new(expected))
            }));
        }
    }
}
