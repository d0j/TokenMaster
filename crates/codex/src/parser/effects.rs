use std::path::{Component, Path};

use tokenmaster_domain::{
    ActivityKind, MAX_METADATA_BYTES, MetadataValue, ProjectAlias, UsageSessionId,
};
use tokenmaster_provider::RepositoryCandidatePath;

use super::state::{MAX_CONTEXT_WINDOW_TOKENS, MAX_TOOL_NAME_BYTES};
use super::value::{LenientU64, ResolvedModel, normalize_explicit_model};
use super::wire::{RawDisplayText, RawLine, RawPathText, RawPayload, RawText};
use super::{ModelCandidate, ParserDiagnosticCode, ParserDiagnostics, ParserState, first_model};
use crate::path_policy::validate_local_root_namespace;

#[derive(Default)]
pub(super) struct MetadataUpdate {
    pub(super) current_model: Option<ResolvedModel>,
    pub(super) service_tier: Option<MetadataValue>,
    pub(super) session_id: Option<UsageSessionId>,
    pub(super) project: Option<ProjectAlias>,
    pub(super) repository_candidate: RepositoryCandidateUpdate,
    pub(super) originator: Option<MetadataValue>,
    pub(super) source_alias: Option<MetadataValue>,
    pub(super) git_branch: Option<MetadataValue>,
    pub(super) context_window: Option<u64>,
    pub(super) parent_session_id: Option<UsageSessionId>,
    pub(super) lineage_conflict: bool,
}

impl MetadataUpdate {
    pub(super) fn has_updates(&self) -> bool {
        self.current_model.is_some()
            || self.service_tier.is_some()
            || self.session_id.is_some()
            || self.project.is_some()
            || !matches!(
                self.repository_candidate,
                RepositoryCandidateUpdate::Missing
            )
            || self.originator.is_some()
            || self.source_alias.is_some()
            || self.git_branch.is_some()
            || self.context_window.is_some()
            || self.parent_session_id.is_some()
            || self.lineage_conflict
    }
}

pub(super) struct ToolUpdate<'a> {
    pub(super) name: &'a str,
    pub(super) activity: ActivityKind,
}

pub(super) fn metadata_update(
    raw: &RawLine<'_>,
    include_model: bool,
    diagnostics: &mut ParserDiagnostics,
) -> MetadataUpdate {
    let Some(payload) = raw.payload.as_ref() else {
        return MetadataUpdate::default();
    };
    let current_model = include_model.then(|| {
        normalize_metadata_model(
            first_model([
                &payload.model,
                &payload.model_name,
                &payload.model_slug,
                &payload.model_id,
                &payload.requested_model,
                &payload.default_model,
            ]),
            diagnostics,
        )
    });
    let (parent_session_id, lineage_conflict) = lineage_metadata(raw, payload, diagnostics);
    MetadataUpdate {
        current_model: current_model.flatten(),
        service_tier: display_metadata(&payload.service_tier, diagnostics),
        session_id: session_metadata(raw.kind.value(), payload, diagnostics),
        project: project_alias(&payload.cwd, diagnostics),
        repository_candidate: repository_candidate(&payload.cwd),
        originator: display_metadata(&payload.originator, diagnostics),
        source_alias: display_metadata(payload.source.display(), diagnostics),
        git_branch: display_metadata(&payload.git.branch, diagnostics),
        context_window: context_window(payload.model_context_window, diagnostics),
        parent_session_id,
        lineage_conflict,
    }
}

#[derive(Default)]
pub(super) enum RepositoryCandidateUpdate {
    #[default]
    Missing,
    Set(RepositoryCandidatePath),
    Clear,
}

fn lineage_metadata(
    raw: &RawLine<'_>,
    payload: &RawPayload<'_>,
    diagnostics: &mut ParserDiagnostics,
) -> (Option<UsageSessionId>, bool) {
    let mut parent = None;
    let mut conflict = false;
    for candidate in [
        &raw.forked_from_id,
        &raw.parent_thread_id,
        &payload.forked_from_id,
        &payload.parent_thread_id,
        payload.source.structured_parent_thread_id(),
    ] {
        if candidate.is_invalid() {
            diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
            continue;
        }
        let Some(value) = candidate.value() else {
            continue;
        };
        let Some(candidate) = UsageSessionId::new(value.to_owned())
            .map_err(|_| diagnostics.record(ParserDiagnosticCode::InvalidMetadata))
            .ok()
        else {
            continue;
        };
        match parent.as_ref() {
            None => parent = Some(candidate),
            Some(existing) if existing == &candidate => {}
            Some(_) => conflict = true,
        }
    }
    (parent, conflict)
}

fn normalize_metadata_model(
    candidate: ModelCandidate<'_>,
    diagnostics: &mut ParserDiagnostics,
) -> Option<ResolvedModel> {
    for _ in 0..candidate.invalid_aliases {
        diagnostics.record(ParserDiagnosticCode::InvalidModel);
    }
    let value = candidate.value?;
    let resolved = normalize_explicit_model(value);
    if resolved.is_none() {
        diagnostics.record(ParserDiagnosticCode::InvalidModel);
    }
    resolved
}

fn display_metadata(
    raw: &RawDisplayText<'_>,
    diagnostics: &mut ParserDiagnostics,
) -> Option<MetadataValue> {
    if raw.is_invalid() {
        diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
        return None;
    }
    let value = raw.value()?.trim();
    if value.is_empty() {
        diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
        return None;
    }
    let (value, truncated) = truncate_utf8(value, MAX_METADATA_BYTES);
    if truncated {
        diagnostics.record(ParserDiagnosticCode::MetadataTruncated);
    }
    MetadataValue::new(value.to_owned()).map_or_else(
        |_| {
            diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
            None
        },
        Some,
    )
}

fn session_metadata(
    line_kind: Option<&str>,
    payload: &RawPayload<'_>,
    diagnostics: &mut ParserDiagnostics,
) -> Option<UsageSessionId> {
    if matches!(line_kind, Some("session_meta" | "sessionMeta")) {
        session_identity(&payload.id, diagnostics)
            .or_else(|| session_identity(&payload.session_id, diagnostics))
    } else {
        session_identity(&payload.session_id, diagnostics)
    }
}

fn session_identity(
    raw: &RawText<'_>,
    diagnostics: &mut ParserDiagnostics,
) -> Option<UsageSessionId> {
    if raw.is_invalid() {
        diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
        return None;
    }
    let value = raw.value()?;
    UsageSessionId::new(value.to_owned()).map_or_else(
        |_| {
            diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
            None
        },
        Some,
    )
}

fn project_alias(
    raw: &RawPathText<'_>,
    diagnostics: &mut ParserDiagnostics,
) -> Option<ProjectAlias> {
    if raw.is_invalid() {
        diagnostics.record(ParserDiagnosticCode::InvalidPath);
        return None;
    }
    let value = raw.value()?.trim();
    let path = Path::new(value);
    if value.is_empty()
        || validate_local_root_namespace(path).is_err()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        diagnostics.record(ParserDiagnosticCode::InvalidPath);
        return None;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        diagnostics.record(ParserDiagnosticCode::InvalidPath);
        return None;
    };
    let (name, truncated) = truncate_utf8(name, MAX_METADATA_BYTES);
    if truncated {
        diagnostics.record(ParserDiagnosticCode::MetadataTruncated);
    }
    ProjectAlias::new(name.to_owned()).map_or_else(
        |_| {
            diagnostics.record(ParserDiagnosticCode::InvalidPath);
            None
        },
        Some,
    )
}

fn repository_candidate(raw: &RawPathText<'_>) -> RepositoryCandidateUpdate {
    if raw.is_missing() {
        return RepositoryCandidateUpdate::Missing;
    }
    if raw.is_invalid() {
        return RepositoryCandidateUpdate::Clear;
    }
    let Some(value) = raw.value() else {
        return RepositoryCandidateUpdate::Clear;
    };
    let value = value.trim();
    let path = Path::new(value);
    if value.is_empty()
        || validate_local_root_namespace(path).is_err()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return RepositoryCandidateUpdate::Clear;
    }
    RepositoryCandidatePath::new(path.to_path_buf()).map_or(
        RepositoryCandidateUpdate::Clear,
        RepositoryCandidateUpdate::Set,
    )
}

fn context_window(raw: LenientU64, diagnostics: &mut ParserDiagnostics) -> Option<u64> {
    match raw {
        LenientU64::Valid(value) if (1..=MAX_CONTEXT_WINDOW_TOKENS).contains(&value) => Some(value),
        LenientU64::Missing => None,
        LenientU64::Valid(_) | LenientU64::Invalid => {
            diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
            None
        }
    }
}

pub(super) fn tool_update<'a>(
    raw: &'a RawLine<'a>,
    diagnostics: &mut ParserDiagnostics,
) -> Option<ToolUpdate<'a>> {
    let payload = raw.payload.as_ref()?;
    let payload_kind = payload.kind.value();
    let line_kind = raw.kind.value();
    let discriminator = payload_kind.or(line_kind)?;
    let recognized_kind = ["function_call", "tool_call", "exec_command", "mcp"]
        .into_iter()
        .any(|marker| contains_ascii_case_insensitive(discriminator, marker));
    let response_item_with_name = line_kind == Some("response_item")
        && payload.name.value().is_some_and(|name| !name.is_empty());
    if !recognized_kind && !response_item_with_name {
        return None;
    }
    if payload.name.is_invalid() {
        diagnostics.record(ParserDiagnosticCode::InvalidMetadata);
    }
    let raw_name = payload
        .name
        .value()
        .filter(|name| !name.is_empty())
        .or(payload_kind)
        .unwrap_or("unknown");
    let trimmed = raw_name.trim();
    let safe_name = if trimmed.is_empty() {
        "unknown"
    } else {
        trimmed
    };
    let (safe_name, truncated) = truncate_utf8(safe_name, MAX_TOOL_NAME_BYTES);
    if truncated {
        diagnostics.record(ParserDiagnosticCode::MetadataTruncated);
    }
    let activity = classify_tool(safe_name);
    Some(ToolUpdate {
        name: safe_name,
        activity,
    })
}

fn classify_tool(name: &str) -> ActivityKind {
    let matches_any = |markers: &[&str]| {
        markers
            .iter()
            .any(|marker| contains_ascii_case_insensitive(name, marker))
    };
    if matches_any(&["read", "cat", "view"]) {
        ActivityKind::Read
    } else if matches_any(&["write", "edit", "patch", "create"]) {
        ActivityKind::EditWrite
    } else if matches_any(&["search", "grep", "find"]) {
        ActivityKind::Search
    } else if matches_any(&["git"]) {
        ActivityKind::Git
    } else if matches_any(&["test", "build", "compile"]) {
        ActivityKind::BuildTest
    } else if matches_any(&["web", "browser", "http"]) {
        ActivityKind::Web
    } else if matches_any(&["agent", "delegate", "spawn"]) {
        ActivityKind::Subagents
    } else {
        ActivityKind::Terminal
    }
}

fn contains_ascii_case_insensitive(value: &str, needle: &str) -> bool {
    value
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (&value[..end], true)
}

pub(super) fn apply_metadata(state: &mut ParserState, update: MetadataUpdate) {
    if let Some(parent) = update.parent_session_id {
        match state.parent_session_id.as_ref() {
            None => state.parent_session_id = Some(parent),
            Some(existing) if existing == &parent => {}
            Some(_) => state.lineage_conflict = true,
        }
    }
    state.lineage_conflict |= update.lineage_conflict;
    if let Some(value) = update.current_model {
        state.current_model = Some(value);
    }
    if let Some(value) = update.service_tier {
        state.service_tier = Some(value);
    }
    if let Some(value) = update.session_id {
        state.session_id = Some(value);
    }
    let repository_project = update.project.clone();
    if let Some(value) = update.project {
        state.project = Some(value);
    }
    match update.repository_candidate {
        RepositoryCandidateUpdate::Missing => {}
        RepositoryCandidateUpdate::Set(value) => {
            state.repository_candidate = Some(value);
            state.repository_project = repository_project;
        }
        RepositoryCandidateUpdate::Clear => {
            state.repository_candidate = None;
            state.repository_project = None;
        }
    }
    if let Some(value) = update.originator {
        state.originator = Some(value);
    }
    if let Some(value) = update.source_alias {
        state.source_alias = Some(value);
    }
    if let Some(value) = update.git_branch {
        state.git_branch = Some(value);
    }
    if let Some(value) = update.context_window {
        state.context_window = Some(value);
    }
}
