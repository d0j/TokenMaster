use std::collections::BTreeSet;

use crate::{ProviderError, ProviderErrorCode, ProviderId};

pub const MAX_DISPLAY_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProviderCapability {
    History,
    Quota,
    Activity,
    Projects,
    Models,
    CodeOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    id: ProviderId,
    display_name: String,
    capabilities: BTreeSet<ProviderCapability>,
}

impl ProviderDescriptor {
    pub fn new(
        id: ProviderId,
        display_name: impl Into<String>,
        capabilities: impl IntoIterator<Item = ProviderCapability>,
    ) -> Result<Self, ProviderError> {
        let display_name = display_name.into();
        if display_name.is_empty() || display_name.len() > MAX_DISPLAY_NAME_BYTES {
            return Err(ProviderError::with_limit(
                ProviderErrorCode::InvalidDisplayName,
                MAX_DISPLAY_NAME_BYTES,
            ));
        }

        Ok(Self {
            id,
            display_name,
            capabilities: capabilities.into_iter().collect(),
        })
    }

    #[must_use]
    pub const fn id(&self) -> &ProviderId {
        &self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn capabilities(&self) -> &BTreeSet<ProviderCapability> {
        &self.capabilities
    }

    #[must_use]
    pub fn supports(&self, capability: ProviderCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}
