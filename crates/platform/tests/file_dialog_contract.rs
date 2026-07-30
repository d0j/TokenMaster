use std::fs;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokenmaster_platform::{
    ControlledFileDialog, FileDialogErrorCode, FileDialogFileType, FileDialogResult,
    FileDialogSelector, SelectedInputFile, SelectedOutputFile, ValidatedLocalDirectory,
};

const OLD: &[u8] = b"old-config";
const NEW: &[u8] = b"new-config";

fn fixture() -> (TempDir, ValidatedLocalDirectory) {
    let root = TempDir::new().expect("temporary root");
    let directory = ValidatedLocalDirectory::new(root.path()).expect("validated root");
    (root, directory)
}

fn selected_input(result: FileDialogResult<SelectedInputFile>) -> SelectedInputFile {
    match result {
        FileDialogResult::Selected(selection) => selection,
        FileDialogResult::Cancelled | FileDialogResult::Failed(_) => {
            panic!("input must be selected")
        }
    }
}

fn selected_output(result: FileDialogResult<SelectedOutputFile>) -> SelectedOutputFile {
    match result {
        FileDialogResult::Selected(selection) => selection,
        FileDialogResult::Cancelled | FileDialogResult::Failed(_) => {
            panic!("output must be selected")
        }
    }
}

fn sealed_stage(
    output: &mut SelectedOutputFile,
    bytes: &[u8],
) -> tokenmaster_platform::DurableStagedFile {
    let mut staged = output.create_staged(1024).expect("stage");
    staged.write_chunk(bytes).expect("write");
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    staged
        .seal(u64::try_from(bytes.len()).expect("length"), digest)
        .expect("seal");
    staged
}

fn read_all(mut input: SelectedInputFile) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 16];
    loop {
        let count = input.read_chunk(&mut buffer).expect("read chunk");
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    bytes
}

#[test]
fn file_types_pin_exact_filters_extensions_and_default_names() {
    assert_eq!(FileDialogFileType::Config.filter_pattern(), "*.tmconfig");
    assert_eq!(FileDialogFileType::Config.extension(), "tmconfig");
    assert_eq!(
        FileDialogFileType::Config.default_file_name(),
        "tokenmaster-config.tmconfig"
    );
    assert_eq!(FileDialogFileType::Backup.filter_pattern(), "*.tmbackup");
    assert_eq!(FileDialogFileType::Backup.extension(), "tmbackup");
    assert_eq!(
        FileDialogFileType::EncryptedBackup.filter_pattern(),
        "*.tmbackup.age"
    );
    assert_eq!(
        FileDialogFileType::EncryptedBackup.extension(),
        "tmbackup.age"
    );
}

#[test]
fn controlled_input_opens_one_existing_bounded_exact_type() {
    let (root, directory) = fixture();
    fs::write(root.path().join("settings.tmconfig"), OLD).expect("input");
    let dialog =
        ControlledFileDialog::selected(&directory, "settings.tmconfig").expect("controlled dialog");

    let input = selected_input(dialog.select_input(FileDialogFileType::Config, 1024));
    assert_eq!(input.len(), u64::try_from(OLD.len()).expect("length"));
    assert_eq!(read_all(input), OLD);
}

#[test]
fn wrong_extension_directory_hard_link_and_oversize_fail_closed() {
    let (root, directory) = fixture();
    fs::write(root.path().join("settings.txt"), OLD).expect("wrong extension");
    let wrong = ControlledFileDialog::selected(&directory, "settings.txt").expect("dialog");
    assert!(matches!(
        wrong.select_input(FileDialogFileType::Config, 1024),
        FileDialogResult::Failed(error)
            if error.code() == FileDialogErrorCode::InvalidSelection
    ));

    fs::create_dir(root.path().join("directory.tmconfig")).expect("directory selection");
    let directory_dialog =
        ControlledFileDialog::selected(&directory, "directory.tmconfig").expect("dialog");
    assert!(matches!(
        directory_dialog.select_input(FileDialogFileType::Config, 1024),
        FileDialogResult::Failed(error)
            if error.code() == FileDialogErrorCode::UnexpectedType
    ));

    fs::write(root.path().join("source.tmconfig"), OLD).expect("hard-link source");
    fs::hard_link(
        root.path().join("source.tmconfig"),
        root.path().join("linked.tmconfig"),
    )
    .expect("hard link");
    let linked = ControlledFileDialog::selected(&directory, "linked.tmconfig").expect("dialog");
    assert!(matches!(
        linked.select_input(FileDialogFileType::Config, 1024),
        FileDialogResult::Failed(error)
            if error.code() == FileDialogErrorCode::UnsupportedLocation
    ));

    fs::write(root.path().join("large.tmconfig"), [0_u8; 9]).expect("large input");
    let large = ControlledFileDialog::selected(&directory, "large.tmconfig").expect("dialog");
    assert!(matches!(
        large.select_input(FileDialogFileType::Config, 8),
        FileDialogResult::Failed(error)
            if error.code() == FileDialogErrorCode::CapacityExceeded
    ));
}

#[cfg(unix)]
#[test]
fn symbolic_link_input_is_rejected() {
    use std::os::unix::fs::symlink;

    let (root, directory) = fixture();
    fs::write(root.path().join("source.tmconfig"), OLD).expect("source");
    symlink(
        root.path().join("source.tmconfig"),
        root.path().join("linked.tmconfig"),
    )
    .expect("symlink");
    let dialog = ControlledFileDialog::selected(&directory, "linked.tmconfig").expect("dialog");
    assert!(matches!(
        dialog.select_input(FileDialogFileType::Config, 1024),
        FileDialogResult::Failed(error)
            if error.code() == FileDialogErrorCode::UnsupportedLocation
    ));
}

#[cfg(windows)]
#[test]
fn symbolic_or_reparse_input_is_rejected() {
    use std::os::windows::fs::symlink_file;

    let (root, directory) = fixture();
    fs::write(root.path().join("source.tmconfig"), OLD).expect("source");
    symlink_file(
        root.path().join("source.tmconfig"),
        root.path().join("linked.tmconfig"),
    )
    .expect("symlink");
    let dialog = ControlledFileDialog::selected(&directory, "linked.tmconfig").expect("dialog");
    assert!(matches!(
        dialog.select_input(FileDialogFileType::Config, 1024),
        FileDialogResult::Failed(error)
            if error.code() == FileDialogErrorCode::UnsupportedLocation
    ));
}

#[test]
fn create_new_output_is_absent_until_complete_candidate_is_published() {
    let (root, directory) = fixture();
    let dialog = ControlledFileDialog::selected(&directory, "export.tmconfig").expect("dialog");
    let mut output = selected_output(dialog.select_output(FileDialogFileType::Config));

    let mut staged = sealed_stage(&mut output, NEW);
    assert!(!root.path().join("export.tmconfig").exists());
    output.publish(&mut staged).expect("publish new");
    assert_eq!(
        fs::read(root.path().join("export.tmconfig")).expect("published"),
        NEW
    );
    assert_eq!(read_all(output.open_reader(1024).expect("reopen")), NEW);
}

#[test]
fn selected_output_grants_exactly_one_stage_for_its_lifetime() {
    let (_root, directory) = fixture();
    let dialog = ControlledFileDialog::selected(&directory, "export.tmconfig").expect("dialog");
    let mut output = selected_output(dialog.select_output(FileDialogFileType::Config));

    let mut staged = output.create_staged(1024).expect("first stage");
    staged.discard().expect("discard first stage");
    assert_eq!(
        output
            .create_staged(1024)
            .expect_err("second stage authority must stay closed")
            .code(),
        FileDialogErrorCode::InvalidState
    );
}

#[test]
fn confirmed_existing_output_is_not_truncated_before_atomic_replace() {
    let (root, directory) = fixture();
    fs::write(root.path().join("export.tmconfig"), OLD).expect("old output");
    let dialog = ControlledFileDialog::selected(&directory, "export.tmconfig").expect("dialog");
    let mut output = selected_output(dialog.select_output(FileDialogFileType::Config));

    let mut staged = sealed_stage(&mut output, NEW);
    assert_eq!(
        fs::read(root.path().join("export.tmconfig")).expect("old retained"),
        OLD
    );
    output.publish(&mut staged).expect("replace");
    assert_eq!(
        fs::read(root.path().join("export.tmconfig")).expect("new output"),
        NEW
    );
}

#[test]
fn output_identity_change_after_selection_never_overwrites_the_new_file() {
    let (root, directory) = fixture();
    let path = root.path().join("export.tmconfig");
    fs::write(&path, OLD).expect("old output");
    let dialog = ControlledFileDialog::selected(&directory, "export.tmconfig").expect("dialog");
    let mut output = selected_output(dialog.select_output(FileDialogFileType::Config));
    let mut staged = sealed_stage(&mut output, NEW);

    fs::remove_file(&path).expect("remove selected identity");
    fs::write(&path, b"concurrent").expect("replacement identity");
    let error = output.publish(&mut staged).expect_err("identity drift");
    assert_eq!(error.code(), FileDialogErrorCode::SelectionChanged);
    assert_eq!(fs::read(path).expect("concurrent retained"), b"concurrent");
}

#[test]
fn unicode_output_name_is_supported_without_path_disclosure() {
    let (root, directory) = fixture();
    let dialog = ControlledFileDialog::selected(&directory, "тема.tmconfig").expect("dialog");
    let mut output = selected_output(dialog.select_output(FileDialogFileType::Config));
    let mut staged = sealed_stage(&mut output, NEW);
    output.publish(&mut staged).expect("publish unicode");
    assert_eq!(
        fs::read(root.path().join("тема.tmconfig")).expect("output"),
        NEW
    );
}

#[test]
fn cancel_and_stable_failures_return_no_capability_and_debug_is_redacted() {
    let cancelled = ControlledFileDialog::cancelled();
    assert!(matches!(
        cancelled.select_input(FileDialogFileType::Config, 1024),
        FileDialogResult::Cancelled
    ));
    assert!(matches!(
        cancelled.select_output(FileDialogFileType::Config),
        FileDialogResult::Cancelled
    ));

    let failed = ControlledFileDialog::failed(FileDialogErrorCode::Unavailable);
    let failure = failed.select_output(FileDialogFileType::Config);
    assert!(matches!(
        &failure,
        FileDialogResult::Failed(error) if error.to_string() == "unavailable"
    ));
    assert_eq!(format!("{failed:?}"), "ControlledFileDialog([redacted])");
    assert_eq!(
        format!("{failure:?}"),
        "FileDialogResult::Failed(unavailable)"
    );
}

#[cfg(windows)]
#[test]
fn windows_native_dialog_source_pins_com_filters_cancel_and_no_process_authority() {
    let source = include_str!("../src/windows.rs");
    for required in [
        "CoInitializeEx(None, COINIT_APARTMENTTHREADED)",
        "CoCreateInstance(&FileOpenDialog",
        "CoCreateInstance(&FileSaveDialog",
        "FOS_FORCEFILESYSTEM",
        "FOS_FILEMUSTEXIST",
        "FOS_PATHMUSTEXIST",
        "FOS_NODEREFERENCELINKS",
        "FOS_OVERWRITEPROMPT",
        "FOS_STRICTFILETYPES",
        "FOS_NOTESTFILECREATE",
        "ERROR_CANCELLED",
        "SIGDN_FILESYSPATH",
        "GetActiveWindow",
        "CoTaskMemFree",
        "CoUninitialize",
        "FILE_FLAG_OPEN_REPARSE_POINT",
        "FileAttributeTagInfo",
        "SetFileInformationByHandle",
    ] {
        assert!(
            source.contains(required),
            "missing native contract: {required}"
        );
    }
    for forbidden in [
        "std::process",
        "Command::new",
        "ShellExecute",
        "powershell",
        "cmd.exe",
        "explorer.exe",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden native authority: {forbidden}"
        );
    }

    assert!(
        source.contains("PhantomData<Rc<()>>")
            || include_str!("../src/file_dialog.rs").contains("PhantomData<Rc<()>>"),
        "native dialog must stay thread-affine"
    );
}
