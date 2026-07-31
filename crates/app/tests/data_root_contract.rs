use std::ffi::OsString;
use std::fs;

use tempfile::TempDir;
use tokenmaster_app::{ApplicationEnvironment, DataMode, DataRoot, DataRootErrorCode};

fn environment(
    executable: std::path::PathBuf,
    local_app_data: Option<std::path::PathBuf>,
) -> ApplicationEnvironment {
    ApplicationEnvironment::new(executable, local_app_data, None, None::<OsString>)
}

/// Every spelling a private path could reach a rendering through.
///
/// `Debug` for `Path`, `OsStr` and `str` escapes a backslash as two, so asking whether
/// `C:\Users\...` appears in `format!("{value:?}")` asks about text that would spell it
/// `C:\\Users\\...`. It cannot, whatever the code does. Both assertions in this file were
/// written that way and so could not fail on the only platform this product ships to --
/// the strongest-reading privacy check in the crate, unable to fire. Comparing both forms
/// covers a `Display` leak and a `Debug` leak with one needle.
fn disclosure_forms(private: &std::path::Path) -> [String; 2] {
    let raw = private.display().to_string();
    let escaped = format!("{raw:?}");
    [raw, escaped.trim_matches('"').to_owned()]
}

#[test]
fn resolved_root_creates_one_exact_reliable_state_child() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let executable = executable(temporary.path());
    let root = DataRoot::resolve(&environment(
        executable,
        Some(temporary.path().to_path_buf()),
    ))
    .expect("installed data root");
    let reliable_state = root.directory().join("reliable-state");

    let metadata = std::fs::symlink_metadata(&reliable_state).expect("reliable-state metadata");
    assert!(metadata.is_dir());
    assert!(!metadata.file_type().is_symlink());
    let rendered = format!("{root:?}");
    for form in disclosure_forms(&reliable_state) {
        assert!(!rendered.contains(&form), "{rendered} disclosed {form}");
    }
}

#[test]
fn non_directory_reliable_state_child_fails_without_replacement() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let executable = executable(temporary.path());
    let application_root = temporary.path().join("TokenMaster");
    std::fs::create_dir(&application_root).expect("application root");
    let reliable_state = application_root.join("reliable-state");
    std::fs::write(&reliable_state, b"evidence").expect("blocking evidence");

    let error = DataRoot::resolve(&environment(
        executable,
        Some(temporary.path().to_path_buf()),
    ))
    .expect_err("file cannot become the reliable-state directory");

    assert_eq!(error.code(), DataRootErrorCode::DataDirectoryUnavailable);
    assert_eq!(
        std::fs::read(reliable_state).expect("evidence preserved"),
        b"evidence"
    );
}

fn executable(directory: &std::path::Path) -> std::path::PathBuf {
    let path = directory.join("TokenMaster.exe");
    fs::write(&path, b"test executable").expect("test executable");
    path
}

#[test]
fn zero_length_marker_selects_exact_portable_data_child() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = executable(temporary.path());
    fs::write(temporary.path().join("tokenmaster.portable"), []).expect("portable marker");

    let root = DataRoot::resolve(&environment(executable, None)).expect("portable data root");
    let expected_directory =
        fs::canonicalize(temporary.path().join("data")).expect("canonical portable directory");

    assert_eq!(root.mode(), DataMode::Portable);
    assert_eq!(root.directory(), expected_directory);
    assert_eq!(
        root.archive_path(),
        expected_directory.join("tokenmaster.sqlite3")
    );
    assert!(root.directory().is_dir());
    assert_eq!(
        fs::read_dir(temporary.path())
            .expect("package directory")
            .count(),
        3,
        "resolution may create only the exact data child"
    );
}

#[test]
fn absent_marker_selects_exact_installed_child_without_using_cwd() {
    let package = TempDir::new().expect("package directory");
    let installed = TempDir::new().expect("installed base");
    let executable = executable(package.path());

    let root = DataRoot::resolve(&environment(
        executable,
        Some(installed.path().to_path_buf()),
    ))
    .expect("installed data root");
    let expected_directory = fs::canonicalize(installed.path().join("TokenMaster"))
        .expect("canonical installed directory");

    assert_eq!(root.mode(), DataMode::Installed);
    assert_eq!(root.directory(), expected_directory);
    assert_eq!(
        root.archive_path(),
        expected_directory.join("tokenmaster.sqlite3")
    );
    assert!(root.directory().is_dir());
    assert!(!package.path().join("data").exists());
}

#[test]
fn invalid_marker_fails_closed_without_falling_back_to_installed_storage() {
    let package = TempDir::new().expect("package directory");
    let installed = TempDir::new().expect("installed base");
    let executable = executable(package.path());
    fs::write(
        package.path().join("tokenmaster.portable"),
        b"must be empty",
    )
    .expect("invalid marker");

    let error = DataRoot::resolve(&environment(
        executable,
        Some(installed.path().to_path_buf()),
    ))
    .expect_err("nonempty marker must fail");

    assert_eq!(error.code(), DataRootErrorCode::InvalidPortableMarker);
    assert_eq!(error.to_string(), "invalid_portable_marker");
    assert!(!installed.path().join("TokenMaster").exists());
    assert!(!package.path().join("data").exists());
}

#[test]
fn marker_directory_and_missing_installed_base_are_rejected_stably() {
    let package = TempDir::new().expect("package directory");
    let executable = executable(package.path());
    fs::create_dir(package.path().join("tokenmaster.portable")).expect("marker directory");
    let marker_error = DataRoot::resolve(&environment(executable.clone(), None))
        .expect_err("marker directory must fail");
    assert_eq!(
        marker_error.code(),
        DataRootErrorCode::InvalidPortableMarker
    );

    fs::remove_dir(package.path().join("tokenmaster.portable")).expect("remove marker");
    let missing = package.path().join("missing-installed-base");
    let installed_error = DataRoot::resolve(&environment(executable, Some(missing)))
        .expect_err("missing installed base must fail");
    assert_eq!(
        installed_error.code(),
        DataRootErrorCode::UnsupportedDataLocation
    );
}

#[test]
fn environment_and_data_root_debug_never_disclose_absolute_paths() {
    let package = TempDir::new().expect("package directory");
    let installed = TempDir::new().expect("installed base");
    let executable = executable(package.path());
    let environment = environment(executable, Some(installed.path().to_path_buf()));
    let root = DataRoot::resolve(&environment).expect("installed data root");

    let rendered = format!("{environment:?} {root:?}");
    for private in [package.path(), installed.path(), root.archive_path()] {
        for form in disclosure_forms(private) {
            assert!(!rendered.contains(&form), "{rendered} disclosed {form}");
        }
    }
    // Counted, not merely present: `ApplicationEnvironment` redacts four fields and `DataRoot`
    // three, and one surviving `[redacted]` satisfied the old assertion no matter how many of
    // the other six had started rendering their real value.
    assert_eq!(rendered.matches("[redacted]").count(), 7, "{rendered}");
}
