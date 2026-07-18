use core::fmt;

use tokenmaster_platform::{BackupDirectory, BackupDirectoryError, MAX_BACKUP_DIRECTORY_FILES};

use crate::catalog::CatalogDirectoryIdentity;
use crate::{
    BACKUP_RETENTION_DEFAULT_BYTES, BACKUP_RETENTION_MAX_BYTES, BACKUP_RETENTION_MIN_BYTES,
    BackupCatalog, BackupMetadata, BackupPurpose, CatalogGeneration, CatalogHealth,
    CatalogSelection, StateError, VerifiedBackupPackage,
};

pub const RETENTION_NEWEST_POINTS: usize = 4;
pub const RETENTION_DAILY_POINTS: usize = 7;
pub const RETENTION_WEEKLY_POINTS: usize = 4;
pub const MAX_RETAINED_VERIFIED_POINTS: usize = 15;

const UTC_DAY_MILLISECONDS: i64 = 86_400_000;

/// Exact compressed-byte budget for automatic backup retention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    budget_bytes: u64,
}

impl RetentionPolicy {
    pub fn new(budget_bytes: u64) -> Result<Self, StateError> {
        if !(BACKUP_RETENTION_MIN_BYTES..=BACKUP_RETENTION_MAX_BYTES).contains(&budget_bytes) {
            return Err(StateError::invalid_input());
        }
        Ok(Self { budget_bytes })
    }

    #[must_use]
    pub const fn budget_bytes(self) -> u64 {
        self.budget_bytes
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            budget_bytes: BACKUP_RETENTION_DEFAULT_BYTES,
        }
    }
}

/// Pre-publication capacity proof. This value has no deletion authority.
pub struct RetentionAdmission {
    prior_catalog_generation: CatalogGeneration,
    prior_directory_identity: CatalogDirectoryIdentity,
    prior_hashes: Vec<[u8; 32]>,
    candidate_hash: [u8; 32],
    candidate_size: u64,
    candidate_metadata: BackupMetadata,
    policy: RetentionPolicy,
}

impl RetentionAdmission {
    /// Checks the proposed verified package without deleting or publishing anything.
    pub fn preflight(
        catalog: &BackupCatalog,
        candidate: &VerifiedBackupPackage,
        policy: RetentionPolicy,
    ) -> Result<Self, StateError> {
        if catalog.points().len() >= MAX_BACKUP_DIRECTORY_FILES {
            return Err(StateError::capacity_exceeded());
        }
        let receipt = candidate.receipt();
        let candidate_hash = *receipt.file_sha256();
        if catalog
            .points()
            .iter()
            .any(|point| point.observed_file_sha256 == candidate_hash)
        {
            return Err(StateError::integrity());
        }
        let candidate_fact = RetentionFact {
            key: candidate_hash,
            created_at_utc_ms: Some(candidate.metadata().created_at_utc_ms()),
            purpose: Some(candidate.metadata().purpose()),
            size_bytes: receipt.package_len(),
            verified: true,
            candidate: true,
            selection: None,
        };
        let _ = plan_retention(catalog, Some(candidate_fact), candidate_hash, policy)?;
        Ok(Self {
            prior_catalog_generation: catalog.generation(),
            prior_directory_identity: catalog.directory_identity(),
            prior_hashes: catalog
                .points()
                .iter()
                .map(|point| point.observed_file_sha256)
                .collect(),
            candidate_hash,
            candidate_size: receipt.package_len(),
            candidate_metadata: candidate.metadata(),
            policy,
        })
    }

    /// Confirms that exactly the admitted package was added and fully verified.
    pub fn confirm_published(
        self,
        catalog: &BackupCatalog,
        published: CatalogSelection,
    ) -> Result<RetentionCycle, StateError> {
        let expected_generation = self
            .prior_catalog_generation
            .get()
            .checked_add(1)
            .ok_or_else(StateError::capacity_exceeded)?;
        if catalog.generation().get() != expected_generation
            || catalog.directory_identity() == self.prior_directory_identity
            || catalog.points().len() != self.prior_hashes.len().saturating_add(1)
            || self.prior_hashes.iter().any(|prior| {
                !catalog
                    .points()
                    .iter()
                    .any(|point| point.observed_file_sha256 == *prior)
            })
        {
            return Err(StateError::recovery_required());
        }
        let point = catalog
            .points()
            .get(usize::from(published.ordinal()))
            .filter(|point| point.selection == published)
            .ok_or_else(StateError::invalid_input)?;
        if point.health != CatalogHealth::Verified
            || point.observed_file_sha256 != self.candidate_hash
            || point.size_bytes() != self.candidate_size
            || point.created_at_utc_ms != Some(self.candidate_metadata.created_at_utc_ms())
            || point.purpose != Some(self.candidate_metadata.purpose())
        {
            return Err(StateError::integrity());
        }
        let cycle = RetentionCycle {
            candidate_hash: self.candidate_hash,
            policy: self.policy,
        };
        let _ = cycle.next_deletion(catalog)?;
        Ok(cycle)
    }
}

impl fmt::Debug for RetentionAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RetentionAdmission([redacted])")
    }
}

/// Post-publication authority to recompute and delete one exact point at a time.
pub struct RetentionCycle {
    candidate_hash: [u8; 32],
    policy: RetentionPolicy,
}

impl RetentionCycle {
    /// Returns only the next oldest exact deletion under the current catalog generation.
    pub fn next_deletion(
        &self,
        catalog: &BackupCatalog,
    ) -> Result<Option<CatalogSelection>, StateError> {
        let candidate = catalog
            .points()
            .iter()
            .find(|point| point.observed_file_sha256 == self.candidate_hash)
            .ok_or_else(StateError::recovery_required)?;
        if candidate.health != CatalogHealth::Verified {
            return Err(StateError::recovery_required());
        }
        Ok(
            plan_retention(catalog, None, self.candidate_hash, self.policy)?
                .deletions
                .first()
                .copied(),
        )
    }

    /// Deletes at most one exact current point. The caller must rebuild before retrying.
    pub fn delete_next(
        &self,
        catalog: &BackupCatalog,
        directory: &BackupDirectory,
    ) -> Result<bool, StateError> {
        let candidate = catalog
            .points()
            .iter()
            .find(|point| point.observed_file_sha256 == self.candidate_hash)
            .ok_or_else(StateError::recovery_required)?;
        if candidate.health != CatalogHealth::Verified
            || !catalog.revalidate_all_verified(directory)?
        {
            return Err(StateError::recovery_required());
        }
        let Some(selection) = self.next_deletion(catalog)? else {
            return Ok(false);
        };
        if !catalog.matches_directory(directory)? {
            return Err(StateError::recovery_required());
        }
        let point = catalog
            .points()
            .get(usize::from(selection.ordinal()))
            .filter(|point| point.selection == selection)
            .ok_or_else(StateError::recovery_required)?;
        if point.health != CatalogHealth::Verified
            || point.observed_file_sha256 == self.candidate_hash
            || !catalog.revalidate_point(directory, selection)?
        {
            return Err(StateError::recovery_required());
        }
        directory
            .delete(&point.entry)
            .map_err(map_directory_error)?;
        Ok(true)
    }
}

impl fmt::Debug for RetentionCycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RetentionCycle([redacted])")
    }
}

#[derive(Clone)]
struct RetentionFact {
    key: [u8; 32],
    created_at_utc_ms: Option<i64>,
    purpose: Option<BackupPurpose>,
    size_bytes: u64,
    verified: bool,
    candidate: bool,
    selection: Option<CatalogSelection>,
}

struct RetentionOutcome {
    deletions: Vec<CatalogSelection>,
    #[cfg(test)]
    retained_keys: Vec<[u8; 32]>,
}

fn plan_retention(
    catalog: &BackupCatalog,
    candidate: Option<RetentionFact>,
    candidate_hash: [u8; 32],
    policy: RetentionPolicy,
) -> Result<RetentionOutcome, StateError> {
    let mut facts = catalog
        .points()
        .iter()
        .map(|point| RetentionFact {
            key: point.observed_file_sha256,
            created_at_utc_ms: point.created_at_utc_ms,
            purpose: point.purpose,
            size_bytes: point.size_bytes(),
            verified: point.health == CatalogHealth::Verified,
            candidate: point.observed_file_sha256 == candidate_hash,
            selection: Some(point.selection),
        })
        .collect::<Vec<_>>();
    if let Some(candidate) = candidate {
        facts.push(candidate);
    }
    plan_facts(facts, policy)
}

fn plan_facts(
    mut facts: Vec<RetentionFact>,
    policy: RetentionPolicy,
) -> Result<RetentionOutcome, StateError> {
    facts.sort_unstable_by(|left, right| {
        right
            .created_at_utc_ms
            .unwrap_or(i64::MIN)
            .cmp(&left.created_at_utc_ms.unwrap_or(i64::MIN))
            .then_with(|| left.key.cmp(&right.key))
    });
    let verified = facts
        .iter()
        .enumerate()
        .filter_map(|(index, fact)| fact.verified.then_some(index))
        .collect::<Vec<_>>();
    let mut keep = vec![false; facts.len()];
    let mut protected = vec![false; facts.len()];

    for &index in &verified {
        if facts[index].candidate {
            protected[index] = true;
        }
    }
    for &index in verified.iter().take(2) {
        protected[index] = true;
    }
    if let Some(pre_migration) = verified.iter().copied().find(|index| {
        facts[*index].purpose == Some(BackupPurpose::PreMigration)
            && !verified.iter().any(|post| {
                facts[*post].purpose == Some(BackupPurpose::PostMigration)
                    && facts[*post].created_at_utc_ms > facts[*index].created_at_utc_ms
            })
    }) {
        protected[pre_migration] = true;
    }

    let protected_verified_count = verified.iter().filter(|index| protected[**index]).count();
    if protected_verified_count > MAX_RETAINED_VERIFIED_POINTS {
        return Err(StateError::capacity_exceeded());
    }
    for &index in &verified {
        keep[index] = protected[index];
    }

    for &index in verified.iter().take(RETENTION_NEWEST_POINTS) {
        add_if_capacity(&mut keep, index);
    }

    let mut represented_days = keep
        .iter()
        .enumerate()
        .filter_map(|(index, kept)| {
            (*kept)
                .then(|| facts[index].created_at_utc_ms.map(utc_day_bucket))
                .flatten()
        })
        .collect::<Vec<_>>();
    let mut daily_added = 0_usize;
    for &index in &verified {
        if daily_added >= RETENTION_DAILY_POINTS || retained_verified_count(&keep, &verified) >= 15
        {
            break;
        }
        let Some(day) = facts[index].created_at_utc_ms.map(utc_day_bucket) else {
            continue;
        };
        if !keep[index] && !represented_days.contains(&day) {
            keep[index] = true;
            represented_days.push(day);
            daily_added += 1;
        }
    }

    let mut represented_weeks = keep
        .iter()
        .enumerate()
        .filter_map(|(index, kept)| {
            (*kept)
                .then(|| facts[index].created_at_utc_ms.map(utc_week_bucket))
                .flatten()
        })
        .collect::<Vec<_>>();
    let mut weekly_added = 0_usize;
    for &index in &verified {
        if weekly_added >= RETENTION_WEEKLY_POINTS
            || retained_verified_count(&keep, &verified) >= 15
        {
            break;
        }
        let Some(week) = facts[index].created_at_utc_ms.map(utc_week_bucket) else {
            continue;
        };
        if !keep[index] && !represented_weeks.contains(&week) {
            keep[index] = true;
            represented_weeks.push(week);
            weekly_added += 1;
        }
    }

    let mut kept_bytes = facts
        .iter()
        .enumerate()
        .try_fold(0_u64, |total, (index, fact)| {
            if !fact.verified || keep[index] {
                total
                    .checked_add(fact.size_bytes)
                    .ok_or_else(StateError::capacity_exceeded)
            } else {
                Ok(total)
            }
        })?;
    while kept_bytes > policy.budget_bytes {
        let removable = verified
            .iter()
            .rev()
            .copied()
            .find(|index| keep[*index] && !protected[*index]);
        let Some(removable) = removable else {
            return Err(StateError::capacity_exceeded());
        };
        keep[removable] = false;
        kept_bytes = kept_bytes
            .checked_sub(facts[removable].size_bytes)
            .ok_or_else(StateError::internal_invariant)?;
    }

    let mut deletions = verified
        .iter()
        .rev()
        .filter_map(|index| (!keep[*index]).then_some(facts[*index].selection).flatten())
        .collect::<Vec<_>>();
    deletions.dedup();
    #[cfg(test)]
    let retained_keys = verified
        .iter()
        .filter_map(|index| keep[*index].then_some(facts[*index].key))
        .collect();
    Ok(RetentionOutcome {
        deletions,
        #[cfg(test)]
        retained_keys,
    })
}

fn add_if_capacity(keep: &mut [bool], index: usize) {
    if keep.iter().filter(|kept| **kept).count() < MAX_RETAINED_VERIFIED_POINTS {
        keep[index] = true;
    }
}

fn retained_verified_count(keep: &[bool], verified: &[usize]) -> usize {
    verified.iter().filter(|index| keep[**index]).count()
}

const fn utc_day_bucket(created_at_utc_ms: i64) -> i64 {
    created_at_utc_ms / UTC_DAY_MILLISECONDS
}

const fn utc_week_bucket(created_at_utc_ms: i64) -> i64 {
    (utc_day_bucket(created_at_utc_ms) + 3) / 7
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
    use super::{RetentionFact, RetentionPolicy, plan_facts};
    use crate::{BACKUP_RETENTION_MIN_BYTES, BackupPurpose, StateErrorCode};

    fn fact(
        key: u8,
        time: i64,
        purpose: BackupPurpose,
        size_bytes: u64,
        candidate: bool,
    ) -> RetentionFact {
        RetentionFact {
            key: [key; 32],
            created_at_utc_ms: Some(time),
            purpose: Some(purpose),
            size_bytes,
            verified: true,
            candidate,
            selection: None,
        }
    }

    #[test]
    fn post_migration_unpins_pre_migration_but_protected_bytes_fail_before_it() {
        let mebibyte = 1024 * 1024;
        let policy = RetentionPolicy {
            budget_bytes: BACKUP_RETENTION_MIN_BYTES,
        };
        let without_post = vec![
            fact(1, 300, BackupPurpose::Periodic, 80 * mebibyte, true),
            fact(2, 200, BackupPurpose::Periodic, 80 * mebibyte, false),
            fact(3, 100, BackupPurpose::PreMigration, 100 * mebibyte, false),
        ];
        let error = match plan_facts(without_post, policy) {
            Ok(_) => panic!("protected bytes must exceed budget"),
            Err(error) => error,
        };
        assert_eq!(error.code(), StateErrorCode::CapacityExceeded);

        let with_post = vec![
            fact(1, 300, BackupPurpose::PostMigration, 80 * mebibyte, true),
            fact(2, 200, BackupPurpose::Periodic, 80 * mebibyte, false),
            fact(3, 100, BackupPurpose::PreMigration, 100 * mebibyte, false),
        ];
        assert!(plan_facts(with_post, policy).is_ok());
    }

    #[test]
    fn admitted_candidate_is_protected_even_when_its_clock_is_older() {
        let mebibyte = 1024 * 1024;
        let policy = RetentionPolicy {
            budget_bytes: BACKUP_RETENTION_MIN_BYTES,
        };
        let facts = vec![
            fact(1, 400, BackupPurpose::Periodic, 80 * mebibyte, false),
            fact(2, 300, BackupPurpose::Periodic, 80 * mebibyte, false),
            fact(3, 100, BackupPurpose::Periodic, 100 * mebibyte, true),
        ];
        let error = match plan_facts(facts, policy) {
            Ok(_) => panic!("time-skewed candidate bytes must stay protected"),
            Err(error) => error,
        };
        assert_eq!(error.code(), StateErrorCode::CapacityExceeded);
    }

    #[test]
    fn tier_selection_adds_exact_four_newest_seven_daily_and_four_weekly() {
        const DAY: i64 = 86_400_000;
        let base = 1_735_689_600_000_i64;
        let mut facts = vec![fact(1, base, BackupPurpose::Periodic, 1, true)];
        for key in 2..=8 {
            facts.push(fact(
                key,
                base - i64::from(key),
                BackupPurpose::Periodic,
                1,
                false,
            ));
        }
        for day in 1..=8 {
            facts.push(fact(
                8 + day,
                base - i64::from(day) * DAY,
                BackupPurpose::Periodic,
                1,
                false,
            ));
        }
        for week in 2..=9 {
            facts.push(fact(
                15 + week,
                base - i64::from(week) * 7 * DAY,
                BackupPurpose::Periodic,
                1,
                false,
            ));
        }
        let policy = RetentionPolicy {
            budget_bytes: BACKUP_RETENTION_MIN_BYTES,
        };
        let outcome = match plan_facts(facts, policy) {
            Ok(outcome) => outcome,
            Err(error) => panic!("tier selection failed: {error}"),
        };
        assert_eq!(outcome.retained_keys.len(), 15);
        assert!(outcome.retained_keys.contains(&[1; 32]));
        assert_eq!(
            outcome
                .retained_keys
                .iter()
                .filter(|key| (1..=8).contains(&key[0]))
                .count(),
            4
        );
        assert_eq!(
            outcome
                .retained_keys
                .iter()
                .filter(|key| (9..=16).contains(&key[0]))
                .count(),
            7
        );
        assert_eq!(
            outcome
                .retained_keys
                .iter()
                .filter(|key| (17..=24).contains(&key[0]))
                .count(),
            4
        );
    }
}
