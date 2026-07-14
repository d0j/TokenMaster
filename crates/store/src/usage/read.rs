use rusqlite::{OptionalExtension, Params, Row, Statement, params};

use super::{UsageStore, types::*};
use crate::{StoreError, StoreErrorCode};

const FIRST_EVENT_PAGE_SQL: &str =
    "SELECT event_id, timestamp_seconds, timestamp_nanos, model, total_tokens, fingerprint
     FROM usage_event
     ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC
     LIMIT ?1";
const CURSOR_EVENT_PAGE_SQL: &str =
    "SELECT event_id, timestamp_seconds, timestamp_nanos, model, total_tokens, fingerprint
     FROM usage_event
     WHERE (timestamp_seconds, timestamp_nanos, fingerprint) < (?1, ?2, ?3)
     ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC
     LIMIT ?4";
const FIRST_LEGACY_EVENT_PAGE_SQL: &str =
    "SELECT event_id, timestamp_seconds, timestamp_nanos, model, total_tokens, fingerprint
     FROM usage_legacy_event
     WHERE snapshot_id = 1
     ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC
     LIMIT ?1";
const CURSOR_LEGACY_EVENT_PAGE_SQL: &str =
    "SELECT event_id, timestamp_seconds, timestamp_nanos, model, total_tokens, fingerprint
     FROM usage_legacy_event
     WHERE snapshot_id = 1
       AND (timestamp_seconds, timestamp_nanos, fingerprint) < (?1, ?2, ?3)
     ORDER BY timestamp_seconds DESC, timestamp_nanos DESC, fingerprint DESC
     LIMIT ?4";

#[derive(Clone, Copy)]
enum VisibleEventSource {
    Materialized,
    Legacy,
}

impl UsageStore {
    pub fn archive_state(&self) -> Result<ArchiveState, StoreError> {
        let current = self
            .connection
            .query_row(
                "SELECT
                   revision_id, canonicalizer_version, fingerprint_version,
                   replay_signature_version
                 FROM usage_replay_revision
                 WHERE status = 'current'",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let (staging_count, legacy_count): (i64, i64) = self.connection.query_row(
            "SELECT
               (SELECT count(*) FROM usage_replay_revision WHERE status = 'staging'),
               (SELECT count(*) FROM usage_legacy_snapshot WHERE snapshot_id = 1)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let rebuild_staging = boolean(staging_count)?;
        let has_legacy = boolean(legacy_count)?;

        let (mode, active_revision) = match current {
            Some((revision_id, canonicalizer, fingerprint, replay_signature)) => {
                let revision_id = ReplayRevisionId::from_stored(revision_id)?;
                let versions =
                    AccountingVersions::from_stored(canonicalizer, fingerprint, replay_signature)?;
                let mode = if versions == AccountingVersions::compiled() {
                    ArchiveMode::ReplayVerified
                } else {
                    ArchiveMode::ReplayVersionStale
                };
                (mode, Some(revision_id))
            }
            None if has_legacy => (ArchiveMode::LegacyUnverified, None),
            None => (ArchiveMode::Empty, None),
        };
        Ok(ArchiveState {
            mode,
            active_revision,
            rebuild_staging,
        })
    }

    pub fn replay_quality(
        &self,
        revision_id: ReplayRevisionId,
    ) -> Result<ReplayQualityCounts, StoreError> {
        let raw = self
            .connection
            .query_row(
                "SELECT
                   coalesce(sum(CASE WHEN o.disposition = 'eligible' THEN 1 ELSE 0 END), 0),
                   coalesce(sum(CASE WHEN o.disposition = 'replay' THEN 1 ELSE 0 END), 0),
                   coalesce(sum(CASE WHEN o.disposition = 'pending' THEN 1 ELSE 0 END), 0),
                   coalesce(sum(CASE WHEN o.disposition = 'conflict' THEN 1 ELSE 0 END), 0)
                 FROM usage_replay_revision AS r
                 LEFT JOIN usage_replay_observation AS o
                   ON o.revision_id = r.revision_id
                 WHERE r.revision_id = ?1
                 GROUP BY r.revision_id",
                [revision_id.as_sql()?],
                |row| {
                    Ok([
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ])
                },
            )
            .optional()?
            .ok_or_else(|| StoreError::new(StoreErrorCode::StaleRevision))?;
        Ok(ReplayQualityCounts {
            eligible: nonnegative(raw[0])?,
            replay: nonnegative(raw[1])?,
            pending: nonnegative(raw[2])?,
            conflict: nonnegative(raw[3])?,
        })
    }

    pub fn counts(&self) -> Result<UsageStoreCounts, StoreError> {
        let counts = self.connection.query_row(
            "SELECT
               (SELECT count(*) FROM usage_source),
               (SELECT count(*) FROM usage_generation),
               (SELECT count(*) FROM usage_observation),
               (SELECT count(*) FROM usage_event),
               (SELECT count(*) FROM usage_source_chunk),
               (SELECT count(*) FROM usage_scan)",
            [],
            |row| {
                Ok([
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ])
            },
        )?;
        let mut converted = [0_u64; 6];
        for (target, value) in converted.iter_mut().zip(counts) {
            *target = u64::try_from(value)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        }
        Ok(UsageStoreCounts {
            sources: converted[0],
            generations: converted[1],
            observations: converted[2],
            canonical_events: converted[3],
            chunks: converted[4],
            scans: converted[5],
        })
    }

    pub fn generation_snapshot(
        &self,
        source_key: SourceKey,
    ) -> Result<Option<GenerationSnapshot>, StoreError> {
        let raw = self
            .connection
            .query_row(
                "SELECT
                   g.generation, g.status, g.parser_schema_version,
                   g.physical_identity, g.logical_identity,
                   g.committed_offset, g.scan_offset, g.observed_file_length,
                   g.modified_time_ns, g.anchor_start, g.anchor_len, g.anchor_sha256,
                   g.resume_payload, g.discarding_oversized_line, g.incomplete_tail,
                   g.verification_level
                 FROM usage_source AS s
                 JOIN usage_generation AS g
                   ON g.file_key = s.file_key AND g.generation = s.current_generation
                 WHERE s.file_key = ?1",
                params![source_key.as_bytes().as_slice()],
                |row| {
                    Ok(RawGeneration {
                        generation: row.get(0)?,
                        status: row.get(1)?,
                        parser_schema_version: row.get(2)?,
                        physical_identity: row.get(3)?,
                        logical_identity: row.get(4)?,
                        committed_offset: row.get(5)?,
                        scan_offset: row.get(6)?,
                        observed_file_length: row.get(7)?,
                        modified_time_ns: row.get(8)?,
                        anchor_start: row.get(9)?,
                        anchor_len: row.get(10)?,
                        anchor_sha256: row.get(11)?,
                        resume: row.get(12)?,
                        discarding: row.get(13)?,
                        incomplete: row.get(14)?,
                        verification: row.get(15)?,
                    })
                },
            )
            .optional()?;
        raw.map(|raw| raw.validate(source_key)).transpose()
    }

    pub fn event_page_before(
        &self,
        before: Option<EventCursor>,
        requested_size: usize,
    ) -> Result<Vec<StoredUsageEvent>, StoreError> {
        let page_size = requested_size.clamp(1, MAX_USAGE_EVENT_PAGE_SIZE);
        let limit =
            i64::try_from(page_size).map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?;
        let source = if self.archive_state()?.mode() == ArchiveMode::LegacyUnverified {
            VisibleEventSource::Legacy
        } else {
            VisibleEventSource::Materialized
        };
        match (source, before) {
            (VisibleEventSource::Materialized, None) => {
                let mut statement = self.connection.prepare_cached(FIRST_EVENT_PAGE_SQL)?;
                query_events(&mut statement, params![limit], page_size)
            }
            (VisibleEventSource::Legacy, None) => {
                let mut statement = self
                    .connection
                    .prepare_cached(FIRST_LEGACY_EVENT_PAGE_SQL)?;
                query_events(&mut statement, params![limit], page_size)
            }
            (VisibleEventSource::Materialized, Some(cursor)) => query_event_page_before_cursor(
                &self.connection,
                CURSOR_EVENT_PAGE_SQL,
                cursor,
                limit,
                page_size,
            ),
            (VisibleEventSource::Legacy, Some(cursor)) => query_event_page_before_cursor(
                &self.connection,
                CURSOR_LEGACY_EVENT_PAGE_SQL,
                cursor,
                limit,
                page_size,
            ),
        }
    }
}

fn query_event_page_before_cursor(
    connection: &rusqlite::Connection,
    sql: &'static str,
    cursor: EventCursor,
    limit: i64,
    page_size: usize,
) -> Result<Vec<StoredUsageEvent>, StoreError> {
    let fingerprint = cursor.fingerprint();
    let mut statement = connection.prepare_cached(sql)?;
    query_events(
        &mut statement,
        params![
            cursor.timestamp_seconds(),
            i64::from(cursor.timestamp_nanos()),
            fingerprint.as_slice(),
            limit
        ],
        page_size,
    )
}

fn query_events(
    statement: &mut Statement<'_>,
    parameters: impl Params,
    page_size: usize,
) -> Result<Vec<StoredUsageEvent>, StoreError> {
    let rows = statement.query_map(parameters, raw_event)?;
    let mut events = Vec::with_capacity(page_size);
    for row in rows {
        events.push(row?.validate()?);
    }
    Ok(events)
}

fn raw_event(row: &Row<'_>) -> rusqlite::Result<RawEvent> {
    Ok(RawEvent {
        event_id: row.get(0)?,
        timestamp_seconds: row.get(1)?,
        timestamp_nanos: row.get(2)?,
        model: row.get(3)?,
        total_tokens: row.get(4)?,
        fingerprint: row.get(5)?,
    })
}

struct RawGeneration {
    generation: i64,
    status: String,
    parser_schema_version: i64,
    physical_identity: Option<Vec<u8>>,
    logical_identity: Vec<u8>,
    committed_offset: i64,
    scan_offset: i64,
    observed_file_length: i64,
    modified_time_ns: Option<i64>,
    anchor_start: i64,
    anchor_len: i64,
    anchor_sha256: Vec<u8>,
    resume: Vec<u8>,
    discarding: i64,
    incomplete: i64,
    verification: String,
}

impl RawGeneration {
    fn validate(self, source_key: SourceKey) -> Result<GenerationSnapshot, StoreError> {
        let physical_identity = self
            .physical_identity
            .map(|value| digest(value.as_slice()))
            .transpose()?;
        let checkpoint = StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: u16::try_from(self.parser_schema_version)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
            physical_identity,
            logical_identity: digest(&self.logical_identity)?,
            committed_offset: nonnegative(self.committed_offset)?,
            scan_offset: nonnegative(self.scan_offset)?,
            observed_file_length: nonnegative(self.observed_file_length)?,
            modified_time_ns: self.modified_time_ns,
            anchor_start: nonnegative(self.anchor_start)?,
            anchor_len: u16::try_from(self.anchor_len)
                .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
            anchor_sha256: digest(&self.anchor_sha256)?,
            resume: self.resume.into_boxed_slice(),
            discarding_oversized_line: boolean(self.discarding)?,
            incomplete_tail: boolean(self.incomplete)?,
            verification: StoredVerification::from_sql(&self.verification)?,
        })
        .map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))?;
        Ok(GenerationSnapshot {
            source_key,
            generation: nonnegative(self.generation)?,
            status: GenerationStatus::from_sql(&self.status)?,
            checkpoint,
        })
    }
}

struct RawEvent {
    event_id: String,
    timestamp_seconds: i64,
    timestamp_nanos: i64,
    model: String,
    total_tokens: Option<i64>,
    fingerprint: Vec<u8>,
}

impl RawEvent {
    fn validate(self) -> Result<StoredUsageEvent, StoreError> {
        if !valid_text(&self.event_id, 128) || !valid_text(&self.model, 64) {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        Ok(StoredUsageEvent {
            event_id: self.event_id.into_boxed_str(),
            timestamp_seconds: self.timestamp_seconds,
            timestamp_nanos: u32::try_from(self.timestamp_nanos)
                .ok()
                .filter(|value| *value < 1_000_000_000)
                .ok_or_else(|| StoreError::new(StoreErrorCode::InvalidStoredValue))?,
            model: self.model.into_boxed_str(),
            total_tokens: self.total_tokens.map(nonnegative).transpose()?,
            fingerprint: digest(&self.fingerprint)?,
        })
    }
}

fn digest(value: &[u8]) -> Result<[u8; 32], StoreError> {
    <[u8; 32]>::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn nonnegative(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn boolean(value: i64) -> Result<bool, StoreError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StoreError::new(StoreErrorCode::InvalidStoredValue)),
    }
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::{CURSOR_EVENT_PAGE_SQL, UsageStore};

    #[test]
    fn cursor_page_uses_composite_keyset_search() -> Result<(), Box<dyn std::error::Error>> {
        let store = UsageStore::in_memory()?;
        let explain = format!("EXPLAIN QUERY PLAN {CURSOR_EVENT_PAGE_SQL}");
        let mut statement = store.connection.prepare(&explain)?;
        let details = statement
            .query_map(params![0_i64, 0_i64, [0_u8; 32].as_slice(), 1_i64], |row| {
                row.get::<_, String>(3)
            })?;
        let mut uses_composite_search = false;
        for detail in details {
            let detail = detail?;
            uses_composite_search |=
                detail.contains("SEARCH usage_event USING INDEX usage_event_time_desc");
        }
        assert!(
            uses_composite_search,
            "cursor page must seek through the time index"
        );
        Ok(())
    }
}
