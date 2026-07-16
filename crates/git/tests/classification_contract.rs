use tokenmaster_domain::{GitLineMetrics, GitOutputCategory};
use tokenmaster_git::{
    GitAggregateBatch, GitCommitAccumulator, GitCommitFingerprint, GitCoreError, GitIdentitySalt,
    GitPathStat, GitRefHead, GitScanAccumulator, MAX_GIT_COMMITS_PER_BATCH,
    MAX_GIT_PATHS_PER_COMMIT, MAX_GIT_REFS, classify_destination_path, derive_author_fingerprint,
    derive_commit_fingerprint, derive_ref_fingerprint, derive_repository_id,
};

fn salt(seed: u8) -> GitIdentitySalt {
    GitIdentitySalt::from_bytes([seed; 32])
}

fn commit(seed: u8, day: i32) -> tokenmaster_git::GitCommitAggregate {
    GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([seed; 32]), day, 1)
        .expect("valid commit")
        .finish()
        .expect("finish commit")
}

#[test]
fn identities_are_deterministic_framed_ordered_and_redacted() {
    let first_salt = salt(1);
    let second_salt = salt(2);
    let repository = derive_repository_id(&first_salt, b"c:/code/tokenmaster/.git")
        .expect("repository identity");
    assert_eq!(
        repository,
        derive_repository_id(&first_salt, b"c:/code/tokenmaster/.git")
            .expect("stable repository identity")
    );
    assert_ne!(
        repository,
        derive_repository_id(&second_salt, b"c:/code/tokenmaster/.git")
            .expect("salt-separated repository identity")
    );

    let author =
        derive_author_fingerprint(&first_salt, b"  User@Example.COM ").expect("author fingerprint");
    assert_eq!(
        author,
        derive_author_fingerprint(&first_salt, b"user@example.com")
            .expect("normalized author fingerprint")
    );
    assert_eq!(
        derive_author_fingerprint(&first_salt, "ПОЛЬЗОВАТЕЛЬ@ПРИМЕР.РФ".as_bytes())
            .expect("international author"),
        derive_author_fingerprint(&first_salt, "пользователь@пример.рф".as_bytes())
            .expect("normalized international author")
    );
    assert_eq!(format!("{author:?}"), "GitAuthorFingerprint([redacted])");

    let commit =
        derive_commit_fingerprint(&first_salt, b"0123456789012345678901234567890123456789")
            .expect("commit fingerprint");
    assert_eq!(format!("{commit:?}"), "GitCommitFingerprint([redacted])");

    let refs_a = vec![
        GitRefHead::new(
            b"refs/heads/main",
            b"0123456789012345678901234567890123456789",
        )
        .expect("main ref"),
        GitRefHead::new(
            b"refs/heads/feature",
            b"abcdefabcdefabcdefabcdefabcdefabcdefabcd",
        )
        .expect("feature ref"),
    ];
    let refs_b = vec![refs_a[1].clone(), refs_a[0].clone()];
    assert_eq!(
        derive_ref_fingerprint(&first_salt, &refs_a).expect("ref fingerprint"),
        derive_ref_fingerprint(&first_salt, &refs_b).expect("order-neutral ref fingerprint")
    );
    assert_eq!(
        derive_ref_fingerprint(&first_salt, &[refs_a[0].clone(), refs_a[0].clone()]),
        Err(GitCoreError::DuplicateValue)
    );

    let too_many = (0..=MAX_GIT_REFS)
        .map(|index| {
            let name = format!("refs/heads/{index}");
            GitRefHead::new(name.as_bytes(), b"0123456789012345678901234567890123456789")
                .expect("bounded ref")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        derive_ref_fingerprint(&first_salt, &too_many),
        Err(GitCoreError::CapacityExceeded {
            limit: MAX_GIT_REFS
        })
    );
}

#[test]
fn classifier_uses_destination_path_precedence_without_path_retention() {
    let cases: &[(&[u8], GitOutputCategory)] = &[
        (b"src/main.rs", GitOutputCategory::ProductCode),
        (b"src/lib.test.ts", GitOutputCategory::Test),
        (b"tests/api.rs", GitOutputCategory::Test),
        (b"spec/widget_spec.rb", GitOutputCategory::Test),
        (b"docs/design.md", GitOutputCategory::DocsSpec),
        (b"SPECIFICATION.md", GitOutputCategory::DocsSpec),
        (
            b"migrations/20260716_create.sql",
            GitOutputCategory::SchemaMigration,
        ),
        (b".github/workflows/ci.yml", GitOutputCategory::ConfigBuild),
        (b"Cargo.toml", GitOutputCategory::ConfigBuild),
        (b"Cargo.lock", GitOutputCategory::VendorGenerated),
        (b"web/app.min.js", GitOutputCategory::VendorGenerated),
        (
            b"third_party/reference/src/main.go",
            GitOutputCategory::VendorGenerated,
        ),
        (b"assets/logo.svg", GitOutputCategory::Asset),
        (b"notes/unknown.txt", GitOutputCategory::Other),
    ];
    for (path, expected) in cases {
        assert_eq!(
            classify_destination_path(path).expect("classify path"),
            *expected,
            "path={}",
            String::from_utf8_lossy(path)
        );
    }

    for invalid in [
        b"../private.txt".as_slice(),
        b"/absolute/path.rs".as_slice(),
        br"C:\private\path.rs".as_slice(),
        b"src/../../private.rs".as_slice(),
        b"".as_slice(),
    ] {
        assert_eq!(
            classify_destination_path(invalid),
            Err(GitCoreError::InvalidPath)
        );
    }
}

#[test]
fn commit_aggregation_handles_merge_binary_submodule_and_checked_limits() {
    let mut accumulator =
        GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([3; 32]), 20_000, 1)
            .expect("ordinary accumulator");
    accumulator
        .record(GitPathStat::text(b"src/main.rs", 20, 3).expect("product stat"))
        .expect("record product");
    accumulator
        .record(GitPathStat::text(b"tests/main.rs", 8, 4).expect("test stat"))
        .expect("record test");
    accumulator
        .record(GitPathStat::binary(b"assets/logo.png").expect("binary stat"))
        .expect("record binary");
    accumulator
        .record(GitPathStat::submodule(b"vendor/submodule").expect("submodule stat"))
        .expect("record submodule");
    let aggregate = accumulator.finish().expect("finish aggregate");

    assert!(!aggregate.is_merge());
    assert_eq!(aggregate.parent_count(), 1);
    assert_eq!(aggregate.lines(), GitLineMetrics::new(28, 7));
    assert_eq!(
        aggregate.category_lines(GitOutputCategory::ProductCode),
        GitLineMetrics::new(20, 3)
    );
    assert_eq!(
        aggregate.category_lines(GitOutputCategory::Test),
        GitLineMetrics::new(8, 4)
    );
    assert_eq!(aggregate.binary_files(), 1);
    assert_eq!(aggregate.submodule_changes(), 1);
    assert_eq!(aggregate.changed_paths(), 4);

    let mut merge = GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([8; 32]), 20_000, 2)
        .expect("merge accumulator");
    assert_eq!(
        merge.record(GitPathStat::text(b"src/merged.rs", 10, 0).expect("merge path")),
        Err(GitCoreError::IncoherentState)
    );
    let merge = merge.finish().expect("finish merge");
    assert!(merge.is_merge());
    assert_eq!(merge.lines(), GitLineMetrics::new(0, 0));

    let mut bounded =
        GitCommitAccumulator::new(GitCommitFingerprint::from_bytes([4; 32]), 20_000, 1)
            .expect("bounded accumulator");
    for _ in 0..MAX_GIT_PATHS_PER_COMMIT {
        bounded
            .record(GitPathStat::text(b"src/reused.rs", 0, 0).expect("path stat"))
            .expect("within path bound");
    }
    assert_eq!(
        bounded.record(GitPathStat::text(b"src/overflow.rs", 0, 0).expect("overflow stat")),
        Err(GitCoreError::CapacityExceeded {
            limit: MAX_GIT_PATHS_PER_COMMIT
        })
    );
}

#[test]
fn scan_and_batch_state_are_bounded_without_whole_history_retention() {
    let mut scan = GitScanAccumulator::new();
    for day in 0..=tokenmaster_domain::MAX_GIT_OUTPUT_DAYS {
        scan.push(commit(
            u8::try_from(day % 251).expect("bounded seed"),
            i32::try_from(day).expect("bounded day"),
        ))
        .expect("scan commit");
    }
    let summary = scan.finish().expect("scan summary");
    assert_eq!(
        summary.totals().commits(),
        u64::try_from(tokenmaster_domain::MAX_GIT_OUTPUT_DAYS + 1).expect("bounded total")
    );
    assert_eq!(
        summary.retained_days().len(),
        tokenmaster_domain::MAX_GIT_OUTPUT_DAYS
    );
    assert_eq!(
        summary.retained_day_categories().len(),
        tokenmaster_domain::MAX_GIT_OUTPUT_DAYS * tokenmaster_domain::MAX_GIT_OUTPUT_CATEGORIES
    );
    assert_eq!(summary.retained_days()[0].day_index(), 1);
    assert_eq!(summary.retained_day_categories()[0].day_index(), 1);
    assert_eq!(
        summary.retained_day_categories()[0].category(),
        GitOutputCategory::ProductCode
    );
    assert!(summary.daily_history_truncated());

    let records = (0..MAX_GIT_COMMITS_PER_BATCH)
        .map(|index| {
            commit(
                u8::try_from(index).expect("bounded batch seed"),
                i32::try_from(index).expect("bounded batch day"),
            )
        })
        .collect::<Vec<_>>();
    assert!(GitAggregateBatch::new(records.clone()).is_ok());
    let mut oversized = records;
    oversized.push(commit(255, 999));
    assert_eq!(
        GitAggregateBatch::new(oversized),
        Err(GitCoreError::CapacityExceeded {
            limit: MAX_GIT_COMMITS_PER_BATCH
        })
    );
    assert_eq!(
        GitAggregateBatch::new(vec![commit(1, 1), commit(1, 2)]),
        Err(GitCoreError::DuplicateValue)
    );
}
