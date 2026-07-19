use core::fmt;

/// Exact path-free state of the current user's fixed TokenMaster sign-in entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentUserStartupStatus {
    Disabled,
    EnabledVerified,
    StaleRelocation,
    Conflict,
    AccessDenied,
    Unavailable,
}

impl CurrentUserStartupStatus {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::EnabledVerified => "enabled_verified",
            Self::StaleRelocation => "stale_relocation",
            Self::Conflict => "conflict",
            Self::AccessDenied => "access_denied",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Fixed mutations accepted by the current-user startup boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentUserStartupAction {
    Enable,
    RepairStale,
    Disable,
}

/// Stable, path-free mutation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CurrentUserStartupError {
    #[error("current-user startup access denied")]
    AccessDenied,
    #[error("current-user startup unavailable")]
    Unavailable,
    #[error("current-user startup stale registration requires explicit repair")]
    StaleRequiresRepair,
    #[error("current-user startup registration conflict")]
    Conflict,
    #[error("current-user startup action is invalid for the observed state")]
    InvalidState,
    #[error("current-user startup readback verification failed")]
    ReadbackFailed,
}

impl CurrentUserStartupError {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::AccessDenied => "startup_access_denied",
            Self::Unavailable => "startup_unavailable",
            Self::StaleRequiresRepair => "startup_stale_requires_repair",
            Self::Conflict => "startup_conflict",
            Self::InvalidState => "startup_invalid_state",
            Self::ReadbackFailed => "startup_readback_failed",
        }
    }
}

/// Bounded observation that carries no registry data, executable path, or file identity.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CurrentUserStartupSnapshot {
    status: CurrentUserStartupStatus,
}

impl CurrentUserStartupSnapshot {
    const fn new(status: CurrentUserStartupStatus) -> Self {
        Self { status }
    }

    #[must_use]
    pub const fn status(self) -> CurrentUserStartupStatus {
        self.status
    }
}

impl fmt::Debug for CurrentUserStartupSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "CurrentUserStartupSnapshot({})",
            self.status.stable_code()
        )
    }
}

/// Fixed namespace for read-only inspection and explicit current-user startup actions.
pub struct CurrentUserStartup;

impl CurrentUserStartup {
    /// Inspects the fixed current-user entry without creating, deleting, or repairing it.
    #[must_use]
    pub fn inspect() -> CurrentUserStartupSnapshot {
        imp::inspect()
    }

    /// Applies one fixed action and publishes success only after exact readback.
    pub fn apply(
        action: CurrentUserStartupAction,
    ) -> Result<CurrentUserStartupSnapshot, CurrentUserStartupError> {
        imp::apply(action)
    }
}

trait StartupBackend {
    fn observe(&mut self) -> Result<CurrentUserStartupStatus, CurrentUserStartupError>;
    fn write_current(&mut self) -> Result<(), CurrentUserStartupError>;
    fn delete(&mut self) -> Result<(), CurrentUserStartupError>;
}

fn inspect_with(backend: &mut impl StartupBackend) -> CurrentUserStartupSnapshot {
    match backend.observe() {
        Ok(status) => CurrentUserStartupSnapshot::new(status),
        Err(CurrentUserStartupError::AccessDenied) => {
            CurrentUserStartupSnapshot::new(CurrentUserStartupStatus::AccessDenied)
        }
        Err(_) => CurrentUserStartupSnapshot::new(CurrentUserStartupStatus::Unavailable),
    }
}

fn apply_with(
    backend: &mut impl StartupBackend,
    action: CurrentUserStartupAction,
) -> Result<CurrentUserStartupSnapshot, CurrentUserStartupError> {
    let before = backend.observe()?;
    let expected = match (action, before) {
        (CurrentUserStartupAction::Enable, CurrentUserStartupStatus::Disabled) => {
            backend.write_current()?;
            CurrentUserStartupStatus::EnabledVerified
        }
        (CurrentUserStartupAction::Enable, CurrentUserStartupStatus::EnabledVerified)
        | (CurrentUserStartupAction::RepairStale, CurrentUserStartupStatus::EnabledVerified) => {
            return Ok(CurrentUserStartupSnapshot::new(
                CurrentUserStartupStatus::EnabledVerified,
            ));
        }
        (CurrentUserStartupAction::Enable, CurrentUserStartupStatus::StaleRelocation) => {
            return Err(CurrentUserStartupError::StaleRequiresRepair);
        }
        (CurrentUserStartupAction::RepairStale, CurrentUserStartupStatus::StaleRelocation) => {
            backend.write_current()?;
            CurrentUserStartupStatus::EnabledVerified
        }
        (CurrentUserStartupAction::RepairStale, CurrentUserStartupStatus::Disabled) => {
            return Err(CurrentUserStartupError::InvalidState);
        }
        (CurrentUserStartupAction::Disable, CurrentUserStartupStatus::Disabled) => {
            return Ok(CurrentUserStartupSnapshot::new(
                CurrentUserStartupStatus::Disabled,
            ));
        }
        (
            CurrentUserStartupAction::Disable,
            CurrentUserStartupStatus::EnabledVerified | CurrentUserStartupStatus::StaleRelocation,
        ) => {
            backend.delete()?;
            CurrentUserStartupStatus::Disabled
        }
        (_, CurrentUserStartupStatus::Conflict) => {
            return Err(CurrentUserStartupError::Conflict);
        }
        (_, CurrentUserStartupStatus::AccessDenied) => {
            return Err(CurrentUserStartupError::AccessDenied);
        }
        (_, CurrentUserStartupStatus::Unavailable) => {
            return Err(CurrentUserStartupError::Unavailable);
        }
    };
    let after = backend.observe()?;
    if after != expected {
        return Err(CurrentUserStartupError::ReadbackFailed);
    }
    Ok(CurrentUserStartupSnapshot::new(after))
}

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::fs::{self, File};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use windows::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR,
    };
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, REG_VALUE_TYPE, RRF_NOEXPAND, RRF_RT_ANY, RegDeleteKeyValueW,
        RegGetValueW, RegSetKeyValueW,
    };
    use windows::core::{PCWSTR, w};

    use super::{
        CurrentUserStartupAction, CurrentUserStartupError, CurrentUserStartupSnapshot,
        CurrentUserStartupStatus, StartupBackend, apply_with, inspect_with,
    };
    use crate::PhysicalFileIdentity;

    const RUN_SUBKEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const VALUE_NAME: PCWSTR = w!("TokenMaster");
    const MAX_COMMAND_BYTES: u32 = 32 * 1024;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    struct VerifiedExecutable {
        path: PathBuf,
        identity: PhysicalFileIdentity,
        basename: OsString,
    }

    impl VerifiedExecutable {
        fn current() -> Result<Self, CurrentUserStartupError> {
            let path = std::env::current_exe().map_err(|_| CurrentUserStartupError::Unavailable)?;
            validate_ordinary_file(&path)?;
            let file = File::open(&path).map_err(map_io_error)?;
            let identity = PhysicalFileIdentity::from_file(&file)
                .map_err(|_| CurrentUserStartupError::Unavailable)?;
            let basename = path
                .file_name()
                .ok_or(CurrentUserStartupError::Unavailable)?
                .to_os_string();
            Ok(Self {
                path,
                identity,
                basename,
            })
        }

        fn command(&self) -> Result<Vec<u16>, CurrentUserStartupError> {
            let path = self.path.as_os_str().encode_wide().collect::<Vec<_>>();
            if path.contains(&u16::from(b'"')) {
                return Err(CurrentUserStartupError::Unavailable);
            }
            let mut command = Vec::with_capacity(path.len().saturating_add(3));
            command.push(u16::from(b'"'));
            command.extend(path);
            command.push(u16::from(b'"'));
            command.push(0);
            let bytes = command
                .len()
                .checked_mul(2)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(CurrentUserStartupError::Unavailable)?;
            if bytes > MAX_COMMAND_BYTES {
                return Err(CurrentUserStartupError::Unavailable);
            }
            Ok(command)
        }
    }

    struct NativeBackend {
        executable: VerifiedExecutable,
    }

    impl NativeBackend {
        fn new() -> Result<Self, CurrentUserStartupError> {
            Ok(Self {
                executable: VerifiedExecutable::current()?,
            })
        }
    }

    impl StartupBackend for NativeBackend {
        fn observe(&mut self) -> Result<CurrentUserStartupStatus, CurrentUserStartupError> {
            let Some(command) = read_command()? else {
                return Ok(CurrentUserStartupStatus::Disabled);
            };
            classify_command(&self.executable, &command)
        }

        fn write_current(&mut self) -> Result<(), CurrentUserStartupError> {
            let command = self.executable.command()?;
            let bytes = u32::try_from(command.len().saturating_mul(2))
                .map_err(|_| CurrentUserStartupError::Unavailable)?;
            // SAFETY: hive/subkey/value/type are fixed; the UTF-16 buffer is owned,
            // NUL-terminated, and remains valid for the duration of the call.
            let result = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN_SUBKEY,
                    VALUE_NAME,
                    REG_SZ.0,
                    Some(command.as_ptr().cast()),
                    bytes,
                )
            };
            map_registry_result(result)
        }

        fn delete(&mut self) -> Result<(), CurrentUserStartupError> {
            // SAFETY: hive/subkey/value are fixed constants and no borrowed output exists.
            let result = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_SUBKEY, VALUE_NAME) };
            if is_missing(result) || result == ERROR_SUCCESS {
                Ok(())
            } else {
                map_registry_result(result)
            }
        }
    }

    pub(super) fn inspect() -> CurrentUserStartupSnapshot {
        NativeBackend::new().map_or_else(
            |error| match error {
                CurrentUserStartupError::AccessDenied => {
                    CurrentUserStartupSnapshot::new(CurrentUserStartupStatus::AccessDenied)
                }
                _ => CurrentUserStartupSnapshot::new(CurrentUserStartupStatus::Unavailable),
            },
            |mut backend| inspect_with(&mut backend),
        )
    }

    pub(super) fn apply(
        action: CurrentUserStartupAction,
    ) -> Result<CurrentUserStartupSnapshot, CurrentUserStartupError> {
        let mut backend = NativeBackend::new()?;
        apply_with(&mut backend, action)
    }

    fn read_command() -> Result<Option<Vec<u16>>, CurrentUserStartupError> {
        let mut value_type = REG_VALUE_TYPE::default();
        let mut bytes = 0_u32;
        // SAFETY: the fixed hive/key/value are valid and the first call requests only
        // bounded size/type metadata into initialized scalar outputs.
        let size_result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN_SUBKEY,
                VALUE_NAME,
                RRF_RT_ANY | RRF_NOEXPAND,
                Some(&mut value_type),
                None,
                Some(&mut bytes),
            )
        };
        if is_missing(size_result) {
            return Ok(None);
        }
        map_registry_result(size_result)?;
        if value_type != REG_SZ
            || !(2..=MAX_COMMAND_BYTES).contains(&bytes)
            || !bytes.is_multiple_of(2)
        {
            return Ok(Some(Vec::new()));
        }
        let mut command = vec![0_u16; (bytes / 2) as usize];
        let mut read_bytes = bytes;
        // SAFETY: the second call uses the exact bounded byte capacity obtained above;
        // races that change the required size fail rather than retry or truncate.
        let read_result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN_SUBKEY,
                VALUE_NAME,
                RRF_RT_ANY | RRF_NOEXPAND,
                Some(&mut value_type),
                Some(command.as_mut_ptr().cast()),
                Some(&mut read_bytes),
            )
        };
        map_registry_result(read_result)?;
        if value_type != REG_SZ || read_bytes != bytes || command.last() != Some(&0) {
            return Ok(Some(Vec::new()));
        }
        command.pop();
        if command.is_empty() || command.contains(&0) {
            return Ok(Some(Vec::new()));
        }
        Ok(Some(command))
    }

    fn classify_command(
        current: &VerifiedExecutable,
        command: &[u16],
    ) -> Result<CurrentUserStartupStatus, CurrentUserStartupError> {
        let Some(path) = parse_exact_quoted_path(command) else {
            return Ok(CurrentUserStartupStatus::Conflict);
        };
        let Some(basename) = path.file_name() else {
            return Ok(CurrentUserStartupStatus::Conflict);
        };
        if !same_ascii_name(basename, &current.basename) {
            return Ok(CurrentUserStartupStatus::Conflict);
        }
        match validate_ordinary_file(&path).and_then(|()| File::open(&path).map_err(map_io_error)) {
            Ok(file) => {
                let identity = PhysicalFileIdentity::from_file(&file)
                    .map_err(|_| CurrentUserStartupError::Unavailable)?;
                if identity == current.identity {
                    Ok(CurrentUserStartupStatus::EnabledVerified)
                } else {
                    Ok(CurrentUserStartupStatus::StaleRelocation)
                }
            }
            Err(CurrentUserStartupError::Unavailable) if !path.exists() => {
                Ok(CurrentUserStartupStatus::StaleRelocation)
            }
            Err(error) => Err(error),
        }
    }

    fn parse_exact_quoted_path(command: &[u16]) -> Option<PathBuf> {
        let quote = u16::from(b'"');
        if command.len() < 3
            || command.first() != Some(&quote)
            || command.last() != Some(&quote)
            || command[1..command.len() - 1].contains(&quote)
        {
            return None;
        }
        let path = PathBuf::from(OsString::from_wide(&command[1..command.len() - 1]));
        path.is_absolute().then_some(path)
    }

    fn same_ascii_name(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
        let left = left.to_string_lossy();
        let right = right.to_string_lossy();
        left.is_ascii() && right.is_ascii() && left.eq_ignore_ascii_case(&right)
    }

    fn validate_ordinary_file(path: &Path) -> Result<(), CurrentUserStartupError> {
        if !path.is_absolute() {
            return Err(CurrentUserStartupError::Unavailable);
        }
        let metadata = fs::symlink_metadata(path).map_err(map_io_error)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(CurrentUserStartupError::Unavailable);
        }
        Ok(())
    }

    fn map_io_error(error: std::io::Error) -> CurrentUserStartupError {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            CurrentUserStartupError::AccessDenied
        } else {
            CurrentUserStartupError::Unavailable
        }
    }

    const fn is_missing(result: WIN32_ERROR) -> bool {
        result.0 == ERROR_FILE_NOT_FOUND.0 || result.0 == ERROR_PATH_NOT_FOUND.0
    }

    fn map_registry_result(result: WIN32_ERROR) -> Result<(), CurrentUserStartupError> {
        if result == ERROR_SUCCESS {
            Ok(())
        } else if result == ERROR_ACCESS_DENIED {
            Err(CurrentUserStartupError::AccessDenied)
        } else {
            Err(CurrentUserStartupError::Unavailable)
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{
        CurrentUserStartupAction, CurrentUserStartupError, CurrentUserStartupSnapshot,
        CurrentUserStartupStatus,
    };

    pub(super) const fn inspect() -> CurrentUserStartupSnapshot {
        CurrentUserStartupSnapshot::new(CurrentUserStartupStatus::Unavailable)
    }

    pub(super) const fn apply(
        _action: CurrentUserStartupAction,
    ) -> Result<CurrentUserStartupSnapshot, CurrentUserStartupError> {
        Err(CurrentUserStartupError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend {
        states: Vec<Result<CurrentUserStartupStatus, CurrentUserStartupError>>,
        writes: usize,
        deletes: usize,
    }

    impl FakeBackend {
        fn new(states: impl IntoIterator<Item = CurrentUserStartupStatus>) -> Self {
            let mut states = states.into_iter().map(Ok).collect::<Vec<_>>();
            states.reverse();
            Self {
                states,
                writes: 0,
                deletes: 0,
            }
        }

        fn failing(error: CurrentUserStartupError) -> Self {
            Self {
                states: vec![Err(error)],
                writes: 0,
                deletes: 0,
            }
        }
    }

    impl StartupBackend for FakeBackend {
        fn observe(&mut self) -> Result<CurrentUserStartupStatus, CurrentUserStartupError> {
            self.states
                .pop()
                .unwrap_or(Err(CurrentUserStartupError::Unavailable))
        }

        fn write_current(&mut self) -> Result<(), CurrentUserStartupError> {
            self.writes += 1;
            Ok(())
        }

        fn delete(&mut self) -> Result<(), CurrentUserStartupError> {
            self.deletes += 1;
            Ok(())
        }
    }

    #[test]
    fn enable_is_idempotent_and_stale_requires_explicit_repair() {
        let mut disabled = FakeBackend::new([
            CurrentUserStartupStatus::Disabled,
            CurrentUserStartupStatus::EnabledVerified,
        ]);
        assert_eq!(
            apply_with(&mut disabled, CurrentUserStartupAction::Enable),
            Ok(CurrentUserStartupSnapshot::new(
                CurrentUserStartupStatus::EnabledVerified
            ))
        );
        assert_eq!((disabled.writes, disabled.deletes), (1, 0));

        let mut enabled = FakeBackend::new([CurrentUserStartupStatus::EnabledVerified]);
        assert!(apply_with(&mut enabled, CurrentUserStartupAction::Enable).is_ok());
        assert_eq!((enabled.writes, enabled.deletes), (0, 0));

        let mut stale = FakeBackend::new([CurrentUserStartupStatus::StaleRelocation]);
        assert_eq!(
            apply_with(&mut stale, CurrentUserStartupAction::Enable),
            Err(CurrentUserStartupError::StaleRequiresRepair)
        );
        assert_eq!((stale.writes, stale.deletes), (0, 0));
    }

    #[test]
    fn repair_and_disable_mutate_only_owned_shapes_and_verify_readback() {
        let mut stale = FakeBackend::new([
            CurrentUserStartupStatus::StaleRelocation,
            CurrentUserStartupStatus::EnabledVerified,
        ]);
        assert!(apply_with(&mut stale, CurrentUserStartupAction::RepairStale).is_ok());
        assert_eq!((stale.writes, stale.deletes), (1, 0));

        let mut remove_stale = FakeBackend::new([
            CurrentUserStartupStatus::StaleRelocation,
            CurrentUserStartupStatus::Disabled,
        ]);
        assert!(apply_with(&mut remove_stale, CurrentUserStartupAction::Disable).is_ok());
        assert_eq!((remove_stale.writes, remove_stale.deletes), (0, 1));

        let mut conflict = FakeBackend::new([CurrentUserStartupStatus::Conflict]);
        assert_eq!(
            apply_with(&mut conflict, CurrentUserStartupAction::Disable),
            Err(CurrentUserStartupError::Conflict)
        );
        assert_eq!((conflict.writes, conflict.deletes), (0, 0));

        let mut mismatched = FakeBackend::new([
            CurrentUserStartupStatus::Disabled,
            CurrentUserStartupStatus::Disabled,
        ]);
        assert_eq!(
            apply_with(&mut mismatched, CurrentUserStartupAction::Enable),
            Err(CurrentUserStartupError::ReadbackFailed)
        );
    }

    #[test]
    fn denied_and_unavailable_inspection_are_visible_without_mutation() {
        let mut denied = FakeBackend::failing(CurrentUserStartupError::AccessDenied);
        assert_eq!(
            inspect_with(&mut denied).status(),
            CurrentUserStartupStatus::AccessDenied
        );
        let mut unavailable = FakeBackend::failing(CurrentUserStartupError::Unavailable);
        assert_eq!(
            inspect_with(&mut unavailable).status(),
            CurrentUserStartupStatus::Unavailable
        );
    }
}
