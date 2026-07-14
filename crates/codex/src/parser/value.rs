use std::borrow::Cow;
use std::fmt;

use chrono::{DateTime, Datelike, Utc};
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};
use tokenmaster_domain::{MetadataValue, ModelKey, TokenCount, UtcTimestamp};

use super::{ParserDiagnosticCode, ParserDiagnostics};
use crate::identity::push_hex;

pub(crate) const MAX_TIMESTAMP_BYTES: usize = 64;
pub(crate) const MAX_RAW_MODEL_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum LenientU64 {
    #[default]
    Missing,
    Valid(u64),
    Invalid,
}

impl<'de> Deserialize<'de> for LenientU64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(LenientU64Visitor)
    }
}

struct LenientU64Visitor;

impl<'de> Visitor<'de> for LenientU64Visitor {
    type Value = LenientU64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an unsigned integer or bounded decimal string")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(LenientU64::Valid(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(u64::try_from(value).map_or(LenientU64::Invalid, LenientU64::Valid))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(LenientU64::Invalid)
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(LenientU64::Invalid)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(parse_decimal(value))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok(parse_decimal(value))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(parse_decimal(&value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(LenientU64::Invalid)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(LenientU64::Invalid)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(LenientU64::Invalid)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(LenientU64::Invalid)
    }
}

fn parse_decimal(value: &str) -> LenientU64 {
    let value = value.trim();
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return LenientU64::Invalid;
    }
    value
        .parse::<u64>()
        .map_or(LenientU64::Invalid, LenientU64::Valid)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum BoundedText<'a, const MAX_BYTES: usize> {
    #[default]
    Missing,
    Valid(Cow<'a, str>),
    Invalid,
}

impl<const MAX_BYTES: usize> BoundedText<'_, MAX_BYTES> {
    pub(crate) fn value(&self) -> Option<&str> {
        match self {
            Self::Valid(value) => Some(value),
            Self::Missing | Self::Invalid => None,
        }
    }

    pub(crate) const fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid)
    }
}

impl<'de: 'a, 'a, const MAX_BYTES: usize> Deserialize<'de> for BoundedText<'a, MAX_BYTES> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(BoundedTextVisitor::<MAX_BYTES>(std::marker::PhantomData))
    }
}

struct BoundedTextVisitor<'a, const MAX_BYTES: usize>(std::marker::PhantomData<&'a ()>);

impl<'de: 'a, 'a, const MAX_BYTES: usize> Visitor<'de> for BoundedTextVisitor<'a, MAX_BYTES> {
    type Value = BoundedText<'a, MAX_BYTES>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded text value")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        let value: &'a str = value;
        Ok(classify_borrowed_text(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(classify_owned_text(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(classify_owned_text(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(BoundedText::Invalid)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(BoundedText::Invalid)
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(BoundedText::Invalid)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(BoundedText::Invalid)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(BoundedText::Invalid)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(BoundedText::Invalid)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(BoundedText::Invalid)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(BoundedText::Invalid)
    }
}

fn classify_borrowed_text<const MAX_BYTES: usize>(value: &str) -> BoundedText<'_, MAX_BYTES> {
    if valid_bounded_text::<MAX_BYTES>(value) {
        BoundedText::Valid(Cow::Borrowed(value))
    } else {
        BoundedText::Invalid
    }
}

fn classify_owned_text<const MAX_BYTES: usize>(value: String) -> BoundedText<'static, MAX_BYTES> {
    if valid_bounded_text::<MAX_BYTES>(&value) {
        BoundedText::Valid(Cow::Owned(value))
    } else {
        BoundedText::Invalid
    }
}

fn valid_bounded_text<const MAX_BYTES: usize>(value: &str) -> bool {
    value.len() <= MAX_BYTES && !value.chars().any(char::is_control)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum TimestampScalar<'a> {
    #[default]
    Missing,
    Text(Cow<'a, str>),
    Signed(i64),
    Unsigned(u64),
    Invalid,
}

impl<'de: 'a, 'a> Deserialize<'de> for TimestampScalar<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TimestampVisitor(std::marker::PhantomData))
    }
}

struct TimestampVisitor<'a>(std::marker::PhantomData<&'a ()>);

impl<'de: 'a, 'a> Visitor<'de> for TimestampVisitor<'a> {
    type Value = TimestampScalar<'a>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded RFC3339 text or an integral Unix timestamp")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        let value: &'a str = value;
        if valid_timestamp_text(value) {
            Ok(TimestampScalar::Text(Cow::Borrowed(value)))
        } else {
            Ok(TimestampScalar::Invalid)
        }
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        if valid_timestamp_text(value) {
            Ok(TimestampScalar::Text(Cow::Owned(value.to_owned())))
        } else {
            Ok(TimestampScalar::Invalid)
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        if valid_timestamp_text(&value) {
            Ok(TimestampScalar::Text(Cow::Owned(value)))
        } else {
            Ok(TimestampScalar::Invalid)
        }
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(TimestampScalar::Signed(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(TimestampScalar::Unsigned(value))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(TimestampScalar::Invalid)
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(TimestampScalar::Invalid)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(TimestampScalar::Invalid)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(TimestampScalar::Invalid)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(TimestampScalar::Invalid)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(TimestampScalar::Invalid)
    }
}

fn valid_timestamp_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TIMESTAMP_BYTES && !value.chars().any(char::is_control)
}

pub(crate) fn timestamp_value(value: &TimestampScalar<'_>) -> Option<UtcTimestamp> {
    match value {
        TimestampScalar::Text(text) => {
            let parsed = DateTime::parse_from_rfc3339(text).ok()?.with_timezone(&Utc);
            normalized_timestamp(parsed)
        }
        TimestampScalar::Signed(number) => numeric_timestamp(*number),
        TimestampScalar::Unsigned(number) => {
            i64::try_from(*number).ok().and_then(numeric_timestamp)
        }
        TimestampScalar::Missing | TimestampScalar::Invalid => None,
    }
}

fn numeric_timestamp(number: i64) -> Option<UtcTimestamp> {
    let (seconds, nanos) = if number > 1_000_000_000_000 {
        (
            number / 1_000,
            u32::try_from(number % 1_000)
                .ok()?
                .saturating_mul(1_000_000),
        )
    } else {
        (number, 0)
    };
    normalized_timestamp(DateTime::<Utc>::from_timestamp(seconds, nanos)?)
}

fn normalized_timestamp(value: DateTime<Utc>) -> Option<UtcTimestamp> {
    if !(0..=9_999).contains(&value.year()) {
        return None;
    }
    UtcTimestamp::new(value.timestamp(), value.timestamp_subsec_nanos()).ok()
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedModel {
    pub(crate) key: ModelKey,
    pub(crate) raw: Option<MetadataValue>,
    pub(crate) fallback: bool,
}

pub(crate) fn resolve_model(
    explicit: Option<&str>,
    current: Option<&ResolvedModel>,
    invalid_aliases: usize,
    diagnostics: &mut ParserDiagnostics,
) -> Option<ResolvedModel> {
    for _ in 0..invalid_aliases {
        diagnostics.record(ParserDiagnosticCode::InvalidModel);
    }
    if let Some(value) = explicit {
        if let Some(resolved) = normalize_explicit_model(value) {
            return Some(resolved);
        }
        diagnostics.record(ParserDiagnosticCode::InvalidModel);
    }
    if let Some(value) = current {
        if value.fallback {
            diagnostics.record(ParserDiagnosticCode::ModelFallback);
        }
        return Some(value.clone());
    }
    diagnostics.record(ParserDiagnosticCode::ModelFallback);
    ModelKey::new("unknown").ok().map(|key| ResolvedModel {
        key,
        raw: None,
        fallback: true,
    })
}

pub(crate) fn normalize_explicit_model(value: &str) -> Option<ResolvedModel> {
    let raw = MetadataValue::new(value.to_owned()).ok()?;
    if raw.as_str().len() > MAX_RAW_MODEL_BYTES {
        return None;
    }
    if let Ok(key) = ModelKey::new(raw.as_str().to_owned()) {
        return Some(ResolvedModel {
            key,
            raw: Some(raw),
            fallback: false,
        });
    }

    let digest = Sha256::digest(raw.as_str().as_bytes());
    let mut key = String::with_capacity(40);
    key.push_str("unknown_");
    push_hex(&mut key, &digest[..16]);
    ModelKey::new(key).ok().map(|key| ResolvedModel {
        key,
        raw: Some(raw),
        fallback: false,
    })
}

pub(crate) fn first_token(
    values: &[LenientU64],
    diagnostics: &mut ParserDiagnostics,
) -> TokenCount {
    for value in values {
        match value {
            LenientU64::Missing => {}
            LenientU64::Valid(value) => return TokenCount::Available(*value),
            LenientU64::Invalid => diagnostics.record(ParserDiagnosticCode::InvalidToken),
        }
    }
    TokenCount::Unavailable
}
