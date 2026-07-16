use tokenmaster_domain::{
    GitActivityAssociationId, GitOutputQuality, GitOutputUnavailableReason, GitRepositoryId,
};
use tokenmaster_git::{
    GitAuthorFingerprint, GitCommitAccumulator, GitCommitFingerprint, GitMailmapFingerprint,
    GitObjectFormat, GitPathStat, GitRefFingerprint, GitScanAccumulator, GitScanSummary,
};
use tokenmaster_store::{
    GitCacheIdentity, GitProjectKey, GitProjectionInput, GitProjectionInputParts,
};

pub fn summary(day: i32, added: u64, removed: u64) -> GitScanSummary {
    let mut commit =
        GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([9; 32]), day, 1)
            .expect("commit accumulator");
    commit
        .record(GitPathStat::text(b"src/main.rs", added, removed).expect("path stat"))
        .expect("record path");
    let mut scan = GitScanAccumulator::new();
    scan.push(commit.finish().expect("finish commit"))
        .expect("scan commit");
    scan.finish().expect("scan summary")
}

pub fn summary_range(start_day: i32, day_count: usize) -> GitScanSummary {
    let mut scan = GitScanAccumulator::new();
    for offset in 0..day_count {
        let mut fingerprint = [0_u8; 32];
        fingerprint[..8].copy_from_slice(
            &u64::try_from(offset)
                .expect("bounded offset")
                .to_be_bytes(),
        );
        let day = start_day
            .checked_add(i32::try_from(offset).expect("bounded day offset"))
            .expect("bounded day");
        let mut commit =
            GitCommitAccumulator::new(GitCommitFingerprint::from_bytes(fingerprint), day, 1)
                .expect("commit accumulator");
        commit
            .record(GitPathStat::text(b"src/main.rs", 1, 0).expect("path stat"))
            .expect("record path");
        scan.push(commit.finish().expect("finish commit"))
            .expect("scan commit");
    }
    scan.finish().expect("scan summary")
}

pub fn cache(heads_seed: u8) -> GitCacheIdentity {
    GitCacheIdentity::new(
        GitObjectFormat::Sha1,
        GitRefFingerprint::from_bytes([heads_seed; 32]),
        GitMailmapFingerprint::from_bytes([2; 32]),
        GitAuthorFingerprint::from_bytes([3; 32]),
        1,
        false,
    )
    .expect("cache identity")
}

pub fn input(
    repository_seed: u8,
    association_seed: u8,
    heads_seed: u8,
    observed_at_ms: i64,
    summary: GitScanSummary,
) -> GitProjectionInput {
    GitProjectionInput::new(GitProjectionInputParts {
        repository_id: GitRepositoryId::from_bytes([repository_seed; 32]),
        association_id: GitActivityAssociationId::from_bytes([association_seed; 32]),
        project_key: Some(GitProjectKey::from_bytes([association_seed; 32])),
        activity_at_ms: observed_at_ms - 1,
        observed_at_ms,
        data_through_ms: Some(observed_at_ms - 1),
        quality: GitOutputQuality::Complete,
        unavailable_reason: None,
        warnings: Vec::new(),
        summary: Some(summary),
        cache: Some(cache(heads_seed)),
    })
    .expect("projection input")
}

pub fn unavailable_input(
    repository_seed: u8,
    association_seed: u8,
    observed_at_ms: i64,
) -> GitProjectionInput {
    GitProjectionInput::new(GitProjectionInputParts {
        repository_id: GitRepositoryId::from_bytes([repository_seed; 32]),
        association_id: GitActivityAssociationId::from_bytes([association_seed; 32]),
        project_key: None,
        activity_at_ms: observed_at_ms - 1,
        observed_at_ms,
        data_through_ms: None,
        quality: GitOutputQuality::Unavailable,
        unavailable_reason: Some(GitOutputUnavailableReason::AuthorIdentityMissing),
        warnings: Vec::new(),
        summary: None,
        cache: None,
    })
    .expect("unavailable projection")
}
