use std::fmt;
use std::path::PathBuf;

use tokenmaster_codex::CodexProvider;
use tokenmaster_engine::Adapter;
use tokenmaster_provider::{
    DiscoveryProvider, DiscoveryRequest, ProviderCapability, ProviderDescriptor,
};

use crate::{GitRepositoryHintIngress, MAX_WATCH_ROOTS, RuntimeError, RuntimeErrorCode};

/// A bounded, provider-owned set of filesystem roots for the live watcher.
#[derive(Clone, Eq, PartialEq)]
pub struct ProviderWatchRoots {
    roots: Vec<PathBuf>,
}

impl fmt::Debug for ProviderWatchRoots {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderWatchRoots")
            .field("root_count", &self.roots.len())
            .finish()
    }
}

impl ProviderWatchRoots {
    #[must_use]
    pub const fn empty() -> Self {
        Self { roots: Vec::new() }
    }

    pub fn try_new(roots: Vec<PathBuf>) -> Result<Self, RuntimeError> {
        if roots.len() > MAX_WATCH_ROOTS {
            return Err(RuntimeError::new(RuntimeErrorCode::InvalidConfiguration));
        }
        Ok(Self { roots })
    }

    pub(crate) fn from_bounded(roots: Vec<PathBuf>) -> Self {
        debug_assert!(roots.len() <= MAX_WATCH_ROOTS);
        Self { roots }
    }

    #[must_use]
    pub(crate) fn as_slice(&self) -> &[PathBuf] {
        &self.roots
    }
}

pub trait LiveProviderAdapter: Adapter {
    fn watch_roots(&self) -> ProviderWatchRoots;
}

pub trait UsageProviderFactory: Send + 'static {
    fn descriptor(&self) -> &ProviderDescriptor;

    fn build(
        self: Box<Self>,
        repository_hints: Option<GitRepositoryHintIngress>,
    ) -> Result<Box<dyn LiveProviderAdapter>, RuntimeError>;
}

pub struct CodexUsageProviderFactory {
    descriptor: ProviderDescriptor,
    request: DiscoveryRequest,
}

impl CodexUsageProviderFactory {
    pub fn new(request: DiscoveryRequest) -> Result<Self, RuntimeError> {
        let provider = CodexProvider::new()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::ProviderUnavailable))?;
        Ok(Self {
            descriptor: provider.descriptor().clone(),
            request,
        })
    }
}

impl UsageProviderFactory for CodexUsageProviderFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn build(
        self: Box<Self>,
        repository_hints: Option<GitRepositoryHintIngress>,
    ) -> Result<Box<dyn LiveProviderAdapter>, RuntimeError> {
        let adapter = crate::CodexAdapter::new(self.request)?;
        let adapter = match repository_hints {
            Some(ingress) => adapter.with_repository_hint_ingress(ingress),
            None => adapter,
        };
        Ok(Box::new(adapter))
    }
}

pub(crate) fn repository_hints_for(
    factory: &dyn UsageProviderFactory,
    ingress: GitRepositoryHintIngress,
) -> Option<GitRepositoryHintIngress> {
    factory
        .descriptor()
        .supports(ProviderCapability::RepositoryActivity)
        .then_some(ingress)
}
