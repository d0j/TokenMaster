use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AccountResponseWire {
    pub requires_openai_auth: bool,
    pub account: Option<AccountWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AccountWire {
    #[serde(rename = "type")]
    pub kind: String,
    pub email: Option<String>,
    pub plan_type: Option<PlanTypeWire>,
    pub credential_source: Option<AmazonBedrockCredentialSourceWire>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PlanTypeWire {
    Free,
    Go,
    Plus,
    Pro,
    Prolite,
    Team,
    SelfServeBusinessUsageBased,
    Business,
    EnterpriseCbpUsageBased,
    Enterprise,
    Edu,
    Unknown,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum AmazonBedrockCredentialSourceWire {
    CodexManaged,
    AwsManaged,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RateLimitsResponseWire {
    pub rate_limit_reset_credits: Option<RateLimitResetCreditsSummaryWire>,
    pub rate_limits: RateLimitSnapshotWire,
    pub rate_limits_by_limit_id: Option<BTreeMap<String, RateLimitSnapshotWire>>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RateLimitSnapshotWire {
    pub credits: Option<CreditsSnapshotWire>,
    pub individual_limit: Option<SpendControlLimitSnapshotWire>,
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub plan_type: Option<PlanTypeWire>,
    pub primary: Option<RateLimitWindowWire>,
    pub rate_limit_reached_type: Option<RateLimitReachedTypeWire>,
    pub secondary: Option<RateLimitWindowWire>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CreditsSnapshotWire {
    pub balance: Option<String>,
    pub has_credits: bool,
    pub unlimited: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SpendControlLimitSnapshotWire {
    pub limit: String,
    pub remaining_percent: i64,
    pub resets_at: i64,
    pub used: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RateLimitReachedTypeWire {
    RateLimitReached,
    WorkspaceOwnerCreditsDepleted,
    WorkspaceMemberCreditsDepleted,
    WorkspaceOwnerUsageLimitReached,
    WorkspaceMemberUsageLimitReached,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RateLimitWindowWire {
    pub resets_at: Option<i64>,
    pub used_percent: i64,
    pub window_duration_mins: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RateLimitResetCreditsSummaryWire {
    pub available_count: i64,
    pub credits: Option<Vec<RateLimitResetCreditWire>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RateLimitResetCreditWire {
    pub description: Option<String>,
    pub expires_at: Option<i64>,
    pub granted_at: i64,
    pub id: String,
    pub reset_type: RateLimitResetTypeWire,
    pub status: RateLimitResetCreditStatusWire,
    pub title: Option<String>,
}

#[derive(Clone, Copy, Deserialize)]
pub(super) enum RateLimitResetTypeWire {
    #[serde(rename = "codexRateLimits")]
    CodexRateLimits,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RateLimitResetCreditStatusWire {
    Available,
    Redeeming,
    Redeemed,
    Unknown,
}
