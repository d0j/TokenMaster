use core::fmt;
use core::marker::PhantomData;
use std::io::{self, Write};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokenmaster_platform::{
    DurableFileError, DurableFileTarget, DurableStagedFile, MAX_DURABLE_WRITE_CHUNK_BYTES,
    ValidatedLocalDirectory,
};

use crate::StateError;

pub(crate) const MAX_RECORD_PAYLOAD_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordValueError {
    Invalid,
    UnsupportedVersion,
}

pub(crate) trait RecordValue: Serialize + Sized {
    fn decode_json(bytes: &[u8]) -> Result<Self, RecordValueError>;
}

const RECORD_MAGIC: &[u8; 8] = b"TMREC001";
const RECORD_FOOTER_MAGIC: &[u8; 8] = b"TMEND001";
const RECORD_FORMAT_VERSION: u16 = 1;
const RECORD_HEADER_BYTES: usize = 64;
const RECORD_FOOTER_BYTES: usize = 40;
const RECORD_OVERHEAD_BYTES: u64 = (RECORD_HEADER_BYTES + RECORD_FOOTER_BYTES) as u64;

/// Fixed-purpose record families in the controlled reliable-state directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordKind {
    Settings,
    RunState,
    RecoveryJournal,
}

/// Truthful redundancy available for the selected record generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordRedundancy {
    Complete,
    Single,
    Fallback,
}

/// Selected typed record, or an explicit absence of any valid generation.
pub(crate) enum RecordLoad<T> {
    Loaded(LoadedRecord<T>),
    NoValidRecord,
}

impl<T> fmt::Debug for RecordLoad<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loaded(_) => formatter.write_str("RecordLoad::Loaded([redacted])"),
            Self::NoValidRecord => formatter.write_str("RecordLoad::NoValidRecord"),
        }
    }
}

/// One verified typed value with its ordering and redundancy facts.
pub(crate) struct LoadedRecord<T> {
    generation: u64,
    redundancy: RecordRedundancy,
    payload_sha256: [u8; 32],
    value: T,
}

impl<T> LoadedRecord<T> {
    #[must_use]
    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub(crate) const fn redundancy(&self) -> RecordRedundancy {
        self.redundancy
    }

    #[must_use]
    pub(crate) const fn payload_sha256(&self) -> [u8; 32] {
        self.payload_sha256
    }

    #[must_use]
    pub(crate) fn into_value(self) -> T {
        self.value
    }
}

impl<T> fmt::Debug for LoadedRecord<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LoadedRecord([redacted])")
    }
}

/// Path- and payload-private proof of a completed save and reread.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RecordSaveReceipt {
    generation: u64,
    redundancy: RecordRedundancy,
    payload_sha256: [u8; 32],
}

impl RecordSaveReceipt {
    #[must_use]
    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub(crate) const fn redundancy(self) -> RecordRedundancy {
        self.redundancy
    }

    #[must_use]
    pub(crate) const fn payload_sha256(self) -> [u8; 32] {
        self.payload_sha256
    }
}

impl fmt::Debug for RecordSaveReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecordSaveReceipt([redacted])")
    }
}

/// Typed A/B record store over two fixed children of one validated local directory.
pub(crate) struct RedundantRecordStore<T> {
    slots: [DurableFileTarget; 2],
    max_payload_bytes: u64,
    value: PhantomData<fn() -> T>,
}

impl<T> RedundantRecordStore<T> {
    pub(crate) fn new(
        directory: &ValidatedLocalDirectory,
        kind: RecordKind,
        max_payload_bytes: u64,
    ) -> Result<Self, StateError> {
        if max_payload_bytes > MAX_RECORD_PAYLOAD_BYTES {
            return Err(StateError::capacity_exceeded());
        }
        let slots = match kind {
            RecordKind::Settings => [
                DurableFileTarget::exact_child(directory, "settings-a.tms")
                    .map_err(map_durable_error)?,
                DurableFileTarget::exact_child(directory, "settings-b.tms")
                    .map_err(map_durable_error)?,
            ],
            RecordKind::RunState => [
                DurableFileTarget::exact_child(directory, "run-a.tms")
                    .map_err(map_durable_error)?,
                DurableFileTarget::exact_child(directory, "run-b.tms")
                    .map_err(map_durable_error)?,
            ],
            RecordKind::RecoveryJournal => [
                DurableFileTarget::exact_child(directory, "recovery-a.tms")
                    .map_err(map_durable_error)?,
                DurableFileTarget::exact_child(directory, "recovery-b.tms")
                    .map_err(map_durable_error)?,
            ],
        };
        Ok(Self {
            slots,
            max_payload_bytes,
            value: PhantomData,
        })
    }
}

impl<T> RedundantRecordStore<T>
where
    T: RecordValue,
{
    pub(crate) fn load(&self) -> Result<RecordLoad<T>, StateError> {
        Ok(match self.select_record()? {
            Some(selected) => RecordLoad::Loaded(LoadedRecord {
                generation: selected.record.generation,
                redundancy: selected.redundancy,
                payload_sha256: selected.record.payload_sha256,
                value: selected.record.value,
            }),
            None => RecordLoad::NoValidRecord,
        })
    }

    pub(crate) fn save(&self, value: &T) -> Result<RecordSaveReceipt, StateError> {
        self.save_with_hook(value, &mut NoopRecordSaveHook)
    }

    /// Performs an explicit typed save, including deterministic recovery from two
    /// invalid slots. At most one invalid slot is replaced; the peer remains as
    /// forensic evidence and the reread truth is reported as fallback redundancy.
    pub(crate) fn save_explicit(&self, value: &T) -> Result<RecordSaveReceipt, StateError> {
        self.save_with_policy(value, &mut NoopRecordSaveHook, true)
    }

    pub(crate) fn save_with_hook(
        &self,
        value: &T,
        hook: &mut impl RecordSaveHook,
    ) -> Result<RecordSaveReceipt, StateError> {
        self.save_with_policy(value, hook, false)
    }

    fn save_with_policy(
        &self,
        value: &T,
        hook: &mut impl RecordSaveHook,
        explicit_recovery: bool,
    ) -> Result<RecordSaveReceipt, StateError> {
        let first = self.read_slot(0)?;
        let second = self.read_slot(1)?;
        let plan = if explicit_recovery {
            explicit_save_plan(&first, &second)?
        } else {
            save_plan(&first, &second)?
        };
        drop(first);
        drop(second);
        let measured = measure_json(value, self.max_payload_bytes)?;
        let payload_len =
            usize::try_from(measured.len).map_err(|_| StateError::capacity_exceeded())?;
        let header = encode_header(plan.generation, payload_len, measured.sha256)?;

        let total_len = RECORD_OVERHEAD_BYTES
            .checked_add(measured.len)
            .ok_or_else(StateError::capacity_exceeded)?;
        let target = &self.slots[plan.slot];
        let mut staged = target.create_staged(total_len).map_err(map_durable_error)?;
        staged.write_chunk(&header).map_err(map_durable_error)?;
        let streamed = stream_json(&mut staged, value, measured.len, &header)?;
        if streamed.payload_sha256 != measured.sha256 {
            return Err(StateError::integrity());
        }
        staged
            .write_chunk(RECORD_FOOTER_MAGIC)
            .map_err(map_durable_error)?;
        staged
            .write_chunk(&streamed.record_sha256)
            .map_err(map_durable_error)?;

        let mut file_hasher = streamed.prefix_hasher;
        file_hasher.update(RECORD_FOOTER_MAGIC);
        file_hasher.update(streamed.record_sha256);
        staged
            .seal(total_len, file_hasher.finalize().into())
            .map_err(map_durable_error)?;
        hook.hit(RecordSaveBoundary::BeforePublication)?;
        if plan.target_exists {
            staged
                .replace_existing_redundant(target)
                .map_err(map_durable_error)?;
        } else {
            staged.publish_new(target).map_err(map_durable_error)?;
        }
        hook.hit(RecordSaveBoundary::AfterPublication)
            .map_err(|_| StateError::recovery_required())?;

        let selected = match self.select_record() {
            Ok(Some(selected)) => selected,
            Ok(None) | Err(_) => return Err(StateError::recovery_required()),
        };
        if selected.record.generation != plan.generation
            || selected.record.payload_sha256 != measured.sha256
        {
            return Err(StateError::recovery_required());
        }
        Ok(RecordSaveReceipt {
            generation: selected.record.generation,
            redundancy: selected.redundancy,
            payload_sha256: selected.record.payload_sha256,
        })
    }

    fn select_record(&self) -> Result<Option<SelectedRecord<T>>, StateError> {
        let first = self.read_slot(0)?;
        let second = self.read_slot(1)?;
        select_record(first, second)
    }

    fn read_slot(&self, slot: usize) -> Result<Slot<T>, StateError> {
        let max_record_bytes = self
            .max_payload_bytes
            .checked_add(RECORD_OVERHEAD_BYTES)
            .ok_or_else(StateError::capacity_exceeded)?;
        match self.slots[slot].read_bounded(max_record_bytes) {
            Ok(None) => Ok(Slot::Missing),
            Ok(Some(bytes)) => match decode_record(&bytes, self.max_payload_bytes)? {
                Some(record) => Ok(Slot::Valid(record)),
                None => Ok(Slot::Invalid),
            },
            Err(DurableFileError::CapacityExceeded | DurableFileError::Integrity) => {
                Ok(Slot::Invalid)
            }
            Err(error) => Err(map_durable_error(error)),
        }
    }
}

impl<T> fmt::Debug for RedundantRecordStore<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RedundantRecordStore([redacted])")
    }
}

struct DecodedRecord<T> {
    generation: u64,
    payload_sha256: [u8; 32],
    value: T,
}

enum Slot<T> {
    Missing,
    Invalid,
    Valid(DecodedRecord<T>),
}

struct SelectedRecord<T> {
    record: DecodedRecord<T>,
    redundancy: RecordRedundancy,
}

struct SavePlan {
    slot: usize,
    generation: u64,
    target_exists: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordSaveBoundary {
    BeforePublication,
    AfterPublication,
}

pub(crate) trait RecordSaveHook {
    fn hit(&mut self, boundary: RecordSaveBoundary) -> Result<(), StateError>;
}

struct NoopRecordSaveHook;

impl RecordSaveHook for NoopRecordSaveHook {
    fn hit(&mut self, _boundary: RecordSaveBoundary) -> Result<(), StateError> {
        Ok(())
    }
}

fn select_record<T>(
    first: Slot<T>,
    second: Slot<T>,
) -> Result<Option<SelectedRecord<T>>, StateError> {
    Ok(match (first, second) {
        (Slot::Valid(first), Slot::Valid(second)) => {
            let redundancy = RecordRedundancy::Complete;
            if first.generation > second.generation {
                Some(SelectedRecord {
                    record: first,
                    redundancy,
                })
            } else if second.generation > first.generation {
                Some(SelectedRecord {
                    record: second,
                    redundancy,
                })
            } else if first.payload_sha256 == second.payload_sha256 {
                Some(SelectedRecord {
                    record: first,
                    redundancy,
                })
            } else {
                return Err(StateError::integrity());
            }
        }
        (Slot::Valid(record), Slot::Missing) | (Slot::Missing, Slot::Valid(record)) => {
            Some(SelectedRecord {
                record,
                redundancy: RecordRedundancy::Single,
            })
        }
        (Slot::Valid(record), Slot::Invalid) | (Slot::Invalid, Slot::Valid(record)) => {
            Some(SelectedRecord {
                record,
                redundancy: RecordRedundancy::Fallback,
            })
        }
        _ => None,
    })
}

fn explicit_save_plan<T>(first: &Slot<T>, second: &Slot<T>) -> Result<SavePlan, StateError> {
    match (first, second) {
        (Slot::Missing, Slot::Invalid) => Ok(SavePlan {
            slot: 0,
            generation: 1,
            target_exists: false,
        }),
        (Slot::Invalid, Slot::Missing) => Ok(SavePlan {
            slot: 1,
            generation: 1,
            target_exists: false,
        }),
        (Slot::Invalid, Slot::Invalid) => Ok(SavePlan {
            slot: 0,
            generation: 1,
            target_exists: true,
        }),
        _ => save_plan(first, second),
    }
}

fn save_plan<T>(first: &Slot<T>, second: &Slot<T>) -> Result<SavePlan, StateError> {
    match (first, second) {
        (Slot::Missing, Slot::Missing) => Ok(SavePlan {
            slot: 0,
            generation: 1,
            target_exists: false,
        }),
        (Slot::Valid(first), Slot::Valid(second)) if first.generation != second.generation => {
            let (slot, highest) = if first.generation > second.generation {
                (1, first.generation)
            } else {
                (0, second.generation)
            };
            Ok(SavePlan {
                slot,
                generation: highest
                    .checked_add(1)
                    .ok_or_else(StateError::capacity_exceeded)?,
                target_exists: true,
            })
        }
        (Slot::Valid(first), Slot::Valid(second)) => {
            if first.payload_sha256 != second.payload_sha256 {
                return Err(StateError::integrity());
            }
            Ok(SavePlan {
                slot: 0,
                generation: first
                    .generation
                    .checked_add(1)
                    .ok_or_else(StateError::capacity_exceeded)?,
                target_exists: true,
            })
        }
        (Slot::Valid(record), other) => Ok(SavePlan {
            slot: 1,
            generation: record
                .generation
                .checked_add(1)
                .ok_or_else(StateError::capacity_exceeded)?,
            target_exists: !matches!(other, Slot::Missing),
        }),
        (other, Slot::Valid(record)) => Ok(SavePlan {
            slot: 0,
            generation: record
                .generation
                .checked_add(1)
                .ok_or_else(StateError::capacity_exceeded)?,
            target_exists: !matches!(other, Slot::Missing),
        }),
        _ => Err(StateError::integrity()),
    }
}

struct JsonMeasureWriter {
    len: u64,
    hasher: Sha256,
    max_bytes: u64,
    capacity_failed: bool,
}

impl Write for JsonMeasureWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let additional =
            u64::try_from(bytes.len()).map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        let Some(next_len) = self
            .len
            .checked_add(additional)
            .filter(|total| *total <= self.max_bytes)
        else {
            self.capacity_failed = true;
            return Err(io::Error::from(io::ErrorKind::OutOfMemory));
        };
        self.hasher.update(bytes);
        self.len = next_len;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct JsonMeasurement {
    len: u64,
    sha256: [u8; 32],
}

fn measure_json<T: Serialize>(value: &T, max_bytes: u64) -> Result<JsonMeasurement, StateError> {
    let mut writer = JsonMeasureWriter {
        len: 0,
        hasher: Sha256::new(),
        max_bytes,
        capacity_failed: false,
    };
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(if writer.capacity_failed {
            StateError::capacity_exceeded()
        } else {
            StateError::invalid_input()
        });
    }
    Ok(JsonMeasurement {
        len: writer.len,
        sha256: writer.hasher.finalize().into(),
    })
}

struct StagedJsonWriter<'a> {
    staged: &'a mut DurableStagedFile,
    expected_len: u64,
    written: u64,
    payload_hasher: Sha256,
    prefix_hasher: Sha256,
    failure: Option<StateError>,
}

impl Write for StagedJsonWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let accepted = bytes.len().min(MAX_DURABLE_WRITE_CHUNK_BYTES);
        let chunk = &bytes[..accepted];
        let additional =
            u64::try_from(accepted).map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        let Some(next_written) = self
            .written
            .checked_add(additional)
            .filter(|total| *total <= self.expected_len)
        else {
            self.failure = Some(StateError::integrity());
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        };
        if let Err(error) = self.staged.write_chunk(chunk) {
            self.failure = Some(map_durable_error(error));
            return Err(io::Error::from(io::ErrorKind::Other));
        }
        self.payload_hasher.update(chunk);
        self.prefix_hasher.update(chunk);
        self.written = next_written;
        Ok(accepted)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct StreamedJson {
    payload_sha256: [u8; 32],
    record_sha256: [u8; 32],
    prefix_hasher: Sha256,
}

fn stream_json<T: Serialize>(
    staged: &mut DurableStagedFile,
    value: &T,
    expected_len: u64,
    header: &[u8; RECORD_HEADER_BYTES],
) -> Result<StreamedJson, StateError> {
    let mut prefix_hasher = Sha256::new();
    prefix_hasher.update(header);
    let mut writer = StagedJsonWriter {
        staged,
        expected_len,
        written: 0,
        payload_hasher: Sha256::new(),
        prefix_hasher,
        failure: None,
    };
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(writer
            .failure
            .take()
            .unwrap_or_else(StateError::invalid_input));
    }
    if writer.written != expected_len {
        return Err(StateError::integrity());
    }
    let payload_sha256: [u8; 32] = writer.payload_hasher.finalize().into();
    let prefix_hasher = writer.prefix_hasher;
    let mut record_hasher = prefix_hasher.clone();
    record_hasher.update(RECORD_FOOTER_MAGIC);
    Ok(StreamedJson {
        payload_sha256,
        record_sha256: record_hasher.finalize().into(),
        prefix_hasher,
    })
}

fn encode_header(
    generation: u64,
    payload_len: usize,
    payload_sha256: [u8; 32],
) -> Result<[u8; RECORD_HEADER_BYTES], StateError> {
    if generation == 0 {
        return Err(StateError::internal_invariant());
    }
    let payload_len = u64::try_from(payload_len).map_err(|_| StateError::capacity_exceeded())?;
    let mut header = [0_u8; RECORD_HEADER_BYTES];
    header[0..8].copy_from_slice(RECORD_MAGIC);
    header[8..10].copy_from_slice(&RECORD_FORMAT_VERSION.to_le_bytes());
    header[10..12].copy_from_slice(&(RECORD_HEADER_BYTES as u16).to_le_bytes());
    header[12..16].copy_from_slice(&0_u32.to_le_bytes());
    header[16..24].copy_from_slice(&generation.to_le_bytes());
    header[24..32].copy_from_slice(&payload_len.to_le_bytes());
    header[32..64].copy_from_slice(&payload_sha256);
    Ok(header)
}

fn decode_record<T: RecordValue>(
    bytes: &[u8],
    max_payload_bytes: u64,
) -> Result<Option<DecodedRecord<T>>, StateError> {
    let Some((generation, payload_sha256, payload)) =
        decode_record_payload(bytes, max_payload_bytes)
    else {
        return Ok(None);
    };
    let value = match T::decode_json(payload) {
        Ok(value) => value,
        Err(RecordValueError::Invalid) => return Ok(None),
        Err(RecordValueError::UnsupportedVersion) => {
            return Err(StateError::unsupported_version());
        }
    };
    Ok(Some(DecodedRecord {
        generation,
        payload_sha256,
        value,
    }))
}

fn decode_record_payload(bytes: &[u8], max_payload_bytes: u64) -> Option<(u64, [u8; 32], &[u8])> {
    if bytes.len() < RECORD_HEADER_BYTES + RECORD_FOOTER_BYTES
        || bytes.get(0..8)? != RECORD_MAGIC
        || read_u16(bytes, 8)? != RECORD_FORMAT_VERSION
        || usize::from(read_u16(bytes, 10)?) != RECORD_HEADER_BYTES
        || read_u32(bytes, 12)? != 0
    {
        return None;
    }
    let generation = read_u64(bytes, 16)?;
    if generation == 0 {
        return None;
    }
    let payload_len_u64 = read_u64(bytes, 24)?;
    if payload_len_u64 > max_payload_bytes {
        return None;
    }
    let payload_len = usize::try_from(payload_len_u64).ok()?;
    let footer_offset = RECORD_HEADER_BYTES.checked_add(payload_len)?;
    let exact_len = footer_offset.checked_add(RECORD_FOOTER_BYTES)?;
    if bytes.len() != exact_len
        || bytes.get(footer_offset..footer_offset.checked_add(8)?)? != RECORD_FOOTER_MAGIC
    {
        return None;
    }
    let payload = bytes.get(RECORD_HEADER_BYTES..footer_offset)?;
    let payload_sha256: [u8; 32] = Sha256::digest(payload).into();
    if bytes.get(32..64)? != payload_sha256 {
        return None;
    }
    let mut record_hasher = Sha256::new();
    record_hasher.update(bytes.get(0..footer_offset.checked_add(8)?)?);
    let record_sha256: [u8; 32] = record_hasher.finalize().into();
    if bytes.get(footer_offset.checked_add(8)?..exact_len)? != record_sha256 {
        return None;
    }
    Some((generation, payload_sha256, payload))
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let mut value = [0_u8; 2];
    value.copy_from_slice(bytes.get(offset..offset.checked_add(2)?)?);
    Some(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let mut value = [0_u8; 4];
    value.copy_from_slice(bytes.get(offset..offset.checked_add(4)?)?);
    Some(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let mut value = [0_u8; 8];
    value.copy_from_slice(bytes.get(offset..offset.checked_add(8)?)?);
    Some(u64::from_le_bytes(value))
}

const fn map_durable_error(error: DurableFileError) -> StateError {
    match error {
        DurableFileError::CapacityExceeded | DurableFileError::CollisionLimit => {
            StateError::capacity_exceeded()
        }
        DurableFileError::Integrity => StateError::integrity(),
        DurableFileError::RecoveryRequired => StateError::recovery_required(),
        DurableFileError::InvalidName | DurableFileError::InvalidState => {
            StateError::internal_invariant()
        }
        DurableFileError::UnsupportedLocation | DurableFileError::UnexpectedType => {
            StateError::invalid_input()
        }
        DurableFileError::TargetExists
        | DurableFileError::TargetMissing
        | DurableFileError::Unavailable => StateError::unavailable(),
    }
}
