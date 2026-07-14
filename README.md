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

M1 has bounded Codex source discovery, streaming JSONL parsing, revalidation,
checkpoints, strict SQLite schema, and atomic current-generation ingest. Provider
output now crosses a bounded neutral draft boundary; only `tokenmaster-accounting`
can create fingerprint/replay identities and canonical events, and Codex preserves
late ancestry separately. The pure bounded replay classifier is implemented; the next
slice is its non-destructive versioned SQLite archive and staging integration.

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
- Measured memory, CPU, handle, thread, USER, GDI, and latency gates.

See [the approved audit and master plan](docs/AUDIT_AND_MASTER_PLAN.md),
[architecture](docs/ARCHITECTURE.md), [feature matrix](docs/FEATURE_PARITY.md),
[roadmap](docs/ROADMAP.md), and [current handoff](docs/HANDOFF.md).
