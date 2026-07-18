use std::io::{self, Read, Write};

use sha2::{Digest, Sha256};

use crate::{PortableSettingsCandidate, StateError};

use super::capability::{
    DurableFileReader, DurableReaderAdapter, DurableStagedFile, DurableWriterAdapter,
    map_durable_error, resolve_codec_error,
};
use super::header::{Header, PackageKind};
use super::manifest::{
    EntryKind, EntryPrefix, EntrySuffix, FOOTER_MAGIC, MANIFEST_BYTES, Manifest,
};
use super::{
    BackupCompression, BackupMetadata, BackupPackage, ConfigPackage, MAX_DATABASE_PACKAGE_BYTES,
    MAX_ENCODED_PACKAGE_BYTES, MAX_PACKAGE_TOTAL_EXPANDED_BYTES, MAX_SETTINGS_PACKAGE_BYTES,
    PACKAGE_IO_BUFFER_BYTES, PACKAGE_WINDOW_LOG, PackageReceipt,
};

impl ConfigPackage {
    /// Writes and seals a deterministic settings-only package in a controlled stage.
    pub fn write(
        settings: &PortableSettingsCandidate,
        created_at_utc_ms: i64,
        destination: &mut DurableStagedFile,
    ) -> Result<PackageReceipt, StateError> {
        let result = (|| {
            let (result, failure) = {
                let mut destination_adapter = DurableWriterAdapter::new(destination);
                let result =
                    write_config_stream(settings, created_at_utc_ms, &mut destination_adapter);
                (result, destination_adapter.failure())
            };
            let receipt = resolve_codec_error(result, &[failure])?;
            destination
                .seal(receipt.package_len(), *receipt.file_sha256())
                .map_err(map_durable_error)?;
            Ok(receipt)
        })();
        discard_failed_stage(result, destination)
    }
}

fn write_config_stream<W: Write>(
    settings: &PortableSettingsCandidate,
    created_at_utc_ms: i64,
    destination: &mut W,
) -> Result<PackageReceipt, StateError> {
    let settings_bytes = settings.encode_json()?;
    let settings_len =
        u64::try_from(settings_bytes.len()).map_err(|_| StateError::capacity_exceeded())?;
    validate_settings_len(settings_len)?;
    let manifest = Manifest::new(
        PackageKind::Config,
        0,
        BackupCompression::Normal,
        created_at_utc_ms,
        None,
    )?;
    let header = Header::new(PackageKind::Config, MANIFEST_BYTES, settings_len)?;
    let mut output = PackageOutput::new(destination);
    let mut descriptor_hasher = Sha256::new();
    output.write_checked(&header.encode())?;
    write_manifest(&mut output, &mut descriptor_hasher, manifest)?;
    let mut source = settings_bytes.as_slice();
    write_entry(
        &mut output,
        &mut descriptor_hasher,
        EntryPrefix::new(
            EntryKind::Settings,
            BackupCompression::Normal,
            settings_len,
            *settings.digest().as_bytes(),
        ),
        &mut source,
    )?;
    finish_package(output, descriptor_hasher)
}

impl BackupPackage {
    /// Writes and seals a typed settings-plus-database package in a controlled stage.
    ///
    /// `database_len` and `database_sha256` must come from a verified standalone
    /// snapshot. The source is independently counted and hashed before success.
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        settings: &PortableSettingsCandidate,
        database: &mut DurableFileReader,
        database_len: u64,
        database_sha256: [u8; 32],
        database_schema_version: u16,
        compression: BackupCompression,
        metadata: BackupMetadata,
        destination: &mut DurableStagedFile,
    ) -> Result<PackageReceipt, StateError> {
        let result = (|| {
            if database.len() != database_len {
                return Err(StateError::integrity());
            }
            let (result, failures) = {
                let mut database_adapter = DurableReaderAdapter::new(database);
                let mut destination_adapter = DurableWriterAdapter::new(destination);
                let result = write_backup_stream(
                    settings,
                    &mut database_adapter,
                    database_len,
                    database_sha256,
                    database_schema_version,
                    compression,
                    metadata,
                    &mut destination_adapter,
                );
                (
                    result,
                    [database_adapter.failure(), destination_adapter.failure()],
                )
            };
            let receipt = resolve_codec_error(result, &failures)?;
            destination
                .seal(receipt.package_len(), *receipt.file_sha256())
                .map_err(map_durable_error)?;
            Ok(receipt)
        })();
        discard_failed_stage(result, destination)
    }
}

fn discard_failed_stage<T>(
    result: Result<T, StateError>,
    destination: &mut DurableStagedFile,
) -> Result<T, StateError> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            destination
                .discard()
                .map_err(|_| StateError::recovery_required())?;
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_backup_stream<R: Read, W: Write>(
    settings: &PortableSettingsCandidate,
    database: &mut R,
    database_len: u64,
    database_sha256: [u8; 32],
    database_schema_version: u16,
    compression: BackupCompression,
    metadata: BackupMetadata,
    destination: &mut W,
) -> Result<PackageReceipt, StateError> {
    validate_database(database_len, database_schema_version)?;
    let settings_bytes = settings.encode_json()?;
    let settings_len =
        u64::try_from(settings_bytes.len()).map_err(|_| StateError::capacity_exceeded())?;
    validate_settings_len(settings_len)?;
    let total_expanded = settings_len
        .checked_add(database_len)
        .ok_or_else(StateError::capacity_exceeded)?;
    if total_expanded > MAX_PACKAGE_TOTAL_EXPANDED_BYTES {
        return Err(StateError::capacity_exceeded());
    }

    let manifest = Manifest::new(
        PackageKind::Backup,
        database_schema_version,
        compression,
        metadata.created_at_utc_ms(),
        Some(metadata.purpose()),
    )?;
    let header = Header::new(PackageKind::Backup, MANIFEST_BYTES, total_expanded)?;
    let mut output = PackageOutput::new(destination);
    let mut descriptor_hasher = Sha256::new();
    output.write_checked(&header.encode())?;
    write_manifest(&mut output, &mut descriptor_hasher, manifest)?;

    let mut settings_source = settings_bytes.as_slice();
    write_entry(
        &mut output,
        &mut descriptor_hasher,
        EntryPrefix::new(
            EntryKind::Settings,
            compression,
            settings_len,
            *settings.digest().as_bytes(),
        ),
        &mut settings_source,
    )?;
    write_entry(
        &mut output,
        &mut descriptor_hasher,
        EntryPrefix::new(
            EntryKind::Database,
            compression,
            database_len,
            database_sha256,
        ),
        database,
    )?;
    finish_package(output, descriptor_hasher)
}

fn validate_settings_len(len: u64) -> Result<(), StateError> {
    if len == 0 {
        return Err(StateError::invalid_input());
    }
    if len > MAX_SETTINGS_PACKAGE_BYTES {
        return Err(StateError::capacity_exceeded());
    }
    Ok(())
}

fn validate_database(len: u64, schema_version: u16) -> Result<(), StateError> {
    if len == 0 || schema_version == 0 {
        return Err(StateError::invalid_input());
    }
    if len > MAX_DATABASE_PACKAGE_BYTES {
        return Err(StateError::capacity_exceeded());
    }
    Ok(())
}

fn write_manifest<W: Write>(
    output: &mut PackageOutput<'_, W>,
    descriptor_hasher: &mut Sha256,
    manifest: Manifest,
) -> Result<(), StateError> {
    let encoded = manifest.encode();
    descriptor_hasher.update(encoded);
    output.write_checked(&encoded)
}

fn write_entry<R: Read, W: Write>(
    output: &mut PackageOutput<'_, W>,
    descriptor_hasher: &mut Sha256,
    prefix: EntryPrefix,
    source: &mut R,
) -> Result<(), StateError> {
    let prefix_bytes = prefix.encode();
    descriptor_hasher.update(prefix_bytes);
    output.write_checked(&prefix_bytes)?;
    let compressed_start = output.len;
    let mut expanded_hasher = Sha256::new();
    let mut expanded = 0_u64;
    let compression_result = (|| -> Result<(), StateError> {
        let mut encoder =
            zstd::stream::write::Encoder::new(&mut *output, prefix.compression.level())
                .map_err(|_| StateError::internal_invariant())?;
        encoder
            .include_checksum(true)
            .and_then(|()| encoder.include_contentsize(true))
            .and_then(|()| encoder.long_distance_matching(false))
            .and_then(|()| encoder.window_log(PACKAGE_WINDOW_LOG))
            .and_then(|()| encoder.set_pledged_src_size(Some(prefix.expanded_len)))
            .map_err(|_| StateError::internal_invariant())?;
        let mut buffer = [0_u8; PACKAGE_IO_BUFFER_BYTES];
        while expanded < prefix.expanded_len {
            let remaining = prefix.expanded_len - expanded;
            let request = usize::try_from(remaining.min(PACKAGE_IO_BUFFER_BYTES as u64))
                .map_err(|_| StateError::capacity_exceeded())?;
            let count = source
                .read(&mut buffer[..request])
                .map_err(|_| StateError::unavailable())?;
            if count == 0 {
                return Err(StateError::integrity());
            }
            encoder
                .write_all(&buffer[..count])
                .map_err(|_| StateError::unavailable())?;
            expanded = expanded
                .checked_add(u64::try_from(count).map_err(|_| StateError::capacity_exceeded())?)
                .ok_or_else(StateError::capacity_exceeded)?;
            expanded_hasher.update(&buffer[..count]);
        }
        let mut extra = [0_u8; 1];
        if source
            .read(&mut extra)
            .map_err(|_| StateError::unavailable())?
            != 0
        {
            return Err(StateError::integrity());
        }
        encoder
            .finish()
            .map(|_| ())
            .map_err(|_| StateError::integrity())
    })();
    if output.capacity_exceeded {
        return Err(StateError::capacity_exceeded());
    }
    compression_result?;
    let expanded_sha256: [u8; 32] = expanded_hasher.finalize().into();
    if expanded != prefix.expanded_len || expanded_sha256 != prefix.expanded_sha256 {
        return Err(StateError::integrity());
    }
    let compressed_len = output
        .len
        .checked_sub(compressed_start)
        .ok_or_else(StateError::internal_invariant)?;
    if compressed_len == 0 {
        return Err(StateError::integrity());
    }
    let suffix_bytes = EntrySuffix::new(compressed_len, expanded).encode();
    descriptor_hasher.update(suffix_bytes);
    output.write_checked(&suffix_bytes)
}

fn finish_package<W: Write>(
    mut output: PackageOutput<'_, W>,
    descriptor_hasher: Sha256,
) -> Result<PackageReceipt, StateError> {
    let descriptor_digest: [u8; 32] = descriptor_hasher.finalize().into();
    output.write_checked(&descriptor_digest)?;
    output.write_checked(FOOTER_MAGIC)?;
    let package_sha256: [u8; 32] = output.package_hasher.clone().finalize().into();
    output.write_unhashed(&package_sha256)?;
    output
        .destination
        .flush()
        .map_err(|_| StateError::unavailable())?;
    let file_sha256: [u8; 32] = output.file_hasher.finalize().into();
    Ok(PackageReceipt::new(output.len, package_sha256, file_sha256))
}

struct PackageOutput<'a, W: Write> {
    destination: &'a mut W,
    package_hasher: Sha256,
    file_hasher: Sha256,
    len: u64,
    capacity_exceeded: bool,
}

impl<'a, W: Write> PackageOutput<'a, W> {
    fn new(destination: &'a mut W) -> Self {
        Self {
            destination,
            package_hasher: Sha256::new(),
            file_hasher: Sha256::new(),
            len: 0,
            capacity_exceeded: false,
        }
    }

    fn write_checked(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        self.write_all(bytes).map_err(|_| {
            if self.capacity_exceeded {
                StateError::capacity_exceeded()
            } else {
                StateError::unavailable()
            }
        })
    }

    fn write_unhashed(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        let additional = u64::try_from(bytes.len()).map_err(|_| StateError::capacity_exceeded())?;
        let next = self
            .len
            .checked_add(additional)
            .ok_or_else(StateError::capacity_exceeded)?;
        if next > MAX_ENCODED_PACKAGE_BYTES {
            return Err(StateError::capacity_exceeded());
        }
        self.destination
            .write_all(bytes)
            .map_err(|_| StateError::unavailable())?;
        self.file_hasher.update(bytes);
        self.len = next;
        Ok(())
    }
}

impl<W: Write> Write for PackageOutput<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let additional = u64::try_from(bytes.len()).map_err(|_| io::ErrorKind::OutOfMemory)?;
        let next = self.len.checked_add(additional);
        if next.is_none_or(|next| next > MAX_ENCODED_PACKAGE_BYTES) {
            self.capacity_exceeded = true;
            return Err(io::ErrorKind::OutOfMemory.into());
        }
        let count = self.destination.write(bytes)?;
        self.package_hasher.update(&bytes[..count]);
        self.file_hasher.update(&bytes[..count]);
        self.len = self
            .len
            .checked_add(u64::try_from(count).map_err(|_| io::ErrorKind::OutOfMemory)?)
            .ok_or(io::ErrorKind::OutOfMemory)?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.destination.flush()
    }
}
