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

## Root delivery governance

The root agent is the senior delivery owner, not an implementation worker managed by
its reviewers. At the start and end of every autonomous cycle, root must state and
reconcile:

1. the current user-visible product milestone;
2. the shortest release-critical next outcome;
3. the remaining real product/release blockers;
4. whether the cycle changed product behavior, correctness, required evidence, or only
   audit machinery.

Root owns architecture, critical path, delegation, integration, stop conditions, and
final acceptance. Children return bounded evidence; they never extend scope or create
an indefinite review queue. Keep the critical-path decision local to root.

Audit and proof work must remain subordinate to product delivery:

- Classify each finding as a production correctness/security/data-loss defect, a
  required acceptance-evidence defect, or audit-only hardening. Do not present an
  audit-only parser/regex weakness as a product defect.
- Every audit rule must map to an existing source-of-truth requirement and a practical
  compiling or executable mutation. Do not expand the threat model merely because a
  new textual decoy can be invented.
- Prefer types, compiler checks, runtime invariants, and focused behavioral tests over
  increasingly complex source-text parsing. A textual audit is a last-mile guard, not
  a substitute parser or a product milestone.
- Normally allow one implementation review and one final re-review. A further review
  round requires a newly demonstrated Critical production/security/data-loss risk,
  not another audit-only bypass.
- Run focused red/green validation while correcting findings, then one relevant final
  aggregate and the required baseline. Do not rerun a long full gate after every
  audit-only edit.

Root must declare `AUDIT_HARDENING_LOOP` when either condition occurs:

- two consecutive correction/review rounds change only audits/tests/docs without
  changing product behavior or closing a required release receipt; or
- more than 60 minutes are spent on audit-only hardening after the production behavior
  and focused contracts already pass.

When triggered, root must immediately stop audit children, report the loop and its
cost, reject further speculative hardening, preserve the last verified product state,
and return to the shortest release-critical product slice. One additional bounded fix
is allowed only for a demonstrated Critical production/security/data-loss defect; it
must have a focused reproducer and a fixed stop condition. Record the trigger and the
chosen disposition in `docs/HANDOFF.md` before continuing.

At every handoff, separately report `product state`, `audit/evidence state`, `release
blockers`, and `Git state`. A green developer baseline must never be described as a
package, M0, release-candidate, or stable-release acceptance.

## Verification

Run the narrowest relevant test first. The baseline quality gate is:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```
