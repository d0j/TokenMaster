use std::{fmt, time::Instant, time::SystemTime, time::UNIX_EPOCH};

use crate::{QueryError, QueryErrorCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryTimeSample {
    wall_time_ms: i64,
    monotonic_ms: u64,
}

impl QueryTimeSample {
    #[must_use]
    pub const fn new(wall_time_ms: i64, monotonic_ms: u64) -> Self {
        Self {
            wall_time_ms,
            monotonic_ms,
        }
    }

    #[must_use]
    pub const fn wall_time_ms(self) -> i64 {
        self.wall_time_ms
    }

    #[must_use]
    pub const fn monotonic_ms(self) -> u64 {
        self.monotonic_ms
    }
}

pub trait QueryClock: Send + Sync {
    fn sample(&self) -> Result<QueryTimeSample, QueryError>;
}

pub struct SystemQueryClock {
    monotonic_origin: Instant,
}

impl SystemQueryClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            monotonic_origin: Instant::now(),
        }
    }
}

impl Default for SystemQueryClock {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryClock for SystemQueryClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        let wall_time_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| QueryError::new(QueryErrorCode::Unavailable))?
            .as_millis();
        let monotonic_ms = self.monotonic_origin.elapsed().as_millis();
        Ok(QueryTimeSample::new(
            i64::try_from(wall_time_ms).map_err(|_| QueryError::new(QueryErrorCode::Overflow))?,
            u64::try_from(monotonic_ms).map_err(|_| QueryError::new(QueryErrorCode::Overflow))?,
        ))
    }
}

impl fmt::Debug for SystemQueryClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SystemQueryClock([redacted])")
    }
}
