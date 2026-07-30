use serde::Serialize;
use tokenmaster_domain::{
    ActivityCounts, LongContextState, ObservationDraft, ObservationDraftParts,
    ObservationVerification, SessionRelationDraft, SessionRelationDraftParts, TokenCount,
    TokenUsage, UsageProfileId, UsageProviderId, UsageSessionId, UsageSourceId,
};
use tokenmaster_provider::{RepositoryActivityHint, RepositoryActivityHintParts};

mod effects;
mod state;
mod value;
mod wire;

use effects::{apply_metadata, metadata_update, tool_update};
pub use state::{
    MAX_TOOL_NAME_BYTES, MAX_TOOL_NAMES, PARSER_SCHEMA_VERSION, ParserResumeError,
    ParserResumeErrorCode, ParserResumeState, ParserState, ToolCountEntry,
};
use value::{first_token, resolve_model, timestamp_value};
use wire::{RawLine, RawResult, RawText, RawUsage};

pub const MAX_LINE_BYTES: usize = 16 << 20;
pub const LONG_CONTEXT_THRESHOLD: u64 = 272_000;

const RELEVANCE_MARKERS: [&[u8]; 9] = [
    b"token_count",
    b"turn_context",
    b"session_meta",
    b"sessionMeta",
    b"\"usage\"",
    b"function_call",
    b"tool_call",
    b"exec_command",
    b"mcp",
];

#[derive(Clone, Debug)]
pub struct ParseContext {
    provider_id: UsageProviderId,
    profile_id: UsageProfileId,
    source_id: UsageSourceId,
    filename_session_hint: Option<UsageSessionId>,
    hashed_session_hint: UsageSessionId,
    source_verification: ObservationVerification,
}

impl ParseContext {
    #[must_use]
    pub fn new(
        profile_id: UsageProfileId,
        source_id: UsageSourceId,
        filename_session_hint: Option<UsageSessionId>,
        hashed_session_hint: UsageSessionId,
    ) -> Self {
        let provider_id = match UsageProviderId::new("codex") {
            Ok(value) => value,
            Err(_) => unreachable!("the built-in Codex provider ID is valid"),
        };
        Self {
            provider_id,
            profile_id,
            source_id,
            filename_session_hint,
            hashed_session_hint,
            source_verification: ObservationVerification::Incremental,
        }
    }

    #[must_use]
    pub fn with_source_verification(mut self, verification: ObservationVerification) -> Self {
        self.source_verification = verification;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserDiagnosticCode {
    LineTooLarge,
    MalformedJson,
    InvalidToken,
    InvalidTimestamp,
    ZeroUsage,
    InvalidModel,
    ModelFallback,
    InvalidMetadata,
    MetadataTruncated,
    InvalidPath,
    ToolCapacity,
    Irrelevant,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ParserDiagnostics {
    lines: u64,
    emitted_events: u64,
    metadata_lines: u64,
    tool_events: u64,
    line_too_large: u64,
    malformed_json: u64,
    invalid_token: u64,
    invalid_timestamp: u64,
    zero_usage: u64,
    invalid_model: u64,
    model_fallback: u64,
    invalid_metadata: u64,
    metadata_truncated: u64,
    invalid_path: u64,
    tool_capacity: u64,
    irrelevant: u64,
}

impl ParserDiagnostics {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lines: 0,
            emitted_events: 0,
            metadata_lines: 0,
            tool_events: 0,
            line_too_large: 0,
            malformed_json: 0,
            invalid_token: 0,
            invalid_timestamp: 0,
            zero_usage: 0,
            invalid_model: 0,
            model_fallback: 0,
            invalid_metadata: 0,
            metadata_truncated: 0,
            invalid_path: 0,
            tool_capacity: 0,
            irrelevant: 0,
        }
    }

    pub(crate) fn record(&mut self, code: ParserDiagnosticCode) {
        let counter = match code {
            ParserDiagnosticCode::LineTooLarge => &mut self.line_too_large,
            ParserDiagnosticCode::MalformedJson => &mut self.malformed_json,
            ParserDiagnosticCode::InvalidToken => &mut self.invalid_token,
            ParserDiagnosticCode::InvalidTimestamp => &mut self.invalid_timestamp,
            ParserDiagnosticCode::ZeroUsage => &mut self.zero_usage,
            ParserDiagnosticCode::InvalidModel => &mut self.invalid_model,
            ParserDiagnosticCode::ModelFallback => &mut self.model_fallback,
            ParserDiagnosticCode::InvalidMetadata => &mut self.invalid_metadata,
            ParserDiagnosticCode::MetadataTruncated => &mut self.metadata_truncated,
            ParserDiagnosticCode::InvalidPath => &mut self.invalid_path,
            ParserDiagnosticCode::ToolCapacity => &mut self.tool_capacity,
            ParserDiagnosticCode::Irrelevant => &mut self.irrelevant,
        };
        *counter = counter.saturating_add(1);
    }

    pub(crate) fn record_line(&mut self) {
        self.lines = self.lines.saturating_add(1);
    }

    pub(crate) fn record_emitted_event(&mut self) {
        self.emitted_events = self.emitted_events.saturating_add(1);
    }

    pub(crate) fn record_metadata_line(&mut self) {
        self.metadata_lines = self.metadata_lines.saturating_add(1);
    }

    pub(crate) fn record_tool_event(&mut self) {
        self.tool_events = self.tool_events.saturating_add(1);
    }

    #[must_use]
    pub const fn lines(&self) -> u64 {
        self.lines
    }

    #[must_use]
    pub const fn emitted_events(&self) -> u64 {
        self.emitted_events
    }

    #[must_use]
    pub const fn metadata_lines(&self) -> u64 {
        self.metadata_lines
    }

    #[must_use]
    pub const fn tool_events(&self) -> u64 {
        self.tool_events
    }

    #[must_use]
    pub const fn count(&self, code: ParserDiagnosticCode) -> u64 {
        match code {
            ParserDiagnosticCode::LineTooLarge => self.line_too_large,
            ParserDiagnosticCode::MalformedJson => self.malformed_json,
            ParserDiagnosticCode::InvalidToken => self.invalid_token,
            ParserDiagnosticCode::InvalidTimestamp => self.invalid_timestamp,
            ParserDiagnosticCode::ZeroUsage => self.zero_usage,
            ParserDiagnosticCode::InvalidModel => self.invalid_model,
            ParserDiagnosticCode::ModelFallback => self.model_fallback,
            ParserDiagnosticCode::InvalidMetadata => self.invalid_metadata,
            ParserDiagnosticCode::MetadataTruncated => self.metadata_truncated,
            ParserDiagnosticCode::InvalidPath => self.invalid_path,
            ParserDiagnosticCode::ToolCapacity => self.tool_capacity,
            ParserDiagnosticCode::Irrelevant => self.irrelevant,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
// Keeping the event inline avoids one heap allocation on every emitted usage record.
#[allow(clippy::large_enum_variant)]
pub enum ParseOutcome {
    Emitted(ObservationDraft),
    SessionRelation(SessionRelationDraft),
    MetadataOnly,
    ToolOnly,
    Skipped,
    Rejected(ParserDiagnosticCode),
}

pub fn parse_line(
    context: &ParseContext,
    state: &mut ParserState,
    diagnostics: &mut ParserDiagnostics,
    source_offset: u64,
    complete_line: &[u8],
) -> ParseOutcome {
    diagnostics.record_line();
    if complete_line.len() > MAX_LINE_BYTES {
        diagnostics.record(ParserDiagnosticCode::LineTooLarge);
        return ParseOutcome::Rejected(ParserDiagnosticCode::LineTooLarge);
    }
    if !looks_relevant(complete_line) {
        diagnostics.record(ParserDiagnosticCode::Irrelevant);
        return ParseOutcome::Skipped;
    }

    let raw: RawLine<'_> = match serde_json::from_slice(complete_line) {
        Ok(value) => value,
        Err(_) => {
            diagnostics.record(ParserDiagnosticCode::MalformedJson);
            return ParseOutcome::Rejected(ParserDiagnosticCode::MalformedJson);
        }
    };
    let Some(effect) = line_effect(&raw, state.previous_totals.as_ref(), diagnostics) else {
        let mut metadata = metadata_update(&raw, true, diagnostics);
        let session_id = resolved_session_id(context, state, &metadata);
        normalize_lineage_conflict(state, &mut metadata, &session_id);
        let relation = session_relation_draft(context, &metadata, &session_id, source_offset);
        let metadata_only = metadata.has_updates();
        let tool = tool_update(&raw, diagnostics);
        let tool_only = tool.is_some();
        record_repository_activity_hint(
            context,
            state,
            &metadata,
            &session_id,
            first_timestamp([&raw.timestamp, &raw.created_at, &raw.created_at_v2]),
        );
        apply_metadata(state, metadata);
        if metadata_only {
            diagnostics.record_metadata_line();
        }
        if let Some(tool) = tool {
            state.record_tool(tool.name, tool.activity, diagnostics);
            diagnostics.record_tool_event();
        }
        return if let Some(relation) = relation {
            ParseOutcome::SessionRelation(relation)
        } else if tool_only {
            ParseOutcome::ToolOnly
        } else if metadata_only {
            ParseOutcome::MetadataOnly
        } else {
            ParseOutcome::Skipped
        };
    };
    let Some(timestamp) = effect.timestamp else {
        diagnostics.record(ParserDiagnosticCode::InvalidTimestamp);
        return ParseOutcome::Rejected(ParserDiagnosticCode::InvalidTimestamp);
    };
    let usage_positive = usage_is_positive(&effect.usage);
    let include_metadata_model =
        !usage_positive || (!effect.model_includes_payload && effect.model.value.is_none());
    let mut metadata = metadata_update(&raw, include_metadata_model, diagnostics);
    let metadata_line = metadata.has_updates();
    let tool = tool_update(&raw, diagnostics);
    if !usage_positive {
        let session_id = resolved_session_id(context, state, &metadata);
        normalize_lineage_conflict(state, &mut metadata, &session_id);
        apply_metadata(state, metadata);
        if metadata_line {
            diagnostics.record_metadata_line();
        }
        if let Some(tool) = tool {
            state.record_tool(tool.name, tool.activity, diagnostics);
            diagnostics.record_tool_event();
        }
        if let Some(baseline_update) = effect.baseline_update {
            state.previous_totals = Some(baseline_update);
        }
        diagnostics.record(ParserDiagnosticCode::ZeroUsage);
        return ParseOutcome::Skipped;
    }

    let Some(model) = resolve_model(
        effect.model.value,
        metadata
            .current_model
            .as_ref()
            .or(state.current_model.as_ref()),
        effect.model.invalid_aliases,
        diagnostics,
    ) else {
        diagnostics.record(ParserDiagnosticCode::InvalidModel);
        return ParseOutcome::Rejected(ParserDiagnosticCode::InvalidModel);
    };
    let long_context = match effect.usage.input() {
        TokenCount::Available(value) if value > LONG_CONTEXT_THRESHOLD => LongContextState::Yes,
        TokenCount::Available(_) => LongContextState::No,
        TokenCount::Unavailable => LongContextState::Unavailable,
    };
    let session_id = resolved_session_id(context, state, &metadata);
    normalize_lineage_conflict(state, &mut metadata, &session_id);
    let parent_session_id = metadata
        .parent_session_id
        .as_ref()
        .or(state.parent_session_id.as_ref())
        .cloned();
    let lineage_conflict = metadata.lineage_conflict || state.lineage_conflict;
    let service_tier = metadata
        .service_tier
        .as_ref()
        .or(state.service_tier.as_ref())
        .cloned();
    let project = metadata
        .project
        .as_ref()
        .or(state.project.as_ref())
        .cloned();
    let originator = metadata
        .originator
        .as_ref()
        .or(state.originator.as_ref())
        .cloned();
    let mut activity = state.pending_activity;
    if let Some(tool) = tool.as_ref() {
        activity.increment(tool.activity);
    }
    let event = match ObservationDraft::new(ObservationDraftParts {
        provider_id: context.provider_id.clone(),
        profile_id: context.profile_id.clone(),
        session_id: session_id.clone(),
        parent_session_id,
        session_ordinal: state.next_usage_ordinal,
        lineage_conflict,
        source_id: context.source_id.clone(),
        source_offset,
        source_verification: context.source_verification,
        timestamp,
        model: model.key.clone(),
        raw_model: model.raw.clone(),
        delta_usage: effect.usage,
        cumulative_usage: effect.baseline_update,
        fallback_model: model.fallback,
        long_context,
        service_tier,
        reported_cost: None,
        project,
        originator,
        activity,
    }) {
        Ok(event) => event,
        Err(_) => {
            diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
            return ParseOutcome::Rejected(ParserDiagnosticCode::InvalidMetadata);
        }
    };
    record_repository_activity_hint(context, state, &metadata, &session_id, Some(timestamp));
    apply_metadata(state, metadata);
    if metadata_line {
        diagnostics.record_metadata_line();
    }
    if let Some(tool) = tool {
        state.record_tool(tool.name, tool.activity, diagnostics);
        diagnostics.record_tool_event();
    }
    if let Some(baseline_update) = effect.baseline_update {
        state.previous_totals = Some(baseline_update);
    }
    state.current_model = Some(model);
    if state.session_id.is_none() {
        state.session_id = Some(session_id);
    }
    state.next_usage_ordinal = state.next_usage_ordinal.saturating_add(1);
    state.pending_activity = ActivityCounts::default();
    diagnostics.record_emitted_event();
    ParseOutcome::Emitted(event)
}

fn record_repository_activity_hint(
    context: &ParseContext,
    state: &mut ParserState,
    metadata: &effects::MetadataUpdate,
    session_id: &UsageSessionId,
    observed_at: Option<tokenmaster_domain::UtcTimestamp>,
) {
    let Some(observed_at) = observed_at else {
        return;
    };
    let (candidate, project) = match &metadata.repository_candidate {
        effects::RepositoryCandidateUpdate::Missing => {
            let Some(candidate) = state.repository_candidate.clone() else {
                return;
            };
            (candidate, state.repository_project.clone())
        }
        effects::RepositoryCandidateUpdate::Set(candidate) => {
            (candidate.clone(), metadata.project.clone())
        }
        effects::RepositoryCandidateUpdate::Clear => return,
    };
    state.record_repository_activity_hint(RepositoryActivityHint::new(
        RepositoryActivityHintParts {
            provider_id: context.provider_id.clone(),
            profile_id: context.profile_id.clone(),
            source_id: context.source_id.clone(),
            session_id: session_id.clone(),
            observed_at,
            project,
            candidate,
        },
    ));
}

fn resolved_session_id(
    context: &ParseContext,
    state: &ParserState,
    metadata: &effects::MetadataUpdate,
) -> UsageSessionId {
    metadata
        .session_id
        .as_ref()
        .or(state.session_id.as_ref())
        .or(context.filename_session_hint.as_ref())
        .unwrap_or(&context.hashed_session_hint)
        .clone()
}

fn normalize_lineage_conflict(
    state: &ParserState,
    metadata: &mut effects::MetadataUpdate,
    session_id: &UsageSessionId,
) {
    let incoming_parent = metadata.parent_session_id.as_ref();
    let effective_parent = incoming_parent.or(state.parent_session_id.as_ref());
    metadata.lineage_conflict |= state.lineage_conflict
        || effective_parent.is_some_and(|parent| parent == session_id)
        || incoming_parent.is_some_and(|parent| {
            state
                .parent_session_id
                .as_ref()
                .is_some_and(|existing| existing != parent)
        });
}

fn session_relation_draft(
    context: &ParseContext,
    metadata: &effects::MetadataUpdate,
    session_id: &UsageSessionId,
    source_offset: u64,
) -> Option<SessionRelationDraft> {
    let parent_session_id = metadata.parent_session_id.clone()?;
    SessionRelationDraft::new(SessionRelationDraftParts {
        provider_id: context.provider_id.clone(),
        profile_id: context.profile_id.clone(),
        session_id: session_id.clone(),
        parent_session_id,
        declared_conflict: metadata.lineage_conflict,
        source_id: context.source_id.clone(),
        source_offset,
    })
    .ok()
}

struct LineEffect<'a> {
    timestamp: Option<tokenmaster_domain::UtcTimestamp>,
    model: ModelCandidate<'a>,
    usage: TokenUsage,
    baseline_update: Option<TokenUsage>,
    model_includes_payload: bool,
}

struct ModelCandidate<'a> {
    value: Option<&'a str>,
    invalid_aliases: usize,
}

fn line_effect<'a>(
    raw: &'a RawLine<'a>,
    previous_totals: Option<&TokenUsage>,
    diagnostics: &mut ParserDiagnostics,
) -> Option<LineEffect<'a>> {
    if raw.kind.value() == Some("event_msg")
        && let Some(payload) = raw.payload.as_ref()
        && payload.kind.value() == Some("token_count")
    {
        let info = payload.info.as_ref()?;
        let baseline_update = info
            .total_token_usage
            .as_ref()
            .map(|usage| normalize_usage(usage, diagnostics));
        let usage = if let Some(usage) = info.last_token_usage.as_ref() {
            normalize_usage(usage, diagnostics)
        } else {
            subtract_usage(baseline_update.as_ref()?, previous_totals)
        };
        return Some(LineEffect {
            timestamp: first_timestamp([&raw.timestamp, &raw.created_at, &raw.created_at_v2]),
            model: first_model([
                &payload.model,
                &payload.model_name,
                &payload.model_slug,
                &payload.model_id,
                &info.model,
                &info.model_name,
                &payload.requested_model,
                &payload.default_model,
            ]),
            usage,
            baseline_update,
            model_includes_payload: true,
        });
    }

    if let Some(result) = raw
        .data
        .as_ref()
        .or(raw.result.as_ref())
        .or(raw.response.as_ref())
        && let Some(usage) = result.usage.as_ref()
    {
        return Some(effect_from_result(raw, result, usage, diagnostics));
    }

    raw.usage.as_ref().map(|usage| LineEffect {
        timestamp: first_timestamp([&raw.timestamp, &raw.created_at, &raw.created_at_v2]),
        model: first_model([&raw.model, &raw.model_name]),
        usage: normalize_usage(usage, diagnostics),
        baseline_update: None,
        model_includes_payload: false,
    })
}

fn effect_from_result<'a>(
    raw: &'a RawLine<'a>,
    result: &'a RawResult<'a>,
    usage: &RawUsage,
    diagnostics: &mut ParserDiagnostics,
) -> LineEffect<'a> {
    LineEffect {
        timestamp: first_timestamp([
            &result.timestamp,
            &result.created_at,
            &result.created_at_v2,
            &raw.timestamp,
            &raw.created_at,
            &raw.created_at_v2,
        ]),
        model: first_model([
            &result.model,
            &result.model_name,
            &raw.model,
            &raw.model_name,
        ]),
        usage: normalize_usage(usage, diagnostics),
        baseline_update: None,
        model_includes_payload: false,
    }
}

fn first_timestamp<const N: usize>(
    values: [&value::TimestampScalar<'_>; N],
) -> Option<tokenmaster_domain::UtcTimestamp> {
    values.into_iter().find_map(timestamp_value)
}

fn first_model<'a, const N: usize>(values: [&'a RawText<'a>; N]) -> ModelCandidate<'a> {
    let mut invalid_aliases = 0_usize;
    for value in values {
        if let Some(value) = value.value() {
            return ModelCandidate {
                value: Some(value),
                invalid_aliases,
            };
        }
        invalid_aliases = invalid_aliases.saturating_add(usize::from(value.is_invalid()));
    }
    ModelCandidate {
        value: None,
        invalid_aliases,
    }
}

fn normalize_usage(raw: &RawUsage, diagnostics: &mut ParserDiagnostics) -> TokenUsage {
    let input = first_token(
        &[raw.input_tokens, raw.prompt_tokens, raw.input],
        diagnostics,
    );
    let mut cached = first_token(
        &[
            raw.cached_input_tokens,
            raw.cache_read_input_tokens,
            raw.cached_tokens,
        ],
        diagnostics,
    );
    let output = first_token(
        &[raw.output_tokens, raw.completion_tokens, raw.output],
        diagnostics,
    );
    let reasoning = first_token(
        &[raw.reasoning_output_tokens, raw.reasoning_tokens],
        diagnostics,
    );
    let provider_total = first_token(&[raw.total_tokens], diagnostics);

    cached = match (input, cached) {
        (TokenCount::Available(input), TokenCount::Available(cached)) => {
            TokenCount::Available(cached.min(input))
        }
        _ => TokenCount::Unavailable,
    };
    let total = match provider_total {
        TokenCount::Available(value) if value > 0 => TokenCount::Available(value),
        _ => match (input, output, reasoning) {
            (
                TokenCount::Available(input),
                TokenCount::Available(output),
                TokenCount::Available(reasoning),
            ) => TokenCount::Available(input.saturating_add(output).saturating_add(reasoning)),
            _ => TokenCount::Unavailable,
        },
    };
    TokenUsage::new(input, cached, output, reasoning, total)
}

fn usage_is_positive(usage: &TokenUsage) -> bool {
    [
        usage.input(),
        usage.cached(),
        usage.output(),
        usage.reasoning(),
        usage.total(),
    ]
    .into_iter()
    .any(|value| matches!(value, TokenCount::Available(count) if count > 0))
}

fn subtract_usage(current: &TokenUsage, previous: Option<&TokenUsage>) -> TokenUsage {
    let Some(previous) = previous else {
        return *current;
    };
    TokenUsage::new(
        subtract_count(current.input(), previous.input()),
        subtract_count(current.cached(), previous.cached()),
        subtract_count(current.output(), previous.output()),
        subtract_count(current.reasoning(), previous.reasoning()),
        subtract_count(current.total(), previous.total()),
    )
}

fn subtract_count(current: TokenCount, previous: TokenCount) -> TokenCount {
    match (current, previous) {
        (TokenCount::Available(current), TokenCount::Available(previous))
            if current >= previous =>
        {
            TokenCount::Available(current.saturating_sub(previous))
        }
        (TokenCount::Available(current), _) => TokenCount::Available(current),
        (TokenCount::Unavailable, _) => TokenCount::Unavailable,
    }
}

fn looks_relevant(line: &[u8]) -> bool {
    line.contains(&b'\\')
        || RELEVANCE_MARKERS
            .iter()
            .any(|marker| line.windows(marker.len()).any(|window| window == *marker))
}

#[cfg(test)]
mod tests {
    use tokenmaster_domain::{TokenCount, UsageProfileId, UsageSessionId, UsageSourceId};

    use super::{ParseContext, ParseOutcome, ParserDiagnostics, ParserState, parse_line};

    #[test]
    fn zero_cumulative_usage_updates_the_working_baseline() {
        let profile = match UsageProfileId::new("profile_fixture") {
            Ok(value) => value,
            Err(error) => panic!("static profile must be valid: {error}"),
        };
        let source = match UsageSourceId::new("source_fixture") {
            Ok(value) => value,
            Err(error) => panic!("static source must be valid: {error}"),
        };
        let session = match UsageSessionId::new("session_fixture") {
            Ok(value) => value,
            Err(error) => panic!("static session must be valid: {error}"),
        };
        let context = ParseContext::new(profile, source, Some(session.clone()), session);
        let mut state = ParserState::new();
        let mut diagnostics = ParserDiagnostics::new();

        let outcome = parse_line(
            &context,
            &mut state,
            &mut diagnostics,
            0,
            br#"{"timestamp":1700000000,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":0,"cached_input_tokens":0,"output_tokens":0,"reasoning_tokens":0,"total_tokens":0}}}}"#,
        );

        assert!(matches!(outcome, ParseOutcome::Skipped));
        assert!(state.previous_totals.is_some_and(|usage| {
            usage.input() == TokenCount::Available(0)
                && usage.cached() == TokenCount::Available(0)
                && usage.output() == TokenCount::Available(0)
                && usage.reasoning() == TokenCount::Available(0)
                && usage.total() == TokenCount::Available(0)
        }));
    }
}
