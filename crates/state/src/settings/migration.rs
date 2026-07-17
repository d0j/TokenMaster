use serde::Deserialize;

use super::value::{PortableSettings, SETTINGS_SCHEMA_VERSION};
use crate::StateError;
use crate::record::MAX_RECORD_PAYLOAD_BYTES;

#[derive(Deserialize)]
struct VersionProbe {
    schema_version: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableCandidateWire {
    schema_version: u16,
    portable: PortableSettings,
}

pub(super) fn decode_portable_candidate(bytes: &[u8]) -> Result<PortableSettings, StateError> {
    let len = u64::try_from(bytes.len()).map_err(|_| StateError::capacity_exceeded())?;
    if len > MAX_RECORD_PAYLOAD_BYTES {
        return Err(StateError::capacity_exceeded());
    }
    let probe: VersionProbe =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    match probe.schema_version {
        SETTINGS_SCHEMA_VERSION => migrate_v1(bytes),
        _ => Err(StateError::unsupported_version()),
    }
}

fn migrate_v1(bytes: &[u8]) -> Result<PortableSettings, StateError> {
    let wire: PortableCandidateWire =
        serde_json::from_slice(bytes).map_err(|_| StateError::invalid_input())?;
    if wire.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(StateError::unsupported_version());
    }
    Ok(wire.portable)
}
