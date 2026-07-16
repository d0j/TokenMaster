#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::env;
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process;
use std::thread;
use std::time::Duration;

use serde_json::Value;

fn main() {
    if run().is_err() {
        process::exit(91);
    }
}

fn run() -> Result<(), ()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args != ["app-server", "--stdio"] {
        return Err(());
    }
    let executable = env::current_exe().map_err(|_| ())?;
    let file_stem = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or(())?;
    let mode = file_stem
        .rsplit_once("__")
        .map(|(_, mode)| mode)
        .ok_or(())?;
    let receipt = executable.with_extension("receipt");
    append_receipt(&receipt, &format!("pid={}", process::id()))?;
    append_receipt(&receipt, "argv=app-server|--stdio")?;

    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();

    let initialize = read_message(&mut input)?;
    validate_initialize_request(&initialize)?;
    record_message(&receipt, &initialize)?;
    if mode == "hang" {
        thread::sleep(Duration::from_secs(30));
        return Ok(());
    }
    if mode == "early_exit" {
        return Ok(());
    }
    if mode == "empty_initialize" {
        writeln!(
            output,
            "{{\"id\":0,\"result\":{{\"codexHome\":\"\",\
             \"platformFamily\":\"windows\",\"platformOs\":\"windows\",\
             \"userAgent\":\"Codex Fixture/0.144.1 (windows)\"}}}}"
        )
        .map_err(|_| ())?;
        output.flush().map_err(|_| ())?;
        return Ok(());
    }
    if mode == "unknown_initialize" {
        writeln!(
            output,
            "{{\"id\":0,\"result\":{{\"codexHome\":\"C:\\\\private\\\\codex-home\",\
             \"platformFamily\":\"windows\",\"platformOs\":\"windows\",\
             \"userAgent\":\"Codex Fixture/0.144.1 (windows)\",\
             \"privateUnexpected\":\"secret\"}}}}"
        )
        .map_err(|_| ())?;
        output.flush().map_err(|_| ())?;
        return Ok(());
    }
    if mode == "jsonrpc_initialize" {
        writeln!(
            output,
            "{{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{{\
             \"codexHome\":\"C:\\\\private\\\\codex-home\",\
             \"platformFamily\":\"windows\",\"platformOs\":\"windows\",\
             \"userAgent\":\"Codex Fixture/0.144.1 (windows)\"}}}}"
        )
        .map_err(|_| ())?;
        output.flush().map_err(|_| ())?;
        return Ok(());
    }
    let user_agent = if mode == "unsupported_version" {
        "Codex Fixture/0.145.0 (windows)"
    } else {
        "Codex Fixture/0.144.1 (windows)"
    };
    writeln!(
        output,
        "{{\"id\":0,\"result\":{{\"codexHome\":\"C:\\\\private\\\\codex-home\",\
         \"platformFamily\":\"windows\",\"platformOs\":\"windows\",\
         \"userAgent\":\"{user_agent}\"}}}}"
    )
    .map_err(|_| ())?;
    output.flush().map_err(|_| ())?;

    let initialized = read_message(&mut input)?;
    validate_initialized_notification(&initialized)?;
    record_message(&receipt, &initialized)?;
    let account = read_message(&mut input)?;
    validate_account_request(&account)?;
    record_message(&receipt, &account)?;

    match mode {
        "malformed" => {
            writeln!(output, "{{not-json").map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "unknown_field" => {
            writeln!(
                output,
                "{{\"id\":1,\"result\":{{\"requiresOpenaiAuth\":true,\
                 \"account\":{{\"type\":\"chatgpt\",\"email\":\"private@example.com\",\
                 \"planType\":\"pro\"}},\"privateUnexpected\":\"secret\"}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "oversized" => {
            output.write_all(&vec![b'x'; 300 * 1024]).map_err(|_| ())?;
            output.write_all(b"\n").map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "wrong_id" => {
            write_account_response(&mut output, 99)?;
            return Ok(());
        }
        "duplicate_id" => {
            writeln!(
                output,
                "{{\"id\":0,\"result\":{{\"codexHome\":\"C:\\\\private\\\\duplicate\",\
                 \"platformFamily\":\"windows\",\"platformOs\":\"windows\",\
                 \"userAgent\":\"Codex Fixture/0.144.1 (windows)\"}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "out_of_order" => {
            write_quota_response(&mut output, 2)?;
            return Ok(());
        }
        "rpc_error" => {
            writeln!(
                output,
                "{{\"id\":1,\"error\":{{\"code\":-32000,\
                 \"message\":\"private backend failure\"}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "blank" => {
            writeln!(output).map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "notification" => {
            writeln!(
                output,
                "{{\"method\":\"remoteControl/status/changed\",\"params\":{{}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "both_result_error" => {
            writeln!(
                output,
                "{{\"id\":1,\"result\":{{\"requiresOpenaiAuth\":true,\
                 \"account\":null}},\"error\":{{\"code\":-32000,\
                 \"message\":\"private backend failure\"}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "missing_result" => {
            writeln!(output, "{{\"id\":1}}").map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "negative_id" => {
            writeln!(
                output,
                "{{\"id\":-1,\"result\":{{\"requiresOpenaiAuth\":true,\
                 \"account\":null}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "rpc_error_unknown" => {
            writeln!(
                output,
                "{{\"id\":1,\"error\":{{\"code\":-32000,\
                 \"message\":\"private backend failure\",\
                 \"privateUnexpected\":\"secret\"}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "oversized_rpc_error" => {
            let message = "x".repeat(2 * 1024);
            writeln!(
                output,
                "{{\"id\":1,\"error\":{{\"code\":-32000,\
                 \"message\":\"{message}\"}}}}"
            )
            .map_err(|_| ())?;
            output.flush().map_err(|_| ())?;
            return Ok(());
        }
        "stderr" => {
            let mut stderr = io::stderr().lock();
            writeln!(stderr, "private stderr fixture noise").map_err(|_| ())?;
        }
        "success" | "unsupported_version" => {}
        _ => return Err(()),
    }

    write_account_response(&mut output, 1)?;
    let quota = read_message(&mut input)?;
    validate_quota_request(&quota)?;
    record_message(&receipt, &quota)?;
    write_quota_response(&mut output, 2)
}

fn write_account_response(output: &mut impl Write, id: u64) -> Result<(), ()> {
    writeln!(
        output,
        "{{\"id\":{id},\"result\":{{\"requiresOpenaiAuth\":true,\
         \"account\":{{\"type\":\"chatgpt\",\"email\":\"private@example.com\",\
         \"planType\":\"pro\"}}}}}}"
    )
    .map_err(|_| ())?;
    output.flush().map_err(|_| ())
}

fn write_quota_response(output: &mut impl Write, id: u64) -> Result<(), ()> {
    writeln!(
        output,
        "{{\"id\":{id},\"result\":{{\"rateLimitResetCredits\":null,\
         \"rateLimits\":{{\"limitId\":\"codex\",\"limitName\":null,\
         \"planType\":\"pro\",\"primary\":{{\"usedPercent\":42,\
         \"resetsAt\":1700100000,\"windowDurationMins\":10080}},\
         \"secondary\":null}},\"rateLimitsByLimitId\":null}}}}"
    )
    .map_err(|_| ())?;
    output.flush().map_err(|_| ())
}

fn read_message(input: &mut impl BufRead) -> Result<Value, ()> {
    let mut line = String::new();
    if input.read_line(&mut line).map_err(|_| ())? == 0 {
        return Err(());
    }
    serde_json::from_str(&line).map_err(|_| ())
}

fn record_message(receipt: &Path, value: &Value) -> Result<(), ()> {
    let method = value.get("method").and_then(Value::as_str).ok_or(())?;
    let kind = if value.get("id").is_some() {
        "request"
    } else {
        "notification"
    };
    append_receipt(receipt, &format!("{kind}={method}"))
}

fn validate_initialize_request(value: &Value) -> Result<(), ()> {
    let expected = serde_json::json!({
        "method": "initialize",
        "id": 0,
        "params": {
            "clientInfo": {
                "name": "tokenmaster",
                "title": "TokenMaster",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "optOutNotificationMethods": [
                    "account/rateLimits/updated",
                    "remoteControl/status/changed"
                ]
            }
        }
    });
    if value == &expected { Ok(()) } else { Err(()) }
}

fn validate_initialized_notification(value: &Value) -> Result<(), ()> {
    let expected = serde_json::json!({"method": "initialized", "params": {}});
    if value == &expected { Ok(()) } else { Err(()) }
}

fn validate_account_request(value: &Value) -> Result<(), ()> {
    let expected = serde_json::json!({
        "method": "account/read",
        "id": 1,
        "params": {"refreshToken": false}
    });
    if value == &expected { Ok(()) } else { Err(()) }
}

fn validate_quota_request(value: &Value) -> Result<(), ()> {
    let expected = serde_json::json!({
        "method": "account/rateLimits/read",
        "id": 2,
        "params": null
    });
    if value == &expected { Ok(()) } else { Err(()) }
}

fn append_receipt(path: &Path, line: &str) -> Result<(), ()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| ())?;
    writeln!(file, "{line}").map_err(|_| ())
}
