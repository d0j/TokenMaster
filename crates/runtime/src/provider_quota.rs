use std::fmt;

use tokenmaster_codex::{CodexQuotaErrorCode, CodexQuotaSnapshot, CodexQuotaTransport};
use tokenmaster_domain::{BenefitInventoryObservation, QuotaSample, QuotaWindowDefinition};

use crate::{CodexExecutableDiscoveryErrorCode, CodexQuotaRuntimeConfig};

pub const MAX_PROVIDER_QUOTA_WINDOWS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderPollErrorCode {
    DiscoveryUnavailable,
    DiscoveryCapacityExceeded,
    Unavailable,
    SpawnFailed,
    ProcessExited,
    ProcessCleanupFailed,
    InvalidData,
    AccountIdentityUnavailable,
    InvalidTime,
    InvalidCommand,
    ProtocolError,
    UnsupportedVersion,
    RpcError,
    DeadlineExceeded,
    CapacityExceeded,
}

impl ProviderPollErrorCode {
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::DiscoveryUnavailable
            | Self::Unavailable
            | Self::SpawnFailed
            | Self::ProcessExited
            | Self::ProcessCleanupFailed => "unavailable",
            Self::DiscoveryCapacityExceeded | Self::CapacityExceeded => "capacity_exceeded",
            Self::InvalidData
            | Self::AccountIdentityUnavailable
            | Self::InvalidTime
            | Self::InvalidCommand
            | Self::ProtocolError
            | Self::UnsupportedVersion
            | Self::RpcError => "invalid_data",
            Self::DeadlineExceeded => "deadline_exceeded",
        }
    }
}

impl fmt::Display for ProviderPollErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProviderQuotaObservation {
    definition: QuotaWindowDefinition,
    sample: QuotaSample,
}

impl ProviderQuotaObservation {
    #[must_use]
    pub const fn new(definition: QuotaWindowDefinition, sample: QuotaSample) -> Self {
        Self { definition, sample }
    }

    #[must_use]
    pub const fn definition(&self) -> &QuotaWindowDefinition {
        &self.definition
    }

    #[must_use]
    pub const fn sample(&self) -> &QuotaSample {
        &self.sample
    }
}

impl fmt::Debug for ProviderQuotaObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderQuotaObservation([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProviderQuotaPoll {
    observed_at_ms: i64,
    quota: Box<[ProviderQuotaObservation]>,
    benefits: Option<BenefitInventoryObservation>,
}

impl ProviderQuotaPoll {
    #[must_use]
    pub fn new(
        observed_at_ms: i64,
        quota: Vec<ProviderQuotaObservation>,
        benefits: Option<BenefitInventoryObservation>,
    ) -> Self {
        Self {
            observed_at_ms,
            quota: quota.into_boxed_slice(),
            benefits,
        }
    }

    #[must_use]
    pub const fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    #[must_use]
    pub const fn quota(&self) -> &[ProviderQuotaObservation] {
        &self.quota
    }

    #[must_use]
    pub const fn benefits(&self) -> Option<&BenefitInventoryObservation> {
        self.benefits.as_ref()
    }

    pub(crate) fn from_codex(observed_at_ms: i64, snapshot: CodexQuotaSnapshot) -> Self {
        let quota = snapshot
            .observations()
            .iter()
            .map(|observation| {
                ProviderQuotaObservation::new(
                    observation.definition().clone(),
                    observation.sample().clone(),
                )
            })
            .collect();
        Self::new(
            observed_at_ms,
            quota,
            snapshot.benefit_observation().cloned(),
        )
    }
}

impl fmt::Debug for ProviderQuotaPoll {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderQuotaPoll")
            .field("quota_count", &self.quota.len())
            .field("has_benefits", &self.benefits.is_some())
            .finish()
    }
}

pub trait ProviderQuotaSource: Send + 'static {
    fn poll(&mut self, observed_at_ms: i64) -> Result<ProviderQuotaPoll, ProviderPollErrorCode>;
}

impl<T> ProviderQuotaSource for Box<T>
where
    T: ProviderQuotaSource + ?Sized,
{
    fn poll(&mut self, observed_at_ms: i64) -> Result<ProviderQuotaPoll, ProviderPollErrorCode> {
        (**self).poll(observed_at_ms)
    }
}

pub struct CodexQuotaSource {
    config: CodexQuotaRuntimeConfig,
}

impl CodexQuotaSource {
    #[must_use]
    pub const fn new(config: CodexQuotaRuntimeConfig) -> Self {
        Self { config }
    }
}

impl ProviderQuotaSource for CodexQuotaSource {
    fn poll(&mut self, observed_at_ms: i64) -> Result<ProviderQuotaPoll, ProviderPollErrorCode> {
        let command = self
            .config
            .resolve_current_command()
            .map_err(|error| map_discovery_error(error.code()))?;
        let transport = CodexQuotaTransport::new(command, self.config.transport_timeout())
            .map_err(|error| map_codex_error(error.code()))?;
        transport
            .poll(observed_at_ms)
            .map(|snapshot| ProviderQuotaPoll::from_codex(observed_at_ms, snapshot))
            .map_err(|error| map_codex_error(error.code()))
    }
}

const fn map_discovery_error(error: CodexExecutableDiscoveryErrorCode) -> ProviderPollErrorCode {
    match error {
        CodexExecutableDiscoveryErrorCode::Unavailable => {
            ProviderPollErrorCode::DiscoveryUnavailable
        }
        CodexExecutableDiscoveryErrorCode::CapacityExceeded => {
            ProviderPollErrorCode::DiscoveryCapacityExceeded
        }
    }
}

const fn map_codex_error(error: CodexQuotaErrorCode) -> ProviderPollErrorCode {
    match error {
        CodexQuotaErrorCode::DeadlineExceeded => ProviderPollErrorCode::DeadlineExceeded,
        CodexQuotaErrorCode::CapacityExceeded => ProviderPollErrorCode::CapacityExceeded,
        CodexQuotaErrorCode::Unavailable => ProviderPollErrorCode::Unavailable,
        CodexQuotaErrorCode::SpawnFailed => ProviderPollErrorCode::SpawnFailed,
        CodexQuotaErrorCode::ProcessExited => ProviderPollErrorCode::ProcessExited,
        CodexQuotaErrorCode::ProcessCleanupFailed => ProviderPollErrorCode::ProcessCleanupFailed,
        CodexQuotaErrorCode::InvalidData => ProviderPollErrorCode::InvalidData,
        CodexQuotaErrorCode::AccountIdentityUnavailable => {
            ProviderPollErrorCode::AccountIdentityUnavailable
        }
        CodexQuotaErrorCode::InvalidTime => ProviderPollErrorCode::InvalidTime,
        CodexQuotaErrorCode::InvalidCommand => ProviderPollErrorCode::InvalidCommand,
        CodexQuotaErrorCode::ProtocolError => ProviderPollErrorCode::ProtocolError,
        CodexQuotaErrorCode::UnsupportedVersion => ProviderPollErrorCode::UnsupportedVersion,
        CodexQuotaErrorCode::RpcError => ProviderPollErrorCode::RpcError,
    }
}
