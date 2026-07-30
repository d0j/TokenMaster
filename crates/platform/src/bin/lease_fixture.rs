use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use tokenmaster_platform::ExclusiveFileLease;

fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

fn run() -> Result<(), ()> {
    let archive = std::env::args_os().nth(1).map(PathBuf::from).ok_or(())?;
    let lease = ExclusiveFileLease::for_archive(&archive).map_err(|_| ())?;
    let _guard = lease.try_acquire().map_err(|_| ())?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"acquired\n").map_err(|_| ())?;
    stdout.flush().map_err(|_| ())?;
    let mut command = String::new();
    io::stdin().lock().read_line(&mut command).map_err(|_| ())?;
    if command == "exit\n" || command == "exit\r\n" {
        Ok(())
    } else {
        Err(())
    }
}
