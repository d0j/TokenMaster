use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use tokenmaster_provider::{
    DiagnosticCode, DiscoveryDiagnostics, DiscoveryRequest, DiscoveryRoot, MAX_PROFILES,
    ProviderError, RootOrigin,
};

use crate::identity::{comparison_key, normalize_absolute_path};

#[derive(Clone, PartialEq, Eq)]
pub struct ConfiguredCodexRoot {
    path: PathBuf,
    label: Option<String>,
    enabled: bool,
}

impl ConfiguredCodexRoot {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, label: Option<String>, enabled: bool) -> Self {
        Self {
            path: path.into(),
            label,
            enabled,
        }
    }
}

impl fmt::Debug for ConfiguredCodexRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredCodexRoot")
            .field("path", &"[redacted]")
            .field("label", &self.label)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct CodexRootInput<'a> {
    pub user_profile: Option<&'a Path>,
    pub codex_home: Option<&'a str>,
    pub configured: &'a [ConfiguredCodexRoot],
}

pub fn build_discovery_request(
    input: CodexRootInput<'_>,
) -> Result<DiscoveryRequest, ProviderError> {
    if input.configured.len() > MAX_PROFILES {
        return Err(ProviderError::too_many_roots(MAX_PROFILES));
    }

    let mut diagnostics = DiscoveryDiagnostics::default();
    let mut roots = Vec::new();
    let mut positions: HashMap<Vec<u8>, usize> = HashMap::new();

    if let Some(user_profile) = input.user_profile {
        add_candidate(
            user_profile.join(".codex"),
            RootOrigin::Default,
            None,
            true,
            &mut roots,
            &mut positions,
            &mut diagnostics,
        )?;
    }

    if let Some(raw) = input.codex_home {
        let mut non_empty = 0_usize;
        for value in raw.split(',') {
            let value = value.trim();
            if value.is_empty() {
                diagnostics.record(DiagnosticCode::EmptyRoot);
                continue;
            }
            non_empty += 1;
            if non_empty > MAX_PROFILES {
                return Err(ProviderError::too_many_roots(MAX_PROFILES));
            }
            add_candidate(
                PathBuf::from(value),
                RootOrigin::Environment,
                None,
                true,
                &mut roots,
                &mut positions,
                &mut diagnostics,
            )?;
        }
    }

    for configured in input.configured {
        add_candidate(
            configured.path.clone(),
            RootOrigin::Configured,
            configured.label.clone(),
            configured.enabled,
            &mut roots,
            &mut positions,
            &mut diagnostics,
        )?;
    }

    DiscoveryRequest::with_diagnostics(roots, diagnostics)
}

#[allow(clippy::too_many_arguments)]
fn add_candidate(
    path: PathBuf,
    origin: RootOrigin,
    label: Option<String>,
    enabled: bool,
    roots: &mut Vec<DiscoveryRoot>,
    positions: &mut HashMap<Vec<u8>, usize>,
    diagnostics: &mut DiscoveryDiagnostics,
) -> Result<(), ProviderError> {
    let normalized = match normalize_absolute_path(&path) {
        Ok(normalized) => normalized,
        Err(_) => {
            diagnostics.record(DiagnosticCode::InvalidRoot);
            return Ok(());
        }
    };
    let key = comparison_key(&normalized);
    let candidate = DiscoveryRoot::new(normalized, origin, label, enabled)?;

    if let Some(index) = positions.get(&key).copied() {
        if origin > roots[index].origin() {
            roots[index] = candidate;
        }
        return Ok(());
    }

    if roots.len() == MAX_PROFILES {
        return Err(ProviderError::too_many_roots(MAX_PROFILES));
    }
    positions.insert(key, roots.len());
    roots.push(candidate);
    Ok(())
}
