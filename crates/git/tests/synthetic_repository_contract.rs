use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use tempfile::TempDir;
use tokenmaster_domain::{GitLineMetrics, GitOutputCategory};
use tokenmaster_git::{
    GitAuthorSource, GitCancellation, GitExecutable, GitIdentitySalt, GitObjectFormat, GitProcess,
    GitRepositoryCandidate, GitRunControl, derive_author_fingerprint,
};

fn git_executable() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(if cfg!(windows) { "git.exe" } else { "git" }))
        .find(|candidate| candidate.is_file())
        .expect("native Git on PATH")
}

fn process() -> GitProcess {
    let executable = GitExecutable::new(git_executable()).expect("native git");
    let control =
        GitRunControl::new(Duration::from_secs(10), GitCancellation::new()).expect("run control");
    GitProcess::new(executable, control)
}

fn run_git(repository: &Path, args: &[&str]) {
    let status = Command::new(git_executable())
        .current_dir(repository)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .expect("run git");
    assert!(status.success(), "git failed: {args:?}");
}

fn git_output(repository: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new(git_executable())
        .current_dir(repository)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("run git for output");
    assert!(output.status.success(), "git failed: {args:?}");
    output.stdout
}

fn write(repository: &Path, relative: &str, bytes: &[u8]) {
    let path = repository.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, bytes).expect("write fixture");
}

fn commit(repository: &Path, message: &str, timestamp: i64) {
    run_git(repository, &["add", "-A"]);
    commit_staged(repository, message, timestamp);
}

fn commit_staged(repository: &Path, message: &str, timestamp: i64) {
    let date = format!("@{timestamp} +0000");
    let status = Command::new(git_executable())
        .current_dir(repository)
        .args(["commit", "-m", message])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .expect("commit fixture");
    assert!(status.success(), "commit failed: {message}");
}

fn setup_repository() -> (TempDir, PathBuf) {
    let temp = TempDir::new().expect("repository root");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("create repository");
    run_git(&repository, &["init", "-b", "main"]);
    run_git(&repository, &["config", "user.name", "User"]);
    run_git(&repository, &["config", "user.email", "user@example.com"]);

    write(&repository, "src/main.rs", b"one\n");
    commit(&repository, "root", 1_728_000_001);

    write(&repository, "src/main.rs", b"one\ntwo\n");
    run_git(&repository, &["add", "-A"]);
    let status = Command::new(git_executable())
        .current_dir(&repository)
        .args([
            "-c",
            "user.name=Other",
            "-c",
            "user.email=other@example.com",
            "commit",
            "-m",
            "other",
        ])
        .env("GIT_AUTHOR_DATE", "@1728000100 +0000")
        .env("GIT_COMMITTER_DATE", "@1728000100 +0000")
        .status()
        .expect("other commit");
    assert!(status.success());

    fs::create_dir_all(repository.join("tests")).expect("create rename destination");
    run_git(&repository, &["mv", "src/main.rs", "tests/renamed.rs"]);
    write(&repository, "tests/new.rs", b"test one\ntest two\n");
    write(&repository, "docs/readme.md", b"documentation\n");
    write(&repository, "assets/blob.bin", b"\0binary\0");
    commit(&repository, "rename and files", 1_728_086_401);

    let submodule = repository.join("vendor/submodule");
    fs::create_dir_all(&submodule).expect("create nested repository");
    run_git(&submodule, &["init", "-b", "main"]);
    run_git(&submodule, &["config", "user.name", "Nested"]);
    run_git(&submodule, &["config", "user.email", "nested@example.com"]);
    write(&submodule, "README.md", b"nested\n");
    commit(&submodule, "nested root", 1_728_172_700);
    run_git(&repository, &["add", "vendor/submodule"]);
    commit_staged(&repository, "gitlink", 1_728_172_801);

    run_git(&repository, &["switch", "-c", "feature"]);
    write(&repository, "src/feature.rs", b"feature one\nfeature two\n");
    commit(&repository, "feature", 1_728_259_201);

    run_git(&repository, &["switch", "main"]);
    write(&repository, "config/app.toml", b"enabled=true\n");
    commit(&repository, "config", 1_728_345_601);
    let status = Command::new(git_executable())
        .current_dir(&repository)
        .args(["merge", "--no-ff", "feature", "-m", "merge feature"])
        .env("GIT_AUTHOR_DATE", "@1728435601 +0000")
        .env("GIT_COMMITTER_DATE", "@1728435601 +0000")
        .status()
        .expect("merge feature");
    assert!(status.success());

    (temp, repository)
}

#[test]
fn synthetic_history_has_exact_author_merge_rename_binary_submodule_and_branch_semantics() {
    let (_temp, repository) = setup_repository();
    run_git(
        &repository,
        &["config", "core.pager", "tokenmaster-missing-pager"],
    );
    run_git(
        &repository,
        &["config", "diff.external", "tokenmaster-missing-diff"],
    );
    write(
        &repository,
        ".git/hooks/post-checkout",
        b"#!/bin/sh\nexit 99\n",
    );
    let config_before = fs::read(repository.join(".git/config")).expect("config before scan");
    let index_before = fs::read(repository.join(".git/index")).expect("index before scan");
    let refs_before = git_output(
        &repository,
        &["for-each-ref", "--format=%(refname)%00%(objectname)"],
    );
    let status_before = git_output(&repository, &["status", "--porcelain=v2", "-z"]);
    let candidate = GitRepositoryCandidate::new(repository.clone()).expect("repository candidate");
    let scan = process()
        .scan(&candidate, GitIdentitySalt::from_bytes([7; 32]))
        .expect("scan repository");
    assert_eq!(
        fs::read(repository.join(".git/config")).expect("config after scan"),
        config_before
    );
    assert_eq!(
        fs::read(repository.join(".git/index")).expect("index after scan"),
        index_before
    );
    assert_eq!(
        git_output(
            &repository,
            &["for-each-ref", "--format=%(refname)%00%(objectname)"],
        ),
        refs_before
    );
    assert_eq!(
        git_output(&repository, &["status", "--porcelain=v2", "-z"]),
        status_before
    );

    assert_eq!(scan.object_format(), GitObjectFormat::Sha1);
    assert_eq!(scan.author_source(), GitAuthorSource::Repository);
    assert!(!scan.is_shallow());
    assert_eq!(scan.ref_count(), 2);
    assert_eq!(scan.summary().totals().commits(), 6);
    assert_eq!(scan.summary().totals().merge_commits(), 1);
    assert_eq!(scan.summary().totals().lines(), GitLineMetrics::new(7, 0));
    assert_eq!(scan.summary().totals().binary_files(), 1);
    assert_eq!(scan.summary().totals().submodule_changes(), 1);
    assert_eq!(
        scan.summary()
            .category_lines(GitOutputCategory::ProductCode),
        GitLineMetrics::new(3, 0)
    );
    assert_eq!(
        scan.summary().category_lines(GitOutputCategory::Test),
        GitLineMetrics::new(2, 0)
    );
    assert_eq!(
        scan.summary().category_lines(GitOutputCategory::DocsSpec),
        GitLineMetrics::new(1, 0)
    );
    assert_eq!(
        scan.summary()
            .category_lines(GitOutputCategory::ConfigBuild),
        GitLineMetrics::new(1, 0)
    );
}

#[test]
fn worktrees_share_repository_identity_and_empty_repository_is_complete_zero() {
    let (temp, repository) = setup_repository();
    let worktree = temp.path().join("worktree");
    run_git(
        &repository,
        &[
            "worktree",
            "add",
            worktree.to_str().expect("worktree path"),
            "feature",
        ],
    );
    let salt = GitIdentitySalt::from_bytes([8; 32]);
    let main = process()
        .scan(
            &GitRepositoryCandidate::new(repository).expect("main candidate"),
            salt,
        )
        .expect("main scan");
    let linked = process()
        .scan(
            &GitRepositoryCandidate::new(worktree).expect("worktree candidate"),
            salt,
        )
        .expect("worktree scan");
    assert_eq!(main.repository_id(), linked.repository_id());
    assert_eq!(main.ref_fingerprint(), linked.ref_fingerprint());
    assert_eq!(main.summary(), linked.summary());

    let empty = temp.path().join("empty");
    fs::create_dir(&empty).expect("empty repository");
    run_git(&empty, &["init", "-b", "main"]);
    run_git(&empty, &["config", "user.name", "User"]);
    run_git(&empty, &["config", "user.email", "user@example.com"]);
    let empty = process()
        .scan(
            &GitRepositoryCandidate::new(empty).expect("empty candidate"),
            salt,
        )
        .expect("empty scan");
    assert_eq!(empty.ref_count(), 0);
    assert_eq!(empty.summary().totals().commits(), 0);
}

#[test]
fn mailmap_canonicalizes_author_without_exposing_email() {
    let temp = TempDir::new().expect("mailmap root");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("create repository");
    run_git(&repository, &["init", "-b", "main"]);
    run_git(&repository, &["config", "user.name", "User"]);
    run_git(&repository, &["config", "user.email", "alias@example.com"]);
    write(&repository, "src/alias.rs", b"alias\n");
    run_git(&repository, &["add", "-A"]);
    let status = Command::new(git_executable())
        .current_dir(&repository)
        .args([
            "-c",
            "user.name=Alias",
            "-c",
            "user.email=alias@example.com",
            "commit",
            "-m",
            "alias",
        ])
        .env("GIT_AUTHOR_DATE", "@1728000001 +0000")
        .env("GIT_COMMITTER_DATE", "@1728000001 +0000")
        .status()
        .expect("alias commit");
    assert!(status.success());
    write(
        &repository,
        ".mailmap",
        b"User <user@example.com> Alias <alias@example.com>\n",
    );
    commit(&repository, "mailmap", 1_728_086_401);

    let salt = GitIdentitySalt::from_bytes([5; 32]);
    let scan = process()
        .scan(
            &GitRepositoryCandidate::new(repository.clone()).expect("mailmap candidate"),
            salt,
        )
        .expect("mailmap scan");
    assert_eq!(scan.summary().totals().commits(), 2);
    assert_eq!(scan.summary().totals().lines().added(), 2);
    assert_eq!(
        scan.author_fingerprint(),
        derive_author_fingerprint(&salt, b"alias@example.com").expect("configured author")
    );
    let ref_fingerprint = scan.ref_fingerprint();
    let mailmap_fingerprint = scan.mailmap_fingerprint();
    let debug = format!("{scan:?}");
    assert!(!debug.contains("user@example.com"));
    assert!(!debug.contains("alias@example.com"));

    write(
        &repository,
        ".mailmap",
        b"Canonical <canonical@example.com> Alias <alias@example.com>\n",
    );
    let changed = process()
        .scan(
            &GitRepositoryCandidate::new(repository).expect("changed mailmap candidate"),
            salt,
        )
        .expect("changed mailmap scan");
    assert_eq!(changed.ref_fingerprint(), ref_fingerprint);
    assert_ne!(changed.mailmap_fingerprint(), mailmap_fingerprint);
}

#[test]
fn octopus_merge_is_counted_once_without_recounting_merged_lines() {
    let temp = TempDir::new().expect("octopus root");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("create repository");
    run_git(&repository, &["init", "-b", "main"]);
    run_git(&repository, &["config", "user.name", "User"]);
    run_git(&repository, &["config", "user.email", "user@example.com"]);
    write(&repository, "src/root.rs", b"root\n");
    commit(&repository, "root", 1_728_000_001);

    for (branch, path, timestamp) in [
        ("one", "src/one.rs", 1_728_086_401),
        ("two", "src/two.rs", 1_728_172_801),
        ("three", "src/three.rs", 1_728_259_201),
    ] {
        run_git(&repository, &["switch", "-c", branch, "main"]);
        write(&repository, path, format!("{branch}\n").as_bytes());
        commit(&repository, branch, timestamp);
    }
    run_git(&repository, &["switch", "main"]);
    let status = Command::new(git_executable())
        .current_dir(&repository)
        .args(["merge", "--no-ff", "one", "two", "three", "-m", "octopus"])
        .env("GIT_AUTHOR_DATE", "@1728345601 +0000")
        .env("GIT_COMMITTER_DATE", "@1728345601 +0000")
        .status()
        .expect("octopus merge");
    assert!(status.success());

    let scan = process()
        .scan(
            &GitRepositoryCandidate::new(repository).expect("octopus candidate"),
            GitIdentitySalt::from_bytes([6; 32]),
        )
        .expect("octopus scan");
    assert_eq!(scan.ref_count(), 4);
    assert_eq!(scan.summary().totals().commits(), 5);
    assert_eq!(scan.summary().totals().merge_commits(), 1);
    assert_eq!(scan.summary().totals().lines(), GitLineMetrics::new(4, 0));
}

#[test]
fn shallow_boundary_is_explicit_and_never_fabricates_hidden_history() {
    let temp = TempDir::new().expect("shallow root");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("create repository");
    run_git(&repository, &["init", "-b", "main"]);
    run_git(&repository, &["config", "user.name", "User"]);
    run_git(&repository, &["config", "user.email", "user@example.com"]);
    for (index, timestamp) in [(1, 1_728_000_001), (2, 1_728_086_401), (3, 1_728_172_801)] {
        write(
            &repository,
            "src/main.rs",
            format!("{}\n", "line\n".repeat(index)).as_bytes(),
        );
        commit(&repository, &format!("commit {index}"), timestamp);
    }
    let head = git_output(&repository, &["rev-parse", "HEAD"]);
    fs::write(repository.join(".git/shallow"), head).expect("mark shallow boundary");

    let scan = process()
        .scan(
            &GitRepositoryCandidate::new(repository).expect("shallow candidate"),
            GitIdentitySalt::from_bytes([3; 32]),
        )
        .expect("shallow scan");
    assert!(scan.is_shallow());
    assert_eq!(scan.summary().totals().commits(), 1);
}

#[test]
fn sha256_repository_uses_the_same_bounded_protocol() {
    let temp = TempDir::new().expect("sha256 root");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("create repository");
    run_git(
        &repository,
        &["init", "--object-format=sha256", "-b", "main"],
    );
    run_git(&repository, &["config", "user.name", "User"]);
    run_git(&repository, &["config", "user.email", "user@example.com"]);
    write(&repository, "src/main.rs", b"sha256\n");
    commit(&repository, "root", 1_728_000_001);

    let scan = process()
        .scan(
            &GitRepositoryCandidate::new(repository).expect("sha256 candidate"),
            GitIdentitySalt::from_bytes([1; 32]),
        )
        .expect("sha256 scan");
    assert_eq!(scan.object_format(), GitObjectFormat::Sha256);
    assert_eq!(scan.summary().totals().commits(), 1);
    assert_eq!(scan.summary().totals().lines(), GitLineMetrics::new(1, 0));
}
