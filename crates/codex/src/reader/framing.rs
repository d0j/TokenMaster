use std::io::BufRead;

use sha2::{Digest, Sha256};
use tokenmaster_domain::{ObservationDraft, ObservationVerification, SessionRelationDraft};

use super::{
    MAX_BATCH_COMPLETE_BYTES, MAX_BATCH_EVENTS, READ_BUFFER_BYTES, ReaderDiagnosticCode,
    ReaderDiagnostics, ReaderError, ReaderErrorCode,
};
use crate::{
    MAX_LINE_BYTES, ParseContext, ParseOutcome, ParserDiagnosticCode, ParserDiagnostics,
    ParserState, SourceFileDescriptor, parse_line,
};

pub(super) struct FramingResult {
    pub(super) state: ParserState,
    pub(super) events: Vec<ObservationDraft>,
    pub(super) relations: Vec<SessionRelationDraft>,
    pub(super) diagnostics: ReaderDiagnostics,
    pub(super) parser_diagnostics: ParserDiagnostics,
    pub(super) committed_offset: u64,
    pub(super) scan_offset: u64,
    pub(super) bytes_read: u64,
    pub(super) reached_snapshot_end: bool,
    pub(super) incomplete_tail: bool,
    pub(super) discarding_oversized_line: bool,
    pub(super) consumed_sha256: [u8; 32],
}

pub(super) struct FramingInput<'a> {
    pub(super) descriptor: &'a SourceFileDescriptor,
    pub(super) start_offset: u64,
    pub(super) committed_offset: u64,
    pub(super) state: ParserState,
    pub(super) snapshot_end_offset: u64,
    pub(super) discarding_oversized_line: bool,
    pub(super) source_verification: ObservationVerification,
}

pub(super) fn read_lines(
    reader: &mut impl BufRead,
    input: FramingInput<'_>,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<FramingResult, ReaderError> {
    let FramingInput {
        descriptor,
        start_offset,
        committed_offset: initial_committed_offset,
        mut state,
        snapshot_end_offset,
        discarding_oversized_line: initial_discarding_oversized_line,
        source_verification,
    } = input;
    let context = ParseContext::new(
        descriptor.profile_id().clone(),
        descriptor.source_id().clone(),
        descriptor.filename_session_hint().cloned(),
        descriptor.hashed_session_hint().clone(),
    )
    .with_source_verification(source_verification);
    let mut diagnostics = ReaderDiagnostics::default();
    let mut parser_diagnostics = ParserDiagnostics::new();
    let mut events = Vec::with_capacity(MAX_BATCH_EVENTS);
    let mut relations = Vec::with_capacity(MAX_BATCH_EVENTS);
    let mut line = Vec::with_capacity(READ_BUFFER_BYTES.min(MAX_LINE_BYTES));
    let mut discarding_oversized_line = initial_discarding_oversized_line;
    let mut absolute_offset = start_offset;
    let mut committed_offset = initial_committed_offset;
    let mut bytes_read = 0_u64;
    let mut complete_bytes = 0_u64;
    let mut current_line_batch_bytes = 0_u64;
    let mut consumed_hasher = Sha256::new();
    let reached_snapshot_end;
    let discard_read_limit = if initial_discarding_oversized_line {
        MAX_BATCH_COMPLETE_BYTES
    } else {
        u64::try_from(MAX_LINE_BYTES.saturating_add(READ_BUFFER_BYTES)).unwrap_or(u64::MAX)
    };

    loop {
        if should_cancel() {
            return Err(ReaderError::new(ReaderErrorCode::Cancelled));
        }
        let available = reader
            .fill_buf()
            .map_err(|_| ReaderError::new(ReaderErrorCode::ReadFailed))?;
        if available.is_empty() {
            reached_snapshot_end = true;
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(available.len());
        let required = line.len().saturating_add(content_len);
        let oversized_completion = if discarding_oversized_line {
            newline.is_some()
        } else if required > MAX_LINE_BYTES {
            let retain = MAX_LINE_BYTES.saturating_sub(line.len()).min(content_len);
            if retain > 0 {
                reserve_line(&mut line, retain)?;
                line.extend_from_slice(&available[..retain]);
                diagnostics.observe_line_bytes(line.len());
            }
            discarding_oversized_line = true;
            line = Vec::new();
            newline.is_some()
        } else {
            reserve_line(&mut line, content_len)?;
            line.extend_from_slice(&available[..content_len]);
            diagnostics.observe_line_bytes(line.len());
            false
        };
        let consumed = content_len.saturating_add(usize::from(newline.is_some()));
        let complete = newline.is_some();
        consumed_hasher.update(&available[..consumed]);
        reader.consume(consumed);
        let consumed_u64 = u64::try_from(consumed).unwrap_or(u64::MAX);
        absolute_offset = absolute_offset.saturating_add(consumed_u64);
        bytes_read = bytes_read.saturating_add(consumed_u64);
        current_line_batch_bytes = current_line_batch_bytes.saturating_add(consumed_u64);

        if complete {
            diagnostics.record(ReaderDiagnosticCode::CompleteLine);
            if oversized_completion {
                diagnostics.record(ReaderDiagnosticCode::OversizedLine);
                parser_diagnostics.record_line();
                parser_diagnostics.record(ParserDiagnosticCode::LineTooLarge);
                discarding_oversized_line = false;
            } else {
                let parse_bytes = if line.last() == Some(&b'\r') {
                    diagnostics.record(ReaderDiagnosticCode::CrlfLine);
                    &line[..line.len().saturating_sub(1)]
                } else {
                    line.as_slice()
                };
                if !parse_bytes.is_empty() {
                    match parse_line(
                        &context,
                        &mut state,
                        &mut parser_diagnostics,
                        committed_offset,
                        parse_bytes,
                    ) {
                        ParseOutcome::Emitted(event) => events.push(event),
                        ParseOutcome::SessionRelation(relation) => relations.push(relation),
                        ParseOutcome::MetadataOnly
                        | ParseOutcome::ToolOnly
                        | ParseOutcome::Skipped
                        | ParseOutcome::Rejected(_) => {}
                    }
                }
            }
            committed_offset = absolute_offset;
            complete_bytes = complete_bytes.saturating_add(current_line_batch_bytes);
            current_line_batch_bytes = 0;
            line.clear();
            if events.len().saturating_add(relations.len()) >= MAX_BATCH_EVENTS
                || complete_bytes >= MAX_BATCH_COMPLETE_BYTES
            {
                reached_snapshot_end = absolute_offset >= snapshot_end_offset;
                break;
            }
        } else if discarding_oversized_line && bytes_read >= discard_read_limit {
            reached_snapshot_end = absolute_offset >= snapshot_end_offset;
            break;
        }
    }

    let incomplete_tail = discarding_oversized_line || (reached_snapshot_end && !line.is_empty());
    if incomplete_tail {
        diagnostics.record(ReaderDiagnosticCode::IncompleteTail);
    }
    let scan_offset = if discarding_oversized_line {
        absolute_offset
    } else {
        committed_offset
    };
    Ok(FramingResult {
        state,
        events,
        relations,
        diagnostics,
        parser_diagnostics,
        committed_offset,
        scan_offset,
        bytes_read,
        reached_snapshot_end,
        incomplete_tail,
        discarding_oversized_line,
        consumed_sha256: consumed_hasher.finalize().into(),
    })
}

fn reserve_line(line: &mut Vec<u8>, additional: usize) -> Result<(), ReaderError> {
    let required = line.len().saturating_add(additional);
    if required > line.capacity() {
        line.try_reserve_exact(required.saturating_sub(line.len()))
            .map_err(|_| {
                ReaderError::with_limit(ReaderErrorCode::CapacityExceeded, MAX_LINE_BYTES as u64)
            })?;
    }
    Ok(())
}
