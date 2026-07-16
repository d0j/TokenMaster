# TokenMaster Codex Quota Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use
> `superpowers:executing-plans`, `superpowers:test-driven-development`, and
> `superpowers:verification-before-completion`. Mark a checkbox only after its
> validator passes.

**Status:** completed and verified on 2026-07-16

**Goal:** discover the installed native Codex executable and publish official
app-server quota observations through a separate bounded runtime that never holds the
archive writer lease or SQLite state during provider I/O.

**Design:**
`docs/superpowers/specs/2026-07-16-tokenmaster-codex-quota-runtime-design.md`

**Tech stack:** Rust 1.97, edition 2024, existing `tokenmaster-engine`
worker/scheduler, existing `tokenmaster-codex` app-server transport, existing
process-owned writer lease, bundled SQLite. No async runtime, HTTP client, browser
dependency, shell execution, or new crate.

## Global constraints

- Work only on `cx/tokenmaster-product-architecture`; do not push, package, or claim a
  release.
- Keep `LiveRuntime` usage execution behavior unchanged.
- Provider I/O must complete before writer-lease acquisition and SQLite open.
- One automatic discovery pass tests exact native filenames only; never execute
  `.cmd`, `.ps1`, JavaScript wrappers, aliases, or package managers.
- Explicit executable configuration is authoritative and never silently falls back.
- Keep path/account/window/value/raw-response data out of `Debug`, errors, health,
  docs, fixtures, and commits.
- Use focused red/green tests and commit independently reviewable checkpoints.
- Run the repository baseline only after focused contours pass.

**Execution note:** Tasks 2 and 3 were committed together because their internal
execution types intentionally remain private and strict dead-code lint cannot pass
until runtime composition consumes them. No lint suppression or temporary public
surface was added.

---

### Task 1: Freeze executable selection and discovery

**Files:**

- Create: `crates/runtime/src/quota/mod.rs`
- Create: `crates/runtime/src/quota/config.rs`
- Create: `crates/runtime/src/quota/discovery.rs`
- Modify: `crates/runtime/src/lib.rs`
- Create: `crates/runtime/tests/quota_discovery_contract.rs`

**RED:**

- [x] Explicit absolute native executable is accepted and remains path-private.
- [x] Invalid explicit path fails configuration and never falls back to a valid
  `PATH` candidate.
- [x] Automatic search follows directory order and accepts only exact `codex.exe` on
  Windows or `codex` elsewhere.
- [x] `.cmd`, `.ps1`, extensionless Windows shims, symlinks/reparse points, relative
  entries, absent entries, oversized `PATH`, and excessive entry counts are rejected
  or skipped exactly as designed.
- [x] Config/discovery `Debug` and errors contain no executable or archive path.

**GREEN:**

- [x] Add redacted `CodexQuotaRuntimeConfig` with auto and explicit selection.
- [x] Validate explicit selection during runtime construction.
- [x] Add bounded environment `PATH` discovery used afresh for every automatic poll.
- [x] Return only stable discovery error codes.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test quota_discovery_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-runtime --all-targets --locked
```

**Checkpoint commit:** `feat(runtime): add bounded Codex executable discovery`

---

### Task 2: Prove I/O-before-lease execution and bounded publication

**Files:**

- Create: `crates/runtime/src/quota/execution.rs`
- Create: `crates/runtime/src/quota/health.rs`
- Modify: `crates/runtime/src/quota/mod.rs`
- Modify: `crates/runtime/src/lib.rs`
- Add unit contract tests beside `crates/runtime/src/quota/execution.rs`

**RED:**

- [x] Fake source events prove poll completion precedes writer acquisition/store open.
- [x] Cancellation or deadline after source completion causes zero publication.
- [x] Writer contention maps to `Busy`, performs zero writes, and retains no snapshot.
- [x] Successful at-most-32 publication counts started/advanced/duplicate/stale,
  allowance-change, and reset statuses exactly.
- [x] A store failure after N observations reports processed/changed counts for the
  committed prefix and fails the refresh without exposing store details.
- [x] Repeating one normalized snapshot is idempotent and bounded.
- [x] Automatic discovery/transport/store failures map to the correct redacted stage
  and stable code.

**GREEN:**

- [x] Add a private quota source interface and production discovery/transport source.
- [x] Add a private publisher owning the archive path and `RuntimeWriterLease`, but no
  idle SQLite connection.
- [x] Add one execution object that checks control before poll, after poll, before
  each publication, and after the bounded loop.
- [x] Hold the shared writer guard across the complete publication loop and drop
  store/guard before health publication.
- [x] Add a copyable latest-only refresh snapshot with bounded counts, elapsed time,
  observation time, last-success time, stage, and stable error code.
- [x] Classify only transient source/publication failures for accelerated retry.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-runtime quota::execution --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-runtime --all-targets --locked
```

**Checkpoint commit:** `feat(runtime): add bounded Codex quota execution`

---

### Task 3: Add the independent quota scheduler/worker lifecycle

**Files:**

- Create: `crates/runtime/src/quota/runtime.rs`
- Modify: `crates/runtime/src/quota/mod.rs`
- Modify: `crates/runtime/src/hints.rs`
- Modify: `crates/runtime/src/lib.rs`
- Create: `crates/runtime/tests/quota_runtime_contract.rs`
- Add private fake-source lifecycle tests beside
  `crates/runtime/src/quota/runtime.rs`

**RED:**

- [x] Start performs one immediate recovery refresh on the dedicated worker.
- [x] Ten thousand manual refresh requests retain at most one active plus one
  coalesced follow-up.
- [x] Successful refresh selects the 15-minute normal cadence.
- [x] Writer contention, temporary spawn/unavailable, deadline, and early-exit
  failures select the 60-second accelerated cadence; permanent failures do not.
- [x] Pause closes admission and cancelled in-flight source results cannot publish.
- [x] Resume and power-resume force exactly one coalesced recovery refresh.
- [x] Shutdown/Drop join scheduler and worker; repeated shutdown is idempotent.
- [x] Worker panic faults only quota health and leaves a separately running
  `LiveRuntime` usage snapshot unchanged.
- [x] Public snapshots contain no configured/archive path, account/window/value, or
  fixture-private text.

**GREEN:**

- [x] Compose a distinct `CodexQuotaRuntime` from the existing scheduler and worker.
- [x] Add crate-private scheduler retry-mode setters that change cadence without
  manufacturing filesystem hints or immediate retry loops.
- [x] Expose `start`, `refresh_now`, `snapshot`, `try_completion`, `pause`, `resume`,
  `apply_power_event`, and `shutdown`.
- [x] Translate the internal scheduler snapshot to quota-specific normal/accelerated
  schedule health instead of exposing watcher terminology.
- [x] Preserve path-private `Debug` and stable runtime error mapping.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test quota_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime quota::runtime --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-runtime --all-targets --locked
```

**Checkpoint commit:** `feat(runtime): add dedicated Codex quota runtime`

---

### Task 4: Add adversarial resource and privacy gates

**Files:**

- Create: `crates/runtime/tests/quota_runtime_resource_contract.rs`
- Create: `scripts/audit-codex-quota-runtime.ps1`
- Modify: `crates/runtime/Cargo.toml` only if the harness declaration is required

**RED/GREEN:**

- [x] Repeated source success, transport failure, writer contention, pause/resume, and
  shutdown return host private memory, handles, threads, USER, and GDI counts to the
  documented Windows tolerance.
- [x] No task-owned Codex/fixture process or quota worker thread remains after tests.
- [x] Source audit rejects browser/cookie/private endpoint/auth-file/shell/listener
  dependencies and raw response/path persistence.
- [x] Release dependency tree adds no browser, network, shell, async-runtime, or
  foreign-language runtime dependency.
- [x] Automatic discovery never selects the installed npm `.cmd`/`.ps1` wrappers.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test quota_runtime_resource_contract --locked
pwsh -NoProfile -File scripts\audit-codex-quota-runtime.ps1 -RepositoryRoot (Get-Location).Path
```

**Checkpoint commit:** `test(runtime): close Codex quota runtime gates`

---

### Task 5: Update project truth and run the full gate

**Files:**

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
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify:
  `docs/superpowers/specs/2026-07-16-tokenmaster-codex-quota-runtime-design.md`
- Modify:
  `docs/superpowers/plans/2026-07-16-tokenmaster-codex-quota-runtime.md`

**Actions:**

- [x] Record exact discovery trust boundary, retry classification, I/O-before-lease
  invariant, per-window transactional publication, cancellation limitation, resource
  bounds, and separate health contract.
- [x] Advance TM-FUNC-009 only to runtime publication complete; keep quota UI and
  benefits/reminders/activation incomplete.
- [x] Set the next contour to typed reset-credit benefit inventory and expiration
  reminders before activation or UI.
- [x] Record verification evidence without current commit hashes or private paths.
- [x] Inspect the complete diff and repository language/dependency surface.

**Baseline:**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

**Verification result:** every command above exited zero after the final isolation
regression was added. The focused quota resource harness also repeated its 16 warm-up
plus 48 measured rounds with bounded memory/handle/thread/USER/GDI results and no
remaining task-owned fixture child.

**Final checkpoint commit:** `docs(runtime): close Codex quota refresh contour`

## Stop conditions

Stop and report the exact blocker instead of weakening the contract if:

- the official installed Codex protocol no longer matches the pinned transport;
- exact native executable discovery cannot avoid script/shim execution;
- cancellation can publish after pause/shutdown;
- any path/account/raw provider value reaches a public error, snapshot, log, or
  tracked fixture;
- a focused resource test leaves a child, thread, handle, or monotonic memory growth;
- clean-root, strict clippy, or workspace tests fail for an in-scope reason that
  cannot be corrected without changing the approved architecture.
