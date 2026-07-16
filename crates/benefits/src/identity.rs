use std::fmt;

use sha2::{Digest, Sha256};
use tokenmaster_domain::{BenefitLotId, BenefitScope, NotificationChannel, ReminderLeadTime};

use crate::BenefitRevision;

const SCOPE_DOMAIN: &[u8] = b"tokenmaster.benefit.scope.v1";
const CHANGE_DOMAIN: &[u8] = b"tokenmaster.benefit.change.v1";
const DELIVERY_DOMAIN: &[u8] = b"tokenmaster.benefit.delivery.v1";

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, PartialEq)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

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

opaque_id!(BenefitScopeId);
opaque_id!(BenefitChangeId);
opaque_id!(ReminderDeliveryId);

#[must_use]
pub fn benefit_scope_id(scope: &BenefitScope) -> BenefitScopeId {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, SCOPE_DOMAIN);
    update_field(&mut hasher, scope.provider_id().as_str().as_bytes());
    update_field(&mut hasher, scope.account_id().as_str().as_bytes());
    match scope.workspace_id() {
        Some(workspace_id) => {
            update_field(&mut hasher, &[1]);
            update_field(&mut hasher, workspace_id.as_str().as_bytes());
        }
        None => update_field(&mut hasher, &[0]),
    }
    BenefitScopeId::from_bytes(hasher.finalize().into())
}

pub(crate) fn benefit_change_id(
    scope_id: BenefitScopeId,
    sequence: u64,
    lot_id: BenefitLotId,
    revision: BenefitRevision,
    kind_code: u8,
) -> BenefitChangeId {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, CHANGE_DOMAIN);
    update_field(&mut hasher, scope_id.as_bytes());
    update_field(&mut hasher, &sequence.to_be_bytes());
    update_field(&mut hasher, lot_id.as_bytes());
    update_field(&mut hasher, &revision.get().to_be_bytes());
    update_field(&mut hasher, &[kind_code]);
    BenefitChangeId::from_bytes(hasher.finalize().into())
}

#[must_use]
pub fn reminder_delivery_id(
    scope: &BenefitScope,
    lot_id: BenefitLotId,
    revision: BenefitRevision,
    lead_time: ReminderLeadTime,
    channel: NotificationChannel,
) -> ReminderDeliveryId {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, DELIVERY_DOMAIN);
    update_field(&mut hasher, benefit_scope_id(scope).as_bytes());
    update_field(&mut hasher, lot_id.as_bytes());
    update_field(&mut hasher, &revision.get().to_be_bytes());
    update_field(&mut hasher, &lead_time.seconds().to_be_bytes());
    update_field(&mut hasher, &[notification_channel_code(channel)]);
    ReminderDeliveryId::from_bytes(hasher.finalize().into())
}

pub(crate) const fn notification_channel_code(channel: NotificationChannel) -> u8 {
    match channel {
        NotificationChannel::InApp => 1,
        NotificationChannel::OsScheduled => 2,
    }
}

fn update_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}
