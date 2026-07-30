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
    use std::os::windows::io::AsRawHandle;
    use std::path::{Component, Path, PathBuf, Prefix};

    use windows::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_PATH_NOT_FOUND,
        ERROR_SUCCESS, HANDLE, WIN32_ERROR,
    };
    use windows::Win32::Storage::FileSystem::{
        FILE_NAME_NORMALIZED, GETFINALPATHNAMEBYHANDLE_FLAGS, GetDriveTypeW,
        GetFinalPathNameByHandleW, VOLUME_NAME_DOS,
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
    // The Windows Run contract limits command data to 260 UTF-16 code units.
    const MAX_COMMAND_UTF16_UNITS: usize = 260;
    const MAX_REGISTRY_VALUE_BYTES: u32 = ((MAX_COMMAND_UTF16_UNITS + 1) * 2) as u32;
    const MAX_RESOLVED_PATH_UTF16_UNITS: usize = 1024;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    struct VerifiedExecutable {
        path: PathBuf,
        identity: PhysicalFileIdentity,
        basename: OsString,
        command: Vec<u16>,
    }

    impl VerifiedExecutable {
        fn current() -> Result<Self, CurrentUserStartupError> {
            let launch_path =
                std::env::current_exe().map_err(|_| CurrentUserStartupError::Unavailable)?;
            let (file, path) = open_verified_local_file(&launch_path)?;
            let command = build_command(&path)?;
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
                command,
            })
        }
    }

    fn build_command(path: &Path) -> Result<Vec<u16>, CurrentUserStartupError> {
        let path = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if path.contains(&u16::from(b'"')) {
            return Err(CurrentUserStartupError::Unavailable);
        }
        let mut command = Vec::with_capacity(path.len().saturating_add(3));
        command.push(u16::from(b'"'));
        command.extend(path);
        command.push(u16::from(b'"'));
        if command.len() > MAX_COMMAND_UTF16_UNITS {
            return Err(CurrentUserStartupError::Unavailable);
        }
        command.push(0);
        let bytes = command
            .len()
            .checked_mul(2)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(CurrentUserStartupError::Unavailable)?;
        if bytes > MAX_REGISTRY_VALUE_BYTES {
            return Err(CurrentUserStartupError::Unavailable);
        }
        Ok(command)
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
            let command = &self.executable.command;
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
        if !valid_registry_shape(value_type, bytes) {
            return Ok(Some(Vec::new()));
        }
        let expected_units = (bytes / 2) as usize;
        let buffer_units = expected_units
            .checked_add(1)
            .ok_or(CurrentUserStartupError::Unavailable)?;
        let mut command = vec![u16::MAX; buffer_units];
        let mut read_bytes = bytes
            .checked_add(2)
            .ok_or(CurrentUserStartupError::Unavailable)?;
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
        if read_result == ERROR_MORE_DATA {
            return Ok(Some(Vec::new()));
        }
        map_registry_result(read_result)?;
        Ok(Some(finalize_registry_command(
            command,
            expected_units,
            bytes,
            read_bytes,
            value_type,
        )))
    }

    fn finalize_registry_command(
        mut command: Vec<u16>,
        expected_units: usize,
        expected_bytes: u32,
        read_bytes: u32,
        value_type: REG_VALUE_TYPE,
    ) -> Vec<u16> {
        if value_type != REG_SZ
            || read_bytes != expected_bytes
            || command.get(expected_units.saturating_sub(1)) != Some(&0)
        {
            return Vec::new();
        }
        command.truncate(expected_units);
        command.pop();
        if command.is_empty() || command.contains(&0) {
            Vec::new()
        } else {
            command
        }
    }

    fn valid_registry_shape(value_type: REG_VALUE_TYPE, bytes: u32) -> bool {
        value_type == REG_SZ
            && (2..=MAX_REGISTRY_VALUE_BYTES).contains(&bytes)
            && bytes.is_multiple_of(2)
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
        if !supported_local_drive(&path) {
            return Ok(CurrentUserStartupStatus::Conflict);
        }
        // A registry-controlled alternate path is stale without being opened. This
        // prevents inspection from following UNC, device, mapped-remote, or reparse
        // destinations and keeps repair explicit.
        if path != current.path {
            return Ok(CurrentUserStartupStatus::StaleRelocation);
        }
        let (file, resolved_path) = open_verified_local_file(&path)?;
        if resolved_path != current.path {
            return Ok(CurrentUserStartupStatus::StaleRelocation);
        }
        let identity = PhysicalFileIdentity::from_file(&file)
            .map_err(|_| CurrentUserStartupError::Unavailable)?;
        if identity == current.identity {
            Ok(CurrentUserStartupStatus::EnabledVerified)
        } else {
            Ok(CurrentUserStartupStatus::StaleRelocation)
        }
    }

    fn parse_exact_quoted_path(command: &[u16]) -> Option<PathBuf> {
        let quote = u16::from(b'"');
        if command.len() < 3
            || command.len() > MAX_COMMAND_UTF16_UNITS
            || command.first() != Some(&quote)
            || command.last() != Some(&quote)
            || command[1..command.len() - 1].contains(&quote)
        {
            return None;
        }
        let path = PathBuf::from(OsString::from_wide(&command[1..command.len() - 1]));
        local_drive_root(&path).map(|_| path)
    }

    fn local_drive_root(path: &Path) -> Option<[u16; 4]> {
        let mut components = path.components();
        let drive = match components.next()? {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(drive) => drive,
                _ => return None,
            },
            _ => return None,
        };
        if !matches!(components.next(), Some(Component::RootDir))
            || !components.all(|component| matches!(component, Component::Normal(_)))
        {
            return None;
        }
        Some([u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0])
    }

    fn supported_local_drive(path: &Path) -> bool {
        let Some(root) = local_drive_root(path) else {
            return false;
        };
        // SAFETY: `root` is a valid NUL-terminated `X:\` UTF-16 string and the API
        // does not retain its pointer. No file or network path is opened here.
        let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
        supported_drive_type(drive_type)
    }

    const fn supported_drive_type(drive_type: u32) -> bool {
        const DRIVE_REMOVABLE: u32 = 2;
        const DRIVE_FIXED: u32 = 3;
        const DRIVE_RAMDISK: u32 = 6;

        matches!(drive_type, DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_RAMDISK)
    }

    fn same_ascii_name(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
        let left = left.to_string_lossy();
        let right = right.to_string_lossy();
        left.is_ascii() && right.is_ascii() && left.eq_ignore_ascii_case(&right)
    }

    fn validate_ordinary_file(path: &Path) -> Result<(), CurrentUserStartupError> {
        if !supported_local_drive(path) {
            return Err(CurrentUserStartupError::Unavailable);
        }
        reject_reparse_ancestry(path)?;
        Ok(())
    }

    fn open_verified_local_file(path: &Path) -> Result<(File, PathBuf), CurrentUserStartupError> {
        validate_ordinary_file(path)?;
        let file = crate::windows::open_regular_no_follow(path)
            .map_err(|_| CurrentUserStartupError::Unavailable)?;
        let resolved_path = resolved_local_path(&file)?;
        Ok((file, resolved_path))
    }

    fn resolved_local_path(file: &File) -> Result<PathBuf, CurrentUserStartupError> {
        let mut resolved = vec![0_u16; MAX_RESOLVED_PATH_UTF16_UNITS];
        // SAFETY: `file` remains open, `resolved` is one bounded writable UTF-16
        // buffer, and GetFinalPathNameByHandleW does not retain either argument.
        let written = unsafe {
            GetFinalPathNameByHandleW(
                HANDLE(file.as_raw_handle()),
                &mut resolved,
                GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
            )
        } as usize;
        if written == 0 || written >= resolved.len() {
            return Err(CurrentUserStartupError::Unavailable);
        }
        resolved.truncate(written);
        parse_resolved_dos_path(&resolved).ok_or(CurrentUserStartupError::Unavailable)
    }

    fn parse_resolved_dos_path(resolved: &[u16]) -> Option<PathBuf> {
        if !resolved.starts_with(&wide_prefix()) {
            return None;
        }
        let path = PathBuf::from(OsString::from_wide(&resolved[wide_prefix().len()..]));
        supported_local_drive(&path).then_some(path)
    }

    const fn wide_prefix() -> [u16; 4] {
        [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16]
    }

    fn reject_reparse_ancestry(path: &Path) -> Result<(), CurrentUserStartupError> {
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component.as_os_str());
            if matches!(component, Component::Prefix(_) | Component::RootDir) {
                continue;
            }
            let metadata =
                fs::symlink_metadata(&current).map_err(|_| CurrentUserStartupError::Unavailable)?;
            if metadata.file_type().is_symlink()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(CurrentUserStartupError::Unavailable);
            }
        }
        Ok(())
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

    #[cfg(test)]
    mod tests {
        use std::ffi::OsStr;

        use super::*;

        fn wide(value: &str) -> Vec<u16> {
            OsStr::new(value).encode_wide().collect()
        }

        #[test]
        fn exact_command_parser_accepts_only_one_quoted_absolute_path() {
            assert_eq!(
                parse_exact_quoted_path(&wide(r#""C:\Apps\TokenMaster.exe""#)),
                Some(PathBuf::from(r"C:\Apps\TokenMaster.exe"))
            );
            assert_eq!(
                parse_exact_quoted_path(&wide(r#""C:\Apps\TokenMaster.exe" --hidden"#)),
                None
            );
            assert_eq!(
                parse_exact_quoted_path(&wide(r"C:\Apps\TokenMaster.exe")),
                None
            );
            assert_eq!(parse_exact_quoted_path(&wide(r#""TokenMaster.exe""#)), None);
            assert_eq!(
                parse_exact_quoted_path(&wide(r#""\\server\share\TokenMaster.exe""#)),
                None
            );
            assert_eq!(
                parse_exact_quoted_path(&wide(r#""\\?\C:\Apps\TokenMaster.exe""#)),
                None
            );
            assert_eq!(
                parse_exact_quoted_path(&wide(r#""\\.\C:\Apps\TokenMaster.exe""#)),
                None
            );
            assert_eq!(
                parse_exact_quoted_path(&wide(r#""C:\Apps\"TokenMaster.exe""#)),
                None
            );

            let at_limit = format!(r#""C:\{}""#, "a".repeat(255));
            let over_limit = format!(r#""C:\{}""#, "a".repeat(256));
            assert_eq!(wide(&at_limit).len(), MAX_COMMAND_UTF16_UNITS);
            assert!(parse_exact_quoted_path(&wide(&at_limit)).is_some());
            assert_eq!(wide(&over_limit).len(), MAX_COMMAND_UTF16_UNITS + 1);
            assert_eq!(parse_exact_quoted_path(&wide(&over_limit)), None);
        }

        #[test]
        fn current_command_builder_rejects_an_impossible_run_registration_early() {
            let at_limit = PathBuf::from(format!(r"C:\{}", "a".repeat(255)));
            let over_limit = PathBuf::from(format!(r"C:\{}", "a".repeat(256)));

            let command = build_command(&at_limit)
                .unwrap_or_else(|error| panic!("260-unit command must fit: {error:?}"));
            assert_eq!(command.len(), MAX_COMMAND_UTF16_UNITS + 1);
            assert_eq!(command.last(), Some(&0));
            assert_eq!(
                build_command(&over_limit),
                Err(CurrentUserStartupError::Unavailable)
            );
        }

        #[test]
        fn registry_string_finalizer_rejects_missing_or_embedded_nul_and_size_drift() {
            let valid = wide(r#""C:\Apps\TokenMaster.exe""#);
            let expected_units = valid.len() + 1;
            let expected_bytes = (expected_units * 2) as u32;

            let mut terminated = valid.clone();
            terminated.extend([0, u16::MAX]);
            assert_eq!(
                finalize_registry_command(
                    terminated,
                    expected_units,
                    expected_bytes,
                    expected_bytes,
                    REG_SZ,
                ),
                valid
            );

            let mut missing_nul = valid.clone();
            missing_nul.extend([u16::from(b'x'), 0]);
            assert!(
                finalize_registry_command(
                    missing_nul,
                    expected_units,
                    expected_bytes,
                    expected_bytes,
                    REG_SZ,
                )
                .is_empty()
            );

            let mut embedded_nul = valid;
            embedded_nul[2] = 0;
            embedded_nul.extend([0, u16::MAX]);
            assert!(
                finalize_registry_command(
                    embedded_nul,
                    expected_units,
                    expected_bytes,
                    expected_bytes,
                    REG_SZ,
                )
                .is_empty()
            );

            assert!(
                finalize_registry_command(
                    vec![0; expected_units + 1],
                    expected_units,
                    expected_bytes,
                    expected_bytes + 2,
                    REG_SZ,
                )
                .is_empty()
            );

            assert!(
                finalize_registry_command(
                    vec![0; expected_units + 1],
                    expected_units,
                    expected_bytes,
                    expected_bytes,
                    REG_VALUE_TYPE(999),
                )
                .is_empty()
            );

            assert!(valid_registry_shape(REG_SZ, 2));
            assert!(valid_registry_shape(REG_SZ, MAX_REGISTRY_VALUE_BYTES));
            assert!(!valid_registry_shape(REG_SZ, 0));
            assert!(!valid_registry_shape(REG_SZ, 1));
            assert!(!valid_registry_shape(REG_SZ, 3));
            assert!(!valid_registry_shape(REG_SZ, MAX_REGISTRY_VALUE_BYTES + 2));
            assert!(!valid_registry_shape(REG_VALUE_TYPE(999), 2));
        }

        #[test]
        fn owned_basename_match_is_ascii_case_insensitive_and_otherwise_closed() {
            assert!(same_ascii_name(
                OsStr::new("TOKENMASTER.EXE"),
                OsStr::new("TokenMaster.exe")
            ));
            assert!(!same_ascii_name(
                OsStr::new("Other.exe"),
                OsStr::new("TokenMaster.exe")
            ));
            assert!(!same_ascii_name(
                OsStr::new("TokenMastеr.exe"),
                OsStr::new("TokenMaster.exe")
            ));
            assert!(supported_drive_type(2));
            assert!(supported_drive_type(3));
            assert!(supported_drive_type(6));
            assert!(!supported_drive_type(0));
            assert!(!supported_drive_type(4));

            assert_eq!(
                parse_resolved_dos_path(&wide(r"\\?\C:\Apps\TokenMaster.exe")),
                Some(PathBuf::from(r"C:\Apps\TokenMaster.exe"))
            );
            assert_eq!(
                parse_resolved_dos_path(&wide(r"\\?\UNC\server\TokenMaster.exe")),
                None
            );
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
        write_result: Result<(), CurrentUserStartupError>,
        delete_result: Result<(), CurrentUserStartupError>,
    }

    impl FakeBackend {
        fn new(states: impl IntoIterator<Item = CurrentUserStartupStatus>) -> Self {
            let mut states = states.into_iter().map(Ok).collect::<Vec<_>>();
            states.reverse();
            Self {
                states,
                writes: 0,
                deletes: 0,
                write_result: Ok(()),
                delete_result: Ok(()),
            }
        }

        fn failing(error: CurrentUserStartupError) -> Self {
            Self {
                states: vec![Err(error)],
                writes: 0,
                deletes: 0,
                write_result: Ok(()),
                delete_result: Ok(()),
            }
        }

        fn with_write_error(mut self, error: CurrentUserStartupError) -> Self {
            self.write_result = Err(error);
            self
        }

        fn with_delete_error(mut self, error: CurrentUserStartupError) -> Self {
            self.delete_result = Err(error);
            self
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
            self.write_result
        }

        fn delete(&mut self) -> Result<(), CurrentUserStartupError> {
            self.deletes += 1;
            self.delete_result
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

        let mut repair_enabled = FakeBackend::new([CurrentUserStartupStatus::EnabledVerified]);
        assert!(apply_with(&mut repair_enabled, CurrentUserStartupAction::RepairStale).is_ok());
        assert_eq!((repair_enabled.writes, repair_enabled.deletes), (0, 0));

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

        let mut remove_enabled = FakeBackend::new([
            CurrentUserStartupStatus::EnabledVerified,
            CurrentUserStartupStatus::Disabled,
        ]);
        assert!(apply_with(&mut remove_enabled, CurrentUserStartupAction::Disable).is_ok());
        assert_eq!((remove_enabled.writes, remove_enabled.deletes), (0, 1));

        let mut already_disabled = FakeBackend::new([CurrentUserStartupStatus::Disabled]);
        assert!(apply_with(&mut already_disabled, CurrentUserStartupAction::Disable).is_ok());
        assert_eq!((already_disabled.writes, already_disabled.deletes), (0, 0));

        let mut invalid_repair = FakeBackend::new([CurrentUserStartupStatus::Disabled]);
        assert_eq!(
            apply_with(&mut invalid_repair, CurrentUserStartupAction::RepairStale),
            Err(CurrentUserStartupError::InvalidState)
        );

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

        let mut mismatched_repair = FakeBackend::new([
            CurrentUserStartupStatus::StaleRelocation,
            CurrentUserStartupStatus::StaleRelocation,
        ]);
        assert_eq!(
            apply_with(
                &mut mismatched_repair,
                CurrentUserStartupAction::RepairStale
            ),
            Err(CurrentUserStartupError::ReadbackFailed)
        );

        let mut mismatched_disable = FakeBackend::new([
            CurrentUserStartupStatus::EnabledVerified,
            CurrentUserStartupStatus::EnabledVerified,
        ]);
        assert_eq!(
            apply_with(&mut mismatched_disable, CurrentUserStartupAction::Disable),
            Err(CurrentUserStartupError::ReadbackFailed)
        );
    }

    #[test]
    fn degraded_states_reject_every_action_without_mutation() {
        for (status, expected) in [
            (
                CurrentUserStartupStatus::Conflict,
                CurrentUserStartupError::Conflict,
            ),
            (
                CurrentUserStartupStatus::AccessDenied,
                CurrentUserStartupError::AccessDenied,
            ),
            (
                CurrentUserStartupStatus::Unavailable,
                CurrentUserStartupError::Unavailable,
            ),
        ] {
            for action in [
                CurrentUserStartupAction::Enable,
                CurrentUserStartupAction::RepairStale,
                CurrentUserStartupAction::Disable,
            ] {
                let mut backend = FakeBackend::new([status]);
                assert_eq!(apply_with(&mut backend, action), Err(expected));
                assert_eq!((backend.writes, backend.deletes), (0, 0));
            }
        }
    }

    #[test]
    fn mutation_failures_propagate_without_false_success_or_readback() {
        let mut write_failure = FakeBackend::new([CurrentUserStartupStatus::Disabled])
            .with_write_error(CurrentUserStartupError::AccessDenied);
        assert_eq!(
            apply_with(&mut write_failure, CurrentUserStartupAction::Enable),
            Err(CurrentUserStartupError::AccessDenied)
        );
        assert_eq!((write_failure.writes, write_failure.deletes), (1, 0));

        let mut delete_failure = FakeBackend::new([CurrentUserStartupStatus::EnabledVerified])
            .with_delete_error(CurrentUserStartupError::Unavailable);
        assert_eq!(
            apply_with(&mut delete_failure, CurrentUserStartupAction::Disable),
            Err(CurrentUserStartupError::Unavailable)
        );
        assert_eq!((delete_failure.writes, delete_failure.deletes), (0, 1));
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
