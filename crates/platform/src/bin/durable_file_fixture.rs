use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use tokenmaster_platform::{DurableFileTarget, ValidatedLocalDirectory};

const PAYLOAD_BYTES: usize = 256 * 1024;

fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

fn run() -> Result<(), ()> {
    let mut args = std::env::args_os().skip(1);
    let root = args.next().map(PathBuf::from).ok_or(())?;
    let target_name = args
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or(())?;
    let backup_name = args
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or(())?;
    if args.next().is_some() {
        return Err(());
    }

    let directory = ValidatedLocalDirectory::new(&root).map_err(|_| ())?;
    let target = DurableFileTarget::exact_child(&directory, &target_name).map_err(|_| ())?;
    let backup = DurableFileTarget::exact_child(&directory, &backup_name).map_err(|_| ())?;
    let mut stdout = io::stdout().lock();
    let current = std::fs::read(root.join(&target_name)).map_err(|_| ())?;
    let byte = if current.first().copied() == Some(b'A') {
        b'B'
    } else {
        b'A'
    };
    let payload = vec![byte; PAYLOAD_BYTES];
    let mut staged = target.create_staged(PAYLOAD_BYTES as u64).map_err(|_| ())?;
    for chunk in payload.chunks(64 * 1024) {
        staged.write_chunk(chunk).map_err(|_| ())?;
    }
    staged
        .seal(PAYLOAD_BYTES as u64, Sha256::digest(&payload).into())
        .map_err(|_| ())?;
    stdout.write_all(b"prepared\n").map_err(|_| ())?;
    stdout.flush().map_err(|_| ())?;

    let mut stdin = io::stdin().lock();
    let mut command = String::new();
    stdin.read_line(&mut command).map_err(|_| ())?;
    if command != "publish\n" {
        return Err(());
    }
    stdout.write_all(b"publishing\n").map_err(|_| ())?;
    stdout.flush().map_err(|_| ())?;

    command.clear();
    stdin.read_line(&mut command).map_err(|_| ())?;
    if command != "commit\n" {
        return Err(());
    }
    staged.replace_existing(&target, &backup).map_err(|_| ())?;
    stdout.write_all(b"published\n").map_err(|_| ())?;
    stdout.flush().map_err(|_| ())?;
    loop {
        std::thread::park();
    }
}
