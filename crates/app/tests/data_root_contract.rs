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
    for private in [
        package.path().to_string_lossy(),
        installed.path().to_string_lossy(),
        root.archive_path().to_string_lossy(),
    ] {
        assert!(!rendered.contains(private.as_ref()));
    }
    assert!(rendered.contains("[redacted]"));
}
