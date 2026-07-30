use serde::Serialize;

use crate::{CanonicalUsageEvent, ReplayEvidence};

pub const MAX_REPLAY_DEPTH: usize = 32;
pub const MAX_REPLAY_FANOUT: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayDisposition {
    Eligible,
    Replay,
    Pending,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionReplayState {
    Root,
    Matching,
    Diverged,
    Pending,
    Conflict,
}

#[derive(Clone, Copy)]
pub struct ReplayEventFacts<'a> {
    provider_id: &'a str,
    profile_id: &'a str,
    session_id: &'a str,
    parent_session_id: Option<&'a str>,
    session_ordinal: u64,
    replay_signature: &'a [u8; 32],
    evidence: ReplayEvidence,
    declared_conflict: bool,
}

impl<'a> ReplayEventFacts<'a> {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        provider_id: &'a str,
        profile_id: &'a str,
        session_id: &'a str,
        parent_session_id: Option<&'a str>,
        session_ordinal: u64,
        replay_signature: &'a [u8; 32],
        evidence: ReplayEvidence,
        declared_conflict: bool,
    ) -> Self {
        Self {
            provider_id,
            profile_id,
            session_id,
            parent_session_id,
            session_ordinal,
            replay_signature,
            evidence,
            declared_conflict,
        }
    }

    #[must_use]
    pub fn from_event(event: &'a CanonicalUsageEvent) -> Self {
        Self::new(
            event.provider_id().as_str(),
            event.profile_id().as_str(),
            event.session_id().as_str(),
            event
                .lineage()
                .parent_session_id()
                .map(|value| value.as_str()),
            event.lineage().session_ordinal(),
            event.replay_signature_bytes(),
            event.lineage().evidence(),
            event.lineage().declared_conflict(),
        )
    }

    #[must_use]
    pub const fn provider_id(self) -> &'a str {
        self.provider_id
    }

    #[must_use]
    pub const fn profile_id(self) -> &'a str {
        self.profile_id
    }

    #[must_use]
    pub const fn session_id(self) -> &'a str {
        self.session_id
    }

    #[must_use]
    pub const fn parent_session_id(self) -> Option<&'a str> {
        self.parent_session_id
    }

    #[must_use]
    pub const fn session_ordinal(self) -> u64 {
        self.session_ordinal
    }

    #[must_use]
    pub const fn replay_signature(self) -> &'a [u8; 32] {
        self.replay_signature
    }

    #[must_use]
    pub const fn evidence(self) -> ReplayEvidence {
        self.evidence
    }

    #[must_use]
    pub const fn declared_conflict(self) -> bool {
        self.declared_conflict
    }
}

#[derive(Clone, Copy)]
pub enum ParentOrdinal<'a> {
    NotApplicable,
    Present(ReplayEventFacts<'a>),
    MissingOpen,
    MissingComplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayTraversalFacts {
    depth: usize,
    direct_children: usize,
    cycle: bool,
    relation_conflict: bool,
}

impl ReplayTraversalFacts {
    #[must_use]
    pub const fn new(
        depth: usize,
        direct_children: usize,
        cycle: bool,
        relation_conflict: bool,
    ) -> Self {
        Self {
            depth,
            direct_children,
            cycle,
            relation_conflict,
        }
    }

    #[must_use]
    pub const fn depth(self) -> usize {
        self.depth
    }

    #[must_use]
    pub const fn direct_children(self) -> usize {
        self.direct_children
    }

    #[must_use]
    pub const fn cycle(self) -> bool {
        self.cycle
    }

    #[must_use]
    pub const fn relation_conflict(self) -> bool {
        self.relation_conflict
    }

    const fn exceeds_bound(self) -> bool {
        self.depth > MAX_REPLAY_DEPTH || self.direct_children > MAX_REPLAY_FANOUT
    }
}

#[derive(Clone, Copy)]
pub struct ReplayClassificationInput<'a> {
    prior_state: SessionReplayState,
    child: ReplayEventFacts<'a>,
    parent: ParentOrdinal<'a>,
    traversal: ReplayTraversalFacts,
}

impl<'a> ReplayClassificationInput<'a> {
    #[must_use]
    pub const fn new(
        prior_state: SessionReplayState,
        child: ReplayEventFacts<'a>,
        parent: ParentOrdinal<'a>,
        traversal: ReplayTraversalFacts,
    ) -> Self {
        Self {
            prior_state,
            child,
            parent,
            traversal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayClassification {
    disposition: ReplayDisposition,
    next_state: SessionReplayState,
}

impl ReplayClassification {
    #[must_use]
    pub const fn disposition(self) -> ReplayDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn next_state(self) -> SessionReplayState {
        self.next_state
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReplayClassifier;

impl ReplayClassifier {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn classify(&self, input: ReplayClassificationInput<'_>) -> ReplayClassification {
        if input.prior_state == SessionReplayState::Conflict
            || input.child.declared_conflict()
            || input.traversal.cycle
            || input.traversal.relation_conflict
            || !structurally_valid(&input)
        {
            return classification(ReplayDisposition::Conflict, SessionReplayState::Conflict);
        }
        if input.traversal.exceeds_bound() {
            return classification(ReplayDisposition::Pending, SessionReplayState::Pending);
        }

        match input.prior_state {
            SessionReplayState::Root => {
                classification(ReplayDisposition::Eligible, SessionReplayState::Root)
            }
            SessionReplayState::Matching => classify_matching(input),
            SessionReplayState::Diverged => {
                classification(ReplayDisposition::Eligible, SessionReplayState::Diverged)
            }
            SessionReplayState::Pending => {
                classification(ReplayDisposition::Pending, SessionReplayState::Pending)
            }
            SessionReplayState::Conflict => {
                classification(ReplayDisposition::Conflict, SessionReplayState::Conflict)
            }
        }
    }
}

fn structurally_valid(input: &ReplayClassificationInput<'_>) -> bool {
    let child = input.child;
    match (child.parent_session_id(), input.prior_state, input.parent) {
        (None, SessionReplayState::Root, ParentOrdinal::NotApplicable) => {
            input.traversal.depth == 0
        }
        (
            Some(parent_session_id),
            SessionReplayState::Matching
            | SessionReplayState::Diverged
            | SessionReplayState::Pending,
            ParentOrdinal::Present(parent),
        ) => {
            !parent.declared_conflict()
                && parent.provider_id() == child.provider_id()
                && parent.profile_id() == child.profile_id()
                && parent.session_id() == parent_session_id
                && parent.session_ordinal() == child.session_ordinal()
        }
        (
            Some(_),
            SessionReplayState::Matching
            | SessionReplayState::Diverged
            | SessionReplayState::Pending,
            ParentOrdinal::MissingOpen | ParentOrdinal::MissingComplete,
        ) => true,
        _ => false,
    }
}

fn classify_matching(input: ReplayClassificationInput<'_>) -> ReplayClassification {
    match input.parent {
        ParentOrdinal::Present(parent) => {
            if input.child.evidence() != ReplayEvidence::StrongCumulative
                || parent.evidence() != ReplayEvidence::StrongCumulative
            {
                return classification(ReplayDisposition::Pending, SessionReplayState::Matching);
            }
            if input.child.replay_signature() == parent.replay_signature() {
                classification(ReplayDisposition::Replay, SessionReplayState::Matching)
            } else {
                classification(ReplayDisposition::Eligible, SessionReplayState::Diverged)
            }
        }
        ParentOrdinal::MissingOpen => {
            classification(ReplayDisposition::Pending, SessionReplayState::Pending)
        }
        ParentOrdinal::MissingComplete => {
            classification(ReplayDisposition::Eligible, SessionReplayState::Diverged)
        }
        ParentOrdinal::NotApplicable => {
            classification(ReplayDisposition::Conflict, SessionReplayState::Conflict)
        }
    }
}

const fn classification(
    disposition: ReplayDisposition,
    next_state: SessionReplayState,
) -> ReplayClassification {
    ReplayClassification {
        disposition,
        next_state,
    }
}
