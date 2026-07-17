# TokenMaster P3-B.3 Application Composition Implementation Plan

> Execute task by task with focused red/green tests. Keep `tokenmaster-desktop` free
> of direct runtime/platform/provider/store authority and commit each coherent task.

**Goal:** Produce the first truthfully live-wired `TokenMaster.exe` with deterministic
installed/portable storage, sole runtime ownership, capacity-one runtime-health join,
event-driven UI refresh, and deterministic cleanup.

**Architecture:** A new `tokenmaster-app` package owns data-root selection and all
runtime lifecycles. Existing engine workers emit optional lossy completion hints. The
application copies fixed product health into one desktop observation slot; the desktop
query worker joins it into the existing reducer and P3-B.2 delivers one newest snapshot.

**Stack:** Rust 1.97, Slint 1.17 software renderer, bundled SQLite, existing TokenMaster
engine/runtime/query/product/platform/Codex packages, PowerShell/Pester authority audits.

---

## Task 1 — Add a lossy worker completion notifier

**Files:**
- Modify: `crates/engine/src/worker.rs`
- Modify: `crates/engine/src/lib.rs`

1. Add failing worker tests for one notification after publication, optional notifier
   behavior, and panic isolation with the completion receipt still readable.
2. Add `WorkerCompletionNotifier` and `RefreshWorker::spawn_notified`; keep `spawn`
   delegating to the no-notifier path.
3. Invoke the notifier outside worker locks after `publish_latest` on executed,
   not-started, and panicked completions. Catch and redact notifier panic.
4. Run:
   `cargo +1.97.0 test -p tokenmaster-engine worker --locked`
5. Commit: `feat(engine): add completion notifier`.

## Task 2 — Propagate the notifier through live runtimes

**Files:**
- Modify: `crates/runtime/src/live.rs`
- Modify: `crates/runtime/src/git/runtime.rs`
- Modify: `crates/runtime/src/quota/runtime.rs`
- Modify: `crates/runtime/src/reminder/runtime.rs`
- Modify: runtime tests near each owner

1. Add focused tests that a notified start observes a forced completion and that the
   live notifier also receives nested Git completion.
2. Add additive `start_notified` constructors while preserving all existing `start`
   call sites and behavior.
3. Pass one cloned notifier to live, nested Git, quota, and reminder workers. Do not
   add a dispatcher, timer, or callback queue.
4. Run:
   `cargo +1.97.0 test -p tokenmaster-runtime --locked`
5. Commit: `feat(runtime): propagate completion hints`.

## Task 3 — Accept copied product runtime health

**Files:**
- Modify: `crates/product/src/reducer.rs`
- Modify: `crates/product/src/lib.rs`
- Modify: product reducer tests

1. Add failing tests for direct typed health publication and typed observation failure,
   including stale-generation rejection and last-health retention on failure.
2. Add public `publish_*_runtime_health` and `fail_*_runtime_observation` methods.
   Existing runtime-snapshot methods delegate to them, preserving API behavior.
3. Run:
   `cargo +1.97.0 test -p tokenmaster-product --locked`
4. Commit: `feat(product): accept copied runtime health`.

## Task 4 — Add the capacity-one desktop runtime observation

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: controller tests

1. Add failing tests for accepted/newer and ignored/equal-or-older observations, one
   retained observation under 10,000 updates, a race with an active attempt, and all
   four health/error values appearing only in a complete published snapshot.
2. Add fixed `DesktopRuntimeObservation`, result values, and one observation mailbox.
3. Apply the newest observation inside the worker-confined reducer before query
   reduction. Leave a racing newer observation for the coalesced follow-up.
4. Keep the UI mailbox and bridge unchanged; add no runtime dependency.
5. Run:
   `cargo +1.97.0 test -p tokenmaster-desktop controller --locked`
6. Commit: `feat(desktop): join runtime observations`.

## Task 5 — Implement and test the data-root policy

**Files:**
- Create: `crates/app/Cargo.toml`
- Create: `crates/app/src/lib.rs`
- Create: `crates/app/src/data_root.rs`
- Modify: `Cargo.toml`

1. Add failing installed/portable, marker rejection, one-child creation, no-fallback,
   redacted Debug/error, and local-directory validation tests using injected values.
2. Implement `ApplicationEnvironment`, `DataMode`, and validated `DataRoot` with the
   exact `tokenmaster.portable`, `data`, `TokenMaster`, and `tokenmaster.sqlite3`
   contract.
3. Run:
   `cargo +1.97.0 test -p tokenmaster-app data_root --locked`
4. Commit: `feat(app): resolve validated data root`.

## Task 6 — Compose the production application

**Files:**
- Create: `crates/app/src/application.rs`
- Create: `crates/app/src/main.rs`
- Modify: `crates/desktop/Cargo.toml`
- Delete: `crates/desktop/src/main.rs`
- Modify: `crates/desktop/src/shell.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `Cargo.lock`

1. Add composition tests/fakes for the early-notification pending bit, checked runtime
   generations, independent quota/reminder startup failure observations, and shutdown
   order without a lock held across joins.
2. Move sole `TokenMaster.exe` ownership to `tokenmaster-app`; expose only renderer
   selection from the desktop shell package.
3. Start mandatory live plus independently degradable quota/reminder owners, controller,
   shell, bridge, initial health/query refresh, and event loop.
4. Implement weak completion notification and deterministic pause/controller-join/
   runtime-shutdown cleanup. Map every boundary failure to a stable path-free code.
5. Run:
   `cargo +1.97.0 test -p tokenmaster-app --locked`
   `cargo +1.97.0 test -p tokenmaster-desktop --locked`
6. Commit: `feat(app): compose live desktop runtime`.

## Task 7 — Freeze authority and binary audits

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Create: `scripts/audit-application-composition.ps1`
- Create: `scripts/tests/audit-application-composition.Tests.ps1`

1. Update the desktop audit for a six-Rust-file library boundary while preserving its
   exact dependency and no-authority contracts.
2. Add adversarial application audit fixtures rejecting duplicate binaries/live owners/
   controllers/bridges, polling/timers, arbitrary roots, renderer drift, probe/old
   project dependencies, and forbidden binary strings.
3. Require release build of `tokenmaster-app` and exactly one `TokenMaster.exe`.
4. Run:
   `Invoke-Pester scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed`
   `Invoke-Pester scripts/tests/audit-application-composition.Tests.ps1 -Output Detailed`
   `pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path`
   `pwsh -NoProfile -File scripts/audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path`
5. Commit: `test(app): audit application composition`.

## Task 8 — Synchronize architecture and project state

**Files:**
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`

Record ADR-052, exact root/marker policy, package authority split, completion-hint and
observation bounds, cleanup order, focused evidence, and honest remaining P3-C-P6
work. Do not write a current commit hash into tracked documents.

Commit: `docs(app): complete composition checklist`.

## Task 9 — Final verification and cleanup

Run in order:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Then verify `git status --short`, inspect the final diff/stat, and audit for task-owned
Cargo, compiler, test, PowerShell, TokenMaster, and temporary server processes. Do not
claim P3 route completeness, M0 acceptance, packaging, signing, soak acceptance, or
release.
