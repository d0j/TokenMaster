mod package_support;

use std::fs;

use tempfile::TempDir;
use tokenmaster_platform::{
    BackupDirectory, BackupDirectoryEntry, MAX_DURABLE_FILE_BYTES, ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BACKUP_RETENTION_DEFAULT_BYTES, BACKUP_RETENTION_MAX_BYTES, BACKUP_RETENTION_MIN_BYTES,
    BackupCatalog, BackupCompression, BackupPurpose, CatalogHealth, RetentionAdmission,
    RetentionPolicy, StateErrorCode,
};

use package_support::{backup_bytes_at, digest, read_backup_bytes};

fn fixture() -> (TempDir, BackupDirectory) {
    let root = TempDir::new().expect("retention root");
    let validated = ValidatedLocalDirectory::new(root.path()).expect("validated root");
    let directory = BackupDirectory::open_or_create(&validated).expect("backup directory");
    (root, directory)
}

fn publish(directory: &BackupDirectory, bytes: &[u8]) -> BackupDirectoryEntry {
    let mut staged = directory
        .create_staged(MAX_DURABLE_FILE_BYTES)
        .expect("backup stage");
    staged.write_chunk(bytes).expect("write backup");
    staged
        .seal(bytes.len() as u64, digest(bytes))
        .expect("seal backup");
    directory.publish(&mut staged).expect("publish backup")
}

#[test]
fn policy_range_is_exact_and_default_is_two_gib() {
    assert_eq!(
        RetentionPolicy::default().budget_bytes(),
        BACKUP_RETENTION_DEFAULT_BYTES
    );
    assert_eq!(
        RetentionPolicy::new(BACKUP_RETENTION_MIN_BYTES)
            .expect("minimum")
            .budget_bytes(),
        BACKUP_RETENTION_MIN_BYTES
    );
    assert_eq!(
        RetentionPolicy::new(BACKUP_RETENTION_MAX_BYTES)
            .expect("maximum")
            .budget_bytes(),
        BACKUP_RETENTION_MAX_BYTES
    );
    for invalid in [
        BACKUP_RETENTION_MIN_BYTES - 1,
        BACKUP_RETENTION_MAX_BYTES + 1,
    ] {
        assert_eq!(
            RetentionPolicy::new(invalid)
                .expect_err("out-of-range budget")
                .code(),
            StateErrorCode::InvalidInput
        );
    }
}

#[test]
fn post_publication_cycle_keeps_bounded_tiers_and_replans_after_each_delete() {
    let (root, directory) = fixture();
    let day_ms = 86_400_000_i64;
    let base = 1_735_689_600_000_i64;
    let mut proofs = Vec::new();

    for age_days in (1..=20).rev() {
        let created_at = base - i64::from(age_days) * day_ms;
        let purpose = if age_days == 18 {
            BackupPurpose::PreMigration
        } else {
            BackupPurpose::Periodic
        };
        let (bytes, _) = backup_bytes_at(
            format!("SQLite format 3\0retention-{age_days:02}").as_bytes(),
            BackupCompression::Normal,
            purpose,
            created_at,
        );
        let (verified, _) = read_backup_bytes(&bytes).expect("verified historical package");
        publish(&directory, &bytes);
        proofs.push((created_at, bytes, verified));
    }

    let mut catalog = BackupCatalog::rebuild(&directory, None).expect("cold catalog");
    for point_index in 0..catalog.points().len() {
        let point = &catalog.points()[point_index];
        let created_at = point.created_at_utc_ms().expect("valid point time");
        let selection = point.selection();
        let proof = proofs
            .iter()
            .find(|(time, _, _)| *time == created_at)
            .map(|(_, _, proof)| proof)
            .expect("matching proof");
        catalog
            .bind_verified(selection, proof)
            .expect("bind historical proof");
    }

    let candidate_time = base;
    let (candidate_bytes, _) = backup_bytes_at(
        b"SQLite format 3\0new retention candidate",
        BackupCompression::Normal,
        BackupPurpose::Periodic,
        candidate_time,
    );
    let (candidate, _) = read_backup_bytes(&candidate_bytes).expect("verified candidate");
    let admission = RetentionAdmission::preflight(&catalog, &candidate, RetentionPolicy::default())
        .expect("capacity preflight");
    assert_eq!(format!("{admission:?}"), "RetentionAdmission([redacted])");

    publish(&directory, &candidate_bytes);
    let mut published = BackupCatalog::rebuild(&directory, Some(&catalog)).expect("published scan");
    let candidate_selection = published
        .points()
        .iter()
        .find(|point| point.created_at_utc_ms() == Some(candidate_time))
        .expect("published candidate")
        .selection();
    published
        .bind_verified(candidate_selection, &candidate)
        .expect("bind published proof");
    let cycle = admission
        .confirm_published(&published, candidate_selection)
        .expect("confirm publication");
    assert_eq!(format!("{cycle:?}"), "RetentionCycle([redacted])");

    let candidate_path = fs::read_dir(root.path().join("backups"))
        .expect("backup directory")
        .map(|entry| entry.expect("backup entry").path())
        .find(|path| fs::read(path).expect("backup bytes") == candidate_bytes)
        .expect("candidate path");
    let mut changed_candidate = candidate_bytes.clone();
    let changed_index = changed_candidate.len() / 2;
    changed_candidate[changed_index] ^= 1;
    fs::write(&candidate_path, &changed_candidate).expect("same-length candidate corruption");
    let before_failed_delete = directory.scan().expect("pre-failure scan").entries().len();
    assert_eq!(
        cycle
            .delete_next(&published, &directory)
            .expect_err("changed candidate blocks every deletion")
            .code(),
        StateErrorCode::RecoveryRequired
    );
    assert_eq!(
        directory.scan().expect("post-failure scan").entries().len(),
        before_failed_delete
    );
    fs::write(&candidate_path, &candidate_bytes).expect("restore exact candidate bytes");

    let deletion_selection = cycle
        .next_deletion(&published)
        .expect("planned deletion")
        .expect("one old point must be deletable");
    let deletion_time = published.points()[usize::from(deletion_selection.ordinal())]
        .created_at_utc_ms()
        .expect("deletion point time");
    let deletion_bytes = proofs
        .iter()
        .find(|(time, _, _)| *time == deletion_time)
        .map(|(_, bytes, _)| bytes)
        .expect("deletion point bytes");
    let deletion_path = fs::read_dir(root.path().join("backups"))
        .expect("backup directory")
        .map(|entry| entry.expect("backup entry").path())
        .find(|path| fs::read(path).expect("backup bytes") == *deletion_bytes)
        .expect("deletion point path");
    let mut changed_deletion = deletion_bytes.clone();
    let changed_index = changed_deletion.len() / 2;
    changed_deletion[changed_index] ^= 1;
    fs::write(&deletion_path, &changed_deletion).expect("same-length deletion target corruption");
    let before_stale_target = directory.scan().expect("pre-target scan").entries().len();
    assert_eq!(
        cycle
            .delete_next(&published, &directory)
            .expect_err("changed deletion target blocks deletion")
            .code(),
        StateErrorCode::RecoveryRequired
    );
    assert_eq!(
        directory.scan().expect("post-target scan").entries().len(),
        before_stale_target
    );
    fs::write(&deletion_path, deletion_bytes).expect("restore exact deletion target bytes");

    let changed_protected_time = base - day_ms;
    let (changed_protected_bytes, changed_protected_proof) = proofs
        .iter()
        .find(|(time, _, _)| *time == changed_protected_time)
        .map(|(_, bytes, proof)| (bytes, proof))
        .expect("protected point fixture");
    let changed_protected_path = fs::read_dir(root.path().join("backups"))
        .expect("backup directory")
        .map(|entry| entry.expect("backup entry").path())
        .find(|path| fs::read(path).expect("backup bytes") == *changed_protected_bytes)
        .expect("protected point path");
    let mut corrupted_protected = changed_protected_bytes.clone();
    let changed_index = corrupted_protected.len() / 2;
    corrupted_protected[changed_index] ^= 1;
    fs::write(&changed_protected_path, &corrupted_protected)
        .expect("same-length non-target protected corruption");
    let before_stale_plan = directory.scan().expect("pre-plan scan").entries().len();
    assert_eq!(
        cycle
            .delete_next(&published, &directory)
            .expect_err("changed protected point invalidates the whole plan")
            .code(),
        StateErrorCode::RecoveryRequired
    );
    assert_eq!(
        directory.scan().expect("post-plan scan").entries().len(),
        before_stale_plan
    );

    let changed_catalog =
        BackupCatalog::rebuild(&directory, Some(&published)).expect("rebuild changed catalog");
    assert_eq!(
        changed_catalog
            .points()
            .iter()
            .find(|point| point.created_at_utc_ms() == Some(changed_protected_time))
            .expect("changed catalog point")
            .health(),
        CatalogHealth::HeaderValid
    );
    let promoted_newest_time = base - 2 * day_ms;
    let replanned_deletion = cycle
        .next_deletion(&changed_catalog)
        .expect("replan with changed point")
        .expect("one old point remains deletable");
    assert_ne!(
        changed_catalog.points()[usize::from(replanned_deletion.ordinal())].created_at_utc_ms(),
        Some(promoted_newest_time),
        "newly protected verified point must not be deleted"
    );

    fs::write(&changed_protected_path, changed_protected_bytes)
        .expect("restore exact protected point bytes");
    let mut current = BackupCatalog::rebuild(&directory, Some(&changed_catalog))
        .expect("rebuild restored protected point");
    let restored_selection = current
        .points()
        .iter()
        .find(|point| point.created_at_utc_ms() == Some(changed_protected_time))
        .expect("restored protected point")
        .selection();
    current
        .bind_verified(restored_selection, changed_protected_proof)
        .expect("rebind restored protected proof");

    let mut deletion_count = 0_usize;
    while cycle
        .next_deletion(&current)
        .expect("deterministic next deletion")
        .is_some()
    {
        assert!(
            cycle
                .delete_next(&current, &directory)
                .expect("delete one exact point")
        );
        deletion_count += 1;
        assert!(deletion_count <= 32, "bounded deletion loop");
        current = BackupCatalog::rebuild(&directory, Some(&current)).expect("rebuild after delete");
    }

    assert!(deletion_count > 0);
    let verified = current
        .points()
        .iter()
        .filter(|point| point.health() == CatalogHealth::Verified)
        .collect::<Vec<_>>();
    assert!(verified.len() <= 15);
    assert!(verified.iter().any(|point| {
        point.created_at_utc_ms() == Some(base - 18 * day_ms)
            && point.purpose() == Some(BackupPurpose::PreMigration)
    }));
    for newest in 0..4 {
        let expected = base - i64::from(newest) * day_ms;
        assert!(
            verified
                .iter()
                .any(|point| point.created_at_utc_ms() == Some(expected)),
            "newest point {newest} must remain"
        );
    }
    let rendered = format!("{current:?}");
    assert!(!rendered.contains(root.path().to_string_lossy().as_ref()));
    assert!(!rendered.contains("sha256"));
}
