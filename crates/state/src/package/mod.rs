use core::fmt;

use crate::PortableSettingsCandidate;

mod capability;
mod encryption;
mod header;
mod manifest;
mod reader;
mod writer;

pub(crate) const CATALOG_HEADER_BYTES: usize = header::HEADER_BYTES + manifest::MANIFEST_BYTES;

pub(crate) struct CatalogPackageHeader {
    pub(crate) database_schema_version: u16,
    pub(crate) compression: BackupCompression,
    pub(crate) metadata: BackupMetadata,
}

pub(crate) fn decode_catalog_header(
    bytes: &[u8; CATALOG_HEADER_BYTES],
) -> Result<CatalogPackageHeader, crate::StateError> {
    let header_bytes = bytes[..header::HEADER_BYTES]
        .try_into()
        .map_err(|_| crate::StateError::integrity())?;
    let header = header::Header::decode(header_bytes)?;
    if header.kind != header::PackageKind::Backup
        || usize::try_from(header.manifest_len)
            .map_err(|_| crate::StateError::capacity_exceeded())?
            != manifest::MANIFEST_BYTES
    {
        return Err(crate::StateError::integrity());
    }
    let manifest_bytes = bytes[header::HEADER_BYTES..]
        .try_into()
        .map_err(|_| crate::StateError::integrity())?;
    let manifest = manifest::Manifest::decode(manifest_bytes)?;
    if manifest.kind != header.kind || manifest.entry_count != header.entry_count {
        return Err(crate::StateError::integrity());
    }
    Ok(CatalogPackageHeader {
        database_schema_version: manifest.database_schema_version,
        compression: manifest.compression,
        metadata: BackupMetadata::new(
            manifest.created_at_utc_ms,
            manifest
                .backup_purpose
                .ok_or_else(crate::StateError::integrity)?,
        )?,
    })
}

pub const MAX_PACKAGE_ENTRIES: usize = 8;
pub const MAX_PACKAGE_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_SETTINGS_PACKAGE_BYTES: u64 = 1024 * 1024;
pub const MAX_DATABASE_PACKAGE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_PACKAGE_TOTAL_EXPANDED_BYTES: u64 = MAX_DATABASE_PACKAGE_BYTES + 2 * 1024 * 1024;
pub const PACKAGE_DECODER_WINDOW_BYTES: u64 = 8 * 1024 * 1024;
pub const PACKAGE_IO_BUFFER_BYTES: usize = 64 * 1024;

pub(crate) const PACKAGE_WINDOW_LOG: u32 = 23;
pub(crate) const MAX_ENCODED_PACKAGE_BYTES: u64 = MAX_DATABASE_PACKAGE_BYTES + 2 * 1024 * 1024;
const MAX_PACKAGE_UTC_MS: i64 = 253_402_300_799_999;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BackupCompression {
    Automatic = 1,
    Normal = 2,
    Compact = 3,
}

impl BackupCompression {
    #[must_use]
    pub const fn level(self) -> i32 {
        match self {
            Self::Automatic => 6,
            Self::Normal => 12,
            Self::Compact => 19,
        }
    }

    pub(crate) const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Automatic),
            2 => Some(Self::Normal),
            3 => Some(Self::Compact),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BackupPurpose {
    Periodic = 1,
    Manual = 2,
    PreMigration = 3,
    PostMigration = 4,
    PreRestore = 5,
    PreDestructiveMaintenance = 6,
}

impl BackupPurpose {
    pub(crate) const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Periodic),
            2 => Some(Self::Manual),
            3 => Some(Self::PreMigration),
            4 => Some(Self::PostMigration),
            5 => Some(Self::PreRestore),
            6 => Some(Self::PreDestructiveMaintenance),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupMetadata {
    created_at_utc_ms: i64,
    purpose: BackupPurpose,
}

impl BackupMetadata {
    pub fn new(created_at_utc_ms: i64, purpose: BackupPurpose) -> Result<Self, crate::StateError> {
        validate_package_time(created_at_utc_ms)?;
        Ok(Self {
            created_at_utc_ms,
            purpose,
        })
    }

    #[must_use]
    pub const fn created_at_utc_ms(self) -> i64 {
        self.created_at_utc_ms
    }

    #[must_use]
    pub const fn purpose(self) -> BackupPurpose {
        self.purpose
    }
}

pub(crate) fn validate_package_time(created_at_utc_ms: i64) -> Result<(), crate::StateError> {
    if !(0..=MAX_PACKAGE_UTC_MS).contains(&created_at_utc_ms) {
        return Err(crate::StateError::invalid_input());
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PackageReceipt {
    package_len: u64,
    package_sha256: [u8; 32],
    file_sha256: [u8; 32],
}

impl PackageReceipt {
    pub(crate) const fn new(
        package_len: u64,
        package_sha256: [u8; 32],
        file_sha256: [u8; 32],
    ) -> Self {
        Self {
            package_len,
            package_sha256,
            file_sha256,
        }
    }

    #[must_use]
    pub const fn package_len(self) -> u64 {
        self.package_len
    }

    #[must_use]
    pub const fn package_sha256(&self) -> &[u8; 32] {
        &self.package_sha256
    }

    pub(crate) const fn file_sha256(&self) -> &[u8; 32] {
        &self.file_sha256
    }
}

impl fmt::Debug for PackageReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PackageReceipt([redacted])")
    }
}

pub struct VerifiedConfigPackage {
    settings: PortableSettingsCandidate,
    receipt: PackageReceipt,
    created_at_utc_ms: i64,
}

impl VerifiedConfigPackage {
    #[must_use]
    pub const fn settings(&self) -> &PortableSettingsCandidate {
        &self.settings
    }

    #[must_use]
    pub const fn receipt(&self) -> PackageReceipt {
        self.receipt
    }

    #[must_use]
    pub const fn created_at_utc_ms(&self) -> i64 {
        self.created_at_utc_ms
    }
}

impl fmt::Debug for VerifiedConfigPackage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VerifiedConfigPackage([redacted])")
    }
}

pub struct VerifiedBackupPackage {
    settings: PortableSettingsCandidate,
    receipt: PackageReceipt,
    database_schema_version: u16,
    database_len: u64,
    database_sha256: [u8; 32],
    compression: BackupCompression,
    metadata: BackupMetadata,
}

impl VerifiedBackupPackage {
    #[must_use]
    pub const fn settings(&self) -> &PortableSettingsCandidate {
        &self.settings
    }

    #[must_use]
    pub const fn receipt(&self) -> PackageReceipt {
        self.receipt
    }

    #[must_use]
    pub const fn database_schema_version(&self) -> u16 {
        self.database_schema_version
    }

    #[must_use]
    pub const fn database_len(&self) -> u64 {
        self.database_len
    }

    #[must_use]
    pub const fn database_sha256(&self) -> &[u8; 32] {
        &self.database_sha256
    }

    #[must_use]
    pub const fn compression(&self) -> BackupCompression {
        self.compression
    }

    #[must_use]
    pub const fn metadata(&self) -> BackupMetadata {
        self.metadata
    }
}

impl fmt::Debug for VerifiedBackupPackage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VerifiedBackupPackage([redacted])")
    }
}

pub struct ConfigPackage;

pub struct BackupPackage;

pub use encryption::{
    AGE_SCRYPT_LOG_N, BackupEncryptionContext, BackupPassphrase, EncryptedBackupPackage,
    MAX_BACKUP_PASSPHRASE_SCALARS, MIN_BACKUP_PASSPHRASE_SCALARS, ProtectedPackageReceipt,
};
