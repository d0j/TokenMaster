# TokenMaster handoff

## First five minutes

1. Read `AGENTS.md`, then the `spec/` files in its declared order.
2. Confirm `git status --short` is empty and use a feature branch or worktree.
3. Run the clean-root audit and the focused test for the next requirement.
4. Do not infer an interactive or release result from developer-only evidence.

## Current implementation boundary

The next M1 work is P0-A under
`docs/superpowers/plans/2026-07-14-tokenmaster-p0-authority-boundary.md`. Implement
bounded provider-neutral drafts and the exclusive accounting canonicalizer before
Codex lineage/classifier/schema work. The complete approved order and audit decisions
are in `docs/AUDIT_AND_MASTER_PLAN.md`.

Tasks 3+ in `2026-07-14-tokenmaster-p0-replay-correctness.md` are superseded. Do not
execute its Codex-owned fingerprint/signature or destructive migration steps. Do not
add Wasmtime to the GUI or current Codex path.

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
