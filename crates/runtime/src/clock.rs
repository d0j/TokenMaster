use std::time::Instant;

use tokenmaster_engine::{Clock, MonotonicTime};

pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> MonotonicTime {
        let millis = u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        MonotonicTime::from_millis(millis)
    }
}

impl core::fmt::Debug for SystemClock {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SystemClock")
            .finish_non_exhaustive()
    }
}
