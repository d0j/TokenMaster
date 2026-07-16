use std::fs;
use std::io;
use std::path::Path;

use tokenmaster_provider::{
    DiagnosticCode, DiscoveryDiagnostics, DiscoveryProvider, DiscoveryRequest, DiscoverySnapshot,
    MAX_PROFILES, MAX_SOURCES, ProfileAvailability, ProfileDescriptor, ProviderCapability,
    ProviderDescriptor, ProviderError, ProviderId, SourceDescriptor, SourceKind,
};

use crate::identity::{profile_id_for_root, source_id_for_root};
use crate::path_policy::{PathPolicyCode, is_reparse_point, validate_local_root_namespace};

#[derive(Debug, Clone)]
pub struct CodexProvider {
    descriptor: ProviderDescriptor,
}

impl CodexProvider {
    pub fn new() -> Result<Self, ProviderError> {
        let descriptor = ProviderDescriptor::new(
            ProviderId::new("codex")?,
            "Codex",
            [
                ProviderCapability::History,
                ProviderCapability::Quota,
                ProviderCapability::Activity,
                ProviderCapability::Projects,
                ProviderCapability::Models,
                ProviderCapability::CodeOutput,
                ProviderCapability::RepositoryActivity,
            ],
        )?;
        Ok(Self { descriptor })
    }
}

impl DiscoveryProvider for CodexProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn discover(&self, request: &DiscoveryRequest) -> Result<DiscoverySnapshot, ProviderError> {
        let mut profiles = Vec::with_capacity(request.roots().len().min(MAX_PROFILES));
        let mut sources =
            Vec::with_capacity(request.roots().len().saturating_mul(2).min(MAX_SOURCES));
        let mut diagnostics = request.diagnostics().clone();

        for root in request.roots() {
            let profile_id = profile_id_for_root(root.path())?;
            let availability = if !root.enabled() {
                diagnostics.record(DiagnosticCode::DisabledRoot);
                ProfileAvailability::Rejected
            } else if let Some(availability) = namespace_availability(root.path(), &mut diagnostics)
            {
                availability
            } else {
                root_availability(root.path(), &mut diagnostics)?
            };
            let profile = ProfileDescriptor::new(
                profile_id.clone(),
                root.origin(),
                availability,
                root.label().map(ToOwned::to_owned),
                root.path(),
            )?;

            if profiles.len() == MAX_PROFILES {
                return Err(ProviderError::capacity_exceeded(MAX_PROFILES));
            }
            profiles.push(profile);

            if availability == ProfileAvailability::Available {
                discover_sources(
                    self.descriptor.id(),
                    &profile_id,
                    root.path(),
                    &mut sources,
                    &mut diagnostics,
                )?;
            }
        }

        DiscoverySnapshot::new(profiles, sources, diagnostics)
    }
}

fn namespace_availability(
    path: &Path,
    diagnostics: &mut DiscoveryDiagnostics,
) -> Option<ProfileAvailability> {
    match validate_local_root_namespace(path) {
        Ok(()) => None,
        Err(PathPolicyCode::UnsupportedRootNamespace) => {
            diagnostics.record(DiagnosticCode::UnsupportedRootNamespace);
            Some(ProfileAvailability::Rejected)
        }
        Err(PathPolicyCode::InvalidPath) => {
            diagnostics.record(DiagnosticCode::InvalidRoot);
            Some(ProfileAvailability::Rejected)
        }
    }
}

fn root_availability(
    path: &Path,
    diagnostics: &mut DiscoveryDiagnostics,
) -> Result<ProfileAvailability, ProviderError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_reparse_point(&metadata) => {
            diagnostics.record(DiagnosticCode::SymlinkRoot);
            Ok(ProfileAvailability::Rejected)
        }
        Ok(metadata) if metadata.is_dir() => Ok(ProfileAvailability::Available),
        Ok(_) => {
            diagnostics.record(DiagnosticCode::InvalidRoot);
            Ok(ProfileAvailability::Rejected)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ProfileAvailability::Missing),
        Err(_) => Err(ProviderError::io()),
    }
}

fn discover_sources(
    provider_id: &ProviderId,
    profile_id: &tokenmaster_provider::ProfileId,
    root: &Path,
    sources: &mut Vec<SourceDescriptor>,
    diagnostics: &mut DiscoveryDiagnostics,
) -> Result<(), ProviderError> {
    let start_len = sources.len();
    for (directory, kind) in [
        ("sessions", SourceKind::Active),
        ("archived_sessions", SourceKind::Archived),
    ] {
        let path = root.join(directory);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_reparse_point(&metadata) || !metadata.is_dir() => {
                diagnostics.record(DiagnosticCode::InvalidSource);
            }
            Ok(_) => add_source(provider_id, profile_id, kind, &path, sources)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(ProviderError::io()),
        }
    }

    if sources.len() == start_len {
        add_source(provider_id, profile_id, SourceKind::Direct, root, sources)?;
    }
    Ok(())
}

fn add_source(
    provider_id: &ProviderId,
    profile_id: &tokenmaster_provider::ProfileId,
    kind: SourceKind,
    path: &Path,
    sources: &mut Vec<SourceDescriptor>,
) -> Result<(), ProviderError> {
    if sources.len() == MAX_SOURCES {
        return Err(ProviderError::capacity_exceeded(MAX_SOURCES));
    }
    let source_id = source_id_for_root(provider_id, profile_id, kind, path)?;
    sources.push(SourceDescriptor::new(
        source_id,
        profile_id.clone(),
        kind,
        path,
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tokenmaster_provider::{DiagnosticCode, DiscoveryDiagnostics, ProfileAvailability};

    use super::namespace_availability;

    #[test]
    fn unsupported_namespace_becomes_rejected_state_before_filesystem_probe() {
        let mut diagnostics = DiscoveryDiagnostics::default();

        assert_eq!(
            namespace_availability(Path::new(r"\\.\PhysicalDrive0"), &mut diagnostics),
            Some(ProfileAvailability::Rejected)
        );
        assert_eq!(
            diagnostics.count(DiagnosticCode::UnsupportedRootNamespace),
            1
        );
        assert_eq!(
            namespace_availability(Path::new(r"C:\fixture"), &mut diagnostics),
            None
        );
    }
}
