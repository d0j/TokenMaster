use core::fmt;

use sha2::{Digest, Sha256};
use tokenmaster_platform::{
    BackupDirectory, BackupDirectoryEntry, BackupDirectoryError, BackupDirectoryGeneration,
    DurableFileReader, MAX_DURABLE_FILE_BYTES,
};

use crate::package::{CATALOG_HEADER_BYTES, CatalogPackageHeader, decode_catalog_header};
use crate::{
    BackupCompression, BackupPackage, BackupPurpose, PACKAGE_IO_BUFFER_BYTES, StateError,
    StateErrorCode, VerifiedBackupPackage,
};

/// Checked process-local identity for one immutable catalog projection.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CatalogGeneration(u64);

impl CatalogGeneration {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for CatalogGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CatalogGeneration")
            .field(&self.0)
            .finish()
    }
}

/// Generation-bound, path-free UI selection for one catalog point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogSelection {
    generation: CatalogGeneration,
    ordinal: u8,
}

impl CatalogSelection {
    #[must_use]
    pub const fn generation(self) -> CatalogGeneration {
        self.generation
    }

    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }
}

/// Bounded truth available without opening uncompressed database content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogHealth {
    Corrupt,
    HeaderValid,
    Verified,
}

/// One public path- and digest-free restore-point summary.
pub struct CatalogPoint {
    pub(crate) selection: CatalogSelection,
    pub(crate) entry: BackupDirectoryEntry,
    pub(crate) observed_file_sha256: [u8; 32],
    pub(crate) created_at_utc_ms: Option<i64>,
    pub(crate) purpose: Option<BackupPurpose>,
    pub(crate) database_schema_version: Option<u16>,
    pub(crate) compression: Option<BackupCompression>,
    pub(crate) health: CatalogHealth,
}

impl CatalogPoint {
    #[must_use]
    pub const fn selection(&self) -> CatalogSelection {
        self.selection
    }

    #[must_use]
    pub const fn created_at_utc_ms(&self) -> Option<i64> {
        self.created_at_utc_ms
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.entry.len()
    }

    #[must_use]
    pub const fn purpose(&self) -> Option<BackupPurpose> {
        self.purpose
    }

    #[must_use]
    pub const fn database_schema_version(&self) -> Option<u16> {
        self.database_schema_version
    }

    #[must_use]
    pub const fn compression(&self) -> Option<BackupCompression> {
        self.compression
    }

    #[must_use]
    pub const fn health(&self) -> CatalogHealth {
        self.health
    }
}

impl fmt::Debug for CatalogPoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CatalogPoint")
            .field("selection", &self.selection)
            .field("created_at_utc_ms", &self.created_at_utc_ms)
            .field("size_bytes", &self.size_bytes())
            .field("purpose", &self.purpose)
            .field("database_schema_version", &self.database_schema_version)
            .field("compression", &self.compression)
            .field("health", &self.health)
            .finish()
    }
}

/// Disposable bounded projection rebuilt from self-describing package bytes.
pub struct BackupCatalog {
    generation: CatalogGeneration,
    directory_identity: CatalogDirectoryIdentity,
    points: Vec<CatalogPoint>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CatalogDirectoryIdentity(BackupDirectoryGeneration);

impl BackupCatalog {
    /// Rebuilds from the exact controlled directory and carries only unchanged proofs.
    pub fn rebuild(
        directory: &BackupDirectory,
        previous: Option<&Self>,
    ) -> Result<Self, StateError> {
        let generation = next_generation(previous.map(|catalog| catalog.generation))?;
        let snapshot = directory.scan().map_err(map_directory_error)?;
        let mut points = Vec::with_capacity(snapshot.entries().len());

        for entry in snapshot.entries() {
            let (header, observed_file_sha256) = inspect_header(directory, entry)?;
            if points
                .iter()
                .any(|point: &CatalogPoint| point.observed_file_sha256 == observed_file_sha256)
            {
                return Err(StateError::integrity());
            }
            let carried_verified = previous.is_some_and(|catalog| {
                catalog.points.iter().any(|point| {
                    point.health == CatalogHealth::Verified
                        && point.entry == *entry
                        && point.observed_file_sha256 == observed_file_sha256
                })
            });
            let (created_at_utc_ms, purpose, database_schema_version, compression, health) =
                match header {
                    Ok(header) => (
                        Some(header.metadata.created_at_utc_ms()),
                        Some(header.metadata.purpose()),
                        Some(header.database_schema_version),
                        Some(header.compression),
                        if carried_verified {
                            CatalogHealth::Verified
                        } else {
                            CatalogHealth::HeaderValid
                        },
                    ),
                    Err(_) => (None, None, None, None, CatalogHealth::Corrupt),
                };
            points.push(CatalogPoint {
                selection: CatalogSelection {
                    generation,
                    ordinal: 0,
                },
                entry: entry.clone(),
                observed_file_sha256,
                created_at_utc_ms,
                purpose,
                database_schema_version,
                compression,
                health,
            });
        }

        points.sort_unstable_by(|left, right| {
            right
                .created_at_utc_ms
                .unwrap_or(i64::MIN)
                .cmp(&left.created_at_utc_ms.unwrap_or(i64::MIN))
                .then_with(|| left.entry.ordinal().cmp(&right.entry.ordinal()))
        });
        for (ordinal, point) in points.iter_mut().enumerate() {
            point.selection.ordinal =
                u8::try_from(ordinal).map_err(|_| StateError::capacity_exceeded())?;
        }

        Ok(Self {
            generation,
            directory_identity: CatalogDirectoryIdentity(snapshot.generation()),
            points,
        })
    }

    #[must_use]
    pub const fn generation(&self) -> CatalogGeneration {
        self.generation
    }

    #[must_use]
    pub fn points(&self) -> &[CatalogPoint] {
        &self.points
    }

    /// Fully verifies every header-valid package in one unchanged controlled directory.
    /// Corrupt package bytes remain visible as corrupt catalog points; transient or
    /// ambiguous directory failures stop the pass without fabricating verification.
    pub fn verify_all_packages(
        &mut self,
        directory: &BackupDirectory,
    ) -> Result<usize, StateError> {
        if !self.matches_directory(directory)? {
            return Err(StateError::recovery_required());
        }
        let selections = self
            .points
            .iter()
            .filter(|point| point.health == CatalogHealth::HeaderValid)
            .map(|point| point.selection)
            .collect::<Vec<_>>();
        for selection in selections {
            let result = (|| {
                let mut reader = self.open_recovery_selection(directory, selection)?;
                let verified = BackupPackage::inspect(&mut reader)?;
                self.bind_verified(selection, &verified)
            })();
            if let Err(error) = result {
                if matches!(
                    error.code(),
                    StateErrorCode::InvalidInput
                        | StateErrorCode::UnsupportedVersion
                        | StateErrorCode::CapacityExceeded
                        | StateErrorCode::Integrity
                ) {
                    let point = self
                        .points
                        .get_mut(usize::from(selection.ordinal))
                        .filter(|point| point.selection == selection)
                        .ok_or_else(StateError::internal_invariant)?;
                    point.health = CatalogHealth::Corrupt;
                    continue;
                }
                return Err(error);
            }
        }
        Ok(self
            .points
            .iter()
            .filter(|point| point.health == CatalogHealth::Verified)
            .count())
    }

    /// Binds one exact current full-package proof without exposing its digest.
    pub fn bind_verified(
        &mut self,
        selection: CatalogSelection,
        verified: &VerifiedBackupPackage,
    ) -> Result<(), StateError> {
        if selection.generation != self.generation {
            return Err(StateError::invalid_input());
        }
        let point = self
            .points
            .get_mut(usize::from(selection.ordinal))
            .filter(|point| point.selection == selection)
            .ok_or_else(StateError::invalid_input)?;
        let metadata = verified.metadata();
        let receipt = verified.receipt();
        if point.health == CatalogHealth::Corrupt
            || point.entry.len() != receipt.package_len()
            || point.observed_file_sha256 != *receipt.file_sha256()
            || point.created_at_utc_ms != Some(metadata.created_at_utc_ms())
            || point.purpose != Some(metadata.purpose())
            || point.database_schema_version != Some(verified.database_schema_version())
            || point.compression != Some(verified.compression())
        {
            return Err(StateError::integrity());
        }
        point.health = CatalogHealth::Verified;
        Ok(())
    }

    /// Binds the one exact newly published point to its full package proof.
    pub fn bind_published(
        &mut self,
        verified: &VerifiedBackupPackage,
    ) -> Result<CatalogSelection, StateError> {
        let metadata = verified.metadata();
        let receipt = verified.receipt();
        let mut matches = self
            .points
            .iter()
            .filter(|point| {
                point.entry.len() == receipt.package_len()
                    && point.observed_file_sha256 == *receipt.file_sha256()
                    && point.created_at_utc_ms == Some(metadata.created_at_utc_ms())
                    && point.purpose == Some(metadata.purpose())
                    && point.database_schema_version == Some(verified.database_schema_version())
                    && point.compression == Some(verified.compression())
            })
            .map(|point| point.selection);
        let selection = matches.next().ok_or_else(StateError::integrity)?;
        if matches.next().is_some() {
            return Err(StateError::integrity());
        }
        self.bind_verified(selection, verified)?;
        Ok(selection)
    }

    pub(crate) const fn directory_identity(&self) -> CatalogDirectoryIdentity {
        self.directory_identity
    }

    pub(crate) fn matches_directory(
        &self,
        directory: &BackupDirectory,
    ) -> Result<bool, StateError> {
        directory
            .scan()
            .map(|snapshot| snapshot.generation() == self.directory_identity.0)
            .map_err(map_directory_error)
    }

    pub(crate) fn revalidate_point(
        &self,
        directory: &BackupDirectory,
        selection: CatalogSelection,
    ) -> Result<bool, StateError> {
        if selection.generation != self.generation {
            return Err(StateError::invalid_input());
        }
        let point = self
            .points
            .get(usize::from(selection.ordinal))
            .filter(|point| point.selection == selection)
            .ok_or_else(StateError::invalid_input)?;
        let (header, observed_file_sha256) = inspect_header(directory, &point.entry)?;
        let Ok(header) = header else {
            return Ok(false);
        };
        Ok(observed_file_sha256 == point.observed_file_sha256
            && point.created_at_utc_ms == Some(header.metadata.created_at_utc_ms())
            && point.purpose == Some(header.metadata.purpose())
            && point.database_schema_version == Some(header.database_schema_version)
            && point.compression == Some(header.compression))
    }

    pub(crate) fn open_verified_selection(
        &self,
        directory: &BackupDirectory,
        selection: CatalogSelection,
    ) -> Result<DurableFileReader, StateError> {
        let point = self.selected_point(selection)?;
        if point.health != CatalogHealth::Verified
            || !self.matches_directory(directory)?
            || !self.revalidate_point(directory, selection)?
        {
            return Err(StateError::integrity());
        }
        directory
            .open_reader(&point.entry, MAX_DURABLE_FILE_BYTES)
            .map_err(map_directory_error)
    }

    pub(crate) fn open_recovery_selection(
        &self,
        directory: &BackupDirectory,
        selection: CatalogSelection,
    ) -> Result<DurableFileReader, StateError> {
        let point = self.selected_point(selection)?;
        if point.health == CatalogHealth::Corrupt
            || !self.matches_directory(directory)?
            || !self.revalidate_point(directory, selection)?
        {
            return Err(StateError::integrity());
        }
        directory
            .open_reader(&point.entry, MAX_DURABLE_FILE_BYTES)
            .map_err(map_directory_error)
    }

    pub(crate) fn selected_package_identity(
        &self,
        selection: CatalogSelection,
    ) -> Result<(u8, u64, [u8; 32]), StateError> {
        let point = self.selected_point(selection)?;
        if point.health != CatalogHealth::Verified {
            return Err(StateError::integrity());
        }
        Ok((
            point.entry.ordinal(),
            point.entry.len(),
            point.observed_file_sha256,
        ))
    }

    pub(crate) fn selection_for_package_identity(
        &self,
        backup_slot: u8,
        package_len: u64,
        package_sha256: [u8; 32],
    ) -> Result<CatalogSelection, StateError> {
        let mut matches = self.points.iter().filter(|point| {
            point.entry.ordinal() == backup_slot
                && point.entry.len() == package_len
                && point.observed_file_sha256 == package_sha256
        });
        let point = matches.next().ok_or_else(StateError::integrity)?;
        if matches.next().is_some() || point.health == CatalogHealth::Corrupt {
            return Err(StateError::integrity());
        }
        Ok(point.selection)
    }

    fn selected_point(&self, selection: CatalogSelection) -> Result<&CatalogPoint, StateError> {
        if selection.generation != self.generation {
            return Err(StateError::invalid_input());
        }
        self.points
            .get(usize::from(selection.ordinal))
            .filter(|point| point.selection == selection)
            .ok_or_else(StateError::invalid_input)
    }

    pub(crate) fn revalidate_all_verified(
        &self,
        directory: &BackupDirectory,
    ) -> Result<bool, StateError> {
        for point in self
            .points
            .iter()
            .filter(|point| point.health == CatalogHealth::Verified)
        {
            if !self.revalidate_point(directory, point.selection)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl fmt::Debug for BackupCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupCatalog")
            .field("generation", &self.generation)
            .field("point_count", &self.points.len())
            .finish()
    }
}

fn inspect_header(
    directory: &BackupDirectory,
    entry: &BackupDirectoryEntry,
) -> Result<(Result<CatalogPackageHeader, StateError>, [u8; 32]), StateError> {
    let mut reader = directory
        .open_reader(entry, MAX_DURABLE_FILE_BYTES)
        .map_err(map_directory_error)?;
    let mut prefix = [0_u8; CATALOG_HEADER_BYTES];
    let mut prefix_len = 0_usize;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; PACKAGE_IO_BUFFER_BYTES];

    loop {
        let count = reader
            .read_chunk(&mut buffer)
            .map_err(|_| StateError::unavailable())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        if prefix_len < prefix.len() {
            let copied = count.min(prefix.len() - prefix_len);
            prefix[prefix_len..prefix_len + copied].copy_from_slice(&buffer[..copied]);
            prefix_len += copied;
        }
    }

    let observed_file_sha256 = hasher.finalize().into();
    let header = if prefix_len == prefix.len() {
        decode_catalog_header(&prefix)
    } else {
        Err(StateError::integrity())
    };
    Ok((header, observed_file_sha256))
}

fn next_generation(previous: Option<CatalogGeneration>) -> Result<CatalogGeneration, StateError> {
    match previous {
        Some(previous) => previous
            .0
            .checked_add(1)
            .map(CatalogGeneration)
            .ok_or_else(StateError::capacity_exceeded),
        None => Ok(CatalogGeneration(1)),
    }
}

const fn map_directory_error(error: BackupDirectoryError) -> StateError {
    match error {
        BackupDirectoryError::UnexpectedEntry
        | BackupDirectoryError::UnexpectedType
        | BackupDirectoryError::LinkedEntry
        | BackupDirectoryError::AmbiguousIdentity => StateError::integrity(),
        BackupDirectoryError::CapacityExceeded => StateError::capacity_exceeded(),
        BackupDirectoryError::RecoveryRequired => StateError::recovery_required(),
        BackupDirectoryError::UnsupportedLocation
        | BackupDirectoryError::StaleEntry
        | BackupDirectoryError::InvalidState
        | BackupDirectoryError::Unavailable => StateError::unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogGeneration, next_generation};
    use crate::StateErrorCode;

    #[test]
    fn catalog_generation_overflow_fails_closed() {
        let error = match next_generation(Some(CatalogGeneration(u64::MAX))) {
            Ok(_) => panic!("generation overflow must fail"),
            Err(error) => error,
        };
        assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    }
}
