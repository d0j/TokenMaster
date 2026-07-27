# TokenMaster 10/10 Ingestion Plan

**Status:** Task 1 complete; Task 2 is next. Later tasks remain locked.

**Goal:** Make TokenMaster's local-history ingestion faster and quieter than the
pinned WhereMyTokens reference without weakening exact accounting, crash recovery,
bounded memory, privacy, or the provider-neutral architecture.

**Current measured baseline (2026-07-27):**

- clean cold import: 4,010 sources, about 5.36 GiB input, 610,312 observations,
  128,976 selected events, `Complete` in 474 seconds, about 58 MiB private memory;
- pinned WhereMyTokens aggregate-ledger receipt: about 262 seconds on the same history;
- warm single-active-file workload: about 0.82-1.04 CPU cores and 42.6-73.5 MiB/s
  sustained reads because one watcher hint preflights and applies all 4,010 sources;
- correctness: current source generations caught up, replay queue drained, UI totals
  matched direct SQLite queries.

The first causal defect is therefore the unit of incremental work, not JSON decoding:
one changed path currently becomes a whole-manifest refresh.

## Research conclusions

### Adopt

1. **Path-targeted watcher hints with a bounded dirty-source set.**
   `notify` 8.2 already returns event paths and exposes `need_rescan()`. Coalesce
   duplicate paths, preserve rename endpoints, and schedule only those sources.
   Overflow, unknown paths, startup, and explicit recovery take the separate
   reconciliation path.
2. **Descriptor/file-identity revalidation.**
   Keep logical provider identity, but bind the opened source to Windows volume serial
   plus 128-bit file ID where supported. Revalidate length, modification time,
   generation, anchor, and resume proof to distinguish append, truncate, replace,
   rename, and delete without reading unrelated files.
3. **Tail-only warm parsing.**
   Read from the durable complete-line offset, carry the bounded incomplete tail and
   SHA-256 continuation, and commit only the changed source. A one-line append must
   not enumerate or open historical sources.
4. **Bounded cold parse workers, balanced by source byte size.**
   Use a small measured worker range rather than all logical CPUs. Each worker keeps
   one 128-256 KiB sequential buffer, byte-prefilters lines before typed
   deserialization, and returns bounded canonical batches. Preserve deterministic
   source and event ordering at the archive boundary.
5. **Two-phase cold persistence.**
   Phase A appends exact immutable observations/checkpoints with reused prepared
   statements in bounded transactions. Phase B performs replay selection, affected
   session invalidation, and rollups with indexed/set-based SQL after all manifest
   sources are complete. Partial UI publication may read committed phase-A truth, but
   `Complete` remains impossible before phase B and all exact proofs finish.
6. **Explicit WAL control.**
   Retain WAL and the required durability policy. Run bounded passive checkpoints
   outside UI-critical commits, detect readers that prevent progress, and truncate at
   a safe terminal boundary. Do not use durability-weakening PRAGMAs to win a benchmark.
7. **Measure before changing the JSON engine.**
   The existing reader already uses `memchr`, bounded buffered I/O, and typed
   `serde_json::from_slice`. Compare `serde_json` with `sonic-rs` only in an isolated
   real-line corpus benchmark after source targeting and store batching. Adopt a new
   parser only if end-to-end cold time improves by at least 10%, exact differential
   results match, and binary/dependency/security costs remain acceptable.

### Reject for the release-critical path

- **USN journal as the primary watcher:** excellent for privileged NTFS indexing
  recovery, but volume-specific, unavailable on SMB and non-NTFS inputs, and a larger
  authority/compatibility surface. It may become an optional later accelerator, never
  the correctness authority.
- **Memory mapping active JSONL files:** useful for stable large random-access files,
  but adds mutation/view-lifetime hazards and has no proven advantage over sequential
  buffered reads for append-active logs.
- **Unbuffered/overlapped I/O or IOCP for source files:** alignment, cancellation, and
  ordering complexity is not justified until profiling proves storage wait dominates
  after bounded parallel sequential reads.
- **RocksDB, DuckDB, differential dataflow, or DBSP rewrite:** these validate the
  delta-maintenance principle but would replace a mature exact SQLite authority and
  expand release risk. Implement the needed delta algebra with the existing schema and
  set-based SQLite.
- **`synchronous=NORMAL/OFF`, disabled foreign keys, or lossy aggregate-only storage:**
  incompatible with TokenMaster's durability and exact replay contracts.
- **Unlimited parse parallelism:** ccusage can retain the complete event vector and
  use every CPU; TokenMaster must keep memory and archive-writer pressure bounded.

## Locked execution order

Do not start a later task until the prior task satisfies its validator. Do not mix UI
work into Tasks 1-4.

### Task 1 — Warm changed-source path

**Completed 2026-07-27.** The watcher now retains a bounded, deduplicated,
runtime-private path batch. A known append takes the targeted provider path; unknown,
new, deleted, renamed, ambiguous, overflow/rescan, forced, periodic, startup, and
recovery work takes authoritative reconciliation. Forced/pathless provenance cannot
be downgraded when it coalesces with a watcher path.

Receipt: 4,010 sources, 20 one-line appends, one file examined per sample,
`p95=3.824 ms`, `max=6.992 ms`, exact appended bytes, with a 250 ms limit. Focused
append/unknown/truncate, scheduler, watcher, live-runtime, Codex, privacy, and strict
Clippy contracts pass. Cold import is deliberately unchanged.

- Carry normalized event paths and `need_rescan` through watcher/scheduler/runtime.
- Maintain a bounded deduplicated dirty-source set with an explicit overflow flag.
- Refresh only matched sources; new/deleted/renamed paths use a bounded inventory
  delta. Unknown or overflow events request reconciliation, not an immediate global
  content pass.
- Validator: a one-line append with 4,009 unchanged fixtures opens/reads one source;
  rename, replace, truncate, delete, overflow, missed-event, restart, and cancellation
  contracts remain exact.
- Stop condition: warm p95 is not below 250 ms or read amplification exceeds appended
  bytes plus 1 MiB. Re-profile before another implementation round.

### Task 2 — Idle and reconciliation path

- Separate event-driven refresh from periodic/full reconciliation.
- Use metadata first; open source bytes only when identity/length/mtime/proof requires
  it. Reconciliation must be cancellable, lowest urgency, and never run continuously.
- Validator: 10-minute idle receipt with zero source changes has average CPU below
  0.5%, zero sustained source reads, no archive generation churn, and stable private
  memory below 64 MiB.

### Task 3 — Cold parse and durable fact load

- Add instrumentation for discover/read/parse/canonicalize/write/project/checkpoint.
- Benchmark worker counts 1, 2, 4, and 8 using size-balanced source partitions.
- Reuse prepared statements and commit bounded fact batches; keep one SQLite writer.
- Defer only derived work whose absence is represented truthfully in Partial state.
- Validator: identical source checkpoints, observation fingerprints, selected events,
  totals, quality, session lineage, and restart result versus the current implementation.
- Stop condition: choose the smallest worker count within 5% of best throughput and
  within the memory/I/O limits; do not keep tuning after the target is met.

### Task 4 — Set-based final projection and WAL lifecycle

- Apply replay relation settlement, selection invalidation, session rollups, and time
  rollups by affected keys or complete-generation set operations.
- Remove repeated per-observation derived-table work only when an equivalent
  differential test is red first.
- Validator: crash at every fact/projection/promotion boundary resumes without
  duplicates or false `Complete`; steady WAL is below 256 MiB after terminal safe
  checkpoint; main database remains exact after reopen.

### Task 5 — Final real-history acceptance

Run once after focused Tasks 1-4 are green:

- three clean cold runs and report median plus worst;
- 100 warm single-line appends and report p50/p95/p99;
- append, incomplete tail, truncate, same-path replacement, rename, delete, missed
  watcher event, watcher overflow, restart, forced cancellation, and power-loss-style
  process termination;
- direct SQLite differential totals and fingerprints against the accepted baseline;
- peak private memory, CPU time, read/write bytes, WAL peak/final size, time to first
  useful Partial data, and time to Complete.

Acceptance targets:

- clean cold median `<= 240 s`, worst `<= 262 s`;
- first truthful useful data `<= 2 s`;
- warm append p95 `<= 250 ms`, p99 `<= 500 ms`;
- warm read amplification `<= appended bytes + 1 MiB`;
- unchanged idle CPU `< 0.5%`, no sustained disk traffic;
- peak private memory `<= 96 MiB` cold and `<= 64 MiB` steady;
- exact results and recovery contracts unchanged.

If the cold target is missed after Tasks 1-4, use the stage timings to select exactly
one next bottleneck. Do not reopen parser, PRAGMA, index, audit, or UI work
speculatively. If 60 minutes do not close that measured blocker, stop and reconsider
the architecture as required by `AGENTS.md`.

## Primary research sources

- Microsoft
  [`ReadDirectoryChangesW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw),
  [change journals](https://learn.microsoft.com/en-us/windows/win32/fileio/change-journals),
  [`FILE_ID_INFO`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info),
  and
  [`FILE_FLAG_SEQUENTIAL_SCAN`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
  documentation.
- [`notify` 8.2 Event](https://docs.rs/notify/8.2.0/notify/struct.Event.html)
  path and rescan contracts.
- SQLite [WAL](https://www.sqlite.org/wal.html),
  [transactions](https://www.sqlite.org/lang_transaction.html),
  [prepared statements](https://www.sqlite.org/c3ref/stmt.html),
  [`PRAGMA synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous),
  [statement status](https://www.sqlite.org/c3ref/c_stmtstatus_counter.html), and
  [`WITHOUT ROWID`](https://www.sqlite.org/withoutrowid.html) documentation.
- pinned WhereMyTokens commit `2bcedf1d`:
  [byte-offset/mtime/size incremental importer](https://github.com/jeongwookie/WhereMyTokens/blob/2bcedf1df4bf7c1acb096cfafc4a453152986f98/src/main/usageLedgerImporter.ts)
  and
  [compact cached summaries](https://github.com/jeongwookie/WhereMyTokens/blob/2bcedf1df4bf7c1acb096cfafc4a453152986f98/src/main/jsonlParser.ts).
- pinned ccusage commit `997ad7f9`:
  [size-balanced parallel file loading](https://github.com/ccusage/ccusage/blob/997ad7f90189867d9f218aa0e7401586e3b9fde8/rust/crates/ccusage/src/adapter/codex/loader.rs),
  [128 KiB buffered Codex reads](https://github.com/ccusage/ccusage/blob/997ad7f90189867d9f218aa0e7401586e3b9fde8/rust/crates/ccusage/src/adapter/codex/parser.rs),
  and
  [byte-prefiltered typed JSONL](https://github.com/ccusage/ccusage/blob/997ad7f90189867d9f218aa0e7401586e3b9fde8/rust/crates/ccusage/src/adapter/jsonl.rs).
- [simdjson On-Demand](https://github.com/simdjson/simdjson) and
  [sonic-rs](https://docs.rs/sonic-rs/latest/sonic_rs/) performance designs.
- [DBSP incremental view-maintenance paper](https://www.vldb.org/pvldb/vol16/p1601-budiu.pdf),
  used as design validation rather than a dependency recommendation.
