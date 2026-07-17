# TokenMaster P3-A Production Desktop Shell Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` and complete
> each checkbox in order. Use `superpowers:test-driven-development` for every behavior
> change and `superpowers:verification-before-completion` before any completion claim.

**Goal:** Add the first production TokenMaster desktop contour: an immutable bounded
`ProductSnapshot` projection rendered by a software-only Slint shell with all 11
truthful product routes and no mock data.

**Architecture:** Preserve `tokenmaster-m0` as a separate evidence package. Add a new
frontend leaf `tokenmaster-desktop`; map one current `Arc<ProductSnapshot>` into one
owned fixed route projection, then atomically replace one 11-row Slint model. Slint
callbacks may select a known route but own no database, provider, runtime, filesystem,
process, or network authority.

**Tech stack:** Rust 1.97, Slint 1.17, software renderer, existing
`tokenmaster-product` contract, Cargo workspace, PowerShell source audit.

---

### Task 1: Freeze package and renderer boundaries

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/probe-app/Cargo.toml`
- Create: `crates/desktop/Cargo.toml`
- Create: `crates/desktop/build.rs`
- Create: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Record the current feature failure**

Run:

```powershell
cargo +1.97.0 tree -p tokenmaster-m0 -e features | rg "renderer-femtovg"
```

Expected: the shared workspace Slint dependency currently enables FemtoVG globally.

- [ ] **Step 2: Add the production package skeleton**

Add `crates/desktop` to the root workspace. Define package `tokenmaster-desktop`,
binary `TokenMaster`, direct normal dependencies only on `anyhow`, `slint`, and
`tokenmaster-product`, and `slint-build` as its build dependency. Keep generated Slint
code inside the desktop package.

- [ ] **Step 3: Split renderer features**

Remove `renderer-femtovg` from the workspace Slint feature list. Add it explicitly to
`tokenmaster-m0` while the new desktop uses only the workspace software feature.

- [ ] **Step 4: Verify dependency intent**

Run:

```powershell
cargo +1.97.0 tree -p tokenmaster-desktop -e features | rg "renderer-(software|femtovg)"
cargo +1.97.0 tree -p tokenmaster-desktop -e normal | rg "tokenmaster-m0"
```

Expected: software is present, FemtoVG and `tokenmaster-m0` are absent. The second
command exits 1 because it finds no match.

### Task 2: Build the fixed product-to-desktop projection

**Files:**
- Create: `crates/desktop/src/presentation.rs`
- Create: `crates/desktop/tests/presentation_contract.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/Cargo.toml`

- [ ] **Step 1: Write failing projection contracts**

Tests must require:

- exactly 11 rows in `ProductRoute::ALL` order;
- stable route keys and label keys;
- initial Settings/Help ready and all data routes truthful;
- exact product route state and canonical reason-code mapping;
- a fixed maximum of 11 reasons without a dynamic retained diagnostic collection;
- unknown selection rejection with prior selection preserved.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_contract --locked
```

Expected: FAIL because the projection API is absent.

- [ ] **Step 2: Implement the minimum fixed projection**

Implement `DesktopRouteKey`, `DesktopRouteState`, `DesktopRouteProjection`,
`DesktopProjection`, and `DesktopSelectionError`. Use fixed arrays, stable ASCII
codes, and exhaustive matches over public product enums. Do not clone query payloads.

- [ ] **Step 3: Prove the projection**

Run the focused test again. Expected: PASS.

### Task 3: Enforce UI generation ordering

**Files:**
- Modify: `crates/desktop/src/presentation.rs`
- Modify: `crates/desktop/tests/presentation_contract.rs`

- [ ] **Step 1: Write failing update-order tests**

Require one state owner to accept a strictly newer `ProductGeneration`, ignore equal
or older candidates, retain selection across accepted updates, and retain only the
current projection.

- [ ] **Step 2: Run the focused test and observe failure**

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_contract --locked
```

- [ ] **Step 3: Implement `DesktopState` and `DesktopApplyOutcome`**

Build the next projection before replacement. Do not retain snapshot or projection
history.

- [ ] **Step 4: Run focused tests**

Expected: PASS.

### Task 4: Compile and wire the Slint production shell

**Files:**
- Create: `crates/desktop/ui/models.slint`
- Create: `crates/desktop/ui/tokens.slint`
- Create: `crates/desktop/ui/components/route-nav-item.slint`
- Create: `crates/desktop/ui/components/route-state.slint`
- Create: `crates/desktop/ui/main.slint`
- Create: `crates/desktop/src/ui.rs`
- Create: `crates/desktop/tests/ui_contract.rs`
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Write failing compiled-UI tests**

Tests must instantiate one `MainWindow`, apply the initial real product snapshot,
assert 11 route rows, select Settings through the callback without recreating the
window, reject an unknown route, and observe the selected route/state update.

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
```

Expected: FAIL because the generated component and adapter do not exist.

- [ ] **Step 2: Add the minimal semantic shell**

Create an original TokenMaster header, fixed left navigation, and route-state panel.
Use stable semantic tokens. Show product generation, route state, and bounded reason
codes. Do not add seeded quota/session/chart values or copy probe layouts/assets.

- [ ] **Step 3: Wire one state owner**

`DesktopShell` owns the component plus one `DesktopState`. Its callback validates the
route key, updates selection, and replaces one bounded model. It exposes a snapshot
apply method for P3-B. No callback blocks or accesses external authority.

- [ ] **Step 4: Run focused UI tests**

Expected: PASS.

### Task 5: Add the production executable and source audit

**Files:**
- Create: `crates/desktop/src/main.rs`
- Create: `crates/desktop/src/shell.rs`
- Create: `scripts/audit-desktop-shell.ps1`
- Create: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Write the failing audit contract**

The Pester test must prove that the audit rejects a fixture containing a probe
dependency, mock/seed production helper, FemtoVG production feature, fewer/more than
11 routes, direct SQLite/store/provider authority, or forbidden HTTP/browser/shell
surface.

- [ ] **Step 2: Implement the software-only entry point**

Select `winit-software`, create `ProductReducer::new().snapshot()`, construct one
`DesktopShell`, show it, and run the Slint event loop. No renderer override or
diagnostic fallback is accepted in the production binary.

- [ ] **Step 3: Implement the deterministic source audit**

Audit the root manifest, desktop manifest/source/UI, dependency tree, and compiled
binary strings where applicable. Report bounded counts and stable pass/fail output.

- [ ] **Step 4: Run focused gates**

```powershell
Invoke-Pester -Path scripts\tests\audit-desktop-shell.Tests.ps1 -Output Detailed
pwsh -NoProfile -File scripts\audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 test -p tokenmaster-desktop --locked
cargo +1.97.0 build -p tokenmaster-desktop --locked
```

Expected: PASS.

### Task 6: Synchronize normative truth

**Files:**
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/DECISIONS.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`

- [ ] **Step 1: Correct reader-version drift**

Change only the two stale exact-reader references from schema v12 to the implemented
schema v13. Preserve legitimate schema-v12 benefit-foundation history.

- [ ] **Step 2: Record P3-A honestly**

Record the new package boundary, fixed projection, software-only shell, tests/audit,
and P3-B as next. Do not claim complete P3, P4 presentation, M0 acceptance, package,
signing, or release.

- [ ] **Step 3: Check documentation consistency**

```powershell
rg -n "requires exact schema v12" spec\DATA_CONTRACT.md spec\DECISIONS.md
rg -n "P3-A|tokenmaster-desktop|P3-B" spec docs
git diff --check
```

Expected: no stale exact-reader match; P3-A truth is traceable; diff check passes.

### Task 7: Run the complete quality gate and checkpoint

**Files:** all files above.

- [ ] **Step 1: Run the baseline quality gate**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

- [ ] **Step 2: Inspect repository and process cleanliness**

Confirm only intentional files changed and no task-owned test, GUI, diagnostic, or
temporary server process remains.

- [ ] **Step 3: Commit intentional checkpoints**

Use concise English commits that preserve review history. Do not push, package, sign,
or make release claims without separate authority.
