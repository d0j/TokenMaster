# TokenMaster

TokenMaster is a Windows-first, portable, local-first usage monitor for Codex. It is
being built as an original Rust application with a responsive Slint desktop UI and a
bounded SQLite archive.

TokenMaster's original code is licensed under Apache-2.0. WhereMyTokens and ccusage
remain separately attributed external MIT references.

WhereMyTokens is the UX and feature-breadth reference. ccusage is the usage-analysis,
model, pricing, and reporting reference. TokenMaster owns the implementation and uses
neither project as a runtime dependency.

## What it is built on

A Rust/Slint/SQLite baseline with three layouts, hot-switchable skins, Russian and
English localization, a tray lifecycle, virtualized presentation models and resource
gates. Bounded Codex discovery and parsing feed a strict migrated archive at
`USAGE_SCHEMA_VERSION`; unchanged refreshes read zero JSONL payload bytes, append
resumes from a persisted checkpoint, and replacement, rewrite or truncation requests a
non-destructive full rebuild. Every number the interface shows arrives by snapshot
rather than being recomputed beside it.

## What is done and what is not

**This file does not answer that, on purpose.** Requirement status lives in
[`spec/TRACEABILITY.md`](spec/TRACEABILITY.md) and delivery order in
[`docs/RELEASE_EXIT_PLAN.md`](docs/RELEASE_EXIT_PLAN.md), each with the evidence behind
it. A second copy of status in prose has already been paid for repeatedly here: four
narrative state documents were deleted for contradicting each other, `docs/ROADMAP.md`
followed them for declaring the exit plan authoritative and then carrying a rival phase
numbering, and this section was the next one -- it still announced that P4 localization,
later-page Sessions navigation and the secret scan were absent after all three existed.

There is no accepted interactive Windows validation and no signed release.

## Build and verify

The gate is one command and it takes no arguments:

```powershell
pwsh -NoProfile -File scripts\verify-m0.ps1
```

Sixteen stages, in order, stopping at the first failure: clean-root, immutable-actions,
dependency-policy, eight Pester suites, `fmt`, `clippy`, the SQLite million-row check, the
workspace tests, and the release build. It runs `audit-clean-root.ps1` and
`verify-dependency-policy.ps1` itself, so running those separately proves less than the
whole gate and takes the same tree to do it. This repository once reported green for
thirty-six hours on `cargo test` alone while the real gate was red across seventy-three
consecutive runs, which is why the list above used to sit here as five commands and no
longer does.

The gate runs the secret scan's own contract suite but never runs the scan itself, because
a scan needs a built package rather than a tree:

```powershell
pwsh -NoProfile -File scripts\verify-secret-scan.ps1 -PackagePath dist\TokenMaster-0.1.0-windows-x64-unsigned.zip
```

**Do not add a `+toolchain` override to these commands.** `rust-toolchain.toml` already
pins `1.97.0-x86_64-pc-windows-msvc`, and rustup installs it on demand. A bare
`cargo +1.97.0` overrides that pin and resolves the version against the machine's
default host instead, so on a host defaulting to `windows-gnu` it selects the GNU
toolchain, which has no `dlltool.exe` and cannot build `getrandom` at all.

The gate bootstraps the exact reviewed `cargo-deny` 0.20.2 Windows binary, so it needs
network access to the current RustSec database, and it needs Pester 5.7.1 installed. It
records developer evidence only and claims no release acceptance.

## Run it

```powershell
cargo run --release --bin TokenMaster
```

The renderer is Skia, chosen by measurement rather than taste: forcing OpenGL cost
518 MB against 56 MB for the default, and the software renderer is not compiled in at
all. `.cargo/config.toml` sets `SLINT_BACKEND=winit-skia` without forcing it, so the
environment can still override it for a diagnostic comparison.

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

See [architecture](docs/ARCHITECTURE.md), [feature matrix](docs/FEATURE_PARITY.md),
and [the release exit plan](docs/RELEASE_EXIT_PLAN.md).
