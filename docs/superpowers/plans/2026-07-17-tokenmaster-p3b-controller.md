# TokenMaster P3-B.1 Bounded Desktop Controller Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` and complete
> each checkbox in order. Use `superpowers:test-driven-development` for behavior
> changes and `superpowers:verification-before-completion` before completion claims.

**Goal:** Add one bounded production desktop query controller that coalesces refresh
intents, reduces typed query results into one product generation, retains only the
latest immutable snapshot, and shuts down deterministically.

**Architecture:** Add direct desktop dependencies on the existing query and engine
contracts. Keep one `QueryService` and `ProductReducer` inside one reused
`RefreshWorker`; replace a capacity-one snapshot slot only after a complete attempt.
Keep Slint event-loop delivery and production data-root/runtime composition outside
this slice.

**Tech stack:** Rust 1.97, Slint 1.17, bundled SQLite through `tokenmaster-query`,
`tokenmaster-engine::RefreshWorker`, existing `tokenmaster-product` reducer, Cargo
workspace, PowerShell source audit.

---

### Task 1: Freeze the controller surface with failing tests

**Files:**
- Create: `crates/desktop/tests/controller_contract.rs`
- Modify: `crates/desktop/Cargo.toml`

- [x] **Step 1: Write the public contract test**

Require typed `DesktopQueryPlan`, refresh urgency/admission/completion values,
`DesktopController::spawn`, `refresh`, `take_snapshot`, and `shutdown`. The test must
use a controllable typed source, not raw SQL or a filesystem mock.

- [x] **Step 2: Observe the intended compile failure**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test controller_contract --locked
```

Expected: FAIL because the controller module and API do not exist.

### Task 2: Implement typed plan and source adapter

**Files:**
- Create: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/Cargo.toml`

- [x] **Step 1: Add bounded plan tests**

Require overview requests to stay within 240 chart points, 256 activity/session rows,
and 32 repositories. Benefit scope must be explicit and optional. No plan field may
accept arbitrary SQL, provider text, command, URL, or output path.

- [x] **Step 2: Add `DesktopQuerySource`**

Define only the seven typed read methods needed by one refresh. Implement it for
`QueryService<C>` where the query clock is safe to move to the worker. Map failures by
stable `QueryErrorCode`, never formatted raw errors.

- [x] **Step 3: Prove the focused contract**

Run the focused controller test. Expected: it advances to worker-related failures,
with plan/source tests passing.

### Task 3: Implement one worker, reducer, and latest slot

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/tests/controller_contract.rs`

- [x] **Step 1: Add red tests for complete publication**

Require data status first, sibling publication through `ProductReducer`, one attempt
to one product generation, and exactly one final snapshot. A section-local query
failure must not suppress successful sibling sections.

- [x] **Step 2: Add red tests for bounded admission**

Block the first source call, submit many refresh hints, release it, and require at
most one coalesced follow-up. Publish multiple results without consuming them and
require `take_snapshot` to return only the newest one.

- [x] **Step 3: Implement the minimum controller**

Reuse `RefreshWorker`; keep the source and reducer in its closure; use permit IDs as
product attempt generations; check cancellation/deadline between reads; replace one
`Option<Arc<ProductSnapshot>>` under a short lock only after all reads finish.

- [x] **Step 4: Run focused tests**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test controller_contract --locked
```

Expected: PASS.

### Task 4: Prove cancellation, redaction, and shutdown

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/tests/controller_contract.rs`

- [x] **Step 1: Add red lifecycle tests**

Require cancellation/deadline before completion to leave the prior latest snapshot
unchanged, explicit shutdown to join the worker, and post-shutdown admission to fail
with a stable controller error.

- [x] **Step 2: Add red privacy tests**

Open a deliberately invalid path containing a unique marker and require the public
error/display text to omit that marker and all wrapped raw SQLite/OS text.

- [x] **Step 3: Implement stable lifecycle/error mapping**

Expose bounded enums and stable ASCII codes. Do not expose engine worker errors,
panic payloads, raw `QueryError`, or paths through the public desktop surface.

- [x] **Step 4: Run focused package tests**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --locked
```

Expected: PASS.

### Task 5: Update and prove the source boundary

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`

- [x] **Step 1: Add failing audit fixtures**

Permit direct desktop dependencies only on `anyhow`, `slint`, `tokenmaster-product`,
`tokenmaster-query`, and `tokenmaster-engine`. Continue rejecting M0, runtime, store,
provider, SQLite, HTTP, browser, shell, arbitrary SQL, and seeded/mock production
surfaces.

- [x] **Step 2: Update the deterministic audit**

Require exactly one controller worker construction, one capacity-one latest slot,
and no controller query call from Slint callbacks.

- [x] **Step 3: Run focused audit gates**

```powershell
Invoke-Pester -Path scripts\tests\audit-desktop-shell.Tests.ps1 -Output Detailed
pwsh -NoProfile -File scripts\audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
```

Expected: PASS.

### Task 6: Integrate a real query service

**Files:**
- Modify: `crates/desktop/tests/controller_contract.rs`
- Modify: `crates/desktop/Cargo.toml`

- [x] **Step 1: Add a real empty-archive test**

Create a temporary schema-v13 archive through the existing store test API, open it
through `QueryService`, refresh once, and require truthful data-status, analytics,
quota, Git, activity, and sessions route inputs. Do not seed usage or identity data.

- [x] **Step 2: Prove package build and tests**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --locked
cargo +1.97.0 build -p tokenmaster-desktop --locked
```

Expected: PASS.

### Task 7: Synchronize project truth

**Files:**
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`

- [x] **Step 1: Record P3-B.1 honestly**

Record the controller ownership, bounds, tests, and exact remaining P3-B.2/P3-B.3
work. Preserve benefit-scope and data-root decisions as open bounded contracts. Do
not claim a live-wired GUI, complete P3, resource soak, package, signing, or release.

- [x] **Step 2: Check documentation consistency**

```powershell
rg -n "P3-B|DesktopController|data-root|benefit scope" spec docs
git diff --check
```

Expected: controller truth and remaining boundaries are consistent; diff passes.

### Task 8: Run the complete quality gate and checkpoint

**Files:** all files above.

- [x] **Step 1: Run the baseline quality gate**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

- [x] **Step 2: Inspect repository and process cleanliness**

Confirm only intentional files changed and no task-owned test, GUI, diagnostic, or
temporary server process remains.
