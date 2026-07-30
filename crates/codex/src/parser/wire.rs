use std::fmt;

use serde::de::{IgnoredAny, MapAccess, Visitor, value::MapAccessDeserializer};
use serde::{Deserialize, Deserializer};

use super::value::{BoundedText, LenientU64, MAX_RAW_MODEL_BYTES, TimestampScalar};

pub(crate) type RawText<'a> = BoundedText<'a, MAX_RAW_MODEL_BYTES>;
pub(crate) type RawDisplayText<'a> = BoundedText<'a, 4096>;
pub(crate) type RawPathText<'a> = BoundedText<'a, 4096>;

#[derive(Debug, Default)]
pub(crate) struct RawSource<'a> {
    display: RawDisplayText<'a>,
    structured_parent_thread_id: RawText<'a>,
}

impl RawSource<'_> {
    pub(crate) const fn display(&self) -> &RawDisplayText<'_> {
        &self.display
    }

    pub(crate) const fn structured_parent_thread_id(&self) -> &RawText<'_> {
        &self.structured_parent_thread_id
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawSourceObject<'a> {
    #[serde(borrow, default)]
    subagent: RawSubagent<'a>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSubagent<'a> {
    #[serde(borrow, default)]
    thread_spawn: RawThreadSpawn<'a>,
}

#[derive(Debug, Default, Deserialize)]
struct RawThreadSpawn<'a> {
    #[serde(borrow, default)]
    parent_thread_id: RawText<'a>,
}

impl<'de: 'a, 'a> Deserialize<'de> for RawSource<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(RawSourceVisitor(std::marker::PhantomData))
    }
}

struct RawSourceVisitor<'a>(std::marker::PhantomData<&'a ()>);

impl<'de: 'a, 'a> Visitor<'de> for RawSourceVisitor<'a> {
    type Value = RawSource<'a>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded source label or structured source object")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        let value: &'a str = value;
        Ok(RawSource {
            display: RawDisplayText::from_borrowed(value),
            structured_parent_thread_id: RawText::Missing,
        })
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(RawSource {
            display: RawDisplayText::from_owned(value.to_owned()),
            structured_parent_thread_id: RawText::Missing,
        })
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(RawSource {
            display: RawDisplayText::from_owned(value),
            structured_parent_thread_id: RawText::Missing,
        })
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let object = RawSourceObject::deserialize(MapAccessDeserializer::new(map))?;
        Ok(RawSource {
            display: RawDisplayText::Missing,
            structured_parent_thread_id: object.subagent.thread_spawn.parent_thread_id,
        })
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(invalid_source())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(invalid_source())
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(invalid_source())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(invalid_source())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(invalid_source())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(invalid_source())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(invalid_source())
    }
}

fn invalid_source<'a>() -> RawSource<'a> {
    RawSource {
        display: RawDisplayText::Invalid,
        structured_parent_thread_id: RawText::Missing,
    }
}

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
    pub(crate) source: RawSource<'a>,
    #[serde(borrow, default)]
    pub(crate) session_id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) forked_from_id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) parent_thread_id: RawText<'a>,
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
    pub(crate) forked_from_id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) parent_thread_id: RawText<'a>,
    #[serde(borrow, default)]
    pub(crate) data: Option<RawResult<'a>>,
    #[serde(borrow, default)]
    pub(crate) result: Option<RawResult<'a>>,
    #[serde(borrow, default)]
    pub(crate) response: Option<RawResult<'a>>,
}
