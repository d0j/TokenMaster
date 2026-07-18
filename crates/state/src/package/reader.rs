use std::io::{self, BufRead, Read, Write};

use sha2::{Digest, Sha256};

use crate::{PortableSettingsCandidate, StateError};

use super::capability::{
    BackupStagedFile, DurableFileReader, DurableReaderAdapter, DurableStagedFile,
    DurableWriterAdapter, map_backup_directory_error, map_durable_error, resolve_codec_error,
};
use super::encryption::AuthenticatedBackupPayload;
use super::header::{HEADER_BYTES, Header, PackageKind};
use super::manifest::{
    ENTRY_PREFIX_BYTES, ENTRY_SUFFIX_BYTES, EntryKind, EntryPrefix, EntrySuffix,
    FOOTER_DIGEST_BYTES, FOOTER_MAGIC, MANIFEST_BYTES, Manifest,
};
use super::{
    BackupCompression, BackupMetadata, BackupPackage, ConfigPackage, MAX_DATABASE_PACKAGE_BYTES,
    MAX_ENCODED_PACKAGE_BYTES, MAX_SETTINGS_PACKAGE_BYTES, PACKAGE_IO_BUFFER_BYTES,
    PACKAGE_WINDOW_LOG, PackageReceipt, VerifiedBackupPackage, VerifiedConfigPackage,
};

impl ConfigPackage {
    /// Verifies and decodes a settings-only package from a controlled durable reader.
    pub fn read(source: &mut DurableFileReader) -> Result<VerifiedConfigPackage, StateError> {
        let mut source_adapter = DurableReaderAdapter::new(source);
        let mut sink = io::sink();
        let result = read_package(&mut source_adapter, PackageKind::Config, &mut sink);
        let failure = source_adapter.failure();
        let parsed = resolve_codec_error(result, &[failure])?;
        Ok(VerifiedConfigPackage {
            settings: parsed.settings,
            receipt: parsed.receipt,
            created_at_utc_ms: parsed.manifest.created_at_utc_ms,
        })
    }
}

impl BackupPackage {
    /// Verifies a backup and seals its database only after the complete package passes.
    pub fn read(
        source: &mut DurableFileReader,
        database_sink: &mut DurableStagedFile,
    ) -> Result<VerifiedBackupPackage, StateError> {
        let mut source_adapter = DurableReaderAdapter::new(source);
        let result = read_backup_stream(&mut source_adapter, database_sink);
        resolve_codec_error(result, &[source_adapter.failure()])
    }

    /// Fully verifies one sealed unpublished exact-slot package without publishing it.
    pub fn verify_backup_stage(
        source: &BackupStagedFile,
    ) -> Result<VerifiedBackupPackage, StateError> {
        let mut reader = source.open_reader().map_err(map_backup_directory_error)?;
        let mut source_adapter = DurableReaderAdapter::new(&mut reader);
        let mut sink = io::sink();
        let result = read_package(&mut source_adapter, PackageKind::Backup, &mut sink)
            .and_then(verified_backup_from_parsed);
        resolve_codec_error(result, &[source_adapter.failure()])
    }
}

pub(super) fn read_authenticated_backup(
    source: AuthenticatedBackupPayload<'_>,
    database_sink: &mut DurableStagedFile,
) -> Result<VerifiedBackupPackage, StateError> {
    read_backup_stream(source, database_sink)
}

fn read_backup_stream<R: Read>(
    source: R,
    database_sink: &mut DurableStagedFile,
) -> Result<VerifiedBackupPackage, StateError> {
    let result = (|| {
        let (result, failure) = {
            let mut sink_adapter = DurableWriterAdapter::new(database_sink);
            let result = read_package(source, PackageKind::Backup, &mut sink_adapter);
            (result, sink_adapter.failure())
        };
        let parsed = resolve_codec_error(result, &[failure])?;
        let database = parsed.database.ok_or_else(StateError::internal_invariant)?;
        database_sink
            .seal(database.expanded_len, database.expanded_sha256)
            .map_err(map_durable_error)?;
        verified_backup_from_parsed(parsed)
    })();
    match result {
        Ok(verified) => Ok(verified),
        Err(error) => {
            database_sink
                .discard()
                .map_err(|_| StateError::recovery_required())?;
            Err(error)
        }
    }
}

fn verified_backup_from_parsed(parsed: ParsedPackage) -> Result<VerifiedBackupPackage, StateError> {
    let database = parsed.database.ok_or_else(StateError::internal_invariant)?;
    Ok(VerifiedBackupPackage {
        settings: parsed.settings,
        receipt: parsed.receipt,
        database_schema_version: parsed.manifest.database_schema_version,
        database_len: database.expanded_len,
        database_sha256: database.expanded_sha256,
        compression: parsed.manifest.compression,
        metadata: BackupMetadata::new(
            parsed.manifest.created_at_utc_ms,
            parsed
                .manifest
                .backup_purpose
                .ok_or_else(StateError::internal_invariant)?,
        )?,
    })
}

struct ParsedPackage {
    settings: PortableSettingsCandidate,
    receipt: PackageReceipt,
    manifest: Manifest,
    database: Option<EntryPrefix>,
}

fn read_package<R: Read>(
    source: R,
    expected_kind: PackageKind,
    database_sink: &mut dyn Write,
) -> Result<ParsedPackage, StateError> {
    let mut input = PackageInput::new(source);
    let header_bytes = input.read_array_hashed::<HEADER_BYTES>()?;
    let header = Header::decode(&header_bytes)?;
    if header.kind != expected_kind {
        return Err(StateError::invalid_input());
    }
    if usize::try_from(header.manifest_len).map_err(|_| StateError::capacity_exceeded())?
        != MANIFEST_BYTES
    {
        return Err(StateError::unsupported_version());
    }

    let manifest_bytes = input.read_array_hashed::<MANIFEST_BYTES>()?;
    let manifest = Manifest::decode(&manifest_bytes)?;
    if manifest.kind != header.kind || manifest.entry_count != header.entry_count {
        return Err(StateError::integrity());
    }
    if expected_kind == PackageKind::Config && manifest.compression != BackupCompression::Normal {
        return Err(StateError::unsupported_version());
    }
    let mut descriptor_hasher = Sha256::new();
    descriptor_hasher.update(manifest_bytes);
    let mut settings_bytes = Vec::new();
    let mut expanded_total = 0_u64;
    let mut database = None;

    for index in 0..header.entry_count {
        let prefix_bytes = input.read_array_hashed::<ENTRY_PREFIX_BYTES>()?;
        descriptor_hasher.update(prefix_bytes);
        let prefix = EntryPrefix::decode(&prefix_bytes)?;
        let expected_entry = if index == 0 {
            EntryKind::Settings
        } else {
            EntryKind::Database
        };
        if prefix.kind != expected_entry || prefix.compression != manifest.compression {
            return Err(StateError::invalid_input());
        }
        validate_entry(prefix)?;
        let compressed_start = input.consumed;
        if prefix.kind == EntryKind::Settings {
            decode_frame(&mut input, &mut settings_bytes, prefix)?;
        } else {
            decode_frame(&mut input, database_sink, prefix)?;
            database = Some(prefix);
        }
        let compressed_len = input
            .consumed
            .checked_sub(compressed_start)
            .ok_or_else(StateError::internal_invariant)?;
        let suffix_bytes = input.read_array_hashed::<ENTRY_SUFFIX_BYTES>()?;
        descriptor_hasher.update(suffix_bytes);
        let suffix = EntrySuffix::decode(&suffix_bytes)?;
        if compressed_len == 0
            || suffix.compressed_len != compressed_len
            || suffix.expanded_len != prefix.expanded_len
        {
            return Err(StateError::integrity());
        }
        expanded_total = expanded_total
            .checked_add(prefix.expanded_len)
            .ok_or_else(StateError::capacity_exceeded)?;
    }
    if expanded_total != header.total_expanded {
        return Err(StateError::integrity());
    }

    let binding = input.read_array_hashed::<FOOTER_DIGEST_BYTES>()?;
    let expected_binding: [u8; 32] = descriptor_hasher.finalize().into();
    if binding != expected_binding {
        return Err(StateError::integrity());
    }
    let footer_magic = input.read_array_hashed::<8>()?;
    if &footer_magic != FOOTER_MAGIC {
        return Err(StateError::integrity());
    }
    let computed_package_sha256: [u8; 32] = input.hasher.clone().finalize().into();
    let stored_package_sha256 = input.read_array_unhashed::<FOOTER_DIGEST_BYTES>()?;
    if stored_package_sha256 != computed_package_sha256 || input.has_trailing()? {
        return Err(StateError::integrity());
    }
    let file_sha256: [u8; 32] = input.file_hasher.finalize().into();
    let receipt = PackageReceipt::new(input.consumed, computed_package_sha256, file_sha256);
    let settings = PortableSettingsCandidate::decode(&settings_bytes)?;
    Ok(ParsedPackage {
        settings,
        receipt,
        manifest,
        database,
    })
}

fn validate_entry(prefix: EntryPrefix) -> Result<(), StateError> {
    if prefix.expanded_len == 0 {
        return Err(StateError::invalid_input());
    }
    let limit = match prefix.kind {
        EntryKind::Settings => MAX_SETTINGS_PACKAGE_BYTES,
        EntryKind::Database => MAX_DATABASE_PACKAGE_BYTES,
    };
    if prefix.expanded_len > limit {
        return Err(StateError::capacity_exceeded());
    }
    Ok(())
}

fn decode_frame<R: Read>(
    input: &mut PackageInput<R>,
    sink: &mut dyn Write,
    prefix: EntryPrefix,
) -> Result<(), StateError> {
    let frame_prefix = input.peek()?;
    validate_zstd_frame_header(frame_prefix)?;
    let content_size = zstd::zstd_safe::get_frame_content_size(frame_prefix)
        .map_err(|_| StateError::integrity())?;
    if content_size != Some(prefix.expanded_len) {
        return Err(StateError::integrity());
    }
    let mut decoder = zstd::stream::read::Decoder::with_buffer(&mut *input)
        .map_err(|_| StateError::integrity())?;
    decoder
        .window_log_max(PACKAGE_WINDOW_LOG)
        .map_err(|_| StateError::integrity())?;
    let mut decoder = decoder.single_frame();
    let mut buffer = [0_u8; PACKAGE_IO_BUFFER_BYTES];
    let mut expanded = 0_u64;
    let mut hasher = Sha256::new();
    let decode_result = loop {
        match decoder.read(&mut buffer) {
            Ok(0) => break Ok(()),
            Ok(count) => {
                expanded = expanded
                    .checked_add(u64::try_from(count).map_err(|_| StateError::capacity_exceeded())?)
                    .ok_or_else(StateError::capacity_exceeded)?;
                if expanded > prefix.expanded_len {
                    break Err(StateError::integrity());
                }
                hasher.update(&buffer[..count]);
                sink.write_all(&buffer[..count])
                    .map_err(|_| StateError::unavailable())?;
            }
            Err(_) => break Err(StateError::integrity()),
        }
    };
    drop(decoder);
    if input.capacity_exceeded {
        return Err(StateError::capacity_exceeded());
    }
    if let Some(kind) = input.source_error {
        return Err(map_source_error(kind));
    }
    decode_result?;
    let expanded_sha256: [u8; 32] = hasher.finalize().into();
    if expanded != prefix.expanded_len || expanded_sha256 != prefix.expanded_sha256 {
        return Err(StateError::integrity());
    }
    Ok(())
}

fn validate_zstd_frame_header(bytes: &[u8]) -> Result<(), StateError> {
    const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
    const CHECKSUM_FLAG: u8 = 0b0000_0100;
    const RESERVED_FLAGS: u8 = 0b0001_1000;
    const DICTIONARY_ID_FLAGS: u8 = 0b0000_0011;
    if bytes.len() < 5 || bytes[0..4] != ZSTD_MAGIC {
        return Err(StateError::integrity());
    }
    let descriptor = bytes[4];
    if descriptor & CHECKSUM_FLAG == 0
        || descriptor & RESERVED_FLAGS != 0
        || descriptor & DICTIONARY_ID_FLAGS != 0
    {
        return Err(StateError::unsupported_version());
    }
    Ok(())
}

struct PackageInput<R: Read> {
    source: R,
    buffer: Box<[u8; PACKAGE_IO_BUFFER_BYTES]>,
    position: usize,
    available: usize,
    hasher: Sha256,
    file_hasher: Sha256,
    consumed: u64,
    source_error: Option<io::ErrorKind>,
    capacity_exceeded: bool,
}

impl<R: Read> PackageInput<R> {
    fn new(source: R) -> Self {
        Self {
            source,
            buffer: Box::new([0_u8; PACKAGE_IO_BUFFER_BYTES]),
            position: 0,
            available: 0,
            hasher: Sha256::new(),
            file_hasher: Sha256::new(),
            consumed: 0,
            source_error: None,
            capacity_exceeded: false,
        }
    }

    fn read_array_hashed<const N: usize>(&mut self) -> Result<[u8; N], StateError> {
        let mut bytes = [0_u8; N];
        self.read_exact_internal(&mut bytes, true)?;
        Ok(bytes)
    }

    fn read_array_unhashed<const N: usize>(&mut self) -> Result<[u8; N], StateError> {
        let mut bytes = [0_u8; N];
        self.read_exact_internal(&mut bytes, false)?;
        Ok(bytes)
    }

    fn read_exact_internal(
        &mut self,
        mut output: &mut [u8],
        hashed: bool,
    ) -> Result<(), StateError> {
        while !output.is_empty() {
            let available = self.peek()?;
            if available.is_empty() {
                return Err(StateError::integrity());
            }
            let count = available.len().min(output.len());
            output[..count].copy_from_slice(&available[..count]);
            self.consume_internal(count, hashed);
            output = &mut output[count..];
        }
        Ok(())
    }

    fn peek(&mut self) -> Result<&[u8], StateError> {
        if self.refill().is_err() {
            return Err(if self.capacity_exceeded {
                StateError::capacity_exceeded()
            } else if let Some(kind) = self.source_error {
                map_source_error(kind)
            } else {
                StateError::unavailable()
            });
        }
        Ok(&self.buffer[self.position..self.available])
    }

    fn refill(&mut self) -> io::Result<()> {
        if self.position == self.available {
            if self.consumed >= MAX_ENCODED_PACKAGE_BYTES {
                self.capacity_exceeded = true;
                return Err(io::ErrorKind::OutOfMemory.into());
            }
            self.position = 0;
            let remaining = MAX_ENCODED_PACKAGE_BYTES - self.consumed;
            let request = usize::try_from(remaining.min(PACKAGE_IO_BUFFER_BYTES as u64))
                .map_err(|_| io::ErrorKind::OutOfMemory)?;
            self.available = match self.source.read(&mut self.buffer[..request]) {
                Ok(count) => count,
                Err(error) => {
                    self.source_error = Some(error.kind());
                    return Err(error);
                }
            };
        }
        Ok(())
    }

    fn has_trailing(&mut self) -> Result<bool, StateError> {
        if self.position < self.available {
            return Ok(true);
        }
        let mut byte = [0_u8; 1];
        self.source
            .read(&mut byte)
            .map(|count| count != 0)
            .map_err(|error| map_source_error(error.kind()))
    }

    fn consume_internal(&mut self, count: usize, hashed: bool) {
        let count = count.min(self.available.saturating_sub(self.position));
        self.file_hasher
            .update(&self.buffer[self.position..self.position + count]);
        if hashed {
            self.hasher
                .update(&self.buffer[self.position..self.position + count]);
        }
        self.position += count;
        self.consumed = self.consumed.saturating_add(count as u64);
    }
}

const fn map_source_error(kind: io::ErrorKind) -> StateError {
    match kind {
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => StateError::integrity(),
        io::ErrorKind::OutOfMemory => StateError::capacity_exceeded(),
        _ => StateError::unavailable(),
    }
}

impl<R: Read> Read for PackageInput<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        if available.is_empty() {
            return Ok(0);
        }
        let count = available.len().min(output.len());
        output[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl<R: Read> BufRead for PackageInput<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.refill()?;
        Ok(&self.buffer[self.position..self.available])
    }

    fn consume(&mut self, count: usize) {
        self.consume_internal(count, true);
    }
}
