use std::fmt;

use sha2::{Digest, Sha256};
use tokenmaster_domain::{QuotaObservationId, QuotaScope, QuotaWindowKey};

const SCOPE_DOMAIN: &[u8] = b"tokenmaster.quota.scope.v1";
const EPOCH_DOMAIN: &[u8] = b"tokenmaster.quota.epoch.v1";
const TRANSITION_DOMAIN: &[u8] = b"tokenmaster.quota.transition.v1";

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, PartialEq)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }
    };
}

opaque_id!(QuotaScopeId);
opaque_id!(QuotaEpochId);
opaque_id!(QuotaTransitionId);

#[must_use]
pub fn quota_scope_id(scope: &QuotaScope) -> QuotaScopeId {
    let workspace_presence = [u8::from(scope.workspace_id().is_some())];
    let mut hasher = Sha256::new();
    update_field(&mut hasher, SCOPE_DOMAIN);
    update_field(&mut hasher, scope.provider_id().as_str().as_bytes());
    update_field(&mut hasher, scope.account_id().as_str().as_bytes());
    update_field(&mut hasher, &workspace_presence);
    if let Some(workspace_id) = scope.workspace_id() {
        update_field(&mut hasher, workspace_id.as_str().as_bytes());
    }
    QuotaScopeId(hasher.finalize().into())
}

#[must_use]
pub fn quota_epoch_id(
    key: &QuotaWindowKey,
    definition_revision: u64,
    first_observation_id: QuotaObservationId,
) -> QuotaEpochId {
    let scope_id = quota_scope_id(key.scope());
    let mut hasher = Sha256::new();
    update_field(&mut hasher, EPOCH_DOMAIN);
    update_field(&mut hasher, scope_id.as_bytes());
    update_field(&mut hasher, key.window_id().as_str().as_bytes());
    update_field(&mut hasher, &definition_revision.to_be_bytes());
    update_field(&mut hasher, first_observation_id.as_bytes());
    QuotaEpochId(hasher.finalize().into())
}

pub(crate) struct TransitionIdentityInput<'a> {
    pub key: &'a QuotaWindowKey,
    pub definition_revision: u64,
    pub sequence: u64,
    pub kind_code: u8,
    pub previous_epoch_id: QuotaEpochId,
    pub current_epoch_id: QuotaEpochId,
    pub pre_observation_id: QuotaObservationId,
    pub post_observation_id: QuotaObservationId,
}

pub(crate) fn quota_transition_id(input: TransitionIdentityInput<'_>) -> QuotaTransitionId {
    let scope_id = quota_scope_id(input.key.scope());
    let kind_code = [input.kind_code];
    let mut hasher = Sha256::new();
    update_field(&mut hasher, TRANSITION_DOMAIN);
    update_field(&mut hasher, scope_id.as_bytes());
    update_field(&mut hasher, input.key.window_id().as_str().as_bytes());
    update_field(&mut hasher, &input.definition_revision.to_be_bytes());
    update_field(&mut hasher, &input.sequence.to_be_bytes());
    update_field(&mut hasher, &kind_code);
    update_field(&mut hasher, input.previous_epoch_id.as_bytes());
    update_field(&mut hasher, input.current_epoch_id.as_bytes());
    update_field(&mut hasher, input.pre_observation_id.as_bytes());
    update_field(&mut hasher, input.post_observation_id.as_bytes());
    QuotaTransitionId(hasher.finalize().into())
}

fn update_field(hasher: &mut Sha256, field: &[u8]) {
    let length = field.len() as u64;
    hasher.update(length.to_be_bytes());
    hasher.update(field);
}
