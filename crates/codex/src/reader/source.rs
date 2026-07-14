use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Component;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use tokenmaster_platform::{PhysicalFileIdentity, PhysicalIdentityError};

use super::{
    BoundaryAnchor, MAX_ANCHOR_BYTES, ReaderError, ReaderErrorCode, SOURCE_CHUNK_BYTES,
    SourceChunkDigest,
};
use crate::SourceFileDescriptor;

const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) struct OpenSource {
    pub(super) file: File,
    pub(super) physical_identity: Option<PhysicalFileIdentity>,
    pub(super) file_length: u64,
    pub(super) modified_time_ns: Option<i64>,
}

pub(super) fn open_source(descriptor: &SourceFileDescriptor) -> Result<OpenSource, ReaderError> {
    validate_descriptor_components(descriptor)?;

    let mut options = OpenOptions::new();
    options.read(true);
    configure_safe_open(&mut options);
    let file = options
        .open(descriptor.absolute_path())
        .map_err(|_| ReaderError::new(ReaderErrorCode::OpenFailed))?;
    let metadata = file
        .metadata()
        .map_err(|_| ReaderError::new(ReaderErrorCode::OpenFailed))?;
    if is_reparse_point(&metadata) {
        return Err(ReaderError::new(ReaderErrorCode::ReparsePoint));
    }
    if !metadata.is_file() {
        return Err(ReaderError::new(ReaderErrorCode::NonRegular));
    }
    let physical_identity = physical_identity(&file)?;

    Ok(OpenSource {
        file,
        physical_identity,
        file_length: metadata.len(),
        modified_time_ns: modified_time_ns(&metadata),
    })
}

fn validate_descriptor_components(descriptor: &SourceFileDescriptor) -> Result<(), ReaderError> {
    let mut root = descriptor.absolute_path().to_path_buf();
    let mut component_count = 0_usize;
    for component in descriptor.relative_path().components() {
        if !matches!(component, Component::Normal(_)) || !root.pop() {
            return Err(ReaderError::new(ReaderErrorCode::InvalidDescriptor));
        }
        component_count = component_count.saturating_add(1);
    }
    if component_count == 0 || root.join(descriptor.relative_path()) != descriptor.absolute_path() {
        return Err(ReaderError::new(ReaderErrorCode::InvalidDescriptor));
    }

    let root_metadata = std::fs::symlink_metadata(&root)
        .map_err(|_| ReaderError::new(ReaderErrorCode::OpenFailed))?;
    if is_reparse_point(&root_metadata) {
        return Err(ReaderError::new(ReaderErrorCode::ReparsePoint));
    }
    if !root_metadata.is_dir() {
        return Err(ReaderError::new(ReaderErrorCode::NonRegular));
    }

    let mut candidate = root;
    let mut components = descriptor.relative_path().components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(value) = component else {
            return Err(ReaderError::new(ReaderErrorCode::InvalidDescriptor));
        };
        candidate.push(value);
        let metadata = std::fs::symlink_metadata(&candidate)
            .map_err(|_| ReaderError::new(ReaderErrorCode::OpenFailed))?;
        if is_reparse_point(&metadata) {
            return Err(ReaderError::new(ReaderErrorCode::ReparsePoint));
        }
        let valid_kind = if components.peek().is_some() {
            metadata.is_dir()
        } else {
            metadata.is_file()
        };
        if !valid_kind {
            return Err(ReaderError::new(ReaderErrorCode::NonRegular));
        }
    }
    Ok(())
}

pub(super) fn current_handle_observation(
    file: &File,
) -> Result<(Option<PhysicalFileIdentity>, u64, Option<i64>), ReaderError> {
    let metadata = file
        .metadata()
        .map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    Ok((
        physical_identity(file)?,
        metadata.len(),
        modified_time_ns(&metadata),
    ))
}

pub(super) fn hash_range(file: &mut File, start: u64, len: u64) -> Result<[u8; 32], ReaderError> {
    file.seek(SeekFrom::Start(start))
        .map_err(|_| ReaderError::new(ReaderErrorCode::SeekFailed))?;
    let mut remaining = len;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    while remaining > 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file
            .read(&mut buffer[..requested])
            .map_err(|_| ReaderError::new(ReaderErrorCode::ReadFailed))?;
        if read == 0 {
            return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
        }
        hasher.update(&buffer[..read]);
        remaining = remaining.saturating_sub(read as u64);
    }
    Ok(hasher.finalize().into())
}

pub(super) fn boundary_anchor(
    file: &mut File,
    committed: u64,
) -> Result<BoundaryAnchor, ReaderError> {
    if committed == 0 {
        return BoundaryAnchor::new(0, 0, [0; 32])
            .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid));
    }
    let len = committed.min(u64::from(MAX_ANCHOR_BYTES));
    let start = committed.saturating_sub(len);
    let sha256 = hash_range(file, start, len)?;
    BoundaryAnchor::new(
        start,
        u16::try_from(len).map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?,
        sha256,
    )
    .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))
}

pub(super) fn source_chunks_for_range(
    file: &mut File,
    first_chunk: u64,
    covered: u64,
) -> Result<Vec<SourceChunkDigest>, ReaderError> {
    let first_start = first_chunk
        .checked_mul(SOURCE_CHUNK_BYTES)
        .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
    if first_start >= covered {
        return Ok(Vec::new());
    }

    let chunk_count = covered
        .saturating_sub(first_start)
        .div_ceil(SOURCE_CHUNK_BYTES);
    let capacity = usize::try_from(chunk_count)
        .map_err(|_| ReaderError::new(ReaderErrorCode::CapacityExceeded))?;
    let mut chunks = Vec::with_capacity(capacity);
    for relative_index in 0..chunk_count {
        let index = first_chunk.saturating_add(relative_index);
        let start = index
            .checked_mul(SOURCE_CHUNK_BYTES)
            .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
        let len = covered.saturating_sub(start).min(SOURCE_CHUNK_BYTES);
        let sha256 = hash_range(file, start, len)?;
        chunks.push(SourceChunkDigest::from_verified_parts(
            index,
            u32::try_from(len).map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?,
            sha256,
        ));
    }
    Ok(chunks)
}

pub(super) fn extended_partial_chunk(
    file: &mut File,
    previous_offset: u64,
    covered: u64,
) -> Result<(SourceChunkDigest, SourceChunkDigest), ReaderError> {
    let previous_len = previous_offset % SOURCE_CHUNK_BYTES;
    if previous_len == 0 || covered <= previous_offset {
        return Err(ReaderError::new(ReaderErrorCode::CheckpointInvalid));
    }
    let index = previous_offset / SOURCE_CHUNK_BYTES;
    let start = index
        .checked_mul(SOURCE_CHUNK_BYTES)
        .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
    let covered_len = covered.saturating_sub(start).min(SOURCE_CHUNK_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|_| ReaderError::new(ReaderErrorCode::SeekFailed))?;

    let mut remaining = covered_len;
    let mut hashed = 0_u64;
    let mut hasher = Sha256::new();
    let mut previous_sha256 = None;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    while remaining > 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file
            .read(&mut buffer[..requested])
            .map_err(|_| ReaderError::new(ReaderErrorCode::ReadFailed))?;
        if read == 0 {
            return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
        }
        let read_u64 = read as u64;
        if hashed < previous_len && hashed.saturating_add(read_u64) >= previous_len {
            let prefix = usize::try_from(previous_len.saturating_sub(hashed))
                .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
            hasher.update(&buffer[..prefix]);
            previous_sha256 = Some(hasher.clone().finalize().into());
            hasher.update(&buffer[prefix..read]);
        } else {
            hasher.update(&buffer[..read]);
        }
        hashed = hashed.saturating_add(read_u64);
        remaining = remaining.saturating_sub(read_u64);
    }

    let previous_sha256 =
        previous_sha256.ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
    Ok((
        SourceChunkDigest::from_verified_parts(
            index,
            u32::try_from(previous_len)
                .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?,
            previous_sha256,
        ),
        SourceChunkDigest::from_verified_parts(
            index,
            u32::try_from(covered_len)
                .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?,
            hasher.finalize().into(),
        ),
    ))
}

pub(super) fn revalidate_path_identity(
    descriptor: &SourceFileDescriptor,
    expected: PhysicalFileIdentity,
) -> Result<(), ReaderError> {
    let observed =
        open_source(descriptor).map_err(|_| ReaderError::new(ReaderErrorCode::SourceChanged))?;
    if observed.physical_identity == Some(expected) {
        Ok(())
    } else {
        Err(ReaderError::new(ReaderErrorCode::SourceChanged))
    }
}

fn physical_identity(file: &File) -> Result<Option<PhysicalFileIdentity>, ReaderError> {
    match PhysicalFileIdentity::from_file(file) {
        Ok(identity) => Ok(Some(identity)),
        Err(PhysicalIdentityError::Unavailable) => Ok(None),
        Err(PhysicalIdentityError::QueryFailed) => {
            Err(ReaderError::new(ReaderErrorCode::OpenFailed))
        }
    }
}

fn modified_time_ns(metadata: &Metadata) -> Option<i64> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
}

#[cfg(windows)]
fn configure_safe_open(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ_WRITE_DELETE: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options
        .share_mode(FILE_SHARE_READ_WRITE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(not(windows))]
fn configure_safe_open(_options: &mut OpenOptions) {}

#[cfg(windows)]
fn is_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &Metadata) -> bool {
    false
}
