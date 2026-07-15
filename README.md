# TokenMaster

TokenMaster is a Windows-first, portable, local-first usage monitor for Codex. It is
being built as an original Rust application with a responsive Slint desktop UI and a
bounded SQLite archive.

WhereMyTokens is the UX and feature-breadth reference. ccusage is the usage-analysis,
model, pricing, and reporting reference. TokenMaster owns the implementation and uses
neither project as a runtime dependency.

## Current state

M0, the native architecture proof, has a working Rust/Slint/SQLite baseline with
three layouts, hot-switchable skins, English/Russian localization, tray lifecycle,
virtualized presentation models, and resource gates. It is not a product release or
an accepted interactive Windows validation.

M1 has bounded Codex discovery/parsing, replay-safe accounting, strict SQLite schema
v6, exact full rebuild, and a production incremental tail refresh. Unchanged refreshes
read zero JSONL payload bytes; append resumes from the persisted checkpoint; new and
missing sources follow exact complete-scan authority; replacement, rewrite, and
truncation or a changed profile scope durably request a non-destructive full rebuild.
That rebuild safely recovers an unadmitted provisional source. The live runtime now
assembles startup recovery, the process-owned writer lease, the bounded worker,
scheduler and pathless watcher, incremental/rebuild selection, pause/resume, and
joined shutdown. P1-E immutable query snapshots and sleep/race integration are next.

## Build and verify

```powershell
cargo +1.97.0 test --workspace --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path
```

The last command requires Pester 5.7.1 and a validated Windows GNU linker. It records
developer evidence only; it does not claim release acceptance.

## Run the M0 probe

```powershell
cargo +1.97.0 run -p tokenmaster-m0 --release
```

The software renderer is the default. `TOKENMASTER_RENDERER=femtovg` is retained only
for diagnostic comparison and cannot be the default renderer.

## Quality commitments

- Bounded, streaming source processing; no whole-history rescan on the fast path.
- No persistence or exposure of prompts, responses, reasoning, commands, source
  contents, credentials, or absolute user paths.
- Instant modular skin/layout/locale switching without rebuilding the archive.
- Pathless filesystem hints collapse into one fixed atomic aggregate and one bounded
  scheduler wake; periodic reconciliation remains the source of liveness.
- Startup recovery and every write run under the OS writer lease; shutdown stops
  admission and watcher ownership before joining scheduler and worker threads.
- Measured memory, CPU, handle, thread, USER, GDI, and latency gates.

See [the approved audit and master plan](docs/AUDIT_AND_MASTER_PLAN.md),
[architecture](docs/ARCHITECTURE.md), [feature matrix](docs/FEATURE_PARITY.md),
[roadmap](docs/ROADMAP.md), and [current handoff](docs/HANDOFF.md).
