use crate::QueryError;

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
