use serde::Deserialize;

use super::value::{BoundedText, LenientU64, MAX_RAW_MODEL_BYTES, TimestampScalar};

pub(crate) type RawText<'a> = BoundedText<'a, MAX_RAW_MODEL_BYTES>;
pub(crate) type RawDisplayText<'a> = BoundedText<'a, 4096>;
pub(crate) type RawPathText<'a> = BoundedText<'a, 4096>;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawUsage {
    #[serde(default)]
    pub(crate) input_tokens: LenientU64,
    #[serde(default)]
    pub(crate) prompt_tokens: LenientU64,
    #[serde(default)]
    pub(crate) input: LenientU64,
    #[serde(default)]
    pub(crate) cached_input_tokens: LenientU64,
    #[serde(default)]
    pub(crate) cache_read_input_tokens: LenientU64,
    #[serde(default)]
    pub(crate) cached_tokens: LenientU64,
    #[serde(default)]
    pub(crate) output_tokens: LenientU64,
    #[serde(default)]
    pub(crate) completion_tokens: LenientU64,
    #[serde(default)]
    pub(crate) output: LenientU64,
    #[serde(default)]
    pub(crate) reasoning_output_tokens: LenientU64,
    #[serde(default)]
    pub(crate) reasoning_tokens: LenientU64,
    #[serde(default)]
    pub(crate) total_tokens: LenientU64,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawInfo<'a> {
    #[serde(default)]
    pub(crate) last_token_usage: Option<RawUsage>,
    #[serde(default)]
    pub(crate) total_token_usage: Option<RawUsage>,
    #[serde(borrow, default)]
    pub(crate) model: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) model_name: RawText<'a>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawPayload<'a> {
    #[serde(rename = "type", borrow, default)]
    pub(crate) kind: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) model: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) model_name: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) model_slug: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) model_id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) requested_model: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) default_model: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) service_tier: RawDisplayText<'a>,
    #[serde(default)]
    pub(crate) model_context_window: LenientU64,
    #[serde(borrow, default)]
    pub(crate) name: RawDisplayText<'a>,
    #[serde(borrow, default)]
    pub(crate) cwd: RawPathText<'a>,
    #[serde(borrow, default)]
    pub(crate) originator: RawDisplayText<'a>,
    #[serde(borrow, default)]
    pub(crate) source: RawDisplayText<'a>,
    #[serde(borrow, default)]
    pub(crate) session_id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) git: RawGit<'a>,
    #[serde(borrow, default)]
    pub(crate) info: Option<RawInfo<'a>>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawGit<'a> {
    #[serde(borrow, default)]
    pub(crate) branch: RawDisplayText<'a>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawResult<'a> {
    #[serde(borrow, default)]
    pub(crate) timestamp: TimestampScalar<'a>,
    #[serde(rename = "created_at", borrow, default)]
    pub(crate) created_at: TimestampScalar<'a>,
    #[serde(rename = "createdAt", borrow, default)]
    pub(crate) created_at_v2: TimestampScalar<'a>,
    #[serde(default)]
    pub(crate) usage: Option<RawUsage>,
    #[serde(borrow, default)]
    pub(crate) model: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) model_name: RawText<'a>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawLine<'a> {
    #[serde(rename = "type", borrow, default)]
    pub(crate) kind: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) timestamp: TimestampScalar<'a>,
    #[serde(rename = "created_at", borrow, default)]
    pub(crate) created_at: TimestampScalar<'a>,
    #[serde(rename = "createdAt", borrow, default)]
    pub(crate) created_at_v2: TimestampScalar<'a>,
    #[serde(borrow, default)]
    pub(crate) payload: Option<RawPayload<'a>>,
    #[serde(default)]
    pub(crate) usage: Option<RawUsage>,
    #[serde(borrow, default)]
    pub(crate) model: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) model_name: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) data: Option<RawResult<'a>>,
    #[serde(borrow, default)]
    pub(crate) result: Option<RawResult<'a>>,
    #[serde(borrow, default)]
    pub(crate) response: Option<RawResult<'a>>,
}
