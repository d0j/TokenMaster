# TokenMaster P4-A Hot Density Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the first production P4 vertical slice: instant bounded switching among comfortable, compact, and ultra-compact density in the existing `MainWindow`, with one current checked presentation revision and no data/runtime rebuild.

**Architecture:** `tokenmaster-desktop` owns one pure `DesktopPresentationStyle` value beside, but independent from, the immutable product projection. A single Slint root property selects density-derived spacing and radii from `UiTokens`; one synchronous UI-thread callback validates a fixed density index, advances the checked revision only on a real change, and leaves route selection and every bounded model untouched. This slice is deliberately runtime-only; portable persistence and the other four P4 axes receive later independent plans.

**Tech Stack:** Rust 1.97.0, Slint 1.17.1 software renderer, existing desktop testing backend, PowerShell/Pester source audits.

## Global Constraints

- Presentation axes remain independent; this slice implements only `density` and must not add placeholder behavior for skin, layout, color scheme, or locale.
- A supported density switch must not reparse input, query or write SQLite, scan a source, recreate the window, replace a product model, or lose route/selection state.
- Invalid input and checked revision overflow retain the prior visible density and revision.
- Retained production state is exactly one current density plus one `u64` revision; no history, queue, worker, timer, watcher, cache, channel, or polling loop is permitted.
- Stable density keys are exactly `comfortable`, `compact`, and `ultra_compact`; Slint indices are exactly `0`, `1`, and `2` in that order.
- Comfortable/compact/ultra-compact spacing is `16/12/8 px`; small spacing is `8/6/4 px`; extra-small spacing is `4/3/2 px`; radii are `8/6/4 px`, `5/4/3 px`, and `12/9/6 px` for normal/small/large respectively.
- The production software renderer, one-window ownership, existing bounded route/data models, privacy boundary, and no-`tokenmaster-m0` dependency remain unchanged.
- This plan does not claim persistence, skins, built-in layout families, color-scheme switching, localization, DPI/screen-reader acceptance, visible-paint latency, M0, packaging, signing, soak, or release.

---

### Task 1: Pure checked density state

**Files:**
- Create: `crates/desktop/src/presentation_style.rs`
- Modify: `crates/desktop/src/lib.rs`
- Create: `crates/desktop/tests/presentation_style_contract.rs`

**Interfaces:**
- Produces: `DesktopDensity::{Comfortable,Compact,UltraCompact}` with `stable_key()` and `slint_index()`.
- Produces: `DesktopPresentationRevision::initial()`, `get()`, and checked successor semantics.
- Produces: `DesktopPresentationStyle::new()`, `density()`, `revision()`, and `select_density_index(i32) -> DesktopPresentationApplyOutcome`.
- `DesktopPresentationApplyOutcome` is exactly `Applied`, `Unchanged`, `Rejected`, or `RevisionExhausted`.

- [ ] **Step 1: Write the failing pure contract**

```rust
#[test]
fn density_selection_is_checked_revisioned_and_constant_state() {
    let mut style = DesktopPresentationStyle::new();
    assert_eq!(style.density(), DesktopDensity::Comfortable);
    assert_eq!(style.revision().get(), 0);
    assert_eq!(style.select_density_index(1), DesktopPresentationApplyOutcome::Applied);
    assert_eq!(style.density(), DesktopDensity::Compact);
    assert_eq!(style.revision().get(), 1);
    assert_eq!(style.select_density_index(1), DesktopPresentationApplyOutcome::Unchanged);
    assert_eq!(style.select_density_index(3), DesktopPresentationApplyOutcome::Rejected);
    for index in 0..10_000 {
        let expected = match index % 3 { 0 => 0, 1 => 1, _ => 2 };
        let _ = style.select_density_index(expected);
    }
    assert!(style.revision().get() <= 10_001);
}
```

- [ ] **Step 2: Run RED**

Run:

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4a'
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_style_contract --locked
```

Expected: compilation fails because `presentation_style` and its public types do not exist.

- [ ] **Step 3: Implement the minimal pure state**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopDensity { Comfortable, Compact, UltraCompact }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopPresentationRevision(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopPresentationApplyOutcome { Applied, Unchanged, Rejected, RevisionExhausted }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopPresentationStyle {
    density: DesktopDensity,
    revision: DesktopPresentationRevision,
}
```

`select_density_index` must validate before mutation, return `Unchanged` without incrementing, use `checked_add(1)`, and assign density/revision together only after the successor exists.

- [ ] **Step 4: Run GREEN and strict focused Clippy**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4a'
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_style_contract --locked
$env:RUSTFLAGS='-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-desktop --all-targets --locked
```

Expected: pure contract passes; Clippy exits zero.

- [ ] **Step 5: Commit**

```powershell
git add crates/desktop/src/presentation_style.rs crates/desktop/src/lib.rs crates/desktop/tests/presentation_style_contract.rs
git commit -m "feat(ui): add checked presentation density state"
```

### Task 2: Same-window Slint hot switch

**Files:**
- Modify: `crates/desktop/ui/tokens.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/settings-view.slint`
- Modify: `crates/desktop/src/ui.rs`
- Create: `crates/desktop/tests/presentation_density_ui_contract.rs`

**Interfaces:**
- Consumes: `DesktopPresentationStyle::select_density_index` from Task 1.
- Produces Slint root properties: `presentation-density-id`, `presentation-density-key`, `presentation-revision`, `presentation-space`, and `presentation-radius`.
- Produces Slint callback: `select-presentation-density(int)`.
- `DesktopShell` retains one `Rc<RefCell<DesktopPresentationStyle>>`; it does not add an app/backend intent.

- [ ] **Step 1: Write the failing compiled Slint contract**

```rust
#[test]
fn density_hot_switch_keeps_the_same_window_route_and_models() {
    let shell = test_shell();
    let window = shell.window();
    window.invoke_select_route("settings".into());
    let routes = window.get_route_rows().row_count();
    let quotas = window.get_dashboard_quota_rows().row_count();
    assert_eq!(window.get_presentation_density_key(), "comfortable");
    window.invoke_select_presentation_density(1);
    assert_eq!(window.get_presentation_density_key(), "compact");
    assert_eq!(window.get_active_route_key(), "settings");
    assert_eq!(window.get_route_rows().row_count(), routes);
    assert_eq!(window.get_dashboard_quota_rows().row_count(), quotas);
    let revision = window.get_presentation_revision();
    window.invoke_select_presentation_density(1);
    assert_eq!(window.get_presentation_revision(), revision);
    window.invoke_select_presentation_density(9);
    assert_eq!(window.get_presentation_density_key(), "compact");
}
```

The final test must also perform 10,000 valid switches and prove route/model counts and the sole window remain unchanged.

- [ ] **Step 2: Run RED**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4a'
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_density_ui_contract --locked
```

Expected: Slint/Rust compilation fails because the properties and callback do not exist.

- [ ] **Step 3: Add density-derived tokens**

In `UiTokens`, add one bounded `in-out property <int> density-id: 0` and derive the spacing/radius tokens with fixed ternaries. Do not change colors, fonts, animations, renderer selection, or create a second token object.

```slint
in-out property <int> density-id: 0;
out property <length> space-xs: density-id == 2 ? 2px : (density-id == 1 ? 3px : 4px);
out property <length> space-sm: density-id == 2 ? 4px : (density-id == 1 ? 6px : 8px);
out property <length> space: density-id == 2 ? 8px : (density-id == 1 ? 12px : 16px);
```

- [ ] **Step 4: Add the accessible fixed selector and one-root binding**

`SettingsView` receives one density index and forwards a `ComboBox` selection through `select-presentation-density(int)`. `MainWindow` owns the callback and root properties, and one `presentation-density-id` change updates `UiTokens.density-id`; the Settings view does not own another style state.

- [ ] **Step 5: Wire the checked Rust owner**

Create one `Rc<RefCell<DesktopPresentationStyle>>` during `DesktopShell` construction, apply its initial value once, and wire the Slint callback. Borrow only for validation/state transition, release the borrow, then set one root density property and the revision label. Invalid/equal/exhausted inputs must not call setters.

- [ ] **Step 6: Run GREEN and existing compiled UI regression**

```powershell
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4a'
cargo +1.97.0 test -p tokenmaster-desktop --test presentation_density_ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
```

Expected: new density contract and existing compiled UI contract pass.

- [ ] **Step 7: Commit**

```powershell
git add crates/desktop/ui/tokens.slint crates/desktop/ui/main.slint crates/desktop/ui/views/settings-view.slint crates/desktop/src/ui.rs crates/desktop/tests/presentation_density_ui_contract.rs
git commit -m "feat(ui): hot switch production density"
```

### Task 3: Authority audit, documentation, and closure evidence

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `spec/TRACEABILITY.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`

**Interfaces:**
- Consumes Task 1/2 source facts.
- Produces a deterministic source/mutation audit proving fixed three-value density, one owner, 10,000-cycle coverage, and absence of forbidden authority.

- [ ] **Step 1: Write failing source and mutation checks**

Add exact audit checks for all three stable keys/indices, one root density property, one callback, checked revision update, and the 10,000-switch contract. Add mutations that remove one key, widen the index, remove `checked_add`, or introduce a timer/worker/query/window creation in the new module; each mutation must fail the audit.

- [ ] **Step 2: Run RED**

```powershell
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
Invoke-Pester -Path scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed
```

Expected: the new checks fail before their production markers/contract are complete.

- [ ] **Step 3: Complete the audit and update project truth**

Record P4-A density as developer-complete while explicitly retaining skins/layouts/schemes/locales, persistence, accessibility/DPI/paint, P5/P6, M0, package/signing/soak, and release as open. Do not change a parity row to `implemented` unless its full terminal requirement is satisfied.

- [ ] **Step 4: Run focused and baseline gates**

```powershell
pwsh -NoProfile -File scripts/audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'
$env:CARGO_TARGET_DIR='C:\code\.tokenmaster-target\p4a'
cargo +1.97.0 clippy -p tokenmaster-desktop --all-targets --locked
cargo +1.97.0 test -p tokenmaster-desktop --locked
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path -SourceOnly
Invoke-Pester -Path scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed
```

Expected: every command exits zero with no warning accepted as a substitute for a gate.

- [ ] **Step 5: Commit**

```powershell
git add scripts/audit-desktop-shell.ps1 scripts/tests/audit-desktop-shell.Tests.ps1 spec/TRACEABILITY.md docs/FEATURE_PARITY.md docs/CURRENT_STATE.md docs/HANDOFF.md docs/ROADMAP.md docs/CHANGELOG.md docs/AUDIT_AND_MASTER_PLAN.md
git commit -m "docs: record hot density verification"
```

### Task 4: Independent review and build-artifact cleanup

**Files:**
- No product file changes unless the reviewer finds a Critical or Important defect.
- Delete only: `C:\code\.tokenmaster-target\p4a` after all required evidence is recorded.

- [ ] **Step 1: Request independent read-only review**

Review against this plan and the five-axis architecture. Require separate Critical/Important/Minor counts, spec-compliance verdict, code-quality verdict, and explicit confirmation that the slice adds no persistence or authority claim.

- [ ] **Step 2: Resolve findings and rerun affected gates**

Dispatch one bounded fixer for all Critical/Important findings, require focused test output, and repeat review until no blocking finding remains.

- [ ] **Step 3: Remove task-owned build artifacts safely**

Resolve `C:\code\.tokenmaster-target\p4a`, verify it is under `C:\code\.tokenmaster-target`, then remove only that directory. Confirm the repository worktree remains clean apart from intended commits and no Cargo/Rust/TokenMaster task process remains.

- [ ] **Step 4: Record the next critical slice**

The next plan is P4-B portable presentation settings and exact v1-to-v2 migration, followed by the remaining skin/layout/scheme/locale vertical slices. Do not start another backend foundation.
