use std::env;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

fn main() {
    if run().is_err() {
        std::process::exit(97);
    }
}

fn run() -> io::Result<()> {
    let executable = env::current_exe()?;
    let mode = executable
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("invalid");
    let args = env::args().skip(1).collect::<Vec<_>>();
    write_receipt(&executable, &args)?;

    match mode {
        "success" => version_or_fail(&args, "git version 2.54.0.windows.1\n"),
        "unsupported" => version_or_fail(&args, "git version 2.20.0\n"),
        "hang" => {
            std::thread::sleep(Duration::from_secs(10));
            Ok(())
        }
        "stdout_oversized" => {
            io::stdout().write_all(&vec![b'x'; 4096])?;
            Ok(())
        }
        "stderr_oversized" => {
            io::stderr().write_all(&vec![b'x'; 4096])?;
            Ok(())
        }
        "missing_author" => missing_author(&args),
        "author_error" => author_error(&args),
        "history_change" => history_change(&executable, &args),
        "slow_scan" => {
            std::thread::sleep(Duration::from_millis(35));
            stable_scan(&args)
        }
        _ => Ok(()),
    }
}

fn version_or_fail(args: &[String], version: &str) -> io::Result<()> {
    if args == ["--version"] {
        io::stdout().write_all(version.as_bytes())
    } else {
        std::process::exit(2);
    }
}

fn missing_author(args: &[String]) -> io::Result<()> {
    if args == ["--version"] {
        return io::stdout().write_all(b"git version 2.54.0\n");
    }
    if args.iter().any(|arg| arg == "rev-parse") {
        let worktree = env::current_dir()?;
        let common_dir = worktree.join(".git");
        writeln!(
            io::stdout(),
            "{}\nfalse\nsha1\n{}",
            common_dir.to_string_lossy(),
            worktree.to_string_lossy()
        )?;
        return Ok(());
    }
    if args.iter().any(|arg| arg == "config") {
        std::process::exit(1);
    }
    std::process::exit(2);
}

fn author_error(args: &[String]) -> io::Result<()> {
    if args == ["--version"] {
        return io::stdout().write_all(b"git version 2.54.0\n");
    }
    if args.iter().any(|arg| arg == "rev-parse") {
        let worktree = env::current_dir()?;
        let common_dir = worktree.join(".git");
        writeln!(
            io::stdout(),
            "{}\nfalse\nsha1\n{}",
            common_dir.to_string_lossy(),
            worktree.to_string_lossy()
        )?;
        return Ok(());
    }
    std::process::exit(2);
}

fn history_change(executable: &Path, args: &[String]) -> io::Result<()> {
    const OID_A: &str = "1111111111111111111111111111111111111111";
    const OID_B: &str = "2222222222222222222222222222222222222222";
    if args == ["--version"] {
        return io::stdout().write_all(b"git version 2.54.0\n");
    }
    if args.iter().any(|arg| arg == "rev-parse") {
        let worktree = env::current_dir()?;
        let common_dir = worktree.join(".git");
        writeln!(
            io::stdout(),
            "{}\nfalse\nsha1\n{}",
            common_dir.to_string_lossy(),
            worktree.to_string_lossy()
        )?;
        return Ok(());
    }
    if args.iter().any(|arg| arg == "config") {
        return io::stdout().write_all(b"user@example.com\n");
    }
    if args.iter().any(|arg| arg == "for-each-ref") {
        let counter = executable
            .parent()
            .ok_or_else(|| io::Error::other("missing executable parent"))?
            .join("refs-count.txt");
        let count = std::fs::read_to_string(&counter)
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0);
        std::fs::write(counter, count.saturating_add(1).to_string())?;
        let oid = if count == 0 { OID_A } else { OID_B };
        return writeln!(io::stdout(), "refs/heads/main\0{oid}");
    }
    if args.iter().any(|arg| arg == "log") {
        let mut output = Vec::new();
        output.push(0x1e);
        output.extend_from_slice(OID_A.as_bytes());
        output.extend_from_slice(b"\x001728000001\0user@example.com\0user@example.com\0\0");
        return io::stdout().write_all(&output);
    }
    std::process::exit(2);
}

fn stable_scan(args: &[String]) -> io::Result<()> {
    const OID: &str = "1111111111111111111111111111111111111111";
    if args == ["--version"] {
        return io::stdout().write_all(b"git version 2.54.0\n");
    }
    if args.iter().any(|arg| arg == "rev-parse") {
        let worktree = env::current_dir()?;
        let common_dir = worktree.join(".git");
        writeln!(
            io::stdout(),
            "{}\nfalse\nsha1\n{}",
            common_dir.to_string_lossy(),
            worktree.to_string_lossy()
        )?;
        return Ok(());
    }
    if args.iter().any(|arg| arg == "config") {
        return io::stdout().write_all(b"user@example.com\n");
    }
    if args.iter().any(|arg| arg == "for-each-ref") {
        return writeln!(io::stdout(), "refs/heads/main\0{OID}");
    }
    if args.iter().any(|arg| arg == "log") {
        let mut output = Vec::new();
        output.push(0x1e);
        output.extend_from_slice(OID.as_bytes());
        output.extend_from_slice(b"\x001728000001\0user@example.com\0user@example.com\0\0");
        return io::stdout().write_all(&output);
    }
    std::process::exit(2);
}

fn write_receipt(executable: &Path, args: &[String]) -> io::Result<()> {
    let receipt = executable
        .parent()
        .map(|directory| directory.join("receipt.txt"))
        .ok_or_else(|| io::Error::other("missing executable parent"))?;
    let mut file = OpenOptions::new().create(true).append(true).open(receipt)?;
    writeln!(file, "pid={}", std::process::id())?;
    writeln!(file, "argv={}", args.join("|"))?;
    writeln!(
        file,
        "env=optional_locks:{};prompt:{};pager:{};no_color:{}",
        env::var("GIT_OPTIONAL_LOCKS").unwrap_or_default(),
        env::var("GIT_TERMINAL_PROMPT").unwrap_or_default(),
        env::var("GIT_PAGER").unwrap_or_default(),
        env::var("NO_COLOR").unwrap_or_default(),
    )?;
    writeln!(
        file,
        "isolated=dir:{};work_tree:{};index:{};config:{};trace:{};askpass:{}",
        env::var("GIT_DIR").unwrap_or_default(),
        env::var("GIT_WORK_TREE").unwrap_or_default(),
        env::var("GIT_INDEX_FILE").unwrap_or_default(),
        env::var("GIT_CONFIG_PARAMETERS").unwrap_or_default(),
        env::var("GIT_TRACE").unwrap_or_default(),
        env::var("GIT_ASKPASS").unwrap_or_default(),
    )?;
    file.flush()?;
    Ok(())
}
