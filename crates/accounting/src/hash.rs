use sha2::{Digest, Sha256};
use tokenmaster_domain::{ObservationDraft, TokenCount, TokenUsage};

use crate::{EventFingerprint, ReplaySignature};

const EVENT_FINGERPRINT_DOMAIN: &[u8] = b"tokenmaster.event-fingerprint.v2\0";
const REPLAY_SIGNATURE_DOMAIN: &[u8] = b"tokenmaster.replay-signature.v1\0";

pub(super) fn event_fingerprint(draft: &ObservationDraft) -> EventFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(EVENT_FINGERPRINT_DOMAIN);
    update_text(&mut hasher, draft.provider_id().as_str());
    update_text(&mut hasher, draft.profile_id().as_str());
    update_text(&mut hasher, draft.session_id().as_str());
    hasher.update(draft.session_ordinal().to_be_bytes());
    update_text(&mut hasher, draft.model().as_str());
    update_usage(&mut hasher, draft.delta_usage());
    EventFingerprint::from_bytes(hasher.finalize().into())
}

pub(super) fn replay_signature(draft: &ObservationDraft) -> ReplaySignature {
    let mut hasher = Sha256::new();
    hasher.update(REPLAY_SIGNATURE_DOMAIN);
    update_text(&mut hasher, draft.model().as_str());
    update_usage(&mut hasher, draft.delta_usage());
    match draft.cumulative_usage() {
        Some(cumulative) => {
            hasher.update([1]);
            update_usage(&mut hasher, cumulative);
        }
        None => hasher.update([0]),
    }
    ReplaySignature::from_bytes(hasher.finalize().into())
}

fn update_text(hasher: &mut Sha256, value: &str) {
    let length = u32::try_from(value.len()).unwrap_or(u32::MAX);
    hasher.update(length.to_be_bytes());
    hasher.update(value.as_bytes());
}

fn update_usage(hasher: &mut Sha256, usage: &TokenUsage) {
    for count in [
        usage.input(),
        usage.cached(),
        usage.output(),
        usage.reasoning(),
        usage.total(),
    ] {
        update_count(hasher, count);
    }
}

fn update_count(hasher: &mut Sha256, count: TokenCount) {
    match count {
        TokenCount::Available(value) => {
            hasher.update([1]);
            hasher.update(value.to_be_bytes());
        }
        TokenCount::Unavailable => hasher.update([0]),
    }
}
