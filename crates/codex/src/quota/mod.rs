mod normalize;
mod transport;
mod wire;

use std::fmt;

use tokenmaster_domain::{
    BenefitInventoryObservation, QuotaAccountId, QuotaSample, QuotaWindowDefinition,
};

pub const CODEX_QUOTA_FRESH_MILLIS: i64 = 20 * 60 * 1_000;
pub const CODEX_QUOTA_STALE_MILLIS: i64 = 2 * 60 * 60 * 1_000;
pub const MAX_CODEX_QUOTA_JSON_BYTES: usize = 256 * 1024;
pub const MAX_CODEX_QUOTA_WINDOWS: usize = 32;
pub const MAX_CODEX_RESET_CREDIT_DETAILS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexQuotaErrorCode {
    InvalidData,
    CapacityExceeded,
    AccountIdentityUnavailable,
    InvalidTime,
    Unavailable,
    InvalidCommand,
    SpawnFailed,
    DeadlineExceeded,
    ProtocolError,
    UnsupportedVersion,
    RpcError,
    ProcessExited,
    ProcessCleanupFailed,
}

impl fmt::Display for CodexQuotaErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidData => "invalid_data",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::AccountIdentityUnavailable => "account_identity_unavailable",
            Self::InvalidTime => "invalid_time",
            Self::Unavailable => "unavailable",
            Self::InvalidCommand => "invalid_command",
            Self::SpawnFailed => "spawn_failed",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::ProtocolError => "protocol_error",
            Self::UnsupportedVersion => "unsupported_version",
            Self::RpcError => "rpc_error",
            Self::ProcessExited => "process_exited",
            Self::ProcessCleanupFailed => "process_cleanup_failed",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodexQuotaError {
    code: CodexQuotaErrorCode,
    limit: Option<usize>,
}

impl CodexQuotaError {
    pub(super) const fn new(code: CodexQuotaErrorCode) -> Self {
        Self { code, limit: None }
    }

    pub(super) const fn with_limit(code: CodexQuotaErrorCode, limit: usize) -> Self {
        Self {
            code,
            limit: Some(limit),
        }
    }

    #[must_use]
    pub const fn code(self) -> CodexQuotaErrorCode {
        self.code
    }

    #[must_use]
    pub const fn limit(self) -> Option<usize> {
        self.limit
    }
}

impl fmt::Display for CodexQuotaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Codex quota error: {}", self.code)
    }
}

impl std::error::Error for CodexQuotaError {}

#[derive(Clone, Eq, PartialEq)]
pub struct CodexQuotaObservation {
    definition: QuotaWindowDefinition,
    sample: QuotaSample,
    display_label: Option<Box<str>>,
}

impl CodexQuotaObservation {
    pub(super) fn new(
        definition: QuotaWindowDefinition,
        sample: QuotaSample,
        display_label: Option<Box<str>>,
    ) -> Self {
        Self {
            definition,
            sample,
            display_label,
        }
    }

    #[must_use]
    pub const fn definition(&self) -> &QuotaWindowDefinition {
        &self.definition
    }

    #[must_use]
    pub const fn sample(&self) -> &QuotaSample {
        &self.sample
    }

    #[must_use]
    pub fn display_label(&self) -> Option<&str> {
        self.display_label.as_deref()
    }
}

impl fmt::Debug for CodexQuotaObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexQuotaObservation")
            .field("definition", &"[redacted]")
            .field("sample", &"[redacted]")
            .field("has_display_label", &self.display_label.is_some())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CodexQuotaSnapshot {
    account_id: QuotaAccountId,
    observations: Box<[CodexQuotaObservation]>,
    benefit_observation: Option<BenefitInventoryObservation>,
}

impl CodexQuotaSnapshot {
    pub(super) fn new(
        account_id: QuotaAccountId,
        observations: Vec<CodexQuotaObservation>,
        benefit_observation: Option<BenefitInventoryObservation>,
    ) -> Self {
        Self {
            account_id,
            observations: observations.into_boxed_slice(),
            benefit_observation,
        }
    }

    #[must_use]
    pub const fn account_id(&self) -> &QuotaAccountId {
        &self.account_id
    }

    #[must_use]
    pub const fn observations(&self) -> &[CodexQuotaObservation] {
        &self.observations
    }

    #[must_use]
    pub const fn benefit_observation(&self) -> Option<&BenefitInventoryObservation> {
        self.benefit_observation.as_ref()
    }
}

impl fmt::Debug for CodexQuotaSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexQuotaSnapshot")
            .field("account_id", &"[redacted]")
            .field("observation_count", &self.observations.len())
            .field(
                "has_benefit_observation",
                &self.benefit_observation.is_some(),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CodexQuotaNormalizer;

impl CodexQuotaNormalizer {
    pub fn normalize(
        account_json: &[u8],
        quota_json: &[u8],
        observed_at_ms: i64,
    ) -> Result<CodexQuotaSnapshot, CodexQuotaError> {
        normalize::normalize_json(account_json, quota_json, observed_at_ms)
    }
}

pub use transport::{
    CodexAppServerCommand, CodexQuotaTransport, MAX_CODEX_APP_SERVER_FRAME_BYTES,
    MAX_CODEX_APP_SERVER_FRAMES, MAX_CODEX_APP_SERVER_STDOUT_BYTES, MAX_CODEX_APP_SERVER_TIMEOUT,
    SUPPORTED_CODEX_APP_SERVER_VERSION,
};
