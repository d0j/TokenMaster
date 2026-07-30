#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

#[cfg(all(windows, debug_assertions))]
fn detach_debug_console() {
    use windows::Win32::System::Console::FreeConsole;

    unsafe {
        let _ = FreeConsole();
    }
}

fn main() -> ExitCode {
    #[cfg(all(windows, debug_assertions))]
    detach_debug_console();

    match tokenmaster_app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tokenmaster_error={}", error.code().stable_code());
            ExitCode::FAILURE
        }
    }
}
