use std::env;
use std::fs;
use std::io::{self, Write};
use std::time::Duration;

const OID: &str = "1111111111111111111111111111111111111111";

fn main() {
    if run().is_err() {
        std::process::exit(97);
    }
}

fn run() -> io::Result<()> {
    let executable = env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("missing executable parent"))?;
    let mode = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("invalid");
    let args = env::args().skip(1).collect::<Vec<_>>();
    if mode == "hang" && args == ["--version"] {
        fs::write(directory.join("child-started.txt"), std::process::id().to_string())?;
        std::thread::sleep(Duration::from_secs(30));
        return Ok(());
    }
    if args == ["--version"] {
        return io::stdout().write_all(b"git version 2.54.0\n");
    }
    if args.iter().any(|arg| arg == "rev-parse") {
        let worktree = env::current_dir()?;
        let common_dir = worktree.join(".git");
        return writeln!(
            io::stdout(),
            "{}\nfalse\nsha1\n{}",
            common_dir.to_string_lossy(),
            worktree.to_string_lossy()
        );
    }
    if args.iter().any(|arg| arg == "config") {
        if mode == "missing_author" {
            std::process::exit(1);
        }
        return io::stdout().write_all(b"runtime@example.com\n");
    }
    if args.iter().any(|arg| arg == "for-each-ref") {
        return writeln!(io::stdout(), "refs/heads/main\0{OID}");
    }
    if args.iter().any(|arg| arg == "log") {
        fs::write(directory.join("log-started.txt"), b"1")?;
        if mode == "slow_scan" {
            std::thread::sleep(Duration::from_millis(500));
        }
        let mut output = Vec::new();
        output.push(0x1e);
        output.extend_from_slice(OID.as_bytes());
        output.extend_from_slice(b"\x001728000001\0runtime@example.com\0runtime@example.com\0\0");
        return io::stdout().write_all(&output);
    }
    std::process::exit(2);
}
