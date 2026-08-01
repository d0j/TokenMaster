// Shared by the fixture binary and the contract tests through `include!`, because the
// fixture is compiled on its own by `rustc` from a single source file and cannot depend on
// a module. Both sides therefore describe one receipt entry, not two.

/// Writes one receipt entry in exactly one call on `sink`.
///
/// The one call is the contract, not an optimisation. `writeln!` issues a separate write for
/// every piece of its format string -- `"pid="`, the digits, then the newline -- and this
/// fixture exists to be killed mid-run by deadline tests. A process terminated between those
/// writes leaves `pid=` in the file permanently, and the reader then parses an empty string.
/// Building the whole entry first makes a torn line impossible rather than unlikely, and it
/// also keeps entries from concurrent fixture children from interleaving line by line.
#[allow(dead_code)]
pub fn write_receipt_entry<W: std::io::Write>(
    sink: &mut W,
    pid: u32,
    args: &[String],
) -> std::io::Result<()> {
    let entry = format!(
        "pid={pid}\n\
         argv={argv}\n\
         env=optional_locks:{optional_locks};prompt:{prompt};pager:{pager};no_color:{no_color}\n\
         isolated=dir:{dir};work_tree:{work_tree};index:{index};config:{config};trace:{trace};askpass:{askpass}\n",
        argv = args.join("|"),
        optional_locks = std::env::var("GIT_OPTIONAL_LOCKS").unwrap_or_default(),
        prompt = std::env::var("GIT_TERMINAL_PROMPT").unwrap_or_default(),
        pager = std::env::var("GIT_PAGER").unwrap_or_default(),
        no_color = std::env::var("NO_COLOR").unwrap_or_default(),
        dir = std::env::var("GIT_DIR").unwrap_or_default(),
        work_tree = std::env::var("GIT_WORK_TREE").unwrap_or_default(),
        index = std::env::var("GIT_INDEX_FILE").unwrap_or_default(),
        config = std::env::var("GIT_CONFIG_PARAMETERS").unwrap_or_default(),
        trace = std::env::var("GIT_TRACE").unwrap_or_default(),
        askpass = std::env::var("GIT_ASKPASS").unwrap_or_default(),
    );
    // One `write`, not `write_all`. `write_all` loops when the sink reports a short write, and
    // each extra call is another point at which a deadline test's kill tears the entry or lets
    // a concurrent child interleave -- exactly what building the whole string first was meant
    // to make impossible. The review bot caught the gap: the claim above said one call and the
    // implementation could make several. A short write is reported rather than retried, so the
    // fixture fails visibly instead of leaving a half-line for the reader to parse. Measured
    // with a sink that consumes half of what it is offered: `write_all` called `write` nine
    // times for one entry.
    let written = sink.write(entry.as_bytes())?;
    if written == entry.len() {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::WriteZero,
        "receipt entry was written partially",
    ))
}
