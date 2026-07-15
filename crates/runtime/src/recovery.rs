use tokenmaster_engine::{
    Archive, ArchiveReplay, MAX_REPLAY_CONTINUATIONS_PER_RUN, PortError, PortErrorCode,
    ReplayContinuationState,
};
use tokenmaster_store::{AccountingVersions, MAX_SCAN_SCOPES, ReplayRevisionStatus, ScanOutcome};

use crate::StoreArchive;
use crate::error::store_port_error;
use crate::store_archive::archive_replay;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagingRecoveryOutcome {
    None,
    Resumed,
    Discarded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupRecoveryReport {
    orphan_scan_sets_closed: u64,
    orphan_scans_closed: u64,
    staging: StagingRecoveryOutcome,
}

impl StartupRecoveryReport {
    #[must_use]
    pub const fn orphan_scan_sets_closed(self) -> u64 {
        self.orphan_scan_sets_closed
    }

    #[must_use]
    pub const fn orphan_scans_closed(self) -> u64 {
        self.orphan_scans_closed
    }

    #[must_use]
    pub const fn staging(self) -> StagingRecoveryOutcome {
        self.staging
    }
}

pub(crate) fn recover_startup(
    archive: &mut StoreArchive,
) -> Result<StartupRecoveryReport, PortError> {
    let (orphan_scan_sets_closed, orphan_scans_closed) = close_orphan_scan(archive)?;
    let staging = recover_staging(archive)?;
    Ok(StartupRecoveryReport {
        orphan_scan_sets_closed,
        orphan_scans_closed,
        staging,
    })
}

fn close_orphan_scan(archive: &mut StoreArchive) -> Result<(u64, u64), PortError> {
    let Some(scan_set) = archive
        .store()
        .running_scan_set()
        .map_err(|error| store_port_error(&error))?
    else {
        return Ok((0, 0));
    };
    let scans = archive
        .store()
        .scan_page(scan_set.id(), None, MAX_SCAN_SCOPES)
        .map_err(|error| store_port_error(&error))?;
    let expected = usize::try_from(scan_set.expected_scope_count())
        .map_err(|_| PortError::new(PortErrorCode::CapacityExceeded))?;
    if scans.len() != expected || scans.len() > MAX_SCAN_SCOPES {
        return Err(PortError::new(PortErrorCode::InvalidData));
    }

    let mut closed = 0_u64;
    let mut floor = scan_set.started_at_ms();
    for scan in &scans {
        floor = floor.max(scan.started_at_ms());
        if let Some(completed_at) = scan.completed_at_ms() {
            floor = floor.max(completed_at);
        }
        if scan.outcome().is_none() {
            let completed_at = archive.recovery_timestamp_ms(floor)?;
            archive
                .store_mut()
                .finish_scan(
                    scan.id(),
                    ScanOutcome::Failed,
                    completed_at,
                    scan.counters(),
                )
                .map_err(|error| store_port_error(&error))?;
            floor = completed_at;
            closed = closed
                .checked_add(1)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or_else(|| PortError::new(PortErrorCode::CapacityExceeded))?;
        }
    }
    let completed_at = archive.recovery_timestamp_ms(floor)?;
    archive
        .store_mut()
        .finish_scan_set(scan_set.id(), completed_at)
        .map_err(|error| store_port_error(&error))?;
    Ok((1, closed))
}

fn recover_staging(archive: &mut StoreArchive) -> Result<StagingRecoveryOutcome, PortError> {
    let Some(snapshot) = archive
        .store()
        .staging_replay_revision()
        .map_err(|error| store_port_error(&error))?
    else {
        return Ok(StagingRecoveryOutcome::None);
    };
    let replay = archive_replay(snapshot.id(), snapshot.epoch())?;
    if snapshot.status() != ReplayRevisionStatus::Staging
        || snapshot.versions() != AccountingVersions::compiled()
        || snapshot.scan_set_id().is_none()
        || snapshot.promoted()
    {
        archive.discard_replay(replay)?;
        return Ok(StagingRecoveryOutcome::Discarded);
    }
    if snapshot.sealed() {
        return promote_or_discard(archive, replay);
    }

    let mut current = replay;
    let mut continuation_complete = false;
    for _ in 0..MAX_REPLAY_CONTINUATIONS_PER_RUN {
        let continuation = match archive.continue_replay(current) {
            Ok(continuation) => continuation,
            Err(error) => return discard_invalid_or_preserve_unavailable(archive, current, error),
        };
        let next = continuation.replay();
        if next.revision_id() != current.revision_id() || next.epoch() < current.epoch() {
            return discard_invalid_or_preserve_unavailable(
                archive,
                current,
                PortError::new(PortErrorCode::InvalidData),
            );
        }
        current = next;
        if continuation.state() == ReplayContinuationState::Complete {
            continuation_complete = true;
            break;
        }
    }
    if !continuation_complete {
        archive.discard_replay(current)?;
        return Ok(StagingRecoveryOutcome::Discarded);
    }
    let sealed = match archive.seal_replay(current) {
        Ok(sealed) => sealed,
        Err(error) => return discard_invalid_or_preserve_unavailable(archive, current, error),
    };
    if sealed.revision_id() != current.revision_id() || sealed.epoch() < current.epoch() {
        // The returned identity cannot safely authorize deleting either revision.
        return Err(PortError::new(PortErrorCode::InvalidData));
    }
    promote_or_discard(archive, sealed)
}

fn promote_or_discard(
    archive: &mut StoreArchive,
    replay: ArchiveReplay,
) -> Result<StagingRecoveryOutcome, PortError> {
    match archive.promote_replay(replay) {
        Ok(()) => Ok(StagingRecoveryOutcome::Resumed),
        Err(error) => discard_invalid_or_preserve_unavailable(archive, replay, error),
    }
}

fn discard_invalid_or_preserve_unavailable(
    archive: &mut StoreArchive,
    replay: ArchiveReplay,
    error: PortError,
) -> Result<StagingRecoveryOutcome, PortError> {
    if error.code() == PortErrorCode::Unavailable {
        return Err(error);
    }
    archive.discard_replay(replay)?;
    Ok(StagingRecoveryOutcome::Discarded)
}
