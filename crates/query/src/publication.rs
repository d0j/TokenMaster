use std::{cmp::Ordering, fmt};

use crate::{QueryEnvelope, SnapshotGeneration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    Accepted,
    Coalesced,
    RejectedOlder,
}

pub struct QuerySnapshotSlot<T> {
    current: Option<QueryEnvelope<T>>,
}

impl<T> QuerySnapshotSlot<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self { current: None }
    }

    pub fn publish(&mut self, candidate: QueryEnvelope<T>) -> PublishOutcome {
        let Some(current) = self.current.as_ref() else {
            self.current = Some(candidate);
            return PublishOutcome::Accepted;
        };
        match candidate
            .header()
            .snapshot_generation()
            .cmp(&current.header().snapshot_generation())
        {
            Ordering::Greater => {
                self.current = Some(candidate);
                PublishOutcome::Accepted
            }
            Ordering::Equal => PublishOutcome::Coalesced,
            Ordering::Less => PublishOutcome::RejectedOlder,
        }
    }

    #[must_use]
    pub const fn current(&self) -> Option<&QueryEnvelope<T>> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn generation(&self) -> Option<SnapshotGeneration> {
        self.current
            .as_ref()
            .map(|current| current.header().snapshot_generation())
    }
}

impl<T> Default for QuerySnapshotSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for QuerySnapshotSlot<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuerySnapshotSlot")
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}
