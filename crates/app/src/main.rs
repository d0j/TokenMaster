#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    match tokenmaster_app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tokenmaster_error={}", error.code().stable_code());
            ExitCode::FAILURE
        }
    }
}
