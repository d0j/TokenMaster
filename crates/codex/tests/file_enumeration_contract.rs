use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;
use tokenmaster_codex::{
    EnumerationCompletion, EnumerationDiagnosticCode, EnumerationErrorCode, SinkDecision,
    enumerate_profile_sources,
};
use tokenmaster_provider::{ProfileId, SourceDescriptor, SourceId, SourceKind};

fn write_fixture(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("fixture file must be written");
}

fn source(
    kind: SourceKind,
    root: impl Into<PathBuf>,
    profile_suffix: &str,
    source_suffix: &str,
) -> SourceDescriptor {
    SourceDescriptor::new(
        SourceId::new(format!("source_{source_suffix}")).expect("fixture source ID must be valid"),
        ProfileId::new(format!("profile_{profile_suffix}"))
            .expect("fixture profile ID must be valid"),
        kind,
        root,
    )
    .expect("fixture source must be valid")
}

fn create_junction(target: &Path, junction: &Path) {
    let status = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "New-Item -ItemType Junction -Path $env:TOKENMASTER_TEST_JUNCTION -Target $env:TOKENMASTER_TEST_TARGET | Out-Null",
        ])
        .env("TOKENMASTER_TEST_JUNCTION", junction)
        .env("TOKENMASTER_TEST_TARGET", target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Windows PowerShell must be available for the junction security gate");
    assert!(status.success(), "junction fixture must be created");
}

fn create_nested_fixture(root: &Path, depth: usize) {
    fs::create_dir_all(root).expect("depth root must be created");
    let mut current = root.to_path_buf();
    for index in 0..depth {
        current.push(format!("d{index}"));
        fs::create_dir(&current).expect("depth fixture directory must be created");
    }
    write_fixture(&current.join("session.jsonl"), b"{}\n");
}

#[test]
fn direct_source_streams_only_regular_jsonl_without_retaining_a_list() {
    let temp = TempDir::new().expect("temporary directory must be created");
    fs::create_dir_all(temp.path().join("nested")).expect("nested fixture must be created");
    write_fixture(&temp.path().join("a.jsonl"), b"{}\n");
    write_fixture(&temp.path().join("ignore.txt"), b"x");
    write_fixture(&temp.path().join("nested").join("b.JSONL"), b"{}\n");
    let sources = [source(SourceKind::Direct, temp.path(), "one", "direct")];
    let mut hints = Vec::new();

    let report = enumerate_profile_sources(
        &sources,
        || false,
        |file| {
            hints.push(file.hashed_session_hint().as_str().to_owned());
            SinkDecision::Continue
        },
    )
    .expect("enumeration must succeed");

    assert_eq!(report.completion(), EnumerationCompletion::Complete);
    assert_eq!(report.diagnostics().emitted_files(), 2);
    assert_eq!(hints.len(), 2);
    assert!(hints.iter().all(|value| !value.is_empty()));
    assert_ne!(hints[0], hints[1]);
}

#[test]
fn empty_source_set_fails_before_traversal() {
    let mut callback_calls = 0_u64;

    let error = enumerate_profile_sources(
        &[],
        || false,
        |_| {
            callback_calls += 1;
            SinkDecision::Continue
        },
    )
    .expect_err("empty source set must fail");

    assert_eq!(error.code(), EnumerationErrorCode::EmptySourceSet);
    assert_eq!(callback_calls, 0);
}

#[test]
fn mixed_profiles_fail_before_traversal() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let active = temp.path().join("active");
    let archived = temp.path().join("archived");
    fs::create_dir_all(&active).expect("active fixture must be created");
    fs::create_dir_all(&archived).expect("archive fixture must be created");
    let sources = [
        source(SourceKind::Active, active, "one", "active"),
        source(SourceKind::Archived, archived, "two", "archived"),
    ];
    let mut callback_calls = 0_u64;

    let error = enumerate_profile_sources(
        &sources,
        || false,
        |_| {
            callback_calls += 1;
            SinkDecision::Continue
        },
    )
    .expect_err("mixed profiles must fail");

    assert_eq!(error.code(), EnumerationErrorCode::MixedProfiles);
    assert_eq!(callback_calls, 0);
}

#[test]
fn duplicate_source_kind_fails_before_traversal() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    fs::create_dir_all(&first).expect("first fixture must be created");
    fs::create_dir_all(&second).expect("second fixture must be created");
    let sources = [
        source(SourceKind::Active, first, "one", "active_one"),
        source(SourceKind::Active, second, "one", "active_two"),
    ];
    let mut callback_calls = 0_u64;

    let error = enumerate_profile_sources(
        &sources,
        || false,
        |_| {
            callback_calls += 1;
            SinkDecision::Continue
        },
    )
    .expect_err("duplicate source kind must fail");

    assert_eq!(error.code(), EnumerationErrorCode::DuplicateSourceKind);
    assert_eq!(callback_calls, 0);
}

#[test]
fn direct_cannot_mix_with_active_or_archived() {
    for conflicting_kind in [SourceKind::Active, SourceKind::Archived] {
        let temp = TempDir::new().expect("temporary directory must be created");
        let direct = temp.path().join("direct");
        let conflicting = temp.path().join("conflicting");
        fs::create_dir_all(&direct).expect("direct fixture must be created");
        fs::create_dir_all(&conflicting).expect("conflicting fixture must be created");
        let sources = [
            source(SourceKind::Direct, direct, "one", "direct"),
            source(conflicting_kind, conflicting, "one", "conflicting"),
        ];
        let mut callback_calls = 0_u64;

        let error = enumerate_profile_sources(
            &sources,
            || false,
            |_| {
                callback_calls += 1;
                SinkDecision::Continue
            },
        )
        .expect_err("direct source conflict must fail");

        assert_eq!(error.code(), EnumerationErrorCode::DirectSourceConflict);
        assert_eq!(callback_calls, 0);
    }
}

#[test]
fn descriptor_is_path_private() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let file_path = temp.path().join("private.jsonl");
    write_fixture(&file_path, b"{}\n");
    let sources = [source(SourceKind::Direct, temp.path(), "one", "direct")];
    let mut callback_calls = 0_u64;

    enumerate_profile_sources(
        &sources,
        || false,
        |file| {
            callback_calls += 1;
            assert_eq!(file.provider_id(), "codex");
            assert_eq!(file.profile_id().as_str(), "profile_one");
            assert_eq!(file.source_id().as_str(), "source_direct");
            assert_eq!(file.source_kind(), SourceKind::Direct);
            assert_eq!(file.absolute_path(), file_path);
            assert_eq!(file.relative_path(), Path::new("private.jsonl"));
            assert_eq!(file.metadata_hint().len(), 3);
            assert_eq!(
                file.filename_session_hint().map(|value| value.as_str()),
                Some("private")
            );
            assert!(file.hashed_session_hint().as_str().starts_with("session_"));
            let debug = format!("{file:?}");
            assert!(!debug.contains(temp.path().to_string_lossy().as_ref()));
            assert!(!debug.contains("private.jsonl"));
            SinkDecision::Continue
        },
    )
    .expect("enumeration must succeed");

    assert_eq!(callback_calls, 1);
}

#[test]
fn hashed_session_hint_is_deterministic_and_source_kind_independent() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let active = temp.path().join("active");
    let archived = temp.path().join("archived");
    fs::create_dir_all(&active).expect("active fixture must be created");
    fs::create_dir_all(&archived).expect("archive fixture must be created");
    let filename = format!("{}.jsonl", "界".repeat(171));
    write_fixture(&active.join(&filename), b"{}\n");
    write_fixture(&archived.join(&filename), b"{}\n");

    let mut active_hint = None;
    enumerate_profile_sources(
        &[source(SourceKind::Active, &active, "one", "active")],
        || false,
        |file| {
            assert!(file.filename_session_hint().is_none());
            active_hint = Some(file.hashed_session_hint().as_str().to_owned());
            SinkDecision::Continue
        },
    )
    .expect("active enumeration must succeed");

    let mut archived_hint = None;
    enumerate_profile_sources(
        &[source(SourceKind::Archived, &archived, "one", "archived")],
        || false,
        |file| {
            assert!(file.filename_session_hint().is_none());
            archived_hint = Some(file.hashed_session_hint().as_str().to_owned());
            SinkDecision::Continue
        },
    )
    .expect("archive enumeration must succeed");

    let active_hint = active_hint.expect("active file must emit");
    let archived_hint = archived_hint.expect("archive file must emit");
    assert_eq!(active_hint, archived_hint);
    assert!(active_hint.starts_with("session_"));
    assert_eq!(active_hint.len(), 40);
}

#[test]
fn active_files_shadow_matching_archive_paths_without_a_dedupe_set() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let active = temp.path().join("active");
    let archived = temp.path().join("archived");
    fs::create_dir_all(&active).expect("active fixture must be created");
    fs::create_dir_all(&archived).expect("archive fixture must be created");
    write_fixture(&active.join("same.jsonl"), b"active\n");
    write_fixture(&archived.join("same.jsonl"), b"archived\n");
    write_fixture(&archived.join("only.jsonl"), b"archive-only\n");
    let sources = [
        source(SourceKind::Archived, &archived, "one", "archived"),
        source(SourceKind::Active, &active, "one", "active"),
    ];
    let mut emitted = Vec::new();

    let report = enumerate_profile_sources(
        &sources,
        || false,
        |file| {
            emitted.push((
                file.source_kind(),
                file.relative_path().to_string_lossy().into_owned(),
            ));
            SinkDecision::Continue
        },
    )
    .expect("active/archive enumeration must succeed");

    assert_eq!(report.completion(), EnumerationCompletion::Complete);
    assert_eq!(
        report
            .diagnostics()
            .count(EnumerationDiagnosticCode::ArchiveShadowed),
        1
    );
    assert_eq!(emitted.len(), 2);
    assert!(emitted.contains(&(SourceKind::Active, "same.jsonl".to_owned())));
    assert!(emitted.contains(&(SourceKind::Archived, "only.jsonl".to_owned())));
}

#[test]
fn callback_cancel_returns_cancelled() {
    let temp = TempDir::new().expect("temporary directory must be created");
    write_fixture(&temp.path().join("session.jsonl"), b"{}\n");
    let sources = [source(SourceKind::Direct, temp.path(), "one", "direct")];
    let mut callback_calls = 0_u64;

    let report = enumerate_profile_sources(
        &sources,
        || false,
        |_| {
            callback_calls += 1;
            SinkDecision::Cancel
        },
    )
    .expect("callback cancellation must return a report");

    assert_eq!(callback_calls, 1);
    assert_eq!(report.completion(), EnumerationCompletion::Cancelled);
    assert_ne!(
        report
            .diagnostics()
            .count(EnumerationDiagnosticCode::Cancelled),
        0
    );
}

#[test]
fn reparse_descendant_makes_scan_partial() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let root = temp.path().join("root");
    let target = temp.path().join("target");
    fs::create_dir_all(&root).expect("root fixture must be created");
    fs::create_dir_all(&target).expect("target fixture must be created");
    write_fixture(&target.join("hidden.jsonl"), b"{}\n");
    create_junction(&target, &root.join("linked"));
    let sources = [source(SourceKind::Direct, &root, "one", "direct")];
    let mut callback_calls = 0_u64;

    let report = enumerate_profile_sources(
        &sources,
        || false,
        |_| {
            callback_calls += 1;
            SinkDecision::Continue
        },
    )
    .expect("reparse descendant must degrade rather than fail");

    assert_eq!(callback_calls, 0);
    assert_eq!(report.completion(), EnumerationCompletion::Partial);
    assert_eq!(
        report
            .diagnostics()
            .count(EnumerationDiagnosticCode::ReparsePoint),
        1
    );
}

#[test]
fn archive_shadow_check_never_crosses_active_reparse_ancestor() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let active = temp.path().join("active");
    let archived = temp.path().join("archived");
    let target = temp.path().join("target");
    fs::create_dir_all(&active).expect("active fixture must be created");
    fs::create_dir_all(archived.join("linked")).expect("archive fixture must be created");
    fs::create_dir_all(&target).expect("target fixture must be created");
    write_fixture(&target.join("same.jsonl"), b"outside\n");
    write_fixture(&archived.join("linked").join("same.jsonl"), b"archive\n");
    create_junction(&target, &active.join("linked"));
    let sources = [
        source(SourceKind::Active, &active, "one", "active"),
        source(SourceKind::Archived, &archived, "one", "archived"),
    ];
    let mut archived_emitted = false;

    let report = enumerate_profile_sources(
        &sources,
        || false,
        |file| {
            archived_emitted = file.source_kind() == SourceKind::Archived;
            SinkDecision::Continue
        },
    )
    .expect("reparse-safe shadow check must preserve archive data");

    assert!(archived_emitted);
    assert_eq!(report.completion(), EnumerationCompletion::Partial);
    assert_ne!(
        report
            .diagnostics()
            .count(EnumerationDiagnosticCode::ReparsePoint),
        0
    );
}

#[test]
fn sink_failure_returns_only_stable_error() {
    let temp = TempDir::new().expect("temporary directory must be created");
    write_fixture(&temp.path().join("session.jsonl"), b"{}\n");
    let sources = [source(SourceKind::Direct, temp.path(), "one", "direct")];

    let error = enumerate_profile_sources(&sources, || false, |_| SinkDecision::Fail)
        .expect_err("sink failure must not return a report");

    assert_eq!(error.code(), EnumerationErrorCode::SinkFailed);
    assert_eq!(error.to_string(), "enumeration sink failed");
}

#[test]
fn repeated_scans_release_directory_handles() {
    let temp = TempDir::new().expect("temporary directory must be created");
    write_fixture(&temp.path().join("session.jsonl"), b"{}\n");
    let sources = [source(SourceKind::Direct, temp.path(), "one", "direct")];

    for _ in 0..100 {
        let report = enumerate_profile_sources(&sources, || false, |_| SinkDecision::Cancel)
            .expect("cancelled scan must release its frames");
        assert_eq!(report.completion(), EnumerationCompletion::Cancelled);
    }

    drop(sources);
    let owned = temp.keep();
    let renamed = owned.with_extension("renamed");
    fs::rename(&owned, &renamed).expect("enumeration must release root handles");
    fs::remove_dir_all(&renamed).expect("renamed fixture must be removable");
}

#[test]
fn thousands_of_files_stream_through_a_counting_sink() {
    const DIRECTORY_COUNT: usize = 32;
    const FILES_PER_DIRECTORY: usize = 64;
    const EXPECTED_FILES: usize = DIRECTORY_COUNT * FILES_PER_DIRECTORY;

    let temp = TempDir::new().expect("temporary directory must be created");
    for directory_index in 0..DIRECTORY_COUNT {
        let directory = temp.path().join(format!("d{directory_index:02}"));
        fs::create_dir(&directory).expect("fixture directory must be created");
        for file_index in 0..FILES_PER_DIRECTORY {
            write_fixture(
                &directory.join(format!("session-{file_index:02}.jsonl")),
                b"{}\n",
            );
        }
    }
    let sources = [source(SourceKind::Direct, temp.path(), "one", "direct")];
    let mut callback_count = 0_usize;

    let report = enumerate_profile_sources(
        &sources,
        || false,
        |_| {
            callback_count += 1;
            SinkDecision::Continue
        },
    )
    .expect("large streaming enumeration must succeed");

    assert_eq!(report.completion(), EnumerationCompletion::Complete);
    assert_eq!(callback_count, EXPECTED_FILES);
    assert_eq!(
        report.diagnostics().emitted_files(),
        u64::try_from(EXPECTED_FILES).expect("fixture count fits u64")
    );
}

#[test]
fn depth_64_is_accepted_and_depth_65_is_partial() {
    let temp = TempDir::new().expect("temporary directory must be created");
    let accepted = temp.path().join("accepted");
    let rejected = temp.path().join("rejected");
    create_nested_fixture(&accepted, 64);
    create_nested_fixture(&rejected, 65);

    let mut accepted_files = 0_u64;
    let accepted_report = enumerate_profile_sources(
        &[source(SourceKind::Direct, &accepted, "one", "accepted")],
        || false,
        |_| {
            accepted_files += 1;
            SinkDecision::Continue
        },
    )
    .expect("depth 64 enumeration must succeed");
    assert_eq!(accepted_files, 1);
    assert_eq!(
        accepted_report.completion(),
        EnumerationCompletion::Complete
    );

    let mut rejected_files = 0_u64;
    let rejected_report = enumerate_profile_sources(
        &[source(SourceKind::Direct, &rejected, "one", "rejected")],
        || false,
        |_| {
            rejected_files += 1;
            SinkDecision::Continue
        },
    )
    .expect("depth 65 must return partial truth");
    assert_eq!(rejected_files, 0);
    assert_eq!(rejected_report.completion(), EnumerationCompletion::Partial);
    assert_eq!(
        rejected_report
            .diagnostics()
            .count(EnumerationDiagnosticCode::DepthRejected),
        1
    );
}
