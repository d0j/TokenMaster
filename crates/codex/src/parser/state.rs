use std::fmt;

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use tokenmaster_domain::{
    ActivityCounts, ActivityKind, MetadataValue, ProjectAlias, TokenCount, TokenUsage,
    UsageSessionId,
};

use super::value::{ResolvedModel, normalize_explicit_model};
use super::{ParserDiagnosticCode, ParserDiagnostics};

pub const PARSER_SCHEMA_VERSION: u16 = 2;
pub const MAX_TOOL_NAMES: usize = 64;
pub const MAX_TOOL_NAME_BYTES: usize = 80;
pub(crate) const MAX_CONTEXT_WINDOW_TOKENS: u64 = 10_000_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolCountEntry {
    name: Box<str>,
    count: u64,
}

impl ToolCountEntry {
    fn new(name: Box<str>) -> Self {
        Self { name, count: 1 }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCountEntryWire {
    name: BoundedToolName,
    count: u64,
}

struct BoundedToolName(Box<str>);

impl<'de> Deserialize<'de> for BoundedToolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BoundedToolNameVisitor)
    }
}

struct BoundedToolNameVisitor;

impl Visitor<'_> for BoundedToolNameVisitor {
    type Value = BoundedToolName;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded tool name")
    }

    fn visit_borrowed_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        bounded_tool_name(value)
            .map(BoundedToolName)
            .ok_or_else(|| {
                E::invalid_value(
                    serde::de::Unexpected::Str(value),
                    &"a valid bounded tool name",
                )
            })
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_borrowed_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if valid_tool_name(&value) {
            Ok(BoundedToolName(value.into_boxed_str()))
        } else {
            Err(E::invalid_value(
                serde::de::Unexpected::Str(&value),
                &"a valid bounded tool name",
            ))
        }
    }
}

impl<'de> Deserialize<'de> for ToolCountEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ToolCountEntryWire::deserialize(deserializer)?;
        Ok(Self {
            name: wire.name.0,
            count: wire.count,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResumeModel {
    key: tokenmaster_domain::ModelKey,
    raw: Option<MetadataValue>,
    fallback: bool,
}

impl From<&ResolvedModel> for ResumeModel {
    fn from(value: &ResolvedModel) -> Self {
        Self {
            key: value.key.clone(),
            raw: value.raw.clone(),
            fallback: value.fallback,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParserResumeState {
    version: u16,
    current_model: Option<ResumeModel>,
    previous_totals: Option<TokenUsage>,
    service_tier: Option<MetadataValue>,
    session_id: Option<UsageSessionId>,
    #[serde(default)]
    parent_session_id: Option<UsageSessionId>,
    #[serde(default)]
    lineage_conflict: bool,
    #[serde(default)]
    next_usage_ordinal: u64,
    project: Option<ProjectAlias>,
    originator: Option<MetadataValue>,
    source_alias: Option<MetadataValue>,
    git_branch: Option<MetadataValue>,
    context_window: Option<u64>,
    pending_activity: ActivityCounts,
    aggregate_activity: ActivityCounts,
    #[serde(deserialize_with = "deserialize_tool_counts")]
    tool_counts: Vec<ToolCountEntry>,
    other_tools: u64,
}

fn deserialize_tool_counts<'de, D>(deserializer: D) -> Result<Vec<ToolCountEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_seq(BoundedToolCountsVisitor)
}

struct BoundedToolCountsVisitor;

impl<'de> Visitor<'de> for BoundedToolCountsVisitor {
    type Value = Vec<ToolCountEntry>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("at most 64 sorted tool counters")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if sequence
            .size_hint()
            .is_some_and(|size| size > MAX_TOOL_NAMES)
        {
            return Err(serde::de::Error::invalid_length(MAX_TOOL_NAMES + 1, &self));
        }
        let mut entries = Vec::with_capacity(MAX_TOOL_NAMES);
        while entries.len() < MAX_TOOL_NAMES {
            let Some(entry) = sequence.next_element()? else {
                return Ok(entries);
            };
            entries.push(entry);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(serde::de::Error::invalid_length(MAX_TOOL_NAMES + 1, &self));
        }
        Ok(entries)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserResumeErrorCode {
    UnsupportedVersion,
    InvalidState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParserResumeError {
    code: ParserResumeErrorCode,
}

impl ParserResumeError {
    const fn new(code: ParserResumeErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(&self) -> ParserResumeErrorCode {
        self.code
    }
}

impl fmt::Display for ParserResumeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            ParserResumeErrorCode::UnsupportedVersion => {
                formatter.write_str("unsupported parser resume version")
            }
            ParserResumeErrorCode::InvalidState => {
                formatter.write_str("invalid parser resume state")
            }
        }
    }
}

impl std::error::Error for ParserResumeError {}

#[derive(Clone, Debug)]
pub struct ParserState {
    pub(super) current_model: Option<ResolvedModel>,
    pub(super) previous_totals: Option<TokenUsage>,
    pub(super) service_tier: Option<MetadataValue>,
    pub(super) session_id: Option<UsageSessionId>,
    pub(super) parent_session_id: Option<UsageSessionId>,
    pub(super) lineage_conflict: bool,
    pub(super) next_usage_ordinal: u64,
    pub(super) project: Option<ProjectAlias>,
    pub(super) originator: Option<MetadataValue>,
    pub(super) source_alias: Option<MetadataValue>,
    pub(super) git_branch: Option<MetadataValue>,
    pub(super) context_window: Option<u64>,
    pub(super) pending_activity: ActivityCounts,
    pub(super) aggregate_activity: ActivityCounts,
    tool_counts: Vec<ToolCountEntry>,
    other_tools: u64,
}

impl Default for ParserState {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserState {
    pub const MAX_RETAINED_TEXT_BYTES: usize = 9_280;

    #[must_use]
    pub fn new() -> Self {
        Self {
            current_model: None,
            previous_totals: None,
            service_tier: None,
            session_id: None,
            parent_session_id: None,
            lineage_conflict: false,
            next_usage_ordinal: 0,
            project: None,
            originator: None,
            source_alias: None,
            git_branch: None,
            context_window: None,
            pending_activity: ActivityCounts::default(),
            aggregate_activity: ActivityCounts::default(),
            tool_counts: Vec::with_capacity(MAX_TOOL_NAMES),
            other_tools: 0,
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> ParserResumeState {
        ParserResumeState {
            version: PARSER_SCHEMA_VERSION,
            current_model: self.current_model.as_ref().map(ResumeModel::from),
            previous_totals: self.previous_totals,
            service_tier: self.service_tier.clone(),
            session_id: self.session_id.clone(),
            parent_session_id: self.parent_session_id.clone(),
            lineage_conflict: self.lineage_conflict,
            next_usage_ordinal: self.next_usage_ordinal,
            project: self.project.clone(),
            originator: self.originator.clone(),
            source_alias: self.source_alias.clone(),
            git_branch: self.git_branch.clone(),
            context_window: self.context_window,
            pending_activity: self.pending_activity,
            aggregate_activity: self.aggregate_activity,
            tool_counts: self.tool_counts.clone(),
            other_tools: self.other_tools,
        }
    }

    pub fn from_resume(value: ParserResumeState) -> Result<Self, ParserResumeError> {
        if value.version != PARSER_SCHEMA_VERSION {
            return Err(ParserResumeError::new(
                ParserResumeErrorCode::UnsupportedVersion,
            ));
        }
        if value
            .context_window
            .is_some_and(|window| !(1..=MAX_CONTEXT_WINDOW_TOKENS).contains(&window))
            || value
                .previous_totals
                .as_ref()
                .is_some_and(|usage| !valid_previous_totals(usage))
            || !valid_tool_entries(&value.tool_counts)
            || (value.other_tools > 0 && value.tool_counts.len() < MAX_TOOL_NAMES)
            || !valid_activity_relation(&value.pending_activity, &value.aggregate_activity)
            || value.next_usage_ordinal > i64::MAX as u64
            || (!value.lineage_conflict
                && value.parent_session_id.is_some()
                && value.parent_session_id.as_ref() == value.session_id.as_ref())
        {
            return Err(ParserResumeError::new(ParserResumeErrorCode::InvalidState));
        }
        let current_model = value
            .current_model
            .map(resolved_model_from_resume)
            .transpose()?;
        let mut tool_counts = Vec::with_capacity(MAX_TOOL_NAMES);
        tool_counts.extend(value.tool_counts);
        let state = Self {
            current_model,
            previous_totals: value.previous_totals,
            service_tier: value.service_tier,
            session_id: value.session_id,
            parent_session_id: value.parent_session_id,
            lineage_conflict: value.lineage_conflict,
            next_usage_ordinal: value.next_usage_ordinal,
            project: value.project,
            originator: value.originator,
            source_alias: value.source_alias,
            git_branch: value.git_branch,
            context_window: value.context_window,
            pending_activity: value.pending_activity,
            aggregate_activity: value.aggregate_activity,
            tool_counts,
            other_tools: value.other_tools,
        };
        if state.retained_text_bytes() > Self::MAX_RETAINED_TEXT_BYTES {
            return Err(ParserResumeError::new(ParserResumeErrorCode::InvalidState));
        }
        Ok(state)
    }

    pub(super) fn record_tool(
        &mut self,
        name: &str,
        activity: ActivityKind,
        diagnostics: &mut ParserDiagnostics,
    ) {
        self.pending_activity.increment(activity);
        self.aggregate_activity.increment(activity);
        match self
            .tool_counts
            .binary_search_by(|entry| entry.name.as_ref().cmp(name.as_ref()))
        {
            Ok(index) => {
                self.tool_counts[index].count = self.tool_counts[index].count.saturating_add(1);
            }
            Err(index) if self.tool_counts.len() < MAX_TOOL_NAMES => {
                self.tool_counts
                    .insert(index, ToolCountEntry::new(name.to_owned().into_boxed_str()));
            }
            Err(_) => {
                self.other_tools = self.other_tools.saturating_add(1);
                diagnostics.record(ParserDiagnosticCode::ToolCapacity);
            }
        }
    }

    #[must_use]
    pub fn tool_count_len(&self) -> usize {
        self.tool_counts.len()
    }

    #[must_use]
    pub fn tool_counts(&self) -> &[ToolCountEntry] {
        &self.tool_counts
    }

    #[must_use]
    pub const fn other_tools(&self) -> u64 {
        self.other_tools
    }

    #[must_use]
    pub const fn pending_activity(&self) -> &ActivityCounts {
        &self.pending_activity
    }

    #[must_use]
    pub const fn aggregate_activity(&self) -> &ActivityCounts {
        &self.aggregate_activity
    }

    #[must_use]
    pub const fn context_window(&self) -> Option<u64> {
        self.context_window
    }

    #[must_use]
    pub fn source_alias(&self) -> Option<&str> {
        self.source_alias.as_ref().map(MetadataValue::as_str)
    }

    #[must_use]
    pub fn git_branch(&self) -> Option<&str> {
        self.git_branch.as_ref().map(MetadataValue::as_str)
    }

    #[must_use]
    pub fn parent_session_id(&self) -> Option<&UsageSessionId> {
        self.parent_session_id.as_ref()
    }

    #[must_use]
    pub const fn lineage_conflict(&self) -> bool {
        self.lineage_conflict
    }

    #[must_use]
    pub const fn next_usage_ordinal(&self) -> u64 {
        self.next_usage_ordinal
    }

    #[must_use]
    pub fn retained_text_bytes(&self) -> usize {
        let model_bytes = self.current_model.as_ref().map_or(0, |model| {
            model.key.as_str().len() + model.raw.as_ref().map_or(0, |value| value.as_str().len())
        });
        model_bytes
            + metadata_len(self.service_tier.as_ref())
            + self
                .session_id
                .as_ref()
                .map_or(0, |value| value.as_str().len())
            + self
                .parent_session_id
                .as_ref()
                .map_or(0, |value| value.as_str().len())
            + self
                .project
                .as_ref()
                .map_or(0, |value| value.as_str().len())
            + metadata_len(self.originator.as_ref())
            + metadata_len(self.source_alias.as_ref())
            + metadata_len(self.git_branch.as_ref())
            + self
                .tool_counts
                .iter()
                .map(|entry| entry.name.len())
                .sum::<usize>()
    }
}

fn metadata_len(value: Option<&MetadataValue>) -> usize {
    value.map_or(0, |value| value.as_str().len())
}

fn valid_tool_entries(entries: &[ToolCountEntry]) -> bool {
    entries.len() <= MAX_TOOL_NAMES
        && entries
            .iter()
            .all(|entry| valid_tool_name(&entry.name) && entry.count > 0)
        && entries
            .windows(2)
            .all(|pair| pair[0].name.as_ref() < pair[1].name.as_ref())
}

fn valid_previous_totals(usage: &TokenUsage) -> bool {
    match (usage.input(), usage.cached()) {
        (TokenCount::Available(input), TokenCount::Available(cached)) => cached <= input,
        (_, TokenCount::Unavailable) => true,
        (TokenCount::Unavailable, TokenCount::Available(_)) => false,
    }
}

fn valid_activity_relation(pending: &ActivityCounts, aggregate: &ActivityCounts) -> bool {
    pending
        .as_array()
        .iter()
        .zip(aggregate.as_array())
        .all(|(pending, aggregate)| pending <= aggregate)
}

fn valid_tool_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TOOL_NAME_BYTES && !value.chars().any(char::is_control)
}

fn bounded_tool_name(value: &str) -> Option<Box<str>> {
    valid_tool_name(value).then(|| value.to_owned().into_boxed_str())
}

fn resolved_model_from_resume(value: ResumeModel) -> Result<ResolvedModel, ParserResumeError> {
    if value.fallback {
        if value.key.as_str() == "unknown" && value.raw.is_none() {
            return Ok(ResolvedModel {
                key: value.key,
                raw: None,
                fallback: true,
            });
        }
        return Err(ParserResumeError::new(ParserResumeErrorCode::InvalidState));
    }
    let Some(raw) = value.raw.as_ref() else {
        return Err(ParserResumeError::new(ParserResumeErrorCode::InvalidState));
    };
    let Some(normalized) = normalize_explicit_model(raw.as_str()) else {
        return Err(ParserResumeError::new(ParserResumeErrorCode::InvalidState));
    };
    if normalized.key != value.key || normalized.fallback {
        return Err(ParserResumeError::new(ParserResumeErrorCode::InvalidState));
    }
    Ok(ResolvedModel {
        key: value.key,
        raw: value.raw,
        fallback: false,
    })
}

#[cfg(test)]
mod tests {
    use super::{MAX_TOOL_NAMES, ParserState};

    #[test]
    fn working_tool_storage_is_preallocated_to_the_exact_bound() {
        let state = ParserState::new();
        assert_eq!(state.tool_counts.capacity(), MAX_TOOL_NAMES);
    }
}
