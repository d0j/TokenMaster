# TokenMaster agent instructions

## Product boundary

TokenMaster is the only product in this repository. It is a Rust 1.97, Slint 1.17,
and bundled SQLite application. The root Cargo workspace is the sole workspace.

WhereMyTokens is the external UI/product reference. ccusage is the external
usage-analysis reference. Both are pinned in `third_party/UPSTREAM.toml`; neither is
a runtime dependency or source to vendor.

## Source of truth

Read in this order before changing behavior:

1. `spec/SPECIFICATION.md`
2. `spec/DATA_CONTRACT.md`
3. `spec/API_CONTRACT.md`
4. `spec/SECURITY.md`
5. `spec/TRACEABILITY.md`
6. `spec/DECISIONS.md`
7. `docs/CURRENT_STATE.md`
8. `docs/HANDOFF.md`
9. `docs/ROADMAP.md`

## Workflow

- Work on a feature branch or isolated worktree.
- Use focused red/green tests for behavior changes, then run the relevant workspace
  tests.
- Keep input, retained memory, archive writes, and UI snapshots bounded.
- Never persist or expose prompts, responses, reasoning, commands, source contents,
  credentials, raw incomplete lines, or absolute user paths.
- Do not add arbitrary SQL, shell, HTTP, or filesystem access to future CLI/MCP
  surfaces.
- After substantial work update traceability, current state, project history, and the
  affected security or operational document. Do not put a current commit hash in
  tracked documents.
- A release is not accepted without the exact receipt gates in `M0_ACCEPTANCE.md` and
  the future product-release checklist.

## Verification

Run the narrowest relevant test first. The baseline quality gate is:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```
