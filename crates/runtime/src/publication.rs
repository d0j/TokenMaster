use tokenmaster_engine::RefreshOutcome;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EngineSnapshotGeneration(u64);

impl EngineSnapshotGeneration {
    const FIRST: Self = Self(1);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnginePublicationQuality {
    Empty,
    Complete,
    Partial,
    RecoveryPending,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EngineDiagnostics {
    completed_refreshes: u64,
    busy_refreshes: u64,
    cancelled_refreshes: u64,
    deadline_exceeded_refreshes: u64,
    failed_refreshes: u64,
    equal_archive_candidates: u64,
    older_archive_candidates: u64,
    counter_overflowed: bool,
}

impl EngineDiagnostics {
    #[must_use]
    pub const fn completed_refreshes(self) -> u64 {
        self.completed_refreshes
    }

    #[must_use]
    pub const fn busy_refreshes(self) -> u64 {
        self.busy_refreshes
    }

    #[must_use]
    pub const fn cancelled_refreshes(self) -> u64 {
        self.cancelled_refreshes
    }

    #[must_use]
    pub const fn deadline_exceeded_refreshes(self) -> u64 {
        self.deadline_exceeded_refreshes
    }

    #[must_use]
    pub const fn failed_refreshes(self) -> u64 {
        self.failed_refreshes
    }

    #[must_use]
    pub const fn equal_archive_candidates(self) -> u64 {
        self.equal_archive_candidates
    }

    #[must_use]
    pub const fn older_archive_candidates(self) -> u64 {
        self.older_archive_candidates
    }

    #[must_use]
    pub const fn counter_overflowed(self) -> bool {
        self.counter_overflowed
    }

    fn record_outcome(&mut self, outcome: RefreshOutcome) {
        let counter = match outcome {
            RefreshOutcome::Completed => &mut self.completed_refreshes,
            RefreshOutcome::Busy => &mut self.busy_refreshes,
            RefreshOutcome::Cancelled => &mut self.cancelled_refreshes,
            RefreshOutcome::DeadlineExceeded => &mut self.deadline_exceeded_refreshes,
            RefreshOutcome::Failed => &mut self.failed_refreshes,
        };
        checked_increment(counter, &mut self.counter_overflowed);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineSnapshot {
    generation: EngineSnapshotGeneration,
    archive_generation: u64,
    archive_revision: Option<u64>,
    scan_set_id: Option<u64>,
    data_through_ms: Option<i64>,
    quality: EnginePublicationQuality,
    diagnostics: EngineDiagnostics,
}

impl EngineSnapshot {
    #[must_use]
    pub const fn generation(self) -> EngineSnapshotGeneration {
        self.generation
    }

    #[must_use]
    pub const fn archive_generation(self) -> u64 {
        self.archive_generation
    }

    #[must_use]
    pub const fn archive_revision(self) -> Option<u64> {
        self.archive_revision
    }

    #[must_use]
    pub const fn scan_set_id(self) -> Option<u64> {
        self.scan_set_id
    }

    #[must_use]
    pub const fn data_through_ms(self) -> Option<i64> {
        self.data_through_ms
    }

    #[must_use]
    pub const fn quality(self) -> EnginePublicationQuality {
        self.quality
    }

    #[must_use]
    pub const fn diagnostics(self) -> EngineDiagnostics {
        self.diagnostics
    }

    #[must_use]
    pub const fn is_newer_than(self, current: Option<Self>) -> bool {
        match current {
            Some(current) => self.generation.0 > current.generation.0,
            None => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArchiveSnapshotCandidate {
    pub(crate) archive_generation: u64,
    pub(crate) archive_revision: Option<u64>,
    pub(crate) scan_set_id: Option<u64>,
    pub(crate) data_through_ms: Option<i64>,
    pub(crate) quality: EnginePublicationQuality,
}

pub(crate) struct EnginePublicationState {
    current: EngineSnapshot,
    diagnostics: EngineDiagnostics,
}

impl EnginePublicationState {
    pub(crate) fn seed(candidate: ArchiveSnapshotCandidate) -> Self {
        let diagnostics = EngineDiagnostics::default();
        Self {
            current: snapshot_from_candidate(
                EngineSnapshotGeneration::FIRST,
                candidate,
                diagnostics,
            ),
            diagnostics,
        }
    }

    pub(crate) const fn snapshot(&self) -> EngineSnapshot {
        self.current
    }

    pub(crate) fn record_outcome(&mut self, outcome: RefreshOutcome) {
        self.diagnostics.record_outcome(outcome);
    }

    pub(crate) fn publish(&mut self, candidate: ArchiveSnapshotCandidate) -> bool {
        if candidate.archive_generation < self.current.archive_generation {
            checked_increment(
                &mut self.diagnostics.older_archive_candidates,
                &mut self.diagnostics.counter_overflowed,
            );
            return false;
        }
        if candidate.archive_generation == self.current.archive_generation {
            checked_increment(
                &mut self.diagnostics.equal_archive_candidates,
                &mut self.diagnostics.counter_overflowed,
            );
            return false;
        }
        let Some(generation) = self.current.generation.checked_next() else {
            self.diagnostics.counter_overflowed = true;
            return false;
        };
        self.current = snapshot_from_candidate(generation, candidate, self.diagnostics);
        true
    }
}

fn snapshot_from_candidate(
    generation: EngineSnapshotGeneration,
    candidate: ArchiveSnapshotCandidate,
    diagnostics: EngineDiagnostics,
) -> EngineSnapshot {
    EngineSnapshot {
        generation,
        archive_generation: candidate.archive_generation,
        archive_revision: candidate.archive_revision,
        scan_set_id: candidate.scan_set_id,
        data_through_ms: candidate.data_through_ms,
        quality: candidate.quality,
        diagnostics,
    }
}

fn checked_increment(counter: &mut u64, overflowed: &mut bool) {
    match counter.checked_add(1) {
        Some(next) => *counter = next,
        None => *overflowed = true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(archive_generation: u64) -> ArchiveSnapshotCandidate {
        ArchiveSnapshotCandidate {
            archive_generation,
            archive_revision: Some(archive_generation + 10),
            scan_set_id: Some(archive_generation + 20),
            data_through_ms: Some(1_000 + archive_generation as i64),
            quality: EnginePublicationQuality::Complete,
        }
    }

    #[test]
    fn fixed_state_rejects_equal_and_older_candidates_without_generation_regression() {
        let mut state = EnginePublicationState::seed(candidate(4));
        let first = state.snapshot();
        assert_eq!(first.generation().get(), 1);
        assert!(!state.publish(candidate(4)));
        assert!(!state.publish(candidate(3)));
        assert_eq!(state.snapshot().generation(), first.generation());

        assert!(state.publish(candidate(5)));
        let second = state.snapshot();
        assert_eq!(second.generation().get(), 2);
        assert_eq!(second.archive_generation(), 5);
        assert_eq!(second.archive_revision(), Some(15));
        assert_eq!(second.scan_set_id(), Some(25));
        assert_eq!(second.data_through_ms(), Some(1_005));
        assert_eq!(second.quality(), EnginePublicationQuality::Complete);
        assert_eq!(second.diagnostics().equal_archive_candidates(), 1);
        assert_eq!(second.diagnostics().older_archive_candidates(), 1);
        assert!(second.is_newer_than(Some(first)));
        assert!(!first.is_newer_than(Some(second)));
    }

    #[test]
    fn ten_thousand_candidates_retain_one_snapshot_and_checked_counters() {
        let mut state = EnginePublicationState::seed(candidate(1));
        for _ in 0..10_000 {
            state.record_outcome(RefreshOutcome::Busy);
            assert!(!state.publish(candidate(1)));
        }
        assert_eq!(state.snapshot().generation().get(), 1);
        assert!(state.publish(candidate(2)));
        let snapshot = state.snapshot();
        assert_eq!(snapshot.diagnostics().busy_refreshes(), 10_000);
        assert_eq!(snapshot.diagnostics().equal_archive_candidates(), 10_000);
        assert!(!snapshot.diagnostics().counter_overflowed());
        assert!(std::mem::size_of::<EnginePublicationState>() <= 256);
    }

    #[test]
    fn generation_and_counter_overflow_fail_closed_without_wrapping() {
        let mut state = EnginePublicationState::seed(candidate(1));
        state.current.generation = EngineSnapshotGeneration(u64::MAX);
        state.diagnostics.failed_refreshes = u64::MAX;
        state.record_outcome(RefreshOutcome::Failed);
        assert!(!state.publish(candidate(2)));
        assert_eq!(state.snapshot().generation().get(), u64::MAX);
        assert_eq!(state.snapshot().archive_generation(), 1);
        assert!(state.diagnostics.counter_overflowed());
    }
}
