#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod error;
mod package;
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Task 3 record core is consumed by Task 4 typed stores"
    )
)]
mod record;
#[cfg(test)]
mod record_contract_tests;
mod settings;

pub use error::{StateError, StateErrorCode};
pub use package::{
    BackupCompression, BackupMetadata, BackupPackage, BackupPurpose, ConfigPackage,
    MAX_DATABASE_PACKAGE_BYTES, MAX_PACKAGE_ENTRIES, MAX_PACKAGE_MANIFEST_BYTES,
    MAX_PACKAGE_TOTAL_EXPANDED_BYTES, MAX_SETTINGS_PACKAGE_BYTES, PACKAGE_DECODER_WINDOW_BYTES,
    PACKAGE_IO_BUFFER_BYTES, PackageReceipt, VerifiedBackupPackage, VerifiedConfigPackage,
};
pub use settings::{
    BACKUP_INTERVAL_DEFAULT_SECONDS, BACKUP_INTERVAL_MAX_SECONDS, BACKUP_INTERVAL_MIN_SECONDS,
    BACKUP_QUIET_DEFAULT_SECONDS, BACKUP_QUIET_MAX_SECONDS, BACKUP_QUIET_MIN_SECONDS,
    BACKUP_RETENTION_DEFAULT_BYTES, BACKUP_RETENTION_MAX_BYTES, BACKUP_RETENTION_MIN_BYTES,
    BackupPolicy, DeviceRoute, DeviceSettings, PortableSettings, PortableSettingsCandidate,
    PortableSettingsDigest, PortableSettingsTarget, ReminderPolicy, SettingsChangeCategory,
    SettingsCommitReceipt, SettingsHealthCode, SettingsImportPreview, SettingsLoad,
    SettingsLoadOutcome, SettingsStore, SettingsValue,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteLimit(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ItemLimit(usize);

/// Immutable byte and item limits for bounded reliable-state inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateLimits {
    bytes: ByteLimit,
    items: ItemLimit,
}

impl StateLimits {
    /// Creates an exact inclusive byte/item limit pair.
    #[must_use]
    pub const fn new(max_bytes: u64, max_items: usize) -> Self {
        Self {
            bytes: ByteLimit(max_bytes),
            items: ItemLimit(max_items),
        }
    }

    /// Returns the inclusive byte limit.
    #[must_use]
    pub const fn max_bytes(self) -> u64 {
        self.bytes.0
    }

    /// Returns the inclusive item limit.
    #[must_use]
    pub const fn max_items(self) -> usize {
        self.items.0
    }

    /// Adds byte counts without overflow and rejects values above the limit.
    pub fn checked_bytes(self, current: u64, additional: u64) -> Result<u64, StateError> {
        let total = current
            .checked_add(additional)
            .ok_or_else(StateError::capacity_exceeded)?;
        if total > self.bytes.0 {
            return Err(StateError::capacity_exceeded());
        }
        Ok(total)
    }

    /// Adds item counts without overflow and rejects values above the limit.
    pub fn checked_items(self, current: usize, additional: usize) -> Result<usize, StateError> {
        let total = current
            .checked_add(additional)
            .ok_or_else(StateError::capacity_exceeded)?;
        if total > self.items.0 {
            return Err(StateError::capacity_exceeded());
        }
        Ok(total)
    }
}
