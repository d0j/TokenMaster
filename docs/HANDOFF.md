# TokenMaster handoff

## First five minutes

1. Read `AGENTS.md`, then the `spec/` files in its declared order.
2. Confirm `git status --short` is empty and use a feature branch or worktree.
3. Run the clean-root audit and the focused test for the next requirement.
4. Do not infer an interactive or release result from developer-only evidence.

## Current implementation boundary

The next M1 work is replay-safe canonical accounting under
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-replay-correctness.md`. Existing
timestamp-based fingerprints handle exact duplicates but do not yet prove copied
fork/subagent prefixes when timestamps change. Complete that fail-safe lineage slice
before staging generations, scan epochs, analytics, automation, or product UI.

Before continuing the Codex parser tasks, revise the P0 plan against
`docs/superpowers/specs/2026-07-14-tokenmaster-provider-plugin-system-design.md`:
providers emit bounded observation drafts and a provider-neutral TokenMaster
canonicalizer computes fingerprint/replay authority. External plugin runtime work
remains later; do not add Wasmtime to the GUI or current Codex path.

## Commands

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
cargo +1.97.0 test --workspace --locked
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
```

For M0 developer evidence, Pester 5.7.1 and a validated GNU linker are required:

```powershell
pwsh -NoProfile -File scripts\verify-m0.ps1 -RepositoryRoot (Get-Location).Path
```

The clean-root audit, all Pester contracts, full workspace tests, strict Clippy, and
this M0 developer gate were last run successfully after the single-root transition.
It does not accept M0 or package a product release. See `M0_ACCEPTANCE.md`.
