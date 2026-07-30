use crate::StateError;
use crate::settings::{MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION, SETTINGS_SCHEMA_VERSION};

use super::header::PackageKind;
use super::{BackupCompression, BackupPurpose, validate_package_time};

pub(crate) const MANIFEST_BYTES: usize = 40;
pub(crate) const ENTRY_PREFIX_BYTES: usize = 64;
pub(crate) const ENTRY_SUFFIX_BYTES: usize = 24;
pub(crate) const FOOTER_DIGEST_BYTES: usize = 32;
pub(crate) const FOOTER_MAGIC: &[u8; 8] = b"TMEND001";

const MANIFEST_MAGIC: &[u8; 8] = b"TMMNF001";
const ENTRY_MAGIC: &[u8; 8] = b"TMENTR01";
const ENTRY_END_MAGIC: &[u8; 8] = b"TMENEND1";
const MANIFEST_VERSION: u16 = 1;
const CODEC_ZSTD: u8 = 1;
const ENTRY_FLAGS: u8 = 0b0000_0011;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum EntryKind {
    Settings = 1,
    Database = 2,
}

impl EntryKind {
    const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Settings),
            2 => Some(Self::Database),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Manifest {
    pub(crate) kind: PackageKind,
    pub(crate) entry_count: u8,
    pub(crate) settings_schema_version: u16,
    pub(crate) database_schema_version: u16,
    pub(crate) compression: BackupCompression,
    pub(crate) created_at_utc_ms: i64,
    pub(crate) backup_purpose: Option<BackupPurpose>,
}

impl Manifest {
    pub(crate) fn new(
        kind: PackageKind,
        database_schema_version: u16,
        compression: BackupCompression,
        created_at_utc_ms: i64,
        backup_purpose: Option<BackupPurpose>,
    ) -> Result<Self, StateError> {
        if kind == PackageKind::Config && database_schema_version != 0 {
            return Err(StateError::invalid_input());
        }
        if kind == PackageKind::Backup && database_schema_version == 0 {
            return Err(StateError::invalid_input());
        }
        if (kind == PackageKind::Config && backup_purpose.is_some())
            || (kind == PackageKind::Backup && backup_purpose.is_none())
        {
            return Err(StateError::invalid_input());
        }
        validate_package_time(created_at_utc_ms)?;
        Ok(Self {
            kind,
            entry_count: kind.entry_count(),
            settings_schema_version: SETTINGS_SCHEMA_VERSION,
            database_schema_version,
            compression,
            created_at_utc_ms,
            backup_purpose,
        })
    }

    pub(crate) fn encode(self) -> [u8; MANIFEST_BYTES] {
        let mut bytes = [0_u8; MANIFEST_BYTES];
        bytes[0..8].copy_from_slice(MANIFEST_MAGIC);
        bytes[8..10].copy_from_slice(&MANIFEST_VERSION.to_le_bytes());
        bytes[10..12].copy_from_slice(&(MANIFEST_BYTES as u16).to_le_bytes());
        bytes[12] = self.kind as u8;
        bytes[13] = self.entry_count;
        bytes[14..16].copy_from_slice(&self.settings_schema_version.to_le_bytes());
        bytes[16..18].copy_from_slice(&self.database_schema_version.to_le_bytes());
        bytes[18] = self.compression as u8;
        bytes[19] = self.backup_purpose.map_or(0, |purpose| purpose as u8);
        bytes[20..28].copy_from_slice(&self.created_at_utc_ms.to_le_bytes());
        bytes
    }

    pub(crate) fn decode(bytes: &[u8; MANIFEST_BYTES]) -> Result<Self, StateError> {
        if &bytes[0..8] != MANIFEST_MAGIC {
            return Err(StateError::integrity());
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        let length = u16::from_le_bytes([bytes[10], bytes[11]]);
        if version != MANIFEST_VERSION || usize::from(length) != MANIFEST_BYTES {
            return Err(StateError::unsupported_version());
        }
        let kind = PackageKind::from_wire(bytes[12]).ok_or_else(StateError::unsupported_version)?;
        let settings_schema = u16::from_le_bytes([bytes[14], bytes[15]]);
        if !(MIN_SUPPORTED_SETTINGS_SCHEMA_VERSION..=SETTINGS_SCHEMA_VERSION)
            .contains(&settings_schema)
        {
            return Err(StateError::unsupported_version());
        }
        if bytes[28..40] != [0_u8; 12] {
            return Err(StateError::unsupported_version());
        }
        let backup_purpose = match kind {
            PackageKind::Config if bytes[19] == 0 => None,
            PackageKind::Config => return Err(StateError::unsupported_version()),
            PackageKind::Backup => Some(
                BackupPurpose::from_wire(bytes[19]).ok_or_else(StateError::unsupported_version)?,
            ),
        };
        let manifest = Self {
            kind,
            entry_count: bytes[13],
            settings_schema_version: settings_schema,
            database_schema_version: u16::from_le_bytes([bytes[16], bytes[17]]),
            compression: BackupCompression::from_wire(bytes[18])
                .ok_or_else(StateError::unsupported_version)?,
            created_at_utc_ms: i64::from_le_bytes(
                bytes[20..28]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ),
            backup_purpose,
        };
        Self::new(
            manifest.kind,
            manifest.database_schema_version,
            manifest.compression,
            manifest.created_at_utc_ms,
            manifest.backup_purpose,
        )
        .and_then(|expected| {
            if expected.entry_count == manifest.entry_count {
                Ok(manifest)
            } else {
                Err(StateError::invalid_input())
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EntryPrefix {
    pub(crate) kind: EntryKind,
    pub(crate) compression: BackupCompression,
    pub(crate) expanded_len: u64,
    pub(crate) expanded_sha256: [u8; 32],
}

impl EntryPrefix {
    pub(crate) const fn new(
        kind: EntryKind,
        compression: BackupCompression,
        expanded_len: u64,
        expanded_sha256: [u8; 32],
    ) -> Self {
        Self {
            kind,
            compression,
            expanded_len,
            expanded_sha256,
        }
    }

    pub(crate) fn encode(self) -> [u8; ENTRY_PREFIX_BYTES] {
        let mut bytes = [0_u8; ENTRY_PREFIX_BYTES];
        bytes[0..8].copy_from_slice(ENTRY_MAGIC);
        bytes[8] = self.kind as u8;
        bytes[9] = CODEC_ZSTD;
        bytes[10] = self.compression as u8;
        bytes[11] = ENTRY_FLAGS;
        bytes[12..16].copy_from_slice(&(ENTRY_PREFIX_BYTES as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(&self.expanded_len.to_le_bytes());
        bytes[24..56].copy_from_slice(&self.expanded_sha256);
        bytes[56..60].copy_from_slice(&super::PACKAGE_WINDOW_LOG.to_le_bytes());
        bytes
    }

    pub(crate) fn decode(bytes: &[u8; ENTRY_PREFIX_BYTES]) -> Result<Self, StateError> {
        if &bytes[0..8] != ENTRY_MAGIC {
            return Err(StateError::integrity());
        }
        let kind = EntryKind::from_wire(bytes[8]).ok_or_else(StateError::unsupported_version)?;
        if bytes[9] != CODEC_ZSTD
            || bytes[11] != ENTRY_FLAGS
            || u32::from_le_bytes(
                bytes[12..16]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ) != ENTRY_PREFIX_BYTES as u32
            || u32::from_le_bytes(
                bytes[56..60]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ) != super::PACKAGE_WINDOW_LOG
            || bytes[60..64] != [0_u8; 4]
        {
            return Err(StateError::unsupported_version());
        }
        Ok(Self {
            kind,
            compression: BackupCompression::from_wire(bytes[10])
                .ok_or_else(StateError::unsupported_version)?,
            expanded_len: u64::from_le_bytes(
                bytes[16..24]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ),
            expanded_sha256: bytes[24..56]
                .try_into()
                .map_err(|_| StateError::integrity())?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EntrySuffix {
    pub(crate) compressed_len: u64,
    pub(crate) expanded_len: u64,
}

impl EntrySuffix {
    pub(crate) const fn new(compressed_len: u64, expanded_len: u64) -> Self {
        Self {
            compressed_len,
            expanded_len,
        }
    }

    pub(crate) fn encode(self) -> [u8; ENTRY_SUFFIX_BYTES] {
        let mut bytes = [0_u8; ENTRY_SUFFIX_BYTES];
        bytes[0..8].copy_from_slice(ENTRY_END_MAGIC);
        bytes[8..16].copy_from_slice(&self.compressed_len.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.expanded_len.to_le_bytes());
        bytes
    }

    pub(crate) fn decode(bytes: &[u8; ENTRY_SUFFIX_BYTES]) -> Result<Self, StateError> {
        if &bytes[0..8] != ENTRY_END_MAGIC {
            return Err(StateError::integrity());
        }
        Ok(Self {
            compressed_len: u64::from_le_bytes(
                bytes[8..16]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ),
            expanded_len: u64::from_le_bytes(
                bytes[16..24]
                    .try_into()
                    .map_err(|_| StateError::integrity())?,
            ),
        })
    }
}
