use std::fmt;

use serde::Serialize;
use tokenmaster_domain::{
    ActivityCounts, LongContextState, MetadataValue, ModelKey, ObservationDraft,
    ObservationVerification, ProjectAlias, TokenUsage, UsageProfileId, UsageProviderId,
    UsageSessionId, UsageSourceId, UtcTimestamp,
};

use crate::{CANONICALIZER_VERSION, EVENT_FINGERPRINT_VERSION, REPLAY_SIGNATURE_VERSION};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct EventFingerprint([u8; 32]);

impl EventFingerprint {
    pub(super) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        bytes_to_hex(&self.0)
    }
}

impl fmt::Debug for EventFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EventFingerprint([redacted])")
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ReplaySignature([u8; 32]);

impl ReplaySignature {
    pub(super) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        bytes_to_hex(&self.0)
    }
}

impl fmt::Debug for ReplaySignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ReplaySignature([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayEvidence {
    StrongCumulative,
    WeakUsageOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageLineage {
    parent_session_id: Option<UsageSessionId>,
    session_ordinal: u64,
    signature: ReplaySignature,
    evidence: ReplayEvidence,
    declared_conflict: bool,
}

impl UsageLineage {
    #[must_use]
    pub const fn parent_session_id(&self) -> Option<&UsageSessionId> {
        self.parent_session_id.as_ref()
    }

    #[must_use]
    pub const fn session_ordinal(&self) -> u64 {
        self.session_ordinal
    }

    #[must_use]
    pub const fn signature(&self) -> ReplaySignature {
        self.signature
    }

    #[must_use]
    pub const fn evidence(&self) -> ReplayEvidence {
        self.evidence
    }

    #[must_use]
    pub const fn declared_conflict(&self) -> bool {
        self.declared_conflict
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EventId(Box<str>);

impl EventId {
    fn from_fingerprint(fingerprint: EventFingerprint) -> Self {
        let mut value = String::with_capacity(26);
        value.push_str("event_");
        append_hex(&mut value, &fingerprint.0[..10]);
        Self(value.into_boxed_str())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalUsageEvent {
    draft: ObservationDraft,
    fingerprint: EventFingerprint,
    id: EventId,
    lineage: UsageLineage,
}

impl CanonicalUsageEvent {
    pub(super) fn from_draft(
        draft: ObservationDraft,
        fingerprint: EventFingerprint,
        signature: ReplaySignature,
        evidence: ReplayEvidence,
    ) -> Self {
        let id = EventId::from_fingerprint(fingerprint);
        let lineage = UsageLineage {
            parent_session_id: draft.parent_session_id().cloned(),
            session_ordinal: draft.session_ordinal(),
            signature,
            evidence,
            declared_conflict: draft.lineage_conflict(),
        };
        Self {
            draft,
            fingerprint,
            id,
            lineage,
        }
    }

    #[must_use]
    pub const fn canonicalizer_version(&self) -> u16 {
        CANONICALIZER_VERSION
    }

    #[must_use]
    pub const fn fingerprint_version(&self) -> u16 {
        EVENT_FINGERPRINT_VERSION
    }

    #[must_use]
    pub const fn replay_signature_version(&self) -> u16 {
        REPLAY_SIGNATURE_VERSION
    }

    #[must_use]
    pub const fn provider_id(&self) -> &UsageProviderId {
        self.draft.provider_id()
    }

    #[must_use]
    pub const fn profile_id(&self) -> &UsageProfileId {
        self.draft.profile_id()
    }

    #[must_use]
    pub const fn session_id(&self) -> &UsageSessionId {
        self.draft.session_id()
    }

    #[must_use]
    pub const fn source_id(&self) -> &UsageSourceId {
        self.draft.source_id()
    }

    #[must_use]
    pub const fn source_offset(&self) -> u64 {
        self.draft.source_offset()
    }

    #[must_use]
    pub const fn source_verification(&self) -> ObservationVerification {
        self.draft.source_verification()
    }

    #[must_use]
    pub const fn timestamp(&self) -> &UtcTimestamp {
        self.draft.timestamp()
    }

    #[must_use]
    pub const fn model(&self) -> &ModelKey {
        self.draft.model()
    }

    #[must_use]
    pub fn raw_model(&self) -> Option<&MetadataValue> {
        self.draft.raw_model()
    }

    #[must_use]
    pub const fn delta_usage(&self) -> &TokenUsage {
        self.draft.delta_usage()
    }

    #[must_use]
    pub const fn usage(&self) -> &TokenUsage {
        self.draft.delta_usage()
    }

    #[must_use]
    pub const fn cumulative_usage(&self) -> Option<&TokenUsage> {
        self.draft.cumulative_usage()
    }

    #[must_use]
    pub const fn fallback_model(&self) -> bool {
        self.draft.fallback_model()
    }

    #[must_use]
    pub const fn long_context(&self) -> LongContextState {
        self.draft.long_context()
    }

    #[must_use]
    pub fn service_tier(&self) -> Option<&MetadataValue> {
        self.draft.service_tier()
    }

    #[must_use]
    pub fn project(&self) -> Option<&ProjectAlias> {
        self.draft.project()
    }

    #[must_use]
    pub fn originator(&self) -> Option<&MetadataValue> {
        self.draft.originator()
    }

    #[must_use]
    pub const fn activity(&self) -> &ActivityCounts {
        self.draft.activity()
    }

    #[must_use]
    pub const fn fingerprint(&self) -> &EventFingerprint {
        &self.fingerprint
    }

    #[must_use]
    pub const fn id(&self) -> &EventId {
        &self.id
    }

    #[must_use]
    pub const fn lineage(&self) -> &UsageLineage {
        &self.lineage
    }

    pub(crate) const fn replay_signature_bytes(&self) -> &[u8; 32] {
        self.lineage.signature.as_bytes()
    }
}

impl fmt::Debug for CanonicalUsageEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalUsageEvent")
            .field("provider_id", &self.provider_id())
            .field("profile_id", &self.profile_id())
            .field("session_id", &self.session_id())
            .field("session_ordinal", &self.lineage.session_ordinal)
            .field("source", &"[redacted]")
            .field("fingerprint", &self.fingerprint)
            .field("replay_signature", &self.lineage.signature)
            .finish()
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len().saturating_mul(2));
    append_hex(&mut value, bytes);
    value
}

fn append_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}
