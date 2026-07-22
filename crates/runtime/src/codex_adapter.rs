use std::path::PathBuf;

use tokenmaster_codex::{
    BoundaryAnchor, CodexCheckpointV1, CodexProvider, EnumerationCompletion, LogicalFileIdentity,
    ParserDiagnosticCode, ParserResumeState, ReaderCheckpointParts, ReaderCheckpointV1,
    ReaderDiagnosticCode, ReaderErrorCode, ReaderOutcome, SinkDecision, SourceCheckpointStatus,
    SourceChunkDigest, SourceFileDescriptor, enumerate_profile_sources,
    initialize_source_checkpoint, logical_file_identity, read_source_batch,
    validate_source_checkpoint,
};
use tokenmaster_engine::{
    Adapter, AdapterBatch, AdapterBatchParts, AdapterCheckpoint, AdapterCompletion,
    AdapterCounters, AdapterDiagnosticCode, AdapterDiagnostics, AdapterSourceProgress,
    AdapterSourceProgressParts, AdapterSourceState, AdapterVerification, BatchState, ChunkProof,
    ChunkProofBatch, CompletionQuality, DiscoveredSource, OperationControl, PortError,
    PortErrorCode, ReplaySourceSink, ScopeIdentity, ScopeSink, SinkControl, SourceBatchReader,
    SourceIdentity, SourceKind, SourceSink,
};
use tokenmaster_platform::PhysicalFileIdentity;
use tokenmaster_provider::{
    DiscoveryProvider, DiscoveryRequest, DiscoverySnapshot, ProfileAvailability, SourceDescriptor,
};

use crate::error::provider_port_error;
use crate::{RuntimeError, RuntimeErrorCode};

pub struct CodexAdapter {
    provider: CodexProvider,
    request: DiscoveryRequest,
    snapshot: Option<DiscoverySnapshot>,
    repository_hint_ingress: Option<crate::GitRepositoryHintIngress>,
}

impl CodexAdapter {
    pub fn new(request: DiscoveryRequest) -> Result<Self, RuntimeError> {
        let provider = CodexProvider::new()
            .map_err(|_| RuntimeError::new(RuntimeErrorCode::ProviderUnavailable))?;
        Ok(Self {
            provider,
            request,
            snapshot: None,
            repository_hint_ingress: None,
        })
    }

    pub(crate) fn with_repository_hint_ingress(
        mut self,
        ingress: crate::GitRepositoryHintIngress,
    ) -> Self {
        self.repository_hint_ingress = Some(ingress);
        self
    }

    pub(crate) fn watch_roots(&self) -> Option<Vec<PathBuf>> {
        let snapshot = self.snapshot.as_ref()?;
        let mut roots = snapshot
            .profiles()
            .iter()
            .filter(|profile| profile.availability() != ProfileAvailability::Rejected)
            .map(|profile| profile.path().to_path_buf())
            .collect::<Vec<_>>();
        roots.sort_unstable();
        roots.dedup();
        Some(roots)
    }

    fn profile_sources<'a>(
        snapshot: &'a DiscoverySnapshot,
        scope: &ScopeIdentity,
    ) -> Result<
        (
            &'a tokenmaster_provider::ProfileDescriptor,
            Vec<SourceDescriptor>,
        ),
        PortError,
    > {
        if scope.provider_id() != "codex" {
            return Err(PortError::new(PortErrorCode::InvalidData));
        }
        let profile = snapshot
            .profiles()
            .iter()
            .find(|profile| profile.id().as_str() == scope.profile_id())
            .ok_or_else(|| PortError::new(PortErrorCode::InvalidData))?;
        let sources = snapshot
            .sources()
            .iter()
            .filter(|source| source.profile_id() == profile.id())
            .cloned()
            .collect();
        Ok((profile, sources))
    }

    fn visit_profile(
        &self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        mut emit: impl FnMut(
            DiscoveredSource,
            AdapterSourceState,
            SourceFileDescriptor,
        ) -> Result<SinkControl, PortError>,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| PortError::new(PortErrorCode::StaleState))?;
        let (profile, sources) = Self::profile_sources(snapshot, scope)?;
        if profile.availability() != ProfileAvailability::Available {
            return completion(CompletionQuality::Partial, 0, true);
        }
        if sources.is_empty() {
            return completion(CompletionQuality::Partial, 0, true);
        }

        let mut emitted = 0_u64;
        let mut degraded = false;
        let mut sink_error = None;
        let report = enumerate_profile_sources(
            &sources,
            || control.check().is_err(),
            |descriptor| {
                let initial = match initialize_source_checkpoint(&descriptor) {
                    Ok(checkpoint) => checkpoint,
                    Err(_) => {
                        degraded = true;
                        return SinkDecision::Continue;
                    }
                };
                let source = match discovered_source(&descriptor) {
                    Ok(source) => source,
                    Err(error) => {
                        sink_error = Some(error);
                        return SinkDecision::Fail;
                    }
                };
                let checkpoint = match encode_checkpoint(initial.clone()) {
                    Ok(checkpoint) => checkpoint,
                    Err(error) => {
                        sink_error = Some(error);
                        return SinkDecision::Fail;
                    }
                };
                let progress = match source_progress(&initial) {
                    Ok(progress) => progress,
                    Err(error) => {
                        sink_error = Some(error);
                        return SinkDecision::Fail;
                    }
                };
                let state = match AdapterSourceState::new(checkpoint, progress) {
                    Ok(state) => state,
                    Err(error) => {
                        sink_error = Some(error.into());
                        return SinkDecision::Fail;
                    }
                };
                match emit(source, state, descriptor) {
                    Ok(SinkControl::Continue) => {
                        emitted = emitted.saturating_add(1);
                        SinkDecision::Continue
                    }
                    Ok(SinkControl::Stop) => {
                        degraded = true;
                        SinkDecision::Cancel
                    }
                    Err(error) => {
                        sink_error = Some(error);
                        SinkDecision::Fail
                    }
                }
            },
        )
        .map_err(|_| sink_error.unwrap_or_else(|| PortError::new(PortErrorCode::InvalidData)))?;
        if let Some(error) = sink_error {
            return Err(error);
        }
        control.check()?;
        let quality = match report.completion() {
            EnumerationCompletion::Complete if !degraded => CompletionQuality::Complete,
            EnumerationCompletion::Complete | EnumerationCompletion::Partial => {
                CompletionQuality::Partial
            }
            EnumerationCompletion::Cancelled => CompletionQuality::Cancelled,
        };
        completion(
            quality,
            emitted,
            degraded || quality != CompletionQuality::Complete,
        )
    }
}

impl Adapter for CodexAdapter {
    fn visit_scopes(
        &mut self,
        control: &OperationControl<'_>,
        sink: &mut dyn ScopeSink,
    ) -> Result<AdapterCompletion, PortError> {
        control.check()?;
        let snapshot = self
            .provider
            .discover(&self.request)
            .map_err(|error| provider_port_error(&error))?;
        if snapshot.profiles().is_empty() {
            self.snapshot = Some(snapshot);
            return completion(CompletionQuality::Partial, 0, true);
        }
        let mut emitted = 0_u64;
        let mut quality = CompletionQuality::Complete;
        for profile in snapshot.profiles() {
            control.check()?;
            let scope =
                ScopeIdentity::new("codex", profile.id().as_str()).map_err(PortError::from)?;
            if sink.on_scope(scope)? == SinkControl::Stop {
                quality = CompletionQuality::Partial;
                break;
            }
            emitted = emitted.saturating_add(1);
        }
        self.snapshot = Some(snapshot);
        completion(quality, emitted, quality != CompletionQuality::Complete)
    }

    fn visit_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn SourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        self.visit_profile(scope, control, |source, state, _descriptor| {
            sink.on_source(source, state)
        })
    }

    fn visit_replay_sources(
        &mut self,
        scope: &ScopeIdentity,
        control: &OperationControl<'_>,
        sink: &mut dyn ReplaySourceSink,
    ) -> Result<AdapterCompletion, PortError> {
        self.visit_profile(scope, control, |source, state, descriptor| {
            let mut reader = CodexSourceBatchReader {
                descriptor,
                source: source.identity().clone(),
                latest_repository_activity_hint: None,
                repository_hint_ingress: self.repository_hint_ingress.clone(),
            };
            sink.on_source(source, state, &mut reader)
        })
    }
}

impl core::fmt::Debug for CodexAdapter {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CodexAdapter")
            .field("has_snapshot", &self.snapshot.is_some())
            .field(
                "repository_hint_ingress",
                &self.repository_hint_ingress.is_some(),
            )
            .finish()
    }
}

struct CodexSourceBatchReader {
    descriptor: SourceFileDescriptor,
    source: SourceIdentity,
    latest_repository_activity_hint: Option<tokenmaster_provider::RepositoryActivityHint>,
    repository_hint_ingress: Option<crate::GitRepositoryHintIngress>,
}

impl SourceBatchReader for CodexSourceBatchReader {
    fn restore_checkpoint(
        &mut self,
        progress: &AdapterSourceProgress,
        control: &OperationControl<'_>,
    ) -> Result<AdapterCheckpoint, PortError> {
        control.check()?;
        if progress.logical_identity() != self.source.logical_file_key() {
            return Err(PortError::new(PortErrorCode::InvalidData));
        }
        let resume: ParserResumeState = serde_json::from_slice(progress.provider_resume())
            .map_err(|_| PortError::new(PortErrorCode::InvalidData))?;
        let anchor = BoundaryAnchor::new(
            progress.anchor_start(),
            progress.anchor_len(),
            *progress.anchor_sha256(),
        )
        .map_err(|_| PortError::new(PortErrorCode::InvalidData))?;
        let checkpoint = ReaderCheckpointV1::new(ReaderCheckpointParts {
            parser_schema_version: progress.schema_version(),
            physical_identity: progress
                .physical_identity()
                .copied()
                .map(PhysicalFileIdentity::from_persisted_bytes),
            logical_identity: LogicalFileIdentity::from_bytes(*progress.logical_identity()),
            committed_offset: progress.committed_offset(),
            scan_offset: progress.scan_offset(),
            observed_file_length: progress.observed_extent(),
            modified_time_ns: progress.modified_time_ns(),
            anchor,
            resume,
            discarding_oversized_line: progress.discarding_oversized_record(),
            incomplete_tail: progress.incomplete_tail(),
            verification: match progress.verification() {
                AdapterVerification::Incremental => {
                    tokenmaster_codex::VerificationLevel::Incremental
                }
                AdapterVerification::Full => tokenmaster_codex::VerificationLevel::FullPrefix,
            },
        })
        .map_err(|_| PortError::new(PortErrorCode::InvalidData))?;
        encode_checkpoint(checkpoint)
    }

    fn validate_checkpoint(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<(), PortError> {
        control.check()?;
        let logical = logical_file_identity(&self.descriptor);
        let reader_checkpoint = CodexCheckpointV1::decode(checkpoint.as_bytes(), logical)
            .map_err(|_| PortError::new(PortErrorCode::InvalidData))?
            .into_reader();
        match validate_source_checkpoint(&self.descriptor, &reader_checkpoint)
            .map_err(|error| reader_port_error(error.code()))?
        {
            SourceCheckpointStatus::Unchanged | SourceCheckpointStatus::Appended => Ok(()),
            SourceCheckpointStatus::RebuildRequired(_) => {
                Err(PortError::new(PortErrorCode::RebuildRequired))
            }
        }
    }

    fn read_batch(
        &mut self,
        checkpoint: &AdapterCheckpoint,
        control: &OperationControl<'_>,
    ) -> Result<AdapterBatch, PortError> {
        self.latest_repository_activity_hint = None;
        control.check()?;
        let logical = logical_file_identity(&self.descriptor);
        let reader_checkpoint = CodexCheckpointV1::decode(checkpoint.as_bytes(), logical)
            .map_err(|_| PortError::new(PortErrorCode::InvalidData))?
            .into_reader();
        let outcome = read_source_batch(&self.descriptor, Some(&reader_checkpoint), || {
            control.check().is_err()
        })
        .map_err(|error| {
            if error.code() == ReaderErrorCode::Cancelled {
                control
                    .check()
                    .err()
                    .unwrap_or_else(|| PortError::new(PortErrorCode::Cancelled))
            } else {
                reader_port_error(error.code())
            }
        })?;

        match outcome {
            ReaderOutcome::RebuildRequired(_) => {
                Err(PortError::new(PortErrorCode::RebuildRequired))
            }
            ReaderOutcome::Unchanged(_) => AdapterBatch::new(
                &self.source,
                AdapterBatchParts {
                    observations: Box::default(),
                    relations: Box::default(),
                    chunk_proofs: ChunkProofBatch::new(None, Box::default())
                        .map_err(PortError::from)?,
                    next_checkpoint: checkpoint.clone(),
                    next_progress: source_progress(&reader_checkpoint)?,
                    state: BatchState::SnapshotEnd,
                    counters: AdapterCounters::default(),
                    diagnostics: AdapterDiagnostics::default(),
                },
            )
            .map_err(PortError::from),
            ReaderOutcome::Batch(mut batch) => {
                let mut diagnostics = AdapterDiagnostics::default();
                map_batch_diagnostics(&batch, &mut diagnostics)?;
                if has_blocking_input_diagnostic(&diagnostics) {
                    return Err(PortError::new(PortErrorCode::InvalidData));
                }
                let diagnostic_count = diagnostics.total().map_err(PortError::from)?;
                let chunk_proofs =
                    chunk_proofs(batch.previous_partial_chunk(), batch.source_chunks())?;
                let next_checkpoint = encode_checkpoint(batch.checkpoint().clone())?;
                let state = if batch.reached_snapshot_end() {
                    BatchState::SnapshotEnd
                } else {
                    BatchState::More
                };
                let counters = AdapterCounters::new(
                    1,
                    batch.bytes_read(),
                    u64::try_from(batch.events().len())
                        .map_err(|_| PortError::new(PortErrorCode::CapacityExceeded))?,
                    diagnostic_count,
                )
                .map_err(PortError::from)?;
                let result = AdapterBatch::new(
                    &self.source,
                    AdapterBatchParts {
                        observations: batch.events().to_vec().into_boxed_slice(),
                        relations: batch.relations().to_vec().into_boxed_slice(),
                        chunk_proofs,
                        next_checkpoint,
                        next_progress: source_progress(batch.checkpoint())?,
                        state,
                        counters,
                        diagnostics,
                    },
                )
                .map_err(PortError::from);
                if result.is_ok() {
                    self.latest_repository_activity_hint =
                        batch.take_latest_repository_activity_hint();
                    if let (Some(ingress), Some(hint)) = (
                        self.repository_hint_ingress.as_ref(),
                        self.latest_repository_activity_hint.as_ref(),
                    ) {
                        let _ = ingress.submit(hint.clone());
                    }
                }
                result
            }
        }
    }

    fn take_repository_activity_hint(
        &mut self,
    ) -> Option<tokenmaster_provider::RepositoryActivityHint> {
        self.latest_repository_activity_hint.take()
    }
}

fn discovered_source(descriptor: &SourceFileDescriptor) -> Result<DiscoveredSource, PortError> {
    let scope =
        ScopeIdentity::new("codex", descriptor.profile_id().as_str()).map_err(PortError::from)?;
    let identity = SourceIdentity::new(
        scope,
        descriptor.source_id().as_str(),
        *logical_file_identity(descriptor).as_bytes(),
    )
    .map_err(PortError::from)?;
    let kind = match descriptor.source_kind() {
        tokenmaster_provider::SourceKind::Active => SourceKind::Active,
        tokenmaster_provider::SourceKind::Direct => SourceKind::Direct,
        tokenmaster_provider::SourceKind::Archived => SourceKind::Archived,
    };
    Ok(DiscoveredSource::new(identity, kind))
}

pub(crate) fn encode_checkpoint(
    checkpoint: tokenmaster_codex::ReaderCheckpointV1,
) -> Result<AdapterCheckpoint, PortError> {
    let encoded = CodexCheckpointV1::new(checkpoint)
        .encode()
        .map_err(|_| PortError::new(PortErrorCode::CapacityExceeded))?;
    AdapterCheckpoint::new(encoded.into_boxed_slice()).map_err(PortError::from)
}

fn source_progress(
    checkpoint: &tokenmaster_codex::ReaderCheckpointV1,
) -> Result<AdapterSourceProgress, PortError> {
    AdapterSourceProgress::new(AdapterSourceProgressParts {
        schema_version: checkpoint.parser_schema_version(),
        physical_identity: checkpoint
            .physical_identity()
            .map(|identity| *identity.as_bytes()),
        logical_identity: *checkpoint.logical_identity().as_bytes(),
        committed_offset: checkpoint.committed_offset(),
        scan_offset: checkpoint.scan_offset(),
        observed_extent: checkpoint.observed_file_length(),
        modified_time_ns: checkpoint.modified_time_ns(),
        anchor_start: checkpoint.anchor().start(),
        anchor_len: checkpoint.anchor().len(),
        anchor_sha256: *checkpoint.anchor().sha256(),
        provider_resume: serde_json::to_vec(checkpoint.resume())
            .map_err(|_| PortError::new(PortErrorCode::InvalidData))?
            .into_boxed_slice(),
        discarding_oversized_record: checkpoint.discarding_oversized_line(),
        incomplete_tail: checkpoint.incomplete_tail(),
        verification: match checkpoint.verification() {
            tokenmaster_codex::VerificationLevel::Incremental => AdapterVerification::Incremental,
            tokenmaster_codex::VerificationLevel::FullPrefix => AdapterVerification::Full,
        },
    })
    .map_err(PortError::from)
}

fn completion(
    quality: CompletionQuality,
    files_read: u64,
    diagnostic: bool,
) -> Result<AdapterCompletion, PortError> {
    let mut diagnostics = AdapterDiagnostics::default();
    if diagnostic {
        diagnostics
            .record(AdapterDiagnosticCode::Other)
            .map_err(PortError::from)?;
    }
    let diagnostic_count = diagnostics.total().map_err(PortError::from)?;
    let counters =
        AdapterCounters::new(files_read, 0, 0, diagnostic_count).map_err(PortError::from)?;
    AdapterCompletion::new(quality, counters, diagnostics).map_err(PortError::from)
}

fn reader_port_error(code: ReaderErrorCode) -> PortError {
    let code = match code {
        ReaderErrorCode::Cancelled => PortErrorCode::Cancelled,
        ReaderErrorCode::CapacityExceeded => PortErrorCode::CapacityExceeded,
        ReaderErrorCode::OpenFailed | ReaderErrorCode::ReadFailed | ReaderErrorCode::SeekFailed => {
            PortErrorCode::Unavailable
        }
        ReaderErrorCode::SourceChanged => PortErrorCode::StaleState,
        ReaderErrorCode::InvalidDescriptor
        | ReaderErrorCode::NonRegular
        | ReaderErrorCode::ReparsePoint
        | ReaderErrorCode::CheckpointInvalid
        | ReaderErrorCode::AnchorMismatch
        | ReaderErrorCode::ResumeInvalid => PortErrorCode::InvalidData,
    };
    PortError::new(code)
}

fn chunk_proofs(
    previous: Option<SourceChunkDigest>,
    updates: &[SourceChunkDigest],
) -> Result<ChunkProofBatch, PortError> {
    let previous = previous
        .map(|proof| {
            ChunkProof::new(
                proof.index(),
                u64::from(proof.covered_len()),
                *proof.sha256(),
            )
        })
        .transpose()
        .map_err(PortError::from)?;
    let updates = updates
        .iter()
        .map(|proof| {
            ChunkProof::new(
                proof.index(),
                u64::from(proof.covered_len()),
                *proof.sha256(),
            )
            .map_err(PortError::from)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    ChunkProofBatch::new(previous, updates).map_err(PortError::from)
}

fn map_batch_diagnostics(
    batch: &tokenmaster_codex::ReadBatch,
    target: &mut AdapterDiagnostics,
) -> Result<(), PortError> {
    record_if(
        target,
        batch
            .diagnostics()
            .count(ReaderDiagnosticCode::IncompleteTail)
            > 0,
        AdapterDiagnosticCode::IncompleteInput,
    )?;
    record_if(
        target,
        batch
            .diagnostics()
            .count(ReaderDiagnosticCode::OversizedLine)
            > 0
            || batch
                .parser_diagnostics()
                .count(ParserDiagnosticCode::LineTooLarge)
                > 0,
        AdapterDiagnosticCode::OversizedInput,
    )?;
    let malformed = [
        ParserDiagnosticCode::MalformedJson,
        ParserDiagnosticCode::InvalidToken,
        ParserDiagnosticCode::InvalidTimestamp,
        ParserDiagnosticCode::InvalidModel,
        ParserDiagnosticCode::InvalidMetadata,
        ParserDiagnosticCode::InvalidPath,
    ]
    .into_iter()
    .any(|code| batch.parser_diagnostics().count(code) > 0);
    record_if(target, malformed, AdapterDiagnosticCode::MalformedInput)?;
    let other = [
        ParserDiagnosticCode::ZeroUsage,
        ParserDiagnosticCode::ModelFallback,
        ParserDiagnosticCode::MetadataTruncated,
        ParserDiagnosticCode::ToolCapacity,
    ]
    .into_iter()
    .any(|code| batch.parser_diagnostics().count(code) > 0);
    record_if(target, other, AdapterDiagnosticCode::Other)
}

fn has_blocking_input_diagnostic(diagnostics: &AdapterDiagnostics) -> bool {
    [
        AdapterDiagnosticCode::IncompleteInput,
        AdapterDiagnosticCode::MalformedInput,
        AdapterDiagnosticCode::OversizedInput,
    ]
    .into_iter()
    .any(|code| diagnostics.count(code) > 0)
}

fn record_if(
    target: &mut AdapterDiagnostics,
    condition: bool,
    code: AdapterDiagnosticCode,
) -> Result<(), PortError> {
    if condition {
        target.record(code).map_err(PortError::from)?;
    }
    Ok(())
}
