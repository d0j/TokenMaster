use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Component;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use tokenmaster_platform::{PhysicalFileIdentity, PhysicalIdentityError};

use super::{
    BoundaryAnchor, MAX_ANCHOR_BYTES, ReaderError, ReaderErrorCode, ReaderProofCache,
    SOURCE_CHUNK_BYTES, SourceChunkDigest,
};
use crate::SourceFileDescriptor;

const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) struct ChunkProofValidation {
    pub(super) observed_sha256: [u8; 32],
    pub(super) chunks: Vec<SourceChunkDigest>,
    pub(super) previous_partial: Option<SourceChunkDigest>,
}

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

pub(super) fn validate_and_extend_chunk_proofs(
    file: &mut File,
    start: u64,
    observed_len: u64,
    verified_end: u64,
    cache: &mut ReaderProofCache,
) -> Result<ChunkProofValidation, ReaderError> {
    let observed_end = start
        .checked_add(observed_len)
        .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
    if verified_end > observed_end {
        return Err(ReaderError::new(ReaderErrorCode::CheckpointInvalid));
    }

    let advances_proof = verified_end > start;
    let (chunk_index, previous_len) = (start / SOURCE_CHUNK_BYTES, start % SOURCE_CHUNK_BYTES);
    if advances_proof
        && (cache.chunk_index != chunk_index || u64::from(cache.covered_len) != previous_len)
    {
        cache.chunk_index = chunk_index;
        cache.covered_len = 0;
        cache.hasher = Sha256::new();
        if previous_len != 0 {
            let chunk_start = chunk_index
                .checked_mul(SOURCE_CHUNK_BYTES)
                .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
            file.seek(SeekFrom::Start(chunk_start))
                .map_err(|_| ReaderError::new(ReaderErrorCode::SeekFailed))?;
            update_hasher_from_file(file, previous_len, &mut cache.hasher)?;
            cache.covered_len = u32::try_from(previous_len)
                .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
            cache.recovery_bytes_read = cache
                .recovery_bytes_read
                .checked_add(previous_len)
                .ok_or_else(|| ReaderError::new(ReaderErrorCode::CapacityExceeded))?;
        }
    }

    let previous_partial_chunk = (advances_proof && previous_len != 0).then(|| {
        SourceChunkDigest::from_verified_parts(
            chunk_index,
            cache.covered_len,
            cache.hasher.clone().finalize().into(),
        )
    });
    file.seek(SeekFrom::Start(start))
        .map_err(|_| ReaderError::new(ReaderErrorCode::SeekFailed))?;
    let mut observed_hasher = Sha256::new();
    let mut remaining = observed_len;
    let mut position = start;
    let mut chunks = Vec::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    while remaining > 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file
            .read(&mut buffer[..requested])
            .map_err(|_| ReaderError::new(ReaderErrorCode::ReadFailed))?;
        if read == 0 {
            return Err(ReaderError::new(ReaderErrorCode::SourceChanged));
        }
        observed_hasher.update(&buffer[..read]);

        let verified_read = if advances_proof {
            verified_end.saturating_sub(position).min(read as u64) as usize
        } else {
            0
        };
        let mut verified_offset = 0_usize;
        while verified_offset < verified_read {
            let room =
                usize::try_from(SOURCE_CHUNK_BYTES.saturating_sub(u64::from(cache.covered_len)))
                    .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
            let take = room.min(verified_read.saturating_sub(verified_offset));
            cache
                .hasher
                .update(&buffer[verified_offset..verified_offset.saturating_add(take)]);
            cache.covered_len = cache
                .covered_len
                .checked_add(
                    u32::try_from(take)
                        .map_err(|_| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?,
                )
                .ok_or_else(|| ReaderError::new(ReaderErrorCode::CheckpointInvalid))?;
            verified_offset = verified_offset.saturating_add(take);
            if u64::from(cache.covered_len) == SOURCE_CHUNK_BYTES {
                chunks.push(SourceChunkDigest::from_verified_parts(
                    cache.chunk_index,
                    cache.covered_len,
                    cache.hasher.clone().finalize().into(),
                ));
                cache.chunk_index = cache
                    .chunk_index
                    .checked_add(1)
                    .ok_or_else(|| ReaderError::new(ReaderErrorCode::CapacityExceeded))?;
                cache.covered_len = 0;
                cache.hasher = Sha256::new();
            }
        }
        position = position.saturating_add(read as u64);
        remaining = remaining.saturating_sub(read as u64);
    }
    if advances_proof && cache.covered_len != 0 {
        chunks.push(SourceChunkDigest::from_verified_parts(
            cache.chunk_index,
            cache.covered_len,
            cache.hasher.clone().finalize().into(),
        ));
    }
    Ok(ChunkProofValidation {
        observed_sha256: observed_hasher.finalize().into(),
        chunks,
        previous_partial: previous_partial_chunk,
    })
}

fn update_hasher_from_file(
    file: &mut File,
    len: u64,
    hasher: &mut Sha256,
) -> Result<(), ReaderError> {
    let mut remaining = len;
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
    Ok(())
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
