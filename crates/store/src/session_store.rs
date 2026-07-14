use std::path::Path;

use rusqlite::{Connection, params, types::Type};
use tokenmaster_domain::SessionSummary;

use crate::{StoreError, StoreErrorCode, schema::SCHEMA};

pub const EXPECTED_SQLITE_VERSION: &str = "3.53.2";
pub const MAX_PAGE_SIZE: usize = 256;
pub const MAX_SEED_SESSIONS: u64 = 1_000_000;

pub struct ProbeStore {
    connection: Connection,
}

impl ProbeStore {
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::initialize(Connection::open_in_memory()?)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::initialize(Connection::open(path)?)
    }

    fn initialize(connection: Connection) -> Result<Self, StoreError> {
        let store = Self { connection };
        let actual = store.sqlite_version()?;
        if actual != EXPECTED_SQLITE_VERSION {
            return Err(StoreError::new(StoreErrorCode::VersionMismatch));
        }
        store.connection.execute_batch(SCHEMA)?;
        Ok(store)
    }

    pub fn sqlite_version(&self) -> Result<String, StoreError> {
        Ok(self
            .connection
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))?)
    }

    pub fn seed_sessions(&mut self, count: u64) -> Result<(), StoreError> {
        if count > MAX_SEED_SESSIONS {
            return Err(StoreError::with_limit(
                StoreErrorCode::CapacityExceeded,
                MAX_SEED_SESSIONS,
            ));
        }

        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM session_probe", [])?;
        {
            let mut insert = transaction.prepare_cached(
                "INSERT INTO session_probe(id, started_at_ms, total_tokens, model_key) \
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for id in 1..=count {
                let id = i64::try_from(id).map_err(|_| {
                    StoreError::with_limit(StoreErrorCode::CapacityExceeded, MAX_SEED_SESSIONS)
                })?;
                insert.execute(params![
                    id,
                    id * 1_000,
                    id * 10,
                    format!("model-{}", id % 4)
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn session_count(&self) -> Result<u64, StoreError> {
        let count: i64 =
            self.connection
                .query_row("SELECT count(*) FROM session_probe", [], |row| row.get(0))?;
        u64::try_from(count).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
    }

    pub fn page_before(
        &self,
        before_id: Option<i64>,
        requested_size: usize,
    ) -> Result<Vec<SessionSummary>, StoreError> {
        let page_size = requested_size.clamp(1, MAX_PAGE_SIZE) as i64;
        let mut statement = self.connection.prepare_cached(
            "SELECT id, started_at_ms, total_tokens, model_key
             FROM session_probe
             WHERE (?1 IS NULL OR id < ?1)
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![before_id, page_size], |row| {
            let stored_tokens: i64 = row.get(2)?;
            let total_tokens = u64::try_from(stored_tokens).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(2, Type::Integer, Box::new(error))
            })?;
            Ok(SessionSummary {
                id: row.get(0)?,
                started_at_ms: row.get(1)?,
                total_tokens,
                model_key: row.get(3)?,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
