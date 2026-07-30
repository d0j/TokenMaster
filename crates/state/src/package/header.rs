use crate::StateError;

use super::{MAX_PACKAGE_ENTRIES, MAX_PACKAGE_MANIFEST_BYTES, MAX_PACKAGE_TOTAL_EXPANDED_BYTES};

pub(crate) const HEADER_BYTES: usize = 32;
const MAGIC: &[u8; 8] = b"TMPKG001";
const VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum PackageKind {
    Config = 1,
    Backup = 2,
}

impl PackageKind {
    pub(crate) const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Config),
            2 => Some(Self::Backup),
            _ => None,
        }
    }

    pub(crate) const fn entry_count(self) -> u8 {
        match self {
            Self::Config => 1,
            Self::Backup => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Header {
    pub(crate) kind: PackageKind,
    pub(crate) entry_count: u8,
    pub(crate) manifest_len: u32,
    pub(crate) total_expanded: u64,
}

impl Header {
    pub(crate) fn new(
        kind: PackageKind,
        manifest_len: usize,
        total_expanded: u64,
    ) -> Result<Self, StateError> {
        let manifest_len =
            u32::try_from(manifest_len).map_err(|_| StateError::capacity_exceeded())?;
        let header = Self {
            kind,
            entry_count: kind.entry_count(),
            manifest_len,
            total_expanded,
        };
        header.validate()?;
        Ok(header)
    }

    pub(crate) fn encode(self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0_u8; HEADER_BYTES];
        bytes[0..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
        bytes[10..12].copy_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
        bytes[12] = self.kind as u8;
        bytes[13] = self.entry_count;
        bytes[16..20].copy_from_slice(&self.manifest_len.to_le_bytes());
        bytes[20..28].copy_from_slice(&self.total_expanded.to_le_bytes());
        bytes
    }

    pub(crate) fn decode(bytes: &[u8; HEADER_BYTES]) -> Result<Self, StateError> {
        if &bytes[0..8] != MAGIC {
            return Err(StateError::integrity());
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != VERSION {
            return Err(StateError::unsupported_version());
        }
        let header_len = u16::from_le_bytes([bytes[10], bytes[11]]);
        if usize::from(header_len) != HEADER_BYTES {
            return Err(StateError::unsupported_version());
        }
        let kind = PackageKind::from_wire(bytes[12]).ok_or_else(StateError::unsupported_version)?;
        let flags = u16::from_le_bytes([bytes[14], bytes[15]]);
        if flags != 0 || bytes[28..32] != [0_u8; 4] {
            return Err(StateError::unsupported_version());
        }
        let header = Self {
            kind,
            entry_count: bytes[13],
            manifest_len: u32::from_le_bytes(
                bytes[16..20]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ),
            total_expanded: u64::from_le_bytes(
                bytes[20..28]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ),
        };
        header.validate()?;
        Ok(header)
    }

    fn validate(self) -> Result<(), StateError> {
        if usize::from(self.entry_count) > MAX_PACKAGE_ENTRIES {
            return Err(StateError::capacity_exceeded());
        }
        if self.entry_count != self.kind.entry_count() {
            return Err(StateError::invalid_input());
        }
        if usize::try_from(self.manifest_len).map_err(|_| StateError::capacity_exceeded())?
            > MAX_PACKAGE_MANIFEST_BYTES
        {
            return Err(StateError::capacity_exceeded());
        }
        if self.total_expanded > MAX_PACKAGE_TOTAL_EXPANDED_BYTES {
            return Err(StateError::capacity_exceeded());
        }
        Ok(())
    }
}
