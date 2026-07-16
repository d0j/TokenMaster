mod normalize;
mod wire;

use std::fmt;

use tokenmaster_domain::{QuotaAccountId, QuotaSample, QuotaWindowDefinition};

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
}

impl fmt::Display for CodexQuotaErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidData => "invalid_data",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::AccountIdentityUnavailable => "account_identity_unavailable",
            Self::InvalidTime => "invalid_time",
            Self::Unavailable => "unavailable",
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
}

impl CodexQuotaSnapshot {
    pub(super) fn new(
        account_id: QuotaAccountId,
        observations: Vec<CodexQuotaObservation>,
    ) -> Self {
        Self {
            account_id,
            observations: observations.into_boxed_slice(),
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
}

impl fmt::Debug for CodexQuotaSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexQuotaSnapshot")
            .field("account_id", &"[redacted]")
            .field("observation_count", &self.observations.len())
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
