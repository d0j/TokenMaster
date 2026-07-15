use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{UsageStore, types::*, write::sql_u64};
use crate::{StoreError, StoreErrorCode};

impl UsageStore {
    pub fn begin_scan_set(
        &mut self,
        manifest: &ScanSetManifest,
        started_at_ms: i64,
    ) -> Result<ScanSetSnapshot, StoreError> {
        self.begin_scan_set_inner(manifest, started_at_ms, ScanFault::None)
    }

    fn begin_scan_set_inner(
        &mut self,
        manifest: &ScanSetManifest,
        started_at_ms: i64,
        _fault: ScanFault,
    ) -> Result<ScanSetSnapshot, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let running: i64 = transaction.query_row(
            "SELECT count(*) FROM usage_scan_set WHERE completion_state = 'running'",
            [],
            |row| row.get(0),
        )?;
        if running != 0 {
            return Err(StoreError::new(StoreErrorCode::ScanInProgress));
        }

        let max_set: Option<i64> =
            transaction.query_row("SELECT max(scan_set_id) FROM usage_scan_set", [], |row| {
                row.get(0)
            })?;
        let set_value = max_set
            .map_or(Some(0), |value| value.checked_add(1))
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let set_id = ScanSetId::from_stored(set_value)?;
        let scope_count = u64::try_from(manifest.scope_count())
            .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        transaction.execute(
            "INSERT INTO usage_scan_set(
               scan_set_id, started_at_ms, completion_state, expected_scope_count
             ) VALUES (?1, ?2, 'running', ?3)",
            params![set_id.as_sql()?, started_at_ms, sql_u64(scope_count)?],
        )?;
        #[cfg(test)]
        if _fault == ScanFault::AfterSetInsert {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }

        let max_scan: Option<i64> =
            transaction.query_row("SELECT max(scan_id) FROM usage_scan", [], |row| row.get(0))?;
        let first_scan = max_scan
            .map_or(Some(0), |value| value.checked_add(1))
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        let final_scan = first_scan
            .checked_add(
                i64::try_from(manifest.scope_count().saturating_sub(1))
                    .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?,
            )
            .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
        ScanId::from_stored(final_scan)?;
        for (index, scope) in manifest.scopes().iter().enumerate() {
            let scan_value = first_scan
                .checked_add(
                    i64::try_from(index)
                        .map_err(|_| StoreError::new(StoreErrorCode::CapacityExceeded))?,
                )
                .ok_or_else(|| StoreError::new(StoreErrorCode::CapacityExceeded))?;
            transaction.execute(
                "INSERT INTO usage_scan(
                   scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
                   completion_state
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'running')",
                params![
                    scan_value,
                    set_id.as_sql()?,
                    scope.provider_id(),
                    scope.profile_id(),
                    started_at_ms,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(ScanSetSnapshot {
            id: set_id,
            started_at_ms,
            completed_at_ms: None,
            outcome: None,
            expected_scope_count: scope_count,
        })
    }

    pub fn running_scan_set(&self) -> Result<Option<ScanSetSnapshot>, StoreError> {
        self.connection
            .query_row(
                "SELECT scan_set_id, started_at_ms, completed_at_ms,
                        completion_state, expected_scope_count
                 FROM usage_scan_set WHERE completion_state = 'running'",
                [],
                scan_set_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn scan_page(
        &self,
        scan_set_id: ScanSetId,
        after: Option<ScanId>,
        page_size: usize,
    ) -> Result<Box<[ScanSnapshot]>, StoreError> {
        load_scan_set(&self.connection, scan_set_id)?;
        let limit = i64::try_from(page_size.clamp(1, MAX_SCAN_SCOPES))
            .map_err(|_| StoreError::new(StoreErrorCode::InvalidValue))?;
        let after = after.map(ScanId::as_sql).transpose()?.unwrap_or(-1);
        let mut statement = self.connection.prepare(
            "SELECT scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
                    completed_at_ms, completion_state, sources_seen, files_read,
                    bytes_read, events_observed, diagnostics
             FROM usage_scan
             WHERE scan_set_id = ?1 AND scan_id > ?2
             ORDER BY scan_id
             LIMIT ?3",
        )?;
        let rows =
            statement.query_map(params![scan_set_id.as_sql()?, after, limit], scan_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
            .map_err(Into::into)
    }

    pub fn observe_scan_source(
        &mut self,
        scan_id: ScanId,
        source_key: SourceKey,
    ) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scan: Option<ObservedScanScope> = transaction
            .query_row(
                "SELECT scan.completion_state, scan.provider_id, scan.profile_id,
                        source.provider_id, source.profile_id
                 FROM usage_scan AS scan
                 LEFT JOIN usage_source AS source ON source.file_key = ?2
                 WHERE scan.scan_id = ?1",
                params![scan_id.as_sql()?, source_key.as_bytes().as_slice()],
                |row| {
                    Ok(ObservedScanScope {
                        state: row.get(0)?,
                        scan_provider: row.get(1)?,
                        scan_profile: row.get(2)?,
                        source_provider: row.get(3)?,
                        source_profile: row.get(4)?,
                    })
                },
            )
            .optional()?;
        let Some(scan) = scan else {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        };
        if scan.state != "running" {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        }
        if scan.source_provider.as_deref() != Some(scan.scan_provider.as_str())
            || scan.source_profile.as_deref() != Some(scan.scan_profile.as_str())
        {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let updated = transaction.execute(
            "UPDATE usage_source SET last_seen_scan_id = ?1 WHERE file_key = ?2",
            params![scan_id.as_sql()?, source_key.as_bytes().as_slice()],
        )?;
        if updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn finish_scan(
        &mut self,
        scan_id: ScanId,
        outcome: ScanOutcome,
        completed_at_ms: i64,
        counters: ScanCounters,
    ) -> Result<ScanSnapshot, StoreError> {
        self.finish_scan_inner(scan_id, outcome, completed_at_ms, counters, ScanFault::None)
    }

    fn finish_scan_inner(
        &mut self,
        scan_id: ScanId,
        outcome: ScanOutcome,
        completed_at_ms: i64,
        counters: ScanCounters,
        _fault: ScanFault,
    ) -> Result<ScanSnapshot, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = load_scan(&transaction, scan_id)?;
        if current.outcome.is_some() {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        }
        if completed_at_ms < current.started_at_ms {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let sources_seen: i64 = transaction.query_row(
            "SELECT count(*) FROM usage_source WHERE last_seen_scan_id = ?1",
            [scan_id.as_sql()?],
            |row| row.get(0),
        )?;
        if outcome == ScanOutcome::Complete {
            transaction.execute(
                "UPDATE usage_source SET
                   missing = CASE WHEN last_seen_scan_id = ?1 THEN 0 ELSE 1 END
                 WHERE provider_id = ?2 AND profile_id = ?3",
                params![
                    scan_id.as_sql()?,
                    current.scope.provider_id(),
                    current.scope.profile_id(),
                ],
            )?;
        }
        #[cfg(test)]
        if _fault == ScanFault::AfterPresenceFinalization {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        let updated = transaction.execute(
            "UPDATE usage_scan SET
               completed_at_ms = ?1, completion_state = ?2, sources_seen = ?3,
               files_read = ?4, bytes_read = ?5, events_observed = ?6,
               diagnostics = ?7
             WHERE scan_id = ?8 AND completion_state = 'running'",
            params![
                completed_at_ms,
                outcome.as_sql(),
                sources_seen,
                sql_u64(counters.files_read())?,
                sql_u64(counters.bytes_read())?,
                sql_u64(counters.events_observed())?,
                sql_u64(counters.diagnostics())?,
                scan_id.as_sql()?,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        }
        let snapshot = load_scan(&transaction, scan_id)?;
        transaction.commit()?;
        Ok(snapshot)
    }

    #[cfg(test)]
    fn begin_scan_set_with_fault(
        &mut self,
        manifest: &ScanSetManifest,
        started_at_ms: i64,
    ) -> Result<ScanSetSnapshot, StoreError> {
        self.begin_scan_set_inner(manifest, started_at_ms, ScanFault::AfterSetInsert)
    }

    #[cfg(test)]
    fn finish_scan_with_fault(
        &mut self,
        scan_id: ScanId,
        completed_at_ms: i64,
    ) -> Result<ScanSnapshot, StoreError> {
        self.finish_scan_inner(
            scan_id,
            ScanOutcome::Complete,
            completed_at_ms,
            ScanCounters::default(),
            ScanFault::AfterPresenceFinalization,
        )
    }

    pub fn finish_scan_set(
        &mut self,
        scan_set_id: ScanSetId,
        completed_at_ms: i64,
    ) -> Result<ScanSetSnapshot, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = load_scan_set(&transaction, scan_set_id)?;
        if current.outcome().is_some() {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        }
        if completed_at_ms < current.started_at_ms() {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let counts: (i64, i64, i64, i64, i64, i64, i64, Option<i64>) = transaction.query_row(
            "SELECT
                   count(*),
                   sum(completion_state = 'running'),
                   sum(completion_state = 'complete'),
                   sum(completion_state = 'partial'),
                   sum(completion_state = 'cancelled'),
                   sum(completion_state = 'failed'),
                   sum(completion_state = 'timed_out'),
                   max(completed_at_ms)
                 FROM usage_scan WHERE scan_set_id = ?1",
            [scan_set_id.as_sql()?],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )?;
        if stored_count(counts.0)? != current.expected_scope_count() {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        }
        if counts.1 != 0 {
            return Err(StoreError::new(StoreErrorCode::PendingScan));
        }
        if counts.7.is_some_and(|last| completed_at_ms < last) {
            return Err(StoreError::new(StoreErrorCode::InvalidValue));
        }
        let outcome = if counts.5 != 0 {
            ScanOutcome::Failed
        } else if counts.6 != 0 {
            ScanOutcome::TimedOut
        } else if counts.4 != 0 {
            ScanOutcome::Cancelled
        } else if counts.3 != 0 {
            ScanOutcome::Partial
        } else if stored_count(counts.2)? == current.expected_scope_count() {
            ScanOutcome::Complete
        } else {
            return Err(StoreError::new(StoreErrorCode::InvalidStoredValue));
        };
        let updated = transaction.execute(
            "UPDATE usage_scan_set SET completed_at_ms = ?1, completion_state = ?2
             WHERE scan_set_id = ?3 AND completion_state = 'running'",
            params![completed_at_ms, outcome.as_sql(), scan_set_id.as_sql()?],
        )?;
        if updated != 1 {
            return Err(StoreError::new(StoreErrorCode::StaleScan));
        }
        let snapshot = load_scan_set(&transaction, scan_set_id)?;
        transaction.commit()?;
        Ok(snapshot)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScanFault {
    None,
    #[cfg(test)]
    AfterSetInsert,
    #[cfg(test)]
    AfterPresenceFinalization,
}

struct ObservedScanScope {
    state: String,
    scan_provider: String,
    scan_profile: String,
    source_provider: Option<String>,
    source_profile: Option<String>,
}

fn load_scan_set(
    connection: &Connection,
    scan_set_id: ScanSetId,
) -> Result<ScanSetSnapshot, StoreError> {
    connection
        .query_row(
            "SELECT scan_set_id, started_at_ms, completed_at_ms,
                    completion_state, expected_scope_count
             FROM usage_scan_set WHERE scan_set_id = ?1",
            [scan_set_id.as_sql()?],
            scan_set_from_row,
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::StaleScan))
}

fn load_scan(connection: &Connection, scan_id: ScanId) -> Result<ScanSnapshot, StoreError> {
    connection
        .query_row(
            "SELECT scan_id, scan_set_id, provider_id, profile_id, started_at_ms,
                    completed_at_ms, completion_state, sources_seen, files_read,
                    bytes_read, events_observed, diagnostics
             FROM usage_scan WHERE scan_id = ?1",
            [scan_id.as_sql()?],
            scan_from_row,
        )
        .optional()?
        .ok_or_else(|| StoreError::new(StoreErrorCode::StaleScan))
}

fn scan_set_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanSetSnapshot> {
    let state: String = row.get(3)?;
    let outcome = outcome_from_state(&state).map_err(sql_conversion_error)?;
    let id = ScanSetId::from_stored(row.get(0)?).map_err(sql_conversion_error)?;
    let expected_scope_count = stored_count(row.get(4)?).map_err(sql_conversion_error)?;
    Ok(ScanSetSnapshot {
        id,
        started_at_ms: row.get(1)?,
        completed_at_ms: row.get(2)?,
        outcome,
        expected_scope_count,
    })
}

fn scan_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanSnapshot> {
    let state: String = row.get(6)?;
    let outcome = outcome_from_state(&state).map_err(sql_conversion_error)?;
    let id = ScanId::from_stored(row.get(0)?).map_err(sql_conversion_error)?;
    let scan_set_id = ScanSetId::from_stored(row.get(1)?).map_err(sql_conversion_error)?;
    let scope = ScanScope::new(row.get::<_, String>(2)?, row.get::<_, String>(3)?)
        .map_err(sql_conversion_error)?;
    let counters = ScanCounters::new(
        stored_count(row.get(8)?).map_err(sql_conversion_error)?,
        stored_count(row.get(9)?).map_err(sql_conversion_error)?,
        stored_count(row.get(10)?).map_err(sql_conversion_error)?,
        stored_count(row.get(11)?).map_err(sql_conversion_error)?,
    )
    .map_err(sql_conversion_error)?;
    Ok(ScanSnapshot {
        id,
        scan_set_id,
        scope,
        started_at_ms: row.get(4)?,
        completed_at_ms: row.get(5)?,
        outcome,
        sources_seen: stored_count(row.get(7)?).map_err(sql_conversion_error)?,
        counters,
    })
}

fn outcome_from_state(value: &str) -> Result<Option<ScanOutcome>, StoreError> {
    if value == "running" {
        Ok(None)
    } else {
        ScanOutcome::from_sql(value).map(Some)
    }
}

fn stored_count(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}

fn sql_conversion_error(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn manifest() -> Result<ScanSetManifest, StoreError> {
        ScanSetManifest::new(vec![ScanScope::new("codex", "default")?].into_boxed_slice())
    }

    #[test]
    fn begin_scan_set_fault_rolls_back_parent_and_children() -> TestResult {
        let mut store = UsageStore::in_memory()?;
        let error = match store.begin_scan_set_with_fault(&manifest()?, 1_000) {
            Ok(_) => return Err("injected begin failure unexpectedly committed".into()),
            Err(error) => error,
        };
        assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);

        let counts: (i64, i64) = store.connection.query_row(
            "SELECT
               (SELECT count(*) FROM usage_scan_set),
               (SELECT count(*) FROM usage_scan)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert_eq!(counts, (0, 0));
        assert!(store.running_scan_set()?.is_none());
        Ok(())
    }

    #[test]
    fn finish_scan_fault_rolls_back_presence_and_terminal_state() -> TestResult {
        let mut store = UsageStore::in_memory()?;
        let source_key = [7_u8; 32];
        store.connection.execute(
            "INSERT INTO usage_source(
               file_key, provider_id, profile_id, source_id, source_kind,
               logical_identity, current_generation, missing, diagnostic_count
             ) VALUES (?1, 'codex', 'default', 'fixture', 'active', ?2, NULL, 0, 0)",
            params![source_key.as_slice(), [8_u8; 32].as_slice()],
        )?;
        let set = store.begin_scan_set(&manifest()?, 2_000)?;
        let scan = store.scan_page(set.id(), None, 1)?[0].id();

        let error = match store.finish_scan_with_fault(scan, 2_010) {
            Ok(_) => return Err("injected finish failure unexpectedly committed".into()),
            Err(error) => error,
        };
        assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
        let missing: i64 = store.connection.query_row(
            "SELECT missing FROM usage_source WHERE file_key = ?1",
            [source_key.as_slice()],
            |row| row.get(0),
        )?;
        assert_eq!(missing, 0);
        assert_eq!(store.scan_page(set.id(), None, 1)?[0].outcome(), None);

        store.finish_scan(scan, ScanOutcome::Complete, 2_010, ScanCounters::default())?;
        let missing: i64 = store.connection.query_row(
            "SELECT missing FROM usage_source WHERE file_key = ?1",
            [source_key.as_slice()],
            |row| row.get(0),
        )?;
        assert_eq!(missing, 1);
        Ok(())
    }
}
