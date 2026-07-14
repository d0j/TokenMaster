use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{ProfileId, ProviderDescriptor, ProviderError, ProviderErrorCode, SourceId};

pub const MAX_PROFILES: usize = 64;
pub const MAX_SOURCES: usize = 128;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_LABEL_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RootOrigin {
    Default,
    Environment,
    Configured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProfileAvailability {
    Available,
    Missing,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    Active,
    Direct,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    EmptyRoot,
    InvalidRoot,
    DisabledRoot,
    SymlinkRoot,
    UnsupportedRootNamespace,
    InvalidSource,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveryDiagnostics {
    empty_root: u16,
    invalid_root: u16,
    disabled_root: u16,
    symlink_root: u16,
    unsupported_root_namespace: u16,
    invalid_source: u16,
}

impl DiscoveryDiagnostics {
    pub fn record(&mut self, code: DiagnosticCode) {
        let counter = match code {
            DiagnosticCode::EmptyRoot => &mut self.empty_root,
            DiagnosticCode::InvalidRoot => &mut self.invalid_root,
            DiagnosticCode::DisabledRoot => &mut self.disabled_root,
            DiagnosticCode::SymlinkRoot => &mut self.symlink_root,
            DiagnosticCode::UnsupportedRootNamespace => &mut self.unsupported_root_namespace,
            DiagnosticCode::InvalidSource => &mut self.invalid_source,
        };
        *counter = counter.saturating_add(1);
    }

    #[must_use]
    pub const fn count(&self, code: DiagnosticCode) -> u16 {
        match code {
            DiagnosticCode::EmptyRoot => self.empty_root,
            DiagnosticCode::InvalidRoot => self.invalid_root,
            DiagnosticCode::DisabledRoot => self.disabled_root,
            DiagnosticCode::SymlinkRoot => self.symlink_root,
            DiagnosticCode::UnsupportedRootNamespace => self.unsupported_root_namespace,
            DiagnosticCode::InvalidSource => self.invalid_source,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DiscoveryRoot {
    path: PathBuf,
    origin: RootOrigin,
    label: Option<String>,
    enabled: bool,
}

impl DiscoveryRoot {
    pub fn new(
        path: impl Into<PathBuf>,
        origin: RootOrigin,
        label: Option<String>,
        enabled: bool,
    ) -> Result<Self, ProviderError> {
        let path = path.into();
        validate_path(&path)?;
        Ok(Self {
            path,
            origin,
            label: label.map(|value| truncate_utf8(value, MAX_LABEL_BYTES)),
            enabled,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn origin(&self) -> RootOrigin {
        self.origin
    }

    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

impl fmt::Debug for DiscoveryRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscoveryRoot")
            .field("path", &"[redacted]")
            .field("origin", &self.origin)
            .field("label", &self.label)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRequest {
    roots: Vec<DiscoveryRoot>,
    diagnostics: DiscoveryDiagnostics,
}

impl DiscoveryRequest {
    pub fn new(roots: Vec<DiscoveryRoot>) -> Result<Self, ProviderError> {
        Self::with_diagnostics(roots, DiscoveryDiagnostics::default())
    }

    pub fn with_diagnostics(
        roots: Vec<DiscoveryRoot>,
        diagnostics: DiscoveryDiagnostics,
    ) -> Result<Self, ProviderError> {
        if roots.len() > MAX_PROFILES {
            return Err(ProviderError::with_limit(
                ProviderErrorCode::TooManyRoots,
                MAX_PROFILES,
            ));
        }
        Ok(Self { roots, diagnostics })
    }

    #[must_use]
    pub fn roots(&self) -> &[DiscoveryRoot] {
        &self.roots
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &DiscoveryDiagnostics {
        &self.diagnostics
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProfileDescriptor {
    id: ProfileId,
    origin: RootOrigin,
    availability: ProfileAvailability,
    label: Option<String>,
    path: PathBuf,
}

impl ProfileDescriptor {
    pub fn new(
        id: ProfileId,
        origin: RootOrigin,
        availability: ProfileAvailability,
        label: Option<String>,
        path: impl Into<PathBuf>,
    ) -> Result<Self, ProviderError> {
        let path = path.into();
        validate_path(&path)?;
        Ok(Self {
            id,
            origin,
            availability,
            label: label.map(|value| truncate_utf8(value, MAX_LABEL_BYTES)),
            path,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &ProfileId {
        &self.id
    }

    #[must_use]
    pub const fn origin(&self) -> RootOrigin {
        self.origin
    }

    #[must_use]
    pub const fn availability(&self) -> ProfileAvailability {
        self.availability
    }

    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Debug for ProfileDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProfileDescriptor")
            .field("id", &self.id)
            .field("origin", &self.origin)
            .field("availability", &self.availability)
            .field("label", &self.label)
            .field("path", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SourceDescriptor {
    id: SourceId,
    profile_id: ProfileId,
    kind: SourceKind,
    path: PathBuf,
}

impl SourceDescriptor {
    pub fn new(
        id: SourceId,
        profile_id: ProfileId,
        kind: SourceKind,
        path: impl Into<PathBuf>,
    ) -> Result<Self, ProviderError> {
        let path = path.into();
        validate_path(&path)?;
        Ok(Self {
            id,
            profile_id,
            kind,
            path,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &SourceId {
        &self.id
    }

    #[must_use]
    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub const fn kind(&self) -> SourceKind {
        self.kind
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Debug for SourceDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceDescriptor")
            .field("id", &self.id)
            .field("profile_id", &self.profile_id)
            .field("kind", &self.kind)
            .field("path", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverySnapshot {
    profiles: Vec<ProfileDescriptor>,
    sources: Vec<SourceDescriptor>,
    diagnostics: DiscoveryDiagnostics,
}

impl DiscoverySnapshot {
    pub fn new(
        profiles: Vec<ProfileDescriptor>,
        sources: Vec<SourceDescriptor>,
        diagnostics: DiscoveryDiagnostics,
    ) -> Result<Self, ProviderError> {
        if profiles.len() > MAX_PROFILES {
            return Err(ProviderError::capacity_exceeded(MAX_PROFILES));
        }
        if sources.len() > MAX_SOURCES {
            return Err(ProviderError::capacity_exceeded(MAX_SOURCES));
        }
        Ok(Self {
            profiles,
            sources,
            diagnostics,
        })
    }

    #[must_use]
    pub fn profiles(&self) -> &[ProfileDescriptor] {
        &self.profiles
    }

    #[must_use]
    pub fn sources(&self) -> &[SourceDescriptor] {
        &self.sources
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &DiscoveryDiagnostics {
        &self.diagnostics
    }
}

pub trait DiscoveryProvider {
    fn descriptor(&self) -> &ProviderDescriptor;

    fn discover(&self, request: &DiscoveryRequest) -> Result<DiscoverySnapshot, ProviderError>;
}

fn validate_path(path: &Path) -> Result<(), ProviderError> {
    if !path.is_absolute() || path_has_nul(path) || path_byte_len(path) > MAX_PATH_BYTES {
        return Err(ProviderError::with_limit(
            ProviderErrorCode::InvalidPath,
            MAX_PATH_BYTES,
        ));
    }
    Ok(())
}

fn truncate_utf8(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value
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

#[cfg(windows)]
fn path_has_nul(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().any(|unit| unit == 0)
}

#[cfg(not(windows))]
fn path_has_nul(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().contains(&0)
}
